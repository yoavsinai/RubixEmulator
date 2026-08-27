//! Reading a mouse drag across a sticker as a layer turn.

use three_d::*;

use crate::render::geometry::perpendicular_axes;
use crate::rubix::Rubix;
use crate::shape::Move;
use crate::vec3::{LatticeKey, Vec3 as ModelVec3};

use super::scene::StickerRef;

/// How far (in physical pixels) the cursor must travel across a sticker before the drag is
/// read as a layer turn rather than a stray click.
const MIN_DRAG_PIXELS: f32 = 6.0;

/// A minimum alignment (dot product of unit screen vectors) between the drag and a
/// candidate turn's on-screen motion, below which the drag is treated as ambiguous.
const MIN_ALIGNMENT: f32 = 0.2;

/// A layer turn the user is dragging out: they pressed on a sticker and are moving the
/// cursor.
pub(crate) struct DragState {
    sticker: StickerRef,
    /// The pick's world-space hit point, in centered scene coords.
    hit: Vec3,
    start_pixel: (f32, f32),
    current_pixel: (f32, f32),
}

impl DragState {
    pub fn begin(sticker: StickerRef, hit: Vec3, pixel: (f32, f32)) -> Self {
        Self { sticker, hit, start_pixel: pixel, current_pixel: pixel }
    }

    pub fn move_to(&mut self, pixel: (f32, f32)) {
        self.current_pixel = pixel;
    }

    /// Resolves the finished drag to a `(move name, clockwise)` pair, or `None` if it was
    /// too small or too ambiguous.
    ///
    /// Method: for each of the four possible quarter-turns of a layer through the picked
    /// face (the two in-face axes, each direction), rotate the hit point and project it
    /// back to the screen; the candidate whose screen motion best lines up with the actual
    /// drag wins. The layer is then whichever move's selector grabs the picked piece.
    pub fn resolve(&self, camera: &Camera, moves: &[Move], rubix: &Rubix) -> Option<(String, bool)> {
        let screen_drag = vec2(
            self.current_pixel.0 - self.start_pixel.0,
            self.current_pixel.1 - self.start_pixel.1,
        );
        if screen_drag.magnitude() < MIN_DRAG_PIXELS {
            return None;
        }
        let drag_dir = screen_drag.normalize();

        let (axis_a, axis_b) = perpendicular_axes(self.sticker.normal);
        let hit = ModelVec3::new(self.hit.x as f64, self.hit.y as f64, self.hit.z as f64);
        let hit_pixel = camera.pixel_at_position(self.hit);

        let mut best: Option<(ModelVec3, bool, f32)> = None;
        for axis in [axis_a, axis_b] {
            for clockwise in [true, false] {
                let angle = if clockwise { 90.0 } else { -90.0 };
                let moved = hit.rotate_about(axis, angle);
                let moved_pixel = camera
                    .pixel_at_position(vec3(moved.x as f32, moved.y as f32, moved.z as f32));
                let screen_delta = vec2(moved_pixel.x - hit_pixel.x, moved_pixel.y - hit_pixel.y);
                if screen_delta.magnitude() < 1e-3 {
                    continue;
                }
                let score = drag_dir.dot(screen_delta.normalize());
                if best.is_none_or(|(_, _, best_score)| score > best_score) {
                    best = Some((axis, clockwise, score));
                }
            }
        }

        let (axis, clockwise, score) = best?;
        if score < MIN_ALIGNMENT {
            return None;
        }

        let picked = LatticeKey::from(self.sticker.piece_position);
        let piece = rubix.pieces().iter().find(|p| LatticeKey::from(p.position) == picked)?;
        let m = moves
            .iter()
            .find(|m| LatticeKey::from(m.axis) == LatticeKey::from(axis) && (m.selector)(piece))?;
        Some((m.name.clone(), clockwise))
    }
}
