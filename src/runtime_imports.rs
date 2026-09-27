//! Plain imports merge declarations and initializers into one lexical program.
//! Keep the source of each statement while ordering that complete program.

use super::*;

#[derive(Clone)]
pub(super) struct RuntimeStatementSource {
    directory: Option<String>,
    diagnostics: Option<runtime_diagnostics::RuntimeDiagnosticContext>,
    type_source: Option<(Rc<str>, String)>,
}

pub(super) type RuntimeStatementSources = BTreeMap<usize, Rc<RuntimeStatementSource>>;

struct PlainImportProgram {
    statements: Vec<Stmt>,
    sources: Vec<Rc<RuntimeStatementSource>>,
    imports: BTreeSet<String>,
}

impl PlainImportProgram {
    fn append(&mut self, statement: &Stmt, source: &Rc<RuntimeStatementSource>) {
        self.statements.push(statement.clone());
        self.sources.push(source.clone());
    }

    fn expand(
        &mut self,
        interpreter: &Interpreter,
        statements: &[Stmt],
        source: Rc<RuntimeStatementSource>,
        namespace: &RuntimeNamespace,
        imported: bool,
    ) -> Result<(), String> {
        let prelude_len = TypeChecker::leading_rule_dispatch_prelude_indices(statements).len();
        for statement in &statements[..prelude_len] {
            self.append(statement, &source);
        }
        let statements = &statements[prelude_len..];
        // Imports establish the shared declarations regardless of where their
        // directives are written. Preserve import order within that prefix.
        for statement in statements {
            let Stmt::Import(path) = statement else {
                continue;
            };
            let directory = source.directory.as_deref().unwrap_or(".");
            let file = Interpreter::resolve_import_path_for_source(path, directory)
                .ok_or_else(|| format!("Cannot resolve import {path}"))?;
            let canonical = canonical_parsed_source_path(Path::new(&file));
            let canonical = canonical.to_string_lossy().into_owned();
            if namespace
                .state
                .borrow()
                .completed_imports
                .contains(&format!("plain:{canonical}"))
                || interpreter.runtime_active_import_paths.contains(&canonical)
                || !self.imports.insert(canonical.clone())
            {
                continue;
            }
            let module = parse_source_module_file_cached(Path::new(&canonical))
                .map_err(|error| format!("Cannot import {canonical}: {error}"))?;
            let imported_source = Rc::new(RuntimeStatementSource {
                directory: Some(Interpreter::imported_source_dir(&canonical)),
                diagnostics: source.diagnostics.as_ref().map(|_| {
                    interpreter.diagnostic_source_context(Path::new(&canonical), module.source())
                }),
                type_source: Some((
                    Rc::from(format!("{canonical:?}#{}", module.content_hash())),
                    namespace.identity_key(),
                )),
            });
            self.expand(
                interpreter,
                module.statements(),
                imported_source,
                namespace,
                true,
            )?;
        }
        for statement in statements {
            if !matches!(statement, Stmt::Import(_))
                && (!imported || Interpreter::is_runtime_import_statement(statement))
            {
                self.append(statement, &source);
            }
        }
        Ok(())
    }
}

impl Interpreter {
    fn current_statement_source(&self) -> RuntimeStatementSource {
        RuntimeStatementSource {
            directory: self.source_dir.clone(),
            diagnostics: self.runtime_diagnostic_context.clone(),
            type_source: self.runtime_plain_type_source.clone(),
        }
    }

    fn activate_statement_source(&mut self, source: &RuntimeStatementSource) {
        self.source_dir = source.directory.clone();
        self.runtime_diagnostic_context = source.diagnostics.clone();
        self.runtime_plain_type_source = source.type_source.clone();
    }

    pub(super) fn activate_program_statement_source(
        &mut self,
        statement: &Stmt,
        sources: Option<&RuntimeStatementSources>,
    ) {
        if let Some(source) =
            sources.and_then(|sources| sources.get(&(statement as *const Stmt as usize)))
        {
            self.activate_statement_source(source);
        }
    }

    pub(super) fn run_program_internal(
        &mut self,
        statements: &[Stmt],
        env: &mut Env,
        initialization_mode: RuntimeInitializationMode,
        order_value_bindings: bool,
    ) -> Value {
        if !statements
            .iter()
            .any(|statement| matches!(statement, Stmt::Import(_)))
        {
            return self.run_program_body(
                statements,
                env,
                initialization_mode,
                order_value_bindings,
                None,
            );
        }
        let previous = self.current_statement_source();
        let namespace = self.namespace_for_env(env);
        let mut program = PlainImportProgram {
            statements: Vec::new(),
            sources: Vec::new(),
            imports: BTreeSet::new(),
        };
        if let Err(error) = program.expand(
            self,
            statements,
            Rc::new(previous.clone()),
            &namespace,
            false,
        ) {
            self.report_runtime_import_failure(error);
            return Value::Unit;
        }
        let sources = program
            .statements
            .iter()
            .zip(&program.sources)
            .map(|(statement, source)| (statement as *const Stmt as usize, source.clone()))
            .collect::<RuntimeStatementSources>();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_program_body(
                &program.statements,
                env,
                initialization_mode,
                order_value_bindings,
                Some(&sources),
            )
        }));
        self.activate_statement_source(&previous);
        let value = match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        };
        if !self.calculation_failed()
            && !self.budget_exceeded
            && self.ground_error.borrow().is_none()
            && self.exhaustive_preview_error.borrow().is_none()
        {
            let mut state = namespace.state.borrow_mut();
            for path in program.imports {
                state.completed_imports.insert(format!("plain:{path}"));
                self.runtime_loaded_sources.insert(path);
            }
        }
        value
    }
}
