//! Adversarial Review: Futuruna Algorithmic Unlocks (#258) + Future Frontiers (#259)
//!
//! Tests:
//! 1. "Inexpressible" claim — can consequence-aware sorting be done in Python?
//! 2. TSP S_τ heuristic — expanded benchmark (20 seeds, multiple sizes)
//! 3. S_τ on call graphs — does it just correlate with degree? (null model)
//! 4. S_τ on "harmonic graphs" — does it distinguish structure or just size?
//! 5. "Reactive DP" — is incremental invalidation actually a language feature or a library?

use std::collections::BTreeMap;
use std::collections::BTreeSet;

// ── Dense matrix math (from molecular_entropy pattern) ──

fn mat_mul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = vec![0.0f64; n * n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i * n + k];
            if aik == 0.0 { continue; }
            for j in 0..n {
                c[i * n + j] += aik * b[k * n + j];
            }
        }
    }
    c
}

fn mat_pow(p: &[f64], n: usize, tau: u32) -> Vec<f64> {
    let mut result = vec![0.0f64; n * n];
    for i in 0..n { result[i * n + i] = 1.0; }
    let mut base = p.to_vec();
    let mut exp = tau;
    while exp > 0 {
        if exp & 1 == 1 { result = mat_mul(&result, &base, n); }
        base = mat_mul(&base, &base, n);
        exp >>= 1;
    }
    result
}

fn shannon_entropy(dist: &[f64]) -> f64 {
    let mut h = 0.0f64;
    for &p in dist {
        if p > 1e-30 { h -= p * p.log2(); }
    }
    h
}

fn avg_entropy(ptau: &[f64], n: usize) -> f64 {
    let mut total = 0.0;
    for i in 0..n {
        let dist = &ptau[i * n..(i + 1) * n];
        total += shannon_entropy(dist);
    }
    total / n as f64
}

fn node_entropy(ptau: &[f64], n: usize, node: usize) -> f64 {
    let dist = &ptau[node * n..(node + 1) * n];
    shannon_entropy(dist)
}

// ── Graph helpers ──

struct Graph {
    n: usize,
    edges: Vec<(usize, usize, f64)>, // (from, to, weight)
}

impl Graph {
    fn new(n: usize) -> Self {
        Graph { n, edges: Vec::new() }
    }

    fn edge(&mut self, a: usize, b: usize) {
        self.edges.push((a, b, 1.0));
        self.edges.push((b, a, 1.0));
    }

    fn directed_edge(&mut self, a: usize, b: usize, w: f64) {
        self.edges.push((a, b, w));
    }

    fn transition_matrix(&self) -> Vec<f64> {
        let n = self.n;
        let mut deg = vec![0.0f64; n];
        for &(u, _, w) in &self.edges {
            deg[u] += w;
        }
        let mut p = vec![0.0f64; n * n];
        for &(u, v, w) in &self.edges {
            if deg[u] > 0.0 {
                p[u * n + v] += w / deg[u];
            }
        }
        p
    }

    fn s_tau_avg(&self, tau: u32) -> f64 {
        let p = self.transition_matrix();
        let ptau = mat_pow(&p, self.n, tau);
        avg_entropy(&ptau, self.n)
    }

    fn s_tau_node(&self, tau: u32, node: usize) -> f64 {
        let p = self.transition_matrix();
        let ptau = mat_pow(&p, self.n, tau);
        node_entropy(&ptau, self.n, node)
    }

    fn degrees(&self) -> Vec<usize> {
        let mut deg = vec![0usize; self.n];
        for &(u, _, _) in &self.edges {
            deg[u] += 1;
        }
        deg
    }
}

// ── LCG ──

struct Lcg { state: u64 }
impl Lcg {
    fn new(seed: u64) -> Self { Lcg { state: seed } }
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 33) as f64 / (1u64 << 31) as f64
    }
    fn next_usize(&mut self, max: usize) -> usize {
        (self.next_f64() * max as f64) as usize % max
    }
}

