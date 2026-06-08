use super::super::lua::runtime::PhysicsCommand;
use super::GameInstance;

impl GameInstance {
    pub(super) fn process_physics_commands(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        for command in runtime.game().drain_physics_commands() {
            match command {
                PhysicsCommand::ApplyImpulse { part_id, impulse } => {
                    self.physics.apply_impulse_to_part(part_id, impulse.to_array());
                }
                PhysicsCommand::ApplyImpulseAtPosition {
                    part_id,
                    impulse,
                    position,
                } => {
                    self.physics.apply_impulse_at_position_to_part(
                        part_id,
                        impulse.to_array(),
                        position.to_array(),
                    );
                }
                PhysicsCommand::ApplyAngularImpulse { part_id, impulse } => {
                    self.physics
                        .apply_angular_impulse_to_part(part_id, impulse.to_array());
                }
            }
        }
    }
}
