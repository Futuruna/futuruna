//! `runa fmt --migrate`: write each rule family's types once, on its
//! declaring line, and declare its result type where that is mechanical
//! (docs/rfcs/named-scalar-types-and-rule-result-types.md, Part 2,
//! "Migration").
//!
//! 1. A family with more than one line in its home scope, where every line
//!    writes the same parameter types and names: the first line keeps its
//!    types (and takes a type another line writes at a name of its own), its
//!    later lines lose theirs, and a value family gains `: R`, the result
//!    type the checker inferred.
//! 2. A family with a clause whose value converts a number to a unit
//!    (`-> Ore(0)`): the declaring line gains `: Ore` and the clause becomes
//!    `-> 0`.
//! 3. A family whose lines write different types, or different names where it
//!    is typed, whose first line is an exception, or whose first line matches
//!    a pattern where another line writes a type, is reported and not
//!    rewritten: the writer declares it.
//!
//! `: R` is written only where it changes nothing: the checker resolved one
//! concrete type, the family is a value family (not a relation of facts and
//! goals), and a declared miss (`False` for `: Bool`, an error otherwise)
//! is the miss the family has today. Otherwise the types are still written
//! once and the family is reported as not declared, with the reason.

use crate::{
    parse_prelude, parse_type_annotation, prepend_prelude, rule_family_miss_is_value_error,
    rule_head_is_irrefutable, typed_rule_head_argument, with_legacy_rule_types, Expr, ExprKind,
    Lexer, Literal, Parser, Rule, RuleDispatchKey, Stmt, Ty, TypeCheckArtifacts, TypeChecker,
    TypeDecl,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The result of migrating one file's rule families.
#[derive(Debug, Default)]
pub struct RuleTypeMigration {
    pub source: String,
    /// Families that gained `: R`, as `line: name: R`.
    pub declared: Vec<String>,
    /// Families whose later lines lost their repeated types and that were not
    /// declared, with the reason, as `line: name: reason`.
    pub not_declared: Vec<String>,
    /// Families left as they are, with the reason, as `line: name: reason`.
    pub skipped: Vec<String>,
    /// Families whose later lines lost their repeated parameter types.
    pub types_once: Vec<String>,
}

impl RuleTypeMigration {
    pub fn changed(&self, original: &str) -> bool {
        self.source != original
    }
}

struct Edit {
    start: usize,
    end: usize,
    text: String,
}

fn is_name(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Var(name)
            if name != "_" && !name.chars().next().is_some_and(char::is_uppercase) =>
        {
            Some(name)
        }
        _ => None,
    }
}

/// One head argument: the written argument and, when typed, the char range
/// of `: Type` after it and the type's text.
struct Argument<'a> {
    inner: &'a Expr,
    typed: Option<(usize, usize, String)>,
}

/// The end of a type written at `start` in a head argument list: before the
/// `,` or `)` that closes the argument.
fn type_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0i32;
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '(' | '[' => depth += 1,
            ')' | ']' if depth == 0 => break,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => break,
            '\n' if depth == 0 => break,
            _ => {}
        }
        index += 1;
    }
    while index > start && chars[index - 1].is_whitespace() {
        index -= 1;
    }
    index
}

fn arguments<'a>(head: &'a Expr, chars: &[char]) -> Option<Vec<Argument<'a>>> {
    let ExprKind::App(_, arguments) = &head.kind else {
        return Some(Vec::new());
    };
    arguments
        .iter()
        .map(|argument| {
            let Some((inner, _)) = typed_rule_head_argument(argument) else {
                return Some(Argument {
                    inner: argument,
                    typed: None,
                });
            };
            if inner.span.is_dummy() {
                return None;
            }
            let mut colon = inner.span.end;
            while colon < chars.len() && chars[colon].is_whitespace() {
                colon += 1;
            }
            if chars.get(colon) != Some(&':') {
                // The type is the declaring line's (or an imported
                // declaration's), copied onto a name written without one.
                return Some(Argument { inner, typed: None });
            }
            let mut type_start = colon + 1;
            while type_start < chars.len() && chars[type_start] == ' ' {
                type_start += 1;
            }
            let end = type_end(chars, type_start);
            let text = chars[type_start..end].iter().collect::<String>();
            Some(Argument {
                inner,
                typed: Some((inner.span.end, end, text)),
            })
        })
        .collect()
}

