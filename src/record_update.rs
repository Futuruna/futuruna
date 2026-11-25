//! Record update: `base with { field = value, ... }`.
//!
//! The value is a copy of the record `base` with the named fields replaced.
//! The parser writes it as `App(Var(RECORD_UPDATE_MARKER), [base,
//! named_arg(field, value), ...])`. The checker accepts it when the base's
//! type is known and is a record with one constructor and named fields,
//! every named field exists and appears once, and each value has its field's
//! type (units, named scalars and the base's type arguments included). The
//! expression has the base's type.
//!
//! The checker records each update with its record's constructor and
//! fields, and elaboration writes it as that constructor applied to every
//! field: the new value where the update names it, the base's field
//! otherwise. `case with { offeret_lå_ned = Ja }` on a `Legemsangreb` becomes
//! `Legemsangreb(case.a, ..., Ja, ...)`. A base that is not a name or a field
//! of one is computed once, in a block. The interpreter, generated Rust and
//! WebAssembly, the solver and the audits therefore read an ordinary
//! constructor call. Explore evaluates the checked program before
//! elaboration; its checked resolutions record the update as an internal
//! marker over its base and new values, and the interpreter evaluates it
//! directly (`Interpreter::eval_record_update`), with the same result.

use super::unit_types::{rewrite_stmts, ProgramRewrite};
use super::*;

/// A checked record update: the record's type and constructor and every
/// field in declaration order.
#[derive(Debug, Clone)]
pub(crate) struct RecordUpdate {
    owner: String,
    constructor: String,
    fields: Vec<String>,
}

/// The local that holds a base which is not a plain name.
const UPDATED_RECORD: &str = "__updated_record";

/// The single constructor of a record type: its name and signature.
type RecordConstructor = (String, TypeConstructorSignature);

impl TypeChecker {
    /// Every constructor declared for `owner`, with its signature.
    fn constructors_of(&self, owner: &str) -> Vec<RecordConstructor> {
        let mut constructors = self
            .constructor_signatures
            .iter()
            .flat_map(|(constructor, signatures)| {
                signatures
                    .iter()
                    .filter(|signature| signature.parent == owner)
                    .map(|signature| (constructor.clone(), signature.clone()))
            })
            .collect::<Vec<_>>();
        if let Some(variants) = self.type_variants.get(owner) {
            constructors.sort_by_key(|(constructor, _)| {
                variants
                    .iter()
                    .position(|variant| variant == constructor)
                    .unwrap_or(usize::MAX)
            });
        }
        constructors
    }

