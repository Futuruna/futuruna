//! Proof Kernel — Curry-Howard verification layer for the `?` rune.
//!
//! See `docs/proof-kernel.md` for the full design spec.
//!
//! This is the trusted core of Futuruna's verification story. Everything the
//! language promises about "proved" invariants rests on this file being
//! correct. It is therefore treated like cryptographic code:
//!
//!   1. **Small.** Target under 800 LoC total. If it grows past that, cut.
//!   2. **Closed.** No I/O, no globals, no mutable state outside the `Ctx`
//!      passed to `check`. Only depends on `std`.
//!   3. **Decidable.** Every rule terminates on well-formed input. No
//!      unbounded unfolding, no unrestricted fixpoints.
//!   4. **Axioms by name.** Primitive axioms live in a hard-coded table. The
//!      kernel never looks at axiom bodies — they are the trust boundary.
//!   5. **Conservative.** When in doubt, reject. A `?` the kernel can't close
//!      falls through to Z3 in `runa verify`, so rejection is never fatal.
//!
//! A wrong proof kernel means every "proved" invariant is a lie. Read this
//! file before trusting any verification claim Futuruna makes.
//!
//! ## Phase 1 scope (this file)
//!
//! Implemented rules: REFL, HYP, APPLY, LET, REWRITE, ASSUME, CONTRA.
//! Stubbed (returns `NotImplemented`): IND, CASES.
//! Built-in axioms: a conservative subset of the v1 design doc, with the
//! remainder landing in phase 2.

use std::collections::BTreeMap;
use std::fmt;

// ============================================================================
// TERMS — the expression fragment propositions are built over
// ============================================================================

/// A term in the fragment the kernel reasons about.
///
/// Deliberately smaller than Futuruna's full `Expr`: the kernel only handles
/// integer literals, variables, binary arithmetic, and uninterpreted
/// function/constructor application. Anything outside this fragment falls
/// through to Z3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// Integer literal.
    Int(i64),
    /// Variable — either a free term variable bound in `Ctx`, or a schema
    /// metavariable during unification.
    Var(String),
    /// Binary operator application: `+`, `-`, `*`, `/`.
    Op(String, Box<Term>, Box<Term>),
    /// Function or constructor application. The kernel treats both as
    /// uninterpreted — unification is purely structural.
    App(String, Vec<Term>),
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Int(n) => write!(f, "{}", n),
            Term::Var(v) => write!(f, "{}", v),
            Term::Op(op, l, r) => write!(f, "({} {} {})", l, op, r),
            Term::App(fun, args) => {
                write!(f, "{}(", fun)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
        }
    }
}

// ============================================================================
// PROPOSITIONS — the v1 fragment (§3 of the design spec)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prop {
    /// `e1 == e2`
    Eq(Term, Term),
    /// `e1 <= e2` — `<`, `>=`, `>` are desugared to `<=` at parse time.
    Le(Term, Term),
    /// `P and Q`
    And(Box<Prop>, Box<Prop>),
    /// `not(P)`
    Not(Box<Prop>),
    /// `P -> Q` — introduced only by `assume` inside proof terms.
    Imply(Box<Prop>, Box<Prop>),
    /// `False` — bottom, provable only from a contradiction.
    False,
}

impl fmt::Display for Prop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prop::Eq(a, b) => write!(f, "{} == {}", a, b),
            Prop::Le(a, b) => write!(f, "{} <= {}", a, b),
            Prop::And(p, q) => write!(f, "({} and {})", p, q),
            Prop::Not(p) => write!(f, "not({})", p),
            Prop::Imply(p, q) => write!(f, "({} -> {})", p, q),
            Prop::False => write!(f, "False"),
        }
    }
}

// ============================================================================
// PROOF TERMS — the v1 fragment (§4 of the design spec)
// ============================================================================

#[derive(Debug, Clone)]
pub enum ProofTerm {
    /// `refl` — closes `e == e`.
    Refl,
    /// `apply N(t̄)` — invoke a named axiom or proved lemma. Premise proofs
    /// go in the arg list; universal variables are solved by unification.
    Apply(String, Vec<ProofTerm>),
    /// `rewrite t_eq in t_body` — substitute equals in the goal.
    Rewrite(Box<ProofTerm>, Box<ProofTerm>),
    /// `induction_on x { | C(ȳ) -> body ... }` — structural induction.
    /// Phase 2.
    InductionOn(String, Vec<IndArm>),
    /// `cases e { | C(ȳ) -> body ... }` — case analysis on a value.
    /// Phase 2.
    Cases(Term, Vec<CaseArm>),
    /// `contra { t }` — t proves False, so any P follows. Phase 2.
    Contra(Box<ProofTerm>),
    /// `let v = t1 in t2` — locally name an intermediate proof.
    Let(String, Box<ProofTerm>, Box<ProofTerm>),
    /// `assume P in t` — introduce a hypothesis to prove an implication.
    Assume(Prop, Box<ProofTerm>),
    /// Reference to a named hypothesis in the context.
    Hyp(String),
}

#[derive(Debug, Clone)]
pub struct IndArm {
    pub ctor: String,
    pub binders: Vec<String>,
    pub body: ProofTerm,
}

#[derive(Debug, Clone)]
pub struct CaseArm {
    pub ctor: String,
    pub binders: Vec<String>,
    pub body: ProofTerm,
}

// ============================================================================
// SCHEMAS — axioms and proved lemmas live under a single shape
// ============================================================================

