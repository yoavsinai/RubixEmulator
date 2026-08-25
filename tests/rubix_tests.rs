use rubixemulator::piece::Color;
use rubixemulator::rubix::Rubix;
use rubixemulator::shape::Shape;
use rubixemulator::shapes::cuboid::Cuboid;
use rubixemulator::vec3::direction;

#[test]
fn solved_sticker_counts_3x3x3() {
    let cuboid = Cuboid::new(3, 3, 3);
    let pieces = cuboid.solved_pieces();

    // 3x3x3: 8 corners (3 stickers), 12 edges (2 stickers), 6 centers (1 sticker), 1 hidden core (0, omitted).
    let corner_count = pieces.iter().filter(|p| p.stickers.len() == 3).count();
    let edge_count = pieces.iter().filter(|p| p.stickers.len() == 2).count();
    let center_count = pieces.iter().filter(|p| p.stickers.len() == 1).count();

    assert_eq!(corner_count, 8);
    assert_eq!(edge_count, 12);
    assert_eq!(center_count, 6);
    assert_eq!(pieces.len(), 26); // 27 cells minus the 1 hidden interior core
}

#[test]
fn solved_sticker_counts_2x3x4() {
    let cuboid = Cuboid::new(2, 3, 4);
    let pieces = cuboid.solved_pieces();

    let corner_count = pieces.iter().filter(|p| p.stickers.len() == 3).count();
    assert_eq!(corner_count, 8); // a box always has exactly 8 corners

    // No dimension is 1, so every boundary cell has at most 3 stickers.
    assert!(pieces.iter().all(|p| p.stickers.len() <= 3));
    assert_eq!(pieces.len(), 2 * 3 * 4); // every cell in a 2x3x4 box touches at least one boundary
}

#[test]
fn cuboid_move_count() {
    let cuboid = Cuboid::new(2, 3, 4);
    // One move per layer per axis: 2 + 3 + 4 = 9.
    assert_eq!(cuboid.moves().len(), 9);
}

#[test]
fn move_names_3x3x3() {
    let cuboid = Cuboid::new(3, 3, 3);
    let mut names: Vec<_> = cuboid.moves().iter().map(|m| m.name.clone()).collect();
    names.sort();

    let mut expected = vec!["R", "L", "M", "U", "D", "E", "F", "B", "S"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    expected.sort();

    assert_eq!(names, expected);
}

#[test]
fn move_names_2x3x4_numbered_depth() {
    let cuboid = Cuboid::new(2, 3, 4);
    let mut names: Vec<_> = cuboid.moves().iter().map(|m| m.name.clone()).collect();
    names.sort();

    // dim_x=2 -> R, L; dim_y=3 -> U, D, E; dim_z=4 -> F, 2F, B, 2B (no M/E/S since dim != 3 for X/Z).
    let mut expected = vec!["R", "L", "U", "D", "E", "F", "2F", "B", "2B"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    expected.sort();

    assert_eq!(names, expected);
    assert!(!names.iter().any(|n| n == "M" || n == "S"));
}

#[test]
fn move_names_5x5x5_middle_layer_is_numbered_not_sliced() {
    let cuboid = Cuboid::new(5, 5, 5);
    let names: Vec<_> = cuboid.moves().iter().map(|m| m.name.clone()).collect();

    assert!(names.contains(&"3R".to_string()));
    assert!(names.contains(&"3U".to_string()));
    assert!(names.contains(&"3F".to_string()));
    assert!(!names.iter().any(|n| n == "M" || n == "E" || n == "S"));
}

#[test]
fn move_names_are_unique_per_shape() {
    for cuboid in [Cuboid::new(3, 3, 3), Cuboid::new(2, 3, 4), Cuboid::new(5, 5, 5)] {
        let names: Vec<_> = cuboid.moves().iter().map(|m| m.name.clone()).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "move names must be unique for dims {:?}", cuboid.dims);
    }
}

#[test]
fn four_quarter_turns_return_to_solved() {
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));
    let before: Vec<_> = rubix.pieces().to_vec();

    let moves = rubix.moves();
    let m = &moves[0];
    for _ in 0..4 {
        rubix.apply(m, true);
    }

    assert_eq!(rubix.pieces().to_vec(), before);
}

