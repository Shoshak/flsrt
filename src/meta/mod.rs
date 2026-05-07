pub mod file;
pub mod video;

use mlua::{FromLuaMulti, IntoLuaMulti};

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
        let func = lua.create_function(move |_, arg: A| {
            (self.get)(arg).map_err(|e| mlua::Error::external(e))
        })?;
        Ok(mlua::Value::Function(func))
    }
}

#[macro_export]
macro_rules! into_lua {
    (pub struct $name:ident {
        $($field_name:ident: $field_type:ty,)*
    }) => {
        pub struct $name {
            $($field_name: $field_type,)*
        }

        impl mlua::IntoLua for $name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                let table = lua.create_table()?;
                $(
                    table.set(stringify!($field_name), self.$field_name)?;
                )*
                Ok(mlua::Value::Table(table))
            }
        }
    }
}

into_lua! {
    pub struct Meta {
        file: file::Meta,
        video: video::Meta,
    }
}

impl Meta {
    pub fn new(path: &std::path::Path) -> Meta {
        Meta {
            file: file::Meta::new(path),
            video: video::Meta::new(),
        }
    }
}
