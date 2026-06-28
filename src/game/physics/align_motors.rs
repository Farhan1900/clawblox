//! AlignPosition/AlignOrientation backed by persistent rapier motor joints.
//!
//! Align constraints used to be integrated explicitly: a spring force was
//! computed once per tick from pre-step state and applied blind for the whole
//! step. That caps the usable gain at the explicit stability limit, stacks
//! destructively when several constraints share one body, and pumps energy
//! when constraints conflict. Motorized joints are solved implicitly at the
//! velocity level together with contacts and the other joints: high gains
//! stay stable, and redundant or conflicting constraints resolve to a
//! bounded compromise instead of oscillating.
//!
//! Joints persist across ticks (preserving solver warm-start impulses) and
//! are recreated only when the bodies they connect change, e.g. after an
//! assembly rebuild.

use rapier3d::prelude::*;
use std::collections::HashSet;

use super::PhysicsWorld;

/// Which degrees of freedom an align constraint drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignKind {
    Position,
    Orientation,
}

/// Where an align constraint's target anchor lives.
#[derive(Debug, Clone, Copy)]
pub enum AlignTarget {
    /// Attachment1 on a part: `attachment_frame` is local to that part.
    Part {
        part_id: u64,
        attachment_frame: Isometry<Real>,
    },
    /// No Attachment1: a fixed world-space goal.
    World(Isometry<Real>),
}

/// One AlignPosition/AlignOrientation constraint, described at the part /
/// attachment level. The physics layer resolves parts to bodies and
/// attachment frames to body-local joint frames.
#[derive(Debug, Clone, Copy)]
pub struct AlignMotorSpec {
    /// Lua ID of the align constraint instance (joint map key).
    pub align_id: u64,
    pub kind: AlignKind,
    /// Part carrying Attachment0 (the driven side).
    pub part_id: u64,
    /// Attachment0 frame, local to its part.
    pub attachment_frame: Isometry<Real>,
    pub target: AlignTarget,
    /// Roblox-style responsiveness. With rapier's acceleration-based motor
    /// model, stiffness = responsiveness^2 and damping = 2 * responsiveness
    /// give a critically damped, mass/inertia-independent response.
    pub responsiveness: f32,
    /// Per-axis force (Position) or torque (Orientation) cap.
    pub max_output: [f32; 3],
    /// Position only: cap on the commanded approach speed, emulated by
    /// rate-limiting the target the motor sees. <= 0 disables the cap.
    pub max_velocity: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AlignJointEntry {
    handle: ImpulseJointHandle,
    body1: RigidBodyHandle,
    body2: RigidBodyHandle,
    kind: AlignKind,
}

const LINEAR_AXES: [JointAxis; 3] = [JointAxis::LinX, JointAxis::LinY, JointAxis::LinZ];
const ANGULAR_AXES: [JointAxis; 3] = [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ];

/// Error below which the constraint is considered settled and the driven
/// body is allowed to fall asleep.
const POSITION_WAKE_EPSILON: f32 = 0.01;
const ORIENTATION_WAKE_EPSILON: f32 = 0.005;

impl PhysicsWorld {
    /// The fixed body that world-space align targets attach to.
    fn world_anchor(&mut self) -> RigidBodyHandle {
        if let Some(handle) = self.world_anchor_body {
            if self.rigid_body_set.get(handle).is_some() {
                return handle;
            }
        }
        let handle = self
            .rigid_body_set
            .insert(RigidBodyBuilder::fixed().build());
        self.world_anchor_body = Some(handle);
        handle
    }

    /// Body-local frame of an attachment living on `part_id`. Parts share a
    /// body when welded, so the attachment frame is composed with the part's
    /// collider frame within that body.
    fn attachment_body_frame(
        &self,
        part_id: u64,
        attachment_frame: &Isometry<Real>,
    ) -> Option<Isometry<Real>> {
        let local = self.lua_to_local_frame.get(&part_id)?;
        Some(local * attachment_frame)
    }