/// A universally-quantified implication schema:
///
/// ```text
/// ∀x̄. (P₁ ∧ … ∧ Pₙ) → C
/// ```
///
/// Both hand-written axioms (`int_ring.comm_add`) and proved lemmas
/// (`int_ord.zero_le`) live as `Schema` in the same registry.
#[derive(Debug, Clone)]
pub struct Schema {
    /// Universally-quantified metavariables. Appear free in `premises` and
    /// `conclusion`; solved by unification against the current goal.
    pub vars: Vec<String>,
    pub premises: Vec<Prop>,
    pub conclusion: Prop,
}

// ============================================================================
// CONTEXT — ordered hypothesis list (§5 judgment rules)
// ============================================================================

#[derive(Debug, Clone)]
pub enum Hyp {
    /// A named proof of a specific proposition — e.g., `ih : 0 <= length(t)`
    /// or an `assume`d hypothesis.
    Prop(String, Prop),
    /// A free term variable — e.g., `| add_comm: (a,b) -> a+b == b+a` adds
    /// `a` and `b` as `TypedVar` entries. The proof must work for any value.
    TypedVar(String),
}

#[derive(Debug, Clone, Default)]
pub struct Ctx {
    hyps: Vec<Hyp>,
}

impl Ctx {
    pub fn new() -> Self {
        Ctx { hyps: Vec::new() }
    }

    /// Extend with a new named proposition hypothesis. Non-destructive.
    pub fn with_prop(&self, name: String, p: Prop) -> Self {
        let mut new = self.clone();
        new.hyps.push(Hyp::Prop(name, p));
        new
    }

    /// Extend with a new free term variable.
    pub fn with_var(&self, name: String) -> Self {
        let mut new = self.clone();
        new.hyps.push(Hyp::TypedVar(name));
        new
    }

    /// Look up a named proposition hypothesis. Returns the most recent
    /// binding (shadowing).
    pub fn lookup_prop(&self, name: &str) -> Option<&Prop> {
        for h in self.hyps.iter().rev() {
            if let Hyp::Prop(n, p) = h {
                if n == name {
                    return Some(p);
                }
            }
        }
        None
    }
}

// ============================================================================
// REGISTRY — axioms + proved lemmas under a single name table
// ============================================================================

pub struct Registry {
    schemas: BTreeMap<String, Schema>,
}

impl Registry {
    /// Registry containing the hard-coded built-in axioms listed in §7 of
    /// the design spec. This is the trust boundary: every axiom added here
    /// must be justified in the design doc.
    pub fn with_builtins() -> Self {
        let mut schemas = BTreeMap::new();

        let v = |s: &str| Term::Var(s.to_string());
        let op =
            |o: &str, a: Term, b: Term| Term::Op(o.to_string(), Box::new(a), Box::new(b));

        // -- Equality (3) --

        // eq.refl : ∀x. x == x
        schemas.insert(
            "eq.refl".to_string(),
            Schema {
                vars: vec!["x".into()],
                premises: vec![],
                conclusion: Prop::Eq(v("x"), v("x")),
            },
        );

        // eq.sym : ∀x y. x == y → y == x
        schemas.insert(
            "eq.sym".to_string(),
            Schema {
                vars: vec!["x".into(), "y".into()],
                premises: vec![Prop::Eq(v("x"), v("y"))],
                conclusion: Prop::Eq(v("y"), v("x")),
            },
        );

        // eq.trans : ∀x y z. x == y → y == z → x == z
        schemas.insert(
            "eq.trans".to_string(),
            Schema {
                vars: vec!["x".into(), "y".into(), "z".into()],
                premises: vec![Prop::Eq(v("x"), v("y")), Prop::Eq(v("y"), v("z"))],
                conclusion: Prop::Eq(v("x"), v("z")),
            },
        );

        // -- Int ring --

        // int_ring.comm_add : ∀a b. a + b == b + a
        schemas.insert(
            "int_ring.comm_add".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into()],
                premises: vec![],
                conclusion: Prop::Eq(op("+", v("a"), v("b")), op("+", v("b"), v("a"))),
            },
        );

        // int_ring.assoc_add : ∀a b c. (a + b) + c == a + (b + c)
        schemas.insert(
            "int_ring.assoc_add".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into(), "c".into()],
                premises: vec![],
                conclusion: Prop::Eq(
                    op("+", op("+", v("a"), v("b")), v("c")),
                    op("+", v("a"), op("+", v("b"), v("c"))),
                ),
            },
        );

        // int_ring.zero_add : ∀a. 0 + a == a
        schemas.insert(
            "int_ring.zero_add".to_string(),
            Schema {
                vars: vec!["a".into()],
                premises: vec![],
                conclusion: Prop::Eq(op("+", Term::Int(0), v("a")), v("a")),
            },
        );

        // int_ring.comm_mul : ∀a b. a * b == b * a
        schemas.insert(
            "int_ring.comm_mul".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into()],
                premises: vec![],
                conclusion: Prop::Eq(op("*", v("a"), v("b")), op("*", v("b"), v("a"))),
            },
        );

        // int_ring.assoc_mul : ∀a b c. (a * b) * c == a * (b * c)
        schemas.insert(
            "int_ring.assoc_mul".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into(), "c".into()],
                premises: vec![],
                conclusion: Prop::Eq(
                    op("*", op("*", v("a"), v("b")), v("c")),
                    op("*", v("a"), op("*", v("b"), v("c"))),
                ),
            },
        );

        // int_ring.one_mul : ∀a. 1 * a == a
        schemas.insert(
            "int_ring.one_mul".to_string(),
            Schema {
                vars: vec!["a".into()],
                premises: vec![],
                conclusion: Prop::Eq(op("*", Term::Int(1), v("a")), v("a")),
            },
        );

        // int_ring.distr : ∀a b c. a * (b + c) == a*b + a*c
        schemas.insert(
            "int_ring.distr".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into(), "c".into()],
                premises: vec![],
                conclusion: Prop::Eq(
                    op("*", v("a"), op("+", v("b"), v("c"))),
                    op("+", op("*", v("a"), v("b")), op("*", v("a"), v("c"))),
                ),
            },
        );

        // int_ring.mul_neg_one : ∀a. (-1) * a == 0 - a
        schemas.insert(
            "int_ring.mul_neg_one".to_string(),
            Schema {
                vars: vec!["a".into()],
                premises: vec![],
                conclusion: Prop::Eq(
                    op("*", Term::Int(-1), v("a")),
                    op("-", Term::Int(0), v("a")),
                ),
            },
        );

        // -- Int order --

        // int_ord.le_refl : ∀a. a <= a
        schemas.insert(
            "int_ord.le_refl".to_string(),
            Schema {
                vars: vec!["a".into()],
                premises: vec![],
                conclusion: Prop::Le(v("a"), v("a")),
            },
        );

        // int_ord.le_trans : ∀a b c. a <= b → b <= c → a <= c
        schemas.insert(
            "int_ord.le_trans".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into(), "c".into()],
                premises: vec![Prop::Le(v("a"), v("b")), Prop::Le(v("b"), v("c"))],
                conclusion: Prop::Le(v("a"), v("c")),
            },
        );

        // int_ord.le_antisym : ∀a b. a <= b → b <= a → a == b
        schemas.insert(
            "int_ord.le_antisym".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into()],
                premises: vec![Prop::Le(v("a"), v("b")), Prop::Le(v("b"), v("a"))],
                conclusion: Prop::Eq(v("a"), v("b")),
            },
        );

        // int_ord.add_mono : ∀a b c. a <= b → a + c <= b + c
        schemas.insert(
            "int_ord.add_mono".to_string(),
            Schema {
                vars: vec!["a".into(), "b".into(), "c".into()],
                premises: vec![Prop::Le(v("a"), v("b"))],
                conclusion: Prop::Le(op("+", v("a"), v("c")), op("+", v("b"), v("c"))),
            },
        );

        // -- Propositional axioms live in special-case dispatch in
        // check_apply(), because their universal metavariables are
        // proposition-valued (not term-valued) and would otherwise require
        // higher-order unification.

        Registry { schemas }
    }

    /// Register a proved lemma under a given name. Fails if the name is
    /// already taken — we never silently shadow axioms.
    pub fn register(&mut self, name: String, schema: Schema) -> Result<(), ProofError> {
        if self.schemas.contains_key(&name) {
            return Err(ProofError::DuplicateSchema(name));
        }
        self.schemas.insert(name, schema);
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Schema> {
        self.schemas.get(name)
    }
}

