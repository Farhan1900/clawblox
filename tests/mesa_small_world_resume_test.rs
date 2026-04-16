use std::fs;
use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
use clawblox::game::script_bundle::load_world_and_entry_script;
use clawblox::snapshot::{capture_game_instance, restore_game_instance};
use uuid::Uuid;

fn mesa_small_world_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("mesa-small-world")
}

fn workspace_inventory(instance: &GameInstance) -> (i64, i64, i64, i64, i64, i64) {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            local blockCount = 0
            local block1Count = 0
            local groundCount = 0
            local peakCount = 0
            local spireCount = 0
            local ringCount = 0

            for _, child in ipairs(Workspace:GetChildren()) do
                if string.match(child.Name, "^Block_%d+$") then
                    blockCount = blockCount + 1
                end
                if child.Name == "Block_1" then
                    block1Count = block1Count + 1
                end
                if child.Name == "Ground" then
                    groundCount = groundCount + 1
                end
                if child.Name == "Peak" then
                    peakCount = peakCount + 1
                end
                if child.Name == "Spire" then
                    spireCount = spireCount + 1
                end
                if string.match(child.Name, "^Ring_%d+$") then
                    ringCount = ringCount + 1
                end
            end

            return blockCount, block1Count, groundCount, peakCount, spireCount, ringCount
        "#,
        )
        .eval()
        .expect("failed to read workspace inventory")
}

fn workspace_name_by_id(instance: &GameInstance, lua_id: u64) -> Option<String> {
    let runtime = instance.lua_runtime.as_ref()?;
    runtime
        .workspace()
        .get_descendants()
        .into_iter()
        .find(|inst| inst.id().0 == lua_id)
        .map(|inst| inst.name())
}

#[test]
fn mesa_small_world_resume_preserves_workspace_inventory() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let (_config, source) = load_world_and_entry_script(&mesa_small_world_dir())
        .expect("failed to load bundled mesa-small-world");
    instance.load_script(&source);

    for _ in 0..5 {
        instance.tick();
    }

    let before_restore = workspace_inventory(&instance);
    assert_eq!(
        before_restore,
        (12, 1, 1, 1, 1, 1),
        "unexpected fresh mesa-small-world inventory"
    );

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");

    for _ in 0..5 {
        restored.tick();
    }

    let after_restore = workspace_inventory(&restored);
    assert_eq!(
        after_restore, before_restore,
        "workspace inventory changed across restore: before={before_restore:?} after={after_restore:?}"
    );
}

#[test]
fn mesa_small_world_legacy_resume_rebinds_input_handlers() {
    let mut legacy_instance = GameInstance::new(Uuid::new_v4(), None);
    let raw_source =
        fs::read_to_string(mesa_small_world_dir().join("main.lua")).expect("failed to read main.lua");
    legacy_instance.load_script(&raw_source);

    let agent_id = Uuid::new_v4();
    assert!(legacy_instance.add_player(agent_id, "ResumeProbe"));
    for _ in 0..30 {
        legacy_instance.tick();
    }

    let snapshot = capture_game_instance(&legacy_instance).expect("failed to capture snapshot");
    let before_restore = workspace_inventory(&legacy_instance);
    assert_eq!(
        before_restore,
        (12, 1, 1, 1, 1, 1),
        "unexpected legacy mesa-small-world inventory"
    );
    let (_config, bundled_source) = load_world_and_entry_script(&mesa_small_world_dir())
        .expect("failed to load bundled mesa-small-world");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");
    restored.load_script(&bundled_source);

    for _ in 0..10 {
        restored.tick();
    }

    let after_restore = workspace_inventory(&restored);
    assert_eq!(
        after_restore, before_restore,
        "legacy resume should not duplicate workspace objects: before={before_restore:?} after={after_restore:?}"
    );

    let connection_count = restored
        .lua_runtime
        .as_ref()
        .expect("runtime missing")
        .agent_input_service()
        .data
        .lock()
        .unwrap()
        .input_received
        .connection_count();
    assert!(
        connection_count > 0,
        "legacy resume should recreate InputReceived connections"
    );

    let restored_hrp_id = *restored
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing restored humanoid root part");
    let restored_user_id = *restored
        .players
        .get(&agent_id)
        .expect("missing restored player user id");
    let before_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing restored player position");
    restored.queue_agent_input(
        restored_user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [20.0, before_move[1], 0.0]}),
    );

    restored.tick();

    let state_after_input = restored
        .physics
        .get_character_state(restored_hrp_id)
        .expect("missing character controller state after input");
    assert!(
        state_after_input.target_position.is_some(),
        "MoveTo target should be latched into controller state"
    );

    for _ in 0..119 {
        restored.tick();
    }

    let after_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing moved player position");
    let final_state = restored
        .physics
        .get_character_state(restored_hrp_id)
        .expect("missing final controller state");
    let grounded = final_state.grounded;
    let horizontal_velocity = final_state.horizontal_velocity;
    let final_target = final_state.target_position;
    let move_to_elapsed = final_state.move_to_elapsed;
    let horizontal_distance =
        ((after_move[0] - before_move[0]).powi(2) + (after_move[2] - before_move[2]).powi(2))
            .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "legacy resume should rebind MoveTo handlers; before={before_move:?} after={after_move:?} grounded={grounded} hv={horizontal_velocity:?} target={final_target:?} elapsed={move_to_elapsed}"
    );
}

