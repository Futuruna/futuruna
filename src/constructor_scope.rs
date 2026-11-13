//! Constructor names scoped to their type.
//!
//! Two sum types in one scope may declare the same constructor name. A bare
//! name resolves by the type its context expects: a pattern by the type of
//! the matched value, an argument, field or result by its declared type, and
//! one side of `==` by the other side. `Type.Constructor` names the owner
//! explicitly. A shared name with no expected type is an error; an owner is
//! never chosen by arity or by field names.
//!
//! After checking, [`ConstructorElaboration`] gives every shared constructor
//! except the first declared one an internal name, so the interpreter, the
//! native backend and the solver keep their name-keyed registries. Each place
//! where a constructor name leaves the program (display, `show`, collection
//! order, calculation JSON, solver models) maps it back with
//! [`constructor_display_name`].

use super::*;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::RwLock;

/// The spelling of `Owner.Constructor` in a pattern.
pub fn qualified_constructor_name(owner: &str, constructor: &str) -> String {
    format!("{owner}.{constructor}")
}

/// Split a `Owner.Constructor` pattern name.
pub fn split_qualified_constructor(name: &str) -> Option<(&str, &str)> {
    let (owner, constructor) = name.split_once('.')?;
    let upper = |part: &str| part.chars().next().is_some_and(char::is_uppercase);
    (upper(owner) && upper(constructor)).then_some((owner, constructor))
}

/// The constructor a pattern name refers to, without its owner.
pub fn bare_constructor_name(name: &str) -> &str {
    split_qualified_constructor(name).map_or(name, |(_, constructor)| constructor)
}

#[derive(Debug, Default)]
struct ConstructorNameRegistry {
    /// Internal name -> constructor name as written.
    display: BTreeMap<String, String>,
    /// (owner type, constructor name as written) -> internal name.
    internal: BTreeMap<(String, String), String>,
    /// Elaborated imported modules, by canonical path.
    modules: BTreeMap<PathBuf, Arc<ParsedSourceModule>>,
}

static REGISTRY_ACTIVE: AtomicBool = AtomicBool::new(false);
static REGISTRY: std::sync::LazyLock<RwLock<ConstructorNameRegistry>> =
    std::sync::LazyLock::new(|| RwLock::new(ConstructorNameRegistry::default()));

fn registry() -> std::sync::RwLockReadGuard<'static, ConstructorNameRegistry> {
    REGISTRY
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The name a constructor has in source and in data: the internal name of a
/// shared constructor maps back to the name its declaration wrote.
pub fn constructor_display_name(name: &str) -> Cow<'_, str> {
    if !REGISTRY_ACTIVE.load(AtomicOrdering::Acquire) {
        return Cow::Borrowed(name);
    }
    match registry().display.get(name) {
        Some(display) => Cow::Owned(display.clone()),
        None => Cow::Borrowed(name),
    }
}

/// The runtime name of `owner`'s constructor `constructor`.
pub fn constructor_runtime_name<'a>(owner: &str, constructor: &'a str) -> Cow<'a, str> {
    if !REGISTRY_ACTIVE.load(AtomicOrdering::Acquire) {
        return Cow::Borrowed(constructor);
    }
    match registry()
        .internal
        .get(&(owner.to_string(), constructor.to_string()))
    {
        Some(internal) => Cow::Owned(internal.clone()),
        None => Cow::Borrowed(constructor),
    }
}

/// The elaborated syntax of an imported module, when the installed
/// elaboration renamed constructors in it.
pub(crate) fn elaborated_source_module(
    path: &Path,
    content_hash: &str,
) -> Option<Arc<ParsedSourceModule>> {
    if !REGISTRY_ACTIVE.load(AtomicOrdering::Acquire) {
        return None;
    }
    registry()
        .modules
        .get(path)
        .filter(|module| module.content_hash() == content_hash)
        .cloned()
}

/// Forget an installed elaboration. Checking a new root program starts here,
/// so its imports are read as written.
pub fn clear_constructor_elaboration() {
    if !REGISTRY_ACTIVE.swap(false, AtomicOrdering::AcqRel) {
        return;
    }
    let mut registry = REGISTRY
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *registry = ConstructorNameRegistry::default();
}

/// The checked program with every shared constructor given its internal name
/// and every `Type.Constructor` written as the constructor alone.
#[derive(Debug, Clone, Default)]
pub struct ConstructorElaboration {
    root: Option<Vec<Stmt>>,
    modules: BTreeMap<PathBuf, Arc<ParsedSourceModule>>,
    /// (owner type, constructor name as written, internal name).
    names: Vec<(String, String, String)>,
}

impl ConstructorElaboration {
    /// The root statements to run, when elaboration changed them.
    pub fn root_statements(&self) -> Option<&[Stmt]> {
        self.root.as_deref()
    }

    /// Whether any constructor received an internal name.
    pub fn renames_constructors(&self) -> bool {
        !self.names.is_empty()
    }

    /// Make the elaborated imports and the internal names visible to the
    /// interpreter, the backends and every data edge of this process.
    pub fn install(&self) {
        let mut registry = REGISTRY
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *registry = ConstructorNameRegistry::default();
        for (owner, constructor, internal) in &self.names {
            registry
                .display
                .insert(internal.clone(), constructor.clone());
            registry
                .internal
                .insert((owner.clone(), constructor.clone()), internal.clone());
        }
        registry.modules = self.modules.clone();
        let active = !registry.display.is_empty() || !registry.modules.is_empty();
        drop(registry);
        REGISTRY_ACTIVE.store(active, AtomicOrdering::Release);
    }
}

/// Owners of one constructor name and the internal name each owner after the
/// first receives.
#[derive(Debug, Default)]
struct SharedConstructors {
    /// Constructor name -> owner types in declaration order (two or more).
    owners: BTreeMap<String, Vec<String>>,
    /// (owner, constructor) -> internal name; absent for the first owner.
    internal: BTreeMap<(String, String), String>,
}

impl SharedConstructors {
    fn runtime_name(&self, owner: &str, constructor: &str) -> String {
        self.internal
            .get(&(owner.to_string(), constructor.to_string()))
            .cloned()
            .unwrap_or_else(|| constructor.to_string())
    }
}

fn owner_list(owners: &[String]) -> String {
    let quoted = owners
        .iter()
        .map(|owner| format!("`{owner}`"))
        .collect::<Vec<_>>();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {}", init.join(", "), last),
    }
}

fn qualified_choices(owners: &[String], constructor: &str) -> String {
    let quoted = owners
        .iter()
        .map(|owner| format!("`{owner}.{constructor}`"))
        .collect::<Vec<_>>();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} or {}", init.join(", "), last),
    }
}

/// Why a constructor has no expected type, for the ambiguity diagnostic.
#[derive(Debug, Clone)]
enum Unexpected {
    /// Nothing in the context gives the value a type.
    Context,
    /// An unannotated binding of this name.
    Binding(String),
    /// One side of `==` or `!=` whose other side has no known type either.
    Equality(String),
    /// An argument of a callable that accepts any type.
    Generic(String),
    /// An element of a list literal with no element type.
    List,
    /// An argument of a rule whose clauses type the parameter differently.
    RuleFamily {
        rule: String,
        types: Vec<String>,
    },
}

#[derive(Debug, Clone)]
enum Expect {
    Type(Ty),
    Unknown(Unexpected),
}

impl Expect {
    fn none() -> Self {
        Expect::Unknown(Unexpected::Context)
    }

    fn ty(&self) -> Option<&Ty> {
        match self {
            Expect::Type(ty) => Some(ty),
            Expect::Unknown(_) => None,
        }
    }
}

