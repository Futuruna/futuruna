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
//!   leave the value unchanged.
//! - `# unit Ore "DKK øre" = DKK / 100` declares a scale: 100 `Ore` make one
//!   `DKK`. Two units of one base convert by a whole factor:
//!   `to(MicroDkk, amount)` multiplies, and a conversion that divides states
//!   its rounding in its name (`to_nearest`, `to_floor`, `to_ceiling`,
//!   `to_toward_zero`). The checker records each conversion and elaboration
//!   rewrites it into the `Int` arithmetic it stands for, so every backend
//!   runs the same arithmetic.
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

/// How a conversion that divides rounds, named by the conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitRounding {
    /// Halves away from zero.
    Nearest,
    Floor,
    Ceiling,
    TowardZero,
}

/// The conversions between units of one base: `to` for a conversion that
/// multiplies, and one name per rounding for a conversion that divides.
const SCALED_CONVERSIONS: &[(&str, Option<UnitRounding>)] = &[
    ("to", None),
    ("to_nearest", Some(UnitRounding::Nearest)),
    ("to_floor", Some(UnitRounding::Floor)),
    ("to_ceiling", Some(UnitRounding::Ceiling)),
    ("to_toward_zero", Some(UnitRounding::TowardZero)),
];

/// One checked conversion: the target unit and the whole factor between the
/// two scales. Without a rounding the amount is multiplied by the factor;
/// with one it is divided and rounded so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnitConversion {
    target: String,
    factor: i64,
    rounding: Option<UnitRounding>,
}

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

    /// The declared scale of every unit that has one.
    pub fn unit_scales(&self) -> &BTreeMap<String, UnitScale> {
        &self.unit_scales
    }

    /// Record `# unit Name "description" = Base / n` in the declaration pass.
    pub(super) fn collect_unit_declaration(
        &mut self,
        name: &str,
        description: &str,
        scale: Option<&UnitScale>,
    ) {
        let key = format!("unit:{name}");
        if let Some(known) = self.unit_types.get(name) {
            let known_scale = self.unit_scales.get(name);
            if known != description || known_scale != scale {
                let show = |description: &str, scale: Option<&UnitScale>| match scale {
                    Some(scale) => format!("\"{description}\" = {scale}"),
                    None => format!("\"{description}\""),
                };
                self.declaration_error(
                    &key,
                    format!(
                        "unit `{name}` is declared twice, as {} and as {}; declare it once",
                        show(known, known_scale),
                        show(description, scale)
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
        if let Some(unit) = self.unit_with_base(name) {
            self.declaration_error(
                &key,
                format!("`{name}` is the base of unit `{unit}`; a unit and a base have different names, and every unit of one base names the base itself (`= {name} / 100`)"),
            );
            return;
        }
        if let Some(scale) = scale {
            if scale.of != "1" {
                let problem = if scale.of == name {
                    Some("a unit is not its own base".to_string())
                } else if declaration_contracts::is_builtin_type_name(&scale.of) {
                    Some(format!(
                        "`{}` is a built-in type; write `1` for a plain number",
                        scale.of
                    ))
                } else if self.is_unit_type(&scale.of) {
                    Some(format!("`{}` is a unit; name its base instead, so that every unit of one base says how many of it make one base", scale.of))
                } else if self.types.contains(&scale.of)
                    || self.constructors.contains_key(&scale.of)
                {
                    Some(format!("`{}` is a declared type or constructor", scale.of))
                } else {
                    None
                };
                if let Some(problem) = problem {
                    self.declaration_error(
                        &key,
                        format!("the base of unit `{name}` is a name of its own, such as `DKK`, or `1`: {problem}"),
                    );
                    return;
                }
            }
            self.unit_scales.insert(name.to_string(), scale.clone());
        }
        self.types.insert(name.to_string());
        self.unit_types
            .insert(name.to_string(), description.to_string());
    }

    /// A unit whose scale names `base` as its base.
    fn unit_with_base(&self, base: &str) -> Option<String> {
        self.unit_scales
            .iter()
            .find(|(_, scale)| scale.of == base)
            .map(|(unit, _)| unit.clone())
    }

    /// A type or constructor declared after a unit, or a unit's base, of the
    /// same name.
    pub(super) fn check_name_against_units(&mut self, name: &str) {
        if self.unit_types.contains_key(name) {
            self.declaration_error(
                &format!("variant:{name}"),
                format!("`{name}` is already declared as a unit; choose another name"),
            );
        } else if let Some(unit) = self.unit_with_base(name) {
            self.declaration_error(
                &format!("variant:{name}"),
                format!("`{name}` is the base of unit `{unit}`; choose another name"),
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
                "unsupported operands for operator `{operator}`: `{l}` and `{r}` are different units; {}",
                self.different_units_advice(l, r)
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

    /// The built-in conversion from `from` to `to`, when both declare a scale
    /// of one base and a whole factor relates them.
    pub(super) fn scaled_conversion_advice(&self, from: &str, to: &str) -> Option<String> {
        let (source, target) = (self.unit_scales.get(from)?, self.unit_scales.get(to)?);
        if source.of != target.of {
            return None;
        }
        if target.per % source.per == 0 {
            return Some(format!("`to({to}, ...)`"));
        }
        (source.per % target.per == 0).then(|| {
            format!("`to_nearest({to}, ...)`, `to_floor`, `to_ceiling` or `to_toward_zero`, whichever rounding the rule states")
        })
    }

    /// How to bring two units together: the built-in conversion of the
    /// coarser into the finer, which never rounds; else a function that
    /// states the conversion.
    fn different_units_advice(&self, left: &str, right: &str) -> String {
        for (from, to) in [(right, left), (left, right)] {
            let exact = self
                .unit_scales
                .get(from)
                .zip(self.unit_scales.get(to))
                .is_some_and(|(source, target)| {
                    source.of == target.of && target.per % source.per == 0
                });
            if exact {
                return format!("convert `{from}` with `to({to}, ...)`");
            }
        }
        format!("state the conversion in a function that uses `Int(...)` and `{left}(...)` or `{right}(...)`")
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

    /// `[a] == [m]`, `Some(a) == Some(m)`: values that hold units are equal
    /// only when the units in them are. Reports `==` or `!=` between two types
    /// that differ in a unit inside them; returns true when it did.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_unit_element_equality(
        &mut self,
        expression: &Expr,
        operator: &str,
        left: &Expr,
        left_type: Option<&str>,
        right: &Expr,
        right_type: Option<&str>,
    ) -> bool {
        if self.unit_types.is_empty() || !matches!(operator, "==" | "!=") {
            return false;
        }
        let (Some(left_name), Some(right_name)) = (left_type, right_type) else {
            return false;
        };
        if left_name == right_name {
            return false;
        }
        let (Ok(left_ty), Ok(right_ty)) = (
            parse_type_annotation(left_name),
            parse_type_annotation(right_name),
        ) else {
            return false;
        };
        // `[0, 5]` and `Some(0)` take the unit of the other side.
        if self.unit_literal_fits(left, &right_ty) || self.unit_literal_fits(right, &left_ty) {
            return false;
        }
        let Some((l, r)) = self.inner_unit_mismatch(&left_ty, &right_ty) else {
            return false;
        };
        let reason = if self.is_unit_type(&l) && self.is_unit_type(&r) {
            format!(
                "`{l}` and `{r}` are different units; {}",
                self.different_units_advice(&l, &r)
            )
        } else {
            let unit = if self.is_unit_type(&l) { &l } else { &r };
            format!("`{unit}` and a plain `Int`; write `{unit}(n)` when the number is an amount in `{unit}`, or take the unit off with `Int(...)`")
        };
        self.error_at_expr(
            expression,
            format!("unsupported operands for operator `{operator}`: `{left_name}` and `{right_name}`: {reason}"),
        );
        true
    }

    /// The first place where two types of the same shape hold a unit on one
    /// side and another unit or a plain `Int` on the other.
    fn inner_unit_mismatch(&self, left: &Ty, right: &Ty) -> Option<(String, String)> {
        match (left, right) {
            (Ty::Name(l), Ty::Name(r)) => {
                let numeric = |name: &str| name == "Int" || self.is_unit_type(name);
                (l != r
                    && numeric(l)
                    && numeric(r)
                    && (self.is_unit_type(l) || self.is_unit_type(r)))
                .then(|| (l.clone(), r.clone()))
            }
            (Ty::App(left_head, left_arguments), Ty::App(right_head, right_arguments))
                if left_head == right_head && left_arguments.len() == right_arguments.len() =>
            {
                left_arguments
                    .iter()
                    .zip(right_arguments)
                    .find_map(|(l, r)| self.inner_unit_mismatch(l, r))
            }
            (Ty::Optional(l), Ty::Optional(r))
            | (Ty::Ref(l), Ty::Ref(r))
            | (Ty::MutRef(l), Ty::MutRef(r))
            | (Ty::Shared(l), Ty::Shared(r)) => self.inner_unit_mismatch(l, r),
            _ => None,
        }
    }

    /// Whether a call of `name` is a unit conversion: a declared unit, or
    /// `Int`, applied to one argument and not shadowed by a local binding.
    pub(super) fn is_unit_conversion_name(&self, name: &str) -> bool {
        name == "Int" || self.is_unit_type(name)
    }

    /// The rounding a conversion name states, when `name` is a conversion
    /// between scaled units that no declaration of the program replaces.
    fn scaled_conversion(&self, name: &str) -> Option<Option<UnitRounding>> {
        if self.unit_scales.is_empty() || self.user_functions.contains(name) {
            return None;
        }
        SCALED_CONVERSIONS
            .iter()
            .find(|(conversion, _)| *conversion == name)
            .map(|(_, rounding)| *rounding)
    }

    /// The unit `to(Ore, amount)` and its rounding forms convert to.
    pub(super) fn scaled_conversion_target(
        &self,
        name: &str,
        args: &[Expr],
        locals: &BTreeMap<String, String>,
    ) -> Option<String> {
        self.scaled_conversion(name)?;
        let [target, _] = args else {
            return None;
        };
        match &target.kind {
            ExprKind::Var(unit) if self.is_unit_type(unit) && !locals.contains_key(unit) => {
                Some(unit.clone())
            }
            _ => None,
        }
    }

    /// Check `to(MicroDkk, amount)` and `to_nearest(Ore, amount)`: both units
    /// have a scale of one base, the factor between them is whole, and the
    /// name states a rounding exactly when the conversion divides. Records
    /// the conversion for elaboration. Returns true when `name` is such a
    /// conversion.
    fn check_scaled_conversion_call(
        &mut self,
        expr: &Expr,
        name: &str,
        args: &[Expr],
        in_fn: Option<&str>,
    ) -> bool {
        let Some(rounding) = self.scaled_conversion(name) else {
            return false;
        };
        if self.var_defined(name) {
            return false;
        }
        for arg in args.iter().skip(1) {
            self.check_expr(named_arg_parts(arg).map_or(arg, |(_, value)| value), in_fn);
        }
        let usage = format!("`{name}(Ore, amount)`");
        let [target, value] = args else {
            self.error_at_expr(
                expr,
                format!(
                    "`{name}` takes the unit to convert to and an amount, as in {usage}; got {} arguments",
                    args.len()
                ),
            );
            return true;
        };
        if is_named_arg_expr(target) || is_named_arg_expr(value) {
            self.error_at_expr(
                expr,
                format!("`{name}` takes its arguments without names, as in {usage}"),
            );
            return true;
        }
        let target = match &target.kind {
            ExprKind::Var(unit) if self.is_unit_type(unit) && !self.var_defined(unit) => {
                unit.clone()
            }
            _ => {
                self.error_at_expr(
                    target,
                    format!(
                        "the first argument of `{name}` is the unit to convert to, as in {usage}"
                    ),
                );
                return true;
            }
        };
        if Self::is_unitless_literal(value) {
            self.error_at_expr(
                value,
                format!("`{name}` converts an amount of another unit; a number written in the source has none, so write `{target}(n)`"),
            );
            return true;
        }
        let Some(source) = self.ordinary_expression_type(value) else {
            self.error_at_expr(
                value,
                format!("`{name}` needs the unit of the amount it converts, and the type of this value is not known here; give it a unit type"),
            );
            return true;
        };
        if !self.is_unit_type(&source) {
            self.error_at_expr(
                value,
                format!("`{name}` converts an amount of another unit, got `{source}`; a plain `Int` becomes an amount with `{target}(n)`"),
            );
            return true;
        }
        if source == target {
            self.error_at_expr(
                expr,
                format!("the amount is already `{target}`; remove the conversion"),
            );
            return true;
        }
        let scales = (
            self.unit_scales.get(&source).cloned(),
            self.unit_scales.get(&target).cloned(),
        );
        let (Some(from), Some(to)) = scales else {
            let unscaled = if self.unit_scales.contains_key(&source) {
                &target
            } else {
                &source
            };
            self.error_at_expr(
                expr,
                format!("unit `{unscaled}` declares no scale, so `{name}` cannot convert it; declare it as `# unit {unscaled} \"{}\" = BASE / n`", self.unit_types[unscaled]),
            );
            return true;
        };
        if from.of != to.of {
            self.error_at_expr(
                expr,
                format!("`{source}` is `{from}` and `{target}` is `{to}`; units of different bases do not convert"),
            );
            return true;
        }
        let conversion = if to.per % from.per == 0 {
            let factor = to.per / from.per;
            if rounding.is_some() {
                self.error_at_expr(
                    expr,
                    format!("`{source}` to `{target}` multiplies by {factor} and is exact, so it does not round; write `to({target}, ...)`"),
                );
                return true;
            }
            UnitConversion {
                target,
                factor,
                rounding: None,
            }
        } else if from.per % to.per == 0 {
            let factor = from.per / to.per;
            let Some(rounding) = rounding else {
                self.error_at_expr(
                    expr,
                    format!("`{source}` to `{target}` divides by {factor}; state how it rounds with `to_nearest` (halves away from zero), `to_floor`, `to_ceiling` or `to_toward_zero`"),
                );
                return true;
            };
            UnitConversion {
                target,
                factor,
                rounding: Some(rounding),
            }
        } else {
            self.error_at_expr(
                expr,
                format!("`{source}` is `{from}` and `{target}` is `{to}`; neither is a whole multiple of the other, so no whole factor converts them"),
            );
            return true;
        };
        if expr.span.is_dummy() {
            self.error_at_expr(
                expr,
                format!("`{name}` needs a position in the source to be elaborated; write it in the source"),
            );
            return true;
        }
        self.unit_conversions
            .entry(self.unit_conversion_module.clone())
            .or_default()
            .insert((expr.span.start, expr.span.end), conversion);
        true
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
        if self.check_scaled_conversion_call(expr, name, args, in_fn) {
            return true;
        }
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
                    "`{name}(...)` gives a plain `Int` the unit `{name}`, got `{actual}`; {}",
                    match self.scaled_conversion_advice(&actual, name) {
                        Some(advice) => format!("convert it with {advice}"),
                        None => format!("state the conversion from `{actual}` in a function that uses `Int(...)`"),
                    }
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
    /// `Int` it is not a plain `Int` (so a count `n + 1` does not qualify, and
    /// `if c { total + item.amount } else { total }` does). One such unit is
    /// required; otherwise the fold is a plain `Int`.
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
                && step_type("Int").as_deref() != Some("Int")
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
    fn merge_unit_branch(
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

    /// Join the types of two branches (`if` branches, `match` arms): equal
    /// types, or a unit and a number written in the source. Every place that
    /// joins branches uses this, so they agree on `| None -> 0` beside an
    /// `Ore` arm.
    pub(super) fn merge_branch_types(
        &self,
        known: &str,
        known_is_literal: bool,
        next: &str,
        next_is_literal: bool,
    ) -> Option<String> {
        Self::merge_inferred_type_names(known, next)
            .or_else(|| self.merge_unit_branch(known, known_is_literal, next, next_is_literal))
    }

    /// A type as its runtime representation, with every unit an `Int`. The
    /// canonical rule-dispatch proof judges representations, as generated
    /// Rust and the solver do; the ordinary checker has already kept the
    /// units apart, so a unit rule family is as type-safe as its `Int` form.
    pub(super) fn canonical_type_without_units(&self, type_name: &str) -> String {
        let canonical = Self::canonical_explore_type_name(type_name);
        if self.unit_types.is_empty() {
            return canonical;
        }
        match parse_type_annotation(&canonical) {
            Ok(mut ty) => {
                erase_ty(&mut ty, &self.unit_types);
                Self::canonical_explore_ty_name(&ty)
            }
            Err(_) => canonical,
        }
    }

    /// Join two branch types for the canonical proof: equal types, or a unit
    /// and a number written in the source (`Ore` and `Int`), which have one
    /// representation and keep the unit.
    pub(super) fn canonical_merge_branch_types(&self, left: &str, right: &str) -> Option<String> {
        Self::merge_inferred_type_names(left, right).or_else(|| {
            (self.canonical_type_without_units(left) == self.canonical_type_without_units(right))
                .then(|| {
                    if self.canonical_type_without_units(left)
                        == Self::canonical_explore_type_name(left)
                    {
                        right.to_string()
                    } else {
                        left.to_string()
                    }
                })
        })
    }
}

impl TypeCheckArtifacts {
    /// A checked type name as a unit-erasing backend sees it: every unit is
    /// an `Int` (`Ore` is `Int`, `List(Ore)` is `List(Int)`). Rust generation
    /// erases the program it emits, so the checker metadata it consumes must
    /// name the same representations.
    pub fn erase_unit_type_name(&self, type_name: &str) -> String {
        if self.unit_names.is_empty() {
            return type_name.to_string();
        }
        match parse_type_annotation(type_name) {
            Ok(ty) => {
                let mut erased = ty.clone();
                erase_ty(&mut erased, &self.unit_names);
                if erased == ty {
                    type_name.to_string()
                } else {
                    TypeChecker::canonical_explore_ty_name(&erased)
                }
            }
            Err(_) => type_name.to_string(),
        }
    }
}

impl TypeChecker {
    /// Rewrite every checked conversion (`to_nearest(Ore, amount)`) into the
    /// `Int` arithmetic it stands for, in the root program and in each
    /// imported file, so the interpreter, the solver and generated Rust run
    /// one definition of each rounding. The conversion keeps its source span.
    pub(crate) fn elaborate_unit_conversions(
        &mut self,
        root: &[Stmt],
        elaboration: &mut ConstructorElaboration,
    ) {
        for (module, conversions) in std::mem::take(&mut self.unit_conversions) {
            let mut rewrite = ConversionElaboration { conversions };
            elaboration.rewrite_file(module.as_deref(), root, |statements| {
                rewrite_stmts(statements, &mut rewrite)
            });
        }
    }

    /// Whether a checked conversion waits for elaboration.
    pub(crate) fn has_unit_conversions(&self) -> bool {
        !self.unit_conversions.is_empty()
    }
}

/// The checked conversions of one file, by source span; each is rewritten
/// once.
struct ConversionElaboration {
    conversions: BTreeMap<(usize, usize), UnitConversion>,
}

impl ProgramRewrite for ConversionElaboration {
    fn expr(&mut self, expr: &mut Expr) {
        let ExprKind::App(_, arguments) = &mut expr.kind else {
            return;
        };
        if arguments.len() != 2 {
            return;
        }
        let Some(conversion) = self.conversions.remove(&(expr.span.start, expr.span.end)) else {
            return;
        };
        let amount = arguments.pop().expect("a conversion has an amount");
        *expr = conversion.elaborate(amount, expr.span);
    }
}

impl UnitConversion {
    /// `to(MicroDkk, amount)` is `MicroDkk(Int(amount) * 10000)`;
    /// `to_nearest(Ore, amount)` divides `Int(amount)` by the factor with
    /// quotient and remainder, so no intermediate exceeds the amount, and
    /// moves the quotient one away from zero when the remainder is at least
    /// half the factor. An amount that is not a name or a field of one is
    /// computed once, in a block.
    fn elaborate(&self, amount: Expr, span: Span) -> Expr {
        let at = |kind| Expr::new(kind, span);
        let int = |value: i64| at(ExprKind::Lit(Literal::Int(value)));
        let call = |function: &str, argument: Expr| {
            at(ExprKind::App(
                Box::new(at(ExprKind::Var(function.to_string()))),
                vec![argument],
            ))
        };
        let binary = |operator: &str, left: Expr, right: Expr| {
            at(ExprKind::BinOp(
                operator.to_string(),
                Box::new(left),
                Box::new(right),
            ))
        };
        let conditional = |condition: Expr, then: Expr, otherwise: Expr| {
            at(ExprKind::If(
                Box::new(condition),
                Box::new(then),
                Box::new(otherwise),
            ))
        };
        let k = self.factor;
        let Some(rounding) = self.rounding else {
            let plain = call("Int", amount);
            let scaled = if k == 1 {
                plain
            } else {
                binary("*", plain, int(k))
            };
            return call(&self.target, scaled);
        };
        let named = is_path(&amount);
        let value = if named {
            call("Int", amount.clone())
        } else {
            at(ExprKind::Var(CONVERTED_AMOUNT.to_string()))
        };
        let quotient = || binary("/", value.clone(), int(k));
        let remainder = || binary("%", value.clone(), int(k));
        let step = |operator: &str| binary(operator, quotient(), int(1));
        let half = k - k / 2;
        let rounded = match rounding {
            UnitRounding::TowardZero => quotient(),
            UnitRounding::Floor => {
                conditional(binary("<", remainder(), int(0)), step("-"), quotient())
            }
            UnitRounding::Ceiling => {
                conditional(binary(">", remainder(), int(0)), step("+"), quotient())
            }
            UnitRounding::Nearest => conditional(
                binary(">=", remainder(), int(half)),
                step("+"),
                conditional(binary("<=", remainder(), int(-half)), step("-"), quotient()),
            ),
        };
        let converted = call(&self.target, rounded);
        if named {
            return converted;
        }
        at(ExprKind::Block(vec![
            Stmt::Bind(
                Pat::Var(CONVERTED_AMOUNT.to_string()),
                None,
                call("Int", amount),
            ),
            Stmt::Expr(converted),
        ]))
    }
}

/// The local that holds a converted amount which is not a plain name.
const CONVERTED_AMOUNT: &str = "__converted_amount";

/// A name, or a field of one: reading it twice reads the same value.
fn is_path(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Var(_) => true,
        ExprKind::Field(base, _) => is_path(base),
        _ => false,
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
    rewrite_stmts(stmts, &mut Erasure { units: &units });
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

struct Erasure<'a> {
    units: &'a BTreeSet<String>,
}

impl ProgramRewrite for Erasure<'_> {
    fn ty(&mut self, ty: &mut Ty) {
        erase_ty(ty, self.units);
    }

    fn typed_annotation(&mut self, type_name: &mut String) {
        if let Ok(mut ty) = parse_type_annotation(type_name) {
            erase_ty(&mut ty, self.units);
            *type_name = ty.to_string();
        }
    }

    fn expr(&mut self, expr: &mut Expr) {
        while let Some(argument) = unit_conversion_argument(expr, self.units) {
            *expr = argument;
        }
    }
}

/// The declared unit names, as a set or as the checker's unit table.
trait UnitNames {
    fn is_unit(&self, name: &str) -> bool;
}

impl UnitNames for BTreeSet<String> {
    fn is_unit(&self, name: &str) -> bool {
        self.contains(name)
    }
}

impl UnitNames for BTreeMap<String, String> {
    fn is_unit(&self, name: &str) -> bool {
        self.contains_key(name)
    }
}

fn erase_ty(ty: &mut Ty, units: &impl UnitNames) {
    match ty {
        Ty::Name(name) if units.is_unit(name) => *name = "Int".to_string(),
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

/// A rewrite of a whole program: of every type written in it, and of every
/// expression before its parts are visited.
trait ProgramRewrite {
    fn ty(&mut self, _ty: &mut Ty) {}
    /// The type name a typed rule argument (`__typed`) carries.
    fn typed_annotation(&mut self, _type_name: &mut String) {}
    fn expr(&mut self, expr: &mut Expr);
}

fn rewrite_params(params: &mut [Param], rewrite: &mut impl ProgramRewrite) {
    for param in params {
        if let Some(ty) = &mut param.ty {
            rewrite.ty(ty);
        }
    }
}

fn rewrite_stmts(stmts: &mut [Stmt], rewrite: &mut impl ProgramRewrite) {
    for stmt in stmts {
        rewrite_stmt(stmt, rewrite);
    }
}

fn rewrite_defn(defn: &mut Defn, rewrite: &mut impl ProgramRewrite) {
    match defn {
        Defn::Fn {
            params,
            ret_ty,
            body,
            ..
        } => {
            rewrite_params(params, rewrite);
            if let Some(ty) = ret_ty {
                rewrite.ty(ty);
            }
            rewrite_expr(body, rewrite);
        }
        Defn::Actor {
            state_param,
            handlers,
            ..
        } => {
            rewrite_params(std::slice::from_mut(state_param), rewrite);
            for handler in handlers {
                rewrite_expr(&mut handler.body, rewrite);
            }
        }
        Defn::Module { body, .. } => rewrite_stmts(body, rewrite),
    }
}

fn rewrite_variants(variants: &mut [Variant], rewrite: &mut impl ProgramRewrite) {
    for variant in variants {
        for field in &mut variant.fields {
            rewrite.ty(&mut field.ty);
        }
    }
}

fn rewrite_stmt(stmt: &mut Stmt, rewrite: &mut impl ProgramRewrite) {
    match stmt {
        Stmt::Defn(defn) => rewrite_defn(defn, rewrite),
        Stmt::TypeDecl(decl) => match decl {
            TypeDecl::ADT {
                params,
                variants,
                methods,
                ..
            } => {
                rewrite_params(params, rewrite);
                rewrite_variants(variants, rewrite);
                for method in methods {
                    rewrite_defn(method, rewrite);
                }
            }
            TypeDecl::WhenType {
                condition,
                variants,
                ..
            } => {
                rewrite_expr(condition, rewrite);
                rewrite_variants(variants, rewrite);
            }
            TypeDecl::EffectDecl { ops, .. } => {
                for (_, params, ret_ty) in ops {
                    rewrite_params(params, rewrite);
                    if let Some(ty) = ret_ty {
                        rewrite.ty(ty);
                    }
                }
            }
            TypeDecl::TraitDecl {
                params, methods, ..
            } => {
                rewrite_params(params, rewrite);
                for method in methods {
                    rewrite_params(&mut method.params, rewrite);
                    if let Some(ty) = &mut method.ret_ty {
                        rewrite.ty(ty);
                    }
                    if let Some(body) = &mut method.default_body {
                        rewrite_expr(body, rewrite);
                    }
                }
            }
            TypeDecl::ImplBlock { methods, .. } => {
                for method in methods {
                    rewrite_defn(method, rewrite);
                }
            }
            TypeDecl::RuleScope { params, body, .. } => {
                rewrite_params(params, rewrite);
                rewrite_stmts(body, rewrite);
            }
            TypeDecl::Unit { .. } => {}
        },
        Stmt::Rule(rule) => match rule {
            Rule::Clause { head, body } => {
                rewrite_expr(head, rewrite);
                if let Some(body) = body {
                    rewrite_expr(body, rewrite);
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
                rewrite_expr(head, rewrite);
                rewrite_expr(value, rewrite);
                if let Some(condition) = condition {
                    rewrite_expr(condition, rewrite);
                }
            }
            Rule::ReactiveScope { body, .. } => rewrite_stmts(body, rewrite),
        },
        Stmt::Bind(_, ty, value) | Stmt::MonadicBind(_, ty, value) => {
            if let Some(ty) = ty {
                rewrite.ty(ty);
            }
            rewrite_expr(value, rewrite);
        }
        Stmt::For(_, collection, body) => {
            rewrite_expr(collection, rewrite);
            rewrite_stmts(body, rewrite);
        }
        Stmt::While(condition, body) => {
            rewrite_expr(condition, rewrite);
            rewrite_stmts(body, rewrite);
        }
        Stmt::Send(target, message) => {
            rewrite_expr(target, rewrite);
            rewrite_expr(message, rewrite);
        }
        Stmt::StreamBind(_, value) | Stmt::Expr(value) => rewrite_expr(value, rewrite),
        Stmt::StreamSub(subject, arms) => {
            rewrite_expr(subject, rewrite);
            rewrite_arms(arms, rewrite);
        }
        Stmt::Invariant {
            subject, predicate, ..
        } => {
            rewrite_expr(subject, rewrite);
            rewrite_expr(predicate, rewrite);
        }
        Stmt::Prove {
            pass_block,
            else_block,
            ..
        } => {
            for block in [pass_block, else_block].into_iter().flatten() {
                rewrite_stmts(block, rewrite);
            }
        }
        Stmt::Annot(_, arguments) => {
            for argument in arguments {
                rewrite_expr(argument, rewrite);
            }
        }
        Stmt::Explore(query) => rewrite_explore(query, rewrite),
        Stmt::PreludeBoundary
        | Stmt::Use(_)
        | Stmt::Import(_)
        | Stmt::QualifiedImport(_, _)
        | Stmt::HashImport(_, _)
        | Stmt::Depend(_, _)
        | Stmt::RustBlock(_) => {}
    }
}

fn rewrite_explore(query: &mut ExploreQuery, rewrite: &mut impl ProgramRewrite) {
    for binding in &mut query.source.bindings {
        match &mut binding.kind {
            ExploreSourceBindingKind::Singleton { value }
            | ExploreSourceBindingKind::Finite { domain: value } => rewrite_expr(value, rewrite),
        }
    }
    match &mut query.successor.kind {
        ExploreSuccessorKind::Singleton { value }
        | ExploreSuccessorKind::Finite { domain: value } => rewrite_expr(value, rewrite),
    }
    for admission in &mut query.admissions {
        rewrite_expr(&mut admission.predicate, rewrite);
    }
    for find in &mut query.finds {
        match &mut find.selection {
            ExploreSelection::Matches { predicate, .. }
            | ExploreSelection::Violations { predicate, .. } => rewrite_expr(predicate, rewrite),
            ExploreSelection::All { .. } => {}
        }
    }
    for node in &mut query.analysis {
        if let ExploreAnalysisNode::Result(view) = node {
            for field in view.measures.iter_mut().chain(view.select.iter_mut()) {
                rewrite_expr(&mut field.value, rewrite);
            }
        }
    }
}

fn rewrite_arms(arms: &mut [MatchArm], rewrite: &mut impl ProgramRewrite) {
    for arm in arms {
        if let Some(guard) = &mut arm.guard {
            rewrite_expr(guard, rewrite);
        }
        rewrite_expr(&mut arm.body, rewrite);
    }
}

fn rewrite_expr(expr: &mut Expr, rewrite: &mut impl ProgramRewrite) {
    rewrite.expr(expr);
    match &mut expr.kind {
        ExprKind::App(function, arguments) => {
            if matches!(&function.kind, ExprKind::Var(name) if name == "__typed") {
                if let [value, annotation] = arguments.as_mut_slice() {
                    rewrite_expr(value, rewrite);
                    if let ExprKind::Var(type_name) = &mut annotation.kind {
                        rewrite.typed_annotation(type_name);
                    }
                    return;
                }
            }
            rewrite_expr(function, rewrite);
            for argument in arguments {
                rewrite_expr(argument, rewrite);
            }
        }
        ExprKind::Lambda(params, body) => {
            rewrite_params(params, rewrite);
            rewrite_expr(body, rewrite);
        }
        ExprKind::BinOp(_, left, right)
        | ExprKind::Index(left, right)
        | ExprKind::Pipe(left, right) => {
            rewrite_expr(left, rewrite);
            rewrite_expr(right, rewrite);
        }
        ExprKind::UnOp(_, inner) | ExprKind::Field(inner, _) | ExprKind::Try(inner) => {
            rewrite_expr(inner, rewrite)
        }
        ExprKind::If(condition, then_expr, else_expr) => {
            rewrite_expr(condition, rewrite);
            rewrite_expr(then_expr, rewrite);
            rewrite_expr(else_expr, rewrite);
        }
        ExprKind::Match(subject, arms) => {
            rewrite_expr(subject, rewrite);
            rewrite_arms(arms, rewrite);
        }
        ExprKind::Block(stmts) => rewrite_stmts(stmts, rewrite),
        ExprKind::List(items)
        | ExprKind::Tuple(items)
        | ExprKind::Effect(_, items)
        | ExprKind::Conjunction(items)
        | ExprKind::Disjunction(items) => {
            for item in items {
                rewrite_expr(item, rewrite);
            }
        }
        ExprKind::Handle { handlers, body, .. } => {
            for handler in handlers {
                rewrite_expr(&mut handler.body, rewrite);
            }
            rewrite_expr(body, rewrite);
        }
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
    }
}
