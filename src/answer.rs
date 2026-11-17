//! Answering an entry's needs round by round, and the AnswerGuide every
//! entry generates.
//!
//! `answer(entry, case, resolve)` queries the entry, as `query entry(case)`
//! does, calls `resolve` once per round with all of that round's needs and
//! the round's case, and writes the answers it gives with `fill`. A round
//! that answers nothing ends `Unanswered` with that round's needs open.
//! Which source answers which need is the caller's business: `resolve` reads
//! each need's path and metadata and decides.
//!
//! `answer_guide(entry)` is the entry's AnswerGuide, derived from its
//! contract as `runa sdk ts` derives `<entry>Guide`: every input path
//! pattern at which a need can arise, with its type and the law's metadata.

use super::*;
use crate::query::{
    display_location, list_items, named_fields, optional, record, reference, string,
};

/// The answer prelude: the resolver's reply, the rounds and the outcome,
/// `from_case`, the round loop of `answer`, and the AnswerGuide of
/// `answer_guide`. It joins every program that calls `answer`, `from_case`
/// or `answer_guide`.
const ANSWER_PRELUDE: &str = r#"-- Answering an entry's needs round by round.
# Answers(i) = Answers(from: i, given: List(Path(i)))
# Round(i) = Round(paths: List(Path(i)), given: List(Path(i)))
# Outcome(o, i) = Answered(result: o) | Unanswered(open: List(Need(i)), case: i)
# Answering(o, i) = Answering(rounds: List(Round(i)), outcome: Outcome(o, i))
# AnswerStep(i) = AnswerStep(given: List(Path(i)), case: i)
-- The AnswerGuide an entry generates: every need that can arise for it.
# GuideNeed = GuideNeed(pattern: String, kind: NeedKind, value_type: String, label: Option(String), question: Option(String), help: Option(String), unit: Option(String), keys: List(String), sources: List(NeedSource), entry: Option(ProgramReference), subject: Option(String))
# AnswerGuide = AnswerGuide(entry: String, needs: List(GuideNeed))

-- `answer_guide(entry)`: the parser passes the form's site.
> answer_guide(site: String) -> AnswerGuide {
    __answer_guide(site)
}

-- A resolver that answers every need from `complete`.
> from_case(complete: i) -> (List(Need(i)), i) -> Answers(i) {
    |needs, case| Answers(from = complete, given = map(needs, |n| n.path))
}

-- `answer(entry, case, resolve)`: the parser passes the query of the entry
-- and the form's site.
> answer(ask: i -> Query(o, i), case: i, resolve: (List(Need(i)), i) -> Answers(i), site: String) -> Answering(o, i) {
    __answer_from(ask, case, resolve, site, [])
}

> __answer_from(ask: i -> Query(o, i), case: i, resolve: (List(Need(i)), i) -> Answers(i), site: String, rounds: List(Round(i))) -> Answering(o, i) {
    = q = ask(case)
    match q {
        | Complete -> Answering(rounds = rounds, outcome = Answered(result = q.result))
        | Needs -> {
            = step = __answer_step(case, q.needs, resolve, site, length(rounds) + 1)
            = asked = concat(rounds, [Round(paths = map(q.needs, |n| n.path), given = step.given)])
            if length(step.given) > 0 {
                __answer_from(ask, step.case, resolve, site, asked)
            } else {
                Answering(rounds = asked, outcome = Unanswered(open = q.needs, case = case))
            }
        }
    }
}

-- One round: the resolver with all of the round's needs.
> __answer_step(case: i, needs: List(Need(i)), resolve: (List(Need(i)), i) -> Answers(i), site: String, round: Int) -> AnswerStep(i) {
    __answer_round(case, needs, resolve, site, round)
}
"#;

pub fn answer_prelude_source() -> &'static str {
    ANSWER_PRELUDE
}

/// The declarations of [`answer_prelude_source`].
pub fn answer_prelude() -> Vec<Stmt> {
    let tokens = Lexer::new(ANSWER_PRELUDE).tokenize();
    match Parser::new(tokens, ANSWER_PRELUDE).parse_program() {
        Ok(statements) => statements
            .into_iter()
            .filter(|statement| matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_)))
            .collect(),
        Err(error) => {
            eprintln!("BUG: answer prelude parse error: {error}");
            Vec::new()
        }
    }
}

/// Names whose use brings the answer prelude, unless the program declares
/// them itself.
const PRELUDE_NAMES: [&str; 1] = ["from_case"];

/// Whether a program calls `answer` or `from_case`.
pub fn program_uses_answer(statements: &[Stmt]) -> bool {
    let user = statements
        .iter()
        .position(|statement| matches!(statement, Stmt::PreludeBoundary))
        .map_or(statements, |boundary| &statements[boundary + 1..]);
    let declared: BTreeSet<&str> = user
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Defn(Defn::Fn { name, .. }) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let mut uses = false;
    for statement in user {
        walk_ast_stmt(statement, &mut |child| {
            if uses {
                return;
            }
            if let AstChild::Expr(expr) = child {
                uses = answer_parts(expr).is_some()
                    || answer_guide_parts(expr).is_some()
                    || matches!(&expr.kind, ExprKind::Var(name)
                        if PRELUDE_NAMES.contains(&name.as_str()) && !declared.contains(name.as_str()));
            }
        });
        if uses {
            return true;
        }
    }
    false
}

/// The value of the field `name` of a record value.
fn field<'v>(value: &'v Value, name: &str) -> Option<&'v Value> {
    named_fields(value)?
        .1
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
}

