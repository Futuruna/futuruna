//! Units of measure for whole numbers.
//!
//! `# unit Ore "DKK øre"` declares `Ore`, a type whose values are `Int` at
//! runtime, on the wire and in proofs. The checker keeps values of different
//! units apart:
//!
//! - `Ore + Ore`, `Ore - Ore` and `Ore % Ore` are `Ore`; `Ore * Int`,
//!   `Int * Ore`, `Ore / Int` and `Ore % Int` are `Ore`; `Ore / Ore` is a plain
//!   `Int` ratio; `-Ore` is `Ore`.
//! - `Ore * Ore`, `Int / Ore` and `Int % Ore` have no unit and are rejected.
//! - Two different units never meet in an operator; a plain `Int` value never
//!   meets a unit in `+`, `-`, `%` or a comparison.
//! - A number written in the source (`0`, `-5`, `100 * 100`) has no unit of its
//!   own: it takes the unit of the other operand, parameter, field, binding or
//!   result it stands for, so `amount > 0` and `Ore` fields given `0` need no
//!   conversion.
//! - `Ore(n)` gives a plain `Int` the unit; `Int(amount)` takes it off. Both
//!   leave the value unchanged. A rule that changes unit is a function that
//!   uses them, so the rule is written down once.
//! - `abs`, `min_int`, `max_int`, `clamp`, `sum` and `sum_list` keep the unit
//!   of their arguments.
//!
//! Like the rest of the checker, these rules judge the operands whose types
//! are known; an operand of unknown type is not evidence of a mistake.

use super::*;

/// Prelude and builtin functions on `Int` that keep the unit of their
/// arguments: every argument has the same unit, and so does the result.
const UNIT_PRESERVING_FUNCTIONS: &[(&str, usize)] =
    &[("abs", 1), ("min_int", 2), ("max_int", 2), ("clamp", 3)];

/// Builtins over a list whose sum has the unit of its items.
const UNIT_SUMMING_FUNCTIONS: &[&str] = &["sum", "sum_list"];

impl TypeChecker {
    pub(super) fn is_unit_type(&self, name: &str) -> bool {
        self.unit_types.contains_key(name)
    }

    /// The declared description of a unit (`"DKK øre"` for `Ore`).
    pub fn unit_description(&self, name: &str) -> Option<&str> {
        self.unit_types.get(name).map(String::as_str)
    }

    /// Every declared unit and its description.
    pub fn unit_declarations(&self) -> &BTreeMap<String, String> {
        &self.unit_types
    }

    /// Record `# unit Name "description"` in the declaration pass.
    pub(super) fn collect_unit_declaration(&mut self, name: &str, description: &str) {
        let key = format!("unit:{name}");
        if let Some(known) = self.unit_types.get(name) {
            if known != description {
                self.declaration_error(
                    &key,
                    format!(
                        "unit `{name}` is declared twice, as \"{known}\" and as \"{description}\"; declare it once"
                    ),
                );
            }
            return;
        }
        if declaration_contracts::is_builtin_type_name(name) {
            self.declaration_error(
                &key,
                format!(
                    "unit `{name}` would rename the built-in type `{name}`; choose another name"
                ),
            );
            return;
        }
        if self.types.contains(name) || self.constructors.contains_key(name) {
            self.declaration_error(
                &key,
                format!("unit `{name}` has the name of a type or constructor that is already declared; choose another name"),
            );
            return;
        }
        self.types.insert(name.to_string());
        self.unit_types
            .insert(name.to_string(), description.to_string());
    }

    /// A type or constructor declared after a unit of the same name.
    pub(super) fn check_name_against_units(&mut self, name: &str) {
        if self.unit_types.contains_key(name) {
            self.declaration_error(
                &format!("variant:{name}"),
                format!("`{name}` is already declared as a unit; choose another name"),
            );
        }
    }

