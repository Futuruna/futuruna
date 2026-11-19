//! Provenance of calculation results (`runa call --explain`).
//!
//! While an explained case runs, every scalar value that comes from the
//! input or from a rule clause is a [`Value::Traced`]: the value and the node
//! of a provenance graph that says where it came from. A clause's result is
//! tagged with a [`Node::Rule`] naming the clause and the node of what its
//! body computed; an input leaf with a [`Node::Input`] naming its path; an
//! operator or builtin over traced operands with a [`Node::Computed`] over
//! theirs. Nodes are interned per case, so a computation repeated with the
//! same provenance adds no node.
//!
//! Every calculation case, explained or not, records the rule clauses that
//! fired ([`FiredClauses`]): a result's assumptions are read from them, and
//! an explanation's rule nodes name them. Recording costs one table lookup
//! per clause that returns a value; no value is traced.
//!
//! Nothing is traced when the mode is off: no traced value exists, the
//! interpreter's creation sites test one `Option`, and the sites that read
//! values see through a trace with one more `match` arm. Every site that
//! decides on a value (a condition, a guard, a pattern, a builtin reading a
//! scalar) reads it untraced, so an explained case computes exactly what an
//! unexplained one computes. The contract is `docs/rfcs/explanations.md`.

use super::*;
use std::collections::HashSet;

/// A node of a case's provenance graph.
pub type NodeId = u32;

/// A traced value: a scalar or a nullary variant, and where it came from.
#[derive(Debug)]
pub struct TracedValue {
    pub value: Value,
    pub node: NodeId,
}

/// A traced value's data, as the graph keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Leaf {
    Int(i64),
    /// The bits of the `f64`.
    Float(u64),
    Str(Rc<str>),
    Char(char),
    Bool(bool),
    /// A variant without fields.
    Variant(Rc<str>),
}

impl Leaf {
    pub(crate) fn of(value: &Value) -> Option<Leaf> {
        Some(match value {
            Value::Int(number) => Leaf::Int(*number),
            Value::Float(number) => Leaf::Float(number.to_bits()),
            Value::Str(text) => Leaf::Str(Rc::from(text.as_str())),
            Value::Char(character) => Leaf::Char(*character),
            Value::Bool(boolean) => Leaf::Bool(*boolean),
            Value::Constructor(name, arguments)
            | Value::NamespacedConstructor {
                name, arguments, ..
            } if arguments.is_empty() && !matches!(name.as_str(), "Nil" | "None") => {
                Leaf::Variant(Rc::from(name.as_str()))
            }
            Value::NamedConstructor(name, fields)
            | Value::NamespacedNamedConstructor { name, fields, .. }
                if fields.is_empty() && !matches!(name.as_str(), "Nil" | "None") =>
            {
                Leaf::Variant(Rc::from(name.as_str()))
            }
            _ => return None,
        })
    }

    /// The canonical JSON of the value.
    pub(crate) fn json(&self) -> serde_json::Value {
        match self {
            Leaf::Int(number) => serde_json::Value::from(*number),
            Leaf::Float(bits) => serde_json::Number::from_f64(f64::from_bits(*bits))
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Leaf::Str(text) => serde_json::Value::from(text.as_ref()),
            Leaf::Char(character) => serde_json::Value::from(character.to_string()),
            Leaf::Bool(boolean) => serde_json::Value::Bool(*boolean),
            // A shared constructor shows the name it was declared with.
            Leaf::Variant(name) => {
                serde_json::Value::from(crate::constructor_display_name(name).as_ref())
            }
        }
    }
}

/// Where a traced value came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Node {
    /// Passed in at an input path.
    Input { path: Rc<str>, value: Leaf },
    /// Returned by a rule clause (its id in the case's [`FiredClauses`]);
    /// `from` is what the clause's body computed, `because` what the guards
    /// that chose the clause were computed from.
    Rule {
        clause: u32,
        from: Option<NodeId>,
        value: Leaf,
        because: Box<[NodeId]>,
    },
    /// Computed outside any clause (an operator, a builtin) from these.
    Computed { from: Box<[NodeId]> },
}

/// An assumption written on a rule clause or the section around it: its
/// place in the model (file, line, and its place among the values written
/// there), and the condition it holds under (`@ meta(...) when ...`).
#[derive(Debug, Clone)]
pub(crate) struct PlacedAssumption {
    pub(crate) place: (Rc<str>, usize, usize),
    pub(crate) assumption: Rc<crate::calculate::CalculationAssumption>,
    pub(crate) condition: Option<Expr>,
}

/// What a calculation finds a rule clause's assumptions with, by the clause
/// and its rule's name; set by the calculation for each case.
pub(crate) type AssumptionsOf = Rc<dyn Fn(&Interpreter, &Rc<Rule>, &str) -> Rc<[PlacedAssumption]>>;