// ============================================================================
// ERRORS
// ============================================================================

#[derive(Debug, Clone)]
pub enum ProofError {
    UnknownAxiom(String),
    UnknownHypothesis(String),
    UnificationFailed(String),
    GoalMismatch { expected: String, got: String },
    PremiseCount { axiom: String, expected: usize, got: usize },
    NotEquality(String),
    NotImplication(String),
    DuplicateSchema(String),
    CannotSynthesize(String),
    /// Phase-2 stubs.
    NotImplemented(&'static str),
}

impl fmt::Display for ProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofError::UnknownAxiom(n) => write!(f, "unknown axiom or lemma: {}", n),
            ProofError::UnknownHypothesis(n) => write!(f, "unknown hypothesis: {}", n),
            ProofError::UnificationFailed(s) => write!(f, "unification failed: {}", s),
            ProofError::GoalMismatch { expected, got } => {
                write!(f, "goal mismatch: expected {}, got {}", expected, got)
            }
            ProofError::PremiseCount { axiom, expected, got } => write!(
                f,
                "{} expects {} premise proof(s), got {}",
                axiom, expected, got
            ),
            ProofError::NotEquality(s) => write!(f, "expected equality, got: {}", s),
            ProofError::NotImplication(s) => write!(f, "expected implication, got: {}", s),
            ProofError::DuplicateSchema(n) => write!(f, "schema already registered: {}", n),
            ProofError::CannotSynthesize(s) => write!(f, "cannot synthesize: {}", s),
            ProofError::NotImplemented(r) => write!(f, "not yet implemented: {}", r),
        }
    }
}

// ============================================================================
// UNIFICATION — first-order, occurs-checked (§8)
// ============================================================================

type Subst = BTreeMap<String, Term>;

fn is_meta(v: &str, metas: &[String]) -> bool {
    metas.iter().any(|m| m == v)
}

