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
    #[arg(short, long, default_value_t = true)]
    fail_fast: bool,
}

#[derive(Deserialize)]
struct Rule {
    name: String,
    paths: Vec<PathBuf>,
    script: String,

    immediately: Option<bool>,
    listen: Option<bool>,
    every: Option<u32>,

    description: Option<String>,
    groups: Option<Vec<String>>,

    recursive: Option<bool>,

    disabled: Option<bool>,
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

fn create_dir(dir: &Path) -> std::io::Result<()> {
    match std::fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

fn prepare_directories(config_dir: &Path) -> anyhow::Result<Directories> {
    create_dir(&config_dir).context(format!("{:?}", config_dir))?;

    let rules_dir = config_dir.join("rules");
    create_dir(&rules_dir).context(format!("{:?}", rules_dir))?;

    let scripts_dir = config_dir.join("scripts");
    create_dir(&scripts_dir).context(format!("{:?}", scripts_dir))?;

    Ok(Directories {
        rules: rules_dir,
        scripts: scripts_dir,
    })
}

fn get_dir_contents(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    dir.read_dir()?.map(|res| res.map(|e| e.path())).collect()
}

fn process_rule_file(f: &Path) -> anyhow::Result<Rule> {
    let content = std::fs::read_to_string(&f)?;
    let rule: Rule = serde_json::from_str(&content)?;
    Ok(rule)
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

    let dirs = prepare_directories(&config_dir).with_context(|| {
        format!(
            "Failed to prepare config directory {}",
            config_dir.display()
        )
    })?;
    let rule_files = get_dir_contents(&dirs.rules)
        .with_context(|| format!("Failed to list the contents of {}", dirs.rules.display()))?;

    let mut rules = Vec::with_capacity(rule_files.len());
    for rf in rule_files {
        match process_rule_file(&rf) {
            Ok(r) => {
                if !r.disabled.unwrap_or(false) {
                    rules.push(r);
                }
            }
            Err(e) if cli.fail_fast => Err(e).context(format!("{rf:?}"))?,
            Err(e) => eprintln!("Failed to process rule {rf:?}: {e}"),
        }
    }
    println!(
        "Finished processing rules. Total rule count: {}",
        rules.len()
    );

    rules.sort_by(|a, b| a.name.cmp(&b.name));

    println!("Rule order:");
    rules
        .iter()
        .enumerate()
        .for_each(|(i, r)| println!("{}. {}", i + 1, r.name));

    for rule in rules {
        if rule.immediately.unwrap_or(true) {
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
        }

        if rule.listen.unwrap_or(false) {
            todo!("Implement listening to directory updates");
        }

        if let Some(e) = rule.every {
            todo!("Implement ticking rules");
        }
    }

    Ok(())
}