/// A rule clause that fired, with the assumptions written on it.
#[derive(Debug, Clone)]
pub(crate) struct ClauseRecord {
    pub(crate) rule: Rc<Rule>,
    pub(crate) scope: Option<String>,
    pub(crate) name: String,
    pub(crate) assumptions: Rc<[PlacedAssumption]>,
}

/// A rule call that selected a clause, with its arguments as the dispatch
/// read them (traced in an explained case), for `runa cases`.
#[derive(Debug, Clone)]
pub(crate) struct RuleCallRecord {
    pub(crate) family: RuleDispatchKey,
    /// The clause that produced the value; none when a Boolean family's
    /// `false` clause answered.
    pub(crate) rule: Option<Rc<Rule>>,
    pub(crate) args: Vec<Value>,
}

/// The rule clauses that fired in one calculation case, each once; their
/// ids are `0..` in the order they first fired.
#[derive(Debug, Default)]
pub struct FiredClauses {
    clauses: Vec<ClauseRecord>,
    ids: HashMap<usize, u32>,
    /// `(clause, assumption)`: a conditional assumption of a clause whose
    /// condition held on one of its firings.
    held: HashSet<(u32, usize)>,
}

impl FiredClauses {
    pub(crate) fn clause(&self, id: u32) -> &ClauseRecord {
        &self.clauses[id as usize]
    }

    pub(crate) fn len(&self) -> usize {
        self.clauses.len()
    }

    /// The id of `rule` when it fired.
    pub(crate) fn id_of(&self, rule: &Rc<Rule>) -> Option<u32> {
        self.ids.get(&(Rc::as_ptr(rule) as usize)).copied()
    }

    /// The assumptions of a fired clause that hold: those without a
    /// condition, and those whose condition held when it fired.
    pub(crate) fn holding(&self, id: u32) -> impl Iterator<Item = &PlacedAssumption> {
        self.clause(id)
            .assumptions
            .iter()
            .enumerate()
            .filter(move |(index, placed)| {
                placed.condition.is_none() || self.held.contains(&(id, *index))
            })
            .map(|(_, placed)| placed)
    }
}

/// The provenance graph of one case.
#[derive(Debug, Default)]
pub struct ExplainState {
    nodes: Vec<Node>,
    interned: HashMap<Node, NodeId>,
    /// Per active builtin: whether it reads its callbacks' results whole.
    callback_whole: Vec<bool>,
    /// Per active rule dispatch, innermost last: the nodes of the guard
    /// values evaluated so far (the guards that chose or refuted a clause).
    guards: Vec<Vec<NodeId>>,
    /// Set for a decided case (`runa cases`, a rejected result run again):
    /// a branch's value is computed from what chose the branch too.
    decisions: Option<Decisions>,
}

/// What a decided case keeps besides the graph.
#[derive(Debug, Default)]
struct Decisions {
    /// The result variants that are rejections, by display name.
    rejections: HashSet<String>,
    /// The values the conditions that chose a rejection were computed from.
    rejected_by: Vec<NodeId>,
}

impl ExplainState {
    pub(crate) fn intern(&mut self, node: Node) -> NodeId {
        if let Some(id) = self.interned.get(&node) {
            return *id;
        }
        #[cfg(test)]
        PROVENANCE_NODES.with(|count| count.set(count.get() + 1));
        let id = NodeId::try_from(self.nodes.len()).expect("provenance graph fits u32");
        self.nodes.push(node.clone());
        self.interned.insert(node, id);
        id
    }

    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The rules and inputs `id` was computed from: through computed nodes,
    /// to the nearest rule or input nodes, in first-reached order. At most
    /// `limit` are returned; the second value counts the rest.
    pub(crate) fn contributors(&self, id: NodeId, limit: Option<usize>) -> (Vec<NodeId>, usize) {
        let mut found = Vec::new();
        let mut omitted = 0;
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(next) = stack.pop() {
            if !seen.insert(next) {
                continue;
            }
            match self.node(next) {
                Node::Computed { from } => stack.extend(from.iter().rev().copied()),
                Node::Input { .. } | Node::Rule { .. } => {
                    if limit.is_some_and(|limit| found.len() >= limit) {
                        omitted += 1;
                    } else {
                        found.push(next);
                    }
                }
            }
        }
        (found, omitted)
    }

    /// The input leaf `id` is, when it is one unchanged.
    pub(crate) fn input_path(&self, id: NodeId) -> Option<Rc<str>> {
        match self.node(id) {
            Node::Input { path, .. } => Some(path.clone()),
            _ => None,
        }
    }

    /// The input paths `id` was computed from: through computed nodes and
    /// what rule clauses computed (not the guards that chose them), each
    /// once, in first-reached order.
    pub(crate) fn input_paths(&self, id: NodeId) -> Vec<Rc<str>> {
        let mut found: Vec<Rc<str>> = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(next) = stack.pop() {
            if !seen.insert(next) {
                continue;
            }
            match self.node(next) {
                Node::Input { path, .. } => {
                    if !found.contains(path) {
                        found.push(path.clone());
                    }
                }
                Node::Computed { from } => stack.extend(from.iter().rev().copied()),
                Node::Rule { from, .. } => stack.extend(*from),
            }
        }
        found
    }

