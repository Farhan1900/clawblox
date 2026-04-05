./launch_multi.sh --tmux-session river-bridge --base-port 8099 --num-worlds 1 --agents-per-world 1 --record true --sandbox --reset-every 3600

./launch_multi_claude.sh --tmux-session river-bridge --base-port 8099 --num-worlds 1 --agents-per-world 1 --world-dir worlds/mesa-island/eval-worlds/river-bridge --template worlds/mesa-island/agent-template --record true --reset-every 3600