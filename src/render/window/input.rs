//! Keyboard handling: turning key presses into [`Action`]s, including the inner-slice
//! digit prefix ("2" then "R" -> the `2R` move).

use three_d::Key;

use super::Action;

/// The keyboard's small bit of memory: a pending inner-slice depth prefix, set by a digit
/// key and consumed by the next letter.
#[derive(Default)]
pub(crate) struct KeyState {
    pending_depth: Option<u32>,
}

impl KeyState {
    /// The depth digit waiting for a face letter, if any — shown in the controls panel.
    pub fn pending_depth(&self) -> Option<u32> {
        self.pending_depth
    }

    pub fn clear_pending(&mut self) {
        self.pending_depth = None;
    }

    /// Interprets one key press. Sets `*exit` for quit keys; otherwise returns an `Action`
    /// when the press completes a move or a command.
    pub fn on_key(&mut self, kind: Key, shift: bool, exit: &mut bool) -> Option<Action> {
        match kind {
            Key::Q | Key::Escape => {
                *exit = true;
                None
            }
            Key::Space => Some(Action::Scramble),
            Key::Enter => Some(Action::Solve),
            _ => {
                if let Some(digit) = digit_of(kind) {
                    if (2..=9).contains(&digit) {
                        self.pending_depth = Some(digit);
                    }
                    return None;
                }
                let Some(letter) = letter_of(kind) else {
                    self.pending_depth = None;
                    return None;
                };
                let name = match self.pending_depth.take() {
                    Some(depth) => format!("{depth}{letter}"),
                    None => letter.to_string(),
                };
                // Shift = counterclockwise, matching the terminal renderer's
                // "lowercase is clockwise" convention.
                Some(Action::Move { name, clockwise: !shift })
            }
        }
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