/// One constructor application whose fields are checked against the owner
/// its context chose.
struct ConstructorCallCheck {
    expression: Expr,
    label: String,
    signature: TypeConstructorSignature,
    owner_type: String,
    locals: BTreeMap<String, String>,
}

/// Declared parameter names and types of each function, by name and arity.
type FunctionSignatures = BTreeMap<(String, usize), Vec<Vec<(String, Option<Ty>)>>>;
/// Declared parameter types of each rule family, by name and arity: the
/// distinct types its clauses give each position.
type RuleSignatures = BTreeMap<(String, usize), Vec<Vec<Ty>>>;

struct ConstructorScopeWalk<'a> {
    checker: &'a TypeChecker,
    shared: &'a SharedConstructors,
    functions: &'a FunctionSignatures,
    rule_parameters: &'a RuleSignatures,
    rule_scope: Option<String>,
    /// Source range of the match arm pattern being resolved.
    pattern_span: Span,
    diagnostics: Vec<(Span, String)>,
    calls: Vec<ConstructorCallCheck>,
    rewrites: usize,
}

type Locals = BTreeMap<String, String>;

impl TypeChecker {
    /// The owner whose constructor identity `owner`'s constructor carries:
    /// a subset type `# T = A.X` shares `A`'s constructor `X`.
    pub(crate) fn constructor_identity_owner(&self, owner: &str, constructor: &str) -> String {
        let mut owner = owner.to_string();
        let mut seen = BTreeSet::new();
        while let Some(source) = self
            .constructor_subset_sources
            .get(&(owner.clone(), constructor.to_string()))
        {
            if !seen.insert(owner.clone()) {
                break;
            }
            owner = source.clone();
        }
        owner
    }

