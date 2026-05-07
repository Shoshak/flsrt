mod meta;

use anyhow::{Context, anyhow};
use clap::Parser;
use mlua::LuaSerdeExt;
use serde::Deserialize;
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

fn process_file(
    lua: &mlua::Lua,
    rule: &Rule,
    path: &Path,
    scripts_dir: &Path,
) -> anyhow::Result<()> {
    lua.globals()
        .set("meta", meta::Meta::new(path))
        .context("Failed to fill meta")?;

    let script = std::fs::read_to_string(scripts_dir.join(&rule.script))
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
                        .with_context(|| format!("Failed to move {} to {}", path.display(), target))
                }
            }
        }?;
    }

    Ok(())
}

struct Directories {
    rules: PathBuf,
    scripts: PathBuf,
}

fn create_dir(dir: PathBuf) -> std::io::Result<PathBuf> {
    match std::fs::create_dir(&dir) {
        Ok(()) => Ok(dir),
        Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(dir),
        Err(e) => Err(e),
    }
}

fn prepare_directories(config_dir: PathBuf) -> std::io::Result<Directories> {
    let config_dir = create_dir(config_dir)?;

    Ok(Directories {
        rules: create_dir(config_dir.join("rules"))?,
        scripts: create_dir(config_dir.join("scripts"))?,
    })
}

fn get_dir_contents(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    dir.read_dir()?.map(|res| res.map(|e| e.path())).collect()
}

fn main() -> anyhow::Result<()> {
    let lua = mlua::Lua::new();
    let cli = Cli::parse();

    let config_dir = match cli.rules {
        Some(r) => Ok(r),
        None => {
            if let Some(mut config_dir) = dirs::config_dir() {
                config_dir.push("flsrt");
                Ok(config_dir)
            } else {
                Err(anyhow!(
                    "No config directory provided and default config directory does not exist."
                ))
            }
        }
    }?;
    let dirs =
        prepare_directories(config_dir).context("Failed to prepare config directory for use")?;

    let rule_files = get_dir_contents(&dirs.rules)
        .with_context(|| format!("Failed to list the contents of {}", dirs.rules.display()))?;
    for rf in rule_files {
        let content = std::fs::read_to_string(&rf)
            .with_context(|| format!("Failed to read rule file {}", rf.display()))?;
        let rule: Rule = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse rule {}", rf.display()))?;

        if !cli.listen {
            for p in &rule.paths {
                let process_files = get_dir_contents(p).with_context(|| {
                    format!(
                        "Failed to list directory {} contents for rule {}",
                        p.display(),
                        rule.name
                    )
                })?;

                for pf in process_files {
                    process_file(&lua, &rule, &pf, &dirs.scripts).with_context(|| {
                        format!(
                            "Failed to apply rule {} to file {}",
                            rule.name,
                            pf.display()
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
