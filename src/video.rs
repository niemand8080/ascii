extern crate ffmpeg_next as ffmpeg;

use crate::Pixels;
use crate::wait_for_terminal_scale;

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use imageproc::image::EncodableLayout;

use ffmpeg::format::{Pixel, Sample as FFmpegSample, context::Input, sample::Type as SampleType};
use ffmpeg::media::Type as MediaType;
use ffmpeg::software::scaling::context::Context;
use ffmpeg::util::frame::{self, Audio, Video};

use ringbuf::RingBuffer;

use std::fs::{self, File};
use std::io::Read;
use std::time::SystemTime;

trait SampleFormatConversion {
    fn as_ffmpeg_sample(&self) -> FFmpegSample;
}

impl SampleFormatConversion for SampleFormat {
    fn as_ffmpeg_sample(&self) -> FFmpegSample {
        match self {
            Self::I16 => FFmpegSample::I16(SampleType::Packed),
            Self::F32 => FFmpegSample::F32(SampleType::Packed),
            f => {
                panic!("ffmpeg resampler doesn't support {f}")
            }
        }
    }
}

fn write_audio(
    data: &mut [f32],
    samples: &mut ringbuf::Consumer<f32>,
    _cbinfo: &cpal::OutputCallbackInfo,
) {
    for d in data {
        match samples.pop() {
            Some(sample) => *d = sample,
            None => *d = 0.0,
        }
    }
}

pub fn packed<T: frame::audio::Sample>(frame: &frame::Audio) -> &[T] {
    if !frame.is_packed() {
        panic!("data is not packed");
    }

    if !<T as frame::audio::Sample>::is_valid(frame.format(), frame.channels()) {
        panic!("unsupported type");
    }

    unsafe {
        std::slice::from_raw_parts(
            (*frame.as_ptr()).data[0] as *const T,
            frame.samples() * frame.channels() as usize,
        )
    }
}

fn get_audio(
    ictx: &mut Input,
    mut consumer: ringbuf::Consumer<f32>,
) -> (
    Option<ffmpeg::codec::decoder::Audio>,
    Option<ffmpeg::software::resampling::Context>,
    Option<cpal::Stream>,
    Option<usize>,
) {
    match ictx.streams().best(MediaType::Audio) {
        Some(audio) => {
            let host = cpal::default_host();

            let device = host
                .default_output_device()
                .expect("No output device available");

            let supported_configs = device
                .supported_output_configs()
                .expect("Device disconected")
                .next()
                .expect("No supported configs");
            let audio_config = supported_configs.with_max_sample_rate();

            // create audio decoder
            let audio_decoder =
                ffmpeg::codec::context::Context::from_parameters(audio.parameters())
                    .expect("Couldn't construct audio decoder context")
                    .decoder()
                    .audio()
                    .expect("Couldn't get audio decoder");

            // setup audio resampler
            let resampler = ffmpeg::software::resampling::Context::get(
                // in
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.rate(),
                // out
                audio_config.sample_format().as_ffmpeg_sample(),
                audio_decoder.channel_layout(),
                audio_config.sample_rate().0,
            )
            .expect("Couldn't get resampling context");

            let audio_stream = match audio_config.sample_format() {
                SampleFormat::F32 => device.build_output_stream(
                    &audio_config.into(),
                    move |data: &mut [f32], cbinfo| write_audio(data, &mut consumer, &cbinfo),
                    |err| {
                        eprintln!("{err}");
                    },
                    None,
                ),
                fm => panic!("{fm} is not implemented"),
            }
            .expect("Failed to build output stream");
            (
                Some(audio_decoder),
                Some(resampler),
                Some(audio_stream),
                Some(audio.index()),
            )
        }
        None => (None, None, None, None),
    }
}

