// T05: Enums, match, pattern matching
enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
}

fn area(s: &Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Rectangle(w, h) => w * h,
    }
}

fn describe(s: &Shape) -> String {
    match s {
        Shape::Circle(_) => "circle".to_string(),
        Shape::Rectangle(_, _) => "rectangle".to_string(),
    }
}

fn main() {
    let c = Shape::Circle(5.0);
    let r = Shape::Rectangle(3.0, 4.0);
    println!("{}", describe(&c));
    println!("{}", area(&c));
    println!("{}", describe(&r));
    println!("{}", area(&r));
}
