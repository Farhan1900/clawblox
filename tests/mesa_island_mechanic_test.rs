use std::fs;
use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
use clawblox::snapshot::{capture_game_instance, restore_game_instance};
use uuid::Uuid;

fn mesa_island_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("mesa-island")
        .join("main.lua")
}

#[test]
fn mesa_island_coated_block_bond_moves_as_one_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    for _ in 0..10 {
        instance.tick();
    }

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new(260, 50, 0)
                target.Position = Vector3.new(260.74, 50, 0)
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")

                _G.block = block
                _G.target = target
            "#,
            )
            .exec()
            .expect("failed to set up mesa-island bond scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    let (phase, has_bond) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        let phase: String = runtime
            .lua()
            .load(
                r#"
                return game:GetService("ServerStorage").MesaIslandState.Blocks.Block_1:GetAttribute("CoatingPhase")
            "#,
            )
            .eval()
            .expect("failed to read coating phase");
        let has_bond: bool = runtime
            .lua()
            .load(r#"return _G.block:FindFirstChild("SkuttlerBond") ~= nil"#)
            .eval()
            .expect("failed to check bond existence");
        (phase, has_bond)
    };

    assert_eq!(phase, "Set", "coated block should set once bonding succeeds");
    assert!(has_bond, "mesa-island should create the residue bond weld");

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(r#"_G.target.Position = Vector3.new(268, 50, 0)"#)
            .exec()
            .expect("failed to teleport target block");
    }

    for _ in 0..60 {
        instance.tick();
    }

    let (block_x, target_x) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        let block_x: f64 = runtime
            .lua()
            .load(r#"return _G.block.Position.X"#)
            .eval()
            .expect("failed to read bonded block position");
        let target_x: f64 = runtime
            .lua()
            .load(r#"return _G.target.Position.X"#)
            .eval()
            .expect("failed to read target block position");
        (block_x, target_x)
    };
    let spacing = target_x - block_x;

    assert!(
        block_x > 263.0,
        "moving the target block should drag the coated block in mesa-island; block_x={block_x}, target_x={target_x}"
    );
    assert!(
        (spacing - 0.74).abs() < 0.2,
        "bonded mesa-island blocks should preserve spacing; block_x={block_x}, target_x={target_x}, spacing={spacing}"
    );
}

#[test]
fn mesa_island_pickup_keeps_bonded_blocks_attached() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");
    for _ in 0..10 {
        instance.tick();
    }

    let hrp_pos = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");
    let block_x = hrp_pos[0] + 2.0;
    let block_y = hrp_pos[1];
    let block_z = hrp_pos[2];

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new({block_x}, {block_y}, {block_z})
                target.Position = Vector3.new({}, {block_y}, {block_z})
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")

                _G.block = block
                _G.target = target
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure pickup bond scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [block_x, block_y, block_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    let (pickup_result, still_bonded) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        let pickup_result: String = runtime
            .lua()
            .load(
                r#"
                return game:GetService("Players"):GetPlayers()[1]:GetAttribute("LastActionResult")
            "#,
            )
            .eval()
            .expect("failed to read pickup result");
        let still_bonded: bool = runtime
            .lua()
            .load(r#"return _G.block:FindFirstChild("SkuttlerBond") ~= nil"#)
            .eval()
            .expect("failed to check bond after pickup");
        (pickup_result, still_bonded)
    };

    assert_eq!(pickup_result, "PickupSuccess");
    assert!(still_bonded, "pickup should not strip the mesa-island bond");

    instance
        .physics
        .set_character_position(hrp_id, [block_x + 8.0, block_y, block_z]);

    for _ in 0..90 {
        instance.tick();
    }

    let (held_x, target_x) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        let held_x: f64 = runtime
            .lua()
            .load(r#"return _G.block.Position.X"#)
            .eval()
            .expect("failed to read held block position");
        let target_x: f64 = runtime
            .lua()
            .load(r#"return _G.target.Position.X"#)
            .eval()
            .expect("failed to read bonded target position");
        (held_x, target_x)
    };
    let spacing = target_x - held_x;

    assert!(
        held_x > (block_x as f64) + 4.0,
        "held block should move with the player; held_x={held_x}, initial_x={block_x}"
    );
    assert!(
        target_x > (block_x as f64) + 4.0,
        "bonded block should stay attached during pickup movement; target_x={target_x}, initial_x={block_x}"
    );
    assert!(
        (spacing - 0.74).abs() < 0.25,
        "pickup should preserve bonded spacing; held_x={held_x}, target_x={target_x}, spacing={spacing}"
    );
}

#[test]
fn mesa_island_place_checks_full_bonded_assembly_occupancy() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");
    let player_state_key = user_id.to_string();

    for _ in 0..10 {
        instance.tick();
    }

    let hrp_pos = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");
    let block_x = hrp_pos[0] + 2.0;
    let block_y = hrp_pos[1];
    let block_z = hrp_pos[2];
    let place_x = hrp_pos[0] + 4.0;

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new({block_x}, {block_y}, {block_z})
                target.Position = Vector3.new({}, {block_y}, {block_z})
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure bonded placement scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [block_x, block_y, block_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                local blocker = Instance.new("Part")
                blocker.Name = "PartnerBlocker"
                blocker.Size = Vector3.new(0.75, 0.75, 0.75)
                blocker.Anchored = true
                blocker.CanCollide = true
                blocker.Position = Vector3.new({}, {block_y}, {block_z})
                blocker.Parent = Workspace
            "#,
                place_x + 0.74
            ))
            .exec()
            .expect("failed to create blocker at bonded partner position");
    }

    instance.queue_agent_input(
        user_id,
        "Place".to_string(),
        serde_json::json!({ "position": [place_x, block_y, block_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    let (place_result, still_holding): (String, bool) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                local player = game:GetService("Players"):GetPlayers()[1]
                local heldValue = game:GetService("ServerStorage").MesaIslandState.Players["{}"].HeldObject
                return player:GetAttribute("LastActionResult"), heldValue.Value ~= nil
            "#,
                player_state_key
            ))
            .eval()
            .expect("failed to inspect occupied bonded placement result")
    };

    assert_eq!(place_result, "PlaceFailed:Occupied");
    assert!(still_holding, "occupied placement should keep the bonded assembly in hand");
}

