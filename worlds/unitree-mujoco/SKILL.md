# Unitree G1 Bricklaying Simulation

You control a Unitree G1 humanoid robot in a MuJoCo physics simulation.
The robot stands near a table with red bricks. Your goal is to pick up bricks
and build structures by placing them precisely on the table.

## Action Schemas

| Input | Data | Duration |
|-------|------|----------|
| InitArm | `{}` | ~10s |
| Pick | `{}` | ~30s |
| Place | `{"position": [x, y, z, roll, pitch, yaw]}` | ~30s |
| WalkForward | `{}` | ~1s |
| WalkBackward | `{}` | ~1s |
| TurnLeft | `{}` | ~1s |
| TurnRight | `{}` | ~1s |
| Reset | `{}` | ~2s |

## API Endpoints

The server runs on `http://localhost:8080`.

### Send Action
`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns JSON with action result and updated observation. Actions are **blocking** — the response arrives after the action completes (may take 30+ seconds for Pick/Place).

Example:
```bash
curl -sS -X POST http://localhost:8080/input -d '{"type":"WalkForward","data":{}}'
curl -sS -X POST http://localhost:8080/input -d '{"type":"Pick","data":{}}'
curl -sS -X POST http://localhost:8080/input -d '{"type":"Place","data":{"position":[0.09, 0.0, 0.05, 0.0, 0.0, 0.0]}}'
```

### Get Observation
`GET /observe`

Returns current robot state: arm state, joint positions, end-effector position, camera status, last action result.

### Get Camera Image
`GET /camera`

Returns a JPEG image from the robot's head-mounted RealSense camera. Save to a file to view:
```bash
curl -sS http://localhost:8080/camera -o /tmp/camera.jpg
```

### Join
`POST /join?name=<AgentName>`

Returns `{"session":"<token>"}`. Call once at startup.

### Get Skill Documentation
`GET /skill.md`

Returns this file.

### Reset Simulation
`POST /reset`

Resets robot and bricks to initial positions.

## Workflow

1. The robot starts initialized with arm at ready position
2. Use **WalkForward/WalkBackward/TurnLeft/TurnRight** to position near the brick pile
3. Use **Pick** to detect and grab a brick (uses camera + AI segmentation)
4. Use **Place** with a target position to put the brick down
5. Repeat to build a structure

## Place Coordinates

The `position` array for Place is `[x, y, z, roll_deg, pitch_deg, yaw_deg]` in table-relative coordinates:
- `x`: distance from table edge toward center (0.09 = near edge, 0.17 = further)
- `y`: lateral position (-0.36 to 0.36, 0 = center)
- `z`: height above table (0.05 = first layer, 0.11 = second, 0.17 = third)
- `roll_deg`, `pitch_deg`, `yaw_deg`: brick rotation in degrees

Brick dimensions: 9cm × 20cm × 6cm.

Example wall pattern:
```
Layer 1: [0.09, -0.12, 0.05, 0, 0, 0], [0.09, 0.12, 0.05, 0, 0, 0]
Layer 2: [0.09, 0.0, 0.11, 0, 0, 0]
```

## Tips

- Always observe before acting to understand the scene
- Pick automatically detects bricks via camera — no position needed
- Only bricks on the robot's right side (y ≤ 0 in pelvis frame) can be picked
- If Pick fails, try repositioning with Walk/Turn commands
- Camera view helps understand what the robot sees
- Reset if things go wrong — it restores everything to initial state
