//! Top-level bindings that only metadata reads.
//!
//! `@ meta`, `@ section` and `@ field` resolve their values while the program
//! is checked. A top-level binding such a value is built from is compile-time
//! data when the value resolves to pure ground data. When no runtime code
//! reads the binding either, it is metadata only: no backend emits or
//! initializes it, and its value may be what only metadata can hold. A
//! metadata role list is erased, each role value keeping its own type
//! (`[Source(value = law), Guidance(value = note)]` with `law` and `note` of
//! different types), while a runtime list holds values of one type: such a
//! list is a check error in a binding runtime code reads.

use super::*;

impl TypeChecker {
    /// The top-level bindings, in the root file and in every file it imports,
    /// that `@ meta`, `@ section` or `@ field` values resolve to ground data,
    /// with the bindings those are built from. Runtime code may read them too.
    pub(crate) fn metadata_binding_candidates(&mut self, stmts: &[Stmt]) -> BTreeSet<String> {
        if let Some(candidates) = &self.metadata_binding_candidates {
            return candidates.clone();
        }
        let source = self.source_text.clone();
        let mut candidates = scan_meta_comments_with_dir_from_stmts_and_checker(
            &source,
            stmts,
            self.source_dir.clone(),
            Some(self),
        )
        .resolved_binding_names;
        candidates.extend(collect_imported_resolved_meta_binding_names_with_checker(
            stmts,
            self.source_dir.as_deref(),
            self,
        ));
        self.metadata_binding_candidates = Some(candidates.clone());
        candidates
    }

