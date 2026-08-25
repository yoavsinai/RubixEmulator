use crate::vec3::{direction, Vec3};

const MAX_ELEVATION_DEGREES: f64 = 89.0;
const MIN_ELEVATION_DEGREES: f64 = -89.0;

/// An orbiting camera: rotates around the puzzle at a fixed distance, looking at the origin.
pub struct Camera {
    pub azimuth_degrees: f64,
    pub elevation_degrees: f64,
    pub zoom: f64,
}

impl Camera {
    /// Classic isometric-ish starting angle.
    pub fn new() -> Self {
        Camera {
            azimuth_degrees: 45.0,
            elevation_degrees: 35.264,
            zoom: 1.0,
        }
    }

    pub fn orbit(&mut self, delta_azimuth: f64, delta_elevation: f64) {
        self.azimuth_degrees += delta_azimuth;
        self.elevation_degrees = (self.elevation_degrees + delta_elevation)
            .clamp(MIN_ELEVATION_DEGREES, MAX_ELEVATION_DEGREES);
    }

    /// Rotates a world-space point into camera space: camera looks down -Z afterward.
    pub fn view_position(&self, world_pos: Vec3) -> Vec3 {
        world_pos
            .rotate_about(direction::POS_Y, -self.azimuth_degrees)
            .rotate_about(direction::POS_X, -self.elevation_degrees)
    }

    /// Unit vector from the origin toward the camera, in world space.
    pub fn forward(&self) -> Vec3 {
        direction::POS_Z
            .rotate_about(direction::POS_X, self.elevation_degrees)
            .rotate_about(direction::POS_Y, self.azimuth_degrees)
    }
}
