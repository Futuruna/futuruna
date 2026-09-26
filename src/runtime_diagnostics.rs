//! An opt-in ordinary-program error boundary. User faults unwind immediately
//! without invoking Rust's panic hook; unrelated panics keep their identity.

use super::*;

pub(super) struct RuntimeDiagnosticContext {
    path: PathBuf,
    source: Arc<str>,
    pub(super) span: Option<Span>,
    pub(super) function_depth: usize,
    pub(super) rule_depth: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_calculation_and_ground_failure_channels() {
        let source = "\"value\".missing";
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .unwrap();
        let mut interpreter = Interpreter::new();
        let mut env = interpreter.default_env();
        let result = interpreter
            .with_calculation_runtime(|runtime| runtime.run_program(&statements, &mut env));
        assert!(
            matches!(result,
            Err(CalculationRuntimeFailure::InvalidValue(ref message))
                if message.contains("no field or method `missing`")),
            "{result:?}"
        );

        interpreter.ground_collection_limit = Some(1024);
        interpreter.run_program(&statements, &mut env);
        let error = interpreter.ground_error.take();
        assert!(
            matches!(error,
            Some(ExploreRuntimeFailure::RuntimeError { ref message })
                if message.contains("no field or method `missing`")),
            "{error:?}"
        );
    }

    #[test]
    fn unknown_effect_uses_calculation_and_ground_failure_channels() {
        let mut interpreter = Interpreter::new();
        let result = interpreter.with_calculation_runtime(|runtime| {
            runtime.eval_effect("assert", vec![Value::Bool(false)])
        });
        assert!(
            matches!(result,
            Err(CalculationRuntimeFailure::InvalidValue(ref message))
                if message.contains("unknown effect `assert`")),
            "{result:?}"
        );

        interpreter.ground_collection_limit = Some(1024);
        interpreter.eval_effect("log", vec![Value::Str("audit trail".into())]);
        let error = interpreter.ground_error.take();
        assert!(
            matches!(error,
            Some(ExploreRuntimeFailure::RuntimeError { ref message })
                if message.contains("unknown effect `log`")),
            "{error:?}"
        );
    }

    #[test]
    fn source_error_boundary_does_not_reclassify_internal_panics() {
        let mut interpreter = Interpreter::new();
        let mut env = interpreter.default_env();
        let source = "# Entry(value: Int)\n";
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .unwrap();
        // A conflicting internal registry borrow simulates an interpreter bug,
        // rather than any fault caused by the authored source program.
        let namespace = interpreter.runtime_root.clone();
        let _borrow = namespace.state.borrow_mut();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            interpreter.run_program_with_diagnostics(
                &statements,
                &mut env,
                Path::new("internal.runa"),
                source,
            )
        }));
        assert!(
            result.is_err(),
            "an internal panic must not become a user diagnostic"
        );
        assert!(interpreter.runtime_diagnostic_context.is_none());
    }
}

struct RuntimeUserError(Diagnostic);

impl Interpreter {
    pub(super) fn diagnostic_source_context(
        &self,
        path: &Path,
        source: &str,
    ) -> RuntimeDiagnosticContext {
        RuntimeDiagnosticContext {
            path: path.to_path_buf(),
            source: Arc::from(source),
            span: None,
            function_depth: self.runtime_function_call_depth.get(),
            rule_depth: self.runtime_rule_call_depth.get(),
        }
    }

    /// Execute an ordinary program, reporting expected user faults as source
    /// diagnostics. Internal Rust panics still unwind. Discard the interpreter
    /// after an error; effects that already completed are not rolled back.
    pub fn run_program_with_diagnostics(
        &mut self,
        statements: &[Stmt],
        env: &mut Env,
        path: &Path,
        source: &str,
    ) -> Result<Value, Diagnostic> {
        let context = self.diagnostic_source_context(path, source);
        let previous = self.runtime_diagnostic_context.replace(context);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_program(statements, env)
        }));
        self.runtime_diagnostic_context = previous;
        match result {
            Ok(value) => Ok(value),
            Err(payload) => match payload.downcast::<RuntimeUserError>() {
                Ok(error) => Err(error.0),
                Err(payload) => std::panic::resume_unwind(payload),
            },
        }
    }

    pub(super) fn ordinary_runtime_fail(&self, message: String) -> ! {
        if let Some(context) = &self.runtime_diagnostic_context {
            let mut diagnostic = Diagnostic::error(message);
            diagnostic.origin = Some(DiagnosticOrigin {
                path: context.path.clone(),
                source: context.source.clone(),
                span: context.span,
            });
            diagnostic
                .context
                .push("while evaluating this expression".into());
            // Unlike panic_any, resume_unwind does not run the process-wide
            // panic hook. The private payload is caught only by our boundary.
            std::panic::resume_unwind(Box::new(RuntimeUserError(diagnostic)));
        }
        panic!("{message}")
    }
}
