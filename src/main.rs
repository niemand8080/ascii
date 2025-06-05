use ascii::{image, video};
use std::fs::File;

use ab_glyph::FontRef;

use ffmpeg_next::software::scaling::flag::Flags;

const MAX_WIDTH: f64 = 5.0 * 32.0;

// PKG_CONFIG_PATH=$PKG_CONFIG_PATH:/opt/homebrew/lib/pkgconfig cargo run --release

fn main() {
    ffmpeg_next::init().unwrap();

    let font = FontRef::try_from_slice(include_bytes!("/Users/ben/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf")).unwrap();

    video::draw_to_file("examples/BigBuckBunny.mp4", "tmp/out.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));

    // 3439s
    // video::draw_to_file("assets/All My Fellas.mp4", "tmp/All My Fellas.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Deichkind.mp4", "tmp/Deichkind.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Dancin.mp4", "tmp/Dancin.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Flashback.mp4", "tmp/Flashback.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/unreval.mp4", "tmp/unreval.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));

    // video::draw_to_file("assets/Nakimushi.mp4", "tmp/Nakimushi.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Hell's Paradise.mp4", "tmp/Hell's Paradise.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Mob Psycho 100 99.mp4", "tmp/Mob Psycho 100 99.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/Mob Psycho 100.mp4", "tmp/Mob Psycho 100.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/This Is It.mp4", "tmp/This Is It.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));
    // video::draw_to_file("assets/We love Arataka Reigen.mp4", "tmp/We love Arataka Reigen.mp4", &font, Flags::BICUBIC, Some(MAX_WIDTH));

    // image::draw_to_file("examples/ascii-torii-gate-japan.jpg", &font, image::get_rows("examples/torii-gate-japan.jpg", MAX_WIDTH).0);

    // for i in 1..=8 {
    //     let file = File::open(format!("assets/{i}.jpg")).expect("failed to open file");
    //     image::draw(file, MAX_WIDTH);
    // }

    // let file = File::open("examples/torii-gate-japan.jpg").unwrap();
    // image::draw(file, MAX_WIDTH);

    // video::draw("assets/All My Fellas.mp4", Flags::BICUBLIN, Some(MAX_WIDTH));
    // video::draw("assets/Flashback.mp4", Flags::BICUBLIN, Some(MAX_WIDTH));
    // video::draw("examples/BigBuckBunny.mp4", Flags::BICUBLIN, Some(MAX_WIDTH));
}
