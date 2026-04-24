bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --resume --experiment-id genexp-claude-20260423T040201Z

scripts/krb-longrun run --name mesa-small-world-3-0 -- bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox


scripts/krb-longrun run --name mesa-small-world-3-1 -- bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox


tmux new-session -d -s clawblox-watchdog -n monitor 'exec scripts/clawblox-watchdog'
