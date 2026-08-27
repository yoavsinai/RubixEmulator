use std::io::{stdout, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color as CtColor, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{queue, ExecutableCommand};

use crate::piece::Color;
use crate::render::camera::Camera;
use crate::render::projection::{self, QuadKind, StickerQuad};
use crate::rubix::Rubix;
use crate::shape::Move;
use crate::vec3::Vec3;

const ORBIT_STEP_DEGREES: f64 = 5.0;
const ZOOM_STEP_FACTOR: f64 = 1.15;
const SCRAMBLE_MOVE_COUNT: usize = 25;

/// A small, dependency-free xorshift64* generator — scrambling is the only place this
/// renderer needs randomness, so it isn't worth pulling in a whole RNG crate for it.
struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(seed | 1) // xorshift needs a nonzero state
    }

    /// A pseudo-random value in `0..bound`.
    fn next(&mut self, bound: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % bound as u64) as usize
    }
}

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

enum Action {
    Quit,
    ApplyMove(usize, bool),
    Scramble,
    Solve,
    None,
}

/// Runs the interactive render loop: draws the cube, polls for keyboard input, applies
/// camera orbits or face turns, and redraws, until the user quits.
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
    let Some(dims) = crate::render::setup::choose_dimensions(default_dims)? else {
        return Ok(());
    };
    let mut rubix = build(dims);
    run_loop(&mut rubix)
}

fn run_loop(rubix: &mut Rubix) -> std::io::Result<()> {
    let mut camera = Camera::new();
    let mut rng = Rng::new();
    let moves = rubix.moves();
    // A digit key starts a depth prefix ("2", "3", ...); the next letter completes it
    // into a wide-move name like "2R". Cleared after the letter, or by any non-letter key.
    let mut pending_depth: Option<char> = None;

    loop {
        draw_frame(rubix, &camera)?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match handle_key(key, &mut camera, &moves, &mut pending_depth) {
                    Action::Quit => break,
                    Action::ApplyMove(idx, clockwise) => rubix.apply(&moves[idx], clockwise),
                    Action::Scramble => rubix.scramble(SCRAMBLE_MOVE_COUNT, |bound| rng.next(bound)),
                    Action::Solve => rubix.solve(),
                    Action::None => {}
                }
            }
        }
    }

    Ok(())
}

fn handle_key(
    key: KeyEvent,
    camera: &mut Camera,
    moves: &[Move],
    pending_depth: &mut Option<char>,
) -> Action {
    // Complete or abandon a pending depth prefix before anything else.
    let prefix = pending_depth.take();
    if let Some(depth) = prefix {
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_alphabetic() {
                return key_to_move(&format!("{depth}{}", c.to_ascii_uppercase()), c.is_lowercase(), moves) // lowercase = clockwise
                    .map(|(idx, cw)| Action::ApplyMove(idx, cw))
                    .unwrap_or(Action::None);
            }
        }
        // Not a letter: the prefix is dropped and the key falls through as normal.
    }

    match key.code {
        KeyCode::Char(c @ '2'..='9') => {
            *pending_depth = Some(c);
            Action::None
        }
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char(' ') => Action::Scramble,
        KeyCode::Enter => Action::Solve,
        KeyCode::Left => {
            camera.orbit(-ORBIT_STEP_DEGREES, 0.0);
            Action::None
        }
        KeyCode::Right => {
            camera.orbit(ORBIT_STEP_DEGREES, 0.0);
            Action::None
        }
        KeyCode::Up => {
            camera.orbit(0.0, ORBIT_STEP_DEGREES);
            Action::None
        }
        KeyCode::Down => {
            camera.orbit(0.0, -ORBIT_STEP_DEGREES);
            Action::None
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            camera.zoom_by(ZOOM_STEP_FACTOR);
            Action::None
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            camera.zoom_by(1.0 / ZOOM_STEP_FACTOR);
            Action::None
        }
        KeyCode::Char(c) => key_to_move(&c.to_ascii_uppercase().to_string(), c.is_lowercase(), moves) // lowercase = clockwise
            .map(|(idx, clockwise)| Action::ApplyMove(idx, clockwise))
            .unwrap_or(Action::None),
        _ => Action::None,
    }
}

fn key_to_move(name: &str, clockwise: bool, moves: &[Move]) -> Option<(usize, bool)> {
    moves
        .iter()
        .position(|m| m.name == name)
        .map(|idx| (idx, clockwise))
}

fn color_to_crossterm(color: Color) -> CtColor {
    match color {
        Color::White => CtColor::Rgb { r: 235, g: 235, b: 235 },
        Color::Yellow => CtColor::Rgb { r: 255, g: 220, b: 0 },
        Color::Red => CtColor::Rgb { r: 200, g: 0, b: 0 },
        Color::Orange => CtColor::Rgb { r: 255, g: 140, b: 0 },
        Color::Blue => CtColor::Rgb { r: 0, g: 60, b: 220 },
        Color::Green => CtColor::Rgb { r: 0, g: 160, b: 0 },
    }
}

/// The cube's plastic body, shown in the thin gaps between inset stickers and anywhere
/// else a piece's solid face is nearer to the camera than any sticker.
const BODY_COLOR: CtColor = CtColor::Rgb { r: 25, g: 25, b: 25 };

fn quad_kind_to_crossterm(kind: QuadKind) -> CtColor {
    match kind {
        QuadKind::Sticker(color) => color_to_crossterm(color),
        QuadKind::Body => BODY_COLOR,
    }
}

