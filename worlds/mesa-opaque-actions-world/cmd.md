bash launch_multi.sh --force --goal "Goal: climb to the highest possible point in the world" --num-worlds 1 --base-port 8085

bash launch_multi.sh --force --num-worlds 1 --base-port 8085

uv run analyze_recording.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/20260311T032536Z/worlds/world-0/recordings/20260311_040913.clawrec --max-duration 3h

uv run python analysis/plot_sustained_max_y_runs.py results/20260311T032536Z_run1 results/20260311T141543Z_run2 results/20260311T150153Z_run3 results/20260311T153109Z_run4


uv run python analysis/plot_sustained_max_y_runs.py results/20260311T200926Z_play1 results/20260311T215544Z_play2 results/20260311T231238Z_play3 results/20260312T150549Z_play4




bash launch_multi_generations.sh --generations 5 --generation-duration 2h --base-port 8085

uv run python analysis/plot_sustained_max_y_runs.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g*


bash launch_multi_generations.sh --generations 10 --generation-duration 1h --base-port 8085


# evaluate on tasks
bash ./launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --duration 1h

uv run python analysis/plot_eval_naive_vs_experienced.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/evals

# evaluate in new world
bash /storage/nacloos/projects/clawblox/worlds/mesa-world/launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --world-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/eval-worlds/single-peak-2xblocks --duration 1h


bash /storage/nacloos/projects/clawblox/worlds/mesa-world/launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/runs/genexp-20260315T021154Z-g010/worlds/world-0/agents/mesa-rgenexp-20260315T021154Z-g010-w0-a0/workspace --world-dir /storage/nacloos/projects/clawblox/worlds/mesa-world/eval-worlds/single-peak-2xblocks --duration 1h --repeats 3 --objectives highest


uv run python analysis/plot_eval_naive_vs_experienced.py /storage/nacloos/projects/clawblox/worlds/mesa-world/results/genexp-20260315T021154Z/evals/single-peak-2xblocks



----

random actions:
bash ./launch_parallel_evals.sh --experiment-dir /storage/nacloos/projects/clawblox/worlds/mesa-opaque-actions-world/results/genexp-20260317T214525Z --template-workspace /storage/nacloos/projects/clawblox/worlds/mesa-opaque-actions-world/results/genexp-20260317T214525Z/runs/genexp-20260317T214525Z-g010/worlds/world-0/agents/mesa-rgenexp-20260317T214525Z-g010-w0-a0/workspace --duration 1h


---

claude code:

 ./launch_multi_claude.sh --sandbox --tmux-session mesa --base-port 8090 --num-worlds 1 --agents-per-world 1 --world-dir worlds/mesa-opaque-actions-world --template agent/template --record true --reset-every 3600