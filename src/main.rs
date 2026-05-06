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
    meta::fill_meta(lua, path).context("Failed to fill meta")?;

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

    if !config_dir.exists() {
        std::fs::create_dir(&config_dir)?;
    }
    let rules_dir = config_dir.join("rules");
    if !rules_dir.exists() {
        std::fs::create_dir(&rules_dir)?;
    }
    let scripts_dir = config_dir.join("scripts");
    if !scripts_dir.exists() {
        std::fs::create_dir(&scripts_dir)?;
    }

    let listen = cli.listen;

    let rule_files = rules_dir
        .read_dir()
        .with_context(|| format!("Failed to read rules directory {}", rules_dir.display()))?
        .collect::<std::io::Result<Vec<std::fs::DirEntry>>>()
        .with_context(|| format!("Failed to iterate directory {}", rules_dir.display()))?;
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
                    process_file(&lua, &rule, &pf.path(), &scripts_dir).with_context(|| {
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
