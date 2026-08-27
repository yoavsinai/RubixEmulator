//! A running list of the turns made this session, shown in its own overlay panel.
//!
//! This lives entirely in the window layer: the engine already keeps its own undo
//! history (`Rubix::history`), but that's keyed by touched pieces, not notation, and is
//! consumed by `solve`. This is just a human-readable log for the player.

use three_d::egui;

/// The turns made since the puzzle was last built or solved, newest last, in cube
/// notation (`R`, `U'`, …). Bulk operations like a scramble are recorded as a single
/// note rather than every underlying turn.
#[derive(Default)]
pub(crate) struct MoveHistory {
    entries: Vec<String>,
}

impl MoveHistory {
    /// Appends one turn in notation form.
    pub fn record(&mut self, name: &str, clockwise: bool) {
        self.entries.push(if clockwise { name.to_string() } else { format!("{name}'") });
    }

    /// Appends a free-text marker for something that isn't a single turn (e.g. a scramble).
    pub fn note(&mut self, text: &str) {
        self.entries.push(format!("({text})"));
    }

    /// Forgets everything — the puzzle was reset, rebuilt, or solved back to start.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub(crate) fn draw(ctx: &egui::Context, history: &MoveHistory) {
    egui::Window::new("History")
        .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .resizable(false)
        .show(ctx, |ui| {
            if history.entries.is_empty() {
                ui.label("no moves yet");
                return;
            }
            ui.label(format!("{} move(s)", history.entries.len()));
            egui::ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                ui.label(history.entries.join("  "));
            });
        });
}
