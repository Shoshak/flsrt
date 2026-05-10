use crate::into_lua;
#[cfg(unix)]
use std::fs::Permissions;
use std::time::SystemTime;

into_lua! {
    pub struct Meta {
        name: String,
        path: std::path::PathBuf,
        metadata: Option<OsMetadata>,
    }
}

into_lua! {
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
}

fn to_epoch(t: SystemTime) -> Result<u64, std::time::SystemTimeError> {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
}

#[cfg(unix)]
fn get_mode(perms: std::fs::Permissions) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(perms.mode())
}

#[cfg(not(unix))]
fn get_mode(_perms: std::fs::Permissions) -> Option<u32> {
    None
}

impl Meta {
    pub fn new(path: &std::path::Path) -> Meta {
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
                mode: get_mode(permissions),
            };
            Some(meta)
        } else {
            None
        };
        Meta {
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
