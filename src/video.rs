extern crate ffmpeg_next as ffmpeg;

use crate::wait_for_terminal_scale;

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use ffmpeg::format::{Pixel, Sample as FFmpegSample, sample::Type as SampleType};
use ffmpeg::media::Type as MediaType;
use ffmpeg::software::scaling::context::Context;
use ffmpeg::util::frame::{self, Audio, Video};

use ringbuf::RingBuffer;

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

fn new_audio() -> (cpal::Device, cpal::SupportedStreamConfig) {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("No output device available");

    let supported_configs = device
        .supported_output_configs()
        .expect("Device disconected")
        .next()
        .expect("No supported configs");

    (device, supported_configs.with_max_sample_rate())
}

pub fn draw_not_working(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: f64,
) {
    ffmpeg::init().unwrap();

    let (device, audio_config) = new_audio();
    // new input ctx
    let mut ictx = ffmpeg::format::input(path).expect("Couldn't open file");

    // get best video stream index
    let video_input = ictx
        .streams()
        .best(MediaType::Video)
        .expect("No video stream found");
    let video_stream_index = video_input.index();

    // create buffer to store audio data
    let buffer = RingBuffer::<f32>::new(8192);
    let (mut producer, mut consumer) = buffer.split();

    // get best audio stream index AND create audio decoder AND create resampler
    let (mut audio_decoder, audio_stream_index, mut resampler, audio_stream) =
        if let Some(audio) = ictx.streams().best(MediaType::Audio) {
            let decoder = ffmpeg::codec::context::Context::from_parameters(audio.parameters())
                .expect("Couldn't construct audio decoder context")
                .decoder()
                .audio()
                .expect("Couldn't get audio decoder");
            // create audio resampler
            let resampler = ffmpeg::software::resampling::Context::get(
                decoder.format(),
                decoder.channel_layout(),
                decoder.rate(),
                audio_config.sample_format().as_ffmpeg_sample(),
                decoder.channel_layout(),
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
                Some(decoder),
                Some(audio.index()),
                Some(resampler),
                Some(audio_stream),
            )
        } else {
            (None, None, None, None)
        };

    // construct video decoder AND scaler
    let (mut video_decoder, mut scaler) = {
        let context_decoder =
            ffmpeg::codec::context::Context::from_parameters(video_input.parameters())
                .expect("Couldn't construct deocder context");
        // create video decoder
        let decoder = context_decoder
            .decoder()
            .video()
            .expect("Couldn't find decoder");

        let factor = match decoder.width() as f64 {
            w if w <= max_width => 1.0,
            w => max_width / w,
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
        // todo: test different flags aka. scaler options
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

        (decoder, scaler)
    };

    // wait_for_terminal_scale(scaler.output().width * 2, scaler.output().height + 2);

    let mut process_audio_frames = |decoder: &mut ffmpeg::decoder::Audio| {
        if resampler.is_none() {
            panic!("Resampler is none");
        }
        let mut decoded = Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut resampled = Audio::empty();
            resampler
                .as_mut()
                .unwrap()
                .run(&decoded, &mut resampled)
                .expect("Input or output changed");

            let both_channels = packed(&resampled);

            while producer.remaining() < both_channels.len() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            producer.push_slice(both_channels);
        }
    };

    let mut process_video_frames = |decoder: &mut ffmpeg::decoder::Video| {
        let mut decoded = Video::empty();
        print!("\x1b[?25l"); // hide cursor
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb_frame = Video::empty();
            // scale and convert color space
            scaler
                .run(&decoded, &mut rgb_frame)
                .expect("Input or output changed");
            let pixels = rgb_frame.data(0);
            let rows = crate::format_pixels(pixels, rgb_frame.width() as u16);
            // draw the frame
            crate::draw(rows);
            // move cursor to start position, so a new frame can be drawn
            print!("\x1b[{}A", rgb_frame.height());
        }
        print!("\x1b[?25h"); // show cursor
    };

    if let Some(audio_stream) = audio_stream {
        audio_stream.play().unwrap();
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            video_decoder
                .send_packet(&packet)
                .expect("Failed to send video packet");
            process_video_frames(&mut video_decoder);
        }
        if let Some(audio_stream_index) = audio_stream_index {
            if stream.index() == audio_stream_index {
                if let Some(audio_decoder) = &mut audio_decoder {
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
    process_video_frames(&mut video_decoder);
}

pub fn draw(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: f64,
) {
    ffmpeg::init().unwrap();

    let (device, audio_config) = new_audio();
    // new input ctx
    let mut ictx = ffmpeg::format::input(path).expect("Couldn't open file");

    // create buffer to store audio data
    let buffer = RingBuffer::<f32>::new(2usize.pow(13));
    let (mut producer, mut consumer) = buffer.split();

    // get best audio stream index AND creat audio decoder AND create resampler
    let (mut audio_decoder, mut resampler, mut audio_stream, audio_stream_index) =
        if let Some(audio) = ictx.streams().best(MediaType::Audio) {
            println!("create audio decoder");
            let audio_decoder =
                ffmpeg::codec::context::Context::from_parameters(audio.parameters())
                    .expect("Couldn't construct audio decoder context")
                    .decoder()
                    .audio()
                    .expect("Couldn't get audio decoder");

            println!("Set up audio resampler");
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
        } else {
            (None, None, None, None)
        };

    // contruct video decoder AND scaler AND get best video stream index
    let (mut video_decoder, mut scaler, video_stream_index) = {
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

        let factor = match decoder.width() as f64 {
            w if w <= max_width => 1.0,
            w => max_width / w,
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
        // todo: test different flags aka. scaler options
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
    };

    wait_for_terminal_scale(scaler.output().width * 2, scaler.output().height + 2);

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
        print!("\x1b[?25l"); // hide cursor
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb_frame = Video::empty();
            scaler
                .run(&decoded, &mut rgb_frame)
                .expect("Input or output changed");
            let pixels = rgb_frame.data(0);
            let rows = crate::format_pixels(pixels, rgb_frame.width() as u16);
            crate::draw(rows);
            print!("\x1b[{}A", rgb_frame.height());
        }
        print!("\x1b[?25h"); // show cursor
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

pub fn draw_dont_check_audio(
    path: &str,
    scale_algorithm: ffmpeg_next::software::scaling::flag::Flags,
    max_width: f64,
) {
    ffmpeg::init().unwrap();

    let (device, audio_config) = new_audio();
    // new input ctx
    let mut ictx = ffmpeg::format::input(path).expect("Couldn't open file");

    let video_input = ictx
        .streams()
        .best(MediaType::Video)
        .expect("No video stream found");
    let video_stream_index = video_input.index();

    let buffer = RingBuffer::<f32>::new(8192);
    let (mut producer, mut consumer) = buffer.split();

    // audio
    println!("finding best audio stream");
    let audio = ictx
        .streams()
        .best(MediaType::Audio)
        .expect("No audio stream found");
    let audio_stream_index = audio.index();

    println!("create audio decoder");
    let mut audio_decoder = ffmpeg::codec::context::Context::from_parameters(audio.parameters())
        .expect("Couldn't construct audio decoder context")
        .decoder()
        .audio()
        .expect("Couldn't get audio decoder");

    println!("Set up audio resampler");
    let mut resampler = ffmpeg::software::resampling::Context::get(
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
    // audio end

    println!("constructing decoder context");
    let context_decoder =
        ffmpeg::codec::context::Context::from_parameters(video_input.parameters())
            .expect("Couldn't construct deocder context");
    println!("decoding video");
    let mut video_decoder = context_decoder
        .decoder()
        .video()
        .expect("Couldn't find decoder");

    let factor = match video_decoder.width() as f64 {
        w if w <= max_width => 1.0,
        w => max_width / w,
    };

    println!("creating scaler");
    let dst_width = (video_decoder.width() as f64 * factor) as u32;
    let dst_height = (video_decoder.height() as f64 * factor) as u32;
    // todo: test different flags aka. scaler options
    let mut scaler = Context::get(
        video_decoder.format(),
        video_decoder.width(),
        video_decoder.height(),
        Pixel::RGB24,
        dst_width,
        dst_height,
        scale_algorithm,
    )
    .expect("Failed to get context");

    println!(
        "{}x{} -> {}x{}",
        scaler.input().width,
        scaler.input().height,
        scaler.output().width,
        scaler.output().height
    );

    let mut frame_index = 0;

    let mut process_audio_frames = |decoder: &mut ffmpeg::decoder::Audio| {
        let mut decoded = Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut resampled = Audio::empty();
            resampler
                .run(&decoded, &mut resampled)
                .expect("Input or output changed");

            let both_channels = packed(&resampled);

            while producer.remaining() < both_channels.len() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            producer.push_slice(both_channels);
        }
    };

    let mut process_frames = |decoder: &mut ffmpeg::decoder::Video| {
        let mut decoded = Video::empty();
        print!("\x1b[?25l"); // hide cursor
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb_frame = Video::empty();
            scaler
                .run(&decoded, &mut rgb_frame)
                .expect("Input or output changed");
            // println!(
            //     "{frame_index} ({}x{} -> {}x{})",
            //     decoded.width(),
            //     decoded.height(),
            //     rgb_frame.width(),
            //     rgb_frame.height()
            // );
            // if frame_index > 20 {
            let pixels = rgb_frame.data(0);
            // println!("{}, {}", pixels.len(), pixels.len() as f64 / (dst_width as f64 * 3.0));
            let rows = crate::format_pixels(pixels, rgb_frame.width() as u16);
            // println!("{}x{}", rows[1].len(), rows.len());
            crate::draw(rows);
            print!("\x1b[{}A", rgb_frame.height());
            // }
            frame_index += 1;
        }
        print!("\x1b[?25h"); // show cursor
    };

    audio_stream.play().unwrap();

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            video_decoder
                .send_packet(&packet)
                .expect("Failed to send video packet");
            process_frames(&mut video_decoder);
        }
        if stream.index() == audio_stream_index {
            audio_decoder
                .send_packet(&packet)
                .expect("Failed to send audio packet");
            process_audio_frames(&mut audio_decoder);
        }
    }
    video_decoder
        .send_eof()
        .expect("Failed to send eof (end of file)");
    process_frames(&mut video_decoder);
}
