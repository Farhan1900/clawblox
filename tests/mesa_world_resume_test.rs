use std::fs;
use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
use clawblox::snapshot::{capture_game_instance, restore_game_instance};
use uuid::Uuid;

fn mesa_world_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("mesa-world")
        .join("main.lua")
}

#[test]
fn mesa_world_resume_preserves_existing_player_position() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    let source = fs::read_to_string(mesa_world_script_path()).expect("failed to read mesa-world");
    instance.load_script(&source);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ResumeProbe"));
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("missing humanoid root part");

    for _ in 0..10 {
        instance.tick();
    }

    // Place the player on top of the large mesa so the restored position remains stable under
    // the world's default gravity after the script reloads.
    instance.physics.set_character_position(hrp_id, [20.0, 8.0, 20.0]);

    for _ in 0..30 {
        instance.tick();
    }

    let before_restore: (String, f64, f64, f64) = {
        let runtime = instance.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local player = game:GetService("Players"):GetPlayers()[1]
                local hrp = player.Character and player.Character:FindFirstChild("HumanoidRootPart")
                assert(player ~= nil, "player missing before snapshot")
                assert(hrp ~= nil, "humanoid root part missing before snapshot")
                return player.Name, hrp.Position.X, hrp.Position.Y, hrp.Position.Z
            "#,
            )
            .eval()
            .expect("failed to read player position before snapshot")
    };

    let snapshot = capture_game_instance(&instance).expect("failed to capture snapshot");

    let mut restored = GameInstance::new(Uuid::new_v4(), None);
    restored.init_runtime();
    restore_game_instance(&mut restored, &snapshot).expect("failed to restore snapshot");
    restored.load_script(&source);

    for _ in 0..10 {
        restored.tick();
    }

    let after_restore: (String, f64, f64, f64) = {
        let runtime = restored.lua_runtime.as_ref().expect("runtime missing");
        runtime
            .lua()
            .load(
                r#"
                local player = game:GetService("Players"):GetPlayers()[1]
                local hrp = player.Character and player.Character:FindFirstChild("HumanoidRootPart")
                assert(player ~= nil, "player missing after restore")
                assert(hrp ~= nil, "humanoid root part missing after restore")
                return player.Name, hrp.Position.X, hrp.Position.Y, hrp.Position.Z
            "#,
            )
            .eval()
            .expect("failed to read player position after restore")
    };

    assert_eq!(before_restore.0, "ResumeProbe");
    assert_eq!(after_restore.0, "ResumeProbe");

    let before_pos = [before_restore.1, before_restore.2, before_restore.3];
    let after_pos = [after_restore.1, after_restore.2, after_restore.3];
    for axis in 0..3 {
        let tolerance = if axis == 1 { 5e-2 } else { 1e-2 };
        assert!(
            (before_pos[axis] - after_pos[axis]).abs() < tolerance,
            "player position changed across restore on axis {axis}: before={:?} after={:?}",
            before_pos,
            after_pos
        );
    }

    assert!(
        (after_restore.1 - 0.0).abs() > 1.0 || (after_restore.2 - 5.0).abs() > 1.0,
        "resume should not snap the player back to spawn; restored position={:?}",
        after_pos
    );
}
