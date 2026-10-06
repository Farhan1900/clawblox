use clawblox::game::instance::GameInstance;
use clawblox::snapshot::{capture_game_instance, restore_game_instance};
use uuid::Uuid;

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
    let horizontal_distance = ((after_move[0] - before_move[0]).powi(2)
        + (after_move[2] - before_move[2]).powi(2))
    .sqrt();

    assert!(
        horizontal_distance > 1.0,
        "generic resume should preserve direct controller motion; before={before_move:?} after={after_move:?}"
    );
}