#[test]
fn mesa_island_rotated_bonded_placement_rotates_the_full_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");

    for _ in 0..10 {
        instance.tick();
    }

    let hrp_pos = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");
    let block_x = hrp_pos[0] + 2.0;
    let block_y = hrp_pos[1];
    let block_z = hrp_pos[2];
    let place_x = hrp_pos[0] + 4.0;
    let place_z = hrp_pos[2];

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new({block_x}, {block_y}, {block_z})
                target.Position = Vector3.new({}, {block_y}, {block_z})
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure rotated placement scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [block_x, block_y, block_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    instance.queue_agent_input(
        user_id,
        "Place".to_string(),
        serde_json::json!({
            "position": [place_x, block_y, place_z],
            "rotation": { "yaw": 90.0, "pitch": 0.0, "roll": 0.0 }
        }),
    );

    for _ in 0..20 {
        instance.tick();
    }

    let (place_result, block_x_after, block_z_after, target_x_after, target_z_after, still_bonded): (
        String,
        f64,
        f64,
        f64,
        f64,
        bool,
    ) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local player = game:GetService("Players"):GetPlayers()[1]
                local block = Workspace.Block_1
                local target = Workspace.Block_2
                return player:GetAttribute("LastActionResult"),
                    block.Position.X, block.Position.Z,
                    target.Position.X, target.Position.Z,
                    block:FindFirstChild("SkuttlerBond") ~= nil
            "#,
            )
            .eval()
            .expect("failed to inspect rotated bonded placement")
    };

    let root_dx = (block_x_after - place_x as f64).abs();
    let root_dz = (block_z_after - place_z as f64).abs();
    let partner_dx = (target_x_after - block_x_after).abs();
    let partner_dz = (target_z_after - block_z_after).abs();

    assert_eq!(place_result, "PlaceSuccess");
    assert!(still_bonded, "rotated placement should preserve the bonded assembly");
    assert!(
        root_dx < 0.25 && root_dz < 0.25,
        "placed root block should land near the requested root position; got ({block_x_after}, {block_z_after}) vs ({place_x}, {place_z})"
    );
    assert!(
        partner_dx < 0.2 && (partner_dz - 0.74).abs() < 0.25,
        "rotated placement should swing the bonded partner around the root; root=({block_x_after}, {block_z_after}), partner=({target_x_after}, {target_z_after})"
    );
}