    /// An `Int` literal, or arithmetic of literals only (`-5`, `100 * 100`):
    /// a number without a unit of its own. It takes the unit of the place it
    /// stands in.
    pub(super) fn is_unitless_literal(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Lit(Literal::Int(_)) => true,
            ExprKind::Block(stmts) => {
                matches!(stmts.as_slice(), [Stmt::Expr(inner)] if Self::is_unitless_literal(inner))
            }
            ExprKind::UnOp(operator, inner) if operator == "-" || operator == "+" => {
                Self::is_unitless_literal(inner)
            }
            ExprKind::BinOp(operator, left, right)
                if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%") =>
            {
                Self::is_unitless_literal(left) && Self::is_unitless_literal(right)
            }
            _ => false,
        }
    }

    fn unit_of<'a>(&self, type_name: Option<&'a str>) -> Option<&'a str> {
        type_name.filter(|name| self.is_unit_type(name))
    }

    /// The result type of `left operator right`, with the unit rules.
    pub(super) fn binary_result_type(
        &self,
        operator: &str,
        left: Option<&str>,
        right: Option<&str>,
    ) -> Option<String> {
        self.unit_binary_result(operator, left, right)
            .unwrap_or_else(|| Self::binary_expression_result_type(operator, left, right))
    }

    /// The result type of `left operator right` when an operand has a unit.
    /// `None` when no unit is involved; `Some(None)` when the combination has
    /// no type (the operand check reports it).
    pub(super) fn unit_binary_result(
        &self,
        operator: &str,
        left: Option<&str>,
        right: Option<&str>,
    ) -> Option<Option<String>> {
        let left_unit = self.unit_of(left);
        let right_unit = self.unit_of(right);
        if left_unit.is_none() && right_unit.is_none() {
            return None;
        }
        let plain = |ty: Option<&str>| ty == Some("Int");
        let result = match operator {
            "<" | ">" | "<=" | ">=" | "==" | "!=" | "&&" | "||" => Some("Bool"),
            "+" if left == Some("String") || right == Some("String") => Some("String"),
            "+" | "-" | "%" => match (left_unit, right_unit) {
                (Some(l), Some(r)) if l == r => Some(l),
                (Some(l), None) if plain(right) => Some(l),
                (None, Some(r)) if plain(left) && operator != "%" => Some(r),
                _ => None,
            },
            "*" => match (left_unit, right_unit) {
                (Some(l), None) if plain(right) => Some(l),
                (None, Some(r)) if plain(left) => Some(r),
                _ => None,
            },
            "/" => match (left_unit, right_unit) {
                (Some(l), Some(r)) if l == r => Some("Int"),
                (Some(l), None) if plain(right) => Some(l),
                _ => None,
            },
            _ => None,
        };
        Some(result.map(str::to_string))
    }

    /// Report a binary operator whose operands break the unit rules.
    /// Returns true when an operand has a unit, so the ordinary numeric check
    /// does not judge the operands a second time.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_unit_operands(
        &mut self,
        expression: &Expr,
        operator: &str,
        left: &Expr,
        left_type: Option<&str>,
        right: &Expr,
        right_type: Option<&str>,
    ) -> bool {
        let left_unit = self.unit_of(left_type).map(str::to_string);
        let right_unit = self.unit_of(right_type).map(str::to_string);
        if left_unit.is_none() && right_unit.is_none() {
            return false;
        }
        if matches!(operator, "&&" | "||") {
            // The ordinary check reports a non-Bool operand.
            return false;
        }
        if operator == "+" && (left_type == Some("String") || right_type == Some("String")) {
            // Text concatenation shows the number.
            return true;
        }
        let message = match (left_unit.as_deref(), right_unit.as_deref()) {
            (Some(l), Some(r)) if l != r => Some(format!(
                "unsupported operands for operator `{operator}`: `{l}` and `{r}` are different units; state the conversion in a function that uses `Int(...)` and `{l}(...)` or `{r}(...)`"
            )),
            (Some(l), Some(_)) => match operator {
                "*" => Some(format!(
                    "unsupported operands for operator `*`: `{l}` times `{l}` has no unit; multiply by a plain `Int`, or take the unit off one side with `Int(...)`"
                )),
                _ => None,
            },
            (Some(unit), None) | (None, Some(unit)) => {
                let unit_on_left = left_unit.is_some();
                let (other, other_type) = if unit_on_left {
                    (right, right_type)
                } else {
                    (left, left_type)
                };
                match other_type {
                    // A number written in the source takes the unit.
                    _ if Self::is_unitless_literal(other) => {
                        if !unit_on_left && matches!(operator, "/" | "%") {
                            Some(self.unit_divisor_message(operator, unit))
                        } else {
                            None
                        }
                    }
                    Some("Int") => match operator {
                        "*" => None,
                        "/" | "%" if unit_on_left => None,
                        "/" | "%" => Some(self.unit_divisor_message(operator, unit)),
                        _ => Some(format!(
                            "unsupported operands for operator `{operator}`: `{unit}` and a plain `Int`; write `{unit}(n)` when the number is an amount in `{unit}`, or take the unit off with `Int(...)`"
                        )),
                    },
                    Some(other_type) => {
                        let (l, r) = if unit_on_left {
                            (unit, other_type)
                        } else {
                            (other_type, unit)
                        };
                        Some(format!(
                            "unsupported operands for operator `{operator}`: `{l}` and `{r}`"
                        ))
                    }
                    // An operand of unknown type is not evidence.
                    None => None,
                }
            }
            (None, None) => None,
        };
        if let Some(message) = message {
            self.error_at_expr(expression, message);
        }
        true
    }

    fn unit_divisor_message(&self, operator: &str, unit: &str) -> String {
        let verb = if operator == "/" {
            "divided by"
        } else {
            "remainder by"
        };
        format!(
            "unsupported operands for operator `{operator}`: a plain `Int` {verb} `{unit}` has no unit; divide by a plain `Int`, or take the unit off with `Int(...)`"
        )
    }

    /// Whether a call of `name` is a unit conversion: a declared unit, or
    /// `Int`, applied to one argument and not shadowed by a local binding.
    pub(super) fn is_unit_conversion_name(&self, name: &str) -> bool {
        name == "Int" || self.is_unit_type(name)
    }

    /// Check `Ore(n)` and `Int(amount)`. Returns true when `name` is a unit
    /// conversion.
    pub(super) fn check_unit_conversion_call(
        &mut self,
        expr: &Expr,
        name: &str,
        args: &[Expr],
        in_fn: Option<&str>,
    ) -> bool {
        if !self.is_unit_conversion_name(name) || self.var_defined(name) {
            return false;
        }
        for arg in args {
            self.check_expr(named_arg_parts(arg).map_or(arg, |(_, value)| value), in_fn);
        }
        let [argument] = args else {
            self.error_at_expr(
                expr,
                format!(
                    "`{name}(...)` takes one number, got {} arguments",
                    args.len()
                ),
            );
            return true;
        };
        if is_named_arg_expr(argument) {
            self.error_at_expr(
                argument,
                format!("`{name}(...)` takes one number without a name"),
            );
            return true;
        }
        let Some(actual) = self.ordinary_expression_type(argument) else {
            return true;
        };
        let accepted = if name == "Int" {
            actual == "Int" || self.is_unit_type(&actual)
        } else {
            actual == "Int" || actual == name
        };
        if !accepted {
            let message = if name == "Int" {
                format!("`Int(...)` takes the unit off a number, got `{actual}`")
            } else if self.is_unit_type(&actual) {
                format!(
                    "`{name}(...)` gives a plain `Int` the unit `{name}`, got `{actual}`; state the conversion from `{actual}` in a function that uses `Int(...)`"
                )
            } else {
                format!("`{name}(...)` gives a plain `Int` the unit `{name}`, got `{actual}`")
            };
            self.error_at_expr(argument, message);
        }
        true
    }

    /// The result of a call that keeps the unit of its arguments (`abs`,
    /// `min_int`, `max_int`, `clamp`, `sum`, `sum_list`), when an argument has
    /// a unit.
    pub(super) fn unit_preserving_call_result(
        &self,
        name: &str,
        argument_types: &[Option<String>],
    ) -> Option<String> {
        if !self.unit_preserving_builtin(name, argument_types.len()) {
            return None;
        }
        if UNIT_SUMMING_FUNCTIONS.contains(&name) {
            let item = Self::applied_type_argument(argument_types.first()?.as_deref()?, "List", 0)?;
            return self.is_unit_type(&item).then_some(item);
        }
        argument_types
            .iter()
            .flatten()
            .find(|ty| self.is_unit_type(ty))
            .cloned()
    }

    fn unit_preserving_builtin(&self, name: &str, arity: usize) -> bool {
        if UNIT_SUMMING_FUNCTIONS.contains(&name) {
            return arity == 1 && !self.user_functions_shadow_prelude(name);
        }
        UNIT_PRESERVING_FUNCTIONS
            .iter()
            .any(|(builtin, builtin_arity)| *builtin == name && *builtin_arity == arity)
            && !self.user_functions_shadow_prelude(name)
    }

    /// Whether the program replaces a prelude or builtin function with its
    /// own declaration of the name.
    fn user_functions_shadow_prelude(&self, name: &str) -> bool {
        self.user_functions.contains(name) && !self.prelude_functions.contains(name)
    }

    /// Check a call of a unit-keeping function. Returns true when an argument
    /// has a unit, so the ordinary `Int` parameter check does not reject it.
    pub(super) fn check_unit_preserving_call(&mut self, name: &str, args: &[Expr]) -> bool {
        if !self.unit_preserving_builtin(name, args.len()) || has_named_args(args) {
            return false;
        }
        let types = args
            .iter()
            .map(|argument| self.ordinary_expression_type(argument))
            .collect::<Vec<_>>();
        if UNIT_SUMMING_FUNCTIONS.contains(&name) {
            return types[0]
                .as_deref()
                .and_then(|ty| Self::applied_type_argument(ty, "List", 0))
                .is_some_and(|item| self.is_unit_type(&item));
        }
        let Some(unit) = types
            .iter()
            .flatten()
            .find(|ty| self.is_unit_type(ty))
            .cloned()
        else {
            return false;
        };
        for (argument, actual) in args.iter().zip(&types) {
            let Some(actual) = actual else {
                continue;
            };
            if actual == &unit || Self::is_unitless_literal(argument) {
                continue;
            }
            let message = if self.is_unit_type(actual) {
                format!(
                    "`{name}` mixes units `{unit}` and `{actual}`; every argument has the same unit"
                )
            } else {
                format!(
                    "`{name}` mixes `{unit}` and `{actual}`; every argument has the unit `{unit}`, written as `{unit}(n)` for a plain `Int`"
                )
            };
            self.error_at_expr(argument, message);
        }
        true
    }

    /// A number written in the source stands for a value of the unit its
    /// place expects: `0` for an `Ore` parameter, `Some(0)` for `Ore?`,
    /// `[0, 5]` for `List(Ore)`.
    pub(super) fn unit_literal_fits(&self, expr: &Expr, expected: &Ty) -> bool {
        let expr = named_arg_parts(expr).map_or(expr, |(_, value)| value);
        match expected {
            Ty::Name(name) if self.is_unit_type(name) => Self::is_unitless_literal(expr),
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                self.unit_literal_fits(expr, inner)
            }
            Ty::Optional(inner) => self.unit_literal_fits_option(expr, inner),
            Ty::App(head, arguments) => match (head.as_ref(), arguments.as_slice()) {
                (Ty::Name(head), [inner]) if head == "Option" => {
                    self.unit_literal_fits_option(expr, inner)
                }
                (Ty::Name(head), [inner]) if head == "List" => match &expr.kind {
                    ExprKind::List(items) => {
                        !items.is_empty()
                            && items.iter().all(|item| {
                                self.unit_literal_fits(item, inner)
                                    || self.ordinary_expression_type(item).is_some_and(
                                        |ty| matches!(inner, Ty::Name(name) if *name == ty),
                                    )
                            })
                            && items.iter().any(|item| self.unit_literal_fits(item, inner))
                    }
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    fn unit_literal_fits_option(&self, expr: &Expr, inner: &Ty) -> bool {
        match &expr.kind {
            ExprKind::App(function, arguments) if arguments.len() == 1 => {
                matches!(&function.kind, ExprKind::Var(name) if name == "Some")
                    && self.unit_literal_fits(&arguments[0], inner)
            }
            _ => false,
        }
    }

    /// `foldl(items, 0, |total, item| total + item.amount)`: a start written as
    /// a number takes the unit `U` that the step itself brings in: with the
    /// accumulator a `U` the step is a `U`, and with the accumulator a plain
    /// `Int` it is still a `U` (so `total + n` for a plain `n` does not
    /// qualify). One such unit is required; otherwise the fold is a plain `Int`.
    pub(super) fn unit_of_literal_fold(
        &self,
        args: &[Expr],
        locals: &BTreeMap<String, String>,
        active_rule_scope: Option<&str>,
    ) -> Option<String> {
        if self.unit_types.is_empty() || !Self::is_unitless_literal(&args[1]) {
            return None;
        }
        let ExprKind::Lambda(params, body) = &args[2].kind else {
            return None;
        };
        let [accumulator, item] = params.as_slice() else {
            return None;
        };
        if accumulator.ty.is_some() {
            return None;
        }
        let item_type = match &item.ty {
            Some(ty) => Self::type_name_from_ty(ty),
            None => self
                .infer_expr_type_name_with_locals_in_scope(&args[0], locals, active_rule_scope)
                .and_then(|collection| Self::applied_type_argument(&collection, "List", 0)),
        }
        .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
        let step_type = |accumulator_type: &str| {
            let mut step_locals = locals.clone();
            step_locals.insert(accumulator.name.clone(), accumulator_type.to_string());
            step_locals.insert(item.name.clone(), item_type.clone());
            self.infer_expr_type_name_with_locals_in_scope(body, &step_locals, active_rule_scope)
        };
        let mut found = self.unit_types.keys().filter(|unit| {
            step_type(unit).as_deref() == Some(unit.as_str())
                && step_type("Int").as_deref() == Some(unit.as_str())
        });
        let unit = found.next()?;
        found.next().is_none().then(|| unit.clone())
    }

    /// In a rule family, a clause whose result is a number written in the
    /// source has the unit of the family's other clauses (`| fee(x: Ore) ->
    /// 0 under x < 100` beside `| fee(x: Ore) -> x / 10`).
    pub(super) fn adopt_unit_for_literal_clauses(
        &self,
        rules: &[&Rule],
        inferred: &mut [Option<String>],
    ) {
        if self.unit_types.is_empty() {
            return;
        }
        let literal = |rule: &Rule| match rule {
            Rule::Clause {
                body: Some(value), ..
            }
            | Rule::Default { value, .. }
            | Rule::Exception { value, .. } => Self::is_unitless_literal(value),
            _ => false,
        };
        let units = rules
            .iter()
            .zip(inferred.iter())
            .filter(|(rule, _)| !literal(rule))
            .filter_map(|(_, ty)| ty.as_deref().filter(|ty| self.is_unit_type(ty)))
            .collect::<BTreeSet<_>>();
        let [unit] = units.into_iter().collect::<Vec<_>>()[..] else {
            return;
        };
        let unit = unit.to_string();
        for (rule, ty) in rules.iter().zip(inferred.iter_mut()) {
            if literal(rule) && ty.as_deref() == Some("Int") {
                *ty = Some(unit.clone());
            }
        }
    }

    /// The type of a branching expression whose branches are a unit and a
    /// number written in the source (`if c { 0 } else { amount }`).
    pub(super) fn merge_unit_branch(
        &self,
        known: &str,
        known_is_literal: bool,
        next: &str,
        next_is_literal: bool,
    ) -> Option<String> {
        if known == "Int" && known_is_literal && self.is_unit_type(next) {
            return Some(next.to_string());
        }
        if next == "Int" && next_is_literal && self.is_unit_type(known) {
            return Some(known.to_string());
        }
        None
    }
}

/// Erase units for a backend that needs only runtime representations: every
/// unit type becomes `Int` and every conversion (`Ore(n)`, `Int(amount)`)
/// becomes its argument. The checker has already kept the units apart, so
/// generated Rust and SMT see exactly the `Int` arithmetic the interpreter
/// runs. `# unit` declarations stay; Rust generation emits them as aliases.
pub fn erase_unit_types(stmts: &mut [Stmt]) {
    let mut units = BTreeSet::new();
    collect_unit_names(stmts, &mut units);
    for stmt in stmts {
        erase_stmt(stmt, &units);
    }
}

fn collect_unit_names(stmts: &[Stmt], units: &mut BTreeSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::TypeDecl(TypeDecl::Unit { name, .. }) => {
                units.insert(name.clone());
            }
            Stmt::Defn(Defn::Module { body, .. }) => collect_unit_names(body, units),
            _ => {}
        }
    }
}

