//! Syntactic S_τ: Evaluating and Discovering Optimal Syntax
//!
//! Models syntax as a weighted POS transition graph. Computes S_τ on it to
//! measure sentence construction freedom. Compares real English (SVO) with
//! synthetic SOV, VSO, free word order, and novel structures discovered by
//! perturbing the transition matrix to maximize combined Φ.
//!
//! Phase 1: Real English syntactic graph from Brown corpus POS bigrams
//! Phase 2: Synthetic word order variants (SOV, VSO, V2, free)
//! Phase 3: Combined semantic + syntactic eigenstate (10D)
//! Phase 4: Novel syntax discovery via perturbation
//!
//! Run: cargo run --release --bin syntax-stau

use std::collections::BTreeSet;
use std::fs;

const N_DIM: usize = 5;
const TAU_MAX: usize = 7;

// ── Weighted transition graph ──

struct SyntaxGraph {
    n: usize,
    labels: Vec<String>,
    weights: Vec<Vec<f64>>, // weights[i][j] = transition count from i to j
}

impl SyntaxGraph {
    fn from_tsv(pos_path: &str, bigram_path: &str) -> Self {
        let pos_str = fs::read_to_string(pos_path).expect("Need data/pos_tags.tsv");
        let bg_str = fs::read_to_string(bigram_path).expect("Need data/pos_bigrams.tsv");

        let mut labels = Vec::new();
        let mut pos_freq = Vec::new();
        for line in pos_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                labels.push(parts[1].to_string());
                let _freq: f64 = parts[2].parse().unwrap_or(0.0);
                pos_freq.push(_freq);
            }
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
                p[i][i] = 1.0; // absorbing state
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

    // Create a variant by reweighting transitions.
    fn variant(&self, name: &str, modifier: &dyn Fn(&[String], usize, usize, f64) -> f64) -> SyntaxGraph {
        let n = self.n;
        let mut weights = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            for j in 0..n {
                weights[i][j] = modifier(&self.labels, i, j, self.weights[i][j]);
            }
        }
        SyntaxGraph { n, labels: self.labels.clone(), weights }
    }
}

// ── Eigenstate computation (same as language_value_learning) ──

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

// ── Syntactic dimensions per POS ──
// D0: S_τ(1) — immediate construction freedom
// D1: S_τ(3) — medium-range sentence planning
// D2: S_τ(5) — long-range narrative reach
// D3: In-degree concentration — how many POS can precede this one
// D4: Out-degree concentration — how many POS can follow this one

fn syntactic_dims(g: &SyntaxGraph) -> Vec<[f64; N_DIM]> {
    let s1 = g.stau_all(1);
    let s3 = g.stau_all(3);
    let s5 = g.stau_all(5);
    let p = g.transition_matrix();
    let n = g.n;

    // Compute in-degree (column sums of weight matrix, normalized).
    let mut in_deg = vec![0.0f64; n];
    let total_weight: f64 = g.weights.iter().flat_map(|r| r.iter()).sum();
    for j in 0..n {
        let col_sum: f64 = (0..n).map(|i| g.weights[i][j]).sum();
        in_deg[j] = if total_weight > 0.0 { col_sum / total_weight } else { 0.0 };
    }

    // Out-degree entropy — how evenly distributed are outgoing transitions?
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

// Correlation-matrix eigenstate from per-POS dimensions.
fn compute_eigenstate(dims: &[[f64; N_DIM]]) -> (usize, [f64; N_DIM], f64) {
    let n = dims.len();
    if n < 3 { return (0, [0.0; N_DIM], 0.0); }

    let mut means = [0.0f64; N_DIM];
    let mut stds = [0.0f64; N_DIM];
    for d in 0..N_DIM {
        means[d] = dims.iter().map(|w| w[d]).sum::<f64>() / n as f64;
    }
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
            for w in dims {
                s += (w[a] - means[a]) * (w[b] - means[b]);
            }
            corr[a][b] = s / (n as f64 * stds[a] * stds[b]);
        }
    }}

    let (evals, _evecs) = jacobi_eigen(&corr);
    let d_eff = compute_d_eff(&evals);

    // Aggregate Φ: fraction of variance in top d_eff dimensions.
    let lambda_sum: f64 = evals.iter().sum();
    let phi = if d_eff >= 2 && lambda_sum > 1e-15 {
        evals.iter().take(d_eff).sum::<f64>() / lambda_sum
    } else {
        0.0
    };

    (d_eff, evals, phi)
}

