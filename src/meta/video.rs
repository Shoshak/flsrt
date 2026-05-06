use crate::meta::Request;
use crate::into_lua;

into_lua! {
    pub struct VideoMeta {
        length: Request<std::path::PathBuf, f64>,
    }
}

impl VideoMeta {
    pub fn new(path: &std::path::Path) -> VideoMeta {
        VideoMeta {
            length: Request::new(|p| get_video_length(p)),
        }
    }
}

fn get_video_length(path: std::path::PathBuf) -> anyhow::Result<f64> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path.to_str().expect("Invalid UTF-8 string"),
        ])
        .output()?;
    let utf8_stdout = String::from_utf8(output.stdout)?;
    let duration_str = utf8_stdout.trim();
    let duration: f64 = duration_str.parse()?;
    Ok(duration)
}
