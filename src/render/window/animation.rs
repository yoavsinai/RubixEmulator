//! The visual sweep of a single layer turn. The move is applied to the model up front, so
//! this only rotates the affected meshes from one quarter-turn behind their final pose up
//! to it.

use three_d::*;

use crate::rubix::Rubix;
use crate::shape::Move;

use super::scene::Scene;

/// How long a single layer turn takes to sweep from start to finish.
const TURN_ANIMATION_MS: f64 = 140.0;

pub(crate) struct TurnAnimation {
    /// The turn axis, a unit vector in scene space (same as the move's engine axis).
    axis: Vec3,
    /// The signed angle already applied to the model, in degrees.
    applied_degrees: f32,
    /// Per-object (parallel to `Scene::objects`): is this mesh part of the turning layer?
    affected: Vec<bool>,
    /// Per-object: its final transform, to rotate away from and settle back onto.
    base: Vec<Mat4>,
    elapsed_ms: f64,
}

impl TurnAnimation {
    /// Begins animating `move_`, which has just been applied to `rubix` (rotating the model
    /// by the signed `applied_degrees`), across a freshly built `scene`.
    pub fn start(scene: &Scene, rubix: &Rubix, move_: &Move, applied_degrees: f32) -> Self {
        let affected = scene
            .piece_index
            .iter()
            .map(|&pi| (move_.selector)(&rubix.pieces()[pi]))
            .collect();
        let base = scene.objects.iter().map(|o| o.transformation()).collect();
        Self {
            axis: vec3(move_.axis.x as f32, move_.axis.y as f32, move_.axis.z as f32),
            applied_degrees,
            affected,
            base,
            elapsed_ms: 0.0,
        }
    }

    /// Advances by `dt_ms` and writes the interpolated transforms into `scene`. Returns
    /// `true` once the turn has settled onto its final pose and the animation can be dropped.
    pub fn advance(&mut self, scene: &mut Scene, dt_ms: f64) -> bool {
        self.elapsed_ms += dt_ms;
        let t = (self.elapsed_ms / TURN_ANIMATION_MS).min(1.0) as f32;
        let eased = 1.0 - (1.0 - t).powi(3);
        let angle = -self.applied_degrees * (1.0 - eased);
        let rotation = Mat4::from_axis_angle(self.axis, degrees(angle));

        for (i, &affected) in self.affected.iter().enumerate() {
            if affected {
                scene.objects[i].set_transformation(rotation * self.base[i]);
            }
        }

        t >= 1.0
    }
}