    /// Creates or updates the motor joint for one align constraint. Returns
    /// false when the constraint cannot be represented this tick (missing
    /// bodies/attachments, or an anchored driven part).
    pub fn upsert_align_motor(&mut self, spec: &AlignMotorSpec) -> bool {
        let Some(&body1) = self.lua_to_body.get(&spec.part_id) else {
            self.remove_align_motor(spec.align_id);
            return false;
        };
        if !self.is_dynamic_body(body1) {
            self.remove_align_motor(spec.align_id);
            return false;
        }
        let Some(frame1) = self.attachment_body_frame(spec.part_id, &spec.attachment_frame) else {
            self.remove_align_motor(spec.align_id);
            return false;
        };

        let (mut body2, mut frame2) = match spec.target {
            AlignTarget::Part {
                part_id,
                attachment_frame,
            } => {
                let Some(&body2) = self.lua_to_body.get(&part_id) else {
                    self.remove_align_motor(spec.align_id);
                    return false;
                };
                let Some(frame2) = self.attachment_body_frame(part_id, &attachment_frame) else {
                    self.remove_align_motor(spec.align_id);
                    return false;
                };
                (body2, frame2)
            }
            AlignTarget::World(iso) => (self.world_anchor(), iso),
        };

        // Roblox align constraints are reactionless: the target assembly
        // feels nothing. A joint to a dynamic target would yank it, so the
        // joint attaches to the fixed world anchor instead, with its anchor
        // frame re-sampled from the target's pose every tick. This also
        // covers targets on the driven body itself.
        if self.is_dynamic_body(body2) {
            let target_world = match self.rigid_body_set.get(body2) {
                Some(body) => body.position() * frame2,
                None => {
                    self.remove_align_motor(spec.align_id);
                    return false;
                }
            };
            body2 = self.world_anchor();
            frame2 = target_world;
        }

        let world1 = match self.rigid_body_set.get(body1) {
            Some(body) => body.position() * frame1,
            None => return false,
        };
        let world2 = match self.rigid_body_set.get(body2) {
            Some(body) => body.position() * frame2,
            None => return false,
        };

        let should_wake = match spec.kind {
            AlignKind::Position => {
                let error = world2.translation.vector - world1.translation.vector;
                let distance = error.norm();

                // MaxVelocity: rate-limit the target the motor sees so the
                // steady-state approach speed (responsiveness * error) cannot
                // exceed it.
                if spec.max_velocity > 0.0 && spec.responsiveness > 0.0 {
                    let max_error = spec.max_velocity / spec.responsiveness;
                    if distance > max_error {
                        let limited = world1.translation.vector + error * (max_error / distance);
                        let world2_limited = Isometry::from_parts(limited.into(), world2.rotation);
                        if let Some(body) = self.rigid_body_set.get(body2) {
                            frame2 = body.position().inverse() * world2_limited;
                        }
                    }
                }
                distance > POSITION_WAKE_EPSILON
            }
            AlignKind::Orientation => {
                world1.rotation.angle_to(&world2.rotation) > ORIENTATION_WAKE_EPSILON
            }
        };

        // Acceleration-based motors: response is independent of the driven
        // assembly's mass and inertia, matching Roblox's Responsiveness.
        let stiffness = spec.responsiveness * spec.responsiveness;
        let damping = 2.0 * spec.responsiveness;
        let axes = match spec.kind {
            AlignKind::Position => &LINEAR_AXES,
            AlignKind::Orientation => &ANGULAR_AXES,
        };

        let reuse = self
            .align_joints
            .get(&spec.align_id)
            .filter(|entry| entry.body1 == body1 && entry.body2 == body2 && entry.kind == spec.kind)
            .map(|entry| entry.handle)
            .filter(|&handle| self.impulse_joint_set.get(handle).is_some());

        if let Some(handle) = reuse {
            if let Some(joint) = self.impulse_joint_set.get_mut(handle, should_wake) {
                joint.data.set_local_frame1(frame1);
                joint.data.set_local_frame2(frame2);
                for (axis, max) in axes.iter().zip(spec.max_output) {
                    joint
                        .data
                        .set_motor_position(*axis, 0.0, stiffness, damping)
                        .set_motor_max_force(*axis, max);
                }
            }
            return true;
        }

        self.remove_align_motor(spec.align_id);
        let mut builder = GenericJointBuilder::new(JointAxesMask::empty())
            .local_frame1(frame1)
            .local_frame2(frame2);
        for (axis, max) in axes.iter().zip(spec.max_output) {
            builder = builder
                .motor_model(*axis, MotorModel::AccelerationBased)
                .motor_position(*axis, 0.0, stiffness, damping)
                .motor_max_force(*axis, max);
        }
        let handle = self.impulse_joint_set.insert(body1, body2, builder, true);
        self.align_joints.insert(
            spec.align_id,
            AlignJointEntry {
                handle,
                body1,
                body2,
                kind: spec.kind,
            },
        );
        true
    }

    /// Removes the motor joint backing one align constraint, if any.
    pub fn remove_align_motor(&mut self, align_id: u64) {
        if let Some(entry) = self.align_joints.remove(&align_id) {
            self.impulse_joint_set.remove(entry.handle, true);
        }
    }

    /// Removes motor joints whose align constraint no longer exists (or is
    /// no longer in motor mode) this tick.
    pub fn retain_align_motors(&mut self, live: &HashSet<u64>) {
        let stale: Vec<u64> = self
            .align_joints
            .keys()
            .filter(|align_id| !live.contains(align_id))
            .copied()
            .collect();
        for align_id in stale {
            self.remove_align_motor(align_id);
        }
    }
}
