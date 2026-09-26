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
            }
        }
        Self::binary_expression_result_type(operator, left_type.as_deref(), right_type.as_deref())
    }
}