fn correlation(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    if vx < 1e-30 || vy < 1e-30 { return 0.0; }
    cov / (vx.sqrt() * vy.sqrt())
}

// ══════════════════════════════════════════════════════════
// TEST 1: "Inexpressible" — Is consequence-aware sorting
// truly impossible in other languages?
// ══════════════════════════════════════════════════════════

fn test_inexpressible() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 1: Is consequence-aware sorting inexpressible?   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // The claim: a sort whose comparison function references downstream
    // consumers is "inexpressible" in Python/Rust/Haskell.
    //
    // Adversarial counter-argument:
    // - In Python: sorted(items, key=lambda x: simulate_downstream(x, consumers))
    //   This DOES work. The key function can call any code including downstream simulation.
    // - In Rust: items.sort_by(|a, b| { let sa = simulate(a); ... })
    //   Same thing. The closure captures the consumer reference.
    // - In Haskell: sortBy (\a b -> comparing (simulateDownstream consumers) a b)
    //
    // The REAL difference: in Futuruna, the sort AUTOMATICALLY re-evaluates when
    // downstream state changes. In Python, you'd have to know to re-call sorted().
    //
    // But: RxJS, Svelte stores, React useMemo with deps — all do automatic
    // re-evaluation of derived computations when dependencies change.
    //
    // Verdict: consequence-aware sorting is NOT inexpressible in other languages.
    // It IS more naturally expressed in Futuruna (one line vs framework + glue code).
    // The claim should be "naturally expressible" not "inexpressible."

    println!("  CLAIM: Consequence-aware sorting is 'inexpressible' in Python/Rust/Haskell");
    println!();
    println!("  Counter-examples:");
    println!("    Python:  sorted(items, key=lambda x: simulate_downstream(x, consumers))");
    println!("    Rust:    items.sort_by(|a, b| simulate(a, &consumers).cmp(&simulate(b, &consumers)))");
    println!("    Haskell: sortBy (comparing (simulateDownstream consumers))");
    println!();
    println!("  These ALL work. The comparison function can reference any state.");
    println!();
    println!("  The AUTOMATIC RE-EVALUATION when downstream changes:");
    println!("    RxJS:    items$.pipe(switchMap(items => combineLatest([consumers$]).pipe(");
    println!("               map(([c]) => items.sort((a,b) => simulate(a,c) - simulate(b,c)))");
    println!("             )))");
    println!("    Svelte:  $: sorted = items.sort((a,b) => simulate(a, $consumers) - simulate(b, $consumers))");
    println!();
    println!("  ❌ VERDICT: NOT INEXPRESSIBLE. Consequence-aware sorting works in all major");
    println!("     languages. Reactive re-evaluation exists in RxJS, Svelte, MobX, etc.");
    println!("     Futuruna makes it MORE NATURAL (one construct vs framework), not POSSIBLE.");
    println!("     The claim 'inexpressible' is FALSE. Replace with 'naturally expressible.'");
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 2: TSP S_τ heuristic — expanded 20-seed benchmark
// ══════════════════════════════════════════════════════════

struct City { x: f64, y: f64 }

fn dist(a: &City, b: &City) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn tour_length(cities: &[City], tour: &[usize]) -> f64 {
    let n = tour.len();
    (0..n).map(|i| dist(&cities[tour[i]], &cities[tour[(i + 1) % n]])).sum()
}

fn nearest_neighbor(cities: &[City], start: usize) -> Vec<usize> {
    let n = cities.len();
    let mut tour = vec![start];
    let mut visited = BTreeSet::new();
    visited.insert(start);
    for _ in 1..n {
        let current = *tour.last().unwrap();
        let best = (0..n)
            .filter(|j| !visited.contains(j))
            .min_by(|&a, &b| dist(&cities[current], &cities[a])
                .partial_cmp(&dist(&cities[current], &cities[b])).unwrap())
            .unwrap();
        tour.push(best);
        visited.insert(best);
    }
    tour
}

