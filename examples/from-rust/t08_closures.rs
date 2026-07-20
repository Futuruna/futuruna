// T08: Closures, higher-order functions, captures
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
fn twice(f: fn(i64) -> i64, x: i64) -> i64 { f(f(x)) }

fn make_adder(n: i64) -> Box<dyn Fn(i64) -> i64> {
    Box::new(move |x| x + n)
}

fn main() {
    println!("{}", apply(|x| x * 2, 5));
    println!("{}", twice(|x| x + 3, 10));

    let add5 = make_adder(5);
    println!("{}", add5(10));
    println!("{}", add5(20));

    let nums = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let evens: Vec<i64> = nums.iter().filter(|x| **x % 2 == 0).copied().collect();
    let doubled: Vec<i64> = evens.iter().map(|x| x * 2).collect();
    let total: i64 = doubled.iter().sum();
    println!("{}", total);
}
