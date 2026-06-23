use std::path::PathBuf;
use std::time::{Duration, Instant};

use clawblox::game::instance::{ErrorMode, GameInstance};
use clawblox::game::script_bundle::load_world_and_entry_script;
use clawblox::snapshot::{capture_game_instance, restore_game_instance};
use uuid::Uuid;

fn pyramid_world_3_dir() -> PathBuf {
    std::env::var_os("PYRAMID_WORLD_3_DIR")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn configured_instance() -> Option<(GameInstance, String)> {
    let world_dir = pyramid_world_3_dir();
    if world_dir.as_os_str().is_empty() {
        eprintln!("skipping pyramid-world-3 exact resume test; PYRAMID_WORLD_3_DIR is not set");
        return None;
    }
    if !world_dir.is_dir() {
        eprintln!(
            "skipping pyramid-world-3 exact resume test; {} is not present",
            world_dir.display()
        );
        return None;
    }

    let (config, source) =
        load_world_and_entry_script(&world_dir).expect("failed to load pyramid-world-3");
    let mut instance = GameInstance::new_with_config(
        Uuid::new_v4(),
        config.max_players,
        config.observation.radius,
        None,
        ErrorMode::Halt,
    );
    instance.set_player_object_physics(config.clawblox.player_object_physics);
    instance.set_physics_config(&config.clawblox.physics);
    if config.afk_timeout == 0 {
        instance.set_afk_timeout(Duration::from_secs(u64::MAX));
    } else {
        instance.set_afk_timeout(Duration::from_secs(config.afk_timeout));
    }

    Some((instance, source))
}

fn workspace_inventory(instance: &GameInstance) -> (i64, i64, i64) {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            local blocks = 0
            local planks = 0
            local pickupObjects = 0

            for _, child in ipairs(Workspace:GetChildren()) do
                if string.match(child.Name, "^Block_%d+$") then
                    blocks = blocks + 1
                end
                if string.match(child.Name, "^Plank_%d+$") then
                    planks = planks + 1
                end
                if child:IsA("Part") and child:GetAttribute("IsPickupObject") then
                    pickupObjects = pickupObjects + 1
                end
            end

            return blocks, planks, pickupObjects
        "#,
        )
        .eval()
        .expect("failed to read workspace inventory")
}

fn first_pickup_object(instance: &GameInstance) -> (String, f64, f64, f64) {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            for _, child in ipairs(Workspace:GetChildren()) do
                if child:IsA("Part") and child:GetAttribute("IsPickupObject") then
                    return child.Name, child.Position.X, child.Position.Y, child.Position.Z
                end
            end
            error("no pickup object found")
        "#,
        )
        .eval()
        .expect("failed to find pickup object")
}

fn block_position(instance: &GameInstance, name: &str) -> (f64, f64, f64, bool, bool) {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            local name = ...
            local block = Workspace:FindFirstChild(name)
            assert(block ~= nil, "block missing")
            return block.Position.X, block.Position.Y, block.Position.Z, block.CanCollide, block.Anchored
        "#,
        )
        .call(name)
        .expect("failed to read block position")
}

fn player_action_result(instance: &GameInstance) -> String {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            local player = game:GetService("Players"):GetPlayers()[1]
            assert(player ~= nil, "player missing")
            return player:GetAttribute("LastActionResult")
        "#,
        )
        .eval()
        .expect("failed to read player action result")
}

fn set_player_position(instance: &GameInstance, x: f64, y: f64, z: f64) {
    let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
    runtime
        .lua()
        .load(
            r#"
            local x, y, z = ...
            local player = game:GetService("Players"):GetPlayers()[1]
            local hrp = player.Character and player.Character:FindFirstChild("HumanoidRootPart")
            assert(hrp ~= nil, "humanoid root part missing")
            hrp.Position = Vector3.new(x, y, z)
        "#,
        )
        .call::<()>((x, y, z))
        .expect("failed to set player position");
}

fn process_inputs(instance: &GameInstance) {
    instance
        .lua_runtime
        .as_ref()
        .expect("runtime missing")
        .process_agent_inputs()
        .expect("failed to process agent inputs");
}

fn heartbeat(instance: &GameInstance) {
    instance
        .lua_runtime
        .as_ref()
        .expect("runtime missing")
        .end_frame(1.0 / 60.0)
        .expect("failed to run heartbeat");
}

