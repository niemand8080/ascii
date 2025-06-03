use std::process::Command;

pub fn convert(ogpath: &str, ipath: &str, opath: &str) {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-v", "error", "-stats", // Only show progress (or errors)
        "-vn", "-i", ogpath, // get audio (or not video) from original video
        "-an", "-i", ipath, // get video (or not audio) from input video
        // "-c:v", "libx265", // video codec
        // "-b:v", "700k", // 700k bitrate
        // "-c:a", "libmp3lame", // audio codec
        opath,
    ]);

    if let Ok(mut child) = command.spawn() {
        let s = std::time::SystemTime::now();
        child.wait().expect("command was not running");
        println!("Successfully converted {ipath} to {opath}, in {}ms",
            std::time::SystemTime::now().duration_since(s).unwrap().as_millis());
    } else {
        println!("ffmpeg command didn't start");
    }
}