/// Upper-half-block: setting its foreground to one color and background to another packs
/// two vertically-stacked pixels into a single character cell, roughly doubling the
/// renderer's effective vertical resolution.
const HALF_BLOCK: char = '▀';

fn draw_frame(rubix: &Rubix, camera: &Camera) -> std::io::Result<()> {
    let (cols, rows) = crossterm::terminal::size()?;
    let content_rows = rows.saturating_sub(1);
    let subrows = content_rows as usize * 2;

    let origin_x = cols as f64 / 2.0;
    // Sub-cells are roughly square, so the same scale applies to columns and sub-rows.
    let origin_subrow = subrows as f64 / 2.0;
    let scale = 6.0_f64.min(cols as f64 / 6.0).min(subrows as f64 / 6.0);

    let cube_center_offset = cube_center_offset(rubix);
    let quads = projection::build_sticker_quads(rubix.pieces(), camera, cube_center_offset);

    let mut framebuffer: Vec<Option<CtColor>> = vec![None; cols as usize * subrows];
    let mut depth_buffer: Vec<f64> = vec![f64::INFINITY; cols as usize * subrows];

    for quad in &quads {
        fill_quad(
            &mut framebuffer,
            &mut depth_buffer,
            quad,
            origin_x,
            origin_subrow,
            scale,
            cols as usize,
            subrows,
        );
    }

    let mut out = stdout();
    queue!(out, Clear(ClearType::All))?;

    for row in 0..content_rows {
        for col in 0..cols {
            let top = framebuffer[row as usize * 2 * cols as usize + col as usize];
            let bottom = framebuffer[(row as usize * 2 + 1) * cols as usize + col as usize];
            if top.is_none() && bottom.is_none() {
                continue;
            }

            queue!(
                out,
                MoveTo(col, row),
                SetForegroundColor(top.unwrap_or(CtColor::Reset)),
                SetBackgroundColor(bottom.unwrap_or(CtColor::Reset))
            )?;
            write!(out, "{HALF_BLOCK}")?;
        }
    }

    queue!(out, MoveTo(0, rows.saturating_sub(1)), ResetColor)?;
    write!(
        out,
        "arrows: rotate camera  +/-: zoom  letters: turn faces (shift = ccw)  digit then letter: inner slice (e.g. 2 R)  space: scramble  enter: solve  q: quit"
    )?;
    out.flush()?;

    Ok(())
}

/// Rasterizes `quad` into the framebuffer, only writing a cell when its depth beats
/// whatever is already there. Depth is interpolated exactly across the quad rather than
/// using one flat value per face: a sticker viewed at an angle is tilted, so its true
/// depth varies from corner to corner, and treating the whole face as one depth caused
/// neighboring cubies to occlude each other incorrectly (visible as a sheared, jumbled
/// cube when rotating). Since the projection is orthographic and each quad is a planar
/// parallelogram, both screen position and depth are exact affine functions of its two
/// local axes, so a single 2x2 solve recovers correct containment and depth together.
fn fill_quad(
    framebuffer: &mut [Option<CtColor>],
    depth_buffer: &mut [f64],
    quad: &StickerQuad,
    origin_x: f64,
    origin_subrow: f64,
    scale: f64,
    cols: usize,
    subrows: usize,
) {
    // corners[0..4] are laid out (+h,+h), (+h,-h), (-h,-h), (-h,+h) in the quad's local
    // axes, so corners[1]-corners[2] and corners[3]-corners[2] are its two edge vectors.
    let screen: [(f64, f64, f64); 4] = std::array::from_fn(|i| {
        let p = quad.corners[i];
        (origin_x + p.x * scale, origin_subrow + p.y * scale, p.depth)
    });

    let origin = screen[2];
    let edge_u = (screen[1].0 - origin.0, screen[1].1 - origin.1, screen[1].2 - origin.2);
    let edge_v = (screen[3].0 - origin.0, screen[3].1 - origin.1, screen[3].2 - origin.2);

    let det = edge_u.0 * edge_v.1 - edge_v.0 * edge_u.1;
    if det.abs() < 1e-12 {
        return; // Degenerate: the quad is exactly edge-on to the camera, no area to draw.
    }

    let xs = [screen[0].0, screen[1].0, screen[2].0, screen[3].0];
    let ys = [screen[0].1, screen[1].1, screen[2].1, screen[3].1];
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
    let max_x = xs
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .min(cols.saturating_sub(1) as f64) as usize;
    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
    let max_y = ys
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .min(subrows.saturating_sub(1) as f64) as usize;

    let color = quad_kind_to_crossterm(quad.kind);

    for sub_y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5 - origin.0;
            let py = sub_y as f64 + 0.5 - origin.1;

            // Solve px = a*edge_u.x + b*edge_v.x, py = a*edge_u.y + b*edge_v.y for (a, b).
            let a = (px * edge_v.1 - edge_v.0 * py) / det;
            let b = (edge_u.0 * py - px * edge_u.1) / det;
            if !(0.0..=1.0).contains(&a) || !(0.0..=1.0).contains(&b) {
                continue;
            }

            let depth = origin.2 + a * edge_u.2 + b * edge_v.2;
            let cell = sub_y * cols + x;
            if depth < depth_buffer[cell] {
                depth_buffer[cell] = depth;
                framebuffer[cell] = Some(color);
            }
        }
    }
}

fn cube_center_offset(rubix: &Rubix) -> Vec3 {
    let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    for piece in rubix.pieces() {
        let p = piece.position;
        min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }

    Vec3::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0, (min.z + max.z) / 2.0)
}
