//! Ordinary expression diagnostics reuse child type results while walking a
//! binary tree. Re-inferring every prefix of a long sum is quadratic work.

use super::*;

impl TypeChecker {
    pub(super) fn binary_expression_result_type(
        operator: &str,
        left: Option<&str>,
        right: Option<&str>,
    ) -> Option<String> {
        match operator {
            "<" | ">" | "<=" | ">=" | "==" | "!=" | "&&" | "||" => Some("Bool".into()),
            "+" if left == Some("String") || right == Some("String") => Some("String".into()),
            "+" | "-" | "*" | "/"
                if matches!(
                    (left, right),
                    (Some("Int"), Some("Float")) | (Some("Float"), Some("Int"))
                ) =>
            {
                Some("Float".into())
            }
            _ => Self::merge_inferred_type_names(left?, right?),
        }
    }

    pub(super) fn check_binary_expression(
        &mut self,
        expression: &Expr,
        in_fn: Option<&str>,
    ) -> Option<String> {
        let ExprKind::BinOp(operator, left, right) = &expression.kind else {
            self.check_expr(expression, in_fn);
            return self.infer_expr_type_name(expression);
        };
        let left_type = self.check_binary_expression(left, in_fn);
        let right_type = self.check_binary_expression(right, in_fn);
        if operator == "+" {
            let is_list = |expression: &Expr, type_name: Option<&str>| {
                matches!(expression.kind, ExprKind::List(_))
                    || type_name
                        .is_some_and(|ty| Self::applied_type_argument(ty, "List", 0).is_some())
            };
            if is_list(left, left_type.as_deref()) && is_list(right, right_type.as_deref()) {
                self.error_at_expr(
                    expression,
                    "operator `+` does not concatenate lists; use concat(left, right)".into(),
                );
                // Retain the inferred list type so enclosing operators can
                // still diagnose their operands after this local error.
                return Self::binary_expression_result_type(
                    operator,
                    left_type.as_deref(),
                    right_type.as_deref(),
                );
            }
        }
        self.check_operator_operands(
            expression,
            operator,
            left,
            left_type.as_deref(),
            right,
            right_type.as_deref(),
        );
        Self::binary_expression_result_type(operator, left_type.as_deref(), right_type.as_deref())
    }

    fn ordinary_operand_type(expression: &Expr, inferred: Option<&str>) -> Option<Ty> {
        inferred
            .and_then(|ty| parse_type_annotation(ty).ok())
            .or_else(|| {
                matches!(expression.kind, ExprKind::List(_))
                    .then(|| Ty::App(Box::new(Ty::Name("List".into())), vec![Ty::Hole]))
            })
    }

