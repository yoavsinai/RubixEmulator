use rubixemulator::render;
use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;

/// Usage:
///   cargo run                 # GUI window, 3x3x3
///   cargo run -- 4 4 4        # GUI window, custom dimensions
///   cargo run -- --tui        # terminal renderer (with its size-picker setup screen)
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--tui") {
        render::run_with_setup((3, 3, 3), |(x, y, z)| {
            Rubix::solved(Box::new(Cuboid::new(x, y, z)))
        })?;
        return Ok(());
    }

    let dims = parse_dims(&args).unwrap_or((3, 3, 3));
    render::run_window(dims, |(x, y, z)| Rubix::solved(Box::new(Cuboid::new(x, y, z))))
}

fn parse_dims(args: &[String]) -> Option<(usize, usize, usize)> {
    let nums: Vec<usize> = args.iter().filter_map(|a| a.parse().ok()).collect();
    match nums.as_slice() {
        [x, y, z] => Some((*x, *y, *z)),
        _ => None,
    }
}
