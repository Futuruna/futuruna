//! A name in a file means what that file's scope says: the file's own
//! declaration, or else the one its plain imports provide (a later import's
//! over an earlier one's), never a declaration of a file that imports it.
//! Functions and top-level bindings follow this rule, each in its own
//! namespace: a bare name means a binding the file sees, a call a function
//! the file sees (else a binding holding one).
//!
//! Plain imports still merge every file into one program, and the checker,
//! the interpreter, the native backend and the solver keep one registry of
//! functions and one of top-level bindings by name. So when two functions,
//! or two bindings, of one name meet in a merged program, each one except
//! the declaration the root file sees gets an internal name, and every name
//! that means it is written with that name: in the file that declares it and
//! in each file that reaches it through its imports. Every binding is then
//! unique, so bindings initialize in dependency order across the files.
//! `Lib.fee` of a qualified import is rewritten the same way. The
//! rewritten files belong to the checked program and are read through its
//! elaboration (see `constructor_scope`), like its constructor names; a
//! module's identity stays its written content. The elaboration also keeps
//! each internal name's written name and file, so no internal name reaches
//! the reader: diagnostics, runtime errors, function values, explanations
//! and the solver's script show `` `ratio` in a.runa ``
//! (`constructor_scope::function_display_text` and its siblings).
//!
//! A qualified import is a merged program of its own: two functions or
//! bindings of one name in it are told apart in the same way.
//!
//! A name in an imported file that no function or binding of the file or
//! its imports declares, while a file importing it declares one, would
//! reach that importer's declaration in the merged program; it is an error
//! in the imported file.
//!
//! A rule family spans plain imports: an importer's clause joins the
//! imported family. So a rule and a function of one name in the scope of one
//! file are an error naming both lines, as in a single file.

use super::*;
use crate::constructor_scope::ScopedFunction;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

/// One file of the checked program: the root, or a file it imports.
struct ScopeFile {
    /// Canonical path; `None` for the root.
    path: Option<PathBuf>,
    module: Option<Arc<ParsedSourceModule>>,
    /// The root's import directive through which the file is first reached.
    via: Option<String>,
    plain: Vec<usize>,
    qualified: Vec<(String, usize)>,
    /// Top-level functions and rule families, at their first declaration.
    functions: BTreeMap<String, Span>,
    rules: BTreeMap<String, Span>,
    /// Top-level bindings, at their first value.
    values: BTreeMap<String, Span>,
}

impl ScopeFile {
    fn new(
        path: Option<PathBuf>,
        module: Option<Arc<ParsedSourceModule>>,
        via: Option<String>,
        statements: &[Stmt],
    ) -> Self {
        let mut functions = BTreeMap::new();
        let mut rules = BTreeMap::new();
        let mut values = BTreeMap::new();
        for statement in statements {
            match statement {
                Stmt::Defn(Defn::Fn { name, body, .. }) => {
                    functions.entry(name.clone()).or_insert(body.span);
                }
                Stmt::Rule(rule) => {
                    if let (Some(head), Some((name, _))) = (rule.head(), rule.callable_name_arity())
                    {
                        rules.entry(name).or_insert(head.span);
                    }
                }
                Stmt::Bind(pattern, _, value) | Stmt::MonadicBind(pattern, _, value) => {
                    let mut names = BTreeSet::new();
                    collect_pattern_names(pattern, &mut names);
                    for name in names {
                        values.entry(name).or_insert(value.span);
                    }
                }
                Stmt::StreamBind(name, value) => {
                    values.entry(name.clone()).or_insert(value.span);
                }
                _ => {}
            }
        }
        Self {
            path,
            module,
            via,
            plain: Vec::new(),
            qualified: Vec::new(),
            functions,
            rules,
            values,
        }
    }
}

/// A rule family and a function of one name that one file's scope reaches.
struct RuleAndFunction {
    name: String,
    rule: (usize, Span),
    function: (usize, Span),
}

/// A name in an imported file that no function (or binding) of the file or
/// its imports declares, while a file importing it declares a function (or
/// binding) of that name: the merged program would reach the importer's
/// declaration there.
struct ImporterFunction {
    name: String,
    reference: (usize, Span),
    function: (usize, Span),
    /// The importer's declaration is a top-level binding.
    binding: bool,
}

/// The files of a checked program as its import scopes rewrite them.
#[derive(Default)]
pub(crate) struct ImportScopes {
    /// The root statements, when a qualified member access in them is
    /// rewritten.
    root: Option<Vec<Stmt>>,
    /// Rewritten imported files, by canonical path.
    modules: BTreeMap<PathBuf, Arc<ParsedSourceModule>>,
    files: Vec<ScopeFile>,
    collisions: Vec<RuleAndFunction>,
    importer_functions: Vec<ImporterFunction>,
    /// Each renamed function's written name and file, by internal name.
    functions: BTreeMap<String, ScopedFunction>,
}

