// runa-from-rust: expect-unsupported associated types outside the checked Functor fixture

trait Source {
    type Item;

    fn next(self) -> Self::Item;
}

struct Counter;

impl Source for Counter {
    type Item = i64;

    fn next(self) -> i64 {
        7
    }
}

fn main() {
    println!("next={}", Counter.next());
}
