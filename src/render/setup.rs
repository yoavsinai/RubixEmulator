use std::io::{stdout, Write};
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Print, ResetColor, SetAttribute, Attribute};
use crossterm::terminal::{Clear, ClearType};
use crossterm::queue;

/// Smallest and largest number of pieces allowed along any one axis. 1 is a valid
/// degenerate slab; the upper bound just keeps the terminal renderer usable.
const MIN_DIM: usize = 1;
const MAX_DIM: usize = 10;

const AXIS_LABELS: [&str; 3] = ["X (width)", "Y (height)", "Z (depth)"];

/// Shows a small interactive setup screen for picking the puzzle's three axis sizes.
/// Returns the chosen `(x, y, z)` dimensions, or `None` if the user quit. Assumes raw
/// mode / the alternate screen are already active (owned by the caller).
pub fn choose_dimensions(
    default: (usize, usize, usize),
) -> std::io::Result<Option<(usize, usize, usize)>> {
    let mut dims = [default.0, default.1, default.2];
    let mut selected = 0usize;

    loop {
        draw(&dims, selected)?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            match resolve(key, &mut dims, &mut selected) {
                Step::Quit => return Ok(None),
                Step::Confirm => return Ok(Some((dims[0], dims[1], dims[2]))),
                Step::Redraw => {}
            }
        }
    }
}

enum Step {
    Quit,
    Confirm,
    Redraw,
}

fn resolve(key: KeyEvent, dims: &mut [usize; 3], selected: &mut usize) -> Step {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Step::Quit,
        KeyCode::Enter => Step::Confirm,
        KeyCode::Up => {
            *selected = (*selected + 2) % 3;
            Step::Redraw
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % 3;
            Step::Redraw
        }
        KeyCode::Left => {
            dims[*selected] = (dims[*selected] - 1).max(MIN_DIM);
            Step::Redraw
        }
        KeyCode::Right => {
            dims[*selected] = (dims[*selected] + 1).min(MAX_DIM);
            Step::Redraw
        }
        KeyCode::Char(c @ '1'..='9') => {
            dims[*selected] = (c as usize - '0' as usize).clamp(MIN_DIM, MAX_DIM);
            Step::Redraw
        }
        _ => Step::Redraw,
    }
}

fn draw(dims: &[usize; 3], selected: usize) -> std::io::Result<()> {
    let mut out = stdout();
    queue!(out, Clear(ClearType::All), MoveTo(2, 1), SetAttribute(Attribute::Bold))?;
    queue!(out, Print("RubixEmulator — choose puzzle size"))?;
    queue!(out, SetAttribute(Attribute::Reset))?;

    for (i, label) in AXIS_LABELS.iter().enumerate() {
        let marker = if i == selected { '>' } else { ' ' };
        queue!(
            out,
            MoveTo(2, 3 + i as u16),
            Print(format!("{marker} {label:<12}  < {:>2} >", dims[i])),
        )?;
    }

    queue!(
        out,
        MoveTo(2, 8),
        Print(format!("= {} x {} x {} puzzle", dims[0], dims[1], dims[2])),
        MoveTo(2, 10),
        Print("up/down: pick axis   left/right or 1-9: change size   enter: start   q: quit"),
        ResetColor,
    )?;
    out.flush()
}
