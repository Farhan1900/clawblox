#!/bin/bash
set -e

SRC=/mnt/c/Users/naclo/Documents/Code/clawblox/worlds/unitree-mujoco/Unitree-Mujoco-Dex3/simulate
BUILD=~/unitree_sim_build

# Link MuJoCo
ln -sf ~/.mujoco/mujoco-3.3.6 "$SRC/mujoco"

# Build in native WSL filesystem for speed
rm -rf "$BUILD"
mkdir -p "$BUILD"
cd "$BUILD"

cmake "$SRC" -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=/opt/unitree_robotics/lib/cmake
make -j$(nproc)

echo "---"
ldd ./unitree_mujoco | grep "not found" || echo "All libs OK"
echo "Binary: $BUILD/unitree_mujoco"
