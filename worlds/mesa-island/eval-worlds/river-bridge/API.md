
## Action Schemas

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Reset | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z], "rotation": {"yaw": deg, "pitch": deg, "roll": deg}}` |

Blocks can be coated with residue clumps to make them sticky. A coated block bonds to adjacent blocks on contact. You can then pick up and move the entire bonded assembly.

Blocks fall in the river.
You need to build a long bridge first and then transport it over the river.

## API Endpoints

Use the current world URL provided by your runner. Do not assume port `8080`.

### Send Input

`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns an observation payload.

### Get Observation

`GET /observe`

### Get Agent API

`GET /api.md`

Returns this file as markdown text.
