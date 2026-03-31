#!/bin/bash
set -e
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
source ~/g1-env/bin/activate

REPO=/mnt/c/Users/naclo/Documents/Code/clawblox/worlds/unitree-mujoco

echo "=== Installing unitree_sdk2_python ==="
cd "$REPO/unitree_sdk2_python"
uv pip install -e .

echo ""
echo "=== Installing bricklaying ==="
cd "$REPO/G1-Bricklaying-Simulation"
uv pip install -e .

echo ""
echo "=== Testing all imports ==="
python -c "
import mujoco, numpy, scipy, cv2, open3d, quaternion
print('Core: OK')
import pinocchio
print('pinocchio: OK')
from unitree_sdk2py.core.channel import ChannelFactoryInitialize
print('unitree_sdk2py: OK')
from bricklaying.robot import DDSInterface, G1URDFModel, DualArmIK, TrajectoryController
print('bricklaying.robot: OK')
from bricklaying.planning import MotionPlanner, EllipsoidRegion
print('bricklaying.planning: OK')
from bricklaying.perception import SimRealSenseCamera
print('bricklaying.perception: OK')
from bricklaying.segmentation import FastSAMSegmentor
print('bricklaying.segmentation: OK')
print()
print('All imports OK!')
"
