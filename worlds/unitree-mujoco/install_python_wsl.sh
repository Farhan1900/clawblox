#!/bin/bash
set -e

REPO=/mnt/c/Users/naclo/Documents/Code/clawblox/worlds/unitree-mujoco

echo "=== Installing Python dependencies ==="
pip3 install mujoco numpy scipy matplotlib opencv-python ultralytics open3d numpy-quaternion

echo ""
echo "=== Installing CycloneDDS + Unitree SDK2 Python ==="
pip3 install cyclonedds
pip3 install -e "$REPO/unitree_sdk2_python"

echo ""
echo "=== Installing G1-Bricklaying-Simulation ==="
pip3 install -e "$REPO/G1-Bricklaying-Simulation"

echo ""
echo "=== Checking imports ==="
python3 -c "
import mujoco, numpy, scipy, cv2, ultralytics, open3d, quaternion
print('Core deps: OK')
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
" 2>&1 || echo "Some imports failed — pinocchio may need conda install"

echo ""
echo "=== Done ==="
echo "NOTE: pinocchio must be installed via conda:"
echo "  conda install -c conda-forge pinocchio"
