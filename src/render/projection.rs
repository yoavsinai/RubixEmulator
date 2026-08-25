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

/// What a projected quad represents: an actual sticker, or a cubie's opaque plastic
/// backing (drawn full-size, behind the sticker, so gaps between inset stickers still
/// show solid cube body instead of letting far/interior geometry show through).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuadKind {
    Sticker(Color),
    Body,
}

/// One projected quad — a sticker or a body backing — ready to be depth-tested and
/// rasterized. Every quad is emitted regardless of which way it faces; correct occlusion
/// is handled entirely by per-pixel depth comparison at draw time, not by pre-filtering.
pub struct StickerQuad {
    pub kind: QuadKind,
    pub corners: [ScreenPoint; 4],
    pub depth: f64,
}

const COS_30: f64 = 0.866_025_403_784_438_6;
const SIN_30: f64 = 0.5;

/// How far each sticker's edge is pulled in from the cubie's true edge, in cubie-widths.
const STICKER_INSET: f64 = 0.07;

/// How much farther than its sticker a cubie's body backing sits, so the sticker always
/// wins the depth test over its own backing where they overlap.
const BODY_DEPTH_EPSILON: f64 = 1e-4;

/// Projects a world-space point into isometric screen units (aspect-neutral — the caller
/// maps these to actual terminal cells/sub-cells and corrects for their aspect ratio).
pub fn project_point(world: Vec3, camera: &Camera) -> ScreenPoint {
    let view = camera.view_position(world);
    ScreenPoint {
        x: (view.x - view.z) * COS_30 * camera.zoom,
        y: ((view.x + view.z) * SIN_30 - view.y) * camera.zoom,
        // Camera looks down -Z in view space, so smaller (more negative) view.z is nearer;
        // depth increases the farther a point is from the camera.
        depth: -view.z,
    }
}

/// True if a sticker facing `sticker_dir` (world space) is oriented toward the camera.
/// Not used for occlusion (that's handled by per-pixel depth testing) — kept as a general
/// utility, e.g. for tests that reason about a single face's orientation.
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

fn face_corners(center: Vec3, u: Vec3, v: Vec3, half_extent: f64, camera: &Camera) -> [ScreenPoint; 4] {
    let corner_offsets = [
        (half_extent, half_extent),
        (half_extent, -half_extent),
        (-half_extent, -half_extent),
        (-half_extent, half_extent),
    ];
    std::array::from_fn(|i| {
        let (su, sv) = corner_offsets[i];
        let corner_world = Vec3::new(
            center.x + u.x * su + v.x * sv,
            center.y + u.y * su + v.y * sv,
            center.z + u.z * su + v.z * sv,
        );
        project_point(corner_world, camera)
    })
}

/// Builds every piece's projected quads — every sticker (all 6 sides always considered,
/// no directional culling) plus a full-size opaque body backing behind each one.
/// `cube_center_offset` recenters the shape's corner-origin piece grid around the origin
/// before projecting. The caller is expected to resolve visibility with a per-pixel depth
/// buffer rather than relying on draw order.
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
            let (u, v) = perpendicular_axes(direction);
            let center = Vec3::new(
                centered_position.x + direction.x * 0.5,
                centered_position.y + direction.y * 0.5,
                centered_position.z + direction.z * 0.5,
            );

            let depth = project_point(center, camera).depth;

            // Full-size opaque backing first, so a gap in the inset sticker still shows
            // solid cube plastic rather than whatever geometry happens to lie behind it.
            // Every corner (not just the center) is pushed back by the epsilon, so the
            // backing stays farther than its sticker everywhere the two overlap, even
            // once depth is interpolated per-pixel across the tilted face.
            let mut body_corners = face_corners(center, u, v, 0.5, camera);
            for corner in &mut body_corners {
                corner.depth += BODY_DEPTH_EPSILON;
            }
            quads.push(StickerQuad {
                kind: QuadKind::Body,
                corners: body_corners,
                depth: depth + BODY_DEPTH_EPSILON,
            });

            // Shrunk sticker on top, leaving a thin backing-colored gap between
            // neighboring cubies that reads as a grid line.
            quads.push(StickerQuad {
                kind: QuadKind::Sticker(color),
                corners: face_corners(center, u, v, 0.5 - STICKER_INSET, camera),
                depth,
            });
        }
    }

    quads
}
