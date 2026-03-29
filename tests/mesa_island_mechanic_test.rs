use std::fs;
use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
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