impl ImportScopes {
    /// Resolve the scopes of `root` (with or without its prelude) and the
    /// files it imports from `source_dir`.
    pub(crate) fn resolve(root: &[Stmt], source_dir: Option<&str>) -> Self {
        let program_start = root
            .iter()
            .rposition(|statement| matches!(statement, Stmt::PreludeBoundary))
            .map_or(0, |boundary| boundary + 1);
        let program = &root[program_start..];
        if !program
            .iter()
            .any(|statement| matches!(statement, Stmt::Import(_) | Stmt::QualifiedImport(..)))
        {
            return Self::default();
        }
        let files = discover(program, source_dir.unwrap_or("."));
        let function_scopes = scopes(&files, Declared::Functions);
        let value_scopes = scopes(&files, Declared::Values);
        let closures = (0..files.len())
            .map(|file| plain_closure(&files, file))
            .collect::<Vec<_>>();
        let [internal_functions, internal_values] = internal_names(
            &files,
            [
                renamed_declarations(&files, &function_scopes, &closures, Declared::Functions),
                renamed_declarations(&files, &value_scopes, &closures, Declared::Values),
            ],
        );
        let collisions = rules_and_functions(&files, &function_scopes, &closures);
        let importer_functions =
            importer_functions(&files, &function_scopes, &value_scopes, &closures);

        let renamed_in = |scope: &BTreeMap<String, usize>,
                          internal: &BTreeMap<(usize, String), String>| {
            scope
                .iter()
                .filter_map(|(name, declaring)| {
                    internal
                        .get(&(*declaring, name.clone()))
                        .map(|internal| (name.clone(), internal.clone()))
                })
                .collect::<BTreeMap<_, _>>()
        };
        let mut root_statements = None;
        let mut modules = BTreeMap::new();
        for (index, file) in files.iter().enumerate() {
            let functions = renamed_in(&function_scopes[index], &internal_functions);
            let values = renamed_in(&value_scopes[index], &internal_values);
            let mut members = BTreeMap::new();
            for (alias, target) in &file.qualified {
                for (scope, internal) in [
                    (&value_scopes[*target], &internal_values),
                    (&function_scopes[*target], &internal_functions),
                ] {
                    for (name, internal) in renamed_in(scope, internal) {
                        members.insert((alias.clone(), name), internal);
                    }
                }
            }
            if functions.is_empty() && values.is_empty() && members.is_empty() {
                continue;
            }
            let names = FileNames {
                functions: &functions,
                values: &values,
                members: &members,
                visible_functions: function_scopes[index].keys().cloned().collect(),
                visible_values: value_scopes[index].keys().cloned().collect(),
            };
            match &file.module {
                None => {
                    let mut statements = root.to_vec();
                    Renamer::new(names).file(&mut statements[program_start..]);
                    root_statements = Some(statements);
                }
                Some(module) => {
                    let mut statements = module.statements().to_vec();
                    Renamer::new(names).file(&mut statements);
                    let path = file.path.clone().expect("an imported file has a path");
                    modules.insert(
                        path,
                        Arc::new(ParsedSourceModule::elaborated(module, statements)),
                    );
                }
            }
        }
        let functions = internal_functions
            .iter()
            .chain(internal_values.iter())
            .map(|((file, name), internal)| {
                let path = files[*file].path.as_deref().unwrap_or(Path::new(""));
                (
                    internal.clone(),
                    ScopedFunction {
                        name: name.clone(),
                        file: file_label(path, source_dir),
                    },
                )
            })
            .collect();
        Self {
            root: root_statements,
            modules,
            files,
            collisions,
            importer_functions,
            functions,
        }
    }

    /// The root statements with their qualified member accesses rewritten,
    /// when there are any.
    pub(crate) fn take_root(&mut self) -> Option<Vec<Stmt>> {
        self.root.take()
    }

    /// Read the rewritten files, and show their renamed functions by their
    /// written names, on this thread from now on.
    pub(crate) fn install(&self) {
        ConstructorElaboration::install_import_scopes(&self.modules, &self.functions);
    }
}

