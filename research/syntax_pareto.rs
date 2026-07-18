//! Syntactic Pareto Frontier: Multi-Objective Optimization of Language Structure
//!
//! Explores the 3D Pareto frontier of S_τ (reach) × JSD (discrimination) × Φ (integration)
//! for syntactic transition graphs. Uses evolutionary search (NSGA-II style) starting from
//! English, Lojban, and Chinese, discovering what structural properties characterize
//! languages at the frontier of all three objectives simultaneously.
//!
//! Run: cargo run --release --bin syntax-pareto

use std::fs;

const N_DIM: usize = 5;

// ── Weighted transition graph ──

#[derive(Clone)]
struct SyntaxGraph {
    n: usize,
    labels: Vec<String>,
    weights: Vec<Vec<f64>>,
}

impl SyntaxGraph {
    fn from_tsv(pos_path: &str, bigram_path: &str) -> Self {
        let pos_str = fs::read_to_string(pos_path).expect("Need data/pos_tags.tsv");
        let bg_str = fs::read_to_string(bigram_path).expect("Need bigram file");

        let mut labels = Vec::new();
        for line in pos_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 { labels.push(parts[1].to_string()); }
        }

        let n = labels.len();
        let mut weights = vec![vec![0.0f64; n]; n];
        for line in bg_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let i: usize = parts[0].parse().unwrap_or(usize::MAX);
                let j: usize = parts[1].parse().unwrap_or(usize::MAX);
                let w: f64 = parts[2].parse().unwrap_or(0.0);
                if i < n && j < n { weights[i][j] = w; }
            }
        }

        SyntaxGraph { n, labels, weights }
    }

    fn transition_matrix(&self) -> Vec<Vec<f64>> {
        let n = self.n;
        let mut p = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            let row_sum: f64 = self.weights[i].iter().sum();
            if row_sum > 0.0 {
                for j in 0..n { p[i][j] = self.weights[i][j] / row_sum; }
            } else {
                p[i][i] = 1.0;
            }
        }
        p
    }

    fn stau_all(&self, tau: usize) -> Vec<f64> {
        let p = self.transition_matrix();
        let n = self.n;
        let mut results = vec![0.0f64; n];
        for start in 0..n {
            let mut pi = vec![0.0f64; n];
            pi[start] = 1.0;
            for _ in 0..tau {
                let mut next = vec![0.0f64; n];
                for i in 0..n {
                    if pi[i] < 1e-30 { continue; }
                    for j in 0..n { next[j] += pi[i] * p[i][j]; }
                }
                pi = next;
            }
            let mut h = 0.0f64;
            for &pr in &pi { if pr > 1e-30 { h -= pr * pr.log2(); } }
            results[start] = h;
        }
        results
    }

    fn label_idx(&self, name: &str) -> Option<usize> {
        self.labels.iter().position(|l| l == name)
    }
}

// ── Eigenstate computation ──

fn jacobi_eigen(mat: &[[f64; N_DIM]; N_DIM]) -> ([f64; N_DIM], [[f64; N_DIM]; N_DIM]) {
    let mut a = *mat;
    let mut v = [[0.0f64; N_DIM]; N_DIM];
    for i in 0..N_DIM { v[i][i] = 1.0; }

    for _ in 0..100 {
        let mut max_off = 0.0;
        let mut p = 0; let mut q = 1;
        for i in 0..N_DIM { for j in (i+1)..N_DIM {
            if a[i][j].abs() > max_off { max_off = a[i][j].abs(); p = i; q = j; }
        }}
        if max_off < 1e-12 { break; }

        let diff = a[q][q] - a[p][p];
        let t = if diff.abs() < 1e-15 { 1.0 } else {
            let tau = diff / (2.0 * a[p][q]);
            1.0 / (tau.abs() + (1.0 + tau * tau).sqrt()) * tau.signum()
        };
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;

        let app = a[p][p] - t * a[p][q];
        let aqq = a[q][q] + t * a[p][q];
        a[p][p] = app; a[q][q] = aqq; a[p][q] = 0.0; a[q][p] = 0.0;

        for r in 0..N_DIM {
            if r == p || r == q { continue; }
            let arp = a[r][p]; let arq = a[r][q];
            a[r][p] = c * arp - s * arq; a[p][r] = a[r][p];
            a[r][q] = s * arp + c * arq; a[q][r] = a[r][q];
        }
        for r in 0..N_DIM {
            let vrp = v[r][p]; let vrq = v[r][q];
            v[r][p] = c * vrp - s * vrq;
            v[r][q] = s * vrp + c * vrq;
        }
    }

    let mut idx: Vec<(usize, f64)> = (0..N_DIM).map(|i| (i, a[i][i].max(0.0))).collect();
    idx.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut sorted_evals = [0.0f64; N_DIM];
    let mut sorted_evecs = [[0.0f64; N_DIM]; N_DIM];
    for (new_i, &(old_i, _)) in idx.iter().enumerate() {
        sorted_evals[new_i] = a[old_i][old_i].max(0.0);
        for r in 0..N_DIM { sorted_evecs[r][new_i] = v[r][old_i]; }
    }
    (sorted_evals, sorted_evecs)
}

fn compute_d_eff(evals: &[f64; N_DIM]) -> usize {
    let total: f64 = evals.iter().sum();
    if total < 1e-15 { return 0; }
    let mut cum = 0.0;
    for (i, &e) in evals.iter().enumerate() {
        cum += e;
        if cum / total >= 0.80 { return (i + 1).min(N_DIM); }
    }
    N_DIM
}

fn syntactic_dims(g: &SyntaxGraph) -> Vec<[f64; N_DIM]> {
    let s1 = g.stau_all(1);
    let s3 = g.stau_all(3);
    let s5 = g.stau_all(5);
    let p = g.transition_matrix();
    let n = g.n;

    let mut in_deg = vec![0.0f64; n];
    let total_weight: f64 = g.weights.iter().flat_map(|r| r.iter()).sum();
    for j in 0..n {
        let col_sum: f64 = (0..n).map(|i| g.weights[i][j]).sum();
        in_deg[j] = if total_weight > 0.0 { col_sum / total_weight } else { 0.0 };
    }

    let mut out_entropy = vec![0.0f64; n];
    for i in 0..n {
        let mut h = 0.0f64;
        for j in 0..n {
            if p[i][j] > 1e-30 { h -= p[i][j] * p[i][j].log2(); }
        }
        out_entropy[i] = h;
    }

    let mut result = vec![[0.0f64; N_DIM]; n];
    for i in 0..n {
        result[i] = [s1[i], s3[i], s5[i], in_deg[i], out_entropy[i]];
    }
    result
}

