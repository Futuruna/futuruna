//! Danish source files name builtins, builtin types and the `Option`/`Result`
//! constructors in Danish. The names are resolved once per file, after the
//! file has been parsed:
//!
//! - a lowercase Danish builtin name refers to the builtin unless a binding in
//!   scope, a top-level declaration of the file, or a top-level declaration of
//!   a plainly imported module has that name;
//! - a Danish type or constructor name refers to the builtin unless the file
//!   or a plainly imported module declares a type or constructor of that name.
//!
//! User identifiers are never rewritten, and English files never see Danish
//! names.

use super::*;
use std::sync::LazyLock;

static DANISH_FUNCTION_NAMES: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| DANISH_BUILTIN_NAMES.iter().copied().collect());

static DANISH_TYPE_NAME_TABLE: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| DANISH_TYPE_NAMES.iter().copied().collect());

/// Translate the tokens that name Danish builtins: identifier tokens at the
/// source offsets in `references`, and type tokens whose Danish name `globals`
/// does not declare. A name after `.` is a field or member name and is never
/// translated. Returns `None` when no token names a builtin.
pub(crate) fn translate_danish_builtin_names(
    tokens: &[Token],
    offset: impl Fn(&Token) -> usize,
    references: &BTreeSet<usize>,
    globals: &BTreeSet<String>,
) -> Option<Vec<Token>> {
    let mut translated: Option<Vec<Token>> = None;
    for (index, token) in tokens.iter().enumerate() {
        if token.text != token.source_text
            || (index > 0 && tokens[index - 1].kind == TokenKind::Dot)
        {
            continue;
        }
        let english = match token.kind {
            TokenKind::Ident if references.contains(&offset(token)) => {
                DANISH_FUNCTION_NAMES.get(token.text.as_str())
            }
            TokenKind::Type if !globals.contains(&token.text) => {
                DANISH_TYPE_NAME_TABLE.get(token.text.as_str())
            }
            _ => None,
        };
        if let Some(english) = english {
            translated.get_or_insert_with(|| tokens.to_vec())[index].text = english.to_string();
        }
    }
    translated
}

/// The source offsets of the references to Danish builtin names in `stmts`
/// that no binding in scope and no name in `globals` claims.
pub(crate) fn danish_builtin_references(
    stmts: &[Stmt],
    globals: &BTreeSet<String>,
) -> BTreeSet<usize> {
    let mut resolver = ReferenceResolver {
        globals,
        bound: Vec::new(),
        references: BTreeSet::new(),
    };
    resolver.block(stmts);
    resolver.references
}

struct ReferenceResolver<'a> {
    globals: &'a BTreeSet<String>,
    bound: Vec<String>,
    references: BTreeSet<usize>,
}

