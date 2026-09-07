//! Bounded integer dependency obligations from checked abstract evaluation.
//! Only a fresh UNSAT result can strengthen a checked comparison. SAT in
//! this relaxation is not a concrete case or a discovered income cliff.
//! Unknown expressions are independent, unbounded integers, never constants
//! inferred from equal enclosures. Only declared source axes receive bounds.

use super::{affine_interval::MAX_AXES, Correlation, IntInterval};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

type Id = [u8; 32];
const MAX_NODES: usize = 65_536;
const MAX_REACHABLE: usize = 4096;
const MAX_SCRIPT_BYTES: usize = 1_048_576;
const MAX_PAIRED_UNARY: usize = 128;
const MAX_PAIRED_FACTS: usize = 256;

/// Checked affine guard roots nominate adjacent source cuts. This is only a
/// search heuristic: the resulting children still need independent proofs.
pub(super) fn split_hints(
    left: IntInterval,
    right: IntInterval,
    source_axes: usize,
) -> Vec<(usize, i64)> {
    let Some((coefficients, constant, _)) = left
        .symbolic()
        .and_then(|a| a.add(right.symbolic()?.scale(-1)?))
        .and_then(|difference| difference.exact_form())
    else {
        return vec![];
    };
    let mut terms = coefficients.iter().enumerate().filter(|(_, c)| **c != 0);
    let Some((axis, &coefficient)) = terms.next() else {
        return vec![];
    };
    if axis >= source_axes || terms.next().is_some() {
        return vec![];
    }
    let Some(numerator) = constant.checked_neg() else {
        return vec![];
    };
    let (Some(mut root), Some(remainder)) = (
        numerator.checked_div(coefficient),
        numerator.checked_rem(coefficient),
    ) else {
        return vec![];
    };
    if remainder != 0 && (remainder < 0) != (coefficient < 0) {
        let Some(floor) = root.checked_sub(1) else {
            return vec![];
        };
        root = floor;
    }
    [Some(root), root.checked_add(1)]
        .into_iter()
        .flatten()
        .filter_map(|value| i64::try_from(value).ok().map(|value| (axis, value)))
        .collect()
}

#[derive(Debug)]
pub(super) struct ArithmeticUnsat([u8; 32]);

impl ArithmeticUnsat {
    pub(super) fn digest(&self) -> [u8; 32] {
        self.0
    }

    fn for_script(script: &str) -> Self {
        let mut hash = Sha256::new();
        hash.update(b"futuruna.checked-integer-smt-unsat.v1\0");
        hash.update(script.as_bytes());
        Self(hash.finalize().into())
    }
}

#[derive(Debug)]
pub(super) enum SolverResult {
    Unsat(ArithmeticUnsat),
    Sat,
    Unknown,
    Unavailable,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug)]
