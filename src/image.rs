use crate::{Pixels, wait_for_terminal_scale};

use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::{draw_text_mut, text_size};
use imageproc::image::{Rgb, RgbImage};
use jpeg_decoder::Decoder;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Get `pixels`, `width` and `height` of the given `image path` after scaling it down to the given
/// `max_widh`.
pub fn get_rows(path: &str, max_width: f64) -> (Pixels, u16, u16) {
    let file = File::open(path).expect("Invalid file path");

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

    (crate::format_pixels(&pixels, w), w, h)
}

/// Draws the given `image path` to stdout after scaling it to `max_width`.
pub fn draw(path: &str, max_width: f64) {
    let (rows, w, h) = get_rows(path, max_width);

    wait_for_terminal_scale(w as u32 * 2, h as u32);

    crate::draw(rows);
}

/// Draws the given `pixels` with the given `font` to the given `target` path.
pub fn draw_to_file(target: &str, font: &FontRef<'_>, pixels: Pixels) {
    let path = Path::new(&target);

    let font_size = 28.0;
    let scale = PxScale {
        x: font_size,
        y: font_size,
    };

    let width = pixels[0].len() as u32 * font_size as u32;
    let height = pixels.len() as u32 * font_size as u32;

    let mut image = RgbImage::new(width, height);

    let mut row_index = 0;

    for row in pixels {
        let mut pixel_index = 0;
        for (r, g, b) in row {
            let l = crate::get_lightness(r, g, b);
            let s = crate::symbol(l).to_string();
            draw_text_mut(
                &mut image,
                Rgb([r, g, b]),
                pixel_index * scale.x as i32,
                row_index * scale.y as i32,
                scale,
                &font,
                &s,
            );
            pixel_index += 1;
        }
        row_index += 1;
    }

    image.save(path).unwrap();
}