    /// The rule clauses a decided case's rejection rests on: those whose
    /// value, or whose guards, the result's leaves (`result`) and the
    /// conditions that chose a rejection were computed from.
    pub(crate) fn rejection_clauses(&self, result: &[NodeId]) -> BTreeSet<u32> {
        let mut clauses = BTreeSet::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<NodeId> = result.to_vec();
        if let Some(decisions) = &self.decisions {
            stack.extend(&decisions.rejected_by);
        }
        while let Some(next) = stack.pop() {
            if !seen.insert(next) {
                continue;
            }
            match self.node(next) {
                Node::Input { .. } => {}
                Node::Computed { from } => stack.extend(from.iter().copied()),
                Node::Rule {
                    clause,
                    from,
                    because,
                    ..
                } => {
                    clauses.insert(*clause);
                    stack.extend(*from);
                    stack.extend(because.iter().copied());
                }
            }
        }
        clauses
    }
}

/// The value without its trace.
#[inline]
pub(crate) fn untraced(value: &Value) -> &Value {
    match value {
        Value::Traced(traced) => &traced.value,
        other => other,
    }
}

/// The value without its trace, by value.
#[inline]
pub(crate) fn strip(value: Value) -> Value {
    match value {
        Value::Traced(traced) => traced.value.clone(),
        other => other,
    }
}

/// The trace's node, if the value is traced.
#[inline]
pub(crate) fn node_of(value: &Value) -> Option<NodeId> {
    match value {
        Value::Traced(traced) => Some(traced.node),
        _ => None,
    }
}

pub(crate) fn traced(value: Value, node: NodeId) -> Value {
    #[cfg(test)]
    TRACED_VALUES.with(|count| count.set(count.get() + 1));
    Value::Traced(Rc::new(TracedValue { value, node }))
}