#[test]
fn mesa_island_rotated_bonded_placement_checks_rotated_partner_occupancy() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");
    let player_state_key = user_id.to_string();

    for _ in 0..10 {
        instance.tick();
    }

    let hrp_pos = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");
    let block_x = hrp_pos[0] + 2.0;
    let block_y = hrp_pos[1];
    let block_z = hrp_pos[2];
    let place_x = hrp_pos[0] + 4.0;
    let place_z = hrp_pos[2];

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new({block_x}, {block_y}, {block_z})
                target.Position = Vector3.new({}, {block_y}, {block_z})
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure rotated occupied placement scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [block_x, block_y, block_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                local blocker_a = Instance.new("Part")
                blocker_a.Name = "RotatedPartnerBlockerA"
                blocker_a.Size = Vector3.new(0.75, 0.75, 0.75)
                blocker_a.Anchored = true
                blocker_a.CanCollide = true
                blocker_a.Position = Vector3.new({place_x}, {block_y}, {})
                blocker_a.Parent = Workspace

                local blocker_b = Instance.new("Part")
                blocker_b.Name = "RotatedPartnerBlockerB"
                blocker_b.Size = Vector3.new(0.75, 0.75, 0.75)
                blocker_b.Anchored = true
                blocker_b.CanCollide = true
                blocker_b.Position = Vector3.new({place_x}, {block_y}, {})
                blocker_b.Parent = Workspace
            "#,
                place_z + 0.74,
                place_z - 0.74
            ))
            .exec()
            .expect("failed to create blockers at the rotated partner positions");
    }

    instance.queue_agent_input(
        user_id,
        "Place".to_string(),
        serde_json::json!({
            "position": [place_x, block_y, place_z],
            "rotation": { "yaw": 90.0, "pitch": 0.0, "roll": 0.0 }
        }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    let (place_result, still_holding): (String, bool) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                local player = game:GetService("Players"):GetPlayers()[1]
                local heldValue = game:GetService("ServerStorage").MesaIslandState.Players["{}"].HeldObject
                return player:GetAttribute("LastActionResult"), heldValue.Value ~= nil
            "#,
                player_state_key
            ))
            .eval()
            .expect("failed to inspect rotated occupied placement result")
    };

    assert_eq!(place_result, "PlaceFailed:Occupied");
    assert!(still_holding, "rotated occupied placement should keep the bonded assembly in hand");
}

