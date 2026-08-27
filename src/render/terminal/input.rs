//! Turning a key press into an [`Action`], including the inner-slice digit prefix
//! ("2" then "R" -> the `2R` move).

use crossterm::event::{KeyCode, KeyEvent};

use crate::render::camera::Camera;
use crate::shape::Move;

const ORBIT_STEP_DEGREES: f64 = 5.0;
const ZOOM_STEP_FACTOR: f64 = 1.15;

/// What one key press asks the render loop to do.
pub(super) enum Action {
    Quit,
    ApplyMove(usize, bool),
    Scramble,
    Solve,
    None,
}

/// Interprets one key press: mutates `camera` for orbit/zoom keys and `pending_depth` for a
/// digit prefix, and otherwise returns the [`Action`] to run.
pub(super) fn handle_key(
    key: KeyEvent,
    camera: &mut Camera,
    moves: &[Move],
    pending_depth: &mut Option<char>,
) -> Action {
    // Complete a pending depth prefix, or drop it and let the key fall through as normal.
    if let (Some(depth), KeyCode::Char(c)) = (pending_depth.take(), key.code)
        && c.is_ascii_alphabetic()
    {
        // lowercase = clockwise
        let name = format!("{depth}{}", c.to_ascii_uppercase());
        return move_action(&name, c.is_lowercase(), moves);
    }

    match key.code {
        KeyCode::Char(c @ '2'..='9') => {
            *pending_depth = Some(c);
            Action::None
        }
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char(' ') => Action::Scramble,
        KeyCode::Enter => Action::Solve,
        KeyCode::Left => orbit(camera, -ORBIT_STEP_DEGREES, 0.0),
        KeyCode::Right => orbit(camera, ORBIT_STEP_DEGREES, 0.0),
        KeyCode::Up => orbit(camera, 0.0, ORBIT_STEP_DEGREES),
        KeyCode::Down => orbit(camera, 0.0, -ORBIT_STEP_DEGREES),
        KeyCode::Char('+') | KeyCode::Char('=') => zoom(camera, ZOOM_STEP_FACTOR),
        KeyCode::Char('-') | KeyCode::Char('_') => zoom(camera, 1.0 / ZOOM_STEP_FACTOR),
        // lowercase = clockwise
        KeyCode::Char(c) => move_action(&c.to_ascii_uppercase().to_string(), c.is_lowercase(), moves),
        _ => Action::None,
    }
}

fn orbit(camera: &mut Camera, azimuth: f64, elevation: f64) -> Action {
    camera.orbit(azimuth, elevation);
    Action::None
}

fn zoom(camera: &mut Camera, factor: f64) -> Action {
    camera.zoom_by(factor);
    Action::None
}

fn move_action(name: &str, clockwise: bool, moves: &[Move]) -> Action {
    match moves.iter().position(|m| m.name == name) {
        Some(idx) => Action::ApplyMove(idx, clockwise),
        None => Action::None,
    }
}