fn rule_head(rule: &Rule) -> Option<&Expr> {
    rule.head()
}

fn is_value_rule(rule: &Rule) -> bool {
    match rule {
        Rule::Clause {
            body: Some(body), ..
        } => !matches!(
            body.kind,
            ExprKind::Conjunction(_) | ExprKind::Disjunction(_)
        ),
        Rule::Default { .. } | Rule::Exception { .. } => true,
        Rule::Clause { body: None, .. } | Rule::ReactiveScope { .. } => false,
    }
}

/// Whether a candidate answers every call that reaches it: no miss can
/// follow it.
fn is_total(rule: &Rule) -> bool {
    rule_head_is_irrefutable(rule)
        && match rule {
            Rule::Clause { body, .. } => body.is_some(),
            Rule::Default { condition, .. } | Rule::Exception { condition, .. } => {
                condition.is_none()
            }
            Rule::ReactiveScope { .. } => false,
        }
}

fn value_of(rule: &Rule) -> Option<&Expr> {
    match rule {
        Rule::Clause { body, .. } => body.as_ref(),
        Rule::Default { value, .. } | Rule::Exception { value, .. } => Some(value),
        Rule::ReactiveScope { .. } => None,
    }
}

/// `U(<number>)` for a unit `U`: the unit and the number's range.
fn unit_literal<'a>(value: &'a Expr, units: &BTreeSet<String>) -> Option<(&'a str, &'a Expr)> {
    let ExprKind::App(function, arguments) = &value.kind else {
        return None;
    };
    let ExprKind::Var(unit) = &function.kind else {
        return None;
    };
    let [number] = arguments.as_slice() else {
        return None;
    };
    let literal = match &number.kind {
        ExprKind::Lit(Literal::Int(_)) => true,
        ExprKind::UnOp(operator, inner) => {
            operator == "-" && matches!(inner.kind, ExprKind::Lit(Literal::Int(_)))
        }
        _ => false,
    };
    (literal && units.contains(unit) && !number.span.is_dummy()).then_some((unit.as_str(), number))
}

/// The written form of a resolved result type: `T?` for `Option(T)`; `None`
/// for a type that is not concrete.
fn written_type(canonical: &str) -> Option<String> {
    fn write(ty: &Ty) -> Option<String> {
        Some(match ty {
            Ty::Name(name) if name != "_" && !name.contains('.') => name.clone(),
            Ty::Unit => "()".to_string(),
            Ty::Optional(inner) => format!("{}?", write(inner)?),
            Ty::App(constructor, arguments) => {
                let Ty::Name(constructor) = constructor.as_ref() else {
                    return None;
                };
                if constructor == "Option" && arguments.len() == 1 {
                    if let Ty::Name(_) = &arguments[0] {
                        return Some(format!("{}?", write(&arguments[0])?));
                    }
                }
                let arguments = arguments.iter().map(write).collect::<Option<Vec<_>>>()?;
                format!("{constructor}({})", arguments.join(", "))
            }
            _ => return None,
        })
    }
    write(&parse_type_annotation(canonical).ok()?)
}

fn resolved_result(
    artifacts: &TypeCheckArtifacts,
    key: &RuleDispatchKey,
    arities: usize,
) -> Option<String> {
    if let Some(ty) = artifacts
        .rule_dispatch_backend_return_types
        .get(key)
        .filter(|_| {
            !artifacts
                .rule_dispatch_backend_return_issues
                .contains_key(key)
        })
        .or_else(|| {
            artifacts
                .rule_dispatch_return_types
                .get(key)
                .filter(|_| !artifacts.rule_dispatch_return_issues.contains_key(key))
        })
    {
        return Some(ty.clone());
    }
    if arities != 1 {
        return None;
    }
    match &key.scope {
        None => artifacts.rule_return_types.get(&key.name).cloned(),
        Some(scope) => artifacts
            .rule_scope_member_return_types
            .get(scope)
            .and_then(|members| members.get(&key.name))
            .cloned(),
    }
}