/// `path` relative to the root file's directory, as diagnostics show a file.
fn file_label(path: &Path, root_dir: Option<&str>) -> String {
    let root = root_dir.map(|dir| canonical_source_path(Path::new(dir)));
    root.as_deref()
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The root and every file it reaches through plain and qualified imports.
fn discover(program: &[Stmt], root_dir: &str) -> Vec<ScopeFile> {
    let mut files = vec![ScopeFile::new(None, None, None, program)];
    let mut directories = vec![root_dir.to_string()];
    let mut indices = BTreeMap::<PathBuf, usize>::new();
    let mut next = 0;
    while next < files.len() {
        let current = next;
        next += 1;
        let module = files[current].module.clone();
        let statements = module
            .as_deref()
            .map_or(program, |module| module.statements());
        for statement in statements {
            let (alias, path) = match statement {
                Stmt::Import(path) => (None, path),
                Stmt::QualifiedImport(alias, path) => (Some(alias), path),
                _ => continue,
            };
            let Some(file) =
                Interpreter::resolve_import_path_for_source(path, &directories[current])
            else {
                continue;
            };
            let canonical = canonical_source_path(Path::new(&file));
            let target = match indices.get(&canonical) {
                Some(target) => *target,
                None => {
                    let Ok(module) = parse_source_module_file_cached(&canonical) else {
                        continue;
                    };
                    let via = if current == 0 {
                        Some(path.clone())
                    } else {
                        files[current].via.clone()
                    };
                    let target = files.len();
                    files.push(ScopeFile::new(
                        Some(canonical.clone()),
                        Some(module.clone()),
                        via,
                        module.statements(),
                    ));
                    directories.push(Interpreter::imported_source_dir(&file));
                    indices.insert(canonical, target);
                    target
                }
            };
            match alias {
                None => files[current].plain.push(target),
                Some(alias) => files[current].qualified.push((alias.clone(), target)),
            }
        }
    }
    files
}

/// The kind of top-level name an import scope resolves: functions and
/// bindings follow the same rule, each in its own namespace.
#[derive(Clone, Copy)]
enum Declared {
    Functions,
    Values,
}

impl Declared {
    fn names(self, file: &ScopeFile) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Declared::Functions => Box::new(file.functions.keys()),
            Declared::Values => Box::new(file.values.keys()),
        }
    }
}

/// For each file, the file declaring each name of `declared` it sees: its
/// own declaration, else the latest import's.
fn scopes(files: &[ScopeFile], declared: Declared) -> Vec<Rc<BTreeMap<String, usize>>> {
    /// The scope of `file` reached along the import path `active`, and
    /// whether an import back into that path was left out. Only a scope
    /// without such a cut is the file's own, to keep.
    fn scope(
        files: &[ScopeFile],
        declared: Declared,
        file: usize,
        done: &mut Vec<Option<Rc<BTreeMap<String, usize>>>>,
        active: &mut Vec<bool>,
    ) -> (Rc<BTreeMap<String, usize>>, bool) {
        if let Some(scope) = &done[file] {
            return (scope.clone(), false);
        }
        active[file] = true;
        let mut cut = false;
        let mut names = BTreeMap::new();
        for &import in &files[file].plain {
            if active[import] {
                // A cycle: the importing file on the path adds its names
                // itself.
                cut = true;
                continue;
            }
            let (imported, imported_cut) = scope(files, declared, import, done, active);
            cut |= imported_cut;
            for (name, declaring) in imported.iter() {
                names.insert(name.clone(), *declaring);
            }
        }
        for name in declared.names(&files[file]) {
            names.insert(name.clone(), file);
        }
        active[file] = false;
        let names = Rc::new(names);
        if !cut {
            done[file] = Some(names.clone());
        }
        (names, cut)
    }
    let mut done = vec![None; files.len()];
    let mut active = vec![false; files.len()];
    (0..files.len())
        .map(|file| scope(files, declared, file, &mut done, &mut active).0)
        .collect()
}

/// A file and the files it reaches through plain imports.
fn plain_closure(files: &[ScopeFile], file: usize) -> BTreeSet<usize> {
    let mut closure = BTreeSet::new();
    let mut pending = vec![file];
    while let Some(file) = pending.pop() {
        if closure.insert(file) {
            pending.extend(files[file].plain.iter().copied());
        }
    }
    closure
}

/// The declarations of `declared` that are not the one their merged program
/// means by their name: in the root's program every declaration but the one
/// the root sees; in a qualified import's program every declaration but one
/// when two remain.
fn renamed_declarations(
    files: &[ScopeFile],
    scopes: &[Rc<BTreeMap<String, usize>>],
    closures: &[BTreeSet<usize>],
    declared: Declared,
) -> BTreeSet<(usize, String)> {
    let root_sees = |file: usize, name: &str| scopes[0].get(name) == Some(&file);
    let mut renamed = BTreeSet::<(usize, String)>::new();
    for &file in &closures[0] {
        for name in declared.names(&files[file]) {
            if !root_sees(file, name) {
                renamed.insert((file, name.clone()));
            }
        }
    }
    let targets = files
        .iter()
        .flat_map(|file| file.qualified.iter().map(|(_, target)| *target))
        .collect::<BTreeSet<_>>();
    for target in targets {
        let mut declarations = BTreeMap::<&str, Vec<usize>>::new();
        for &file in &closures[target] {
            for name in declared.names(&files[file]) {
                if !renamed.contains(&(file, name.clone())) {
                    declarations.entry(name).or_default().push(file);
                }
            }
        }
        for (name, declaring) in declarations {
            if declaring.len() < 2 {
                continue;
            }
            let kept = declaring
                .iter()
                .copied()
                .find(|file| closures[0].contains(file) && root_sees(*file, name))
                .or_else(|| {
                    scopes[target]
                        .get(name)
                        .copied()
                        .filter(|file| declaring.contains(file))
                })
                .unwrap_or(declaring[0]);
            for file in declaring {
                if file != kept {
                    renamed.insert((file, name.to_string()));
                }
            }
        }
    }
    renamed
}

