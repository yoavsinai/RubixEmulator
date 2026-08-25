use std::collections::HashSet;

use crate::piece::{Color, Piece};
use crate::shape::{Move, Shape};
use crate::vec3::{LatticeKey, Vec3};

/// One rotation applied through `apply`, recorded so `solve` can undo it later. Doesn't
/// store the `Move` itself (its selector is an opaque, non-`Clone` closure) — instead
/// records exactly which pieces ended up touched and where, which is enough to reselect
/// that same set and spin it back by the opposite angle.
struct HistoryEntry {
    axis: Vec3,
    angle_degrees: f64,
    positions_after: Vec<Vec3>,
}

/// The whole puzzle, generic over any `Shape`. Pieces are the source of truth;
/// faces are always derived, never stored, so a rotation can never leave them
/// inconsistent.
pub struct Rubix {
    shape: Box<dyn Shape>,
    pieces: Vec<Piece>,
    history: Vec<HistoryEntry>,
}

impl Rubix {
    pub fn solved(shape: Box<dyn Shape>) -> Self {
        let pieces = shape.solved_pieces();
        Rubix { shape, pieces, history: Vec::new() }
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

        let touched: Vec<usize> = self
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, piece)| selector(piece))
            .map(|(i, _)| i)
            .collect();

        self.rotate(|piece| selector(piece), m.axis, angle);

        let positions_after = touched.iter().map(|&i| self.pieces[i].position).collect();
        self.history.push(HistoryEntry { axis: m.axis, angle_degrees: angle, positions_after });
    }

    /// Undoes every move applied since this `Rubix` was created (or since the last
    /// `solve`), restoring it to its solved state — moves are unwound in exact reverse
    /// order, each by re-selecting the same pieces it touched (identified by where they
    /// ended up) and spinning them back by the opposite angle.
    pub fn solve(&mut self) {
        while let Some(entry) = self.history.pop() {
            let target: HashSet<LatticeKey> =
                entry.positions_after.iter().map(|&p| LatticeKey::from(p)).collect();

            self.rotate(
                |piece| target.contains(&LatticeKey::from(piece.position)),
                entry.axis,
                -entry.angle_degrees,
            );
        }
    }

    /// Applies `move_count` random legal moves. Takes a `random` closure — given an
    /// exclusive upper bound, it returns a value in `0..bound` — rather than reaching for
    /// a random number generator itself, so the engine stays dependency-free and this is
    /// deterministically testable; callers that need real randomness supply their own RNG.
    pub fn scramble(&mut self, move_count: usize, mut random: impl FnMut(usize) -> usize) {
        let moves = self.moves();
        for _ in 0..move_count {
            let m = &moves[random(moves.len())];
            let clockwise = random(2) == 0;
            self.apply(m, clockwise);
        }
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
