//! Partial calculation cases.
//!
//! A partial case may leave input facts out. Each left-out fact decodes to
//! `Value::Unknown` carrying its concrete input path. Evaluation carries
//! unknown values freely and, where it reads one, either stops (strict mode,
//! the test oracle) or propagates an unknown result (the default). The case
//! then answers `Complete` with its result or `Needs` with the paths that block
//! it. The contract is `docs/rfcs/calculation-needs.md`.

use super::*;
use std::cell::Cell;

/// The input paths an unknown value depends on.
pub type UnknownPaths = Rc<BTreeSet<String>>;

/// Evaluation steps a case may spend on speculation: everything evaluated
/// only because a condition is unknown.
pub(crate) const SPECULATION_STEP_BUDGET: usize = 100_000;

thread_local! {
    static UNKNOWN_LEAK: Cell<Option<&'static str>> = const { Cell::new(None) };
    static CALCULATION_RUNTIME_NESTING: Cell<usize> = const { Cell::new(0) };
}

/// Enter a calculation evaluation; returns how many enclose it.
pub(crate) fn enter_calculation_runtime() -> usize {
    CALCULATION_RUNTIME_NESTING.with(|nesting| {
        let outer = nesting.get();
        nesting.set(outer + 1);
        outer
    })
}

pub(crate) fn exit_calculation_runtime() {
    CALCULATION_RUNTIME_NESTING.with(|nesting| nesting.set(nesting.get().saturating_sub(1)));
}

/// Record that an unknown value reached an operation that must never see one
/// (display, equality, hashing, ordering, encoding). The calculation runtime
/// turns the record into an internal error for the case, so the value never
/// shapes a result.
pub(crate) fn report_unknown_leak(site: &'static str) {
    UNKNOWN_LEAK.with(|leak| {
        if leak.get().is_none() {
            leak.set(Some(site));
        }
    });
}

/// Take the first recorded leak, clearing the record.
pub(crate) fn take_unknown_leak() -> Option<&'static str> {
    UNKNOWN_LEAK.with(|leak| leak.take())
}

pub(crate) fn unknown_leak_message(site: &str) -> String {
    format!("internal error: an unknown input value reached {site}")
}

/// How a partial case treats a read of an unknown value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialMode {
    /// Stop at the first read and report its paths.
    Strict,
    /// Propagate unknown results; decide wherever the known part decides.
    Propagate,
}

pub fn unknown(paths: BTreeSet<String>) -> Value {
    Value::Unknown(Rc::new(paths))
}

/// Per-case bookkeeping of a partial evaluation.
#[derive(Debug, Default)]
pub(crate) struct PartialState {
    /// The calculation entry, the waiting rule of a value read by no rule.
    pub(crate) entry: String,
    /// Rules and named functions being evaluated, innermost last; `true`
    /// marks a rule family.
    pub(crate) call_stack: Vec<(String, bool)>,
    /// The rule family that first read each unknown path.
    pub(crate) waiting: BTreeMap<String, String>,
    /// Paths whose unknown value fed an operation that can fail.
    pub(crate) fallible: BTreeSet<String>,
    /// Input records, variants and optionals by allocation, with their paths.
    pub(crate) containers: HashMap<usize, String>,
    /// Input paths read as one node (a scalar, `null`, a collection's length,
    /// a variant tag).
    pub(crate) read_exact: BTreeSet<String>,
    /// Input paths read with everything below them.
    pub(crate) read_subtree: BTreeSet<String>,
    /// Condition paths of each active speculation, innermost last.
    pub(crate) speculation: Vec<BTreeSet<String>>,
    pub(crate) speculation_steps: usize,
    pub(crate) speculation_exhausted: bool,
    /// While rule dispatch matches a clause head or a logic body, the paths
    /// of unknown values that make it a possible rather than a certain match.
    pub(crate) probe: Option<BTreeSet<String>>,
    /// How each active builtin, innermost last, reads its callbacks' results.
    pub(crate) callback_reads: Vec<CallbackRead>,
}

/// How a higher-order builtin reads what its callback returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallbackRead {
    /// Stored or passed on unread (`map`'s elements, `foldl`'s accumulator).
    Carries,
    /// Read as a structure (`flat_map` splices the returned list).
    Structure,
    /// Read whole (a predicate's Bool, a sort key).
    Whole,
}

fn callback_read(name: &str) -> CallbackRead {
    match name {
        "map" | "foldl" | "scan" | "reduce" | "subscribe" => CallbackRead::Carries,
        "flat_map" => CallbackRead::Structure,
        _ => CallbackRead::Whole,
    }
}

/// What a speculative evaluation produced.
pub(crate) enum Speculation<T> {
    Done(T),
    /// The evaluation failed; some completion of the condition fails.
    Failed,
    /// The speculation budget is spent or a recursive call was cut.
    Cut,
}

/// The allocation identity of an input container, if the value has one.
pub(crate) fn container_id(value: &Value) -> Option<usize> {
    match value {
        Value::Constructor(_, values)
        | Value::NamespacedConstructor {
            arguments: values, ..
        } => Some(Rc::as_ptr(values) as *const () as usize),
        Value::NamedConstructor(_, fields) | Value::NamespacedNamedConstructor { fields, .. } => {
            Some(Rc::as_ptr(fields) as *const () as usize)
        }
        Value::RuleScopeInstance { bindings, .. }
        | Value::NamespacedRuleScopeInstance { bindings, .. } => {
            Some(Rc::as_ptr(bindings) as *const () as usize)
        }
        _ => None,
    }
}

