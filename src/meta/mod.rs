pub mod file;
pub mod video;

use std::path::PathBuf;
use mlua::{IntoLuaMulti, FromLuaMulti};

pub struct Request<A: FromLuaMulti, R: IntoLuaMulti> {
    get: Box<dyn Fn(A) -> anyhow::Result<R> + 'static>,
}

impl<A: FromLuaMulti, R: IntoLuaMulti> Request<A, R> {
    fn new(get: impl Fn(A) -> anyhow::Result<R> + 'static) -> Request<A, R> {
        Request { get: Box::new(get) }
    }
}

impl<A: FromLuaMulti + 'static, R: IntoLuaMulti + 'static> mlua::IntoLua for Request<A, R> {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let func = lua.create_function(move |_, arg: A| (self.get)(arg).map_err(|e| mlua::Error::external(e)))?;
        Ok(mlua::Value::Function(func))
    }
}

pub fn fill_meta(lua: &mlua::Lua, path: &std::path::Path) -> mlua::Result<()> {
    let meta = lua.create_table()?;

    meta.set("file", file::FileMeta::new(path))?;
    meta.set("video", video::VideoMeta::new(path))?;

    let globals = lua.globals();
    globals.set("meta", meta)?;

    Ok(())
}