fn erase_ty(ty: &mut Ty, units: &BTreeSet<String>) {
    match ty {
        Ty::Name(name) if units.contains(name.as_str()) => *name = "Int".to_string(),
        Ty::App(head, arguments) => {
            erase_ty(head, units);
            for argument in arguments {
                erase_ty(argument, units);
            }
        }
        Ty::Arrow(input, output) => {
            erase_ty(input, units);
            erase_ty(output, units);
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
            erase_ty(inner, units)
        }
        Ty::Name(_) | Ty::Var(_) | Ty::Unit | Ty::Hole => {}
    }
}

fn erase_params(params: &mut [Param], units: &BTreeSet<String>) {
    for param in params {
        if let Some(ty) = &mut param.ty {
            erase_ty(ty, units);
        }
    }
}

fn erase_stmts(stmts: &mut [Stmt], units: &BTreeSet<String>) {
    for stmt in stmts {
        erase_stmt(stmt, units);
    }
}

fn erase_defn(defn: &mut Defn, units: &BTreeSet<String>) {
    match defn {
        Defn::Fn {
            params,
            ret_ty,
            body,
            ..
        } => {
            erase_params(params, units);
            if let Some(ty) = ret_ty {
                erase_ty(ty, units);
            }
            erase_expr(body, units);
        }
        Defn::Actor {
            state_param,
            handlers,
            ..
        } => {
            erase_params(std::slice::from_mut(state_param), units);
            for handler in handlers {
                erase_expr(&mut handler.body, units);
            }
        }
        Defn::Module { body, .. } => erase_stmts(body, units),
    }
}

