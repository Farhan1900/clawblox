use mlua::UserDataFields;

use super::super::Instance;

mod actuators;
mod base_part;
mod core;
mod gui;
mod humanoid;
mod media;
mod player;
mod spatial;
mod values;
mod welds;

pub(crate) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    core::add_fields(fields);
    spatial::add_fields(fields);
    base_part::add_fields(fields);
    actuators::add_fields(fields);
    values::add_fields(fields);
    humanoid::add_fields(fields);
    player::add_fields(fields);
    media::add_fields(fields);
    welds::add_fields(fields);
    gui::add_fields(fields);
}