/// Collect the paths of unknown values at the top of `value` or, when `deep`,
/// anywhere inside it.
pub(crate) fn collect_unknown_paths(value: &Value, deep: bool, paths: &mut BTreeSet<String>) {
    match value {
        Value::Unknown(unknown) => paths.extend(unknown.iter().cloned()),
        _ if !deep => {}
        Value::List(items) | Value::Tuple(items) | Value::Stream(items) => {
            for item in items {
                collect_unknown_paths(item, true, paths);
            }
        }
        Value::Constructor(_, items)
        | Value::NamespacedConstructor {
            arguments: items, ..
        } => {
            for item in items.iter() {
                collect_unknown_paths(item, true, paths);
            }
        }
        Value::NamedConstructor(_, fields) | Value::NamespacedNamedConstructor { fields, .. } => {
            for (_, item) in fields.iter() {
                collect_unknown_paths(item, true, paths);
            }
        }
        Value::Map(entries) | Value::Set(entries) => {
            for item in entries.values() {
                collect_unknown_paths(item, true, paths);
            }
        }
        Value::RuleScopeInstance { bindings, .. }
        | Value::NamespacedRuleScopeInstance { bindings, .. } => {
            for item in bindings.values() {
                collect_unknown_paths(item, true, paths);
            }
        }
        _ => {}
    }
}

pub(crate) fn unknown_paths(value: &Value, deep: bool) -> Option<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    collect_unknown_paths(value, deep, &mut paths);
    (!paths.is_empty()).then_some(paths)
}

pub(crate) fn has_unknown(value: &Value) -> bool {
    match value {
        Value::Unknown(_) => true,
        Value::List(items) | Value::Tuple(items) | Value::Stream(items) => {
            items.iter().any(has_unknown)
        }
        Value::Constructor(_, items)
        | Value::NamespacedConstructor {
            arguments: items, ..
        } => items.iter().any(has_unknown),
        Value::NamedConstructor(_, fields) | Value::NamespacedNamedConstructor { fields, .. } => {
            fields.iter().any(|(_, item)| has_unknown(item))
        }
        Value::Map(entries) | Value::Set(entries) => entries.values().any(has_unknown),
        Value::RuleScopeInstance { bindings, .. }
        | Value::NamespacedRuleScopeInstance { bindings, .. } => bindings.values().any(has_unknown),
        _ => false,
    }
}

/// Values that decide a branching evaluation: plain data whose equality
/// means the same result.
fn decidable_data(value: &Value) -> bool {
    match value {
        Value::Int(_)
        | Value::Float(_)
        | Value::Str(_)
        | Value::Char(_)
        | Value::Bool(_)
        | Value::Unit => true,
        Value::List(items) | Value::Tuple(items) => items.iter().all(decidable_data),
        Value::Constructor(_, items) => items.iter().all(decidable_data),
        Value::NamedConstructor(_, fields) => fields.iter().all(|(_, item)| decidable_data(item)),
        Value::Map(entries) | Value::Set(entries) => entries.values().all(decidable_data),
        _ => false,
    }
}

/// The one value every possible outcome gives, when they are all the same
/// known data.
pub(crate) fn agreed_value(outcomes: &[Value]) -> Option<Value> {
    let first = outcomes.first()?;
    if !decidable_data(first) {
        return None;
    }
    outcomes[1..]
        .iter()
        .all(|other| decidable_data(other) && values_equal(first, other))
        .then(|| first.clone())
}