fn stau_on_remaining(cities: &[City], remaining: &[usize], source: usize, tau: u32) -> f64 {
    let n = remaining.len();
    if n <= 1 { return 0.0; }
    let mut id_to_idx = BTreeMap::new();
    for (idx, &node) in remaining.iter().enumerate() {
        id_to_idx.insert(node, idx);
    }
    let source_idx = match id_to_idx.get(&source) {
        Some(&idx) => idx,
        None => return 0.0,
    };
    let mut p = vec![0.0f64; n * n];
    for (idx_i, &node_i) in remaining.iter().enumerate() {
        let mut ws = 0.0f64;
        for (idx_j, &node_j) in remaining.iter().enumerate() {
            if idx_i != idx_j {
                let w = 1.0 / (dist(&cities[node_i], &cities[node_j]) + 1e-10);
                p[idx_i * n + idx_j] = w;
                ws += w;
            }
        }
        if ws > 0.0 { for j in 0..n { p[idx_i * n + j] /= ws; } }
    }
    let ptau = mat_pow(&p, n, tau);
    shannon_entropy(&ptau[source_idx * n..(source_idx + 1) * n])
}

fn stau_greedy_tsp(cities: &[City], start: usize, tau: u32) -> Vec<usize> {
    let n = cities.len();
    let mut tour = vec![start];
    let mut visited = BTreeSet::new();
    visited.insert(start);
    for _ in 1..n {
        let remaining: Vec<usize> = (0..n).filter(|x| !visited.contains(x)).collect();
        if remaining.len() == 1 {
            tour.push(remaining[0]);
            visited.insert(remaining[0]);
            continue;
        }
        let mut best = remaining[0];
        let mut best_s = f64::NEG_INFINITY;
        for &c in &remaining {
            let future: Vec<usize> = remaining.iter().filter(|&&x| x != c).copied().collect();
            if future.is_empty() { continue; }
            let mut fw = vec![c];
            fw.extend_from_slice(&future);
            let s = stau_on_remaining(cities, &fw, c, tau);
            if s > best_s { best_s = s; best = c; }
        }
        tour.push(best);
        visited.insert(best);
    }
    tour
}

fn two_opt(cities: &[City], tour: &mut Vec<usize>) {
    let n = tour.len();
    let mut improved = true;
    while improved {
        improved = false;
        for i in 0..n - 1 {
            for j in i + 2..n {
                if j == n - 1 && i == 0 { continue; }
                let d_old = dist(&cities[tour[i]], &cities[tour[i + 1]])
                    + dist(&cities[tour[j]], &cities[tour[(j + 1) % n]]);
                let d_new = dist(&cities[tour[i]], &cities[tour[j]])
                    + dist(&cities[tour[i + 1]], &cities[tour[(j + 1) % n]]);
                if d_new < d_old - 1e-10 {
                    tour[i + 1..=j].reverse();
                    improved = true;
                }
            }
        }
    }
}

fn clustered_cities(n: usize, k: usize, seed: u64) -> Vec<City> {
    let mut rng = Lcg::new(seed);
    let centers: Vec<City> = (0..k).map(|_| City { x: rng.next_f64() * 100.0, y: rng.next_f64() * 100.0 }).collect();
    (0..n).map(|i| {
        let c = &centers[i % k];
        let dx = (rng.next_f64() + rng.next_f64() + rng.next_f64() - 1.5) * 8.0;
        let dy = (rng.next_f64() + rng.next_f64() + rng.next_f64() - 1.5) * 8.0;
        City { x: c.x + dx, y: c.y + dy }
    }).collect()
}

fn random_cities(n: usize, seed: u64) -> Vec<City> {
    let mut rng = Lcg::new(seed);
    (0..n).map(|_| City { x: rng.next_f64() * 100.0, y: rng.next_f64() * 100.0 }).collect()
}

