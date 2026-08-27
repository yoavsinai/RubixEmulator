//! A real, cross-platform GUI window for the puzzle, built on `three-d` (which wraps
//! `winit` + OpenGL/WebGL and builds unchanged for Windows, macOS, Linux and the web).
//!
//! The puzzle is drawn as one small black plastic cube per cubie with a slightly raised
//! colored tile on each stickered face. On top of the 3D scene sits an `egui` overlay:
//! a setup panel (puzzle dimensions), an instructions panel, and clickable buttons for
//! every legal move. The GPU meshes are rebuilt only when the puzzle actually changes.

use three_d::*;

use crate::piece::Color as PieceColor;
use crate::render::rng::Rng;
use crate::rubix::Rubix;
use crate::vec3::Vec3 as ModelVec3;

const SCRAMBLE_MOVE_COUNT: usize = 25;
const MIN_DIM: usize = 1;
const MAX_DIM: usize = 10;

/// `Cuboid` spaces adjacent layers 2 raw units apart; dividing by this normalizes to one
/// world unit per cubie so the scene's scale doesn't depend on that engine-internal choice.
const WORLD_UNITS_PER_CUBIE: f32 = 2.0;
const CUBIE_HALF: f32 = 0.47;
const STICKER_HALF: f32 = 0.40;
const STICKER_THICKNESS: f32 = 0.04;

/// One thing the user asked for this frame, via a button or a key.
enum Action {
    Move(String, bool),
    Scramble,
    Solve,
    Reset,
    Rebuild(usize, usize, usize),
}

