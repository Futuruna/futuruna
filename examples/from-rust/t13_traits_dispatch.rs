// T13: Structs with methods (ADT-style in Futuruna)
struct Dog {
    name: String,
    breed: String,
}

struct Cat {
    name: String,
    indoor: bool,
}

fn describe_dog(d: &Dog) -> String {
    format!("{} ({})", d.name, d.breed)
}

fn describe_cat(c: &Cat) -> String {
    if c.indoor {
        format!("{} (indoor cat)", c.name)
    } else {
        format!("{} (outdoor cat)", c.name)
    }
}

fn main() {
    let d = Dog { name: "Rex".to_string(), breed: "Shepherd".to_string() };
    let c = Cat { name: "Whiskers".to_string(), indoor: true };
    println!("{}", describe_dog(&d));
    println!("{}", describe_cat(&c));
}