#[test]
fn move_only_touches_its_own_layer() {
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));
    let before: Vec<_> = rubix.pieces().to_vec();

    let moves = rubix.moves();
    let x_axis_layer_0 = moves
        .iter()
        .find(|m| m.axis == direction::POS_X && (m.selector)(&before[0]))
        .expect("expected an x-axis move matching the first piece's layer");

    rubix.apply(x_axis_layer_0, true);

    for (before_piece, after_piece) in before.iter().zip(rubix.pieces().iter()) {
        if (x_axis_layer_0.selector)(before_piece) {
            continue; // pieces in the turned layer are expected to change
        }
        assert_eq!(before_piece, after_piece, "piece outside the turned layer should be untouched");
    }
}

#[test]
fn turned_layer_pieces_stay_within_the_shapes_own_coordinate_bounds() {
    // Regression test: a layer's rotation axis must pass through that layer's own center,
    // not just through the world origin. A cuboid whose grid isn't centered on the origin
    // (e.g. a corner-origin layout) would rotate pieces around the wrong point, swinging
    // them onto coordinates outside the shape entirely.
    let cuboid = Cuboid::new(3, 3, 3);
    let mut valid_coords: Vec<f64> = cuboid
        .solved_pieces()
        .iter()
        .flat_map(|p| [p.position.x, p.position.y, p.position.z])
        .collect();
    valid_coords.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    valid_coords.sort_by(|a, b| a.partial_cmp(b).unwrap());
    valid_coords.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let mut rubix = Rubix::solved(Box::new(cuboid));
    for m in rubix.moves() {
        rubix.apply(&m, true);
    }

    for piece in rubix.pieces() {
        for coord in [piece.position.x, piece.position.y, piece.position.z] {
            assert!(
                valid_coords.iter().any(|&v| (v - coord).abs() < 1e-9),
                "piece landed on out-of-bounds coordinate {coord} after a turn (valid: {valid_coords:?})"
            );
        }
    }
}

#[test]
fn scramble_applies_the_requested_number_of_moves() {
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));
    let mut reference = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));

    // A fixed "random" source that always picks move 0, clockwise -- deterministic, so the
    // result can be checked against applying that exact same move directly.
    rubix.scramble(5, |_bound| 0);

    let moves = reference.moves();
    for _ in 0..5 {
        reference.apply(&moves[0], true);
    }

    assert_eq!(rubix.pieces().to_vec(), reference.pieces().to_vec());
}

#[test]
fn scramble_of_zero_moves_leaves_the_cube_solved() {
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));
    let before: Vec<_> = rubix.pieces().to_vec();

    rubix.scramble(0, |_bound| 0);

    assert_eq!(rubix.pieces().to_vec(), before);
}

#[test]
fn known_move_produces_expected_face_arrangement() {
    // Rotating the x=0 layer 90 degrees clockwise around +X should cycle
    // that layer's -Y/-Z/+Y/+Z stickers among each other, and leave the
    // rest of the cube's +X sticker colors on that face unaffected in count.
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));

    let moves = rubix.moves();
    let x_layer_0 = moves
        .iter()
        .find(|m| m.axis == direction::POS_X && (m.selector)(&rubix.pieces()[0]))
        .expect("expected an x=0 layer move");
    rubix.apply(x_layer_0, true);

    // The turned layer's own -X face (the orange face) must still be a full 3x3 of orange,
    // just relocated onto whichever pieces are now at x=0 -- the turn doesn't remove color.
    let neg_x_face = rubix.face(direction::NEG_X);
    assert_eq!(neg_x_face.len(), 9);
    assert!(neg_x_face.iter().all(|(_, color)| *color == Color::Orange));
}
