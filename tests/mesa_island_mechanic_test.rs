use std::path::PathBuf;

use clawblox::game::instance::GameInstance;
use clawblox::game::script_bundle::load_world_and_entry_script;
use serde_json::json;
use uuid::Uuid;

#[test]
fn mesa_island_pickup_place_and_throw() {
    let world_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("worlds/mesa-island");
    let (_, source) = load_world_and_entry_script(&world_dir).unwrap();
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(&source);
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "MechanicProbe"));
    let user_id = instance.players[&agent_id];
    for _ in 0..10 {
        instance.tick();
    }
    // Isolate the action checks from gravity and the world's initial block layout.
    let (x, y, z): (f64, f64, f64) = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
        Workspace.Gravity = 0
        local player = game:GetService("Players"):GetPlayers()[1]
        local position = player.Character.HumanoidRootPart.Position + Vector3.new(3, 0, 0)
        Workspace.Block_1.Position = position
        Workspace.Block_1.Anchored = true
        return position.X, position.Y, position.Z
    "#,
        )
        .eval()
        .unwrap();

    let actions = [
        (
            "Throw",
            json!({"direction": [1, 0, 0]}),
            "ThrowFailed:EmptyHands",
        ),
        ("Pickup", json!({"position": [x, y, z]}), "PickupSuccess"),
        (
            "Place",
            json!({"position": [x, y, z], "rotation": {"yaw": 90, "pitch": 0, "roll": 0}}),
            "PlaceSuccess",
        ),
        ("Pickup", json!({"position": [x, y, z]}), "PickupSuccess"),
        (
            "Throw",
            json!({"direction": [1, 0, 0], "speed": 20}),
            "ThrowSuccess",
        ),
        (
            "Place",
            json!({"position": [x, y, z]}),
            "PlaceFailed:EmptyHands",
        ),
    ];
    for (action, data, expected) in actions {
        instance.queue_agent_input(user_id, action.to_string(), data);
        for _ in 0..25 {
            instance.tick();
        }
        let result: String = instance.lua_runtime.as_ref().unwrap().lua().load(
            "return game:GetService('Players'):GetPlayers()[1]:GetAttribute('LastActionResult')"
        ).eval().unwrap();
        assert_eq!(result, expected, "unexpected result for {action}");
    }
    let thrown: bool = instance.lua_runtime.as_ref().unwrap().lua().load(
        "return not Workspace.Block_1.Anchored and Workspace.Block_1.AssemblyLinearVelocity.X > 0"
    ).eval().unwrap();
    assert!(
        thrown,
        "throw should release the block with forward velocity"
    );
}