/// Opens the window and runs the interactive render loop until the user quits.
/// `build` turns a `(x, y, z)` dimension triple into a solved puzzle, so the setup panel
/// can spin up a fresh puzzle of any size without this module knowing about `Cuboid`.
pub fn run_window(
    initial_dims: (usize, usize, usize),
    build: impl Fn((usize, usize, usize)) -> Rubix + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = Window::new(WindowSettings {
        title: "RubixEmulator".to_string(),
        max_size: Some((1280, 800)),
        ..Default::default()
    })?;
    let context = window.gl();

    let mut camera = Camera::new_perspective(
        window.viewport(),
        vec3(6.0, 6.0, 9.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        0.1,
        1000.0,
    );
    let mut control = OrbitControl::new(vec3(0.0, 0.0, 0.0), 3.0, 40.0);
    let mut gui = GUI::new(&context);

    let ambient = AmbientLight::new(&context, 0.55, Srgba::WHITE);
    let key_light = DirectionalLight::new(&context, 1.6, Srgba::WHITE, vec3(-0.5, -1.0, -0.7));
    let fill_light = DirectionalLight::new(&context, 0.8, Srgba::WHITE, vec3(0.8, 0.4, 0.6));

    let mut dims = initial_dims;
    let mut rubix = build(dims);
    let mut moves = rubix.moves();
    let mut objects = build_objects(&context, &rubix);
    let mut rng = Rng::new();

    // The setup panel edits these until "New puzzle" is pressed.
    let mut pending_dims: [usize; 3] = [dims.0, dims.1, dims.2];
    // A digit key starts a depth prefix ("2", "3", ...); the next letter completes it into
    // a wide-move name like "2R". Cleared after the letter, or by any non-letter key.
    let mut pending_depth: Option<u32> = None;

    window.render_loop(move |mut frame_input| {
        camera.set_viewport(frame_input.viewport);

        let mut exit = false;
        let mut action: Option<Action> = None;

        gui.update(
            &mut frame_input.events,
            frame_input.accumulated_time,
            frame_input.viewport,
            frame_input.device_pixel_ratio,
            |ctx| {
                setup_panel(ctx, &mut pending_dims, dims, &mut action);
                controls_panel(ctx, pending_depth, &mut action);
                moves_panel(ctx, &moves, &mut action);
            },
        );

        // egui has already flagged events it consumed as handled, so the orbit control
        // and the key handler below naturally ignore drags/clicks that landed on a panel.
        control.handle_events(&mut camera, &mut frame_input.events);

        for event in &frame_input.events {
            if let Event::KeyPress { kind, modifiers, handled, .. } = event {
                if *handled {
                    continue;
                }
                match kind {
                    Key::Q | Key::Escape => exit = true,
                    Key::Space => action = Some(Action::Scramble),
                    Key::Enter => action = Some(Action::Solve),
                    _ => {
                        if let Some(digit) = digit_of(*kind) {
                            if (2..=9).contains(&digit) {
                                pending_depth = Some(digit);
                            }
                            continue;
                        }
                        if let Some(letter) = letter_of(*kind) {
                            let name = match pending_depth.take() {
                                Some(depth) => format!("{depth}{letter}"),
                                None => letter.to_string(),
                            };
                            // Shift = counterclockwise, matching the terminal renderer's
                            // "lowercase is clockwise" convention.
                            action = Some(Action::Move(name, !modifiers.shift));
                        } else {
                            pending_depth = None;
                        }
                    }
                }
            }
        }

        let mut dirty = false;
        match action {
            Some(Action::Move(name, clockwise)) => {
                if let Some(m) = moves.iter().find(|m| m.name == name) {
                    rubix.apply(m, clockwise);
                    dirty = true;
                }
            }
            Some(Action::Scramble) => {
                rubix.scramble(SCRAMBLE_MOVE_COUNT, |bound| rng.next(bound));
                dirty = true;
            }
            Some(Action::Solve) => {
                rubix.solve();
                dirty = true;
            }
            Some(Action::Reset) => {
                rubix = build(dims);
                moves = rubix.moves();
                dirty = true;
            }
            Some(Action::Rebuild(x, y, z)) => {
                dims = (x, y, z);
                pending_dims = [x, y, z];
                rubix = build(dims);
                moves = rubix.moves();
                pending_depth = None;
                dirty = true;
            }
            None => {}
        }

        if dirty {
            objects = build_objects(&context, &rubix);
        }

        let screen = frame_input.screen();
        screen
            .clear(ClearState::color_and_depth(0.09, 0.09, 0.11, 1.0, 1.0))
            .render(&camera, &objects, &[&ambient, &key_light, &fill_light]);
        let _ = screen.write(|| gui.render());

        FrameOutput { exit, ..Default::default() }
    });

    Ok(())
}

fn setup_panel(
    ctx: &egui::Context,
    pending: &mut [usize; 3],
    current: (usize, usize, usize),
    action: &mut Option<Action>,
) {
    egui::Window::new("Setup")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .show(ctx, |ui| {
            for (label, value) in ["X (width)", "Y (height)", "Z (depth)"].iter().zip(pending.iter_mut()) {
                ui.add(egui::Slider::new(value, MIN_DIM..=MAX_DIM).text(*label));
            }
            let requested = (pending[0], pending[1], pending[2]);
            ui.horizontal(|ui| {
                let changed = requested != current;
                if ui.add_enabled(changed, egui::Button::new("New puzzle")).clicked() {
                    *action = Some(Action::Rebuild(requested.0, requested.1, requested.2));
                }
                if ui.button("Reset").clicked() {
                    *action = Some(Action::Reset);
                }
            });
            ui.label(format!("current: {} x {} x {}", current.0, current.1, current.2));
        });
}

fn controls_panel(ctx: &egui::Context, pending_depth: Option<u32>, action: &mut Option<Action>) {
    egui::Window::new("Controls")
        .anchor(egui::Align2::LEFT_BOTTOM, [12.0, -12.0])
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Scramble").clicked() {
                    *action = Some(Action::Scramble);
                }
                if ui.button("Solve").clicked() {
                    *action = Some(Action::Solve);
                }
            });
            ui.separator();
            ui.label("Mouse drag: orbit    scroll: zoom");
            ui.label("Key = turn face, Shift+key = counterclockwise");
            ui.label("Digit then letter = inner slice (e.g. 2 then R)");
            ui.label("Space: scramble    Enter: solve    Q: quit");
            if let Some(depth) = pending_depth {
                ui.colored_label(egui::Color32::LIGHT_BLUE, format!("waiting for a face letter after {depth}…"));
            }
        });
}