#[test]
fn mesa_island_plain_block_on_set_block_bonds_and_pickup_moves_both() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");

    for _ in 0..10 {
        instance.tick();
    }

    let hrp_pos = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");
    let base_x = hrp_pos[0] + 2.0;
    let base_y = hrp_pos[1];
    let base_z = hrp_pos[2];

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(&format!(
                r#"
                Workspace.Gravity = 0

                local sticky = Workspace.Block_1
                local plain = Workspace.Block_2
                sticky.Position = Vector3.new({base_x}, {base_y}, {base_z})
                plain.Position = Vector3.new({}, {base_y}, {base_z})
                sticky.CanCollide = false
                plain.CanCollide = false
                sticky.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                plain.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")

                _G.sticky = sticky
                _G.plain = plain
            "#,
                base_x + 0.74
            ))
            .exec()
            .expect("failed to configure set-target bond scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    let (sticky_phase, sticky_has_bond): (String, bool) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local phase = game:GetService("ServerStorage").MesaIslandState.Blocks.Block_1:GetAttribute("CoatingPhase")
                return phase, _G.sticky:FindFirstChild("SkuttlerBond") ~= nil
            "#,
            )
            .eval()
            .expect("failed to inspect set-target bond state")
    };

    assert_eq!(sticky_phase, "Set");
    assert!(sticky_has_bond, "sticky block should own the bond when a plain block attaches to it");

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [base_x + 0.74, base_y, base_z] }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    instance
        .physics
        .set_character_position(hrp_id, [base_x + 8.0, base_y, base_z]);

    for _ in 0..90 {
        instance.tick();
    }

    let (sticky_x, plain_x): (f64, f64) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                return _G.sticky.Position.X, _G.plain.Position.X
            "#,
            )
            .eval()
            .expect("failed to read bonded movement after pickup")
    };

    assert!(
        sticky_x > (base_x as f64) + 4.0,
        "sticky set block should move when the plain bonded block is picked up; sticky_x={sticky_x}, initial_x={base_x}"
    );
    assert!(
        plain_x > (base_x as f64) + 4.0,
        "picked-up plain block should move with the player; plain_x={plain_x}, initial_x={base_x}"
    );
}

#[test]
fn mesa_island_out_of_bounds_bonded_blocks_reset_to_spawn_and_clear_bonds() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");
    instance.load_script(&source);

    for _ in 0..10 {
        instance.tick();
    }

    let (spawn1x, spawn1y, spawn1z, spawn2x, spawn2y, spawn2z): (f64, f64, f64, f64, f64, f64) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local b1 = Workspace.Block_1.Position
                local b2 = Workspace.Block_2.Position
                return b1.X, b1.Y, b1.Z, b2.X, b2.Y, b2.Z
            "#,
            )
            .eval()
            .expect("failed to read initial block positions")
    };

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new(260, 10, 0)
                target.Position = Vector3.new(260.74, 10, 0)
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
            )
            .exec()
            .expect("failed to set up bonded reset scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                Workspace.Block_1.Position = Vector3.new(260, -205, 0)
                Workspace.Block_2.Position = Vector3.new(260.74, -205, 0)
            "#,
            )
            .exec()
            .expect("failed to drop bonded blocks out of bounds");
    }

    for _ in 0..10 {
        instance.tick();
    }

    let (block1x, block1y, block1z, block2x, block2y, block2z, phase, still_bonded): (
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        String,
        bool,
    ) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local b1 = Workspace.Block_1
                local b2 = Workspace.Block_2
                local phase = game:GetService("ServerStorage").MesaIslandState.Blocks.Block_1:GetAttribute("CoatingPhase")
                local stillBonded = b1:FindFirstChild("SkuttlerBond") ~= nil
                return b1.Position.X, b1.Position.Y, b1.Position.Z,
                    b2.Position.X, b2.Position.Y, b2.Position.Z,
                    phase, stillBonded
            "#,
            )
            .eval()
            .expect("failed to inspect reset bonded blocks")
    };

    let dx1 = (block1x - spawn1x).abs();
    let dy1 = (block1y - spawn1y).abs();
    let dz1 = (block1z - spawn1z).abs();
    let dx2 = (block2x - spawn2x).abs();
    let dy2 = (block2y - spawn2y).abs();
    let dz2 = (block2z - spawn2z).abs();

    assert!(
        dx1 < 0.25 && dy1 < 0.75 && dz1 < 0.25,
        "Block_1 should reset near spawn; got ({block1x}, {block1y}, {block1z}) vs ({spawn1x}, {spawn1y}, {spawn1z})"
    );
    assert!(
        dx2 < 0.25 && dy2 < 0.75 && dz2 < 0.25,
        "Block_2 should reset near spawn; got ({block2x}, {block2y}, {block2z}) vs ({spawn2x}, {spawn2y}, {spawn2z})"
    );
    assert_eq!(phase, "Bare", "reset should clear the coating/bond treatment");
    assert!(!still_bonded, "reset should remove residue bond welds");
}

