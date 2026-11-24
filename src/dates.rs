//! The standard `Date`: the record `# Date(year: Int, month: Int, day: Int)`,
//! its fields in any order. Dates are ordered chronologically, by year, then
//! month, then day: `<`, `<=`, `>` and `>=` compare two dates in the checker,
//! the interpreter, generated Rust and the SMT encoding, and collection order
//! (`sort`, sets, maps) orders dates the same way.

use super::*;

/// A date's fields in chronological order of significance.
const DATE_FIELDS: [&str; 3] = ["year", "month", "day"];

/// The comparison operators that order values.
pub(crate) fn is_ordering_operator(operator: &str) -> bool {
    matches!(operator, "<" | "<=" | ">" | ">=")
}

/// Where year, month and day stand among a standard `Date`'s fields: the
/// order in which two dates compare them. `None` for any other type.
pub fn standard_date_field_order(
    name: &str,
    params: &[Param],
    variants: &[Variant],
) -> Option<[usize; 3]> {
    let [variant] = variants else {
        return None;
    };
    if name != "Date" || variant.name != "Date" || !params.is_empty() || variant.fields.len() != 3 {
        return None;
    }
    let position = |part: &str| {
        variant.fields.iter().position(|field| {
            field.name == part && matches!(&field.ty, Ty::Name(ty) if ty == "Int")
        })
    };
    Some([
        position(DATE_FIELDS[0])?,
        position(DATE_FIELDS[1])?,
        position(DATE_FIELDS[2])?,
    ])
}

/// A standard `Date` value's year, month and day, in that order.
pub(crate) fn date_value_parts(value: &Value) -> Option<[&Value; 3]> {
    let (name, fields) = query::named_fields(value)?;
    if constructor_display_name(name) != "Date" || fields.len() != 3 {
        return None;
    }
    let part = |part: &str| {
        fields
            .iter()
            .find(|(field, _)| field == part)
            .map(|(_, value)| value)
    };
    Some([
        part(DATE_FIELDS[0])?,
        part(DATE_FIELDS[1])?,
        part(DATE_FIELDS[2])?,
    ])
}

/// The program with every comparison of two dates written over their year,
/// month and day, for the SMT encoding, whose `<` compares numbers only.
/// `a < b` becomes `a.year < b.year || a.year == b.year && (a.month < b.month
/// || a.month == b.month && a.day < b.day)`: exact for every three integers,
/// and the order of day numbers for valid dates.
pub fn smt_date_order(mut statements: Vec<Stmt>) -> Vec<Stmt> {
    let mut checker = TypeChecker::new();
    checker.install_constructor_prepass(&statements);
    checker.collect_declarations(&statements);
    if !checker.is_standard_date("Date") {
        return statements;
    }
    checker.prepare_rule_dispatch_metadata(&statements);
    checker.order_dates_in_statements(&mut statements, &mut BTreeMap::new());
    statements
}

fn date_comparison(operator: &str, left: &Expr, right: &Expr, span: Span) -> Expr {
    let (left, right, strict) = match operator {
        "<" => (left, right, true),
        "<=" => (left, right, false),
        ">" => (right, left, true),
        _ => (right, left, false),
    };
    let node = |kind| Expr::new(kind, span);
    let field =
        |value: &Expr, part: &str| node(ExprKind::Field(Box::new(value.clone()), part.to_string()));
    let binary = |operator: &str, left: Expr, right: Expr| {
        node(ExprKind::BinOp(
            operator.to_string(),
            Box::new(left),
            Box::new(right),
        ))
    };
    let [year, month, day] = DATE_FIELDS;
    let last = if strict { "<" } else { "<=" };
    let days = binary(last, field(left, day), field(right, day));
    let months = binary(
        "||",
        binary("<", field(left, month), field(right, month)),
        binary(
            "&&",
            binary("==", field(left, month), field(right, month)),
            days,
        ),
    );
    binary(
        "||",
        binary("<", field(left, year), field(right, year)),
        binary(
            "&&",
            binary("==", field(left, year), field(right, year)),
            months,
        ),
    )
}

