//! Closed search-universe elaboration for bounded `? explore` declarations.
//!
//! The parser and type checker deliberately retain source expressions.  This
//! pass is the trust boundary that proves every declared domain is finite,
//! deterministic, and exact before a solver or exhaustive executor may see it.

use super::*;

const EXPLORE_GROUND_COLLECTION_LIMIT: u64 = 1_000_000;
const EXPLORE_GROUND_WORK_LIMIT: u64 = 4_000_000;
const EXPLORE_FINITE_PLAN_WORK_LIMIT: usize = 100_000;
const EXPLORE_RECURSION_LIMIT: usize = 64;
const EXPLORE_GROUND_RECURSION_LIMIT: usize = 16;

/// Canonical first-order value used for domain identity, ordering, SMT
/// constants, and replay.  Floats use their exact IEEE bits rather than the
/// interpreter's approximate equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExploreValue {
    Int(i64),
    FloatBits(u64),
    String(String),
    Character(char),
    Boolean(bool),
    Unit,
    List(Vec<ExploreValue>),
    Set(Vec<ExploreValue>),
    Tuple(Vec<ExploreValue>),
    Constructor {
        type_name: String,
        variant: String,
        positional: bool,
        fields: Vec<(String, ExploreValue)>,
    },
}

impl ExploreValue {
    pub fn int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    fn runtime_display_key(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::FloatBits(bits) => f64::from_bits(*bits).to_string(),
            Self::String(value) => value.clone(),
            Self::Character(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Unit => "()".to_string(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Set(values) => format!(
                "{{{}}}",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Tuple(values) => format!(
                "({})",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } if variant == "Nil" && fields.is_empty() => "[]".to_string(),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } if variant == "Cons" && fields.len() == 2 => {
                let mut items = vec![&fields[0].1];
                let mut tail = &fields[1].1;
                while let Self::Constructor {
                    variant,
                    positional: true,
                    fields,
                    ..
                } = tail
                {
                    if variant != "Cons" || fields.len() != 2 {
                        break;
                    }
                    items.push(&fields[0].1);
                    tail = &fields[1].1;
                }
                format!(
                    "[{}]",
                    items
                        .into_iter()
                        .map(Self::runtime_display_key)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Constructor {
                variant,
                positional: _,
                fields,
                ..
            } if fields.is_empty() => variant.clone(),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } => format!(
                "{}({})",
                variant,
                fields
                    .iter()
                    .map(|(_, value)| value.runtime_display_key())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Constructor {
                variant,
                positional: false,
                fields,
                ..
            } => format!(
                "{}({})",
                variant,
                fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value.runtime_display_key()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

fn explore_value_node_count(value: &ExploreValue, cap: u64) -> u64 {
    let exceeded = cap.saturating_add(1);
    let mut count = 0_u64;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        count = count.saturating_add(1);
        if count > cap {
            return exceeded;
        }
        match value {
            ExploreValue::List(values)
            | ExploreValue::Set(values)
            | ExploreValue::Tuple(values) => stack.extend(values),
            ExploreValue::Constructor { fields, .. } => {
                stack.extend(fields.iter().map(|(_, value)| value));
            }
            _ => {}
        }
    }
    count
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreCardinality {
    Exact(u128),
    ExceedsU128,
}

impl ExploreCardinality {
    fn zero() -> Self {
        Self::Exact(0)
    }

    fn one() -> Self {
        Self::Exact(1)
    }

    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_add(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }

    fn multiply(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(0), _) | (_, Self::Exact(0)) => Self::zero(),
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_mul(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }

    pub fn exact(&self) -> Option<u128> {
        match self {
            Self::Exact(value) => Some(*value),
            Self::ExceedsU128 => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExploreEnumeratedSource {
    ExplicitList,
    NamedList { name: String },
    NamedSet { name: String },
}

#[derive(Debug, Clone)]
pub struct ExploreFiniteFieldPlan {
    pub name: String,
    pub plan: ExploreFiniteTypePlan,
}

#[derive(Debug, Clone)]
pub struct ExploreFiniteVariantPlan {
    pub name: String,
    pub positional: bool,
    pub fields: Vec<ExploreFiniteFieldPlan>,
}

/// A lazy, exact description of every inhabitant of a finite declared type.
/// It avoids allocating a large Cartesian product during type checking.
#[derive(Debug, Clone)]
pub enum ExploreFiniteTypePlan {
    Unit,
    Bool,
    Tuple {
        elements: Vec<ExploreFiniteTypePlan>,
        cardinality: ExploreCardinality,
    },
    Sum {
        type_name: String,
        variants: Vec<ExploreFiniteVariantPlan>,
        cardinality: ExploreCardinality,
    },
}

impl ExploreFiniteTypePlan {
    pub fn cardinality(&self) -> ExploreCardinality {
        match self {
            Self::Unit => ExploreCardinality::one(),
            Self::Bool => ExploreCardinality::Exact(2),
            Self::Tuple { cardinality, .. } => cardinality.clone(),
            Self::Sum { cardinality, .. } => cardinality.clone(),
        }
    }

    /// Materialize a small finite type for diagnostics/tests/replay.  The
    /// universe itself remains lazy and exact when the limit is exceeded.
    pub fn enumerate(&self, limit: usize) -> Result<Vec<ExploreValue>, String> {
        let count = self
            .cardinality()
            .exact()
            .ok_or_else(|| "finite type has more than u128::MAX inhabitants".to_string())?;
        if count > limit as u128 {
            return Err(format!(
                "finite type has {} inhabitants, exceeding materialization limit {}",
                count, limit
            ));
        }
        self.enumerate_unchecked()
    }

    fn enumerate_unchecked(&self) -> Result<Vec<ExploreValue>, String> {
        match self {
            Self::Unit => Ok(vec![ExploreValue::Unit]),
            Self::Bool => Ok(vec![
                ExploreValue::Boolean(false),
                ExploreValue::Boolean(true),
            ]),
            Self::Tuple { elements, .. } => {
                let mut combinations = vec![Vec::new()];
                for element in elements {
                    let element_values = element.enumerate_unchecked()?;
                    let mut next = Vec::new();
                    for prefix in combinations {
                        for value in &element_values {
                            let mut combined = prefix.clone();
                            combined.push(value.clone());
                            next.push(combined);
                        }
                    }
                    combinations = next;
                }
                Ok(combinations.into_iter().map(ExploreValue::Tuple).collect())
            }
            Self::Sum {
                type_name,
                variants,
                ..
            } => {
                let mut values = Vec::new();
                for variant in variants {
                    let mut combinations = vec![Vec::<(String, ExploreValue)>::new()];
                    for field in &variant.fields {
                        let field_values = field.plan.enumerate_unchecked()?;
                        let mut next = Vec::new();
                        for prefix in combinations {
                            for value in &field_values {
                                let mut combined = prefix.clone();
                                combined.push((field.name.clone(), value.clone()));
                                next.push(combined);
                            }
                        }
                        combinations = next;
                    }
                    for fields in combinations {
                        values.push(ExploreValue::Constructor {
                            type_name: type_name.clone(),
                            variant: variant.name.clone(),
                            positional: variant.positional,
                            fields,
                        });
                    }
                }
                Ok(values)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExploreExactDomain {
    Enumerated {
        values: Vec<ExploreValue>,
        source: ExploreEnumeratedSource,
    },
    IntRange {
        start: i64,
        end_exclusive: i64,
        cardinality: u64,
    },
    FiniteType {
        ty: Ty,
        plan: ExploreFiniteTypePlan,
    },
}

impl ExploreExactDomain {
    pub fn cardinality(&self) -> ExploreCardinality {
        match self {
            Self::Enumerated { values, .. } => ExploreCardinality::Exact(values.len() as u128),
            Self::IntRange { cardinality, .. } => ExploreCardinality::Exact(*cardinality as u128),
            Self::FiniteType { plan, .. } => plan.cardinality(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreDimensionIr {
    pub name: String,
    pub value_ty: Ty,
    pub domain: ExploreExactDomain,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExploreFactValue {
    Fixed(ExploreValue),
    Derived {
        expression: Expr,
        dependencies: BTreeSet<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ExploreFactIr {
    pub name: String,
    pub value_ty: Ty,
    pub value: ExploreFactValue,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreConstraintScope {
    Candidate,
    BothBoundaryEndpoints,
}

#[derive(Debug, Clone)]
pub struct ExploreConstraintIr {
    pub predicate: Expr,
    pub scope: ExploreConstraintScope,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExploreBoundaryIr {
    pub axis: String,
    pub axis_dimension_index: usize,
    pub step: i64,
    /// Both the before value and checked `before + step` value must be members
    /// of the declared axis domain.
    pub requires_both_endpoints_in_domain: bool,
    /// Source-order derived facts whose transitive dependencies include the
    /// axis.  They are recomputed after substituting the upper endpoint.
    pub recomputed_fact_indices: Vec<usize>,
    pub eligible_axis_pairs: ExploreCardinality,
    pub eligible_unconstrained_pairs: ExploreCardinality,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExploreUniverseIr {
    pub dimensions: Vec<ExploreDimensionIr>,
    pub facts: Vec<ExploreFactIr>,
    pub constraints: Vec<ExploreConstraintIr>,
    pub sliced_inputs: Vec<TypedExploreInput>,
    /// Product before `where` and before the queried rule.  This is never
    /// presented as the admissible/result count.
    pub cartesian_count_before_constraints: ExploreCardinality,
    pub boundary: Option<ExploreBoundaryIr>,
}

#[derive(Debug, Clone)]
pub struct ExploreQueryIr {
    pub query: TypedExploreQuery,
    pub universe: ExploreUniverseIr,
}

#[derive(Debug, Clone)]
struct SourcedBinding {
    expression: Expr,
    annotated_ty: Option<Ty>,
    origin: String,
}

#[derive(Debug, Clone)]
struct SourcedFunction {
    params: Vec<Param>,
    return_ty: Option<Ty>,
    effects: Vec<String>,
    body: Expr,
    origin: String,
}

#[derive(Debug, Clone, Default)]
struct GroundDefinitions {
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    functions: BTreeMap<(String, usize), Vec<SourcedFunction>>,
    rules: BTreeMap<(String, usize), Vec<String>>,
    rule_definitions: BTreeMap<(String, usize), Vec<Rule>>,
    constructors: BTreeMap<(String, usize), Vec<String>>,
    unsupported_callables: BTreeMap<(String, usize), Vec<String>>,
    unsupported_values: BTreeMap<String, Vec<String>>,
    origin_order: BTreeMap<String, usize>,
    runtime_declarations: Vec<Stmt>,
}

#[derive(Debug)]
struct ExploreGroundEvaluator<'a> {
    catalog: &'a calculate::TypeCatalog,
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    functions: BTreeMap<(String, usize), Vec<SourcedFunction>>,
    rules: BTreeMap<(String, usize), Vec<String>>,
    constructors: BTreeMap<(String, usize), Vec<String>>,
    unsupported_callables: BTreeMap<(String, usize), Vec<String>>,
    unsupported_values: BTreeMap<String, Vec<String>>,
    origin_order: BTreeMap<String, usize>,
    origin_stack: Vec<String>,
    locals: BTreeMap<String, ExploreValue>,
    memo: BTreeMap<String, ExploreValue>,
    memo_order: Vec<String>,
    visiting: Vec<String>,
    visiting_calls: Vec<(String, usize)>,
    work_remaining: u64,
}

impl<'a> ExploreGroundEvaluator<'a> {
    fn new(catalog: &'a calculate::TypeCatalog, definitions: GroundDefinitions) -> Self {
        Self {
            catalog,
            bindings: definitions.bindings,
            functions: definitions.functions,
            rules: definitions.rules,
            constructors: definitions.constructors,
            unsupported_callables: definitions.unsupported_callables,
            unsupported_values: definitions.unsupported_values,
            origin_order: definitions.origin_order,
            origin_stack: Vec::new(),
            locals: BTreeMap::new(),
            memo: BTreeMap::new(),
            memo_order: Vec::new(),
            visiting: Vec::new(),
            visiting_calls: Vec::new(),
            work_remaining: EXPLORE_GROUND_WORK_LIMIT,
        }
    }

    fn charge_work(&mut self, amount: u64, operation: &str) -> Result<(), String> {
        let Some(remaining) = self.work_remaining.checked_sub(amount) else {
            return Err(format!(
                "ground exploration {} exceeds the checked work limit {}",
                operation, EXPLORE_GROUND_WORK_LIMIT
            ));
        };
        self.work_remaining = remaining;
        Ok(())
    }

    fn charge_value_clone(&mut self, value: &ExploreValue, operation: &str) -> Result<(), String> {
        self.charge_work(
            explore_value_node_count(value, self.work_remaining),
            operation,
        )
    }

    fn ensure_origin_visible(&self, target: &str, symbol: &str) -> Result<(), String> {
        let Some(current) = self.origin_stack.last() else {
            return Ok(());
        };
        let current_order = self
            .origin_order
            .get(current)
            .copied()
            .unwrap_or(usize::MAX);
        let target_order = self.origin_order.get(target).copied().unwrap_or(usize::MAX);
        if target_order > current_order {
            return Err(format!(
                "ground exploration declaration from `{}` depends on later declaration `{}` from `{}`; imported finite data must be closed over its initialized dependency prefix",
                current, symbol, target
            ));
        }
        Ok(())
    }

    fn set_local(&mut self, name: impl Into<String>, value: ExploreValue) {
        self.locals.insert(name.into(), value);
    }

    fn eval(&mut self, expression: &Expr, expected: Option<&Ty>) -> Result<ExploreValue, String> {
        self.charge_work(1, "expression evaluation")?;
        match &expression.kind {
            ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
            ExprKind::Lit(Literal::Float(value)) => Ok(ExploreValue::FloatBits(value.to_bits())),
            ExprKind::Lit(Literal::Str(value)) => Ok(ExploreValue::String(value.clone())),
            ExprKind::Lit(Literal::Char(value)) => Ok(ExploreValue::Character(*value)),
            ExprKind::Lit(Literal::Bool(value)) => Ok(ExploreValue::Boolean(*value)),
            ExprKind::Unit => Ok(ExploreValue::Unit),
            ExprKind::List(items) => {
                if items.len() > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                    return Err(format!(
                        "ground list literal exceeds materialization limit {}",
                        EXPLORE_GROUND_COLLECTION_LIMIT
                    ));
                }
                self.charge_work(items.len() as u64, "list materialization")?;
                let item_ty = collection_item_ty(expected);
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item, item_ty.as_ref())?);
                }
                Ok(ExploreValue::List(values))
            }
            ExprKind::Tuple(items) => {
                self.charge_work(items.len() as u64, "tuple materialization")?;
                let item_tys = tuple_item_tys(expected);
                if item_tys
                    .as_ref()
                    .is_some_and(|types| types.len() != items.len())
                {
                    return Err(format!(
                        "ground tuple has {} elements but expected type `{}` has {}",
                        items.len(),
                        expected.expect("tuple types were present"),
                        item_tys.as_ref().map_or(0, Vec::len)
                    ));
                }
                let mut values = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    values.push(
                        self.eval(item, item_tys.as_ref().and_then(|types| types.get(index)))?,
                    );
                }
                Ok(ExploreValue::Tuple(values))
            }
            ExprKind::Var(name) => {
                if self.locals.contains_key(name) {
                    let nodes = explore_value_node_count(
                        self.locals.get(name).expect("checked local"),
                        self.work_remaining,
                    );
                    self.charge_work(nodes, "local value copy")?;
                    return Ok(self.locals.get(name).expect("checked local").clone());
                }
                if self.bindings.contains_key(name) && self.unsupported_values.contains_key(name) {
                    return Err(format!(
                        "ground exploration name `{}` has both an ordinary binding and a runtime value declaration; exact resolution is ambiguous",
                        name
                    ));
                }
                if self.bindings.contains_key(name) {
                    return self.eval_binding(name, expected);
                }
                if let Some(origins) = self.unsupported_values.get(name) {
                    return Err(format!(
                        "ground exploration name `{}` is shadowed by a runtime value declared in {}",
                        name,
                        origins.join(", ")
                    ));
                }
                let function_count = self
                    .functions
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, definitions)| definitions.len())
                    .sum::<usize>();
                let rule_count = self
                    .rules
                    .keys()
                    .filter(|(candidate, _)| candidate == name)
                    .count();
                let unsupported_count = self
                    .unsupported_callables
                    .keys()
                    .filter(|(candidate, _)| candidate == name)
                    .count();
                if function_count > 0 || rule_count > 0 || unsupported_count > 0 {
                    return Err(format!(
                        "ground exploration name `{}` is ambiguous between a bare value/constructor and a callable declaration",
                        name
                    ));
                }
                let constructor_count = self
                    .constructors
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, origins)| origins.len())
                    .sum::<usize>();
                if constructor_count > 1 {
                    return Err(format!(
                        "ground exploration constructor `{}` has {} visible declarations and cannot identify one exact value",
                        name, constructor_count
                    ));
                }
                if let Some(expected) = expected {
                    if let Some(value) = self.eval_nullary_constructor(expected, name)? {
                        return Ok(value);
                    }
                }
                Err(format!("unresolved ground name `{}`", name))
            }
            ExprKind::UnOp(operator, value) => {
                let value = self.eval(value, expected)?;
                match (operator.as_str(), value) {
                    ("-", ExploreValue::Int(value)) => {
                        value.checked_neg().map(ExploreValue::Int).ok_or_else(|| {
                            "integer negation overflow in exploration bound".to_string()
                        })
                    }
                    ("-", ExploreValue::FloatBits(bits)) => {
                        Ok(ExploreValue::FloatBits((-f64::from_bits(bits)).to_bits()))
                    }
                    ("+", ExploreValue::Int(value)) => Ok(ExploreValue::Int(value)),
                    ("!", ExploreValue::Boolean(value)) => Ok(ExploreValue::Boolean(!value)),
                    _ => Err(format!(
                        "unsupported unary operator `{}` in ground exploration expression",
                        operator
                    )),
                }
            }
            ExprKind::BinOp(operator, left, right) => {
                let left = self.eval(left, None)?;
                let right = self.eval(right, None)?;
                eval_ground_binary(operator, left, right)
            }
            ExprKind::If(condition, then_value, else_value) => {
                match self.eval(condition, Some(&Ty::Name("Bool".to_string())))? {
                    ExploreValue::Boolean(true) => self.eval(then_value, expected),
                    ExploreValue::Boolean(false) => self.eval(else_value, expected),
                    _ => Err("ground exploration `if` condition is not Boolean".to_string()),
                }
            }
            ExprKind::Block(statements) => self.eval_block(statements, expected),
            ExprKind::App(function, arguments) => {
                let ExprKind::Var(name) = &function.kind else {
                    return Err(
                        "qualified or computed calls are not exact ground domain expressions"
                            .to_string(),
                    );
                };
                if self.locals.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a local value; expose an unambiguous pure helper or literal finite collection",
                        name
                    ));
                }
                if self.bindings.contains_key(name) && self.unsupported_values.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` has both an ordinary binding and a runtime value declaration; exact resolution is ambiguous",
                        name
                    ));
                }
                if self.bindings.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a top-level binding; expose an unambiguous pure helper or literal finite collection",
                        name
                    ));
                }
                if let Some(origins) = self.unsupported_values.get(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a runtime value declared in {}; expose an unambiguous pure helper or literal finite collection",
                        name,
                        origins.join(", ")
                    ));
                }
                let function_key = (name.clone(), arguments.len());
                let function_count = self
                    .functions
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, definitions)| definitions.len())
                    .sum::<usize>();
                let has_function = self.functions.contains_key(&function_key);
                let constructor_origins = self
                    .constructors
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                let unsupported_origins = self
                    .unsupported_callables
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                if !unsupported_origins.is_empty() {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves to an unsupported callable from {}; expose an unambiguous pure top-level `>` helper or literal finite collection",
                        name,
                        arguments.len(),
                        unsupported_origins.join(", ")
                    ));
                }
                if let Some(origins) = self
                    .rules
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .reduce(|mut joined, origin| {
                        joined.push_str(", ");
                        joined.push_str(&origin);
                        joined
                    })
                {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves to a rule from {}; expose an unambiguous pure `>` helper or literal finite collection",
                        name,
                        arguments.len(),
                        origins
                    ));
                }
                if ground_intrinsic_arity(name).is_some() && function_count > 0 {
                    return Err(format!(
                        "ground exploration intrinsic `{}` is shadowed by a program function; exact import-time call resolution is ambiguous",
                        name
                    ));
                }
                if has_function && !constructor_origins.is_empty() {
                    return Err(format!(
                        "ground exploration call `{}` is ambiguous between a function and constructor declared in {}; expose an unambiguous pure helper",
                        name,
                        constructor_origins.join(", ")
                    ));
                }
                if has_function && function_count != 1 {
                    return Err(format!(
                        "ground exploration helper `{}` has {} declarations across arities; exact runtime resolution is ambiguous",
                        name, function_count
                    ));
                }
                if has_function {
                    return self.eval_function(name, arguments, expected);
                }
                if function_count > 0 {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves by name to a function declared with a different arity; exact runtime resolution is ambiguous",
                        name,
                        arguments.len()
                    ));
                }
                let is_intrinsic = ground_intrinsic_arity(name) == Some(arguments.len());
                if is_intrinsic && !constructor_origins.is_empty() {
                    return Err(format!(
                        "ground exploration intrinsic `{}({} arguments)` is shadowed by a constructor declared in {}; expose an unambiguous literal finite collection",
                        name,
                        arguments.len(),
                        constructor_origins.join(", ")
                    ));
                }
                if name == "range" && arguments.len() == 2 {
                    let int_ty = Ty::Name("Int".to_string());
                    let start = self
                        .eval(&arguments[0], Some(&int_ty))?
                        .int()
                        .ok_or_else(|| "ground `range` start is not an Int".to_string())?;
                    let end_exclusive = self
                        .eval(&arguments[1], Some(&int_ty))?
                        .int()
                        .ok_or_else(|| "ground `range` end is not an Int".to_string())?;
                    let cardinality = exact_range_cardinality(start, end_exclusive)?;
                    if cardinality > EXPLORE_GROUND_COLLECTION_LIMIT {
                        return Err(format!(
                            "ground `range({}, {})` has {} members, exceeding materialization limit {}; use `range` directly as the exploration domain",
                            start,
                            end_exclusive,
                            cardinality,
                            EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(cardinality, "range materialization")?;
                    let values = (0..cardinality)
                        .map(|offset| ExploreValue::Int((start as i128 + offset as i128) as i64))
                        .collect();
                    return Ok(ExploreValue::List(values));
                }
                if name == "set_from_list" && arguments.len() == 1 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_from_list` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let item_ty = collection_item_ty(expected).ok_or_else(|| {
                        "`set_from_list` ground domain needs an expected `Set(T)` type".to_string()
                    })?;
                    let list_ty = Ty::App(Box::new(Ty::Name("List".to_string())), vec![item_ty]);
                    let ExploreValue::List(values) = self.eval(&arguments[0], Some(&list_ty))?
                    else {
                        return Err("`set_from_list` argument is not a finite list".to_string());
                    };
                    self.charge_work(values.len() as u64, "set construction")?;
                    return Ok(ExploreValue::Set(runtime_set_values(values)));
                }
                if name == "set_new" && arguments.is_empty() {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err("`set_new` ground result must have type `Set(T)`".to_string());
                    }
                    return Ok(ExploreValue::Set(Vec::new()));
                }
                if name == "concat" && arguments.len() == 2 {
                    let ExploreValue::List(mut left) = self.eval(&arguments[0], expected)? else {
                        return Err("`concat` left argument is not a finite list".to_string());
                    };
                    let ExploreValue::List(right) = self.eval(&arguments[1], expected)? else {
                        return Err("`concat` right argument is not a finite list".to_string());
                    };
                    let size = left
                        .len()
                        .checked_add(right.len())
                        .ok_or_else(|| "ground `concat` collection size overflow".to_string())?;
                    if size > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                        return Err(format!(
                            "ground `concat` has {} members, exceeding materialization limit {}",
                            size, EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(size as u64, "concat materialization")?;
                    left.extend(right);
                    return Ok(ExploreValue::List(left));
                }
                if name == "distinct" && arguments.len() == 1 {
                    let ExploreValue::List(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`distinct` argument is not a finite list".to_string());
                    };
                    self.charge_work(values.len() as u64, "distinct traversal")?;
                    return Ok(ExploreValue::List(deduplicate_runtime_list(values)));
                }
                if name == "set_insert" && arguments.len() == 2 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_insert` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let ExploreValue::Set(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`set_insert` first argument is not a finite set".to_string());
                    };
                    let item_ty = collection_item_ty(expected);
                    let inserted = self.eval(&arguments[1], item_ty.as_ref())?;
                    let mut values = runtime_set_map(values);
                    values
                        .entry(inserted.runtime_display_key())
                        .or_insert(inserted);
                    if values.len() > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                        return Err(format!(
                            "ground `set_insert` has {} members, exceeding materialization limit {}",
                            values.len(),
                            EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(values.len() as u64, "set insertion")?;
                    return Ok(ExploreValue::Set(values.into_values().collect()));
                }
                if name == "set_remove" && arguments.len() == 2 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_remove` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let ExploreValue::Set(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`set_remove` first argument is not a finite set".to_string());
                    };
                    let item_ty = collection_item_ty(expected);
                    let removed = self.eval(&arguments[1], item_ty.as_ref())?;
                    self.charge_work(values.len() as u64, "set removal traversal")?;
                    let mut values = runtime_set_map(values);
                    values.remove(&removed.runtime_display_key());
                    return Ok(ExploreValue::Set(values.into_values().collect()));
                }
                self.eval_constructor(expected, name, arguments)
            }
            _ => Err(format!(
                "unsupported ground exploration expression: {:?}",
                expression.kind
            )),
        }
    }

    fn eval_block(
        &mut self,
        statements: &[Stmt],
        expected: Option<&Ty>,
    ) -> Result<ExploreValue, String> {
        let mut shadowed = Vec::new();
        let result = (|| {
            let mut result = ExploreValue::Unit;
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    Stmt::Bind(Pat::Var(name), ty, expression) => {
                        let value = self.eval(expression, ty.as_ref())?;
                        let previous = self.locals.insert(name.clone(), value);
                        shadowed.push((name.clone(), previous));
                        result = ExploreValue::Unit;
                    }
                    Stmt::Expr(expression) if index + 1 == statements.len() => {
                        result = self.eval(expression, expected)?;
                    }
                    Stmt::Expr(expression) => {
                        self.eval(expression, None)?;
                        result = ExploreValue::Unit;
                    }
                    _ => {
                        return Err(
                            "ground exploration helper blocks support only pure bindings and expressions"
                                .to_string(),
                        );
                    }
                }
            }
            Ok(result)
        })();
        for (name, previous) in shadowed.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        result
    }

    fn eval_function(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<&Ty>,
    ) -> Result<ExploreValue, String> {
        let key = (name.to_string(), arguments.len());
        let definition_count = self
            .functions
            .iter()
            .filter(|((candidate, _), _)| candidate == name)
            .map(|(_, definitions)| definitions.len())
            .sum::<usize>();
        if definition_count != 1 {
            return Err(format!(
                "ground exploration helper `{}` has {} declarations across arities; exact runtime resolution is ambiguous",
                name, definition_count
            ));
        }
        let definitions = self.functions.get(&key).cloned().unwrap_or_default();
        if definitions.len() != 1 {
            return Err(format!(
                "ground exploration helper `{}({} arguments)` has {} definitions",
                name,
                arguments.len(),
                definitions.len()
            ));
        }
        let definition = &definitions[0];
        self.ensure_origin_visible(&definition.origin, name)?;
        if self.origin_stack.len() >= EXPLORE_GROUND_RECURSION_LIMIT {
            return Err(format!(
                "ground exploration helper recursion exceeds the safe depth limit {}",
                EXPLORE_GROUND_RECURSION_LIMIT
            ));
        }
        if !definition.effects.is_empty() {
            return Err(format!(
                "ground exploration helper `{}({} arguments)` declares effects",
                name,
                arguments.len()
            ));
        }
        if let Some(start) = self
            .visiting_calls
            .iter()
            .position(|candidate| candidate == &key)
        {
            let mut cycle = self.visiting_calls[start..]
                .iter()
                .map(|(name, arity)| format!("{}({})", name, arity))
                .collect::<Vec<_>>();
            cycle.push(format!("{}({})", name, arguments.len()));
            return Err(format!(
                "recursive ground exploration helper call: {}",
                cycle.join(" -> ")
            ));
        }
        let mut values = Vec::with_capacity(arguments.len());
        for (argument, parameter) in arguments.iter().zip(&definition.params) {
            values.push(self.eval(argument, parameter.ty.as_ref())?);
        }
        let mut shadowed = Vec::new();
        for (parameter, value) in definition.params.iter().zip(values) {
            let previous = self.locals.insert(parameter.name.clone(), value);
            shadowed.push((parameter.name.clone(), previous));
        }
        self.visiting_calls.push(key);
        self.origin_stack.push(definition.origin.clone());
        let result = self.eval(&definition.body, definition.return_ty.as_ref().or(expected));
        self.origin_stack.pop();
        self.visiting_calls.pop();
        for (name, previous) in shadowed.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        result.map_err(|message| {
            format!(
                "ground exploration helper `{}` from {} failed: {}",
                name, definition.origin, message
            )
        })
    }

    fn eval_binding(&mut self, name: &str, expected: Option<&Ty>) -> Result<ExploreValue, String> {
        let definitions = self
            .bindings
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unresolved ground binding `{}`", name))?;
        if definitions.len() != 1 {
            let origins = definitions
                .iter()
                .map(|definition| definition.origin.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "ground exploration binding `{}` has {} definitions ({})",
                name,
                definitions.len(),
                origins
            ));
        }
        let definition = &definitions[0];
        self.ensure_origin_visible(&definition.origin, name)?;
        if self.memo.contains_key(name) {
            let nodes = explore_value_node_count(
                self.memo.get(name).expect("checked memoized binding"),
                self.work_remaining,
            );
            self.charge_work(nodes, "memoized binding copy")?;
            return Ok(self
                .memo
                .get(name)
                .expect("checked memoized binding")
                .clone());
        }
        if let Some(start) = self.visiting.iter().position(|candidate| candidate == name) {
            let mut cycle = self.visiting[start..].to_vec();
            cycle.push(name.to_string());
            return Err(format!(
                "cyclic ground exploration binding dependency: {}",
                cycle.join(" -> ")
            ));
        }
        if self.origin_stack.len() >= EXPLORE_GROUND_RECURSION_LIMIT {
            return Err(format!(
                "ground exploration binding recursion exceeds the safe depth limit {}",
                EXPLORE_GROUND_RECURSION_LIMIT
            ));
        }
        self.visiting.push(name.to_string());
        let expected = definition.annotated_ty.as_ref().or(expected);
        let saved_locals = std::mem::take(&mut self.locals);
        self.origin_stack.push(definition.origin.clone());
        let value = self.eval(&definition.expression, expected);
        self.origin_stack.pop();
        self.locals = saved_locals;
        self.visiting.pop();
        let value = value?;
        self.charge_value_clone(&value, "binding memoization")?;
        self.memo.insert(name.to_string(), value.clone());
        self.memo_order.push(name.to_string());
        Ok(value)
    }

    fn eval_nullary_constructor(
        &self,
        expected: &Ty,
        constructor: &str,
    ) -> Result<Option<ExploreValue>, String> {
        let Some((type_name, substitutions)) = instantiated_named_type(expected, self.catalog)?
        else {
            return Ok(None);
        };
        if self.catalog.is_rule_scope(&type_name) {
            return Err(format!(
                "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
                type_name
            ));
        }
        let constructor_origins = self
            .constructors
            .get(&(constructor.to_string(), 0))
            .cloned()
            .unwrap_or_default();
        if constructor_origins.len() == 1 {
            self.ensure_origin_visible(&constructor_origins[0], constructor)?;
        }
        for variant in self.catalog.resolved_variants(&type_name)? {
            if variant.name == constructor && variant.fields.is_empty() {
                return Ok(Some(ExploreValue::Constructor {
                    type_name,
                    variant: constructor.to_string(),
                    // Bare nullary names always evaluate as positional
                    // Value::Constructor, even when an explicit `Foo()` call
                    // uses the declaration's named-constructor shape.
                    positional: true,
                    fields: Vec::new(),
                }));
            }
        }
        let _ = substitutions;
        Ok(None)
    }

    fn eval_constructor(
        &mut self,
        expected: Option<&Ty>,
        constructor: &str,
        arguments: &[Expr],
    ) -> Result<ExploreValue, String> {
        let expected = expected.ok_or_else(|| {
            format!(
                "constructor `{}` in a ground domain needs an expected declared type",
                constructor
            )
        })?;
        let Some((type_name, substitutions)) = instantiated_named_type(expected, self.catalog)?
        else {
            return Err(format!(
                "constructor `{}` cannot inhabit primitive type `{}`",
                constructor, expected
            ));
        };
        if self.catalog.is_rule_scope(&type_name) {
            return Err(format!(
                "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
                type_name
            ));
        }
        let constructor_origins = self
            .constructors
            .get(&(constructor.to_string(), arguments.len()))
            .cloned()
            .unwrap_or_default();
        if constructor_origins.len() == 1 {
            self.ensure_origin_visible(&constructor_origins[0], constructor)?;
        }
        let variant = self
            .catalog
            .resolved_variants(&type_name)?
            .into_iter()
            .find(|variant| variant.name == constructor)
            .ok_or_else(|| {
                format!(
                    "type `{}` has no constructor `{}` in ground exploration domain",
                    expected, constructor
                )
            })?;
        if variant.fields.len() != arguments.len() {
            return Err(format!(
                "constructor `{}` expects {} fields but got {}",
                constructor,
                variant.fields.len(),
                arguments.len()
            ));
        }
        let mut values = Vec::with_capacity(arguments.len());
        if arguments
            .iter()
            .any(|argument| named_arg_parts(argument).is_some())
        {
            for field in &variant.fields {
                let argument = arguments
                    .iter()
                    .find_map(|argument| {
                        named_arg_parts(argument)
                            .filter(|(name, _)| *name == field.name)
                            .map(|(_, value)| value)
                    })
                    .ok_or_else(|| {
                        format!(
                            "constructor `{}` is missing field `{}`",
                            constructor, field.name
                        )
                    })?;
                let field_ty = calculate::substitute_type(&field.ty, &substitutions);
                values.push((field.name.clone(), self.eval(argument, Some(&field_ty))?));
            }
        } else {
            for (field, argument) in variant.fields.iter().zip(arguments) {
                let field_ty = calculate::substitute_type(&field.ty, &substitutions);
                values.push((field.name.clone(), self.eval(argument, Some(&field_ty))?));
            }
        }
        Ok(ExploreValue::Constructor {
            type_name,
            variant: variant.name,
            // A nullary variant has one semantic inhabitant.  Futuruna's
            // runtime happens to represent bare `Foo` and explicit `Foo()`
            // with different constructor layouts, but that layout detail
            // must not create two exploration-domain values.
            positional: variant.fields.is_empty() || variant.positional,
            fields: values,
        })
    }
}

struct ExploreRuntimeGroundEvaluator {
    interpreter: Interpreter,
    base_env: Env,
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    evaluated_bindings: BTreeSet<String>,
    locals: BTreeMap<String, Value>,
}

impl ExploreRuntimeGroundEvaluator {
    fn new(definitions: &GroundDefinitions) -> Self {
        let declarations = prepend_prelude(parse_prelude(), &definitions.runtime_declarations);
        let mut interpreter = Interpreter::new();
        interpreter.suppress_output = true;
        let mut base_env = interpreter.default_env();
        interpreter.register_static_declarations(&declarations, &mut base_env);
        Self {
            interpreter,
            base_env,
            bindings: definitions.bindings.clone(),
            evaluated_bindings: BTreeSet::new(),
            locals: BTreeMap::new(),
        }
    }

    fn set_local(&mut self, name: impl Into<String>, value: Value) {
        self.locals.insert(name.into(), value);
    }

    fn evaluate_required_bindings(&mut self, order: &[String]) -> Result<(), String> {
        for name in order {
            if self.evaluated_bindings.contains(name) {
                continue;
            }
            let Some(definitions) = self.bindings.get(name) else {
                return Err(format!(
                    "ground exploration binding `{}` disappeared from the checked declaration graph",
                    name
                ));
            };
            if definitions.len() != 1 {
                return Err(format!(
                    "ground exploration binding `{}` has {} definitions",
                    name,
                    definitions.len()
                ));
            }
            let value = self.interpreter.eval_ground(
                &definitions[0].expression,
                &self.base_env,
                1_000_000,
                EXPLORE_GROUND_COLLECTION_LIMIT as usize,
            )?;
            self.base_env.set(name.clone(), value);
            self.evaluated_bindings.insert(name.clone());
        }
        Ok(())
    }

    fn eval(&mut self, expression: &Expr, binding_order: &[String]) -> Result<Value, String> {
        self.evaluate_required_bindings(binding_order)?;
        let mut env = self.base_env.child();
        for (name, value) in &self.locals {
            env.set(name.clone(), value.clone());
        }
        self.interpreter.eval_ground(
            expression,
            &env,
            1_000_000,
            EXPLORE_GROUND_COLLECTION_LIMIT as usize,
        )
    }
}

fn eval_ground_exact(
    preflight: &mut ExploreGroundEvaluator<'_>,
    runtime: &mut ExploreRuntimeGroundEvaluator,
    expression: &Expr,
    expected: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<(ExploreValue, Value), String> {
    let checked_value = preflight.eval(expression, Some(expected))?;
    let runtime_value = runtime.eval(expression, &preflight.memo_order)?;
    let canonical_value = runtime_value_to_explore_value(&runtime_value, expected, catalog)?;
    if checked_value != canonical_value {
        return Err(
            "ground expression has different checked and runtime values; expose a literal finite collection or simpler pure helper"
                .to_string(),
        );
    }
    Ok((canonical_value, runtime_value))
}

fn eval_ground_binary(
    operator: &str,
    left: ExploreValue,
    right: ExploreValue,
) -> Result<ExploreValue, String> {
    match (operator, left, right) {
        ("+", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_add(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer addition overflow in exploration bound".to_string()),
        ("-", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_sub(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer subtraction overflow in exploration bound".to_string()),
        ("*", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_mul(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer multiplication overflow in exploration bound".to_string()),
        ("/", ExploreValue::Int(_), ExploreValue::Int(0)) => {
            Err("division by zero in exploration bound".to_string())
        }
        ("/", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_div(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer division overflow in exploration bound".to_string()),
        ("%", ExploreValue::Int(_), ExploreValue::Int(0)) => {
            Err("remainder by zero in exploration bound".to_string())
        }
        ("%", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_rem(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer remainder overflow in exploration bound".to_string()),
        ("<", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left < right))
        }
        ("<=", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left <= right))
        }
        (">", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left > right))
        }
        (">=", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left >= right))
        }
        ("==", left, right) => ground_runtime_equality(&left, &right)
            .map(ExploreValue::Boolean)
            .ok_or_else(|| {
                format!(
                    "ground equality does not produce a Boolean for values {:?} and {:?} under Futuruna runtime semantics",
                    left, right
                )
            }),
        ("!=", left, right) => Ok(ExploreValue::Boolean(
            ground_runtime_equality(&left, &right).map_or(true, |equal| !equal),
        )),
        ("&&", ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => {
            Ok(ExploreValue::Boolean(left && right))
        }
        ("||", ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => {
            Ok(ExploreValue::Boolean(left || right))
        }
        (operator, left, right) => Err(format!(
            "operator `{}` does not support ground values {:?} and {:?}",
            operator, left, right
        )),
    }
}

/// Mirror `Interpreter::eval_binop("==", ...)` for the first-order values
/// accepted by ground domain evaluation. `None` means ordinary execution
/// returns a non-Boolean value for this equality shape.
fn ground_runtime_equality(left: &ExploreValue, right: &ExploreValue) -> Option<bool> {
    match (left, right) {
        (ExploreValue::Int(left), ExploreValue::Int(right)) => Some(left == right),
        (ExploreValue::FloatBits(left), ExploreValue::FloatBits(right)) => {
            Some(f64::from_bits(*left) == f64::from_bits(*right))
        }
        (ExploreValue::String(left), ExploreValue::String(right)) => Some(left == right),
        (ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => Some(left == right),
        (
            ExploreValue::Constructor {
                variant: left_variant,
                positional: true,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                variant: right_variant,
                positional: true,
                fields: right_fields,
                ..
            },
        ) => Some(
            left_variant == right_variant
                && left_fields.len() == right_fields.len()
                && left_fields
                    .iter()
                    .zip(right_fields)
                    .all(|((_, left), (_, right))| {
                        ground_runtime_equality(left, right).unwrap_or(false)
                    }),
        ),
        (
            left @ ExploreValue::Constructor {
                positional: false, ..
            },
            right @ ExploreValue::Constructor {
                positional: false, ..
            },
        ) => Some(ground_values_equal(left, right)),
        (left @ ExploreValue::Constructor { .. }, right)
        | (left, right @ ExploreValue::Constructor { .. }) => {
            Some(ground_values_equal(left, right))
        }
        // Source lists and the supported list-producing helpers execute as
        // positional Cons/Nil values.  Interpreter::eval_binop therefore
        // compares each Cons field with direct runtime equality rather than
        // the broader fact-matching equality used by Value::List.
        (ExploreValue::List(left), ExploreValue::List(right)) => Some(
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| ground_runtime_equality(left, right).unwrap_or(false)),
        ),
        _ => None,
    }
}

/// Mirror `values_equal`, which is deliberately different from direct Float
/// equality when values are nested in lists or named constructors.
fn ground_values_equal(left: &ExploreValue, right: &ExploreValue) -> bool {
    match (left, right) {
        (ExploreValue::Int(left), ExploreValue::Int(right)) => left == right,
        (ExploreValue::FloatBits(left), ExploreValue::FloatBits(right)) => {
            (f64::from_bits(*left) - f64::from_bits(*right)).abs() < f64::EPSILON
        }
        (ExploreValue::String(left), ExploreValue::String(right)) => left == right,
        (ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => left == right,
        (ExploreValue::Character(left), ExploreValue::Character(right)) => left == right,
        (
            ExploreValue::Constructor {
                variant: left_variant,
                positional: left_positional,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                variant: right_variant,
                positional: right_positional,
                fields: right_fields,
                ..
            },
        ) => {
            left_positional == right_positional
                && left_variant == right_variant
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(
                    |((left_name, left), (right_name, right))| {
                        (*left_positional || left_name == right_name)
                            && ground_values_equal(left, right)
                    },
                )
        }
        (ExploreValue::List(left), ExploreValue::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| ground_values_equal(left, right))
        }
        _ => false,
    }
}

fn collection_item_ty(ty: Option<&Ty>) -> Option<Ty> {
    let Ty::App(base, arguments) = ty? else {
        return None;
    };
    if matches!(base.as_ref(), Ty::Name(name) if (name == "List" || name == "Set") && arguments.len() == 1)
    {
        arguments.first().cloned()
    } else {
        None
    }
}

fn tuple_item_tys(ty: Option<&Ty>) -> Option<Vec<Ty>> {
    let Ty::App(constructor, arguments) = ty? else {
        return None;
    };
    matches!(constructor.as_ref(), Ty::Name(name) if name == "Tuple").then(|| arguments.clone())
}

fn collection_kind(ty: &Ty) -> Option<&str> {
    let Ty::App(base, arguments) = ty else {
        return None;
    };
    match base.as_ref() {
        Ty::Name(name) if (name == "List" || name == "Set") && arguments.len() == 1 => {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn explore_value_matches_ty(
    value: &ExploreValue,
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<bool, String> {
    match ty {
        Ty::Unit => {
            return Ok(matches!(value, ExploreValue::Unit));
        }
        Ty::Name(name) if name == "Unit" => return Ok(matches!(value, ExploreValue::Unit)),
        Ty::Name(name) => {
            let primitive = match name.as_str() {
                "Int" => Some(matches!(value, ExploreValue::Int(_))),
                "Nat" => Some(matches!(value, ExploreValue::Int(number) if *number >= 0)),
                "Float" => Some(matches!(value, ExploreValue::FloatBits(_))),
                "String" => Some(matches!(value, ExploreValue::String(_))),
                "Bool" => Some(matches!(value, ExploreValue::Boolean(_))),
                "Char" => Some(matches!(value, ExploreValue::Character(_))),
                "Any" | "_" => Some(false),
                _ => None,
            };
            if let Some(matches) = primitive {
                return Ok(matches);
            }
        }
        Ty::Optional(inner) => {
            return explore_value_matches_ty(
                value,
                &Ty::App(
                    Box::new(Ty::Name("Option".to_string())),
                    vec![*inner.clone()],
                ),
                catalog,
            );
        }
        Ty::App(base, arguments) => {
            if matches!(base.as_ref(), Ty::Name(name) if name == "List") {
                let ExploreValue::List(values) = value else {
                    return Ok(false);
                };
                if arguments.len() != 1 {
                    return Ok(false);
                }
                for value in values {
                    if !explore_value_matches_ty(value, &arguments[0], catalog)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if matches!(base.as_ref(), Ty::Name(name) if name == "Set") {
                let ExploreValue::Set(values) = value else {
                    return Ok(false);
                };
                if arguments.len() != 1 {
                    return Ok(false);
                }
                for value in values {
                    if !explore_value_matches_ty(value, &arguments[0], catalog)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if matches!(base.as_ref(), Ty::Name(name) if name == "Tuple") {
                let ExploreValue::Tuple(values) = value else {
                    return Ok(false);
                };
                if values.len() != arguments.len() {
                    return Ok(false);
                }
                for (value, ty) in values.iter().zip(arguments) {
                    if !explore_value_matches_ty(value, ty, catalog)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
        }
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            return Ok(false)
        }
    }

    let Some((expected_type, substitutions)) = instantiated_named_type(ty, catalog)? else {
        return Ok(false);
    };
    if catalog.is_rule_scope(&expected_type) {
        return Err(format!(
            "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
            expected_type
        ));
    }
    let ExploreValue::Constructor {
        type_name,
        variant,
        positional,
        fields,
    } = value
    else {
        return Ok(false);
    };
    if type_name != &expected_type {
        return Ok(false);
    }
    let Some(declaration) = catalog
        .resolved_variants(&expected_type)?
        .into_iter()
        .find(|candidate| candidate.name == *variant)
    else {
        return Ok(false);
    };
    if declaration.fields.len() != fields.len()
        || (!declaration.fields.is_empty() && declaration.positional != *positional)
    {
        return Ok(false);
    }
    for (field, (actual_name, actual_value)) in declaration.fields.iter().zip(fields) {
        if field.name != *actual_name {
            return Ok(false);
        }
        let field_ty = calculate::substitute_type(&field.ty, &substitutions);
        if !explore_value_matches_ty(actual_value, &field_ty, catalog)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn strict_runtime_list_items(value: &Value) -> Result<Vec<&Value>, String> {
    if let Value::List(items) = value {
        return Ok(items.iter().collect());
    }
    let mut items = Vec::new();
    let mut current = value;
    loop {
        match current {
            Value::Constructor(name, fields) if name == "Nil" && fields.is_empty() => {
                return Ok(items)
            }
            Value::Constructor(name, fields) if name == "Cons" && fields.len() == 2 => {
                if items.len() >= EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                    return Err(format!(
                        "ground list exceeds materialization limit {}",
                        EXPLORE_GROUND_COLLECTION_LIMIT
                    ));
                }
                items.push(&fields[0]);
                current = &fields[1];
            }
            _ => {
                return Err(
                    "ground List value is not a complete Cons/Nil chain or runtime List"
                        .to_string(),
                )
            }
        }
    }
}

fn runtime_value_to_explore_value(
    value: &Value,
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<ExploreValue, String> {
    match ty {
        Ty::Unit => {
            return matches!(value, Value::Unit)
                .then_some(ExploreValue::Unit)
                .ok_or_else(|| "runtime value does not have type Unit".to_string())
        }
        Ty::Name(name) => {
            let primitive = match (name.as_str(), value) {
                ("Unit", Value::Unit) => Some(ExploreValue::Unit),
                ("Int", Value::Int(value)) => Some(ExploreValue::Int(*value)),
                ("Nat", Value::Int(value)) if *value >= 0 => Some(ExploreValue::Int(*value)),
                ("Float", Value::Float(value)) => Some(ExploreValue::FloatBits(value.to_bits())),
                ("String", Value::Str(value)) => Some(ExploreValue::String(value.clone())),
                ("Bool", Value::Bool(value)) => Some(ExploreValue::Boolean(*value)),
                ("Char", Value::Char(value)) => Some(ExploreValue::Character(*value)),
                ("Any" | "_", _) => {
                    return Err(format!(
                        "runtime ground value cannot use open exploration type `{}`",
                        name
                    ))
                }
                _ => None,
            };
            if let Some(primitive) = primitive {
                return Ok(primitive);
            }
            if matches!(
                name.as_str(),
                "Unit" | "Int" | "Nat" | "Float" | "String" | "Bool" | "Char"
            ) {
                return Err(format!("runtime value does not have type `{}`", name));
            }
        }
        Ty::Optional(inner) => {
            return runtime_value_to_explore_value(
                value,
                &Ty::App(
                    Box::new(Ty::Name("Option".to_string())),
                    vec![*inner.clone()],
                ),
                catalog,
            )
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "List") => {
            if arguments.len() != 1 {
                return Err(format!("invalid ground List type `{}`", ty));
            }
            let items = strict_runtime_list_items(value)?;
            let mut converted = Vec::with_capacity(items.len());
            for (index, item) in items.into_iter().enumerate() {
                converted.push(
                    runtime_value_to_explore_value(item, &arguments[0], catalog).map_err(|_| {
                        format!(
                            "ground list member {} does not have declared type `{}`",
                            index + 1,
                            arguments[0]
                        )
                    })?,
                );
            }
            return Ok(ExploreValue::List(converted));
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "Set") => {
            if arguments.len() != 1 {
                return Err(format!("invalid ground Set type `{}`", ty));
            }
            let Value::Set(items) = value else {
                return Err(format!("runtime value does not have type `{}`", ty));
            };
            let mut converted = Vec::with_capacity(items.len());
            for (index, item) in items.values().enumerate() {
                converted.push(
                    runtime_value_to_explore_value(item, &arguments[0], catalog).map_err(|_| {
                        format!(
                            "ground set member {} does not have declared type `{}`",
                            index + 1,
                            arguments[0]
                        )
                    })?,
                );
            }
            return Ok(ExploreValue::Set(converted));
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "Tuple") => {
            let Value::Tuple(items) = value else {
                return Err(format!("runtime value does not have type `{}`", ty));
            };
            if items.len() != arguments.len() {
                return Err(format!(
                    "runtime tuple has {} fields but `{}` requires {}",
                    items.len(),
                    ty,
                    arguments.len()
                ));
            }
            return items
                .iter()
                .zip(arguments)
                .map(|(item, ty)| runtime_value_to_explore_value(item, ty, catalog))
                .collect::<Result<Vec<_>, _>>()
                .map(ExploreValue::Tuple);
        }
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            return Err(format!(
                "runtime ground value cannot use unsupported exploration type `{}`",
                ty
            ))
        }
        _ => {}
    }

    let Some((type_name, substitutions)) = instantiated_named_type(ty, catalog)? else {
        return Err(format!(
            "runtime value cannot be converted to declared type `{}`",
            ty
        ));
    };
    if catalog.is_rule_scope(&type_name) {
        return Err(format!(
            "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
            type_name
        ));
    }
    let (variant_name, positional, runtime_fields): (&str, bool, Vec<(&str, &Value)>) = match value
    {
        Value::Constructor(name, fields) => (
            name,
            true,
            fields
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let _ = index;
                    ("", value)
                })
                .collect(),
        ),
        Value::NamedConstructor(name, fields) => (
            name,
            false,
            fields
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
        ),
        _ => {
            return Err(format!(
                "runtime value does not have declared type `{}`",
                ty
            ))
        }
    };
    let declaration = catalog
        .resolved_variants(&type_name)?
        .into_iter()
        .find(|variant| variant.name == variant_name)
        .ok_or_else(|| {
            format!(
                "runtime constructor `{}` does not inhabit declared type `{}`",
                variant_name, ty
            )
        })?;
    if runtime_fields.len() != declaration.fields.len()
        || (!declaration.fields.is_empty() && declaration.positional != positional)
    {
        return Err(format!(
            "runtime constructor `{}` has a shape incompatible with `{}`",
            variant_name, ty
        ));
    }
    let mut fields = Vec::with_capacity(declaration.fields.len());
    for (index, field) in declaration.fields.iter().enumerate() {
        let runtime_value = if positional {
            runtime_fields[index].1
        } else {
            runtime_fields
                .iter()
                .find(|(name, _)| *name == field.name)
                .map(|(_, value)| *value)
                .ok_or_else(|| {
                    format!(
                        "runtime constructor `{}` is missing field `{}`",
                        variant_name, field.name
                    )
                })?
        };
        let field_ty = calculate::substitute_type(&field.ty, &substitutions);
        fields.push((
            field.name.clone(),
            runtime_value_to_explore_value(runtime_value, &field_ty, catalog)?,
        ));
    }
    Ok(ExploreValue::Constructor {
        type_name,
        variant: variant_name.to_string(),
        // Normalize both runtime spellings of a nullary constructor to the
        // single declared inhabitant used by finite-type enumeration.
        positional: declaration.fields.is_empty() || positional,
        fields,
    })
}

fn instantiated_named_type(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<Option<(String, BTreeMap<String, Ty>)>, String> {
    let (name, arguments) = match ty {
        Ty::Name(name) => (name.clone(), Vec::new()),
        Ty::App(base, arguments) => {
            let Ty::Name(name) = base.as_ref() else {
                return Ok(None);
            };
            (name.clone(), arguments.clone())
        }
        Ty::Optional(inner) => ("Option".to_string(), vec![*inner.clone()]),
        _ => return Ok(None),
    };
    if !catalog.contains_type(&name) {
        return Ok(None);
    }
    let parameters = catalog.type_parameters(&name)?;
    if parameters.len() != arguments.len() {
        return Err(format!(
            "type `{}` expects {} arguments but got {}",
            name,
            parameters.len(),
            arguments.len()
        ));
    }
    Ok(Some((
        name,
        parameters.into_iter().zip(arguments).collect(),
    )))
}

fn collect_declared_type_dependencies(ty: &Ty, dependencies: &mut BTreeSet<String>) {
    match ty {
        Ty::Name(name) => {
            dependencies.insert(name.clone());
        }
        Ty::App(base, arguments) => {
            collect_declared_type_dependencies(base, dependencies);
            for argument in arguments {
                collect_declared_type_dependencies(argument, dependencies);
            }
        }
        Ty::Optional(inner) => {
            dependencies.insert("Option".to_string());
            collect_declared_type_dependencies(inner, dependencies);
        }
        Ty::Arrow(input, output) => {
            collect_declared_type_dependencies(input, dependencies);
            collect_declared_type_dependencies(output, dependencies);
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
            collect_declared_type_dependencies(inner, dependencies)
        }
        Ty::Var(_) | Ty::Unit | Ty::Hole => {}
    }
}

fn declaration_reaches_type(
    catalog: &calculate::TypeCatalog,
    current: &str,
    target: &str,
    visiting: &mut BTreeSet<String>,
) -> Result<bool, String> {
    if visiting.len() >= EXPLORE_RECURSION_LIMIT {
        return Err(format!(
            "finite type dependency exceeds the safe depth limit {}",
            EXPLORE_RECURSION_LIMIT
        ));
    }
    if !visiting.insert(current.to_string()) {
        return Ok(false);
    }
    for variant in catalog.resolved_variants(current)? {
        for field in variant.fields {
            let mut dependencies = BTreeSet::new();
            collect_declared_type_dependencies(&field.ty, &mut dependencies);
            for dependency in dependencies {
                if dependency == target {
                    visiting.remove(current);
                    return Ok(true);
                }
                if catalog.type_parameters(&dependency).is_ok()
                    && declaration_reaches_type(catalog, &dependency, target, visiting)?
                {
                    visiting.remove(current);
                    return Ok(true);
                }
            }
        }
    }
    visiting.remove(current);
    Ok(false)
}

fn finite_type_plan(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
    path: &str,
    active: &mut BTreeSet<String>,
) -> Result<ExploreFiniteTypePlan, String> {
    let mut budget = EXPLORE_FINITE_PLAN_WORK_LIMIT;
    finite_type_plan_with_budget(ty, catalog, path, active, &mut budget, 0)
}

fn finite_type_plan_with_budget(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
    path: &str,
    active: &mut BTreeSet<String>,
    budget: &mut usize,
    depth: usize,
) -> Result<ExploreFiniteTypePlan, String> {
    if depth >= EXPLORE_RECURSION_LIMIT {
        return Err(format!(
            "`values({})` exceeds the finite-type depth limit {}",
            ty, EXPLORE_RECURSION_LIMIT
        ));
    }
    let Some(remaining) = budget.checked_sub(1) else {
        return Err(format!(
            "`values({})` exceeds the finite-type plan work limit {}",
            ty, EXPLORE_FINITE_PLAN_WORK_LIMIT
        ));
    };
    *budget = remaining;
    match ty {
        Ty::Unit => return Ok(ExploreFiniteTypePlan::Unit),
        Ty::Name(name) if name == "Unit" => return Ok(ExploreFiniteTypePlan::Unit),
        Ty::Name(name) if name == "Bool" => return Ok(ExploreFiniteTypePlan::Bool),
        Ty::App(constructor, elements) if matches!(constructor.as_ref(), Ty::Name(name) if name == "Tuple") =>
        {
            let identity = ty.to_string();
            if !active.insert(identity.clone()) {
                return Err(format!(
                    "`values({})` is recursive through `{}` and is not finite",
                    ty, path
                ));
            }
            let mut plans = Vec::with_capacity(elements.len());
            let mut cardinality = ExploreCardinality::one();
            for (index, element) in elements.iter().enumerate() {
                let plan = finite_type_plan_with_budget(
                    element,
                    catalog,
                    &format!("{}[{}]", path, index),
                    active,
                    budget,
                    depth + 1,
                )?;
                cardinality = cardinality.multiply(plan.cardinality());
                plans.push(plan);
            }
            active.remove(&identity);
            return Ok(ExploreFiniteTypePlan::Tuple {
                elements: plans,
                cardinality,
            });
        }
        Ty::Optional(inner) => {
            return finite_type_plan_with_budget(
                &Ty::App(
                    Box::new(Ty::Name("Option".to_string())),
                    vec![*inner.clone()],
                ),
                catalog,
                path,
                active,
                budget,
                depth + 1,
            )
        }
        Ty::Name(name)
            if matches!(
                name.as_str(),
                "Int"
                    | "Nat"
                    | "Any"
                    | "Float"
                    | "String"
                    | "Char"
                    | "List"
                    | "Set"
                    | "Map"
                    | "Stream"
            ) =>
        {
            return Err(format!(
                "`values({})` is unbounded at `{}`; provide an explicit list or range",
                ty, path
            ))
        }
        Ty::App(base, _) if matches!(base.as_ref(), Ty::Name(name) if matches!(name.as_str(), "List" | "Set" | "Map" | "Stream")) => {
            return Err(format!(
                "`values({})` is unbounded at `{}`; provide an explicit finite collection",
                ty, path
            ))
        }
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            return Err(format!(
                "`values({})` cannot enumerate `{}` at `{}`",
                ty, ty, path
            ))
        }
        _ => {}
    }

    let identity = ty.to_string();
    if !active.insert(identity.clone()) {
        return Err(format!(
            "`values({})` is recursive through `{}` and is not finite",
            ty, path
        ));
    }
    let Some((type_name, substitutions)) = instantiated_named_type(ty, catalog)? else {
        active.remove(&identity);
        return Err(format!("`values({})` names an unknown finite type", ty));
    };
    if declaration_reaches_type(catalog, &type_name, &type_name, &mut BTreeSet::new())? {
        active.remove(&identity);
        return Err(format!(
            "`values({})` cannot enumerate recursive declared type `{}`",
            ty, type_name
        ));
    }
    if catalog.is_rule_scope(&type_name) {
        active.remove(&identity);
        return Err(format!(
            "`values({})` cannot enumerate rule scope `{}`",
            ty, type_name
        ));
    }
    let variants = catalog.resolved_variants(&type_name)?;
    let mut seen_variants = BTreeSet::new();
    let mut plans = Vec::with_capacity(variants.len());
    let mut total = ExploreCardinality::zero();
    for variant in variants {
        let Some(remaining) = budget.checked_sub(1) else {
            active.remove(&identity);
            return Err(format!(
                "`values({})` exceeds the finite-type plan work limit {}",
                ty, EXPLORE_FINITE_PLAN_WORK_LIMIT
            ));
        };
        *budget = remaining;
        if !seen_variants.insert(variant.name.clone()) {
            active.remove(&identity);
            return Err(format!(
                "finite type `{}` resolves constructor `{}` more than once",
                type_name, variant.name
            ));
        }
        let canonical_positional = variant.fields.is_empty() || variant.positional;
        let mut fields = Vec::with_capacity(variant.fields.len());
        let mut count = ExploreCardinality::one();
        for field in variant.fields {
            let field_ty = calculate::substitute_type(&field.ty, &substitutions);
            let field_path = format!("{}.{}.{}", path, variant.name, field.name);
            let plan = finite_type_plan_with_budget(
                &field_ty,
                catalog,
                &field_path,
                active,
                budget,
                depth + 1,
            )?;
            count = count.multiply(plan.cardinality());
            fields.push(ExploreFiniteFieldPlan {
                name: field.name,
                plan,
            });
        }
        total = total.add(count);
        plans.push(ExploreFiniteVariantPlan {
            name: variant.name,
            positional: canonical_positional,
            fields,
        });
    }
    active.remove(&identity);
    Ok(ExploreFiniteTypePlan::Sum {
        type_name,
        variants: plans,
        cardinality: total,
    })
}

fn collect_ground_bindings(
    statements: &[Stmt],
    source_dir: Option<&str>,
) -> Result<GroundDefinitions, Vec<String>> {
    let mut definitions = GroundDefinitions::default();
    let mut visited = BTreeSet::new();
    let mut errors = Vec::new();
    collect_ground_bindings_inner(
        statements,
        source_dir,
        "<root>",
        &mut visited,
        &mut definitions,
        &mut errors,
    );
    if errors.is_empty() {
        Ok(definitions)
    } else {
        Err(errors)
    }
}

fn ground_declaration_identity(statement: &Stmt) -> Option<(String, String, String)> {
    match statement {
        Stmt::Defn(definition) => {
            let name = match definition {
                Defn::Fn { name, .. } | Defn::Actor { name, .. } | Defn::Module { name, .. } => {
                    name
                }
            };
            Some((
                "definition".to_string(),
                name.clone(),
                content_hash_defn(definition),
            ))
        }
        Stmt::TypeDecl(declaration) => {
            let (kind, name) = match declaration {
                TypeDecl::ADT { name, .. } => ("adt", name),
                TypeDecl::WhenType { name, .. } => ("when", name),
                TypeDecl::EffectDecl { name, .. } => ("effect", name),
                TypeDecl::TraitDecl { name, .. } => ("trait", name),
                TypeDecl::ImplBlock {
                    trait_name,
                    for_type,
                    ..
                } => {
                    return Some((
                        "impl".to_string(),
                        format!("{} for {}", trait_name, for_type),
                        content_hash_type(declaration),
                    ))
                }
                TypeDecl::RuleScope { name, .. } => ("rule-scope", name),
            };
            Some((
                kind.to_string(),
                name.clone(),
                content_hash_type(declaration),
            ))
        }
        _ => None,
    }
}

fn standard_prelude_declaration_identities() -> Vec<(String, String, String)> {
    parse_prelude()
        .iter()
        .filter_map(ground_declaration_identity)
        .collect()
}

fn leading_injected_prelude_indices(statements: &[Stmt], origin: &str) -> BTreeSet<usize> {
    if origin != "<root>" {
        return BTreeSet::new();
    }
    let prelude = standard_prelude_declaration_identities();
    let mut cursor = 0;
    let mut indices = BTreeSet::new();
    for (index, statement) in statements.iter().enumerate() {
        let Some(identity) = ground_declaration_identity(statement) else {
            break;
        };
        let Some(relative) = prelude[cursor..]
            .iter()
            .position(|candidate| candidate == &identity)
        else {
            break;
        };
        cursor += relative + 1;
        indices.insert(index);
    }
    indices
}

fn collect_ground_bindings_inner(
    statements: &[Stmt],
    source_dir: Option<&str>,
    origin: &str,
    visited: &mut BTreeSet<String>,
    definitions: &mut GroundDefinitions,
    errors: &mut Vec<String>,
) {
    let injected_prelude = leading_injected_prelude_indices(statements, origin);
    if !injected_prelude.is_empty() && !definitions.origin_order.contains_key("<prelude>") {
        let next = definitions.origin_order.len();
        definitions
            .origin_order
            .insert("<prelude>".to_string(), next);
    }
    let mut saw_local_program_statement = false;
    for (index, statement) in statements.iter().enumerate() {
        if injected_prelude.contains(&index) {
            continue;
        }
        match statement {
            Stmt::Import(path) | Stmt::HashImport(_, path) => {
                if saw_local_program_statement {
                    errors.push(format!(
                        "exploration import `{}` appears after a local declaration or executable statement; exact ground evaluation requires imports in the module prefix",
                        path
                    ));
                }
            }
            Stmt::Annot(_, _)
            | Stmt::Use(_)
            | Stmt::RustBlock(_)
            | Stmt::Depend(_, _)
            | Stmt::QualifiedImport(_, _) => {}
            _ => saw_local_program_statement = true,
        }
    }

    for statement in statements {
        match statement {
            Stmt::Import(path) => {
                let Some(directory) = source_dir else {
                    errors.push(format!(
                        "cannot resolve exploration import `{}` without a source directory",
                        path
                    ));
                    continue;
                };
                let Some(file_path) = Interpreter::resolve_import_path_for_source(path, directory)
                else {
                    errors.push(format!("cannot resolve exploration import `{}`", path));
                    continue;
                };
                let canonical = std::fs::canonicalize(&file_path)
                    .unwrap_or_else(|_| PathBuf::from(&file_path))
                    .to_string_lossy()
                    .to_string();
                if !visited.insert(canonical.clone()) {
                    continue;
                }
                let module = match parse_source_module_file_cached(Path::new(&file_path)) {
                    Ok(module) => module,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let nested_dir = Path::new(&file_path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string());
                collect_ground_bindings_inner(
                    module.statements(),
                    Some(&nested_dir),
                    &canonical,
                    visited,
                    definitions,
                    errors,
                );
            }
            Stmt::HashImport(hash, path) => {
                let Some(directory) = source_dir else {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}` without a source directory",
                        hash, path
                    ));
                    continue;
                };
                let Some(file_path) = Interpreter::resolve_import_path_for_source(path, directory)
                else {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}`",
                        hash, path
                    ));
                    continue;
                };
                let canonical = std::fs::canonicalize(&file_path)
                    .unwrap_or_else(|_| PathBuf::from(&file_path))
                    .to_string_lossy()
                    .to_string();
                let import_key = format!("{}#{}", canonical, hash);
                if !visited.insert(import_key.clone()) {
                    continue;
                }
                let module = match parse_source_module_file_cached(Path::new(&file_path)) {
                    Ok(module) => module,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let matched = module
                    .statements()
                    .iter()
                    .filter(|statement| match statement {
                        Stmt::Defn(definition) => content_hash_defn(definition) == *hash,
                        Stmt::TypeDecl(declaration) => content_hash_type(declaration) == *hash,
                        _ => false,
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if matched.len() != 1 {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}`: expected exactly one matching definition, found {}",
                        hash,
                        path,
                        matched.len()
                    ));
                    continue;
                }
                let nested_dir = Path::new(&file_path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string());
                collect_ground_bindings_inner(
                    &matched,
                    Some(&nested_dir),
                    &import_key,
                    visited,
                    definitions,
                    errors,
                );
            }
            _ => {}
        }
    }

    if !definitions.origin_order.contains_key(origin) {
        let next = definitions.origin_order.len();
        definitions.origin_order.insert(origin.to_string(), next);
    }

    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Bind(Pat::Var(name), annotated_ty, expression) = statement else {
            continue;
        };
        definitions
            .bindings
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push(SourcedBinding {
                expression: expression.clone(),
                annotated_ty: annotated_ty.clone(),
                origin: statement_origin.to_string(),
            });
    }
    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let mut names = BTreeSet::new();
        match statement {
            Stmt::Bind(pattern, _, _) if !matches!(pattern, Pat::Var(_)) => {
                collect_pattern_names(pattern, &mut names)
            }
            Stmt::MonadicBind(pattern, _, _) => collect_pattern_names(pattern, &mut names),
            Stmt::StreamBind(name, _)
            | Stmt::QualifiedImport(name, _)
            | Stmt::Defn(Defn::Actor { name, .. })
            | Stmt::Defn(Defn::Module { name, .. })
            | Stmt::Rule(Rule::ReactiveScope { name, .. }) => {
                names.insert(name.clone());
            }
            _ => {}
        }
        for name in names {
            definitions
                .unsupported_values
                .entry(name)
                .or_insert_with(Vec::new)
                .push(statement_origin.to_string());
        }
    }
    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Defn(Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        }) = statement
        else {
            continue;
        };
        definitions
            .functions
            .entry((name.clone(), params.len()))
            .or_insert_with(Vec::new)
            .push(SourcedFunction {
                params: params.clone(),
                return_ty: ret_ty.clone(),
                effects: effects.clone(),
                body: body.clone(),
                origin: statement_origin.to_string(),
            });
    }
    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Rule(rule) = statement else {
            continue;
        };
        let Some((name, arity)) = ground_rule_name_arity(rule) else {
            continue;
        };
        definitions
            .rules
            .entry((name.clone(), arity))
            .or_insert_with(Vec::new)
            .push(statement_origin.to_string());
        definitions
            .rule_definitions
            .entry((name, arity))
            .or_insert_with(Vec::new)
            .push(rule.clone());
    }
    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        match statement {
            Stmt::Defn(Defn::Actor { name, handlers, .. }) => {
                definitions
                    .unsupported_callables
                    .entry((name.clone(), handlers.len()))
                    .or_insert_with(Vec::new)
                    .push(statement_origin.to_string());
            }
            Stmt::TypeDecl(TypeDecl::ADT {
                variants, methods, ..
            }) => {
                for variant in variants {
                    definitions
                        .constructors
                        .entry((variant.name.clone(), variant.fields.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
                record_unsupported_methods(methods, statement_origin, definitions);
            }
            Stmt::TypeDecl(TypeDecl::WhenType { variants, .. }) => {
                for variant in variants {
                    definitions
                        .constructors
                        .entry((variant.name.clone(), variant.fields.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                record_unsupported_methods(methods, statement_origin, definitions);
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { ops, .. }) => {
                for (name, parameters, _) in ops {
                    definitions
                        .unsupported_callables
                        .entry((name.clone(), parameters.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
            }
            Stmt::TypeDecl(TypeDecl::RuleScope {
                name, params, body, ..
            }) => {
                definitions
                    .constructors
                    .entry((name.clone(), params.len()))
                    .or_insert_with(Vec::new)
                    .push(statement_origin.to_string());
                for member in body {
                    if let Stmt::Defn(Defn::Fn { name, params, .. }) = member {
                        definitions
                            .unsupported_callables
                            .entry((name.clone(), params.len()))
                            .or_insert_with(Vec::new)
                            .push(statement_origin.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    definitions
        .runtime_declarations
        .extend(
            statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    (!injected_prelude.contains(&index)
                        && matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Rule(_)))
                    .then(|| statement.clone())
                }),
        );
}

fn record_unsupported_methods(methods: &[Defn], origin: &str, definitions: &mut GroundDefinitions) {
    for method in methods {
        if let Defn::Fn { name, params, .. } = method {
            definitions
                .unsupported_callables
                .entry((name.clone(), params.len()))
                .or_insert_with(Vec::new)
                .push(origin.to_string());
        }
    }
}

fn ground_rule_name_arity(rule: &Rule) -> Option<(String, usize)> {
    let head = match rule {
        Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. } => {
            head
        }
        Rule::ReactiveScope { .. } => return None,
    };
    match &head.kind {
        ExprKind::Var(name) => Some((name.clone(), 0)),
        ExprKind::App(function, arguments) => {
            let ExprKind::Var(name) = &function.kind else {
                return None;
            };
            Some((name.clone(), arguments.len()))
        }
        _ => None,
    }
}

fn ground_intrinsic_arity(name: &str) -> Option<usize> {
    match name {
        "range" => Some(2),
        "set_from_list" | "distinct" => Some(1),
        "set_new" => Some(0),
        "concat" | "set_insert" | "set_remove" => Some(2),
        _ => None,
    }
}

fn collect_ground_rule_pattern_names(expression: &Expr, names: &mut BTreeSet<String>) {
    if let ExprKind::App(function, arguments) = &expression.kind {
        if matches!(&function.kind, ExprKind::Var(name) if name == "__typed")
            && arguments.len() == 2
        {
            collect_ground_rule_pattern_names(&arguments[0], names);
            return;
        }
    }
    match &expression.kind {
        ExprKind::Var(name)
            if name != "_" && !name.chars().next().is_some_and(char::is_uppercase) =>
        {
            names.insert(name.clone());
        }
        ExprKind::App(_, arguments) | ExprKind::Tuple(arguments) => {
            for argument in arguments {
                collect_ground_rule_pattern_names(argument, names);
            }
        }
        _ => {}
    }
}

fn ground_rule_bound_names(rule: &Rule) -> BTreeSet<String> {
    let (head, body) = match rule {
        Rule::Clause { head, body } => (head, body.as_ref()),
        Rule::Default { head, .. } | Rule::Exception { head, .. } => (head, None),
        Rule::ReactiveScope { .. } => return BTreeSet::new(),
    };
    let mut bound = BTreeSet::new();
    if let ExprKind::App(_, arguments) = &head.kind {
        for argument in arguments {
            collect_ground_rule_pattern_names(argument, &mut bound);
        }
    }

    // Rule conjunction/disjunction goals introduce logic variables in the
    // same places that Interpreter::apply_rule clears from the caller env.
    fn collect_goal_names(expression: &Expr, names: &mut BTreeSet<String>) {
        match &expression.kind {
            ExprKind::Conjunction(goals) | ExprKind::Disjunction(goals) => {
                for goal in goals {
                    collect_goal_names(goal, names);
                }
            }
            ExprKind::App(_, arguments) => {
                for argument in arguments {
                    collect_ground_rule_pattern_names(argument, names);
                }
            }
            _ => {}
        }
    }
    if body.is_some_and(|body| {
        matches!(
            &body.kind,
            ExprKind::Conjunction(_) | ExprKind::Disjunction(_)
        )
    }) {
        collect_goal_names(body.expect("checked rule body"), &mut bound);
    }
    bound
}

fn ground_rule_expressions(rule: &Rule) -> Vec<&Expr> {
    match rule {
        Rule::Clause { body, .. } => body.iter().collect(),
        Rule::Default {
            value, condition, ..
        }
        | Rule::Exception {
            value, condition, ..
        } => std::iter::once(value).chain(condition.iter()).collect(),
        Rule::ReactiveScope { .. } => Vec::new(),
    }
}

fn expression_query_dependencies(
    expression: &Expr,
    names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
) -> BTreeSet<String> {
    let mut free = BTreeSet::new();
    collect_true_free_vars(expression, &mut free, &BTreeSet::new());
    free.retain(|name| names.contains(name));
    let mut memo = BTreeMap::new();
    let mut work_remaining = EXPLORE_FINITE_PLAN_WORK_LIMIT;
    free.extend(expression_dynamic_helper_dependencies(
        expression,
        names,
        definitions,
        &mut BTreeSet::new(),
        &mut memo,
        &mut work_remaining,
        0,
    ));
    free
}

fn expression_bare_runtime_calls(expression: &Expr) -> Vec<(String, usize)> {
    // A pipe adds its input as the first argument at runtime.  Remember the
    // transform roots so their nested `App` node is not also recorded with the
    // source-only arity.
    let mut pipe_transform_roots = BTreeSet::new();
    walk_ast_expr(expression, &mut |child| {
        let AstChild::Expr(expression) = child else {
            return;
        };
        if let ExprKind::Pipe(_, transform) = &expression.kind {
            pipe_transform_roots.insert(transform.as_ref() as *const Expr as usize);
        }
    });

    let mut calls = Vec::new();
    walk_ast_expr(expression, &mut |child| {
        let AstChild::Expr(expression) = child else {
            return;
        };
        match &expression.kind {
            ExprKind::App(function, arguments)
                if !pipe_transform_roots.contains(&(expression as *const Expr as usize)) =>
            {
                if let ExprKind::Var(name) = &function.kind {
                    calls.push((name.clone(), arguments.len()));
                }
            }
            ExprKind::Pipe(_, transform) => match &transform.kind {
                ExprKind::Var(name) => calls.push((name.clone(), 1)),
                ExprKind::App(function, arguments) => {
                    if let ExprKind::Var(name) = &function.kind {
                        calls.push((name.clone(), arguments.len() + 1));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    });
    calls
}

fn explore_replay_callable_identity_issue(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    validated: &mut BTreeSet<(String, usize)>,
) -> Option<String> {
    let key = (name.to_string(), arity);
    if validated.contains(&key) || !visiting.insert(key.clone()) {
        return None;
    }

    let function_arities = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .flat_map(|((_, arity), declarations)| std::iter::repeat_n(*arity, declarations.len()))
        .collect::<Vec<_>>();
    let issue = if function_arities.len() > 1 {
        let declared_arities = function_arities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|arity| arity.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "exploration replay cannot resolve helper `{}({} argument{})` exactly: `{}` has declarations across arities ({}), but ordinary runtime functions resolve by bare name; give every reachable helper a unique name",
            name,
            arity,
            if arity == 1 { "" } else { "s" },
            name,
            declared_arities
        ))
    } else if function_arities.len() == 1 {
        let exact = definitions.functions.get(&key);
        if exact.is_none_or(|declarations| declarations.len() != 1) {
            Some(format!(
                "exploration replay call `{}({} argument{})` resolves by signature to a different callable, but a different-arity ordinary function with the same runtime name shadows it",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ))
        } else if definitions.bindings.contains_key(name) {
            Some(format!(
                "exploration replay call `{}` is shadowed by a top-level binding; ordinary runtime functions resolve by bare name",
                name
            ))
        } else if definitions
            .rules
            .keys()
            .any(|(candidate, _)| candidate == name)
        {
            Some(format!(
                "exploration replay call `{}` is ambiguous between a function and rule sharing one runtime name",
                name
            ))
        } else if definitions
            .constructors
            .keys()
            .any(|(candidate, _)| candidate == name)
        {
            Some(format!(
                "exploration replay call `{}` is ambiguous between a function and constructor sharing one runtime name",
                name
            ))
        } else if definitions
            .unsupported_callables
            .keys()
            .any(|(candidate, _)| candidate == name)
        {
            Some(format!(
                "exploration replay call `{}` collides with an unsupported callable sharing one runtime name",
                name
            ))
        } else if definitions.unsupported_values.contains_key(name) {
            Some(format!(
                "exploration replay call `{}` is shadowed by a runtime value declaration",
                name
            ))
        } else if ground_intrinsic_arity(name).is_some() {
            Some(format!(
                "exploration replay helper `{}` shadows a built-in intrinsic with the same runtime name",
                name
            ))
        } else {
            let definition = &exact.expect("one exact helper definition")[0];
            expression_replay_callable_identity_issue(
                &definition.body,
                definitions,
                visiting,
                validated,
            )
        }
    } else if let Some(rules) = definitions.rule_definitions.get(&key) {
        rules.iter().find_map(|rule| {
            ground_rule_expressions(rule)
                .into_iter()
                .find_map(|expression| {
                    expression_replay_callable_identity_issue(
                        expression,
                        definitions,
                        visiting,
                        validated,
                    )
                })
        })
    } else {
        None
    };

    visiting.remove(&key);
    if issue.is_none() {
        validated.insert(key);
    }
    issue
}

fn expression_replay_callable_identity_issue(
    expression: &Expr,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    validated: &mut BTreeSet<(String, usize)>,
) -> Option<String> {
    expression_bare_runtime_calls(expression)
        .into_iter()
        .find_map(|(name, arity)| {
            explore_replay_callable_identity_issue(&name, arity, definitions, visiting, validated)
        })
}

fn validate_query_replay_callable_identities(
    query: &TypedExploreQuery,
    definitions: &GroundDefinitions,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut validated = BTreeSet::new();
    if let Some(message) = explore_replay_callable_identity_issue(
        &query.rule_name,
        query.rule_arity,
        definitions,
        &mut BTreeSet::new(),
        &mut validated,
    ) {
        diagnostics.push(Diagnostic::error_at(query.span, message));
    }
    let mut check_expression = |expression: &Expr| {
        if let Some(message) = expression_replay_callable_identity_issue(
            expression,
            definitions,
            &mut BTreeSet::new(),
            &mut validated,
        ) {
            diagnostics.push(Diagnostic::error_at(expression.span, message));
        }
    };
    for bound in &query.bounds {
        match bound {
            TypedExploreBound::Domain { domain, .. } => match domain {
                TypedExploreDomain::FiniteExpr { expression, .. } => check_expression(expression),
                TypedExploreDomain::Range {
                    start,
                    end_exclusive,
                } => {
                    check_expression(start);
                    check_expression(end_exclusive);
                }
                TypedExploreDomain::Values { .. } => {}
            },
            TypedExploreBound::Value { value, .. } => check_expression(value),
            TypedExploreBound::Where { predicate, .. } => check_expression(predicate),
        }
    }
    if let Some(boundary) = &query.boundary {
        check_expression(&boundary.step);
    }
    for field in query.output.key.iter().chain(&query.output.show) {
        check_expression(&field.value);
    }
    match &query.output.representative {
        ExploreRepresentative::First { .. } => {}
        ExploreRepresentative::Maximize { objective, .. }
        | ExploreRepresentative::Minimize { objective, .. } => check_expression(objective),
    }
    diagnostics
}

fn expression_dynamic_helper_dependencies(
    expression: &Expr,
    query_local_names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    memo: &mut BTreeMap<(String, usize), BTreeSet<String>>,
    work_remaining: &mut usize,
    depth: usize,
) -> BTreeSet<String> {
    if depth >= EXPLORE_RECURSION_LIMIT || *work_remaining == 0 {
        return query_local_names.clone();
    }
    *work_remaining -= 1;
    let mut calls = Vec::new();
    walk_ast_expr(expression, &mut |child| {
        let AstChild::Expr(expression) = child else {
            return;
        };
        let ExprKind::App(function, arguments) = &expression.kind else {
            return;
        };
        let ExprKind::Var(name) = &function.kind else {
            return;
        };
        calls.push((name.clone(), arguments.len()));
    });

    let mut dependencies = BTreeSet::new();
    for (name, arity) in calls {
        if *work_remaining == 0 {
            dependencies.extend(query_local_names.iter().cloned());
            break;
        }
        *work_remaining -= 1;
        let key = (name.clone(), arity);
        if query_local_names.contains(&name) {
            dependencies.insert(name.clone());
        }
        let any_rule = definitions
            .rule_definitions
            .keys()
            .any(|(candidate, _)| candidate == &name);
        let any_function = definitions
            .functions
            .keys()
            .any(|(candidate, _)| candidate == &name);
        let any_unsupported_callable = definitions
            .unsupported_callables
            .keys()
            .any(|(candidate, _)| candidate == &name);
        if definitions.bindings.contains_key(&name) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if any_rule
            && (any_function
                || any_unsupported_callable
                || definitions.unsupported_values.contains_key(&name))
        {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if any_rule {
            if let Some(cached) = memo.get(&key) {
                dependencies.extend(cached.iter().cloned());
                continue;
            }
            let Some(rules) = definitions.rule_definitions.get(&key) else {
                // Runtime rule lookup is name based. If the exact arity cannot
                // be identified, retain every query local conservatively.
                dependencies.extend(query_local_names.iter().cloned());
                continue;
            };
            if !visiting.insert(key.clone()) {
                continue;
            }
            let mut resolved = BTreeSet::new();
            for rule in rules {
                let bound = ground_rule_bound_names(rule);
                for expression in ground_rule_expressions(rule) {
                    let mut free = BTreeSet::new();
                    collect_true_free_vars(expression, &mut free, &bound);
                    free.retain(|name| query_local_names.contains(name));
                    resolved.extend(free);
                    resolved.extend(expression_dynamic_helper_dependencies(
                        expression,
                        query_local_names,
                        definitions,
                        visiting,
                        memo,
                        work_remaining,
                        depth + 1,
                    ));
                }
            }
            visiting.remove(&key);
            memo.insert(key, resolved.clone());
            dependencies.extend(resolved);
            continue;
        }
        if any_unsupported_callable || definitions.unsupported_values.contains_key(&name) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        let all_definitions = definitions
            .functions
            .iter()
            .filter(|((candidate, _), _)| candidate == &name)
            .flat_map(|(_, definitions)| definitions.iter())
            .collect::<Vec<_>>();
        if all_definitions.is_empty() {
            continue;
        }
        if let Some(cached) = memo.get(&key) {
            dependencies.extend(cached.iter().cloned());
            continue;
        }
        let exact = definitions.functions.get(&key);
        if all_definitions.len() != 1 || exact.is_none_or(|definitions| definitions.len() != 1) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if !visiting.insert(key.clone()) {
            continue;
        }
        let definition = &exact.expect("one exact helper definition")[0];
        let bound = definition
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<BTreeSet<_>>();
        let mut free = BTreeSet::new();
        collect_true_free_vars(&definition.body, &mut free, &bound);
        free.retain(|name| query_local_names.contains(name));
        let mut resolved = free;
        resolved.extend(expression_dynamic_helper_dependencies(
            &definition.body,
            query_local_names,
            definitions,
            visiting,
            memo,
            work_remaining,
            depth + 1,
        ));
        visiting.remove(&key);
        memo.insert(key, resolved.clone());
        dependencies.extend(resolved);
    }
    dependencies
}

fn deduplicate_list(values: Vec<ExploreValue>) -> Vec<ExploreValue> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn deduplicate_runtime_list(values: Vec<ExploreValue>) -> Vec<ExploreValue> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.runtime_display_key()))
        .collect()
}

fn runtime_set_map(values: Vec<ExploreValue>) -> BTreeMap<String, ExploreValue> {
    let mut set = BTreeMap::new();
    for value in values {
        set.entry(value.runtime_display_key()).or_insert(value);
    }
    set
}

fn runtime_set_values(values: Vec<ExploreValue>) -> Vec<ExploreValue> {
    runtime_set_map(values).into_values().collect()
}

fn exact_range_cardinality(start: i64, end_exclusive: i64) -> Result<u64, String> {
    if start > end_exclusive {
        return Err(format!(
            "exploration range start {} is greater than end {}",
            start, end_exclusive
        ));
    }
    let distance = (end_exclusive as i128) - (start as i128);
    u64::try_from(distance).map_err(|_| {
        format!(
            "exploration range {}..{} has a cardinality that cannot be represented",
            start, end_exclusive
        )
    })
}

fn axis_pair_count(domain: &ExploreExactDomain, step: i64) -> Result<ExploreCardinality, String> {
    let step = u64::try_from(step)
        .map_err(|_| "exploration boundary step must be positive".to_string())?;
    match domain {
        ExploreExactDomain::IntRange { cardinality, .. } => Ok(ExploreCardinality::Exact(
            cardinality.saturating_sub(step) as u128,
        )),
        ExploreExactDomain::Enumerated { values, .. } => {
            let ints = values
                .iter()
                .map(|value| {
                    value.int().ok_or_else(|| {
                        "exploration boundary axis contains a non-Int value".to_string()
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            let count = ints
                .iter()
                .filter(|value| {
                    value
                        .checked_add(step as i64)
                        .is_some_and(|upper| ints.contains(&upper))
                })
                .count();
            Ok(ExploreCardinality::Exact(count as u128))
        }
        ExploreExactDomain::FiniteType { .. } => Err(
            "exploration boundary axis must use an explicit Int list or symbolic Int range"
                .to_string(),
        ),
    }
}

pub(crate) fn elaborate_queries(
    statements: &[Stmt],
    source_dir: Option<&str>,
    queries: &[TypedExploreQuery],
) -> Result<Vec<ExploreQueryIr>, Vec<Diagnostic>> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }
    let catalog_statements = prepend_prelude(parse_prelude(), statements);
    let catalog = calculate::TypeCatalog::collect_checked(&catalog_statements, source_dir)
        .map_err(|errors| {
            errors
                .into_iter()
                .map(Diagnostic::error)
                .collect::<Vec<_>>()
        })?;
    let definitions = collect_ground_bindings(statements, source_dir).map_err(|errors| {
        errors
            .into_iter()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;
    let mut universes = Vec::with_capacity(queries.len());
    let mut diagnostics = Vec::new();

    for query in queries {
        match elaborate_query(query, &catalog, definitions.clone()) {
            Ok(universe) => universes.push(ExploreQueryIr {
                query: query.clone(),
                universe,
            }),
            Err(mut query_diagnostics) => diagnostics.append(&mut query_diagnostics),
        }
    }
    if diagnostics.is_empty() {
        Ok(universes)
    } else {
        Err(diagnostics)
    }
}

fn elaborate_query(
    query: &TypedExploreQuery,
    catalog: &calculate::TypeCatalog,
    definitions: GroundDefinitions,
) -> Result<ExploreUniverseIr, Vec<Diagnostic>> {
    let replay_diagnostics = validate_query_replay_callable_identities(query, &definitions);
    if !replay_diagnostics.is_empty() {
        return Err(replay_diagnostics);
    }
    let mut evaluator = ExploreGroundEvaluator::new(catalog, definitions.clone());
    let mut runtime_evaluator = ExploreRuntimeGroundEvaluator::new(&definitions);
    let all_local_names = query
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .chain(query.bounds.iter().filter_map(|bound| match bound {
            TypedExploreBound::Domain { name, .. } | TypedExploreBound::Value { name, .. } => {
                Some(name.clone())
            }
            TypedExploreBound::Where { .. } => None,
        }))
        .chain(query.output.show.iter().map(|field| field.name.clone()))
        .collect::<BTreeSet<_>>();
    let mut dimensions = Vec::new();
    let mut available_names = BTreeSet::new();
    let mut dimension_names = BTreeSet::new();
    let mut derived_names = BTreeSet::new();
    let mut facts = Vec::new();
    let mut constraints = Vec::new();
    let mut diagnostics = Vec::new();

    for bound in &query.bounds {
        match bound {
            TypedExploreBound::Domain {
                name,
                value_ty,
                domain,
                span,
            } => {
                let exact = match domain {
                    TypedExploreDomain::FiniteExpr {
                        expression,
                        element_ty,
                        collection_ty,
                    } => {
                        let dependencies = expression_query_dependencies(
                            expression,
                            &all_local_names,
                            &definitions,
                        );
                        let unavailable = dependencies
                            .difference(&available_names)
                            .cloned()
                            .collect::<Vec<_>>();
                        if !unavailable.is_empty() {
                            Err(format!(
                                "exploration domain `{}` depends on input(s) that are not yet available: {}",
                                name,
                                unavailable.join(", ")
                            ))
                        } else if dependencies
                            .iter()
                            .any(|dependency| dimension_names.contains(dependency))
                        {
                            Err(format!(
                                "exploration domain `{}` depends on varying input(s): {}",
                                name,
                                dependencies.into_iter().collect::<Vec<_>>().join(", ")
                            ))
                        } else if dependencies
                            .iter()
                            .any(|dependency| derived_names.contains(dependency))
                        {
                            Err(format!(
                                "exploration domain `{}` depends on derived value(s): {}",
                                name,
                                dependencies.into_iter().collect::<Vec<_>>().join(", ")
                            ))
                        } else {
                            eval_ground_exact(
                                &mut evaluator,
                                &mut runtime_evaluator,
                                expression,
                                collection_ty,
                                catalog,
                            )
                                .map(|(value, _)| value)
                                .and_then(|value| {
                                    let kind = collection_kind(collection_ty).unwrap_or("List");
                                    let values = match (kind, value) {
                                        ("List", ExploreValue::List(values))
                                        | ("Set", ExploreValue::Set(values)) => values,
                                        ("List", _) => {
                                            return Err(format!(
                                                "exploration domain `{}` did not evaluate to a finite list",
                                                name
                                            ));
                                        }
                                        ("Set", _) => {
                                            return Err(format!(
                                                "exploration domain `{}` did not evaluate to a finite set",
                                                name
                                            ));
                                        }
                                        (_, _) => {
                                            return Err(format!(
                                                "exploration domain `{}` has unsupported collection type `{}`",
                                                name, collection_ty
                                            ));
                                        }
                                    };
                                    for (index, value) in values.iter().enumerate() {
                                        if !explore_value_matches_ty(value, element_ty, catalog)? {
                                            return Err(format!(
                                                "exploration domain `{}` member {} does not have declared type `{}`",
                                                name,
                                                index + 1,
                                                element_ty
                                            ));
                                        }
                                    }
                                    let expression_name = match &expression.kind {
                                        ExprKind::Var(name) => Some(name.clone()),
                                        _ => None,
                                    };
                                    let (values, source) = if kind == "Set" {
                                        let values = values
                                            .into_iter()
                                            .collect::<BTreeSet<_>>()
                                            .into_iter()
                                            .collect();
                                        (
                                            values,
                                            ExploreEnumeratedSource::NamedSet {
                                                name: expression_name.unwrap_or_else(|| {
                                                    "<expression>".to_string()
                                                }),
                                            },
                                        )
                                    } else {
                                        let source = expression_name
                                            .map(|name| ExploreEnumeratedSource::NamedList { name })
                                            .unwrap_or(ExploreEnumeratedSource::ExplicitList);
                                        (deduplicate_list(values), source)
                                    };
                                    Ok(ExploreExactDomain::Enumerated { values, source })
                                })
                        }
                    }
                    TypedExploreDomain::Range {
                        start,
                        end_exclusive,
                    } => {
                        let dependencies =
                            expression_query_dependencies(start, &all_local_names, &definitions)
                                .into_iter()
                                .chain(expression_query_dependencies(
                                    end_exclusive,
                                    &all_local_names,
                                    &definitions,
                                ))
                                .collect::<BTreeSet<_>>();
                        let unavailable = dependencies
                            .difference(&available_names)
                            .cloned()
                            .collect::<Vec<_>>();
                        if !unavailable.is_empty() {
                            Err(format!(
                                "exploration range `{}` depends on input(s) that are not yet available: {}",
                                name,
                                unavailable.join(", ")
                            ))
                        } else if dependencies.iter().any(|dependency| {
                            dimension_names.contains(dependency)
                                || derived_names.contains(dependency)
                        }) {
                            Err(format!(
                                "exploration range `{}` depends on varying or derived input(s): {}",
                                name,
                                dependencies.into_iter().collect::<Vec<_>>().join(", ")
                            ))
                        } else {
                            let int_ty = Ty::Name("Int".to_string());
                            eval_ground_exact(
                                &mut evaluator,
                                &mut runtime_evaluator,
                                start,
                                &int_ty,
                                catalog,
                            )
                            .map(|(value, _)| value)
                            .and_then(|start| {
                                eval_ground_exact(
                                    &mut evaluator,
                                    &mut runtime_evaluator,
                                    end_exclusive,
                                    &int_ty,
                                    catalog,
                                )
                                .map(|(end, _)| (start, end))
                            })
                            .and_then(|(start, end)| {
                                let start = start.int().ok_or_else(|| {
                                    "exploration range start is not an Int".to_string()
                                })?;
                                let end_exclusive = end.int().ok_or_else(|| {
                                    "exploration range end is not an Int".to_string()
                                })?;
                                let cardinality = exact_range_cardinality(start, end_exclusive)?;
                                Ok(ExploreExactDomain::IntRange {
                                    start,
                                    end_exclusive,
                                    cardinality,
                                })
                            })
                        }
                    }
                    TypedExploreDomain::Values { ty } => {
                        finite_type_plan(ty, catalog, &ty.to_string(), &mut BTreeSet::new())
                            .and_then(|plan| {
                                if matches!(plan.cardinality(), ExploreCardinality::ExceedsU128) {
                                    return Err(format!(
                                        "`values({})` has more than u128::MAX inhabitants",
                                        ty
                                    ));
                                }
                                Ok(ExploreExactDomain::FiniteType {
                                    ty: ty.clone(),
                                    plan,
                                })
                            })
                    }
                };
                match exact {
                    Ok(domain) => {
                        dimension_names.insert(name.clone());
                        dimensions.push(ExploreDimensionIr {
                            name: name.clone(),
                            value_ty: value_ty.clone(),
                            domain,
                            span: *span,
                        });
                    }
                    Err(message) => diagnostics.push(Diagnostic::error_at(*span, message)),
                }
                available_names.insert(name.clone());
            }
            TypedExploreBound::Value {
                name,
                value_ty,
                value,
                span,
            } => {
                let dependencies =
                    expression_query_dependencies(value, &all_local_names, &definitions);
                let unavailable = dependencies
                    .difference(&available_names)
                    .cloned()
                    .collect::<Vec<_>>();
                if !unavailable.is_empty() {
                    diagnostics.push(Diagnostic::error_at(
                        *span,
                        format!(
                            "exploration value `{}` depends on input(s) that are not yet available: {}",
                            name,
                            unavailable.join(", ")
                        ),
                    ));
                    available_names.insert(name.clone());
                    continue;
                }
                available_names.insert(name.clone());
                let varies = dependencies.iter().any(|dependency| {
                    dimension_names.contains(dependency) || derived_names.contains(dependency)
                });
                let fact = if varies {
                    derived_names.insert(name.clone());
                    ExploreFactValue::Derived {
                        expression: value.clone(),
                        dependencies,
                    }
                } else {
                    match eval_ground_exact(
                        &mut evaluator,
                        &mut runtime_evaluator,
                        value,
                        value_ty,
                        catalog,
                    ) {
                        Ok((value, runtime_value)) => {
                            match explore_value_matches_ty(&value, value_ty, catalog) {
                                Ok(true) => {}
                                Ok(false) => {
                                    diagnostics.push(Diagnostic::error_at(
                                        *span,
                                        format!(
                                            "fixed exploration value `{}` does not have declared type `{}`",
                                            name, value_ty
                                        ),
                                    ));
                                    continue;
                                }
                                Err(message) => {
                                    diagnostics.push(Diagnostic::error_at(
                                        *span,
                                        format!(
                                            "cannot validate fixed exploration value `{}`: {}",
                                            name, message
                                        ),
                                    ));
                                    continue;
                                }
                            }
                            evaluator.set_local(name.clone(), value.clone());
                            runtime_evaluator.set_local(name.clone(), runtime_value);
                            ExploreFactValue::Fixed(value)
                        }
                        Err(message) => {
                            diagnostics.push(Diagnostic::error_at(
                                *span,
                                format!(
                                    "cannot evaluate fixed exploration value `{}`: {}",
                                    name, message
                                ),
                            ));
                            continue;
                        }
                    }
                };
                facts.push(ExploreFactIr {
                    name: name.clone(),
                    value_ty: value_ty.clone(),
                    value: fact,
                    span: *span,
                });
            }
            TypedExploreBound::Where { predicate, span } => {
                let dependencies =
                    expression_query_dependencies(predicate, &all_local_names, &definitions);
                let unavailable = dependencies
                    .difference(&available_names)
                    .cloned()
                    .collect::<Vec<_>>();
                if !unavailable.is_empty() {
                    diagnostics.push(Diagnostic::error_at(
                        *span,
                        format!(
                            "exploration `where` depends on input(s) that are not yet available: {}",
                            unavailable.join(", ")
                        ),
                    ));
                    continue;
                }
                constraints.push(ExploreConstraintIr {
                    predicate: predicate.clone(),
                    scope: if query.boundary.is_some() {
                        ExploreConstraintScope::BothBoundaryEndpoints
                    } else {
                        ExploreConstraintScope::Candidate
                    },
                    span: *span,
                });
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut cartesian_count_before_constraints = ExploreCardinality::one();
    for dimension in &dimensions {
        cartesian_count_before_constraints =
            cartesian_count_before_constraints.multiply(dimension.domain.cardinality());
    }

    let boundary = query.boundary.as_ref().and_then(|boundary| {
        let (axis_dimension_index, dimension) = dimensions
            .iter()
            .enumerate()
            .find(|(_, dimension)| dimension.name == boundary.axis)?;
        let step_dependencies =
            expression_query_dependencies(&boundary.step, &all_local_names, &definitions);
        let varying_step_dependencies = step_dependencies
            .iter()
            .filter(|dependency| {
                dimension_names.contains(*dependency) || derived_names.contains(*dependency)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !varying_step_dependencies.is_empty() {
            diagnostics.push(Diagnostic::error_at(
                boundary.span,
                format!(
                    "exploration boundary step depends on varying or derived input(s): {}",
                    varying_step_dependencies.join(", ")
                ),
            ));
            return None;
        }
        let step = match eval_ground_exact(
            &mut evaluator,
            &mut runtime_evaluator,
            &boundary.step,
            &boundary.step_ty,
            catalog,
        ) {
            Ok((ExploreValue::Int(step), _)) if step > 0 => step,
            Ok(_) => {
                diagnostics.push(Diagnostic::error_at(
                    boundary.span,
                    "exploration boundary step must be a positive fixed Int",
                ));
                return None;
            }
            Err(message) => {
                diagnostics.push(Diagnostic::error_at(
                    boundary.span,
                    format!("cannot evaluate exploration boundary step: {}", message),
                ));
                return None;
            }
        };
        let eligible_axis_pairs = match axis_pair_count(&dimension.domain, step) {
            Ok(count) => count,
            Err(message) => {
                diagnostics.push(Diagnostic::error_at(boundary.span, message));
                return None;
            }
        };
        let mut eligible_unconstrained_pairs = eligible_axis_pairs.clone();
        for other in dimensions
            .iter()
            .filter(|candidate| candidate.name != boundary.axis)
        {
            eligible_unconstrained_pairs =
                eligible_unconstrained_pairs.multiply(other.domain.cardinality());
        }
        let mut axis_sensitive = BTreeSet::from([boundary.axis.clone()]);
        let mut recomputed_fact_indices = Vec::new();
        for (index, fact) in facts.iter().enumerate() {
            let ExploreFactValue::Derived { dependencies, .. } = &fact.value else {
                continue;
            };
            if dependencies
                .iter()
                .any(|dependency| axis_sensitive.contains(dependency))
            {
                recomputed_fact_indices.push(index);
                axis_sensitive.insert(fact.name.clone());
            }
        }
        Some(ExploreBoundaryIr {
            axis: boundary.axis.clone(),
            axis_dimension_index,
            step,
            requires_both_endpoints_in_domain: true,
            recomputed_fact_indices,
            eligible_axis_pairs,
            eligible_unconstrained_pairs,
            span: boundary.span,
        })
    });

    let mut output_available_names = available_names.clone();
    for field in &query.output.key {
        let dependencies =
            expression_query_dependencies(&field.value, &all_local_names, &definitions);
        let unavailable = dependencies
            .difference(&output_available_names)
            .cloned()
            .collect::<Vec<_>>();
        if !unavailable.is_empty() {
            diagnostics.push(Diagnostic::error_at(
                field.span,
                format!(
                    "exploration output key `{}` depends on value(s) that are not yet available: {}",
                    field.name,
                    unavailable.join(", ")
                ),
            ));
        }
    }
    for field in &query.output.show {
        let dependencies =
            expression_query_dependencies(&field.value, &all_local_names, &definitions);
        let unavailable = dependencies
            .difference(&output_available_names)
            .cloned()
            .collect::<Vec<_>>();
        if !unavailable.is_empty() {
            diagnostics.push(Diagnostic::error_at(
                field.span,
                format!(
                    "exploration output field `{}` depends on value(s) that are not yet available: {}",
                    field.name,
                    unavailable.join(", ")
                ),
            ));
        }
        output_available_names.insert(field.name.clone());
    }
    if let ExploreRepresentative::Maximize { objective, span }
    | ExploreRepresentative::Minimize { objective, span } = &query.output.representative
    {
        let dependencies = expression_query_dependencies(objective, &all_local_names, &definitions);
        let unavailable = dependencies
            .difference(&output_available_names)
            .cloned()
            .collect::<Vec<_>>();
        if !unavailable.is_empty() {
            diagnostics.push(Diagnostic::error_at(
                *span,
                format!(
                    "exploration representative depends on value(s) that are not yet available: {}",
                    unavailable.join(", ")
                ),
            ));
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(ExploreUniverseIr {
        dimensions,
        facts,
        constraints,
        sliced_inputs: query.sliced_inputs.clone(),
        cartesian_count_before_constraints,
        boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifacts(source: &str) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse explore domain fixture");
        TypeChecker::check_with_artifacts(&statements, None, source)
    }

    fn artifacts_with_dir(source: &str, source_dir: &Path) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse imported explore domain fixture");
        TypeChecker::check_with_artifacts(
            &statements,
            Some(source_dir.to_string_lossy().to_string()),
            source,
        )
    }

    #[test]
    fn exact_range_cardinality_handles_full_i64_width() {
        assert_eq!(exact_range_cardinality(7, 7), Ok(0));
        assert!(exact_range_cardinality(8, 7).is_err());
        assert_eq!(exact_range_cardinality(i64::MIN, i64::MAX), Ok(u64::MAX));
    }

    #[test]
    fn finite_plan_enumerates_payloads_in_declaration_order() {
        let source = r#"
# Bit = High | Low
# Flag = On | Off
# Payload = Empty | Full(bit: Bit, flag: Flag)
| condition(value: Payload) -> True

? explore payloads {
    over condition(value)
    find matches
    bounds { value in values(Payload) }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let ExploreExactDomain::FiniteType { plan, .. } =
            &artifacts.exploration_universes[0].universe.dimensions[0].domain
        else {
            panic!("expected finite type plan")
        };
        assert_eq!(plan.cardinality(), ExploreCardinality::Exact(5));
        let values = plan.enumerate(10).expect("materialize Payload");
        assert_eq!(values.len(), 5);
        assert!(matches!(
            &values[0],
            ExploreValue::Constructor { variant, fields, .. }
                if variant == "Empty" && fields.is_empty()
        ));
        assert!(matches!(
            &values[1],
            ExploreValue::Constructor { variant, fields, .. }
                if variant == "Full"
                    && matches!(fields[0].1, ExploreValue::Constructor { ref variant, .. } if variant == "High")
                    && matches!(fields[1].1, ExploreValue::Constructor { ref variant, .. } if variant == "On")
        ));
    }

    #[test]
    fn finite_plan_has_a_total_node_budget() {
        let source = r#"
# Leaf = A | B
# P0 = Node(left: Leaf, right: Leaf)
# P1 = Node(left: P0, right: P0)
# P2 = Node(left: P1, right: P1)
# P3 = Node(left: P2, right: P2)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse repeated-product type fixture");
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect repeated-product types");
        let mut budget = 10;
        let error = finite_type_plan_with_budget(
            &Ty::Name("P3".to_string()),
            &catalog,
            "P3",
            &mut BTreeSet::new(),
            &mut budget,
            0,
        )
        .expect_err("repeated products must exhaust the test plan budget");
        assert!(error.contains("finite-type plan work limit"), "{error}");

        let variant_source = "# Many = A | B | C\n";
        let mut lexer = Lexer::new(variant_source);
        let tokens = lexer.tokenize();
        let variants = Parser::new(tokens, variant_source)
            .parse_program()
            .expect("parse many-variant type");
        let catalog = calculate::TypeCatalog::collect_checked(&variants, None)
            .expect("collect many-variant type");
        let mut budget = 3;
        let error = finite_type_plan_with_budget(
            &Ty::Name("Many".to_string()),
            &catalog,
            "Many",
            &mut BTreeSet::new(),
            &mut budget,
            0,
        )
        .expect_err("variant plan nodes must consume the total plan budget");
        assert!(error.contains("finite-type plan work limit"), "{error}");
    }

    #[test]
    fn domain_lists_deduplicate_and_ranges_stay_symbolic() {
        let source = r#"
| condition(choice: Int, income: Int, step: Int) -> income >= choice

? explore exact_domains {
    over condition(choice, income, step)
    find matches
    bounds {
        choice in [2, 1, 2]
        income in range(-2, 3)
        step = 1
        doubled = income * 2
        quadrupled = doubled * 2
        where quadrupled >= -8
    }
    boundaries on income by step
    output { key [choice, income] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let universe = &artifacts.exploration_universes[0].universe;
        assert!(matches!(
            &universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![ExploreValue::Int(2), ExploreValue::Int(1)]
        ));
        assert!(matches!(
            &universe.dimensions[1].domain,
            ExploreExactDomain::IntRange {
                start: -2,
                end_exclusive: 3,
                cardinality: 5
            }
        ));
        assert_eq!(
            universe.cartesian_count_before_constraints,
            ExploreCardinality::Exact(10)
        );
        let boundary = universe.boundary.as_ref().expect("boundary");
        assert_eq!(boundary.eligible_axis_pairs, ExploreCardinality::Exact(4));
        assert_eq!(
            boundary.eligible_unconstrained_pairs,
            ExploreCardinality::Exact(8)
        );
        assert_eq!(boundary.axis_dimension_index, 1);
        assert!(boundary.requires_both_endpoints_in_domain);
        assert_eq!(boundary.recomputed_fact_indices, vec![1, 2]);
        assert!(universe.constraints.iter().all(|constraint| {
            constraint.scope == ExploreConstraintScope::BothBoundaryEndpoints
        }));
    }

    #[test]
    fn empty_list_and_range_domains_form_a_complete_empty_universe() {
        let source = r#"
| condition(left: Int, right: Int) -> True
? explore empty {
    over condition(left, right)
    find matches
    bounds { left in []; right in range(7, 7) }
    output { key [left, right] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let universe = &artifacts.exploration_universes[0].universe;
        assert_eq!(
            universe.cartesian_count_before_constraints,
            ExploreCardinality::Exact(0)
        );
        assert!(universe
            .dimensions
            .iter()
            .all(|dimension| dimension.domain.cardinality() == ExploreCardinality::Exact(0)));
    }

    #[test]
    fn values_rejects_first_unbounded_payload_path() {
        let source = r#"
# FilingStatus = Online | Paper(copies: Int)
| condition(status: FilingStatus) -> True
? explore invalid {
    over condition(status)
    find matches
    bounds { status in values(FilingStatus) }
    output { key [status] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("FilingStatus.Paper.copies")
                    && diagnostic.message.contains("unbounded")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn explicit_domains_reject_rule_scope_instances() {
        let source = r#"
# Profile(x: Int) {
    | amount() -> x
}
= profiles: List(Profile) = [Profile(1)]
| condition(profile: Profile) -> True
? explore invalid_scope {
    over condition(profile)
    find matches
    bounds { profile in profiles }
    output { key [group = 1] show [profile] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("rule scope `Profile`")
                    && diagnostic.message.contains("cannot be used")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn values_rejects_ambiguous_duplicate_type_declarations() {
        let source = r#"
# Status = Alpha
# Status = Beta
| condition(status: Status) -> True
? explore invalid {
    over condition(status)
    find matches
    bounds { status in values(Status) }
    output { key [status] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("multiple declarations")
                    && diagnostic.message.contains("Status")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn values_supports_generic_finite_type_applications() {
        let source = r#"
# Bit = High | Low
# Flag = On | Off
| condition(option: Option(Bit), result: Result(Bit, Flag), pair: Pair(Bit, Flag), boolean: Bool) -> True
? explore generic_values {
    over condition(option, result, pair, boolean)
    find matches
    bounds {
        option in values(Option(Bit))
        result in values(Result(Bit, Flag))
        pair in values(Pair(Bit, Flag))
        boolean in values(Bool)
    }
    output { key [option, result, pair, boolean] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let cardinalities = artifacts.exploration_universes[0]
            .universe
            .dimensions
            .iter()
            .map(|dimension| dimension.domain.cardinality())
            .collect::<Vec<_>>();
        assert_eq!(
            cardinalities,
            vec![
                ExploreCardinality::Exact(3),
                ExploreCardinality::Exact(4),
                ExploreCardinality::Exact(4),
                ExploreCardinality::Exact(2),
            ]
        );
        assert_eq!(
            artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(96)
        );
    }

    #[test]
    fn optional_sugar_and_option_domains_share_one_semantic_type() {
        let source = r#"
# Status = Active | Inactive
| condition(explicit: Option(Status), optional: Status?) -> True
? explore optional_values {
    over condition(explicit, optional)
    find matches
    bounds {
        explicit in values(Option(Status))
        optional in values(Option(Status))
    }
    output { key [explicit, optional] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        for dimension in &artifacts.exploration_universes[0].universe.dimensions {
            let ExploreExactDomain::FiniteType { plan, .. } = &dimension.domain else {
                panic!("expected canonical Option finite-type plan")
            };
            assert_eq!(plan.cardinality(), ExploreCardinality::Exact(3));
            let values = plan.enumerate(3).expect("enumerate canonical Option");
            assert!(
                matches!(&values[0], ExploreValue::Constructor { variant, fields, .. }
                    if variant == "None" && fields.is_empty())
            );
            assert!(
                matches!(&values[1], ExploreValue::Constructor { variant, fields, .. }
                    if variant == "Some"
                        && matches!(&fields[0].1, ExploreValue::Constructor { variant, .. }
                            if variant == "Active"))
            );
            assert!(
                matches!(&values[2], ExploreValue::Constructor { variant, fields, .. }
                    if variant == "Some"
                        && matches!(&fields[0].1, ExploreValue::Constructor { variant, .. }
                            if variant == "Inactive"))
            );
        }
    }

    #[test]
    fn values_rejects_a_user_option_that_disagrees_with_runtime_semantics() {
        let explicit = r#"
# Option(a) = Absent | Present(a)
# Status = Active | Inactive
| condition(value: Option(Status)) -> True
? explore shadowed_option {
    over condition(value)
    find matches
    bounds { value in values(Option(Status)) }
    output { key [group = 1] show [value] representative first }
}
"#;
        let optional_sugar = r#"
# Option(a) = Absent | Present(a)
# Status = Active | Inactive
| condition(value: Status?) -> True
? explore shadowed_option_sugar {
    over condition(value)
    find matches
    bounds { value in values(Option(Status)) }
    output { key [group = 1] show [value] representative first }
}
"#;

        for source in [explicit, optional_sugar] {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("declared type `Option` shadows")
                        && diagnostic
                            .message
                            .contains("cannot define an exact exploration universe")
                }),
                "{:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn unit_has_one_finite_inhabitant_and_nat_remains_unbounded() {
        let unit_source = r#"
| condition(value: ()) -> True
? explore unit_value {
    over condition(value)
    find matches
    bounds { value in values(()) }
    output { key [value] representative first }
}
"#;
        let unit_artifacts = artifacts(unit_source);
        assert!(
            unit_artifacts.diagnostics.is_empty(),
            "{:?}",
            unit_artifacts.diagnostics
        );
        assert_eq!(
            unit_artifacts.exploration_universes[0].universe.dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(1)
        );

        let nat_source = r#"
# Nat = Zero | One
| condition(value: Nat) -> True
? explore invalid_nat {
    over condition(value)
    find matches
    bounds { value in values(Nat) }
    output { key [value] representative first }
}
"#;
        let nat_artifacts = artifacts(nat_source);
        assert!(nat_artifacts.exploration_universes.is_empty());
        assert!(nat_artifacts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shadows a built-in primitive or structural type")
        }));
    }

    #[test]
    fn finite_type_recursion_is_nominal_while_nested_type_arguments_remain_finite() {
        let finite = r#"
# Bit = High | Low
| condition(value: Option(Option(Bit))) -> True
? explore nested {
    over condition(value)
    find matches
    bounds { value in values(Option(Option(Bit))) }
    output { key [value] representative first }
}
"#;
        let finite_artifacts = artifacts(finite);
        assert!(
            finite_artifacts.diagnostics.is_empty(),
            "{:?}",
            finite_artifacts.diagnostics
        );
        assert_eq!(
            finite_artifacts.exploration_universes[0]
                .universe
                .dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(4)
        );

        let recursive = r#"
# Nest(a) = Done | More(next: Nest(Option(a)))
| condition(value: Nest(Bool)) -> True
? explore recursive {
    over condition(value)
    find matches
    bounds { value in values(Nest(Bool)) }
    output { key [group = 1] show [value] representative first }
}
"#;
        let recursive_artifacts = artifacts(recursive);
        assert!(recursive_artifacts.exploration_universes.is_empty());
        assert!(
            recursive_artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("recursive declared type `Nest`")
            }),
            "{:?}",
            recursive_artifacts.diagnostics
        );
    }

    #[test]
    fn named_lists_fixed_ranges_and_all_rule_inputs_are_exact() {
        let source = r#"
= choices: List(Int) = [10, 2, 10]
| condition(choice: Int, income: Int, step: Int, note: String) -> income >= choice
? explore named_domain {
    over condition(choice, income, step, note)
    find matches
    bounds {
        choice in choices
        start = 7
        income in range(start, start + 3)
        step = 1
        note = "declared"
    }
    boundaries on income by step
    output { key [choice, income] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let result = &artifacts.exploration_universes[0];
        assert!(result.query.sliced_inputs.is_empty());
        assert!(matches!(
            &result.universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, source: ExploreEnumeratedSource::NamedList { name } }
                if name == "choices"
                    && values == &vec![ExploreValue::Int(10), ExploreValue::Int(2)]
        ));
        assert!(matches!(
            &result.universe.dimensions[1].domain,
            ExploreExactDomain::IntRange {
                start: 7,
                end_exclusive: 10,
                cardinality: 3
            }
        ));
        assert!(matches!(
            &result.universe.facts[0].value,
            ExploreFactValue::Fixed(ExploreValue::Int(7))
        ));
    }

    #[test]
    fn boundary_membership_uses_declared_values_not_numeric_envelope() {
        let source = r#"
| condition(axis: Int, step: Int) -> axis >= 0
? explore gaps {
    over condition(axis, step)
    find matches
    bounds { axis in [0, 2, 3]; step = 1 }
    boundaries on axis by step
    output { key [axis] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let boundary = artifacts.exploration_universes[0]
            .universe
            .boundary
            .as_ref()
            .expect("boundary");
        assert_eq!(boundary.eligible_axis_pairs, ExploreCardinality::Exact(1));
    }

    #[test]
    fn named_set_domains_use_canonical_typed_order() {
        let source = r#"
= choices: Set(Int) = set_from_list([10, 2, 10])
| condition(choice: Int) -> choice > 0
? explore canonical_set {
    over condition(choice)
    find matches
    bounds { choice in choices }
    output { key [choice] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated {
                values,
                source: ExploreEnumeratedSource::NamedSet { name }
            } if name == "choices"
                && values == &vec![ExploreValue::Int(2), ExploreValue::Int(10)]
        ));
    }

    #[test]
    fn named_list_domains_can_use_closed_pure_helpers() {
        let source = r#"
> choices() -> List(Int) { concat([1, 2], [2, 3]) }
= declared_choices: List(Int) = choices()
| condition(choice: Int) -> choice > 0
? explore helper_domain {
    over condition(choice)
    find matches
    bounds { choice in declared_choices }
    output { key [choice] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![
                    ExploreValue::Int(1),
                    ExploreValue::Int(2),
                    ExploreValue::Int(3),
                ]
        ));
    }

    #[test]
    fn unbound_inputs_are_rejected_until_a_canonical_slice_proves_irrelevance() {
        let source = r#"
= x: Int = 0
> hidden() -> Bool { x > 0 }
| condition(x: Int, value: Int) -> hidden()
? explore hidden_relevance {
    over condition(x, value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_queries.is_empty());
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("exploration input `x` is unbound")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn call_site_helpers_make_facts_depend_on_available_dimensions() {
        let source = r#"
= axis: Int = 0
> hidden() -> Bool { axis > 0 }
| condition(axis: Int, flag: Bool) -> flag
? explore hidden_derived {
    over condition(axis, flag)
    find matches
    bounds {
        axis in [-1, 1]
        flag = hidden()
    }
    output { key [axis] show [flag] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.facts[0].value,
            ExploreFactValue::Derived { dependencies, .. }
                if dependencies == &BTreeSet::from(["axis".to_string()])
        ));
    }

    #[test]
    fn dynamic_replay_requires_one_runtime_identity_per_reachable_helper() {
        let ambiguous = r#"
> helper(axis: Int) -> Int { axis + 1 }
> helper() -> Int { 99 }
| condition(axis: Int, derived: Int) -> derived > axis
? explore overloaded_derived {
    over condition(axis, derived)
    find matches
    bounds {
        axis in [1, 2]
        derived = helper(axis)
    }
    output { key [axis] show [derived] representative first }
}
"#;
        let ambiguous_artifacts = artifacts(ambiguous);
        assert!(ambiguous_artifacts.exploration_queries.is_empty());
        assert!(ambiguous_artifacts.exploration_universes.is_empty());
        assert!(
            ambiguous_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("`helper` has declarations across arities (0, 1), but ordinary runtime functions resolve by bare name")),
            "{:?}",
            ambiguous_artifacts.diagnostics
        );

        let unique = r#"
> helper(axis: Int) -> Int { axis + 1 }
| condition(axis: Int, derived: Int) -> derived > axis
? explore unique_derived {
    over condition(axis, derived)
    find matches
    bounds {
        axis in [1, 2]
        derived = helper(axis)
    }
    output { key [axis] show [derived] representative first }
}
"#;
        let unique_artifacts = artifacts(unique);
        assert!(
            unique_artifacts.diagnostics.is_empty(),
            "{:?}",
            unique_artifacts.diagnostics
        );
        assert!(matches!(
            &unique_artifacts.exploration_universes[0].universe.facts[0].value,
            ExploreFactValue::Derived { dependencies, .. }
                if dependencies == &BTreeSet::from(["axis".to_string()])
        ));
    }

    #[test]
    fn replay_identity_gate_covers_where_without_a_derived_helper_call() {
        let source = r#"
> eligible(axis: Int) -> Bool { axis > 0 }
> eligible() -> Bool { False }
| condition(axis: Int) -> True
? explore overloaded_where {
    over condition(axis)
    find matches
    bounds {
        axis in [1, 2]
        where eligible(axis)
    }
    output { key [axis] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_queries.is_empty());
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("`eligible` has declarations across arities (0, 1), but ordinary runtime functions resolve by bare name")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn helper_captures_cannot_bypass_bound_source_order() {
        let fixtures = [
            r#"
= later: Int = 99
> hidden() -> Bool { later > 0 }
| condition(flag: Bool, later: Int) -> flag
? explore future_fact {
    over condition(flag, later)
    find matches
    bounds { flag = hidden(); later in [-1, 1] }
    output { key [later] show [flag] representative first }
}
"#,
            r#"
= later: Int = 99
> hidden() -> Bool { later > 0 }
| condition(later: Int) -> True
? explore future_where {
    over condition(later)
    find matches
    bounds { where hidden(); later in [-1, 1] }
    output { key [later] representative first }
}
"#,
            r#"
= later: Int = 3
> choices() -> List(Int) { range(0, later) }
| condition(choice: Int, later: Int) -> True
? explore future_domain {
    over condition(choice, later)
    find matches
    bounds { choice in choices(); later in [-1, 1] }
    output { key [choice, later] representative first }
}
"#,
            r#"
= later: Int = 99
| hidden() -> later > 0
| condition(flag: Bool, later: Int) -> flag
? explore future_rule_fact {
    over condition(flag, later)
    find matches
    bounds { flag = hidden(); later in [-1, 1] }
    output { key [later] show [flag] representative first }
}
"#,
            r#"
= later: Int = 99
| hidden() -> later > 0
| condition(later: Int) -> True
? explore future_rule_where {
    over condition(later)
    find matches
    bounds { where hidden(); later in [-1, 1] }
    output { key [later] representative first }
}
"#,
            r#"
= later: Int = 99
> hidden(value: Int) -> Bool { value > 0 }
> hidden() -> Bool { later > 0 }
| condition(later: Int) -> True
? explore future_overloaded_helper_where {
    over condition(later)
    find matches
    bounds { where hidden(); later in [-1, 1] }
    output { key [later] representative first }
}
"#,
        ];
        for source in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            let expected = if source.contains("future_overloaded_helper_where") {
                "ordinary runtime functions resolve by bare name"
            } else {
                "not yet available: later"
            };
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn rule_captures_cannot_bypass_output_source_order() {
        let source = r#"
= later_show: Int = 7
| hidden() -> later_show > 0
| condition(value: Int) -> True
? explore future_output {
    over condition(value)
    find matches
    bounds { value in [1] }
    output {
        key [value]
        show [early = hidden(), later_show = value]
        representative first
    }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("output field `early` depends on value(s) that are not yet available: later_show")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn evaluated_domain_members_must_match_their_declared_type() {
        let source = r#"
> choices() -> List(Int) { [True] }
= declared_choices: List(Int) = choices()
| condition(choice: Int) -> True
? explore invalid_members {
    over condition(choice)
    find matches
    bounds { choice in declared_choices }
    output { key [choice] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("member 1 does not have declared type `Int`")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn ground_helpers_use_runtime_equality_inside_finite_collection_code() {
        let source = r#"
> choices() -> List(Int) {
    if [0.0] == [0.0000000000000001] { [1] } else { [2] }
}
= declared_choices: List(Int) = choices()
| condition(choice: Int) -> True
? explore runtime_equality {
    over condition(choice)
    find matches
    bounds { choice in declared_choices }
    output { key [choice] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![ExploreValue::Int(2)]
        ));
    }

    #[test]
    fn ground_equality_rejects_deep_runtime_lists_before_stack_recursion() {
        let source = r#"
> choices() -> List(Int) {
    if range(0, 1024) == range(0, 1024) { [1] } else { [2] }
}
= declared_choices: List(Int) = choices()
| condition(choice: Int) -> True
? explore bounded_equality {
    over condition(choice)
    find matches
    bounds { choice in declared_choices }
    output { key [choice] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("ground equality exceeds the safe structural limit")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn ground_set_and_distinct_calls_use_runtime_display_identity() {
        let set_source = r#"
= pairs: Set(Tuple(String, String)) = set_from_list([
    ("a, b", "c"),
    ("a", "b, c")
])
| condition(pair: Tuple(String, String)) -> True
? explore display_collision {
    over condition(pair)
    find matches
    bounds { pair in pairs }
    output { key [pair] representative first }
}
"#;
        let set_artifacts = artifacts(set_source);
        assert!(
            set_artifacts.diagnostics.is_empty(),
            "{:?}",
            set_artifacts.diagnostics
        );
        assert_eq!(
            set_artifacts.exploration_universes[0].universe.dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(1)
        );

        let distinct_source = r#"
= pairs: List(Tuple(String, String)) = distinct([
    ("a, b", "c"),
    ("a", "b, c")
])
| condition(pair: Tuple(String, String)) -> True
? explore display_collision {
    over condition(pair)
    find matches
    bounds { pair in pairs }
    output { key [pair] representative first }
}
"#;
        let distinct_artifacts = artifacts(distinct_source);
        assert!(
            distinct_artifacts.diagnostics.is_empty(),
            "{:?}",
            distinct_artifacts.diagnostics
        );
        assert_eq!(
            distinct_artifacts.exploration_universes[0]
                .universe
                .dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(1)
        );
    }

    #[test]
    fn positional_cons_variants_use_the_runtime_set_identity() {
        let source = r#"
# Weird = Cons(Bool, Bool)
= weirds: Set(Weird) = set_from_list([
    Cons(false, false),
    Cons(false, true)
])
| condition(value: Weird) -> True
? explore weird_cons {
    over condition(value)
    find matches
    bounds { value in weirds }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert_eq!(
            artifacts.exploration_universes[0].universe.dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(1)
        );
    }

    #[test]
    fn nullary_constructor_spellings_share_one_domain_identity() {
        let source = r#"
# Status = Alpha | Beta
| condition(value: Status) -> True
? explore explicit_nullary {
    over condition(value)
    find matches
    bounds { value in [Alpha, Alpha()] }
    output { key [value] representative first }
}
? explore all_nullary {
    over condition(value)
    find matches
    bounds { value in values(Status) }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let ExploreExactDomain::Enumerated { values, .. } =
            &artifacts.exploration_universes[0].universe.dimensions[0].domain
        else {
            panic!("expected explicit finite domain")
        };
        assert_eq!(values.len(), 1);
        let ExploreExactDomain::FiniteType { plan, .. } =
            &artifacts.exploration_universes[1].universe.dimensions[0].domain
        else {
            panic!("expected declared finite type")
        };
        let inhabitants = plan.enumerate(2).expect("enumerate Status");
        assert_eq!(inhabitants.len(), 2);
        assert_eq!(values[0], inhabitants[0]);

        let runtime_source = r#"
# Status = Alpha | Beta
= same_nullary_value = Alpha == Alpha()
"#;
        let mut lexer = Lexer::new(runtime_source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, runtime_source)
            .parse_program()
            .expect("parse nullary runtime fixture");
        let mut interpreter = Interpreter::new();
        let mut environment = interpreter.default_env();
        interpreter.run_program(&statements, &mut environment);
        assert!(matches!(
            environment.get("same_nullary_value"),
            Some(Value::Bool(true))
        ));
    }

    #[test]
    fn ground_collection_intrinsics_preserve_list_and_set_kinds() {
        let fixtures = [
            (
                r#"
= base: Set(Int) = set_from_list([1, 2])
= choices: Set(Int) = concat(base, base)
| condition(value: Int) -> True
? explore invalid_concat {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "`concat` left argument is not a finite list",
            ),
            (
                r#"
= choices: List(Int) = set_from_list([1, 2])
| condition(value: Int) -> True
? explore invalid_set {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "`set_from_list` ground result must have type `Set(T)`",
            ),
        ];
        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn named_ground_ranges_materialize_with_a_checked_limit() {
        let source = r#"
= choices: List(Int) = range(0, 3)
| condition(choice: Int) -> True
? explore named_range {
    over condition(choice)
    find matches
    bounds { choice in choices }
    output { key [choice] representative first }
}
"#;
        let range_artifacts = artifacts(source);
        assert!(
            range_artifacts.diagnostics.is_empty(),
            "{:?}",
            range_artifacts.diagnostics
        );
        assert!(matches!(
            &range_artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![
                    ExploreValue::Int(0),
                    ExploreValue::Int(1),
                    ExploreValue::Int(2),
                ]
        ));

        let too_large = r#"
= choices: List(Int) = range(0, 1000001)
| condition(choice: Int) -> True
? explore named_range {
    over condition(choice)
    find matches
    bounds { choice in choices }
    output { key [choice] representative first }
}
"#;
        let oversized_artifacts = artifacts(too_large);
        assert!(oversized_artifacts.exploration_universes.is_empty());
        assert!(oversized_artifacts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exceeding materialization limit 1000000")
        }));
    }

    #[test]
    fn nested_ground_helpers_inherit_the_runtime_call_site_scope() {
        let source = r#"
= x: Int = 42
> inner() -> List(Int) { [x] }
> outer(x: Int) -> List(Int) { inner() }
= choices: List(Int) = outer(7)
| condition(value: Int) -> True
? explore call_site_scope {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![ExploreValue::Int(7)]
        ));
    }

    #[test]
    fn callable_collisions_fail_closed_before_ground_elaboration() {
        let fixtures = [
            (
                r#"
| set_from_list(items: List(Int)) -> [42]
| condition(value: Int) -> True
? explore shadowed_builtin {
    over condition(value)
    find matches
    bounds { value in set_from_list([1, 2]) }
    output { key [value] representative first }
}
"#,
                "resolves to a rule",
            ),
            (
                r#"
> choose(x: Int) -> List(Int) { [1] }
> choose(x: Int, y: Int) -> List(Int) { [2] }
= choices: List(Int) = choose(0)
| condition(value: Int) -> True
? explore overloaded {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "declarations across arities",
            ),
            (
                r#"
> Make(x: Int) -> List(Int) { [1] }
# T = Make(value: Int)
= choices: List(Int) = Make(0)
| condition(value: Int) -> True
? explore constructor_collision {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "ambiguous between a function and constructor",
            ),
            (
                r#"
# Choice = Foo | Bar
> Foo() -> Int { 1 }
= choices: List(Choice) = [Foo]
| condition(value: Choice) -> True
? explore bare_constructor_collision {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "ambiguous between a bare value/constructor and a callable declaration",
            ),
            (
                r#"
# Choice = Foo | Bar
> module Foo { = value = 1 }
= choices: List(Choice) = [Foo]
| condition(value: Choice) -> True
? explore bare_module_collision {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "shadowed by a runtime value",
            ),
            (
                r#"
> range(start: Int) -> List(Int) { [42] }
= choices: List(Int) = range(0, 3)
| condition(value: Int) -> True
? explore wrong_arity_shadow {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "`range` expects 1 argument but got 2",
            ),
            (
                r#"
> make() -> List(Int) {
    = concat = 1
    concat([1], [2])
}
= choices: List(Int) = make()
| condition(value: Int) -> True
? explore local_shadow {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#,
                "shadowed by a local value",
            ),
            (
                r#"
> choices() -> List(Int) { [1] }
# Box = Box(value: Int) {
    > choices() -> List(Int) { [2] }
}
= declared_choices: List(Int) = choices()
| condition(value: Int) -> True
? explore method_shadow {
    over condition(value)
    find matches
    bounds { value in declared_choices }
    output { key [value] representative first }
}
"#,
                "resolves to an unsupported callable",
            ),
            (
                r#"
# Span = range(Int, Int)
| condition(value: Int) -> True
? explore constructor_range {
    over condition(value)
    find matches
    bounds { value in range(0, 3) }
    output { key [value] representative first }
}
"#,
                "shadowed by an available query value or program declaration",
            ),
            (
                r#"
| condition(range: Int, value: Int) -> True
? explore local_range {
    over condition(range, value)
    find matches
    bounds { range = 99; value in range(0, 3) }
    output { key [value] representative first }
}
"#,
                "shadowed by an available query value or program declaration",
            ),
            (
                r#"
> actor range(state: Int) { | Ping -> state }
| condition(value: Int) -> True
? explore actor_range {
    over condition(value)
    find matches
    bounds { value in range(0, 3) }
    output { key [value] representative first }
}
"#,
                "shadowed by an available query value or program declaration",
            ),
            (
                r#"
> module range { = value = 1 }
| condition(value: Int) -> True
? explore module_range {
    over condition(value)
    find matches
    bounds { value in range(0, 3) }
    output { key [value] representative first }
}
"#,
                "shadowed by an available query value or program declaration",
            ),
        ];
        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn explicit_negative_float_members_preserve_exact_bits() {
        let source = r#"
| condition(value: Float) -> True
? explore explicit_floats {
    over condition(value)
    find matches
    bounds { value in [-0.1, 0.1] }
    output { key [group = 1] show [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![
                    ExploreValue::FloatBits((-0.1_f64).to_bits()),
                    ExploreValue::FloatBits(0.1_f64.to_bits()),
                ]
        ));
    }

    #[test]
    fn reversed_and_overflowing_ranges_fail_closed() {
        for (range, expected) in [
            ("range(8, 7)", "greater than end"),
            (
                "range(9223372036854775807, 9223372036854775807 + 1)",
                "addition overflow",
            ),
        ] {
            let source = format!(
                r#"
| condition(value: Int) -> True
? explore invalid {{
    over condition(value)
    find matches
    bounds {{ value in {range} }}
    output {{ key [value] representative first }}
}}
"#
            );
            let artifacts = artifacts(&source);
            assert!(artifacts.exploration_queries.is_empty());
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn named_ground_boundary_steps_are_evaluated_once() {
        let source = r#"
= global_step: Int = 1
| condition(axis: Int) -> axis >= 0
? explore named_step {
    over condition(axis)
    find matches
    bounds { axis in range(0, 3) }
    boundaries on axis by global_step
    output { key [axis] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let boundary = artifacts.exploration_universes[0]
            .universe
            .boundary
            .as_ref()
            .expect("boundary");
        assert_eq!(boundary.step, 1);
        assert_eq!(boundary.eligible_axis_pairs, ExploreCardinality::Exact(2));
    }

    #[test]
    fn primitive_shadowing_fails_closed_and_explicit_float_values_are_exact() {
        let primitive_shadow = r#"
# Bool = Yes | No
| condition(value: Bool) -> True
? explore shadowed {
    over condition(value)
    find matches
    bounds { value in values(Bool) }
    output { key [value] representative first }
}
"#;
        let shadow_artifacts = artifacts(primitive_shadow);
        assert!(shadow_artifacts.exploration_universes.is_empty());
        assert!(
            shadow_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("shadows a built-in primitive") }),
            "{:?}",
            shadow_artifacts.diagnostics
        );

        let evolved_primitive = r#"
# Bool WHEN True -> Maybe
| condition(value: Bool) -> True
? explore evolved_builtin {
    over condition(value)
    find matches
    bounds { value in values(Bool) }
    output { key [value] representative first }
}
"#;
        let evolved_artifacts = artifacts(evolved_primitive);
        assert!(evolved_artifacts.exploration_universes.is_empty());
        assert!(
            evolved_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "conditional type evolution for `Bool` changes a built-in primitive"
                )),
            "{:?}",
            evolved_artifacts.diagnostics
        );

        let tuple_shadow = r#"
# Bit = High | Low
# Tuple(a, b) = Only
| condition(value: Tuple(Bit, Bool)) -> True
? explore shadowed_tuple {
    over condition(value)
    find matches
    bounds { value in values(Tuple(Bit, Bool)) }
    output { key [value] representative first }
}
"#;
        let tuple_artifacts = artifacts(tuple_shadow);
        assert!(tuple_artifacts.exploration_universes.is_empty());
        assert!(
            tuple_artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("shadows a built-in primitive or structural type")
            }),
            "{:?}",
            tuple_artifacts.diagnostics
        );

        let composite_float = r#"
# Wrapped(amount: Float)
| condition(value: Wrapped) -> True
? explore floating {
    over condition(value)
    find matches
    bounds { value in [Wrapped(amount = 0.1)] }
    output { key [group = 1] show [value] representative first }
}
"#;
        let float_artifacts = artifacts(composite_float);
        assert!(
            float_artifacts.diagnostics.is_empty(),
            "{:?}",
            float_artifacts.diagnostics
        );
        assert!(matches!(
            &float_artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if matches!(
                    values.as_slice(),
                    [ExploreValue::Constructor { fields, .. }]
                        if matches!(fields.as_slice(), [(name, ExploreValue::FloatBits(bits))]
                            if name == "amount" && *bits == 0.1_f64.to_bits())
                )
        ));
    }

    #[test]
    fn tuple_values_and_explicit_tuple_members_preserve_element_types() {
        let finite = r#"
# Status = Alpha | Beta
| condition(value: Tuple(Status, Bool)) -> True
? explore tuples {
    over condition(value)
    find matches
    bounds { value in values(Tuple(Status, Bool)) }
    output { key [value] representative first }
}
"#;
        let finite_artifacts = artifacts(finite);
        assert!(
            finite_artifacts.diagnostics.is_empty(),
            "{:?}",
            finite_artifacts.diagnostics
        );
        let domain = &finite_artifacts.exploration_universes[0]
            .universe
            .dimensions[0]
            .domain;
        assert_eq!(domain.cardinality(), ExploreCardinality::Exact(4));

        let explicit = r#"
# Status = Alpha | Beta
| condition(value: Tuple(Status, Bool)) -> True
? explore tuples {
    over condition(value)
    find matches
    bounds { value in [(Alpha, True), (Beta, False)] }
    output { key [value] representative first }
}
"#;
        let explicit_artifacts = artifacts(explicit);
        assert!(
            explicit_artifacts.diagnostics.is_empty(),
            "{:?}",
            explicit_artifacts.diagnostics
        );
        assert!(matches!(
            &explicit_artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. } if values.len() == 2
        ));

        let runtime = r#"
# Status = Alpha | Beta
| condition(value: Tuple(Status, Bool)) -> True
= tuple_matches = condition((Alpha, True))
"#;
        let mut lexer = Lexer::new(runtime);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, runtime)
            .parse_program()
            .expect("parse tuple runtime fixture");
        let mut interpreter = Interpreter::new();
        let mut environment = interpreter.default_env();
        interpreter.run_program(&statements, &mut environment);
        assert!(matches!(
            environment.get("tuple_matches"),
            Some(Value::Bool(true))
        ));
    }

    #[test]
    fn global_ground_bindings_do_not_capture_helper_parameters() {
        let source = r#"
= seed: Int = 10
= values_from_global: List(Int) = [seed]
> choose(seed: Int) -> List(Int) { values_from_global }
= choices: List(Int) = choose(1)
| condition(value: Int) -> True
? explore lexical_scope {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert!(matches!(
            &artifacts.exploration_universes[0].universe.dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![ExploreValue::Int(10)]
        ));
    }

    #[test]
    fn ground_helper_calls_fail_closed_when_a_binding_shadows_the_callable() {
        let source = r#"
> one() -> List(Int) { [1] }
> two() -> List(Int) { [2] }
> make() -> List(Int) { one() }
= make = two
= choices: List(Int) = make()
| condition(value: Int) -> True
? explore shadowed_callable {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("ground exploration call `make` is shadowed by a top-level binding")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn contextual_range_fails_closed_when_a_program_function_shadows_it() {
        let source = r#"
> range(start: Int, end: Int) -> List(Int) { [42] }
| condition(value: Int) -> True
? explore shadowed_range {
    over condition(value)
    find matches
    bounds { value in range(0, 3) }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("exploration `range(start, end)` is shadowed")
            }),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn overflowing_integer_literal_is_never_coerced_to_zero() {
        let source = "= impossible = 9223372036854775808\n";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let error = Parser::new(tokens, source)
            .parse_program()
            .expect_err("overflowing Int must fail parsing");
        assert!(error.contains("outside Futuruna Int range"), "{error}");

        let minimum = "= minimum = -9223372036854775808\n";
        let mut lexer = Lexer::new(minimum);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, minimum)
            .parse_program()
            .expect("i64::MIN is a valid Futuruna Int");
        assert!(matches!(
            &statements[0],
            Stmt::Bind(_, _, Expr { kind: ExprKind::Lit(Literal::Int(value)), .. })
                if *value == i64::MIN
        ));
    }

    #[test]
    fn varying_ranges_and_boundary_steps_cannot_capture_same_named_globals() {
        let fixtures = [
            (
                r#"
= start: Int = 1
| condition(start: Int, value: Int) -> True
? explore dependent_range {
    over condition(start, value)
    find matches
    bounds { start in [0, 1]; value in range(start, 3) }
    output { key [start, value] representative first }
}
"#,
                "exploration range `value` depends on varying or derived input(s): start",
            ),
            (
                r#"
= axis: Int = 1
| condition(axis: Int) -> True
? explore varying_step {
    over condition(axis)
    find matches
    bounds { axis in range(0, 3) }
    boundaries on axis by axis
    output { key [axis] representative first }
}
"#,
                "exploration boundary step depends on varying or derived input(s): axis",
            ),
        ];
        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn flat_imported_finite_type_and_named_list_elaborate_exactly() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_exact_domain_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create exact-domain import directory");
        std::fs::write(
            directory.join("domain.runa"),
            r#"
# Municipality = Beta | Alpha
= municipalities: List(Municipality) = [Beta, Alpha, Beta]
= unrelated_values: List(Int) = [99]
"#,
        )
        .expect("write exact-domain import");
        let source = r#"
@ import ./domain
| condition(municipality: Municipality, declared: Municipality) -> True
? explore imported {
    over condition(municipality, declared)
    find matches
    bounds {
        municipality in municipalities
        declared in values(Municipality)
    }
    output { key [municipality, declared] representative first }
}
"#;
        let artifacts = artifacts_with_dir(source, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let dimensions = &artifacts.exploration_universes[0].universe.dimensions;
        assert!(matches!(
            &dimensions[0].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values.len() == 2
                    && matches!(&values[0], ExploreValue::Constructor { variant, .. } if variant == "Beta")
                    && matches!(&values[1], ExploreValue::Constructor { variant, .. } if variant == "Alpha")
        ));
        let ExploreExactDomain::FiniteType { plan, .. } = &dimensions[1].domain else {
            panic!("expected imported finite type")
        };
        assert_eq!(plan.cardinality(), ExploreCardinality::Exact(2));
        let values = plan.enumerate(2).expect("enumerate imported type");
        assert!(
            matches!(&values[0], ExploreValue::Constructor { variant, .. } if variant == "Beta")
        );
        assert!(
            matches!(&values[1], ExploreValue::Constructor { variant, .. } if variant == "Alpha")
        );
    }

    #[test]
    fn injected_prelude_keeps_a_user_prefix_import_explorable() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_prelude_import_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create prelude-import directory");
        std::fs::write(
            directory.join("domain.runa"),
            "# Flag = On | Off\n= flags: List(Flag) = [On, Off]\n= optional_values: List(Option(Int)) = [Some(1), None]\n= scores: List(Int) = [max_int(1, 2)]\n",
        )
        .expect("write prelude-import fixture");
        let source = r#"
@ import ./domain
| condition(flag: Flag, optional: Option(Int), score: Int) -> True
? explore imported_with_prelude {
    over condition(flag, optional, score)
    find matches
    bounds { flag in flags; optional in optional_values; score in scores }
    output { key [flag, score] show [optional] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let user_statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse prelude-import fixture");
        let statements = prepend_prelude(parse_prelude(), &user_statements);
        let ground_definitions =
            collect_ground_bindings(&statements, Some(directory.to_string_lossy().as_ref()))
                .expect("collect prelude/import declaration order");
        let runtime_declarations =
            prepend_prelude(parse_prelude(), &ground_definitions.runtime_declarations);
        let declaration_names = runtime_declarations
            .iter()
            .filter_map(|statement| match statement {
                Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let option_index = declaration_names
            .iter()
            .position(|name| *name == "Option")
            .expect("runtime declarations include the prepended Option type");
        let flag_index = declaration_names
            .iter()
            .position(|name| *name == "Flag")
            .expect("runtime declarations include the imported Flag type");
        assert!(option_index < flag_index, "{declaration_names:?}");
        let artifacts = TypeChecker::check_with_artifacts(
            &statements,
            Some(directory.to_string_lossy().to_string()),
            source,
        );
        std::fs::remove_dir_all(&directory).ok();
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert_eq!(
            artifacts.exploration_universes[0].universe.dimensions[0]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(2)
        );
        assert_eq!(
            artifacts.exploration_universes[0].universe.dimensions[1]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(2)
        );
        assert_eq!(
            artifacts.exploration_universes[0].universe.dimensions[2]
                .domain
                .cardinality(),
            ExploreCardinality::Exact(1)
        );
    }

    #[test]
    fn imported_ground_bindings_reject_later_intrinsic_shadowing() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_import_shadow_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create import-shadow directory");
        std::fs::write(
            directory.join("domain.runa"),
            "= choices: List(Int) = range(0, 3)\n",
        )
        .expect("write import-shadow fixture");
        let source = r#"
@ import ./domain
> range(start: Int, end: Int) -> List(Int) { [99] }
| condition(value: Int) -> True
? explore import_shadow {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts_with_dir(source, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("intrinsic `range` is shadowed by a program function")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn imported_ground_bindings_cannot_capture_later_root_values() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_import_capture_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create import-capture directory");
        std::fs::write(
            directory.join("domain.runa"),
            "= choices: List(Int) = root_values\n",
        )
        .expect("write import-capture fixture");
        let source = r#"
@ import ./domain
= root_values: List(Int) = [1, 2]
| condition(value: Int) -> True
? explore import_capture {
    over condition(value)
    find matches
    bounds { value in choices }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts_with_dir(source, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("depends on later declaration `root_values`")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn hash_imported_finite_types_and_helpers_elaborate_exactly() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_hash_domain_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create hash-domain import directory");
        let imported = r#"
# Flag = On | Off
> choices() -> List(Int) { [1, 2, 2] }
"#;
        std::fs::write(directory.join("domain.runa"), imported).expect("write hash-domain import");
        let mut lexer = Lexer::new(imported);
        let tokens = lexer.tokenize();
        let imported_statements = Parser::new(tokens, imported)
            .parse_program()
            .expect("parse hash-domain definitions");
        let type_hash = imported_statements
            .iter()
            .find_map(|statement| match statement {
                Stmt::TypeDecl(declaration) => Some(content_hash_type(declaration)),
                _ => None,
            })
            .expect("type hash");
        let function_hash = imported_statements
            .iter()
            .find_map(|statement| match statement {
                Stmt::Defn(definition) => Some(content_hash_defn(definition)),
                _ => None,
            })
            .expect("function hash");
        let source = format!(
            r#"
@ import #{type_hash} from ./domain
@ import #{function_hash} from ./domain
| condition(flag: Flag, choice: Int) -> True
? explore hash_domain {{
    over condition(flag, choice)
    find matches
    bounds {{ flag in values(Flag); choice in choices() }}
    output {{ key [flag, choice] representative first }}
}}
"#
        );
        let artifacts = artifacts_with_dir(&source, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let universe = &artifacts.exploration_universes[0].universe;
        assert_eq!(
            universe.dimensions[0].domain.cardinality(),
            ExploreCardinality::Exact(2)
        );
        assert!(matches!(
            &universe.dimensions[1].domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values == &vec![ExploreValue::Int(1), ExploreValue::Int(2)]
        ));
    }

    #[test]
    fn ambiguous_content_hash_imports_fail_closed() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_hash_ambiguity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create hash-ambiguity directory");
        let imported = "> first() -> List(Int) { [1] }\n> second() -> List(Int) { [1] }\n";
        std::fs::write(directory.join("domain.runa"), imported)
            .expect("write hash-ambiguity fixture");
        let statements = {
            let mut lexer = Lexer::new(imported);
            let tokens = lexer.tokenize();
            Parser::new(tokens, imported)
                .parse_program()
                .expect("parse hash-ambiguity fixture")
        };
        let hashes = statements
            .iter()
            .filter_map(|statement| match statement {
                Stmt::Defn(definition) => Some(content_hash_defn(definition)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hashes.len(), 2);
        assert_eq!(hashes[0], hashes[1]);
        let source = format!(
            r#"
@ import #{} from ./domain
| condition(value: Int) -> True
? explore ambiguous_hash {{
    over condition(value)
    find matches
    bounds {{ value in [1] }}
    output {{ key [value] representative first }}
}}
"#,
            hashes[0]
        );
        let artifacts = artifacts_with_dir(&source, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("expected exactly one matching definition, found 2")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn manifest_hash_import_uses_the_same_runtime_path_resolver() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_manifest_hash_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let dependency = directory.join("vendor").join("rules");
        std::fs::create_dir_all(&dependency).expect("create manifest dependency");
        std::fs::write(
            directory.join("runa.toml"),
            "[package]\nname = \"root\"\n\n[dependencies]\ntaxlib = { path = \"./vendor/rules\" }\n",
        )
        .expect("write manifest");
        let imported = "> imported_value() -> Int { 7 }\n";
        std::fs::write(dependency.join("domain.runa"), imported)
            .expect("write manifest dependency module");
        let imported_statements = {
            let mut lexer = Lexer::new(imported);
            let tokens = lexer.tokenize();
            Parser::new(tokens, imported)
                .parse_program()
                .expect("parse manifest dependency")
        };
        let hash = imported_statements
            .iter()
            .find_map(|statement| match statement {
                Stmt::Defn(definition) => Some(content_hash_defn(definition)),
                _ => None,
            })
            .expect("dependency function hash");
        let source = format!(
            "@ import #{} from taxlib/domain\n= imported_result = imported_value()\n",
            hash
        );
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, &source)
            .parse_program()
            .expect("parse manifest hash-import program");
        let mut interpreter = Interpreter::new();
        interpreter.source_dir = Some(directory.to_string_lossy().to_string());
        let mut environment = interpreter.default_env();
        interpreter.run_program(&statements, &mut environment);
        std::fs::remove_dir_all(&directory).ok();
        assert!(matches!(
            environment.get("imported_result"),
            Some(Value::Int(7))
        ));
    }

    #[test]
    fn preflight_has_a_total_work_budget() {
        let source = r#"
> f0() -> Int { 1 }
> f1() -> Int { f0() + f0() }
> f2() -> Int { f1() + f1() }
> f3() -> Int { f2() + f2() }
> f4() -> Int { f3() + f3() }
= choice: Int = f4()
"#;
        let statements = {
            let mut lexer = Lexer::new(source);
            let tokens = lexer.tokenize();
            Parser::new(tokens, source)
                .parse_program()
                .expect("parse work-budget fixture")
        };
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect work-budget types");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect work-budget declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        evaluator.work_remaining = 20;
        let error = evaluator
            .eval_binding("choice", Some(&Ty::Name("Int".to_string())))
            .expect_err("fan-out must exhaust the preflight budget");
        assert!(error.contains("checked work limit"), "{error}");
    }

    #[test]
    fn preflight_collection_transforms_consume_the_total_work_budget() {
        let source = "= choices: List(Int) = distinct(distinct([1, 2, 3]))\n";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse collection-work fixture");
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect collection-work types");
        let definitions = collect_ground_bindings(&statements, None)
            .expect("collect collection-work declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        evaluator.work_remaining = 10;
        let error = evaluator
            .eval_binding(
                "choices",
                Some(&Ty::App(
                    Box::new(Ty::Name("List".to_string())),
                    vec![Ty::Name("Int".to_string())],
                )),
            )
            .expect_err("nested linear transforms must exhaust the preflight budget");
        assert!(error.contains("checked work limit"), "{error}");
    }

    #[test]
    fn preflight_rejects_deep_acyclic_helper_chains() {
        let mut source = "> f0() -> Int { 1 }\n".to_string();
        for index in 1..=260 {
            source.push_str(&format!("> f{}() -> Int {{ f{}() }}\n", index, index - 1));
        }
        source.push_str("= choice: Int = f260()\n");
        let statements = {
            let mut lexer = Lexer::new(&source);
            let tokens = lexer.tokenize();
            Parser::new(tokens, &source)
                .parse_program()
                .expect("parse helper-depth fixture")
        };
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect helper-depth types");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect helper-depth declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        let error = evaluator
            .eval_binding("choice", Some(&Ty::Name("Int".to_string())))
            .expect_err("deep helper chain must fail before stack recursion");
        assert!(error.contains("safe depth limit"), "{error}");
    }

    #[test]
    fn dependency_analysis_is_bounded_for_deep_helper_chains() {
        let mut source = "= later: Int = 1\n> f0() -> Bool { later > 0 }\n".to_string();
        for index in 1..=260 {
            source.push_str(&format!("> f{}() -> Bool {{ f{}() }}\n", index, index - 1));
        }
        source.push_str("= probe: Bool = f260()\n");
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, &source)
            .parse_program()
            .expect("parse dependency-depth fixture");
        let definitions = collect_ground_bindings(&statements, None)
            .expect("collect dependency-depth declarations");
        let probe = statements
            .iter()
            .find_map(|statement| match statement {
                Stmt::Bind(Pat::Var(name), _, expression) if name == "probe" => Some(expression),
                _ => None,
            })
            .expect("probe expression");
        let dependencies = expression_query_dependencies(
            probe,
            &BTreeSet::from(["later".to_string()]),
            &definitions,
        );
        assert_eq!(dependencies, BTreeSet::from(["later".to_string()]));
    }

    #[test]
    fn values_rejects_forward_type_composition_and_indirect_rule_scopes() {
        let fixtures = [
            r#"
# Combined = Base | Third
# Base = First | Second
| condition(value: Combined) -> True
? explore forward_include {
    over condition(value)
    find matches
    bounds { value in values(Combined) }
    output { key [value] representative first }
}
"#,
            r#"
# Scope(flag: Bool) { | current() -> flag }
# Combined = Scope | Closed
| condition(value: Combined) -> True
? explore nested_scope {
    over condition(value)
    find matches
    bounds { value in values(Combined) }
    output { key [value] representative first }
}
"#,
        ];
        for source in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("already initialized declaration prefix")
                        || diagnostic.message.contains("includes open rule scope")
                }),
                "{:?}",
                artifacts.diagnostics
            );
        }
    }
}
