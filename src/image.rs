use jpeg_decoder::Decoder;
use std::fs::File;
use std::io::BufReader;

pub fn draw(file: File, max_width: f64) {
    let mut decoder = Decoder::new(BufReader::new(file));
    decoder.read_info().expect("failed to read info");
    let metadata = decoder.info().unwrap();
    let factor = match metadata.width as f64 {
        w if w <= max_width => 1.0,
        w => max_width / w,
    };
    let new_width = (metadata.width as f64 * factor) as u16;
    let new_height = (metadata.height as f64 * factor) as u16;
    let (w, h) = decoder.scale(new_width, new_height).expect("scale failed");
    let pixels = decoder.decode().expect("failed to decode");

    let rows = crate::format_pixels(&pixels, w);

    crate::draw(rows);
    println!("{w} x {h} ({new_width} x {new_height})");
}
