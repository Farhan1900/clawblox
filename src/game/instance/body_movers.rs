use super::GameInstance;
use crate::game::lua::types::Vector3;

fn finite_vector(value: Vector3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn valid_limit_vector(value: Vector3) -> bool {
    !value.x.is_nan() && !value.y.is_nan() && !value.z.is_nan()
}

fn clamp_axis(value: f32, limit: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    if limit.is_infinite() {
        return value;
    }
    if !limit.is_finite() {
        return 0.0;
    }
    let limit = limit.abs();
    value.clamp(-limit, limit)
}

fn limited_pd_output(target: Vector3, current: [f32; 3], p: f32, limit: Vector3) -> [f32; 3] {
    if !finite_vector(target) || !valid_limit_vector(limit) || !p.is_finite() {
        return [0.0, 0.0, 0.0];
    }
    [
        clamp_axis((target.x - current[0]) * p, limit.x),
        clamp_axis((target.y - current[1]) * p, limit.y),
        clamp_axis((target.z - current[2]) * p, limit.z),
    ]
}

impl GameInstance {
    pub(super) fn apply_body_movers(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        for mover in runtime.workspace().get_descendants() {
            let (body_velocity, body_force, body_angular_velocity, parent) = {
                let data = mover.data.lock().unwrap();
                (
                    data.body_velocity_data.clone(),
                    data.body_force_data,
                    data.body_angular_velocity_data,
                    data.parent.as_ref().and_then(|weak| weak.upgrade()),
                )
            };

            let Some(parent) = parent.map(crate::game::lua::instance::Instance::from_ref) else {
                continue;
            };
            if !parent.is_a("BasePart") {
                continue;
            }
            let part_id = parent.id().0;

            if let Some(body_force) = body_force {
                if finite_vector(body_force.force) {
                    self.physics
                        .add_force_to_part(part_id, body_force.force.to_array());
                }
            }

            if let Some(body_velocity) = body_velocity {
                if let Some(current_velocity) =
                    self.physics.get_assembly_linear_velocity_for_part(part_id)
                {
                    let force = limited_pd_output(
                        body_velocity.velocity,
                        current_velocity,
                        body_velocity.p,
                        body_velocity.max_force,
                    );
                    self.physics.add_force_to_part(part_id, force);
                }
            }

            if let Some(body_angular_velocity) = body_angular_velocity {
                if let Some(current_angular_velocity) =
                    self.physics.get_assembly_angular_velocity_for_part(part_id)
                {
                    // Parity note: BodyAngularVelocity is deprecated in Roblox.
                    // This compatibility path preserves its observable target
                    // angular velocity behavior for existing content, but does
                    // not model the old solver's exact torque dynamics.
                    let target = body_angular_velocity.angular_velocity;
                    let max_torque = body_angular_velocity.max_torque;
                    if finite_vector(target)
                        && valid_limit_vector(max_torque)
                        && body_angular_velocity.p.is_finite()
                        && body_angular_velocity.p > 0.0
                    {
                        self.physics.set_assembly_angular_velocity_for_part(
                            part_id,
                            [
                                if max_torque.x == 0.0 {
                                    current_angular_velocity[0]
                                } else {
                                    target.x
                                },
                                if max_torque.y == 0.0 {
                                    current_angular_velocity[1]
                                } else {
                                    target.y
                                },
                                if max_torque.z == 0.0 {
                                    current_angular_velocity[2]
                                } else {
                                    target.z
                                },
                            ],
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::GameInstance;

    #[test]
    fn body_force_accelerates_parent_assembly() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local p = Instance.new("Part")
            p.Name = "ForcePart"
            p.Size = Vector3.new(2, 2, 2)
            p.Position = Vector3.new(0, 10, 0)
            p.CanCollide = false
            p.Parent = Workspace

            local force = Instance.new("BodyForce")
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
        assert!(vx > 8.0, "BodyForce should accelerate parent, got {vx}");
    }

    #[test]
    fn body_velocity_drives_welded_assembly() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local a = Instance.new("Part")
            a.Name = "BodyVelocityRoot"
            a.Size = Vector3.new(2, 2, 2)
            a.Position = Vector3.new(0, 10, 0)
            a.CanCollide = false
            a.Parent = Workspace

            local b = Instance.new("Part")
            b.Name = "BodyVelocityChild"
            b.Size = Vector3.new(2, 2, 2)
            b.Position = Vector3.new(3, 10, 0)
            b.CanCollide = false
            b.Parent = Workspace

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = a
            weld.Part1 = b
            weld.Parent = Workspace

            local bv = Instance.new("BodyVelocity")
            bv.Velocity = Vector3.new(12, 0, 0)
            bv.P = 1250
            bv.MaxForce = Vector3.new(100000, 0, 0)
            bv.Parent = a

            _G.a = a
            _G.b = b
        "#,
        );

        for _ in 0..20 {
            instance.tick();
        }

        let (a_x, b_x): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(
                r#"
                return _G.a.Position.X,
                    _G.b.Position.X
            "#,
            )
            .eval()
            .unwrap();

        assert!(a_x > 1.0, "root should move in target direction, got {a_x}");
        assert!(
            b_x > 4.0,
            "welded child should move with assembly, got {b_x}"
        );
    }

    #[test]
    fn body_angular_velocity_spins_parent_assembly() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local p = Instance.new("Part")
            p.Name = "SpinPart"
            p.Size = Vector3.new(2, 2, 2)
            p.Position = Vector3.new(0, 10, 0)
            p.CanCollide = false
            p.Parent = Workspace

            local bav = Instance.new("BodyAngularVelocity")
            bav.AngularVelocity = Vector3.new(0, 5, 0)
            bav.P = 1250
            bav.MaxTorque = Vector3.new(0, 100000, 0)
            bav.Parent = p

            _G.p = p
        "#,
        );

        for _ in 0..20 {
            instance.tick();
        }

        let wy: f64 = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.p.AssemblyAngularVelocity.Y")
            .eval()
            .unwrap();
        assert!(wy > 1.0, "BodyAngularVelocity should spin parent, got {wy}");
    }
}