enum Term {
    Free,
    Affine([i128; MAX_AXES], i128, i128),
    Add(Id, Id),
    Sub(Id, Id),
    Scale(Id, i128),
    Divide(Id, i128),
    Clamp(Id, i128, ClampKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClampKind {
    Maximum,
    Minimum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnaryOperation {
    PositiveDivision(i128),
    Clamp(i128, ClampKind),
}

#[derive(Clone, Debug)]
struct PairedUnary {
    operation: UnaryOperation,
    input: String,
    output: Id,
}

/// Minted together by the checked DAG emitter. Neither a caller-supplied
/// statement/digest pair nor parsed SMT can manufacture an entailed input.
struct ArithmeticObligation {
    canonical: String,
    unary: Option<Vec<PairedUnary>>,
}

impl ArithmeticObligation {
    fn paired_facts(&self) -> Option<Vec<String>> {
        let unary = self.unary.as_ref()?;
        if unary.len() > MAX_PAIRED_UNARY || self.canonical.len() > MAX_SCRIPT_BYTES {
            return None;
        }
        let mut facts = Vec::new();
        let mut bytes = self.canonical.len();
        // At most 128*127/2 comparisons, even when no operations match.
        for (index, a) in unary.iter().enumerate() {
            for b in &unary[index + 1..] {
                if a.operation != b.operation {
                    continue;
                }
                if facts.len() / 2 == MAX_PAIRED_FACTS {
                    return None;
                }
                let dx = format!("(- {} {})", a.input, b.input);
                let dy = format!("(- {} {})", name(a.output), name(b.output));
                let (positive, negative) = match a.operation {
                    UnaryOperation::PositiveDivision(d) if d > 0 => {
                        // For x>=y and d>0, signed truncation is monotone and
                        // its difference is at most ceil((x-y)/d). Crossing
                        // zero can reduce that difference, never increase it.
                        // Do NOT use the floor-division lower bound across zero.
                        let slack = number(d.checked_sub(1)?);
                        (
                            format!("(<= (* {} {dy}) (+ {dx} {slack}))", number(d)),
                            format!("(>= (* {} {dy}) (- {dx} {slack}))", number(d)),
                        )
                    }
                    UnaryOperation::Clamp(..) => {
                        // Equal-kind, equal-threshold clamps preserve order
                        // and cannot increase the distance between inputs.
                        (format!("(<= {dy} {dx})"), format!("(>= {dy} {dx})"))
                    }
                    _ => return None,
                };
                for fact in [
                    format!("(=> (>= {dx} 0) (and (>= {dy} 0) {positive}))"),
                    format!("(=> (<= {dx} 0) (and (<= {dy} 0) {negative}))"),
                ] {
                    bytes = bytes.checked_add(fact.len())?.checked_add(10)?;
                    if bytes > MAX_SCRIPT_BYTES {
                        return None;
                    }
                    facts.push(fact);
                }
            }
        }
        (!facts.is_empty()).then_some(facts)
    }

    fn paired_script(&self) -> Option<String> {
        let facts = self.paired_facts()?;
        let mut script = self.canonical.strip_suffix("(check-sat)\n")?.to_owned();
        for fact in facts {
            script.push_str(&format!("(assert {fact})\n"));
        }
        script.push_str("(check-sat)\n");
        (script.len() <= MAX_SCRIPT_BYTES).then_some(script)
    }

    fn solve_paired(&self, deadline: std::time::Instant) -> Option<SolverResult> {
        if std::time::Instant::now() >= deadline {
            return Some(SolverResult::Unknown);
        }
        let script = self.paired_script()?;
        Some(match solve_script(&script, true, deadline) {
            // Every appended fact follows from definitions emitted into this
            // same immutable obligation. UNSAT therefore proves the original
            // statement, whose receipt identity must not depend on this
            // optional preprocessing. No external digest can be substituted.
            SolverResult::Unsat(_) => {
                SolverResult::Unsat(ArithmeticUnsat::for_script(&self.canonical))
            }
            result => result,
        })
    }
}

impl Term {
    fn children(&self) -> Vec<Id> {
        match self {
            Self::Add(a, b) | Self::Sub(a, b) => vec![*a, *b],
            Self::Scale(a, _) | Self::Divide(a, _) | Self::Clamp(a, _, _) => vec![*a],
            _ => Vec::new(),
        }
    }
}

#[derive(Default)]
pub(super) struct ArithmeticTrace {
    terms: BTreeMap<Id, Term>,
}

impl ArithmeticTrace {
    /// Called only after strict checked dispatch has established this exact
    /// rule shape. Enclosures remain local; the DAG records only the operation
    /// on the original call input, never a branch-local bound or assumption.
    pub(super) fn observe_clamp(
        &mut self,
        input: IntInterval,
        constant: i128,
        kind: ClampKind,
        mut result: IntInterval,
    ) -> Option<IntInterval> {
        if result.singleton_value().is_some() {
            return None;
        }
        let child = self.intern(input)?;
        let mut hash = Sha256::new();
        hash.update(b"futuruna.checked-integer-clamp.v1\0");
        hash.update(child);
        hash.update(constant.to_le_bytes());
        hash.update([match kind {
            ClampKind::Maximum => 0,
            ClampKind::Minimum => 1,
        }]);
        result.correlation =
            Correlation::opaque(hash.finalize().into(), (result.minimum, result.maximum));
        let output = self.intern(result)?;
        if output == child {
            return None;
        }
        match self.terms.get(&output) {
            Some(Term::Free) => {
                self.terms
                    .insert(output, Term::Clamp(child, constant, kind));
            }
            Some(Term::Clamp(..)) => {}
            _ => return None,
        }
        Some(result)
    }

    fn intern(&mut self, value: IntInterval) -> Option<Id> {
        let symbolic = value.symbolic()?;
        let id = symbolic.expression_id();
        if !self.terms.contains_key(&id) {
            if self.terms.len() >= MAX_NODES {
                return None;
            }
            let term = symbolic
                .exact_form()
                .map_or(Term::Free, |(a, c, d)| Term::Affine(a, c, d));
            self.terms.insert(id, term);
        }
        Some(id)
    }

    pub(super) fn observe(
        &mut self,
        operator: &str,
        left: IntInterval,
        right: IntInterval,
        result: IntInterval,
    ) {
        let (Some(a), Some(b), Some(output)) =
            (self.intern(left), self.intern(right), self.intern(result))
        else {
            return;
        };
        // Exact affine forms already have their full semantics. Identity
        // operations must not introduce self-referential graph edges.
        if output == a || output == b || !matches!(self.terms.get(&output), Some(Term::Free)) {
            return;
        }
        let term = match operator {
            "+" => Term::Add(a, b),
            "-" => Term::Sub(a, b),
            "*" => match (
                left.correlation,
                right.singleton_value(),
                right.correlation,
                left.singleton_value(),
            ) {
                // Mirror checked_mul's choice exactly, including narrowed
                // singletons that still carry a nonconstant symbolic identity.
                (Some(_), Some(c), _, _) => Term::Scale(a, i128::from(c)),
                (_, _, Some(_), Some(c)) => Term::Scale(b, i128::from(c)),
                _ => return,
            },
            "/" => match right.singleton_value() {
                Some(c) if c != 0 => Term::Divide(a, i128::from(c)),
                _ => return,
            },
            _ => return,
        };
        self.terms.insert(output, term);
    }

    #[cfg(test)]
    pub(super) fn counterexample_script(
        &mut self,
        operator: &str,
        left: IntInterval,
        right: IntInterval,
        axes: &[(i64, i64)],
    ) -> Option<String> {
        self.script(operator, left, right, axes, true, 2000)
    }

    #[cfg(test)]
    fn script(
        &mut self,
        operator: &str,
        left: IntInterval,
        right: IntInterval,
        axes: &[(i64, i64)],
        expected: bool,
        timeout_ms: u64,
    ) -> Option<String> {
        self.obligation(operator, left, right, axes, expected, timeout_ms)
            .map(|obligation| obligation.canonical)
    }

    fn obligation(
        &mut self,
        operator: &str,
        left: IntInterval,
        right: IntInterval,
        axes: &[(i64, i64)],
        expected: bool,
        timeout_ms: u64,
    ) -> Option<ArithmeticObligation> {
        if !matches!(operator, "<" | "<=" | ">" | ">=") || axes.len() > MAX_AXES {
            return None;
        }
        let (a, b) = (self.intern(left)?, self.intern(right)?);
        let mut pending = vec![(a, false), (b, false)];
        let mut visiting = BTreeSet::new();
        let mut emitted = BTreeSet::new();
        let mut ordered = Vec::new();
        while let Some((id, finishing)) = pending.pop() {
            if emitted.contains(&id) {
                continue;
            }
            if finishing {
                visiting.remove(&id);
                emitted.insert(id);
                ordered.push(id);
                if ordered.len() > MAX_REACHABLE {
                    return None;
                }
            } else {
                if !visiting.insert(id) {
                    return None;
                }
                pending.push((id, true));
                for child in self.terms.get(&id)?.children() {
                    pending.push((child, false));
                }
            }
        }
        let mut script =
            format!("(set-option :timeout {timeout_ms})\n(set-option :memory_max_size 128)\n");
        for axis in 0..MAX_AXES {
            script.push_str(&format!("(declare-fun a{axis} () Int)\n"));
            if let Some(&(low, high)) = axes.get(axis) {
                script.push_str(&format!(
                    "(assert (and (<= {} a{axis}) (<= a{axis} {})))\n",
                    number(i128::from(low)),
                    number(i128::from(high))
                ));
            }
        }
        let mut unary = Some(Vec::new());
        for id in ordered {
            let term = self.terms.get(&id)?;
            let body = match term {
                Term::Free => {
                    script.push_str(&format!("(declare-fun {} () Int)\n", name(id)));
                    continue;
                }
                Term::Affine(coefficients, constant, denominator) => {
                    let sum = affine_sum(coefficients, *constant);
                    truncate(&sum, *denominator)?
                }
                Term::Add(x, y) => format!("(+ {} {})", name(*x), name(*y)),
                Term::Sub(x, y) => format!("(- {} {})", name(*x), name(*y)),
                Term::Scale(x, c) => format!("(* {} {})", name(*x), number(*c)),
                Term::Divide(x, d) => truncate(&name(*x), *d)?,
                Term::Clamp(x, c, kind) => {
                    let operator = match kind {
                        ClampKind::Maximum => ">",
                        ClampKind::Minimum => "<",
                    };
                    format!(
                        "(ite ({operator} {} {}) {} {})",
                        name(*x),
                        number(*c),
                        name(*x),
                        number(*c)
                    )
                }
            };
            script.push_str(&format!("(define-fun {} () Int {body})\n", name(id)));
            if script.len() > MAX_SCRIPT_BYTES {
                return None;
            }
            // Record only the exact operation just emitted, never the abstract
            // enclosure or a branch-local assumption. Negative/zero divisors
            // get no paired facts; their original semantics are untouched.
            let candidate = match term {
                Term::Affine(coefficients, constant, denominator) if *denominator > 1 => Some((
                    UnaryOperation::PositiveDivision(*denominator),
                    affine_sum(coefficients, *constant),
                )),
                Term::Divide(input, divisor) if *divisor > 0 => {
                    Some((UnaryOperation::PositiveDivision(*divisor), name(*input)))
                }
                Term::Clamp(input, constant, kind) => {
                    Some((UnaryOperation::Clamp(*constant, *kind), name(*input)))
                }
                _ => None,
            };
            if let (Some(terms), Some((operation, input))) = (unary.as_mut(), candidate) {
                if terms.len() == MAX_PAIRED_UNARY {
                    unary = None;
                } else {
                    terms.push(PairedUnary {
                        operation,
                        input,
                        output: id,
                    });
                }
            }
        }
        let predicate = format!("({operator} {} {})", name(a), name(b));
        let counterexample = if expected {
            format!("(not {predicate})")
        } else {
            predicate
        };
        script.push_str(&format!("(assert {counterexample})\n(check-sat)\n"));
        Some(ArithmeticObligation {
            canonical: script,
            unary,
        })
    }

    pub(super) fn prove(
        &mut self,
        operator: &str,
        left: IntInterval,
        right: IntInterval,
        axes: &[(i64, i64)],
        expected: bool,
    ) -> SolverResult {
        use std::time::{Duration, Instant};
        let Some(obligation) = self.obligation(operator, left, right, axes, expected, 10_000)
        else {
            return SolverResult::Unsupported;
        };
        let started = Instant::now();
        // Try a short simplex prefix only for the stronger clamp recipe.
        // Default LRA can time out on these, but simplex can also time out on
        // old successful obligations. Retain the original engine's full 10s
        // opportunity within the existing 12s total process guard. The old
        // prefix still goes first; entailed preprocessing gets only the
        // remaining time up to 1.5s, leaving room for the original 10s engine.
        // All strategies prove the same canonical statement/receipt recipe.
        if self
            .terms
            .values()
            .any(|term| matches!(term, Term::Clamp(..)))
        {
            let result = solve_script(
                &obligation.canonical,
                true,
                started + Duration::from_secs(1),
            );
            if matches!(result, SolverResult::Unsat(_) | SolverResult::Sat) {
                return result;
            }
            if let Some(result @ (SolverResult::Unsat(_) | SolverResult::Sat)) =
                obligation.solve_paired(started + Duration::from_millis(1500))
            {
                return result;
            }
        }
        solve_script(
            &obligation.canonical,
            false,
            started + Duration::from_secs(12),
        )
    }
}

fn solve_script(script: &str, simplex: bool, deadline: std::time::Instant) -> SolverResult {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    if Instant::now() >= deadline {
        return SolverResult::Unknown;
    }
    let mut command = Command::new("z3");
    command.args(["-in", "-T:11"]);
    if simplex {
        command.arg("smt.arith.solver=2");
    }
    let mut child = match command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return SolverResult::Unavailable,
    };
    if child
        .stdin
        .take()
        .is_none_or(|mut input| input.write_all(script.as_bytes()).is_err())
    {
        let _ = child.kill();
        let _ = child.wait();
        return SolverResult::Failed;
    }
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return SolverResult::Unknown;
            }
        }
    }
    let Ok(output) = child.wait_with_output() else {
        return SolverResult::Failed;
    };
    if !output.status.success() || !output.stderr.is_empty() || output.stdout.len() > 32 {
        return SolverResult::Failed;
    }
    match output.stdout.as_slice() {
        b"unsat\n" | b"unsat\r\n" => SolverResult::Unsat(ArithmeticUnsat::for_script(script)),
        b"sat\n" | b"sat\r\n" => SolverResult::Sat,
        b"unknown\n" | b"unknown\r\n" => SolverResult::Unknown,
        _ => SolverResult::Failed,
    }
}