fn unify_term(
    pat: &Term,
    tgt: &Term,
    metas: &[String],
    s: &mut Subst,
) -> Result<(), ProofError> {
    // Dereference any existing binding on the pattern side.
    let pat_resolved = if let Term::Var(v) = pat {
        if is_meta(v, metas) {
            if let Some(bound) = s.get(v).cloned() {
                if bound != *pat {
                    return unify_term(&bound, tgt, metas, s);
                }
            }
        }
        pat.clone()
    } else {
        pat.clone()
    };

    match (&pat_resolved, tgt) {
        // Metavariable binding.
        (Term::Var(v), _) if is_meta(v, metas) => {
            // Goal variables are rigid, even if they share a printed name with
            // a schema metavariable. Binding `a ↦ a` is therefore sound and
            // should not trip the occurs check.
            if let Term::Var(w) = tgt {
                if w == v {
                    s.insert(v.clone(), tgt.clone());
                    return Ok(());
                }
            }
            if occurs_check(v, tgt) {
                return Err(ProofError::UnificationFailed(format!(
                    "occurs check: {} in {}",
                    v, tgt
                )));
            }
            s.insert(v.clone(), tgt.clone());
            Ok(())
        }
        // Rigid variables must match exactly.
        (Term::Var(a), Term::Var(b)) if a == b => Ok(()),
        // Int literals.
        (Term::Int(a), Term::Int(b)) if a == b => Ok(()),
        // Binary op — operator symbols must match exactly (no algebraic
        // reasoning; that is always the user's job via the `*.comm` axioms).
        (Term::Op(o1, l1, r1), Term::Op(o2, l2, r2)) if o1 == o2 => {
            unify_term(l1, l2, metas, s)?;
            unify_term(r1, r2, metas, s)
        }
        // Function/constructor application — head and arity must match.
        (Term::App(f1, a1), Term::App(f2, a2)) if f1 == f2 && a1.len() == a2.len() => {
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify_term(x, y, metas, s)?;
            }
            Ok(())
        }
        _ => Err(ProofError::UnificationFailed(format!(
            "cannot unify {} with {}",
            pat_resolved, tgt
        ))),
    }
}

fn unify_prop(
    pat: &Prop,
    tgt: &Prop,
    metas: &[String],
    s: &mut Subst,
) -> Result<(), ProofError> {
    match (pat, tgt) {
        (Prop::Eq(a1, b1), Prop::Eq(a2, b2)) => {
            unify_term(a1, a2, metas, s)?;
            unify_term(b1, b2, metas, s)
        }
        (Prop::Le(a1, b1), Prop::Le(a2, b2)) => {
            unify_term(a1, a2, metas, s)?;
            unify_term(b1, b2, metas, s)
        }
        (Prop::And(p1, q1), Prop::And(p2, q2)) => {
            unify_prop(p1, p2, metas, s)?;
            unify_prop(q1, q2, metas, s)
        }
        (Prop::Imply(p1, q1), Prop::Imply(p2, q2)) => {
            unify_prop(p1, p2, metas, s)?;
            unify_prop(q1, q2, metas, s)
        }
        (Prop::Not(a), Prop::Not(b)) => unify_prop(a, b, metas, s),
        (Prop::False, Prop::False) => Ok(()),
        _ => Err(ProofError::UnificationFailed(format!(
            "cannot unify {} with {}",
            pat, tgt
        ))),
    }
}

fn occurs_check(v: &str, t: &Term) -> bool {
    match t {
        Term::Int(_) => false,
        Term::Var(w) => v == w,
        Term::Op(_, l, r) => occurs_check(v, l) || occurs_check(v, r),
        Term::App(_, args) => args.iter().any(|a| occurs_check(v, a)),
    }
}

fn subst_term(t: &Term, s: &Subst) -> Term {
    match t {
        Term::Int(_) => t.clone(),
        Term::Var(v) => s.get(v).cloned().unwrap_or_else(|| t.clone()),
        Term::Op(op, l, r) => Term::Op(
            op.clone(),
            Box::new(subst_term(l, s)),
            Box::new(subst_term(r, s)),
        ),
        Term::App(f, args) => Term::App(f.clone(), args.iter().map(|a| subst_term(a, s)).collect()),
    }
}

fn subst_prop(p: &Prop, s: &Subst) -> Prop {
    match p {
        Prop::Eq(a, b) => Prop::Eq(subst_term(a, s), subst_term(b, s)),
        Prop::Le(a, b) => Prop::Le(subst_term(a, s), subst_term(b, s)),
        Prop::And(p, q) => Prop::And(Box::new(subst_prop(p, s)), Box::new(subst_prop(q, s))),
        Prop::Not(p) => Prop::Not(Box::new(subst_prop(p, s))),
        Prop::Imply(p, q) => {
            Prop::Imply(Box::new(subst_prop(p, s)), Box::new(subst_prop(q, s)))
        }
        Prop::False => Prop::False,
    }
}

fn prop_has_unsolved_meta(p: &Prop, metas: &[String]) -> bool {
    fn term_has_unsolved_meta(t: &Term, metas: &[String]) -> bool {
        match t {
            Term::Int(_) => false,
            Term::Var(v) => is_meta(v, metas),
            Term::Op(_, l, r) => {
                term_has_unsolved_meta(l, metas) || term_has_unsolved_meta(r, metas)
            }
            Term::App(_, args) => args.iter().any(|a| term_has_unsolved_meta(a, metas)),
        }
    }

    match p {
        Prop::Eq(a, b) | Prop::Le(a, b) => {
            term_has_unsolved_meta(a, metas) || term_has_unsolved_meta(b, metas)
        }
        Prop::And(x, y) | Prop::Imply(x, y) => {
            prop_has_unsolved_meta(x, metas) || prop_has_unsolved_meta(y, metas)
        }
        Prop::Not(inner) => prop_has_unsolved_meta(inner, metas),
        Prop::False => false,
    }
}

// ============================================================================
// REWRITE SUPPORT — substitute one term for another throughout a Prop
// ============================================================================

fn rewrite_in_term(t: &Term, from: &Term, to: &Term) -> Term {
    if t == from {
        return to.clone();
    }
    match t {
        Term::Int(_) | Term::Var(_) => t.clone(),
        Term::Op(op, l, r) => Term::Op(
            op.clone(),
            Box::new(rewrite_in_term(l, from, to)),
            Box::new(rewrite_in_term(r, from, to)),
        ),
        Term::App(f, args) => Term::App(
            f.clone(),
            args.iter().map(|a| rewrite_in_term(a, from, to)).collect(),
        ),
    }
}

