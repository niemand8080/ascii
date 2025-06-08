extern crate ffmpeg_next as ffmpeg;

use crate::Pixels;
use crate::wait_for_terminal_scale;

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use ffmpeg::format::{Pixel, Sample as FFmpegSample, context::Input, sample::Type as SampleType};
use ffmpeg::media::Type as MediaType;
use ffmpeg::software::scaling::context::Context;
use ffmpeg::util::frame::{self, Audio, Video};

use ringbuf::RingBuffer;

use std::collections::HashMap;
use std::fs::{self};
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
        Pixel::RGB24,
        |frame, _, _| {
            let pixels = frame.data(0);
            let pixels = crate::format_pixels(pixels, frame.width() as u16);
            let height = pixels.len();
            crate::draw(pixels);
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
    let title = src.split("/").last().unwrap_or(&src);

    let root = "tmp";

    fs::create_dir_all(root).unwrap();

    let tmp_video = format!("{root}/{id}.video.mp4");

    let mut mp4muxer = minimp4::Mp4Muxer::new(fs::File::create(&tmp_video).unwrap());

    // todo: make get the correct size.
    mp4muxer.init_video(1280, 720, false, dst);

    let mut loading_frame = Vec::new();
    let started = SystemTime::now();
    let mut times: HashMap<&str, Vec<u128>> = HashMap::new();

    let mref = &mut mp4muxer;
    let moved_lframe = &mut loading_frame;
    let mut add_time = |label: &'static str, ns: u128| {
        if let Some(t) = &mut times.get_mut(label) {
            t.push(ns);
        } else {
            let _ = &mut times.insert(label, Vec::from([ns]));
        }
    };
    play(
        src,
        scale_algorithm,
        max_width,
        true,
        false,
        Pixel::RGB24,
        move |frame, frame_rate, duration_micros| {
            let pixels = crate::format_pixels(frame.data(0), frame.width() as u16);

            if counter % 10 == 0 {
                let s = SystemTime::now();
                let (w, h) = term_size::dimensions().unwrap_or((50, 0));
                let w = 52.min(w) - 2;
                let h = 28.min(h);
                let h = if h == 0 && h > 6 { None } else { Some(h - 5) };
                *moved_lframe = crate::downscale_pixels(&pixels, w, h).unwrap_or(pixels.clone());

                let frames = frame_rate * (duration_micros / 1_000_000) as f32;
                let decimal = counter as f32 / frames;

                let frame_color = match decimal {
                    d if d < 0.5 => 31,
                    d if d < 1.0 => 33,
                    _ => 32,
                };
                let fc = format!("\x1b[{frame_color}m");

                let f = |pixels: &Pixels, dec_percentage: f32| {
                    let width = pixels[0].len();
                    let max_count = ((pixels.len() * width) as f32 * dec_percentage) as usize;
                    let mut count: usize = 0;
                    println!(
                        "\x1b[2K{fc}╭{:─^1$}╮",
                        format!(" \x1b[1;{frame_color}m{title}{fc} "),
                        width * 2 + 12
                    );
                    for row in pixels {
                        // ╭───╮
                        // ├───┤
                        // ╰───╯
                        let mut left = row.len();
                        print!("\x1b[2K{fc}│");
                        for (r, g, b) in row {
                            if count >= max_count {
                                break;
                            }
                            let l = crate::get_lightness(*r, *g, *b);
                            let s = crate::symbol(l);
                            let s = if s == ' ' { '.' } else { s };
                            print!("\x1b[38;2;{r};{g};{b}m{s}{s}");
                            count += 1;
                            left -= 1;
                        }
                        print!("{: <w$}{fc}│", "", w = left * 2);
                        println!();
                    }
                    let secs_since = SystemTime::now().duration_since(started).unwrap().as_secs();
                    let secs_since = if secs_since == 0 { 1 } else { secs_since };
                    let fps = if counter == 0 {
                        1.0 / secs_since as f32
                    } else {
                        counter as f32 / secs_since as f32
                    };
                    let fps = format!("{fps:.1}");
                    let info = if dec_percentage < 1.0 {
                        format!(
                            "\x1b[1;32m{}% {:>10}s {:>10} fps",
                            (dec_percentage * 100.0).round(),
                            secs_since,
                            fps,
                        )
                    } else {
                        format!("\x1b[1;32mDone! (in {}s, fps: {fps})", secs_since)
                    };
                    let info_title = format!(" \x1b[1;{frame_color}minfo{fc} ");
                    println!("\x1b[2K{fc}├{:─^w$}┤", info_title, w = width * 2 + 12);
                    println!("\x1b[2K{fc}│{: ^w$}{fc}│", info, w = width * 2 + 7);
                    println!("\x1b[2K{fc}╰{:─<w$}{fc}╯", "", w = width * 2);
                    print!("\x1b[{}A\x1b[0m", moved_lframe.len() + 4);
                };

                f(&moved_lframe, decimal);
                add_time(
                    "loading",
                    SystemTime::now().duration_since(s).unwrap().as_nanos(),
                );
            }

            let s = SystemTime::now();
            // get frame rgb
            let tmp_img = crate::image::get_image_buf(font, &pixels);
            let height = tmp_img.height();
            let width = tmp_img.width();
            let rgb = tmp_img.as_raw();
            add_time(
                "get ascii frame rgb",
                SystemTime::now().duration_since(s).unwrap().as_nanos(),
            );

            let s = SystemTime::now();
            // convert the rgb values to yuv
            let mut encoder = openh264::encoder::Encoder::new().expect("Couldn't create encoder");
            let rgb_source =
                openh264::formats::RgbSliceU8::new(&rgb, (width as usize, height as usize));
            let yuv = openh264::formats::YUVBuffer::from_rgb_source(rgb_source);

            let bitstream = encoder.encode(&yuv).unwrap();

            let mut buf = Vec::new();
            bitstream.write_vec(&mut buf);

            // let mut my_buf = Vec::new();
            // let mut ys = Vec::new();
            // let mut us = Vec::new();
            // let mut vs = Vec::new();
            // for rgb in rgb.chunks(3) {
            //     let (y, u, v) = crate::rgb_to_yuv(rgb[0], rgb[1], rgb[2]);
            //     ys.push(y);
            //     us.push(u);
            //     vs.push(v);
            // }

            // let quarter = |mut values: Vec<u8>| {
            //     values.drain(..)
            //         .enumerate()
            //         .fold(Vec::new(), |mut acc, (i, v)| {
            //             if i % 4 == 0 {
            //                 acc.push(v);
            //             }
            //             acc
            //         })
            // };
            // my_buf.append(&mut ys);
            // let mut us = quarter(us);
            // my_buf.append(&mut us);
            // let mut vs = quarter(vs);
            // my_buf.append(&mut vs);

            add_time(
                "convert rgb to yuv",
                SystemTime::now().duration_since(s).unwrap().as_nanos(),
            );

            let s = SystemTime::now();

            // write the resulting frame to the final video
            mref.write_video_with_fps(&buf, frame_rate.round() as u32);
            add_time(
                "write to video",
                SystemTime::now().duration_since(s).unwrap().as_nanos(),
            );
            counter += 1;
        },
    );
    mp4muxer.close();

    println!(
        "\x1b[{}BConvert file, so its smaler",
        loading_frame.len() + 4
    );
    crate::convert::convert(src, &tmp_video, dst);
    println!("Remove tmp video file: {tmp_video}");
    if let Err(err) = fs::remove_file(&tmp_video) {
        eprintln!("{err}");
    }

    println!("times:");
    for (key, value) in times.drain() {
        let len = value.len() as u128;
        println!(
            "{key}: {}ms",
            (value.iter().sum::<u128>() / len) / 1_000_000
        );
    }
}

fn play<F>(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: Option<f64>,
    disable_audio: bool,
    fit_termianl: bool,
    format: Pixel,
    mut f: F,
) where
    F: FnMut(Video, f32, i64),
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
        get_video(&mut ictx, scale_algorithm, format, max_width);

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
            Some(fr) => fr.numerator() as f32,
            None => 24.0,
        };
        let fps = if fps > 1000.0 { 24.0 } else { fps };
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut frame = Video::empty();
            scaler
                .run(&decoded, &mut frame)
                .expect("Input or output changed");
            f(frame, fps, duration_micros);
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
    format: Pixel,
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
        format,
        dst_width,
        dst_height,
        scale_algorithm,
    )
    .expect("Failed to get context");

    (decoder, scaler, video_stream_index)
}
