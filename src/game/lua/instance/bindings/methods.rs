use mlua::UserDataMethods;

use super::super::Instance;

mod attributes;
mod humanoid;
mod media;
mod object;
mod physical;
mod player;
mod remote_events;

pub(crate) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    object::add_methods(methods);
    remote_events::add_methods(methods);
    attributes::add_methods(methods);
    humanoid::add_methods(methods);
    media::add_methods(methods);
    physical::add_methods(methods);
    player::add_methods(methods);
}
