mod meta;

use std::collections::BTreeMap;
use std::sync::mpsc;

use anyhow::{Context, anyhow};
use clap::Parser;
use mlua::LuaSerdeExt;
use notify::Watcher;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    rules: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Rule {
    name: String,
    paths: Vec<PathBuf>,
    script: String,

    immediately: Option<bool>,
    listen: Option<bool>,

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

fn process_file(lua: &mlua::Lua, script: &str, path: &Path) -> anyhow::Result<()> {
    lua.globals()
        .set("meta", meta::Meta::new(path))
        .context("Failed to fill meta")?;

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

fn process_path(lua: &mlua::Lua, script: &str, path: &Path) -> anyhow::Result<()> {
    let dir_contents = get_dir_contents(path).with_context(|| format!("{path:?}"))?;

    // TODO: recursive iteration
    for f in dir_contents {
        process_file(&lua, &script, &f).with_context(|| format!("{f:?}"))?;
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

enum Event {
    New { rule: String, path: PathBuf },
    CtrlC,
    Error(notify::Error),
}

struct RuleWatcher {
    rule: String,
    sender: mpsc::SyncSender<Event>,
}

impl notify::EventHandler for RuleWatcher {
    fn handle_event(&mut self, event: notify::Result<notify::Event>) {
        match event {
            Ok(e) => match e.kind {
                notify::EventKind::Access(notify::event::AccessKind::Close(
                    notify::event::AccessMode::Write,
                )) => {
                    for path in e.paths {
                        self.sender
                            .send(Event::New {
                                path: path,
                                rule: self.rule.to_string(),
                            })
                            .unwrap();
                    }
                }
                _ => {}
            },
            Err(e) => self.sender.send(Event::Error(e)).unwrap(),
        }
    }
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

    let mut rules: BTreeMap<String, Rule> = BTreeMap::new();
    for rf in rule_files {
        match process_rule_file(&rf) {
            Ok(mut r) => {
                if r.disabled.unwrap_or(false) {
                    continue;
                }
                if rules.contains_key(&r.name) {
                    anyhow!("Duplicate rule name {}", r.name);
                }

                let script_file = &r.script;
                let script_path = dirs.scripts.join(script_file);
                let script = std::fs::read_to_string(&script_path).with_context(|| {
                    format!(
                        "Failed to read script file {:?} for rule {}",
                        script_path, r.name
                    )
                })?;
                r.script = script;

                rules.insert(r.name.to_string(), r);
            }
            Err(e) => Err(e).context(format!("{rf:?}"))?,
        }
    }

    println!(
        "Finished processing rules. Total rule count: {}",
        rules.len()
    );
    rules
        .values()
        .enumerate()
        .for_each(|(i, r)| println!("{}. {}", i + 1, r.name));

    let (event_tx, event_rx) = mpsc::sync_channel::<Event>(0);
    let mut watchers: Vec<Box<dyn notify::Watcher>> = Vec::new();

    for rule in rules.values() {
        if rule.immediately.unwrap_or(true) {
            println!("Running {}...", rule.name);
            for p in &rule.paths {
                process_path(&lua, &rule.script, &p).with_context(|| format!("{}", rule.name))?;
            }
        }

        if rule.listen.unwrap_or(false) {
            println!("Staging {} for file watching...", rule.name);
            for p in &rule.paths {
                let handler = RuleWatcher {
                    rule: rule.name.to_string(),
                    sender: event_tx.clone(),
                };
                let mut watcher = notify::recommended_watcher(handler)?;
                watcher.watch(
                    p,
                    if rule.recursive.unwrap_or(false) {
                        notify::RecursiveMode::Recursive
                    } else {
                        notify::RecursiveMode::NonRecursive
                    },
                );
                watchers.push(Box::new(watcher));
            }
        }
    }

    if watchers.len() == 0 {
        println!("No watched rules. Exiting...");
        return Ok(());
    }

    println!("Staging complete. And now we wait.");

    let ctrlc_sender = event_tx.clone();
    ctrlc::set_handler(move || {
        ctrlc_sender.send(Event::CtrlC).unwrap();
    })?;

    while let Ok(msg) = event_rx.recv() {
        match msg {
            Event::CtrlC => break,
            Event::New { rule, path } => {
                let rule = rules.get(&rule).unwrap();
                process_file(&lua, &rule.script, &path);
            }
            Event::Error(e) => Err(e)?,
        }
    }
    Ok(())
}
