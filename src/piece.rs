use std::collections::HashMap;

use crate::vec3::{LatticeKey, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Yellow,
    Red,
    Orange,
    Blue,
    Green,
}

/// A single cubie: where it currently sits, and which color currently faces which direction.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub position: Vec3,
    pub stickers: HashMap<LatticeKey, Color>,
}

impl Piece {
    pub fn new(position: Vec3) -> Self {
        Piece {
            position,
            stickers: HashMap::new(),
        }
    }

    pub fn with_sticker(mut self, direction: Vec3, color: Color) -> Self {
        self.stickers.insert(LatticeKey::from(direction), color);
        self
    }

    pub fn sticker_at(&self, direction: Vec3) -> Option<Color> {
        self.stickers.get(&LatticeKey::from(direction)).copied()
    }
}
