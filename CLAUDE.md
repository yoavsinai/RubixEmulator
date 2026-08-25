# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# RubixEmulator

A generic twisty-puzzle emulator, starting with rectangular box shapes (X × Y × Z, not necessarily equal), with the long-term goal of supporting arbitrary shapes (Pyraminx, etc.).

## Commands

- Build: `cargo build`
- Run: `cargo run`
- Test (all): `cargo test`
- Test (single): `cargo test <test_name>` (e.g. `cargo test move_only_touches_its_own_layer`)

## Architecture

**Piece-based, not face-based.** The puzzle is never modeled as 6 independent `Side` objects. A face turn mutates 4 neighboring sides, and if sides own state independently that coupling gets messy and shape-specific fast. Instead, individual pieces (cubies) are the source of truth: each has a `position` and a map of which `Color` currently faces which `Direction`. A face is never stored — it's a derived view (`Rubix::face`) that filters pieces for a sticker facing that direction. This means a rotation can never leave faces inconsistent, because there's nothing to keep in sync.

**Geometry is behind a `Shape` trait**, so the rotation engine itself is shape-agnostic:
- `Shape::solved_pieces() -> Vec<Piece>` — the shape's solved layout.
- `Shape::moves() -> Vec<Move>` — its legal twists, each just `{ name: String, axis: Vec3, angle_degrees: f64, selector: Fn(&Piece) -> bool }`.
- `Cuboid` (`src/shapes/cuboid.rs`) is the only `Shape` implemented so far. A future `Pyramid` shape would implement the same trait — different piece generation, different moves (120° corner-tip twists instead of 90° layer twists) — without touching `Piece`, `Vec3`, or the rotation engine at all.

**`Rubix`** (`src/rubix.rs`) is the puzzle engine, generic over any `Shape`. Deliberately not named `Cube` — it gets its shape from whatever `Shape` is plugged in, rather than being cube-specific. Its core operation, `rotate(selector, axis, angle_degrees)`, spins any subset of pieces around any axis by any angle using a real rotation matrix (Rodrigues' formula, in `Vec3::rotate_about`). A box's ordinary 90°/layer moves (`Cuboid::moves`) are just one specific way of using that generic primitive — the engine doesn't know or care that it's a box.

**Positions and directions are `Vec3` (floats), not an integer grid.** This is what lets the same types eventually carry non-lattice-aligned shapes (e.g. a Pyraminx's vertices). Floating-point rotation accumulates rounding error over repeated moves, so every rotation result is snapped to the nearest lattice point (`Vec3::snapped_to_lattice`) immediately after. Since `f64` isn't `Hash`/`Eq`, sticker-direction map keys use `LatticeKey` (`src/vec3.rs`) — an exact `(i64, i64, i64)` built from an already-snapped `Vec3`.

**`Cuboid`, not `Box`**, for the box shape's name — `Box` would collide with Rust's own `std::Box` pointer type (which the codebase also uses, e.g. `Rubix { shape: Box<dyn Shape> }`).

**`Cuboid::moves()` names each move with standard cube notation** (`R`/`L`, `U`/`D`, `F`/`B` for outer layers; `2R`/`3R`/... counting depth in from a face for wider cubes; `M`/`E`/`S` for the exact middle layer only when that axis's dimension is exactly 3). This is a label on `Move`, not a notation parser — there's still no string-to-move parsing (e.g. "R U R'").

## Module layout

- `src/vec3.rs` — `Vec3`, `LatticeKey`, the `direction` module (6 axis-aligned unit vectors), rotation math, lattice snapping.
- `src/piece.rs` — `Piece`, `Color`.
- `src/shape.rs` — `Shape` trait, `Move` struct.
- `src/shapes/cuboid.rs` — `Cuboid`, the box shape's `impl Shape`.
- `src/rubix.rs` — `Rubix`: the puzzle engine (`solved`, `rotate`, `apply`, `face`).
- `tests/rubix_tests.rs` — integration tests (kept out of `src/`, per the project owner's preference — tests live in `tests/`, not `#[cfg(test)]` modules alongside implementation code).

## Scope so far

Only the state model and rotation engine exist: no move-notation parser (e.g. "R U R'"), no solver, no rendering. Colors are a fixed placeholder mapping (`+X` red, `-X` orange, `+Y` yellow, `-Y` white, `+Z` green, `-Z` blue) — not yet configurable.

## Preferences

- Tests go in a separate `tests/` integration-test file, not inline `#[cfg(test)]` modules.

## Version control

This project is committed in pieces, not in one large dump — the git history is meant to read as a coherent build path, the way a person would actually develop it.

- **Commit whenever a feature is complete.** As soon as a coherent unit of work lands and builds/tests pass, commit it before starting the next thing — don't batch unrelated work into one commit.
- **Also commit whenever it's warranted, at your judgment** — e.g. a standalone bug fix, a completed refactor, or any other point where the working tree represents one clean, describable step. Use judgment on granularity: split by *what changed and why*, not by turn boundaries or file count.
- Each commit should build and pass `cargo test` on its own where practical, so the history stays bisectable.
- Write commit messages the way the rest of this repo's history does: a short imperative summary line, then a body explaining the *why* (the motivation, the bug, the tradeoff) rather than restating the diff.
- This standing instruction authorizes committing proactively in this repo specifically — it does not extend to pushing to a remote, which still needs to be confirmed separately.