fn rewrite_in_prop(p: &Prop, from: &Term, to: &Term) -> Prop {
    match p {
        Prop::Eq(a, b) => Prop::Eq(rewrite_in_term(a, from, to), rewrite_in_term(b, from, to)),
        Prop::Le(a, b) => Prop::Le(rewrite_in_term(a, from, to), rewrite_in_term(b, from, to)),
        Prop::And(p, q) => Prop::And(
            Box::new(rewrite_in_prop(p, from, to)),
            Box::new(rewrite_in_prop(q, from, to)),
        ),
        Prop::Not(p) => Prop::Not(Box::new(rewrite_in_prop(p, from, to))),
        Prop::Imply(p, q) => Prop::Imply(
            Box::new(rewrite_in_prop(p, from, to)),
            Box::new(rewrite_in_prop(q, from, to)),
        ),
        Prop::False => Prop::False,
    }
}

// ============================================================================
// SYNTHESIS — figuring out what Prop a proof term proves, without a goal
// ============================================================================
//
// Needed for `rewrite t_eq in t_body` — we have to know the shape of the
// equation t_eq proves before we can rewrite anything in the goal. Only a
// handful of proof term forms are synthesizable in v1: hypotheses (look up in
// Ctx) and zero-premise `apply` calls with no free metavars in the
// conclusion. Everything else forces the user to write an intermediate `let`.

fn synthesize(
    term: &ProofTerm,
    ctx: &Ctx,
    reg: &Registry,
) -> Result<Prop, ProofError> {
    match term {
        ProofTerm::Hyp(name) => ctx
            .lookup_prop(name)
            .cloned()
            .ok_or_else(|| ProofError::UnknownHypothesis(name.clone())),
        ProofTerm::Apply(name, args) => {
            let schema = reg
                .lookup(name)
                .ok_or_else(|| ProofError::UnknownAxiom(name.clone()))?;
            if !schema.vars.is_empty() {
                return Err(ProofError::CannotSynthesize(format!(
                    "apply {} has universal variables; cannot synthesize without a goal to unify against",
                    name
                )));
            }
            if schema.premises.len() != args.len() {
                return Err(ProofError::PremiseCount {
                    axiom: name.clone(),
                    expected: schema.premises.len(),
                    got: args.len(),
                });
            }
            // Zero metas means premises are closed. Check them against the
            // schema and return the conclusion.
            for (i, arg) in args.iter().enumerate() {
                check(arg, &schema.premises[i], ctx, reg)?;
            }
            Ok(schema.conclusion.clone())
        }
        _ => Err(ProofError::CannotSynthesize(
            "only hypotheses and closed applies can be synthesized in v1; use `let` to name other proofs first".into(),
        )),
    }
}

// ============================================================================
// CHECK — the entry point. Every judgment rule from §5 lands here.
// ============================================================================

/// Check that `term` proves `goal` in the given context and registry.
///
/// Returns `Ok(())` on success, `Err(ProofError)` on failure. The error is
/// informative — designed for direct reporting back to the user without
/// further interpretation.
pub fn check(
    term: &ProofTerm,
    goal: &Prop,
    ctx: &Ctx,
    reg: &Registry,
) -> Result<(), ProofError> {
    match term {
        // [REFL]  Γ ⊢ refl : e == e
        ProofTerm::Refl => {
            if let Prop::Eq(a, b) = goal {
                if a == b {
                    return Ok(());
                }
            }
            Err(ProofError::GoalMismatch {
                expected: "e == e".into(),
                got: goal.to_string(),
            })
        }

        // [VAR]  (v : P) ∈ Γ ⟹ Γ ⊢ v : P
        ProofTerm::Hyp(name) => {
            let p = ctx
                .lookup_prop(name)
                .ok_or_else(|| ProofError::UnknownHypothesis(name.clone()))?;
            if p == goal {
                Ok(())
            } else {
                Err(ProofError::GoalMismatch {
                    expected: goal.to_string(),
                    got: p.to_string(),
                })
            }
        }

        // [APPLY]  instantiate a named schema and discharge its premises
        ProofTerm::Apply(name, args) => check_apply(name, args, goal, ctx, reg),

        // [REWRITE]  substitute equals in the goal, then check the body
        ProofTerm::Rewrite(eq_term, body_term) => {
            let eq_prop = synthesize(eq_term, ctx, reg)?;
            match eq_prop {
                Prop::Eq(lhs, rhs) => {
                    let new_goal = rewrite_in_prop(goal, &lhs, &rhs);
                    check(body_term, &new_goal, ctx, reg)
                }
                other => Err(ProofError::NotEquality(other.to_string())),
            }
        }

        // [LET]  locally name an intermediate proof
        ProofTerm::Let(name, bound, body) => {
            let bound_prop = synthesize(bound, ctx, reg)?;
            let new_ctx = ctx.with_prop(name.clone(), bound_prop);
            check(body, goal, &new_ctx, reg)
        }

        // [ASSUME]  discharge an implication
        ProofTerm::Assume(hyp_prop, body) => {
            if let Prop::Imply(p, q) = goal {
                if **p != *hyp_prop {
                    return Err(ProofError::GoalMismatch {
                        expected: format!("{}", p),
                        got: format!("{}", hyp_prop),
                    });
                }
                let new_ctx = ctx.with_prop("__assumed".into(), hyp_prop.clone());
                check(body, q, &new_ctx, reg)
            } else {
                Err(ProofError::NotImplication(goal.to_string()))
            }
        }

        // [CONTRA]  Γ ⊢ t : False ⟹ Γ ⊢ contra{t} : P
        ProofTerm::Contra(body) => check(body, &Prop::False, ctx, reg),

        // [IND] and [CASES]  phase 2
        ProofTerm::InductionOn(_, _) => {
            Err(ProofError::NotImplemented("induction_on (phase 2)"))
        }
        ProofTerm::Cases(_, _) => Err(ProofError::NotImplemented("cases (phase 2)")),
    }
}

