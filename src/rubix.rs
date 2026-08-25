use crate::piece::{Color, Piece};
use crate::shape::{Move, Shape};
use crate::vec3::{LatticeKey, Vec3};

/// The whole puzzle, generic over any `Shape`. Pieces are the source of truth;
/// faces are always derived, never stored, so a rotation can never leave them
/// inconsistent.
pub struct Rubix {
    shape: Box<dyn Shape>,
    pieces: Vec<Piece>,
}

impl Rubix {
    pub fn solved(shape: Box<dyn Shape>) -> Self {
        let pieces = shape.solved_pieces();
        Rubix { shape, pieces }
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn moves(&self) -> Vec<Move> {
        self.shape.moves()
    }

    /// The shared rotation core: spins every piece `selector` accepts by `angle_degrees`
    /// around `axis`. This one function is what makes a face turn touch its 4 neighbors
    /// correctly — there's no separate face-patching step, because faces are never stored.
    pub fn rotate(&mut self, selector: impl Fn(&Piece) -> bool, axis: Vec3, angle_degrees: f64) {
        for piece in &mut self.pieces {
            if !selector(piece) {
                continue;
            }

            piece.position = piece.position.rotate_about(axis, angle_degrees).snapped_to_lattice();

            piece.stickers = piece
                .stickers
                .drain()
                .map(|(dir_key, color)| {
                    let rotated_dir = Vec3::from(dir_key)
                        .rotate_about(axis, angle_degrees)
                        .snapped_to_lattice();
                    (LatticeKey::from(rotated_dir), color)
                })
                .collect();
        }
    }

    /// Applies one of the shape's legal moves, clockwise or counterclockwise.
    pub fn apply(&mut self, m: &Move, clockwise: bool) {
        let angle = if clockwise { m.angle_degrees } else { -m.angle_degrees };
        let selector = &m.selector;
        self.rotate(|piece| selector(piece), m.axis, angle);
    }

    /// Derived view: every (position, color) currently showing on the given face.
    pub fn face(&self, direction: Vec3) -> Vec<(Vec3, Color)> {
        let key = LatticeKey::from(direction);
        self.pieces
            .iter()
            .filter_map(|p| p.stickers.get(&key).map(|color| (p.position, *color)))
            .collect()
    }
}
