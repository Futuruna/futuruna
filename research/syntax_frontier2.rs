//! Syntactic Frontier Deep Dive: Phase transitions, interpolation paths,
//! programming languages, robustness, and equality.
//!
//! A: Interpolation path from English to utopia — when does each metric jump?
//! B: Phase transition mapping — obligation level vs d_eff
//! C: Programming language syntax evaluation
//! D: Robustness analysis — how fragile are frontier members?
//! E: Equality frontier — mean vs min S_τ
//!
//! Run: cargo run --release --bin syntax-frontier2

use std::fs;

const N_DIM: usize = 5;

// ── Core structures (shared with syntax_pareto) ──

#[derive(Clone)]
struct SyntaxGraph {
    n: usize,
    labels: Vec<String>,
    weights: Vec<Vec<f64>>,
}

impl SyntaxGraph {
    fn from_tsv(pos_path: &str, bigram_path: &str) -> Self {
        let pos_str = fs::read_to_string(pos_path).expect("Need pos_tags.tsv");
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

    fn from_labels_weights(labels: Vec<String>, weights: Vec<Vec<f64>>) -> Self {
        let n = labels.len();
        SyntaxGraph { n, labels, weights }
    }

    fn transition_matrix(&self) -> Vec<Vec<f64>> {
        let n = self.n;
        let mut p = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            let row_sum: f64 = self.weights[i].iter().sum();
            if row_sum > 0.0 {
                for j in 0..n { p[i][j] = self.weights[i][j] / row_sum; }
            } else { p[i][i] = 1.0; }
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

fn interpolate(a: &SyntaxGraph, b: &SyntaxGraph, t: f64) -> SyntaxGraph {
    let n = a.n;
    let mut weights = vec![vec![0.0f64; n]; n];
    for i in 0..n { for j in 0..n {
        weights[i][j] = (1.0 - t) * a.weights[i][j] + t * b.weights[i][j];
    }}
    SyntaxGraph { n, labels: a.labels.clone(), weights }
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
        for j in 0..n { if p[i][j] > 1e-30 { h -= p[i][j] * p[i][j].log2(); } }
        out_entropy[i] = h;
    }
    let mut result = vec![[0.0f64; N_DIM]; n];
    for i in 0..n { result[i] = [s1[i], s3[i], s5[i], in_deg[i], out_entropy[i]]; }
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
    let (evals, _) = jacobi_eigen(&corr);
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
    for i in 0..n { for j in (i+1)..n {
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
    }}
    if count > 0 { total / count as f64 } else { 0.0 }
}

#[derive(Clone)]
struct Metrics {
    mean_s3: f64,
    min_s3: f64,
    max_s3: f64,
    jsd: f64,
    phi: f64,
    d_eff: usize,
    obligation: f64,  // fraction of active POS with max_p > 0.60
    tunnel_ratio: f64, // POS with H<1.5 / POS with H>2.5
}

fn full_metrics(g: &SyntaxGraph) -> Metrics {
    let s3 = g.stau_all(3);
    let p = g.transition_matrix();
    let n = g.n;
    let dims = syntactic_dims(g);
    let (d_eff, _, phi) = compute_eigenstate(&dims);
    let jsd = mean_pairwise_jsd(g);

    let start = g.label_idx("START").unwrap_or(usize::MAX);
    let end = g.label_idx("END").unwrap_or(usize::MAX);

    // Filter to active POS (not START/END, has outgoing weight)
    let mut active_s3 = Vec::new();
    let mut obligated = 0;
    let mut tunnels = 0; // H < 1.5
    let mut hubs = 0;    // H > 2.5
    let mut active_count = 0;

    for i in 0..n {
        if i == start || i == end { continue; }
        let row_sum: f64 = g.weights[i].iter().sum();
        if row_sum < 1.0 { continue; }
        active_count += 1;
        active_s3.push(s3[i]);

        let max_p = p[i].iter().copied().fold(0.0f64, f64::max);
        if max_p > 0.60 { obligated += 1; }

        let h: f64 = (0..n).map(|j| {
            if p[i][j] > 1e-30 { -p[i][j] * p[i][j].log2() } else { 0.0 }
        }).sum();
        if h < 1.5 { tunnels += 1; }
        if h > 2.5 { hubs += 1; }
    }

    let mean_s3 = if active_s3.is_empty() { 0.0 } else { active_s3.iter().sum::<f64>() / active_s3.len() as f64 };
    let min_s3 = active_s3.iter().copied().fold(f64::INFINITY, f64::min);
    let max_s3 = active_s3.iter().copied().fold(0.0f64, f64::max);
    let obligation = if active_count > 0 { obligated as f64 / active_count as f64 } else { 0.0 };
    let tunnel_ratio = if hubs > 0 { tunnels as f64 / hubs as f64 } else { 0.0 };

    Metrics { mean_s3, min_s3, max_s3, jsd, phi, d_eff, obligation, tunnel_ratio }
}

// ── Simple RNG ──

struct Rng { state: u64 }
impl Rng {
    fn new(seed: u64) -> Self { Rng { state: seed } }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }
    fn next_f64(&mut self) -> f64 { (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 }
}

// ── Programming language syntax graphs ──

fn make_prog_lang(name: &str) -> SyntaxGraph {
    // Token categories for programming languages, mapped to a common set.
    // We use a simplified model: each "POS" is a token category, weights are
    // approximate bigram frequencies from typical code in that language.
    let labels: Vec<String> = vec![
        "START", "KW",    "IDENT", "OP",   "DELIM", "LIT",
        "TYPE",  "PAREN", "BRACE", "SEMI",  "DOT",   "ARROW",
        "COMMA", "COLON", "END",
    ].iter().map(|s| s.to_string()).collect();
    let n = labels.len();
    let mut w = vec![vec![0.0f64; n]; n];

    let idx = |name: &str| -> usize { labels.iter().position(|l| l == name).unwrap() };

    match name {
        "Python" => {
            // Indentation-based, no braces/semi, heavy keywords
            w[idx("START")][idx("KW")] = 30.0;    // def, class, if, for, import
            w[idx("START")][idx("IDENT")] = 25.0;
            w[idx("START")][idx("LIT")] = 5.0;
            w[idx("KW")][idx("IDENT")] = 40.0;    // def foo, class Bar, if x
            w[idx("KW")][idx("KW")] = 10.0;       // from import, if not
            w[idx("KW")][idx("PAREN")] = 5.0;
            w[idx("KW")][idx("LIT")] = 8.0;
            w[idx("IDENT")][idx("OP")] = 25.0;    // x =, x +, x ==
            w[idx("IDENT")][idx("PAREN")] = 20.0; // foo(
            w[idx("IDENT")][idx("DOT")] = 15.0;   // obj.
            w[idx("IDENT")][idx("COLON")] = 8.0;  // x: type annotation
            w[idx("IDENT")][idx("COMMA")] = 10.0;
            w[idx("IDENT")][idx("END")] = 5.0;
            w[idx("OP")][idx("IDENT")] = 30.0;
            w[idx("OP")][idx("LIT")] = 20.0;
            w[idx("OP")][idx("PAREN")] = 8.0;
            w[idx("OP")][idx("KW")] = 5.0;        // = None, = True
            w[idx("DELIM")][idx("IDENT")] = 15.0;
            w[idx("DELIM")][idx("KW")] = 10.0;
            w[idx("LIT")][idx("OP")] = 12.0;
            w[idx("LIT")][idx("COMMA")] = 10.0;
            w[idx("LIT")][idx("PAREN")] = 5.0;
            w[idx("LIT")][idx("END")] = 8.0;
            w[idx("PAREN")][idx("IDENT")] = 20.0;
            w[idx("PAREN")][idx("LIT")] = 10.0;
            w[idx("PAREN")][idx("PAREN")] = 8.0;
            w[idx("PAREN")][idx("KW")] = 5.0;
            w[idx("PAREN")][idx("END")] = 5.0;
            w[idx("DOT")][idx("IDENT")] = 40.0;   // obj.method
            w[idx("COMMA")][idx("IDENT")] = 20.0;
            w[idx("COMMA")][idx("LIT")] = 10.0;
            w[idx("COMMA")][idx("KW")] = 5.0;
            w[idx("COLON")][idx("IDENT")] = 15.0; // annotation
            w[idx("COLON")][idx("TYPE")] = 10.0;
            w[idx("COLON")][idx("END")] = 8.0;    // block start
            w[idx("TYPE")][idx("OP")] = 8.0;
            w[idx("TYPE")][idx("COMMA")] = 5.0;
            w[idx("TYPE")][idx("PAREN")] = 5.0;
        }
        "Rust" => {
            // Braces, semicolons, heavy type system, arrows
            w[idx("START")][idx("KW")] = 25.0;
            w[idx("START")][idx("IDENT")] = 20.0;
            w[idx("START")][idx("TYPE")] = 10.0;
            w[idx("KW")][idx("IDENT")] = 30.0;    // fn foo, let x, struct Bar
            w[idx("KW")][idx("TYPE")] = 15.0;     // impl Trait
            w[idx("KW")][idx("KW")] = 8.0;        // pub fn, unsafe impl
            w[idx("KW")][idx("BRACE")] = 5.0;
            w[idx("IDENT")][idx("COLON")] = 15.0; // x: Type
            w[idx("IDENT")][idx("OP")] = 20.0;
            w[idx("IDENT")][idx("PAREN")] = 15.0;
            w[idx("IDENT")][idx("DOT")] = 10.0;
            w[idx("IDENT")][idx("SEMI")] = 8.0;
            w[idx("IDENT")][idx("COMMA")] = 8.0;
            w[idx("IDENT")][idx("ARROW")] = 5.0;
            w[idx("IDENT")][idx("BRACE")] = 5.0;
            w[idx("OP")][idx("IDENT")] = 25.0;
            w[idx("OP")][idx("LIT")] = 15.0;
            w[idx("OP")][idx("PAREN")] = 5.0;
            w[idx("OP")][idx("TYPE")] = 5.0;
            w[idx("TYPE")][idx("BRACE")] = 10.0;
            w[idx("TYPE")][idx("COMMA")] = 8.0;
            w[idx("TYPE")][idx("PAREN")] = 8.0;
            w[idx("TYPE")][idx("OP")] = 5.0;
            w[idx("TYPE")][idx("ARROW")] = 5.0;
            w[idx("TYPE")][idx("SEMI")] = 3.0;
            w[idx("PAREN")][idx("IDENT")] = 15.0;
            w[idx("PAREN")][idx("TYPE")] = 10.0;
            w[idx("PAREN")][idx("LIT")] = 8.0;
            w[idx("PAREN")][idx("PAREN")] = 5.0;
            w[idx("PAREN")][idx("ARROW")] = 8.0;  // ) ->
            w[idx("BRACE")][idx("KW")] = 15.0;
            w[idx("BRACE")][idx("IDENT")] = 15.0;
            w[idx("BRACE")][idx("BRACE")] = 5.0;
            w[idx("BRACE")][idx("END")] = 5.0;
            w[idx("SEMI")][idx("KW")] = 15.0;
            w[idx("SEMI")][idx("IDENT")] = 15.0;
            w[idx("SEMI")][idx("BRACE")] = 5.0;
            w[idx("SEMI")][idx("END")] = 3.0;
            w[idx("ARROW")][idx("TYPE")] = 25.0;  // -> Type
            w[idx("ARROW")][idx("IDENT")] = 5.0;
            w[idx("COLON")][idx("TYPE")] = 25.0;
            w[idx("COLON")][idx("COLON")] = 8.0;  // :: path
            w[idx("COLON")][idx("IDENT")] = 5.0;
            w[idx("DOT")][idx("IDENT")] = 30.0;
            w[idx("COMMA")][idx("IDENT")] = 15.0;
            w[idx("COMMA")][idx("TYPE")] = 10.0;
            w[idx("COMMA")][idx("LIT")] = 5.0;
            w[idx("LIT")][idx("OP")] = 10.0;
            w[idx("LIT")][idx("COMMA")] = 8.0;
            w[idx("LIT")][idx("SEMI")] = 8.0;
            w[idx("LIT")][idx("PAREN")] = 3.0;
        }
        "Lisp" => {
            // Parens everywhere, minimal syntax, prefix notation
            w[idx("START")][idx("PAREN")] = 40.0;
            w[idx("START")][idx("LIT")] = 5.0;
            w[idx("PAREN")][idx("KW")] = 25.0;    // (defun, (let, (if
            w[idx("PAREN")][idx("IDENT")] = 20.0;  // (foo
            w[idx("PAREN")][idx("PAREN")] = 15.0;  // ((
            w[idx("PAREN")][idx("LIT")] = 5.0;
            w[idx("KW")][idx("IDENT")] = 30.0;
            w[idx("KW")][idx("PAREN")] = 15.0;
            w[idx("KW")][idx("LIT")] = 5.0;
            w[idx("IDENT")][idx("IDENT")] = 20.0;  // args
            w[idx("IDENT")][idx("PAREN")] = 25.0;  // nested
            w[idx("IDENT")][idx("LIT")] = 10.0;
            w[idx("IDENT")][idx("END")] = 5.0;
            w[idx("LIT")][idx("LIT")] = 10.0;
            w[idx("LIT")][idx("PAREN")] = 15.0;
            w[idx("LIT")][idx("IDENT")] = 10.0;
            w[idx("LIT")][idx("END")] = 5.0;
        }
        "Haskell" => {
            // Type-heavy, pattern matching, arrows, minimal punctuation
            w[idx("START")][idx("IDENT")] = 25.0;
            w[idx("START")][idx("KW")] = 20.0;
            w[idx("START")][idx("TYPE")] = 10.0;
            w[idx("KW")][idx("IDENT")] = 20.0;    // where x, let x, case x
            w[idx("KW")][idx("TYPE")] = 15.0;     // data Type, class Constraint
            w[idx("KW")][idx("KW")] = 5.0;
            w[idx("KW")][idx("PAREN")] = 5.0;
            w[idx("IDENT")][idx("IDENT")] = 15.0;  // f x (function application)
            w[idx("IDENT")][idx("OP")] = 15.0;
            w[idx("IDENT")][idx("COLON")] = 10.0;  // :: type sig
            w[idx("IDENT")][idx("ARROW")] = 10.0;  // -> in types/cases
            w[idx("IDENT")][idx("PAREN")] = 8.0;
            w[idx("IDENT")][idx("END")] = 5.0;
            w[idx("OP")][idx("IDENT")] = 20.0;
            w[idx("OP")][idx("LIT")] = 10.0;
            w[idx("OP")][idx("PAREN")] = 8.0;
            w[idx("OP")][idx("TYPE")] = 5.0;
            w[idx("TYPE")][idx("ARROW")] = 20.0;   // Type -> Type
            w[idx("TYPE")][idx("TYPE")] = 10.0;    // Maybe Int
            w[idx("TYPE")][idx("IDENT")] = 5.0;
            w[idx("TYPE")][idx("KW")] = 5.0;       // where clause
            w[idx("TYPE")][idx("PAREN")] = 5.0;
            w[idx("ARROW")][idx("TYPE")] = 20.0;
            w[idx("ARROW")][idx("IDENT")] = 15.0;  // -> expression
            w[idx("ARROW")][idx("PAREN")] = 5.0;
            w[idx("PAREN")][idx("IDENT")] = 15.0;
            w[idx("PAREN")][idx("TYPE")] = 10.0;
            w[idx("PAREN")][idx("OP")] = 5.0;
            w[idx("PAREN")][idx("PAREN")] = 5.0;
            w[idx("COLON")][idx("TYPE")] = 25.0;   // :: Type
            w[idx("COLON")][idx("COLON")] = 5.0;   // :: (double colon)
            w[idx("LIT")][idx("OP")] = 10.0;
            w[idx("LIT")][idx("IDENT")] = 5.0;
            w[idx("LIT")][idx("END")] = 5.0;
            w[idx("DOT")][idx("IDENT")] = 20.0;    // composition
            w[idx("COMMA")][idx("IDENT")] = 10.0;
            w[idx("COMMA")][idx("TYPE")] = 8.0;
        }
        "SQL" => {
            // Very keyword-heavy, rigid structure
            w[idx("START")][idx("KW")] = 45.0;    // SELECT, INSERT, UPDATE, CREATE
            w[idx("KW")][idx("IDENT")] = 25.0;    // SELECT col, FROM table
            w[idx("KW")][idx("KW")] = 20.0;       // ORDER BY, GROUP BY, NOT NULL
            w[idx("KW")][idx("LIT")] = 8.0;       // LIMIT 10
            w[idx("KW")][idx("OP")] = 5.0;        // IS NULL, = ANY
            w[idx("KW")][idx("PAREN")] = 5.0;
            w[idx("IDENT")][idx("KW")] = 20.0;    // col FROM, table WHERE
            w[idx("IDENT")][idx("COMMA")] = 15.0; // col1, col2
            w[idx("IDENT")][idx("OP")] = 15.0;    // col = val
            w[idx("IDENT")][idx("DOT")] = 8.0;    // table.col
            w[idx("IDENT")][idx("END")] = 5.0;
            w[idx("OP")][idx("LIT")] = 20.0;      // = 'value'
            w[idx("OP")][idx("IDENT")] = 15.0;
            w[idx("OP")][idx("PAREN")] = 5.0;
            w[idx("LIT")][idx("KW")] = 12.0;      // 'val' AND
            w[idx("LIT")][idx("COMMA")] = 8.0;
            w[idx("LIT")][idx("END")] = 8.0;
            w[idx("PAREN")][idx("KW")] = 10.0;    // (SELECT
            w[idx("PAREN")][idx("IDENT")] = 10.0;
            w[idx("PAREN")][idx("LIT")] = 5.0;
            w[idx("DOT")][idx("IDENT")] = 30.0;
            w[idx("COMMA")][idx("IDENT")] = 20.0;
            w[idx("COMMA")][idx("LIT")] = 5.0;
        }
        _ => {}
    }
    SyntaxGraph::from_labels_weights(labels, w)
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║  Syntactic Frontier Deep Dive: Transitions, Languages, Robustness    ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");
    println!();

    let eng = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams.tsv");
    let lojban = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_lojban.tsv");
    let chinese = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_chinese.tsv");

    // We need the utopia syntax. Reconstruct it from the Pareto search.
    // For reproducibility, use the SAME seed and evolutionary process as syntax_pareto.
    // Actually, let's use a simpler approach: load English and create the utopia
    // by applying the KNOWN best perturbations from the Phase 4 output of syntax_stau.
    // But that won't match exactly. Instead, let's just re-run a small evolutionary search.
    println!("  Reconstructing Pareto utopia point (quick evolutionary search)...");
    let utopia = find_utopia(&eng, &lojban, &chinese);
    let eng_m = full_metrics(&eng);
    let uto_m = full_metrics(&utopia);
    println!("  English:  S_τ={:.4} JSD={:.4} Φ={:.3} d_eff={}", eng_m.mean_s3, eng_m.jsd, eng_m.phi, eng_m.d_eff);
    println!("  Utopia:   S_τ={:.4} JSD={:.4} Φ={:.3} d_eff={}", uto_m.mean_s3, uto_m.jsd, uto_m.phi, uto_m.d_eff);
    println!();

    // ═══ A: Interpolation path from English to Utopia ═══
    println!("═══ A: INTERPOLATION PATH — ENGLISH → UTOPIA ═══");
    println!();
    println!("  {:>5} {:>8} {:>8} {:>8} {:>5} {:>8} {:>8} {:>8}",
        "t", "S_τ(3)", "JSD", "Φ", "d_eff", "Oblig.", "TunRat", "min S_τ");
    println!("  {:>5} {:>8} {:>8} {:>8} {:>5} {:>8} {:>8} {:>8}",
        "─".repeat(5), "─".repeat(8), "─".repeat(8), "─".repeat(8),
        "─".repeat(5), "─".repeat(8), "─".repeat(8), "─".repeat(8));

    for step in 0..=40 {
        let t = step as f64 / 40.0;
        let g = interpolate(&eng, &utopia, t);
        let m = full_metrics(&g);
        if step % 2 == 0 || m.d_eff != full_metrics(&interpolate(&eng, &utopia, (step - 1) as f64 / 40.0)).d_eff {
            println!("  {:5.2} {:8.4} {:8.4} {:8.3} {:5} {:8.3} {:8.2} {:8.3}",
                t, m.mean_s3, m.jsd, m.phi, m.d_eff, m.obligation, m.tunnel_ratio, m.min_s3);
        }
    }
    println!();

    // Identify the first t where each metric exceeds English by 10%
    println!("  Critical thresholds (t where metric exceeds English + 10%):");
    let eng_s = eng_m.mean_s3 * 1.10;
    let eng_j = eng_m.jsd * 1.10;
    let eng_p = eng_m.phi * 1.10;
    let mut found_s = false; let mut found_j = false; let mut found_p = false;
    for step in 0..=100 {
        let t = step as f64 / 100.0;
        let m = full_metrics(&interpolate(&eng, &utopia, t));
        if !found_s && m.mean_s3 >= eng_s { println!("    S_τ +10%: t={:.2}", t); found_s = true; }
        if !found_j && m.jsd >= eng_j { println!("    JSD +10%: t={:.2}", t); found_j = true; }
        if !found_p && m.phi >= eng_p { println!("    Φ   +10%: t={:.2}", t); found_p = true; }
    }
    println!();

    // ═══ B: Phase transition — obligation vs d_eff ═══
    println!("═══ B: PHASE TRANSITION — OBLIGATION vs d_eff ═══");
    println!();
    println!("  Sweeping obligation level by progressively constraining English...");
    println!();

    // Strategy: for each target obligation level, randomly pick POS and make their
    // top transition more dominant, increasing obligation.
    let mut rng = Rng::new(123);
    let p_eng = eng.transition_matrix();
    let n = eng.n;

    println!("  {:>6} {:>5} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Oblig%", "d_eff", "Φ", "S_τ(3)", "JSD", "TunRat", "λ₃/λ₁");
    println!("  {:>6} {:>5} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "─".repeat(6), "─".repeat(5), "─".repeat(8), "─".repeat(8),
        "─".repeat(8), "─".repeat(8), "─".repeat(8));

    // At each strength level, boost top transitions to create more obligation
    for strength in 0..=20 {
        let factor = 1.0 + strength as f64 * 0.5; // 1.0 to 11.0
        let mut w = eng.weights.clone();
        for i in 0..n {
            let row_sum: f64 = eng.weights[i].iter().sum();
            if row_sum < 1.0 { continue; }
            // Find top transition
            let top_j = (0..n).max_by(|&a, &b| p_eng[i][a].partial_cmp(&p_eng[i][b]).unwrap()).unwrap();
            w[i][top_j] = eng.weights[i][top_j] * factor;
        }
        let g = SyntaxGraph { n, labels: eng.labels.clone(), weights: w };
        let m = full_metrics(&g);
        let dims = syntactic_dims(&g);
        let (_, evals, _) = compute_eigenstate(&dims);
        let ratio = if evals[0] > 1e-10 { evals[2] / evals[0] } else { 0.0 };
        println!("  {:5.1}% {:5} {:8.3} {:8.4} {:8.4} {:8.2} {:8.4}",
            m.obligation * 100.0, m.d_eff, m.phi, m.mean_s3, m.jsd, m.tunnel_ratio, ratio);
    }
    println!();

    // Now the other direction: make English MORE uniform (reduce obligation)
    println!("  And in the other direction — making English MORE uniform...");
    println!();
    for blend in 0..=10 {
        let t = blend as f64 / 10.0; // 0 = English, 1 = fully uniform
        let mut w = eng.weights.clone();
        for i in 0..n {
            let row_sum: f64 = eng.weights[i].iter().sum();
            if row_sum < 1.0 { continue; }
            let mean_w = row_sum / n as f64;
            for j in 0..n {
                if eng.weights[i][j] > 0.0 {
                    w[i][j] = (1.0 - t) * eng.weights[i][j] + t * mean_w;
                }
            }
        }
        let g = SyntaxGraph { n, labels: eng.labels.clone(), weights: w };
        let m = full_metrics(&g);
        println!("  uniform_t={:.1}  oblig={:.1}%  d_eff={}  Φ={:.3}  S_τ={:.4}  JSD={:.4}",
            t, m.obligation * 100.0, m.d_eff, m.phi, m.mean_s3, m.jsd);
    }
    println!();

    // ═══ C: Programming languages ═══
    println!("═══ C: PROGRAMMING LANGUAGE SYNTAX ═══");
    println!();

    let langs = ["Python", "Rust", "Haskell", "Lisp", "SQL"];
    println!("  {:12} {:>8} {:>8} {:>8} {:>5} {:>8} {:>8} {:>8}",
        "Language", "S_τ(3)", "JSD", "Φ", "d_eff", "Oblig.", "TunRat", "min S_τ");
    println!("  {:12} {:>8} {:>8} {:>8} {:>5} {:>8} {:>8} {:>8}",
        "─".repeat(12), "─".repeat(8), "─".repeat(8), "─".repeat(8),
        "─".repeat(5), "─".repeat(8), "─".repeat(8), "─".repeat(8));

    // First show natural languages for comparison
    for (name, g) in &[("English", &eng), ("Lojban", &lojban), ("Chinese", &chinese)] {
        let m = full_metrics(g);
        println!("  {:12} {:8.4} {:8.4} {:8.3} {:5} {:8.3} {:8.2} {:8.3}",
            name, m.mean_s3, m.jsd, m.phi, m.d_eff, m.obligation, m.tunnel_ratio, m.min_s3);
    }
    println!("  {:12}", "─".repeat(80));
    for name in &langs {
        let g = make_prog_lang(name);
        let m = full_metrics(&g);
        println!("  {:12} {:8.4} {:8.4} {:8.3} {:5} {:8.3} {:8.2} {:8.3}",
            name, m.mean_s3, m.jsd, m.phi, m.d_eff, m.obligation, m.tunnel_ratio, m.min_s3);
    }
    println!();

    // Per-token-category analysis for most interesting programming language
    println!("  Per-token S_τ(3) for Rust:");
    let rust = make_prog_lang("Rust");
    let rust_s3 = rust.stau_all(3);
    let rust_p = rust.transition_matrix();
    for i in 0..rust.n {
        let row_sum: f64 = rust.weights[i].iter().sum();
        if row_sum < 1.0 { continue; }
        let h: f64 = (0..rust.n).map(|j| {
            if rust_p[i][j] > 1e-30 { -rust_p[i][j] * rust_p[i][j].log2() } else { 0.0 }
        }).sum();
        let mut top: Vec<(usize, f64)> = (0..rust.n).map(|j| (j, rust_p[i][j])).collect();
        top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top_str: String = top.iter().take(3).filter(|&&(_, p)| p > 0.01)
            .map(|&(j, p)| format!("{}({:.0}%)", rust.labels[j], p * 100.0))
            .collect::<Vec<_>>().join(" ");
        println!("    {:8} S_τ={:.3} H={:.2}  → {}", rust.labels[i], rust_s3[i], h, top_str);
    }
    println!();

    // ═══ D: Robustness analysis ═══
    println!("═══ D: ROBUSTNESS — HOW FRAGILE IS THE FRONTIER? ═══");
    println!();

    // For English and utopia, measure how metrics degrade under random perturbation
    let perturbation_strengths = [0.01, 0.05, 0.10, 0.20, 0.50];
    let n_trials = 50;

    println!("  Metric degradation under random perturbation (mean over {} trials):", n_trials);
    println!();

    for (name, base) in &[("English", &eng), ("Utopia", &utopia), ("Lojban", &lojban)] {
        let base_m = full_metrics(base);
        println!("  {} (baseline: S_τ={:.4} JSD={:.4} Φ={:.3}):", name, base_m.mean_s3, base_m.jsd, base_m.phi);
        println!("  {:>8} {:>10} {:>10} {:>10} {:>10}",
            "ε", "ΔS_τ%", "ΔJSD%", "ΔΦ%", "d_eff_avg");
        for &eps in &perturbation_strengths {
            let mut sum_ds = 0.0; let mut sum_dj = 0.0; let mut sum_dp = 0.0; let mut sum_deff = 0.0;
            for trial in 0..n_trials {
                let mut rng2 = Rng::new(1000 + trial);
                let mut w = base.weights.clone();
                for i in 0..base.n { for j in 0..base.n {
                    if w[i][j] > 0.0 {
                        let noise = 1.0 + eps * (2.0 * rng2.next_f64() - 1.0);
                        w[i][j] = (w[i][j] * noise).max(0.0);
                    }
                }}
                let g = SyntaxGraph { n: base.n, labels: base.labels.clone(), weights: w };
                let m = full_metrics(&g);
                sum_ds += (m.mean_s3 - base_m.mean_s3) / base_m.mean_s3 * 100.0;
                sum_dj += (m.jsd - base_m.jsd) / base_m.jsd.max(0.001) * 100.0;
                sum_dp += if base_m.phi > 0.01 { (m.phi - base_m.phi) / base_m.phi * 100.0 } else { 0.0 };
                sum_deff += m.d_eff as f64;
            }
            println!("  {:8.0}% {:>+10.2}% {:>+10.2}% {:>+10.2}% {:10.2}",
                eps * 100.0,
                sum_ds / n_trials as f64,
                sum_dj / n_trials as f64,
                sum_dp / n_trials as f64,
                sum_deff / n_trials as f64);
        }
        println!();
    }

    // ═══ E: Equality frontier — mean vs min S_τ ═══
    println!("═══ E: EQUALITY — MEAN vs MIN S_τ ═══");
    println!();

    // For each language, compute per-POS S_τ(3) and show Gini + min/max ratio
    println!("  {:12} {:>8} {:>8} {:>8} {:>8} {:>10} {:>12}",
        "Language", "mean", "min", "max", "Gini", "min/max", "worst POS");
    println!("  {:12} {:>8} {:>8} {:>8} {:>8} {:>10} {:>12}",
        "─".repeat(12), "─".repeat(8), "─".repeat(8), "─".repeat(8),
        "─".repeat(8), "─".repeat(10), "─".repeat(12));

    let all_langs: Vec<(&str, &SyntaxGraph)> = vec![
        ("English", &eng), ("Lojban", &lojban), ("Chinese", &chinese), ("Utopia", &utopia),
    ];
    for (name, g) in &all_langs {
        let s3 = g.stau_all(3);
        let start = g.label_idx("START").unwrap_or(usize::MAX);
        let end = g.label_idx("END").unwrap_or(usize::MAX);

        let mut active: Vec<(usize, f64)> = Vec::new();
        for i in 0..g.n {
            if i == start || i == end { continue; }
            let row_sum: f64 = g.weights[i].iter().sum();
            if row_sum < 1.0 { continue; }
            active.push((i, s3[i]));
        }

        let vals: Vec<f64> = active.iter().map(|(_, s)| *s).collect();
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let min_val = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let max_val = vals.iter().copied().fold(0.0f64, f64::max);
        let gini_val = gini(&vals);
        let worst = active.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();

        println!("  {:12} {:8.3} {:8.3} {:8.3} {:8.3} {:10.3} {:>12}",
            name, mean, min_val, max_val, gini_val, min_val / max_val, g.labels[worst.0]);
    }
    println!();

    // Show the programming languages too
    for name in &langs {
        let g = make_prog_lang(name);
        let s3 = g.stau_all(3);
        let start = g.label_idx("START").unwrap_or(usize::MAX);
        let end = g.label_idx("END").unwrap_or(usize::MAX);
        let mut active: Vec<(usize, f64)> = Vec::new();
        for i in 0..g.n {
            if i == start || i == end { continue; }
            let row_sum: f64 = g.weights[i].iter().sum();
            if row_sum < 1.0 { continue; }
            active.push((i, s3[i]));
        }
        let vals: Vec<f64> = active.iter().map(|(_, s)| *s).collect();
        let mean = vals.iter().sum::<f64>() / vals.len().max(1) as f64;
        let min_val = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let max_val = vals.iter().copied().fold(0.0f64, f64::max);
        let gini_val = gini(&vals);
        let worst = active.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let worst_name = worst.map(|(i, _)| g.labels[*i].as_str()).unwrap_or("?");
        println!("  {:12} {:8.3} {:8.3} {:8.3} {:8.3} {:10.3} {:>12}",
            name, mean, min_val, max_val, gini_val, min_val / max_val, worst_name);
    }
    println!();

    // ═══ Summary ═══
    println!("═══ VERDICT ═══");
    println!();
    println!("  Five analyses complete. Key results above.");
}

fn gini(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 2 { return 0.0; }
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let total: f64 = sorted.iter().sum();
    if total < 1e-15 { return 0.0; }
    let mut sum = 0.0;
    for (i, &v) in sorted.iter().enumerate() {
        sum += (2.0 * (i + 1) as f64 - n as f64 - 1.0) * v;
    }
    sum / (n as f64 * total)
}

// Quick evolutionary search to find utopia (same algorithm as syntax_pareto but fewer generations)
fn find_utopia(eng: &SyntaxGraph, lojban: &SyntaxGraph, chinese: &SyntaxGraph) -> SyntaxGraph {
    let mut rng = Rng::new(42); // Same seed as syntax_pareto
    let mut best_g = eng.clone();
    let mut best_score = composite_score(eng);

    let seeds = [eng, lojban, chinese];

    // Generate candidates
    for gen in 0..30 {
        for base in &seeds {
            for _ in 0..20 {
                let g = mutate_quick(base, &mut rng, 0.15, 1.5);
                let score = composite_score(&g);
                if score > best_score { best_score = score; best_g = g; }
            }
        }
        // Also mutate current best
        for _ in 0..30 {
            let g = mutate_quick(&best_g, &mut rng, 0.12, 1.2);
            let score = composite_score(&g);
            if score > best_score { best_score = score; best_g = g; }
        }
        // Crossover
        for base in &seeds {
            for _ in 0..5 {
                let g = crossover_quick(&best_g, base, &mut rng);
                let g = mutate_quick(&g, &mut rng, 0.08, 1.0);
                let score = composite_score(&g);
                if score > best_score { best_score = score; best_g = g; }
            }
        }
    }

    best_g
}

fn composite_score(g: &SyntaxGraph) -> f64 {
    let m = full_metrics(g);
    // Balanced objective: want high on all three simultaneously
    let phi_f = if m.d_eff >= 2 { m.phi } else { 0.1 };
    phi_f * m.mean_s3 * m.jsd
}

fn mutate_quick(base: &SyntaxGraph, rng: &mut Rng, rate: f64, strength: f64) -> SyntaxGraph {
    let n = base.n;
    let mut w = base.weights.clone();
    let start = base.label_idx("START").unwrap_or(usize::MAX);
    let end = base.label_idx("END").unwrap_or(usize::MAX);
    for i in 0..n { for j in 0..n {
        if rng.next_f64() < rate {
            if i == end || j == start { continue; }
            let r = rng.next_f64();
            if r < 0.1 { w[i][j] = 0.0; }
            else if r < 0.25 && base.weights[i][j] < 0.1 { w[i][j] = rng.next_f64() * 50.0; }
            else if base.weights[i][j] > 0.0 {
                let factor = (rng.next_f64() * 2.0 * strength).exp() / strength.exp();
                w[i][j] = (base.weights[i][j] * factor).max(0.0);
            }
        }
    }}
    SyntaxGraph { n, labels: base.labels.clone(), weights: w }
}

fn crossover_quick(a: &SyntaxGraph, b: &SyntaxGraph, rng: &mut Rng) -> SyntaxGraph {
    let n = a.n;
    let mut w = vec![vec![0.0f64; n]; n];
    for i in 0..n { for j in 0..n {
        let r = rng.next_f64();
        if r < 0.4 { w[i][j] = a.weights[i][j]; }
        else if r < 0.8 { w[i][j] = b.weights[i][j]; }
        else { w[i][j] = rng.next_f64() * a.weights[i][j] + (1.0 - rng.next_f64()) * b.weights[i][j]; }
    }}
    SyntaxGraph { n, labels: a.labels.clone(), weights: w }
}
