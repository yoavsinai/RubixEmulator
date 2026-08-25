use crate::piece::Piece;
use crate::vec3::Vec3;

/// One legal twist: which pieces it grabs, and the axis/angle it spins them by.
/// `angle_degrees` is the clockwise angle; `Rubix::apply` negates it for counterclockwise.
pub struct Move {
    pub name: String,
    pub axis: Vec3,
    pub angle_degrees: f64,
    pub selector: Box<dyn Fn(&Piece) -> bool>,
}

/// The geometry of a twisty puzzle: what its solved piece layout looks like,
/// and which twists are legal on it. Everything else (the rotation engine,
/// face derivation) is shared code that works off whatever a `Shape` produces.
pub trait Shape {
    fn solved_pieces(&self) -> Vec<Piece>;
    fn moves(&self) -> Vec<Move>;
}
