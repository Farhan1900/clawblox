# Skuttler Farm Loop

Small flat test world for the skuttler food loop.

## Goal

Use the HTTP API to:

1. Find a glow bulb.
2. Pick it up.
3. Place it near a skuttler.
4. Wait for the skuttler to feed.
5. Observe the skuttler become more amber as secretion builds.
6. Collect the clump it drops.

## Actions

| Input | Data |
|-------|------|
| MoveTo | `{"position": [x, y, z]}` |
| Jump | `{}` |
| Stop | `{}` |
| Pickup | `{"position": [x, y, z]}` |
| Place | `{"position": [x, y, z]}` |
| Consume | `{"position": [x, y, z]}` |

## Notes

- Glow bulbs are carryable and remain edible after placement.
- Skuttlers do not eat instantly; they pause at bulbs before taking bites.
- Clumps come from skuttlers, not directly from bulbs.
