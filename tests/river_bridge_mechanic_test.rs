use std::fs;
use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
use uuid::Uuid;

fn river_bridge_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("mesa-island")
        .join("eval-worlds")
        .join("river-bridge")
        .join("main.lua")
}

#[test]
fn river_bridge_pickup_moves_bonded_blocks_as_one_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(river_bridge_script_path()).expect("failed to read river-bridge");
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
                    .ConstructionTestState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")

                _G.block = block
                _G.target = target
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure river-bridge pickup scenario");
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

    let (pickup_result, root_y, partner_y, still_bonded) = {
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
        let root_y: f64 = runtime
            .lua()
            .load(r#"return _G.block.Position.Y"#)
            .eval()
            .expect("failed to read root block position");
        let partner_y: f64 = runtime
            .lua()
            .load(r#"return _G.target.Position.Y"#)
            .eval()
            .expect("failed to read partner block position");
        let still_bonded: bool = runtime
            .lua()
            .load(r#"return _G.block:FindFirstChild("SkuttlerBond") ~= nil"#)
            .eval()
            .expect("failed to check bond after pickup");
        (pickup_result, root_y, partner_y, still_bonded)
    };

    assert_eq!(pickup_result, "PickupSuccess");
    assert!(still_bonded, "pickup should preserve the residue bond");
    assert!(
        root_y > f64::from(block_y) + 2.0,
        "picked-up root block should lift off the ground; root_y={root_y}, original_y={block_y}"
    );
    assert!(
        partner_y > f64::from(block_y) + 2.0,
        "bonded partner should lift with the root block; partner_y={partner_y}, original_y={block_y}"
    );
}

#[test]
fn river_bridge_place_allows_ground_contact_for_bonded_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(river_bridge_script_path()).expect("failed to read river-bridge");
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
    let place_x = block_x + 2.0;
    let ground_contact_y = 0.37f32;

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
                    .ConstructionTestState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")

                _G.block = block
                _G.target = target
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure river-bridge place scenario");
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
            "position": [place_x, ground_contact_y, block_z],
            "rotation": { "pitch": 0, "roll": 0, "yaw": 0 }
        }),
    );

    for _ in 0..25 {
        instance.tick();
    }

    let (place_result, root_y, partner_y, spacing, still_bonded) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        let place_result: String = runtime
            .lua()
            .load(
                r#"
                return game:GetService("Players"):GetPlayers()[1]:GetAttribute("LastActionResult")
            "#,
            )
            .eval()
            .expect("failed to read place result");
        let root_y: f64 = runtime
            .lua()
            .load(r#"return _G.block.Position.Y"#)
            .eval()
            .expect("failed to read root block Y");
        let partner_y: f64 = runtime
            .lua()
            .load(r#"return _G.target.Position.Y"#)
            .eval()
            .expect("failed to read partner block Y");
        let spacing: f64 = runtime
            .lua()
            .load(r#"return (_G.block.Position - _G.target.Position).Magnitude"#)
            .eval()
            .expect("failed to read block spacing");
        let still_bonded: bool = runtime
            .lua()
            .load(r#"return _G.block:FindFirstChild("SkuttlerBond") ~= nil"#)
            .eval()
            .expect("failed to check bond after place");
        (place_result, root_y, partner_y, spacing, still_bonded)
    };

    assert_eq!(place_result, "PlaceSuccess");
    assert!(still_bonded, "placed assembly should remain bonded");
    assert!(
        ((root_y - 0.375) - f64::from(ground_contact_y)).abs() < 0.2,
        "root block should be placeable resting on the requested support height; root_y={root_y}, support_y={ground_contact_y}"
    );
    assert!(
        ((partner_y - 0.375) - f64::from(ground_contact_y)).abs() < 0.2,
        "bonded partner should also rest on the requested support height; partner_y={partner_y}, support_y={ground_contact_y}"
    );
    assert!(
        (spacing - 0.74).abs() < 0.1,
        "placed bonded blocks should preserve spacing; spacing={spacing}"
    );
}

#[test]
fn river_bridge_place_rejects_overlap_with_player() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(river_bridge_script_path()).expect("failed to read river-bridge");
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
                    .ConstructionTestState
                    .Blocks
                    .Block_1
                    :SetAttribute("CoatingPhase", "Coated")
            "#,
                block_x + 0.74
            ))
            .exec()
            .expect("failed to configure river-bridge player overlap scenario");
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
            "position": [hrp_pos[0], 0.37f32, hrp_pos[2]],
            "rotation": { "pitch": 0, "roll": 0, "yaw": 0 }
        }),
    );

    for _ in 0..10 {
        instance.tick();
    }

    let place_result: String = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                return game:GetService("Players"):GetPlayers()[1]:GetAttribute("LastActionResult")
            "#,
            )
            .eval()
            .expect("failed to read place result")
    };

    assert_eq!(place_result, "PlaceFailed:Occupied");
}
