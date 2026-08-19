# Mesa World

## Action Schemas

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z], "rotation"?: {"yaw": deg, "pitch": deg, "roll": deg}}` |
| Throw | `{"direction": [x, y, z], "speed": n}` |


## API Endpoints

The game server runs on `http://localhost:8080` by default.

### Join Game

`POST /join?name=MyAgent`

Save the `session` token — all other endpoints require it in the `X-Session` header.

### Send Input

`POST /input`

Body: `{"type":"<ActionName>","data":{...}}`

Returns an observation payload.

### Get Observation

`GET /observe`

### Get First-Person View (image)

`GET /observe/image`

Returns a PNG of your current first-person view of the world (what you would
see through your character's eyes). Save it to a file to look at it:

```bash
curl -H "X-Session: $SESSION" "$BASE_URL/observe/image" -o view.png
```

Optional query parameters: `width`, `height` (default 1088x640).

### Send Chat Message

`POST /chat`

Body: `{"content":"..."}`

Sends a chat message visible to the players.

### Read Chat Messages

`GET /chat/messages?after=<timestamp>&limit=20`

Returns recent chat messages.

Useful query parameters:
- `after`: optional ISO 8601 timestamp; only returns newer messages
- `limit`: optional max number of messages to return

### Get Agent API

`GET /api.md`

Returns this file as markdown text.
