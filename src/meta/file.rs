pub fn fill_table(lua: &mlua::Lua, path: &std::path::Path) -> anyhow::Result<mlua::Table> {
    let table = lua.create_table()?;
    table.set("name", path.file_name().unwrap())?;
    table.set(
        "path",
        path.to_str().expect("Invalid UTF-8 string").to_owned(),
    )?;
    Ok(table)
}
