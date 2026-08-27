//! A running list of the turns made this session, shown in its own overlay panel.
//!
//! This lives entirely in the window layer: the engine keeps its own undo history
//! (`Rubix::history`), keyed by touched pieces, but this is the human-readable version
//! in cube notation. The two stay in lock-step — one entry here per `Rubix::apply` —
//! so clicking an entry can undo the engine back to exactly that point.

use three_d::egui;

use super::Action;

/// The turns made since the puzzle was last built or solved, newest last, in cube
/// notation (`R`, `U'`, …). One entry per applied move, scramble turns included.
#[derive(Default)]
pub(crate) struct MoveHistory {
    entries: Vec<String>,
}

impl MoveHistory {
    /// Appends one turn in notation form.
    pub fn record(&mut self, name: &str, clockwise: bool) {
        self.entries.push(if clockwise { name.to_string() } else { format!("{name}'") });
    }

    /// Forgets everything — the puzzle was reset, rebuilt, or solved back to start.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Drops all but the first `len` entries, to match an undo back to that point.
    pub fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

pub(crate) fn draw(ctx: &egui::Context, history: &MoveHistory) -> Option<Action> {
    let mut action = None;
    egui::Window::new("History")
        .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .resizable(false)
        .show(ctx, |ui| {
            if history.entries.is_empty() {
                ui.label("no moves yet");
                return;
            }
            ui.horizontal(|ui| {
                ui.label(format!("{} move(s)", history.entries.len()));
                if ui.button("Undo").clicked() {
                    action = Some(Action::UndoTo(history.entries.len() - 1));
                }
            });
            ui.label("click a move to undo back to it");
            egui::ScrollArea::vertical().max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, entry) in history.entries.iter().enumerate() {
                        if ui.small_button(entry).clicked() {
                            action = Some(Action::UndoTo(i));
                        }
                    }
                });
            });
        });
    action
}
