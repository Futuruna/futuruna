// runa-from-rust: expect-unsupported iterator state machine outside the checked scan subset

fn main() {
    let values: Vec<i64> = (0..4)
        .scan(0, |state, x| {
            *state = *state + x;
            Some(*state)
        })
        .collect();

    println!("values={:?}", values);
}
