// T06: Vectors, iteration, functional operations
fn sum(xs: &[i64]) -> i64 {
    xs.iter().sum()
}

fn count_positive(xs: &[i64]) -> i64 {
    xs.iter().filter(|x| **x > 0).count() as i64
}

fn main() {
    let nums = vec![1, 2, 3, 4, 5];
    println!("{}", sum(&nums));

    let doubled: Vec<i64> = nums.iter().map(|x| x * 2).collect();
    for x in &doubled {
        println!("{}", x);
    }

    println!("{}", count_positive(&vec![-1, 2, -3, 4, 5]));
}