fn erase_variants(variants: &mut [Variant], units: &BTreeSet<String>) {
    for variant in variants {
        for field in &mut variant.fields {
            erase_ty(&mut field.ty, units);
        }
    }
}

fn erase_stmt(stmt: &mut Stmt, units: &BTreeSet<String>) {
    match stmt {
        Stmt::Defn(defn) => erase_defn(defn, units),
        Stmt::TypeDecl(decl) => match decl {
            TypeDecl::ADT {
                params,
                variants,
                methods,
                ..
            } => {
                erase_params(params, units);
                erase_variants(variants, units);
                for method in methods {
                    erase_defn(method, units);
                }
            }
            TypeDecl::WhenType {
                condition,
                variants,
                ..
            } => {
                erase_expr(condition, units);
                erase_variants(variants, units);
            }
            TypeDecl::EffectDecl { ops, .. } => {
                for (_, params, ret_ty) in ops {
                    erase_params(params, units);
                    if let Some(ty) = ret_ty {
                        erase_ty(ty, units);
                    }
                }
            }
            TypeDecl::TraitDecl {
                params, methods, ..
            } => {
                erase_params(params, units);
                for method in methods {
                    erase_params(&mut method.params, units);
                    if let Some(ty) = &mut method.ret_ty {
                        erase_ty(ty, units);
                    }
                    if let Some(body) = &mut method.default_body {
                        erase_expr(body, units);
                    }
                }
            }
            TypeDecl::ImplBlock { methods, .. } => {
                for method in methods {
                    erase_defn(method, units);
                }
            }
            TypeDecl::RuleScope { params, body, .. } => {
                erase_params(params, units);
                erase_stmts(body, units);
            }
            TypeDecl::Unit { .. } => {}
        },
        Stmt::Rule(rule) => match rule {
            Rule::Clause { head, body } => {
                erase_expr(head, units);
                if let Some(body) = body {
                    erase_expr(body, units);
                }
            }
            Rule::Default {
                head,
                value,
                condition,
            }
            | Rule::Exception {
                head,
                value,
                condition,
                ..
            } => {
                erase_expr(head, units);
                erase_expr(value, units);
                if let Some(condition) = condition {
                    erase_expr(condition, units);
                }
            }
            Rule::ReactiveScope { body, .. } => erase_stmts(body, units),
        },
        Stmt::Bind(_, ty, value) | Stmt::MonadicBind(_, ty, value) => {
            if let Some(ty) = ty {
                erase_ty(ty, units);
            }
            erase_expr(value, units);
        }
        Stmt::For(_, collection, body) => {
            erase_expr(collection, units);
            erase_stmts(body, units);
        }
        Stmt::While(condition, body) => {
            erase_expr(condition, units);
            erase_stmts(body, units);
        }
        Stmt::Send(target, message) => {
            erase_expr(target, units);
            erase_expr(message, units);
        }
        Stmt::StreamBind(_, value) | Stmt::Expr(value) => erase_expr(value, units),
        Stmt::StreamSub(subject, arms) => {
            erase_expr(subject, units);
            erase_arms(arms, units);
        }
        Stmt::Invariant {
            subject, predicate, ..
        } => {
            erase_expr(subject, units);
            erase_expr(predicate, units);
        }
        Stmt::Prove {
            pass_block,
            else_block,
            ..
        } => {
            for block in [pass_block, else_block].into_iter().flatten() {
                erase_stmts(block, units);
            }
        }
        Stmt::Annot(_, arguments) => {
            for argument in arguments {
                erase_expr(argument, units);
            }
        }
        Stmt::Explore(query) => erase_explore(query, units),
        Stmt::PreludeBoundary
        | Stmt::Use(_)
        | Stmt::Import(_)
        | Stmt::QualifiedImport(_, _)
        | Stmt::HashImport(_, _)
        | Stmt::Depend(_, _)
        | Stmt::RustBlock(_) => {}
    }
}

