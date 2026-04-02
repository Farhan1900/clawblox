# River Bridge

## Action Schemas

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Reset | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z], "rotation": {"yaw": deg, "pitch": deg, "roll": deg}}` |

`Reset` fully rebuilds the world: terrain, loose blocks, residue clumps, and player position all return to the initial state.

Blocks can be coated with residue clumps to make them sticky. A coated block bonds to adjacent blocks on contact. You can then pick up and move the entire bonded assembly.

The river channel does not support the player. Build a stable bridge from the near bank to the far bank, then cross it.

The player starts on the west bank. Loose blocks are stacked in one corner, and residue clumps are piled in the opposite corner. The goal is simply to reach the east bank.

## API Endpoints

Use the current world URL provided by your runner. Do not assume port `8080`.

### Send Input

`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns an observation payload.

### Get Observation

`GET /observe`

### Get Game Skill

`GET /skill.md`

Returns this file as markdown text.
