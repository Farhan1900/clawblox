#!/bin/bash
set -e
source ~/miniforge3/etc/profile.d/conda.sh

REPO=/mnt/c/Users/naclo/Documents/Code/clawblox/worlds/unitree-mujoco

echo "=== Creating conda env ==="
conda create -n g1 python=3.10 -y
conda activate g1

echo "=== Installing pinocchio with CasADi ==="
conda install -c conda-forge pinocchio -y

echo "=== Installing pip deps ==="
pip install mujoco numpy scipy matplotlib opencv-python ultralytics open3d numpy-quaternion casadi
pip install cyclonedds
pip install -e "$REPO/unitree_sdk2_python"
pip install -e ~/g1-bricklaying

echo "=== Testing imports ==="
python -c "
import mujoco, numpy, scipy, cv2, open3d, quaternion, casadi
print('Core: OK')
import pinocchio
print('pinocchio: OK')
from pinocchio import casadi as cpin
print('pinocchio.casadi: OK')
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
