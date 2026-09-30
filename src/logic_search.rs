//! Binding and backtracking runtime for generated relational queries.
//!
//! This module deliberately depends only on `std`: code generation can embed
//! it in an ordinary Rust executable without linking the compiler or parser.

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::BTreeMap;

/// Deepest nesting of derived-rule goals within one logic query. The
/// interpreter applies the same limit.
pub const LOGIC_QUERY_DEPTH_LIMIT: usize = 10_000;

pub fn logic_query_depth_error(name: &str) -> String {
    format!(
        "logic query `{name}` exceeded its recursion limit of {LOGIC_QUERY_DEPTH_LIMIT}; evaluation is incomplete"
    )
}

thread_local! {
    /// Goal depth of the expression a running query is evaluating. A query
    /// started from that expression continues at this depth.
    static EVALUATION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Debug)]
pub enum Term {
    Variable(usize),
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Unit,
    Compound(String, Vec<Term>),
}

/// Values with an unambiguous native representation for rule-head matching.
/// Keep this distinct from Rust's PartialEq: logic matching preserves value
/// kinds and uses the language's Float comparison, including in generic facts.
pub trait LogicValue {
    fn logic_value(&self) -> Term;
}

macro_rules! scalar_logic_value {
    ($ty:ty, $variant:ident) => {
        impl LogicValue for $ty {
            fn logic_value(&self) -> Term {
                Term::$variant(self.clone())
            }
        }
    };
}

scalar_logic_value!(i64, Int);
scalar_logic_value!(f64, Float);
scalar_logic_value!(bool, Bool);
scalar_logic_value!(char, Char);
scalar_logic_value!(String, String);

impl LogicValue for str {
    fn logic_value(&self) -> Term {
        Term::String(self.to_owned())
    }
}

impl LogicValue for () {
    fn logic_value(&self) -> Term {
        Term::Unit
    }
}

impl<T: LogicValue + ?Sized> LogicValue for &T {
    fn logic_value(&self) -> Term {
        T::logic_value(self)
    }
}