// ── Syntax profile: aggregate metrics for a syntactic graph ──

struct SyntaxProfile {
    name: String,
    mean_s1: f64,
    mean_s3: f64,
    mean_s5: f64,
    mean_s7: f64,
    d_eff: usize,
    phi: f64,
    // JSD-like: mean pairwise divergence of transition distributions.
    mean_jsd: f64,
    // Gini of S_τ(3) — inequality of construction freedom.
    gini_s3: f64,
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

fn mean_pairwise_jsd(g: &SyntaxGraph) -> f64 {
    let p = g.transition_matrix();
    let n = g.n;
    let mut total = 0.0;
    let mut count = 0;
    for i in 0..n {
        for j in (i+1)..n {
            // JSD between transition distributions of POS i and POS j.
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

fn profile(name: &str, g: &SyntaxGraph) -> SyntaxProfile {
    let s1 = g.stau_all(1);
    let s3 = g.stau_all(3);
    let s5 = g.stau_all(5);
    let s7 = g.stau_all(7);
    let n = g.n as f64;

    let dims = syntactic_dims(g);
    let (d_eff, _evals, phi) = compute_eigenstate(&dims);

    SyntaxProfile {
        name: name.to_string(),
        mean_s1: s1.iter().sum::<f64>() / n,
        mean_s3: s3.iter().sum::<f64>() / n,
        mean_s5: s5.iter().sum::<f64>() / n,
        mean_s7: s7.iter().sum::<f64>() / n,
        d_eff,
        phi,
        mean_jsd: mean_pairwise_jsd(g),
        gini_s3: gini(&s3),
    }
}

fn print_profiles(profiles: &[SyntaxProfile]) {
    println!("  {:20} {:>7} {:>7} {:>7} {:>7} {:>5} {:>7} {:>7} {:>7}",
        "Syntax", "S_τ(1)", "S_τ(3)", "S_τ(5)", "S_τ(7)", "d_eff", "Φ", "JSD", "Gini");
    println!("  {:20} {:>7} {:>7} {:>7} {:>7} {:>5} {:>7} {:>7} {:>7}",
        "─".repeat(20), "─".repeat(7), "─".repeat(7), "─".repeat(7),
        "─".repeat(7), "─".repeat(5), "─".repeat(7), "─".repeat(7), "─".repeat(7));
    for p in profiles {
        println!("  {:20} {:7.3} {:7.3} {:7.3} {:7.3} {:5} {:7.3} {:7.3} {:7.3}",
            p.name, p.mean_s1, p.mean_s3, p.mean_s5, p.mean_s7,
            p.d_eff, p.phi, p.mean_jsd, p.gini_s3);
    }
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║  Syntactic S_τ: Evaluating and Discovering Optimal Syntax        ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();

    // ═══ Phase 1: Real English ═══
    println!("═══ PHASE 1: REAL ENGLISH SYNTACTIC GRAPH ═══");
    println!();

    let eng = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams.tsv");
    println!("  {} POS categories, {} transitions",
        eng.n, eng.weights.iter().flat_map(|r| r.iter()).filter(|&&w| w > 0.0).count());

    let s3 = eng.stau_all(3);
    println!();
    println!("  Per-POS S_τ(3) — sentence construction freedom:");
    let mut pos_s3: Vec<(usize, f64)> = (0..eng.n).map(|i| (i, s3[i])).collect();
    pos_s3.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for &(i, s) in pos_s3.iter().take(15) {
        let top_trans: Vec<String> = {
            let p = eng.transition_matrix();
            let mut trans: Vec<(usize, f64)> = (0..eng.n).map(|j| (j, p[i][j])).collect();
            trans.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            trans.iter().take(3).filter(|&&(_, w)| w > 0.01)
                .map(|&(j, w)| format!("{}({:.0}%)", eng.labels[j], w * 100.0))
                .collect()
        };
        println!("    {:8} S_τ={:.3}  → {}", eng.labels[i], s, top_trans.join(", "));
    }
    println!();

    // ═══ Phase 2: Synthetic word order variants ═══
    println!("═══ PHASE 2: WORD ORDER VARIANTS ═══");
    println!();

    // English SVO (baseline).
    let eng_profile = profile("English (SVO)", &eng);

    // SOV: Boost NOUN→NOUN, NOUN→VERB; reduce VERB→NOUN, VERB→DET.
    // Japanese/Korean/Turkish pattern: subject first, object next, verb last.
    let sov = eng.variant("SOV", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        match (from.as_str(), to.as_str()) {
            // Boost S→O patterns.
            ("START", "NOUN") | ("START", "DET") | ("START", "PRON") => w * 2.0,
            ("NOUN", "NOUN") | ("DET", "NOUN") | ("ADJ", "NOUN") => w * 1.5,
            // Verb goes to END.
            ("VERB", "END") => w * 3.0,
            // Reduce V→O.
            ("VERB", "DET") | ("VERB", "NOUN") | ("VERB", "PRON") => w * 0.3,
            // Boost O→V.
            ("NOUN", "VERB") | ("PRON", "VERB") => w * 2.5,
            _ => w,
        }
    });
    let sov_profile = profile("SOV (Japanese-like)", &sov);

    // VSO: Verb first.
    let vso = eng.variant("VSO", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        match (from.as_str(), to.as_str()) {
            ("START", "VERB") | ("START", "BE") | ("START", "AUX") => w * 3.0,
            ("START", "NOUN") | ("START", "DET") | ("START", "PRON") => w * 0.3,
            ("VERB", "NOUN") | ("VERB", "DET") | ("VERB", "PRON") => w * 2.0,
            _ => w,
        }
    });
    let vso_profile = profile("VSO (Arabic-like)", &vso);

    // V2: Verb in second position (Germanic).
    let v2 = eng.variant("V2", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        match (from.as_str(), to.as_str()) {
            // Any constituent can be first.
            ("START", _) => w * 1.2,
            // But first constituent MUST go to verb.
            ("NOUN", "VERB") | ("ADV", "VERB") | ("PREP", "VERB") |
            ("ADJ", "VERB") | ("PRON", "VERB") => w * 2.0,
            // Verb early, then rest is free.
            ("VERB", _) => w * 1.3,
            _ => w,
        }
    });
    let v2_profile = profile("V2 (Germanic)", &v2);

    // Free word order: flatten all transitions toward uniform.
    let free = eng.variant("Free", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        // Keep START/END structure, flatten everything else.
        if from == "START" || to == "END" { w }
        else if from == "END" || to == "START" { 0.0 }
        else {
            let base = w.max(1.0); // Minimum weight 1 for all transitions.
            base.sqrt() // Compress toward uniform while preserving some structure.
        }
    });
    let free_profile = profile("Free (Latin-like)", &free);