fn test_tsp_expanded() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 2: TSP S_τ heuristic — 20-seed expanded test    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let tau = 3;

    // Test: does S_τ-greedy + 2-opt beat NN + 2-opt across 20 seeds?
    for (label, n, k) in [("Clustered N=15, k=4", 15usize, 4usize),
                           ("Clustered N=20, k=4", 20, 4),
                           ("Clustered N=25, k=5", 25, 5),
                           ("Uniform N=15", 15, 0),
                           ("Uniform N=20", 20, 0)] {
        let mut nn_wins = 0u32;
        let mut stau_wins = 0u32;
        let mut ties = 0u32;
        let mut nn_total = 0.0;
        let mut stau_total = 0.0;

        for seed in 1..=20u64 {
            let cities = if k > 0 { clustered_cities(n, k, seed * 97) }
                         else { random_cities(n, seed * 97) };

            // Best over all starts
            let mut best_nn = f64::MAX;
            let mut best_stau = f64::MAX;

            for start in 0..n {
                let nn = nearest_neighbor(&cities, start);
                let mut nn2 = nn;
                two_opt(&cities, &mut nn2);
                let nn_len = tour_length(&cities, &nn2);
                if nn_len < best_nn { best_nn = nn_len; }

                let st = stau_greedy_tsp(&cities, start, tau);
                let mut st2 = st;
                two_opt(&cities, &mut st2);
                let st_len = tour_length(&cities, &st2);
                if st_len < best_stau { best_stau = st_len; }
            }

            nn_total += best_nn;
            stau_total += best_stau;

            if (best_nn - best_stau).abs() < 0.01 {
                ties += 1;
            } else if best_nn < best_stau {
                nn_wins += 1;
            } else {
                stau_wins += 1;
            }
        }

        let pct = (stau_total / nn_total - 1.0) * 100.0;
        println!("  {:<25}  NN wins: {:>2}  S_τ wins: {:>2}  ties: {:>2}  S_τ vs NN: {:>+.2}%",
            label, nn_wins, stau_wins, ties, pct);
    }

    println!();
    println!("  If S_τ wins ≈ NN wins and total difference < 1%:");
    println!("  → S_τ is a DIVERSIFIER (different basin), not a BETTER heuristic");
    println!("  → F127 should say 'comparable starting point' not 'beats'");
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 3: S_τ on call graphs — degree confound
// ══════════════════════════════════════════════════════════

