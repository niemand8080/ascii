extern crate ffmpeg_next as ffmpeg;

use crate::MAX_WIDTH;

use ffmpeg::format::Pixel;
use ffmpeg::media::Type;
use ffmpeg::software::{
    self,
    scaling::{context::Context, flag::Flags},
};
use ffmpeg::util::frame::Video;

pub fn draw(path: &str) {
    ffmpeg::init().unwrap();

    println!("opening file");
    let mut ictx = ffmpeg::format::input(path).expect("Couldn't open file");
    println!("finding best stream");
    let input = ictx.streams().best(Type::Video).expect("No stream found");
    let video_stream_index = input.index();

    println!("constructing decoder context");
    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())
        .expect("Couldn't construct deocder context");
    println!("decoding video");
    let mut decoder = context_decoder
        .decoder()
        .video()
        .expect("Couldn't find decoder");

    let factor = match decoder.width() as f64 {
        w if w <= MAX_WIDTH => 1.0,
        w => MAX_WIDTH / w,
    };

    println!("creating scaler");
    let dst_width = (decoder.width() as f64 * factor) as u32;
    let dst_height = (decoder.height() as f64 * factor) as u32;
    // todo: test different flags aka. scaler options
    let mut scaler = Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        dst_width,
        dst_height,
        Flags::BICUBIC,
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

    let mut process_frames = |decoder: &mut ffmpeg::decoder::Video| {
        let mut decoded = Video::empty();
        let frame_rate = match decoder.frame_rate() {
            None => 24,
            Some(fr) => fr.numerator(),
        };
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
            if frame_index > 20 {
                let pixels = rgb_frame.data(0);
                // println!("{}, {}", pixels.len(), pixels.len() as f64 / (dst_width as f64 * 3.0));
                let rows = crate::format_pixels(pixels, rgb_frame.width() as u16);
                // println!("{}x{}", rows[1].len(), rows.len());
                crate::draw(rows);
                std::thread::sleep_ms((1000.0 / frame_rate as f64) as u32);
                print!("\x1b[{}A", rgb_frame.height());
            }
            frame_index += 1;
        }
        print!("\x1b[?25h"); // show cursor
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet).expect("Failed to send packet");
            process_frames(&mut decoder);
        }
    }
    decoder
        .send_eof()
        .expect("Failed to send eof (end of file)");
    process_frames(&mut decoder);
}
