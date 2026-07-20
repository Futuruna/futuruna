// T04: Structs, field access, methods
struct Point {
    x: i64,
    y: i64,
}

fn manhattan(p: &Point) -> i64 {
    let ax = if p.x < 0 { -p.x } else { p.x };
    let ay = if p.y < 0 { -p.y } else { p.y };
    ax + ay
}

fn add_points(a: &Point, b: &Point) -> Point {
    Point { x: a.x + b.x, y: a.y + b.y }
}

fn main() {
    let p1 = Point { x: 3, y: -4 };
    let p2 = Point { x: 1, y: 2 };
    let p3 = add_points(&p1, &p2);
    println!("{}", manhattan(&p1));
    println!("{}", manhattan(&p2));
    println!("{}", p3.x);
    println!("{}", p3.y);
}
