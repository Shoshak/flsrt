use std::time::SystemTime;

pub struct FileMeta {
    name: String,
    path: std::path::PathBuf,
    metadata: Option<OsMetadata>,
}

pub struct OsMetadata {
    size: u64,

    created: Option<u64>,
    accessed: Option<u64>,
    modified: Option<u64>,

    is_file: bool,
    is_dir: bool,
    is_symlink: bool,

    readonly: bool,
    mode: Option<u32>,
}

fn to_epoch(t: SystemTime) -> Result<u64, std::time::SystemTimeError> {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
}

impl FileMeta {
    pub fn new(path: &std::path::Path) -> FileMeta {
        let meta = if let Ok(m) = path.metadata() {
            let permissions = m.permissions();
            let meta = OsMetadata {
                size: m.len(),

                created: if let Ok(c) = m.created() {
                    to_epoch(c).ok()
                } else {
                    None
                },
                accessed: if let Ok(a) = m.accessed() {
                    to_epoch(a).ok()
                } else {
                    None
                },
                modified: if let Ok(m) = m.modified() {
                    to_epoch(m).ok()
                } else {
                    None
                },

                is_file: m.is_file(),
                is_dir: m.is_dir(),
                is_symlink: m.is_symlink(),

                readonly: permissions.readonly(),
                mode: if cfg!(unix) {
                    use std::os::unix::fs::PermissionsExt;
                    Some(permissions.mode())
                } else {
                    None
                },
            };
            Some(meta)
        } else {
            None
        };
        FileMeta {
            name: path
                .file_name()
                .unwrap()
                .to_str()
                .expect("Invalid UTF-8 file name")
                .to_owned(),
            path: path.to_owned(),
            metadata: meta,
        }
    }
}

impl mlua::IntoLua for FileMeta {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let table = lua.create_table()?;
        table.set("name", self.name)?;
        table.set("path", self.path)?;

        if let Some(m) = self.metadata {
            let mt = lua.create_table()?;
            mt.set("size", m.size)?;

            mt.set("created", m.created)?;
            mt.set("accessed", m.accessed)?;
            mt.set("modified", m.modified)?;

            mt.set("is_file", m.is_file)?;
            mt.set("is_dir", m.is_dir)?;
            mt.set("is_symlink", m.is_symlink)?;

            mt.set("readonly", m.readonly)?;
            mt.set("mode", m.mode)?;

            table.set("metadata", mt);
        }

        Ok(mlua::Value::Table(table))
    }
}
