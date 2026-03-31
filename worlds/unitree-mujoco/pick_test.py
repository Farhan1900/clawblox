"""Direct arm control pick test for C++ MuJoCo simulator."""
import sys, time
import numpy as np
from scipy.spatial.transform import Rotation as R

from bricklaying.robot import DDSInterface, G1URDFModel, DualArmIK, TrajectoryController
from bricklaying.planning import MotionPlanner

import bricklaying.robot.dds_interface as dds_mod
def quiet_shutdown(self):
    self._running = False
    time.sleep(0.1)
dds_mod.DDSInterface.shutdown = quiet_shutdown

dds = DDSInterface("eth0")
urdf = G1URDFModel(reduced=True)
ik = DualArmIK()
ctrl = TrajectoryController(dds=dds, urdf_model=urdf, ik_solver=ik)
ctrl.max_track_q_error = 10.0
ctrl.max_track_pos_error = 10.0
ctrl.max_track_rot_error = 10.0
planner = MotionPlanner()
free_joints = ik.right_q_idx + ik.waist_q_idx

def make_pose(pos, rot):
    T = np.eye(4)
    T[:3, 3] = pos
    T[:3, :3] = rot
    return T

def get_state():
    q, _ = dds.get_upper_body_state()
    left_ee = urdf.get_frame_transform(q, 'left_ee', use_reduced=True)
    right_ee = urdf.get_frame_transform(q, 'right_ee', use_reduced=True)
    return q, left_ee, right_ee

def move_right_to(target_T, duration=5.0):
    q, left_ee, right_ee = get_state()
    traj = planner.plan_through_waypoints(
        [left_ee, left_ee],
        [right_ee, target_T],
        duration=duration
    )
    success = ctrl.execute(traj, free_joints=free_joints)
    q2, _, right_ee2 = get_state()
    err = np.linalg.norm(right_ee2[:3, 3] - target_T[:3, 3]) * 1000
    print('  EE: %s  err: %.1fmm  %s' % (np.round(right_ee2[:3, 3], 3), err, 'OK' if success else 'FAILED'))
    return success

# Grasp rotation: hand pointing down
R_grasp = R.from_euler('x', -90, degrees=True).as_matrix()

# Brick 3 in robot frame (reachable one at -0.55, 0, 0.96)
# Robot at (-0.15, 0, 1.0) facing -x (180 deg)
# Relative: brick - pelvis = (-0.4, 0, -0.04)
# In robot frame (rotated 180): (0.4, 0, -0.04)
brick = np.array([0.4, 0.0, -0.04])

q, _, right_ee = get_state()
print('Start EE: %s' % np.round(right_ee[:3, 3], 3))
print('Target brick (robot frame): %s' % brick)

# 1) Approach
print('\n1) Approach (above brick)')
move_right_to(make_pose(brick + [0, 0, 0.1], R_grasp), duration=8.0)

# 2) Lower
print('\n2) Lower to brick')
move_right_to(make_pose(brick, R_grasp), duration=5.0)

# 3) Grasp
print('\n3) Grasp')
dds.set_hand_mode('close', 'grasp')
time.sleep(1.5)
print('  Hand closed.')

# 4) Lift
print('\n4) Lift')
move_right_to(make_pose(brick + [0, 0, 0.15], R_grasp), duration=5.0)

time.sleep(3.0)
print('\nDone!')
dds.shutdown()
