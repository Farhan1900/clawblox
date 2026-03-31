# Unitree G1 MuJoCo Simulation

Simulated pick-and-place with the Unitree G1 humanoid robot in MuJoCo.

## Structure

- `G1-Bricklaying-Simulation/` — Perception, planning, and control stack for the G1
- `Unitree-Mujoco-Dex3/` — MuJoCo simulator with Unitree SDK2 bridge (C++ and Python)
- `unitree_sdk2_python/` — Unitree SDK2 Python bindings (cloned from GitHub)

## Prerequisites

- Windows 11 with WSL2 (Ubuntu 24.04)
- Docker Desktop with WSL2 integration enabled (Settings → Resources → WSL Integration → Enable Ubuntu-24.04)
- NVIDIA GPU
- Add user to docker group in WSL: `sudo usermod -aG docker $USER` (reopen terminal after)

## Setup

### Build the Docker image

From a WSL2 terminal:

```bash
cd /mnt/c/Users/naclo/Documents/Code/clawblox/worlds/unitree-mujoco/G1-Bricklaying-Simulation
docker compose build
docker compose up -d
```

`Dockerfile.local` is identical to the original `Dockerfile` with two changes for driver compatibility:
- CUDA 12.8 → 12.3 (matches local NVIDIA driver)
- PyTorch cu128 → cu121

### Install additional runtime dependencies (one-time)

```bash
docker exec g1-bricklaying pip install ros-rmw-fastrtps-cpp ros-geometry-msgs mujoco pygame numpy-quaternion casadi scipy matplotlib ultralytics
```

## Running

All commands from WSL2 terminals.

### Terminal 1 — Simulator

```bash
docker exec -it g1-bricklaying bash -c "cd /ws/Unitree-Mujoco-Dex3/simulate_python && MUJOCO_GL=egl python3 unitree_mujoco.py"
```

`MUJOCO_GL=egl` is required — see "Camera rendering" below.

### Terminal 2 — Interactive demo

```bash
docker exec -it g1-bricklaying bash -c "cd /ws/g1-bricklaying && python3 demo/interface.py"
```

Controls:
- `p` — Pick: detect brick via camera + FastSAM, estimate pose via ICP, plan and execute trajectory
- `P` — Place: localize table via ArUco markers, place brick at target position
- `w` — Step forward
- `s` — Step backward
- `q` — Turn left
- `e` — Turn right

## How It Works

The simulator and demo run in the same Docker container, communicating over DDS (CycloneDDS) on the `docker0` interface.

- **Simulator** (`unitree_mujoco.py`): runs MuJoCo physics, publishes joint state (`rt/lowstate`), hand state (`rt/dex3/*/state`), and camera images (`rt/realsense/color`, `rt/realsense/depth`) over DDS. Subscribes to motor commands (`rt/lowcmd`) and hand commands (`rt/dex3/*/cmd`).
- **Demo** (`interface.py`): subscribes to state/camera topics, runs perception (FastSAM segmentation + ICP pose estimation), plans trajectories via IK (pinocchio + casadi), and publishes motor commands.

GPU-accelerated rendering is provided by WSLg: Docker mounts `/dev/dxg`, `/mnt/wslg`, and `/usr/lib/wsl` from the WSL2 host. Mesa's D3D12 backend routes OpenGL calls through the NVIDIA GPU.

## Scene Configuration

| What | File | Notes |
|------|------|-------|
| Robot position | `Unitree-Mujoco-Dex3/unitree_robots/g1/g1_with_hands_modified.xml` | `base_link` pos attribute |
| Robot heading | `Unitree-Mujoco-Dex3/simulate_python/unitree_sdk2py_bridge.py` line 62 | `theta` variable (radians, 0=+x, pi=-x) |
| Brick positions | `Unitree-Mujoco-Dex3/unitree_robots/g1/bricklaying_props.xml` | Free-joint bodies on table |
| Table | `Unitree-Mujoco-Dex3/unitree_robots/g1/table.xml` | Surface at z≈0.93, collider x∈[-1.26,-0.50] y∈[-0.76,0.76] |
| DDS interface | `Unitree-Mujoco-Dex3/simulate_python/config.py` | Must match demo default (`docker0`) |
| Reachable workspace | Ellipsoid center (0,0,0.302) relative to pelvis, radii (0.55, 0.59, 0.46)m | Defined in `planning/reachability.py` |

## Known Issues

### Camera rendering requires MUJOCO_GL=egl
The simulator runs the MuJoCo viewer (GLFW/OpenGL) on the main thread and camera rendering (`mujoco.Renderer`) on a daemon thread. On Mesa D3D12, two GL contexts in different threads of the same process conflict — the camera thread silently crashes and publishes zero frames. `MUJOCO_GL=egl` makes the offscreen renderer use EGL (independent context), fixing the conflict. Camera then publishes at ~3-4 fps.

### Robot heading controlled by code, not XML
The `update_twist` thread in the simulator bridge overwrites `mocap_quat` every 100ms from the global `theta` variable. Changing `base_link` quat in the XML has no effect. Modify `theta` in `unitree_sdk2py_bridge.py` line 62 to change heading.

### ICP pose estimation fails in simulation
FastSAM correctly detects objects (bricks, ArUco markers) but ICP registration returns fitness=0 for all detections. The brick reference point cloud (from `assets/flat_brick_large.npy`) doesn't match the simulated point clouds due to differences in geometry appearance, depth characteristics, and scene composition between simulation and reality. This is a sim-to-real perception gap. The arm control pipeline (IK, trajectory planning, execution) works correctly — only perception is affected.

### Control loop overruns
The 10ms control loop budget is frequently exceeded (~12ms average, up to 65ms spikes) due to the Python simulator's threading overhead + EGL camera rendering competing for CPU. Despite this, trajectory tracking stays within acceptable limits (max ~10° joint error, ~33mm position error).

### Walking
Walking commands (`w/s/q/e`) send velocity pulses via ROS2 `cmd_vel` topic. Each keypress moves for 0.5 seconds then stops. Requires rclpy in both the simulator and demo (installed via `ros-rclpy` pip package).

## Changes from Remote Repos

### G1-Bricklaying-Simulation
| File | Change | Reason |
|------|--------|--------|
| `Dockerfile.local` | New file: Dockerfile with CUDA 12.3 + cu121 | Driver compatibility |
| `docker-compose.yaml` | WSLg mounts, DISPLAY/GPU env vars, Dockerfile.local | GPU rendering in Docker |
| `demo/interface.py` | `rclpy.init()` + default arg for `G1TwistCmdNode`, fixed `publish_twist` signature, wired up walking commands | Bug fixes + walking implementation |

### Unitree-Mujoco-Dex3
| File | Change | Reason |
|------|--------|--------|
| `simulate_python/config.py` | `INTERFACE = "docker0"` | Match demo's default interface |
| `simulate_python/unitree_sdk2py_bridge.py` | `theta = np.pi` | Face robot toward table |
| `unitree_robots/g1/g1_with_hands_modified.xml` | `pos="-0.4 0 0"` | Position robot near table |
| `unitree_robots/g1/bricklaying_props.xml` | 5 bricks on right side of table | Simulation scene setup |
