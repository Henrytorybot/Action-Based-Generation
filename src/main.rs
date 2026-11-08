use std::collections::HashMap;

fn main() {
    let mut world: HashMap<Point, Tile> = HashMap::new();

    for x in 0..80 {    
        for y in 0..80 {

            world.insert(Point{x: x, y: y}, Tile{wall: rand::random()});
        }
    };

    for y in 0..80{
        for x in 0..80{
            if let Some(tile) = world.get(&Point{x:x,y:y}){
                if tile.wall == true{
                    print!("█");
                }else{
                    print!(" ");
                };
            };
        }
        println!("");
    }
}

#[derive(Hash, Eq, PartialEq)]
struct Point {
    x: i8,
    y: i8,
}

struct Tile {
//    roomPosition: Point,
//    housePosition: Point,
//    lotPosition: Point,
//    blockPosition: Point,
    wall: bool,
}