    /// Of the metadata binding candidates, those no runtime code reads: no
    /// statement but metadata reads them, directly or through another
    /// binding runtime code reads. A binding written twice, or marked
    /// `@ comptime`, is runtime data as the backends emit it.
    fn metadata_only_bindings(&mut self, stmts: &[Stmt]) -> BTreeSet<String> {
        let candidates = self.metadata_binding_candidates(stmts);
        if candidates.is_empty() {
            return candidates;
        }
        let files = query::program_files(stmts, &self.source_text, self);
        let mut binding_counts = BTreeMap::<&str, usize>::new();
        let mut binding_values = BTreeMap::<&str, Vec<&Expr>>::new();
        let mut comptime = BTreeSet::<&str>::new();
        for file in &files {
            let mut after_comptime = false;
            for stmt in file.statements {
                match stmt {
                    Stmt::Annot(name, _) => {
                        after_comptime = name == "comptime";
                        continue;
                    }
                    Stmt::Bind(Pat::Var(name), _, value) => {
                        *binding_counts.entry(name).or_default() += 1;
                        binding_values.entry(name).or_default().push(value);
                        if after_comptime {
                            comptime.insert(name);
                        }
                    }
                    Stmt::StreamBind(name, _) => {
                        *binding_counts.entry(name).or_default() += 1;
                    }
                    Stmt::Bind(..)
                    | Stmt::PreludeBoundary
                    | Stmt::Defn(..)
                    | Stmt::TypeDecl(..)
                    | Stmt::Rule(..)
                    | Stmt::Use(..)
                    | Stmt::Import(..)
                    | Stmt::QualifiedImport(..)
                    | Stmt::HashImport(..)
                    | Stmt::Depend(..)
                    | Stmt::RustBlock(..)
                    | Stmt::MonadicBind(..)
                    | Stmt::For(..)
                    | Stmt::While(..)
                    | Stmt::Send(..)
                    | Stmt::StreamSub(..)
                    | Stmt::Invariant { .. }
                    | Stmt::Prove { .. }
                    | Stmt::Explore(..)
                    | Stmt::Expr(..) => {}
                }
                after_comptime = false;
            }
        }
        let compile_time = candidates
            .iter()
            .map(String::as_str)
            .filter(|name| binding_counts.get(name) == Some(&1) && !comptime.contains(name))
            .collect::<BTreeSet<_>>();

        let no_qualified_modules = BTreeSet::new();
        let mut runtime_reads = BTreeSet::new();
        for file in &files {
            for stmt in file.statements {
                if matches!(stmt, Stmt::Bind(Pat::Var(name), _, _) if compile_time.contains(name.as_str()))
                {
                    continue;
                }
                runtime_reads.extend(references::statement_references(
                    stmt,
                    &no_qualified_modules,
                ));
            }
        }
        let mut pending = runtime_reads
            .iter()
            .filter(|name| compile_time.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        while let Some(name) = pending.pop() {
            for value in binding_values.get(name.as_str()).into_iter().flatten() {
                for read in references::expression_references(value, &no_qualified_modules) {
                    if compile_time.contains(read.as_str()) && runtime_reads.insert(read.clone()) {
                        pending.push(read);
                    }
                }
            }
        }
        compile_time
            .into_iter()
            .filter(|name| !runtime_reads.contains(*name))
            .map(str::to_string)
            .collect()
    }

    /// A list holds values of one type. A list whose elements have types
    /// that do not join is an error, unless metadata alone reads the
    /// top-level binding it is written in (`metadata_only_bindings`), which
    /// `check_program` decides once the whole program is checked.
    pub(super) fn check_list_element_types(&mut self, items: &[Expr]) {
        if self.checking_metadata_value {
            return;
        }
        let mut joined: Option<(String, bool)> = None;
        for item in items {
            let Some(item_type) = self.infer_expr_type_name(item) else {
                continue;
            };
            let literal = Self::is_unitless_literal_value(item);
            let Some((expected, expected_literal)) = joined.take() else {
                joined = Some((item_type, literal));
                continue;
            };
            if let Some(merged) =
                self.merge_branch_types(&expected, expected_literal, &item_type, literal)
            {
                joined = Some((merged, expected_literal && literal));
                continue;
            }
            let mut message = format!(
                "list elements must have the same type; first element is `{expected}` but this element is `{item_type}`"
            );
            if let Some(binding) = self.top_level_binding.clone() {
                message.push_str(&format!(
                    ". `{binding}` is runtime data, and a runtime list holds values of one type"
                ));
                let role_types = self.implementations_for_trait("MetaRole");
                let is_role = |type_name: &str| {
                    role_types.contains(type_name.split('(').next().unwrap_or(type_name))
                };
                if is_role(&expected) && is_role(&item_type) {
                    message.push_str(&format!(
                        "; only a binding that nothing but metadata (`@ field`, `@ meta`) reads may list role values of different types, so read `{binding}` from metadata alone or give its role values one type"
                    ));
                }
                self.mixed_list_bindings.push((binding, message.clone()));
            }
            self.error_at_expr(item, message);
            return;
        }
    }

    /// Withdraw the list errors of the bindings only metadata reads: their
    /// values are compile-time role lists, never runtime lists.
    pub(super) fn withdraw_metadata_only_list_errors(&mut self, stmts: &[Stmt]) {
        if self.mixed_list_bindings.is_empty() {
            return;
        }
        let metadata_only = self.metadata_only_bindings(stmts);
        let withdrawn = std::mem::take(&mut self.mixed_list_bindings)
            .into_iter()
            .filter(|(binding, _)| metadata_only.contains(binding))
            .map(|(_, message)| message)
            .collect::<BTreeSet<_>>();
        if !withdrawn.is_empty() {
            self.diagnostics
                .retain(|diagnostic| !withdrawn.contains(&diagnostic.message));
        }
    }

    /// Check a top-level statement of a file, remembering the binding it
    /// declares for the list check.
    pub(super) fn with_top_level_binding(&mut self, stmt: &Stmt, check: impl FnOnce(&mut Self)) {
        let binding = if let Stmt::Bind(Pat::Var(name), _, _) = stmt {
            Some(name.clone())
        } else {
            None
        };
        let previous = std::mem::replace(&mut self.top_level_binding, binding);
        check(self);
        self.top_level_binding = previous;
    }
}

/// The bindings the `@ field(...)` values in `stmts` are built from, through
/// the bindings those read in turn, for each value that resolves to ground
/// data.
pub(crate) fn field_meta_binding_names(
    source: &str,
    stmts: &[Stmt],
    source_dir: Option<&str>,
    checker: &TypeChecker,
) -> BTreeSet<String> {
    let values = stmts
        .iter()
        .flat_map(|stmt| -> Vec<&FieldMetaDecl> {
            match stmt {
                Stmt::TypeDecl(TypeDecl::ADT {
                    field_meta: Some(field_meta),
                    ..
                }) => field_meta
                    .type_default
                    .iter()
                    .chain(field_meta.members.iter().map(|member| &member.meta))
                    .collect(),
                Stmt::TypeDecl(TypeDecl::Scalar {
                    field_meta: Some(field_meta),
                    ..
                }) => vec![field_meta],
                Stmt::TypeDecl(..)
                | Stmt::PreludeBoundary
                | Stmt::Defn(..)
                | Stmt::Rule(..)
                | Stmt::Use(..)
                | Stmt::Import(..)
                | Stmt::QualifiedImport(..)
                | Stmt::HashImport(..)
                | Stmt::Depend(..)
                | Stmt::RustBlock(..)
                | Stmt::Annot(..)
                | Stmt::Bind(..)
                | Stmt::MonadicBind(..)
                | Stmt::For(..)
                | Stmt::While(..)
                | Stmt::Send(..)
                | Stmt::StreamBind(..)
                | Stmt::StreamSub(..)
                | Stmt::Invariant { .. }
                | Stmt::Prove { .. }
                | Stmt::Explore(..)
                | Stmt::Expr(..) => Vec::new(),
            }
        })
        .flat_map(|declaration| declaration.entries.iter().map(|(_, value)| value))
        .filter(|value| !matches!(value.kind, ExprKind::Lit(_)))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return BTreeSet::new();
    }
    let mut roots = BTreeSet::new();
    for value in &values {
        collect_meta_expr_dependencies(value, &mut roots);
    }
    let mut bindings = BTreeMap::new();
    collect_local_meta_bindings(source, stmts, None, &mut bindings);
    if meta_binding_dependencies(&roots, &bindings, &checker.constructors)
        .iter()
        .any(|name| !bindings.contains_key(name))
    {
        collect_imported_meta_bindings(stmts, source_dir, &mut bindings, &mut BTreeSet::new());
    }
    let value_bindings = bindings
        .iter()
        .map(|(name, definition)| (name.clone(), &definition.expr))
        .collect::<BTreeMap<_, _>>();
    let mut evaluator = MetaGroundEvaluator::new(stmts, source_dir);
    let mut resolved_roots = BTreeSet::new();
    for value in values {
        if resolve_static_meta_expr(
            value,
            &value_bindings,
            &checker.constructors,
            &mut BTreeSet::new(),
            &mut evaluator,
        )
        .is_some()
        {
            collect_meta_expr_dependencies(value, &mut resolved_roots);
        }
    }
    meta_binding_dependencies(&resolved_roots, &bindings, &checker.constructors)
        .into_iter()
        .filter(|name| bindings.contains_key(name))
        .collect()
}