impl ReferenceResolver<'_> {
    fn reference(&mut self, name: &str, span: Span) {
        if span.end > span.start
            && DANISH_FUNCTION_NAMES.contains_key(name)
            && !self.globals.contains(name)
            && !self.bound.iter().any(|bound| bound == name)
        {
            self.references.insert(span.start);
        }
    }

    fn scoped(&mut self, names: impl IntoIterator<Item = String>, visit: impl FnOnce(&mut Self)) {
        let mark = self.bound.len();
        self.bound.extend(names);
        visit(self);
        self.bound.truncate(mark);
    }

    fn block(&mut self, stmts: &[Stmt]) {
        let mut names = BTreeSet::new();
        collect_top_level_names(stmts, &mut names);
        self.scoped(names, |resolver| {
            for stmt in stmts {
                resolver.stmt(stmt);
            }
        });
    }

    fn defn(&mut self, defn: &Defn, scope: &[String]) {
        match defn {
            Defn::Fn { params, body, .. } => self.scoped(
                scope.iter().cloned().chain(param_names(params)),
                |resolver| resolver.expr(body),
            ),
            Defn::Actor {
                state_param,
                handlers,
                ..
            } => {
                for handler in handlers {
                    let mut names = pattern_names(&handler.msg_pat);
                    names.insert(state_param.name.clone());
                    self.scoped(names, |resolver| resolver.expr(&handler.body));
                }
            }
            Defn::Module { body, .. } => self.block(body),
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Defn(defn) => self.defn(defn, &[]),
            Stmt::TypeDecl(TypeDecl::ADT {
                variants, methods, ..
            }) => {
                let fields: Vec<String> = variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.name.clone()))
                    .collect();
                for method in methods {
                    self.defn(method, &fields);
                }
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                for method in methods {
                    self.defn(method, &[]);
                }
            }
            Stmt::TypeDecl(TypeDecl::TraitDecl { methods, .. }) => {
                for method in methods {
                    if let Some(body) = &method.default_body {
                        self.scoped(param_names(&method.params), |resolver| resolver.expr(body));
                    }
                }
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { params, body, .. }) => {
                self.scoped(param_names(params), |resolver| resolver.block(body));
            }
            Stmt::Rule(rule) => self.rule(rule),
            Stmt::For(name, iterable, body) => {
                self.expr(iterable);
                self.scoped([name.clone()], |resolver| resolver.block(body));
            }
            Stmt::StreamSub(stream, arms) => {
                self.expr(stream);
                self.arms(arms);
            }
            Stmt::Prove {
                capture,
                pass_block,
                else_block,
                ..
            } => self.scoped(capture.clone(), |resolver| {
                for block in [pass_block, else_block].into_iter().flatten() {
                    resolver.block(block);
                }
            }),
            Stmt::Explore(query) => {
                let names = query
                    .source
                    .bindings
                    .iter()
                    .map(|binding| binding.name.clone())
                    .chain(query.finds.iter().map(|find| find.name.clone()));
                self.scoped(names, |resolver| resolver.children(stmt));
            }
            _ => self.children(stmt),
        }
    }

    fn children(&mut self, stmt: &Stmt) {
        visit_ast_stmt_children(stmt, &mut |child| match child {
            AstChild::Expr(expr) => self.expr(expr),
            AstChild::Stmt(stmt) => self.stmt(stmt),
        });
    }

    fn rule(&mut self, rule: &Rule) {
        let (head, parts): (&Expr, Vec<&Expr>) = match rule {
            Rule::Clause { head, body } => (head, body.iter().collect()),
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
            } => (head, std::iter::once(value).chain(condition).collect()),
            Rule::ReactiveScope { body, .. } => return self.block(body),
        };
        let mut names = BTreeSet::new();
        if let ExprKind::App(_, arguments) = &head.kind {
            for argument in arguments {
                let argument = typed_rule_head_argument(argument)
                    .map(|(inner, _)| inner)
                    .unwrap_or(argument);
                walk_ast_expr(argument, &mut |node| {
                    if let AstChild::Expr(Expr {
                        kind: ExprKind::Var(name),
                        ..
                    }) = node
                    {
                        names.insert(name.clone());
                    }
                });
            }
        }
        self.scoped(names, |resolver| {
            for part in parts {
                resolver.expr(part);
            }
        });
    }

    fn arms(&mut self, arms: &[MatchArm]) {
        for arm in arms {
            self.scoped(pattern_names(&arm.pat), |resolver| {
                if let Some(guard) = &arm.guard {
                    resolver.expr(guard);
                }
                resolver.expr(&arm.body);
            });
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Var(name) => self.reference(name, expr.span),
            ExprKind::Effect(name, arguments) => {
                self.reference(name, expr.span);
                for argument in arguments {
                    self.expr(argument);
                }
            }
            ExprKind::Lambda(params, body) => {
                self.scoped(param_names(params), |resolver| resolver.expr(body));
            }
            ExprKind::Match(scrutinee, arms) => {
                self.expr(scrutinee);
                self.arms(arms);
            }
            ExprKind::Block(stmts) => self.block(stmts),
            ExprKind::Handle { handlers, body, .. } => {
                self.expr(body);
                for handler in handlers {
                    self.scoped(handler.params.iter().cloned(), |resolver| {
                        resolver.expr(&handler.body)
                    });
                }
            }
            _ => visit_ast_expr_children(expr, &mut |child| match child {
                AstChild::Expr(expr) => self.expr(expr),
                AstChild::Stmt(stmt) => self.stmt(stmt),
            }),
        }
    }
}

fn param_names(params: &[Param]) -> impl Iterator<Item = String> + '_ {
    params.iter().map(|param| param.name.clone())
}

fn pattern_names(pattern: &Pat) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_pattern_names(pattern, &mut names);
    names
}