#[test]
fn mesa_small_world_modern_resume_preserves_player_movement() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let (_config, bundled_source) = load_world_and_entry_script(&mesa_small_world_dir())
        .expect("failed to load bundled mesa-small-world");
    instance.load_script(&bundled_source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ResumeProbe"));

    for _ in 0..30 {
        instance.tick();
    }

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");

    for _ in 0..10 {
        restored.tick();
    }

    let restored_hrp_id = *restored
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing restored humanoid root part");
    let restored_user_id = *restored
        .players
        .get(&agent_id)
        .expect("missing restored player user id");
    let before_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing restored player position");

    restored.queue_agent_input(
        restored_user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [20.0, before_move[1], 0.0]}),
    );

    for _ in 0..120 {
        restored.tick();
    }

    let after_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing moved player position");
    let horizontal_distance =
        ((after_move[0] - before_move[0]).powi(2) + (after_move[2] - before_move[2]).powi(2))
            .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "modern resume should preserve movement; before={before_move:?} after={after_move:?}"
    );
}

#[test]
fn mesa_small_world_modern_resume_preserves_direct_controller_motion() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let (_config, bundled_source) = load_world_and_entry_script(&mesa_small_world_dir())
        .expect("failed to load bundled mesa-small-world");
    instance.load_script(&bundled_source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ResumeProbe"));

    for _ in 0..30 {
        instance.tick();
    }

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");

    for _ in 0..10 {
        restored.tick();
    }

    let restored_hrp_id = *restored
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing restored humanoid root part");
    let before_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing restored player position");

    restored.physics.set_character_target(
        restored_hrp_id,
        Some([20.0, before_move[1], 0.0]),
    );

    for _ in 0..120 {
        restored.tick();
    }

    let after_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing moved player position");
    let overlaps: Vec<String> = restored
        .physics
        .detect_overlaps()
        .into_iter()
        .filter(|(a, b)| *a == restored_hrp_id || *b == restored_hrp_id)
        .map(|(a, b)| {
            let other = if a == restored_hrp_id { b } else { a };
            let other_name =
                workspace_name_by_id(&restored, other).unwrap_or_else(|| format!("lua:{other}"));
            format!("{other}:{other_name}")
        })
        .collect();
    let horizontal_distance =
        ((after_move[0] - before_move[0]).powi(2) + (after_move[2] - before_move[2]).powi(2))
            .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "modern resume should preserve direct controller motion; before={before_move:?} after={after_move:?} overlaps={overlaps:?}"
    );
}

#[test]
fn mesa_small_world_fresh_direct_controller_motion_works() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let (_config, bundled_source) = load_world_and_entry_script(&mesa_small_world_dir())
        .expect("failed to load bundled mesa-small-world");
    instance.load_script(&bundled_source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ResumeProbe"));

    for _ in 0..30 {
        instance.tick();
    }

    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");
    let before_move = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing player position");

    instance
        .physics
        .set_character_target(hrp_id, Some([20.0, before_move[1], 0.0]));

    for _ in 0..120 {
        instance.tick();
    }

    let after_move = instance
        .physics
        .get_character_position(hrp_id)
        .expect("missing moved player position");
    let horizontal_distance =
        ((after_move[0] - before_move[0]).powi(2) + (after_move[2] - before_move[2]).powi(2))
            .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "fresh direct controller motion should work; before={before_move:?} after={after_move:?}"
    );
}

#[test]
fn generic_resume_preserves_direct_controller_motion() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(
        r#"
        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(100, 1, 100)
        floor.Position = Vector3.new(0, 0, 0)
        floor.Anchored = true
        floor.Parent = Workspace
    "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ResumeProbe"));

    for _ in 0..30 {
        instance.tick();
    }

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");

    for _ in 0..10 {
        restored.tick();
    }

    let restored_hrp_id = *restored
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing restored humanoid root part");
    let before_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing restored player position");

    restored
        .physics
        .set_character_target(restored_hrp_id, Some([20.0, before_move[1], 0.0]));

    for _ in 0..120 {
        restored.tick();
    }

    let after_move = restored
        .physics
        .get_character_position(restored_hrp_id)
        .expect("missing moved player position");
    let horizontal_distance =
        ((after_move[0] - before_move[0]).powi(2) + (after_move[2] - before_move[2]).powi(2))
            .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "generic resume should preserve direct controller motion; before={before_move:?} after={after_move:?}"
    );
}