fn test_callgraph_degree_confound() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 3: S_τ on call graphs — degree confound?        ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Build synthetic "call graphs" of different architectures
    // and check: does S_τ per node just correlate with degree?
    // If r(S_τ, degree) > 0.95, the metric adds nothing beyond "count edges"

    let tau = 3;

    // Architecture 1: Star (one hub calls everything)
    let mut star = Graph::new(10);
    for i in 1..10 { star.edge(0, i); }

    // Architecture 2: Chain (A→B→C→D→...)
    let mut chain = Graph::new(10);
    for i in 0..9 { chain.edge(i, i + 1); }

    // Architecture 3: Small-world (chain + shortcuts)
    let mut sw = Graph::new(10);
    for i in 0..10 { sw.edge(i, (i + 1) % 10); }
    sw.edge(0, 5);
    sw.edge(2, 7);
    sw.edge(3, 8);

    // Architecture 4: Two clusters with one bridge
    let mut clusters = Graph::new(10);
    // Cluster A: 0-4 fully connected
    for i in 0..5 { for j in i+1..5 { clusters.edge(i, j); } }
    // Cluster B: 5-9 fully connected
    for i in 5..10 { for j in i+1..10 { clusters.edge(i, j); } }
    // Bridge
    clusters.edge(4, 5);

    // Architecture 5: Random Erdos-Renyi
    let mut er = Graph::new(10);
    let mut rng = Lcg::new(42);
    for i in 0..10 {
        for j in i+1..10 {
            if rng.next_f64() < 0.3 {
                er.edge(i, j);
            }
        }
    }

    let architectures: Vec<(&str, &Graph)> = vec![
        ("Star", &star),
        ("Chain", &chain),
        ("Small-World", &sw),
        ("Two-Clusters", &clusters),
        ("Erdos-Renyi", &er),
    ];

    println!("  Architecture      | Avg S_τ | S_τ range    | r(S_τ, deg) | Adds info?");
    println!("  ------------------|---------|--------------|-------------|----------");

    for (name, g) in &architectures {
        let p = g.transition_matrix();
        let ptau = mat_pow(&p, g.n, tau);
        let degrees = g.degrees();

        let mut s_taus = Vec::new();
        let mut degs_f = Vec::new();
        let mut min_s = f64::MAX;
        let mut max_s = f64::MIN;

        for i in 0..g.n {
            let s = node_entropy(&ptau, g.n, i);
            s_taus.push(s);
            degs_f.push(degrees[i] as f64);
            if s < min_s { min_s = s; }
            if s > max_s { max_s = s; }
        }

        let r = correlation(&s_taus, &degs_f);
        let adds = if r.abs() < 0.9 { "YES" } else { "NO (≈degree)" };

        println!("  {:<18}| {:>7.3} | {:.3}–{:.3}    | {:>+.3}       | {}",
            name,
            s_taus.iter().sum::<f64>() / s_taus.len() as f64,
            min_s, max_s,
            r,
            adds
        );
    }

    println!();
    println!("  KEY QUESTION: If r(S_τ, degree) > 0.9 for most architectures,");
    println!("  then 'S_τ on call graphs' is just a fancy way to count edges.");
    println!("  S_τ adds value ONLY where it diverges from degree.");
    println!();
    // The Two-Clusters case should be interesting: the bridge node (4,5) has
    // lower degree than interior nodes but higher S_τ (more diverse reach)
    println!("  Two-Clusters bridge test (node 4 and 5 are bridges):");
    {
        let g = &clusters;
        let p = g.transition_matrix();
        let ptau = mat_pow(&p, g.n, tau);
        let degrees = g.degrees();
        for i in 0..g.n {
            let s = node_entropy(&ptau, g.n, i);
            let d = degrees[i];
            let role = if i == 4 || i == 5 { "BRIDGE" } else if i < 5 { "cluster_A" } else { "cluster_B" };
            println!("    Node {}: degree={}, S_τ={:.3}  [{}]", i, d, s, role);
        }
    }
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 4: "Bach has higher S_τ" — does it distinguish
// structure or just graph size?
// ══════════════════════════════════════════════════════════