fn lua_frame(instance: &GameInstance) {
    instance
        .lua_runtime
        .as_ref()
        .expect("runtime missing")
        .tick(1.0 / 60.0)
        .expect("failed to run Lua frame");
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: expected {expected}, got {actual}"
    );
}

#[test]
fn pyramid_world_3_exact_resume_preserves_held_block_upvalue_state() {
    let Some((mut instance, source)) = configured_instance() else {
        return;
    };
    let started = Instant::now();
    eprintln!("[pyramid-world-3] loading script");
    instance.load_script(&source);
    lua_frame(&instance);
    eprintln!("[pyramid-world-3] script loaded in {:?}", started.elapsed());

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "PyramidResumeProbe"));
    let user_id = *instance
        .players
        .get(&agent_id)
        .expect("missing player user id");

    let inventory_before = workspace_inventory(&instance);
    assert!(
        inventory_before.2 > 0,
        "expected pyramid-world-3 to create pickup objects: {inventory_before:?}"
    );

    let (block_name, block_x, block_y, block_z) = first_pickup_object(&instance);
    let pickup_player_pos = [block_x + 2.0, block_y, block_z];
    set_player_position(
        &instance,
        pickup_player_pos[0],
        pickup_player_pos[1],
        pickup_player_pos[2],
    );

    instance.queue_agent_input(
        user_id,
        "Pickup".to_string(),
        serde_json::json!({ "position": [block_x, block_y, block_z] }),
    );
    process_inputs(&instance);
    heartbeat(&instance);
    assert_eq!(player_action_result(&instance), "PickupSuccess");
    eprintln!(
        "[pyramid-world-3] pickup succeeded in {:?}",
        started.elapsed()
    );

    let held_before_restore = block_position(&instance, &block_name);
    assert!(
        !held_before_restore.3,
        "held block should be non-collidable before snapshot"
    );

    eprintln!("[pyramid-world-3] capturing snapshot");
    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");
    eprintln!(
        "[pyramid-world-3] snapshot captured in {:?}",
        started.elapsed()
    );

    let Some((mut restored, _)) = configured_instance() else {
        return;
    };
    restored.init_runtime();
    eprintln!("[pyramid-world-3] restoring snapshot");
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");
    eprintln!(
        "[pyramid-world-3] snapshot restored in {:?}",
        started.elapsed()
    );

    let restored_inventory = workspace_inventory(&restored);
    assert_eq!(
        restored_inventory, inventory_before,
        "workspace inventory changed across restore: before={inventory_before:?} after={restored_inventory:?}"
    );
    assert_eq!(player_action_result(&restored), "PickupSuccess");

    assert!(
        restored.player_hrp_ids.contains_key(&agent_id),
        "missing restored humanoid root part"
    );
    let follow_player_pos = [block_x + 4.0, block_y + 1.0, block_z];
    set_player_position(
        &restored,
        follow_player_pos[0],
        follow_player_pos[1],
        follow_player_pos[2],
    );
    eprintln!("[pyramid-world-3] ticking restored follow state");
    heartbeat(&restored);
    eprintln!(
        "[pyramid-world-3] restored follow tick done in {:?}",
        started.elapsed()
    );

    let followed = block_position(&restored, &block_name);
    assert_close(followed.0, follow_player_pos[0], 0.05, "held block X");
    assert_close(
        followed.1,
        follow_player_pos[1] + 4.72 + 1.4 * 2.4 * 0.5,
        0.05,
        "held block Y",
    );
    assert_close(followed.2, follow_player_pos[2], 0.05, "held block Z");
    assert!(
        !followed.3,
        "held block should remain non-collidable after restore"
    );

    let place_pos = [
        follow_player_pos[0],
        follow_player_pos[1] + 5.5,
        follow_player_pos[2],
    ];
    restored.queue_agent_input(
        user_id,
        "Place".to_string(),
        serde_json::json!({ "position": place_pos, "rotation": 0 }),
    );
    process_inputs(&restored);
    assert_eq!(player_action_result(&restored), "PlaceSuccess");
    eprintln!(
        "[pyramid-world-3] post-restore place succeeded in {:?}",
        started.elapsed()
    );

    for _ in 0..25 {
        heartbeat(&restored);
    }

    let placed = block_position(&restored, &block_name);
    assert!(
        placed.3,
        "placed block should become collidable after placement animation"
    );
    assert!(
        !placed.4,
        "placed block should be unanchored after placement animation"
    );
}
