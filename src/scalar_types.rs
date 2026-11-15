//! Named scalar types.
//!
//! `# Isin: String "International Securities Identification Number" where
//! isin_shape, isin_check_digit` declares `Isin`, a type whose values are the
//! Strings that pass every check. A value is its primitive (`String` or
//! `Int`) at runtime, on the wire and in proofs; only the checker sees the
//! name:
//!
//! - an `Isin` is accepted where a `String` is expected, also inside `List`,
//!   `Set`, `Option` and `Map`; a plain `String` or an `AccountId` is not an
//!   `Isin`;
//! - a literal takes the named scalar its place expects, and `runa check`
//!   runs the checks on it;
//! - an `Isin` compares with an `Isin`, a `String` or a literal, never with
//!   an `AccountId`; every other operator applies to the primitive and gives
//!   the primitive;
//! - `Isin(p)` and `parse(Isin, p)` make one after the checks; elaboration
//!   writes each as the checks it stands for, so every backend runs one
//!   definition of them. `String(isin)` and `Int(id)` take the name off.
//!
//! As elsewhere in the checker, an operand of unknown type is not evidence of
//! a mistake.

use super::unit_types::{rewrite_stmts, ProgramRewrite};
use super::*;

/// A declared named scalar type.
#[derive(Debug, Clone)]
pub(crate) struct ScalarType {
    pub(crate) primitive: String,
    pub(crate) description: String,
    pub(crate) checks: Vec<ScalarCheck>,
    pub(crate) field_meta: Option<FieldMetaDecl>,
}

/// A checked `Isin(p)` or `parse(Isin, p)`, written as its checks by
/// elaboration.
#[derive(Debug, Clone)]
pub(crate) struct ScalarConversion {
    scalar: String,
    primitive: String,
    checks: Vec<ScalarCheck>,
    /// `parse(Isin, p)` gives a `Result`; `Isin(p)` stops when a check is
    /// false.
    parse: bool,
}

/// A literal that takes a named scalar type, waiting for its checks.
#[derive(Debug, Clone)]
pub(crate) struct ScalarLiteral {
    scalar: String,
    value: Literal,
    span: Span,
    context: Vec<String>,
    /// Compared with a value of the type (`isin == "DK0060534915"`).
    compared: bool,
}

/// What the checks say about one literal.
#[derive(Debug, Clone)]
enum Verdict {
    Holds,
    /// The first check that is false.
    False(String),
    /// A check that could not run on the literal before the program runs.
    Failed(String),
}

/// Runs the checks of named scalars on literals with the program's own
/// definitions, before the program runs.
pub(crate) struct ScalarLiteralEvaluator {
    evaluator: MetaGroundEvaluator,
    verdicts: BTreeMap<(String, String), Verdict>,
}

impl fmt::Debug for ScalarLiteralEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScalarLiteralEvaluator")
            .field("verdicts", &self.verdicts)
            .finish()
    }
}

/// The local that holds the value a conversion checks.
const SCALAR_VALUE: &str = "__scalar_value";

/// `a` or `an` before a type name.
pub(crate) fn article(name: &str) -> &'static str {
    match name.chars().next() {
        Some('A' | 'E' | 'I' | 'O' | 'U') => "an",
        _ => "a",
    }
}

/// A value as the source would write it: `"DK0060534915"`, `7`.
fn show_literal(value: &Literal) -> String {
    match value {
        Literal::Str(text) => format!("\"{text}\""),
        other => other.to_string(),
    }
}

/// The message for a value that fails a check, as the checker, a conversion
/// and the calculation boundary report it.
pub(crate) fn not_a_scalar_message(shown_value: &str, scalar: &str, check: &str) -> String {
    format!(
        "`{shown_value}` is not {} `{scalar}`: `{check}` is false",
        article(scalar)
    )
}

impl TypeChecker {
    /// The primitive (`String` or `Int`) of a named scalar type.
    pub(super) fn scalar_primitive(&self, name: &str) -> Option<&str> {
        self.scalar_types
            .get(name)
            .map(|scalar| scalar.primitive.as_str())
    }