#[test]
fn mesa_island_resume_rebuilds_bond_groups_for_out_of_bounds_reset() {
    let source = fs::read_to_string(mesa_island_script_path()).expect("failed to read mesa-island");

    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(&source);

    for _ in 0..10 {
        instance.tick();
    }

    let (spawn1x, spawn1y, spawn1z, spawn2x, spawn2y, spawn2z): (f64, f64, f64, f64, f64, f64) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local b1 = Workspace.Block_1.Position
                local b2 = Workspace.Block_2.Position
                return b1.X, b1.Y, b1.Z, b2.X, b2.Y, b2.Z
            "#,
            )
            .eval()
            .expect("failed to read initial block positions")
    };

    {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                Workspace.Gravity = 0

                local block = Workspace.Block_1
                local target = Workspace.Block_2
                block.Position = Vector3.new(260, 10, 0)
                target.Position = Vector3.new(260.74, 10, 0)
                block.CanCollide = false
                target.CanCollide = false
                block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                target.AssemblyLinearVelocity = Vector3.new(0, 0, 0)

                game:GetService("ServerStorage")
                    .MesaIslandState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
            )
            .exec()
            .expect("failed to set up bonded resume scenario");
    }

    for _ in 0..30 {
        instance.tick();
    }

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");
    restored.load_script(&source);

    for _ in 0..5 {
        restored.tick();
    }

    {
        let runtime = restored.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                Workspace.Block_1.Position = Vector3.new(260, -205, 0)
                Workspace.Block_2.Position = Vector3.new(260.74, -205, 0)
            "#,
            )
            .exec()
            .expect("failed to drop restored bonded blocks out of bounds");
    }

    for _ in 0..10 {
        restored.tick();
    }

    let (block1x, block1y, block1z, block2x, block2y, block2z, phase, still_bonded): (
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        String,
        bool,
    ) = {
        let runtime = restored.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local b1 = Workspace.Block_1
                local b2 = Workspace.Block_2
                local phase = game:GetService("ServerStorage").MesaIslandState.Blocks.Block_1:GetAttribute("CoatingPhase")
                local stillBonded = b1:FindFirstChild("SkuttlerBond") ~= nil
                return b1.Position.X, b1.Position.Y, b1.Position.Z,
                    b2.Position.X, b2.Position.Y, b2.Position.Z,
                    phase, stillBonded
            "#,
            )
            .eval()
            .expect("failed to inspect restored reset bonded blocks")
    };

    let dx1 = (block1x - spawn1x).abs();
    let dy1 = (block1y - spawn1y).abs();
    let dz1 = (block1z - spawn1z).abs();
    let dx2 = (block2x - spawn2x).abs();
    let dy2 = (block2y - spawn2y).abs();
    let dz2 = (block2z - spawn2z).abs();

    assert!(
        dx1 < 0.25 && dy1 < 0.75 && dz1 < 0.25,
        "restored Block_1 should reset near spawn; got ({block1x}, {block1y}, {block1z}) vs ({spawn1x}, {spawn1y}, {spawn1z})"
    );
    assert!(
        dx2 < 0.25 && dy2 < 0.75 && dz2 < 0.25,
        "restored Block_2 should reset near spawn; got ({block2x}, {block2y}, {block2z}) vs ({spawn2x}, {spawn2y}, {spawn2z})"
    );
    assert_eq!(phase, "Bare", "restored reset should clear the coating/bond treatment");
    assert!(!still_bonded, "restored reset should remove residue bond welds");
}