/// Internal names of the functions and of the bindings
/// `renamed_declarations` finds, unique among every name the files declare.
fn internal_names(
    files: &[ScopeFile],
    renamed: [BTreeSet<(usize, String)>; 2],
) -> [BTreeMap<(usize, String), String>; 2] {
    let mut taken = files
        .iter()
        .flat_map(|file| {
            file.functions
                .keys()
                .chain(file.rules.keys())
                .chain(file.values.keys())
                .cloned()
        })
        .collect::<BTreeSet<_>>();
    renamed.map(|renamed| {
        renamed
            .into_iter()
            .map(|(file, name)| {
                let stem = files[file]
                    .path
                    .as_deref()
                    .and_then(Path::file_stem)
                    .map(|stem| {
                        stem.to_string_lossy()
                            .chars()
                            .map(|c| {
                                if c.is_ascii_alphanumeric() {
                                    c.to_ascii_lowercase()
                                } else {
                                    '_'
                                }
                            })
                            .collect::<String>()
                    })
                    .filter(|stem| !stem.is_empty())
                    .unwrap_or_else(|| "module".to_string());
                let mut internal = format!("{name}__{stem}");
                let mut counter = 2;
                while taken.contains(&internal) {
                    internal = format!("{name}__{stem}_{counter}");
                    counter += 1;
                }
                taken.insert(internal.clone());
                ((file, name), internal)
            })
            .collect()
    })
}

/// Every rule family and function of one name that one file's scope reaches
/// from different files (one file's own are reported by its declaration
/// check).
fn rules_and_functions(
    files: &[ScopeFile],
    scopes: &[Rc<BTreeMap<String, usize>>],
    closures: &[BTreeSet<usize>],
) -> Vec<RuleAndFunction> {
    let mut reported = BTreeSet::new();
    let mut collisions = Vec::new();
    for (file, closure) in closures.iter().enumerate() {
        for (name, &declaring) in scopes[file].iter() {
            let clauses = closure
                .iter()
                .filter_map(|&rule_file| {
                    files[rule_file]
                        .rules
                        .get(name)
                        .map(|span| (rule_file, *span))
                })
                .collect::<Vec<_>>();
            let Some(&rule) = clauses.first() else {
                continue;
            };
            if clauses.iter().any(|(rule_file, _)| *rule_file == declaring)
                || !reported.insert((name.clone(), declaring, rule.0))
            {
                continue;
            }
            collisions.push(RuleAndFunction {
                name: name.clone(),
                rule,
                function: (declaring, files[declaring].functions[name]),
            });
        }
    }
    collisions
}

/// Every name in a file of the root's program that would reach a function
/// or binding of a file importing it: one the file neither declares nor
/// imports, a builtin or prelude function among them.
fn importer_functions(
    files: &[ScopeFile],
    function_scopes: &[Rc<BTreeMap<String, usize>>],
    value_scopes: &[Rc<BTreeMap<String, usize>>],
    closures: &[BTreeSet<usize>],
) -> Vec<ImporterFunction> {
    let mut found = Vec::new();
    for &file in &closures[0] {
        let Some(module) = &files[file].module else {
            continue;
        };
        let unseen = |name: &str| {
            !function_scopes[file].contains_key(name)
                && !value_scopes[file].contains_key(name)
                && !closures[file]
                    .iter()
                    .any(|inner| files[*inner].rules.contains_key(name))
        };
        let mut importers = BTreeMap::new();
        for (scope, binding) in [(&function_scopes[0], false), (&value_scopes[0], true)] {
            for (name, &declaring) in scope.iter() {
                if declaring != file && closures[declaring].contains(&file) && unseen(name) {
                    importers
                        .entry(name.as_str())
                        .or_insert((declaring, binding));
                }
            }
        }
        if !importers.keys().any(|name| module.source().contains(name)) {
            continue;
        }
        for (name, span) in Renamer::references(module.statements()) {
            if let Some(&(declaring, binding)) = importers.get(name.as_str()) {
                let declarations = if binding {
                    &files[declaring].values
                } else {
                    &files[declaring].functions
                };
                found.push(ImporterFunction {
                    function: (declaring, declarations[&name]),
                    name,
                    reference: (file, span),
                    binding,
                });
            }
        }
    }
    found
}

/// What one file's names mean: the renamed functions and bindings it sees,
/// by written name, and every top-level function and binding it sees.
struct FileNames<'a> {
    functions: &'a BTreeMap<String, String>,
    values: &'a BTreeMap<String, String>,
    members: &'a BTreeMap<(String, String), String>,
    visible_functions: BTreeSet<String>,
    visible_values: BTreeSet<String>,
}

/// Writes the names of one file that mean renamed functions and bindings
/// with their internal names, respecting the parameters and local bindings
/// that hide them.
struct Renamer<'a> {
    names: FileNames<'a>,
    /// When set, the first place of each name that can mean a function.
    references: Option<RefCell<BTreeMap<String, Span>>>,
}

impl<'a> Renamer<'a> {
    fn new(names: FileNames<'a>) -> Self {
        Self {
            names,
            references: None,
        }
    }

