//! A real, cross-platform GUI window for the puzzle, built on `three-d` (which wraps
//! `winit` + OpenGL/WebGL and builds unchanged for Windows, macOS, Linux and the web).
//!
//! The puzzle is drawn as one small black plastic cube per cubie with a slightly raised
//! colored tile on each stickered face. On top of the 3D scene sits an `egui` overlay:
//! a setup panel (puzzle dimensions), an instructions panel, and clickable buttons for
//! every legal move. The GPU meshes are rebuilt only when the puzzle actually changes.

use three_d::*;

use crate::piece::Color as PieceColor;
use crate::render::projection::perpendicular_axes;
use crate::render::rng::Rng;
use crate::rubix::Rubix;
use crate::shape::Move;
use crate::vec3::{LatticeKey, Vec3 as ModelVec3};

const SCRAMBLE_MOVE_COUNT: usize = 25;
const MIN_DIM: usize = 1;
const MAX_DIM: usize = 10;

/// How far (in physical pixels) the cursor must travel across a sticker before the drag
/// is read as a layer turn rather than a stray click.
const MIN_DRAG_PIXELS: f32 = 6.0;

/// Which sticker a projected quad belongs to, so a mouse pick can name the piece and the
/// face it hit. Kept parallel to the object list returned by `build_objects`.
#[derive(Clone, Copy)]
struct StickerRef {
    /// The picked piece's position, in the engine's own (doubled, centered) lattice coords.
    piece_position: ModelVec3,
    /// The outward face normal, an axis-aligned unit vector in engine coords.
    normal: ModelVec3,
}

/// A layer turn in progress: the user pressed on a sticker and is dragging the cursor.
struct DragState {
    sticker: StickerRef,
    /// The pick's world-space hit point, in centered scene coords.
    hit: Vec3,
    start_pixel: (f32, f32),
    current_pixel: (f32, f32),
}

/// How long a single layer turn takes to sweep from start to finish.
const TURN_ANIMATION_MS: f64 = 140.0;