fn get_video(
    ictx: &mut Input,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: Option<f64>,
) -> (ffmpeg::codec::decoder::Video, Context, usize) {
    let video_input = ictx
        .streams()
        .best(MediaType::Video)
        .expect("No video stream found");
    let video_stream_index = video_input.index();

    let context_decoder =
        ffmpeg::codec::context::Context::from_parameters(video_input.parameters())
            .expect("Couldn't construct deocder context");
    // create video decoder
    let decoder = context_decoder
        .decoder()
        .video()
        .expect("Couldn't find decoder");

    let factor = if let Some(max_width) = max_width {
        match decoder.width() as f64 {
            w if w <= max_width => 1.0,
            w => max_width / w,
        }
    } else {
        1.0
    };

    // create scaler
    let dst_width = if factor < 1.0 {
        (decoder.width() as f64 * factor) as u32
    } else {
        decoder.width()
    };
    let dst_height = if factor < 1.0 {
        (decoder.height() as f64 * factor) as u32
    } else {
        decoder.height()
    };
    let scaler = Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        dst_width,
        dst_height,
        scale_algorithm,
    )
    .expect("Failed to get context");

    (decoder, scaler, video_stream_index)
}

pub fn draw(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: f64,
) {
    play(
        path,
        scale_algorithm,
        Some(max_width),
        false,
        true,
        |p, _, _| {
            let height = p.len();
            crate::draw(p);
            print!("\x1b[{height}A");
        },
    );
}

/// `max_width` is the `max_width` of the video that get's converted to images that then get
/// converted back to a video...
pub fn draw_to_file(
    src: &str,
    dst: &str,
    font: &ab_glyph::FontRef<'_>,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: Option<f64>,
) {
    let id = rand::random::<u32>();
    let mut counter = 0;

    let root = "tmp";

    fs::create_dir_all(root).unwrap();

    let tmp_video = format!("{root}/{id}.video.mp4");

    let mut mp4muxer = minimp4::Mp4Muxer::new(fs::File::create(&tmp_video).unwrap());

    // todo: make get the correct size.
    mp4muxer.init_video(1280, 720, false, dst);

    let mut frame = Vec::new();
    let started = SystemTime::now();

    let mref = &mut mp4muxer;
    let moved_frame = &mut frame;
    play(
        src,
        scale_algorithm,
        max_width,
        true,
        false,
        move |pixels, frame_rate, duration_micros| {
            if counter % 10 == 0 {
                *moved_frame = crate::downscale_pixels(&pixels, 50).unwrap_or(pixels.clone());

                let frames = frame_rate * (duration_micros / 1_000_000) as u32;
                let decimal = counter as f64 / frames as f64;

                let f = |pixels: &Pixels, dec_percentage: f64| {
                    let width = pixels[0].len();
                    let max_count = ((pixels.len() * width) as f64 * dec_percentage) as usize;
                    let mut count: usize = 0;
                    println!("╭{:─<1$}╮", "", width * 2);
                    for row in pixels {
                        // ╭───╮
                        // │   │
                        // ╰───╯
                        let mut left = row.len();
                        print!("\x1b[0m│");
                        for (r, g, b) in row {
                            if count >= max_count {
                                break;
                            }
                            let l = crate::get_lightness(*r, *g, *b);
                            let s = crate::symbol(l);
                            print!("\x1b[38;2;{r};{g};{b}m{s}{s}");
                            count += 1;
                            left -= 1;
                        }
                        print!("{: <w$}\x1b[0m│", "", w = left * 2);
                        println!();
                    }
                    let info = format!(
                        "\x1b[1;32m{}% {:>10}s",
                        (dec_percentage * 100.0).round(),
                        SystemTime::now().duration_since(started).unwrap().as_secs()
                    );
                    println!("\x1b[0m│{: ^w$}\x1b[0m│", info, w = width * 2 + 7);
                    println!("╰{:─<w$}╯", "", w = width * 2);
                    print!("\x1b[{}A", moved_frame.len() + 3);
                };

                f(&moved_frame, decimal);
            }

            let target = format!("{root}/{id}.frame.{counter}.jpg");
            // println!("{counter}: draw to file");
            crate::image::draw_to_file(&target, font, &pixels);

            // println!("{counter}: get rgb pixels");
            let (pixels, width, height) = crate::image::get_pixels(&target, None);

            let mut encoder = openh264::encoder::Encoder::new().expect("Couldn't create encoder");

            // println!("{counter}: convert rgb to yuv");
            let rgb_source =
                openh264::formats::RgbSliceU8::new(&pixels, (width as usize, height as usize));
            let yuv = openh264::formats::YUVBuffer::from_rgb_source(rgb_source);

            let bitstream = encoder.encode(&yuv).unwrap();

            let mut buf = Vec::new();
            bitstream.write_vec(&mut buf);

            // println!("{counter}: write to video (buf len: {})", buf.len());
            mref.write_video_with_fps(&buf, frame_rate);
            // println!("{counter}: remove file");
            fs::remove_file(target).expect("Couldn't remove file");
            // crate::draw(pixels);
            counter += 1;
            // print!("\x1b[5A");
        },
    );
    mp4muxer.close();

    println!("\x1b[{}BConvert file, so its smaler", frame.len() + 4);
    crate::convert::convert(src, &tmp_video, dst);
    println!("Remove tmp video file: {tmp_video}");
    if let Err(err) = fs::remove_file(&tmp_video) {
        eprintln!("{err}");
    }
}