    /// The names in `statements` that can mean a function or a binding of
    /// another file, each at its first place: not hidden by a parameter,
    /// local binding or top-level binding of the file.
    fn references(statements: &[Stmt]) -> BTreeMap<String, Span> {
        let none = BTreeMap::new();
        let no_members = BTreeMap::new();
        let mut own_values = BTreeSet::new();
        for statement in statements {
            match statement {
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    collect_pattern_names(pattern, &mut own_values);
                }
                Stmt::StreamBind(name, _) => {
                    own_values.insert(name.clone());
                }
                _ => {}
            }
        }
        let mut renamer = Renamer::new(FileNames {
            functions: &none,
            values: &none,
            members: &no_members,
            visible_functions: BTreeSet::new(),
            visible_values: own_values,
        });
        renamer.references = Some(RefCell::new(BTreeMap::new()));
        renamer.file(&mut statements.to_vec());
        renamer
            .references
            .map(RefCell::into_inner)
            .unwrap_or_default()
    }

    fn file(&self, statements: &mut [Stmt]) {
        let bound = BTreeSet::new();
        for statement in statements {
            match statement {
                Stmt::Defn(Defn::Fn { name, .. }) => {
                    if let Some(internal) = self.names.functions.get(name.as_str()) {
                        *name = internal.clone();
                    }
                }
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    self.declared_pattern(pattern);
                }
                Stmt::StreamBind(name, _) => self.rename_value(name),
                _ => {}
            }
            self.statement(statement, &bound);
        }
    }

    /// The names a top-level binding declares.
    fn declared_pattern(&self, pattern: &mut Pat) {
        match pattern {
            Pat::Var(name) => self.rename_value(name),
            Pat::As(inner, name) => {
                self.declared_pattern(inner);
                self.rename_value(name);
            }
            Pat::Con(_, fields) => fields
                .iter_mut()
                .for_each(|field| self.declared_pattern(field)),
            Pat::NamedCon(_, fields) => fields
                .iter_mut()
                .for_each(|(_, field)| self.declared_pattern(field)),
            Pat::Tuple(elements) => elements
                .iter_mut()
                .for_each(|element| self.declared_pattern(element)),
            Pat::Wild | Pat::Lit(_) => {}
        }
    }

    fn rename_value(&self, name: &mut String) {
        if let Some(internal) = self.names.values.get(name.as_str()) {
            *name = internal.clone();
        }
    }

    fn rename(&self, name: &mut String, span: Span) {
        if let Some(references) = &self.references {
            references.borrow_mut().entry(name.clone()).or_insert(span);
        }
        if let Some(internal) = self.names.functions.get(name.as_str()) {
            *name = internal.clone();
        }
    }

    fn statement(&self, statement: &mut Stmt, bound: &BTreeSet<String>) {
        match statement {
            Stmt::Defn(definition) => self.definition(definition, bound),
            Stmt::TypeDecl(declaration) => self.type_declaration(declaration, bound),
            Stmt::Rule(rule) => self.rule(rule, bound),
            Stmt::Annot(_, arguments) => {
                for argument in arguments {
                    self.expr(argument, bound);
                }
            }
            Stmt::Bind(_, _, value)
            | Stmt::MonadicBind(_, _, value)
            | Stmt::StreamBind(_, value) => self.expr(value, bound),
            Stmt::For(variable, items, body) => {
                self.expr(items, bound);
                let mut inner = bound.clone();
                inner.insert(variable.clone());
                self.block(body, inner);
            }
            Stmt::While(condition, body) => {
                self.expr(condition, bound);
                self.block(body, bound.clone());
            }
            Stmt::Send(target, message) => {
                self.expr(target, bound);
                self.expr(message, bound);
            }
            Stmt::StreamSub(stream, arms) => {
                self.expr(stream, bound);
                self.arms(arms, bound);
            }
            Stmt::Invariant {
                subject, predicate, ..
            } => {
                self.expr(subject, bound);
                self.expr(predicate, bound);
            }
            Stmt::Prove {
                capture,
                pass_block,
                else_block,
                ..
            } => {
                let mut inner = bound.clone();
                inner.extend(capture.iter().cloned());
                for block in [pass_block, else_block].into_iter().flatten() {
                    self.block(block, inner.clone());
                }
            }
            Stmt::Expr(expr) => self.expr(expr, bound),
            Stmt::PreludeBoundary
            | Stmt::Use(_)
            | Stmt::Import(_)
            | Stmt::QualifiedImport(_, _)
            | Stmt::HashImport(_, _)
            | Stmt::Depend(_, _)
            | Stmt::RustBlock(_)
            | Stmt::Explore(_) => {}
        }
    }

    /// Statements of a body: a local function is visible in the whole body,
    /// a local binding from the statement after it.
    fn block(&self, statements: &mut [Stmt], mut bound: BTreeSet<String>) {
        declared_names(statements, &mut bound, false);
        for statement in statements {
            self.statement(statement, &bound);
            match statement {
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    collect_pattern_names(pattern, &mut bound);
                }
                Stmt::StreamBind(name, _) => {
                    bound.insert(name.clone());
                }
                _ => {}
            }
        }
    }

    fn function(&self, params: &[Param], body: &mut Expr, bound: &BTreeSet<String>) {
        let mut inner = bound.clone();
        inner.extend(params.iter().map(|param| param.name.clone()));
        inner.insert("self".to_string());
        self.expr(body, &inner);
    }

    fn definition(&self, definition: &mut Defn, bound: &BTreeSet<String>) {
        match definition {
            Defn::Fn { params, body, .. } => {
                let mut inner = bound.clone();
                inner.extend(params.iter().map(|param| param.name.clone()));
                self.expr(body, &inner);
            }
            Defn::Actor {
                state_param,
                handlers,
                ..
            } => {
                for handler in handlers {
                    let mut inner = bound.clone();
                    inner.insert(state_param.name.clone());
                    collect_pattern_names(&handler.msg_pat, &mut inner);
                    self.expr(&mut handler.body, &inner);
                }
            }
            Defn::Module { body, .. } => {
                let mut inner = bound.clone();
                declared_names(body, &mut inner, true);
                for statement in body {
                    self.statement(statement, &inner);
                }
            }
        }
    }

    fn type_declaration(&self, declaration: &mut TypeDecl, bound: &BTreeSet<String>) {
        match declaration {
            TypeDecl::ADT {
                methods,
                field_meta,
                ..
            } => {
                for method in methods {
                    if let Defn::Fn { params, body, .. } = method {
                        self.function(params, body, bound);
                    }
                }
                if let Some(field_meta) = field_meta {
                    let TypeFieldMeta {
                        type_default,
                        members,
                    } = field_meta.as_mut();
                    for declaration in type_default
                        .iter_mut()
                        .chain(members.iter_mut().map(|member| &mut member.meta))
                    {
                        self.field_meta(declaration, bound);
                    }
                }
            }
            TypeDecl::ImplBlock { methods, .. } => {
                for method in methods {
                    if let Defn::Fn { params, body, .. } = method {
                        self.function(params, body, bound);
                    }
                }
            }
            TypeDecl::TraitDecl { methods, .. } => {
                for method in methods {
                    if let Some(body) = &mut method.default_body {
                        self.function(&method.params, body, bound);
                    }
                }
            }
            TypeDecl::WhenType { condition, .. } => self.expr(condition, bound),
            TypeDecl::RuleScope { params, body, .. } => {
                let mut inner = bound.clone();
                inner.extend(params.iter().map(|param| param.name.clone()));
                declared_names(body, &mut inner, true);
                for statement in body {
                    self.statement(statement, &inner);
                }
            }
            // A check names a function or rule of the declaring file's scope;
            // `text` keeps the check as written for the contract.
            TypeDecl::Scalar {
                checks, field_meta, ..
            } => {
                for check in checks {
                    self.rename(&mut check.name, check.span);
                    for argument in &mut check.arguments {
                        self.expr(argument, bound);
                    }
                }
                if let Some(declaration) = field_meta {
                    self.field_meta(declaration, bound);
                }
            }
            TypeDecl::EffectDecl { .. } | TypeDecl::Unit { .. } => {}
        }
    }

    /// The values of an `@ field(...)` declaration: they read the declaring
    /// file's bindings like its other expressions.
    fn field_meta(&self, declaration: &mut FieldMetaDecl, bound: &BTreeSet<String>) {
        for (_, value) in &mut declaration.entries {
            self.expr(value, bound);
        }
    }

    fn rule(&self, rule: &mut Rule, bound: &BTreeSet<String>) {
        match rule {
            Rule::Clause { head, body } => {
                let inner = head_variables(head, bound);
                if let Some(body) = body {
                    self.expr(body, &inner);
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
                let inner = head_variables(head, bound);
                self.expr(value, &inner);
                if let Some(condition) = condition {
                    self.expr(condition, &inner);
                }
            }
            Rule::ReactiveScope { body, .. } => {
                for statement in body {
                    self.statement(statement, bound);
                }
            }
        }
    }

    fn arms(&self, arms: &mut [MatchArm], bound: &BTreeSet<String>) {
        for arm in arms {
            let mut inner = bound.clone();
            collect_pattern_names(&arm.pat, &mut inner);
            if let Some(guard) = &mut arm.guard {
                self.expr(guard, &inner);
            }
            self.expr(&mut arm.body, &inner);
        }
    }

    /// The function of a call: a name not hidden by a local binding means the
    /// function even where a top-level binding has its name, and the binding
    /// where the file sees no function of the name.
    fn callee(&self, function: &mut Expr, bound: &BTreeSet<String>) {
        match &mut function.kind {
            ExprKind::Var(name) if !bound.contains(name.as_str()) => {
                if !self.names.visible_functions.contains(name.as_str())
                    && self.names.visible_values.contains(name.as_str())
                {
                    self.rename_value(name);
                } else {
                    self.rename(name, function.span);
                }
            }
            ExprKind::Var(_) => {}
            _ => self.expr(function, bound),
        }
    }

    fn expr(&self, expr: &mut Expr, bound: &BTreeSet<String>) {
        let span = expr.span;
        match &mut expr.kind {
            ExprKind::Var(name) if !bound.contains(name.as_str()) => {
                if self.names.visible_values.contains(name.as_str()) {
                    self.rename_value(name);
                } else {
                    self.rename(name, span);
                }
            }
            ExprKind::Var(_) => {}
            ExprKind::App(function, arguments) => {
                self.callee(function, bound);
                for argument in arguments {
                    self.expr(argument, bound);
                }
            }
            ExprKind::Lambda(params, body) => {
                let mut inner = bound.clone();
                inner.extend(params.iter().map(|param| param.name.clone()));
                self.expr(body, &inner);
            }
            ExprKind::BinOp(_, left, right) | ExprKind::Index(left, right) => {
                self.expr(left, bound);
                self.expr(right, bound);
            }
            ExprKind::UnOp(_, operand) | ExprKind::Try(operand) => self.expr(operand, bound),
            ExprKind::If(condition, then_branch, else_branch) => {
                self.expr(condition, bound);
                self.expr(then_branch, bound);
                self.expr(else_branch, bound);
            }
            ExprKind::Match(scrutinee, arms) => {
                self.expr(scrutinee, bound);
                self.arms(arms, bound);
            }
            ExprKind::Block(statements) => self.block(statements, bound.clone()),
            ExprKind::Field(object, member) => {
                if let ExprKind::Var(alias) = &object.kind {
                    if !bound.contains(alias.as_str()) {
                        if let Some(internal) =
                            self.names.members.get(&(alias.clone(), member.clone()))
                        {
                            *member = internal.clone();
                        }
                    }
                }
                self.expr(object, bound);
            }
            ExprKind::List(items)
            | ExprKind::Tuple(items)
            | ExprKind::Conjunction(items)
            | ExprKind::Disjunction(items)
            | ExprKind::Effect(_, items) => {
                for item in items {
                    self.expr(item, bound);
                }
            }
            ExprKind::Handle { handlers, body, .. } => {
                for handler in handlers {
                    let mut inner = bound.clone();
                    inner.extend(handler.params.iter().cloned());
                    self.expr(&mut handler.body, &inner);
                }
                self.expr(body, bound);
            }
            ExprKind::Pipe(input, transform) => {
                self.expr(input, bound);
                if matches!(transform.kind, ExprKind::Var(_)) {
                    self.callee(transform, bound);
                } else {
                    self.expr(transform, bound);
                }
            }
            ExprKind::Lit(_) | ExprKind::Unit => {}
        }
    }
}

