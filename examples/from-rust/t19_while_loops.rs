// T19: While loops — native in Futuruna
fn collatz_steps(start: i64) -> i64 {
    let mut n = start;
    let mut steps: i64 = 0;
    while n > 1 {
        if n % 2 == 0 {
            n = n / 2;
        } else {
            n = 3 * n + 1;
        }
        steps = steps + 1;
    }
    steps
}

fn gcd(a_init: i64, b_init: i64) -> i64 {
    let mut a = a_init;
    let mut b = b_init;
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn sum_while(n: i64) -> i64 {
    let mut i = 1;
    let mut total: i64 = 0;
    while i <= n {
        total = total + i;
        i = i + 1;
    }
    total
}

fn main() {
    println!("{}", collatz_steps(27));
    println!("{}", gcd(48, 18));
    println!("{}", gcd(100, 75));
    println!("{}", sum_while(100));
}