fn play<F>(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: Option<f64>,
    disable_audio: bool,
    fit_termianl: bool,
    mut f: F,
) where
    F: FnMut(Pixels, u32, i64),
{
    // new input ctx
    let mut ictx = ffmpeg::format::input(path).expect("Couldn't open file");
    let duration_micros = ictx.duration();

    // create buffer to store audio data
    let buffer = RingBuffer::<f32>::new(2usize.pow(13));
    let (mut producer, consumer) = buffer.split();

    // get best audio stream index AND creat audio decoder AND create resampler
    let (mut audio_decoder, mut resampler, mut audio_stream, audio_stream_index) = if disable_audio
    {
        (None, None, None, None)
    } else {
        get_audio(&mut ictx, consumer)
    };

    // contruct video decoder AND scaler AND get best video stream index
    let (mut video_decoder, mut scaler, video_stream_index) =
        get_video(&mut ictx, scale_algorithm, max_width);

    if fit_termianl {
        wait_for_terminal_scale(scaler.output().width * 2, scaler.output().height + 2);
    }

    let mut process_audio_frames = |decoder: &mut ffmpeg::decoder::Audio| {
        let mut decoded = Audio::empty();
        if let Some(resampler) = &mut resampler {
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut resampled = Audio::empty();
                resampler
                    .run(&decoded, &mut resampled)
                    .expect("Input or output changed");

                // There maybe more then one audio stream
                let both_channels = packed(&resampled);

                while producer.remaining() < both_channels.len() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }

                producer.push_slice(both_channels);
            }
        }
    };

    let mut process_frames = |decoder: &mut ffmpeg::decoder::Video| {
        let mut decoded = Video::empty();
        let fps = match decoder.frame_rate() {
            Some(fr) => fr.numerator(),
            None => 24,
        };
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb_frame = Video::empty();
            scaler
                .run(&decoded, &mut rgb_frame)
                .expect("Input or output changed");
            let pixels = rgb_frame.data(0);
            let rows = crate::format_pixels(pixels, rgb_frame.width() as u16);
            f(rows, fps as u32, duration_micros);
        }
    };

    if let Some(audio_stream) = &mut audio_stream {
        audio_stream.play().unwrap();
    }

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            video_decoder
                .send_packet(&packet)
                .expect("Failed to send video packet");
            process_frames(&mut video_decoder);
        }
        if let Some(audio_stream_index) = audio_stream_index {
            if let Some(audio_decoder) = &mut audio_decoder {
                if stream.index() == audio_stream_index {
                    audio_decoder
                        .send_packet(&packet)
                        .expect("Failed to send audio packet");
                    process_audio_frames(audio_decoder);
                }
            }
        }
    }
    video_decoder
        .send_eof()
        .expect("Failed to send eof (end of file)");
    process_frames(&mut video_decoder);
}
