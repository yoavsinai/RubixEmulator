use crate::piece::{Color, Piece};
use crate::shape::{Move, Shape};
use crate::vec3::{direction, Vec3};

/// A rectangular box puzzle, X x Y x Z pieces wide (not necessarily equal).
pub struct Cuboid {
    pub dims: (usize, usize, usize),
}

impl Cuboid {
    pub fn new(x: usize, y: usize, z: usize) -> Self {
        Cuboid { dims: (x, y, z) }
    }
}

impl Shape for Cuboid {
    fn solved_pieces(&self) -> Vec<Piece> {
        let (dim_x, dim_y, dim_z) = self.dims;
        let mut pieces = Vec::new();

        for x in 0..dim_x {
            for y in 0..dim_y {
                for z in 0..dim_z {
                    let position = Vec3::new(x as f64, y as f64, z as f64);
                    let mut piece = Piece::new(position);

                    if x == 0 {
                        piece = piece.with_sticker(direction::NEG_X, Color::Orange);
                    }
                    if x == dim_x - 1 {
                        piece = piece.with_sticker(direction::POS_X, Color::Red);
                    }
                    if y == 0 {
                        piece = piece.with_sticker(direction::NEG_Y, Color::White);
                    }
                    if y == dim_y - 1 {
                        piece = piece.with_sticker(direction::POS_Y, Color::Yellow);
                    }
                    if z == 0 {
                        piece = piece.with_sticker(direction::NEG_Z, Color::Blue);
                    }
                    if z == dim_z - 1 {
                        piece = piece.with_sticker(direction::POS_Z, Color::Green);
                    }

                    if !piece.stickers.is_empty() {
                        pieces.push(piece);
                    }
                }
            }
        }

        pieces
    }

    fn moves(&self) -> Vec<Move> {
        let (dim_x, dim_y, dim_z) = self.dims;
        let mut moves = Vec::new();

        for (axis, dim, pos_letter, neg_letter, mid_letter) in [
            (direction::POS_X, dim_x, "R", "L", "M"),
            (direction::POS_Y, dim_y, "U", "D", "E"),
            (direction::POS_Z, dim_z, "F", "B", "S"),
        ] {
            for layer_index in 0..dim {
                let name = layer_name(dim, layer_index, pos_letter, neg_letter, mid_letter);
                let layer_index = layer_index as f64;
                moves.push(Move {
                    name,
                    axis,
                    angle_degrees: 90.0,
                    selector: Box::new(move |piece: &Piece| piece.position.dot(axis) == layer_index),
                });
            }
        }

        moves
    }
}

/// Standard WCA/TNoodle-style NxN single-layer notation: `R`/`2R`/`3R`/...
/// counting depth in from the positive face, `L`/`2L`/`3L`/... from the
/// negative face. For a dimension of exactly 3, the middle layer uses the
/// classic 3x3 slice name (`M`/`E`/`S`) instead of a numbered layer.
fn layer_name(dim: usize, layer_index: usize, pos_letter: &str, neg_letter: &str, mid_letter: &str) -> String {
    if dim == 3 && layer_index == 1 {
        return mid_letter.to_string();
    }

    let half = dim / 2;
    if layer_index < half {
        let depth = layer_index + 1;
        if depth == 1 {
            neg_letter.to_string()
        } else {
            format!("{depth}{neg_letter}")
        }
    } else {
        let depth = dim - layer_index;
        if depth == 1 {
            pos_letter.to_string()
        } else {
            format!("{depth}{pos_letter}")
        }
    }
}
