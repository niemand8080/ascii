use crate::{Pixels, wait_for_terminal_scale};

use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::draw_text_mut;
use imageproc::image::{ImageBuffer, Rgb, RgbImage};
use jpeg_decoder::Decoder;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Get `pixels`, `width` and `height` of the given `image path` after scaling it down to the given
/// `max_widh`.
pub fn get_pixels(path: &str, max_width: Option<f64>) -> (Vec<u8>, u16, u16) {
    let file = File::open(path).expect("Invalid file path");

    let mut decoder = Decoder::new(BufReader::new(file));
    decoder.read_info().expect("failed to read info");
    let metadata = decoder.info().unwrap();
    let mut new_width = metadata.width;
    let mut new_height = metadata.height;
    if let Some(max_width) = max_width {
        let factor = match metadata.width as f64 {
            w if w <= max_width => 1.0,
            w => max_width / w,
        };
        new_width = (metadata.width as f64 * factor) as u16;
        new_height = (metadata.height as f64 * factor) as u16;
    }
    let (w, h) = decoder.scale(new_width, new_height).expect("scale failed");
    let pixels = decoder.decode().expect("failed to decode");

    (pixels, w, h)
}

/// Draws the given `image path` to stdout after scaling it to `max_width`.
pub fn draw(path: &str, max_width: Option<f64>) {
    let (pixels, w, h) = get_pixels(path, max_width);
    let rows = crate::format_pixels(&pixels, w);

    wait_for_terminal_scale(w as u32 * 2, h as u32);

    crate::draw(rows);
}

/// Get `ImageBuffer` with the given pixels.
pub fn get_image_buf(font: &FontRef<'_>, pixels: &Pixels) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let kerning: u32 = 4;
    let font_size = 12.0;
    let font_scale = PxScale {
        x: font_size,
        y: font_size,
    };

    let width = pixels[0].len() as u32 * (font_size as u32 - kerning);
    let height = pixels.len() as u32 * (font_size as u32 - kerning);

    let mut image = RgbImage::new(width, height);

    let mut row_index = 0;

    for row in pixels {
        let mut pixel_index = 0;
        for (r, g, b) in row {
            let l = crate::get_lightness(*r, *g, *b);
            let s = crate::symbol(l).to_string();
            draw_text_mut(
                &mut image,
                Rgb([*r, *g, *b]),
                pixel_index * (font_scale.x as i32 - kerning as i32),
                row_index * (font_scale.y as i32 - kerning as i32),
                font_scale,
                &font,
                &s,
            );
            pixel_index += 1;
        }
        row_index += 1;
    }
    image
}

/// Draws the given `pixels` with the given `font` to the given `target` path.
pub fn draw_to_file(target: &str, font: &FontRef<'_>, pixels: &Pixels) {
    let image = get_image_buf(font, pixels);
    draw_buf_to_file(target, &image);
}

pub fn draw_buf_to_file(target: &str, buf: &ImageBuffer<Rgb<u8>, Vec<u8>>) {
    let path = Path::new(&target);
    buf.save(path).unwrap();
}
