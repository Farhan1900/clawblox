# Construction Test

## Action Schemas

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z], "rotation": {"yaw": deg, "pitch": deg, "roll": deg}}` |


Blocks can be coated with residue clumps to make them sticky. A coated block bonds to adjacent blocks on contact. You can then pick up and move the entire bonded assembly.

## API Endpoints

The game server runs on `http://localhost:8080` by default.

### Send Input

`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns an observation payload.

### Get Observation

`GET /observe`

### Get Game Skill

`GET /skill.md`

Returns this file as markdown text.
