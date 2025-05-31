use ascii::{image, video};
use std::fs::File;

use ffmpeg_next::software::scaling::flag::Flags;

const MAX_WIDTH: f64 = 192.0;

fn main() {
    // for i in 1..=8 {
    //     let file = File::open(format!("assets/{i}.jpg")).expect("failed to open file");
    //     image::draw(file, MAX_WIDTH);
    // }

    // let file = File::open("examples/torii-gate-japan.jpg").unwrap();
    // image::draw(file, MAX_WIDTH);

    // video::draw("examples/BigBuckBunny.mp4", Flags::BICUBLIN, MAX_WIDTH);
}
