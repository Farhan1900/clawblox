use mlua::{
    AnyUserData, FromLua, Lua, Result, Table, UserData, UserDataFields, UserDataMethods, Value,
};
use std::sync::{Arc, Mutex};

mod bindings;
mod class_data;
mod class_name;
mod constructors;
mod datamodel;
mod tree;

pub use class_data::*;
pub use class_name::ClassName;
pub use constructors::register_instance;
pub use datamodel::*;

#[derive(Clone)]
pub struct Instance {
    pub data: InstanceRef,
}

impl UserData for Instance {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        bindings::fields::add_fields(fields);
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        bindings::methods::add_methods(methods);
    }
}
