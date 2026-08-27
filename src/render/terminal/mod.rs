//! The terminal renderer: owns raw mode + the alternate screen, then loops drawing the
//! puzzle ([`raster`]) and folding key presses ([`input`]) into moves.

pub mod camera;
pub mod projection;

mod input;
mod raster;
mod setup;

use std::io::stdout;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;

use crate::render::rng::Rng;
use crate::rubix::Rubix;

use camera::Camera;

use input::{handle_key, Action};
use raster::draw_frame;

const SCRAMBLE_MOVE_COUNT: usize = 25;

/// Enables raw mode + the alternate screen + hides the cursor on construction, and always
/// restores all three on drop (including on panic), so the caller's terminal is never left
/// corrupted by an interrupted render loop.
pub(crate) struct RawModeGuard;

impl RawModeGuard {
    pub(crate) fn new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(Hide)?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = stdout().execute(Show);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Runs the interactive render loop on an already-built puzzle until the user quits.
pub fn run_interactive(mut rubix: Rubix) -> std::io::Result<()> {
    let _guard = RawModeGuard::new()?;
    run_loop(&mut rubix)
}

/// Owns the terminal for the whole session: shows the size-picker setup screen, then
/// (unless the user quit there) builds the puzzle via `build` and runs the render loop —
/// all under a single raw-mode guard so input handling stays consistent throughout.
pub fn run_with_setup(
    default_dims: (usize, usize, usize),
    build: impl FnOnce((usize, usize, usize)) -> Rubix,
) -> std::io::Result<()> {
    let _guard = RawModeGuard::new()?;
    let Some(dims) = setup::choose_dimensions(default_dims)? else {
        return Ok(());
    };
    let mut rubix = build(dims);
    run_loop(&mut rubix)
}

fn run_loop(rubix: &mut Rubix) -> std::io::Result<()> {
    let mut camera = Camera::new();
    let mut rng = Rng::new();
    let moves = rubix.moves();
    // A digit key starts a depth prefix ("2", "3", ...); the next letter completes it into
    // a wide-move name like "2R". Cleared after the letter, or by any non-letter key.
    let mut pending_depth: Option<char> = None;

    loop {
        draw_frame(rubix, &camera)?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            match handle_key(key, &mut camera, &moves, &mut pending_depth) {
                Action::Quit => break,
                Action::ApplyMove(idx, clockwise) => rubix.apply(&moves[idx], clockwise),
                Action::Scramble => {
                    rubix.scramble(SCRAMBLE_MOVE_COUNT, |bound| rng.next(bound));
                }
                Action::Solve => rubix.solve(),
                Action::None => {}
            }
        }
    }

    Ok(())
}