/// The names `statements` declare for the statements around them: local
/// functions, actors and modules, and with `everything` also their rules and
/// bindings (the body of a module or a rule scope).
fn declared_names(statements: &[Stmt], names: &mut BTreeSet<String>, everything: bool) {
    for statement in statements {
        match statement {
            Stmt::Defn(
                Defn::Fn { name, .. } | Defn::Actor { name, .. } | Defn::Module { name, .. },
            ) => {
                names.insert(name.clone());
            }
            Stmt::Rule(rule) if everything => {
                if let Some((name, _)) = rule.callable_name_arity() {
                    names.insert(name);
                }
            }
            Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) if everything => {
                collect_pattern_names(pattern, names);
            }
            _ => {}
        }
    }
}

/// `bound` with every name in a rule head's arguments: the head's variables.
fn head_variables(head: &Expr, bound: &BTreeSet<String>) -> BTreeSet<String> {
    fn collect(expr: &Expr, names: &mut BTreeSet<String>) {
        let mut free = BTreeSet::new();
        collect_true_free_vars(expr, &mut free, &BTreeSet::new());
        names.extend(free);
    }
    let mut names = bound.clone();
    match &head.kind {
        ExprKind::App(_, arguments) => {
            for argument in arguments {
                collect(argument, &mut names);
            }
        }
        _ => collect(head, &mut names),
    }
    names
}