fn compute_eigenstate(dims: &[[f64; N_DIM]]) -> (usize, [f64; N_DIM], f64) {
    let n = dims.len();
    if n < 3 { return (0, [0.0; N_DIM], 0.0); }

    let mut means = [0.0f64; N_DIM];
    let mut stds = [0.0f64; N_DIM];
    for d in 0..N_DIM { means[d] = dims.iter().map(|w| w[d]).sum::<f64>() / n as f64; }
    for d in 0..N_DIM {
        let var = dims.iter().map(|w| (w[d] - means[d]).powi(2)).sum::<f64>() / n as f64;
        stds[d] = var.sqrt();
    }

    let mut corr = [[0.0f64; N_DIM]; N_DIM];
    for a in 0..N_DIM { for b in 0..N_DIM {
        if stds[a] < 1e-12 || stds[b] < 1e-12 {
            corr[a][b] = if a == b { 1.0 } else { 0.0 };
        } else {
            let mut s = 0.0;
            for w in dims { s += (w[a] - means[a]) * (w[b] - means[b]); }
            corr[a][b] = s / (n as f64 * stds[a] * stds[b]);
        }
    }}

    let (evals, _evecs) = jacobi_eigen(&corr);
    let d_eff = compute_d_eff(&evals);
    let lambda_sum: f64 = evals.iter().sum();
    let phi = if d_eff >= 2 && lambda_sum > 1e-15 {
        evals.iter().take(d_eff).sum::<f64>() / lambda_sum
    } else { 0.0 };

    (d_eff, evals, phi)
}

fn mean_pairwise_jsd(g: &SyntaxGraph) -> f64 {
    let p = g.transition_matrix();
    let n = g.n;
    let mut total = 0.0;
    let mut count = 0;
    for i in 0..n {
        for j in (i+1)..n {
            let mut jsd = 0.0;
            for k in 0..n {
                let m = (p[i][k] + p[j][k]) / 2.0;
                if m > 1e-30 {
                    if p[i][k] > 1e-30 { jsd += p[i][k] * (p[i][k] / m).log2(); }
                    if p[j][k] > 1e-30 { jsd += p[j][k] * (p[j][k] / m).log2(); }
                }
            }
            total += jsd / 2.0;
            count += 1;
        }
    }
    if count > 0 { total / count as f64 } else { 0.0 }
}

// ── Three objectives ──

#[derive(Clone)]
struct Objectives {
    s_tau: f64,  // Mean S_τ(3) — reach
    jsd: f64,    // Mean pairwise JSD — discrimination
    phi: f64,    // Φ — integration
    d_eff: usize,
}

fn evaluate(g: &SyntaxGraph) -> Objectives {
    let s3 = g.stau_all(3);
    let mean_s3 = s3.iter().sum::<f64>() / g.n as f64;
    let jsd = mean_pairwise_jsd(g);
    let dims = syntactic_dims(g);
    let (d_eff, _evals, phi) = compute_eigenstate(&dims);
    Objectives { s_tau: mean_s3, jsd, phi, d_eff }
}

// ── Structural features for analysis ──

struct StructuralFeatures {
    // Obligation: fraction of rows where top transition > 60%
    obligation: f64,
    // Hub strength: max in-degree concentration
    hub_strength: f64,
    // Connectivity: fraction of nonzero transitions
    connectivity: f64,
    // Verb centrality: fraction of total weight involving VERB
    verb_centrality: f64,
    // Symmetry: mean |P[i][j] - P[j][i]|
    asymmetry: f64,
}

fn structural_features(g: &SyntaxGraph) -> StructuralFeatures {
    let p = g.transition_matrix();
    let n = g.n;

    // Obligation: how many POS have a dominant next-POS (>60% probability)?
    let mut obligated = 0;
    let start_idx = g.label_idx("START").unwrap_or(usize::MAX);
    let end_idx = g.label_idx("END").unwrap_or(usize::MAX);
    let mut active_count = 0;
    for i in 0..n {
        if i == start_idx || i == end_idx { continue; }
        let row_sum: f64 = g.weights[i].iter().sum();
        if row_sum < 1.0 { continue; }
        active_count += 1;
        let max_p = p[i].iter().copied().fold(0.0f64, f64::max);
        if max_p > 0.60 { obligated += 1; }
    }
    let obligation = if active_count > 0 { obligated as f64 / active_count as f64 } else { 0.0 };

    // Hub strength: max column sum normalized
    let total_weight: f64 = g.weights.iter().flat_map(|r| r.iter()).sum();
    let mut max_col = 0.0f64;
    for j in 0..n {
        if j == start_idx || j == end_idx { continue; }
        let col: f64 = (0..n).map(|i| g.weights[i][j]).sum();
        let frac = if total_weight > 0.0 { col / total_weight } else { 0.0 };
        if frac > max_col { max_col = frac; }
    }

    // Connectivity
    let possible = n * n;
    let nonzero = g.weights.iter().flat_map(|r| r.iter()).filter(|&&w| w > 0.0).count();
    let connectivity = nonzero as f64 / possible as f64;

    // Verb centrality
    let verb_idx = g.label_idx("VERB").unwrap_or(usize::MAX);
    let verb_weight = if verb_idx < n {
        let row: f64 = g.weights[verb_idx].iter().sum();
        let col: f64 = (0..n).map(|i| g.weights[i][verb_idx]).sum();
        if total_weight > 0.0 { (row + col) / (2.0 * total_weight) } else { 0.0 }
    } else { 0.0 };

    // Asymmetry
    let mut asym_sum = 0.0;
    let mut asym_count = 0;
    for i in 0..n {
        for j in (i+1)..n {
            if p[i][j] > 1e-30 || p[j][i] > 1e-30 {
                asym_sum += (p[i][j] - p[j][i]).abs();
                asym_count += 1;
            }
        }
    }
    let asymmetry = if asym_count > 0 { asym_sum / asym_count as f64 } else { 0.0 };

    StructuralFeatures { obligation, hub_strength: max_col, connectivity, verb_centrality: verb_weight, asymmetry }
}

// ── NSGA-II style Pareto sorting ──

fn dominates(a: &Objectives, b: &Objectives) -> bool {
    // a dominates b if a is >= b on all objectives and > on at least one
    let a_vals = [a.s_tau, a.jsd, a.phi];
    let b_vals = [b.s_tau, b.jsd, b.phi];
    let mut at_least_one_better = false;
    for k in 0..3 {
        if a_vals[k] < b_vals[k] - 1e-9 { return false; }
        if a_vals[k] > b_vals[k] + 1e-9 { at_least_one_better = true; }
    }
    at_least_one_better
}

