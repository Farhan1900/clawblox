bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-2 --generations 20 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-2-0 --system-prompt worlds/mesa-world/system_prompt.md


bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-2 --generations 20 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-2-1 --system-prompt worlds/mesa-world/system_prompt.md


resume:
bash worlds/mesa-world/launch_multi_generations_claude.sh --experiment-id genexp-claude-20260417T041918Z --world-dir worlds/mesa-small-world-2 --generations 100 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-2-1 --system-prompt worlds/mesa-world/system_prompt.md --resume

bash worlds/mesa-world/launch_multi_generations_claude.sh --experiment-id genexp-claude-20260417T041834Z --world-dir worlds/mesa-small-world-2 --generations 100 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-2-0 --system-prompt  worlds/mesa-world/system_prompt.md --resume   