impl Term {
    pub fn same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => (left - right).abs() < f64::EPSILON,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Char(left), Self::Char(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Unit, Self::Unit) => true,
            (Self::Compound(left, xs), Self::Compound(right, ys)) => {
                left == right
                    && xs.len() == ys.len()
                    && xs.iter().zip(ys).all(|(x, y)| x.same_value(y))
            }
            _ => false,
        }
    }

    fn renamed(&self, offset: usize) -> Self {
        match self {
            Self::Variable(index) => Self::Variable(index + offset),
            Self::Compound(name, fields) => Self::Compound(
                name.clone(),
                fields.iter().map(|field| field.renamed(offset)).collect(),
            ),
            value => value.clone(),
        }
    }

    fn variable_bound(&self) -> usize {
        match self {
            Self::Variable(index) => index + 1,
            Self::Compound(_, fields) => fields.iter().map(Self::variable_bound).max().unwrap_or(0),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Goal {
    Succeed,
    Fail,
    /// Named binders enumerate rows while unbound. Once they are all bound,
    /// this is a Boolean existence test, including anonymous-only calls.
    Call(String, Vec<Term>, Vec<Term>),
    All(Vec<Goal>),
    Any(Vec<Goal>),
    Not(Box<Goal>),
    /// Evaluate a compiled expression once, then unify its result.
    Evaluate {
        expression: usize,
        arguments: Vec<Term>,
        result: Term,
    },
}

impl Goal {
    fn renamed(&self, offset: usize) -> Self {
        match self {
            Self::Succeed => Self::Succeed,
            Self::Fail => Self::Fail,
            Self::Call(name, arguments, binders) => Self::Call(
                name.clone(),
                arguments.iter().map(|term| term.renamed(offset)).collect(),
                binders.iter().map(|term| term.renamed(offset)).collect(),
            ),
            Self::All(goals) => Self::All(goals.iter().map(|goal| goal.renamed(offset)).collect()),
            Self::Any(goals) => Self::Any(goals.iter().map(|goal| goal.renamed(offset)).collect()),
            Self::Not(goal) => Self::Not(Box::new(goal.renamed(offset))),
            Self::Evaluate {
                expression,
                arguments,
                result,
            } => Self::Evaluate {
                expression: *expression,
                arguments: arguments.iter().map(|term| term.renamed(offset)).collect(),
                result: result.renamed(offset),
            },
        }
    }

    fn variable_bound(&self) -> usize {
        match self {
            Self::Succeed | Self::Fail => 0,
            Self::Call(_, arguments, _) => arguments
                .iter()
                .map(Term::variable_bound)
                .max()
                .unwrap_or(0),
            Self::All(goals) | Self::Any(goals) => {
                goals.iter().map(Self::variable_bound).max().unwrap_or(0)
            }
            Self::Not(goal) => goal.variable_bound(),
            Self::Evaluate {
                arguments, result, ..
            } => arguments
                .iter()
                .map(Term::variable_bound)
                .chain(std::iter::once(result.variable_bound()))
                .max()
                .unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Clause {
    pub head: Vec<Term>,
    pub body: Goal,
}

#[derive(Clone, Debug)]
pub struct RuleValue {
    pub setup: Goal,
    pub result: Term,
}

impl RuleValue {
    fn renamed(&self, offset: usize) -> Self {
        Self {
            setup: self.setup.renamed(offset),
            result: self.result.renamed(offset),
        }
    }

    fn variable_bound(&self) -> usize {
        self.setup
            .variable_bound()
            .max(self.result.variable_bound())
    }
}

#[derive(Clone, Debug)]
pub struct Override {
    pub head: Vec<Term>,
    pub condition: Goal,
    pub value: RuleValue,
}

#[derive(Default)]
pub struct Relation {
    pub clauses: Vec<Clause>,
    /// Exceptions first, then guarded defaults; source order within each tier.
    pub overrides: Vec<Override>,
    pub defaults: Vec<Override>,
}

#[derive(Default)]
pub struct Program {
    pub relations: BTreeMap<(String, usize), Relation>,
}

struct PriorityDecision {
    arguments: Vec<Term>,
    checked: usize,
    selected: Option<bool>,
}

struct Priority<'a> {
    overrides: &'a [Override],
    decisions: Vec<PriorityDecision>,
}

impl Priority<'_> {
    fn decision(&mut self, arguments: &[Term]) -> usize {
        if let Some(index) = self.decisions.iter().position(|decision| {
            decision.arguments.len() == arguments.len()
                && decision
                    .arguments
                    .iter()
                    .zip(arguments)
                    .all(|(a, b)| a.same_value(b))
        }) {
            return index;
        }
        self.decisions.push(PriorityDecision {
            arguments: arguments.to_vec(),
            checked: 0,
            selected: None,
        });
        self.decisions.len() - 1
    }
}

/// One freshened candidate and its progress through a generating guard.
#[derive(Clone, Copy)]
struct Candidate<'a> {
    head: &'a [Term],
    override_position: Option<usize>,
    value: Option<&'a RuleValue>,
    condition_started: bool,
}

/// One binding environment shared by the whole search. Every binding is
/// recorded on a trail; a search step marks the trail before it binds and
/// undoes back to the mark when it backtracks, so extending the environment
/// costs time proportional to the new bindings rather than to the old ones.
#[derive(Default)]
struct Bindings {
    /// Indexed by variable; `None` while the variable is unbound.
    values: Vec<Option<Term>>,
    trail: Vec<usize>,
}

impl Bindings {
    fn mark(&self) -> usize {
        self.trail.len()
    }

    fn undo(&mut self, mark: usize) {
        for index in self.trail.drain(mark..) {
            self.values[index] = None;
        }
    }

    fn resolve(&self, term: &Term) -> Term {
        match term {
            Term::Variable(index) => self
                .values
                .get(*index)
                .and_then(Option::as_ref)
                .map_or_else(|| term.clone(), |value| self.resolve(value)),
            Term::Compound(name, fields) => Term::Compound(
                name.clone(),
                fields.iter().map(|field| self.resolve(field)).collect(),
            ),
            value => value.clone(),
        }
    }

    fn occurs(index: usize, term: &Term) -> bool {
        match term {
            Term::Variable(other) => index == *other,
            Term::Compound(_, fields) => fields.iter().any(|field| Self::occurs(index, field)),
            _ => false,
        }
    }

    fn unify(&mut self, left: &Term, right: &Term) -> bool {
        let left = self.resolve(left);
        let right = self.resolve(right);
        match (&left, &right) {
            (Term::Variable(left), Term::Variable(right)) if left == right => true,
            (Term::Variable(index), value) | (value, Term::Variable(index)) => {
                if Self::occurs(*index, value) {
                    return false;
                }
                if self.values.len() <= *index {
                    self.values.resize(*index + 1, None);
                }
                self.values[*index] = Some(value.clone());
                self.trail.push(*index);
                true
            }
            (Term::Compound(left, xs), Term::Compound(right, ys)) => {
                left == right
                    && xs.len() == ys.len()
                    && xs.iter().zip(ys).all(|(x, y)| self.unify(x, y))
            }
            _ => left.same_value(&right),
        }
    }

    fn ground(&self, term: &Term) -> Result<Term, String> {
        let value = self.resolve(term);
        if value.variable_bound() != 0 {
            return Err("logic expression needs a bound value; evaluation is incomplete".into());
        }
        Ok(value)
    }
}

pub struct Search<'program, E> {
    program: &'program Program,
    evaluate: E,
    next_variable: usize,
    bindings: Bindings,
}

type Visitor<'a, S> = dyn FnMut(&mut S) -> Result<bool, String> + 'a;

impl<'program, E> Search<'program, E>
where
    E: FnMut(usize, &[Term]) -> Result<Term, String>,
{
    pub fn new(program: &'program Program, evaluate: E) -> Self {
        Self {
            program,
            evaluate,
            next_variable: 0,
            bindings: Bindings::default(),
        }
    }

    pub fn findall(&mut self, goal: &Goal, projection: &Term) -> Result<Vec<Term>, String> {
        self.next_variable = goal.variable_bound().max(projection.variable_bound());
        self.bindings = Bindings::default();
        let mut values: Vec<Term> = Vec::new();
        self.visit(goal, EVALUATION_DEPTH.get(), &mut |search| {
            let value = search.bindings.ground(projection)?;
            if !values.iter().any(|previous| previous.same_value(&value)) {
                values.push(value);
            }
            Ok(false)
        })?;
        Ok(values)
    }

    pub fn exists(&mut self, goal: &Goal) -> Result<bool, String> {
        self.next_variable = goal.variable_bound();
        self.bindings = Bindings::default();
        self.visit(goal, EVALUATION_DEPTH.get(), &mut |_| Ok(true))
    }

    /// Run `step` and then drop every binding it made, on success and failure.
    fn scoped<T>(&mut self, step: impl FnOnce(&mut Self) -> T) -> T {
        let mark = self.bindings.mark();
        let result = step(self);
        self.bindings.undo(mark);
        result
    }

    /// Visit every solution of `goal` under the current bindings. The
    /// bindings are the same on return as on entry.
    fn visit(
        &mut self,
        goal: &Goal,
        depth: usize,
        visitor: &mut Visitor<'_, Self>,
    ) -> Result<bool, String> {
        match goal {
            Goal::Succeed => visitor(self),
            Goal::Fail => Ok(false),
            Goal::All(goals) => self.visit_all(goals, depth, visitor),
            Goal::Any(goals) => {
                for goal in goals {
                    if self.visit(goal, depth, visitor)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Goal::Not(goal) => {
                if self.visit(goal, depth, &mut |_| Ok(true))? {
                    Ok(false)
                } else {
                    visitor(self)
                }
            }
            Goal::Evaluate {
                expression,
                arguments,
                result,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|term| self.bindings.ground(term))
                    .collect::<Result<Vec<_>, _>>()?;
                // A rule called from this expression runs its body one level deeper.
                let outer = EVALUATION_DEPTH.replace(depth + 1);
                let value = (self.evaluate)(*expression, &arguments);
                EVALUATION_DEPTH.set(outer);
                let value = value?;
                if value.variable_bound() != 0 {
                    return Err("compiled logic expression returned an unbound value".into());
                }
                self.scoped(|search| {
                    if search.bindings.unify(result, &value) {
                        visitor(search)
                    } else {
                        Ok(false)
                    }
                })
            }
            Goal::Call(name, arguments, binders) => {
                if depth > LOGIC_QUERY_DEPTH_LIMIT {
                    return Err(logic_query_depth_error(name));
                }
                let relation = self
                    .program
                    .relations
                    .get(&(name.clone(), arguments.len()))
                    .ok_or_else(|| {
                        format!(
                            "logic query `{}({})` has no generated relation",
                            name,
                            arguments.len()
                        )
                    })?;
                let enumerate = binders
                    .iter()
                    .any(|term| self.bindings.ground(term).is_err());
                let mut seen: Vec<Vec<Term>> = Vec::new();
                let mut priority = Priority {
                    overrides: &relation.overrides,
                    decisions: Vec::new(),
                };
                let candidates = relation
                    .clauses
                    .iter()
                    .map(|clause| (&clause.head, &clause.body, None, None))
                    .chain(relation.overrides.iter().enumerate().map(|(index, rule)| {
                        (&rule.head, &rule.condition, Some(&rule.value), Some(index))
                    }))
                    .chain(
                        relation
                            .defaults
                            .iter()
                            .map(|rule| (&rule.head, &rule.condition, Some(&rule.value), None)),
                    );
                for (head, body, value, override_position) in candidates {
                    let variable_count = head
                        .iter()
                        .map(Term::variable_bound)
                        .chain(std::iter::once(body.variable_bound()))
                        .chain(value.map(RuleValue::variable_bound))
                        .max()
                        .unwrap_or(0);
                    let offset = self.reserve_variables(variable_count)?;
                    let head: Cow<'_, [Term]> = if variable_count == 0 {
                        Cow::Borrowed(head)
                    } else {
                        Cow::Owned(head.iter().map(|term| term.renamed(offset)).collect())
                    };
                    let found = self.scoped(|search| {
                        if !head
                            .iter()
                            .zip(arguments)
                            .all(|(head, value)| search.bindings.unify(head, value))
                        {
                            return Ok(false);
                        }
                        let body = body.renamed(offset);
                        let value = value.map(|value| value.renamed(offset));
                        let mut accepted = |search: &mut Self| {
                            if !enumerate {
                                return Ok(true);
                            }
                            let values = binders
                                .iter()
                                .map(|term| search.bindings.ground(term))
                                .collect::<Result<Vec<_>, _>>()?;
                            if seen.iter().any(|previous| {
                                previous
                                    .iter()
                                    .zip(&values)
                                    .all(|(left, right)| left.same_value(right))
                            }) {
                                return Ok(false);
                            }
                            seen.push(values);
                            visitor(search)
                        };
                        if relation.overrides.is_empty() && value.is_none() {
                            search.visit(&body, depth + 1, &mut accepted)
                        } else {
                            search.visit_priority_goals(
                                std::slice::from_ref(&body),
                                Candidate {
                                    head: &head,
                                    override_position,
                                    value: value.as_ref(),
                                    condition_started: false,
                                },
                                depth + 1,
                                &mut priority,
                                &mut accepted,
                            )
                        }
                    })?;
                    // Nothing outside this candidate refers to its variables.
                    self.next_variable = offset;
                    if found {
                        return if enumerate { Ok(true) } else { visitor(self) };
                    }
                }
                Ok(false)
            }
        }
    }

    fn reserve_variables(&mut self, count: usize) -> Result<usize, String> {
        let offset = self.next_variable;
        self.next_variable = offset
            .checked_add(count)
            .ok_or_else(|| "logic query exhausted its variable space".to_string())?;
        Ok(offset)
    }

    fn rule_value(&mut self, value: &RuleValue, depth: usize) -> Result<bool, String> {
        let mut result = None;
        self.visit(&value.setup, depth, &mut |search| {
            result = Some(search.bindings.ground(&value.result)?);
            Ok(true)
        })?;
        match result {
            Some(Term::Bool(value)) => Ok(value),
            _ => Err("logic override did not produce a Boolean value".into()),
        }
    }

    /// Resolve only higher-priority candidates, never re-evaluate a clause
    /// that has already produced bindings (and may have performed effects).
    /// `arguments` are ground and each rule is renamed apart, so checking a
    /// rule reads none of the caller's bindings.
    fn resolve_priority(
        &mut self,
        priority: &mut Priority<'_>,
        arguments: &[Term],
        through: usize,
        depth: usize,
    ) -> Result<usize, String> {
        let index = priority.decision(arguments);
        while priority.decisions[index].selected.is_none()
            && priority.decisions[index].checked < through
        {
            let position = priority.decisions[index].checked;
            let rule = &priority.overrides[position];
            let count = rule
                .head
                .iter()
                .map(Term::variable_bound)
                .chain([rule.condition.variable_bound(), rule.value.variable_bound()])
                .max()
                .unwrap_or(0);
            let offset = self.reserve_variables(count)?;
            let selected = self.scoped(|search| {
                if rule
                    .head
                    .iter()
                    .zip(arguments)
                    .all(|(head, value)| search.bindings.unify(&head.renamed(offset), value))
                    && search.visit(&rule.condition.renamed(offset), depth, &mut |_| Ok(true))?
                {
                    Ok(Some(search.rule_value(&rule.value.renamed(offset), depth)?))
                } else {
                    Ok::<_, String>(None)
                }
            })?;
            if selected.is_some() {
                priority.decisions[index].selected = selected;
            }
            priority.decisions[index].checked += 1;
        }
        Ok(index)
    }

    fn visit_priority_goals(
        &mut self,
        goals: &[Goal],
        candidate: Candidate<'_>,
        depth: usize,
        priority: &mut Priority<'_>,
        visitor: &mut Visitor<'_, Self>,
    ) -> Result<bool, String> {
        let Candidate {
            head,
            override_position,
            value,
            condition_started,
        } = candidate;
        let arguments = (!priority.overrides.is_empty())
            .then(|| {
                head.iter()
                    .map(|term| self.bindings.ground(term))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            })
            .flatten();
        if let Some(arguments) = &arguments {
            let through = override_position.map_or(priority.overrides.len(), |position| {
                position + usize::from(!condition_started)
            });
            let index = self.resolve_priority(priority, arguments, through, depth)?;
            let decision = &priority.decisions[index];
            if let Some(selected) = decision.selected {
                return if selected { visitor(self) } else { Ok(false) };
            }
            if override_position.is_some_and(|position| decision.checked > position) {
                return Ok(false);
            }
        }
        let Some((first, rest)) = goals.split_first() else {
            if !priority.overrides.is_empty() && arguments.is_none() {
                if value.is_some_and(|value| {
                    matches!(self.bindings.resolve(&value.result), Term::Bool(false))
                }) {
                    return Ok(false);
                }
                return Err("logic query cannot resolve exception/default priority for an unbound head; evaluation is incomplete".into());
            }
            if let Some(value) = value {
                let selected = self.rule_value(value, depth)?;
                if let (Some(position), Some(arguments)) = (override_position, &arguments) {
                    let index = priority.decision(arguments);
                    priority.decisions[index].checked = position + 1;
                    priority.decisions[index].selected = Some(selected);
                }
                if !selected {
                    return Ok(false);
                }
            }
            return visitor(self);
        };
        match first {
            Goal::All(nested) => {
                let mut flattened = nested.clone();
                flattened.extend_from_slice(rest);
                return self.visit_priority_goals(&flattened, candidate, depth, priority, visitor);
            }
            Goal::Any(alternatives) => {
                for alternative in alternatives {
                    let mut branch = vec![alternative.clone()];
                    branch.extend_from_slice(rest);
                    if self.visit_priority_goals(&branch, candidate, depth, priority, visitor)? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            Goal::Evaluate {
                arguments: inputs, ..
            } if override_position.is_some()
                && arguments.is_none()
                && inputs
                    .iter()
                    .any(|term| self.bindings.ground(term).is_err()) =>
            {
                if value.is_some_and(|value| {
                    matches!(self.bindings.resolve(&value.result), Term::Bool(false))
                }) {
                    return Ok(false);
                }
                return Err("logic query cannot enumerate an unbound override predicate; evaluation is incomplete".into());
            }
            _ => {}
        }
        self.visit(first, depth, &mut |search| {
            search.visit_priority_goals(
                rest,
                Candidate {
                    condition_started: true,
                    ..candidate
                },
                depth,
                priority,
                visitor,
            )
        })
    }

    fn visit_all(
        &mut self,
        goals: &[Goal],
        depth: usize,
        visitor: &mut Visitor<'_, Self>,
    ) -> Result<bool, String> {
        let Some((first, rest)) = goals.split_first() else {
            return visitor(self);
        };
        self.visit(first, depth, &mut |search| {
            search.visit_all(rest, depth, visitor)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: Vec<Term>) -> Goal {
        Goal::Call(name.into(), arguments.clone(), arguments)
    }

    fn add(program: &mut Program, name: &str, head: Vec<Term>, body: Goal) {
        program
            .relations
            .entry((name.into(), head.len()))
            .or_default()
            .clauses
            .push(Clause { head, body });
    }

    fn no_expressions(_: usize, _: &[Term]) -> Result<Term, String> {
        Err("unexpected expression".into())
    }

    #[test]
    fn exclusions_run_before_remaining_body_effects_and_once_per_argument_row() {
        let mut program = Program::default();
        for value in [1, 1, 2] {
            add(&mut program, "item", vec![Term::Int(value)], Goal::Succeed);
        }
        let variable = Term::Variable(0);
        add(
            &mut program,
            "selected",
            vec![variable.clone()],
            Goal::All(vec![
                call("item", vec![variable.clone()]),
                Goal::Evaluate {
                    expression: 1,
                    arguments: vec![variable.clone()],
                    result: Term::Bool(true),
                },
            ]),
        );
        program
            .relations
            .get_mut(&("selected".into(), 1))
            .unwrap()
            .overrides
            .push(Override {
                head: vec![variable.clone()],
                condition: Goal::Evaluate {
                    expression: 0,
                    arguments: vec![variable.clone()],
                    result: Term::Bool(true),
                },
                value: RuleValue {
                    setup: Goal::Succeed,
                    result: Term::Bool(false),
                },
            });
        let mut effects = Vec::new();
        let values = Search::new(&program, |expression, arguments: &[Term]| {
            let Term::Int(value) = arguments[0] else {
                panic!("expected Int")
            };
            effects.push((expression, value));
            Ok(Term::Bool(expression == 1 || value == 1))
        })
        .findall(&call("selected", vec![variable.clone()]), &variable)
        .unwrap();
        assert_eq!(effects, [(0, 1), (0, 2), (1, 2)]);
        assert_eq!(values.len(), 1);
        assert!(values[0].same_value(&Term::Int(2)));
    }

    #[test]
    fn positive_overrides_generate_witnesses_and_cache_the_selected_value() {
        let mut program = Program::default();
        for value in [1, 2] {
            add(&mut program, "item", vec![Term::Int(value)], Goal::Succeed);
        }
        for _ in 0..2 {
            add(&mut program, "selected", vec![Term::Int(1)], Goal::Succeed);
        }
        let variable = Term::Variable(0);
        program
            .relations
            .get_mut(&("selected".into(), 1))
            .unwrap()
            .overrides
            .push(Override {
                head: vec![variable.clone()],
                condition: call("item", vec![variable.clone()]),
                value: RuleValue {
                    setup: Goal::Evaluate {
                        expression: 0,
                        arguments: vec![variable.clone()],
                        result: Term::Variable(1),
                    },
                    result: Term::Variable(1),
                },
            });
        let mut effects = Vec::new();
        let values = Search::new(&program, |_, arguments: &[Term]| {
            let Term::Int(value) = arguments[0] else {
                panic!("expected Int")
            };
            effects.push(value);
            Ok(Term::Bool(true))
        })
        .findall(&call("selected", vec![variable.clone()]), &variable)
        .unwrap();
        assert_eq!(effects, [1, 2]);
        assert_eq!(values.len(), 2);
        assert!(values[0].same_value(&Term::Int(1)));
        assert!(values[1].same_value(&Term::Int(2)));
    }

    #[test]
    fn derived_rows_preserve_correlations_and_project_distinct_values() {
        let mut program = Program::default();
        for row in [[1, 10], [2, 20], [2, 20]] {
            add(
                &mut program,
                "edge",
                row.into_iter().map(Term::Int).collect(),
                Goal::Succeed,
            );
        }
        add(
            &mut program,
            "row",
            vec![Term::Variable(0), Term::Variable(1)],
            call("edge", vec![Term::Variable(0), Term::Variable(1)]),
        );
        let mut search = Search::new(&program, no_expressions);
        let values = search
            .findall(
                &call("row", vec![Term::Variable(0), Term::Variable(1)]),
                &Term::Variable(1),
            )
            .unwrap();
        assert_eq!(values.len(), 2);
        assert!(values[0].same_value(&Term::Int(10)) && values[1].same_value(&Term::Int(20)));
        assert!(!search
            .exists(&call("row", vec![Term::Int(1), Term::Int(20)]))
            .unwrap());
    }

    #[test]
    fn negation_and_alternative_branches_do_not_leak_bindings() {
        let mut program = Program::default();
        add(&mut program, "item", vec![Term::Int(1)], Goal::Succeed);
        add(&mut program, "item", vec![Term::Int(2)], Goal::Succeed);
        add(&mut program, "hidden", vec![Term::Int(1)], Goal::Succeed);
        add(
            &mut program,
            "visible",
            vec![Term::Variable(0)],
            Goal::All(vec![
                call("item", vec![Term::Variable(0)]),
                Goal::Not(Box::new(call("hidden", vec![Term::Variable(0)]))),
            ]),
        );
        let mut search = Search::new(&program, no_expressions);
        let values = search
            .findall(
                &call("visible", vec![Term::Variable(0)]),
                &Term::Variable(0),
            )
            .unwrap();
        assert_eq!(values.len(), 1);
        assert!(values[0].same_value(&Term::Int(2)));
        let alternative = Goal::Any(vec![
            Goal::All(vec![call("hidden", vec![Term::Variable(0)]), Goal::Fail]),
            call("item", vec![Term::Variable(0)]),
        ]);
        assert_eq!(
            search
                .findall(&alternative, &Term::Variable(0))
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn repeated_arguments_are_constrained_before_effects() {
        let mut program = Program::default();
        add(
            &mut program,
            "pair",
            vec![Term::Int(1), Term::Int(2)],
            Goal::Evaluate {
                expression: 0,
                arguments: vec![],
                result: Term::Bool(true),
            },
        );
        let mut calls = 0;
        let mut search = Search::new(&program, |_, _: &[Term]| {
            calls += 1;
            Ok(Term::Bool(true))
        });
        assert!(!search
            .exists(&call("pair", vec![Term::Variable(0), Term::Variable(0)]))
            .unwrap());
        assert_eq!(calls, 0);
    }

    #[test]
    fn structural_unification_propagates_fields_and_rejects_cycles() {
        let mut bindings = Bindings::default();
        let pair = |left, right| Term::Compound("Pair".into(), vec![left, right]);
        assert!(bindings.unify(
            &pair(Term::Variable(0), Term::Int(2)),
            &pair(Term::Int(1), Term::Variable(1))
        ));
        assert!(bindings
            .ground(&Term::Variable(0))
            .unwrap()
            .same_value(&Term::Int(1)));
        assert!(bindings
            .ground(&Term::Variable(1))
            .unwrap()
            .same_value(&Term::Int(2)));
        assert!(!bindings.unify(&Term::Variable(2), &pair(Term::Variable(2), Term::Int(3))));
    }

    #[test]
    fn recursive_search_preserves_discovery_order_and_reports_incomplete_results() {
        let mut program = Program::default();
        for row in [[1, 2], [2, 3], [1, 4]] {
            add(
                &mut program,
                "parent",
                row.into_iter().map(Term::Int).collect(),
                Goal::Succeed,
            );
        }
        add(
            &mut program,
            "ancestor",
            vec![Term::Variable(0), Term::Variable(1)],
            call("parent", vec![Term::Variable(0), Term::Variable(1)]),
        );
        add(
            &mut program,
            "ancestor",
            vec![Term::Variable(0), Term::Variable(1)],
            Goal::All(vec![
                call("parent", vec![Term::Variable(0), Term::Variable(2)]),
                call("ancestor", vec![Term::Variable(2), Term::Variable(1)]),
            ]),
        );
        let mut search = Search::new(&program, no_expressions);
        let values = search
            .findall(
                &call("ancestor", vec![Term::Int(1), Term::Variable(0)]),
                &Term::Variable(0),
            )
            .unwrap();
        assert_eq!(values.len(), 3);
        for (actual, expected) in values.iter().zip([2, 4, 3]) {
            assert!(actual.same_value(&Term::Int(expected)));
        }
        add(
            &mut program,
            "cycle",
            vec![Term::Variable(0)],
            call("cycle", vec![Term::Variable(0)]),
        );
        // Compiled programs search on a 1 GiB worker stack; the cycle nests
        // goals up to the full depth limit.
        let error = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || {
                Search::new(&program, no_expressions)
                    .exists(&call("cycle", vec![Term::Int(1)]))
                    .unwrap_err()
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(error, logic_query_depth_error("cycle"));
    }

    #[test]
    fn existential_success_does_not_evaluate_later_recursive_clauses() {
        let mut program = Program::default();
        add(&mut program, "present", vec![Term::Int(1)], Goal::Succeed);
        add(
            &mut program,
            "present",
            vec![Term::Variable(0)],
            call("present", vec![Term::Variable(0)]),
        );
        assert!(Search::new(&program, no_expressions)
            .exists(&call("present", vec![Term::Variable(0)]))
            .unwrap());
    }
}
