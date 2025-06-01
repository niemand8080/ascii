use std::process::Command;

pub fn convert(ogpath: &str, ipath: &str, opath: &str) {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-v", "quiet", "-stats", // Only show progress
        "-vn", "-i", ogpath, // get audio (or not video) from original video
        "-an", "-i", ipath, // get video (or not audio) from input video
        "-c", "copy", opath,
    ]);

    if let Ok(mut child) = command.spawn() {
        child.wait().expect("command was not running");
        println!("Successfully converted {ipath} to {opath}");
    } else {
        println!("ffmpeg command didn't start");
    }
}
