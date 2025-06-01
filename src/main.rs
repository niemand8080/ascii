use ascii::{image, video};
use std::fs::File;

use ab_glyph::FontRef;

use ffmpeg_next::software::scaling::flag::Flags;

const MAX_WIDTH: f64 = 5.0 * 32.0;

// PKG_CONFIG_PATH=$PKG_CONFIG_PATH:/opt/homebrew/lib/pkgconfig cargo run --release

fn main() {
    let font = FontRef::try_from_slice(include_bytes!("/Users/ben/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf")).unwrap();

    // video::draw_to_file("assets/All My Fellas.mp4", "examples/test.mp4", &font, Flags::BICUBIC, MAX_WIDTH);

    image::draw_to_file("examples/ascii-torii-gate-japan.jpg", &font, image::get_rows("examples/torii-gate-japan.jpg", MAX_WIDTH).0);

    // for i in 1..=8 {
    //     let file = File::open(format!("assets/{i}.jpg")).expect("failed to open file");
    //     image::draw(file, MAX_WIDTH);
    // }

    // let file = File::open("examples/torii-gate-japan.jpg").unwrap();
    // image::draw(file, MAX_WIDTH);

    // video::draw("assets/All My Fellas.mp4", Flags::BICUBLIN, MAX_WIDTH);
    // video::draw("assets/Flashback.mp4", Flags::BICUBLIN, MAX_WIDTH);
    // video::draw("examples/BigBuckBunny.mp4", Flags::BICUBLIN, MAX_WIDTH);
}
