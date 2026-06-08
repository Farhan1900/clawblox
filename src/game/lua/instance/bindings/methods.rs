use mlua::UserDataMethods;

use super::super::Instance;

mod attributes;
mod humanoid;
mod media;
mod multiplayer;
mod object;
mod physical;

pub(crate) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    object::add_methods(methods);
    attributes::add_methods(methods);
    humanoid::add_methods(methods);
    media::add_methods(methods);
    physical::add_methods(methods);
    multiplayer::add_methods(methods);
}
