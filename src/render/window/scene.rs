//! The drawable puzzle: turning the model's pieces into `three-d` meshes, and the
//! bookkeeping that lets a mouse pick and a turn animation refer back to pieces.

use three_d::*;

use crate::piece::Color as PieceColor;
use crate::render::projection::cube_center_offset;
use crate::rubix::Rubix;
use crate::vec3::Vec3 as ModelVec3;

/// `Cuboid` spaces adjacent layers 2 raw units apart; dividing by this normalizes to one
/// world unit per cubie so the scene's scale doesn't depend on that engine-internal choice.
const WORLD_UNITS_PER_CUBIE: f32 = 2.0;
const CUBIE_HALF: f32 = 0.47;
const STICKER_HALF: f32 = 0.40;
const STICKER_THICKNESS: f32 = 0.04;

/// Which sticker a drawn mesh belongs to, so a mouse pick can name the piece and the face
/// it hit. Kept parallel to [`Scene::objects`].
#[derive(Clone, Copy)]
pub(crate) struct StickerRef {
    /// The piece's position, in the engine's own (doubled, centered) lattice coords.
    pub piece_position: ModelVec3,
    /// The outward face normal, an axis-aligned unit vector in engine coords.
    pub normal: ModelVec3,
}

/// One mesh per cubie body plus one per visible sticker, all centered on the origin, with
/// per-object bookkeeping kept in lockstep with `objects`.
pub(crate) struct Scene {
    pub objects: Vec<Gm<Mesh, PhysicalMaterial>>,
    /// The sticker behind each object (`None` for a body), indexed by the pick's reported
    /// geometry id.
    pub sticker_refs: Vec<Option<StickerRef>>,
    /// The index into `rubix.pieces()` of the piece each object belongs to, so a turn
    /// animation can ask a move's selector which meshes are in the moving layer.
    pub piece_index: Vec<usize>,
}

impl Scene {
    /// Builds the drawable puzzle from the current model state.
    pub fn build(context: &Context, rubix: &Rubix) -> Self {
        let offset = cube_center_offset(rubix.pieces());
        let body_material = PhysicalMaterial::new_opaque(
            context,
            &CpuMaterial {
                albedo: Srgba::new_opaque(20, 20, 22),
                roughness: 0.8,
                metallic: 0.0,
                ..Default::default()
            },
        );

        let mut objects = Vec::new();
        let mut sticker_refs: Vec<Option<StickerRef>> = Vec::new();
        let mut piece_index: Vec<usize> = Vec::new();

        for (pi, piece) in rubix.pieces().iter().enumerate() {
            let center = vec3(
                ((piece.position.x - offset.x) as f32) / WORLD_UNITS_PER_CUBIE,
                ((piece.position.y - offset.y) as f32) / WORLD_UNITS_PER_CUBIE,
                ((piece.position.z - offset.z) as f32) / WORLD_UNITS_PER_CUBIE,
            );

            let mut body = Gm::new(Mesh::new(context, &CpuMesh::cube()), body_material.clone());
            body.set_transformation(
                Mat4::from_translation(center)
                    * Mat4::from_nonuniform_scale(CUBIE_HALF, CUBIE_HALF, CUBIE_HALF),
            );
            objects.push(body);
            sticker_refs.push(None);
            piece_index.push(pi);

            for (&dir_key, &color) in &piece.stickers {
                let normal = ModelVec3::from(dir_key);
                let dir = vec3(normal.x as f32, normal.y as f32, normal.z as f32);
                let (sx, sy, sz) = (tile_extent(dir.x), tile_extent(dir.y), tile_extent(dir.z));
                let mut tile = Gm::new(
                    Mesh::new(context, &CpuMesh::cube()),
                    PhysicalMaterial::new_opaque(
                        context,
                        &CpuMaterial {
                            albedo: sticker_srgba(color),
                            roughness: 0.4,
                            metallic: 0.0,
                            ..Default::default()
                        },
                    ),
                );
                tile.set_transformation(
                    Mat4::from_translation(center + dir * CUBIE_HALF)
                        * Mat4::from_nonuniform_scale(sx, sy, sz),
                );
                objects.push(tile);
                sticker_refs.push(Some(StickerRef { piece_position: piece.position, normal }));
                piece_index.push(pi);
            }
        }

        Scene { objects, sticker_refs, piece_index }
    }
}

/// Half-extent of a sticker tile along one axis: thin along the face normal, wide otherwise.
fn tile_extent(component: f32) -> f32 {
    if component.abs() > 0.5 {
        STICKER_THICKNESS
    } else {
        STICKER_HALF
    }
}

fn sticker_srgba(color: PieceColor) -> Srgba {
    match color {
        PieceColor::White => Srgba::new_opaque(235, 235, 235),
        PieceColor::Yellow => Srgba::new_opaque(255, 213, 0),
        PieceColor::Red => Srgba::new_opaque(200, 0, 0),
        PieceColor::Orange => Srgba::new_opaque(255, 128, 0),
        PieceColor::Blue => Srgba::new_opaque(0, 60, 220),
        PieceColor::Green => Srgba::new_opaque(0, 158, 0),
    }
}