    // Ergative-absolutive: different subject marking.
    let erg = eng.variant("Ergative", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        match (from.as_str(), to.as_str()) {
            // Intransitive: S→V (absolutive, unmarked).
            ("NOUN", "VERB") | ("PRON", "VERB") => w * 1.8,
            // Transitive: Agent (ergative) marked differently.
            ("NOUN", "NOUN") => w * 1.5, // Agent→Patient more common.
            // More verb-medial flexibility.
            ("VERB", "NOUN") | ("VERB", "PRON") => w * 1.3,
            _ => w,
        }
    });
    let erg_profile = profile("Ergative (Basque-like)", &erg);

    // Polysynthetic: Verb incorporates arguments (fewer POS transitions, denser verb).
    let poly = eng.variant("Polysynthetic", &|labels, i, j, w| {
        let from = &labels[i]; let to = &labels[j];
        match (from.as_str(), to.as_str()) {
            // VERB is a mega-category that absorbs objects, subjects.
            ("VERB", "VERB") => w * 3.0,
            ("VERB", "END") => w * 2.0,
            // NOUN/PRON less common as standalone (incorporated into verb).
            ("START", "VERB") => w * 2.5,
            ("NOUN", "END") => w * 0.5,
            _ => w,
        }
    });
    let poly_profile = profile("Polysynthetic", &poly);

    // ── Constructed and analytic languages ──

    // Lojban: explicit predicate logic, no obligatory morphology.
    let lojban = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_lojban.tsv");
    let lojban_profile = profile("Lojban", &lojban);

    // Chinese: topic-comment, no articles, serial verbs.
    let chinese = SyntaxGraph::from_tsv("data/pos_tags.tsv", "data/pos_bigrams_chinese.tsv");
    let chinese_profile = profile("Chinese (topic-comment)", &chinese);

    let mut profiles = vec![
        eng_profile, sov_profile, vso_profile, v2_profile,
        free_profile, erg_profile, poly_profile,
        lojban_profile, chinese_profile,
    ];

    print_profiles(&profiles);
    println!();

    // ═══ Phase 3: Combined eigenstate ═══
    println!("═══ PHASE 3: INTEGRATION ANALYSIS ═══");
    println!();

    // For each syntax, compute the per-POS dimension profile and eigenstate.
    let syntaxes: Vec<(&str, &SyntaxGraph)> = vec![
        ("English (SVO)", &eng), ("SOV", &sov), ("VSO", &vso),
        ("V2", &v2), ("Free", &free), ("Ergative", &erg), ("Polysynthetic", &poly),
        ("Lojban", &lojban), ("Chinese", &chinese),
    ];

    println!("  Per-POS eigenstate analysis:");
    println!("  {:20} {:>5} {:>7}  {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Syntax", "d_eff", "Φ", "λ₁", "λ₂", "λ₃", "λ₄", "λ₅");
    println!("  {:20} {:>5} {:>7}  {:>8} {:>8} {:>8} {:>8} {:>8}",
        "─".repeat(20), "─".repeat(5), "─".repeat(7),
        "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8), "─".repeat(8));
    for (name, g) in &syntaxes {
        let dims = syntactic_dims(g);
        let (d_eff, evals, phi) = compute_eigenstate(&dims);
        println!("  {:20} {:5} {:7.3}  {:8.4} {:8.4} {:8.4} {:8.4} {:8.4}",
            name, d_eff, phi, evals[0], evals[1], evals[2], evals[3], evals[4]);
    }
    println!();

    // ═══ Phase 4: Novel syntax discovery ═══
    println!("═══ PHASE 4: NOVEL SYNTAX DISCOVERY ═══");
    println!();
    println!("  Perturbing English syntax to find structures that maximize Φ × S_τ(3)...");
    println!();

    // Strategy: for each POS pair (i,j), try boosting and reducing the weight.
    // Score = Φ × mean_S_τ(3) × mean_JSD (reach × integration × discrimination).
    let eng_score = eng_profile_score(&eng);
    println!("  English baseline score (Φ × S_τ(3) × JSD): {:.4}", eng_score);
    println!();

    // Try single-edge perturbations.
    let mut improvements: Vec<(usize, usize, f64, f64, String)> = Vec::new(); // (i, j, factor, score, description)

    for i in 0..eng.n {
        for j in 0..eng.n {
            if eng.weights[i][j] < 1.0 { continue; }
            // Try boosting 3x.
            let boosted = single_perturb(&eng, i, j, 3.0);
            let score = eng_profile_score(&boosted);
            if score > eng_score * 1.01 {
                improvements.push((i, j, 3.0, score,
                    format!("{}→{} ×3.0", eng.labels[i], eng.labels[j])));
            }
            // Try reducing to 0.2x.
            let reduced = single_perturb(&eng, i, j, 0.2);
            let score = eng_profile_score(&reduced);
            if score > eng_score * 1.01 {
                improvements.push((i, j, 0.2, score,
                    format!("{}→{} ×0.2", eng.labels[i], eng.labels[j])));
            }
            // Try eliminating.
            let eliminated = single_perturb(&eng, i, j, 0.0);
            let score = eng_profile_score(&eliminated);
            if score > eng_score * 1.01 {
                improvements.push((i, j, 0.0, score,
                    format!("{}→{} ×0.0", eng.labels[i], eng.labels[j])));
            }
        }
    }

    improvements.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());

    println!("  Top 20 single-transition perturbations that improve on English:");
    println!("  {:>4} {:30} {:>8} {:>8}",
        "Rank", "Perturbation", "Score", "vs Eng");
    println!("  {:>4} {:30} {:>8} {:>8}",
        "─".repeat(4), "─".repeat(30), "─".repeat(8), "─".repeat(8));
    for (rank, (_, _, _, score, desc)) in improvements.iter().take(20).enumerate() {
        println!("  {:>4} {:30} {:8.4} {:>+7.1}%",
            rank + 1, desc, score, (score / eng_score - 1.0) * 100.0);
    }
    println!();

    // Now try combining the top improvements.
    if improvements.len() >= 3 {
        println!("  Combining top non-conflicting perturbations...");
        let mut combined = eng.weights.clone();
        let mut applied = Vec::new();
        for &(i, j, factor, _, ref desc) in improvements.iter().take(10) {
            // Check if this edge conflicts with already-applied perturbations.
            let dominated = applied.iter().any(|&(ai, aj): &(usize, usize)| ai == i && aj == j);
            if !dominated {
                combined[i][j] = eng.weights[i][j] * factor;
                applied.push((i, j));
            }
        }
        let combined_g = SyntaxGraph { n: eng.n, labels: eng.labels.clone(), weights: combined };
        let combined_score = eng_profile_score(&combined_g);
        let combined_profile = profile("Novel Combined", &combined_g);

        println!("  Applied {} perturbations", applied.len());
        println!("  Combined score: {:.4} ({:+.1}% vs English)",
            combined_score, (combined_score / eng_score - 1.0) * 100.0);
        println!();

        profiles.push(combined_profile);

        // Show what the novel syntax looks like.
        println!("  Novel syntax transition changes:");
        for &(i, j) in &applied {
            let old_w = eng.weights[i][j];
            let new_w = combined_g.weights[i][j];
            let old_p = {
                let row_sum: f64 = eng.weights[i].iter().sum();
                if row_sum > 0.0 { old_w / row_sum } else { 0.0 }
            };
            let new_p = {
                let row_sum: f64 = combined_g.weights[i].iter().sum();
                if row_sum > 0.0 { new_w / row_sum } else { 0.0 }
            };
            println!("    {}→{}: {:.1}%→{:.1}%",
                eng.labels[i], eng.labels[j], old_p * 100.0, new_p * 100.0);
        }
        println!();
    }

    // ═══ Final comparison ═══
    println!("═══ FINAL COMPARISON ═══");
    println!();
    print_profiles(&profiles);
    println!();

    // ═══ Verdict ═══
    println!("═══ VERDICT ═══");
    let best_s3 = profiles.iter().max_by(|a, b| a.mean_s3.partial_cmp(&b.mean_s3).unwrap()).unwrap();
    let best_deff = profiles.iter().max_by_key(|p| p.d_eff).unwrap();
    let best_phi = profiles.iter().max_by(|a, b| a.phi.partial_cmp(&b.phi).unwrap()).unwrap();
    let best_jsd = profiles.iter().max_by(|a, b| a.mean_jsd.partial_cmp(&b.mean_jsd).unwrap()).unwrap();

    println!("  Best S_τ(3):  {} ({:.3})", best_s3.name, best_s3.mean_s3);
    println!("  Best d_eff:   {} ({})", best_deff.name, best_deff.d_eff);
    println!("  Best Φ:       {} ({:.3})", best_phi.name, best_phi.phi);
    println!("  Best JSD:     {} ({:.3})", best_jsd.name, best_jsd.mean_jsd);
    println!();

    // ═══ Phase 5: Sentence structure sampling ═══
    println!("═══ PHASE 5: SENTENCE STRUCTURES ═══");
    println!();
    println!("  Most probable 5-word sentence structures (POS sequences):");
    println!();

    let show_syntaxes: Vec<(&str, &SyntaxGraph)> = vec![
        ("English", &eng), ("Lojban", &lojban), ("Chinese", &chinese),
    ];
    if let Some(combined_g_ref) = profiles.iter().find(|p| p.name == "Novel Combined") {
        // We need to regenerate combined_g for sentence sampling — use a simpler approach.
        let _ = combined_g_ref; // just to avoid unused warning
    }

    for (name, g) in &show_syntaxes {
        let p = g.transition_matrix();
        let start = g.label_idx("START").unwrap_or(0);
        let end = g.label_idx("END").unwrap_or(0);

        // Generate sentences by sampling most-probable paths (beam search).
        let mut sentences: Vec<(Vec<usize>, f64)> = vec![(vec![start], 1.0)];
        for _ in 0..7 { // Up to 7 tokens after START.
            let mut next_sents = Vec::new();
            for (seq, prob) in &sentences {
                let last = *seq.last().unwrap();
                if last == end { next_sents.push((seq.clone(), *prob)); continue; }
                // Take top 3 transitions.
                let mut trans: Vec<(usize, f64)> = (0..g.n).map(|j| (j, p[last][j])).collect();
                trans.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                for &(j, w) in trans.iter().take(3) {
                    if w < 0.05 { continue; }
                    let mut new_seq = seq.clone();
                    new_seq.push(j);
                    next_sents.push((new_seq, prob * w));
                }
            }
            next_sents.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            next_sents.truncate(50); // Keep top 50 beams.
            sentences = next_sents;
        }

        // Filter to completed sentences (ending with END), length 4-8 tokens.
        let mut completed: Vec<(String, f64)> = sentences.iter()
            .filter(|(seq, _)| *seq.last().unwrap() == end && seq.len() >= 4 && seq.len() <= 9)
            .map(|(seq, prob)| {
                let pos_str: Vec<&str> = seq[1..seq.len()-1].iter()
                    .map(|&i| g.labels[i].as_str()).collect();
                (pos_str.join(" → "), *prob)
            })
            .collect();
        completed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        completed.dedup_by(|a, b| a.0 == b.0);

        println!("  {} — top 10 sentence patterns:", name);
        for (i, (pattern, prob)) in completed.iter().take(10).enumerate() {
            println!("    {:2}. [{:.4}] {}", i + 1, prob, pattern);
        }
        println!();
    }

    // ═══ Phase 6: What makes Lojban special ═══
    println!("═══ PHASE 6: LOJBAN vs ENGLISH — STRUCTURAL COMPARISON ═══");
    println!();

    // Per-POS S_τ(3) comparison.
    let eng_s3 = eng.stau_all(3);
    let loj_s3 = lojban.stau_all(3);
    let chi_s3 = chinese.stau_all(3);

    println!("  {:10} {:>10} {:>10} {:>10} {:>10}", "POS", "English", "Lojban", "Chinese", "Loj-Eng");
    println!("  {:10} {:>10} {:>10} {:>10} {:>10}",
        "─".repeat(10), "─".repeat(10), "─".repeat(10), "─".repeat(10), "─".repeat(10));
    let mut pos_order: Vec<usize> = (0..eng.n).collect();
    pos_order.sort_by(|&a, &b| (loj_s3[a] - eng_s3[a]).partial_cmp(&(loj_s3[b] - eng_s3[b])).unwrap().reverse());
    for &i in pos_order.iter().take(15) {
        let diff = loj_s3[i] - eng_s3[i];
        println!("  {:10} {:10.3} {:10.3} {:10.3} {:>+10.3}",
            eng.labels[i], eng_s3[i], loj_s3[i], chi_s3[i], diff);
    }
    println!();

    // Transition entropy comparison — which language gives each POS the most CHOICE?
    println!("  Transition entropy (bits of choice from each POS):");
    let eng_p = eng.transition_matrix();
    let loj_p = lojban.transition_matrix();
    println!("  {:10} {:>10} {:>10} {:>10}", "POS", "English", "Lojban", "Diff");
    println!("  {:10} {:>10} {:>10} {:>10}",
        "─".repeat(10), "─".repeat(10), "─".repeat(10), "─".repeat(10));
    for i in 0..eng.n {
        let eng_h: f64 = (0..eng.n).map(|j| {
            if eng_p[i][j] > 1e-30 { -eng_p[i][j] * eng_p[i][j].log2() } else { 0.0 }
        }).sum();
        let loj_h: f64 = (0..eng.n).map(|j| {
            if loj_p[i][j] > 1e-30 { -loj_p[i][j] * loj_p[i][j].log2() } else { 0.0 }
        }).sum();
        if eng_h > 0.5 || loj_h > 0.5 {
            println!("  {:10} {:10.3} {:10.3} {:>+10.3}",
                eng.labels[i], eng_h, loj_h, loj_h - eng_h);
        }
    }
    println!();
}

fn eng_profile_score(g: &SyntaxGraph) -> f64 {
    let s3 = g.stau_all(3);
    let mean_s3 = s3.iter().sum::<f64>() / g.n as f64;
    let jsd = mean_pairwise_jsd(g);
    let dims = syntactic_dims(g);
    let (d_eff, _evals, phi) = compute_eigenstate(&dims);
    let phi_factor = if d_eff >= 2 { phi } else { 0.1 }; // Penalize d_eff < 2.
    phi_factor * mean_s3 * jsd
}

fn single_perturb(base: &SyntaxGraph, i: usize, j: usize, factor: f64) -> SyntaxGraph {
    let mut weights = base.weights.clone();
    weights[i][j] = base.weights[i][j] * factor;
    SyntaxGraph { n: base.n, labels: base.labels.clone(), weights }
}
