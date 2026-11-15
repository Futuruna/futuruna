//! A name in a file means what that file's scope says: the file's own
//! declaration, or else the one its plain imports provide (a later import's
//! over an earlier one's), never a declaration of a file that imports it.
//!
//! Plain imports still merge every file into one program, and the checker,
//! the interpreter, the native backend and the solver keep one registry of
//! functions by name. So when two functions of one name meet in a merged
//! program, each one except the declaration the root file sees gets an
//! internal name, and every name that means it is written with that name: in
//! the file that declares it and in each file that reaches it through its
//! imports. `Lib.fee` of a qualified import is rewritten the same way. The
//! rewritten files belong to the checked program and are read through its
//! elaboration (see `constructor_scope`), like its constructor names.
//!
//! A qualified import is a merged program of its own: two functions of one
//! name in it are told apart in the same way.
//!
//! A name in an imported file that no function of the file or its imports
//! declares, while a file importing it declares one, would reach that
//! importer's function in the merged program; it is an error in the imported
//! file.
//!
//! A rule family spans plain imports: an importer's clause joins the
//! imported family. So a rule and a function of one name in the scope of one
//! file are an error naming both lines, as in a single file.

use super::*;
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
    /// Top-level bindings.
    values: BTreeSet<String>,
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
        let mut values = BTreeSet::new();
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
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    collect_pattern_names(pattern, &mut values);
                }
                Stmt::StreamBind(name, _) => {
                    values.insert(name.clone());
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

/// A name in an imported file that no function of the file or its imports
/// declares, while a file importing it declares a function of that name: the
/// merged program would call the importer's function there.
struct ImporterFunction {
    name: String,
    reference: (usize, Span),
    function: (usize, Span),
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
        let scopes = scopes(&files);
        let closures = (0..files.len())
            .map(|file| plain_closure(&files, file))
            .collect::<Vec<_>>();
        let internal = internal_names(&files, &scopes, &closures);
        let collisions = rules_and_functions(&files, &scopes, &closures);
        let importer_functions = importer_functions(&files, &scopes, &closures);

        let mut root_statements = None;
        let mut modules = BTreeMap::new();
        for (index, file) in files.iter().enumerate() {
            let names = scopes[index]
                .iter()
                .filter_map(|(name, declaring)| {
                    internal
                        .get(&(*declaring, name.clone()))
                        .map(|internal| (name.clone(), internal.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            let mut members = BTreeMap::new();
            for (alias, target) in &file.qualified {
                for (name, declaring) in scopes[*target].iter() {
                    if let Some(internal) = internal.get(&(*declaring, name.clone())) {
                        members.insert((alias.clone(), name.clone()), internal.clone());
                    }
                }
            }
            if names.is_empty() && members.is_empty() {
                continue;
            }
            match &file.module {
                None => {
                    let mut statements = root.to_vec();
                    Renamer::new(&names, &members, &statements[program_start..])
                        .file(&mut statements[program_start..]);
                    root_statements = Some(statements);
                }
                Some(module) => {
                    let mut statements = module.statements().to_vec();
                    Renamer::new(&names, &members, &statements).file(&mut statements);
                    let path = file.path.clone().expect("an imported file has a path");
                    modules.insert(
                        path,
                        Arc::new(ParsedSourceModule::elaborated(module, statements)),
                    );
                }
            }
        }
        Self {
            root: root_statements,
            modules,
            files,
            collisions,
            importer_functions,
        }
    }

    /// The root statements with their qualified member accesses rewritten,
    /// when there are any.
    pub(crate) fn take_root(&mut self) -> Option<Vec<Stmt>> {
        self.root.take()
    }

    /// Read the rewritten files on this thread from now on.
    pub(crate) fn install(&self) {
        ConstructorElaboration::install_import_scopes(&self.modules);
    }
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

/// For each file, the file declaring each function name it sees: its own
/// declaration, else the latest import's.
fn scopes(files: &[ScopeFile]) -> Vec<Rc<BTreeMap<String, usize>>> {
    /// The scope of `file` reached along the import path `active`, and
    /// whether an import back into that path was left out. Only a scope
    /// without such a cut is the file's own, to keep.
    fn scope(
        files: &[ScopeFile],
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
            let (imported, imported_cut) = scope(files, import, done, active);
            cut |= imported_cut;
            for (name, declaring) in imported.iter() {
                names.insert(name.clone(), *declaring);
            }
        }
        for name in files[file].functions.keys() {
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
        .map(|file| scope(files, file, &mut done, &mut active).0)
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

/// Internal names of the functions that are not the one their merged
/// program means by their name: in the root's program every declaration but
/// the one the root sees; in a qualified import's program every declaration
/// but one when two remain.
fn internal_names(
    files: &[ScopeFile],
    scopes: &[Rc<BTreeMap<String, usize>>],
    closures: &[BTreeSet<usize>],
) -> BTreeMap<(usize, String), String> {
    let root_sees = |file: usize, name: &str| scopes[0].get(name) == Some(&file);
    let mut renamed = BTreeSet::<(usize, String)>::new();
    for &file in &closures[0] {
        for name in files[file].functions.keys() {
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
            for name in files[file].functions.keys() {
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

    let mut taken = files
        .iter()
        .flat_map(|file| file.functions.keys().chain(file.rules.keys()).cloned())
        .collect::<BTreeSet<_>>();
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
/// of a file importing it: one the file neither declares nor imports, a
/// builtin or prelude function among them.
fn importer_functions(
    files: &[ScopeFile],
    scopes: &[Rc<BTreeMap<String, usize>>],
    closures: &[BTreeSet<usize>],
) -> Vec<ImporterFunction> {
    let mut found = Vec::new();
    for &file in &closures[0] {
        let Some(module) = &files[file].module else {
            continue;
        };
        let importers = scopes[0]
            .iter()
            .filter(|(name, &declaring)| {
                declaring != file
                    && closures[declaring].contains(&file)
                    && !scopes[file].contains_key(name.as_str())
                    && !closures[file].iter().any(|inner| {
                        files[*inner].rules.contains_key(name.as_str())
                            || files[*inner].values.contains(name.as_str())
                    })
            })
            .collect::<BTreeMap<_, _>>();
        if !importers
            .keys()
            .any(|name| module.source().contains(name.as_str()))
        {
            continue;
        }
        for (name, span) in Renamer::references(module.statements()) {
            if let Some(&&declaring) = importers.get(&name) {
                found.push(ImporterFunction {
                    function: (declaring, files[declaring].functions[&name]),
                    name,
                    reference: (file, span),
                });
            }
        }
    }
    found
}

/// Writes the names of one file that mean renamed functions with their
/// internal names, respecting the parameters and local bindings that hide
/// them.
struct Renamer<'a> {
    names: &'a BTreeMap<String, String>,
    members: &'a BTreeMap<(String, String), String>,
    /// The file's top-level values: a bare name of one means the value; a
    /// call of the name still means the function.
    values: BTreeSet<String>,
    /// When set, the first place of each name that can mean a function.
    references: Option<RefCell<BTreeMap<String, Span>>>,
}

impl<'a> Renamer<'a> {
    fn new(
        names: &'a BTreeMap<String, String>,
        members: &'a BTreeMap<(String, String), String>,
        statements: &[Stmt],
    ) -> Self {
        let mut values = BTreeSet::new();
        for statement in statements {
            match statement {
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    collect_pattern_names(pattern, &mut values);
                }
                Stmt::StreamBind(name, _) => {
                    values.insert(name.clone());
                }
                _ => {}
            }
        }
        Self {
            names,
            members,
            values,
            references: None,
        }
    }

    /// The names in `statements` that can mean a function, each at its first
    /// place: not hidden by a parameter, local binding or top-level value.
    fn references(statements: &[Stmt]) -> BTreeMap<String, Span> {
        let none = BTreeMap::new();
        let no_members = BTreeMap::new();
        let mut renamer = Renamer::new(&none, &no_members, statements);
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
            if let Stmt::Defn(Defn::Fn { name, .. }) = statement {
                if let Some(internal) = self.names.get(name.as_str()) {
                    *name = internal.clone();
                }
            }
            self.statement(statement, &bound);
        }
    }

    fn rename(&self, name: &mut String, span: Span) {
        if let Some(references) = &self.references {
            references.borrow_mut().entry(name.clone()).or_insert(span);
        }
        if let Some(internal) = self.names.get(name.as_str()) {
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
            TypeDecl::ADT { methods, .. } | TypeDecl::ImplBlock { methods, .. } => {
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
            TypeDecl::EffectDecl { .. } | TypeDecl::Unit { .. } => {}
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
    /// function even where a top-level value has its name.
    fn callee(&self, function: &mut Expr, bound: &BTreeSet<String>) {
        match &mut function.kind {
            ExprKind::Var(name) if !bound.contains(name.as_str()) => {
                self.rename(name, function.span)
            }
            ExprKind::Var(_) => {}
            _ => self.expr(function, bound),
        }
    }

    fn expr(&self, expr: &mut Expr, bound: &BTreeSet<String>) {
        let span = expr.span;
        match &mut expr.kind {
            ExprKind::Var(name) => {
                if !bound.contains(name.as_str()) && !self.values.contains(name.as_str()) {
                    self.rename(name, span);
                }
            }
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
                        if let Some(internal) = self.members.get(&(alias.clone(), member.clone())) {
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
            let message = format!(
                "`{}` ({}) names no function of this file or of the files it imports, but {} declares a function `{}` and imports this file; a file's calls never reach a file that imports it, so declare or import `{}` here, or rename that function",
                found.name,
                self.import_scope_place(scopes, found.reference, anchor),
                self.import_scope_place(scopes, found.function, anchor),
                found.name,
                found.name
            );
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
        let root = self
            .source_dir
            .as_deref()
            .map(|dir| canonical_source_path(Path::new(dir)));
        let shown = root
            .as_deref()
            .and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(path);
        format!("line {line} of {}", shown.display())
    }
}
