use std::time::SystemTime;

pub fn fill_table(lua: &mlua::Lua, path: &std::path::Path) -> anyhow::Result<mlua::Table> {
    let table = lua.create_table()?;
    table.set("name", path.file_name().unwrap())?;
    table.set(
        "path",
        path.to_str().expect("Invalid UTF-8 string").to_owned(),
    )?;

    if let Ok(metadata) = path.metadata() {
        table.set("size", metadata.len())?;
        if let Ok(created) = metadata.created() {
            table.set("created", created.duration_since(SystemTime::UNIX_EPOCH)?.as_secs_f32())?;
        }
        if let Ok(accessed) = metadata.accessed() {
            table.set("accessed", accessed.duration_since(SystemTime::UNIX_EPOCH)?.as_secs_f32())?;
        }
        if let Ok(modified) = metadata.modified() {
            table.set("modified", modified.duration_since(SystemTime::UNIX_EPOCH)?.as_secs_f32())?;
        }

        table.set("is_file", metadata.is_file())?;
        table.set("is_dir", metadata.is_dir())?;
        table.set("is_symlink", metadata.is_symlink())?;

        let permissions = metadata.permissions();
        table.set("readonly", permissions.readonly())?;
        if cfg!(unix) {
            use std::os::unix::fs::PermissionsExt;
            table.set("mode", permissions.mode())?;
        }
    }

    Ok(table)
}
