//! Binding and backtracking runtime for generated relational queries.
//!
//! This module deliberately depends only on `std`: code generation can embed
//! it in an ordinary Rust executable without linking the compiler or parser.

use std::collections::BTreeMap;

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

#[derive(Default)]
pub struct Program {
    pub relations: BTreeMap<(String, usize), Vec<Clause>>,
}

#[derive(Clone, Default)]
struct Bindings(BTreeMap<usize, Term>);

impl Bindings {
    fn resolve(&self, term: &Term) -> Term {
        match term {
            Term::Variable(index) => self
                .0
                .get(index)
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
                self.0.insert(*index, value.clone());
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
}

impl<'program, E> Search<'program, E>
where
    E: FnMut(usize, &[Term]) -> Result<Term, String>,
{
    pub fn new(program: &'program Program, evaluate: E) -> Self {
        Self {
            program,
            evaluate,
            next_variable: 0,
        }
    }

    pub fn findall(&mut self, goal: &Goal, projection: &Term) -> Result<Vec<Term>, String> {
        self.next_variable = goal.variable_bound().max(projection.variable_bound());
        let mut values: Vec<Term> = Vec::new();
        self.visit(goal, &Bindings::default(), 0, &mut |_, bindings| {
            let value = bindings.ground(projection)?;
            if !values.iter().any(|previous| previous.same_value(&value)) {
                values.push(value);
            }
            Ok(false)
        })?;
        Ok(values)
    }

    pub fn exists(&mut self, goal: &Goal) -> Result<bool, String> {
        self.next_variable = goal.variable_bound();
        self.visit(goal, &Bindings::default(), 0, &mut |_, _| Ok(true))
    }

    fn visit(
        &mut self,
        goal: &Goal,
        bindings: &Bindings,
        depth: usize,
        visitor: &mut dyn FnMut(&mut Self, &Bindings) -> Result<bool, String>,
    ) -> Result<bool, String> {
        match goal {
            Goal::Succeed => visitor(self, bindings),
            Goal::Fail => Ok(false),
            Goal::All(goals) => self.visit_all(goals, bindings, depth, visitor),
            Goal::Any(goals) => {
                for goal in goals {
                    if self.visit(goal, bindings, depth, visitor)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Goal::Not(goal) => {
                if self.visit(goal, bindings, depth, &mut |_, _| Ok(true))? {
                    Ok(false)
                } else {
                    visitor(self, bindings)
                }
            }
            Goal::Evaluate {
                expression,
                arguments,
                result,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|term| bindings.ground(term))
                    .collect::<Result<Vec<_>, _>>()?;
                let value = (self.evaluate)(*expression, &arguments)?;
                if value.variable_bound() != 0 {
                    return Err("compiled logic expression returned an unbound value".into());
                }
                let mut next = bindings.clone();
                if next.unify(result, &value) {
                    visitor(self, &next)
                } else {
                    Ok(false)
                }
            }
            Goal::Call(name, arguments, binders) => {
                if depth > 50 {
                    return Err(format!("logic query `{name}` exceeded its recursion limit of 50; evaluation is incomplete"));
                }
                let clauses = self
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
                let enumerate = binders.iter().any(|term| bindings.ground(term).is_err());
                let mut seen: Vec<Vec<Term>> = Vec::new();
                for clause in clauses {
                    let variable_count = clause
                        .head
                        .iter()
                        .map(Term::variable_bound)
                        .chain(std::iter::once(clause.body.variable_bound()))
                        .max()
                        .unwrap_or(0);
                    let offset = self.next_variable;
                    self.next_variable = offset
                        .checked_add(variable_count)
                        .ok_or_else(|| "logic query exhausted its variable space".to_string())?;
                    let mut next = bindings.clone();
                    if !clause
                        .head
                        .iter()
                        .zip(arguments)
                        .all(|(head, value)| next.unify(&head.renamed(offset), value))
                    {
                        continue;
                    }
                    let body = clause.body.renamed(offset);
                    if !enumerate {
                        if self.visit(&body, &next, depth + 1, &mut |_, _| Ok(true))? {
                            return visitor(self, bindings);
                        }
                    } else if self.visit(&body, &next, depth + 1, &mut |search, next| {
                        let values = binders
                            .iter()
                            .map(|term| next.ground(term))
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
                        visitor(search, next)
                    })? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    fn visit_all(
        &mut self,
        goals: &[Goal],
        bindings: &Bindings,
        depth: usize,
        visitor: &mut dyn FnMut(&mut Self, &Bindings) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let Some((first, rest)) = goals.split_first() else {
            return visitor(self, bindings);
        };
        self.visit(first, bindings, depth, &mut |search, next| {
            search.visit_all(rest, next, depth, visitor)
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
            .push(Clause { head, body });
    }

    fn no_expressions(_: usize, _: &[Term]) -> Result<Term, String> {
        Err("unexpected expression".into())
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
        let error = Search::new(&program, no_expressions)
            .exists(&call("cycle", vec![Term::Int(1)]))
            .unwrap_err();
        assert!(error.contains("evaluation is incomplete"));
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