fn test_harmonic_graphs() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 4: S_τ on harmonic graphs — structure or size?   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let tau = 3;

    // Simple hymn: I-IV-V-I chord progression
    // 3 chords, simple cycle
    let mut hymn = Graph::new(3);
    hymn.edge(0, 1); // I → IV
    hymn.edge(1, 2); // IV → V
    hymn.edge(2, 0); // V → I

    // Pop: I-V-vi-IV loop
    // 4 chords, cycle
    let mut pop = Graph::new(4);
    pop.edge(0, 1); // I → V
    pop.edge(1, 2); // V → vi
    pop.edge(2, 3); // vi → IV
    pop.edge(3, 0); // IV → I

    // Jazz: ii-V-I with substitutions
    // 6 chords, richer connections
    let mut jazz = Graph::new(6);
    jazz.edge(0, 1); // ii → V
    jazz.edge(1, 2); // V → I
    jazz.edge(2, 0); // I → ii
    jazz.edge(0, 3); // ii → bII (tritone sub)
    jazz.edge(3, 2); // bII → I
    jazz.edge(2, 4); // I → vi
    jazz.edge(4, 0); // vi → ii
    jazz.edge(1, 5); // V → iii (deceptive)
    jazz.edge(5, 4); // iii → vi

    // Bach fugue: complex harmonic web
    // 8 chords, many connections (modulations)
    let mut fugue = Graph::new(8);
    fugue.edge(0, 1); // I → ii
    fugue.edge(1, 2); // ii → V
    fugue.edge(2, 0); // V → I
    fugue.edge(0, 3); // I → vi
    fugue.edge(3, 1); // vi → ii
    fugue.edge(2, 3); // V → vi (deceptive)
    fugue.edge(0, 4); // I → IV
    fugue.edge(4, 2); // IV → V
    fugue.edge(4, 1); // IV → ii
    fugue.edge(3, 5); // vi → iii
    fugue.edge(5, 4); // iii → IV
    fugue.edge(5, 0); // iii → I (mediant)
    fugue.edge(0, 6); // I → V/V (secondary dominant)
    fugue.edge(6, 2); // V/V → V
    fugue.edge(3, 7); // vi → bVII (modal mixture)
    fugue.edge(7, 0); // bVII → I

    // NULL MODEL: random graph with same number of nodes/edges as fugue
    let mut null_8 = Graph::new(8);
    let mut rng = Lcg::new(42);
    let fugue_edge_count = fugue.edges.len();
    let mut null_edge_count = 0;
    while null_edge_count < fugue_edge_count {
        let a = rng.next_usize(8);
        let b = rng.next_usize(8);
        if a != b {
            null_8.directed_edge(a, b, 1.0);
            null_edge_count += 1;
        }
    }

    let graphs: Vec<(&str, &Graph)> = vec![
        ("Hymn (3 chords)", &hymn),
        ("Pop (4 chords)", &pop),
        ("Jazz (6 chords)", &jazz),
        ("Bach fugue (8 chords)", &fugue),
        ("Random (8 nodes, same edges)", &null_8),
    ];

    println!("  Genre               | Nodes | Edges | Avg S_τ | S_τ/node | S_τ/edge");
    println!("  --------------------|-------|-------|---------|----------|--------");

    for (name, g) in &graphs {
        let s = g.s_tau_avg(tau);
        let edge_count = g.edges.len();
        println!("  {:<20}| {:>5} | {:>5} | {:>7.3} | {:>8.3} | {:>7.4}",
            name, g.n, edge_count, s,
            s / g.n as f64,
            if edge_count > 0 { s / edge_count as f64 } else { 0.0 }
        );
    }

    println!();
    println!("  KEY QUESTION: Does S_τ increase just because the graph is larger?");
    println!("  If S_τ/node and S_τ/edge are roughly constant across genres,");
    println!("  then 'Bach has higher S_τ' is trivially true (more chords = more entropy)");
    println!("  and F135 is unfalsifiable as stated.");
    println!();
    println!("  The NULL model tests: does a random graph with the same size have");
    println!("  similar S_τ? If yes, the harmonic STRUCTURE doesn't matter — just the size.");
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 5: "Reactive DP is a language primitive" — is it?
// ══════════════════════════════════════════════════════════

fn test_reactive_dp() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 5: Is reactive DP a language feature or library? ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  CLAIM: Futuruna makes reactive DP a 'language primitive'");
    println!();
    println!("  Counter-examples of reactive/incremental DP in existing languages:");
    println!("    1. Adapton (Rust) — general-purpose incremental computation");
    println!("       adapton::cell!, adapton::thunk! — automatic change propagation");
    println!("    2. Incremental (OCaml/Jane Street) — production-grade reactive DP");
    println!("       Used in real trading systems at Jane Street");
    println!("    3. Salsa (Rust) — incremental computation framework");
    println!("       Powers rust-analyzer, the Rust IDE engine");
    println!("    4. Differential Dataflow (Rust/Timely) — streaming incremental computation");
    println!("       Frank McSherry's framework, handles trillion-edge graphs");
    println!("    5. React useMemo/useCallback — reactive memoization (limited)");
    println!("    6. Excel — the original reactive DP system (cells auto-update)");
    println!();
    println!("  ❌ VERDICT: Reactive DP exists as libraries in Rust, OCaml, and as");
    println!("     a framework in Differential Dataflow. Salsa powers rust-analyzer.");
    println!("     The claim 'never been a language primitive' is technically true");
    println!("     (these are libraries, not language features), but the PRACTICAL");
    println!("     difference is small — Salsa users don't feel limited by Rust.");
    println!("     Replace 'never been a language primitive' with 'first-class");
    println!("     rather than library-mediated.'");
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 6: "Self-evolving syntax" — has anyone else done this?
// ══════════════════════════════════════════════════════════

