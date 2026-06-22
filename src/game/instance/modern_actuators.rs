use super::GameInstance;
use crate::game::lua::instance::Instance;
use crate::game::lua::types::{ActuatorRelativeTo, CFrame, Vector3};

fn finite_vector(value: Vector3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn rotate_vector(cframe: CFrame, vector: Vector3) -> Vector3 {
    let r = cframe.rotation;
    Vector3::new(
        r[0][0] * vector.x + r[0][1] * vector.y + r[0][2] * vector.z,
        r[1][0] * vector.x + r[1][1] * vector.y + r[1][2] * vector.z,
        r[2][0] * vector.x + r[2][1] * vector.y + r[2][2] * vector.z,
    )
}

impl GameInstance {
    pub(super) fn apply_modern_actuators(&mut self) {
        if self.workspace_feature_counts.vector_forces == 0 {
            return;
        }

        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        for actuator in runtime.workspace().get_descendants() {
            let Some(vector_force) = ({
                let data = actuator.data.lock().unwrap();
                data.vector_force_data.clone()
            }) else {
                continue;
            };
            if !vector_force.enabled || !finite_vector(vector_force.force) {
                continue;
            }

            let Some(attachment0_ref) = vector_force
                .attachment0
                .as_ref()
                .and_then(|weak| weak.upgrade())
            else {
                continue;
            };
            let attachment0 = Instance::from_ref(attachment0_ref);
            let Some(parent_part) = attachment0.attachment_parent_part() else {
                continue;
            };
            let Some(attachment0_world) = attachment0.attachment_world_cframe() else {
                continue;
            };

            let world_force = match vector_force.relative_to {
                ActuatorRelativeTo::World => vector_force.force,
                ActuatorRelativeTo::Attachment0 => {
                    rotate_vector(attachment0_world, vector_force.force)
                }
                ActuatorRelativeTo::Attachment1 => {
                    let Some(attachment1_ref) = vector_force
                        .attachment1
                        .as_ref()
                        .and_then(|weak| weak.upgrade())
                    else {
                        continue;
                    };
                    let attachment1 = Instance::from_ref(attachment1_ref);
                    let Some(attachment1_world) = attachment1.attachment_world_cframe() else {
                        continue;
                    };
                    rotate_vector(attachment1_world, vector_force.force)
                }
            };

            let part_id = parent_part.id().0;
            if vector_force.apply_at_center_of_mass {
                self.physics
                    .add_force_to_part(part_id, world_force.to_array());
            } else {
                self.physics.add_force_at_position_to_part(
                    part_id,
                    world_force.to_array(),
                    attachment0_world.position.to_array(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::GameInstance;

    #[test]
    fn vector_force_accelerates_attachment_parent() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local p = Instance.new("Part")
            p.Name = "VectorForcePart"
            p.Size = Vector3.new(2, 2, 2)
            p.Position = Vector3.new(0, 10, 0)
            p.CanCollide = false
            p.Parent = Workspace

            local attachment = Instance.new("Attachment")
            attachment.Parent = p

            local force = Instance.new("VectorForce")
            force.Attachment0 = attachment
            force.RelativeTo = Enum.ActuatorRelativeTo.World
            force.ApplyAtCenterOfMass = true
            force.Force = Vector3.new(p:GetMass() * 20, 0, 0)
            force.Parent = p

            _G.p = p
        "#,
        );

        for _ in 0..30 {
            instance.tick();
        }

        let vx: f64 = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.p.AssemblyLinearVelocity.X")
            .eval()
            .unwrap();
        assert!(vx > 8.0, "VectorForce should accelerate parent, got {vx}");
    }
}
