bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --resume --experiment-id genexp-claude-20260423T040201Z

scripts/krb-longrun run --name mesa-small-world-3-0 -- bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox


scripts/krb-longrun run --name mesa-small-world-3-1 -- bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox


tmux new-session -d -s clawblox-watchdog -n monitor 'exec scripts/clawblox-watchdog'


2 agents:
scripts/krb-longrun run --name mesa-small-world-3-0 -- bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --agents-per-world 2 --template /storage/nacloos/projects/clawblox/agent/template/agent_voice


---

no soul:
bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-a.env

bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-b.env

bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8285 --tmux-prefix mesa-small-world-3-2 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-b.env


nothing:
bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt /home/naclo/Code/clawblox/agent/template/empty_system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-a.env --template /home/naclo/Code/clawblox/agent/template/empty

bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt /home/naclo/Code/clawblox/agent/template/empty_system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-b.env --template /home/naclo/Code/clawblox/agent/template/empty --auto-loop-message "continue"


builder:
bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --env-file .clawblox-multi/mesa-a.env --template /home/naclo/Code/clawblox/agent/template/agent_builder

bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt worlds/mesa-world/system_prompt.md --model claude-opus-4-7 --sandbox --env-file .clawblox-multi/mesa-b.env --template /home/naclo/Code/clawblox/agent/template/agent_builder


minimal:
bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-small-world-3-1 --system-prompt /home/naclo/Code/clawblox/agent/template/minimal_system_prompt.md --model claude-opus-4-7 --sandbox --skip-soul --env-file .clawblox-multi/mesa-b.env --template /home/naclo/Code/clawblox/agent/template/empty --auto-loop-message "continue"

bash worlds/mesa-world/launch_multi_generations_codex.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt /home/naclo/Code/clawblox/agent/template/minimal_system_prompt.md --model gpt-5.5 --sandbox --skip-soul --env-file .clawblox-multi/mesa-a.env --template /home/naclo/Code/clawblox/agent/template/empty --auto-loop-message "continue"


---

bash worlds/mesa-world/launch_multi_generations_codex.sh --world-dir worlds/mesa-small-world-3 --generations 30 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-small-world-3-0 --system-prompt worlds/mesa-world/system_prompt.md --model gpt-5.5 --sandbox