fn test_self_evolving() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 6: Is self-evolving syntax genuinely novel?      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  CLAIM: No other language can evolve its own syntax because no other");
    println!("  language has a 'quantitative measure of syntactic quality'");
    println!();
    println!("  Counter-examples:");
    println!("    1. Racket — macros + #lang create new syntaxes. Language-oriented programming.");
    println!("       Languages evolve within Racket routinely (Typed Racket, Scribble, etc.)");
    println!("    2. Lisp macros — syntax extension is Lisp's defining feature");
    println!("    3. Grammatical Evolution (Ryan & O'Neill, 2003) — uses genetic algorithms");
    println!("       to evolve BNF grammars. No consciousness metric but does optimize");
    println!("       grammars via evolutionary search.");
    println!("    4. GP-based language design (Helmuth et al.) — evolves domain-specific");
    println!("       languages using genetic programming");
    println!();
    println!("  What IS novel about Futuruna's approach:");
    println!("    - The OBJECTIVE FUNCTION (Φ, d_eff, S_τ) is from physics, not arbitrary");
    println!("    - The same metric applies to language, code, music, economics");
    println!("    - Prior work evolves syntax for TASK PERFORMANCE (lower error on benchmarks)");
    println!("      Futuruna evolves syntax for STRUCTURAL QUALITY (consciousness metrics)");
    println!();
    println!("  ⚠ VERDICT: Self-evolving syntax exists (Racket, Lisp, grammatical evolution).");
    println!("     What's novel is the METRIC, not the capability. Rephrase from");
    println!("     'no other language can do this' to 'no other language has a");
    println!("     physics-grounded metric to guide syntax evolution.'");
    println!();
}

// ══════════════════════════════════════════════════════════
// TEST 7: S_τ on call graphs — partial r after controlling for degree
// ══════════════════════════════════════════════════════════

