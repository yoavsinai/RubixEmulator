use crate::piece::{Color, Piece};
use crate::render::camera::Camera;
use crate::vec3::Vec3;

/// A projected point in character-cell screen space, plus its camera-space depth
/// (larger depth = farther from the camera).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
    pub depth: f64,
}

/// One visible sticker face, projected to a screen-space quad, ready to be rasterized.
pub struct StickerQuad {
    pub color: Color,
    pub corners: [ScreenPoint; 4],
    pub depth: f64,
}

/// Terminal character cells are roughly twice as tall as they are wide, so vertical
/// screen distances are compressed to keep the projection visually square.
const CHAR_ASPECT: f64 = 0.5;

const COS_30: f64 = 0.866_025_403_784_438_6;
const SIN_30: f64 = 0.5;

/// Projects a world-space point into isometric screen space via the camera.
pub fn project_point(world: Vec3, camera: &Camera) -> ScreenPoint {
    let view = camera.view_position(world);
    ScreenPoint {
        x: (view.x - view.z) * COS_30 * camera.zoom,
        y: ((view.x + view.z) * SIN_30 - view.y) * camera.zoom * CHAR_ASPECT,
        // Camera looks down -Z in view space, so smaller (more negative) view.z is nearer;
        // depth increases the farther a point is from the camera.
        depth: -view.z,
    }
}

/// True if a sticker facing `sticker_dir` (world space) is visible to the camera.
pub fn is_facing_camera(sticker_dir: Vec3, camera: &Camera) -> bool {
    sticker_dir.dot(camera.forward()) > 0.0
}

/// The two unit axes perpendicular to `direction` (itself axis-aligned), used to find
/// a sticker's four corners.
fn perpendicular_axes(direction: Vec3) -> (Vec3, Vec3) {
    if direction.x != 0.0 {
        (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    } else if direction.y != 0.0 {
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    } else {
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
    }
}

/// Builds the projected, camera-facing sticker quads for every piece, ready to be
/// depth-sorted and rasterized. `cube_center_offset` recenters the shape's corner-origin
/// piece grid around the origin before projecting.
pub fn build_sticker_quads(
    pieces: &[Piece],
    camera: &Camera,
    cube_center_offset: Vec3,
) -> Vec<StickerQuad> {
    let mut quads = Vec::new();

    for piece in pieces {
        let centered_position = Vec3::new(
            piece.position.x - cube_center_offset.x,
            piece.position.y - cube_center_offset.y,
            piece.position.z - cube_center_offset.z,
        );

        for (&dir_key, &color) in &piece.stickers {
            let direction: Vec3 = dir_key.into();
            if !is_facing_camera(direction, camera) {
                continue;
            }

            let (u, v) = perpendicular_axes(direction);
            let center = Vec3::new(
                centered_position.x + direction.x * 0.5,
                centered_position.y + direction.y * 0.5,
                centered_position.z + direction.z * 0.5,
            );

            let corner_offsets = [
                (0.5, 0.5),
                (0.5, -0.5),
                (-0.5, -0.5),
                (-0.5, 0.5),
            ];
            let corners: [ScreenPoint; 4] = std::array::from_fn(|i| {
                let (su, sv) = corner_offsets[i];
                let corner_world = Vec3::new(
                    center.x + u.x * su + v.x * sv,
                    center.y + u.y * su + v.y * sv,
                    center.z + u.z * su + v.z * sv,
                );
                project_point(corner_world, camera)
            });

            let depth = project_point(center, camera).depth;

            quads.push(StickerQuad { color, corners, depth });
        }
    }

    quads
}
