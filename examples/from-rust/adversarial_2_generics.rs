// Adversarial 2: Generics, trait bounds, associated types
// The kind of Rust that makes intermediate developers sweat

trait Functor {
    type Inner;
    type Mapped<B>;
    fn fmap<B, F: Fn(Self::Inner) -> B>(self, f: F) -> Self::Mapped<B>;
}

impl<A> Functor for Option<A> {
    type Inner = A;
    type Mapped<B> = Option<B>;
    fn fmap<B, F: Fn(A) -> B>(self, f: F) -> Option<B> {
        self.map(f)
    }
}

impl<A, E> Functor for Result<A, E> {
    type Inner = A;
    type Mapped<B> = Result<B, E>;
    fn fmap<B, F: Fn(A) -> B>(self, f: F) -> Result<B, E> {
        self.map(f)
    }
}

fn double_inner<T: Functor<Inner = i64>>(container: T) -> T::Mapped<i64> {
    container.fmap(|x| x * 2)
}

// Higher-kinded vibes
fn apply_twice<A, F: Fn(A) -> A>(f: F, x: A) -> A {
    f(f(x))
}

fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |x| g(f(x))
}

#[derive(Debug, Clone)]
struct Pair<A, B> {
    first: A,
    second: B,
}

impl<A, B> Pair<A, B> {
    fn map_first<C, F: Fn(A) -> C>(self, f: F) -> Pair<C, B> {
        Pair { first: f(self.first), second: self.second }
    }

    fn map_second<C, F: Fn(B) -> C>(self, f: F) -> Pair<A, C> {
        Pair { first: self.first, second: f(self.second) }
    }

    fn both<C, D, F: Fn(A) -> C, G: Fn(B) -> D>(self, f: F, g: G) -> Pair<C, D> {
        Pair { first: f(self.first), second: g(self.second) }
    }
}

fn main() {
    // Functor usage
    let x: Option<i64> = Some(21);
    let doubled = double_inner(x);
    println!("Doubled: {:?}", doubled);

    let y: Result<i64, String> = Ok(10);
    let tripled = y.fmap(|n| n * 3);
    println!("Tripled: {:?}", tripled);

    // Higher-order functions
    let inc = |x: i64| x + 1;
    let result = apply_twice(inc, 5);
    println!("Apply twice: {}", result);

    let add_one_then_double = compose(|x: i64| x + 1, |x: i64| x * 2);
    println!("Composed: {}", add_one_then_double(10));

    // Pair
    let p = Pair { first: "hello", second: 42 };
    let p2 = p.map_second(|n| n * 2);
    println!("Pair: ({}, {})", p2.first, p2.second);
}
