bash worlds/mesa-world/launch_multi_generations.sh --world-dir worlds/mesa-world-2 --generations 20 --generation-duration 1h --base-port 8085 --agents-per-world 1 --tmux-prefix mesa-world-gens --system-prompt worlds/mesa-world/system_prompt.md



bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world-2 --generations 10 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-world-gens --system-prompt worlds/mesa-world/system_prompt.md --keep-world



ANTHROPIC_API_KEY=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 bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world-2 --generations 10 --generation-duration 1h --base-port 8285 --tmux-prefix mesa-world-gens-2 --system-prompt worlds/mesa-world/system_prompt.md --keep-world