    /// Every declared named scalar type.
    pub(crate) fn scalar_declarations(&self) -> &BTreeMap<String, ScalarType> {
        &self.scalar_types
    }

    /// Record `# Name: Primitive "description" where ...` in the declaration
    /// pass.
    pub(super) fn collect_scalar_declaration(
        &mut self,
        name: &str,
        primitive: &str,
        description: &str,
        checks: &[ScalarCheck],
        field_meta: Option<&FieldMetaDecl>,
    ) {
        let key = format!("type:{name}");
        if let Some(known) = self.scalar_types.get(name) {
            let same = known.primitive == primitive
                && known.description == description
                && known.checks.len() == checks.len()
                && known
                    .checks
                    .iter()
                    .zip(checks)
                    .all(|(known, check)| known.text == check.text);
            if !same {
                self.declaration_error(
                    &key,
                    format!("named scalar `{name}` is declared twice, differently; declare it once"),
                );
            }
            return;
        }
        if declaration_contracts::is_builtin_type_name(name) {
            self.declaration_error(
                &key,
                format!("named scalar `{name}` would rename the built-in type `{name}`; choose another name"),
            );
            return;
        }
        if self.types.contains(name) || self.constructors.contains_key(name) {
            let what = if self.is_unit_type(name) {
                "a unit"
            } else {
                "a type or constructor"
            };
            self.declaration_error(
                &key,
                format!("named scalar `{name}` has the name of {what} that is already declared; choose another name"),
            );
            return;
        }
        self.types.insert(name.to_string());
        self.scalar_types.insert(
            name.to_string(),
            ScalarType {
                primitive: primitive.to_string(),
                description: description.to_string(),
                checks: checks.to_vec(),
                field_meta: field_meta.cloned(),
            },
        );
    }

    /// Each check is a declared rule or function that takes the primitive
    /// first and returns `Bool`; its further arguments are checked as
    /// ordinary expressions. The type's `@ field` describes fields, so it
    /// holds text and sources but no `key`: a key belongs to the record.
    pub(super) fn check_scalar_declaration(&mut self, name: &str) {
        let Some(scalar) = self.scalar_types.get(name).cloned() else {
            return;
        };
        for check in &scalar.checks {
            if !self.functions.contains_key(&check.name) && !self.var_defined(&check.name) {
                self.error_at_span(
                    check.span,
                    format!(
                        "check `{}` of `{name}` is not a declared rule or function; a check is a pure rule or function that takes the value first and returns `Bool`",
                        check.name
                    ),
                );
                continue;
            }
            let at = |kind| Expr::new(kind, check.span);
            let mut arguments = vec![at(ExprKind::Var(SCALAR_VALUE.to_string()))];
            arguments.extend(check.arguments.iter().cloned());
            let call = at(ExprKind::App(
                Box::new(at(ExprKind::Var(check.name.clone()))),
                arguments,
            ));
            self.push_scope();
            self.define_var(SCALAR_VALUE);
            self.define_var_type(SCALAR_VALUE, &Ty::Name(scalar.primitive.clone()));
            self.push_context(format!("in check `{}` of `{name}`", check.text));
            self.check_expr(&call, None);
            let result = self.ordinary_expression_type(&call);
            self.pop_context();
            self.pop_scope();
            if let Some(result) = result.filter(|result| result != "Bool") {
                self.error_at_span(
                    check.span,
                    format!(
                        "check `{}` of `{name}` returns `{result}`; a check returns `Bool`",
                        check.text
                    ),
                );
            }
        }
        let Some(meta) = &scalar.field_meta else {
            return;
        };
        for (key, value) in &meta.entries {
            if key == "key" {
                self.error_at_expr(
                    value,
                    format!("`key` belongs to the record that holds a `{name}`, not to `{name}`; write `@ field(key = True)` on the field"),
                );
                continue;
            }
            self.check_expr(value, None);
            let Some(type_name) = self.infer_expr_type_name(value) else {
                continue;
            };
            let expected = if key == "sources" { "List" } else { "String" };
            let fits = if key == "sources" {
                type_name.starts_with("List(")
            } else {
                type_name == "String"
            };
            if !fits {
                self.error_at_expr(
                    value,
                    format!("`@ field` key `{key}` must be a `{expected}`, got `{type_name}`"),
                );
            }
        }
    }

