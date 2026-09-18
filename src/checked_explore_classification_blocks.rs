//! Pure local blocks, including eager evaluation of unused initializers.

use super::*;

impl<'program, 'query> CheckedClassificationProducer<'program, 'query> {
    pub(super) fn lower_strict_block(
        &mut self,
        site: &ExprSiteId,
        statements: &[Stmt],
        ty: ClassificationTypeId,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        if statements.len() > 256 || !matches!(statements.last(), Some(Stmt::Expr(_))) {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [],
            ));
        }
        let mut environment = environment.clone();
        let mut values = Vec::with_capacity(statements.len());
        for (ordinal, statement) in statements.iter().enumerate() {
            let statement_site = child_site(site, ordinal);
            if !matches!(statement, Stmt::Bind(Pat::Var(_), _, _) | Stmt::Expr(_)) {
                return Err(self.residual_error(
                    &statement_site,
                    ClassificationResidualReason::UnsupportedExpression,
                    [],
                ));
            }
            let value = self.lower_expression(&child_site(&statement_site, 0), &environment)?;
            if matches!(statement, Stmt::Bind(Pat::Var(_), _, _)) {
                environment.insert(
                    crate::checked_local_value_binder_site(&statement_site),
                    BinderValue::Lowered(value),
                );
            }
            values.push(value);
        }
        self.lower_strict_sequence(site, &values, ty)
    }

    /// Evaluate every value eagerly, then return the last. Shared by blocks
    /// and matches: substitution must not erase an unused failing expression.
    pub(super) fn lower_strict_sequence(
        &mut self,
        site: &ExprSiteId,
        values: &[LoweredValue],
        ty: ClassificationTypeId,
    ) -> LoweringResult {
        let Some(result) = values.last().copied().filter(|_| values.len() <= 256) else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [],
            ));
        };
        if result.ty != ty {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Node(result.node)],
            ));
        }
        if values.len() == 1 {
            return Ok(result);
        }

        // Substitution alone would erase unused RHSs and preceding expressions,
        // including checked overflow/division failures. A strict Call evaluates
        // every argument in source order before returning its final argument.
        // Both concrete graph evaluation and product proofs enforce that rule.
        // The helper is closed; outer parameters occur only in its arguments.
        let mut hasher = Sha256::new();
        hasher.update(b"futuruna.checked-classification-strict-block.v1\0");
        hasher.update((values.len() as u32).to_le_bytes());
        for value in values {
            hasher.update(value.ty.bytes());
        }
        let callable_id =
            ClassificationCallableId::from_checked_callable_digest(hasher.finalize().into());
        let body = self.intern(
            ty,
            result.scalar,
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal: (values.len() - 1) as u32,
            },
        )?;
        self.callable_definitions
            .entry(callable_id)
            .or_insert_with(|| ClassificationCallableDefinition {
                callable_id,
                parameter_types: values.iter().map(|value| value.ty).collect(),
                return_type: ty,
                body: body.node,
            });
        self.intern(
            ty,
            result.scalar,
            ClassificationNodeKind::Call {
                callable_id,
                arguments: values.iter().map(|value| value.node).collect(),
            },
        )
    }
}