    /// The one constructor of the record `owner` whose fields `with` may
    /// replace, or why there is none.
    pub(crate) fn record_update_constructor(
        &self,
        owner: &str,
        keyword: &str,
    ) -> Result<RecordConstructor, String> {
        let constructors = self.constructors_of(owner);
        match constructors.as_slice() {
            [] => Err(format!(
                "`{keyword}` replaces fields of a record; `{owner}` is not a record type"
            )),
            [(constructor, signature)] => {
                if signature.positional || signature.fields.is_empty() {
                    Err(format!(
                        "`{keyword}` names the fields it replaces, and `{owner}` has no named fields; build the new value with `{constructor}(...)`"
                    ))
                } else {
                    Ok((constructor.clone(), signature.clone()))
                }
            }
            several => Err(format!(
                "`{keyword}` replaces fields of a record with one constructor; `{owner}` has the constructors {}, and which fields a value has depends on its constructor; build the new value with its constructor",
                several
                    .iter()
                    .map(|(constructor, _)| format!("`{constructor}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// The keyword the source writes for a record update: `with`, or `med`
    /// in a Danish file.
    fn record_update_keyword(&self, expr: &Expr) -> String {
        let ExprKind::App(marker, _) = &expr.kind else {
            return "with".to_string();
        };
        let span = marker.span;
        if span.is_dummy() {
            return "with".to_string();
        }
        let written = self
            .source_text
            .chars()
            .skip(span.start)
            .take(span.end.saturating_sub(span.start))
            .collect::<String>();
        if written.is_empty() {
            "with".to_string()
        } else {
            written
        }
    }

    /// Check `base with { field = value, ... }` and record it for
    /// elaboration.
    pub(super) fn check_record_update(
        &mut self,
        expr: &Expr,
        base: &Expr,
        updates: &[Expr],
        in_fn: Option<&str>,
    ) {
        self.check_expr(base, in_fn);
        for update in updates {
            self.check_expr(
                named_arg_parts(update).map_or(update, |(_, value)| value),
                in_fn,
            );
        }
        let keyword = self.record_update_keyword(expr);
        let Some(base_type) = self.ordinary_expression_type(base) else {
            self.error_at_expr(
                base,
                format!(
                    "`{keyword}` needs the record type of the value it copies, and the type of this value is not known here; give the binding or parameter its record type"
                ),
            );
            return;
        };
        let Some(owner) = Self::canonical_nominal_owner(&base_type) else {
            self.error_at_expr(
                base,
                format!(
                    "`{keyword}` replaces fields of a record; `{base_type}` is not a record type"
                ),
            );
            return;
        };
        let (constructor, signature) = match self.record_update_constructor(&owner, &keyword) {
            Ok(found) => found,
            Err(message) => {
                self.error_at_expr(base, message);
                return;
            }
        };
        if let Some(parameters) = self.type_parameters_by_owner.get(&owner) {
            let arguments_known = match parse_type_annotation(&base_type) {
                Ok(Ty::App(_, arguments)) => {
                    arguments.len() == parameters.len() && arguments.iter().all(|argument| {
                        !matches!(argument, Ty::Hole)
                            && !matches!(argument, Ty::Var(name) | Ty::Name(name) if name == "_")
                    })
                }
                _ => parameters.is_empty(),
            };
            if !arguments_known {
                self.error_at_expr(
                    base,
                    format!(
                        "`{keyword}` needs the type arguments of `{owner}`, and they are not known here; give the binding or parameter its full type, such as `{owner}({})`",
                        parameters.join(", ")
                    ),
                );
                return;
            }
        }
        let field_list = signature
            .fields
            .iter()
            .map(|field| format!("`{field}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut seen = BTreeSet::new();
        let mut valid = true;
        for update in updates {
            let Some((field, value)) = named_arg_parts(update) else {
                continue;
            };
            let Some(index) = signature.fields.iter().position(|known| known == field) else {
                self.error_at_expr(
                    update,
                    format!("record `{owner}` has no field `{field}`; its fields are {field_list}"),
                );
                valid = false;
                continue;
            };
            if !seen.insert(field.to_string()) {
                self.error_at_expr(
                    update,
                    format!("record `{owner}` field `{field}` is replaced more than once"),
                );
                valid = false;
                continue;
            }
            let Some(declared) = signature
                .field_tys
                .get(index)
                .and_then(|ty| ty.as_deref())
                .and_then(|ty| parse_type_annotation(ty).ok())
            else {
                continue;
            };
            let Ok(expected) = parse_type_annotation(
                &self.canonical_instantiate_nominal_type(&base_type, &declared),
            ) else {
                continue;
            };
            if self.unit_literal_fits(value, &expected) {
                continue;
            }
            let Some(actual) = self.ordinary_expression_type(value) else {
                continue;
            };
            let Ok(actual_ty) = parse_type_annotation(&actual) else {
                continue;
            };
            if !self.ordinary_argument_matches(&actual_ty, &expected, &mut BTreeMap::new()) {
                self.error_at_expr(
                    value,
                    format!(
                        "record `{owner}` field `{field}` expects `{}`, got `{actual}`{}",
                        Self::canonical_explore_ty_name(&expected),
                        self.numeric_conversion_hint(&actual_ty, &expected)
                    ),
                );
                valid = false;
            }
        }
        if !valid {
            return;
        }
        if expr.span.is_dummy() {
            self.error_at_expr(
                expr,
                format!("`{keyword}` needs a position in the source to be elaborated; write it in the source"),
            );
            return;
        }
        self.record_updates
            .entry(self.unit_conversion_module.clone())
            .or_default()
            .insert(
                (expr.span.start, expr.span.end),
                RecordUpdate {
                    owner,
                    constructor,
                    fields: signature.fields,
                },
            );
    }

    /// Whether a checked record update waits for elaboration.
    pub(crate) fn has_record_updates(&self) -> bool {
        !self.record_updates.is_empty()
    }

    /// Write every checked record update as its record's constructor call,
    /// in the root program and in each imported file. The constructor is
    /// named as elaboration names it (a shared constructor by its internal
    /// name). The call keeps the update's source span.
    pub(crate) fn elaborate_record_updates(
        &mut self,
        root: &[Stmt],
        elaboration: &mut ConstructorElaboration,
    ) {
        for (module, updates) in std::mem::take(&mut self.record_updates) {
            let updates = updates
                .into_iter()
                .map(|(span, update)| {
                    let identity =
                        self.constructor_identity_owner(&update.owner, &update.constructor);
                    let constructor =
                        elaboration.constructor_runtime_name(&identity, &update.constructor);
                    (span, (constructor, update.fields))
                })
                .collect();
            let mut rewrite = RecordUpdateElaboration { updates };
            elaboration.rewrite_file(module.as_deref(), root, |statements| {
                rewrite_stmts(statements, &mut rewrite)
            });
        }
    }
}

/// The checked record updates of one file, by source span: the
/// constructor's runtime name and its fields. Each is rewritten once.
struct RecordUpdateElaboration {
    updates: BTreeMap<(usize, usize), (String, Vec<String>)>,
}

impl ProgramRewrite for RecordUpdateElaboration {
    fn expr(&mut self, expr: &mut Expr) {
        if record_update_parts(expr).is_none() {
            return;
        }
        let span = expr.span;
        let Some((constructor, fields)) = self.updates.remove(&(span.start, span.end)) else {
            return;
        };
        let ExprKind::App(_, arguments) = std::mem::replace(&mut expr.kind, ExprKind::Unit) else {
            unreachable!("a record update is an application");
        };
        *expr = elaborate(constructor, &fields, arguments, span);
    }
}

/// `Record(f1, ..., fn)` with each field the new value the update names, or
/// the base's field. A base that is not a name or a field of one is bound
/// first, so it is computed once.
fn elaborate(constructor: String, fields: &[String], arguments: Vec<Expr>, span: Span) -> Expr {
    let at = |kind| Expr::new(kind, span);
    let mut arguments = arguments.into_iter();
    let base = arguments.next().expect("a record update has a base");
    let mut values = arguments
        .filter_map(|update| {
            let ExprKind::App(_, mut parts) = update.kind else {
                return None;
            };
            let value = parts.pop()?;
            let ExprKind::Lit(Literal::Str(field)) = parts.pop()?.kind else {
                return None;
            };
            Some((field, value))
        })
        .collect::<BTreeMap<_, _>>();
    let named = is_path(&base);
    let record = if named {
        base.clone()
    } else {
        at(ExprKind::Var(format!("{UPDATED_RECORD}_{}", span.start)))
    };
    let call = at(ExprKind::App(
        Box::new(at(ExprKind::Var(constructor))),
        fields
            .iter()
            .map(|field| {
                values
                    .remove(field)
                    .unwrap_or_else(|| at(ExprKind::Field(Box::new(record.clone()), field.clone())))
            })
            .collect(),
    ));
    if named {
        return call;
    }
    at(ExprKind::Block(vec![
        Stmt::Bind(
            Pat::Var(format!("{UPDATED_RECORD}_{}", span.start)),
            None,
            base,
        ),
        Stmt::Expr(call),
    ]))
}

/// A name, or a field of one: reading it twice reads the same value.
fn is_path(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Var(_) => true,
        ExprKind::Field(base, _) => is_path(base),
        _ => false,
    }
}

impl Interpreter {
    /// `base with { field = value }` in a program run as checked, before
    /// elaboration (Explore evaluates its checked program so): the base's
    /// record with the named fields replaced. The base is evaluated first,
    /// then the new values in written order.
    pub(crate) fn eval_record_update(&mut self, expression: &Expr, env: &Env) -> Value {
        let Some((base, updates)) = record_update_parts(expression) else {
            return self.panic_or_ground_fail("malformed record update");
        };
        let record = self.eval(base, env);
        let mut values = Vec::with_capacity(updates.len());
        for update in updates {
            let Some((field, value)) = named_arg_parts(update) else {
                return self.panic_or_ground_fail("malformed record update");
            };
            values.push((field, self.eval(value, env)));
        }
        let replace = |fields: &[(String, Value)]| {
            let mut fields = fields.to_vec();
            for (field, value) in values {
                let slot = fields.iter_mut().find(|(name, _)| name == field)?;
                slot.1 = value;
            }
            Some(Rc::new(fields))
        };
        let updated = match &record {
            Value::NamedConstructor(name, fields) => {
                replace(fields).map(|fields| Value::NamedConstructor(name.clone(), fields))
            }
            Value::NamespacedNamedConstructor {
                namespace,
                name,
                fields,
                declaration_env,
            } => replace(fields).map(|fields| Value::NamespacedNamedConstructor {
                namespace: namespace.clone(),
                name: name.clone(),
                fields,
                declaration_env: declaration_env.clone(),
            }),
            _ => None,
        };
        updated.unwrap_or_else(|| {
            self.panic_or_ground_fail(format!(
                "a record update replaces named fields of a record, got {record}"
            ))
        })
    }
}
