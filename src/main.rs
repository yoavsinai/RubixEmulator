use rubixemulator::render;
use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;

fn main() -> std::io::Result<()> {
    let rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));
    render::run_interactive(rubix)
}
