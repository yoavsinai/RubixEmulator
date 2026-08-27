//! A real, cross-platform GUI window for the puzzle, built on `three-d` (which wraps
//! `winit` + OpenGL/WebGL and builds unchanged for Windows, macOS, Linux and the web).
//!
//! The puzzle is drawn as one small black plastic cube per cubie with a slightly raised
//! colored tile on each stickered face ([`scene`]). On top sits an `egui` overlay
//! ([`panels`]). Turns can come from the keyboard ([`input`]), the move buttons, or
//! dragging a sticker ([`drag`]), and a single turn is animated ([`animation`]).
//!
//! [`WindowApp`] owns all the per-session state; `run_window` just wires it to the render
//! loop. Each frame runs the same pipeline: draw the GUI, read input into one [`Action`],
//! apply it, advance the animation, render.

mod animation;
mod drag;
mod input;
mod panels;
mod scene;

use three_d::*;

use crate::render::rng::Rng;
use crate::rubix::Rubix;
use crate::shape::Move;

use animation::TurnAnimation;
use drag::DragState;
use panels::UiState;
use scene::Scene;

const SCRAMBLE_MOVE_COUNT: usize = 25;

/// The one thing the user asked for this frame, from a key, a button, or a mouse drag.
enum Action {
    Move { name: String, clockwise: bool },
    Scramble,
    Solve,
    Reset,
    Rebuild(usize, usize, usize),
}

/// Opens the window and runs the interactive render loop until the user quits. `build`
/// turns a `(x, y, z)` dimension triple into a solved puzzle, so the setup panel can spin
/// up a fresh puzzle of any size without this module knowing about `Cuboid`.
pub fn run_window(
    initial_dims: (usize, usize, usize),
    build: impl Fn((usize, usize, usize)) -> Rubix + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = Window::new(WindowSettings {
        title: "RubixEmulator".to_string(),
        max_size: Some((1280, 800)),
        ..Default::default()
    })?;
    let mut app = WindowApp::new(&window, initial_dims, build);
    window.render_loop(move |frame_input| app.frame(frame_input));
    Ok(())
}

/// The three fixed lights: soft ambient plus a key/fill pair.
struct Lights {
    ambient: AmbientLight,
    key: DirectionalLight,
    fill: DirectionalLight,
}

struct WindowApp {
    context: Context,
    camera: Camera,
    control: OrbitControl,
    gui: GUI,
    lights: Lights,

    /// Rebuilds the puzzle at a given size (for "New puzzle" / "Reset").
    build: Box<dyn Fn((usize, usize, usize)) -> Rubix>,
    dims: (usize, usize, usize),
    rubix: Rubix,
    moves: Vec<Move>,
    scene: Scene,

    rng: Rng,
    anim: Option<TurnAnimation>,
    drag: Option<DragState>,
    ui: UiState,
}

