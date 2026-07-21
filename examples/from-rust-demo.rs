/// A point in 2D space
struct Point {
    x: f64,
    y: f64,
}

enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle { base: f64, height: f64 },
}

fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Rectangle(w, h) => w * h,
        Shape::Triangle { base, height } => 0.5 * base * height,
    }
}

fn distance(a: &Point, b: &Point) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}

fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn safe_divide(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

fn main() {
    let origin = Point { x: 0.0, y: 0.0 };
    let p = Point { x: 3.0, y: 4.0 };
    println!("Distance: {}", distance(&origin, &p));

    let shapes = vec![
        Shape::Circle(5.0),
        Shape::Rectangle(3.0, 4.0),
        Shape::Triangle {
            base: 6.0,
            height: 3.0,
        },
    ];

    for shape in &shapes {
        println!("Area: {}", area(shape));
    }

    greet("World");
    println!("fib(10) = {}", fibonacci(10));

    let nums: Vec<i64> = vec![1, 2, 3, 4, 5];
    let doubled: Vec<i64> = nums.iter().map(|x| x * 2).collect();
    let evens: Vec<i64> = nums.iter().copied().filter(|x| x % 2 == 0).collect();
    let sum: i64 = nums.iter().sum();

    println!("doubled: {:?}", doubled);
    println!("evens: {:?}", evens);
    println!("sum: {}", sum);

    match safe_divide(10, 3) {
        Ok(result) => println!("10 / 3 = {}", result),
        Err(e) => println!("Error: {}", e),
    }
}
