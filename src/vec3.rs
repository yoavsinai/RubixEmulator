use std::hash::{Hash, Hasher};

/// A 3D vector used for both piece positions and sticker-facing directions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Vec3 {
        let len = self.length();
        Vec3::new(self.x / len, self.y / len, self.z / len)
    }

    /// Rotates this vector by `angle_degrees` around `axis`, using Rodrigues' rotation formula.
    pub fn rotate_about(self, axis: Vec3, angle_degrees: f64) -> Vec3 {
        let axis = axis.normalized();
        let angle = angle_degrees.to_radians();
        let (sin, cos) = angle.sin_cos();

        let cross = Vec3::new(
            axis.y * self.z - axis.z * self.y,
            axis.z * self.x - axis.x * self.z,
            axis.x * self.y - axis.y * self.x,
        );
        let axis_dot_self = axis.dot(self);

        Vec3::new(
            self.x * cos + cross.x * sin + axis.x * axis_dot_self * (1.0 - cos),
            self.y * cos + cross.y * sin + axis.y * axis_dot_self * (1.0 - cos),
            self.z * cos + cross.z * sin + axis.z * axis_dot_self * (1.0 - cos),
        )
    }

    /// Rounds each component to the nearest integer, undoing float drift from repeated rotations.
    pub fn snapped_to_lattice(self) -> Vec3 {
        Vec3::new(self.x.round(), self.y.round(), self.z.round())
    }
}

/// A hashable, exact stand-in for a lattice-aligned `Vec3` (used as a sticker map key).
/// Built from a `Vec3` that has already been snapped to the lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatticeKey {
    x: i64,
    y: i64,
    z: i64,
}

impl From<Vec3> for LatticeKey {
    fn from(v: Vec3) -> Self {
        let snapped = v.snapped_to_lattice();
        LatticeKey {
            x: snapped.x as i64,
            y: snapped.y as i64,
            z: snapped.z as i64,
        }
    }
}

impl From<LatticeKey> for Vec3 {
    fn from(k: LatticeKey) -> Self {
        Vec3::new(k.x as f64, k.y as f64, k.z as f64)
    }
}

impl Hash for LatticeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

/// The 6 axis-aligned unit directions used by a `Cuboid` shape.
pub mod direction {
    use super::Vec3;

    pub const POS_X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const NEG_X: Vec3 = Vec3::new(-1.0, 0.0, 0.0);
    pub const POS_Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const NEG_Y: Vec3 = Vec3::new(0.0, -1.0, 0.0);
    pub const POS_Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
    pub const NEG_Z: Vec3 = Vec3::new(0.0, 0.0, -1.0);
}
