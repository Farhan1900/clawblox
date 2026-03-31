#!/bin/bash
source ~/miniforge3/etc/profile.d/conda.sh
conda activate g1
source /opt/ros/jazzy/setup.bash

echo "=== cyclonedds python location ==="
python3 -c "import cyclonedds; print(cyclonedds.__file__)"

echo ""
echo "=== LD_LIBRARY_PATH ==="
echo $LD_LIBRARY_PATH | tr ':' '\n' | grep -i dds

echo ""
echo "=== libddsc in path ==="
ldconfig -p 2>/dev/null | grep ddsc
find /opt/ros/jazzy/lib -name 'libddsc*' 2>/dev/null
find /usr/local/lib -name 'libddsc*' 2>/dev/null

echo ""
echo "=== test without ROS2 sourced ==="
conda deactivate
source ~/miniforge3/etc/profile.d/conda.sh
conda activate g1
# Don't source ROS2
python3 -c "
from unitree_sdk2py.core.channel import ChannelFactoryInitialize
ChannelFactoryInitialize(0, 'eth0')
print('DDS OK without ROS2')
" 2>&1