/// `diagnostics` with each function an import scope renamed under its
/// written name and file, as the current elaboration records them.
pub(crate) fn display_written_names(diagnostics: &mut [Diagnostic]) {
    fn written(text: &mut String) {
        if let Cow::Owned(shown) = function_display_text(text) {
            *text = shown;
        }
    }
    for diagnostic in diagnostics {
        written(&mut diagnostic.message);
        diagnostic.notes.iter_mut().for_each(written);
        diagnostic.context.iter_mut().for_each(written);
    }
}

impl TypeChecker {
    /// Report each rule family and function of one name that one file's
    /// scope reaches from different files, and each name in an imported
    /// file that only a file importing it declares a function for.
    pub(super) fn report_import_scope_errors(&mut self, scopes: &ImportScopes) {
        for collision in &scopes.collisions {
            let anchor = if collision.function.0 == 0 || collision.rule.0 == 0 {
                0
            } else {
                collision.function.0
            };
            let message = format!(
                "`{}` is declared both as a rule ({}) and as a function ({}); a call cannot show which one it means, so rename one of them",
                collision.name,
                self.import_scope_place(scopes, collision.rule, anchor),
                self.import_scope_place(scopes, collision.function, anchor)
            );
            let span = if collision.function.0 == anchor {
                collision.function.1
            } else {
                collision.rule.1
            };
            self.import_scope_error(scopes, anchor, span, message);
        }
        for found in &scopes.importer_functions {
            let anchor = found.reference.0;
            let message = if found.binding {
                format!(
                    "`{}` ({}) names no binding of this file or of the files it imports, but {} binds `{}` and imports this file; a file's names never reach a file that imports it, so bind or import `{}` here, or rename that binding",
                    found.name,
                    self.import_scope_place(scopes, found.reference, anchor),
                    self.import_scope_place(scopes, found.function, anchor),
                    found.name,
                    found.name
                )
            } else {
                format!(
                    "`{}` ({}) names no function of this file or of the files it imports, but {} declares a function `{}` and imports this file; a file's calls never reach a file that imports it, so declare or import `{}` here, or rename that function",
                    found.name,
                    self.import_scope_place(scopes, found.reference, anchor),
                    self.import_scope_place(scopes, found.function, anchor),
                    found.name,
                    found.name
                )
            };
            self.import_scope_error(scopes, anchor, found.reference.1, message);
        }
    }

