fn get_video_length(path: &std::path::Path) -> anyhow::Result<f64> {
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

pub fn fill_table(lua: &mlua::Lua, path: &std::path::Path) -> anyhow::Result<mlua::Table> {
    let table = lua.create_table()?;
    let video_length = lua.create_function(|_, p: std::path::PathBuf| {
        get_video_length(&p).map_err(|e| mlua::Error::external(e))
    })?;
    table.set("length", video_length)?;
    Ok(table)
}
