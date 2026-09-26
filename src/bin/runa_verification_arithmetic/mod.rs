//! Arithmetic obligations for the semantic verifier. SMT integers represent
//! mathematical values; a separate predicate records whether evaluating the
//! source expression is defined by Futuruna's checked i64 arithmetic.
use super::*;

pub(super) const FLOAT_REASON: &str =
    "Float verification requires runtime floating-point semantics; exact Real arithmetic is unsupported";

pub(super) fn int_domain(value: &str) -> String {
    format!("(and (<= (- 9223372036854775808) {value}) (<= {value} 9223372036854775807))")
}

pub(super) fn trunc_div(left: &str, right: &str) -> String {
    // Synthetic negations and products are mathematical operations used to
    // encode truncation. They are not additional runtime i64 operations.
    let magnitude = format!(
        "(div (ite (< {left} 0) (- {left}) {left}) (ite (< {right} 0) (- {right}) {right}))"
    );
    format!("(ite (= (< {left} 0) (< {right} 0)) {magnitude} (- {magnitude}))")
}

pub(super) fn and(parts: impl IntoIterator<Item = String>) -> String {
    let parts: Vec<_> = parts.into_iter().filter(|part| part != "true").collect();
    if parts.iter().any(|part| part == "false") {
        return "false".into();
    }
    match parts.as_slice() {
        [] => "true".into(),
        [part] => part.clone(),
        _ => format!("(and {})", parts.join(" ")),
    }
}

fn ite(condition: &str, yes: String, no: String) -> String {
    if yes == no {
        yes
    } else {
        format!("(ite {condition} {yes} {no})")
    }
}

pub(super) fn guard_name(name: &str) -> String {
    format!("runa_defined_{}", smt_value_symbol(name))
}

pub(super) fn needs_arithmetic_check(expression: &Expr, known_calls: &BTreeSet<String>) -> bool {
    let mut needed = false;
    walk_ast_expr(expression, &mut |node| {
        if let AstChild::Expr(expr) = node {
            needed |= matches!(&expr.kind, ExprKind::BinOp(op, _, _) if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%"))
                || matches!(&expr.kind, ExprKind::UnOp(op, _) if op == "-");
            if let ExprKind::App(function, _) = &expr.kind {
                // Known function bodies are inspected through the dependency
                // closure. Opaque calls (including abs and collection folds)
                // may perform arithmetic even without a visible operator.
                needed |= !matches!(&function.kind, ExprKind::Var(name) if known_calls.contains(name) || name == "not");
            }
        }
    });
    needed
}

pub(super) fn check_float_expression(expression: &Expr) -> Result<(), String> {
    let mut has_float = false;
    walk_ast_expr(expression, &mut |node| {
        if let AstChild::Expr(expr) = node {
            has_float |= matches!(&expr.kind, ExprKind::Lit(Literal::Float(_)));
        }
    });
    if has_float {
        Err(FLOAT_REASON.into())
    } else {
        Ok(())
    }
}

pub(super) fn check_float_type(ty: &Ty, adts: &[(String, Vec<Variant>)]) -> Result<(), String> {
    fn visit(
        ty: &Ty,
        adts: &[(String, Vec<Variant>)],
        seen: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        match ty {
            Ty::Name(name) if name == "Float" => return Err(FLOAT_REASON.into()),
            Ty::Name(name) if seen.insert(name.clone()) => {
                if let Some((_, variants)) = adts.iter().find(|(adt, _)| adt == name) {
                    for variant in variants {
                        for field in &variant.fields {
                            visit(&field.ty, adts, seen)?;
                        }
                    }
                }
            }
            Ty::App(head, args) => {
                visit(head, adts, seen)?;
                for arg in args {
                    visit(arg, adts, seen)?;
                }
            }
            Ty::Arrow(left, right) => {
                visit(left, adts, seen)?;
                visit(right, adts, seen)?;
            }
            Ty::Optional(inner) | Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                visit(inner, adts, seen)?
            }
            _ => {}
        }
        Ok(())
    }
    visit(ty, adts, &mut BTreeSet::new())
}