/// The names a list of statements declares at its own level: functions,
/// rules, bindings, types, constructors, effect operations and qualified
/// imports. These are the names a module makes visible to its importers.
pub(crate) fn collect_top_level_names(stmts: &[Stmt], names: &mut BTreeSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Defn(Defn::Fn { name, .. })
            | Stmt::Defn(Defn::Actor { name, .. })
            | Stmt::Defn(Defn::Module { name, .. })
            | Stmt::StreamBind(name, _)
            | Stmt::QualifiedImport(name, _) => {
                names.insert(name.clone());
            }
            Stmt::TypeDecl(
                TypeDecl::ADT { name, variants, .. } | TypeDecl::WhenType { name, variants, .. },
            ) => {
                names.insert(name.clone());
                names.extend(variants.iter().map(|variant| variant.name.clone()));
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { name, ops }) => {
                names.insert(name.clone());
                names.extend(ops.iter().map(|(op, _, _)| op.clone()));
            }
            Stmt::TypeDecl(TypeDecl::TraitDecl { name, methods, .. }) => {
                names.insert(name.clone());
                names.extend(methods.iter().map(|method| method.name.clone()));
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { name, .. })
            | Stmt::Rule(Rule::ReactiveScope { name, .. }) => {
                names.insert(name.clone());
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { .. }) => {}
            Stmt::Rule(
                Rule::Clause { head, .. }
                | Rule::Default { head, .. }
                | Rule::Exception { head, .. },
            ) => {
                let function = match &head.kind {
                    ExprKind::App(function, _) => function.as_ref(),
                    _ => head,
                };
                if let ExprKind::Var(name) = &function.kind {
                    names.insert(name.clone());
                }
            }
            Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                collect_pattern_names(pattern, names);
            }
            _ => {}
        }
    }
}

struct ModuleDeclarations {
    content_hash: String,
    names: BTreeSet<String>,
    plain_imports: Vec<String>,
}

static MODULE_DECLARATIONS: LazyLock<Mutex<HashMap<PathBuf, Arc<ModuleDeclarations>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn plain_import_paths(stmts: &[Stmt]) -> Vec<String> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Import(path) => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn read_module_declarations(path: &Path) -> Option<Arc<ModuleDeclarations>> {
    let path = std::fs::canonicalize(path).ok()?;
    let source = std::fs::read_to_string(&path).ok()?;
    let content_hash = parsed_source_content_hash(&source);
    {
        let cache = MODULE_DECLARATIONS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(module) = cache.get(&path) {
            if module.content_hash == content_hash {
                return Some(module.clone());
            }
        }
    }
    let tokens = Lexer::new(&source).tokenize();
    let stmts = Parser::new(tokens, &source)
        .parse_program_without_danish_names()
        .ok()?;
    let mut names = BTreeSet::new();
    collect_top_level_names(&stmts, &mut names);
    let module = Arc::new(ModuleDeclarations {
        content_hash,
        names,
        plain_imports: plain_import_paths(&stmts),
    });
    MODULE_DECLARATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path, module.clone());
    Some(module)
}

/// Add the names declared by the plain imports of `stmts`, transitively.
/// Imports resolve relative to `dir`, as the import itself does.
pub(crate) fn collect_imported_names(stmts: &[Stmt], dir: &str, names: &mut BTreeSet<String>) {
    let mut pending: Vec<(String, String)> = plain_import_paths(stmts)
        .into_iter()
        .map(|import| (import, dir.to_string()))
        .collect();
    let mut visited = BTreeSet::new();
    while let Some((import, dir)) = pending.pop() {
        let Some(file) = Interpreter::resolve_import_path_for_source(&import, &dir) else {
            continue;
        };
        let file = PathBuf::from(file);
        if !visited.insert(std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone())) {
            continue;
        }
        let Some(module) = read_module_declarations(&file) else {
            continue;
        };
        names.extend(module.names.iter().cloned());
        let module_dir = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        pending.extend(
            module
                .plain_imports
                .iter()
                .map(|import| (import.clone(), module_dir.clone())),
        );
    }
}

static SOURCE_ORIGINS: LazyLock<Mutex<HashMap<String, PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Record the file a source text was read from, so that a parse of the same
/// text resolves its imports relative to that file.
pub fn register_source_origin(path: &Path, source: &str) {
    SOURCE_ORIGINS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(parsed_source_content_hash(source), path.to_path_buf());
}

pub(crate) fn registered_source_origin(source: &str) -> Option<PathBuf> {
    SOURCE_ORIGINS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&parsed_source_content_hash(source))
        .cloned()
}

