//! Small pure-geometry helpers shared by both renderers (the terminal rasterizer and the
//! `three-d` window).

use crate::piece::Piece;
use crate::vec3::Vec3;

/// The two unit axes perpendicular to `direction` (itself axis-aligned): the terminal
/// renderer uses them to find a sticker's four corners, the window to pick a drag's turn
/// axis.
pub fn perpendicular_axes(direction: Vec3) -> (Vec3, Vec3) {
    if direction.x != 0.0 {
        (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    } else if direction.y != 0.0 {
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    } else {
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
    }
}

/// The center of a piece cloud's bounding box. Both renderers use it to recenter a shape's
/// corner-origin piece grid on the world origin before projecting.
pub fn cube_center_offset(pieces: &[Piece]) -> Vec3 {
    let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for piece in pieces {
        let p = piece.position;
        min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    Vec3::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0, (min.z + max.z) / 2.0)
}