/// Whether a sequence has a family the migration may rewrite: one with more
/// than one line, or with a number converted to a unit as its value. Other
/// files are not checked.
fn has_candidate_family(statements: &[Stmt]) -> bool {
    let candidate = |sequence: &[Stmt]| {
        let mut lines: BTreeMap<(String, usize), usize> = BTreeMap::new();
        for statement in sequence {
            let Stmt::Rule(rule) = statement else {
                continue;
            };
            if let Some(key) = rule.callable_name_arity() {
                let count = lines.entry(key).or_default();
                *count += 1;
                if *count > 1 {
                    return true;
                }
            }
            if let Some(ExprKind::App(function, arguments)) =
                value_of(rule).map(|value| &value.kind)
            {
                if matches!(&function.kind, ExprKind::Var(name) if name.starts_with(char::is_uppercase))
                    && matches!(arguments.as_slice(), [number] if matches!(number.kind, ExprKind::Lit(Literal::Int(_)) | ExprKind::UnOp(_, _)))
                {
                    return true;
                }
            }
        }
        false
    };
    candidate(statements)
        || statements.iter().any(|statement| {
            matches!(statement, Stmt::TypeDecl(TypeDecl::RuleScope { body, .. }) if candidate(body))
        })
}

/// Migrate the rule families of `source`, the file at `filename`. The
/// program is checked with its imports to learn each family's result type.
pub fn migrate_rule_declarations(
    source: &str,
    filename: &str,
) -> Result<RuleTypeMigration, String> {
    let statements = with_legacy_rule_types(|| {
        let tokens = Lexer::new(source).tokenize();
        Parser::new(tokens, source)
            .with_source_path(Path::new(filename))
            .parse_program()
    })?;
    if !has_candidate_family(&statements) {
        return Ok(RuleTypeMigration {
            source: source.to_string(),
            ..RuleTypeMigration::default()
        });
    }
    let artifacts = with_legacy_rule_types(|| {
        let checked = prepend_prelude(parse_prelude(), &statements);
        let source_dir = Path::new(filename).parent().map(|parent| {
            if parent.as_os_str().is_empty() {
                ".".to_string()
            } else {
                parent.to_string_lossy().to_string()
            }
        });
        TypeChecker::check_with_artifacts(&checked, source_dir, source)
    });
    let chars = source.chars().collect::<Vec<_>>();
    let line_of = |offset: usize| crate::char_offset_to_line_col(source, offset).0;
    let mut migration = RuleTypeMigration::default();
    let mut edits = Vec::new();

    let mut sequences: Vec<(Option<String>, &[Stmt])> = vec![(None, &statements)];
    for statement in &statements {
        if let Stmt::TypeDecl(TypeDecl::RuleScope { name, body, .. }) = statement {
            sequences.push((Some(name.clone()), body));
        }
    }
    for (scope, sequence) in sequences {
        migrate_sequence(
            scope,
            sequence,
            &artifacts,
            &chars,
            &line_of,
            &mut edits,
            &mut migration,
        );
    }

    edits.sort_by_key(|edit| std::cmp::Reverse((edit.start, edit.end)));
    let mut rewritten = chars;
    for edit in edits {
        rewritten.splice(edit.start..edit.end, edit.text.chars());
    }
    migration.source = rewritten.into_iter().collect();
    if migration.source != source {
        // A family reported and not rewritten still repeats its types; the
        // writer declares it. Every other family must now read as written.
        let parse = || {
            let tokens = Lexer::new(&migration.source).tokenize();
            Parser::new(tokens, &migration.source).parse_program()
        };
        let parsed = if migration.skipped.is_empty() {
            parse()
        } else {
            with_legacy_rule_types(parse)
        };
        parsed.map_err(|error| {
            format!("the migrated file does not parse: {error}; file unchanged")
        })?;
    }
    Ok(migration)
}