impl TypeChecker {
    /// Whether `type_name` is the standard `Date`.
    pub(crate) fn is_standard_date(&self, type_name: &str) -> bool {
        let Ok(mut ty) = parse_type_annotation(type_name) else {
            return false;
        };
        while let Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) = ty {
            ty = *inner;
        }
        let Ty::Name(name) = ty else {
            return false;
        };
        name == "Date"
            && self
                .type_parameters_by_owner
                .get(&name)
                .is_none_or(Vec::is_empty)
            // A record: the checker lists the variants of sum types only.
            && !self.type_variants.contains_key(&name)
            && self.type_field_tys.get(&name).is_some_and(|fields| {
                fields.len() == 3
                    && DATE_FIELDS.iter().all(|part| {
                        matches!(fields.get(*part), Some(Ty::Name(ty)) if ty == "Int")
                    })
            })
    }

    fn order_dates_in_statements(
        &self,
        statements: &mut [Stmt],
        locals: &mut BTreeMap<String, String>,
    ) {
        for statement in statements {
            match statement {
                Stmt::Defn(Defn::Fn { params, body, .. }) => {
                    let mut locals = locals.clone();
                    for param in params.iter() {
                        if let Some(ty) = &param.ty {
                            locals.insert(param.name.clone(), ty.to_string());
                        }
                    }
                    self.order_dates(body, &locals);
                }
                Stmt::Defn(Defn::Module { body, .. }) => {
                    self.order_dates_in_statements(body, &mut locals.clone());
                }
                Stmt::TypeDecl(TypeDecl::RuleScope { params, body, .. }) => {
                    let mut locals = locals.clone();
                    for param in params.iter() {
                        if let Some(ty) = &param.ty {
                            locals.insert(param.name.clone(), ty.to_string());
                        }
                    }
                    self.order_dates_in_statements(body, &mut locals);
                }
                Stmt::Rule(rule) => {
                    let mut locals = locals.clone();
                    if let Some(head) = rule.head() {
                        locals.extend(self.rule_head_local_types(head));
                    }
                    match rule {
                        Rule::Clause { body, .. } => {
                            if let Some(body) = body {
                                self.order_dates(body, &locals);
                            }
                        }
                        Rule::Default {
                            value, condition, ..
                        }
                        | Rule::Exception {
                            value, condition, ..
                        } => {
                            self.order_dates(value, &locals);
                            if let Some(condition) = condition {
                                self.order_dates(condition, &locals);
                            }
                        }
                        Rule::ReactiveScope { body, .. } => {
                            self.order_dates_in_statements(body, &mut locals);
                        }
                    }
                }
                Stmt::Bind(pattern, ty, value) => {
                    self.order_dates(value, locals);
                    let bound = ty
                        .as_ref()
                        .map(ToString::to_string)
                        .or_else(|| self.infer_expr_type_name_with_locals(value, locals));
                    locals.extend(self.pattern_type_bindings(pattern, bound.as_deref()));
                }
                Stmt::Invariant {
                    subject, predicate, ..
                } => {
                    self.order_dates(subject, locals);
                    self.order_dates(predicate, locals);
                }
                Stmt::Expr(expression) => self.order_dates(expression, locals),
                _ => {}
            }
        }
    }

    fn order_dates(&self, expression: &mut Expr, locals: &BTreeMap<String, String>) {
        let span = expression.span;
        match &mut expression.kind {
            ExprKind::BinOp(operator, left, right) => {
                self.order_dates(left, locals);
                self.order_dates(right, locals);
                let is_date = |operand: &Expr| {
                    self.infer_expr_type_name_with_locals(operand, locals)
                        .is_some_and(|ty| self.is_standard_date(&ty))
                };
                if is_ordering_operator(operator) && is_date(left) && is_date(right) {
                    *expression = date_comparison(operator, left, right, span);
                }
            }
            ExprKind::Block(statements) => {
                self.order_dates_in_statements(statements, &mut locals.clone());
            }
            ExprKind::Match(scrutinee, arms) => {
                self.order_dates(scrutinee, locals);
                let subject = self.infer_expr_type_name_with_locals(scrutinee, locals);
                for arm in arms {
                    let mut locals = locals.clone();
                    locals.extend(self.pattern_type_bindings(&arm.pat, subject.as_deref()));
                    if let Some(guard) = &mut arm.guard {
                        self.order_dates(guard, &locals);
                    }
                    self.order_dates(&mut arm.body, &locals);
                }
            }
            ExprKind::Lambda(params, body) => {
                let mut locals = locals.clone();
                for param in params.iter() {
                    match &param.ty {
                        Some(ty) => locals.insert(param.name.clone(), ty.to_string()),
                        None => locals.remove(&param.name),
                    };
                }
                self.order_dates(body, &locals);
            }
            ExprKind::App(function, arguments) => {
                self.order_dates(function, locals);
                for argument in arguments {
                    self.order_dates(argument, locals);
                }
            }
            ExprKind::If(condition, then_branch, else_branch) => {
                self.order_dates(condition, locals);
                self.order_dates(then_branch, locals);
                self.order_dates(else_branch, locals);
            }
            ExprKind::UnOp(_, inner) | ExprKind::Try(inner) | ExprKind::Field(inner, _) => {
                self.order_dates(inner, locals);
            }
            ExprKind::Index(left, right) | ExprKind::Pipe(left, right) => {
                self.order_dates(left, locals);
                self.order_dates(right, locals);
            }
            ExprKind::List(items)
            | ExprKind::Tuple(items)
            | ExprKind::Conjunction(items)
            | ExprKind::Disjunction(items)
            | ExprKind::Effect(_, items) => {
                for item in items {
                    self.order_dates(item, locals);
                }
            }
            ExprKind::Handle { .. } | ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
        }
    }
}
