//! The clause catalogue and the witnesses of `runa cases`
//! (`docs/rfcs/conformance-cases.md`): every clause of every rule family an
//! entry reaches, with the audit's verdict, and Z3 models of a family's
//! arguments that make a clause fire or put one of its thresholds just
//! below, at or just above its bound. A model is solved relative to the
//! arguments a run passed (anchors), so it moves as few values as it can.
use super::*;
use serde_json::{Map as JsonMap, Value as JsonValue};

/// The clauses an entry reaches and how to ask for their witnesses.
pub(crate) struct CaseCatalogue {
    pub(crate) families: Vec<CatalogueFamily>,
    /// Every clause, in file and line order.
    pub(crate) clauses: Vec<CatalogueClause>,
    /// Every datatype the encoding declares: its type parameters and
    /// variants.
    datatypes: BTreeMap<String, (Vec<String>, Vec<Variant>)>,
}

pub(crate) struct CatalogueFamily {
    pub(crate) scope: Option<String>,
    pub(crate) name: String,
    pub(crate) arity: usize,
    pub(crate) parameters: Vec<CatalogueParameter>,
    /// The declarations and per-candidate definitions every query of the
    /// family starts with; none when no guard is translatable.
    prefix: Option<String>,
}

impl CatalogueFamily {
    /// `Scope.name` or `name`.
    pub(crate) fn qualified(&self) -> String {
        match &self.scope {
            Some(scope) => format!("{}.{}", scope, self.name),
            None => self.name.clone(),
        }
    }
}

pub(crate) struct CatalogueParameter {
    pub(crate) name: String,
    term: String,
    /// The parameter's type when the encoding represents it.
    ty: Option<Ty>,
}

pub(crate) struct CatalogueClause {
    pub(crate) family: usize,
    /// An exception's label, else `clause-<n>`: its position among the
    /// family's clauses in source order.
    pub(crate) key: String,
    /// The file as the audit names it, and the line of the clause.
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) status: ClauseStatus,
    pub(crate) thresholds: Vec<Threshold>,
    /// The candidate's position in its family's dispatch order.
    index: usize,
}

pub(crate) enum ClauseStatus {
    /// Its guard is in the encoding and it can fire.
    Analysable,
    /// Its guard is outside the encoding, or Z3 rejected the query.
    NotAnalysable(String),
    /// The audit proved it never fires.
    Unreachable(String),
    /// Z3 could not decide whether it fires.
    Unknown(String),
}

/// A top-level conjunct of a guard comparing two integers, or two dates
/// (`dates::smt_date_order` wrote it over their fields).
pub(crate) struct Threshold {
    /// The comparison as the guard writes it, parameters by name.
    pub(crate) text: String,
    /// `lhs - rhs` (in days for dates), as SMT.
    difference: String,
    /// The guard's other top-level conjuncts, as SMT.
    others: Vec<String>,
    /// Validity of the two dates, for a date comparison.
    domain: Option<String>,
    /// The argument leaves the comparison reads: parameter and path (`""`
    /// for the whole argument).
    pub(crate) reads: Vec<(usize, String)>,
}

/// What a witness query asks.
#[derive(Clone, Copy)]
pub(crate) enum Goal {
    /// The clause fires.
    Fires,
    /// Dispatch reaches the clause, the threshold's other conjuncts hold,
    /// and `lhs - rhs` is `offset`.
    Threshold { index: usize, offset: i64 },
}

pub(crate) enum Witness {
    /// Each parameter's value (plain JSON, as `ProbeArg::value`), none for
    /// a parameter the encoding does not represent.
    Model(Vec<Option<JsonValue>>),
    /// No model in the input domain; this one lies outside it.
    OutsideDomain(Vec<Option<JsonValue>>),
    Unsat,
    Unknown(String),
}

/// A run's value of one argument, which the model keeps where it can:
/// leaves at `fixed` paths weigh more (no input moves them).
pub(crate) struct Anchor<'a> {
    pub(crate) parameter: usize,
    pub(crate) value: &'a JsonValue,
    pub(crate) fixed: &'a BTreeSet<String>,
}

