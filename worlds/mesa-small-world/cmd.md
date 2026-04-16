bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world --generations 10 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-0 --system-prompt worlds/mesa-world/system_prompt.md --keep-world

bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world --generations 10 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-1 --system-prompt worlds/mesa-world/system_prompt.md --keep-world


bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world --generations 10 --generation-duration 1h --base-port 8285 --tmux-prefix mesa-small-world-2 --system-prompt worlds/mesa-world/system_prompt.md --keep-world