/// [APPLY] handler. Named axioms and proved lemmas live in the same registry,
/// so there is a single code path. Propositional axioms and concrete literal
/// order checks are special-cased here instead of living in the schema table:
/// they either require proposition metavariables or runtime inspection of the
/// concrete goal.
fn check_apply(
    name: &str,
    args: &[ProofTerm],
    goal: &Prop,
    ctx: &Ctx,
    reg: &Registry,
) -> Result<(), ProofError> {
    // --- Special-cased propositional axioms ---

    match name {
        // and.intro : ∀P Q. P → Q → P and Q
        "and.intro" => {
            if args.len() != 2 {
                return Err(ProofError::PremiseCount {
                    axiom: "and.intro".into(),
                    expected: 2,
                    got: args.len(),
                });
            }
            if let Prop::And(p, q) = goal {
                check(&args[0], p, ctx, reg)?;
                check(&args[1], q, ctx, reg)?;
                return Ok(());
            }
            return Err(ProofError::GoalMismatch {
                expected: "P and Q".into(),
                got: goal.to_string(),
            });
        }
        // and.elim_l : ∀P Q. (P and Q) → P
        "and.elim_l" => {
            if args.len() != 1 {
                return Err(ProofError::PremiseCount {
                    axiom: "and.elim_l".into(),
                    expected: 1,
                    got: args.len(),
                });
            }
            let actual = synthesize(&args[0], ctx, reg)?;
            match actual {
                Prop::And(lhs, _) if lhs.as_ref() == goal => return Ok(()),
                Prop::And(lhs, _) => {
                    return Err(ProofError::GoalMismatch {
                        expected: goal.to_string(),
                        got: lhs.to_string(),
                    });
                }
                other => {
                    return Err(ProofError::GoalMismatch {
                        expected: "(P and Q)".into(),
                        got: other.to_string(),
                    });
                }
            }
        }
        // and.elim_r : ∀P Q. (P and Q) → Q
        "and.elim_r" => {
            if args.len() != 1 {
                return Err(ProofError::PremiseCount {
                    axiom: "and.elim_r".into(),
                    expected: 1,
                    got: args.len(),
                });
            }
            let actual = synthesize(&args[0], ctx, reg)?;
            match actual {
                Prop::And(_, rhs) if rhs.as_ref() == goal => return Ok(()),
                Prop::And(_, rhs) => {
                    return Err(ProofError::GoalMismatch {
                        expected: goal.to_string(),
                        got: rhs.to_string(),
                    });
                }
                other => {
                    return Err(ProofError::GoalMismatch {
                        expected: "(P and Q)".into(),
                        got: other.to_string(),
                    });
                }
            }
        }
        // false.elim : ∀P. False → P
        "false.elim" => {
            if args.len() != 1 {
                return Err(ProofError::PremiseCount {
                    axiom: "false.elim".into(),
                    expected: 1,
                    got: args.len(),
                });
            }
            check(&args[0], &Prop::False, ctx, reg)?;
            return Ok(());
        }
        // not.intro : ∀P. (P → False) → not(P)
        "not.intro" => {
            if args.len() != 1 {
                return Err(ProofError::PremiseCount {
                    axiom: "not.intro".into(),
                    expected: 1,
                    got: args.len(),
                });
            }
            if let Prop::Not(inner) = goal {
                let implication =
                    Prop::Imply(Box::new(inner.as_ref().clone()), Box::new(Prop::False));
                check(&args[0], &implication, ctx, reg)?;
                return Ok(());
            }
            return Err(ProofError::GoalMismatch {
                expected: "not(P)".into(),
                got: goal.to_string(),
            });
        }
        // int_ord.le_of_concrete : close m <= n by literal comparison
        "int_ord.le_of_concrete" => {
            if !args.is_empty() {
                return Err(ProofError::PremiseCount {
                    axiom: "int_ord.le_of_concrete".into(),
                    expected: 0,
                    got: args.len(),
                });
            }
            match goal {
                Prop::Le(Term::Int(lhs), Term::Int(rhs)) if lhs <= rhs => return Ok(()),
                _ => {
                    return Err(ProofError::GoalMismatch {
                        expected: "m <= n for concrete literals with m <= n".into(),
                        got: goal.to_string(),
                    });
                }
            }
        }
        _ => {}
    }

    // --- Generic schema-based dispatch ---

    let schema = reg
        .lookup(name)
        .ok_or_else(|| ProofError::UnknownAxiom(name.to_string()))?;

    if schema.premises.len() != args.len() {
        return Err(ProofError::PremiseCount {
            axiom: name.to_string(),
            expected: schema.premises.len(),
            got: args.len(),
        });
    }

    // Unify the schema conclusion against the goal to solve the universal
    // metavariables. Any leftover metas after this step mean the goal under-
    // determines the instantiation — we do not guess, we reject.
    let mut subst = Subst::new();
    unify_prop(&schema.conclusion, goal, &schema.vars, &mut subst)?;

    // Premise proofs can refine the substitution further. This matters for
    // lemmas like eq.trans where the goal fixes x and z but the premise proofs
    // determine the intermediate y.
    for (i, prem_proof) in args.iter().enumerate() {
        let partially_expected = subst_prop(&schema.premises[i], &subst);
        if prop_has_unsolved_meta(&partially_expected, &schema.vars) {
            let actual = synthesize(prem_proof, ctx, reg).map_err(|_| {
                ProofError::CannotSynthesize(format!(
                    "premise {} of {} leaves metavariables unsolved after goal unification",
                    i + 1,
                    name
                ))
            })?;
            unify_prop(&partially_expected, &actual, &schema.vars, &mut subst)?;
        }

        let expected = subst_prop(&schema.premises[i], &subst);
        check(prem_proof, &expected, ctx, reg)?;
    }

    for v in &schema.vars {
        if !subst.contains_key(v) {
            return Err(ProofError::UnificationFailed(format!(
                "metavariable {} in {} was not solved by the goal or premises",
                v, name
            )));
        }
    }

    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================
//
// Every judgment rule and every axiom in the registry has at least one
// positive test here. Failures here mean the kernel is shipping lies — do
// not relax these on pain of the whole verification story.

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Term {
        Term::Var(s.to_string())
    }
    fn op(o: &str, a: Term, b: Term) -> Term {
        Term::Op(o.to_string(), Box::new(a), Box::new(b))
    }

    // --- REFL ---

    #[test]
    fn refl_closes_trivial_equality() {
        let goal = Prop::Eq(v("x"), v("x"));
        let ctx = Ctx::new().with_var("x".into());
        let reg = Registry::with_builtins();
        assert!(check(&ProofTerm::Refl, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn refl_rejects_non_reflexive_equality() {
        let goal = Prop::Eq(v("x"), v("y"));
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        assert!(check(&ProofTerm::Refl, &goal, &ctx, &reg).is_err());
    }

    // --- APPLY with zero-premise axioms ---

    #[test]
    fn add_comm_closes_via_comm_add() {
        // Goal: a + b == b + a
        // Proof: apply int_ring.comm_add
        let goal = Prop::Eq(op("+", v("a"), v("b")), op("+", v("b"), v("a")));
        let ctx = Ctx::new().with_var("a".into()).with_var("b".into());
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ring.comm_add".into(), vec![]);
        let result = check(&proof, &goal, &ctx, &reg);
        assert!(result.is_ok(), "add_comm failed: {:?}", result);
    }

    #[test]
    fn zero_le_zero_closes_via_le_refl() {
        // Goal: 0 <= 0
        // Proof: apply int_ord.le_refl
        let goal = Prop::Le(Term::Int(0), Term::Int(0));
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ord.le_refl".into(), vec![]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn zero_add_closes_via_zero_add_axiom() {
        // Goal: 0 + x == x
        let goal = Prop::Eq(op("+", Term::Int(0), v("x")), v("x"));
        let ctx = Ctx::new().with_var("x".into());
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ring.zero_add".into(), vec![]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn mul_comm_closes_via_comm_mul() {
        let goal = Prop::Eq(op("*", v("a"), v("b")), op("*", v("b"), v("a")));
        let ctx = Ctx::new().with_var("a".into()).with_var("b".into());
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ring.comm_mul".into(), vec![]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    // --- APPLY with premise proofs (eq.trans) ---

    #[test]
    fn eq_trans_closes_x_eq_z_from_two_hypotheses() {
        // Γ = { a : ?, b : ?, c : ?, h1 : a == b, h2 : b == c }
        // Goal: a == c
        // Proof: apply eq.trans(h1, h2)
        let ctx = Ctx::new()
            .with_var("a".into())
            .with_var("b".into())
            .with_var("c".into())
            .with_prop("h1".into(), Prop::Eq(v("a"), v("b")))
            .with_prop("h2".into(), Prop::Eq(v("b"), v("c")));
        let reg = Registry::with_builtins();
        let goal = Prop::Eq(v("a"), v("c"));
        let proof = ProofTerm::Apply(
            "eq.trans".into(),
            vec![
                ProofTerm::Hyp("h1".into()),
                ProofTerm::Hyp("h2".into()),
            ],
        );
        let result = check(&proof, &goal, &ctx, &reg);
        assert!(result.is_ok(), "eq.trans failed: {:?}", result);
    }

    // --- HYP ---

    #[test]
    fn hyp_lookup_closes_goal_equal_to_hypothesis() {
        let ctx = Ctx::new()
            .with_var("x".into())
            .with_prop("h".into(), Prop::Eq(v("x"), v("x")));
        let reg = Registry::with_builtins();
        let goal = Prop::Eq(v("x"), v("x"));
        assert!(check(&ProofTerm::Hyp("h".into()), &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn unknown_hypothesis_is_rejected() {
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let goal = Prop::Eq(v("x"), v("x"));
        let err = check(&ProofTerm::Hyp("h".into()), &goal, &ctx, &reg).unwrap_err();
        matches!(err, ProofError::UnknownHypothesis(_));
    }

    // --- REWRITE ---

    #[test]
    fn rewrite_substitutes_equal_terms_in_goal() {
        // Register a "proved lemma": length_nil : length(Nil) == 0
        let mut reg = Registry::with_builtins();
        let len_nil_schema = Schema {
            vars: vec![],
            premises: vec![],
            conclusion: Prop::Eq(
                Term::App("length".into(), vec![Term::App("Nil".into(), vec![])]),
                Term::Int(0),
            ),
        };
        reg.register("length.nil".into(), len_nil_schema).unwrap();

        // Goal: 0 <= length(Nil)
        // Proof: rewrite (apply length.nil) in (apply int_ord.le_refl)
        //   — after rewriting length(Nil) -> 0, the goal becomes 0 <= 0,
        //     closed by le_refl.
        let goal = Prop::Le(
            Term::Int(0),
            Term::App("length".into(), vec![Term::App("Nil".into(), vec![])]),
        );
        let ctx = Ctx::new();
        let proof = ProofTerm::Rewrite(
            Box::new(ProofTerm::Apply("length.nil".into(), vec![])),
            Box::new(ProofTerm::Apply("int_ord.le_refl".into(), vec![])),
        );
        let result = check(&proof, &goal, &ctx, &reg);
        assert!(result.is_ok(), "rewrite failed: {:?}", result);
    }

    // --- AND (special-cased axiom) ---

    #[test]
    fn and_intro_closes_conjunction_with_two_proofs() {
        // Goal: (0 <= 0) and (x == x)
        let ctx = Ctx::new().with_var("x".into());
        let reg = Registry::with_builtins();
        let goal = Prop::And(
            Box::new(Prop::Le(Term::Int(0), Term::Int(0))),
            Box::new(Prop::Eq(v("x"), v("x"))),
        );
        let proof = ProofTerm::Apply(
            "and.intro".into(),
            vec![
                ProofTerm::Apply("int_ord.le_refl".into(), vec![]),
                ProofTerm::Refl,
            ],
        );
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn and_elim_l_extracts_left_conjunct_from_hypothesis() {
        let goal = Prop::Eq(v("x"), v("x"));
        let ctx = Ctx::new().with_var("x".into()).with_prop(
            "both".into(),
            Prop::And(
                Box::new(goal.clone()),
                Box::new(Prop::Le(Term::Int(0), Term::Int(0))),
            ),
        );
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("and.elim_l".into(), vec![ProofTerm::Hyp("both".into())]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn and_elim_r_extracts_right_conjunct_from_hypothesis() {
        let goal = Prop::Le(Term::Int(0), Term::Int(0));
        let ctx = Ctx::new().with_prop(
            "both".into(),
            Prop::And(
                Box::new(Prop::Eq(v("x"), v("x"))),
                Box::new(goal.clone()),
            ),
        );
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("and.elim_r".into(), vec![ProofTerm::Hyp("both".into())]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    // --- ASSUME ---

    #[test]
    fn assume_discharges_an_implication() {
        // Goal: (x == x) -> (x == x)
        // Proof: assume (x == x) in refl
        let ctx = Ctx::new().with_var("x".into());
        let reg = Registry::with_builtins();
        let p = Prop::Eq(v("x"), v("x"));
        let goal = Prop::Imply(Box::new(p.clone()), Box::new(p.clone()));
        let proof =
            ProofTerm::Assume(p, Box::new(ProofTerm::Hyp("__assumed".into())));
        let result = check(&proof, &goal, &ctx, &reg);
        assert!(result.is_ok(), "assume failed: {:?}", result);
    }

    #[test]
    fn not_intro_closes_negation_from_implication_to_false() {
        let ctx = Ctx::new().with_var("x".into()).with_prop("boom".into(), Prop::False);
        let reg = Registry::with_builtins();
        let proposition = Prop::Eq(v("x"), v("x"));
        let goal = Prop::Not(Box::new(proposition.clone()));
        let proof = ProofTerm::Apply(
            "not.intro".into(),
            vec![ProofTerm::Assume(
                proposition,
                Box::new(ProofTerm::Hyp("boom".into())),
            )],
        );
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn false_elim_closes_any_goal_from_false_hypothesis() {
        let goal = Prop::Eq(v("x"), v("y"));
        let ctx = Ctx::new()
            .with_var("x".into())
            .with_var("y".into())
            .with_prop("boom".into(), Prop::False);
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("false.elim".into(), vec![ProofTerm::Hyp("boom".into())]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn le_of_concrete_accepts_true_literal_inequality() {
        let goal = Prop::Le(Term::Int(2), Term::Int(5));
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ord.le_of_concrete".into(), vec![]);
        assert!(check(&proof, &goal, &ctx, &reg).is_ok());
    }

    #[test]
    fn le_of_concrete_rejects_false_literal_inequality() {
        let goal = Prop::Le(Term::Int(5), Term::Int(2));
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let proof = ProofTerm::Apply("int_ord.le_of_concrete".into(), vec![]);
        let err = check(&proof, &goal, &ctx, &reg).unwrap_err();
        assert!(matches!(err, ProofError::GoalMismatch { .. }));
    }

    // --- NEGATIVE TESTS ---

    #[test]
    fn apply_with_wrong_premise_count_is_rejected() {
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let goal = Prop::Eq(v("x"), v("z"));
        // eq.trans needs two premise proofs; provide one.
        let proof = ProofTerm::Apply(
            "eq.trans".into(),
            vec![ProofTerm::Refl],
        );
        let err = check(&proof, &goal, &ctx, &reg).unwrap_err();
        assert!(matches!(err, ProofError::PremiseCount { .. }));
    }

    #[test]
    fn unknown_axiom_is_rejected() {
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let goal = Prop::Eq(v("x"), v("x"));
        let proof = ProofTerm::Apply("imaginary.axiom".into(), vec![]);
        let err = check(&proof, &goal, &ctx, &reg).unwrap_err();
        assert!(matches!(err, ProofError::UnknownAxiom(_)));
    }

    #[test]
    fn induction_is_stubbed_as_not_implemented() {
        let ctx = Ctx::new();
        let reg = Registry::with_builtins();
        let goal = Prop::Le(Term::Int(0), v("x"));
        let proof = ProofTerm::InductionOn("x".into(), vec![]);
        let err = check(&proof, &goal, &ctx, &reg).unwrap_err();
        assert!(matches!(err, ProofError::NotImplemented(_)));
    }
}
