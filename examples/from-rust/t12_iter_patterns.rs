// T12: Common iterator patterns people actually write
fn sum_squares(n: i64) -> i64 {
    (1..=n).map(|x| x * x).sum()
}

fn count_divisors(n: i64) -> i64 {
    (1..=n).filter(|i| n % i == 0).count() as i64
}

fn flatten_sum(vecs: &[Vec<i64>]) -> i64 {
    vecs.iter().flat_map(|v| v.iter()).sum()
}

fn zip_sum(a: &[i64], b: &[i64]) -> Vec<i64> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

fn main() {
    println!("{}", sum_squares(10));
    println!("{}", count_divisors(12));
    println!("{}", count_divisors(7));

    let vecs = vec![vec![1, 2], vec![3, 4], vec![5]];
    println!("{}", flatten_sum(&vecs));

    let a = vec![1, 2, 3];
    let b = vec![10, 20, 30];
    let c = zip_sum(&a, &b);
    for x in &c {
        println!("{}", x);
    }
}
