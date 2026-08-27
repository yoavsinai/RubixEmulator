use rubixemulator::render;
use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;

fn main() -> std::io::Result<()> {
    let Some((x, y, z)) = render::run_setup((3, 3, 3))? else {
        return Ok(());
    };
    let rubix = Rubix::solved(Box::new(Cuboid::new(x, y, z)));
    render::run_interactive(rubix)
}
