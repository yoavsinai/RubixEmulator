use rubixemulator::piece::Piece;
use rubixemulator::render::projection::{build_sticker_quads, is_facing_camera, project_point};
use rubixemulator::render::Camera;
use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;
use rubixemulator::vec3::{direction, Vec3};

#[test]
fn camera_default_angles_are_classic_isometric() {
    let camera = Camera::new();
    assert_eq!(camera.azimuth_degrees, 45.0);
    assert!((camera.elevation_degrees - 35.264).abs() < 1e-6);
}

#[test]
fn camera_orbit_clamps_elevation() {
    let mut camera = Camera::new();
    camera.orbit(0.0, 1000.0);
    assert_eq!(camera.elevation_degrees, 89.0);

    camera.orbit(0.0, -1000.0);
    assert_eq!(camera.elevation_degrees, -89.0);
}

#[test]
fn camera_orbit_accumulates_azimuth_unclamped() {
    let mut camera = Camera::new();
    camera.orbit(30.0, 0.0);
    assert_eq!(camera.azimuth_degrees, 75.0);
}

#[test]
fn project_point_places_origin_at_screen_origin() {
    let camera = Camera::new();
    let projected = project_point(Vec3::new(0.0, 0.0, 0.0), &camera);
    assert!(projected.x.abs() < 1e-9);
    assert!(projected.y.abs() < 1e-9);
    assert!(projected.depth.abs() < 1e-9);
}

#[test]
fn project_point_moves_right_when_orbited() {
    // With the camera looking down the default isometric angle, a point on the +X axis
    // should not project to the same screen position as the origin.
    let camera = Camera::new();
    let origin = project_point(Vec3::new(0.0, 0.0, 0.0), &camera);
    let plus_x = project_point(Vec3::new(1.0, 0.0, 0.0), &camera);
    assert!((plus_x.x - origin.x).abs() > 1e-6);
}

#[test]
fn is_facing_camera_true_for_face_toward_camera_false_for_face_away() {
    let camera = Camera::new();
    let toward = camera.forward();
    let away = Vec3::new(-toward.x, -toward.y, -toward.z);

    assert!(is_facing_camera(toward, &camera));
    assert!(!is_facing_camera(away, &camera));
}

#[test]
fn is_facing_camera_culls_opposite_axis_aligned_stickers() {
    let camera = Camera::new();
    // Default camera sits toward +X/+Y/+Z, so -X/-Y/-Z stickers face away from it.
    assert!(is_facing_camera(direction::POS_X, &camera));
    assert!(!is_facing_camera(direction::NEG_X, &camera));
}

#[test]
fn camera_position_is_always_the_nearest_point_in_its_own_direction() {
    // For every orbit angle, a point sitting along the camera's own direction must project
    // to the smallest depth (nearest) compared to a point on the opposite side of the cube.
    for azimuth in [0.0, 45.0, 90.0, 200.0] {
        for elevation in [-80.0, -10.0, 10.0, 80.0] {
            let mut camera = Camera::new();
            camera.azimuth_degrees = azimuth;
            camera.elevation_degrees = elevation;

            let forward = camera.forward();
            let near_point = Vec3::new(forward.x * 5.0, forward.y * 5.0, forward.z * 5.0);
            let far_point = Vec3::new(-forward.x * 5.0, -forward.y * 5.0, -forward.z * 5.0);

            let near_depth = project_point(near_point, &camera).depth;
            let far_depth = project_point(far_point, &camera).depth;
            assert!(
                near_depth < far_depth,
                "azimuth={azimuth}, elevation={elevation}: near_depth={near_depth}, far_depth={far_depth}"
            );
        }
    }
}

#[test]
fn zoom_by_scales_projection_and_clamps() {
    let mut camera = Camera::new();
    let base = project_point(Vec3::new(1.0, 0.0, 0.0), &camera);

    camera.zoom_by(2.0);
    let zoomed_in = project_point(Vec3::new(1.0, 0.0, 0.0), &camera);
    assert!((zoomed_in.x.abs() - base.x.abs() * 2.0).abs() < 1e-9);

    for _ in 0..20 {
        camera.zoom_by(2.0);
    }
    assert!(camera.zoom <= 4.0);

    for _ in 0..20 {
        camera.zoom_by(0.5);
    }
    assert!(camera.zoom >= 0.3);
}

#[test]
fn build_sticker_quads_culls_backfaces_on_single_cubie() {
    let piece = Piece::new(Vec3::new(0.0, 0.0, 0.0))
        .with_sticker(direction::POS_X, rubixemulator::piece::Color::Red)
        .with_sticker(direction::NEG_X, rubixemulator::piece::Color::Orange)
        .with_sticker(direction::POS_Y, rubixemulator::piece::Color::Yellow)
        .with_sticker(direction::NEG_Y, rubixemulator::piece::Color::White)
        .with_sticker(direction::POS_Z, rubixemulator::piece::Color::Green)
        .with_sticker(direction::NEG_Z, rubixemulator::piece::Color::Blue);

    let camera = Camera::new();
    let quads = build_sticker_quads(&[piece], &camera, Vec3::new(0.0, 0.0, 0.0));

    // The default isometric camera sees exactly the 3 stickers on its positive-facing sides.
    assert_eq!(quads.len(), 3);
}

#[test]
fn build_sticker_quads_on_solved_cuboid_only_shows_visible_stickers() {
    let rubix = Rubix::solved(Box::new(Cuboid::new(2, 2, 2)));
    let camera = Camera::new();

    let quads = build_sticker_quads(rubix.pieces(), &camera, Vec3::new(0.5, 0.5, 0.5));

    // A 2x2x2 has 4 stickers per face, 3 faces visible from the default isometric angle.
    assert_eq!(quads.len(), 12);
}
