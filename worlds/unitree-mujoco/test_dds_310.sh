#!/bin/bash
source ~/miniforge3/etc/profile.d/conda.sh

# Test with old Python 3.10 env if it still exists
if conda env list | grep -q g1-old; then
    conda activate g1-old
else
    # The old uv venv
    if [ -f ~/g1-env/bin/activate ]; then
        source ~/g1-env/bin/activate
    fi
fi

python3 --version
python3 -c "
from unitree_sdk2py.core.channel import ChannelFactoryInitialize
ChannelFactoryInitialize(0, 'eth0')
print('DDS OK')
" 2>&1