/// A leaf of an anchored argument: its parameter and path in the value.
pub(crate) struct ArgumentLeaf {
    pub(crate) parameter: usize,
    pub(crate) path: String,
}

/// What the input domain (`calculate::input_domain`) asks of the model where
/// an argument leaf of the run's call is an input leaf unchanged: an integer
/// input stays a safe integer, and an input date a day on the calendar.
pub(crate) enum InputDomain {
    Int(ArgumentLeaf),
    /// The date's year, month and day: each an argument leaf, or the input's
    /// value where no argument leaf is that part.
    Date([Result<ArgumentLeaf, i64>; 3]),
}

/// A leaf of an anchored argument as SMT: its term, the testers of the
/// variants above it, and the constraint that keeps its value.
struct LeafTerm {
    path: String,
    term: String,
    testers: Vec<String>,
    keep: String,
}

/// `domain` as a hard constraint over the anchored leaves, where the
/// variants above them are the anchor's; none when a leaf it names is not
/// anchored.
fn domain_constraint(
    domain: &InputDomain,
    anchored: &BTreeMap<usize, Vec<LeafTerm>>,
) -> Option<String> {
    let mut testers: Vec<&str> = Vec::new();
    let mut term = |leaf: &ArgumentLeaf| -> Option<String> {
        let found = anchored
            .get(&leaf.parameter)?
            .iter()
            .find(|candidate| candidate.path == leaf.path)?;
        for tester in &found.testers {
            if !testers.contains(&tester.as_str()) {
                testers.push(tester);
            }
        }
        Some(found.term.clone())
    };
    let constraint = match domain {
        InputDomain::Int(leaf) => format!("(runa_int_in_domain {})", term(leaf)?),
        InputDomain::Date(parts) => {
            let mut terms = Vec::new();
            for part in parts {
                terms.push(match part {
                    Ok(leaf) => term(leaf)?,
                    Err(value) => smt_int(*value),
                });
            }
            format!("(runa_valid_date {})", terms.join(" "))
        }
    };
    Some(if testers.is_empty() {
        constraint
    } else {
        format!("(=> (and {}) {})", testers.join(" "), constraint)
    })
}

/// The catalogue of the families `reached` names (and those with a clause
/// in `filename`), as `analyze_entry_clause_reachability` selects them.
pub(crate) fn case_catalogue(
    statements: &[Stmt],
    filename: &str,
    source: &str,
    reached: &BTreeSet<String>,
) -> Result<CaseCatalogue, String> {
    let mut catalogue = CaseCatalogue {
        families: Vec::new(),
        clauses: Vec::new(),
        datatypes: BTreeMap::new(),
    };
    for_each_family(
        statements,
        filename,
        source,
        Selection::ReachedFromEntry(reached),
        |view| {
            if catalogue.datatypes.is_empty() {
                for (name, variants) in view.encoder.adts_by_name.iter() {
                    catalogue.datatypes.insert(
                        name.to_string(),
                        (
                            view.encoder.type_parameters(name).to_vec(),
                            variants.to_vec(),
                        ),
                    );
                }
            }
            catalogue_family(&mut catalogue, view)
        },
    )?;
    catalogue.clauses.sort_by(|left, right| {
        (&left.file, left.line, left.index).cmp(&(&right.file, right.line, right.index))
    });
    Ok(catalogue)
}