impl WindowApp {
    fn new(
        window: &Window,
        dims: (usize, usize, usize),
        build: impl Fn((usize, usize, usize)) -> Rubix + 'static,
    ) -> Self {
        let context = window.gl();
        let camera = Camera::new_perspective(
            window.viewport(),
            vec3(6.0, 6.0, 9.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            degrees(45.0),
            0.1,
            1000.0,
        );
        let control = OrbitControl::new(vec3(0.0, 0.0, 0.0), 3.0, 40.0);
        let gui = GUI::new(&context);
        let lights = Lights {
            ambient: AmbientLight::new(&context, 0.55, Srgba::WHITE),
            key: DirectionalLight::new(&context, 1.6, Srgba::WHITE, vec3(-0.5, -1.0, -0.7)),
            fill: DirectionalLight::new(&context, 0.8, Srgba::WHITE, vec3(0.8, 0.4, 0.6)),
        };

        let rubix = build(dims);
        let moves = rubix.moves();
        let scene = Scene::build(&context, &rubix);

        Self {
            context,
            camera,
            control,
            gui,
            lights,
            build: Box::new(build),
            dims,
            rubix,
            moves,
            scene,
            rng: Rng::new(),
            anim: None,
            drag: None,
            ui: UiState::new(dims),
        }
    }

    fn frame(&mut self, mut frame_input: FrameInput) -> FrameOutput {
        self.camera.set_viewport(frame_input.viewport);

        let mut exit = false;
        let gui_action = self.draw_gui(
            &mut frame_input.events,
            frame_input.accumulated_time,
            frame_input.viewport,
            frame_input.device_pixel_ratio,
        );
        let event_action = self.process_events(&mut frame_input.events, &mut exit);
        self.control.handle_events(&mut self.camera, &mut frame_input.events);

        if let Some(action) = event_action.or(gui_action) {
            self.apply(action);
        }

        let anim_done = match &mut self.anim {
            Some(anim) => anim.advance(&mut self.scene, frame_input.elapsed_time),
            None => false,
        };
        if anim_done {
            self.anim = None;
        }

        self.render(&frame_input.screen());
        FrameOutput { exit, ..Default::default() }
    }

    /// Draws the overlay. egui also flags every event it consumes as handled, so the drag,
    /// keyboard and orbit handling below naturally skip clicks and keys aimed at a panel.
    fn draw_gui(
        &mut self,
        events: &mut [Event],
        accumulated_time: f64,
        viewport: Viewport,
        device_pixel_ratio: f32,
    ) -> Option<Action> {
        let Self { gui, ui, dims, moves, .. } = self;
        let dims = *dims;
        let mut action = None;
        gui.update(events, accumulated_time, viewport, device_pixel_ratio, |ctx| {
            action = panels::draw(ctx, ui, dims, moves);
        });
        action
    }

    /// Resolves mouse drags on stickers into layer turns (swallowing their motion events so
    /// the camera doesn't also spin), then handles the keyboard.
    fn process_events(&mut self, events: &mut [Event], exit: &mut bool) -> Option<Action> {
        let mut action = None;
        for event in events.iter_mut() {
            match event {
                Event::MousePress { button: MouseButton::Left, position, handled, .. }
                    if !*handled =>
                {
                    let Some(hit) =
                        pick(&self.context, &self.camera, *position, &self.scene.objects)
                    else {
                        continue;
                    };
                    if let Some(Some(sticker)) =
                        self.scene.sticker_refs.get(hit.geometry_id as usize)
                    {
                        self.drag = Some(DragState::begin(
                            *sticker,
                            hit.position,
                            (position.x, position.y),
                        ));
                    }
                }
                Event::MouseMotion { position, handled, .. } => {
                    if let Some(d) = &mut self.drag {
                        d.move_to((position.x, position.y));
                        *handled = true;
                    }
                }
                Event::MouseRelease { button: MouseButton::Left, .. } => {
                    if let Some((name, clockwise)) = self
                        .drag
                        .take()
                        .and_then(|d| d.resolve(&self.camera, &self.moves, &self.rubix))
                    {
                        action = Some(Action::Move { name, clockwise });
                    }
                }
                Event::KeyPress { kind, modifiers, handled, .. } if !*handled => {
                    if let Some(a) = self.ui.keys.on_key(*kind, modifiers.shift, exit) {
                        action = Some(a);
                    }
                }
                _ => {}
            }
        }
        action
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Move { name, clockwise } => {
                let Some(idx) = self.moves.iter().position(|m| m.name == name) else {
                    return;
                };
                self.rubix.apply(&self.moves[idx], clockwise);
                let signed = (if clockwise {
                    self.moves[idx].angle_degrees
                } else {
                    -self.moves[idx].angle_degrees
                }) as f32;
                self.rebuild_scene();
                self.anim =
                    Some(TurnAnimation::start(&self.scene, &self.rubix, &self.moves[idx], signed));
            }
            Action::Scramble => {
                let Self { rubix, rng, .. } = self;
                rubix.scramble(SCRAMBLE_MOVE_COUNT, |bound| rng.next(bound));
                self.rebuild_scene();
            }
            Action::Solve => {
                self.rubix.solve();
                self.rebuild_scene();
            }
            Action::Reset => {
                self.rubix = (self.build)(self.dims);
                self.moves = self.rubix.moves();
                self.rebuild_scene();
            }
            Action::Rebuild(x, y, z) => {
                self.dims = (x, y, z);
                self.ui.sync_dims(self.dims);
                self.rubix = (self.build)(self.dims);
                self.moves = self.rubix.moves();
                self.ui.keys.clear_pending();
                self.rebuild_scene();
            }
        }
    }

    fn rebuild_scene(&mut self) {
        self.scene = Scene::build(&self.context, &self.rubix);
        self.anim = None;
    }

    fn render(&self, screen: &RenderTarget) {
        screen
            .clear(ClearState::color_and_depth(0.09, 0.09, 0.11, 1.0, 1.0))
            .render(
                &self.camera,
                &self.scene.objects,
                &[&self.lights.ambient, &self.lights.key, &self.lights.fill],
            );
        let _ = screen.write(|| self.gui.render());
    }
}