    // Only reject operands whose outer type is known. Generic parameters and
    // opaque Rust types need their normal call/instantiation checks; missing
    // ordinary type evidence does not establish that an operator is invalid.
    fn ordinary_operand_kind(ty: &Ty) -> Option<&'static str> {
        match ty {
            Ty::Name(name) => match name.as_str() {
                "Int" => Some("Int"),
                "Float" => Some("Float"),
                "Bool" => Some("Bool"),
                "String" => Some("String"),
                "Char" => Some("Char"),
                "Unit" => Some("()"),
                _ => None,
            },
            Ty::Unit => Some("()"),
            Ty::App(head, _) if matches!(head.as_ref(), Ty::Name(name) if matches!(name.as_str(), "List" | "Map" | "Set" | "Tuple" | "Option" | "Result")) => {
                Some("structured")
            }
            Ty::Optional(_) => Some("structured"),
            Ty::Arrow(_, _) => Some("function"),
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                Self::ordinary_operand_kind(inner)
            }
            _ => None,
        }
    }

    fn check_operator_operands(
        &mut self,
        expression: &Expr,
        operator: &str,
        left: &Expr,
        left_type: Option<&str>,
        right: &Expr,
        right_type: Option<&str>,
    ) {
        let left_ty = Self::ordinary_operand_type(left, left_type);
        let right_ty = Self::ordinary_operand_type(right, right_type);
        let left_kind = left_ty.as_ref().and_then(Self::ordinary_operand_kind);
        let right_kind = right_ty.as_ref().and_then(Self::ordinary_operand_kind);
        let numeric = |kind| matches!(kind, "Int" | "Float");
        let mixed_numbers = matches!(
            (left_kind, right_kind),
            (Some("Int"), Some("Float")) | (Some("Float"), Some("Int"))
        );
        let invalid = match operator {
            "+" if left_kind == Some("String") || right_kind == Some("String") => false,
            "+" => left_kind
                .zip(right_kind)
                .is_some_and(|(l, r)| !numeric(l) || !numeric(r)),
            "-" | "*" | "/" | "%" => {
                left_kind.is_some_and(|kind| !numeric(kind))
                    || right_kind.is_some_and(|kind| !numeric(kind))
                    || (operator == "%" && mixed_numbers)
            }
            "==" | "!=" => left_kind.zip(right_kind).is_some_and(|(l, r)| l != r),
            "<" | ">" | "<=" | ">=" => left_kind.zip(right_kind).is_some_and(|(l, r)| {
                !(numeric(l) && numeric(r) || l == r && matches!(l, "String" | "Char"))
            }),
            "&&" | "||" => {
                left_kind.is_some_and(|kind| kind != "Bool")
                    || right_kind.is_some_and(|kind| kind != "Bool")
            }
            _ => false,
        };
        if invalid {
            let type_name = |ty: Option<&Ty>| {
                ty.map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".into())
            };
            let hint = if mixed_numbers {
                "; use to_float to give both operands type Float"
            } else {
                ""
            };
            self.error_at_expr(
                expression,
                format!(
                    "unsupported operands for operator `{operator}`: `{}` and `{}`{hint}",
                    type_name(left_ty.as_ref()),
                    type_name(right_ty.as_ref()),
                ),
            );
        }
    }

    pub(super) fn check_unary_expression(&mut self, expression: &Expr, in_fn: Option<&str>) {
        let ExprKind::UnOp(operator, operand) = &expression.kind else {
            return;
        };
        self.check_expr(operand, in_fn);
        let inferred = self.infer_expr_type_name(operand);
        let ty = Self::ordinary_operand_type(operand, inferred.as_deref());
        if ty
            .as_ref()
            .and_then(Self::ordinary_operand_kind)
            .is_some_and(|kind| match operator.as_str() {
                "!" => kind != "Bool",
                "-" | "+" => !matches!(kind, "Int" | "Float"),
                _ => false,
            })
        {
            self.error_at_expr(
                expression,
                format!(
                    "unsupported operands for operator `{operator}`: `{}`",
                    ty.unwrap()
                ),
            );
        }
    }

    pub(super) fn check_if_condition(&mut self, condition: &Expr, in_fn: Option<&str>) {
        self.check_expr(condition, in_fn);
        let inferred = self.infer_expr_type_name(condition);
        let ty = Self::ordinary_operand_type(condition, inferred.as_deref());
        if ty
            .as_ref()
            .and_then(Self::ordinary_operand_kind)
            .is_some_and(|kind| kind != "Bool")
        {
            self.error_at_expr(
                condition,
                format!("if condition must return Bool, got `{}`", ty.unwrap()),
            );
        }
    }

    /// `= x <- e` unwraps a `Result` or `Option` (returning early on `Err` or
    /// `None`) or resumes from an effect operation. Returns the bound value's
    /// type when it is known.
    pub(super) fn check_monadic_bind_operand(&mut self, operand: &Expr) -> Option<String> {
        if let ExprKind::App(function, _) = &operand.kind {
            if let ExprKind::Var(operation) = &function.kind {
                if is_builtin_effect(builtin_canonical(operation))
                    || self
                        .effect_ops
                        .values()
                        .any(|operations| operations.contains_key(operation))
                {
                    return None;
                }
            }
        }
        let actual = self.ordinary_expression_type(operand)?;
        let mut ty = parse_type_annotation(&actual).ok()?;
        while let Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) = ty {
            ty = *inner;
        }
        let inner = match &ty {
            Ty::Optional(inner) => Some(inner.as_ref().clone()),
            Ty::App(head, arguments) if matches!(head.as_ref(), Ty::Name(name) if name == "Result" || name == "Option") => {
                arguments.first().cloned()
            }
            Ty::Name(name) if name == "Result" || name == "Option" => None,
            Ty::Var(_) | Ty::Hole => return None,
            _ => {
                self.error_at_expr(
                    operand,
                    format!(
                        "`<-` needs a `Result` or `Option` value, got `{actual}`; bind a plain value with `= name = ...`"
                    ),
                );
                return None;
            }
        };
        inner
            .filter(|inner| !matches!(inner, Ty::Var(_) | Ty::Hole))
            .map(|inner| Self::canonical_explore_ty_name(&inner))
    }
}
