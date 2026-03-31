#!/bin/bash
source ~/miniforge3/etc/profile.d/conda.sh
conda activate g1

CLAYER=$(python3 -c "import cyclonedds._clayer as c; print(c.__file__)")
echo "clayer: $CLAYER"
ldd "$CLAYER" 2>&1 | grep -i dds
echo "---"
ls -la /usr/local/lib/libddsc.so*
echo "---"
# Try reinstalling cyclonedds from scratch
pip install --force-reinstall --no-cache-dir cyclonedds==0.10.2 2>&1 | tail -5
