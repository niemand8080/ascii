pub mod image;
pub mod video;
pub mod convert;

use std::time::Duration;

pub type Pixels = Vec<Vec<(u8, u8, u8)>>;

pub const CHARS: [char; 14] = [
    ' ', '.', ':', '-', '~', '=', '+', '*', 'o', '%', '&', '8', '#', '@',
];

/// Prints the given `Pixels` to stdout.
pub fn draw(pixels: Pixels) {
    print!("\x1b[?25l"); // hide cursor
    print!("\x1b[40;2;0;0;0m");
    for row in pixels {
        for (r, g, b) in row {
            let l = get_lightness(r, g, b);
            let s = symbol(l);
            print!("\x1b[38;2;{r};{g};{b}m{s}{s}");
        }
        println!();
    }
    print!("\x1b[0m");
    print!("\x1b[?25h"); // show cursor
}

/// Get the symbol matching the lightness.
pub fn symbol(lightness: u8) -> char {
    match lightness {
        100 => CHARS[CHARS.len() - 1],
        0 => CHARS[0],
        l => {
            let p = 100 / (CHARS.len() - 2);
            for i in (1..CHARS.len() - 1).rev() {
                if l >= (i * p) as u8 {
                    return CHARS[i];
                }
            }
            CHARS[0]
        }
    }
}

/// Get the lightness of the given `RGB` values.
pub fn get_lightness(r: u8, g: u8, b: u8) -> u8 {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    (((max as u16 + min as u16) as f64 / (2.0 * 255.0)) * 100.0).round() as u8
}

/// Convert given `RGB` value into `HSL`.
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let h = match delta {
        d if max == r => 60.0 * ((g - b) / d % 6.0),
        d if max == g => 60.0 * ((b - r) / d + 2.0),
        d if max == b => 60.0 * ((r - g) / d + 4.0),
        _ => 0.0,
    };
    let h = if h < 0.0 { 360.0 + h } else { h };
    let l = (max + min) / 2.0;
    let s = match l {
        l if l > 0.5 => delta / (2.0 - max - min),
        _ => delta / (max + min),
    } * 100.0;

    (h.round() as u16, s.round() as u8, (l * 100.0).round() as u8)
}

/// Formats the given pixels (not structured) into a structured form.
///
/// # Example
///
/// ```rust
/// let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255];
///
/// let pixels = format_pixels(pixels, 1);
/// println!("{pixels:?}"); // [[(255, 0, 0)], [(0, 255, 0)], [(0, 0, 255)]]
/// ```
pub fn format_pixels(pixels: &[u8], width: u16) -> Pixels {
    pixels
        .chunks(width as usize * 3)
        .map(|chunk| {
            chunk
                .chunks(3)
                .map(|pixel| {
                    let [r, g, b] = *pixel else { todo!() };
                    (r, g, b)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Waits until the terminal size is greater than the given `min_widht` and `min_height`.
fn wait_for_terminal_scale(min_width: u32, min_height: u32) {
    if let Some((mut w, mut h)) = term_size::dimensions() {
        println!(
            "\x1b[1;31m{} x {}\x1b[0m (current: {} x {})",
            min_width, min_height, w, h
        );
        while w < min_width as usize || h < min_height as usize {
            println!(
                "\x1b[1A\x1b[2K\x1b[1;31m{} x {}\x1b[0m (current: {} x {})",
                min_width, min_height, w, h
            );
            std::thread::sleep(Duration::from_millis(500));
            (w, h) = term_size::dimensions().unwrap();
        }
        println!("\x1b[1A\x1b[1;32m{w} x {h}\x1b[0m");
    } else {
        eprintln!("Unable to get terminal dimensions");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn convert_speed() {
        let times = 100;
        let from = 0;
        let to = 100;

        let count = times as u64 * to as u64 * to as u64 * to as u64;

        let started_hsl = SystemTime::now();
        // let mut counter = 0;
        for _ in 0..times {
            for r in from..to {
                for g in from..to {
                    for b in from..to {
                        // counter += 1;
                        let _ = rgb_to_hsl(r, g, b);
                    }
                }
            }
        }
        println!("for {count} convertions");
        println!(
            "rgb to hsl: {}ms",
            SystemTime::now()
                .duration_since(started_hsl)
                .unwrap()
                .as_millis()
        );
        let started_l = SystemTime::now();
        for _ in 0..times {
            for r in from..to {
                for g in from..to {
                    for b in from..to {
                        let _ = get_lightness(r, g, b);
                    }
                }
            }
        }
        panic!(
            "rgb to l: {}ms",
            SystemTime::now()
                .duration_since(started_l)
                .unwrap()
                .as_millis()
        );
    }

    // #[test]
    // fn convert() {
    //     // for i in 0..=255 {
    //     //     println!("(255, i, 0) -> {:?}", rgb_to_hsl(255, i, 0));
    //     // }
    //     // for i in (0..=255).rev() {
    //     //     println!("({i}, 255, 0) -> {:?}", rgb_to_hsl(i, 255, 0));
    //     // }
    //     // for i in 0..=255 {
    //     //     println!("(0, 255, {i}) -> {:?}", rgb_to_hsl(0, 255, i));
    //     // }
    //     // for i in (0..=255).rev() {
    //     //     println!("(0, {i}, 255) -> {:?}", rgb_to_hsl(0, i, 255));
    //     // }
    //     // for i in 250..=255 {
    //     //     println!("({i}, 0, 255) -> {:?}", rgb_to_hsl(i, 0, 255));
    //     // }
    //     // for i in (250..=255).rev() {
    //     //     println!("(255, 0, {i}) -> {:?}", rgb_to_hsl(255, 0, i));
    //     // }

    //     assert_eq!(rgb_to_hsl(24, 98, 118), (193, 66, 28));
    //     assert_eq!(rgb_to_hsl(207, 135, 135), (0, 43, 67));
    //     assert_eq!(rgb_to_hsl(128, 128, 0), (60, 100, 25));
    //     assert_eq!(rgb_to_hsl(255, 35, 175), (322, 100, 57));
    //     assert_eq!(rgb_to_hsl(255, 85, 85), (0, 100, 67));
    // }
}
