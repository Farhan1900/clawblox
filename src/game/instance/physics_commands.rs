use super::super::lua::runtime::PhysicsCommand;
use super::controller_runtime::character_part_mass;
use super::GameInstance;

impl GameInstance {
    pub(super) fn process_physics_commands(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        for command in runtime.game().drain_physics_commands() {
            match command {
                PhysicsCommand::ApplyImpulse { part_id, impulse }
                | PhysicsCommand::ApplyImpulseAtPosition {
                    part_id, impulse, ..
                } if self.physics.character_controllers.contains_key(&part_id) => {
                    // Characters are kinematic, so Rapier ignores impulses; feed them to the controller.
                    if let Some(mass) = character_part_mass(self, part_id).filter(|m| *m > 0.0) {
                        let dv = impulse.to_array().map(|v| v / mass);
                        self.physics.add_character_velocity(part_id, dv);
                    }
                }
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

    fn character_world(gravity_multiplier: Option<f32>) -> GameInstance {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        if let Some(m) = gravity_multiplier {
            let mut config = crate::config::PhysicsConfig::default();
            config.character_gravity_multiplier = m;
            instance.set_physics_config(&config);
        }
        instance.load_script(
            r#"
            Workspace.Gravity = 10
            local floor = Instance.new("Part")
            floor.Size = Vector3.new(400, 2, 400)
            floor.Position = Vector3.new(0, -1, 0)
            floor.Anchored = true
            floor.Parent = Workspace
            game:GetService("Players").PlayerAdded:Connect(function(p) _G.player = p end)
        "#,
        );
        assert!(instance.add_player(Uuid::new_v4(), "Subject"));
        instance
    }

    fn root(instance: &GameInstance) -> (f64, f64) {
        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("local p = _G.player.Character.HumanoidRootPart.Position return p.X, p.Y")
            .eval()
            .unwrap()
    }

    #[test]
    fn impulse_on_character_root_shoves_it_and_the_shove_fades() {
        let mut instance = character_world(None);
        for _ in 0..120 {
            instance.tick();
        }
        let (x0, _) = root(&instance);
        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("local r = _G.player.Character.HumanoidRootPart r:ApplyImpulse(Vector3.new(r:GetMass() * 12, 0, 0))")
            .exec()
            .unwrap();
        for _ in 0..60 {
            instance.tick();
        }
        let (x1, _) = root(&instance);
        for _ in 0..60 {
            instance.tick();
        }
        let (x2, _) = root(&instance);
        assert!(x1 - x0 > 0.5, "a 12 u/s shove should move the character: {x0} -> {x1}");
        assert!(x2 - x1 < 0.05, "the shove should have faded on the ground: {x1} -> {x2}");
    }

    #[test]
    fn character_gravity_multiplier_comes_from_physics_config() {
        let mut heavy = character_world(None);
        let mut light = character_world(Some(2.0));
        for _ in 0..12 {
            heavy.tick();
            light.tick();
        }
        let (_, heavy_y) = root(&heavy);
        let (_, light_y) = root(&light);
        assert!(light_y > heavy_y + 1.0, "low character gravity should fall slower: {light_y} vs {heavy_y}");
    }

    #[test]
    fn character_capsule_size_comes_from_physics_config() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        let mut config = crate::config::PhysicsConfig::default();
        config.character_radius = 0.35;
        config.character_height = 1.8;
        instance.set_physics_config(&config);
        instance.load_script(
            r#"
            Workspace.Gravity = 10
            local floor = Instance.new("Part")
            floor.Size = Vector3.new(400, 2, 400)
            floor.Position = Vector3.new(0, -1, 0)
            floor.Anchored = true
            floor.Parent = Workspace
            game:GetService("Players").PlayerAdded:Connect(function(p) _G.player = p end)
        "#,
        );
        assert!(instance.add_player(Uuid::new_v4(), "Small"));
        for _ in 0..180 {
            instance.tick();
        }
        let (size_x, size_y, y): (f64, f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("local r = _G.player.Character.HumanoidRootPart return r.Size.X, r.Size.Y, r.Position.Y")
            .eval()
            .unwrap();
        assert!((size_x - 0.7).abs() < 1e-4 && (size_y - 1.8).abs() < 1e-4, "part size {size_x} x {size_y}");
        assert!((y - 0.9).abs() < 0.15, "a 1.8 m capsule should rest with its centre about 0.9 m up: {y}");
    }
}