/// Danish number text: an optional sign, an integer part written either as
/// plain digits or with `.` between groups of three digits (`1.250.000`), and
/// for decimal numbers an optional `,` followed by digits (`1.234,5`).
/// Surrounding whitespace is ignored. The parsers are defined once and emitted
/// verbatim into compiled programs.
macro_rules! danish_number_parsers {
    ($($item:item)*) => {
        $($item)*
        pub const DANISH_NUMBER_PARSERS_RUST: &str = stringify!($($item)*);
    };
}

danish_number_parsers! {
    fn __futuruna_danish_number(text: &str, decimal: bool) -> Result<String, String> {
        let kind = if decimal { "decimal number" } else { "integer" };
        let invalid = || format!("`{}` is not a Danish {}", text, kind);
        let trimmed = text.trim();
        let (sign, body) = match trimmed.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", trimmed.strip_prefix('+').unwrap_or(trimmed)),
        };
        let (whole, fraction) = match body.split_once(',') {
            Some((whole, fraction)) if decimal => (whole, Some(fraction)),
            Some(_) => return Err(invalid()),
            None => (body, None),
        };
        let groups: Vec<&str> = whole.split('.').collect();
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        let grouped = groups.len() == 1
            || (groups[0].len() <= 3 && groups[1..].iter().all(|group| group.len() == 3));
        if !groups.iter().all(|group| digits(group)) || !grouped || !fraction.map_or(true, digits) {
            return Err(invalid());
        }
        let mut canonical = format!("{}{}", sign, groups.concat());
        if let Some(fraction) = fraction {
            canonical.push('.');
            canonical.push_str(fraction);
        }
        Ok(canonical)
    }

    pub fn __futuruna_parse_danish_int(text: &str) -> Result<i64, String> {
        __futuruna_danish_number(text, false)?
            .parse::<i64>()
            .map_err(|_| format!("`{}` is outside the Int range", text))
    }

    pub fn __futuruna_parse_danish_float(text: &str) -> Result<f64, String> {
        __futuruna_danish_number(text, true)?
            .parse::<f64>()
            .map_err(|_| format!("`{}` is not a Danish decimal number", text))
    }
}

/// In a Danish file a diagnostic names builtins, types, constructors and
/// keywords the way the author wrote them: a backticked English name is shown
/// in Danish when its Danish spelling occurs on the diagnostic's source line.
pub(crate) fn localize_danish_message(source: &str, span: Option<Span>, message: &str) -> String {
    let Some(span) = span else {
        return message.to_string();
    };
    if !message.contains('`') || detect_source_language(source) != SourceLanguage::Danish {
        return message.to_string();
    }
    localize_on_line(source, span.start_line_col(source).0, message)
}

/// Localize parser errors of the form `LINE:COL: message`, including the hint
/// lines that follow each located error.
pub(crate) fn localize_danish_parse_errors(source: &str, errors: &str) -> String {
    let mut line = 0;
    errors
        .lines()
        .map(|text| {
            let mut parts = text.splitn(3, ':');
            if let (Some(row), Some(column), Some(_)) = (parts.next(), parts.next(), parts.next()) {
                if let (Ok(row), Ok(_)) =
                    (row.trim().parse::<usize>(), column.trim().parse::<usize>())
                {
                    line = row;
                }
            }
            if line == 0 || !text.contains('`') {
                text.to_string()
            } else {
                localize_on_line(source, line, text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn localize_on_line(source: &str, line: usize, message: &str) -> String {
    let Some(line_text) = source.lines().nth(line.saturating_sub(1)) else {
        return message.to_string();
    };
    let written: BTreeSet<&str> = line_text
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    let keywords = keyword_table_dansk();
    let danish_spelling = |english: &str| {
        DANISH_BUILTIN_NAMES
            .iter()
            .chain(DANISH_TYPE_NAMES)
            .filter(|(_, canonical)| *canonical == english)
            .map(|(danish, _)| *danish)
            .chain(
                keywords
                    .iter()
                    .filter(|(danish, (canonical, _))| canonical == english && *danish != english)
                    .map(|(danish, _)| danish.as_str()),
            )
            .find(|danish| written.contains(danish))
    };
    let mut localized = String::with_capacity(message.len());
    for (index, part) in message.split('`').enumerate() {
        if index > 0 {
            localized.push('`');
        }
        match (index % 2 == 1).then(|| danish_spelling(part)).flatten() {
            Some(danish) => localized.push_str(danish),
            None => localized.push_str(part),
        }
    }
    localized
}
