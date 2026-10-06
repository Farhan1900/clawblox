mod game;

use game::lua::LuaRuntime;
use mlua::Result;
use uuid::Uuid;

fn run_smoke_test() -> Result<()> {
    let mut runtime = LuaRuntime::new(Uuid::new_v4(), None)?;
    runtime.load_script(
        r#"
        local part = Instance.new("Part")
        part.Name = "BrowserPart"
        part.Position = Vector3.new(1, 2, 3)
        part:SetAttribute("Runtime", "browser")
        part.Parent = Workspace

        task.wait(2 / 60)
        local found = Workspace:FindFirstChild("BrowserPart")
        browserPartReady = found ~= nil
            and found.Position == Vector3.new(1, 2, 3)
            and found:GetAttribute("Runtime") == "browser"

        RunService.Heartbeat:Wait()
        browserHeartbeatReady = true
        "#,
    )?;

    for _ in 0..6 {
        runtime.begin_frame(1.0 / 60.0)?;
        runtime.end_frame(1.0 / 60.0)?;
    }

    let globals = runtime.lua().globals();
    assert!(globals.get::<bool>("browserPartReady")?);
    assert!(globals.get::<bool>("browserHeartbeatReady")?);
    Ok(())
}

fn main() {
    match run_smoke_test() {
        Ok(()) => println!("CLAWBLOX_LUAU_WASM_SPIKE_OK"),
        Err(error) => {
            eprintln!("CLAWBLOX_LUAU_WASM_SPIKE_FAILED: {error}");
            std::process::exit(1);
        }
    }
}
