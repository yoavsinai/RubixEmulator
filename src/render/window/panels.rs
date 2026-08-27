//! The `egui` overlay: setup, controls/instructions, and the move buttons. Each panel
//! only ever produces an [`Action`]; it never touches the puzzle directly.

use three_d::egui;

use crate::shape::Move;

use super::input::KeyState;
use super::Action;

/// Smallest and largest number of pieces along any one axis the setup panel offers.
const MIN_DIM: usize = 1;
const MAX_DIM: usize = 10;

/// The overlay's own mutable state, separate from the puzzle: the setup sliders' current
/// values and the keyboard's pending prefix.
pub(crate) struct UiState {
    pending_dims: [usize; 3],
    pub keys: KeyState,
}

impl UiState {
    pub fn new(dims: (usize, usize, usize)) -> Self {
        Self { pending_dims: [dims.0, dims.1, dims.2], keys: KeyState::default() }
    }

    /// Called when the puzzle is rebuilt, so the sliders track the new size.
    pub fn sync_dims(&mut self, dims: (usize, usize, usize)) {
        self.pending_dims = [dims.0, dims.1, dims.2];
    }
}

/// Draws every panel for this frame and returns the single action they produced, if any.
pub(crate) fn draw(
    ctx: &egui::Context,
    ui: &mut UiState,
    current_dims: (usize, usize, usize),
    moves: &[Move],
) -> Option<Action> {
    let mut action = None;
    setup_panel(ctx, &mut ui.pending_dims, current_dims, &mut action);
    controls_panel(ctx, ui.keys.pending_depth(), &mut action);
    moves_panel(ctx, moves, &mut action);
    action
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
            for (label, value) in ["X (width)", "Y (height)", "Z (depth)"]
                .iter()
                .zip(pending.iter_mut())
            {
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
                ui.colored_label(
                    egui::Color32::LIGHT_BLUE,
                    format!("waiting for a face letter after {depth}…"),
                );
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
                            *action = Some(Action::Move { name: m.name.clone(), clockwise: true });
                        }
                        if ui.button(format!("{}′", m.name)).clicked() {
                            *action = Some(Action::Move { name: m.name.clone(), clockwise: false });
                        }
                        ui.end_row();
                    }
                });
            });
        });
}
