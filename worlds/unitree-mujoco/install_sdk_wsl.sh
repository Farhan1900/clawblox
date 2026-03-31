#!/bin/bash
set -e

echo "=== Building unitree_sdk2 ==="
cd /tmp
rm -rf unitree_sdk2
git clone --quiet https://github.com/unitreerobotics/unitree_sdk2.git
cd unitree_sdk2
mkdir build && cd build
cmake .. -DCMAKE_INSTALL_PREFIX=/opt/unitree_robotics -DBUILD_EXAMPLES=OFF
make -j$(nproc)
sudo make install
echo "unitree_sdk2 installed at /opt/unitree_robotics"
ls /opt/unitree_robotics/lib/cmake/