fn moves_panel(ctx: &egui::Context, moves: &[crate::shape::Move], action: &mut Option<Action>) {
    egui::Window::new("Moves")
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 12.0])
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("click = clockwise, ′ = counterclockwise");
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                egui::Grid::new("move-grid").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                    for m in moves {
                        if ui.button(&m.name).clicked() {
                            *action = Some(Action::Move(m.name.clone(), true));
                        }
                        if ui.button(format!("{}′", m.name)).clicked() {
                            *action = Some(Action::Move(m.name.clone(), false));
                        }
                        ui.end_row();
                    }
                });
            });
        });
}

/// Builds one GPU object per cubie body plus one per visible sticker, centered on the origin.
fn build_objects(context: &Context, rubix: &Rubix) -> Vec<Gm<Mesh, PhysicalMaterial>> {
    let offset = cube_center_offset(rubix);
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

    for piece in rubix.pieces() {
        let center = vec3(
            ((piece.position.x - offset.x) as f32) / WORLD_UNITS_PER_CUBIE,
            ((piece.position.y - offset.y) as f32) / WORLD_UNITS_PER_CUBIE,
            ((piece.position.z - offset.z) as f32) / WORLD_UNITS_PER_CUBIE,
        );

        let mut body = Gm::new(Mesh::new(context, &CpuMesh::cube()), body_material.clone());
        body.set_transformation(
            Mat4::from_translation(center) * Mat4::from_nonuniform_scale(CUBIE_HALF, CUBIE_HALF, CUBIE_HALF),
        );
        objects.push(body);

        for (&dir_key, &color) in &piece.stickers {
            let dir = ModelVec3::from(dir_key);
            let dir = vec3(dir.x as f32, dir.y as f32, dir.z as f32);
            let (sx, sy, sz) = (lerp_extent(dir.x), lerp_extent(dir.y), lerp_extent(dir.z));
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
        }
    }

    objects
}

/// Half-extent of a sticker tile along one axis: thin along the face normal, wide otherwise.
fn lerp_extent(component: f32) -> f32 {
    if component.abs() > 0.5 {
        STICKER_THICKNESS
    } else {
        STICKER_HALF
    }
}

fn cube_center_offset(rubix: &Rubix) -> ModelVec3 {
    let mut min = ModelVec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = ModelVec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for piece in rubix.pieces() {
        let p = piece.position;
        min = ModelVec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = ModelVec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    ModelVec3::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0, (min.z + max.z) / 2.0)
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

fn digit_of(key: Key) -> Option<u32> {
    match key {
        Key::Num0 => Some(0),
        Key::Num1 => Some(1),
        Key::Num2 => Some(2),
        Key::Num3 => Some(3),
        Key::Num4 => Some(4),
        Key::Num5 => Some(5),
        Key::Num6 => Some(6),
        Key::Num7 => Some(7),
        Key::Num8 => Some(8),
        Key::Num9 => Some(9),
        _ => None,
    }
}

fn letter_of(key: Key) -> Option<char> {
    match key {
        Key::A => Some('A'),
        Key::B => Some('B'),
        Key::C => Some('C'),
        Key::D => Some('D'),
        Key::E => Some('E'),
        Key::F => Some('F'),
        Key::G => Some('G'),
        Key::H => Some('H'),
        Key::I => Some('I'),
        Key::J => Some('J'),
        Key::K => Some('K'),
        Key::L => Some('L'),
        Key::M => Some('M'),
        Key::N => Some('N'),
        Key::O => Some('O'),
        Key::P => Some('P'),
        Key::R => Some('R'),
        Key::S => Some('S'),
        Key::T => Some('T'),
        Key::U => Some('U'),
        Key::V => Some('V'),
        Key::W => Some('W'),
        Key::X => Some('X'),
        Key::Y => Some('Y'),
        Key::Z => Some('Z'),
        _ => None,
    }
}
