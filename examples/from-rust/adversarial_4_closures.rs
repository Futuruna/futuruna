// Adversarial 4: Closures, iterators, functional chains
// The kind of Rust that reads like Haskell
// runa-from-rust: expect-unsupported iterator scan/sort_by/entry chains with mutable closure state

fn pipeline_demo() {
    let data = vec![
        ("alice", vec![95, 87, 92, 88]),
        ("bob", vec![72, 68, 75, 80]),
        ("charlie", vec![100, 98, 95, 97]),
        ("diana", vec![60, 55, 70, 65]),
    ];

    // Average scores, filtered to passing (>70), sorted descending
    let mut results: Vec<(&str, f64)> = data.iter()
        .map(|(name, scores)| {
            let avg = scores.iter().sum::<i64>() as f64 / scores.len() as f64;
            (*name, avg)
        })
        .filter(|(_, avg)| *avg > 70.0)
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    for (name, avg) in &results {
        println!("  {}: {:.1}", name, avg);
    }

    // Fibonacci with scan
    let fibs: Vec<i64> = (0..10)
        .scan((0i64, 1i64), |state, _| {
            let next = state.0 + state.1;
            *state = (state.1, next);
            Some(state.0)
        })
        .collect();
    println!("Fibs: {:?}", fibs);

    // Group by first letter
    let words = vec!["apple", "avocado", "banana", "blueberry", "cherry", "coconut"];
    let mut groups: HashMap<char, Vec<&str>> = HashMap::new();
    for word in &words {
        let first = word.chars().next().unwrap();
        groups.entry(first).or_insert_with(Vec::new).push(word);
    }

    for (letter, words) in &groups {
        println!("  {}: {:?}", letter, words);
    }
}

use std::collections::HashMap;

fn main() {
    pipeline_demo();
}
