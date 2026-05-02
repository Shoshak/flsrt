use anyhow::{Context, anyhow};
use clap::Parser;
use mlua::LuaSerdeExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    rules: Option<PathBuf>,
    #[arg(short, long, default_value_t = false)]
    listen: bool,
}

#[derive(Deserialize)]
struct Rule {
    name: String,
    description: String,
    groups: Vec<String>,
    paths: Vec<PathBuf>,
    recursive: bool,
    script: String,
}

#[derive(Deserialize)]
struct Action {
    copy: Option<Vec<String>>,
    r#move: Option<String>,
}

fn get_video_length(path: &Path) -> anyhow::Result<f64> {
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

fn construct_world(lua: &mlua::Lua, path: &Path) -> anyhow::Result<()> {
    let meta = lua.create_table()?;

    let file = lua.create_table()?;
    file.set("name", path.file_name().unwrap());
    file.set(
        "path",
        path.to_str().expect("Invalid UTF-8 string").to_owned(),
    )?;
    meta.set("file", file)?;

    let video = lua.create_table()?;
    let video_length = lua.create_function(|_, p: PathBuf| {
        get_video_length(&p).map_err(|e| mlua::Error::external(e))
    })?;
    video.set("length", video_length)?;
    meta.set("video", video)?;

    let globals = lua.globals();
    globals.set("meta", meta)?;

    Ok(())
}

fn process_file(
    lua: &mlua::Lua,
    rule: &Rule,
    path: &Path,
    config_dir: &Path,
) -> anyhow::Result<()> {
    construct_world(lua, path).context("Failed to fill meta")?;

    let script = std::fs::read_to_string(config_dir.join("scripts").join(&rule.script))
        .with_context(|| format!("Failed to read script {}", rule.script))?;
    let value: mlua::Value = lua.load(script).eval().context("Failed to eval")?;
    let action: Action = lua.from_value(value).context("Failed to deserialize")?;

    if let Some(c) = action.copy {
        for target in c {
            std::fs::copy(path, &target)
                .with_context(|| format!("Failed to copy {} to {}", path.display(), target))?;
        }
    }

    if let Some(target) = action.r#move {
        match std::fs::rename(path, &target) {
            Ok(()) => Ok(()),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::CrossesDevices {
                    std::fs::copy(path, &target).with_context(|| {
                        format!(
                            "Failed to copy when moving {} to {}",
                            path.display(),
                            target
                        )
                    })?;
                    std::fs::remove_file(path).with_context(|| {
                        format!(
                            "Failed to cleanup when moving {} to {}",
                            path.display(),
                            target
                        )
                    })?;
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let lua = mlua::Lua::new();
    let cli = Cli::parse();

    let config_dir = match cli.rules {
        Some(r) => Ok(r),
        None => {
            if let Some(mut config_dir) = dirs::config_dir() {
                // TODO "Create folder"
                config_dir.push("flsrt");
                Ok(config_dir)
            } else {
                Err(anyhow!(
                    "No config directory provided and default config directory does not exist."
                ))
            }
        }
    }?;
    let listen = cli.listen;

    let rule_files = config_dir
        .join("rules")
        .read_dir()
        .with_context(|| format!("Failed to read rules directory {}", config_dir.display()))?
        .collect::<std::io::Result<Vec<std::fs::DirEntry>>>()
        .with_context(|| format!("Failed to iterate directory {}", config_dir.display()))?;
    for rf in rule_files {
        let rule_path = rf.path();
        let content = std::fs::read_to_string(&rule_path)
            .with_context(|| format!("Failed to read rule file {}", rule_path.display()))?;
        let rule: Rule = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse rule {}", rule_path.display()))?;

        if !listen {
            for p in &rule.paths {
                let process_files = p
                    .read_dir()
                    .with_context(|| {
                        format!(
                            "Failed to read target directory {} for rule {}",
                            p.display(),
                            rule.name
                        )
                    })?
                    .collect::<std::io::Result<Vec<std::fs::DirEntry>>>()
                    .with_context(|| {
                        format!(
                            "Failed to iterate directory {} for rule {}",
                            p.display(),
                            rule.name
                        )
                    })?;

                for pf in process_files {
                    process_file(&lua, &rule, &pf.path(), &config_dir).with_context(|| {
                        format!(
                            "Failed to apply rule {} to file {}",
                            rule.name,
                            pf.path().display()
                        )
                    })?;
                }
            }
        } else {
            todo!("Implement listening to filesystem changes");
        }
    }

    Ok(())
}