fn let_bindings(bindings: &[(String, String)], body: String) -> String {
    if bindings.is_empty() || body == "true" || body == "false" {
        return body;
    }
    format!(
        "(let ({}) {body})",
        bindings
            .iter()
            .map(|(name, value)| format!("({} {value})", smt_value_symbol(name)))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

// Match values and arithmetic guards use the same complete pattern test. In
// particular, nested literals may not be erased while selecting a branch.
fn pattern(pat: &Pat, value: &str, bindings: &mut Vec<(String, String)>) -> Result<String, String> {
    match pat {
        Pat::Wild => Ok("true".into()),
        Pat::Var(name) => {
            bindings.push((name.clone(), value.into()));
            Ok("true".into())
        }
        Pat::Lit(Literal::Int(n)) => Ok(format!("(= {value} {n})")),
        Pat::Lit(Literal::Bool(b)) => Ok(format!("(= {value} {b})")),
        Pat::Con(ctor, fields) if fields.is_empty() && (ctor == "True" || ctor == "False") => {
            Ok(format!("(= {value} {})", ctor == "True"))
        }
        Pat::Con(ctor, fields) => {
            let mut conditions = vec![format!("((_ is {}) {value})", smt_symbol(ctor))];
            for (index, field) in fields.iter().enumerate() {
                conditions.push(pattern(
                    field,
                    &format!("({} {value})", smt_symbol(&format!("{ctor}_f{index}"))),
                    bindings,
                )?);
            }
            Ok(and(conditions))
        }
        Pat::NamedCon(ctor, fields) => {
            let mut conditions = vec![format!("((_ is {}) {value})", smt_symbol(ctor))];
            for (name, field) in fields {
                conditions.push(pattern(
                    field,
                    &format!("({} {value})", smt_symbol(name)),
                    bindings,
                )?);
            }
            Ok(and(conditions))
        }
        _ => Err("match pattern has no runtime-aligned SMT encoding".into()),
    }
}

pub(super) fn match_value(scrutinee: &str, arms: &[MatchArm]) -> Result<String, String> {
    let mut rest = None;
    for arm in arms.iter().rev() {
        let mut bindings = Vec::new();
        let test = pattern(&arm.pat, scrutinee, &mut bindings)?;
        let condition = and([
            test,
            let_bindings(
                &bindings,
                arm.guard
                    .as_ref()
                    .map(expr_to_smt)
                    .unwrap_or_else(|| "true".into()),
            ),
        ]);
        let value = let_bindings(&bindings, expr_to_smt(&arm.body));
        // No runtime value exists for an unmatched scrutinee. The independent
        // definedness obligation rejects that path; this total SMT term's
        // otherwise value is immaterial there.
        rest = Some(match rest {
            Some(otherwise) => ite(&condition, value, otherwise),
            None => value,
        });
    }
    rest.ok_or_else(|| "empty match has no SMT value".into())
}

pub(super) fn definedness(
    expr: &Expr,
    guarded_functions: &BTreeSet<String>,
) -> Result<String, String> {
    let defined = |expr: &Expr| definedness(expr, guarded_functions);
    match &expr.kind {
        ExprKind::Lit(Literal::Float(_)) => Err(FLOAT_REASON.into()),
        ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Unit => Ok("true".into()),
        ExprKind::Tuple(items) => Ok(and(items
            .iter()
            .map(defined)
            .collect::<Result<Vec<_>, _>>()?)),
        ExprKind::BinOp(op, left, right) => {
            let left_ok = defined(left)?;
            let right_ok = defined(right)?;
            let left_value = expr_to_smt(left);
            let right_value = expr_to_smt(right);
            let condition = match op.as_str() {
                "&&" => return Ok(and([left_ok, ite(&left_value, right_ok, "true".into())])),
                "||" => return Ok(and([left_ok, ite(&left_value, "true".into(), right_ok)])),
                "+" | "-" | "*" => int_domain(&expr_to_smt(expr)),
                "/" | "%" => format!("(and (distinct {right_value} 0) (not (and (= {left_value} (- 9223372036854775808)) (= {right_value} (- 1)))))"),
                _ => "true".into(),
            };
            Ok(and([left_ok, right_ok, condition]))
        }
        ExprKind::UnOp(op, inner) => Ok(and([
            defined(inner)?,
            if op == "-" {
                int_domain(&expr_to_smt(expr))
            } else {
                "true".into()
            },
        ])),
        ExprKind::App(function, args) => {
            let ExprKind::Var(name) = &function.kind else {
                return Err("higher-order arithmetic has no SMT encoding".into());
            };
            let mut conditions = args.iter().map(defined).collect::<Result<Vec<_>, _>>()?;
            if guarded_functions.contains(name) {
                let name = guard_name(name);
                conditions.push(if args.is_empty() {
                    name
                } else {
                    format!(
                        "({name} {})",
                        args.iter().map(expr_to_smt).collect::<Vec<_>>().join(" ")
                    )
                });
            }
            Ok(and(conditions))
        }
        ExprKind::If(condition, yes, no) => Ok(and([
            defined(condition)?,
            ite(&expr_to_smt(condition), defined(yes)?, defined(no)?),
        ])),
        ExprKind::Field(object, _) => defined(object),
        ExprKind::Match(scrutinee, arms) => {
            let value = expr_to_smt(scrutinee);
            let mut rest = "false".into();
            for arm in arms.iter().rev() {
                let mut bindings = Vec::new();
                let test = pattern(&arm.pat, &value, &mut bindings)?;
                let guard_value = let_bindings(
                    &bindings,
                    arm.guard
                        .as_ref()
                        .map(expr_to_smt)
                        .unwrap_or_else(|| "true".into()),
                );
                let guard_defined = let_bindings(
                    &bindings,
                    arm.guard
                        .as_ref()
                        .map(defined)
                        .transpose()?
                        .unwrap_or_else(|| "true".into()),
                );
                let body_defined = let_bindings(&bindings, defined(&arm.body)?);
                let selected = and([test.clone(), guard_value]);
                // The next arm is outside this arm's bindings: a guard that
                // fails must not shadow values referenced by a later arm.
                rest = and([
                    ite(&test, guard_defined, "true".into()),
                    ite(&selected, body_defined, rest),
                ]);
            }
            Ok(and([defined(scrutinee)?, rest]))
        }
        ExprKind::Block(statements) => {
            let mut result = "true".into();
            for statement in statements.iter().rev() {
                result = match statement {
                    Stmt::Bind(Pat::Var(name), _, value) => and([
                        defined(value)?,
                        let_bindings(&[(name.clone(), expr_to_smt(value))], result),
                    ]),
                    Stmt::Expr(value) => and([defined(value)?, result]),
                    _ => return Err("block statement has no runtime arithmetic encoding".into()),
                };
            }
            Ok(result)
        }
        _ => Err("expression has no runtime arithmetic encoding".into()),
    }
}