fn erase_explore(query: &mut ExploreQuery, units: &BTreeSet<String>) {
    for binding in &mut query.source.bindings {
        match &mut binding.kind {
            ExploreSourceBindingKind::Singleton { value }
            | ExploreSourceBindingKind::Finite { domain: value } => erase_expr(value, units),
        }
    }
    match &mut query.successor.kind {
        ExploreSuccessorKind::Singleton { value }
        | ExploreSuccessorKind::Finite { domain: value } => erase_expr(value, units),
    }
    for admission in &mut query.admissions {
        erase_expr(&mut admission.predicate, units);
    }
    for find in &mut query.finds {
        match &mut find.selection {
            ExploreSelection::Matches { predicate, .. }
            | ExploreSelection::Violations { predicate, .. } => erase_expr(predicate, units),
            ExploreSelection::All { .. } => {}
        }
    }
    for node in &mut query.analysis {
        if let ExploreAnalysisNode::Result(view) = node {
            for field in view.measures.iter_mut().chain(view.select.iter_mut()) {
                erase_expr(&mut field.value, units);
            }
        }
    }
}

fn erase_arms(arms: &mut [MatchArm], units: &BTreeSet<String>) {
    for arm in arms {
        if let Some(guard) = &mut arm.guard {
            erase_expr(guard, units);
        }
        erase_expr(&mut arm.body, units);
    }
}