/// A layer turn playing out visually. The move is already applied to the model, so this
/// just sweeps the affected meshes from `-applied_degrees` back to their final pose.
struct TurnAnimation {
    /// The turn axis, a unit vector in scene space (same as the move's engine axis).
    axis: Vec3,
    /// The signed angle already applied to the model, in degrees.
    applied_degrees: f32,
    /// Per-object: is this mesh part of the turning layer?
    affected: Vec<bool>,
    /// Per-object: its final transform, to rotate away from and settle back onto.
    base: Vec<Mat4>,
    elapsed_ms: f64,
}

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
    let mut scene = build_objects(&context, &rubix);
    let mut rng = Rng::new();

    // A layer turn playing out visually, if any.
    let mut anim: Option<TurnAnimation> = None;
    // A layer turn the user is dragging out with the mouse, if any.
    let mut drag: Option<DragState> = None;
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

        // egui has already flagged events it consumed as handled. Now resolve mouse drags
        // on stickers into layer turns (and swallow their motion events so the orbit
        // control doesn't also spin the camera), then handle the keyboard.
        for event in frame_input.events.iter_mut() {
            match event {
                Event::MousePress { button: MouseButton::Left, position, handled, .. } => {
                    if *handled {
                        continue;
                    }
                    if let Some(hit) = pick(&context, &camera, *position, &scene.objects) {
                        if let Some(Some(sticker)) = scene.sticker_refs.get(hit.geometry_id as usize) {
                            drag = Some(DragState {
                                sticker: *sticker,
                                hit: hit.position,
                                start_pixel: (position.x, position.y),
                                current_pixel: (position.x, position.y),
                            });
                        }
                    }
                }
                Event::MouseMotion { position, handled, .. } => {
                    if let Some(d) = &mut drag {
                        d.current_pixel = (position.x, position.y);
                        *handled = true;
                    }
                }
                Event::MouseRelease { button: MouseButton::Left, .. } => {
                    if let Some(d) = drag.take() {
                        if let Some((name, clockwise)) = resolve_drag(&camera, &d, &moves, &rubix) {
                            action = Some(Action::Move(name, clockwise));
                        }
                    }
                }
                Event::KeyPress { kind, modifiers, handled, .. } => {
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
                                // Shift = counterclockwise, matching the terminal
                                // renderer's "lowercase is clockwise" convention.
                                action = Some(Action::Move(name, !modifiers.shift));
                            } else {
                                pending_depth = None;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        control.handle_events(&mut camera, &mut frame_input.events);

        let mut dirty = false;
        // For a single turn, the move that was applied and the signed angle it applied,
        // so the visual can be animated after the meshes are rebuilt at their final pose.
        let mut turned: Option<(usize, f32)> = None;
        match action {
            Some(Action::Move(name, clockwise)) => {
                if let Some(idx) = moves.iter().position(|m| m.name == name) {
                    rubix.apply(&moves[idx], clockwise);
                    let signed = if clockwise {
                        moves[idx].angle_degrees
                    } else {
                        -moves[idx].angle_degrees
                    };
                    turned = Some((idx, signed as f32));
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
            scene = build_objects(&context, &rubix);
            anim = None;
        }

        // Kick off the turn animation now that the meshes sit at their final pose.
        if let Some((idx, signed)) = turned {
            let m = &moves[idx];
            let affected: Vec<bool> = scene
                .piece_index
                .iter()
                .map(|&pi| (m.selector)(&rubix.pieces()[pi]))
                .collect();
            let base: Vec<Mat4> = scene.objects.iter().map(|o| o.transformation()).collect();
            anim = Some(TurnAnimation {
                axis: vec3(m.axis.x as f32, m.axis.y as f32, m.axis.z as f32),
                applied_degrees: signed,
                affected,
                base,
                elapsed_ms: 0.0,
            });
        }

        // Advance the turn animation: sweep the affected meshes from a full turn behind
        // their final pose up to it, then settle and clear.
        if let Some(a) = &mut anim {
            a.elapsed_ms += frame_input.elapsed_time;
            let t = (a.elapsed_ms / TURN_ANIMATION_MS).min(1.0) as f32;
            let eased = 1.0 - (1.0 - t).powi(3);
            let angle = -a.applied_degrees * (1.0 - eased);
            let rotation = Mat4::from_axis_angle(a.axis, degrees(angle));
            for (i, &is_affected) in a.affected.iter().enumerate() {
                if is_affected {
                    scene.objects[i].set_transformation(rotation * a.base[i]);
                }
            }
            if t >= 1.0 {
                anim = None;
            }
        }

        let screen = frame_input.screen();
        screen
            .clear(ClearState::color_and_depth(0.09, 0.09, 0.11, 1.0, 1.0))
            .render(&camera, &scene.objects, &[&ambient, &key_light, &fill_light]);
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
            ui.label("Drag a sticker: turn that layer");
            ui.label("Drag empty space: orbit    scroll: zoom");
            ui.label("Key = turn face, Shift+key = counterclockwise");
            ui.label("Digit then letter = inner slice (e.g. 2 then R)");
            ui.label("Space: scramble    Enter: solve    Q: quit");
            if let Some(depth) = pending_depth {
                ui.colored_label(egui::Color32::LIGHT_BLUE, format!("waiting for a face letter after {depth}…"));
            }
        });
}

fn moves_panel(ctx: &egui::Context, moves: &[Move], action: &mut Option<Action>) {
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

/// The drawable puzzle: one mesh per cubie body plus one per visible sticker, all centered
/// on the origin, with per-object bookkeeping kept in lockstep with `objects`.
struct Scene {
    objects: Vec<Gm<Mesh, PhysicalMaterial>>,
    /// The sticker behind each object (`None` for a body), so a mouse pick — which reports
    /// an index into `objects` — can name the piece and face it hit.
    sticker_refs: Vec<Option<StickerRef>>,
    /// The index into `rubix.pieces()` of the piece each object belongs to, so a turn
    /// animation can ask a move's selector which meshes are in the moving layer.
    piece_index: Vec<usize>,
}

/// Builds the drawable puzzle from the current model state.
fn build_objects(context: &Context, rubix: &Rubix) -> Scene {
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
    let mut sticker_refs: Vec<Option<StickerRef>> = Vec::new();
    let mut piece_index: Vec<usize> = Vec::new();

    for (pi, piece) in rubix.pieces().iter().enumerate() {
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
        sticker_refs.push(None);
        piece_index.push(pi);

        for (&dir_key, &color) in &piece.stickers {
            let normal = ModelVec3::from(dir_key);
            let dir = vec3(normal.x as f32, normal.y as f32, normal.z as f32);
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
            sticker_refs.push(Some(StickerRef {
                piece_position: piece.position,
                normal,
            }));
            piece_index.push(pi);
        }
    }

    Scene { objects, sticker_refs, piece_index }
}

/// Turns a finished sticker drag into a named move, or `None` if the drag was too small or
/// too ambiguous. Method: for each of the four possible quarter-turns of a layer through
/// this face (the two in-face axes, each direction), rotate the hit point and project it
/// back to the screen; the candidate whose screen motion best lines up with the actual
/// drag wins. The layer itself is then whichever move's selector grabs the picked piece.
fn resolve_drag(
    camera: &Camera,
    drag: &DragState,
    moves: &[Move],
    rubix: &Rubix,
) -> Option<(String, bool)> {
    let screen_drag = vec2(
        drag.current_pixel.0 - drag.start_pixel.0,
        drag.current_pixel.1 - drag.start_pixel.1,
    );
    if screen_drag.magnitude() < MIN_DRAG_PIXELS {
        return None;
    }
    let drag_dir = screen_drag.normalize();

    let (axis_a, axis_b) = perpendicular_axes(drag.sticker.normal);
    let hit = ModelVec3::new(drag.hit.x as f64, drag.hit.y as f64, drag.hit.z as f64);
    let hit_pixel = camera.pixel_at_position(drag.hit);

    let mut best: Option<(ModelVec3, bool, f32)> = None;
    for axis in [axis_a, axis_b] {
        for clockwise in [true, false] {
            let angle = if clockwise { 90.0 } else { -90.0 };
            let moved = hit.rotate_about(axis, angle);
            let moved_pixel =
                camera.pixel_at_position(vec3(moved.x as f32, moved.y as f32, moved.z as f32));
            let screen_delta = vec2(moved_pixel.x - hit_pixel.x, moved_pixel.y - hit_pixel.y);
            if screen_delta.magnitude() < 1e-3 {
                continue;
            }
            let score = drag_dir.dot(screen_delta.normalize());
            if best.map_or(true, |(_, _, best_score)| score > best_score) {
                best = Some((axis, clockwise, score));
            }
        }
    }

    let (axis, clockwise, score) = best?;
    if score < 0.2 {
        return None; // the drag didn't clearly match any of the four turns
    }

    let picked = LatticeKey::from(drag.sticker.piece_position);
    let piece = rubix
        .pieces()
        .iter()
        .find(|p| LatticeKey::from(p.position) == picked)?;
    let m = moves
        .iter()
        .find(|m| LatticeKey::from(m.axis) == LatticeKey::from(axis) && (m.selector)(piece))?;
    Some((m.name.clone(), clockwise))
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
