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
                    self.physics
                        .apply_impulse_to_part(part_id, impulse.to_array());
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

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::GameInstance;

    #[test]
    fn test_apply_impulse_acts_on_assembly_center_of_mass() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);

        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local p = Instance.new("Part")
            p.Name = "ImpulsePart"
            p.Size = Vector3.new(2, 2, 2)
            p.Position = Vector3.new(0, 10, 0)
            p.CanCollide = false
            p.Parent = Workspace

            p:ApplyImpulse(Vector3.new(p:GetMass() * 10, 0, 0))
            _G.p = p
        "#,
        );

        for _ in 0..3 {
            instance.tick();
        }

        let runtime = instance.lua_runtime.as_ref().unwrap();
        let (vx, vy, vz, angular): (f64, f64, f64, f64) = runtime
            .lua()
            .load(
                r#"
                return _G.p.AssemblyLinearVelocity.X,
                    _G.p.AssemblyLinearVelocity.Y,
                    _G.p.AssemblyLinearVelocity.Z,
                    _G.p.AssemblyAngularVelocity.Magnitude
            "#,
            )
            .eval()
            .unwrap();

        assert!((vx - 10.0).abs() < 0.05, "expected vx ~= 10, got {vx}");
        assert!(vy.abs() < 0.05, "expected vy ~= 0, got {vy}");
        assert!(vz.abs() < 0.05, "expected vz ~= 0, got {vz}");
        assert!(
            angular < 0.05,
            "center-of-mass impulse should not spin: {angular}"
        );
    }

    #[test]
    fn test_apply_impulse_at_position_can_spin_welded_assembly() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);

        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local a = Instance.new("Part")
            a.Name = "ImpulseRoot"
            a.Size = Vector3.new(2, 2, 2)
            a.Position = Vector3.new(0, 10, 0)
            a.CanCollide = false
            a.Parent = Workspace

            local b = Instance.new("Part")
            b.Name = "ImpulseChild"
            b.Size = Vector3.new(2, 2, 2)
            b.Position = Vector3.new(3, 10, 0)
            b.CanCollide = false
            b.Parent = Workspace

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = a
            weld.Part1 = b
            weld.Parent = Workspace

            a:ApplyImpulseAtPosition(Vector3.new(0, 0, 100), a.Position + Vector3.new(0, 3, 0))
            _G.a = a
            _G.b = b
        "#,
        );

        for _ in 0..5 {
            instance.tick();
        }

        let runtime = instance.lua_runtime.as_ref().unwrap();
        let (connected_count, angular): (i64, f64) = runtime
            .lua()
            .load(
                r#"
                return #_G.a:GetConnectedParts(true),
                    _G.a.AssemblyAngularVelocity.Magnitude
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(connected_count, 1);
        assert!(
            angular > 0.01,
            "off-center impulse should create angular velocity"
        );
    }

    #[test]
    fn test_can_collide_toggle_preserves_welded_assembly_body() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);

        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local a = Instance.new("Part")
            a.Name = "CollisionToggleRoot"
            a.Size = Vector3.new(2, 2, 2)
            a.Position = Vector3.new(0, 10, 0)
            a.CanCollide = false
            a.Parent = Workspace

            local b = Instance.new("Part")
            b.Name = "CollisionToggleChild"
            b.Size = Vector3.new(2, 2, 2)
            b.Position = Vector3.new(3, 10, 0)
            b.CanCollide = false
            b.Parent = Workspace

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = a
            weld.Part1 = b
            weld.Parent = Workspace

            a:ApplyImpulse(Vector3.new((a:GetMass() + b:GetMass()) * 10, 0, 0))
            _G.a = a
            _G.b = b
        "#,
        );

        for _ in 0..3 {
            instance.tick();
        }

        let runtime = instance.lua_runtime.as_ref().unwrap();
        let root_id = runtime
            .workspace()
            .get_descendants()
            .into_iter()
            .find_map(|inst| {
                let data = inst.data.lock().unwrap();
                (data.name == "CollisionToggleRoot").then_some(data.id.0)
            })
            .expect("root part missing");
        let before_handle = *instance
            .physics
            .lua_to_body
            .get(&root_id)
            .expect("root physics body missing");

        runtime
            .lua()
            .load(
                r#"
                _G.a.CanCollide = true
                _G.b.CanCollide = true
            "#,
            )
            .exec()
            .unwrap();

        for _ in 0..3 {
            instance.tick();
        }

        let after_handle = *instance
            .physics
            .lua_to_body
            .get(&root_id)
            .expect("root physics body missing after CanCollide toggle");
        assert_eq!(
            before_handle, after_handle,
            "interaction flag changes should update colliders without rebuilding the assembly body"
        );
    }
}