fn path_covers(prefix: &str, path: &str) -> bool {
    prefix.is_empty()
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

impl PartialState {
    pub(crate) fn reset(&mut self, entry: &str) {
        *self = PartialState {
            entry: entry.to_string(),
            ..PartialState::default()
        };
    }

    fn waiting_rule(&self) -> String {
        self.call_stack
            .iter()
            .rev()
            .find(|(_, is_rule)| *is_rule)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| self.entry.clone())
    }

    pub(crate) fn record_waiting(&mut self, paths: &BTreeSet<String>) {
        let rule = self.waiting_rule();
        for path in paths {
            self.waiting
                .entry(path.clone())
                .or_insert_with(|| rule.clone());
        }
    }

    pub(crate) fn path_read(&self, path: &str) -> bool {
        self.read_exact.contains(path)
            || self
                .read_subtree
                .iter()
                .any(|prefix| path_covers(prefix, path))
    }

    /// Mark the input leaves of `value`, found at `path`, as read: every
    /// scalar, `null`, collection and variant tag, down to the next tracked
    /// input container (whose own fields are read when accessed).
    fn mark_value_paths(&mut self, value: &Value, path: &str, root: bool) {
        if !root {
            if let Some(id) = container_id(value) {
                if self.containers.contains_key(&id) {
                    return;
                }
            }
        }
        self.read_exact.insert(path.to_string());
        match value {
            Value::List(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.mark_value_paths(item, &format!("{path}[{index}]"), false);
                }
            }
            Value::Set(_) => {
                self.read_subtree.insert(path.to_string());
            }
            Value::Map(entries) => {
                for (stored_key, stored_value) in entries {
                    match runtime_map_string_entry(stored_key, stored_value) {
                        Some((key, item)) => {
                            self.mark_value_paths(item, &join_path(path, key), false)
                        }
                        None => {
                            self.read_subtree.insert(path.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Interpreter {
    pub(crate) fn partial_propagates(&self) -> bool {
        self.partial_mode == Some(PartialMode::Propagate)
    }

    /// Begin a partial case: fresh bookkeeping for `entry`.
    pub(crate) fn begin_partial_case(&mut self, mode: Option<PartialMode>, entry: &str) {
        self.partial_mode = mode;
        self.partial.borrow_mut().reset(entry);
    }

    /// Register an input container and its path for read tracking.
    pub(crate) fn register_partial_input(&self, value: &Value, path: &str) {
        if let Some(id) = container_id(value) {
            self.partial
                .borrow_mut()
                .containers
                .insert(id, path.to_string());
        }
    }

    fn tracked_path(&self, value: &Value) -> Option<String> {
        let id = container_id(value)?;
        self.partial.borrow().containers.get(&id).cloned()
    }

    /// Record that a value is consumed: as a whole when `deep`, otherwise
    /// only its own node (a variant tag, presence, a collection's length).
    pub(crate) fn partial_consume(&self, value: &Value, deep: bool) {
        if self.partial_mode.is_none() {
            return;
        }
        match self.tracked_path(value) {
            Some(path) => {
                let mut state = self.partial.borrow_mut();
                if deep {
                    state.read_subtree.insert(path);
                } else {
                    state.read_exact.insert(join_path(&path, "$variant"));
                    state.read_exact.insert(path);
                }
            }
            None if deep => match value {
                Value::List(items) | Value::Tuple(items) => {
                    for item in items {
                        self.partial_consume(item, true);
                    }
                }
                Value::Constructor(_, items) => {
                    for item in items.iter() {
                        self.partial_consume(item, true);
                    }
                }
                Value::NamedConstructor(_, fields) => {
                    for (_, item) in fields.iter() {
                        self.partial_consume(item, true);
                    }
                }
                Value::Map(entries) | Value::Set(entries) => {
                    for item in entries.values() {
                        self.partial_consume(item, true);
                    }
                }
                _ => {}
            },
            None => {}
        }
    }

    /// Record the read of `field` from `receiver`, if the receiver is an
    /// input container.
    pub(crate) fn partial_field_read(&self, receiver: &Value, field: &str, value: &Value) {
        if self.partial_mode.is_none() {
            return;
        }
        if let Some(path) = self.tracked_path(receiver) {
            let field_path = join_path(&path, field);
            self.partial
                .borrow_mut()
                .mark_value_paths(value, &field_path, false);
        }
    }

    /// A site that needs `values` known. `None` lets the site proceed. When a
    /// value is unknown, strict mode stops the case and propagation gives the
    /// site an unknown result; either way the caller returns the given value.
    pub(crate) fn partial_need(&self, values: &[&Value], deep: bool) -> Option<Value> {
        self.partial_mode?;
        let mut paths = BTreeSet::new();
        for value in values {
            self.partial_consume(value, deep);
            collect_unknown_paths(value, deep, &mut paths);
        }
        if paths.is_empty() {
            return None;
        }
        Some(self.partial_unknown_read(paths))
    }

    /// The outcome of reading unknown `paths`: a strict stop, or an unknown
    /// value carrying them.
    /// The result of a branching site that did not decide: unknown with
    /// `condition` plus the paths of the outcomes it could take.
    pub(crate) fn branching_unknown(
        &self,
        mut condition: BTreeSet<String>,
        outcomes: &[Value],
    ) -> Value {
        for outcome in outcomes {
            collect_unknown_paths(outcome, true, &mut condition);
        }
        self.partial_unknown_read(condition)
    }

    /// `left op right` (`&&` or `||`) where `left` is unknown with `paths`.
    pub(crate) fn partial_short_circuit(
        &mut self,
        operator: &str,
        paths: BTreeSet<String>,
        right: &Expr,
        env: &Env,
    ) -> Value {
        if self.partial_mode == Some(PartialMode::Strict) {
            return self.partial_unknown_read(paths);
        }
        match self.speculate(&paths, |interpreter| interpreter.eval(right, env)) {
            Speculation::Done(Value::Bool(false)) if operator == "&&" => Value::Bool(false),
            Speculation::Done(Value::Bool(true)) if operator == "||" => Value::Bool(true),
            Speculation::Done(right) => self.branching_unknown(paths, &[right]),
            Speculation::Failed => self.speculation_unresolved(paths, true),
            Speculation::Cut => self.speculation_unresolved(paths, false),
        }
    }

    /// Conjunction (`all`) or disjunction of goals in Kleene logic: a false
    /// conjunct (a true alternative) decides even past unknown ones.
    pub(crate) fn partial_kleene(&mut self, items: &[Expr], env: &Env, all: bool) -> Value {
        let mut condition = BTreeSet::new();
        for item in items {
            let value = if condition.is_empty() {
                self.eval(item, env)
            } else {
                match self.speculate(&condition, |interpreter| interpreter.eval(item, env)) {
                    Speculation::Done(value) => value,
                    Speculation::Failed => return self.speculation_unresolved(condition, true),
                    Speculation::Cut => return self.speculation_unresolved(condition, false),
                }
            };
            if self.calculation_failed() {
                return Value::Unit;
            }
            match value {
                Value::Unknown(paths) => {
                    if self.partial_mode == Some(PartialMode::Strict) {
                        return self.partial_unknown_read((*paths).clone());
                    }
                    condition.extend(paths.iter().cloned());
                }
                Value::Bool(holds) if holds != all => return Value::Bool(holds),
                Value::Bool(_) => {}
                other if all => {
                    let _ = other;
                }
                other => {
                    if condition.is_empty() {
                        return other;
                    }
                    return self.branching_unknown(condition, &[other]);
                }
            }
        }
        if condition.is_empty() {
            Value::Bool(all)
        } else {
            self.partial_unknown_read(condition)
        }
    }

    /// A binary operator reads both operands whole. A division or remainder
    /// by an unknown divisor can fail for some completion.
    pub(crate) fn partial_binop(&self, op: &str, left: &Value, right: &Value) -> Option<Value> {
        let result = self.partial_need(&[left, right], true)?;
        if matches!(op, "/" | "%") {
            if let Some(paths) = unknown_paths(right, true) {
                self.partial_fallible(&paths);
            }
        }
        Some(result)
    }

    pub(crate) fn partial_unknown_read(&self, paths: BTreeSet<String>) -> Value {
        self.partial.borrow_mut().record_waiting(&paths);
        match self.partial_mode {
            Some(PartialMode::Strict) => {
                let mut error = self.calculation_error.borrow_mut();
                if error.is_none() {
                    *error = Some(CalculationRuntimeFailure::NeedsInput(paths));
                }
                Value::Unit
            }
            _ => unknown(paths),
        }
    }
    /// Stop the case with a need for `paths`, in either mode: used where an
    /// unknown value reaches a site with no propagation rule.
    pub(crate) fn partial_stop(&self, paths: BTreeSet<String>) {
        self.partial.borrow_mut().record_waiting(&paths);
        let mut error = self.calculation_error.borrow_mut();
        if error.is_none() {
            *error = Some(CalculationRuntimeFailure::NeedsInput(paths));
        }
    }

    /// An unknown value meets a refutable clause-head pattern or logic goal.
    /// Inside a propagating dispatch the clause becomes a possible match and
    /// this returns `true`; anywhere else the case stops and this returns
    /// `false`.
    pub(crate) fn partial_possible_match(&self, paths: &BTreeSet<String>) -> bool {
        {
            let mut state = self.partial.borrow_mut();
            if let Some(probe) = state.probe.as_mut() {
                probe.extend(paths.iter().cloned());
                state.record_waiting(paths);
                return true;
            }
        }
        self.partial_stop(paths.clone());
        false
    }

    /// Run `operation` collecting possible-match paths; returns them too.
    pub(crate) fn with_partial_probe<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> (T, BTreeSet<String>) {
        let previous = self.partial.borrow_mut().probe.replace(BTreeSet::new());
        let value = operation(self);
        let probed =
            std::mem::replace(&mut self.partial.borrow_mut().probe, previous).unwrap_or_default();
        (value, probed)
    }

    /// Record that `paths` fed an operation that can fail for some
    /// completion.
    pub(crate) fn partial_fallible(&self, paths: &BTreeSet<String>) {
        let mut state = self.partial.borrow_mut();
        state.record_waiting(paths);
        state.fallible.extend(paths.iter().cloned());
    }

    /// Paths of every active speculation's condition.
    pub(crate) fn speculation_paths(&self) -> BTreeSet<String> {
        self.partial
            .borrow()
            .speculation
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// Count one evaluation step against the speculation budget. Returns the
    /// value a speculative evaluation gives once the budget is spent.
    pub(crate) fn partial_speculation_step(&self) -> Option<Value> {
        let mut state = self.partial.borrow_mut();
        if state.speculation.is_empty() {
            return None;
        }
        if !state.speculation_exhausted {
            state.speculation_steps += 1;
            if state.speculation_steps <= SPECULATION_STEP_BUDGET {
                return None;
            }
            state.speculation_exhausted = true;
        }
        let paths = state.speculation.iter().flatten().cloned().collect();
        Some(unknown(paths))
    }

    /// Evaluate `operation` only because a condition with `condition` paths
    /// is unknown.
    pub(crate) fn speculate<T>(
        &mut self,
        condition: &BTreeSet<String>,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> Speculation<T> {
        if self.partial.borrow().speculation_exhausted {
            return Speculation::Cut;
        }
        self.partial
            .borrow_mut()
            .speculation
            .push(condition.clone());
        let value = operation(self);
        let exhausted = {
            let mut state = self.partial.borrow_mut();
            state.speculation.pop();
            state.speculation_exhausted
        };
        let failed = matches!(
            &*self.calculation_error.borrow(),
            Some(CalculationRuntimeFailure::InvalidValue(_))
        );
        if failed {
            *self.calculation_error.borrow_mut() = None;
            return Speculation::Failed;
        }
        if exhausted {
            return Speculation::Cut;
        }
        Speculation::Done(value)
    }

    /// The unknown result of a branching site whose condition paths are
    /// `condition`, after its speculation failed or was cut.
    pub(crate) fn speculation_unresolved(
        &self,
        condition: BTreeSet<String>,
        failed: bool,
    ) -> Value {
        if failed {
            self.partial_fallible(&condition);
        }
        self.partial_unknown_read(condition)
    }

    /// Enter a rule family or named function. Returns the call's value when
    /// a recursive call under speculation is cut instead of evaluated.
    pub(crate) fn partial_enter_call(&self, name: &str, is_rule: bool) -> Result<(), Value> {
        let mut state = self.partial.borrow_mut();
        if !state.speculation.is_empty()
            && state.call_stack.iter().any(|(active, _)| active == name)
        {
            let paths: BTreeSet<String> = state.speculation.iter().flatten().cloned().collect();
            state.record_waiting(&paths);
            return Err(unknown(paths));
        }
        state.call_stack.push((name.to_string(), is_rule));
        Ok(())
    }

    pub(crate) fn partial_exit_call(&self) {
        self.partial.borrow_mut().call_stack.pop();
    }
}

/// The result of one partial case after evaluation.
#[derive(Debug)]
pub(crate) enum PartialOutcome {
    Complete {
        result: Value,
        read_exact: BTreeSet<String>,
        read_subtree: BTreeSet<String>,
    },
    Needs {
        paths: BTreeSet<String>,
        waiting: BTreeMap<String, String>,
    },
}

impl Interpreter {
    /// Classify the value a partial case produced.
    pub(crate) fn finish_partial_case(&self, result: Value) -> PartialOutcome {
        let mut state = self.partial.borrow_mut();
        let mut paths = BTreeSet::new();
        collect_unknown_paths(&result, true, &mut paths);
        if paths.is_empty() && state.fallible.is_empty() {
            drop(state);
            // The output is read as a whole when it is encoded.
            self.partial_consume(&result, true);
            let state = self.partial.borrow();
            return PartialOutcome::Complete {
                result,
                read_exact: state.read_exact.clone(),
                read_subtree: state.read_subtree.clone(),
            };
        }
        paths.extend(state.fallible.iter().cloned());
        state.record_waiting(&paths);
        PartialOutcome::Needs {
            paths,
            waiting: state.waiting.clone(),
        }
    }

    /// The waiting rules recorded so far (for a strict stop).
    pub(crate) fn partial_waiting(&self) -> BTreeMap<String, String> {
        self.partial.borrow().waiting.clone()
    }
}

/// Whether `path` was read, given the read sets of a completed case.
pub(crate) fn path_was_read(
    path: &str,
    read_exact: &BTreeSet<String>,
    read_subtree: &BTreeSet<String>,
) -> bool {
    read_exact.contains(path) || read_subtree.iter().any(|prefix| path_covers(prefix, path))
}

/// How a pattern meets a value that may contain unknown parts.
enum PatternFit {
    /// A known part of the value refutes the pattern.
    Refuted,
    /// The pattern matches whatever the unknown parts are, or matches if
    /// they take some values (`paths` non-empty).
    Fits(BTreeSet<String>),
}

/// Bind every variable of `pattern` to `value`.
pub(crate) fn bind_pattern_variables(pattern: &Pat, value: &Value, env: &mut Env) {
    match pattern {
        Pat::Var(name) => env.set(name.clone(), value.clone()),
        Pat::As(inner, name) => {
            env.set(name.clone(), value.clone());
            bind_pattern_variables(inner, value, env);
        }
        Pat::Con(_, arguments) => {
            for argument in arguments {
                bind_pattern_variables(argument, value, env);
            }
        }
        Pat::NamedCon(_, fields) => {
            for (_, field) in fields {
                bind_pattern_variables(field, value, env);
            }
        }
        Pat::Wild | Pat::Lit(_) => {}
    }
}

/// One clause attempted by a partial rule dispatch.
enum Candidate {
    NoMatch,
    /// A clause whose head matched and whose body is false.
    FalseClause,
    /// The clause fires with `value` if the unknown `paths` allow it (empty:
    /// it certainly fires).
    Fires {
        value: Value,
        paths: BTreeSet<String>,
    },
    Failed,
    Cut,
    /// A known part of the evaluation failed or strict mode stopped.
    Stopped,
}

impl Interpreter {
    /// Evaluate `expression`, speculatively when `condition` is non-empty.
    fn partial_eval_under(
        &mut self,
        condition: &BTreeSet<String>,
        expression: &Expr,
        env: &Env,
    ) -> Speculation<Value> {
        if condition.is_empty() {
            Speculation::Done(self.eval(expression, env))
        } else {
            self.speculate(condition, |interpreter| interpreter.eval(expression, env))
        }
    }

    /// `if` whose condition is unknown with `paths`.
    pub(crate) fn partial_if(
        &mut self,
        paths: BTreeSet<String>,
        then_expression: &Expr,
        else_expression: &Expr,
        env: &Env,
    ) -> Value {
        if self.partial_mode != Some(PartialMode::Propagate) {
            return self.partial_unknown_read(paths);
        }
        let mut outcomes = Vec::with_capacity(2);
        for branch in [then_expression, else_expression] {
            match self.speculate(&paths, |interpreter| interpreter.eval(branch, env)) {
                Speculation::Done(value) => outcomes.push(value),
                Speculation::Failed => return self.speculation_unresolved(paths, true),
                Speculation::Cut => return self.speculation_unresolved(paths, false),
            }
        }
        match agreed_value(&outcomes) {
            Some(value) => value,
            None => self.branching_unknown(paths, &outcomes),
        }
    }

    fn pattern_fit(&self, pattern: &Pat, value: &Value, env: &Env, allow_bare: bool) -> PatternFit {
        match (pattern, value) {
            (Pat::Wild | Pat::Var(_), _) => PatternFit::Fits(BTreeSet::new()),
            (Pat::As(inner, _), _) => self.pattern_fit(inner, value, env, allow_bare),
            (_, Value::Unknown(paths)) => PatternFit::Fits((**paths).clone()),
            _ if !has_unknown(value) => {
                let mut scratch = env.child();
                if self.match_pattern_with_options(pattern, value, &mut scratch, allow_bare) {
                    PatternFit::Fits(BTreeSet::new())
                } else {
                    PatternFit::Refuted
                }
            }
            (Pat::Con(name, arguments), _) => {
                let (value_name, items): (&str, Vec<&Value>) = match value {
                    Value::Constructor(value_name, items)
                    | Value::NamespacedConstructor {
                        name: value_name,
                        arguments: items,
                        ..
                    } => (value_name, items.iter().collect()),
                    Value::NamedConstructor(value_name, fields)
                    | Value::NamespacedNamedConstructor {
                        name: value_name,
                        fields,
                        ..
                    } => (value_name, fields.iter().map(|(_, item)| item).collect()),
                    _ => return PatternFit::Fits(unknown_paths(value, true).unwrap_or_default()),
                };
                if name != value_name || !self.pattern_constructor_owner_matches(name, value, env) {
                    return PatternFit::Refuted;
                }
                if arguments.is_empty() {
                    return if allow_bare || items.is_empty() {
                        PatternFit::Fits(BTreeSet::new())
                    } else {
                        PatternFit::Refuted
                    };
                }
                if arguments.len() != items.len() {
                    return PatternFit::Refuted;
                }
                let mut paths = BTreeSet::new();
                for (argument, item) in arguments.iter().zip(items) {
                    match self.pattern_fit(argument, item, env, false) {
                        PatternFit::Refuted => return PatternFit::Refuted,
                        PatternFit::Fits(more) => paths.extend(more),
                    }
                }
                PatternFit::Fits(paths)
            }
            (Pat::NamedCon(name, patterns), _) => {
                let (value_name, fields) = match value {
                    Value::NamedConstructor(value_name, fields)
                    | Value::NamespacedNamedConstructor {
                        name: value_name,
                        fields,
                        ..
                    } => (value_name, fields),
                    _ => return PatternFit::Fits(unknown_paths(value, true).unwrap_or_default()),
                };
                if name != value_name || !self.pattern_constructor_owner_matches(name, value, env) {
                    return PatternFit::Refuted;
                }
                let mut paths = BTreeSet::new();
                for (field_name, field_pattern) in patterns {
                    let Some((_, item)) = fields.iter().find(|(name, _)| name == field_name) else {
                        return PatternFit::Refuted;
                    };
                    match self.pattern_fit(field_pattern, item, env, false) {
                        PatternFit::Refuted => return PatternFit::Refuted,
                        PatternFit::Fits(more) => paths.extend(more),
                    }
                }
                PatternFit::Fits(paths)
            }
            _ => PatternFit::Fits(unknown_paths(value, true).unwrap_or_default()),
        }
    }

    /// `match` on a value with unknown parts: the arms it could take are
    /// tried in order; the match is decided when they all give one value.
    pub(crate) fn partial_match(
        &mut self,
        value: Value,
        arms: &[MatchArm],
        env: &Env,
        allow_bare: bool,
    ) -> Value {
        self.partial_consume(&value, true);
        let mut condition = BTreeSet::new();
        let mut outcomes = Vec::new();
        let mut certain = false;
        for arm in arms {
            let PatternFit::Fits(fit_paths) = self.pattern_fit(&arm.pat, &value, env, allow_bare)
            else {
                continue;
            };
            let mut arm_env = env.child();
            let mut arm_paths = fit_paths;
            if arm_paths.is_empty() {
                if !self.match_pattern_with_options(&arm.pat, &value, &mut arm_env, allow_bare) {
                    continue;
                }
            } else {
                if self.partial_mode == Some(PartialMode::Strict) {
                    return self.partial_unknown_read(arm_paths);
                }
                let mut binding = arm_paths.clone();
                collect_unknown_paths(&value, true, &mut binding);
                bind_pattern_variables(&arm.pat, &unknown(binding), &mut arm_env);
            }
            if let Some(guard) = &arm.guard {
                let mut under = condition.clone();
                under.extend(arm_paths.iter().cloned());
                match self.partial_eval_under(&under, guard, &arm_env) {
                    Speculation::Done(Value::Bool(true)) => {}
                    Speculation::Done(Value::Bool(false)) => continue,
                    Speculation::Done(Value::Unknown(paths)) => {
                        if self.partial_mode == Some(PartialMode::Strict) {
                            return self.partial_unknown_read((*paths).clone());
                        }
                        arm_paths.extend(paths.iter().cloned());
                    }
                    Speculation::Done(_) => {
                        if self.calculation_failed() {
                            return Value::Unit;
                        }
                        return self.panic_or_ground_fail("match guard must return Bool");
                    }
                    Speculation::Failed => {
                        condition.extend(arm_paths);
                        return self.speculation_unresolved(condition, true);
                    }
                    Speculation::Cut => {
                        condition.extend(arm_paths);
                        return self.speculation_unresolved(condition, false);
                    }
                }
            }
            let mut under = condition.clone();
            under.extend(arm_paths.iter().cloned());
            match self.partial_eval_under(&under, &arm.body, &arm_env) {
                Speculation::Done(result) => outcomes.push(result),
                Speculation::Failed => return self.speculation_unresolved(under, true),
                Speculation::Cut => return self.speculation_unresolved(under, false),
            }
            if self.calculation_failed() {
                return Value::Unit;
            }
            if arm_paths.is_empty() {
                certain = true;
                break;
            }
            condition.extend(arm_paths);
        }
        if condition.is_empty() {
            return match outcomes.pop() {
                Some(result) => result,
                None => self.panic_or_ground_fail("non-exhaustive match: no arm matched the value"),
            };
        }
        if !certain {
            // No arm may match some completion.
            return self.speculation_unresolved(condition, true);
        }
        match agreed_value(&outcomes) {
            Some(result) => result,
            None => self.branching_unknown(condition, &outcomes),
        }
    }

    /// Rule dispatch in a partial case. Clauses are tried in priority order;
    /// a clause whose head or guard depends on unknown values might fire and
    /// blocks every clause below it. The call is decided when every clause
    /// that might fire, and a possible miss, give one value.
    pub(crate) fn partial_rule_call(
        &mut self,
        namespace: &RuntimeNamespace,
        family: &RuleDispatchKey,
        dispatch: &PreparedRuntimeRuleDispatch,
        arguments: &[Value],
        env: &Env,
    ) -> Option<Value> {
        if let Err(cut) = self.partial_enter_call(&family.name, true) {
            return Some(cut);
        }
        let result = self.partial_rule_dispatch(namespace, family, dispatch, arguments, env);
        self.partial_exit_call();
        result
    }

    fn partial_rule_dispatch(
        &mut self,
        namespace: &RuntimeNamespace,
        family: &RuleDispatchKey,
        dispatch: &PreparedRuntimeRuleDispatch,
        arguments: &[Value],
        env: &Env,
    ) -> Option<Value> {
        let mut condition = BTreeSet::new();
        let mut outcomes = Vec::new();
        let mut matched_false_clause = false;
        let mut certain = false;
        let tiers = [
            &dispatch.exceptions,
            &dispatch.conditional_defaults,
            &dispatch.clauses,
            &dispatch.unconditional_defaults,
        ];
        'tiers: for tier in tiers {
            for index in tier.iter() {
                let base_env = self.runtime_rule_candidate_base_env(
                    namespace,
                    family,
                    &dispatch.matching[*index].1,
                    env,
                    dispatch,
                )?;
                let rule = dispatch.matching[*index].0.clone();
                match self.partial_rule_candidate(&rule, arguments, &base_env, &condition, family) {
                    Candidate::NoMatch => {}
                    Candidate::FalseClause => matched_false_clause = true,
                    Candidate::Fires { value, paths } => {
                        outcomes.push(value);
                        if paths.is_empty() {
                            certain = true;
                            break 'tiers;
                        }
                        condition.extend(paths);
                    }
                    Candidate::Failed => return Some(self.speculation_unresolved(condition, true)),
                    Candidate::Cut => return Some(self.speculation_unresolved(condition, false)),
                    Candidate::Stopped => return Some(Value::Unit),
                }
            }
        }
        if condition.is_empty() {
            return match outcomes.pop() {
                Some(value) => Some(value),
                None => matched_false_clause.then_some(Value::Bool(false)),
            };
        }
        if !certain {
            // Every clause might fail to apply: the call might miss.
            if matched_false_clause {
                outcomes.push(Value::Bool(false));
            } else {
                match self.speculate(&condition, |interpreter| {
                    interpreter.calculation_rule_miss(namespace, family)
                }) {
                    Speculation::Done(value) => outcomes.push(value),
                    Speculation::Failed => {
                        return Some(self.speculation_unresolved(condition, true))
                    }
                    Speculation::Cut => return Some(self.speculation_unresolved(condition, false)),
                }
            }
        }
        Some(match agreed_value(&outcomes) {
            Some(value) => value,
            None => self.branching_unknown(condition, &outcomes),
        })
    }

    fn partial_rule_candidate(
        &mut self,
        rule: &Rule,
        arguments: &[Value],
        base_env: &Env,
        condition: &BTreeSet<String>,
        family: &RuleDispatchKey,
    ) -> Candidate {
        let head = match rule {
            Rule::Clause { head, .. }
            | Rule::Default { head, .. }
            | Rule::Exception { head, .. } => head,
            Rule::ReactiveScope { .. } => return Candidate::NoMatch,
        };
        let (rule_env, mut paths) = self.with_partial_probe(|interpreter| {
            interpreter.match_rule_head(head, arguments, base_env)
        });
        if self.calculation_failed() {
            return Candidate::Stopped;
        }
        let Some(rule_env) = rule_env else {
            return Candidate::NoMatch;
        };
        if !paths.is_empty() && self.partial_mode == Some(PartialMode::Strict) {
            self.partial_unknown_read(paths);
            return Candidate::Stopped;
        }
        let under = |paths: &BTreeSet<String>| {
            let mut under = condition.clone();
            under.extend(paths.iter().cloned());
            under
        };
        match rule {
            Rule::Default {
                value,
                condition: guard,
                ..
            }
            | Rule::Exception {
                value,
                condition: guard,
                ..
            } => {
                if let Some(guard) = guard {
                    match self.partial_eval_under(&under(&paths), guard, &rule_env) {
                        Speculation::Done(Value::Bool(true)) => {}
                        Speculation::Done(Value::Bool(false)) => return Candidate::NoMatch,
                        Speculation::Done(Value::Unknown(more)) => {
                            if self.partial_mode == Some(PartialMode::Strict) {
                                self.partial_unknown_read((*more).clone());
                                return Candidate::Stopped;
                            }
                            paths.extend(more.iter().cloned());
                        }
                        Speculation::Done(other) => {
                            if !self.calculation_failed() {
                                let _ = self.panic_or_ground_fail(format!(
                                    "rule `{}` guard must return Bool, got {}",
                                    family.name,
                                    self.runtime_value_type_label(&other)
                                ));
                            }
                            return Candidate::Stopped;
                        }
                        Speculation::Failed => return Candidate::Failed,
                        Speculation::Cut => return Candidate::Cut,
                    }
                }
                match self.partial_eval_under(&under(&paths), value, &rule_env) {
                    Speculation::Done(value) => {
                        if self.calculation_failed() {
                            return Candidate::Stopped;
                        }
                        Candidate::Fires { value, paths }
                    }
                    Speculation::Failed => Candidate::Failed,
                    Speculation::Cut => Candidate::Cut,
                }
            }
            Rule::Clause { body: None, .. } => Candidate::Fires {
                value: Value::Bool(true),
                paths,
            },
            Rule::Clause {
                body: Some(body), ..
            } => {
                let spec = under(&paths);
                let evaluate = |interpreter: &mut Self| -> Option<Value> {
                    if let ExprKind::Conjunction(goals) = &body.kind {
                        interpreter
                            .eval_conjunction(goals, &rule_env)
                            .then_some(Value::Bool(true))
                    } else if let ExprKind::Disjunction(alternatives) = &body.kind {
                        alternatives
                            .iter()
                            .any(|alternative| match &alternative.kind {
                                ExprKind::Conjunction(goals) => {
                                    interpreter.eval_conjunction(goals, &rule_env)
                                }
                                _ => interpreter
                                    .eval_conjunction(std::slice::from_ref(alternative), &rule_env),
                            })
                            .then_some(Value::Bool(true))
                    } else {
                        let value = interpreter.eval(body, &rule_env);
                        (!matches!(value, Value::Bool(false))).then_some(value)
                    }
                };
                let outcome = if spec.is_empty() {
                    Speculation::Done(self.with_partial_probe(evaluate))
                } else {
                    self.speculate(&spec, |interpreter| {
                        interpreter.with_partial_probe(evaluate)
                    })
                };
                match outcome {
                    Speculation::Done((value, probed)) => {
                        if self.calculation_failed() {
                            return Candidate::Stopped;
                        }
                        if !probed.is_empty() && self.partial_mode == Some(PartialMode::Strict) {
                            self.partial_unknown_read(probed);
                            return Candidate::Stopped;
                        }
                        paths.extend(probed);
                        match value {
                            Some(Value::Unknown(more)) => {
                                if self.partial_mode == Some(PartialMode::Strict) {
                                    self.partial_unknown_read((*more).clone());
                                    return Candidate::Stopped;
                                }
                                paths.extend(more.iter().cloned());
                                Candidate::Fires {
                                    value: Value::Unknown(more),
                                    paths,
                                }
                            }
                            Some(value) => Candidate::Fires { value, paths },
                            None if paths.is_empty() => Candidate::FalseClause,
                            None => Candidate::NoMatch,
                        }
                    }
                    Speculation::Failed => Candidate::Failed,
                    Speculation::Cut => Candidate::Cut,
                }
            }
            Rule::ReactiveScope { .. } => Candidate::NoMatch,
        }
    }

    /// Apply a named function in a partial case: track it on the call stack
    /// and cut a recursive call under speculation.
    pub(crate) fn partial_apply_named(
        &mut self,
        name: &str,
        func: Value,
        args: Vec<Value>,
        call_env: &Env,
    ) -> Value {
        if let Err(cut) = self.partial_enter_call(name, false) {
            return cut;
        }
        let value = self.apply_inner(func, args, call_env);
        self.partial_exit_call();
        value
    }
}

/// How a builtin reads its arguments in a partial case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinStrictness {
    /// Arguments are carried, never read.
    Carries,
    /// Every argument is needed as a structure (a list's length, an
    /// option's presence); elements pass through to the builtin's function.
    Structure,
    /// Every argument is needed whole.
    Whole,
}

fn builtin_strictness(name: &str) -> BuiltinStrictness {
    match name {
        "shared" | "pair" | "triple" => BuiltinStrictness::Carries,
        "length" | "head" | "tail" | "nth" | "reverse" | "concat" | "map" | "filter" | "foldl"
        | "any" | "all" | "find" | "flat_map" | "zip" | "enumerate" | "take_while"
        | "drop_while" | "partition" | "chunked" | "is_some" | "is_none" | "unwrap_or" | "fst"
        | "snd" | "trd" | "take" | "skip" | "first" | "last" | "count" | "reduce" | "scan" => {
            BuiltinStrictness::Structure
        }
        _ => BuiltinStrictness::Whole,
    }
}

/// Builtins that give a result for every argument of their type, so an
/// unknown argument cannot make them fail.
fn builtin_is_total(name: &str) -> bool {
    matches!(
        name,
        "show"
            | "show_int"
            | "show_float"
            | "length"
            | "not"
            | "concat"
            | "reverse"
            | "map"
            | "filter"
            | "foldl"
            | "any"
            | "all"
            | "find"
            | "zip"
            | "enumerate"
            | "take_while"
            | "drop_while"
            | "partition"
            | "is_some"
            | "is_none"
            | "unwrap_or"
            | "sort"
            | "distinct"
            | "string_length"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "to_upper"
            | "to_lower"
            | "trim"
            | "replace"
            | "to_float"
            | "round"
            | "floor"
            | "max_f"
            | "min_f"
            | "map_len"
            | "map_contains"
            | "set_len"
            | "set_contains"
            | "pair"
            | "triple"
            | "fst"
            | "snd"
            | "trd"
    )
}

impl Interpreter {
    /// A named builtin in a partial case: read its arguments as its
    /// strictness requires, and give an unknown result when a needed part is
    /// unknown or its function's answer is.
    pub(crate) fn partial_builtin(
        &mut self,
        namespace: &RuntimeNamespace,
        name: &str,
        args: Vec<Value>,
        env: &Env,
    ) -> Value {
        let strictness = builtin_strictness(name);
        if strictness != BuiltinStrictness::Carries {
            let deep = strictness == BuiltinStrictness::Whole;
            let mut paths = BTreeSet::new();
            for arg in &args {
                // A function argument is applied, not read.
                if matches!(
                    arg,
                    Value::Closure { .. } | Value::Builtin(_) | Value::NamespacedBuiltin { .. }
                ) {
                    continue;
                }
                self.partial_consume(arg, deep);
                collect_unknown_paths(arg, deep, &mut paths);
            }
            if !paths.is_empty() {
                if !builtin_is_total(name) {
                    self.partial_fallible(&paths);
                }
                return self.partial_unknown_read(paths);
            }
        }
        self.partial
            .borrow_mut()
            .callback_reads
            .push(callback_read(name));
        let (value, probed) = self.with_partial_probe(|interpreter| {
            interpreter.eval_named_builtin(namespace, name, args, env)
        });
        self.partial.borrow_mut().callback_reads.pop();
        if probed.is_empty() || self.calculation_failed() {
            return value;
        }
        if !builtin_is_total(name) {
            self.partial_fallible(&probed);
        }
        self.partial_unknown_read(probed)
    }
}

/// Join an input path and a field name; the input root is the empty path.
pub(crate) fn join_path(parent: &str, field: &str) -> String {
    if parent.is_empty() {
        field.to_string()
    } else {
        format!("{parent}.{field}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown_value() -> Value {
        unknown(BTreeSet::from(["taxpayer.married".to_string()]))
    }

    #[test]
    fn unknown_values_never_display_compare_hash_or_order_silently() {
        take_unknown_leak();
        let _ = unknown_value().to_string();
        assert_eq!(take_unknown_leak(), Some("display"));
        assert!(!values_equal(&unknown_value(), &Value::Bool(true)));
        assert_eq!(take_unknown_leak(), Some("equality"));
        let mut set = BTreeMap::new();
        runtime_set_insert_value(&mut set, unknown_value());
        assert!(take_unknown_leak().is_some());
        let _ = value_sort_cmp(&unknown_value(), &Value::Int(1));
        assert_eq!(take_unknown_leak(), Some("ordering"));
    }

    #[test]
    fn a_leak_fails_the_case_with_an_internal_error() {
        let mut interpreter = Interpreter::new();
        let result = interpreter.with_calculation_runtime(|_| unknown_value().to_string());
        assert!(
            matches!(result, Err(CalculationRuntimeFailure::InvalidValue(ref message))
                if message == "internal error: an unknown input value reached display"),
            "{result:?}"
        );
    }

    #[test]
    fn a_leak_in_a_nested_evaluation_fails_the_enclosing_case() {
        let mut interpreter = Interpreter::new();
        let result = interpreter.with_calculation_runtime(|runtime| {
            let inner = runtime.with_calculation_runtime(|_| unknown_value().to_string());
            assert!(inner.is_err());
            7
        });
        assert!(
            matches!(result, Err(CalculationRuntimeFailure::InvalidValue(ref message))
                if message.contains("unknown input value")),
            "{result:?}"
        );
        assert_eq!(interpreter.with_calculation_runtime(|_| 7), Ok(7));
    }

    #[test]
    fn an_unknown_list_is_never_traversed_as_empty() {
        take_unknown_leak();
        assert!(list_to_vec(&unknown_value()).is_empty());
        assert_eq!(take_unknown_leak(), Some("list traversal"));
    }

    #[test]
    fn agreement_needs_known_equal_data() {
        assert!(agreed_value(&[Value::Int(7), Value::Int(7)]).is_some());
        assert!(agreed_value(&[Value::Int(7), Value::Int(8)]).is_none());
        assert!(agreed_value(&[Value::Int(7), unknown_value()]).is_none());
        assert!(path_covers("", "accounts[0].id"));
        assert!(path_covers("accounts", "accounts[0].id"));
        assert!(!path_covers("account", "accounts"));
    }
}

impl Interpreter {
    /// A higher-order builtin's callback returned `result`. When the builtin
    /// reads that result and it is unknown, the builtin's own result is
    /// unknown: the paths go to the builtin's probe (`partial_builtin`).
    ///
    /// Returns the value the builtin reads instead, so that it never reads
    /// the unknown value itself; its result is discarded.
    pub(crate) fn partial_callback_result(&self, result: &Value) -> Option<Value> {
        let read = self.partial.borrow().callback_reads.last().copied();
        let (deep, stand_in) = match read {
            None | Some(CallbackRead::Carries) => return None,
            Some(CallbackRead::Structure) => (false, Value::List(Vec::new())),
            Some(CallbackRead::Whole) => (true, Value::Bool(false)),
        };
        let paths = unknown_paths(result, deep)?;
        self.partial_possible_match(&paths);
        Some(stand_in)
    }
}