#[cfg(test)]
thread_local! {
    /// Traced values and provenance nodes created on this thread, so tests can
    /// show an unexplained case creates none.
    static TRACED_VALUES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROVENANCE_NODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Traced values and provenance nodes created on this thread so far.
#[cfg(test)]
pub(crate) fn tracing_counts() -> (usize, usize) {
    (
        TRACED_VALUES.with(std::cell::Cell::get),
        PROVENANCE_NODES.with(std::cell::Cell::get),
    )
}

/// Whether a trace occurs anywhere in `value`.
pub(crate) fn has_traced(value: &Value) -> bool {
    match value {
        Value::Traced(_) => true,
        Value::List(items) | Value::Tuple(items) | Value::Stream(items) => {
            items.iter().any(has_traced)
        }
        Value::Constructor(_, items)
        | Value::NamespacedConstructor {
            arguments: items, ..
        } => items.iter().any(has_traced),
        Value::NamedConstructor(_, fields) | Value::NamespacedNamedConstructor { fields, .. } => {
            fields.iter().any(|(_, item)| has_traced(item))
        }
        Value::Map(entries) | Value::Set(entries) => entries.values().any(has_traced),
        Value::RuleScopeInstance { bindings, .. }
        | Value::NamespacedRuleScopeInstance { bindings, .. } => bindings.values().any(has_traced),
        _ => false,
    }
}

/// Collect the nodes of every trace in `value`.
pub(crate) fn collect_nodes(value: &Value, nodes: &mut Vec<NodeId>) {
    match value {
        Value::Traced(traced) => nodes.push(traced.node),
        Value::List(items) | Value::Tuple(items) | Value::Stream(items) => {
            for item in items {
                collect_nodes(item, nodes);
            }
        }
        Value::Constructor(_, items)
        | Value::NamespacedConstructor {
            arguments: items, ..
        } => {
            for item in items.iter() {
                collect_nodes(item, nodes);
            }
        }
        Value::NamedConstructor(_, fields) | Value::NamespacedNamedConstructor { fields, .. } => {
            for (_, item) in fields.iter() {
                collect_nodes(item, nodes);
            }
        }
        Value::Map(entries) | Value::Set(entries) => {
            for item in entries.values() {
                collect_nodes(item, nodes);
            }
        }
        Value::RuleScopeInstance { bindings, .. }
        | Value::NamespacedRuleScopeInstance { bindings, .. } => {
            for item in bindings.values() {
                collect_nodes(item, nodes);
            }
        }
        _ => {}
    }
}

/// `value` with every trace removed.
pub fn strip_deep(value: &Value) -> Value {
    if !has_traced(value) {
        return value.clone();
    }
    map_leaves(value, &mut |leaf| strip(leaf.clone()))
}

/// Rebuild `value` with `leaf` applied to every scalar, nullary variant and
/// unknown value, traced or not. Nil and None, closures, maps and sets are
/// kept as they are.
pub(crate) fn map_leaves(value: &Value, leaf: &mut dyn FnMut(&Value) -> Value) -> Value {
    if matches!(value, Value::Traced(_) | Value::Unknown(_)) || Leaf::of(value).is_some() {
        return leaf(value);
    }
    match value {
        Value::List(items) => {
            Value::List(items.iter().map(|item| map_leaves(item, leaf)).collect())
        }
        Value::Tuple(items) => {
            Value::Tuple(items.iter().map(|item| map_leaves(item, leaf)).collect())
        }
        Value::Stream(items) => {
            Value::Stream(items.iter().map(|item| map_leaves(item, leaf)).collect())
        }
        Value::Constructor(name, items) if name == "Cons" => {
            // A literal list is a chain of Cons cells: rebuild it without
            // recursing once per element.
            let mut elements = Vec::new();
            let mut current = value;
            while let Value::Constructor(name, cell) = current {
                if name != "Cons" || cell.len() != 2 {
                    break;
                }
                elements.push(map_leaves(&cell[0], leaf));
                current = &cell[1];
            }
            let mut rebuilt = map_leaves(current, leaf);
            for element in elements.into_iter().rev() {
                rebuilt = Value::Constructor("Cons".into(), vec![element, rebuilt].into());
            }
            let _ = items;
            rebuilt
        }
        Value::Constructor(name, items) => Value::Constructor(
            name.clone(),
            Rc::new(items.iter().map(|item| map_leaves(item, leaf)).collect()),
        ),
        Value::NamespacedConstructor {
            namespace,
            name,
            arguments,
            declaration_env,
        } => Value::NamespacedConstructor {
            namespace: namespace.clone(),
            name: name.clone(),
            arguments: Rc::new(
                arguments
                    .iter()
                    .map(|item| map_leaves(item, leaf))
                    .collect(),
            ),
            declaration_env: declaration_env.clone(),
        },
        Value::NamedConstructor(name, fields) => Value::NamedConstructor(
            name.clone(),
            Rc::new(
                fields
                    .iter()
                    .map(|(field, item)| (field.clone(), map_leaves(item, leaf)))
                    .collect(),
            ),
        ),
        Value::NamespacedNamedConstructor {
            namespace,
            name,
            fields,
            declaration_env,
        } => Value::NamespacedNamedConstructor {
            namespace: namespace.clone(),
            name: name.clone(),
            fields: Rc::new(
                fields
                    .iter()
                    .map(|(field, item)| (field.clone(), map_leaves(item, leaf)))
                    .collect(),
            ),
            declaration_env: declaration_env.clone(),
        },
        Value::RuleScopeInstance { name, bindings } => Value::RuleScopeInstance {
            name: name.clone(),
            bindings: Rc::new(
                bindings
                    .iter()
                    .map(|(field, item)| (field.clone(), map_leaves(item, leaf)))
                    .collect(),
            ),
        },
        Value::NamespacedRuleScopeInstance {
            namespace,
            name,
            bindings,
            declaration_env,
        } => Value::NamespacedRuleScopeInstance {
            namespace: namespace.clone(),
            name: name.clone(),
            bindings: Rc::new(
                bindings
                    .iter()
                    .map(|(field, item)| (field.clone(), map_leaves(item, leaf)))
                    .collect(),
            ),
            declaration_env: declaration_env.clone(),
        },
        Value::Map(entries) if has_traced(value) => Value::Map(
            entries
                .iter()
                .map(|(key, item)| (key.clone(), strip_deep(item)))
                .collect(),
        ),
        Value::Set(entries) if has_traced(value) => Value::Set(
            entries
                .iter()
                .map(|(key, item)| (key.clone(), strip_deep(item)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// How a builtin treats traced arguments.
enum BuiltinTracing {
    /// It moves values around: elements keep their traces. Top-level traced
    /// arguments are read untraced, except at these positions.
    Carries(&'static [usize]),
    /// It reads its arguments: they are read untraced and its result is
    /// computed from all of them.
    Reads,
}

const ALL_POSITIONS: &[usize] = &[0, 1, 2, 3];

fn builtin_tracing(name: &str) -> BuiltinTracing {
    match name {
        "foldl" | "scan" | "reduce" | "push" | "unwrap_or" => BuiltinTracing::Carries(&[1]),
        "pair" | "triple" | "shared" => BuiltinTracing::Carries(ALL_POSITIONS),
        "map" | "filter" | "find" | "any" | "all" | "flat_map" | "count_by" | "take_while"
        | "drop_while" | "partition" | "enumerate" | "zip" | "reverse" | "head" | "tail"
        | "last" | "first" | "nth" | "take" | "skip" | "chunked" | "is_some" | "is_none"
        | "fst" | "snd" | "trd" | "collect" | "sort" | "sort_by" | "distinct" | "list_min"
        | "list_max" | "concat" | "length" => BuiltinTracing::Carries(&[]),
        _ => BuiltinTracing::Reads,
    }
}

/// Whether a builtin reads its callbacks' results as a whole (a predicate's
/// Bool, a sort key) rather than carrying them (`map`'s elements).
fn builtin_reads_callback_results(name: &str) -> bool {
    !matches!(name, "map" | "foldl" | "scan" | "reduce" | "flat_map")
}

impl Interpreter {
    /// Start recording the rule clauses that fire in a calculation case.
    pub(crate) fn begin_fired_clauses(&mut self) {
        self.fired = Some(RefCell::new(FiredClauses::default()));
    }

    /// Stop recording, returning the clauses that fired.
    pub(crate) fn finish_fired_clauses(&mut self) -> FiredClauses {
        self.fired
            .take()
            .map(RefCell::into_inner)
            .unwrap_or_default()
    }

    /// Start logging every rule call that selects a clause.
    pub(crate) fn begin_rule_call_log(&mut self) {
        self.rule_calls = Some(RefCell::new(Vec::new()));
    }

    /// Stop logging, returning the calls in the order they selected.
    pub(crate) fn finish_rule_call_log(&mut self) -> Vec<RuleCallRecord> {
        self.rule_calls
            .take()
            .map(RefCell::into_inner)
            .unwrap_or_default()
    }

    /// Capture the arguments of every call of the function `name` while a
    /// program runs (`runa cases --scenario`).
    pub fn begin_call_capture(&mut self, name: &str) {
        self.call_capture = Some((name.to_string(), RefCell::new(Vec::new())));
    }

    /// Stop capturing, returning each call's arguments in call order.
    pub fn finish_call_capture(&mut self) -> Vec<Vec<Value>> {
        self.call_capture
            .take()
            .map(|(_, calls)| calls.into_inner())
            .unwrap_or_default()
    }

    /// Start an explained case: a fresh provenance graph.
    pub(crate) fn begin_explained_case(&mut self) {
        self.explain = Some(RefCell::new(ExplainState::default()));
    }

    /// Start a decided case (`runa cases`): an explained case in which the
    /// value an `if` or `match` branch gives is computed from the values
    /// that chose the branch, and a branch that gives one of `rejections`
    /// (result variants by display name) records them.
    pub(crate) fn begin_decided_case(&mut self, rejections: HashSet<String>) {
        self.explain = Some(RefCell::new(ExplainState {
            decisions: Some(Decisions {
                rejections,
                rejected_by: Vec::new(),
            }),
            ..ExplainState::default()
        }));
    }

    /// Whether this is a decided case.
    #[inline]
    pub(crate) fn explain_decides(&self) -> bool {
        self.explain
            .as_ref()
            .is_some_and(|state| state.borrow().decisions.is_some())
    }

    /// In a decided case, a branch chosen by values traced at `from` gave
    /// `result`: every leaf of it is computed from them as well, and a
    /// rejection variant it gives records them.
    pub(crate) fn explain_branch(&self, result: Value, from: &[NodeId]) -> Value {
        let Some(state) = &self.explain else {
            return result;
        };
        let decision = {
            let mut state = state.borrow_mut();
            if state.decisions.is_none() {
                return result;
            }
            let decision = match from {
                [] => return result,
                [one] => *one,
                many => state.intern(Node::Computed {
                    from: many.to_vec().into_boxed_slice(),
                }),
            };
            let name = match untraced(&result) {
                Value::Constructor(name, _)
                | Value::NamedConstructor(name, _)
                | Value::NamespacedConstructor { name, .. }
                | Value::NamespacedNamedConstructor { name, .. } => Some(name),
                _ => None,
            };
            let decisions = state.decisions.as_mut().expect("a decided case");
            if name.is_some_and(|name| {
                decisions
                    .rejections
                    .contains(crate::constructor_display_name(name).as_ref())
            }) {
                decisions.rejected_by.push(decision);
            }
            decision
        };
        map_leaves(&result, &mut |leaf| {
            let inner = untraced(leaf);
            if Leaf::of(inner).is_none() {
                return leaf.clone();
            }
            let from: Box<[NodeId]> = match node_of(leaf) {
                Some(own) if own != decision => Box::new([own, decision]),
                _ => Box::new([decision]),
            };
            match self.explain_intern(Node::Computed { from }) {
                Some(node) => traced(inner.clone(), node),
                None => leaf.clone(),
            }
        })
    }

    /// End an explained case, returning its provenance graph.
    pub(crate) fn finish_explained_case(&mut self) -> Option<ExplainState> {
        self.explain.take().map(RefCell::into_inner)
    }

    pub(crate) fn explain_intern(&self, node: Node) -> Option<NodeId> {
        self.explain
            .as_ref()
            .map(|state| state.borrow_mut().intern(node))
    }

    /// The source file a rule was declared in, when it is not the root file.
    pub(crate) fn rule_source_file(&self, rule: &Rc<Rule>) -> Option<Rc<str>> {
        self.rule_source_files
            .get(&(Rc::as_ptr(rule) as usize))
            .cloned()
    }

    pub(crate) fn record_rule_source(&mut self, rule: &Rc<Rule>) {
        if let Some(file) = &self.runtime_source_file {
            self.rule_source_files
                .insert(Rc::as_ptr(rule) as usize, file.clone());
        }
    }

    /// A `match` arm chose `result`: a scalar the arm gives without a trace
    /// of its own (a literal) is computed from the matched value.
    pub(crate) fn explain_decided(&self, result: Value, matched: &Value) -> Value {
        if node_of(&result).is_some() || Leaf::of(&result).is_none() {
            return result;
        }
        let mut from = Vec::new();
        collect_nodes(matched, &mut from);
        self.explain_computed(result, from)
    }

    /// A leaf computed from traced operands.
    fn explain_computed(&self, result: Value, from: Vec<NodeId>) -> Value {
        if Leaf::of(&result).is_none() || from.is_empty() {
            return result;
        }
        let mut unique = Vec::with_capacity(from.len());
        for node in from {
            if !unique.contains(&node) {
                unique.push(node);
            }
        }
        match self.explain_intern(Node::Computed {
            from: unique.into_boxed_slice(),
        }) {
            Some(node) => traced(result, node),
            None => result,
        }
    }

    /// `left op right` where an operand is traced.
    pub(crate) fn explain_binop(&self, op: &str, left: Value, right: Value) -> Value {
        let left_node = node_of(&left);
        let right_node = node_of(&right);
        let left = strip(left);
        let right = strip(right);
        if Leaf::of(&left).is_none() || Leaf::of(&right).is_none() {
            // Structural equality of containers: read whole, untraced.
            return self.eval_binop(op, strip_deep(&left), strip_deep(&right));
        }
        let is = |value: &Value, unit: i64| match value {
            Value::Int(number) => *number == unit,
            Value::Float(number) => *number == unit as f64,
            _ => false,
        };
        // An identity operation passes its operand's value through unchanged.
        let passes_left = right_node.is_none()
            && match op {
                "+" | "-" => is(&right, 0),
                "*" | "/" => is(&right, 1),
                _ => false,
            };
        let passes_right = left_node.is_none()
            && match op {
                "+" => is(&left, 0),
                "*" => is(&left, 1),
                _ => false,
            };
        let result = self.eval_binop(op, left, right);
        if self.calculation_failed() {
            return result;
        }
        match (left_node, right_node) {
            (Some(node), None) if passes_left => traced(result, node),
            (None, Some(node)) if passes_right => traced(result, node),
            _ => self.explain_computed(result, left_node.into_iter().chain(right_node).collect()),
        }
    }

    /// A unary operator's result from a traced operand.
    #[inline]
    pub(crate) fn explain_unop(&self, operand: Option<NodeId>, result: Value) -> Value {
        match operand {
            Some(node) => self.explain_computed(result, vec![node]),
            None => result,
        }
    }

    /// A named builtin in an explained case.
    pub(crate) fn explain_builtin(
        &mut self,
        namespace: &RuntimeNamespace,
        name: &str,
        args: Vec<Value>,
        env: &Env,
    ) -> Value {
        let call = |interpreter: &mut Self, args: Vec<Value>| {
            if interpreter.partial_mode.is_some() {
                interpreter.partial_builtin(namespace, name, args, env)
            } else {
                interpreter.eval_named_builtin(namespace, name, args, env)
            }
        };
        let push = |interpreter: &Self, whole: bool| {
            if let Some(state) = &interpreter.explain {
                state.borrow_mut().callback_whole.push(whole);
            }
        };
        let pop = |interpreter: &Self| {
            if let Some(state) = &interpreter.explain {
                state.borrow_mut().callback_whole.pop();
            }
        };
        match builtin_tracing(name) {
            BuiltinTracing::Carries(kept) => {
                let args = args
                    .into_iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        if kept.contains(&index) {
                            arg
                        } else {
                            strip(arg)
                        }
                    })
                    .collect();
                push(self, builtin_reads_callback_results(name));
                let result = call(self, args);
                pop(self);
                result
            }
            BuiltinTracing::Reads => {
                let mut from = Vec::new();
                for arg in &args {
                    collect_nodes(arg, &mut from);
                }
                let args = if from.is_empty() {
                    args
                } else {
                    args.iter().map(strip_deep).collect()
                };
                push(self, true);
                let result = call(self, args);
                pop(self);
                self.explain_computed(result, from)
            }
        }
    }

    /// A builtin's callback returned `result`: the value the builtin reads.
    pub(crate) fn explain_callback_result(&self, result: Value) -> Value {
        let whole = self
            .explain
            .as_ref()
            .and_then(|state| state.borrow().callback_whole.last().copied())
            .unwrap_or(false);
        if whole {
            strip(result)
        } else {
            result
        }
    }

    /// Begin a rule dispatch: collect the guard values it reads.
    #[inline]
    pub(crate) fn explain_begin_dispatch(&self) {
        if let Some(state) = &self.explain {
            state.borrow_mut().guards.push(Vec::new());
        }
    }

    /// End a rule dispatch begun with [`Self::explain_begin_dispatch`].
    #[inline]
    pub(crate) fn explain_end_dispatch(&self) {
        if let Some(state) = &self.explain {
            state.borrow_mut().guards.pop();
        }
    }

    /// A rule guard evaluated to `value`: the dispatch records what it was
    /// computed from, before the guard is read untraced.
    #[inline]
    pub(crate) fn explain_guard_read(&self, value: &Value) {
        let (Some(state), Some(node)) = (&self.explain, node_of(value)) else {
            return;
        };
        let mut state = state.borrow_mut();
        if let Some(guards) = state.guards.last_mut() {
            if !guards.contains(&node) {
                guards.push(node);
            }
        }
    }

    /// A clause that fired with an assumption under a condition not yet seen
    /// to hold: its id, for [`Self::record_assumption_conditions`].
    pub(crate) fn conditional_clause(&self, rule: &Rc<Rule>) -> Option<u32> {
        let fired = self.fired.as_ref()?.borrow();
        let clause = *fired.ids.get(&(Rc::as_ptr(rule) as usize))?;
        fired
            .clause(clause)
            .assumptions
            .iter()
            .enumerate()
            .any(|(index, placed)| {
                placed.condition.is_some() && !fired.held.contains(&(clause, index))
            })
            .then_some(clause)
    }

    /// Evaluate the conditions of a fired clause's assumptions in the
    /// clause's environment (its parameters bound). A condition that is not
    /// `false` (true, or not known in a partial case) holds. Evaluating one
    /// changes nothing a case reads: a partial case's reads are put back.
    pub(crate) fn record_assumption_conditions(&mut self, clause: u32, env: &Env) {
        let Some(assumptions) = self
            .fired
            .as_ref()
            .map(|fired| fired.borrow().clause(clause).assumptions.clone())
        else {
            return;
        };
        for (index, placed) in assumptions.iter().enumerate() {
            let Some(condition) = &placed.condition else {
                continue;
            };
            let held = self
                .fired
                .as_ref()
                .is_some_and(|fired| fired.borrow().held.contains(&(clause, index)));
            if held {
                continue;
            }
            // A partial case's reads and needs are set aside: the condition
            // reads only values the clause already has.
            let saved = self
                .partial_mode
                .is_some()
                .then(|| std::mem::take(&mut *self.partial.borrow_mut()));
            let value = self.eval(condition, env);
            if let Some(saved) = saved {
                *self.partial.borrow_mut() = saved;
            }
            if !matches!(untraced(&value), Value::Bool(false)) {
                if let Some(fired) = &self.fired {
                    fired.borrow_mut().held.insert((clause, index));
                }
            }
        }
    }

    /// A rule clause returned `value`: record that it fired and, in an
    /// explained case, tag the value with the clause and with the guards its
    /// dispatch read to choose it.
    pub(crate) fn rule_clause_fired(
        &self,
        rule: &Rc<Rule>,
        family: &RuleDispatchKey,
        value: Value,
    ) -> Value {
        let Some(fired) = &self.fired else {
            return value;
        };
        if matches!(value, Value::Unknown(_)) {
            return value;
        }
        let key = Rc::as_ptr(rule) as usize;
        let known = fired.borrow().ids.get(&key).copied();
        let clause = match known {
            Some(clause) => clause,
            None => {
                let assumptions = self
                    .assumptions_of
                    .as_ref()
                    .map(|of| of(self, rule, &family.name))
                    .unwrap_or_else(|| Rc::from(Vec::new()));
                let mut fired = fired.borrow_mut();
                let id = u32::try_from(fired.clauses.len()).expect("clause table fits u32");
                fired.clauses.push(ClauseRecord {
                    rule: rule.clone(),
                    scope: family.scope.clone(),
                    name: family.name.clone(),
                    assumptions,
                });
                fired.ids.insert(key, id);
                id
            }
        };
        let Some(state) = &self.explain else {
            return value;
        };
        let because: Box<[NodeId]> = state
            .borrow()
            .guards
            .last()
            .map(|guards| guards.clone().into_boxed_slice())
            .unwrap_or_default();
        // In a decided case a clause's `None` is traced as well: a rejection
        // may rest on a rule having nothing (`| None -> Rejected(...)`).
        let none = matches!(
            untraced(&value),
            Value::Constructor(name, arguments) | Value::NamespacedConstructor { name, arguments, .. }
                if arguments.is_empty() && name.as_str() == "None"
        );
        if none && state.borrow().decisions.is_some() {
            let node = state.borrow_mut().intern(Node::Rule {
                clause,
                from: node_of(&value),
                value: Leaf::Variant(Rc::from("None")),
                because,
            });
            return traced(untraced(&value).clone(), node);
        }
        // A clause that returns one value produced it. A clause that returns a
        // structure produced the leaves it computed.
        let structure = Leaf::of(untraced(&value)).is_none();
        let mut tag = |leaf: &Value| -> Value {
            let from = node_of(leaf);
            // A member a structure passes on unchanged from the input or
            // another rule keeps that provenance; a single value the clause
            // returns is the clause's, with the input or rule in `from`.
            let kept = structure
                && from.is_some_and(|node| {
                    !matches!(state.borrow().node(node), Node::Computed { .. })
                });
            if kept {
                return leaf.clone();
            }
            let inner = untraced(leaf).clone();
            let Some(value) = Leaf::of(&inner) else {
                return inner;
            };
            let node = state.borrow_mut().intern(Node::Rule {
                clause,
                from,
                value,
                because: because.clone(),
            });
            traced(inner, node)
        };
        if Leaf::of(untraced(&value)).is_some() {
            return tag(&value);
        }
        let retagged = map_leaves(&value, &mut tag);
        self.explain_keep_partial_containers(&value, &retagged);
        retagged
    }

    /// A partial case tracks reads of input records by their allocation;
    /// a record rebuilt with new traces keeps its input path.
    pub(crate) fn explain_keep_partial_containers(&self, original: &Value, rebuilt: &Value) {
        if self.partial_mode.is_none() {
            return;
        }
        let mut state = self.partial.borrow_mut();
        let mut pairs = vec![(original.clone(), rebuilt.clone())];
        while let Some((original, rebuilt)) = pairs.pop() {
            if let (Some(old), Some(new)) = (
                partial::container_id(&original),
                partial::container_id(&rebuilt),
            ) {
                if let Some(path) = state.containers.get(&old).cloned() {
                    state.containers.insert(new, path);
                }
            }
            match (&original, &rebuilt) {
                (Value::List(left), Value::List(right))
                | (Value::Tuple(left), Value::Tuple(right)) => {
                    pairs.extend(left.iter().cloned().zip(right.iter().cloned()));
                }
                (
                    Value::Constructor(_, left)
                    | Value::NamespacedConstructor {
                        arguments: left, ..
                    },
                    Value::Constructor(_, right)
                    | Value::NamespacedConstructor {
                        arguments: right, ..
                    },
                ) => pairs.extend(left.iter().cloned().zip(right.iter().cloned())),
                (
                    Value::NamedConstructor(_, left)
                    | Value::NamespacedNamedConstructor { fields: left, .. },
                    Value::NamedConstructor(_, right)
                    | Value::NamespacedNamedConstructor { fields: right, .. },
                ) => pairs.extend(
                    left.iter()
                        .map(|(_, item)| item.clone())
                        .zip(right.iter().map(|(_, item)| item.clone())),
                ),
                (
                    Value::RuleScopeInstance { bindings: left, .. }
                    | Value::NamespacedRuleScopeInstance { bindings: left, .. },
                    Value::RuleScopeInstance {
                        bindings: right, ..
                    }
                    | Value::NamespacedRuleScopeInstance {
                        bindings: right, ..
                    },
                ) => {
                    for (name, item) in left.iter() {
                        if let Some(other) = right.get(name) {
                            pairs.push((item.clone(), other.clone()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_traced_value_compares_hashes_orders_and_displays_as_its_value() {
        let traced_seven = traced(Value::Int(7), 0);
        assert!(values_equal(&traced_seven, &Value::Int(7)));
        assert!(runtime_values_semantically_equal(
            &Value::Int(7),
            &traced_seven
        ));
        assert_eq!(
            value_sort_cmp(&traced_seven, &Value::Int(8)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            runtime_value_semantic_key(&traced_seven),
            runtime_value_semantic_key(&Value::Int(7))
        );
        assert_eq!(traced_seven.to_string(), "7");
    }

    #[test]
    fn stripping_rebuilds_only_traced_values() {
        let record = Value::NamedConstructor(
            "Date".into(),
            Rc::new(vec![
                ("year".into(), traced(Value::Int(2026), 0)),
                ("month".into(), Value::Int(2)),
            ]),
        );
        let stripped = strip_deep(&record);
        assert!(!has_traced(&stripped));
        assert!(values_equal(&stripped, &record));
        let list = vec_to_list(vec![traced(Value::Int(1), 0), Value::Int(2)]);
        assert!(!has_traced(&strip_deep(&list)));
    }

    #[test]
    fn interning_reuses_nodes_and_contributors_skip_computations() {
        let mut state = ExplainState::default();
        let input = state.intern(Node::Input {
            path: Rc::from("income"),
            value: Leaf::Int(10),
        });
        let again = state.intern(Node::Input {
            path: Rc::from("income"),
            value: Leaf::Int(10),
        });
        assert_eq!(input, again);
        let other = state.intern(Node::Input {
            path: Rc::from("rate"),
            value: Leaf::Int(3),
        });
        let sum = state.intern(Node::Computed {
            from: vec![input, other].into_boxed_slice(),
        });
        let twice = state.intern(Node::Computed {
            from: vec![sum, input].into_boxed_slice(),
        });
        assert_eq!(state.contributors(twice, None), (vec![input, other], 0));
        assert_eq!(state.contributors(twice, Some(1)), (vec![input], 1));
        assert_eq!(state.node_count(), 4);
    }
}
