bash launch_multi.sh --force --goal "Goal: climb to the highest possible point in the world" --num-worlds 1 --base-port 8085

bash launch_multi.sh --force --num-worlds 1 --base-port 8085

uv run analyze_recording.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/20260311T032536Z/worlds/world-0/recordings/20260311_040913.clawrec --max-duration 3h

uv run python analysis/plot_sustained_max_y_runs.py results/20260311T032536Z_run1 results/20260311T141543Z_run2 results/20260311T150153Z_run3 results/20260311T153109Z_run4


uv run python analysis/plot_sustained_max_y_runs.py results/20260311T200926Z_play1 results/20260311T215544Z_play2 results/20260311T231238Z_play3 results/20260312T150549Z_play4




bash launch_multi_generations.sh --generations 5 --generation-duration 2h --base-port 8085
bash launch_multi_generations.sh --generations 10 --generation-duration 1h --base-port 8085


uv run python analysis/plot_sustained_max_y_runs.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g*

uv run mesa-capture-replay-clip http://127.0.0.1:10181/ /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/clips/g005_spire_breakthrough.mp4 144200 150320 --width 1280 --height 720 --output-fps 12 --speed 4 --probe-timeout-ms 120000 --camera-mode spectator --camera-pan-x -30 --camera-pan-y 20 --camera-pan-z -33 --camera-zoom 1.0 --camera-yaw -1.0 --camera-pitch 0.75 --tail-ms 1000 --debug-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/clips/debug_g005_spire_fixed

uv run python analysis/extract_interesting_clips.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs

uv run python analysis/extract_behavior_clips.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs

# evaluate on tasks
bash ./launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --duration 1h

uv run python analysis/plot_eval_naive_vs_experienced.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/evals

# evaluate in new world
bash /storage/nacloos/projects/clawblox/worlds/mesa-world/launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --world-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/eval-worlds/single-peak-2xblocks --duration 1h


bash /storage/nacloos/projects/clawblox/worlds/mesa-world/launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --world-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/eval-worlds/single-peak-2xblocks --duration 1h --repeats 3 --objectives highest


uv run python analysis/plot_eval_naive_vs_experienced.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/evals


# multi agent
bash launch_multi_generations.sh --generations 10 --generation-duration 1h --base-port 8085 --agents-per-world 2


---

# new launcher

bash /storage/nacloos/projects/clawblox/agent/launch_multi_agent.sh --backend pi --world-dir /storage/nacloos/projects/clawblox/worlds/mesa-world --num-worlds 1 --agents-per-world 1 --base-port 8080 --port-wait-seconds 0 --tmux-session mesa-world-pi --provider anthropic --model claude-opus-4-6 --duration 1h


---

uv run exp_naive_vs_experienced.py --exp 001 --train-duration 10h --eval-duration 1h --reset-every 1h



seems to work less well with the scripts/ in the workspace? not sure
bash worlds/mesa-island/launch_multi_generations.sh --world-dir worlds/mesa-island/eval-worlds/river-bridge-small --generations 10 --generation-duration 1h --base-port 8185 


bash worlds/mesa-world/launch_multi_generations.sh --world-dir worlds/mesa-world --generations 10 --generation-duration 1h --base-port 8085 --tmux-prefix mesa-world-gens --provider anthropic --model claude-sonnet-4-6
bash worlds/mesa-world/launch_multi_generations.sh --world-dir worlds/mesa-world --generations 10 --generation-duration 1h --base-port 8185 --tmux-prefix mesa-world-gens2 --provider anthropic --model claude-sonnet-4-6
bash worlds/mesa-world/launch_multi_generations.sh --world-dir worlds/mesa-world --generations 10 --generation-duration 1h --base-port 8285 --tmux-prefix mesa-world-gens3 --provider anthropic --model claude-sonnet-4-6


# multi generation with claude code:
bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world --generations 10 --generation-duration 1h --base-port 8085 --tmux-prefix claude-gens


bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world --experiment-id genexp-claude-20260411T143033Z --resume --generations 20 --generation-duration 1h --base-port 8085 --tmux-prefix claude-gens


bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world --generations 1 --generation-duration 10h --base-port 8085 --tmux-prefix claude-gens


bash worlds/mesa-world/launch_multi_generations_claude.sh --world-dir worlds/mesa-world --generations 10 --generation-duration 1h --base-port 8085 --tmux-prefix claude-gens --keep-world
