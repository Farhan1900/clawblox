# Mesa Island

## Action Schemas

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z]}` |
| Consume | `{"position": [x, y, z]}` |


## API Endpoints

The game server runs on `http://localhost:8080` by default.

### Send Input

`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns an observation payload.

### Get Observation

`GET /observe`

### Speak

`POST /chat`

Body: `{"content":"..."}`

Say something aloud.

### Listen

`GET /chat/messages?after=<timestamp>&limit=20`

Hear what others have said recently.

### Get Game Skill

`GET /skill.md`

Returns this file as markdown text.
