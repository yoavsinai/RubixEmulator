use rubixemulator::render;
use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;

fn main() -> std::io::Result<()> {
    render::run_with_setup((3, 3, 3), |(x, y, z)| {
        Rubix::solved(Box::new(Cuboid::new(x, y, z)))
    })
}