    /// A type name with every named scalar replaced by its primitive.
    pub(super) fn widen_scalars(&self, type_name: &str) -> String {
        if self.scalar_types.is_empty() {
            return type_name.to_string();
        }
        if let Some(primitive) = self.scalar_primitive(type_name) {
            return primitive.to_string();
        }
        match parse_type_annotation(type_name) {
            Ok(mut ty) if self.contains_scalar(&ty) => {
                self.widen_ty(&mut ty);
                Self::canonical_explore_ty_name(&ty)
            }
            _ => type_name.to_string(),
        }
    }

    fn widen_ty(&self, ty: &mut Ty) {
        match ty {
            Ty::Name(name) => {
                if let Some(primitive) = self.scalar_primitive(name) {
                    *name = primitive.to_string();
                }
            }
            Ty::App(head, arguments) => {
                self.widen_ty(head);
                arguments.iter_mut().for_each(|argument| self.widen_ty(argument));
            }
            Ty::Arrow(input, output) => {
                self.widen_ty(input);
                self.widen_ty(output);
            }
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
                self.widen_ty(inner)
            }
            Ty::Var(_) | Ty::Unit | Ty::Hole => {}
        }
    }

    fn contains_scalar(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Name(name) => self.scalar_primitive(name).is_some(),
            Ty::App(head, arguments) => {
                self.contains_scalar(head) || arguments.iter().any(|a| self.contains_scalar(a))
            }
            Ty::Arrow(input, output) => self.contains_scalar(input) || self.contains_scalar(output),
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
                self.contains_scalar(inner)
            }
            Ty::Var(_) | Ty::Unit | Ty::Hole => false,
        }
    }

    /// A type variable bound to a named scalar meets that scalar's
    /// primitive: the variable is the primitive.
    pub(super) fn scalar_join_with_primitive(&self, known: &Ty, actual: &Ty) -> Option<Ty> {
        match (known, actual) {
            (Ty::Name(known), Ty::Name(actual))
                if self.scalar_primitive(known) == Some(actual.as_str()) =>
            {
                Some(Ty::Name(actual.clone()))
            }
            _ => None,
        }
    }

    /// The advice after "expects `Isin`, got `...`".
    pub(super) fn scalar_mismatch_hint(&self, actual: &str, expected: &str) -> String {
        let Some(primitive) = self.scalar_primitive(expected) else {
            return String::new();
        };
        if self.scalar_primitive(actual).is_some() {
            format!(
                "; `{actual}` and `{expected}` are different named scalars: when the value is {} `{expected}`, convert it with `{expected}({primitive}(...))`",
                article(expected)
            )
        } else if actual == primitive {
            format!(
                "; write `{expected}(value)` or `parse({expected}, value)`, which check that the value is {} `{expected}`",
                article(expected)
            )
        } else {
            String::new()
        }
    }

    /// The value of a literal of `primitive`: `"DK0060534915"` for a
    /// `String`, `7` or `-7` for an `Int`.
    fn scalar_literal_value(expr: &Expr, primitive: &str) -> Option<Literal> {
        let expr = named_arg_parts(expr).map_or(expr, |(_, value)| value);
        match (&expr.kind, primitive) {
            (ExprKind::Lit(Literal::Str(text)), "String") => Some(Literal::Str(text.clone())),
            (ExprKind::Lit(Literal::Int(value)), "Int") => Some(Literal::Int(*value)),
            (ExprKind::UnOp(operator, inner), "Int") if operator == "-" => match &inner.kind {
                ExprKind::Lit(Literal::Int(value)) => Some(Literal::Int(-value)),
                _ => None,
            },
            (ExprKind::Block(stmts), _) => match stmts.as_slice() {
                [Stmt::Expr(inner)] => Self::scalar_literal_value(inner, primitive),
                _ => None,
            },
            _ => None,
        }
    }

    /// A literal of the scalar's primitive takes the scalar type; `runa
    /// check` runs the checks on it.
    pub(super) fn scalar_literal_fits(&self, expr: &Expr, scalar: &str) -> bool {
        self.record_scalar_literal(expr, scalar, false)
    }

    fn record_scalar_literal(&self, expr: &Expr, scalar: &str, compared: bool) -> bool {
        let Some(declaration) = self.scalar_types.get(scalar) else {
            return false;
        };
        let Some(value) = Self::scalar_literal_value(expr, &declaration.primitive) else {
            return false;
        };
        if !declaration.checks.is_empty() && !expr.span.is_dummy() {
            self.scalar_literals.borrow_mut().push(ScalarLiteral {
                scalar: scalar.to_string(),
                value,
                span: expr.span,
                context: self.error_context.clone(),
                compared,
            });
        }
        true
    }

    /// Prepare to run the checks on literals with the root program's
    /// definitions.
    pub(super) fn begin_scalar_literal_checks(&mut self, stmts: &[Stmt]) {
        if self.scalar_evaluator.is_some()
            || self
                .scalar_types
                .values()
                .all(|scalar| scalar.checks.is_empty())
        {
            return;
        }
        self.scalar_evaluator = Some(ScalarLiteralEvaluator {
            evaluator: MetaGroundEvaluator::new(stmts, self.source_dir.as_deref()),
            verdicts: BTreeMap::new(),
        });
    }

    /// Run the checks on the literals recorded since the last call and
    /// report each that fails, once.
    pub(super) fn report_scalar_literals(&mut self) {
        let pending = std::mem::take(&mut *self.scalar_literals.borrow_mut());
        let mut reported = BTreeSet::new();
        for literal in pending {
            if !reported.insert((literal.span.start, literal.span.end, literal.scalar.clone())) {
                continue;
            }
            let Some(scalar) = self.scalar_types.get(&literal.scalar) else {
                continue;
            };
            let Some(evaluator) = self.scalar_evaluator.as_mut() else {
                continue;
            };
            let verdict = evaluator.verdict(&literal.scalar, scalar, &literal.value);
            let shown = show_literal(&literal.value);
            let message = match verdict {
                Verdict::Holds => continue,
                Verdict::False(check) => {
                    let mut message = not_a_scalar_message(&shown, &literal.scalar, &check);
                    if literal.compared {
                        message.push_str(&format!(
                            "; no `{}` equals it",
                            literal.scalar
                        ));
                    }
                    message
                }
                Verdict::Failed(check) => format!(
                    "check `{check}` of `{}` could not run on `{shown}` before the program runs; a check is a pure rule or function that returns `Bool` for every {}",
                    literal.scalar, scalar.primitive
                ),
            };
            let previous = std::mem::replace(&mut self.error_context, literal.context);
            self.error_at_span(literal.span, message);
            self.error_context = previous;
        }
    }

    /// `isin == account_id` compares two named scalars, which no value is
    /// both of; `isin == "dk"` compares with a literal that the checks
    /// reject. Returns true when it reported the operands.
    pub(super) fn check_scalar_operands(
        &mut self,
        expression: &Expr,
        operator: &str,
        left: &Expr,
        left_type: Option<&str>,
        right: &Expr,
        right_type: Option<&str>,
    ) -> bool {
        if self.scalar_types.is_empty()
            || !matches!(operator, "==" | "!=" | "<" | ">" | "<=" | ">=")
        {
            return false;
        }
        let left_scalar = left_type.filter(|ty| self.scalar_primitive(ty).is_some());
        let right_scalar = right_type.filter(|ty| self.scalar_primitive(ty).is_some());
        match (left_scalar, right_scalar) {
            (Some(l), Some(r)) if l != r => {
                let message = format!(
                    "unsupported operands for operator `{operator}`: `{l}` and `{r}` are different named scalars, and no value is both; compare their text with `String(...)` when that is meant"
                );
                self.error_at_expr(expression, message);
                true
            }
            (Some(scalar), None) => {
                self.record_scalar_literal(right, scalar, true);
                false
            }
            (None, Some(scalar)) => {
                self.record_scalar_literal(left, scalar, true);
                false
            }
            _ => false,
        }
    }

    /// The type of a call that is a named scalar conversion: `Isin(p)` is an
    /// `Isin`, `String(isin)` a `String`, `parse(Isin, p)` a
    /// `Result(Isin, String)`.
    pub(super) fn scalar_conversion_result(
        &self,
        name: &str,
        args: &[Expr],
        locals: &BTreeMap<String, String>,
    ) -> Option<String> {
        if self.scalar_types.is_empty() || locals.contains_key(name) {
            return None;
        }
        match args {
            [_] if self.scalar_primitive(name).is_some() => Some(name.to_string()),
            [_] if name == "String" => Some("String".to_string()),
            [target, _] if name == "parse" && !self.user_functions.contains("parse") => {
                match &target.kind {
                    ExprKind::Var(scalar)
                        if self.scalar_primitive(scalar).is_some()
                            && !locals.contains_key(scalar) =>
                    {
                        Some(format!("Result({scalar}, String)"))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Check `Isin(p)`, `String(isin)` and `parse(Isin, p)`, and record the
    /// conversions that run checks for elaboration. Returns true when the
    /// call is one of them.
    pub(super) fn check_scalar_conversion_call(
        &mut self,
        expr: &Expr,
        name: &str,
        args: &[Expr],
        in_fn: Option<&str>,
    ) -> bool {
        if self.scalar_types.is_empty() || self.var_defined(name) {
            return false;
        }
        if name == "parse" && !self.user_functions.contains("parse") {
            return self.check_scalar_parse_call(expr, args, in_fn);
        }
        let scalar = self.scalar_types.get(name).cloned();
        if scalar.is_none() && name != "String" {
            return false;
        }
        for argument in args {
            self.check_expr(
                named_arg_parts(argument).map_or(argument, |(_, value)| value),
                in_fn,
            );
        }
        let [argument] = args else {
            self.error_at_expr(
                expr,
                format!("`{name}(...)` takes one value, got {} arguments", args.len()),
            );
            return true;
        };
        if is_named_arg_expr(argument) {
            self.error_at_expr(argument, format!("`{name}(...)` takes one value without a name"));
            return true;
        }
        let Some(scalar) = scalar else {
            // `String(isin)` takes the name off a String scalar.
            if let Some(actual) = self.ordinary_expression_type(argument) {
                if actual != "String" && self.scalar_primitive(&actual) != Some("String") {
                    self.error_at_expr(
                        argument,
                        format!("`String(...)` takes the name off a named scalar over `String`, got `{actual}`"),
                    );
                }
            }
            return true;
        };
        // Elaboration writes the checks before this trusted form.
        if expr.span.is_dummy() {
            return true;
        }
        // A literal is checked by `runa check`; nothing is left to run.
        if self.scalar_literal_fits(argument, name) {
            return true;
        }
        if let Some(actual) = self.ordinary_expression_type(argument) {
            let message = if actual == name {
                Some(format!("the value is already {} `{name}`; remove the conversion", article(name)))
            } else if self.scalar_primitive(&actual).is_some() {
                Some(format!(
                    "`{name}(...)` makes {} `{name}` from a `{}`, got `{actual}`; convert between two named scalars through `{}(...)`, so that the step is visible",
                    article(name),
                    scalar.primitive,
                    scalar.primitive
                ))
            } else if actual != scalar.primitive {
                Some(format!(
                    "`{name}(...)` makes {} `{name}` from a `{}`, got `{actual}`",
                    article(name),
                    scalar.primitive
                ))
            } else {
                None
            };
            if let Some(message) = message {
                self.error_at_expr(argument, message);
                return true;
            }
        }
        if !scalar.checks.is_empty() {
            self.record_scalar_conversion(expr, name, &scalar, false);
        }
        true
    }

    fn check_scalar_parse_call(&mut self, expr: &Expr, args: &[Expr], in_fn: Option<&str>) -> bool {
        let [target, value] = args else {
            return false;
        };
        let scalar_name = match &target.kind {
            ExprKind::Var(scalar)
                if self.scalar_primitive(scalar).is_some() && !self.var_defined(scalar) =>
            {
                scalar.clone()
            }
            _ => return false,
        };
        let scalar = self.scalar_types[&scalar_name].clone();
        self.check_expr(value, in_fn);
        if is_named_arg_expr(value) {
            self.error_at_expr(value, "`parse` takes its arguments without names, as in `parse(Isin, text)`".to_string());
            return true;
        }
        if let Some(actual) = self.ordinary_expression_type(value) {
            if actual != scalar.primitive {
                self.error_at_expr(
                    value,
                    format!(
                        "`parse({scalar_name}, ...)` reads a `{}`, got `{actual}`",
                        scalar.primitive
                    ),
                );
                return true;
            }
        }
        self.record_scalar_conversion(expr, &scalar_name, &scalar, true);
        true
    }

    fn record_scalar_conversion(&mut self, expr: &Expr, name: &str, scalar: &ScalarType, parse: bool) {
        if expr.span.is_dummy() {
            self.error_at_expr(
                expr,
                format!("`{name}` conversion needs a position in the source to be elaborated; write it in the source"),
            );
            return;
        }
        self.scalar_conversions
            .entry(self.unit_conversion_module.clone())
            .or_default()
            .insert(
                (expr.span.start, expr.span.end),
                ScalarConversion {
                    scalar: name.to_string(),
                    primitive: scalar.primitive.clone(),
                    checks: scalar.checks.clone(),
                    parse,
                },
            );
    }

    /// The builtins that look a value up in a collection take a value of the
    /// collection's key or item type: `map_contains(accounts, isin)` is a
    /// mistake when `accounts` is keyed by `AccountId`.
    pub(super) fn check_scalar_collection_call(&mut self, name: &str, args: &[Expr]) {
        if self.scalar_types.is_empty() || has_named_args(args) {
            return;
        }
        let key = || Ty::Var("k".into());
        let value = || Ty::Var("v".into());
        let item = || Ty::Var("a".into());
        let app = |head: &str, arguments: Vec<Ty>| Ty::App(Box::new(Ty::Name(head.into())), arguments);
        let signature = match (name, args.len()) {
            ("map_contains" | "map_get" | "map_remove", 2) => {
                vec![app("Map", vec![key(), value()]), key()]
            }
            ("map_get_or" | "map_insert", 3) => {
                vec![app("Map", vec![key(), value()]), key(), value()]
            }
            ("set_contains" | "set_insert" | "set_remove", 2) => {
                vec![app("Set", vec![item()]), item()]
            }
            ("contains", 2) => vec![app("List", vec![item()]), item()],
            _ => return,
        };
        let types = args
            .iter()
            .map(|argument| {
                self.ordinary_expression_type(argument)
                    .and_then(|ty| parse_type_annotation(&ty).ok())
            })
            .collect::<Vec<_>>();
        if !types.iter().flatten().any(|ty| self.contains_scalar(ty)) {
            return;
        }
        let mut substitutions = BTreeMap::new();
        for (index, ((argument, actual), expected)) in args.iter().zip(&types).zip(&signature).enumerate() {
            let expected_now = substitute_type_variables(expected, &substitutions);
            if self.unit_literal_fits(argument, &expected_now) {
                continue;
            }
            let Some(actual) = actual else {
                continue;
            };
            if self.ordinary_argument_matches(actual, expected, &mut substitutions) {
                continue;
            }
            let expected_now = substitute_type_variables(expected, &substitutions);
            let actual_name = Self::canonical_explore_ty_name(actual);
            let expected_name = Self::canonical_explore_ty_name(&expected_now);
            let hint = self.numeric_conversion_hint(actual, &expected_now);
            self.error_at_expr(
                argument,
                format!(
                    "argument {} to `{name}` expects `{expected_name}`, got `{actual_name}`{hint}",
                    index + 1
                ),
            );
            return;
        }
    }

    /// A field whose `@ field` says what the `@ field` of its named scalar
    /// says, apart from `key`, says it a second time.
    pub(super) fn check_scalar_field_meta(
        &mut self,
        variants: &[Variant],
        field_meta: Option<&TypeFieldMeta>,
    ) {
        let Some(field_meta) = field_meta else {
            return;
        };
        if self.scalar_types.is_empty() {
            return;
        }
        for member in &field_meta.members {
            let Some(field) = variants
                .iter()
                .find(|variant| variant.name == member.variant)
                .and_then(|variant| variant.fields.iter().find(|field| field.name == member.field))
            else {
                continue;
            };
            let Some(scalar) = self.held_scalar(&field.ty) else {
                continue;
            };
            let Some(type_meta) = self.scalar_types[&scalar].field_meta.clone() else {
                continue;
            };
            let presentation = |meta: &FieldMetaDecl| {
                let mut entries = meta
                    .entries
                    .iter()
                    .filter(|(key, _)| key != "key")
                    .map(|(key, value)| (key.clone(), format!("{:?}", strip_spans_expr(value))))
                    .collect::<Vec<_>>();
                entries.sort();
                entries
            };
            let written = presentation(&member.meta);
            if written.is_empty() || written != presentation(&type_meta) {
                continue;
            }
            let advice = if member.meta.value("key").is_some() {
                "; keep `@ field(key = True)` for the key"
            } else {
                ""
            };
            self.error_at_span(
                member.meta.span,
                format!(
                    "`@ field` on `{}` says what the `@ field` of `{scalar}` says; remove it, and the field takes its type's{advice}",
                    member.field
                ),
            );
        }
    }

    /// The named scalar a field of this type holds: the type itself, or the
    /// item of an `Option`, `List` or `Set`, or the value of a `Map`.
    fn held_scalar(&self, ty: &Ty) -> Option<String> {
        match ty {
            Ty::Name(name) => self.scalar_primitive(name).map(|_| name.clone()),
            Ty::Optional(inner) | Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                self.held_scalar(inner)
            }
            Ty::App(head, arguments) => match (head.as_ref(), arguments.as_slice()) {
                (Ty::Name(head), [item]) if matches!(head.as_str(), "Option" | "List" | "Set") => {
                    self.held_scalar(item)
                }
                (Ty::Name(head), [_, value]) if head == "Map" => self.held_scalar(value),
                _ => None,
            },
            _ => None,
        }
    }

    /// Join two branches: a named scalar and a literal of its primitive are
    /// the scalar; otherwise branches that differ only in named scalars are
    /// their primitives.
    pub(super) fn merge_scalar_branch(
        &self,
        known: &str,
        known_is_literal: bool,
        next: &str,
        next_is_literal: bool,
    ) -> Option<String> {
        if self.scalar_types.is_empty() {
            return None;
        }
        let (known_widened, next_widened) = (self.widen_scalars(known), self.widen_scalars(next));
        if known_widened != next_widened {
            return None;
        }
        if next_is_literal && next == next_widened && known != known_widened {
            return Some(known.to_string());
        }
        if known_is_literal && known == known_widened && next != next_widened {
            return Some(next.to_string());
        }
        Some(known_widened)
    }

    /// Whether a checked conversion waits for elaboration.
    pub(crate) fn has_scalar_conversions(&self) -> bool {
        !self.scalar_conversions.is_empty()
    }

    /// Write every checked `Isin(p)` and `parse(Isin, p)` as the checks it
    /// stands for, in the root program and each imported file. The checks
    /// keep the conversion's source span.
    pub(crate) fn elaborate_scalar_conversions(
        &mut self,
        root: &[Stmt],
        elaboration: &mut ConstructorElaboration,
    ) {
        for (module, conversions) in std::mem::take(&mut self.scalar_conversions) {
            let mut rewrite = ScalarElaboration { conversions };
            elaboration.rewrite_file(module.as_deref(), root, |statements| {
                rewrite_stmts(statements, &mut rewrite)
            });
        }
    }
}

/// Replace bound type variables.
fn substitute_type_variables(ty: &Ty, substitutions: &BTreeMap<String, Ty>) -> Ty {
    match ty {
        Ty::Var(name) => substitutions.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Ty::App(head, arguments) => Ty::App(
            Box::new(substitute_type_variables(head, substitutions)),
            arguments
                .iter()
                .map(|argument| substitute_type_variables(argument, substitutions))
                .collect(),
        ),
        other => other.clone(),
    }
}

impl ScalarLiteralEvaluator {
    fn verdict(&mut self, name: &str, scalar: &ScalarType, value: &Literal) -> Verdict {
        let key = (name.to_string(), show_literal(value));
        if let Some(verdict) = self.verdicts.get(&key) {
            return verdict.clone();
        }
        let mut verdict = Verdict::Holds;
        for check in &scalar.checks {
            let at = |kind| Expr::new(kind, Span::dummy());
            let mut arguments = vec![at(ExprKind::Lit(value.clone()))];
            arguments.extend(check.arguments.iter().cloned());
            let call = at(ExprKind::App(
                Box::new(at(ExprKind::Var(check.name.clone()))),
                arguments,
            ));
            self.evaluator.ensure_initialized();
            match self.evaluator.evaluate_expr(&call, &BTreeMap::new()) {
                Some(Value::Bool(true)) => continue,
                Some(Value::Bool(false)) => verdict = Verdict::False(check.text.clone()),
                _ => verdict = Verdict::Failed(check.text.clone()),
            }
            break;
        }
        self.verdicts.insert(key, verdict.clone());
        verdict
    }
}

/// The checked conversions of one file, by source span; each is written
/// once.
struct ScalarElaboration {
    conversions: BTreeMap<(usize, usize), ScalarConversion>,
}

impl ProgramRewrite for ScalarElaboration {
    fn expr(&mut self, expr: &mut Expr) {
        let ExprKind::App(_, arguments) = &mut expr.kind else {
            return;
        };
        let Some(conversion) = self.conversions.remove(&(expr.span.start, expr.span.end)) else {
            return;
        };
        let value = arguments.pop().expect("a conversion has a value");
        *expr = conversion.elaborate(value, expr.span);
    }
}

impl ScalarConversion {
    /// `Isin(p)` is
    /// `{ = v = p; assert_with_message(isin_shape(v), "..."); ...; Isin(v) }`
    /// and `parse(Isin, p)` is
    /// `{ = v = p; if isin_shape(v) { if ... { Ok(Isin(v)) } else { Err("...") } } else { Err("...") } }`.
    /// The inner `Isin(v)` has no source position: the checker trusts it,
    /// and at runtime it leaves the value as it is.
    fn elaborate(&self, value: Expr, span: Span) -> Expr {
        let at = |kind| Expr::new(kind, span);
        let text = |text: String| at(ExprKind::Lit(Literal::Str(text)));
        let variable = || at(ExprKind::Var(SCALAR_VALUE.to_string()));
        let call = |function: &str, arguments: Vec<Expr>| {
            at(ExprKind::App(
                Box::new(at(ExprKind::Var(function.to_string()))),
                arguments,
            ))
        };
        let join = |left: Expr, right: Expr| {
            at(ExprKind::BinOp("+".to_string(), Box::new(left), Box::new(right)))
        };
        let quote = if self.primitive == "String" { "\"" } else { "" };
        let message = |check: &ScalarCheck| {
            let rest = not_a_scalar_message("\u{0}", &self.scalar, &check.text);
            let (before, after) = rest.split_once('\u{0}').expect("the message shows the value");
            join(
                join(text(format!("{before}{quote}")), variable()),
                text(format!("{quote}{after}")),
            )
        };
        let holds = |check: &ScalarCheck| {
            let mut arguments = vec![variable()];
            arguments.extend(check.arguments.iter().cloned());
            call(&check.name, arguments)
        };
        let trusted = Expr::new(
            ExprKind::App(
                Box::new(at(ExprKind::Var(self.scalar.clone()))),
                vec![variable()],
            ),
            Span::dummy(),
        );
        let mut statements = vec![Stmt::Bind(Pat::Var(SCALAR_VALUE.to_string()), None, value)];
        if self.parse {
            let mut result = call("Ok", vec![trusted]);
            for check in self.checks.iter().rev() {
                result = at(ExprKind::If(
                    Box::new(holds(check)),
                    Box::new(result),
                    Box::new(call("Err", vec![message(check)])),
                ));
            }
            statements.push(Stmt::Expr(result));
        } else {
            for check in &self.checks {
                statements.push(Stmt::Expr(call(
                    "assert_with_message",
                    vec![holds(check), message(check)],
                )));
            }
            statements.push(Stmt::Expr(trusted));
        }
        at(ExprKind::Block(statements))
    }
}