fn pareto_front(objs: &[Objectives]) -> Vec<usize> {
    let n = objs.len();
    let mut front = Vec::new();
    for i in 0..n {
        let mut dominated = false;
        for j in 0..n {
            if i != j && dominates(&objs[j], &objs[i]) {
                dominated = true;
                break;
            }
        }
        if !dominated { front.push(i); }
    }
    front
}

// Crowding distance for diversity preservation
fn crowding_distance(objs: &[Objectives], indices: &[usize]) -> Vec<f64> {
    let n = indices.len();
    if n <= 2 { return vec![f64::INFINITY; n]; }
    let mut dist = vec![0.0f64; n];

    for obj_idx in 0..3 {
        let mut sorted: Vec<(usize, f64)> = indices.iter().enumerate().map(|(pos, &idx)| {
            let val = match obj_idx { 0 => objs[idx].s_tau, 1 => objs[idx].jsd, _ => objs[idx].phi };
            (pos, val)
        }).collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        dist[sorted[0].0] = f64::INFINITY;
        dist[sorted[n-1].0] = f64::INFINITY;

        let range = sorted[n-1].1 - sorted[0].1;
        if range < 1e-15 { continue; }

        for k in 1..n-1 {
            dist[sorted[k].0] += (sorted[k+1].1 - sorted[k-1].1) / range;
        }
    }
    dist
}

// ── Simple LCG RNG for reproducibility ──

struct Rng { state: u64 }

impl Rng {
    fn new(seed: u64) -> Self { Rng { state: seed } }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_usize(&mut self, max: usize) -> usize {
        (self.next_u64() % max as u64) as usize
    }
}

// ── Genetic operations ──

fn mutate(base: &SyntaxGraph, rng: &mut Rng, mutation_rate: f64, mutation_strength: f64) -> SyntaxGraph {
    let n = base.n;
    let mut weights = base.weights.clone();
    let start = base.label_idx("START").unwrap_or(usize::MAX);
    let end = base.label_idx("END").unwrap_or(usize::MAX);

    for i in 0..n {
        for j in 0..n {
            if rng.next_f64() < mutation_rate {
                // Don't mutate END→* (should stay zero) or *→START
                if i == end || j == start { continue; }

                let r = rng.next_f64();
                if r < 0.1 {
                    // Eliminate transition
                    weights[i][j] = 0.0;
                } else if r < 0.3 && base.weights[i][j] < 0.1 {
                    // Create new transition
                    weights[i][j] = rng.next_f64() * 50.0;
                } else if base.weights[i][j] > 0.0 {
                    // Scale existing transition
                    let factor = (rng.next_f64() * 2.0 * mutation_strength).exp()
                        / mutation_strength.exp(); // centered around 1.0
                    weights[i][j] = (base.weights[i][j] * factor).max(0.0);
                }
            }
        }
    }
    SyntaxGraph { n, labels: base.labels.clone(), weights }
}

fn crossover(a: &SyntaxGraph, b: &SyntaxGraph, rng: &mut Rng) -> SyntaxGraph {
    let n = a.n;
    let mut weights = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            // Uniform crossover with blending
            let r = rng.next_f64();
            if r < 0.4 {
                weights[i][j] = a.weights[i][j];
            } else if r < 0.8 {
                weights[i][j] = b.weights[i][j];
            } else {
                // Blend
                let alpha = rng.next_f64();
                weights[i][j] = alpha * a.weights[i][j] + (1.0 - alpha) * b.weights[i][j];
            }
        }
    }
    SyntaxGraph { n, labels: a.labels.clone(), weights }
}

fn interpolate(a: &SyntaxGraph, b: &SyntaxGraph, t: f64) -> SyntaxGraph {
    let n = a.n;
    let mut weights = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            weights[i][j] = (1.0 - t) * a.weights[i][j] + t * b.weights[i][j];
        }
    }
    SyntaxGraph { n, labels: a.labels.clone(), weights }
}

