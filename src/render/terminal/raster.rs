//! Drawing one frame of the puzzle into the terminal: project to quads, rasterize them
//! into a depth-tested framebuffer of half-block sub-pixels, and flush.

use std::io::{stdout, Write};

use crossterm::cursor::MoveTo;
use crossterm::style::{Color as CtColor, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType};
use crossterm::queue;

use crate::piece::Color;
use crate::render::camera::Camera;
use crate::render::projection::{self, QuadKind, StickerQuad};
use crate::rubix::Rubix;

/// Upper-half-block: setting its foreground to one color and its background to another packs
/// two vertically-stacked pixels into one character cell, roughly doubling vertical
/// resolution.
const HALF_BLOCK: char = '▀';

/// The cube's plastic body, shown in the thin gaps between inset stickers and anywhere else
/// a piece's solid face is nearer to the camera than any sticker.
const BODY_COLOR: CtColor = CtColor::Rgb { r: 25, g: 25, b: 25 };

const HELP_LINE: &str = "arrows: rotate camera  +/-: zoom  letters: turn faces (shift = ccw)  digit then letter: inner slice (e.g. 2 R)  space: scramble  enter: solve  q: quit";

pub(super) fn draw_frame(rubix: &Rubix, camera: &Camera) -> std::io::Result<()> {
    let (cols, rows) = crossterm::terminal::size()?;
    let cols = cols as usize;
    let content_rows = rows.saturating_sub(1);
    let subrows = content_rows as usize * 2;

    let origin_x = cols as f64 / 2.0;
    // Sub-cells are roughly square, so the same scale applies to columns and sub-rows.
    let origin_subrow = subrows as f64 / 2.0;
    let scale = 6.0_f64.min(cols as f64 / 6.0).min(subrows as f64 / 6.0);

    let offset = projection::cube_center_offset(rubix.pieces());
    let quads = projection::build_sticker_quads(rubix.pieces(), camera, offset);

    let mut framebuffer: Vec<Option<CtColor>> = vec![None; cols * subrows];
    let mut depth_buffer: Vec<f64> = vec![f64::INFINITY; cols * subrows];
    for quad in &quads {
        fill_quad(&mut framebuffer, &mut depth_buffer, quad, origin_x, origin_subrow, scale, cols, subrows);
    }

    let mut out = stdout();
    queue!(out, Clear(ClearType::All))?;
    for row in 0..content_rows {
        for col in 0..cols {
            let top = framebuffer[row as usize * 2 * cols + col];
            let bottom = framebuffer[(row as usize * 2 + 1) * cols + col];
            if top.is_none() && bottom.is_none() {
                continue;
            }
            queue!(
                out,
                MoveTo(col as u16, row),
                SetForegroundColor(top.unwrap_or(CtColor::Reset)),
                SetBackgroundColor(bottom.unwrap_or(CtColor::Reset))
            )?;
            write!(out, "{HALF_BLOCK}")?;
        }
    }

    queue!(out, MoveTo(0, rows.saturating_sub(1)), ResetColor)?;
    write!(out, "{HELP_LINE}")?;
    out.flush()
}

/// Rasterizes `quad` into the framebuffer, only writing a cell when its depth beats
/// whatever is already there. Depth is interpolated exactly across the quad rather than
/// using one flat value per face: a sticker viewed at an angle is tilted, so its true depth
/// varies from corner to corner, and treating the whole face as one depth caused
/// neighboring cubies to occlude each other incorrectly (visible as a sheared, jumbled cube
/// when rotating). Since the projection is orthographic and each quad is a planar
/// parallelogram, both screen position and depth are exact affine functions of its two
/// local axes, so a single 2x2 solve recovers correct containment and depth together.
#[allow(clippy::too_many_arguments)]
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

    let color = quad_color(quad.kind);

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

fn quad_color(kind: QuadKind) -> CtColor {
    match kind {
        QuadKind::Body => BODY_COLOR,
        QuadKind::Sticker(color) => match color {
            Color::White => CtColor::Rgb { r: 235, g: 235, b: 235 },
            Color::Yellow => CtColor::Rgb { r: 255, g: 220, b: 0 },
            Color::Red => CtColor::Rgb { r: 200, g: 0, b: 0 },
            Color::Orange => CtColor::Rgb { r: 255, g: 140, b: 0 },
            Color::Blue => CtColor::Rgb { r: 0, g: 60, b: 220 },
            Color::Green => CtColor::Rgb { r: 0, g: 160, b: 0 },
        },
    }
}