fn catalogue_family(catalogue: &mut CaseCatalogue, view: &FamilyView) -> Result<(), FamilyError> {
    let candidates = view.lowered.candidates.as_slice();
    let signature = view.signature;
    let family_index = catalogue.families.len();
    let parameters = signature
        .arguments
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let symbol = argument_symbol(index);
            CatalogueParameter {
                name: signature
                    .witness_names
                    .iter()
                    .find(|value| value.term == smt_value_symbol(&symbol))
                    .map(|value| value.display.clone())
                    .unwrap_or_else(|| format!("argument {}", index + 1)),
                term: smt_value_symbol(&symbol),
                ty: signature.sorts.get(&symbol).cloned(),
            }
        })
        .collect::<Vec<_>>();
    let guards: Vec<Option<&(Expr, Requirements)>> = candidates
        .iter()
        .map(|candidate| candidate.guard.as_ref().ok())
        .collect();
    // The audit's verdict on each clause.
    let outcome = analyze_family(
        &view.family,
        signature,
        view.encoder,
        view.lowered,
        view.check_coverage,
    )?;
    let place = |location: &ClauseLocation| (location.file.clone(), location.line);
    let mut statuses: BTreeMap<(String, usize), ClauseStatus> = BTreeMap::new();
    for finding in &outcome.findings {
        let status = match &finding.kind {
            ClauseFindingKind::UnsatisfiableGuard => {
                ClauseStatus::Unreachable("its guard can never hold".to_string())
            }
            ClauseFindingKind::Shadowed { by } => ClauseStatus::Unreachable(if by.is_empty() {
                "earlier clauses always apply first".to_string()
            } else {
                format!(
                    "earlier clauses always apply first: {}",
                    by.iter()
                        .map(ClauseLocation::display)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
            _ => continue,
        };
        statuses.insert(place(&finding.clause), status);
    }
    for skipped in &outcome.not_analyzed {
        let status = if skipped.reason.starts_with("Z3 could not decide") {
            ClauseStatus::Unknown(skipped.reason.clone())
        } else {
            ClauseStatus::NotAnalysable(skipped.reason.clone())
        };
        statuses.entry(place(&skipped.clause)).or_insert(status);
    }

    let mut prefix = None;
    if guards.iter().any(Option::is_some) {
        let mut text = family_preamble(view.encoder, signature, candidates, std::iter::empty());
        text.push_str(&dispatch_definitions(view.encoder, &guards).0);
        text.push_str(DAY_NUMBER);
        text.push_str(&calculate::input_domain::smt_definitions());
        prefix = Some(text);
    }

    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by_key(|index| candidates[*index].source_order);
    for (index, candidate) in candidates.iter().enumerate() {
        let position = order
            .iter()
            .position(|other| *other == index)
            .unwrap_or(index)
            + 1;
        let thresholds = match &candidate.guard {
            Ok((guard, _)) => thresholds(guard, &parameters),
            Err(_) => Vec::new(),
        };
        catalogue.clauses.push(CatalogueClause {
            family: family_index,
            key: candidate
                .label
                .clone()
                .unwrap_or_else(|| format!("clause-{}", position)),
            file: candidate.location.file.clone(),
            line: candidate.location.line,
            status: statuses
                .remove(&place(&candidate.location))
                .unwrap_or(ClauseStatus::Analysable),
            thresholds,
            index,
        });
    }
    catalogue.families.push(CatalogueFamily {
        scope: view.identity.0.clone(),
        name: view.identity.1.clone(),
        arity: view.arity,
        parameters,
        prefix,
    });
    Ok(())
}

/// `runa_days`: the day number of a date (days since 1970-01-01), for date
/// thresholds. `runa_valid_date`, the input domain's, keeps their dates on
/// the calendar.
const DAY_NUMBER: &str = "(define-fun runa_days ((y Int) (m Int) (d Int)) Int \
(let ((yy (- y (ite (<= m 2) 1 0)))) (let ((era (div yy 400))) (let ((yoe (- yy (* era 400)))) \
(let ((doy (+ (div (+ (* 153 (mod (+ m 9) 12)) 2) 5) (- d 1)))) \
(+ (* era 146097) (* yoe 365) (div yoe 4) (- (div yoe 100)) doy -719468))))))\n";

fn conjuncts(expression: &Expr) -> Vec<&Expr> {
    match &expression.kind {
        ExprKind::BinOp(operator, left, right) if operator == "&&" => {
            let mut all = conjuncts(left);
            all.extend(conjuncts(right));
            all
        }
        _ => vec![expression],
    }
}

/// The two dates of a comparison `dates::date_comparison` wrote: `left`
/// and `right` of `left.year < right.year || ...`.
fn date_comparison(expression: &Expr) -> Option<(&Expr, &Expr)> {
    let ExprKind::BinOp(operator, first, _) = &expression.kind else {
        return None;
    };
    if operator != "||" {
        return None;
    }
    let ExprKind::BinOp(less, left, right) = &first.kind else {
        return None;
    };
    match (&left.kind, &right.kind) {
        (ExprKind::Field(left, left_part), ExprKind::Field(right, right_part))
            if less == "<" && left_part == "year" && right_part == "year" =>
        {
            Some((left, right))
        }
        _ => None,
    }
}

fn thresholds(guard: &Expr, parameters: &[CatalogueParameter]) -> Vec<Threshold> {
    let all = conjuncts(guard);
    let mut found = Vec::new();
    for (position, conjunct) in all.iter().enumerate() {
        let others = || {
            all.iter()
                .enumerate()
                .filter(|(other, _)| *other != position)
                .map(|(_, expression)| expr_to_smt(expression))
                .collect::<Vec<_>>()
        };
        // Below, at and above say where the side that reads arguments
        // stands relative to the other (the bound); when both sides read
        // arguments, the left side relative to the right.
        let orient =
            |left: &'_ Expr, right: &'_ Expr| -> Option<(Expr, Expr, Vec<(usize, String)>)> {
                let mut left_reads = Vec::new();
                argument_reads(left, &mut left_reads);
                let mut right_reads = Vec::new();
                argument_reads(right, &mut right_reads);
                let (left, right) = if left_reads.is_empty() && !right_reads.is_empty() {
                    (right, left)
                } else {
                    (left, right)
                };
                let mut reads = left_reads;
                for read in right_reads {
                    if !reads.contains(&read) {
                        reads.push(read);
                    }
                }
                (!reads.is_empty()).then(|| (left.clone(), right.clone(), reads))
            };
        if let Some((left, right)) = date_comparison(conjunct) {
            let Some((left, right, reads)) = orient(left, right) else {
                continue;
            };
            let parts = |date: &Expr| {
                ["year", "month", "day"]
                    .map(|part| {
                        expr_to_smt(&Expr::unspanned(ExprKind::Field(
                            Box::new(date.clone()),
                            part.to_string(),
                        )))
                    })
                    .join(" ")
            };
            found.push(Threshold {
                text: format!(
                    "{} against {} (dates)",
                    readable(&left, parameters),
                    readable(&right, parameters)
                ),
                difference: format!(
                    "(- (runa_days {}) (runa_days {}))",
                    parts(&left),
                    parts(&right)
                ),
                others: others(),
                domain: Some(format!(
                    "(and (runa_valid_date {}) (runa_valid_date {}))",
                    parts(&left),
                    parts(&right)
                )),
                reads,
            });
            continue;
        }
        let ExprKind::BinOp(operator, left, right) = &conjunct.kind else {
            continue;
        };
        if !matches!(operator.as_str(), "<" | "<=" | ">" | ">=")
            || !integer_term(left)
            || !integer_term(right)
        {
            continue;
        }
        let Some((left, right, reads)) = orient(left, right) else {
            continue;
        };
        found.push(Threshold {
            text: readable(conjunct, parameters),
            difference: format!("(- {} {})", expr_to_smt(&left), expr_to_smt(&right)),
            others: others(),
            domain: None,
            reads,
        });
    }
    found
}

/// Whether a comparison operand is an integer: anything but a string or a
/// float literal, a Boolean, or a nested comparison (the checker only lets
/// `<` compare numbers, and dates are rewritten).
fn integer_term(expression: &Expr) -> bool {
    !matches!(
        &expression.kind,
        ExprKind::Lit(Literal::Str(_) | Literal::Float(_) | Literal::Bool(_) | Literal::Char(_))
    )
}

/// The argument leaves `expression` reads: `(parameter, path)`.
fn argument_reads(expression: &Expr, reads: &mut Vec<(usize, String)>) {
    fn path_of(expression: &Expr) -> Option<(usize, Vec<String>)> {
        match &expression.kind {
            ExprKind::Var(name) => {
                let index = name.strip_prefix("__runa_rule_arg_")?.parse().ok()?;
                Some((index, Vec::new()))
            }
            ExprKind::Field(object, field) => {
                let (index, mut path) = path_of(object)?;
                path.push(field.clone());
                Some((index, path))
            }
            _ => None,
        }
    }
    if let Some((index, path)) = path_of(expression) {
        let read = (index, path.join("."));
        if !reads.contains(&read) {
            reads.push(read);
        }
        return;
    }
    walk_ast_expr(expression, &mut |node| {
        if let AstChild::Expr(inner) = node {
            if let ExprKind::Var(name) = &inner.kind {
                if let Some(index) = name
                    .strip_prefix("__runa_rule_arg_")
                    .and_then(|index| index.parse().ok())
                {
                    let read = (index, String::new());
                    if !reads.contains(&read) {
                        reads.push(read);
                    }
                }
            }
        }
    });
}

/// A namespaced SMT symbol (`|runa_smt_v1_rule_4x726f6f74_…_15x61736b…|`)
/// as the name the source writes: its last `<length>x<hex>` part decoded,
/// with its scope when it has one. Any other name is returned as is.
fn source_name(name: &str) -> String {
    let bare = name.trim_matches('|');
    let Some(rest) = bare.strip_prefix("runa_smt_v1_") else {
        return name.to_string();
    };
    let decode = |part: &str| -> Option<String> {
        let (length, hex) = part.split_once('x')?;
        let length: usize = length.parse().ok()?;
        let bytes = (0..hex.len() / 2 * 2)
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
            .collect::<Option<Vec<u8>>>()?;
        (bytes.len() == length)
            .then(|| String::from_utf8(bytes).ok())
            .flatten()
    };
    let parts: Vec<String> = rest
        .split('_')
        .filter(|part| part.contains('x'))
        .filter_map(decode)
        .collect();
    // namespace, kind, scope, name
    match parts.as_slice() {
        [.., scope, name] if !scope.is_empty() && parts.len() >= 4 => format!("{}.{}", scope, name),
        [.., name] => name.clone(),
        [] => name.to_string(),
    }
}

/// A lowered expression in source spelling, parameters by name.
fn readable(expression: &Expr, parameters: &[CatalogueParameter]) -> String {
    match &expression.kind {
        ExprKind::Var(name) => name
            .strip_prefix("__runa_rule_arg_")
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| parameters.get(index))
            .map(|parameter| parameter.name.clone())
            .unwrap_or_else(|| source_name(name)),
        ExprKind::Lit(Literal::Int(number)) => number.to_string(),
        ExprKind::Lit(Literal::Bool(value)) => value.to_string(),
        ExprKind::Lit(Literal::Str(text)) => format!("{:?}", text),
        ExprKind::Field(object, field) => format!("{}.{}", readable(object, parameters), field),
        ExprKind::BinOp(operator, left, right) => format!(
            "{} {} {}",
            readable(left, parameters),
            operator,
            readable(right, parameters)
        ),
        ExprKind::UnOp(operator, operand) => {
            format!("{}{}", operator, readable(operand, parameters))
        }
        ExprKind::App(function, arguments) => format!(
            "{}({})",
            readable(function, parameters),
            arguments
                .iter()
                .map(|argument| readable(argument, parameters))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "…".to_string(),
    }
}

impl CaseCatalogue {
    /// Ask Z3 for a model of `goal` on `clause`, keeping the anchors' values
    /// where it can and the inputs the arguments are in the input domain.
    /// When only a model outside that domain exists, it is
    /// [`Witness::OutsideDomain`]. The error is that no solver is available.
    pub(crate) fn witness(
        &self,
        clause: usize,
        goal: Goal,
        anchors: &[Anchor],
        domain: &[InputDomain],
    ) -> Result<Witness, String> {
        let clause = &self.clauses[clause];
        let family = &self.families[clause.family];
        let Some(prefix) = &family.prefix else {
            return Ok(Witness::Unknown(
                "no guard of the rule is translatable".into(),
            ));
        };
        let index = clause.index;
        let assertion = match goal {
            Goal::Fires => format!(
                "(and {} {} {})",
                guard_defined_name(index),
                guard_name(index),
                none_before_name(index)
            ),
            Goal::Threshold {
                index: threshold,
                offset,
            } => {
                let Some(threshold) = clause.thresholds.get(threshold) else {
                    return Ok(Witness::Unsat);
                };
                let mut parts = vec![none_before_name(index), guard_defined_name(index)];
                parts.extend(threshold.others.iter().cloned());
                parts.extend(threshold.domain.iter().cloned());
                parts.push(format!("(= {} {})", threshold.difference, smt_int(offset)));
                format!("(and {})", parts.join(" "))
            }
        };
        let mut script = prefix.clone();
        script.push_str(&format!("(assert {})\n", assertion));
        let mut anchored: BTreeMap<usize, Vec<LeafTerm>> = BTreeMap::new();
        for anchor in anchors {
            let Some(parameter) = family.parameters.get(anchor.parameter) else {
                continue;
            };
            let Some(ty) = &parameter.ty else {
                continue;
            };
            let mut leaves = Vec::new();
            self.anchor_leaves(&parameter.term, ty, anchor.value, "", &[], 0, &mut leaves);
            for leaf in &leaves {
                let weight = if anchor.fixed.contains(&leaf.path) {
                    50
                } else {
                    1
                };
                let keep = if leaf.testers.is_empty() {
                    leaf.keep.clone()
                } else {
                    format!("(and {} {})", leaf.testers.join(" "), leaf.keep)
                };
                script.push_str(&format!("(assert-soft {} :weight {})\n", keep, weight));
            }
            anchored.insert(anchor.parameter, leaves);
        }
        // Values no run gave stay plain.
        for (position, parameter) in family.parameters.iter().enumerate() {
            if anchored.contains_key(&position) {
                continue;
            }
            if let Some(ty) = &parameter.ty {
                for preference in
                    WitnessValue::new(parameter.term.clone(), String::new(), ty).preferences
                {
                    script.push_str(&format!("(assert-soft {})\n", preference));
                }
            }
        }
        let domain: Vec<String> = domain
            .iter()
            .filter_map(|domain| domain_constraint(domain, &anchored))
            .collect();
        let mut bounded = script.clone();
        for constraint in &domain {
            bounded.push_str(&format!("(assert {})\n", constraint));
        }
        match self.solve(family, bounded)? {
            Witness::Unsat if !domain.is_empty() => Ok(match self.solve(family, script)? {
                Witness::Model(values) => Witness::OutsideDomain(values),
                _ => Witness::Unsat,
            }),
            witness => Ok(witness),
        }
    }

    /// Run a witness query of `family` and read its model.
    fn solve(&self, family: &CatalogueFamily, mut script: String) -> Result<Witness, String> {
        script.push_str("(check-sat)\n");
        let represented: Vec<usize> = family
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.ty.is_some())
            .map(|(position, _)| position)
            .collect();
        if !represented.is_empty() {
            script.push_str(&format!(
                "(get-value ({}))\n",
                represented
                    .iter()
                    .map(|position| family.parameters[*position].term.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        let undecided =
            || Witness::Unknown("Z3 could not decide (unknown or resource limit reached)".into());
        let output = match run_solver(&script) {
            Err(FamilyError::SolverMissing(reason)) => return Err(reason),
            // After `unsat`, Z3 answers `get-value` with an error ("model is
            // not available") and fails: the query was answered. So it may
            // after `unknown`, when its resource limit stopped it early.
            Ok(Err(error)) => {
                let answer = error
                    .split_once("): ")
                    .map(|(_, output)| output.trim_start());
                return Ok(match answer {
                    Some(output) if output.starts_with("unsat\n") => Witness::Unsat,
                    Some(output) if output.starts_with("unknown\n") => undecided(),
                    _ => Witness::Unknown(format!(
                        "Z3 rejected the query: {}",
                        solver_error_summary(&error)
                    )),
                });
            }
            Ok(Ok(output)) => output,
        };
        let responses = smt_responses(&output);
        match responses.first().map(String::as_str) {
            Some("sat") => {}
            Some("unsat") => return Ok(Witness::Unsat),
            _ => return Ok(undecided()),
        }
        let mut values = vec![None; family.parameters.len()];
        if let Some(model) = responses.get(1) {
            let Some(SExpr::List(pairs)) = SExpr::parse(model) else {
                return Ok(Witness::Unknown(format!("unreadable model: {}", model)));
            };
            for (position, pair) in represented.iter().zip(pairs) {
                let SExpr::List(pair) = pair else {
                    continue;
                };
                let (Some(value), Some(ty)) = (pair.get(1), &family.parameters[*position].ty)
                else {
                    continue;
                };
                values[*position] = self.model_json(value, ty, 0);
            }
        }
        Ok(Witness::Model(values))
    }

    /// The variants and type arguments of a datatype type.
    fn datatype(&self, ty: &Ty) -> Option<(&[Variant], &[String], Vec<Ty>)> {
        let (owner, arguments) = smt_type_instance(ty)?;
        let (parameters, variants) = self.datatypes.get(owner)?;
        Some((variants, parameters, arguments))
    }

    /// Each leaf of `value` (as `ProbeArg::value` shows it), by the leaf's
    /// path: a variant's tester, or a scalar's term and value, under the
    /// testers of the variants above it.
    #[allow(clippy::too_many_arguments)]
    fn anchor_leaves(
        &self,
        term: &str,
        ty: &Ty,
        value: &JsonValue,
        path: &str,
        testers: &[String],
        depth: usize,
        leaves: &mut Vec<LeafTerm>,
    ) {
        if depth > 8 {
            return;
        }
        let join = |part: &str| {
            if path.is_empty() {
                part.to_string()
            } else {
                format!("{}.{}", path, part)
            }
        };
        let mut leaf = |path: String, keep: String| {
            leaves.push(LeafTerm {
                path,
                term: term.to_string(),
                testers: testers.to_vec(),
                keep,
            })
        };
        match (ty, value) {
            (Ty::Name(name), JsonValue::Bool(boolean)) if name == "Bool" => {
                leaf(path.to_string(), format!("(= {} {})", term, boolean));
            }
            (Ty::Name(name), JsonValue::String(text)) if name == "String" => {
                if let Ok(literal) = smt_string_literal(text) {
                    leaf(path.to_string(), format!("(= {} {})", term, literal));
                }
            }
            (_, JsonValue::Number(number)) => {
                if let Some(number) = number.as_i64() {
                    leaf(
                        path.to_string(),
                        format!("(= {} {})", term, smt_int(number)),
                    );
                }
            }
            (_, JsonValue::Object(object)) => {
                let Some((variants, parameters, arguments)) = self.datatype(ty) else {
                    return;
                };
                let Some(name) = object.get("$variant").and_then(JsonValue::as_str) else {
                    return;
                };
                let Some(variant) = variants
                    .iter()
                    .find(|variant| constructor_display_text(&variant.name) == name)
                else {
                    return;
                };
                let mut inner_testers = testers.to_vec();
                if variants.len() > 1 {
                    let tester = format!("({} {})", smt_tester(&variant.name), term);
                    leaf(join("$variant"), tester.clone());
                    inner_testers.push(tester);
                }
                for (index, field) in variant.fields.iter().enumerate() {
                    let key = if variant.positional {
                        index.to_string()
                    } else {
                        field.name.clone()
                    };
                    let Some(field_value) = object.get(&key) else {
                        continue;
                    };
                    let field_term = format!(
                        "({} {})",
                        smt_field_function(&smt_constructor_field_key(&variant.name, index)),
                        term
                    );
                    let field_ty = smt_substitute_type(&field.ty, parameters, &arguments);
                    self.anchor_leaves(
                        &field_term,
                        &field_ty,
                        field_value,
                        &join(&key),
                        &inner_testers,
                        depth + 1,
                        leaves,
                    );
                }
            }
            _ => {}
        }
    }

    /// A model value of type `ty` as plain JSON (as `ProbeArg::value`).
    fn model_json(&self, value: &SExpr, ty: &Ty, depth: usize) -> Option<JsonValue> {
        if depth > 16 {
            return None;
        }
        // `(as None (Option Int))` names a constructor with its sort.
        if let SExpr::List(items) = value {
            if let [SExpr::Atom(head), inner, _] = items.as_slice() {
                if head == "as" {
                    return self.model_json(inner, ty, depth);
                }
            }
        }
        match value {
            SExpr::Atom(atom) if atom == "true" || atom == "false" => {
                Some(JsonValue::Bool(atom == "true"))
            }
            SExpr::Atom(atom) if atom.chars().all(|c| c.is_ascii_digit()) => {
                atom.parse::<i64>().ok().map(JsonValue::from)
            }
            SExpr::List(items) if matches!(items.as_slice(), [SExpr::Atom(minus), SExpr::Atom(_)] if minus == "-") =>
            {
                let SExpr::Atom(digits) = &items[1] else {
                    return None;
                };
                digits
                    .parse::<i64>()
                    .ok()
                    .map(|number| JsonValue::from(-number))
            }
            SExpr::Str(text) => Some(JsonValue::from(text.clone())),
            SExpr::Atom(symbol) => self.constructor_json(symbol, &[], ty, depth),
            SExpr::List(items) => match items.split_first() {
                Some((SExpr::Atom(symbol), arguments)) => {
                    self.constructor_json(symbol, arguments, ty, depth)
                }
                _ => None,
            },
        }
    }

    fn constructor_json(
        &self,
        symbol: &str,
        arguments: &[SExpr],
        ty: &Ty,
        depth: usize,
    ) -> Option<JsonValue> {
        let (variants, parameters, type_arguments) = self.datatype(ty)?;
        let variant = variants
            .iter()
            .find(|variant| smt_symbol(&variant.name) == symbol)?;
        let mut object = JsonMap::new();
        object.insert(
            "$variant".to_string(),
            JsonValue::from(constructor_display_text(&variant.name).into_owned()),
        );
        for (index, (field, argument)) in variant.fields.iter().zip(arguments).enumerate() {
            let key = if variant.positional {
                index.to_string()
            } else {
                field.name.clone()
            };
            let field_ty = smt_substitute_type(&field.ty, parameters, &type_arguments);
            object.insert(key, self.model_json(argument, &field_ty, depth + 1)?);
        }
        Some(JsonValue::Object(object))
    }
}

fn smt_int(number: i64) -> String {
    if number < 0 {
        format!("(- {})", number.unsigned_abs())
    } else {
        number.to_string()
    }
}

/// A solver response read as an S-expression.
enum SExpr {
    Atom(String),
    Str(String),
    List(Vec<SExpr>),
}

impl SExpr {
    fn parse(text: &str) -> Option<SExpr> {
        let mut characters = text.chars().peekable();
        let parsed = Self::parse_one(&mut characters)?;
        Some(parsed)
    }

    fn parse_one(characters: &mut std::iter::Peekable<std::str::Chars>) -> Option<SExpr> {
        while characters.peek().is_some_and(|c| c.is_whitespace()) {
            characters.next();
        }
        match characters.next()? {
            '(' => {
                let mut items = Vec::new();
                loop {
                    while characters.peek().is_some_and(|c| c.is_whitespace()) {
                        characters.next();
                    }
                    if characters.peek() == Some(&')') {
                        characters.next();
                        return Some(SExpr::List(items));
                    }
                    items.push(Self::parse_one(characters)?);
                }
            }
            '"' => {
                let mut text = String::new();
                loop {
                    match characters.next()? {
                        '"' if characters.peek() == Some(&'"') => {
                            characters.next();
                            text.push('"');
                        }
                        '"' => break,
                        '\\' if characters.peek() == Some(&'u') => {
                            characters.next();
                            if characters.next()? != '{' {
                                return None;
                            }
                            let mut digits = String::new();
                            loop {
                                match characters.next()? {
                                    '}' => break,
                                    digit => digits.push(digit),
                                }
                            }
                            text.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
                        }
                        other => text.push(other),
                    }
                }
                Some(SExpr::Str(text))
            }
            first => {
                let mut atom = String::from(first);
                while let Some(&next) = characters.peek() {
                    if next.is_whitespace() || next == '(' || next == ')' {
                        break;
                    }
                    atom.push(next);
                    characters.next();
                }
                Some(SExpr::Atom(atom))
            }
        }
    }
}