impl Interpreter {
    /// `__answer_round(case, needs, resolve, site, round)`: one round of
    /// `answer`. The resolver is called once, with all of the round's needs
    /// and its case; the paths it gives must be among the needs' paths, and
    /// are written with `fill`.
    pub(crate) fn eval_answer_round(
        &mut self,
        namespace: &RuntimeNamespace,
        args: Vec<Value>,
        env: &Env,
    ) -> Value {
        let Ok([case, needs, resolve, site, round]) = <[Value; 5]>::try_from(args) else {
            return self.panic_or_ground_fail("`answer` takes an entry, a case and a resolver");
        };
        let (Value::Str(site), Value::Int(round)) = (&site, &round) else {
            return self.panic_or_ground_fail("`answer` is written `answer(entry, case, resolve)`");
        };
        let (entry, offset) = site.split_once('\u{1f}').unwrap_or((site.as_str(), ""));
        let offset = offset.parse::<usize>().ok();
        let at = self
            .query_current_file()
            .map(|file| {
                let line = offset.and_then(|offset| self.query.line(&file, offset));
                format!("{}: ", display_location(&file, line))
            })
            .unwrap_or_default();
        let prefix = format!("{at}`answer({entry}, …)`, round {round}");
        let mut asked = Vec::new();
        for need in list_items(&needs).unwrap_or_default() {
            let Some(Value::Str(path)) = field(&need, "path") else {
                return self.panic_or_ground_fail(format!("{prefix}: a need without a path"));
            };
            asked.push(path.clone());
        }
        let reply = self.apply(resolve, vec![needs, case.clone()], env);
        if self.calculation_failed() {
            return Value::Unit;
        }
        let (Some(from), Some(given)) = (
            field(&reply, "from").cloned(),
            field(&reply, "given").and_then(list_items),
        ) else {
            return self
                .panic_or_ground_fail(format!("{prefix}: the resolver returns no `Answers`"));
        };
        for path in &given {
            let Value::Str(path) = path else {
                return self.panic_or_ground_fail(format!(
                    "{prefix}: the resolver gives a path that is not a `Path`"
                ));
            };
            if !asked.contains(path) {
                return self.panic_or_ground_fail(format!(
                    "{prefix}: the resolver gives `{path}`, which it was not asked; a resolver answers only the needs it is given"
                ));
            }
        }
        let filled = if given.is_empty() {
            case
        } else {
            let filled = self.eval_fill(case, from, Value::List(given.clone()));
            if self.calculation_failed() {
                return Value::Unit;
            }
            filled
        };
        let step = record(
            "AnswerStep",
            "AnswerStep",
            vec![("given", Value::List(given)), ("case", filled)],
        );
        if namespace.same_instance(&self.runtime_root) {
            step
        } else {
            self.constructors_in(namespace, step)
        }
    }

    /// `__answer_guide(site)`: the AnswerGuide of the entry the site names,
    /// derived from its contract.
    pub(crate) fn eval_answer_guide(
        &mut self,
        namespace: &RuntimeNamespace,
        args: Vec<Value>,
    ) -> Value {
        let [Value::Str(site)] = args.as_slice() else {
            return self.panic_or_ground_fail("`answer_guide` is written `answer_guide(entry)`");
        };
        let entry = site
            .split_once('\u{1f}')
            .map_or(site.as_str(), |(entry, _)| entry);
        let contract = match self.query_prepared(entry) {
            Ok(prepared) => prepared.borrow().contract().clone(),
            Err(error) => {
                return self.panic_or_ground_fail(format!("`answer_guide({entry})`: {error}"))
            }
        };
        let text = |value: &Option<String>| optional(value.as_deref().map(string));
        let needs = crate::calculate::answer_guide::answer_guide(&contract)
            .iter()
            .map(|need| {
                let kind = match need.kind {
                    crate::calculate::CalculationNeedKind::Fact => "FactNeed",
                    crate::calculate::CalculationNeedKind::Entry => "EntryNeed",
                };
                record(
                    "GuideNeed",
                    "GuideNeed",
                    vec![
                        ("pattern", string(&need.pattern)),
                        (
                            "kind",
                            Value::Constructor(
                                crate::constructor_scope::constructor_runtime_name(
                                    "NeedKind", kind,
                                )
                                .into_owned(),
                                Rc::new(Vec::new()),
                            ),
                        ),
                        ("value_type", string(&need.ty)),
                        ("label", text(&need.label)),
                        ("question", text(&need.question)),
                        ("help", text(&need.help)),
                        ("unit", text(&need.unit)),
                        (
                            "keys",
                            Value::List(need.keys.iter().map(|key| string(key)).collect()),
                        ),
                        (
                            "sources",
                            Value::List(
                                need.sources
                                    .iter()
                                    .map(|source| {
                                        record(
                                            "NeedSource",
                                            "NeedSource",
                                            vec![
                                                ("label", string(&source.label)),
                                                ("role", string(&source.role)),
                                                (
                                                    "binding",
                                                    optional((!source.binding.is_empty()).then(
                                                        || {
                                                            reference(
                                                                "ProgramSymbolReference",
                                                                &source.binding,
                                                            )
                                                        },
                                                    )),
                                                ),
                                            ],
                                        )
                                    })
                                    .collect(),
                            ),
                        ),
                        (
                            "entry",
                            optional(
                                need.entry
                                    .as_deref()
                                    .map(|entry| reference("ProgramSymbolReference", entry)),
                            ),
                        ),
                        ("subject", text(&need.subject)),
                    ],
                )
            })
            .collect();
        let guide = record(
            "AnswerGuide",
            "AnswerGuide",
            vec![("entry", string(entry)), ("needs", Value::List(needs))],
        );
        if namespace.same_instance(&self.runtime_root) {
            guide
        } else {
            self.constructors_in(namespace, guide)
        }
    }
}
