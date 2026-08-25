use rubixemulator::rubix::Rubix;
use rubixemulator::shapes::cuboid::Cuboid;
use rubixemulator::vec3::direction;

fn main() {
    let mut rubix = Rubix::solved(Box::new(Cuboid::new(3, 3, 3)));

    println!("Top face before: {:?}", rubix.face(direction::POS_Y));

    let moves = rubix.moves();
    let first_move = moves.first().expect("cuboid should have at least one move");
    rubix.apply(first_move, true);

    println!("Top face after: {:?}", rubix.face(direction::POS_Y));
}