fn test_partial_correlation() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TEST 7: Partial r(S_τ, quality | degree) = ???        ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Generate many random graphs, compute S_τ, degree, and a "quality" proxy
    // (we'll use betweenness-like centrality as quality proxy)
    // Then check: does S_τ predict quality BEYOND what degree already predicts?

    let tau = 3;
    let mut rng = Lcg::new(42);
    let mut all_stau = Vec::new();
    let mut all_deg = Vec::new();
    let mut all_betw = Vec::new(); // proxy: path diversity (how many shortest paths go through node)

    for _ in 0..50 {
        let n = 8 + rng.next_usize(8); // 8-15 nodes
        let mut g = Graph::new(n);
        for i in 0..n {
            for j in i+1..n {
                if rng.next_f64() < 0.25 {
                    g.edge(i, j);
                }
            }
        }
        // Ensure connected: add chain
        for i in 0..n-1 {
            g.edge(i, i+1);
        }

        let p = g.transition_matrix();
        let ptau = mat_pow(&p, n, tau);
        let degrees = g.degrees();

        // "Betweenness proxy": for each node, how uniform is its τ-step distribution?
        // High entropy = paths spread to many destinations = high betweenness-like quality
        // (This IS S_τ by definition, so we need a different quality metric)
        //
        // Better proxy: "reach diversity" = number of nodes reachable with P > 0.01
        for i in 0..n {
            let s = node_entropy(&ptau, n, i);
            let d = degrees[i] as f64;
            let dist = &ptau[i * n..(i + 1) * n];
            let reach = dist.iter().filter(|&&p| p > 0.05).count() as f64;
            all_stau.push(s);
            all_deg.push(d);
            all_betw.push(reach);
        }
    }

    let r_sd = correlation(&all_stau, &all_deg);
    let r_sb = correlation(&all_stau, &all_betw);
    let r_db = correlation(&all_deg, &all_betw);

    // Partial correlation: r(S_τ, reach | degree) = (r_sb - r_sd * r_db) / sqrt((1-r_sd²)(1-r_db²))
    let partial = (r_sb - r_sd * r_db) / ((1.0 - r_sd * r_sd) * (1.0 - r_db * r_db)).sqrt();

    println!("  50 random graphs (8-15 nodes), all nodes measured");
    println!("  N = {} node measurements", all_stau.len());
    println!();
    println!("  r(S_τ, degree)           = {:.3}", r_sd);
    println!("  r(S_τ, reach)            = {:.3}", r_sb);
    println!("  r(degree, reach)         = {:.3}", r_db);
    println!("  r(S_τ, reach | degree)   = {:.3}  ← PARTIAL CORRELATION", partial);
    println!();
    if partial.abs() < 0.1 {
        println!("  ❌ S_τ adds NOTHING beyond degree for predicting reach.");
        println!("     F132 is likely FALSE — S_τ on call graphs ≈ counting edges.");
    } else if partial.abs() < 0.3 {
        println!("  ⚠ S_τ adds WEAK information beyond degree (partial r = {:.3}).", partial);
        println!("     F132 needs qualification: S_τ captures some structure beyond degree,");
        println!("     but the effect is small.");
    } else {
        println!("  ✓ S_τ adds SUBSTANTIAL information beyond degree (partial r = {:.3}).", partial);
        println!("     F132 plausible: S_τ captures structural properties that degree misses.");
    }
    println!();

    // Also: the #227 lesson — does a null model (random assignment of S_τ values
    // to nodes with the same degree distribution) give similar correlations?
    println!("  #227 LESSON CHECK: Does degree → S_τ → quality create a circular chain?");
    println!("  If r(S_τ, degree) > 0.9, then S_τ IS degree with extra steps.");
    if r_sd > 0.9 {
        println!("  ❌ YES — r(S_τ, degree) = {:.3}. Same confound as #227.", r_sd);
    } else {
        println!("  ✓ NO — r(S_τ, degree) = {:.3}. S_τ has independent information.", r_sd);
    }
    println!();
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  ADVERSARIAL REVIEW: Futuruna Insights #258 + #259                  ║");
    println!("║  Testing 7 claims from 'Algorithmic Unlocks' + 'Frontiers'     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    test_inexpressible();
    test_tsp_expanded();
    test_callgraph_degree_confound();
    test_harmonic_graphs();
    test_reactive_dp();
    test_self_evolving();
    test_partial_correlation();

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  SUMMARY                                                       ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Claims to REVISE:                                             ║");
    println!("║  1. 'Inexpressible' → 'more naturally expressible'             ║");
    println!("║  2. 'S_τ beats NN' → 'S_τ is a diversifier for local search'  ║");
    println!("║  3. 'Reactive DP never a primitive' → 'first-class, not lib'   ║");
    println!("║  4. 'No other language can evolve syntax' → 'novel METRIC'     ║");
    println!("║                                                                ║");
    println!("║  Claims that SURVIVE or FAIL depend on computational tests:    ║");
    println!("║  5. S_τ on call graphs — check r(S_τ, degree) and partial r   ║");
    println!("║  6. Harmonic S_τ — check S_τ/node normalization               ║");
    println!("║  7. F132 — check partial correlation after degree control      ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
