use std::io::{stdout, Write};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color as CtColor, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{queue, ExecutableCommand};

use crate::piece::Color;
use crate::render::camera::Camera;
use crate::render::projection::{self, StickerQuad};
use crate::rubix::Rubix;
use crate::shape::Move;
use crate::vec3::Vec3;

const ORBIT_STEP_DEGREES: f64 = 5.0;
const ZOOM_STEP_FACTOR: f64 = 1.15;

/// Enables raw mode + the alternate screen + hides the cursor on construction, and always
/// restores all three on drop (including on panic), so the caller's terminal is never left
/// corrupted by an interrupted render loop.
struct RawModeGuard;

impl RawModeGuard {
    fn new() -> std::io::Result<Self> {
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
    None,
}

/// Runs the interactive render loop: draws the cube, polls for keyboard input, applies
/// camera orbits or face turns, and redraws, until the user quits.
pub fn run_interactive(mut rubix: Rubix) -> std::io::Result<()> {
    let _guard = RawModeGuard::new()?;
    let mut camera = Camera::new();
    let moves = rubix.moves();

    loop {
        draw_frame(&rubix, &camera)?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match handle_key(key, &mut camera, &moves) {
                    Action::Quit => break,
                    Action::ApplyMove(idx, clockwise) => rubix.apply(&moves[idx], clockwise),
                    Action::None => {}
                }
            }
        }
    }

    Ok(())
}

fn handle_key(key: KeyEvent, camera: &mut Camera, moves: &[Move]) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
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
        KeyCode::Char(c) => key_to_move(c, moves)
            .map(|(idx, clockwise)| Action::ApplyMove(idx, clockwise))
            .unwrap_or(Action::None),
        _ => Action::None,
    }
}

fn key_to_move(c: char, moves: &[Move]) -> Option<(usize, bool)> {
    let target = c.to_ascii_uppercase().to_string();
    moves
        .iter()
        .position(|m| m.name == target)
        .map(|idx| (idx, c.is_lowercase()))
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
    let mut quads = projection::build_sticker_quads(rubix.pieces(), camera, cube_center_offset);
    quads.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

    let mut framebuffer: Vec<Option<CtColor>> = vec![None; cols as usize * subrows];

    for quad in &quads {
        fill_quad(&mut framebuffer, quad, origin_x, origin_subrow, scale, cols as usize, subrows);
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
        "arrows: rotate camera  +/-: zoom  letters: turn faces (shift = ccw)  q: quit"
    )?;
    out.flush()?;

    Ok(())
}

fn fill_quad(
    framebuffer: &mut [Option<CtColor>],
    quad: &StickerQuad,
    origin_x: f64,
    origin_subrow: f64,
    scale: f64,
    cols: usize,
    subrows: usize,
) {
    let screen_corners: Vec<(f64, f64)> = quad
        .corners
        .iter()
        .map(|p| (origin_x + p.x * scale, origin_subrow + p.y * scale))
        .collect();

    let min_x = screen_corners.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
    let max_x = screen_corners
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .min(cols.saturating_sub(1) as f64) as usize;
    let min_y = screen_corners.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
    let max_y = screen_corners
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .min(subrows.saturating_sub(1) as f64) as usize;

    let color = color_to_crossterm(quad.color);

    for sub_y in min_y..=max_y {
        for x in min_x..=max_x {
            if point_in_polygon((x as f64 + 0.5, sub_y as f64 + 0.5), &screen_corners) {
                framebuffer[sub_y * cols + x] = Some(color);
            }
        }
    }
}

/// Even-odd ray casting point-in-polygon test over the (convex) sticker quad.
fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let (px, py) = point;
    let mut inside = false;
    let n = polygon.len();

    for i in 0..n {
        let (x1, y1) = polygon[i];
        let (x2, y2) = polygon[(i + 1) % n];

        let crosses = (y1 > py) != (y2 > py);
        if crosses {
            let x_intersect = x1 + (py - y1) / (y2 - y1) * (x2 - x1);
            if px < x_intersect {
                inside = !inside;
            }
        }
    }

    inside
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