    /// An error at `span` of the root or of an imported file.
    fn import_scope_error(
        &mut self,
        scopes: &ImportScopes,
        file: usize,
        span: Span,
        message: String,
    ) {
        let file = &scopes.files[file];
        match (&file.path, &file.module) {
            (Some(path), Some(module)) => {
                let start = self.diagnostics.len();
                self.error_at_span(span, message);
                let import = file
                    .via
                    .as_deref()
                    .and_then(|via| self.import_path_span(via));
                self.locate_import_diagnostics(start, path, Arc::from(module.source()), import);
            }
            _ => self.error_at_span(span, message),
        }
    }

    /// `line N` of a declaration in the file a diagnostic is reported in,
    /// else `line N of path`.
    fn import_scope_place(
        &self,
        scopes: &ImportScopes,
        place: (usize, Span),
        anchor: usize,
    ) -> String {
        let (file, span) = place;
        let source = match &scopes.files[file].module {
            Some(module) => module.source(),
            None => self.source_text.as_str(),
        };
        let line = 1 + source
            .chars()
            .take(span.start)
            .filter(|character| *character == '\n')
            .count();
        if file == anchor {
            return format!("line {line}");
        }
        let Some(path) = scopes.files[file].path.as_deref() else {
            return format!("line {line} of the file being checked");
        };
        format!(
            "line {line} of {}",
            file_label(path, self.source_dir.as_deref())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renamed_bindings_keep_a_module_identity_its_written_content() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_import_scope_bindings_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create import directory");
        let lib = "= rate = 2\n> doubled() -> Int { rate * 2 }\n";
        std::fs::write(directory.join("lib.runa"), lib).expect("write lib.runa");
        let root_source = "@ import ./lib\n= rate = 3\n@ print(show(doubled()))\n";
        let root = Parser::new(Lexer::new(root_source).tokenize(), root_source)
            .parse_program()
            .expect("parse root");
        let scopes = ImportScopes::resolve(&root, Some(&directory.to_string_lossy()));
        let path = canonical_source_path(&directory.join("lib.runa"));
        let module = scopes.modules[&path].clone();
        std::fs::remove_dir_all(&directory).ok();

        // The root binds `rate` too: the module's own gets an internal name,
        // which its function reads, and the reader sees the written name.
        let bound = |statement: &Stmt| match statement {
            Stmt::Bind(Pat::Var(name), _, _) => name.clone(),
            other => panic!("expected a binding, got {other:?}"),
        };
        assert_eq!(bound(&module.statements()[0]), "rate__lib");
        let Stmt::Defn(Defn::Fn { body, .. }) = &module.statements()[1] else {
            panic!("expected `doubled`");
        };
        let mut read = BTreeSet::new();
        collect_true_free_vars(body, &mut read, &BTreeSet::new());
        assert!(read.contains("rate__lib"), "{read:?}");
        assert_eq!(
            scopes.functions.get("rate__lib"),
            Some(&ScopedFunction {
                name: "rate".to_string(),
                file: "lib.runa".to_string(),
            })
        );

        // Its identity stays what the file writes: the authored statements,
        // the content hash and the hash-import selection.
        assert_eq!(bound(&module.authored_statements()[0]), "rate");
        assert_eq!(module.content_hash(), parsed_source_content_hash(lib));
        let Stmt::Defn(authored_doubled) = &module.authored_statements()[1] else {
            panic!("expected `doubled`");
        };
        let selected = module.declarations_with_content_hash(&content_hash_defn(authored_doubled));
        assert_eq!(selected.len(), 1);
        assert!(std::ptr::eq(selected[0], &module.statements()[1]));
    }
}