#[allow(clippy::too_many_arguments)]
fn migrate_sequence(
    scope: Option<String>,
    sequence: &[Stmt],
    artifacts: &TypeCheckArtifacts,
    chars: &[char],
    line_of: &dyn Fn(usize) -> usize,
    edits: &mut Vec<Edit>,
    migration: &mut RuleTypeMigration,
) {
    let declared = sequence
        .iter()
        .filter_map(crate::declared_rule_result)
        .map(|declared| (declared.name, declared.arity))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    let mut families: BTreeMap<(String, usize), Vec<&Rule>> = BTreeMap::new();
    let mut arities: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for statement in sequence {
        let Stmt::Rule(rule) = statement else {
            continue;
        };
        let Some((name, arity)) = rule.callable_name_arity() else {
            continue;
        };
        arities.entry(name.clone()).or_default().insert(arity);
        let family = families.entry((name.clone(), arity)).or_default();
        if family.is_empty() {
            order.push((name, arity));
        }
        family.push(rule);
    }

    for key in order {
        let rules = &families[&key];
        let (name, arity) = &key;
        let dispatch_key = RuleDispatchKey {
            scope: scope.clone(),
            name: name.clone(),
            arity: *arity,
        };
        let Some(first_head) = rule_head(rules[0]) else {
            continue;
        };
        let at = format!("{}: `{name}`", line_of(first_head.span.start));
        let Some(heads) = rules
            .iter()
            .map(|rule| rule_head(rule).and_then(|head| Some((head, arguments(head, chars)?))))
            .collect::<Option<Vec<_>>>()
        else {
            migration.skipped.push(format!(
                "{at}: a typed parameter could not be located in the source"
            ));
            continue;
        };
        let already_declared = declared.contains(&key);
        let units = &artifacts.unit_names;
        let unit_values = rules
            .iter()
            .filter_map(|rule| unit_literal(value_of(rule)?, units))
            .collect::<Vec<_>>();
        let typed_later = heads[1..]
            .iter()
            .any(|(_, arguments)| arguments.iter().any(|argument| argument.typed.is_some()));
        if rules.len() == 1 && unit_values.is_empty() {
            // Rule 4: a one-line family states its type in its value.
            continue;
        }
        if !typed_later && already_declared {
            continue;
        }

        // Rule 3: what the writer must declare.
        if matches!(rules[0], Rule::Exception { .. }) {
            if typed_later || heads[0].1.iter().any(|argument| argument.typed.is_some()) {
                migration.skipped.push(format!(
                    "{at}: its first line is an exception, which cannot declare; add `| {name}(...): <result>` above it"
                ));
            }
            continue;
        }
        let mut types: Vec<BTreeSet<String>> = vec![BTreeSet::new(); *arity];
        let mut names: Vec<BTreeSet<String>> = vec![BTreeSet::new(); *arity];
        let mut type_text: Vec<Option<String>> = vec![None; *arity];
        for (_, arguments) in &heads {
            // A name repeated from an earlier position is a pattern, not the
            // parameter's name.
            let mut seen = BTreeSet::new();
            for (position, argument) in arguments.iter().enumerate() {
                if let Some((_, _, text)) = &argument.typed {
                    let normalized = parse_type_annotation(text)
                        .map(|ty| ty.to_string())
                        .unwrap_or_else(|_| text.clone());
                    types[position].insert(normalized);
                    type_text[position].get_or_insert_with(|| text.clone());
                }
                if let Some(name) = is_name(argument.inner) {
                    if seen.insert(name) {
                        names[position].insert(name.to_string());
                    }
                }
            }
        }
        if let Some(position) = types.iter().position(|types| types.len() > 1) {
            migration.skipped.push(format!(
                "{at}: its lines write different types for parameter {} ({}); declare it with one type",
                position + 1,
                types[position].iter().map(|ty| format!("`{ty}`")).collect::<Vec<_>>().join(", ")
            ));
            continue;
        }
        let typed = types.iter().any(|types| !types.is_empty());
        let differing_names = names.iter().position(|names| names.len() > 1);
        if let (true, Some(position)) = (typed, differing_names) {
            migration.skipped.push(format!(
                "{at}: its lines call parameter {} {}; a declared family calls it by one name",
                position + 1,
                names[position]
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ));
            continue;
        }
        if let Some(position) = (0..*arity).find(|position| {
            !types[*position].is_empty() && is_name(heads[0].1[*position].inner).is_none()
        }) {
            migration.skipped.push(format!(
                "{at}: its first line matches a pattern at typed parameter {}; add `| {name}(...): <result>` above it",
                position + 1
            ));
            continue;
        }

        // The result type, where declaring it changes nothing.
        let mut result = None;
        let mut reason = None;
        if already_declared {
        } else if !rules.iter().all(|rule| is_value_rule(rule)) {
            // A relation states `Bool` in its facts and goals; only its
            // repeated types go.
        } else if let Some(position) = differing_names {
            reason = Some(format!(
                "its lines call parameter {} {}",
                position + 1,
                names[position]
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ));
        } else {
            let arity_count = arities.get(name).map_or(1, BTreeSet::len);
            match resolved_result(artifacts, &dispatch_key, arity_count)
                .as_deref()
                .and_then(written_type)
            {
                None => {
                    reason = Some("the checker does not resolve one concrete result type".into());
                }
                Some(written) => {
                    let declared_false =
                        TypeChecker::canonical_explore_type_name(&written) == "Bool";
                    let today_false = artifacts
                        .rule_dispatch_runtime_boolean_miss_keys
                        .contains(&dispatch_key)
                        || !rule_family_miss_is_value_error(rules.iter().copied());
                    if rules.iter().any(|rule| is_total(rule)) || declared_false == today_false {
                        result = Some(written);
                    } else if declared_false {
                        reason = Some(format!(
                            "`: {written}` would answer False where no clause applies, where it stops today"
                        ));
                    } else {
                        reason = Some(format!(
                            "`: {written}` would stop where no clause applies, where it answers False today"
                        ));
                    }
                }
            }
        }

        // The edits: types on the declaring line, names on the others.
        let (first_head, first_arguments) = &heads[0];
        for (position, argument) in first_arguments.iter().enumerate() {
            if let (None, Some(text)) = (&argument.typed, &type_text[position]) {
                edits.push(Edit {
                    start: argument.inner.span.end,
                    end: argument.inner.span.end,
                    text: format!(": {text}"),
                });
            }
        }
        for (_, arguments) in &heads[1..] {
            for argument in arguments {
                if let Some((start, end, _)) = &argument.typed {
                    edits.push(Edit {
                        start: *start,
                        end: *end,
                        text: String::new(),
                    });
                }
            }
        }
        if typed_later {
            migration.types_once.push(at.clone());
        }
        match (&result, reason) {
            (Some(written), _) => {
                edits.push(Edit {
                    start: first_head.span.end,
                    end: first_head.span.end,
                    text: format!(": {written}"),
                });
                // Rule 2: a number takes the declared unit.
                for (unit, number) in &unit_values {
                    if *unit == written {
                        let value = rules
                            .iter()
                            .filter_map(|rule| value_of(rule))
                            .find(|value| matches!(&value.kind, ExprKind::App(_, arguments) if arguments.first().is_some_and(|argument| std::ptr::eq(argument, *number))));
                        if let Some(value) = value {
                            edits.push(Edit {
                                start: value.span.start,
                                end: value.span.end,
                                text: chars[number.span.start..number.span.end].iter().collect(),
                            });
                        }
                    }
                }
                migration.declared.push(format!("{at}: {written}"));
            }
            (None, Some(reason)) if typed_later => {
                migration
                    .not_declared
                    .push(format!("{at}: types written once; not declared: {reason}"));
            }
            (None, Some(reason)) => {
                migration
                    .not_declared
                    .push(format!("{at}: not declared: {reason}"));
            }
            _ => {}
        }
    }
}