fn unit_conversion_argument(expr: &Expr, units: &BTreeSet<String>) -> Option<Expr> {
    let ExprKind::App(function, arguments) = &expr.kind else {
        return None;
    };
    let ExprKind::Var(name) = &function.kind else {
        return None;
    };
    let [argument] = arguments.as_slice() else {
        return None;
    };
    ((name == "Int" || units.contains(name.as_str())) && !is_named_arg_expr(argument))
        .then(|| argument.clone())
}

fn erase_expr(expr: &mut Expr, units: &BTreeSet<String>) {
    while let Some(argument) = unit_conversion_argument(expr, units) {
        *expr = argument;
    }
    match &mut expr.kind {
        ExprKind::App(function, arguments) => {
            if matches!(&function.kind, ExprKind::Var(name) if name == "__typed") {
                if let [value, annotation] = arguments.as_mut_slice() {
                    erase_expr(value, units);
                    if let ExprKind::Var(type_name) = &mut annotation.kind {
                        if let Ok(mut ty) = parse_type_annotation(type_name) {
                            erase_ty(&mut ty, units);
                            *type_name = ty.to_string();
                        }
                    }
                    return;
                }
            }
            erase_expr(function, units);
            for argument in arguments {
                erase_expr(argument, units);
            }
        }
        ExprKind::Lambda(params, body) => {
            erase_params(params, units);
            erase_expr(body, units);
        }
        ExprKind::BinOp(_, left, right)
        | ExprKind::Index(left, right)
        | ExprKind::Pipe(left, right) => {
            erase_expr(left, units);
            erase_expr(right, units);
        }
        ExprKind::UnOp(_, inner) | ExprKind::Field(inner, _) | ExprKind::Try(inner) => {
            erase_expr(inner, units)
        }
        ExprKind::If(condition, then_expr, else_expr) => {
            erase_expr(condition, units);
            erase_expr(then_expr, units);
            erase_expr(else_expr, units);
        }
        ExprKind::Match(subject, arms) => {
            erase_expr(subject, units);
            erase_arms(arms, units);
        }
        ExprKind::Block(stmts) => erase_stmts(stmts, units),
        ExprKind::List(items)
        | ExprKind::Tuple(items)
        | ExprKind::Effect(_, items)
        | ExprKind::Conjunction(items)
        | ExprKind::Disjunction(items) => {
            for item in items {
                erase_expr(item, units);
            }
        }
        ExprKind::Handle { handlers, body, .. } => {
            for handler in handlers {
                erase_expr(&mut handler.body, units);
            }
            erase_expr(body, units);
        }
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
    }
}