fn affine_sum(coefficients: &[i128; MAX_AXES], constant: i128) -> String {
    let mut summands = vec![number(constant)];
    for (axis, coefficient) in coefficients.iter().enumerate().filter(|(_, c)| **c != 0) {
        summands.push(format!("(* {} a{axis})", number(*coefficient)));
    }
    format!("(+ {})", summands.join(" "))
}

fn name(id: Id) -> String {
    format!(
        "n{}",
        id.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn number(value: i128) -> String {
    if value < 0 {
        format!("(- {})", value.unsigned_abs())
    } else {
        value.to_string()
    }
}

fn truncate(value: &str, divisor: i128) -> Option<String> {
    if divisor == 0 {
        return None;
    }
    if divisor == 1 {
        return Some(value.to_string());
    }
    let magnitude = divisor.unsigned_abs();
    let result =
        format!("(ite (>= {value} 0) (div {value} {magnitude}) (- (div (- {value}) {magnitude})))");
    Some(if divisor < 0 {
        format!("(- {result})")
    } else {
        result
    })
}

#[cfg(test)]
mod tests {
    use super::super::Correlation;
    use super::*;

    fn solve(script: &str) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("z3")
            .args(["-in", "-T:3"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("focused solver measurement requires installed z3");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn paired_fixture() -> ArithmeticObligation {
        let mut trace = ArithmeticTrace::default();
        let axis = |index| IntInterval {
            minimum: -20_000,
            maximum: 20_000,
            correlation: Some(Correlation::axis(index).unwrap()),
        };
        let (x, y) = (axis(0), axis(1));
        let mut sums = [IntInterval::singleton(0); 2];
        for (index, input) in [x, y].into_iter().enumerate() {
            for divisor in [2, 100, 10_000] {
                let d = IntInterval::singleton(divisor);
                let value = input.checked_div(d).unwrap();
                trace.observe("/", input, d, value);
                let next = sums[index].checked_add(value).unwrap();
                trace.observe("+", sums[index], value, next);
                sums[index] = next;
            }
            for kind in [ClampKind::Maximum, ClampKind::Minimum] {
                for threshold in [-3, 0, 7] {
                    let value = trace
                        .observe_clamp(
                            input,
                            threshold,
                            kind,
                            IntInterval {
                                correlation: None,
                                ..input
                            },
                        )
                        .unwrap();
                    let next = sums[index].checked_add(value).unwrap();
                    trace.observe("+", sums[index], value, next);
                    sums[index] = next;
                }
            }
        }
        // No source bounds: the emitted facts must hold for arbitrary signed
        // inputs, not just for the intervals used to construct this fixture.
        trace
            .obligation(">=", sums[0], sums[1], &[], true, 1000)
            .unwrap()
    }

    #[test]
    fn paired_integer_augmentation_is_bounded_and_canonical() {
        let mut obligation = paired_fixture();
        assert_eq!(obligation.unary.as_ref().unwrap().len(), 18);
        assert_eq!(obligation.paired_facts().unwrap().len(), 18);
        let original = obligation.canonical.clone();
        let augmented = obligation.paired_script().unwrap();
        assert!(augmented.starts_with(original.strip_suffix("(check-sat)\n").unwrap()));
        assert_eq!(obligation.canonical, original);
        assert!(!augmented.contains("(<= (- 20000) a0)"));
        assert_eq!(paired_fixture().canonical, original);
        assert_eq!(paired_fixture().paired_script().unwrap(), augmented);

        let sample = obligation.unary.as_ref().unwrap()[0].clone();
        // Exact function parameters and clamp kinds must agree. Negative or
        // zero divisors are unsupported, never extra assumptions.
        for operation in [
            UnaryOperation::PositiveDivision(0),
            UnaryOperation::PositiveDivision(-2),
        ] {
            obligation.unary = Some(vec![
                PairedUnary {
                    operation,
                    ..sample.clone()
                };
                2
            ]);
            assert!(obligation.paired_facts().is_none());
        }
        obligation.unary = Some(vec![
            PairedUnary {
                operation: UnaryOperation::Clamp(0, ClampKind::Maximum),
                ..sample.clone()
            },
            PairedUnary {
                operation: UnaryOperation::Clamp(1, ClampKind::Maximum),
                ..sample.clone()
            },
            PairedUnary {
                operation: UnaryOperation::Clamp(0, ClampKind::Minimum),
                ..sample.clone()
            },
        ]);
        assert!(obligation.paired_facts().is_none());
        obligation.unary = Some(vec![sample.clone(); MAX_PAIRED_UNARY + 1]);
        assert!(obligation.paired_facts().is_none());
        // 24 equal unary operations require 276 pairs, above the 256 cap.
        obligation.unary = Some(vec![sample.clone(); 24]);
        assert!(obligation.paired_facts().is_none());
        obligation.unary = Some(vec![sample; 2]);
        obligation.canonical = "x".repeat(MAX_SCRIPT_BYTES);
        assert!(obligation.paired_facts().is_none());
        assert!(matches!(
            obligation.solve_paired(std::time::Instant::now()),
            Some(SolverResult::Unknown)
        ));
    }

    #[test]
    #[ignore = "explicit installed-solver paired arithmetic edge"]
    fn paired_integer_facts_signed_sat_and_receipt_replay() {
        use std::time::{Duration, Instant};
        let obligation = paired_fixture();
        let definitions = obligation.canonical.rsplit_once("(assert ").unwrap().0;
        let facts = obligation.paired_facts().unwrap();
        let counterexample = format!(
            "{definitions}(assert (not (and {})))\n(check-sat)\n",
            facts.join(" ")
        );
        // Includes negative inputs, equality, sign crossings, both orderings,
        // multiple denominators, and positive/zero/negative clamp thresholds.
        assert_eq!(solve(&counterexample).trim(), "unsat");
        // Independent inputs may reverse order: no equal-enclosure shortcut
        // may turn these real relaxed witnesses into an UNSAT receipt.
        assert_eq!(solve(&obligation.canonical).trim(), "sat");
        assert_eq!(solve(&obligation.paired_script().unwrap()).trim(), "sat");

        let monotone = || {
            let mut trace = ArithmeticTrace::default();
            let before = IntInterval {
                minimum: -20_000,
                maximum: 20_000,
                correlation: Some(Correlation::axis(0).unwrap()),
            };
            let after = before.checked_add(IntInterval::singleton(1)).unwrap();
            let mut outputs = Vec::new();
            for input in [before, after] {
                let d = IntInterval::singleton(100);
                let quotient = input.checked_div(d).unwrap();
                trace.observe("/", input, d, quotient);
                outputs.push(
                    trace
                        .observe_clamp(
                            quotient,
                            0,
                            ClampKind::Maximum,
                            IntInterval {
                                minimum: 0,
                                maximum: 201,
                                correlation: None,
                            },
                        )
                        .unwrap(),
                );
            }
            let obligation = trace
                .obligation(
                    ">=",
                    outputs[1],
                    outputs[0],
                    &[(-20_000, 20_000)],
                    true,
                    10_000,
                )
                .unwrap();
            (trace, outputs, obligation)
        };
        let (mut trace, outputs, obligation) = monotone();
        let SolverResult::Unsat(original) = solve_script(
            &obligation.canonical,
            false,
            Instant::now() + Duration::from_secs(2),
        ) else {
            panic!("original monotone statement must close")
        };
        let Some(SolverResult::Unsat(paired)) =
            obligation.solve_paired(Instant::now() + Duration::from_secs(2))
        else {
            panic!("entailed preprocessing must close")
        };
        let SolverResult::Unsat(preferred) =
            trace.prove(">=", outputs[1], outputs[0], &[(-20_000, 20_000)], true)
        else {
            panic!("production policy must close")
        };
        assert_eq!(paired.digest(), original.digest());
        assert_eq!(preferred.digest(), original.digest());
        let (_, _, cold) = monotone();
        assert_eq!(cold.canonical, obligation.canonical);
        let Some(SolverResult::Unsat(replayed)) =
            cold.solve_paired(Instant::now() + Duration::from_secs(2))
        else {
            panic!("fresh checked DAG must re-derive the receipt")
        };
        assert_eq!(replayed.digest(), original.digest());
    }

    #[test]
    #[ignore = "explicit installed-solver arithmetic edge, not a mandatory external dependency"]
    fn checked_arithmetic_trace_signed_rounding_and_distinct_opaque_values() {
        let mut trace = ArithmeticTrace::default();
        let x = IntInterval {
            minimum: -101,
            maximum: 101,
            correlation: Some(Correlation::axis(0).unwrap()),
        };
        let shifted = x.checked_add(IntInterval::singleton(24)).unwrap();
        assert_eq!(
            split_hints(shifted, IntInterval::singleton(0), 1),
            vec![(0, -24), (0, -23)]
        );
        assert!(split_hints(shifted, IntInterval::singleton(0), 0).is_empty());
        for divisor in [25, -25] {
            let d = IntInterval::singleton(divisor);
            let q = x.checked_div(d).unwrap();
            trace.observe("/", x, d, q);
            let script = trace
                .counterexample_script(">=", q, IntInterval::singleton(-4), &[(-101, 101)])
                .unwrap();
            assert_eq!(solve(&script).trim(), "unsat");
            let script = trace
                .counterexample_script(">=", q, IntInterval::singleton(-3), &[(-101, 101)])
                .unwrap();
            assert_eq!(solve(&script).trim(), "sat");
        }
        let opaque = |id| IntInterval {
            minimum: 0,
            maximum: 100,
            correlation: Correlation::opaque([id; 32], (0, 100)),
        };
        let a = opaque(1);
        let b = opaque(2);
        let a1 = a.checked_add(IntInterval::singleton(1)).unwrap();
        trace.observe("+", a, IntInterval::singleton(1), a1);
        assert!(matches!(
            trace.prove("<", a1, a, &[], false),
            SolverResult::Unsat(_)
        ));
        assert!(matches!(
            trace.prove("<", a1, b, &[], false),
            SolverResult::Sat
        ));
        assert!(matches!(
            trace.prove("==", a1, a, &[], false),
            SolverResult::Unsupported
        ));
        assert_eq!(
            solve(&trace.counterexample_script(">=", a1, a, &[]).unwrap()).trim(),
            "unsat"
        );
        assert_eq!(
            solve(&trace.counterexample_script(">=", a1, b, &[]).unwrap()).trim(),
            "sat"
        );

        // The short clamp strategy and the retained legacy engine must commit
        // the exact same statement, not engine-specific receipt identities.
        let clamp = trace
            .observe_clamp(
                x,
                0,
                ClampKind::Maximum,
                IntInterval {
                    minimum: 0,
                    maximum: 101,
                    correlation: None,
                },
            )
            .unwrap();
        let successor = clamp.checked_add(IntInterval::singleton(1)).unwrap();
        trace.observe("+", clamp, IntInterval::singleton(1), successor);
        let script = trace
            .script(">=", successor, clamp, &[(-101, 101)], true, 10_000)
            .unwrap();
        let SolverResult::Unsat(preferred) =
            trace.prove(">=", successor, clamp, &[(-101, 101)], true)
        else {
            panic!("clamp strategy must prove a strict successor");
        };
        let SolverResult::Unsat(legacy) = solve_script(
            &script,
            false,
            std::time::Instant::now() + std::time::Duration::from_secs(12),
        ) else {
            panic!("legacy strategy must prove the same statement");
        };
        assert_eq!(preferred.digest(), legacy.digest());
        assert!(matches!(
            solve_script(&script, true, std::time::Instant::now()),
            SolverResult::Unknown
        ));
    }
}