    /// The distinct types that own a constructor name, in declaration order.
    pub(crate) fn constructor_owners(&self, constructor: &str) -> Vec<String> {
        let mut owners = Vec::new();
        for signature in self
            .constructor_signatures
            .get(constructor)
            .into_iter()
            .flatten()
        {
            let owner = self.constructor_identity_owner(&signature.parent, constructor);
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        owners
    }

    /// Whether more than one type in scope declares this constructor name.
    pub(crate) fn constructor_is_shared(&self, constructor: &str) -> bool {
        let constructor = bare_constructor_name(constructor);
        match self.constructor_signatures.get(constructor) {
            Some(signatures) if signatures.len() > 1 => {
                self.constructor_owners(constructor).len() > 1
            }
            _ => false,
        }
    }

    /// The one owner of an unshared constructor name.
    pub(crate) fn unique_constructor_parent(&self, constructor: &str) -> Option<String> {
        let signatures = self.constructor_signatures.get(constructor)?;
        if self.constructor_is_shared(constructor) {
            return None;
        }
        signatures.last().map(|signature| signature.parent.clone())
    }

    /// `Type.Constructor` written as an expression: `Field(Var(Type), C)`.
    /// The type must declare the constructor and must not also be a value or
    /// a qualified import in scope.
    pub(crate) fn type_qualified_constructor(
        &self,
        base: &Expr,
        constructor: &str,
        locals: Option<&Locals>,
    ) -> Option<TypeConstructorSignature> {
        let ExprKind::Var(owner) = &base.kind else {
            return None;
        };
        if !owner.chars().next().is_some_and(char::is_uppercase)
            || locals.is_some_and(|locals| locals.contains_key(owner))
            || self.module_exports.contains_key(owner)
            || !self.types.contains(owner)
        {
            return None;
        }
        let signature = self
            .constructor_signatures
            .get(constructor)?
            .iter()
            .find(|signature| &signature.parent == owner)?
            .clone();
        self.constructor_scope_used.set(true);
        Some(signature)
    }

    /// `X.C` where `X` is both a qualified import and a type declaring `C`.
    pub(crate) fn ambiguous_type_qualifier(&self, base: &Expr, constructor: &str) -> Option<String> {
        let ExprKind::Var(owner) = &base.kind else {
            return None;
        };
        let declares = self
            .constructor_signatures
            .get(constructor)
            .is_some_and(|signatures| signatures.iter().any(|signature| &signature.parent == owner));
        (self.module_exports.contains_key(owner) && self.types.contains(owner) && declares).then(
            || {
                format!(
                    "`{owner}` names both a qualified import and a type, so `{owner}.{constructor}` is ambiguous; rename the import"
                )
            },
        )
    }

    /// The constructor signature a pattern name refers to, given the type of
    /// the matched value. `Type.Constructor` names its owner.
    pub(crate) fn qualified_pattern_signature(
        &self,
        name: &str,
    ) -> Option<TypeConstructorSignature> {
        let (owner, constructor) = split_qualified_constructor(name)?;
        let signature = self
            .constructor_signatures
            .get(constructor)?
            .iter()
            .find(|signature| signature.parent == owner)?
            .clone();
        self.constructor_scope_used.set(true);
        Some(signature)
    }

    fn shared_constructor_plan(&self) -> SharedConstructors {
        let mut plan = SharedConstructors::default();
        let mut taken = self
            .constructor_signatures
            .keys()
            .cloned()
            .chain(self.types.iter().cloned())
            .collect::<BTreeSet<_>>();
        for constructor in self.constructor_signatures.keys() {
            let owners = self.constructor_owners(constructor);
            if owners.len() < 2 {
                continue;
            }
            for owner in &owners[1..] {
                let mut internal = format!("{owner}__{constructor}");
                while !taken.insert(internal.clone()) {
                    internal.push('_');
                }
                plan.internal
                    .insert((owner.clone(), constructor.clone()), internal);
            }
            plan.owners.insert(constructor.clone(), owners);
        }
        plan
    }

    fn constructor_scope_signatures(
        statements: &[Stmt],
        functions: &mut FunctionSignatures,
        rules: &mut RuleSignatures,
    ) {
        for statement in statements {
            match statement {
                Stmt::Defn(Defn::Fn { name, params, .. }) => {
                    functions
                        .entry((name.clone(), params.len()))
                        .or_default()
                        .push(
                            params
                                .iter()
                                .map(|param| (param.name.clone(), param.ty.clone()))
                                .collect(),
                        );
                }
                Stmt::Rule(
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. },
                ) => {
                    let (Some(name), ExprKind::App(_, arguments)) =
                        (rule_head_name(head), &head.kind)
                    else {
                        continue;
                    };
                    let entry = rules
                        .entry((name.to_string(), arguments.len()))
                        .or_insert_with(|| vec![Vec::new(); arguments.len()]);
                    for (index, argument) in arguments.iter().enumerate() {
                        if let Some(ty) = typed_rule_head_argument(argument)
                            .and_then(|(_, ty)| parse_type_annotation(ty).ok())
                        {
                            if !entry[index].contains(&ty) {
                                entry[index].push(ty);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Resolve every shared constructor and every `Type.Constructor` in the
    /// root program and its plain imports. Reports a shared name that no
    /// context types and returns the elaborated program.
    pub(crate) fn elaborate_constructor_scope(
        &mut self,
        root: &[Stmt],
    ) -> Option<Arc<ConstructorElaboration>> {
        let shared = self.shared_constructor_plan();
        if shared.owners.is_empty() && !self.constructor_scope_used.get() {
            return None;
        }

        // Every module of the merged scope, root first, then plain and hash
        // imports in the order the checker reached them.
        let root_dir = self.source_dir.clone().unwrap_or_else(|| ".".to_string());
        let mut modules = Vec::<(PathBuf, Arc<ParsedSourceModule>, Option<Span>)>::new();
        let mut seen = BTreeSet::<String>::new();
        let mut pending = root
            .iter()
            .filter_map(|statement| match statement {
                Stmt::Import(path) | Stmt::HashImport(_, path) => Some((
                    path.clone(),
                    root_dir.clone(),
                    self.import_path_span(path),
                )),
                _ => None,
            })
            .collect::<VecDeque<_>>();
        while let Some((path, dir, anchor)) = pending.pop_front() {
            let Some((canonical, imported_dir)) = Self::resolve_prepass_import(&path, &dir) else {
                continue;
            };
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let Ok(module) = parse_source_module_file_cached(Path::new(&canonical)) else {
                continue;
            };
            for statement in module.statements() {
                if let Stmt::Import(path) | Stmt::HashImport(_, path) = statement {
                    pending.push_back((path.clone(), imported_dir.clone(), anchor));
                }
            }
            modules.push((PathBuf::from(canonical), module, anchor));
        }

        let mut functions = BTreeMap::new();
        let mut rules = BTreeMap::new();
        Self::constructor_scope_signatures(root, &mut functions, &mut rules);
        for (_, module, _) in &modules {
            Self::constructor_scope_signatures(module.statements(), &mut functions, &mut rules);
        }
        for parameters in rules.values_mut() {
            for types in parameters.iter_mut() {
                let mut distinct = Vec::<Ty>::new();
                for ty in types.drain(..) {
                    if !distinct.contains(&ty) {
                        distinct.push(ty);
                    }
                }
                *types = distinct;
            }
        }

        let mut elaboration = ConstructorElaboration::default();
        for ((owner, constructor), internal) in &shared.internal {
            elaboration
                .names
                .push((owner.clone(), constructor.clone(), internal.clone()));
        }

        let (root_statements, rewrites) =
            self.elaborate_constructor_statements(root, &shared, &functions, &rules);
        if rewrites > 0 {
            elaboration.root = Some(root_statements);
        }
        for (path, module, anchor) in modules {
            let previous_source =
                std::mem::replace(&mut self.source_text, module.source().to_string());
            let start = self.diagnostics.len();
            let (statements, rewrites) =
                self.elaborate_constructor_statements(module.statements(), &shared, &functions, &rules);
            self.locate_import_diagnostics(start, &path, Arc::from(module.source()), anchor);
            self.source_text = previous_source;
            if rewrites > 0 {
                elaboration.modules.insert(
                    path,
                    Arc::new(ParsedSourceModule::elaborated(&module, statements)),
                );
            }
        }
        Some(Arc::new(elaboration))
    }

    fn elaborate_constructor_statements(
        &mut self,
        statements: &[Stmt],
        shared: &SharedConstructors,
        functions: &FunctionSignatures,
        rules: &RuleSignatures,
    ) -> (Vec<Stmt>, usize) {
        let mut statements = statements.to_vec();
        let (diagnostics, calls, rewrites) = {
            let mut walk = ConstructorScopeWalk {
                checker: self,
                shared,
                functions,
                rule_parameters: rules,
                rule_scope: None,
                pattern_span: Span::dummy(),
                diagnostics: Vec::new(),
                calls: Vec::new(),
                rewrites: 0,
            };
            let mut locals = Locals::new();
            walk.statements(&mut statements, &mut locals, None);
            (walk.diagnostics, walk.calls, walk.rewrites)
        };
        for (span, message) in diagnostics {
            self.error_at_span(span, message);
        }
        for call in calls {
            self.check_scoped_constructor_call(call);
        }
        (statements, rewrites)
    }

    /// The field checks the ordinary constructor check makes, against the
    /// owner the context chose for a shared constructor.
    fn check_scoped_constructor_call(&mut self, call: ConstructorCallCheck) {
        let errors = self.scoped_constructor_call_errors(&call);
        for (span, message) in errors {
            self.error_at_span(span, message);
        }
    }

    fn scoped_constructor_call_errors(&self, call: &ConstructorCallCheck) -> Vec<(Span, String)> {
        let ConstructorCallCheck {
            expression,
            label,
            signature,
            owner_type,
            locals,
        } = call;
        let arguments = match &expression.kind {
            ExprKind::App(_, arguments) => arguments.as_slice(),
            _ => &[],
        };
        let mut errors = Vec::new();
        let mut check_value = |index: usize, value: &Expr, field: &str, errors: &mut Vec<_>| {
            let Some(expected) = signature
                .field_tys
                .get(index)
                .and_then(|ty| ty.as_deref())
                .and_then(|ty| parse_type_annotation(ty).ok())
                .and_then(|field| {
                    parse_type_annotation(
                        &self.canonical_instantiate_nominal_type(owner_type, &field),
                    )
                    .ok()
                })
            else {
                return;
            };
            let Some(actual) = self.infer_expr_type_name_with_locals_in_scope(value, locals, None)
            else {
                return;
            };
            let Ok(actual_ty) = parse_type_annotation(&actual) else {
                return;
            };
            if !Self::ordinary_argument_matches(&actual_ty, &expected, &mut BTreeMap::new()) {
                errors.push((
                    value.span,
                    format!(
                        "constructor `{label}` field `{field}` expects `{}`, got `{actual}`",
                        Self::canonical_explore_ty_name(&expected)
                    ),
                ));
            }
        };
        if has_named_args(arguments) {
            if !all_named_args(arguments) {
                errors.push((
                    expression.span,
                    format!("constructor `{label}` call mixes named and positional arguments; use either all named fields or all positional fields"),
                ));
                return errors;
            }
            if signature.positional {
                errors.push((
                    expression.span,
                    format!("constructor `{label}` does not have named fields; use positional arguments"),
                ));
                return errors;
            }
            let mut seen = BTreeSet::new();
            for argument in arguments {
                let Some((field, value)) = named_arg_parts(argument) else {
                    continue;
                };
                match signature.fields.iter().position(|known| known == field) {
                    Some(index) => check_value(index, value, field, &mut errors),
                    None => errors.push((
                        argument.span,
                        format!("constructor `{label}` has no field `{field}`"),
                    )),
                }
                if !seen.insert(field.to_string()) {
                    errors.push((
                        argument.span,
                        format!("constructor `{label}` field `{field}` was provided more than once"),
                    ));
                }
            }
            for field in &signature.fields {
                if !seen.contains(field) {
                    errors.push((
                        expression.span,
                        format!("constructor `{label}` is missing named field `{field}`"),
                    ));
                }
            }
            return errors;
        }
        if arguments.len() != signature.arity() {
            let expected = signature.arity();
            errors.push((
                expression.span,
                format!(
                    "constructor `{label}` expects {expected} field{} but got {}",
                    if expected == 1 { "" } else { "s" },
                    arguments.len()
                ),
            ));
            return errors;
        }
        for (index, argument) in arguments.iter().enumerate() {
            check_value(index, argument, &signature.fields[index], &mut errors);
        }
        errors
    }
}

impl ParsedSourceModule {
    fn elaborated(original: &ParsedSourceModule, statements: Vec<Stmt>) -> Self {
        Self {
            source: original.source.clone(),
            statements: Arc::new(statements),
            content_hash: original.content_hash.clone(),
        }
    }
}

impl ConstructorScopeWalk<'_> {
    fn error(&mut self, span: Span, message: String) {
        self.diagnostics.push((span, message));
    }

    fn infer(&self, expr: &Expr, locals: &Locals) -> Option<Ty> {
        let name = self.checker.infer_expr_type_name_with_locals_in_scope(
            expr,
            locals,
            self.rule_scope.as_deref(),
        )?;
        let ty = parse_type_annotation(&name).ok()?;
        (!Self::is_unknown(&ty)).then_some(ty)
    }

    fn is_unknown(ty: &Ty) -> bool {
        matches!(ty, Ty::Hole | Ty::Var(_))
            || matches!(ty, Ty::Name(name) if name == "_" || name == CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE)
    }

    fn normalize(ty: &Ty) -> Ty {
        parse_type_annotation(&TypeChecker::canonical_explore_ty_name(ty)).unwrap_or_else(|_| ty.clone())
    }

    fn type_string(ty: &Ty) -> String {
        TypeChecker::canonical_explore_ty_name(ty)
    }

    /// `List(T)` -> `T`.
    fn element_type(ty: &Ty) -> Option<Ty> {
        match Self::normalize(ty) {
            Ty::App(head, mut arguments)
                if matches!(head.as_ref(), Ty::Name(name) if name == "List" || name == "Set")
                    && arguments.len() == 1 =>
            {
                let element = arguments.remove(0);
                (!Self::is_unknown(&element)).then_some(element)
            }
            _ => None,
        }
    }

    fn is_local(name: &str, locals: &Locals) -> bool {
        locals.contains_key(name)
    }

    /// A constructor named by `Var(name)` here, when it is not a local value.
    fn constructor_name<'n>(&self, name: &'n str, locals: &Locals) -> Option<&'n str> {
        (!Self::is_local(name, locals)
            && name.chars().next().is_some_and(char::is_uppercase)
            && self.checker.constructor_signatures.contains_key(name))
        .then_some(name)
    }

    /// The owner the expected type gives a constructor, and its signature.
    fn owner_for(
        &self,
        constructor: &str,
        expected: &Ty,
    ) -> Option<(String, TypeConstructorSignature)> {
        let parent = TypeChecker::canonical_nominal_owner(&Self::type_string(expected))?;
        let signature = self
            .checker
            .constructor_signatures
            .get(constructor)?
            .iter()
            .find(|signature| signature.parent == parent)?
            .clone();
        Some((parent, signature))
    }

    fn ambiguity_message(&self, constructor: &str, owners: &[String], why: &Unexpected) -> String {
        let base = format!(
            "constructor `{constructor}` belongs to {}",
            owner_list(owners)
        );
        let choices = qualified_choices(owners, constructor);
        match why {
            Unexpected::Context => {
                format!("{base}; nothing here gives the value a type, so write {choices}")
            }
            Unexpected::Binding(name) => {
                format!("{base}; write {choices}, or give `{name}` a type")
            }
            Unexpected::Equality(op) => format!(
                "{base}; neither side of `{op}` has a known type, so write {choices}"
            ),
            Unexpected::Generic(callee) => {
                format!("{base}; `{callee}` accepts a value of any type, so write {choices}")
            }
            Unexpected::List => {
                format!("{base}; this list has no element type, so write {choices}")
            }
            Unexpected::RuleFamily { rule, types } => format!(
                "{base}; the clauses of rule `{rule}` take {} here, so write {choices}",
                owner_list(types)
            ),
        }
    }

    /// Resolve a constructor occurrence to its owner and runtime name.
    /// Returns the signature the arguments are checked against.
    fn resolve(
        &mut self,
        constructor: &str,
        qualified_owner: Option<&str>,
        expect: &Expect,
        span: Span,
    ) -> Option<(String, TypeConstructorSignature)> {
        if let Some(owner) = qualified_owner {
            let signature = self
                .checker
                .constructor_signatures
                .get(constructor)?
                .iter()
                .find(|signature| signature.parent == owner)?
                .clone();
            return Some((owner.to_string(), signature));
        }
        let Some(owners) = self.shared.owners.get(constructor) else {
            let signature = self
                .checker
                .constructor_signatures
                .get(constructor)?
                .last()?
                .clone();
            return Some((signature.parent.clone(), signature));
        };
        let owners = owners.clone();
        match expect {
            Expect::Type(expected) => {
                if let Some(found) = self.owner_for(constructor, expected) {
                    return Some(found);
                }
                match TypeChecker::canonical_nominal_owner(&Self::type_string(expected)) {
                    Some(parent) if self.checker.types.contains(&parent) => {
                        self.error(
                            span,
                            format!(
                                "constructor `{constructor}` belongs to {}, not to `{parent}`",
                                owner_list(&owners)
                            ),
                        );
                    }
                    _ => {
                        let message =
                            self.ambiguity_message(constructor, &owners, &Unexpected::Context);
                        self.error(span, message);
                    }
                }
                None
            }
            Expect::Unknown(why) => {
                let message = self.ambiguity_message(constructor, &owners, why);
                self.error(span, message);
                None
            }
        }
    }

    fn rename(&mut self, owner: &str, constructor: &str) -> String {
        let identity = self.checker.constructor_identity_owner(owner, constructor);
        self.shared.runtime_name(&identity, constructor)
    }

    /// Field types of a resolved constructor, instantiated for the owner
    /// type the context expects.
    fn field_types(
        &self,
        signature: &TypeConstructorSignature,
        owner: &str,
        expect: &Expect,
    ) -> Vec<Option<Ty>> {
        let subject = match expect.ty() {
            Some(expected)
                if TypeChecker::canonical_nominal_owner(&Self::type_string(expected)).as_deref()
                    == Some(owner) =>
            {
                Self::type_string(expected)
            }
            _ => owner.to_string(),
        };
        signature
            .field_tys
            .iter()
            .map(|field| {
                let field = parse_type_annotation(field.as_deref()?).ok()?;
                let ty = parse_type_annotation(
                    &self.checker.canonical_instantiate_nominal_type(&subject, &field),
                )
                .ok()?;
                (!Self::is_unknown(&ty)).then_some(ty)
            })
            .collect()
    }

    fn statements(&mut self, statements: &mut [Stmt], locals: &mut Locals, result: Option<&Expect>) {
        let last = statements.len().saturating_sub(1);
        for (index, statement) in statements.iter_mut().enumerate() {
            let expect = (index == last).then_some(result).flatten();
            self.statement(statement, locals, expect);
        }
    }

    fn bind_pattern(&mut self, pattern: &mut Pat, ty: Option<&Ty>, locals: &mut Locals) {
        let mut names = BTreeSet::new();
        collect_pattern_names(pattern, &mut names);
        for name in names {
            locals.insert(name, CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
        }
        let ty_name = ty.map(Self::type_string);
        locals.extend(
            self.checker
                .pattern_type_bindings(pattern, ty_name.as_deref()),
        );
        self.pattern(pattern, ty);
    }

    fn statement(&mut self, statement: &mut Stmt, locals: &mut Locals, result: Option<&Expect>) {
        match statement {
            Stmt::Expr(expr) => {
                let expect = result.cloned().unwrap_or_else(Expect::none);
                self.expr(expr, &expect, locals);
            }
            Stmt::Bind(pattern, annotation, value) => {
                let expect = match (annotation.as_ref(), &*pattern) {
                    (Some(ty), _) => Expect::Type(ty.clone()),
                    (None, Pat::Var(name)) => Expect::Unknown(Unexpected::Binding(name.clone())),
                    (None, _) => Expect::none(),
                };
                let ty = annotation.clone().or_else(|| self.infer(value, locals));
                self.expr(value, &expect, locals);
                self.bind_pattern(pattern, ty.as_ref(), locals);
            }
            Stmt::MonadicBind(pattern, annotation, value) => {
                let wrapped = self.infer(value, locals);
                self.expr(value, &Expect::none(), locals);
                let ty = annotation.clone().or_else(|| match wrapped.map(|ty| Self::normalize(&ty)) {
                    Some(Ty::App(head, arguments))
                        if matches!(head.as_ref(), Ty::Name(name) if name == "Option" || name == "Result") =>
                    {
                        arguments.into_iter().next()
                    }
                    _ => None,
                });
                self.bind_pattern(pattern, ty.as_ref(), locals);
            }
            Stmt::For(name, collection, body) => {
                let element = self
                    .infer(collection, locals)
                    .and_then(|ty| Self::element_type(&ty));
                self.expr(collection, &Expect::none(), locals);
                let mut inner = locals.clone();
                inner.insert(
                    name.clone(),
                    element
                        .map(|ty| Self::type_string(&ty))
                        .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                );
                self.statements(body, &mut inner, None);
            }
            Stmt::While(condition, body) => {
                self.expr(condition, &Expect::none(), locals);
                let mut inner = locals.clone();
                self.statements(body, &mut inner, None);
            }
            Stmt::Send(target, message) => {
                self.expr(target, &Expect::none(), locals);
                // An actor message names the actor's handler, not a data
                // constructor; only its arguments are values.
                if let ExprKind::App(_, arguments) = &mut message.kind {
                    for argument in arguments {
                        self.expr(argument, &Expect::none(), locals);
                    }
                }
            }
            Stmt::StreamBind(name, value) => {
                self.expr(value, &Expect::none(), locals);
                locals.insert(name.clone(), CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
            }
            Stmt::StreamSub(stream, arms) => {
                self.expr(stream, &Expect::none(), locals);
                for arm in arms {
                    self.arm(arm, None, &Expect::none(), locals);
                }
            }
            Stmt::Invariant {
                subject, predicate, ..
            } => {
                self.expr(subject, &Expect::none(), locals);
                self.expr(predicate, &Expect::none(), locals);
            }
            Stmt::Prove {
                pass_block,
                else_block,
                capture,
                ..
            } => {
                for block in [pass_block, else_block].into_iter().flatten() {
                    let mut inner = locals.clone();
                    if let Some(capture) = capture {
                        inner.insert(
                            capture.clone(),
                            CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string(),
                        );
                    }
                    self.statements(block, &mut inner, None);
                }
            }
            Stmt::Explore(query) => {
                // An exploration replays the program as written, so it cannot
                // tell shared constructors apart.
                if let Some((constructor, owners)) = self.shared.owners.iter().next() {
                    let message = format!(
                        "explorations do not support constructor names shared between types yet: `{constructor}` belongs to {}",
                        owner_list(owners)
                    );
                    self.error(query.span, message);
                }
            }
            Stmt::Annot(_, arguments) => {
                for argument in arguments {
                    self.expr(argument, &Expect::none(), locals);
                }
            }
            Stmt::Defn(definition) => self.definition(definition, locals, None),
            Stmt::TypeDecl(declaration) => self.type_declaration(declaration, locals),
            Stmt::Rule(rule) => self.rule(rule, locals),
            Stmt::PreludeBoundary
            | Stmt::Use(_)
            | Stmt::Import(_)
            | Stmt::QualifiedImport(_, _)
            | Stmt::HashImport(_, _)
            | Stmt::Depend(_, _)
            | Stmt::RustBlock(_) => {}
        }
    }

    fn function(
        &mut self,
        params: &[Param],
        ret_ty: Option<&Ty>,
        body: &mut Expr,
        locals: &Locals,
        receiver: Option<&str>,
    ) {
        let mut inner = locals.clone();
        for (index, param) in params.iter().enumerate() {
            let ty = param
                .ty
                .as_ref()
                .map(Self::type_string)
                .or_else(|| (index == 0).then_some(receiver).flatten().map(str::to_string))
                .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
            inner.insert(param.name.clone(), ty);
        }
        let expect = ret_ty
            .map(|ty| match receiver {
                Some(receiver) => TypeChecker::canonical_substitute_type_parameters(
                    ty,
                    &BTreeMap::from([("Self".to_string(), Ty::Name(receiver.to_string()))]),
                ),
                None => ty.clone(),
            })
            .map(Expect::Type)
            .unwrap_or_else(Expect::none);
        self.expr(body, &expect, &inner);
    }

    fn definition(&mut self, definition: &mut Defn, locals: &mut Locals, receiver: Option<&str>) {
        match definition {
            Defn::Fn {
                params,
                ret_ty,
                body,
                ..
            } => {
                let params = params.clone();
                let ret_ty = ret_ty.clone();
                self.function(&params, ret_ty.as_ref(), body, locals, receiver);
            }
            Defn::Actor {
                state_param,
                handlers,
                ..
            } => {
                for handler in handlers {
                    let mut inner = locals.clone();
                    inner.insert(
                        state_param.name.clone(),
                        state_param
                            .ty
                            .as_ref()
                            .map(Self::type_string)
                            .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                    );
                    let mut names = BTreeSet::new();
                    collect_pattern_names(&handler.msg_pat, &mut names);
                    for name in names {
                        inner.insert(name, CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
                    }
                    self.expr(&mut handler.body, &Expect::none(), &inner);
                }
            }
            Defn::Module { body, .. } => {
                let mut inner = locals.clone();
                self.statements(body, &mut inner, None);
            }
        }
    }

    fn type_declaration(&mut self, declaration: &mut TypeDecl, locals: &mut Locals) {
        match declaration {
            TypeDecl::ADT {
                name,
                variants,
                methods,
                except_from,
                ..
            } => {
                let owner = name.clone();
                for variant in variants.iter_mut() {
                    let source = variant.from_type.clone().unwrap_or_else(|| owner.clone());
                    let renamed = self.rename(&source, &variant.name);
                    if renamed != variant.name {
                        variant.name = renamed;
                        self.rewrites += 1;
                    }
                }
                if let Some((source, excluded)) = except_from {
                    for constructor in excluded.iter_mut() {
                        let renamed = self.rename(source, constructor);
                        if &renamed != constructor {
                            *constructor = renamed;
                            self.rewrites += 1;
                        }
                    }
                }
                for method in methods {
                    self.definition(method, locals, Some(&owner));
                }
            }
            TypeDecl::WhenType {
                name,
                condition,
                variants,
                ..
            } => {
                self.expr(condition, &Expect::none(), locals);
                let owner = name.clone();
                for variant in variants.iter_mut() {
                    let source = variant.from_type.clone().unwrap_or_else(|| owner.clone());
                    let renamed = self.rename(&source, &variant.name);
                    if renamed != variant.name {
                        variant.name = renamed;
                        self.rewrites += 1;
                    }
                }
            }
            TypeDecl::TraitDecl { methods, .. } => {
                for method in methods {
                    let params = method.params.clone();
                    let ret_ty = method.ret_ty.clone();
                    if let Some(body) = &mut method.default_body {
                        self.function(&params, ret_ty.as_ref(), body, locals, None);
                    }
                }
            }
            TypeDecl::ImplBlock {
                for_type, methods, ..
            } => {
                let receiver = for_type.clone();
                for method in methods {
                    self.definition(method, locals, Some(&receiver));
                }
            }
            TypeDecl::RuleScope { name, params, body } => {
                let previous = self.rule_scope.replace(name.clone());
                let mut inner = locals.clone();
                for param in params.iter() {
                    inner.insert(
                        param.name.clone(),
                        param
                            .ty
                            .as_ref()
                            .map(Self::type_string)
                            .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                    );
                }
                self.statements(body, &mut inner, None);
                self.rule_scope = previous;
            }
            TypeDecl::EffectDecl { .. } => {}
        }
    }

    fn rule(&mut self, rule: &mut Rule, locals: &mut Locals) {
        let head = match rule {
            Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. } => {
                head
            }
            Rule::ReactiveScope { body, .. } => {
                let mut inner = locals.clone();
                self.statements(body, &mut inner, None);
                return;
            }
        };
        let mut inner = locals.clone();
        let key = rule_head_name(head).map(str::to_string);
        if let (Some(name), ExprKind::App(_, arguments)) = (key.clone(), &mut head.kind) {
            let arity = arguments.len();
            let family = self.rule_parameters.get(&(name.clone(), arity)).cloned();
            for (index, argument) in arguments.iter_mut().enumerate() {
                let declared = family.as_ref().and_then(|family| family.get(index)).cloned();
                self.rule_head_argument(argument, &name, declared.unwrap_or_default(), &mut inner);
            }
        }
        let result = key.as_ref().and_then(|name| {
            let arity = match &head.kind {
                ExprKind::App(_, arguments) => arguments.len(),
                _ => 0,
            };
            let key = RuleDispatchKey {
                scope: self.rule_scope.clone(),
                name: name.clone(),
                arity,
            };
            self.checker
                .explore_rule_dispatch_result_type(&key)
                .and_then(|ty| parse_type_annotation(ty).ok())
                .filter(|ty| !Self::is_unknown(ty))
        });
        let expect = result.map(Expect::Type).unwrap_or_else(Expect::none);
        match rule {
            Rule::Clause { body, .. } => {
                if let Some(body) = body {
                    self.expr(body, &Expect::none(), &inner);
                }
            }
            Rule::Default {
                value, condition, ..
            }
            | Rule::Exception {
                value, condition, ..
            } => {
                if let Some(condition) = condition {
                    self.expr(condition, &Expect::none(), &inner);
                }
                self.expr(value, &expect, &inner);
            }
            Rule::ReactiveScope { .. } => {}
        }
    }

    /// A rule head argument is a pattern written as an expression: a binder,
    /// a literal, a constructor applied to sub-patterns, or `x: Type`.
    fn rule_head_argument(&mut self, argument: &mut Expr, rule: &str, declared: Vec<Ty>, locals: &mut Locals) {
        if let ExprKind::App(function, arguments) = &mut argument.kind {
            if matches!(&function.kind, ExprKind::Var(name) if name == "__typed") && arguments.len() == 2 {
                let ty = match &arguments[1].kind {
                    ExprKind::Var(ty) => parse_type_annotation(ty).ok(),
                    _ => None,
                };
                self.head_pattern(&mut arguments[0], ty.map(Expect::Type).unwrap_or_else(Expect::none), locals);
                return;
            }
        }
        let expect = match declared.as_slice() {
            [one] => Expect::Type(one.clone()),
            [] => Expect::none(),
            types => Expect::Unknown(Unexpected::RuleFamily {
                rule: rule.to_string(),
                types: types.iter().map(Self::type_string).collect(),
            }),
        };
        self.head_pattern(argument, expect, locals);
    }

    fn head_pattern(&mut self, pattern: &mut Expr, expect: Expect, locals: &mut Locals) {
        let span = pattern.span;
        match &mut pattern.kind {
            ExprKind::Var(name) if !name.chars().next().is_some_and(char::is_uppercase) => {
                locals.insert(
                    name.clone(),
                    expect
                        .ty()
                        .map(Self::type_string)
                        .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                );
            }
            ExprKind::Var(name) => {
                if self.checker.constructor_signatures.contains_key(name.as_str()) {
                    let constructor = name.clone();
                    if let Some((owner, _)) = self.resolve(&constructor, None, &expect, span) {
                        let renamed = self.rename(&owner, &constructor);
                        if renamed != *name {
                            *name = renamed;
                            self.rewrites += 1;
                        }
                    }
                }
            }
            ExprKind::Field(base, field) => {
                let field = field.clone();
                if let Some(signature) =
                    self.checker
                        .type_qualified_constructor(base, &field, Some(locals))
                {
                    let renamed = self.rename(&signature.parent, &field);
                    pattern.kind = ExprKind::Var(renamed);
                    self.rewrites += 1;
                }
            }
            ExprKind::App(function, arguments) => {
                let (constructor, qualified) = match &function.kind {
                    ExprKind::Var(name)
                        if self.checker.constructor_signatures.contains_key(name.as_str()) =>
                    {
                        (Some(name.clone()), None)
                    }
                    ExprKind::Field(base, field) => match self
                        .checker
                        .type_qualified_constructor(base, field, Some(locals))
                    {
                        Some(signature) => (Some(field.clone()), Some(signature.parent)),
                        None => (None, None),
                    },
                    _ => (None, None),
                };
                let Some(constructor) = constructor else {
                    return;
                };
                let resolved = self.resolve(&constructor, qualified.as_deref(), &expect, span);
                let fields = resolved
                    .as_ref()
                    .map(|(owner, signature)| self.field_types(signature, owner, &expect))
                    .unwrap_or_default();
                if let Some((owner, _)) = &resolved {
                    let renamed = self.rename(owner, &constructor);
                    if qualified.is_some() || renamed != constructor {
                        **function = Expr::new(ExprKind::Var(renamed), function.span);
                        self.rewrites += 1;
                    }
                }
                let signature = resolved.map(|(_, signature)| signature);
                for (index, argument) in arguments.iter_mut().enumerate() {
                    let (value, field_index) = match named_arg_parts(argument) {
                        Some((field, _)) => (
                            1usize,
                            signature
                                .as_ref()
                                .and_then(|signature| signature.fields.iter().position(|known| known == field)),
                        ),
                        None => (0, Some(index)),
                    };
                    let expect = field_index
                        .and_then(|index| fields.get(index).cloned().flatten())
                        .map(Expect::Type)
                        .unwrap_or_else(Expect::none);
                    if value == 1 {
                        if let ExprKind::App(_, parts) = &mut argument.kind {
                            self.head_pattern(&mut parts[1], expect, locals);
                        }
                    } else {
                        self.head_pattern(argument, expect, locals);
                    }
                }
            }
            _ => {}
        }
    }

    fn pattern(&mut self, pattern: &mut Pat, subject: Option<&Ty>) {
        match pattern {
            Pat::Con(name, arguments) => {
                let Some((owner, signature, field_types)) = self.pattern_constructor(name, subject) else {
                    for argument in arguments {
                        self.pattern(argument, None);
                    }
                    return;
                };
                let _ = (owner, signature);
                for (index, argument) in arguments.iter_mut().enumerate() {
                    let ty = field_types.get(index).cloned().flatten();
                    self.pattern(argument, ty.as_ref());
                }
            }
            Pat::NamedCon(name, fields) => {
                let resolved = self.pattern_constructor(name, subject);
                for (field, argument) in fields.iter_mut() {
                    let ty = resolved.as_ref().and_then(|(_, signature, field_types)| {
                        let index = signature.fields.iter().position(|known| known == field)?;
                        field_types.get(index).cloned().flatten()
                    });
                    self.pattern(argument, ty.as_ref());
                }
            }
            Pat::As(inner, _) => self.pattern(inner, subject),
            Pat::Wild | Pat::Var(_) | Pat::Lit(_) => {}
        }
    }

    /// Resolve and rename one constructor pattern name.
    fn pattern_constructor(
        &mut self,
        name: &mut String,
        subject: Option<&Ty>,
    ) -> Option<(String, TypeConstructorSignature, Vec<Option<Ty>>)> {
        let (qualified, constructor) = match split_qualified_constructor(name) {
            Some((owner, constructor)) => (Some(owner.to_string()), constructor.to_string()),
            None => (None, name.clone()),
        };
        if !self.checker.constructor_signatures.contains_key(&constructor) {
            return None;
        }
        let expect = match subject {
            Some(ty) => Expect::Type(ty.clone()),
            None => Expect::none(),
        };
        let resolved = if let Some(owner) = qualified.as_deref() {
            self.resolve(&constructor, Some(owner), &expect, Span::dummy())
        } else if let Some(owners) = self.shared.owners.get(&constructor).cloned() {
            match subject.and_then(|subject| self.owner_for(&constructor, subject)) {
                Some(found) => Some(found),
                None => {
                    let message = format!(
                        "constructor pattern `{constructor}` belongs to {}, and the matched value has no known type; write {}",
                        owner_list(&owners),
                        qualified_choices(&owners, &constructor)
                    );
                    self.error(self.pattern_span, message);
                    None
                }
            }
        } else {
            self.resolve(&constructor, None, &expect, Span::dummy())
        };
        let (owner, signature) = resolved?;
        let renamed = self.rename(&owner, &constructor);
        if &renamed != name {
            *name = renamed;
            self.rewrites += 1;
        }
        let field_types = self.field_types(&signature, &owner, &expect);
        Some((owner, signature, field_types))
    }

    fn arm(&mut self, arm: &mut MatchArm, subject: Option<&Ty>, result: &Expect, locals: &Locals) {
        let mut inner = locals.clone();
        let previous = std::mem::replace(&mut self.pattern_span, arm.pat_span);
        self.bind_pattern(&mut arm.pat, subject, &mut inner);
        self.pattern_span = previous;
        if let Some(guard) = &mut arm.guard {
            self.expr(guard, &Expect::none(), &inner);
        }
        self.expr(&mut arm.body, result, &inner);
    }

    /// The parameters a call gives its arguments: name and expected type.
    fn call_parameter_types(
        &self,
        callee: &str,
        arity: usize,
        locals: &Locals,
    ) -> Option<Vec<(Option<String>, Expect)>> {
        if Self::is_local(callee, locals) {
            return None;
        }
        if let Some(definitions) = self.functions.get(&(callee.to_string(), arity)) {
            if let [parameters] = definitions.as_slice() {
                return Some(
                    parameters
                        .iter()
                        .map(|(name, ty)| {
                            let expect = match ty {
                                Some(ty) if !Self::type_has_variable(ty) => Expect::Type(ty.clone()),
                                Some(_) => Expect::Unknown(Unexpected::Generic(callee.to_string())),
                                None => Expect::none(),
                            };
                            (Some(name.clone()), expect)
                        })
                        .collect(),
                );
            }
        }
        if let Some(family) = self.rule_parameters.get(&(callee.to_string(), arity)) {
            return Some(
                family
                    .iter()
                    .map(|types| {
                        let expect = match types.as_slice() {
                            [one] => Expect::Type(one.clone()),
                            [] => Expect::none(),
                            types => Expect::Unknown(Unexpected::RuleFamily {
                                rule: callee.to_string(),
                                types: types.iter().map(Self::type_string).collect(),
                            }),
                        };
                        (None, expect)
                    })
                    .collect(),
            );
        }
        None
    }

    fn type_has_variable(ty: &Ty) -> bool {
        TypeChecker::canonical_type_contains_variable(&Self::normalize(ty))
    }

    fn expr(&mut self, expr: &mut Expr, expect: &Expect, locals: &Locals) {
        let span = expr.span;
        match &mut expr.kind {
            ExprKind::Var(name) => {
                let Some(constructor) = self.constructor_name(name, locals).map(str::to_string) else {
                    return;
                };
                if let Some((owner, _)) = self.resolve(&constructor, None, expect, span) {
                    let renamed = self.rename(&owner, &constructor);
                    if renamed != *name {
                        *name = renamed;
                        self.rewrites += 1;
                    }
                }
            }
            ExprKind::Field(base, field) => {
                // check_expr reports `X.C` where `X` is an import and a type.
                if self.checker.ambiguous_type_qualifier(base, field).is_some() {
                    return;
                }
                if let Some(signature) =
                    self.checker
                        .type_qualified_constructor(base, field, Some(locals))
                {
                    let renamed = self.rename(&signature.parent, field);
                    expr.kind = ExprKind::Var(renamed);
                    self.rewrites += 1;
                    return;
                }
                self.expr(base, &Expect::none(), locals);
            }
            ExprKind::App(_, _) => self.application(expr, expect, locals),
            ExprKind::Lambda(params, body) => {
                let mut inner = locals.clone();
                let (inputs, output) = match expect.ty().map(Self::normalize) {
                    Some(Ty::Arrow(input, output)) => (vec![*input], Some(*output)),
                    _ => (Vec::new(), None),
                };
                for (index, param) in params.iter().enumerate() {
                    let ty = param
                        .ty
                        .clone()
                        .or_else(|| (params.len() == 1).then(|| inputs.get(index).cloned()).flatten());
                    inner.insert(
                        param.name.clone(),
                        ty.map(|ty| Self::type_string(&ty))
                            .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                    );
                }
                let expect = output.map(Expect::Type).unwrap_or_else(Expect::none);
                self.expr(body, &expect, &inner);
            }
            ExprKind::BinOp(op, left, right) => {
                let comparison = matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=");
                let left_ty = self.infer(left, locals);
                let right_ty = self.infer(right, locals);
                let side = |other: Option<Ty>, own: Option<&Ty>| -> Expect {
                    if comparison {
                        match other {
                            Some(ty) => Expect::Type(ty),
                            None => Expect::Unknown(Unexpected::Equality(op.clone())),
                        }
                    } else if matches!(op.as_str(), "&&" | "||") {
                        Expect::Type(Ty::Name("Bool".into()))
                    } else {
                        match own.cloned().or(other) {
                            Some(ty) => Expect::Type(ty),
                            None => Expect::none(),
                        }
                    }
                };
                let left_expect = side(right_ty.clone(), expect.ty());
                let right_expect = side(left_ty, expect.ty());
                self.expr(left, &left_expect, locals);
                self.expr(right, &right_expect, locals);
            }
            ExprKind::UnOp(_, operand) => self.expr(operand, &Expect::none(), locals),
            ExprKind::If(condition, yes, no) => {
                self.expr(condition, &Expect::none(), locals);
                self.expr(yes, expect, locals);
                self.expr(no, expect, locals);
            }
            ExprKind::Match(subject, arms) => {
                let subject_ty = self.infer(subject, locals);
                self.expr(subject, &Expect::none(), locals);
                for arm in arms {
                    self.arm(arm, subject_ty.as_ref(), expect, locals);
                }
            }
            ExprKind::Block(statements) => {
                let mut inner = locals.clone();
                self.statements(statements, &mut inner, Some(expect));
            }
            ExprKind::Index(collection, index) => {
                self.expr(collection, &Expect::none(), locals);
                self.expr(index, &Expect::none(), locals);
            }
            ExprKind::List(items) => {
                let element = expect
                    .ty()
                    .and_then(Self::element_type)
                    .or_else(|| items.iter().find_map(|item| self.infer(item, locals)));
                let item_expect = match element {
                    Some(ty) => Expect::Type(ty),
                    None => Expect::Unknown(Unexpected::List),
                };
                for item in items {
                    self.expr(item, &item_expect, locals);
                }
            }
            ExprKind::Tuple(items) => {
                let components = match expect.ty().map(Self::normalize) {
                    Some(Ty::App(head, arguments))
                        if matches!(head.as_ref(), Ty::Name(name) if name == "Tuple" || name == "Pair")
                            && arguments.len() == items.len() =>
                    {
                        arguments
                    }
                    _ => Vec::new(),
                };
                for (index, item) in items.iter_mut().enumerate() {
                    let expect = components
                        .get(index)
                        .cloned()
                        .map(Expect::Type)
                        .unwrap_or_else(Expect::none);
                    self.expr(item, &expect, locals);
                }
            }
            ExprKind::Effect(_, arguments) => {
                for argument in arguments {
                    self.expr(argument, &Expect::none(), locals);
                }
            }
            ExprKind::Handle { handlers, body, .. } => {
                for handler in handlers {
                    let mut inner = locals.clone();
                    for param in &handler.params {
                        inner.insert(param.clone(), CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
                    }
                    self.expr(&mut handler.body, &Expect::none(), &inner);
                }
                self.expr(body, expect, locals);
            }
            ExprKind::Try(inner) => self.expr(inner, &Expect::none(), locals),
            ExprKind::Conjunction(goals) | ExprKind::Disjunction(goals) => {
                for goal in goals {
                    self.expr(goal, &Expect::none(), locals);
                }
            }
            ExprKind::Pipe(value, function) => {
                let parameters = match &function.kind {
                    ExprKind::Var(callee) => self.call_parameter_types(callee, 1, locals),
                    ExprKind::App(callee, arguments) => match &callee.kind {
                        ExprKind::Var(callee) => {
                            self.call_parameter_types(callee, arguments.len() + 1, locals)
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let value_expect = parameters
                    .and_then(|parameters| parameters.into_iter().next())
                    .map(|(_, expect)| expect)
                    .unwrap_or_else(Expect::none);
                self.expr(value, &value_expect, locals);
                self.expr(function, &Expect::none(), locals);
            }
            ExprKind::Lit(_) | ExprKind::Unit => {}
        }
    }

    fn application(&mut self, expr: &mut Expr, expect: &Expect, locals: &Locals) {
        let span = expr.span;
        let ExprKind::App(function, arguments) = &mut expr.kind else {
            return;
        };
        // Compiler markers carry names and types, not values.
        if let ExprKind::Var(marker) = &function.kind {
            match marker.as_str() {
                PATHOF_MARKER | REFOF_MARKER => return,
                "__typed" => {
                    if let Some(value) = arguments.first_mut() {
                        self.expr(value, &Expect::none(), locals);
                    }
                    return;
                }
                NAMED_ARG_MARKER => {
                    if let Some(value) = arguments.get_mut(1) {
                        self.expr(value, expect, locals);
                    }
                    return;
                }
                _ => {}
            }
        }

        // A constructor application: bare, or `Type.Constructor(...)`.
        let constructor = match &function.kind {
            ExprKind::Var(name) => self
                .constructor_name(name, locals)
                .map(|name| (name.to_string(), None)),
            ExprKind::Field(base, field) => {
                if self.checker.ambiguous_type_qualifier(base, field).is_some() {
                    return;
                }
                self.checker
                    .type_qualified_constructor(base, field, Some(locals))
                    .map(|signature| (field.clone(), Some(signature.parent)))
            }
            _ => None,
        };
        if let Some((constructor, qualified)) = constructor {
            let original = (qualified.is_some() || self.shared.owners.contains_key(&constructor))
                .then(|| Expr::new(ExprKind::App(function.clone(), arguments.clone()), span));
            let resolved = self.resolve(&constructor, qualified.as_deref(), expect, function.span);
            let mut field_types = Vec::new();
            let mut signature = None;
            if let Some((owner, found)) = resolved {
                field_types = self.field_types(&found, &owner, expect);
                let renamed = self.rename(&owner, &constructor);
                if qualified.is_some() || renamed != constructor {
                    **function = Expr::new(ExprKind::Var(renamed), function.span);
                    self.rewrites += 1;
                }
                if let Some(original) = original {
                    let label = if self.shared.owners.contains_key(&constructor) {
                        format!("{owner}.{constructor}")
                    } else {
                        constructor.clone()
                    };
                    let owner_type = match expect.ty() {
                        Some(expected)
                            if TypeChecker::canonical_nominal_owner(&Self::type_string(expected))
                                .as_deref()
                                == Some(owner.as_str()) =>
                        {
                            Self::type_string(expected)
                        }
                        _ => owner.clone(),
                    };
                    self.calls.push(ConstructorCallCheck {
                        expression: original,
                        label,
                        signature: found.clone(),
                        owner_type,
                        locals: locals.clone(),
                    });
                }
                signature = Some(found);
            }
            for (index, argument) in arguments.iter_mut().enumerate() {
                let field_index = match named_arg_parts(argument) {
                    Some((field, _)) => signature
                        .as_ref()
                        .and_then(|signature| signature.fields.iter().position(|known| known == field)),
                    None => Some(index),
                };
                let expect = field_index
                    .and_then(|index| field_types.get(index).cloned().flatten())
                    .map(Expect::Type)
                    .unwrap_or_else(Expect::none);
                self.expr(argument, &expect, locals);
            }
            return;
        }

        let callee = match &function.kind {
            ExprKind::Var(name) if !Self::is_local(name, locals) => Some(name.clone()),
            _ => None,
        };
        let Some(callee) = callee else {
            self.expr(function, &Expect::none(), locals);
            for argument in arguments {
                self.expr(argument, &Expect::none(), locals);
            }
            return;
        };

        // Collection builtins type their callbacks by the element type.
        let collection_element = arguments
            .first()
            .and_then(|collection| self.infer(collection, locals))
            .and_then(|ty| Self::element_type(&ty));
        let result_element = expect.ty().and_then(Self::element_type);
        let callback = |index: usize, count: usize| -> Option<(Vec<Option<Ty>>, Expect)> {
            match (callee.as_str(), index, count) {
                ("map", 1, 2) => Some((
                    vec![collection_element.clone()],
                    result_element.clone().map(Expect::Type).unwrap_or_else(Expect::none),
                )),
                ("flat_map", 1, 2) => Some((
                    vec![collection_element.clone()],
                    expect.ty().cloned().map(Expect::Type).unwrap_or_else(Expect::none),
                )),
                ("filter" | "any" | "all" | "find" | "sort_by" | "count" | "partition" | "group_by" | "sum_by" | "each" | "for_each", 1, 2) => {
                    Some((vec![collection_element.clone()], Expect::none()))
                }
                ("foldl" | "fold", 2, 3) => Some((
                    vec![expect.ty().cloned(), collection_element.clone()],
                    expect.ty().cloned().map(Expect::Type).unwrap_or_else(Expect::none),
                )),
                _ => None,
            }
        };
        let parameters = self.call_parameter_types(&callee, arguments.len(), locals);
        let count = arguments.len();
        for (index, argument) in arguments.iter_mut().enumerate() {
            if let (Some((inputs, output)), ExprKind::Lambda(params, body)) =
                (callback(index, count), &mut argument.kind)
            {
                let mut inner = locals.clone();
                for (position, param) in params.iter().enumerate() {
                    let ty = param.ty.clone().or_else(|| inputs.get(position).cloned().flatten());
                    inner.insert(
                        param.name.clone(),
                        ty.map(|ty| Self::type_string(&ty))
                            .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                    );
                }
                self.expr(body, &output, &inner);
                continue;
            }
            let special = match callee.as_str() {
                "concat" | "append" => expect.ty().filter(|_| result_element.is_some()).cloned(),
                "push" | "contains" | "prepend" if index == 1 => collection_element.clone(),
                "foldl" | "fold" if index == 1 => expect.ty().cloned(),
                _ => None,
            };
            let expect = match (special, &parameters) {
                (Some(ty), _) => Expect::Type(ty),
                (None, Some(parameters)) => match named_arg_parts(argument) {
                    Some((name, _)) => parameters
                        .iter()
                        .find(|(parameter, _)| parameter.as_deref() == Some(name))
                        .map(|(_, expect)| expect.clone())
                        .unwrap_or_else(Expect::none),
                    None => parameters
                        .get(index)
                        .map(|(_, expect)| expect.clone())
                        .unwrap_or_else(Expect::none),
                },
                (None, None) if self.checker.builtins.contains_key(&callee) => {
                    Expect::Unknown(Unexpected::Generic(callee.clone()))
                }
                (None, None) => Expect::none(),
            };
            self.expr(argument, &expect, locals);
        }
    }
}