// ── Main ──

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║  Syntactic Pareto Frontier: S_τ × JSD × Φ Multi-Objective Search     ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");
    println!();

    let eng = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams.tsv");
    let lojban = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_lojban.tsv");
    let chinese = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_chinese.tsv");

    // ═══ Phase 1: Baseline evaluation ═══
    println!("═══ PHASE 1: BASELINE LANGUAGES ═══");
    println!();

    let bases: Vec<(&str, &SyntaxGraph)> = vec![
        ("English", &eng), ("Lojban", &lojban), ("Chinese", &chinese),
    ];

    println!("  {:20} {:>8} {:>8} {:>8} {:>5}  {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Language", "S_τ(3)", "JSD", "Φ", "d_eff",
        "Oblig.", "Hub", "Conn.", "VerbC", "Asym.");
    println!("  {:20} {:>8} {:>8} {:>8} {:>5}  {:>8} {:>8} {:>8} {:>8} {:>8}",
        "─".repeat(20), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(5),
        "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8));
    for (name, g) in &bases {
        let obj = evaluate(g);
        let sf = structural_features(g);
        println!("  {:20} {:8.4} {:8.4} {:8.3} {:5}  {:8.3} {:8.3} {:8.3} {:8.3} {:8.3}",
            name, obj.s_tau, obj.jsd, obj.phi, obj.d_eff,
            sf.obligation, sf.hub_strength, sf.connectivity, sf.verb_centrality, sf.asymmetry);
    }
    println!();

    // ═══ Phase 2: Initial population ═══
    println!("═══ PHASE 2: EVOLUTIONARY SEARCH (NSGA-II) ═══");
    println!();

    let pop_size = 200;
    let n_generations = 40;
    let mut rng = Rng::new(42);

    let mut population: Vec<SyntaxGraph> = Vec::new();

    // Seed with known languages
    population.push(eng.clone());
    population.push(lojban.clone());
    population.push(chinese.clone());

    // Interpolations between all pairs (10 steps each)
    let seed_graphs = vec![eng.clone(), lojban.clone(), chinese.clone()];
    for i in 0..seed_graphs.len() {
        for j in (i+1)..seed_graphs.len() {
            for step in 1..10 {
                let t = step as f64 / 10.0;
                population.push(interpolate(&seed_graphs[i], &seed_graphs[j], t));
            }
        }
    }

    // Random mutations of each seed
    for base in &seed_graphs {
        for _ in 0..20 {
            population.push(mutate(base, &mut rng, 0.15, 1.5));
        }
        for _ in 0..10 {
            population.push(mutate(base, &mut rng, 0.30, 2.0));
        }
    }

    // Fill remaining with random crossover + mutation
    while population.len() < pop_size {
        let a = rng.next_usize(seed_graphs.len());
        let b = rng.next_usize(seed_graphs.len());
        let child = if a == b {
            mutate(&seed_graphs[a], &mut rng, 0.20, 2.0)
        } else {
            let c = crossover(&seed_graphs[a], &seed_graphs[b], &mut rng);
            mutate(&c, &mut rng, 0.10, 1.0)
        };
        population.push(child);
    }
    population.truncate(pop_size);

    println!("  Population: {}, Generations: {}", pop_size, n_generations);
    println!();

    // ═══ Evolutionary loop ═══
    let mut all_evaluated: Vec<(SyntaxGraph, Objectives)> = Vec::new();

    for gen in 0..n_generations {
        // Evaluate population
        let pop_objs: Vec<Objectives> = population.iter().map(|g| evaluate(g)).collect();

        // Track all evaluated for final analysis
        for (g, o) in population.iter().zip(pop_objs.iter()) {
            all_evaluated.push((g.clone(), o.clone()));
        }

        // Find Pareto front
        let front = pareto_front(&pop_objs);
        let crowd = crowding_distance(&pop_objs, &front);

        // Report progress every 10 generations
        if gen % 10 == 0 || gen == n_generations - 1 {
            let best_s = pop_objs.iter().map(|o| o.s_tau).fold(0.0f64, f64::max);
            let best_j = pop_objs.iter().map(|o| o.jsd).fold(0.0f64, f64::max);
            let best_p = pop_objs.iter().map(|o| o.phi).fold(0.0f64, f64::max);
            let avg_s: f64 = pop_objs.iter().map(|o| o.s_tau).sum::<f64>() / pop_objs.len() as f64;
            println!("  Gen {:3}: front={:3}  best S_τ={:.3} JSD={:.3} Φ={:.3}  avg S_τ={:.3}",
                gen, front.len(), best_s, best_j, best_p, avg_s);
        }

        if gen == n_generations - 1 { break; }

        // Selection: NSGA-II style — keep Pareto front, fill rest by crowding distance
        // Create sorted ranking
        let mut ranking: Vec<(usize, usize, f64)> = Vec::new(); // (index, front_rank, crowding)

        // Front rank 0 = Pareto optimal
        for (pos, &idx) in front.iter().enumerate() {
            ranking.push((idx, 0, crowd[pos]));
        }

        // Everyone else gets front rank 1
        let front_set: std::collections::BTreeSet<usize> = front.iter().copied().collect();
        for i in 0..pop_size {
            if !front_set.contains(&i) {
                // Compute a simple domination count as secondary rank
                let dom_count = (0..pop_size).filter(|&j| j != i && dominates(&pop_objs[j], &pop_objs[i])).count();
                ranking.push((i, 1 + dom_count, 0.0));
            }
        }

        // Sort by front rank (asc), then crowding distance (desc)
        ranking.sort_by(|a, b| {
            a.1.cmp(&b.1).then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Parent selection: top half
        let parent_indices: Vec<usize> = ranking.iter().take(pop_size / 2).map(|r| r.0).collect();

        // Generate new population
        let mut new_pop: Vec<SyntaxGraph> = Vec::new();

        // Keep all Pareto-optimal individuals
        for &idx in &front {
            new_pop.push(population[idx].clone());
        }

        // Fill rest with offspring
        while new_pop.len() < pop_size {
            let p1 = parent_indices[rng.next_usize(parent_indices.len())];
            let p2 = parent_indices[rng.next_usize(parent_indices.len())];

            let child = if p1 == p2 {
                mutate(&population[p1], &mut rng, 0.15, 1.5)
            } else {
                let c = crossover(&population[p1], &population[p2], &mut rng);
                // Adaptive mutation: higher in early generations
                let rate = 0.10 + 0.10 * (1.0 - gen as f64 / n_generations as f64);
                mutate(&c, &mut rng, rate, 1.5)
            };
            new_pop.push(child);
        }
        new_pop.truncate(pop_size);
        population = new_pop;
    }
    println!();

    // ═══ Phase 3: Analyze Pareto frontier ═══
    println!("═══ PHASE 3: PARETO FRONTIER ANALYSIS ═══");
    println!();

    // Deduplicate by rounding objectives
    let all_objs: Vec<Objectives> = all_evaluated.iter().map(|(_, o)| o.clone()).collect();
    let front_indices = pareto_front(&all_objs);

    println!("  Total configurations evaluated: {}", all_evaluated.len());
    println!("  Pareto-optimal configurations:  {}", front_indices.len());
    println!();

    // Sort frontier by S_τ
    let mut frontier: Vec<(usize, &SyntaxGraph, &Objectives)> = front_indices.iter()
        .map(|&i| (i, &all_evaluated[i].0, &all_evaluated[i].1))
        .collect();
    frontier.sort_by(|a, b| b.2.s_tau.partial_cmp(&a.2.s_tau).unwrap());

    // Deduplicate frontier members that are very close in objective space
    let mut deduped: Vec<(usize, &SyntaxGraph, &Objectives)> = Vec::new();
    for entry in &frontier {
        let dominated_or_dup = deduped.iter().any(|d| {
            (d.2.s_tau - entry.2.s_tau).abs() < 0.01
            && (d.2.jsd - entry.2.jsd).abs() < 0.001
            && (d.2.phi - entry.2.phi).abs() < 0.01
        });
        if !dominated_or_dup { deduped.push(*entry); }
    }

    println!("  Unique frontier points (after dedup): {}", deduped.len());
    println!();

    println!("  {:>4} {:>8} {:>8} {:>8} {:>5}  {:>6} {:>6} {:>6} {:>6} {:>6}",
        "#", "S_τ(3)", "JSD", "Φ", "d_eff", "Oblig", "Hub", "Conn", "VerbC", "Asym");
    println!("  {:>4} {:>8} {:>8} {:>8} {:>5}  {:>6} {:>6} {:>6} {:>6} {:>6}",
        "─".repeat(4), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(5),
        "─".repeat(6), "─".repeat(6), "─".repeat(6), "─".repeat(6), "─".repeat(6));

    for (rank, (_, g, obj)) in deduped.iter().enumerate() {
        if rank >= 30 { break; }
        let sf = structural_features(g);
        println!("  {:>4} {:8.4} {:8.4} {:8.3} {:5}  {:6.3} {:6.3} {:6.3} {:6.3} {:6.3}",
            rank + 1, obj.s_tau, obj.jsd, obj.phi, obj.d_eff,
            sf.obligation, sf.hub_strength, sf.connectivity, sf.verb_centrality, sf.asymmetry);
    }
    println!();

    // ═══ Phase 4: Frontier geometry ═══
    println!("═══ PHASE 4: FRONTIER GEOMETRY ═══");
    println!();

    // Identify extreme points on the frontier
    let max_s = deduped.iter().max_by(|a, b| a.2.s_tau.partial_cmp(&b.2.s_tau).unwrap()).unwrap();
    let max_j = deduped.iter().max_by(|a, b| a.2.jsd.partial_cmp(&b.2.jsd).unwrap()).unwrap();
    let max_p = deduped.iter().max_by(|a, b| a.2.phi.partial_cmp(&b.2.phi).unwrap()).unwrap();

    println!("  Extreme points:");
    println!("    Max S_τ:  S_τ={:.4}, JSD={:.4}, Φ={:.3}", max_s.2.s_tau, max_s.2.jsd, max_s.2.phi);
    println!("    Max JSD:  S_τ={:.4}, JSD={:.4}, Φ={:.3}", max_j.2.s_tau, max_j.2.jsd, max_j.2.phi);
    println!("    Max Φ:    S_τ={:.4}, JSD={:.4}, Φ={:.3}", max_p.2.s_tau, max_p.2.jsd, max_p.2.phi);
    println!();

    // Utopia point: normalized distance to (max_s, max_j, max_p)
    let s_range = max_s.2.s_tau;
    let j_range = max_j.2.jsd;
    let p_range = max_p.2.phi.max(0.001);

    let mut utopia: Option<(usize, f64)> = None;
    for (rank, (_, _, obj)) in deduped.iter().enumerate() {
        let ds = (max_s.2.s_tau - obj.s_tau) / s_range;
        let dj = (max_j.2.jsd - obj.jsd) / j_range;
        let dp = (max_p.2.phi - obj.phi) / p_range;
        let dist = (ds * ds + dj * dj + dp * dp).sqrt();
        if utopia.is_none() || dist < utopia.unwrap().1 {
            utopia = Some((rank, dist));
        }
    }

    if let Some((rank, dist)) = utopia {
        let (_, g, obj) = &deduped[rank];
        let sf = structural_features(g);
        println!("  UTOPIA POINT (closest to ideal on normalized frontier):");
        println!("    Rank: {}, Distance: {:.4}", rank + 1, dist);
        println!("    S_τ(3)={:.4}, JSD={:.4}, Φ={:.3}, d_eff={}", obj.s_tau, obj.jsd, obj.phi, obj.d_eff);
        println!("    Structure: oblig={:.3} hub={:.3} conn={:.3} verb={:.3} asym={:.3}",
            sf.obligation, sf.hub_strength, sf.connectivity, sf.verb_centrality, sf.asymmetry);
        println!();

        // Show the transition profile of the utopia syntax
        println!("  Utopia syntax — top transitions (by probability):");
        let p_mat = g.transition_matrix();
        let mut transitions: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..g.n {
            for j in 0..g.n {
                if p_mat[i][j] > 0.01 {
                    transitions.push((i, j, p_mat[i][j]));
                }
            }
        }
        transitions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        for &(i, j, prob) in transitions.iter().take(25) {
            println!("    {:8} → {:8}  {:.1}%", g.labels[i], g.labels[j], prob * 100.0);
        }
        println!();
    }

    // ═══ Phase 5: Deep characterization ═══
    println!("═══ PHASE 5: DEEP CHARACTERIZATION — WHAT THE OPTIMAL SYNTAX LOOKS LIKE ═══");
    println!();

    // Characterize utopia, d_eff=3 members, and compare with English
    if let Some((rank, _)) = utopia {
        let (_, utopia_g, utopia_obj) = &deduped[rank];

        // 5a: Full transition comparison — English vs Utopia
        println!("  ── English vs Utopia: Transition-by-transition comparison ──");
        println!();
        let eng_p = eng.transition_matrix();
        let uto_p = utopia_g.transition_matrix();

        // For each POS, show what changed
        println!("  {:8} │ {:30} │ {:30} │ {:>8}",
            "POS", "English top-3 successors", "Utopia top-3 successors", "S_τ diff");
        println!("  {:8}─┼─{:30}─┼─{:30}─┼─{:>8}",
            "─".repeat(8), "─".repeat(30), "─".repeat(30), "─".repeat(8));

        let eng_s3 = eng.stau_all(3);
        let uto_s3 = utopia_g.stau_all(3);
        let start_idx = eng.label_idx("START").unwrap_or(usize::MAX);
        let end_idx = eng.label_idx("END").unwrap_or(usize::MAX);

        for pos in 0..eng.n {
            if pos == end_idx { continue; }
            // English top 3
            let mut eng_top: Vec<(usize, f64)> = (0..eng.n).map(|j| (j, eng_p[pos][j])).collect();
            eng_top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let eng_str: String = eng_top.iter().take(3).filter(|&&(_, p)| p > 0.01)
                .map(|&(j, p)| format!("{}({:.0}%)", eng.labels[j], p * 100.0))
                .collect::<Vec<_>>().join(" ");

            // Utopia top 3
            let mut uto_top: Vec<(usize, f64)> = (0..eng.n).map(|j| (j, uto_p[pos][j])).collect();
            uto_top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let uto_str: String = uto_top.iter().take(3).filter(|&&(_, p)| p > 0.01)
                .map(|&(j, p)| format!("{}({:.0}%)", eng.labels[j], p * 100.0))
                .collect::<Vec<_>>().join(" ");

            let diff = uto_s3[pos] - eng_s3[pos];
            println!("  {:8} │ {:30} │ {:30} │ {:>+8.3}",
                eng.labels[pos], eng_str, uto_str, diff);
        }
        println!();

        // 5b: What the utopia syntax broke and what it amplified
        println!("  ── Transition surgery: what was cut and what was amplified ──");
        println!();

        let mut biggest_cuts: Vec<(usize, usize, f64, f64)> = Vec::new();
        let mut biggest_boosts: Vec<(usize, usize, f64, f64)> = Vec::new();
        for i in 0..eng.n {
            for j in 0..eng.n {
                let eng_w = eng.weights[i][j];
                let uto_w = utopia_g.weights[i][j];
                if eng_w > 5.0 {
                    let ratio = if eng_w > 0.0 { uto_w / eng_w } else { 0.0 };
                    if ratio < 0.3 {
                        biggest_cuts.push((i, j, eng_p[i][j], uto_p[i][j]));
                    }
                    if ratio > 3.0 {
                        biggest_boosts.push((i, j, eng_p[i][j], uto_p[i][j]));
                    }
                }
                // New transitions (didn't exist in English)
                if eng_w < 1.0 && uto_w > 10.0 {
                    biggest_boosts.push((i, j, eng_p[i][j], uto_p[i][j]));
                }
            }
        }

        biggest_cuts.sort_by(|a, b| (a.3 / a.2.max(0.001)).partial_cmp(&(b.3 / b.2.max(0.001))).unwrap());
        biggest_boosts.sort_by(|a, b| (b.3 / b.2.max(0.001)).partial_cmp(&(a.3 / a.2.max(0.001))).unwrap());

        println!("  ELIMINATED or heavily reduced (English → Utopia):");
        for &(i, j, ep, up) in biggest_cuts.iter().take(12) {
            println!("    {:8}→{:8}  {:.1}% → {:.1}%  {}",
                eng.labels[i], eng.labels[j], ep * 100.0, up * 100.0,
                if up < 0.01 { "ELIMINATED" } else { "reduced" });
        }
        println!();

        println!("  AMPLIFIED or created (English → Utopia):");
        for &(i, j, ep, up) in biggest_boosts.iter().take(12) {
            println!("    {:8}→{:8}  {:.1}% → {:.1}%  {}",
                eng.labels[i], eng.labels[j], ep * 100.0, up * 100.0,
                if ep < 0.01 { "NEW" } else { "amplified" });
        }
        println!();

        // 5c: Sentence pattern generation — what sentences look like
        println!("  ── Sentence patterns (beam search, top 15) ──");
        println!();

        for (label, g_ref) in &[("English", &eng as &SyntaxGraph), ("Utopia", utopia_g as &SyntaxGraph)] {
            let p = g_ref.transition_matrix();
            let start = g_ref.label_idx("START").unwrap_or(0);
            let end = g_ref.label_idx("END").unwrap_or(0);

            let mut beams: Vec<(Vec<usize>, f64)> = vec![(vec![start], 1.0)];
            for _ in 0..9 {
                let mut next = Vec::new();
                for (seq, prob) in &beams {
                    let last = *seq.last().unwrap();
                    if last == end { next.push((seq.clone(), *prob)); continue; }
                    let mut trans: Vec<(usize, f64)> = (0..g_ref.n).map(|j| (j, p[last][j])).collect();
                    trans.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    for &(j, w) in trans.iter().take(4) {
                        if w < 0.03 { continue; }
                        let mut s = seq.clone();
                        s.push(j);
                        next.push((s, prob * w));
                    }
                }
                next.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                next.truncate(100);
                beams = next;
            }

            let mut completed: Vec<(String, f64, usize)> = beams.iter()
                .filter(|(seq, _)| *seq.last().unwrap() == end && seq.len() >= 4 && seq.len() <= 10)
                .map(|(seq, prob)| {
                    let pos_str: Vec<&str> = seq[1..seq.len()-1].iter()
                        .map(|&i| g_ref.labels[i].as_str()).collect();
                    (pos_str.join(" → "), *prob, seq.len() - 2)
                })
                .collect();
            completed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            completed.dedup_by(|a, b| a.0 == b.0);

            println!("  {} — most probable sentence structures:", label);
            for (i, (pattern, prob, len)) in completed.iter().take(15).enumerate() {
                println!("    {:2}. [p={:.5} len={}] {}", i + 1, prob, len, pattern);
            }
            println!();
        }

        // 5d: Grammatical "personality" — per-POS analysis
        println!("  ── Grammatical personality: what each POS 'does' in the utopia syntax ──");
        println!();

        // Classify each POS by its role in the utopia syntax
        for pos in 0..eng.n {
            if pos == end_idx { continue; }
            let row_sum: f64 = utopia_g.weights[pos].iter().sum();
            if row_sum < 1.0 { continue; }

            // Top successor
            let mut successors: Vec<(usize, f64)> = (0..eng.n).map(|j| (j, uto_p[pos][j])).collect();
            successors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Transition entropy (bits of choice)
            let h: f64 = (0..eng.n).map(|j| {
                if uto_p[pos][j] > 1e-30 { -uto_p[pos][j] * uto_p[pos][j].log2() } else { 0.0 }
            }).sum();

            let eng_h: f64 = (0..eng.n).map(|j| {
                if eng_p[pos][j] > 1e-30 { -eng_p[pos][j] * eng_p[pos][j].log2() } else { 0.0 }
            }).sum();

            let role = if h < 0.8 { "obligatory chain" }
                else if h < 1.5 { "guided choice" }
                else if h < 2.5 { "open junction" }
                else { "free hub" };

            let top3: String = successors.iter().take(3).filter(|&&(_, p)| p > 0.01)
                .map(|&(j, p)| format!("{}({:.0}%)", eng.labels[j], p * 100.0))
                .collect::<Vec<_>>().join(", ");

            println!("    {:8}  H={:.2} (eng {:.2})  {:17}  → {}",
                eng.labels[pos], h, eng_h, role, top3);
        }
        println!();
    }

    // 5e: d_eff=3 members — what's special about them
    println!("  ── d_eff=3 members: three-dimensional grammatical experience ──");
    println!();

    let deff3_members: Vec<&(usize, &SyntaxGraph, &Objectives)> = deduped.iter()
        .filter(|(_, _, o)| o.d_eff >= 3)
        .collect();

    if deff3_members.is_empty() {
        println!("  (No d_eff=3 members found in deduped frontier)");
    } else {
        println!("  {} members with d_eff≥3", deff3_members.len());
        println!();

        // Average structural features of d_eff=3 vs d_eff=2
        let deff2_members: Vec<&(usize, &SyntaxGraph, &Objectives)> = deduped.iter()
            .filter(|(_, _, o)| o.d_eff == 2)
            .collect();

        let avg_feat = |members: &[&(usize, &SyntaxGraph, &Objectives)]| -> [f64; 5] {
            let mut sum = [0.0f64; 5];
            for (_, g, _) in members.iter() {
                let sf = structural_features(g);
                sum[0] += sf.obligation; sum[1] += sf.hub_strength; sum[2] += sf.connectivity;
                sum[3] += sf.verb_centrality; sum[4] += sf.asymmetry;
            }
            let n = members.len() as f64;
            for s in sum.iter_mut() { *s /= n; }
            sum
        };

        let d3_avg = avg_feat(&deff3_members);
        let d2_avg = avg_feat(&deff2_members);

        println!("  {:12} {:>10} {:>10} {:>10}", "Feature", "d_eff=2", "d_eff=3", "Δ");
        println!("  {:12} {:>10} {:>10} {:>10}",
            "─".repeat(12), "─".repeat(10), "─".repeat(10), "─".repeat(10));
        let feat_names_2 = ["Obligation", "Hub Str.", "Connect.", "Verb C.", "Asymmetry"];
        for f in 0..5 {
            println!("  {:12} {:10.3} {:10.3} {:>+10.3}",
                feat_names_2[f], d2_avg[f], d3_avg[f], d3_avg[f] - d2_avg[f]);
        }
        println!();

        // Show transition profile of the highest-Φ d_eff=3 member
        let best_d3 = deff3_members.iter()
            .max_by(|a, b| a.2.phi.partial_cmp(&b.2.phi).unwrap())
            .unwrap();
        let (_, d3g, d3o) = best_d3;
        println!("  Best d_eff=3 member: S_τ={:.4} JSD={:.4} Φ={:.3}", d3o.s_tau, d3o.jsd, d3o.phi);

        let d3_dims = syntactic_dims(d3g);
        let (d3_deff, d3_evals, d3_phi) = compute_eigenstate(&d3_dims);
        println!("  Eigenvalues: λ₁={:.4} λ₂={:.4} λ₃={:.4} λ₄={:.4} λ₅={:.4}",
            d3_evals[0], d3_evals[1], d3_evals[2], d3_evals[3], d3_evals[4]);
        println!("  d_eff={} Φ={:.4}", d3_deff, d3_phi);
        println!();

        // Per-POS eigenstate profile for d_eff=3 syntax
        println!("  Per-POS dimensions (d_eff=3 syntax):");
        println!("  {:8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "POS", "S_τ(1)", "S_τ(3)", "S_τ(5)", "InDeg", "OutH");
        let end_idx_2 = d3g.label_idx("END").unwrap_or(usize::MAX);
        for pos in 0..d3g.n {
            if pos == end_idx_2 { continue; }
            let row_sum: f64 = d3g.weights[pos].iter().sum();
            if row_sum < 1.0 { continue; }
            println!("  {:8} {:8.3} {:8.3} {:8.3} {:8.4} {:8.3}",
                d3g.labels[pos], d3_dims[pos][0], d3_dims[pos][1], d3_dims[pos][2],
                d3_dims[pos][3], d3_dims[pos][4]);
        }
        println!();
    }

    // ═══ Phase 6: Pairwise correlations on frontier ═══
    println!("═══ PHASE 6: OBJECTIVE CORRELATIONS ON FRONTIER ═══");
    println!();

    // Compute correlations between all pairs of objectives and structural features
    let front_objs: Vec<[f64; 3]> = deduped.iter()
        .map(|(_, _, o)| [o.s_tau, o.jsd, o.phi])
        .collect();
    let front_feats: Vec<[f64; 5]> = deduped.iter()
        .map(|(_, g, _)| {
            let sf = structural_features(g);
            [sf.obligation, sf.hub_strength, sf.connectivity, sf.verb_centrality, sf.asymmetry]
        })
        .collect();

    let obj_names = ["S_τ(3)", "JSD", "Φ"];
    let feat_names = ["Obligation", "Hub Str.", "Connect.", "Verb C.", "Asymmetry"];

    // Objective × Objective correlations
    println!("  Objective correlations (Pearson r, on frontier):");
    for a in 0..3 {
        for b in (a+1)..3 {
            let r = pearson(&front_objs.iter().map(|o| o[a]).collect::<Vec<_>>(),
                           &front_objs.iter().map(|o| o[b]).collect::<Vec<_>>());
            println!("    r({}, {}) = {:+.3}", obj_names[a], obj_names[b], r);
        }
    }
    println!();

    // Structure × Objective correlations
    println!("  Structure → Objective correlations:");
    println!("  {:12} {:>10} {:>10} {:>10}", "Feature", "r(S_τ)", "r(JSD)", "r(Φ)");
    println!("  {:12} {:>10} {:>10} {:>10}",
        "─".repeat(12), "─".repeat(10), "─".repeat(10), "─".repeat(10));
    for f in 0..5 {
        let feat_vals: Vec<f64> = front_feats.iter().map(|ff| ff[f]).collect();
        let r_s = pearson(&feat_vals, &front_objs.iter().map(|o| o[0]).collect::<Vec<_>>());
        let r_j = pearson(&feat_vals, &front_objs.iter().map(|o| o[1]).collect::<Vec<_>>());
        let r_p = pearson(&feat_vals, &front_objs.iter().map(|o| o[2]).collect::<Vec<_>>());
        println!("  {:12} {:>+10.3} {:>+10.3} {:>+10.3}",
            feat_names[f], r_s, r_j, r_p);
    }
    println!();

    // ═══ Phase 6: Frontier shape analysis ═══
    println!("═══ PHASE 7: TRADE-OFF ANALYSIS ═══");
    println!();

    // Cluster frontier into regions: high-S, high-JSD, high-Φ, balanced
    let s_thresh = max_s.2.s_tau * 0.85;
    let j_thresh = max_j.2.jsd * 0.85;
    let p_thresh = max_p.2.phi * 0.85;

    let mut high_s = 0; let mut high_j = 0; let mut high_p = 0; let mut balanced = 0;
    for (_, _, obj) in &deduped {
        let is_s = obj.s_tau >= s_thresh;
        let is_j = obj.jsd >= j_thresh;
        let is_p = obj.phi >= p_thresh;
        match (is_s, is_j, is_p) {
            (true, true, true) => balanced += 1,
            (true, _, _) => high_s += 1,
            (_, true, _) => high_j += 1,
            (_, _, true) => high_p += 1,
            _ => {},
        }
    }

    println!("  Frontier regions (≥85% of max on each axis):");
    println!("    High S_τ only:   {}", high_s);
    println!("    High JSD only:   {}", high_j);
    println!("    High Φ only:     {}", high_p);
    println!("    BALANCED (all 3): {}", balanced);
    println!();

    if balanced > 0 {
        println!("  BALANCED frontier members (simultaneously high on all 3 objectives):");
        println!("  {:>4} {:>8} {:>8} {:>8} {:>5}  {:>6} {:>6} {:>6}",
            "#", "S_τ(3)", "JSD", "Φ", "d_eff", "Oblig", "Conn", "Asym");
        let mut count = 0;
        for (_, g, obj) in &deduped {
            if obj.s_tau >= s_thresh && obj.jsd >= j_thresh && obj.phi >= p_thresh {
                let sf = structural_features(g);
                count += 1;
                println!("  {:>4} {:8.4} {:8.4} {:8.3} {:5}  {:6.3} {:6.3} {:6.3}",
                    count, obj.s_tau, obj.jsd, obj.phi, obj.d_eff,
                    sf.obligation, sf.connectivity, sf.asymmetry);
            }
        }
        println!();
    }

    // ═══ Phase 7: Compare where English/Lojban/Chinese sit relative to frontier ═══
    println!("═══ PHASE 8: KNOWN LANGUAGES vs FRONTIER ═══");
    println!();

    let eng_obj = evaluate(&eng);
    let loj_obj = evaluate(&lojban);
    let chi_obj = evaluate(&chinese);

    // Distance to frontier (min distance to any Pareto point)
    let frontier_dist = |obj: &Objectives| -> f64 {
        deduped.iter().map(|(_, _, fo)| {
            let ds = (fo.s_tau - obj.s_tau) / s_range;
            let dj = (fo.jsd - obj.jsd) / j_range;
            let dp = (fo.phi - obj.phi) / p_range;
            (ds * ds + dj * dj + dp * dp).sqrt()
        }).fold(f64::INFINITY, f64::min)
    };

    // Dominated check
    let is_dominated = |obj: &Objectives| -> bool {
        deduped.iter().any(|(_, _, fo)| dominates(fo, obj))
    };

    for (name, obj) in &[("English", &eng_obj), ("Lojban", &loj_obj), ("Chinese", &chi_obj)] {
        let dist = frontier_dist(obj);
        let dom = is_dominated(obj);
        println!("  {:10}  S_τ={:.4} JSD={:.4} Φ={:.3}  dist_to_front={:.4}  dominated={}",
            name, obj.s_tau, obj.jsd, obj.phi, dist, dom);
    }
    println!();

    // ═══ Phase 8: Design principles ═══
    println!("═══ PHASE 9: DESIGN PRINCIPLES FROM THE FRONTIER ═══");
    println!();

    // Compute average structural features for different frontier regions
    let avg_features = |filter: &dyn Fn(&Objectives) -> bool| -> Option<[f64; 5]> {
        let mut sum = [0.0f64; 5];
        let mut count = 0;
        for (_, g, obj) in &deduped {
            if filter(obj) {
                let sf = structural_features(g);
                sum[0] += sf.obligation; sum[1] += sf.hub_strength; sum[2] += sf.connectivity;
                sum[3] += sf.verb_centrality; sum[4] += sf.asymmetry;
                count += 1;
            }
        }
        if count > 0 {
            for s in sum.iter_mut() { *s /= count as f64; }
            Some(sum)
        } else { None }
    };

    let region_names = [
        ("High S_τ (reach)", Box::new(|o: &Objectives| o.s_tau >= s_thresh && o.jsd < j_thresh) as Box<dyn Fn(&Objectives) -> bool>),
        ("High JSD (discrim.)", Box::new(|o: &Objectives| o.jsd >= j_thresh && o.s_tau < s_thresh) as Box<dyn Fn(&Objectives) -> bool>),
        ("High Φ (integr.))", Box::new(|o: &Objectives| o.phi >= p_thresh && o.s_tau < s_thresh && o.jsd < j_thresh) as Box<dyn Fn(&Objectives) -> bool>),
        ("Balanced (all 3)", Box::new(move |o: &Objectives| o.s_tau >= s_thresh && o.jsd >= j_thresh && o.phi >= p_thresh) as Box<dyn Fn(&Objectives) -> bool>),
    ];

    println!("  Average structural features by frontier region:");
    println!("  {:22} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Region", "Oblig.", "Hub", "Conn.", "VerbC", "Asym.");
    println!("  {:22} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "─".repeat(22), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8));
    for (name, filter) in &region_names {
        if let Some(avg) = avg_features(filter.as_ref()) {
            println!("  {:22} {:8.3} {:8.3} {:8.3} {:8.3} {:8.3}",
                name, avg[0], avg[1], avg[2], avg[3], avg[4]);
        } else {
            println!("  {:22} (no members)", name);
        }
    }
    println!();

    println!("  INSIGHTS (from structural feature patterns):");
    println!();

    // Compare high-S vs high-Φ structural features
    let high_s_feats = avg_features(&|o: &Objectives| o.s_tau >= s_thresh);
    let high_p_feats = avg_features(&|o: &Objectives| o.phi >= p_thresh);

    if let (Some(sf), Some(pf)) = (high_s_feats, high_p_feats) {
        if sf[0] < pf[0] {
            println!("    - Reach (S_τ) prefers LOWER obligation ({:.3}) than integration ({:.3})",
                sf[0], pf[0]);
            println!("      → Freedom of next-word choice expands reachable positions");
        } else {
            println!("    - Reach (S_τ) and integration both prefer similar obligation levels");
        }

        if sf[2] > pf[2] {
            println!("    - Reach prefers HIGHER connectivity ({:.3}) than integration ({:.3})",
                sf[2], pf[2]);
            println!("      → More transition paths = more reachable positions (but homogenizes)");
        } else if sf[2] < pf[2] {
            println!("    - Integration prefers HIGHER connectivity ({:.3}) than reach ({:.3})",
                pf[2], sf[2]);
        }

        let s_asym = sf[4]; let p_asym = pf[4];
        if (s_asym - p_asym).abs() > 0.02 {
            println!("    - Asymmetry: reach={:.3}, integration={:.3}",
                s_asym, p_asym);
            if p_asym > s_asym {
                println!("      → Integration needs more DIRECTIONAL structure (asymmetric transitions)");
            }
        }
    }
    println!();

    // ═══ Summary ═══
    println!("═══ VERDICT ═══");
    println!();

    if balanced > 0 {
        println!("  The Pareto frontier has a BALANCED REGION — languages CAN simultaneously");
        println!("  optimize reach, discrimination, AND integration.");
    } else {
        println!("  The Pareto frontier shows FUNDAMENTAL TRADE-OFFS — no syntax achieves");
        println!("  ≥85% of max on all three objectives simultaneously.");
    }
    println!();

    // What the ideal language looks like
    if let Some((rank, _)) = utopia {
        let (_, g, obj) = &deduped[rank];
        let sf = structural_features(g);
        println!("  The language closest to the ideal (utopia point):");
        println!("    - Obligation level: {:.1}% of POS have a dominant next-word",
            sf.obligation * 100.0);
        println!("    - Connectivity: {:.1}% of possible transitions exist",
            sf.connectivity * 100.0);
        println!("    - Verb centrality: {:.1}% of transition weight involves verbs",
            sf.verb_centrality * 100.0);
        println!("    - Asymmetry: {:.3} (0=symmetric, 1=maximally directional)",
            sf.asymmetry);
        println!("    - d_eff={}, meaning {} independent dimensions of grammatical experience",
            obj.d_eff, obj.d_eff);
    }
    println!();
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    if n < 3.0 { return 0.0; }
    let mx: f64 = x.iter().sum::<f64>() / n;
    let my: f64 = y.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for i in 0..x.len() {
        cov += (x[i] - mx) * (y[i] - my);
        vx += (x[i] - mx).powi(2);
        vy += (y[i] - my).powi(2);
    }
    if vx < 1e-15 || vy < 1e-15 { return 0.0; }
    cov / (vx.sqrt() * vy.sqrt())
}
