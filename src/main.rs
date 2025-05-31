use ascii::{image, video};
use std::fs::File;

fn main() {
    // for i in 1..=8 {
    //     let file = File::open(format!("examples/{i}.jpg")).expect("failed to open file");
    //     image::draw(file);
    // }

    video::draw("examples/BigBuckBunny.mp4");
}
