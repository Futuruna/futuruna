//! Closed search-universe elaboration for bounded `? explore` declarations.
//!
//! The parser and type checker deliberately retain source expressions.  This
//! pass is the trust boundary that proves every declared domain is finite,
//! deterministic, and exact before a solver or exhaustive executor may see it.

use super::*;
use std::num::{NonZeroU128, NonZeroU16, NonZeroU64};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod boundary_plan;
mod boundary_search;
mod case_graph;
mod certified_region;
mod classification_regions;
mod exact;
mod exact_stream;
mod mechanism;
mod mechanism_request;
mod mechanism_runtime;
mod mechanism_snapshot;
mod mechanism_stream;
mod probe;
mod probe_codec;
mod probe_io;
mod probe_runner;
mod report;
mod resource_governor;
mod resource_sampler;
mod run_state;
mod run_store;
mod run_stream;
mod run_stream_codec;
mod run_stream_store;
mod source_events;
mod source_proof_plan;
mod stream_coordinator;
mod stream_identity;
mod stream_probe;
mod stream_proof;
mod stream_replay;
mod stream_resource;
mod stream_snapshot;
mod transition;

pub(crate) use transition::TransitionSchemaIdentities;

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

fn runtime_value_from_explore_value(value: &ExploreValue) -> Value {
    match value {
        ExploreValue::Int(value) => Value::Int(*value),
        ExploreValue::FloatBits(bits) => Value::Float(f64::from_bits(*bits)),
        ExploreValue::String(value) => Value::Str(value.clone()),
        ExploreValue::Character(value) => Value::Char(*value),
        ExploreValue::Boolean(value) => Value::Bool(*value),
        ExploreValue::Unit => Value::Unit,
        ExploreValue::List(values) => values.iter().rev().fold(
            Value::Constructor("Nil".into(), vec![].into()),
            |tail, value| {
                Value::Constructor(
                    "Cons".into(),
                    vec![runtime_value_from_explore_value(value), tail].into(),
                )
            },
        ),
        ExploreValue::Set(values) => Value::Set(
            values
                .iter()
                .map(|value| {
                    (
                        value.runtime_display_key(),
                        runtime_value_from_explore_value(value),
                    )
                })
                .collect(),
        ),
        ExploreValue::Tuple(values) => Value::Tuple(
            values
                .iter()
                .map(runtime_value_from_explore_value)
                .collect(),
        ),
        ExploreValue::Constructor {
            variant,
            positional: true,
            fields,
            ..
        } => Value::Constructor(
            variant.clone(),
            fields
                .iter()
                .map(|(_, value)| runtime_value_from_explore_value(value))
                .collect::<Vec<_>>()
                .into(),
        ),
        ExploreValue::Constructor {
            variant,
            positional: false,
            fields,
            ..
        } => Value::NamedConstructor(
            variant.clone(),
            fields
                .iter()
                .map(|(name, value)| (name.clone(), runtime_value_from_explore_value(value)))
                .collect::<Vec<_>>()
                .into(),
        ),
    }
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

    /// Materialize a deliberately small exact domain for the exhaustive
    /// developer preview. Solver-backed exploration keeps ranges and finite
    /// plans lazy; this path refuses to cross its explicit case cap.
    pub fn enumerate_preview(&self, limit: usize) -> Result<Vec<ExploreValue>, String> {
        let count = self
            .cardinality()
            .exact()
            .ok_or_else(|| "exploration domain has more than u128::MAX values".to_string())?;
        if count > limit as u128 {
            return Err(format!(
                "exploration domain has {} values, exceeding preview limit {}",
                count, limit
            ));
        }
        match self {
            Self::Enumerated { values, .. } => Ok(values.clone()),
            Self::IntRange {
                start, cardinality, ..
            } => Ok((0..*cardinality)
                .map(|offset| ExploreValue::Int((*start as i128 + offset as i128) as i64))
                .collect()),
            Self::FiniteType { plan, .. } => plan.enumerate(limit),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreDimensionIr {
    /// Source-independent link back to the normalized typed bound that owns
    /// this generator axis. Product construction uses this identity rather
    /// than presentation names, which may repeat across transition roles.
    pub bound_index: usize,
    pub name: String,
    pub value_ty: Ty,
    pub domain: ExploreExactDomain,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
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
    /// Normalized typed-bound identity used when materializing State/Context
    /// products. It is not inferred from the fact's display name.
    pub bound_index: usize,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
    pub name: String,
    pub value_ty: Ty,
    pub value: ExploreFactValue,
    pub span: Span,
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
pub enum ExploreAfterFieldSourceIr {
    FrameBefore {
        before_field_index: usize,
    },
    Derived {
        expression: Expr,
        environment: TypedExploreDerivedEnvironment,
        /// Canonical after-construction DAG predecessors. The evaluator exposes
        /// only these already-constructed fields through the runtime-only
        /// partial `after` product; the partial value never becomes a state.
        after_dependencies: Vec<ExploreAfterDependencyIr>,
    },
    /// One canonical generator coordinate supplies this field. The domain is
    /// owned by `ExploreUniverseIr::dimensions`; transition construction only
    /// retains the closed coordinate index.
    IndependentDomain {
        dimension_index: usize,
    },
}

/// One compiler-owned edge in the normalized after-construction DAG.
/// `binding_name` is the checked State-field spelling used to validate the
/// indexed edge and expose `after.FIELD`. Any compact bare alias is carried
/// separately by `ExploreFlatAliasIr`; runtime construction never infers
/// either relation from mutable environment contents or incidental field order.
#[derive(Debug, Clone)]
pub struct ExploreAfterDependencyIr {
    pub field_index: usize,
    pub binding_name: String,
}

#[derive(Debug, Clone)]
pub struct ExploreAfterFieldIr {
    pub field_index: usize,
    pub name: String,
    pub value_ty: Ty,
    pub source: ExploreAfterFieldSourceIr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreAfterMembershipPreconstructionIr {
    /// The checked after construction is `before + step` for this Int field.
    /// Membership can therefore close before any fallible derived evaluation.
    RelativeIntStep { step: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreAfterMembershipIr {
    pub after_field_index: usize,
    pub before_dimension_index: usize,
    pub preconstruction: ExploreAfterMembershipPreconstructionIr,
}

#[derive(Debug, Clone)]
pub enum ExploreProductFieldSourceIr {
    Dimension { dimension_index: usize },
    Fact { fact_index: usize },
    TransitionExpression { expression: Expr },
}

#[derive(Debug, Clone)]
pub struct ExploreProductFieldIr {
    pub field_index: usize,
    pub name: String,
    pub value_ty: Ty,
    pub source: ExploreProductFieldSourceIr,
    pub span: Span,
}

/// A closed product schema: every field source already names a closed
/// generator/fact slot or a checked transition expression. Exact execution
/// never resolves product membership through source bounds or display names.
#[derive(Debug, Clone)]
pub struct ExploreProductSchemaIr {
    pub identity: TypedExploreProductSchemaIdentity,
    pub fields: Vec<ExploreProductFieldIr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreFlatAliasRole {
    Context { field_index: usize },
    State { field_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreFlatAliasSource {
    Dimension { dimension_index: usize },
    Fact { fact_index: usize },
}

/// Closed provenance for compact source aliases. This is an evaluation view
/// over the canonical frame, never an alternative transition model.
#[derive(Debug, Clone)]
pub struct ExploreFlatAliasIr {
    pub name: String,
    pub role: ExploreFlatAliasRole,
    pub source: ExploreFlatAliasSource,
}

/// Closed, non-optional before/context/after transition contract consumed by
/// exact execution. Mode, field sources, endpoint membership, and scoped
/// validity define semantics; `boundary_hint` can only accelerate them.
#[derive(Debug, Clone)]
pub struct ExploreTransitionIr {
    pub normalization_version: u32,
    pub mode: ExploreTransitionMode,
    pub state_schema: ExploreProductSchemaIr,
    pub context_schema: ExploreProductSchemaIr,
    pub after_fields: Vec<ExploreAfterFieldIr>,
    pub after_membership: Vec<ExploreAfterMembershipIr>,
    pub flat_aliases: Vec<ExploreFlatAliasIr>,
    /// Checked optimizer metadata. Semantic endpoint construction, membership,
    /// and validity are represented elsewhere in this transition IR.
    pub boundary_hint: Option<ExploreBoundaryIr>,
}

fn validate_after_construction_dag(fields: &[ExploreAfterFieldIr]) -> Result<(), String> {
    for (expected_index, field) in fields.iter().enumerate() {
        if field.field_index != expected_index {
            return Err(format!(
                "after field `{}` has canonical index {}, expected {}",
                field.name, field.field_index, expected_index
            ));
        }
        let ExploreAfterFieldSourceIr::Derived {
            after_dependencies, ..
        } = &field.source
        else {
            continue;
        };
        let mut bindings = BTreeSet::new();
        for dependency in after_dependencies {
            if dependency.field_index >= fields.len() {
                return Err(format!(
                    "derived after field `{}` references absent DAG node {}",
                    field.name, dependency.field_index
                ));
            }
            if dependency.binding_name.is_empty() {
                return Err(format!(
                    "derived after field `{}` has an empty DAG binding",
                    field.name
                ));
            }
            if fields[dependency.field_index].name != dependency.binding_name {
                return Err(format!(
                    "derived after field `{}` binds DAG input `{}` to field `{}`",
                    field.name, dependency.binding_name, fields[dependency.field_index].name
                ));
            }
            if !bindings.insert(dependency.binding_name.as_str()) {
                return Err(format!(
                    "derived after field `{}` binds DAG input `{}` more than once",
                    field.name, dependency.binding_name
                ));
            }
        }
    }

    let mut closed = fields
        .iter()
        .filter(|field| !matches!(&field.source, ExploreAfterFieldSourceIr::Derived { .. }))
        .map(|field| field.field_index)
        .collect::<BTreeSet<_>>();
    let mut open = fields
        .iter()
        .filter(|field| matches!(&field.source, ExploreAfterFieldSourceIr::Derived { .. }))
        .map(|field| field.field_index)
        .collect::<BTreeSet<_>>();
    while !open.is_empty() {
        let ready = open.iter().copied().find(|field_index| {
            let ExploreAfterFieldSourceIr::Derived {
                after_dependencies, ..
            } = &fields[*field_index].source
            else {
                return false;
            };
            after_dependencies
                .iter()
                .all(|dependency| closed.contains(&dependency.field_index))
        });
        let Some(field_index) = ready else {
            return Err(format!(
                "after-construction DAG contains a cycle among nodes {:?}",
                open
            ));
        };
        open.remove(&field_index);
        closed.insert(field_index);
    }
    Ok(())
}

fn close_product_schema(
    schema: &TypedExploreProductSchema,
    bound_dimensions: &BTreeMap<usize, usize>,
    bound_facts: &BTreeMap<usize, usize>,
) -> Result<ExploreProductSchemaIr, String> {
    let mut fields = Vec::with_capacity(schema.fields.len());
    for (field_index, field) in schema.fields.iter().enumerate() {
        let source = match &field.binding {
            TypedExploreProductFieldBinding::Bound { bound_index } => {
                match (
                    bound_dimensions.get(bound_index),
                    bound_facts.get(bound_index),
                ) {
                    (Some(dimension_index), None) => ExploreProductFieldSourceIr::Dimension {
                        dimension_index: *dimension_index,
                    },
                    (None, Some(fact_index)) => ExploreProductFieldSourceIr::Fact {
                        fact_index: *fact_index,
                    },
                    (None, None) => {
                        return Err(format!(
                            "product field `{}` references unclosed bound {}",
                            field.name, bound_index
                        ))
                    }
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "product field `{}` bound {} is both a dimension and a fact",
                            field.name, bound_index
                        ))
                    }
                }
            }
            TypedExploreProductFieldBinding::TransitionExpression { expression } => {
                ExploreProductFieldSourceIr::TransitionExpression {
                    expression: expression.clone(),
                }
            }
        };
        fields.push(ExploreProductFieldIr {
            field_index,
            name: field.name.clone(),
            value_ty: field.ty.clone(),
            source,
            span: field.span,
        });
    }
    Ok(ExploreProductSchemaIr {
        identity: schema.identity.clone(),
        fields,
    })
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
}

#[derive(Debug, Clone)]
pub struct ExploreQueryIr {
    pub query: TypedExploreQuery,
    pub transition: ExploreTransitionIr,
    pub universe: ExploreUniverseIr,
}

impl ExploreQueryIr {
    pub fn boundary_hint(&self) -> Option<&ExploreBoundaryIr> {
        self.transition.boundary_hint.as_ref()
    }
}

/// Default answer-search cap for the public exact-finite executor.
///
/// The internal reference engine can be driven without a case cap, but a
/// first-class API must never make a huge finite Cartesian product
/// operationally unbounded by default. Hitting this limit produces an honest
/// `Partial` report with a canonical open suffix.
pub const DEFAULT_EXPLORE_EXACT_CASE_LIMIT: u128 = 100_000;

/// Operational controls for the public exact-finite Explore backend.
///
/// These values are run metadata rather than query identity. Raising a limit
/// may only refine open evidence; it cannot change a previously closed case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExactOptions {
    pub case_limit: NonZeroU128,
}

impl Default for ExploreExactOptions {
    fn default() -> Self {
        Self {
            case_limit: NonZeroU128::new(DEFAULT_EXPLORE_EXACT_CASE_LIMIT)
                .expect("the default Explore case limit is positive"),
        }
    }
}

/// Optional milestone at which one durable Explore invocation should publish
/// a paused snapshot instead of beginning singleton case work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamPauseAfter {
    Probes,
}

/// Explicit case-level disclosure requested for one durable Explore stream.
///
/// The request is part of immutable run identity. A run created with omitted
/// case evidence cannot later be reopened as a graph-bearing run, or vice
/// versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamCaseGraphRequest {
    Omit,
    Full,
}

impl ExploreStreamCaseGraphRequest {
    fn report_request(self) -> report::ExploreReportRequest {
        report::ExploreReportRequest {
            case_graph: match self {
                Self::Omit => report::ExploreCaseGraphRequest::Omit,
                Self::Full => report::ExploreCaseGraphRequest::Include,
            },
            ledger: report::ExploreLedgerRequest::Omit,
        }
    }
}

/// Controls for one resumable Explore invocation.
///
/// Time, milestone, and finalization choices are operational. The explicit
/// case-level disclosure request is immutable report identity. Reopening the
/// same `run_state` may vary slice controls but must repeat that request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamSliceOptions {
    pub run_state: PathBuf,
    pub max_runtime: Option<Duration>,
    pub pause_after: Option<ExploreStreamPauseAfter>,
    /// Privacy-sensitive case-classification DAG disclosure. Omitted streams
    /// publish counts and result rows without exposing the full case graph.
    pub case_graph: ExploreStreamCaseGraphRequest,
    /// Opt in to the bounded atomic-v1 terminal replay/publication phase once
    /// case classification is closed. This does not replace the required
    /// invocation time/milestone control.
    pub finalize: bool,
}

/// Honest nonterminal outcome of one bounded invocation. Case classification
/// closure is reported separately because final representative/extrema replay
/// is its own required frontier obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamSliceStop {
    ProbeMilestone,
    /// A preceding invocation committed a journal-only pause. This resumed
    /// invocation serviced that pending observer boundary before advancing the
    /// semantic frontier; the artifact says whether materialization succeeded
    /// or had to remain deferred.
    SnapshotCatchUp,
    /// Mechanism counterpart to `SnapshotCatchUp`: a preceding post-probe
    /// journal-only pause left its count view unpublished, and this invocation
    /// services that observer boundary before more semantic work.
    MechanismCheckpointCatchUp,
    TimeLimit,
    ResourcePressure {
        detail: String,
    },
    /// One CaseId remains open because the immutable evaluator contract hit a
    /// deterministic per-case limit. Reopening unchanged will retry that same
    /// rank; it is not an ordinary productive pause.
    EvaluationLimit {
        blocked_rank: u128,
        reason: ExploreExecutionStopReason,
    },
    /// The next confirmed mechanism observation cannot fit an immutable V1
    /// reducer ceiling. This is not transient host pressure: unchanged resume
    /// will reach the same rank and requires a later storage-backed contract.
    MechanismLimit {
        blocked_rank: u128,
        detail: String,
    },
    /// Classification is closed, but the current atomic finalizer cannot fit
    /// this answer inside its versioned witness/snapshot/publication envelope.
    /// The evidence remains valid and resumable for a future chunked finalizer.
    FinalizationLimit {
        phase: String,
        detail: String,
    },
    ClassificationClosedFinalizationPending,
    /// Classification and requested mechanism replay are both closed, but
    /// this count-only profile has no terminal mechanism publication contract.
    /// Reopening unchanged can republish the count checkpoint but cannot seal.
    MechanismObservationClosedTerminalUnavailable,
    /// This invocation closed the required frontier, published the immutable
    /// terminal answer and committed its terminal seal.
    TerminalSealed(ExploreStreamTerminalStatus),
    AlreadySealed(ExploreStreamTerminalStatus),
}

/// Terminal kind recovered from an already sealed durable run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamTerminalStatus {
    Completed,
    Partial,
    Unknown,
    Unsupported,
    Error,
    Cancelled,
}

/// Public cursor for one observable point in the append-only Explore stream.
///
/// Hashes use canonical lowercase SHA-256 spelling. A materialized snapshot
/// report exposes its pre-publication and publication cursors; a journal-only
/// pause has only the final pause cursor because no view record was minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamLifecycle {
    Running,
    Paused,
    Sealed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamCursor {
    pub run_id: String,
    pub sequence: u64,
    pub journal_head: String,
    pub evidence_root: String,
    pub lifecycle: ExploreStreamLifecycle,
    pub last_coverage_epoch: Option<u64>,
}

/// Why this invocation committed a replayable journal pause without also
/// materializing its potentially much larger observer view. This is an
/// operational view status, not evidence that a requested graph or count view
/// hit a semantic or schema capacity bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamObserverDeferral {
    TimeLimit,
    ResourceAdmission {
        detail: String,
    },
    /// A mechanism-enabled run may pause while its source-probe obligation is
    /// still open. Its count checkpoint is intentionally unavailable until
    /// that milestone closes, while the journal remains a complete resume
    /// point.
    ProbeIncomplete,
}

/// Observable artifact status returned by one durable invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamArtifact {
    /// Cursor-bearing, bounded observable checkpoint followed by one LF. The
    /// bytes are installed content-addressably and named by a subsequent
    /// `SnapshotPublished` journal record before the invocation pauses.
    CheckpointSnapshotJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
    },
    /// Cursor-bearing bounded receipt published when an admitted full-snapshot
    /// attempt reports capacity at this cursor. This is neither a partial
    /// snapshot nor a claim that a later attempt can never fit.
    CheckpointSnapshotUnavailableJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
        detail: String,
    },
    /// Cursor-bearing, count-only mechanism checkpoint followed by one LF.
    /// Signature definitions, CaseIds and incidence remain in the private
    /// authenticated run state until their public graph schema is defined.
    MechanismCheckpointJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
    },
    /// Bounded receipt published when the admitted mechanism-checkpoint
    /// renderer cannot fit its immutable V1 envelope at this cursor.
    MechanismCheckpointUnavailableJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
        detail: String,
    },
    /// The append-only journal is already a complete resume checkpoint. When
    /// the bounded snapshot phase is not admitted, pausing must not spend the
    /// host reserve to manufacture a materialized view.
    JournalOnlyCheckpoint {
        observer_deferral: ExploreStreamObserverDeferral,
    },
    /// Mechanism-profile journal checkpoint whose separately admitted
    /// count-only observer view was unavailable at this invocation boundary.
    MechanismJournalOnlyCheckpoint {
        observer_deferral: ExploreStreamObserverDeferral,
    },
    /// History-independent immutable terminal answer bytes and their raw blob
    /// address. The final cursor commits the separate semantic payload hash.
    TerminalResultJson {
        canonical_json: Vec<u8>,
        blob_digest: String,
    },
}

/// One canonical observable or terminal point in the durable stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamSliceReport {
    pub stop: ExploreStreamSliceStop,
    /// Cursor after the publication/pause or terminal-seal records committed by
    /// this invocation.
    pub final_cursor: ExploreStreamCursor,
    pub probe_milestone_complete: bool,
    /// Whole singleton cases evaluated and committed by this invocation.
    pub singleton_cases_evaluated_this_slice: u128,
    /// Total newly closed support, including weighted proof/structural regions.
    pub closed_cases_this_slice: u128,
    pub artifact: ExploreStreamArtifact,
}

/// One declared search axis in canonical transition-role order, before
/// constraints or question evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlanAxis {
    pub name: String,
    pub bound_index: usize,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
    pub cardinality: ExploreCardinality,
}

/// Static boundary-search shape derived by the ordinary Explore elaborator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlanBoundary {
    pub axis: String,
    pub axis_dimension_index: usize,
    pub step: i64,
    /// Product of the boundary-eligible axis pairs and every other declared
    /// axis, before `where` constraints or question evaluation.
    pub eligible_unconstrained_pairs: ExploreCardinality,
}

/// A no-execution cost/search plan for one checked Explore query.
///
/// This is planning metadata, not result evidence: it evaluates no cases,
/// establishes no closure, and contains no mechanism or symbolic candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlan {
    pub query_name: String,
    pub axes: Vec<ExploreCostPlanAxis>,
    /// `U`: the declared Cartesian product before constraints and before the
    /// queried rule.
    pub declared_cartesian_count: ExploreCardinality,
    pub boundary: Option<ExploreCostPlanBoundary>,
    pub requested_case_limit: u128,
    /// Number of singleton assignments a naive exact exhaustion would plan to
    /// classify under the requested cap.
    pub naive_singleton_classifications: u128,
    /// Assignments necessarily left open by that cap. Available only when `U`
    /// fits in `u128`; this is still a cost estimate, not observed closure.
    pub naive_remaining_open_lower_bound: Option<u128>,
}

/// Certainty attached to one nonnegative Explore population count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreCountEvidence {
    Exact(u128),
    LowerBound(u128),
    Unknown,
}

/// Exact populations remain distinct: declared assignments (`U`), admissible
/// cases (`D`), matching cases (`M`) and emitted result identities (`R`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionCounts {
    pub declared_assignments: ExploreCountEvidence,
    pub admissible_configurations: ExploreCountEvidence,
    pub matching_configurations: ExploreCountEvidence,
    pub distinct_result_keys: ExploreCountEvidence,
}

/// Group populations surrounding the post-aggregation `having` view.
/// Suppressed cases remain part of D/M and of any requested case evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionGroupCounts {
    pub raw_groups: ExploreCountEvidence,
    pub emitted_groups: ExploreCountEvidence,
    pub suppressed_groups: ExploreCountEvidence,
    pub qualifying_configurations: ExploreCountEvidence,
    pub suppressed_configurations: ExploreCountEvidence,
}

/// Public, name-stable description of the post-aggregation result view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionGroupFilter {
    All,
    Varies { extrema_name: String },
}

/// Matching coverage over the admissible population. This is independent of
/// whether execution itself completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionCoverage {
    Empty,
    None,
    Some,
    All,
    Undetermined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionClosure {
    Open,
    Closed,
}

/// Closure of answer/case/value layers. Mechanism evidence is deliberately
/// reported separately and never downgrades a closed answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionClosures {
    /// Key discovery plus any requested extrema aggregation and `having`
    /// classification.
    pub projection: ExploreExecutionClosure,
    pub admissibility: ExploreExecutionClosure,
    pub polarity: ExploreExecutionClosure,
    pub representatives: ExploreExecutionClosure,
    pub rows: ExploreExecutionClosure,
    pub views: ExploreExecutionClosure,
}

/// One name/value pair authorized by the query's `key` or `show` projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionField {
    pub name: String,
    pub value: ExploreValue,
}

/// Exact closed extrema of one integer measure within a projected key group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionExtrema {
    pub name: String,
    pub minimum: i64,
    pub maximum: i64,
    pub spread: u128,
    pub minimum_tie_support: u128,
    pub maximum_tie_support: u128,
    /// Canonical domain ordinals of a freshly replayed minimum witness.
    pub minimum_witness_case_id: Vec<u128>,
    /// Canonical domain ordinals of a freshly replayed maximum witness.
    pub maximum_witness_case_id: Vec<u128>,
}

/// One canonical projected result with a replay-confirmed representative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionRow {
    pub key: Vec<ExploreExecutionField>,
    pub extrema: Vec<ExploreExecutionExtrema>,
    pub shown: Vec<ExploreExecutionField>,
    /// Exact when the projected key class is closed; otherwise a confirmed
    /// lower bound over the evaluated closed cases.
    pub support: ExploreCountEvidence,
    /// Domain ordinals in canonical Context → Before → independent-After axis
    /// order, not raw hidden input values.
    pub representative_case_id: Vec<u128>,
}

/// Structural identity and presentation label for one CaseId coordinate.
/// Labels may repeat across roles; consumers must use the indexed descriptor
/// rather than parse or compare the display spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionDimension {
    pub bound_index: usize,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
    pub label: String,
}

impl ExploreExecutionDimension {
    pub fn qualified_label(&self) -> String {
        let role = match self.role {
            ExploreGeneratorAxisRole::Context => "context",
            ExploreGeneratorAxisRole::Before => "before",
            ExploreGeneratorAxisRole::AfterIndependent => "after",
        };
        format!("{role}.{}", self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionLimitResource {
    Steps,
    CollectionMembers { operation: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionPhase {
    Initialization,
    DerivedFact { name: String },
    BoundaryEndpoint,
    Constraint { index: usize },
    Question,
    Key { name: String },
    Extrema { name: String },
    Show { name: String },
    Objective,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionStopReason {
    CaseLimit {
        limit: u128,
    },
    RuntimeLimit {
        resource: ExploreExecutionLimitResource,
        limit: u128,
        observed: u128,
        phase: ExploreExecutionPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionMethod {
    ExactFiniteExhaustion,
    ExactFiniteCertifiedClosure,
}

/// Terminal answer status. `Partial` contains only evidence already closed or
/// replay-confirmed; `Unsupported` is never presented as a proof of absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionOutcome {
    Complete {
        method: ExploreExecutionMethod,
        evidence: ExploreExecutionEvidence,
    },
    Partial {
        stop: ExploreExecutionStopReason,
        evidence: ExploreExecutionEvidence,
    },
    Unknown {
        reason: String,
        evidence: ExploreExecutionEvidence,
    },
    Unsupported {
        diagnostic: String,
    },
    Error {
        diagnostics: Vec<String>,
    },
}

impl ExploreExecutionOutcome {
    pub fn evidence(&self) -> Option<&ExploreExecutionEvidence> {
        match self {
            Self::Complete { evidence, .. }
            | Self::Partial { evidence, .. }
            | Self::Unknown { evidence, .. } => Some(evidence),
            Self::Unsupported { .. } | Self::Error { .. } => None,
        }
    }
}

/// Mechanism tracing is orthogonal to exact case closure. The first public
/// exact backend exposes the absence of mechanism evidence explicitly rather
/// than claiming that zero mechanisms exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionMechanismEvidence {
    UnavailableDeferred,
}

/// Work accounting for the exact search order. Source-event identities stay
/// private scheduling metadata; this evidence reports only auditable counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionSearchEvidence {
    Canonical {
        classified_cases: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
    SourceCandidateFirst {
        distinct_source_candidates: u128,
        scheduled_source_candidates: u128,
        evaluated_source_candidates: u128,
        scheduled_fallback_cases: u128,
        evaluated_fallback_cases: u128,
        singleton_closed_cases: u128,
        certified_region_closed_cases: u128,
        pending_evaluations: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionEvidence {
    /// Structural descriptors in canonical CaseId axis order.
    pub dimensions: Vec<ExploreExecutionDimension>,
    pub axis_cardinalities: Vec<u128>,
    pub key_names: Vec<String>,
    pub extrema_names: Vec<String>,
    pub shown_names: Vec<String>,
    pub search: ExploreExecutionSearchEvidence,
    pub counts: ExploreExecutionCounts,
    pub group_counts: ExploreExecutionGroupCounts,
    pub group_filter: ExploreExecutionGroupFilter,
    pub coverage: ExploreExecutionCoverage,
    pub closures: ExploreExecutionClosures,
    pub results: Vec<ExploreExecutionRow>,
}

/// Backend-neutral public view of one exact-finite Explore run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionReport {
    pub query_name: String,
    pub polarity: ExplorePolarity,
    pub outcome: ExploreExecutionOutcome,
    pub mechanism: ExploreExecutionMechanismEvidence,
    pub limits: ExploreExecutionLimits,
}

#[derive(Debug, Clone)]
pub enum ExploreExecutionPreparationError {
    Diagnostics(Vec<Diagnostic>),
    Selection(String),
    Execution(String),
}

impl std::fmt::Display for ExploreExecutionPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "exploration has {} type-check diagnostic{}",
                diagnostics.len(),
                if diagnostics.len() == 1 { "" } else { "s" }
            ),
            Self::Selection(message) | Self::Execution(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ExploreExecutionPreparationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionLimits {
    pub case_limit: u128,
    pub step_limit: usize,
    pub collection_limit: usize,
}

fn public_count(count: report::ExploreCount) -> ExploreCountEvidence {
    match count {
        report::ExploreCount::Exact(value) => ExploreCountEvidence::Exact(value),
        report::ExploreCount::LowerBound(value) => ExploreCountEvidence::LowerBound(value),
        report::ExploreCount::Unknown => ExploreCountEvidence::Unknown,
    }
}

fn public_closure(closure: report::ExploreClosure) -> ExploreExecutionClosure {
    match closure {
        report::ExploreClosure::Open => ExploreExecutionClosure::Open,
        report::ExploreClosure::Closed => ExploreExecutionClosure::Closed,
    }
}

fn public_phase(phase: report::ExploreEvaluationPhase) -> ExploreExecutionPhase {
    match phase {
        report::ExploreEvaluationPhase::Initialization => ExploreExecutionPhase::Initialization,
        report::ExploreEvaluationPhase::DerivedFact { name } => {
            ExploreExecutionPhase::DerivedFact { name }
        }
        report::ExploreEvaluationPhase::BoundaryEndpoint => ExploreExecutionPhase::BoundaryEndpoint,
        report::ExploreEvaluationPhase::Constraint { index } => {
            ExploreExecutionPhase::Constraint { index }
        }
        report::ExploreEvaluationPhase::Question => ExploreExecutionPhase::Question,
        report::ExploreEvaluationPhase::Key { name } => ExploreExecutionPhase::Key { name },
        report::ExploreEvaluationPhase::Extrema { name } => ExploreExecutionPhase::Extrema { name },
        report::ExploreEvaluationPhase::Show { name } => ExploreExecutionPhase::Show { name },
        report::ExploreEvaluationPhase::Objective => ExploreExecutionPhase::Objective,
        report::ExploreEvaluationPhase::Replay => ExploreExecutionPhase::Replay,
    }
}

fn public_stop(stop: report::ExploreStopReason) -> ExploreExecutionStopReason {
    match stop {
        report::ExploreStopReason::CaseLimit { limit } => {
            ExploreExecutionStopReason::CaseLimit { limit }
        }
        report::ExploreStopReason::RuntimeLimit {
            resource,
            limit,
            observed,
            phase,
        } => ExploreExecutionStopReason::RuntimeLimit {
            resource: match resource {
                report::ExploreLimitResource::Steps => ExploreExecutionLimitResource::Steps,
                report::ExploreLimitResource::CollectionMembers { operation } => {
                    ExploreExecutionLimitResource::CollectionMembers { operation }
                }
            },
            limit,
            observed,
            phase: public_phase(phase),
        },
    }
}

fn public_evidence(evidence: report::ExploreExactEvidence) -> ExploreExecutionEvidence {
    let schema = evidence.schema;
    let counts = evidence.counts;
    let group_counts = evidence.group_counts;
    let closures = evidence.closures;
    let search = match evidence.search {
        report::ExploreSearchEvidence::Canonical {
            classified_cases,
            remaining_open_cases,
            exhausted,
        } => ExploreExecutionSearchEvidence::Canonical {
            classified_cases,
            remaining_open_cases,
            exhausted,
        },
        report::ExploreSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates,
            scheduled_source_candidates,
            evaluated_source_candidates,
            scheduled_fallback_cases,
            evaluated_fallback_cases,
            singleton_closed_cases,
            certified_region_closed_cases,
            pending_evaluations,
            remaining_open_cases,
            exhausted,
        } => ExploreExecutionSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates,
            scheduled_source_candidates,
            evaluated_source_candidates,
            scheduled_fallback_cases,
            evaluated_fallback_cases,
            singleton_closed_cases,
            certified_region_closed_cases,
            pending_evaluations,
            remaining_open_cases,
            exhausted,
        },
    };
    let group_filter = match schema.group_filter {
        report::ExploreGroupFilter::All => ExploreExecutionGroupFilter::All,
        report::ExploreGroupFilter::Varies { extrema_index } => {
            ExploreExecutionGroupFilter::Varies {
                extrema_name: schema
                    .extrema_names
                    .get(extrema_index)
                    .cloned()
                    .expect("validated Explore varies index names an extrema field"),
            }
        }
    };
    ExploreExecutionEvidence {
        dimensions: schema
            .dimensions
            .iter()
            .map(|dimension| ExploreExecutionDimension {
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                label: dimension.label.clone(),
            })
            .collect(),
        axis_cardinalities: schema.axis_cardinalities.into_vec(),
        key_names: schema.key_names.clone().into_vec(),
        extrema_names: schema.extrema_names.clone().into_vec(),
        shown_names: schema.shown_names.clone().into_vec(),
        search,
        counts: ExploreExecutionCounts {
            declared_assignments: public_count(counts.declared_assignments),
            admissible_configurations: public_count(counts.admissible_configurations),
            matching_configurations: public_count(counts.matching_configurations),
            distinct_result_keys: public_count(counts.distinct_result_keys),
        },
        group_counts: ExploreExecutionGroupCounts {
            raw_groups: public_count(group_counts.raw_groups),
            emitted_groups: public_count(group_counts.emitted_groups),
            suppressed_groups: public_count(group_counts.suppressed_groups),
            qualifying_configurations: public_count(group_counts.qualifying_configurations),
            suppressed_configurations: public_count(group_counts.suppressed_configurations),
        },
        group_filter,
        coverage: match evidence.coverage {
            report::ExploreCoverage::Empty => ExploreExecutionCoverage::Empty,
            report::ExploreCoverage::None => ExploreExecutionCoverage::None,
            report::ExploreCoverage::Some => ExploreExecutionCoverage::Some,
            report::ExploreCoverage::All => ExploreExecutionCoverage::All,
            report::ExploreCoverage::Undetermined => ExploreExecutionCoverage::Undetermined,
        },
        closures: ExploreExecutionClosures {
            projection: public_closure(closures.projection),
            admissibility: public_closure(closures.admissibility),
            polarity: public_closure(closures.polarity),
            representatives: public_closure(closures.representatives),
            rows: public_closure(closures.rows),
            views: public_closure(closures.views),
        },
        results: evidence
            .results
            .into_vec()
            .into_iter()
            .map(|row| ExploreExecutionRow {
                key: schema
                    .key_names
                    .iter()
                    .cloned()
                    .zip(row.key.values().iter().cloned())
                    .map(|(name, value)| ExploreExecutionField { name, value })
                    .collect(),
                extrema: schema
                    .extrema_names
                    .iter()
                    .cloned()
                    .zip(row.extrema.into_vec())
                    .map(|(name, summary)| ExploreExecutionExtrema {
                        name,
                        minimum: summary.minimum,
                        maximum: summary.maximum,
                        spread: summary.spread,
                        minimum_tie_support: summary.minimum_tie_support,
                        maximum_tie_support: summary.maximum_tie_support,
                        minimum_witness_case_id: summary.minimum_witness.ordinals().to_vec(),
                        maximum_witness_case_id: summary.maximum_witness.ordinals().to_vec(),
                    })
                    .collect(),
                shown: schema
                    .shown_names
                    .iter()
                    .cloned()
                    .zip(row.shown.into_vec())
                    .map(|(name, value)| ExploreExecutionField { name, value })
                    .collect(),
                support: public_count(row.support),
                representative_case_id: row.representative.ordinals().to_vec(),
            })
            .collect(),
    }
}

fn public_exact_report(
    report: report::ExploreExactReport,
    options: ExploreExactOptions,
) -> ExploreExecutionReport {
    let report::ExploreExactReport {
        query_name,
        polarity,
        mechanism,
        outcome,
    } = report;
    let outcome = match outcome {
        report::ExploreExactOutcome::Complete { method, evidence } => {
            ExploreExecutionOutcome::Complete {
                method: match method {
                    report::ExploreCompletionMethod::ExactFiniteExhaustion => {
                        ExploreExecutionMethod::ExactFiniteExhaustion
                    }
                    report::ExploreCompletionMethod::ExactFiniteCertifiedClosure => {
                        ExploreExecutionMethod::ExactFiniteCertifiedClosure
                    }
                },
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Partial { stop, evidence } => {
            ExploreExecutionOutcome::Partial {
                stop: public_stop(stop),
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Unknown { reason, evidence } => {
            ExploreExecutionOutcome::Unknown {
                reason,
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Unsupported { diagnostic } => {
            ExploreExecutionOutcome::Unsupported { diagnostic }
        }
        report::ExploreExactOutcome::Error { diagnostics } => ExploreExecutionOutcome::Error {
            diagnostics: diagnostics.into_vec(),
        },
    };
    let mechanism = match mechanism {
        report::ExploreMechanismEvidence::Unavailable {
            reason: report::ExploreMechanismUnavailableReason::Deferred,
        } => ExploreExecutionMechanismEvidence::UnavailableDeferred,
    };
    ExploreExecutionReport {
        query_name,
        polarity,
        outcome,
        mechanism,
        limits: ExploreExecutionLimits {
            case_limit: options.case_limit.get(),
            step_limit: report::DEFAULT_EXPLORE_STEP_LIMIT,
            collection_limit: report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
        },
    }
}

/// Execute one already checked and elaborated finite Explore query.
///
/// This is the durable exact backend used by the public command. It consumes
/// ordinary `check_with_artifacts` evidence, requires a caller-supplied finite
/// case cap, and publishes
/// only replay-confirmed projected values. Its report request is deliberately
/// the privacy-safe baseline: projected rows only, with no case ledger or case
/// graph disclosure.
fn execute_exact(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    options: ExploreExactOptions,
) -> Result<ExploreExecutionReport, String> {
    let budget = report::ExploreExecutionBudget::new(
        Some(options.case_limit.get()),
        report::DEFAULT_EXPLORE_STEP_LIMIT,
        report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
    )?;
    let report = match source_proof_plan::prepare_source_proof_plan(
        artifacts,
        accepted_query_index,
        source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
    ) {
        Ok(plan) => exact::execute_exact_finite_candidate_first(
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report::ExploreReportRequest::baseline(),
            budget,
            &plan,
        ),
        // Source proof is an optimization. Unsupported or bounded-out
        // analysis cannot shrink U and therefore falls back to canonical
        // exact evaluation under the same caller case limit.
        Err(error) if error.permits_canonical_fallback() => exact::execute_exact_finite(
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report::ExploreReportRequest::baseline(),
            budget,
        ),
        // A proof artifact that was produced but fails extraction,
        // certification, or accounting is an integrity failure. It must not
        // be hidden by silently retrying the same query canonically.
        Err(error) => return Err(error.to_string()),
    }?;
    Ok(public_exact_report(report, options))
}

fn select_checked_exact_query_index(
    artifacts: &TypeCheckArtifacts,
    query_name: Option<&str>,
) -> Result<usize, ExploreExecutionPreparationError> {
    if let Some(query_name) = query_name {
        return artifacts
            .exploration_universes
            .iter()
            .position(|candidate| candidate.query.name.as_deref() == Some(query_name))
            .ok_or_else(|| {
                ExploreExecutionPreparationError::Selection(format!(
                    "exploration `{query_name}` was not found"
                ))
            });
    }
    if artifacts.exploration_universes.len() == 1 {
        return Ok(0);
    }
    if artifacts.exploration_universes.is_empty() {
        return Err(ExploreExecutionPreparationError::Selection(
            "the program contains no selectable exploration".to_string(),
        ));
    }
    let names = artifacts
        .exploration_universes
        .iter()
        .filter_map(|candidate| candidate.query.name.as_deref())
        .collect::<Vec<_>>()
        .join(", ");
    Err(ExploreExecutionPreparationError::Selection(format!(
        "the program contains multiple explorations; select one with --query ({names})"
    )))
}

fn cost_plan(query: &ExploreQueryIr, options: ExploreExactOptions) -> ExploreCostPlan {
    let declared_cartesian_count = query.universe.cartesian_count_before_constraints.clone();
    let exact_declared = declared_cartesian_count.exact();
    let requested_case_limit = options.case_limit.get();
    let naive_singleton_classifications = exact_declared
        .map(|declared| declared.min(requested_case_limit))
        .unwrap_or(requested_case_limit);
    ExploreCostPlan {
        query_name: query
            .query
            .name
            .clone()
            .unwrap_or_else(|| "<anonymous>".to_string()),
        axes: query
            .universe
            .dimensions
            .iter()
            .map(|dimension| ExploreCostPlanAxis {
                name: dimension.name.clone(),
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                cardinality: dimension.domain.cardinality(),
            })
            .collect(),
        declared_cartesian_count,
        boundary: query
            .boundary_hint()
            .map(|boundary| ExploreCostPlanBoundary {
                axis: boundary.axis.clone(),
                axis_dimension_index: boundary.axis_dimension_index,
                step: boundary.step,
                eligible_unconstrained_pairs: boundary.eligible_unconstrained_pairs.clone(),
            }),
        requested_case_limit,
        naive_singleton_classifications,
        naive_remaining_open_lower_bound: exact_declared
            .map(|declared| declared.saturating_sub(naive_singleton_classifications)),
    }
}

/// Check, elaborate, and select one exact-finite exploration without
/// initializing an interpreter or evaluating any case.
///
/// Query selection is shared with [`execute_checked_exact`]. The returned
/// metadata describes the declared search shape and a naive cap-limited cost;
/// it is not result evidence and does not establish closure.
pub fn plan_checked_exact(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreExactOptions,
) -> Result<ExploreCostPlan, ExploreExecutionPreparationError> {
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir, source);
    if !artifacts.diagnostics.is_empty() {
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }
    let selected = select_checked_exact_query_index(&artifacts, query_name)?;
    Ok(cost_plan(
        &artifacts.exploration_universes[selected],
        options,
    ))
}

/// Check, elaborate, select and execute one exact-finite exploration as one
/// inseparable operation. This prevents callers from combining statements,
/// artifacts and a query IR produced by different checks.
pub fn execute_checked_exact(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreExactOptions,
) -> Result<ExploreExecutionReport, ExploreExecutionPreparationError> {
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir.clone(), source);
    if !artifacts.diagnostics.is_empty() {
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }

    let selected = select_checked_exact_query_index(&artifacts, query_name)?;

    execute_exact(
        statements,
        source_dir.as_deref(),
        &artifacts,
        selected,
        options,
    )
    .map_err(ExploreExecutionPreparationError::Execution)
}

enum ExactStreamWorkAdmission {
    Granted(stream_resource::ExactStreamWorkInFlight),
    TimeLimit,
    ResourcePause(stream_resource::ExactStreamResourcePauseReason),
}

fn admit_exact_stream_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    subject: stream_resource::ExactStreamWorkSubject,
    deadline: Option<Instant>,
) -> Result<ExactStreamWorkAdmission, ExploreExecutionPreparationError> {
    loop {
        let now = Instant::now();
        if deadline.is_some_and(|deadline| now >= deadline) {
            let _ = resources.stop_at_work_boundary();
            return Ok(ExactStreamWorkAdmission::TimeLimit);
        }

        let owned = resources.conservative_in_process_owned_snapshot();
        let poll = resources.poll(owned, None, Some(subject));
        match poll.action {
            stream_resource::ExactStreamResourceAction::Dispatch(permit) => {
                if permit.subject() != subject {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor dispatched authority for another Explore work subject"
                            .to_string(),
                    ));
                }
                let in_flight = resources.begin_work(permit).map_err(|error| {
                    ExploreExecutionPreparationError::Execution(format!(
                        "cannot consume exact-stream resource permit: {error:?}"
                    ))
                })?;
                return Ok(ExactStreamWorkAdmission::Granted(in_flight));
            }
            stream_resource::ExactStreamResourceAction::Pause(reason) => {
                return Ok(ExactStreamWorkAdmission::ResourcePause(reason));
            }
            stream_resource::ExactStreamResourceAction::Wait(_) => {
                let now = Instant::now();
                let mut wake = poll
                    .next_host_sample_due
                    .unwrap_or_else(|| now.checked_add(Duration::from_millis(10)).unwrap_or(now));
                if let Some(deadline) = deadline {
                    wake = wake.min(deadline);
                }
                if wake > now {
                    std::thread::sleep(wake.saturating_duration_since(now));
                } else {
                    std::thread::yield_now();
                }
            }
        }
    }
}

/// Try exactly once to admit the optional materialized-view phase. A semantic
/// work loop may wait for a stable resource window; an invocation that has
/// already reached a useful pause boundary must instead durably pause and let
/// a later invocation mint the view. This keeps checkpointing from consuming
/// the host reserve precisely when the governor has withdrawn work authority.
fn try_admit_exact_stream_snapshot_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    deadline: Option<Instant>,
) -> Result<ExactStreamWorkAdmission, ExploreExecutionPreparationError> {
    let now = Instant::now();
    if deadline.is_some_and(|deadline| now >= deadline) {
        let _ = resources.stop_at_work_boundary();
        return Ok(ExactStreamWorkAdmission::TimeLimit);
    }

    let subject = stream_resource::ExactStreamWorkSubject::SnapshotPublicationPhase;
    let owned = resources.conservative_in_process_owned_snapshot();
    let poll = resources.poll(owned, None, Some(subject));
    match poll.action {
        stream_resource::ExactStreamResourceAction::Dispatch(permit) => {
            if permit.subject() != subject {
                return Err(ExploreExecutionPreparationError::Execution(
                    "resource governor dispatched authority for another Explore snapshot phase"
                        .to_string(),
                ));
            }
            let in_flight = resources.begin_work(permit).map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot consume exact-stream snapshot resource permit: {error:?}"
                ))
            })?;
            Ok(ExactStreamWorkAdmission::Granted(in_flight))
        }
        stream_resource::ExactStreamResourceAction::Pause(reason)
        | stream_resource::ExactStreamResourceAction::Wait(reason) => {
            Ok(ExactStreamWorkAdmission::ResourcePause(reason))
        }
    }
}

fn finish_exact_stream_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    in_flight: stream_resource::ExactStreamWorkInFlight,
) -> Result<(), ExploreExecutionPreparationError> {
    resources
        .finish_or_abandon_work(in_flight)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot close exact-stream resource work unit: {error:?}"
            ))
        })
}

fn public_exact_stream_cursor(cursor: run_stream::ExploreRunCursor) -> ExploreStreamCursor {
    ExploreStreamCursor {
        run_id: cursor.run_id().to_lowercase_hex(),
        sequence: cursor.sequence(),
        journal_head: cursor.journal_head().to_lowercase_hex(),
        evidence_root: cursor.evidence_root().to_lowercase_hex(),
        lifecycle: match cursor.lifecycle() {
            run_stream::RunLifecycle::Running => ExploreStreamLifecycle::Running,
            run_stream::RunLifecycle::Paused => ExploreStreamLifecycle::Paused,
            run_stream::RunLifecycle::Sealed => ExploreStreamLifecycle::Sealed,
        },
        last_coverage_epoch: cursor.last_coverage_epoch().map(|epoch| epoch.get()),
    }
}

/// Publish a replay-verifiable checkpoint for the current running cursor, then
/// append the invocation's pause record. Keeping those as two ordered records
/// avoids the circularity of making a snapshot hash name the event that names
/// that same hash. The returned report carries both cursors and the typed stop.
fn publish_prepared_snapshot_and_pause_exact_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    prepared_snapshot: stream_coordinator::PreparedExactObservableSnapshotPublication,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let materialization_capacity_detail = prepared_snapshot
        .materialization_capacity_detail()
        .map(str::to_string);
    let probe_milestone_complete = prepared_snapshot.probe_milestone_complete();
    let checkpoint_cursor = prepared_snapshot.cursor();
    checkpoint_cursor.sequence().checked_add(2).ok_or_else(|| {
        ExploreExecutionPreparationError::Execution(
            "exact-stream journal sequence cannot fit checkpoint publication and pause".to_string(),
        )
    })?;
    let closed_cases_this_slice = prepared_snapshot
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one invocation".to_string(),
            )
        })?;
    let blob_digest = coordinator
        .publish_prepared_snapshot(&prepared_snapshot)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot publish exact-stream checkpoint: {error}"
            ))
        })?;
    let publication_cursor = coordinator.stream().cursor();
    let final_cursor = coordinator.pause(pause_reason).map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "checkpoint {} was published at sequence {}, but the exact stream could not append its pause record: {error}",
            blob_digest.to_lowercase_hex(),
            publication_cursor.sequence(),
        ))
    })?;
    let blob_digest = blob_digest.to_lowercase_hex();
    let checkpoint_cursor = public_exact_stream_cursor(checkpoint_cursor);
    let publication_cursor = public_exact_stream_cursor(publication_cursor);
    let canonical_json_line = prepared_snapshot.into_canonical_json_line();
    let artifact = match materialization_capacity_detail {
        Some(detail) => ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
            detail,
        },
        None => ExploreStreamArtifact::CheckpointSnapshotJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
        },
    };
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(final_cursor),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact,
    })
}

/// Publish the mechanism count checkpoint at the current cursor, then append
/// the ordinary pause record. Mechanism signatures and incidence remain in
/// the private authenticated run state; this bounded observer is deliberately
/// count-only until the public mechanism-DAG schema exists.
fn publish_prepared_mechanism_checkpoint_and_pause_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    prepared_checkpoint: stream_coordinator::PreparedMechanismObservableCheckpointPublicationV1,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let materialization_capacity_detail = prepared_checkpoint
        .materialization_capacity_detail()
        .map(str::to_string);
    let checkpoint_cursor = prepared_checkpoint.cursor();
    checkpoint_cursor.sequence().checked_add(2).ok_or_else(|| {
        ExploreExecutionPreparationError::Execution(
            "mechanism-stream journal sequence cannot fit checkpoint publication and pause"
                .to_string(),
        )
    })?;
    let closed_cases_this_slice = coordinator
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "mechanism-stream closed support regressed during one invocation".to_string(),
            )
        })?;
    let blob_digest = coordinator
        .publish_prepared_mechanism_checkpoint(&prepared_checkpoint)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot publish mechanism-stream checkpoint: {error}"
            ))
        })?;
    let publication_cursor = coordinator.stream().cursor();
    let final_cursor = coordinator.pause(pause_reason).map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "mechanism checkpoint {} was published at sequence {}, but the stream could not append its pause record: {error}",
            blob_digest.to_lowercase_hex(),
            publication_cursor.sequence(),
        ))
    })?;
    let blob_digest = blob_digest.to_lowercase_hex();
    let checkpoint_cursor = public_exact_stream_cursor(checkpoint_cursor);
    let publication_cursor = public_exact_stream_cursor(publication_cursor);
    let canonical_json_line = prepared_checkpoint.into_canonical_json_line();
    let artifact = match materialization_capacity_detail {
        Some(detail) => ExploreStreamArtifact::MechanismCheckpointUnavailableJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
            detail,
        },
        None => ExploreStreamArtifact::MechanismCheckpointJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
        },
    };
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(final_cursor),
        probe_milestone_complete: true,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact,
    })
}

fn pause_exact_stream_slice_without_snapshot(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    observer_deferral: ExploreStreamObserverDeferral,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let probe_milestone_complete = coordinator
        .probe_progress()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive journal-only source-probe progress: {error}"
            ))
        })?
        .complete();
    let closed_cases_this_slice = coordinator
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one invocation".to_string(),
            )
        })?;
    let final_cursor = coordinator.pause(pause_reason).map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "cannot append journal-only exact-stream pause: {error}"
        ))
    })?;
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(final_cursor),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact: if coordinator.mechanism_checkpoint_enabled() {
            ExploreStreamArtifact::MechanismJournalOnlyCheckpoint { observer_deferral }
        } else {
            ExploreStreamArtifact::JournalOnlyCheckpoint { observer_deferral }
        },
    })
}

/// Mint a materialized snapshot only while the 80%-ceiling governor grants a
/// bounded phase. The append-only journal remains the authoritative resume
/// checkpoint, so denied view work degrades to a typed journal-only pause
/// rather than borrowing memory from the host reserve.
fn publish_or_defer_and_pause_exact_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    _query: &ExploreQueryIr,
    deadline: Option<Instant>,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if coordinator.mechanism_checkpoint_enabled() {
        if !coordinator.probe_phase_complete() {
            return pause_exact_stream_slice_without_snapshot(
                coordinator,
                pause_reason,
                stop,
                ExploreStreamObserverDeferral::ProbeIncomplete,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }
        return publish_or_defer_and_pause_mechanism_stream_slice(
            coordinator,
            resources,
            deadline,
            pause_reason,
            stop,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    match try_admit_exact_stream_snapshot_work(resources, deadline)? {
        ExactStreamWorkAdmission::Granted(in_flight) => {
            let mut snapshot_authority = match in_flight.into_snapshot_publication_authority() {
                Ok(authority) => authority,
                Err(in_flight) => {
                    finish_exact_stream_work(resources, in_flight)?;
                    return Err(ExploreExecutionPreparationError::Execution(
                        "admitted Explore work unit did not carry snapshot-publication authority"
                            .to_string(),
                    ));
                }
            };
            let prepared_snapshot = match coordinator
                .prepare_observable_snapshot_publication(&mut snapshot_authority)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
                    return Err(ExploreExecutionPreparationError::Execution(format!(
                        "cannot prepare exact-stream snapshot publication: {error}"
                    )));
                }
            };
            let publication = publish_prepared_snapshot_and_pause_exact_stream_slice(
                coordinator,
                prepared_snapshot,
                pause_reason,
                stop,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
            finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
            publication
        }
        ExactStreamWorkAdmission::TimeLimit => pause_exact_stream_slice_without_snapshot(
            coordinator,
            pause_reason,
            stop,
            ExploreStreamObserverDeferral::TimeLimit,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        ),
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            pause_exact_stream_slice_without_snapshot(
                coordinator,
                pause_reason,
                stop,
                ExploreStreamObserverDeferral::ResourceAdmission {
                    detail: reason.code().to_string(),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
    }
}

fn publish_or_defer_and_pause_mechanism_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    deadline: Option<Instant>,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    match try_admit_exact_stream_snapshot_work(resources, deadline)? {
        ExactStreamWorkAdmission::Granted(in_flight) => {
            let mut snapshot_authority = match in_flight.into_snapshot_publication_authority() {
                Ok(authority) => authority,
                Err(in_flight) => {
                    finish_exact_stream_work(resources, in_flight)?;
                    return Err(ExploreExecutionPreparationError::Execution(
                        "admitted mechanism work unit did not carry snapshot-publication authority"
                            .to_string(),
                    ));
                }
            };
            let prepared_checkpoint = match coordinator
                .prepare_mechanism_checkpoint_publication(&mut snapshot_authority)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
                    return Err(ExploreExecutionPreparationError::Execution(format!(
                        "cannot prepare mechanism-stream checkpoint publication: {error}"
                    )));
                }
            };
            let publication = publish_prepared_mechanism_checkpoint_and_pause_stream_slice(
                coordinator,
                prepared_checkpoint,
                pause_reason,
                stop,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
            finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
            publication
        }
        ExactStreamWorkAdmission::TimeLimit => pause_exact_stream_slice_without_snapshot(
            coordinator,
            pause_reason,
            stop,
            ExploreStreamObserverDeferral::TimeLimit,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        ),
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            pause_exact_stream_slice_without_snapshot(
                coordinator,
                pause_reason,
                stop,
                ExploreStreamObserverDeferral::ResourceAdmission {
                    detail: reason.code().to_string(),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
    }
}

fn render_exact_stream_terminal(
    coordinator: &stream_coordinator::ExactStreamCoordinator<'_>,
    stop: ExploreStreamSliceStop,
    terminal_result_json: Vec<u8>,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let probe_milestone_complete = coordinator
        .probe_progress()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive terminal source-probe progress: {error}"
            ))
        })?
        .complete();
    let closed_cases_this_slice = coordinator
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one terminal invocation".to_string(),
            )
        })?;
    let terminal_blob_digest = coordinator
        .published_terminal_result()
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "terminal artifact has no durable publication receipt".to_string(),
            )
        })?
        .blob_digest()
        .to_lowercase_hex();
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(coordinator.stream().cursor()),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact: ExploreStreamArtifact::TerminalResultJson {
            canonical_json: terminal_result_json,
            blob_digest: terminal_blob_digest,
        },
    })
}

fn render_already_sealed_exact_stream(
    coordinator: &stream_coordinator::ExactStreamCoordinator<'_>,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let status = match coordinator
        .stream()
        .terminal_seal()
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "sealed exact stream is missing its terminal commitment".to_string(),
            )
        })?
        .kind()
    {
        run_stream::TerminalSealKind::Completed => ExploreStreamTerminalStatus::Completed,
        run_stream::TerminalSealKind::Partial => ExploreStreamTerminalStatus::Partial,
        run_stream::TerminalSealKind::Unknown => ExploreStreamTerminalStatus::Unknown,
        run_stream::TerminalSealKind::Unsupported => ExploreStreamTerminalStatus::Unsupported,
        run_stream::TerminalSealKind::Error => ExploreStreamTerminalStatus::Error,
        run_stream::TerminalSealKind::Cancelled => ExploreStreamTerminalStatus::Cancelled,
    };
    let terminal_result_json =
        coordinator
            .read_verified_terminal_result_bytes()
            .map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot read verified terminal artifact from sealed exact Explore run: {error}"
                ))
            })?;
    render_exact_stream_terminal(
        coordinator,
        ExploreStreamSliceStop::AlreadySealed(status),
        terminal_result_json,
        0,
        closed_cases_at_slice_start,
    )
}

enum ExactStreamFinalizationAttempt {
    Sealed(Vec<u8>),
    WitnessOpen {
        rank: u128,
        reason: report::ExploreStopReason,
    },
    LimitReached {
        phase: &'static str,
        detail: String,
    },
}

/// Run the semantic portion of the atomic-v1 finalizer. Production callers
/// must hold the admitted `FinalizationPhase` work unit around this call; the
/// cardinality-one lifecycle test invokes it directly to avoid live telemetry.
fn attempt_atomic_exact_stream_finalization(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    case_graph_publication: &stream_coordinator::PreparedExactCaseGraphPublication,
) -> Result<ExactStreamFinalizationAttempt, ExploreExecutionPreparationError> {
    match coordinator.close_replay_obligation().map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "cannot close exact terminal replay obligation: {error}"
        ))
    })? {
        stream_coordinator::ExactReplayClosureAdvance::AlreadyClosed
        | stream_coordinator::ExactReplayClosureAdvance::Closed { .. } => {}
        stream_coordinator::ExactReplayClosureAdvance::WitnessOpen { rank, reason } => {
            return Ok(ExactStreamFinalizationAttempt::WitnessOpen { rank, reason });
        }
        stream_coordinator::ExactReplayClosureAdvance::LimitReached { detail } => {
            return Ok(ExactStreamFinalizationAttempt::LimitReached {
                phase: "witness_replay",
                detail,
            });
        }
    }

    let receipt = match coordinator.published_terminal_result() {
        Some(receipt) => receipt,
        None => match coordinator
            .publish_current_terminal_result(case_graph_publication)
            .map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot publish exact terminal result: {error}"
                ))
            })? {
            stream_coordinator::ExactTerminalPublicationAdvanceV1::Published(receipt) => receipt,
            stream_coordinator::ExactTerminalPublicationAdvanceV1::LimitReached {
                phase,
                detail,
            } => {
                return Ok(ExactStreamFinalizationAttempt::LimitReached { phase, detail });
            }
        },
    };
    coordinator
        .seal_completed_exact_exhaustion(receipt)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot seal completed exact exploration: {error}"
            ))
        })?;
    let bytes = coordinator
        .read_verified_terminal_result_bytes()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot read back sealed exact terminal result: {error}"
            ))
        })?;
    Ok(ExactStreamFinalizationAttempt::Sealed(bytes))
}

/// Handle the exact point where CaseId classification is closed.
///
/// Without explicit opt-in this remains a cheap durable pause. With opt-in,
/// the existing v1 finalizer is admitted as one atomic resource work unit:
/// at most 65,536 freshly replayed witnesses and 32 MiB of retained replay
/// bodies, followed by one full terminal JSON blob capped by its renderer. It
/// is not a resumable inner loop. The process supervisor may interrupt it;
/// replay then retries from the last committed replay-closure, publication, or
/// seal event.
fn finalize_or_pause_classification_closed_stream(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    query: &ExploreQueryIr,
    finalize: bool,
    deadline: Option<Instant>,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if !finalize {
        return publish_or_defer_and_pause_exact_stream_slice(
            coordinator,
            resources,
            query,
            deadline,
            run_stream::PauseReason::FinalizationPending,
            ExploreStreamSliceStop::ClassificationClosedFinalizationPending,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    let work_subject = stream_resource::ExactStreamWorkSubject::FinalizationPhase;
    let in_flight = match admit_exact_stream_work(resources, work_subject, deadline)? {
        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
        ExactStreamWorkAdmission::TimeLimit => {
            return publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::TimeLimit,
                ExploreStreamSliceStop::TimeLimit,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            return publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::ResourcePressure,
                ExploreStreamSliceStop::ResourcePressure {
                    detail: reason.code().to_string(),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }
    };
    if in_flight.subject() != work_subject {
        return Err(ExploreExecutionPreparationError::Execution(
            "resource governor admitted another work unit instead of terminal finalization"
                .to_string(),
        ));
    }

    // Atomic-v1 may only clone/finalize reducer state that already fits the
    // identity-bound observable snapshot envelope. Larger exact answers remain
    // valid at the finalization frontier for a future chunked publisher.
    let atomic_snapshot = coordinator.exact_snapshot();
    if !atomic_snapshot.result_group_scan_complete {
        let detail = format!(
            "{} observed raw groups do not fit the bounded atomic snapshot envelope",
            atomic_snapshot.observed_result_group_count
        );
        finish_exact_stream_work(resources, in_flight)?;
        return publish_or_defer_and_pause_exact_stream_slice(
            coordinator,
            resources,
            query,
            deadline,
            run_stream::PauseReason::FinalizationPending,
            ExploreStreamSliceStop::FinalizationLimit {
                phase: "result_snapshot".to_string(),
                detail,
            },
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }
    let case_graph_publication = match coordinator.prepare_case_graph_publication() {
        Ok(publication) => publication,
        Err(error) => {
            finish_exact_stream_work(resources, in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "cannot prepare final case-graph publication: {error}"
            )));
        }
    };
    let capacity_status = match stream_snapshot::exact_case_graph_capacity_status_v1(
        case_graph_publication.publication(),
        &atomic_snapshot,
    ) {
        Ok(status) => status,
        Err(error) => {
            finish_exact_stream_work(resources, in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "cannot validate final case-graph publication: {error}"
            )));
        }
    };
    if let Some((resource, maximum, required_at_least)) = capacity_status {
        let detail = format!(
            "requested complete case graph requires at least {required_at_least} {}, exceeding the fixed maximum {maximum}",
            resource.name()
        );
        finish_exact_stream_work(resources, in_flight)?;
        return publish_or_defer_and_pause_exact_stream_slice(
            coordinator,
            resources,
            query,
            deadline,
            run_stream::PauseReason::FinalizationPending,
            ExploreStreamSliceStop::FinalizationLimit {
                phase: "case_graph_publication".to_string(),
                detail,
            },
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }
    drop(atomic_snapshot);

    let attempt = attempt_atomic_exact_stream_finalization(coordinator, &case_graph_publication);
    finish_exact_stream_work(resources, in_flight)?;

    match attempt? {
        ExactStreamFinalizationAttempt::Sealed(bytes) => render_exact_stream_terminal(
            coordinator,
            ExploreStreamSliceStop::TerminalSealed(ExploreStreamTerminalStatus::Completed),
            bytes,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        ),
        ExactStreamFinalizationAttempt::WitnessOpen { rank, reason } => {
            publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::EvaluationLimit,
                ExploreStreamSliceStop::EvaluationLimit {
                    blocked_rank: rank,
                    reason: public_stop(reason),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
        ExactStreamFinalizationAttempt::LimitReached { phase, detail } => {
            publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::FinalizationPending,
                ExploreStreamSliceStop::FinalizationLimit {
                    phase: phase.to_string(),
                    detail,
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MechanismStreamWorkV1 {
    ReplayConfirmedMechanism { rank: u128 },
    ClassifyCase { rank: u128 },
    ClassificationAndMechanismClosed,
}

fn fixed_mechanism_limit_stop_v1(
    blocked_rank: u128,
    detail: String,
) -> (run_stream::PauseReason, ExploreStreamSliceStop) {
    (
        run_stream::PauseReason::StorageLimit,
        ExploreStreamSliceStop::MechanismLimit {
            blocked_rank,
            detail,
        },
    )
}

/// Choose one atomic semantic work unit without mutating the stream. Confirmed
/// mechanism incidence always wins over further classification so a bounded
/// invocation exposes newly discovered signatures promptly and never grows an
/// avoidable replay backlog.
fn next_mechanism_stream_work_v1(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
) -> Result<MechanismStreamWorkV1, ExploreExecutionPreparationError> {
    if let Some(rank) = coordinator.next_mechanism_rank_hint().map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "cannot select confirmed mechanism replay work: {error}"
        ))
    })? {
        return Ok(MechanismStreamWorkV1::ReplayConfirmedMechanism { rank });
    }
    Ok(match coordinator.next_open_rank_hint() {
        Some(rank) => MechanismStreamWorkV1::ClassifyCase { rank },
        None => MechanismStreamWorkV1::ClassificationAndMechanismClosed,
    })
}

#[allow(clippy::too_many_arguments)]
fn advance_mechanism_stream_slice_v1(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    query: &ExploreQueryIr,
    plan: &CheckedMechanismRuntimePlanV1,
    deadline: Option<Instant>,
    mut singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if !coordinator.probe_phase_complete() {
        return Err(ExploreExecutionPreparationError::Execution(
            "mechanism scheduler cannot precede the completed source-probe milestone".to_string(),
        ));
    }

    loop {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = resources.stop_at_work_boundary();
            return publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::TimeLimit,
                ExploreStreamSliceStop::TimeLimit,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }

        match next_mechanism_stream_work_v1(coordinator)? {
            MechanismStreamWorkV1::ReplayConfirmedMechanism { rank } => {
                let work_subject =
                    stream_resource::ExactStreamWorkSubject::MechanismCaseIdRank(rank);
                let in_flight = match admit_exact_stream_work(resources, work_subject, deadline)? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                if in_flight.subject() != work_subject || in_flight.case_id_rank() != Some(rank) {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor began another mechanism CaseId than the coordinator scheduled"
                            .to_string(),
                    ));
                }
                let advance = match plan {
                    CheckedMechanismRuntimePlanV1::NestedIf(plan) => {
                        coordinator.advance_one_nested_if_mechanism_case(plan)
                    }
                    CheckedMechanismRuntimePlanV1::RuleDispatch(plan) => {
                        coordinator.advance_one_rule_dispatch_mechanism_case(plan)
                    }
                };
                finish_exact_stream_work(resources, in_flight)?;
                let advance = match advance {
                    Ok(advance) => advance,
                    Err(error) if error.is_mechanism_fixed_capacity() => {
                        let (pause_reason, stop) =
                            fixed_mechanism_limit_stop_v1(rank, error.to_string());
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            pause_reason,
                            stop,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    Err(error) => {
                        return Err(ExploreExecutionPreparationError::Execution(format!(
                            "cannot advance durable mechanism evidence: {error}"
                        )));
                    }
                };
                match advance {
                    stream_coordinator::MechanismStreamAdvanceV1::Committed {
                        rank: committed_rank,
                        canonical_blob_bytes,
                    } => {
                        if committed_rank != rank || canonical_blob_bytes == 0 {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "resource-bound mechanism replay returned inconsistent committed evidence"
                                    .to_string(),
                            ));
                        }
                    }
                    stream_coordinator::MechanismStreamAdvanceV1::NoConfirmedTargetBacklog => {
                        return Err(ExploreExecutionPreparationError::Execution(
                            "confirmed mechanism backlog disappeared after its ranked dispatch"
                                .to_string(),
                        ));
                    }
                    stream_coordinator::MechanismStreamAdvanceV1::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if open_rank != rank {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "mechanism evaluator blocked another rank than the dispatched CaseId"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                }
            }
            MechanismStreamWorkV1::ClassifyCase { rank } => {
                let work_subject = stream_resource::ExactStreamWorkSubject::CaseIdRank(rank);
                let in_flight = match admit_exact_stream_work(resources, work_subject, deadline)? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                if in_flight.subject() != work_subject || in_flight.case_id_rank() != Some(rank) {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor began another classification CaseId than the mechanism scheduler requested"
                            .to_string(),
                    ));
                }
                let closed_cases_before = coordinator.closed_case_count();
                let advance = coordinator.advance_one_case();
                finish_exact_stream_work(resources, in_flight)?;
                match advance.map_err(|error| {
                    ExploreExecutionPreparationError::Execution(format!(
                        "cannot advance durable mechanism-target classification: {error}"
                    ))
                })? {
                    stream_coordinator::ExactStreamAdvance::Committed {
                        rank: committed_rank,
                        closed_case_count,
                    } => {
                        let expected_closed_case_count =
                            closed_cases_before.checked_add(1).ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "mechanism-stream closed case count exceeds u128::MAX"
                                        .to_string(),
                                )
                            })?;
                        if committed_rank != rank
                            || closed_case_count != expected_closed_case_count
                            || closed_case_count != coordinator.closed_case_count()
                        {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "resource-bound mechanism target returned inconsistent classification evidence"
                                    .to_string(),
                            ));
                        }
                        singleton_cases_evaluated_this_slice =
                            singleton_cases_evaluated_this_slice.checked_add(1).ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "mechanism-stream evaluated case count exceeds u128::MAX"
                                        .to_string(),
                                )
                            })?;
                    }
                    stream_coordinator::ExactStreamAdvance::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if open_rank != rank {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "mechanism target evaluator blocked another rank than the dispatched CaseId"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            coordinator,
                            resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    stream_coordinator::ExactStreamAdvance::ClassificationClosedFinalizationPending => {
                        return Err(ExploreExecutionPreparationError::Execution(
                            "mechanism scheduler dispatched a CaseId after classification had closed"
                                .to_string(),
                        ));
                    }
                }
            }
            MechanismStreamWorkV1::ClassificationAndMechanismClosed => {
                return publish_or_defer_and_pause_exact_stream_slice(
                    coordinator,
                    resources,
                    query,
                    deadline,
                    run_stream::PauseReason::FinalizationPending,
                    ExploreStreamSliceStop::MechanismObservationClosedTerminalUnavailable,
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
        }
    }
}

/// Check, open or resume, and advance one bounded durable exact Explore slice.
///
/// Terminal witness replay remains opt-in because its first-generation
/// manifest is one bounded-but-atomic work unit. Without `finalize`, closed
/// classification pauses at the explicit finalization frontier. A hard process
/// kill may omit the final pause or terminal record but cannot make an
/// uncommitted CaseId, replay closure, publication, or seal disappear from the
/// recovered durable state.
pub fn execute_checked_exact_stream_slice(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreStreamSliceOptions,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    execute_checked_stream_slice_v1(
        statements,
        source_dir,
        source,
        query_name,
        options,
        CheckedStreamExecutionProfileV1::ExactOnly,
        None,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckedStreamExecutionProfileV1 {
    ExactOnly,
    NestedIfMechanism {
        before_show_index: usize,
        after_show_index: usize,
    },
    RuleDispatchMechanism {
        before_show_index: usize,
        after_show_index: usize,
    },
}

enum CheckedMechanismRuntimePlanV1 {
    NestedIf(mechanism_runtime::CheckedNestedIfMechanismRuntimePlanV1),
    RuleDispatch(mechanism_runtime::CheckedRuleDispatchMechanismRuntimePlanV1),
}

impl CheckedMechanismRuntimePlanV1 {
    fn request(&self) -> &mechanism::CheckedMechanismObservationRequestV1 {
        match self {
            Self::NestedIf(plan) => plan.request(),
            Self::RuleDispatch(plan) => plan.request(),
        }
    }
}

/// Execute one bounded slice of the Experimental positional nested-`if`
/// mechanism profile.
///
/// This V1 API is intentionally narrow while the mechanism-DAG result schema
/// and source syntax are designed. The two indexes select distinct checked
/// `output.show` positions; the durable count checkpoint is observable and
/// resumable through the stream report. Callers must treat this surface as
/// Experimental under Futuruna's compatibility policy.
#[allow(clippy::too_many_arguments)]
pub fn execute_checked_nested_if_mechanism_stream_slice_v1(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    before_show_index: usize,
    after_show_index: usize,
    options: ExploreStreamSliceOptions,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if options.finalize {
        return Err(ExploreExecutionPreparationError::Execution(
            "mechanism-stream terminal finalization is not implemented".to_string(),
        ));
    }
    if options.case_graph != ExploreStreamCaseGraphRequest::Omit {
        return Err(ExploreExecutionPreparationError::Execution(
            "mechanism-stream execution currently requires omitted public case-graph disclosure"
                .to_string(),
        ));
    }
    execute_checked_stream_slice_v1(
        statements,
        source_dir,
        source,
        query_name,
        options,
        CheckedStreamExecutionProfileV1::NestedIfMechanism {
            before_show_index,
            after_show_index,
        },
        None,
    )
}

/// Execute one bounded slice of the Experimental direct rule-dispatch
/// mechanism profile.
///
/// Paired checked `output.show` positions must call the same global rule
/// family directly. The canonical interpreter records reached candidate
/// outcomes and the selected rule; durable publication remains count-only in
/// this first surface.
#[allow(clippy::too_many_arguments)]
pub fn execute_checked_rule_dispatch_mechanism_stream_slice_v1(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    before_show_index: usize,
    after_show_index: usize,
    options: ExploreStreamSliceOptions,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if options.finalize {
        return Err(ExploreExecutionPreparationError::Execution(
            "mechanism-stream terminal finalization is not implemented".to_string(),
        ));
    }
    if options.case_graph != ExploreStreamCaseGraphRequest::Omit {
        return Err(ExploreExecutionPreparationError::Execution(
            "mechanism-stream execution currently requires omitted public case-graph disclosure"
                .to_string(),
        ));
    }
    execute_checked_stream_slice_v1(
        statements,
        source_dir,
        source,
        query_name,
        options,
        CheckedStreamExecutionProfileV1::RuleDispatchMechanism {
            before_show_index,
            after_show_index,
        },
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_checked_stream_slice_v1(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreStreamSliceOptions,
    profile: CheckedStreamExecutionProfileV1,
    resources: Option<stream_resource::ExactStreamOneWorkerEnvelope>,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if options.max_runtime.is_some_and(|runtime| runtime.is_zero()) {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream max_runtime must be positive".to_string(),
        ));
    }
    if options.max_runtime.is_none() && options.pause_after.is_none() {
        return Err(ExploreExecutionPreparationError::Execution(
            "a first-generation exact stream slice requires max_runtime or pause_after".to_string(),
        ));
    }
    if options.finalize && options.pause_after.is_some() {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream finalize cannot be combined with pause_after".to_string(),
        ));
    }
    if options.run_state.as_os_str().is_empty() {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream run_state path must not be empty".to_string(),
        ));
    }

    let started = Instant::now();
    let deadline = match options.max_runtime {
        Some(runtime) => Some(started.checked_add(runtime).ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream runtime deadline exceeds the monotonic clock".to_string(),
            )
        })?),
        None => None,
    };
    let mut resources = match resources {
        Some(resources) => resources,
        None => stream_resource::ExactStreamOneWorkerEnvelope::new().map_err(|reason| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot initialize exact-stream resource governor: {}",
                reason.code()
            ))
        })?,
    };
    let preparation_in_flight = match admit_exact_stream_work(
        &mut resources,
        stream_resource::ExactStreamWorkSubject::PreparationPhase,
        deadline,
    )? {
        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
        ExactStreamWorkAdmission::TimeLimit => {
            return Err(ExploreExecutionPreparationError::Execution(
                "exact-stream time limit elapsed before checked preparation could be admitted; no run-state transition was made"
                    .to_string(),
            ))
        }
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "exact-stream checked preparation was not admitted under the host resource envelope: {}",
                reason.code()
            )))
        }
    };
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir.clone(), source);
    if !artifacts.diagnostics.is_empty() {
        finish_exact_stream_work(&mut resources, preparation_in_flight)?;
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }
    let selected = match select_checked_exact_query_index(&artifacts, query_name) {
        Ok(selected) => selected,
        Err(error) => {
            finish_exact_stream_work(&mut resources, preparation_in_flight)?;
            return Err(error);
        }
    };
    let mechanism_plan = match profile {
        CheckedStreamExecutionProfileV1::ExactOnly => None,
        CheckedStreamExecutionProfileV1::NestedIfMechanism {
            before_show_index,
            after_show_index,
        } => {
            let plan =
                match mechanism_runtime::CheckedNestedIfMechanismRuntimePlanV1::from_show_call_roots(
                    &artifacts,
                    selected,
                    before_show_index,
                    after_show_index,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        finish_exact_stream_work(&mut resources, preparation_in_flight)?;
                        return Err(ExploreExecutionPreparationError::Execution(format!(
                            "cannot prepare checked nested mechanism stream: {error}"
                        )));
                    }
                };
            Some(CheckedMechanismRuntimePlanV1::NestedIf(plan))
        }
        CheckedStreamExecutionProfileV1::RuleDispatchMechanism {
            before_show_index,
            after_show_index,
        } => {
            let plan = match mechanism_runtime::CheckedRuleDispatchMechanismRuntimePlanV1::from_show_call_roots(
                &artifacts,
                selected,
                before_show_index,
                after_show_index,
            ) {
                Ok(plan) => plan,
                Err(error) => {
                    finish_exact_stream_work(&mut resources, preparation_in_flight)?;
                    return Err(ExploreExecutionPreparationError::Execution(format!(
                        "cannot prepare checked rule-dispatch mechanism stream: {error}"
                    )));
                }
            };
            Some(CheckedMechanismRuntimePlanV1::RuleDispatch(plan))
        }
    };
    let query = &artifacts.exploration_universes[selected];
    let report_request = options.case_graph.report_request();
    let coordinator_result = match mechanism_plan.as_ref() {
        Some(plan) => stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
            &options.run_state,
            run_store::RunStoreLimits::default(),
            statements,
            source_dir.as_deref(),
            &artifacts,
            selected,
            report_request,
            plan.request().clone(),
        ),
        None => stream_coordinator::ExactStreamCoordinator::open_or_create(
            &options.run_state,
            run_store::RunStoreLimits::default(),
            statements,
            source_dir.as_deref(),
            &artifacts,
            selected,
            report_request,
        ),
    };
    let mut coordinator = match coordinator_result {
        Ok(coordinator) => coordinator,
        Err(error) => {
            finish_exact_stream_work(&mut resources, preparation_in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "cannot open durable exact Explore stream: {error}"
            )));
        }
    };
    let closed_cases_at_slice_start = coordinator.closed_case_count();

    if coordinator.stream().lifecycle() == run_stream::RunLifecycle::Sealed {
        if mechanism_plan.is_some() {
            finish_exact_stream_work(&mut resources, preparation_in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(
                "mechanism-enabled stream unexpectedly recovered a terminal seal".to_string(),
            ));
        }
        let report = render_already_sealed_exact_stream(&coordinator, closed_cases_at_slice_start);
        finish_exact_stream_work(&mut resources, preparation_in_flight)?;
        return report;
    }
    let pending_observable_snapshot_on_resume = coordinator.pending_observable_snapshot_on_resume();
    finish_exact_stream_work(&mut resources, preparation_in_flight)?;

    // The journal is already the resume checkpoint, but a time-boxed slice may
    // have ended without enough admitted tail to mint its observer view. Give
    // that view first claim on the next invocation so repeated deadlines cannot
    // indefinitely hide otherwise durable progress.
    if pending_observable_snapshot_on_resume {
        let catch_up_stop = if mechanism_plan.is_some() {
            ExploreStreamSliceStop::MechanismCheckpointCatchUp
        } else {
            ExploreStreamSliceStop::SnapshotCatchUp
        };
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::Explicit,
            catch_up_stop,
            0,
            closed_cases_at_slice_start,
        );
    }

    let mut singleton_cases_evaluated_this_slice = 0_u128;

    let probe_case_batch_cap =
        NonZeroU16::new(stream_coordinator::EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
            .expect("the first-generation source-probe batch cap is positive");
    while !coordinator.probe_phase_complete() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = resources.stop_at_work_boundary();
            return publish_or_defer_and_pause_exact_stream_slice(
                &mut coordinator,
                &mut resources,
                query,
                deadline,
                run_stream::PauseReason::TimeLimit,
                ExploreStreamSliceStop::TimeLimit,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }

        match coordinator.probe_phase().map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive source-probe phase: {error}"
            ))
        })? {
            stream_probe::ExactSourceProbePhaseV1::Unprepared => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let probe_result = match source_proof_plan::prepare_source_proof_plan(
                    &artifacts,
                    selected,
                    source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
                ) {
                    Ok(plan) => coordinator
                        .persist_source_probe_manifest(&plan)
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    Err(error) if error.permits_canonical_fallback() => coordinator
                        .persist_probe_fallback_manifest()
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                finish_exact_stream_work(&mut resources, in_flight)?;
                probe_result.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::Prepared => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let coverage = coordinator
                    .accept_prepared_probe_coverage(NonZeroU64::new(1).expect("one is nonzero"))
                    .map_err(|error| error.to_string());
                finish_exact_stream_work(&mut resources, in_flight)?;
                coverage.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::CoverageAccepted => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let completion = coordinator
                    .complete_prepared_probe()
                    .map_err(|error| error.to_string());
                finish_exact_stream_work(&mut resources, in_flight)?;
                completion.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::CandidateActive => {
                let rank = coordinator
                    .next_probe_candidate_rank_hint()
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "active source-probe phase has no still-open discovered candidate"
                                .to_string(),
                        )
                    })?;
                let work_subject = stream_resource::ExactStreamWorkSubject::ProbeCandidateBatch {
                    first_rank: rank,
                    case_cap: probe_case_batch_cap,
                };
                let in_flight =
                    match admit_exact_stream_work(&mut resources, work_subject, deadline)? {
                        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                        ExactStreamWorkAdmission::TimeLimit => {
                            return publish_or_defer_and_pause_exact_stream_slice(
                                &mut coordinator,
                                &mut resources,
                                query,
                                deadline,
                                run_stream::PauseReason::TimeLimit,
                                ExploreStreamSliceStop::TimeLimit,
                                singleton_cases_evaluated_this_slice,
                                closed_cases_at_slice_start,
                            );
                        }
                        ExactStreamWorkAdmission::ResourcePause(reason) => {
                            return publish_or_defer_and_pause_exact_stream_slice(
                                &mut coordinator,
                                &mut resources,
                                query,
                                deadline,
                                run_stream::PauseReason::ResourcePressure,
                                ExploreStreamSliceStop::ResourcePressure {
                                    detail: reason.code().to_string(),
                                },
                                singleton_cases_evaluated_this_slice,
                                closed_cases_at_slice_start,
                            );
                        }
                    };
                if in_flight.subject() != work_subject
                    || in_flight.first_case_id_rank() != Some(rank)
                {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor admitted another source-probe candidate block"
                            .to_string(),
                    ));
                }
                let closed_cases_before_batch = coordinator.closed_case_count();
                let advance =
                    coordinator.advance_bounded_probe_candidate_batch(probe_case_batch_cap);
                finish_exact_stream_work(&mut resources, in_flight)?;
                match advance.map_err(|error| {
                    ExploreExecutionPreparationError::Execution(format!(
                        "cannot advance durable source-probe candidate block: {error}"
                    ))
                })? {
                    stream_coordinator::ExactProbeCandidateBatchAdvance::CandidatesComplete => {
                        continue;
                    }
                    stream_coordinator::ExactProbeCandidateBatchAdvance::Committed {
                        ranks,
                        canonical_blob_bytes,
                        closed_case_count,
                        stop,
                    } => {
                        let expected_closed_case_count = closed_cases_before_batch
                            .checked_add(ranks.len() as u128)
                            .ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "source-probe closed case count exceeds u128::MAX".to_string(),
                                )
                            })?;
                        if ranks.is_empty()
                            || canonical_blob_bytes == 0
                            || !ranks.contains(&rank)
                            || closed_case_count != expected_closed_case_count
                            || closed_case_count != coordinator.closed_case_count()
                        {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "source-probe candidate block returned inconsistent evidence"
                                    .to_string(),
                            ));
                        }
                        singleton_cases_evaluated_this_slice = singleton_cases_evaluated_this_slice
                            .checked_add(ranks.len() as u128)
                            .ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "source-probe evaluated case count exceeds u128::MAX"
                                        .to_string(),
                                )
                            })?;
                        match stop {
                            stream_coordinator::ExactProbeCandidateBatchStop::CaseCapReached {
                                next_rank,
                            }
                            | stream_coordinator::ExactProbeCandidateBatchStop::ByteTargetReached {
                                next_rank,
                            } => {
                                if ranks.contains(&next_rank) {
                                    return Err(ExploreExecutionPreparationError::Execution(
                                        "source-probe block reports a committed rank as its next candidate"
                                            .to_string(),
                                    ));
                                }
                            }
                            stream_coordinator::ExactProbeCandidateBatchStop::CandidatesComplete => {
                            }
                            stream_coordinator::ExactProbeCandidateBatchStop::CaseOpen {
                                rank: open_rank,
                                reason,
                            } => {
                                if ranks.contains(&open_rank) {
                                    return Err(ExploreExecutionPreparationError::Execution(
                                        "source-probe block reports its limited candidate as committed"
                                            .to_string(),
                                    ));
                                }
                                return publish_or_defer_and_pause_exact_stream_slice(
                                    &mut coordinator,
                                    &mut resources,
                                    query,
                                    deadline,
                                    run_stream::PauseReason::EvaluationLimit,
                                    ExploreStreamSliceStop::EvaluationLimit {
                                        blocked_rank: open_rank,
                                        reason: public_stop(reason),
                                    },
                                    singleton_cases_evaluated_this_slice,
                                    closed_cases_at_slice_start,
                                );
                            }
                        }
                    }
                    stream_coordinator::ExactProbeCandidateBatchAdvance::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if open_rank != rank {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "source-probe evaluator blocked another rank than dispatched"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                }
            }
            stream_probe::ExactSourceProbePhaseV1::Complete => break,
        }
    }

    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        let _ = resources.stop_at_work_boundary();
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::TimeLimit,
            ExploreStreamSliceStop::TimeLimit,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    if options.pause_after == Some(ExploreStreamPauseAfter::Probes) {
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::ProbeMilestone,
            ExploreStreamSliceStop::ProbeMilestone,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    if let Some(plan) = mechanism_plan.as_ref() {
        return advance_mechanism_stream_slice_v1(
            &mut coordinator,
            &mut resources,
            query,
            plan,
            deadline,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    let case_batch_cap =
        NonZeroU16::new(stream_coordinator::EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
            .expect("the first-generation exact-stream batch cap is positive");
    loop {
        let Some(rank) = coordinator.next_open_rank_hint() else {
            return finalize_or_pause_classification_closed_stream(
                &mut coordinator,
                &mut resources,
                query,
                options.finalize,
                deadline,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        };
        let work_subject = stream_resource::ExactStreamWorkSubject::BoundedCaseIdBatch {
            first_rank: rank,
            case_cap: case_batch_cap,
        };
        let in_flight = match admit_exact_stream_work(&mut resources, work_subject, deadline)? {
            ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
            ExactStreamWorkAdmission::TimeLimit => {
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::TimeLimit,
                    ExploreStreamSliceStop::TimeLimit,
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
            ExactStreamWorkAdmission::ResourcePause(reason) => {
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::ResourcePressure,
                    ExploreStreamSliceStop::ResourcePressure {
                        detail: reason.code().to_string(),
                    },
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
        };
        if in_flight.subject() != work_subject || in_flight.first_case_id_rank() != Some(rank) {
            return Err(ExploreExecutionPreparationError::Execution(
                "resource governor began another bounded CaseId block than the coordinator scheduled"
                    .to_string(),
            ));
        }
        let closed_cases_before_batch = coordinator.closed_case_count();
        let advance = coordinator.advance_bounded_case_batch(case_batch_cap);
        finish_exact_stream_work(&mut resources, in_flight)?;
        match advance.map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot advance durable exact Explore evidence block: {error}"
            ))
        })? {
            stream_coordinator::ExactStreamBatchAdvance::Committed {
                ranks,
                canonical_blob_bytes,
                closed_case_count,
                stop,
            } => {
                let expected_closed_case_count = closed_cases_before_batch
                    .checked_add(ranks.len() as u128)
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "committed exact-stream closed case count exceeds u128::MAX"
                                .to_string(),
                        )
                    })?;
                if ranks.is_empty()
                    || canonical_blob_bytes == 0
                    || !ranks.contains(&rank)
                    || closed_case_count != expected_closed_case_count
                    || closed_case_count != coordinator.closed_case_count()
                {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource-bound CaseId block returned inconsistent committed evidence"
                            .to_string(),
                    ));
                }
                singleton_cases_evaluated_this_slice = singleton_cases_evaluated_this_slice
                    .checked_add(ranks.len() as u128)
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "committed exact-stream case count exceeds u128::MAX".to_string(),
                        )
                    })?;
                match stop {
                    stream_coordinator::ExactStreamBatchStop::CaseCapReached { next_rank }
                    | stream_coordinator::ExactStreamBatchStop::ByteTargetReached { next_rank } => {
                        if ranks.contains(&next_rank) {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "bounded exact evidence block reports a committed rank as its next open CaseId"
                                    .to_string(),
                            ));
                        }
                    }
                    stream_coordinator::ExactStreamBatchStop::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if ranks.contains(&open_rank) {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "bounded exact evidence block reports its evaluation-limited CaseId as committed"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    stream_coordinator::ExactStreamBatchStop::ClassificationClosedFinalizationPending => {
                        return finalize_or_pause_classification_closed_stream(
                            &mut coordinator,
                            &mut resources,
                            query,
                            options.finalize,
                            deadline,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                }
            }
            stream_coordinator::ExactStreamBatchAdvance::CaseOpen {
                rank: open_rank,
                reason,
            } => {
                if open_rank != rank {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource-bound CaseId disagrees with the open exact rank".to_string(),
                    ));
                }
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::EvaluationLimit,
                    ExploreStreamSliceStop::EvaluationLimit {
                        blocked_rank: open_rank,
                        reason: public_stop(reason),
                    },
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
            stream_coordinator::ExactStreamBatchAdvance::ClassificationClosedFinalizationPending => {
                return finalize_or_pause_classification_closed_stream(
                    &mut coordinator,
                    &mut resources,
                    query,
                    options.finalize,
                    deadline,
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
        }
    }
}

/// One named, canonical value in the exhaustive developer-preview ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExplorePreviewField {
    pub name: String,
    pub value: ExploreValue,
}

/// One matching complete assignment evaluated by the ordinary interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorePreviewRow {
    pub inputs: Vec<ExplorePreviewField>,
    pub key: Vec<ExplorePreviewField>,
    pub shown: Vec<ExplorePreviewField>,
}

/// Exact-finite result used only by the hidden `__explore-preview` command.
/// It is intentionally smaller than the accepted public Explore report RFC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorePreviewReport {
    pub query_name: String,
    pub polarity: ExplorePolarity,
    pub declared_assignments: u64,
    pub eligible_configurations: u64,
    pub evaluated_configurations: u64,
    pub matching_configurations: u64,
    pub distinct_keys: u64,
    pub rows: Vec<ExplorePreviewRow>,
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
    rule_dispatch_return_types: BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_return_issues: BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_boolean_miss_safe_keys: BTreeSet<RuleDispatchKey>,
    explore_rule_return_types_by_arity: BTreeMap<(String, usize), Ty>,
    explore_rule_return_issues: BTreeMap<(String, usize), String>,
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
            ExprKind::Field(receiver, field) => {
                let receiver = self.eval(receiver, None)?;
                let value = match receiver {
                    ExploreValue::Constructor {
                        positional: false,
                        fields,
                        ..
                    } => fields
                        .into_iter()
                        .find_map(|(name, value)| (name == *field).then_some(value)),
                    _ => None,
                };
                value
                    .ok_or_else(|| format!("ground exploration value has no named field `{field}`"))
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
        interpreter.install_rule_dispatch_return_metadata(
            &definitions.rule_dispatch_return_types,
            &definitions.rule_dispatch_return_issues,
            &definitions.rule_dispatch_boolean_miss_safe_keys,
        );
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

/// Materialize the checked Context projection needed by boundary-step
/// elaboration. Every referenced field must be coordinate-invariant; unrelated
/// Context axes remain outside this private projection view.
fn fixed_boundary_context(
    query: &TypedExploreQuery,
    facts: &[ExploreFactIr],
    bound_fact_indices: &BTreeMap<usize, usize>,
    required_fields: &BTreeSet<String>,
) -> Result<Option<(ExploreValue, Value)>, String> {
    let TypedExploreProductSchemaIdentity::Declared { ty } =
        &query.transition.context_schema.identity
    else {
        return Ok(None);
    };
    let Ty::Name(type_name) = ty else {
        return Err(format!(
            "explicit exploration Context schema `{ty}` is not a nominal product"
        ));
    };

    let mut fields = Vec::with_capacity(required_fields.len());
    for field in query
        .transition
        .context_schema
        .fields
        .iter()
        .filter(|field| required_fields.contains(&field.name))
    {
        let TypedExploreProductFieldBinding::Bound { bound_index } = &field.binding else {
            return Ok(None);
        };
        let Some(fact_index) = bound_fact_indices.get(bound_index) else {
            return Ok(None);
        };
        let fact = facts.get(*fact_index).ok_or_else(|| {
            format!(
                "fixed Context field `{}` references absent fact {}",
                field.name, fact_index
            )
        })?;
        let ExploreFactValue::Fixed(value) = &fact.value else {
            return Ok(None);
        };
        fields.push((field.name.clone(), value.clone()));
    }
    if fields.len() != required_fields.len() {
        return Err("boundary step references an absent Context field".to_string());
    }

    let canonical = ExploreValue::Constructor {
        type_name: type_name.clone(),
        variant: type_name.clone(),
        positional: false,
        fields,
    };
    let runtime = runtime_value_from_explore_value(&canonical);
    Ok(Some((canonical, runtime)))
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

fn replay_builtin_arity(name: &str) -> Option<usize> {
    static BUILTIN_ARITIES: OnceLock<BTreeMap<String, usize>> = OnceLock::new();
    let canonical = builtin_canonical(name);
    BUILTIN_ARITIES
        .get_or_init(|| TypeChecker::new().builtins)
        .get(canonical)
        .copied()
        // `format_f` is an interpreter-only compatibility builtin.  Keep it
        // out of the language-wide TypeChecker inventory, but include it when
        // auditing the canonical interpreter's Pipe value lookup.
        .or_else(|| (canonical == "format_f").then_some(2))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayCallableKind {
    Function,
    Rule,
    Constructor,
    Intrinsic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplayCallableIdentity {
    kind: ReplayCallableKind,
    arity: usize,
}

fn exact_source_declaration_identity(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
) -> Option<ReplayCallableIdentity> {
    let key = (name.to_string(), arity);
    let function_count = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .map(|(_, declarations)| declarations.len())
        .sum::<usize>();
    if function_count == 1 && definitions.functions.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Function,
            arity,
        });
    }
    if definitions.rule_definitions.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Rule,
            arity,
        });
    }
    if definitions.constructors.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Constructor,
            arity,
        });
    }
    None
}

fn pipe_effective_callable_identity(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
) -> Result<Option<ReplayCallableIdentity>, String> {
    let key = (name.to_string(), arity);
    let function_count = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .map(|(_, declarations)| declarations.len())
        .sum::<usize>();
    if function_count == 1 {
        return if definitions.functions.contains_key(&key) {
            Ok(Some(ReplayCallableIdentity {
                kind: ReplayCallableKind::Function,
                arity,
            }))
        } else {
            Ok(None)
        };
    }

    let constructor_arities = definitions
        .constructors
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .flat_map(|((_, declared_arity), declarations)| {
            std::iter::repeat_n(*declared_arity, declarations.len())
        })
        .collect::<Vec<_>>();
    if constructor_arities.len() > 1 {
        return Err(format!(
            "exploration replay pipe constructor `{}` has multiple runtime declarations and cannot identify one exact callable",
            name
        ));
    }
    if let Some(declared_arity) = constructor_arities.first().copied() {
        if declared_arity != arity {
            return Err(format!(
                "exploration replay pipe constructor `{}` resolves its source form at {} argument{} but executes at {} argument{}",
                name,
                declared_arity,
                if declared_arity == 1 { "" } else { "s" },
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Constructor,
            arity,
        }));
    }

    if let Some(declared_arity) = replay_builtin_arity(name) {
        if declared_arity != arity {
            return Err(format!(
                "exploration replay pipe built-in `{}` is declared for {} argument{} but receives {} argument{} at runtime",
                name,
                declared_arity,
                if declared_arity == 1 { "" } else { "s" },
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Intrinsic,
            arity,
        }));
    }
    if definitions.rule_definitions.contains_key(&key) {
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Rule,
            arity,
        }));
    }
    Ok(None)
}

fn explore_replay_pipe_call_site_issue(
    call: &RuntimeCallUse,
    definitions: &GroundDefinitions,
) -> Option<String> {
    let effective =
        match pipe_effective_callable_identity(&call.name, call.effective_arity, definitions) {
            Ok(identity) => identity,
            Err(issue) => return Some(issue),
        };

    if replay_builtin_arity(&call.name).is_some()
        && definitions
            .rule_definitions
            .contains_key(&(call.name.clone(), call.effective_arity))
    {
        return Some(format!(
            "exploration replay pipe call `{}` executes the built-in intrinsic instead of the exact rule with the same runtime name",
            call.name
        ));
    }

    let Some(source_arity) = call.source_arity else {
        return None;
    };
    let Some(source) = exact_source_declaration_identity(&call.name, source_arity, definitions)
    else {
        return None;
    };
    if effective != Some(source) {
        let subject = if source.kind == ReplayCallableKind::Constructor {
            "pipe constructor"
        } else {
            "pipe call"
        };
        return Some(format!(
            "exploration replay {} `{}` resolves its source form at {} argument{} but executes at {} argument{}",
            subject,
            call.name,
            source_arity,
            if source_arity == 1 { "" } else { "s" },
            call.effective_arity,
            if call.effective_arity == 1 { "" } else { "s" }
        ));
    }
    None
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

    let exact_rule = definitions.rule_definitions.contains_key(&key);
    if exact_rule {
        if let Some(issue) = definitions.explore_rule_return_issues.get(&key) {
            visiting.remove(&key);
            return Some(format!(
                "exploration replay cannot classify reachable rule `{}({} argument{})`: {}",
                name,
                arity,
                if arity == 1 { "" } else { "s" },
                issue
            ));
        }
        if !definitions
            .explore_rule_return_types_by_arity
            .contains_key(&key)
        {
            visiting.remove(&key);
            return Some(format!(
                "exploration replay cannot classify the exact return type of reachable rule `{}({} argument{})`",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
    }
    let issue = if definitions.bindings.contains_key(name) {
        Some(if exact_rule {
            format!(
                "exploration replay rule call `{}` is shadowed by a top-level binding",
                name
            )
        } else {
            format!(
                "exploration replay call `{}` is shadowed by a top-level binding",
                name
            )
        })
    } else if definitions.unsupported_values.contains_key(name) {
        Some(format!(
            "exploration replay call `{}` is shadowed by a runtime value declaration",
            name
        ))
    } else if definitions
        .unsupported_callables
        .keys()
        .any(|(candidate, _)| candidate == name)
    {
        Some(if exact_rule {
            format!(
                "exploration replay rule call `{}` collides with an unsupported callable sharing one runtime name",
                name
            )
        } else {
            format!(
                "exploration replay call `{}` collides with an unsupported callable sharing one runtime name",
                name
            )
        })
    } else {
        None
    };
    if issue.is_some() {
        visiting.remove(&key);
        return issue;
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
        } else {
            let definition = &exact.expect("one exact helper definition")[0];
            let bound = definition
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect::<BTreeSet<_>>();
            expression_replay_callable_identity_issue(
                &definition.body,
                &bound,
                definitions,
                visiting,
                validated,
            )
        }
    } else if let Some(rules) = definitions.rule_definitions.get(&key) {
        if definitions.constructors.contains_key(&key) {
            Some(format!(
                "exploration replay constructor `{}({} argument{})` takes precedence over the rule with the same runtime signature",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ))
        } else {
            rules.iter().find_map(|rule| {
                let bound = ground_rule_bound_names(rule);
                ground_rule_expressions(rule)
                    .into_iter()
                    .find_map(|expression| {
                        expression_replay_callable_identity_issue(
                            expression,
                            &bound,
                            definitions,
                            visiting,
                            validated,
                        )
                    })
            })
        }
    } else if definitions
        .constructors
        .get(&key)
        .is_some_and(|declarations| declarations.len() > 1)
    {
        Some(format!(
            "exploration replay constructor `{}({} argument{})` has multiple visible runtime declarations",
            name,
            arity,
            if arity == 1 { "" } else { "s" }
        ))
    } else if definitions.constructors.contains_key(&key)
        && replay_builtin_arity(name) == Some(arity)
    {
        Some(format!(
            "exploration replay constructor `{}({} argument{})` collides with a built-in intrinsic sharing one runtime name",
            name,
            arity,
            if arity == 1 { "" } else { "s" }
        ))
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
    bound: &BTreeSet<String>,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    validated: &mut BTreeSet<(String, usize)>,
) -> Option<String> {
    collect_scoped_runtime_calls(expression, bound)
        .into_iter()
        .find_map(|call| {
            if call.lexically_bound {
                return Some(format!(
                    "exploration replay call `{}` resolves through a lexical value instead of one exact top-level callable",
                    call.name
                ));
            }
            if matches!(call.name.as_str(), "findall" | "search") {
                return Some(format!(
                    "exploration replay runtime special form `{}({} argument{})` is not an exact replay callable",
                    call.name,
                    call.effective_arity,
                    if call.effective_arity == 1 { "" } else { "s" }
                ));
            }
            if !call.through_pipe
                && replay_builtin_arity(&call.name) == Some(call.effective_arity)
                && !definitions
                    .rule_definitions
                    .contains_key(&(call.name.clone(), call.effective_arity))
                && !definitions
                    .constructors
                    .contains_key(&(call.name.clone(), call.effective_arity))
                && definitions
                    .constructors
                    .keys()
                    .any(|(candidate, _)| candidate == &call.name)
            {
                return Some(format!(
                    "exploration replay direct built-in call `{}({} argument{})` is shadowed at runtime by a different-arity constructor with the same name",
                    call.name,
                    call.effective_arity,
                    if call.effective_arity == 1 { "" } else { "s" }
                ));
            }
            if call.through_pipe {
                if let Some(issue) = explore_replay_pipe_call_site_issue(&call, definitions) {
                    return Some(issue);
                }
            }
            explore_replay_callable_identity_issue(
                &call.name,
                call.effective_arity,
                definitions,
                visiting,
                validated,
            )
        })
}

fn validate_query_replay_callable_identities(
    query: &TypedExploreQuery,
    definitions: &GroundDefinitions,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut validated = BTreeSet::new();
    if matches!(query.rule_name.as_str(), "findall" | "search") && query.rule_arity == 2 {
        diagnostics.push(Diagnostic::error_at(
            query.span,
            format!(
                "exploration replay runtime special form `{}(2 arguments)` takes precedence over the target rule",
                query.rule_name
            ),
        ));
    } else if let Some(message) = explore_replay_callable_identity_issue(
        &query.rule_name,
        query.rule_arity,
        definitions,
        &mut BTreeSet::new(),
        &mut validated,
    ) {
        diagnostics.push(Diagnostic::error_at(query.span, message));
    }
    let query_bound = query
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .chain(query.bounds.iter().filter_map(|bound| match bound {
            TypedExploreBound::Domain {
                target: TypedExploreBoundTarget::CompactScalar,
                name,
                ..
            }
            | TypedExploreBound::Value {
                target: TypedExploreBoundTarget::CompactScalar,
                name,
                ..
            } => Some(name.clone()),
            TypedExploreBound::Domain { .. } | TypedExploreBound::Value { .. } => None,
            TypedExploreBound::Where { .. } => None,
        }))
        .chain(query.output.extrema.iter().map(|field| field.name.clone()))
        .chain(query.output.show.iter().map(|field| field.name.clone()))
        .collect::<BTreeSet<_>>();
    let mut check_expression = |expression: &Expr| {
        if let Some(message) = expression_replay_callable_identity_issue(
            expression,
            &query_bound,
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
    if let Some(boundary) = query.boundary_hint() {
        check_expression(&boundary.step);
    }
    for schema in [
        &query.transition.state_schema,
        &query.transition.context_schema,
    ] {
        for field in &schema.fields {
            if let TypedExploreProductFieldBinding::TransitionExpression { expression } =
                &field.binding
            {
                check_expression(expression);
            }
        }
    }
    for field in &query.transition.after_fields {
        if let TypedExploreAfterFieldSource::Derived { expression, .. } = &field.source {
            check_expression(expression);
        }
    }
    for field in &query.output.key {
        check_expression(&field.value);
    }
    for field in &query.output.extrema {
        check_expression(&field.value);
    }
    for field in &query.output.show {
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
    rule_dispatch_return_types: &BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_return_issues: &BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_boolean_miss_safe_keys: &BTreeSet<RuleDispatchKey>,
    explore_rule_return_types_by_arity: &BTreeMap<(String, usize), Ty>,
    explore_rule_return_issues: &BTreeMap<(String, usize), String>,
    validate_replay_callables: bool,
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
    let mut definitions = collect_ground_bindings(statements, source_dir).map_err(|errors| {
        errors
            .into_iter()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;
    definitions.rule_dispatch_return_types = rule_dispatch_return_types.clone();
    definitions.rule_dispatch_return_issues = rule_dispatch_return_issues.clone();
    definitions.rule_dispatch_boolean_miss_safe_keys = rule_dispatch_boolean_miss_safe_keys.clone();
    definitions.explore_rule_return_types_by_arity = explore_rule_return_types_by_arity.clone();
    definitions.explore_rule_return_issues = explore_rule_return_issues.clone();
    let mut universes = Vec::with_capacity(queries.len());
    let mut diagnostics = Vec::new();

    for query in queries {
        match elaborate_query(
            query,
            &catalog,
            definitions.clone(),
            validate_replay_callables,
        ) {
            Ok((universe, transition)) => universes.push(ExploreQueryIr {
                query: query.clone(),
                transition,
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
    validate_replay_callables: bool,
) -> Result<(ExploreUniverseIr, ExploreTransitionIr), Vec<Diagnostic>> {
    if validate_replay_callables {
        let replay_diagnostics = validate_query_replay_callable_identities(query, &definitions);
        if !replay_diagnostics.is_empty() {
            return Err(replay_diagnostics);
        }
    }
    let mut evaluator = ExploreGroundEvaluator::new(catalog, definitions.clone());
    let mut runtime_evaluator = ExploreRuntimeGroundEvaluator::new(&definitions);
    let all_local_names = query
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .chain(query.bounds.iter().filter_map(|bound| match bound {
            TypedExploreBound::Domain {
                target: TypedExploreBoundTarget::CompactScalar,
                name,
                ..
            }
            | TypedExploreBound::Value {
                target: TypedExploreBoundTarget::CompactScalar,
                name,
                ..
            } => Some(name.clone()),
            TypedExploreBound::Domain { .. } | TypedExploreBound::Value { .. } => None,
            TypedExploreBound::Where { .. } => None,
        }))
        .chain(query.output.extrema.iter().map(|field| field.name.clone()))
        .chain(query.output.show.iter().map(|field| field.name.clone()))
        .collect::<BTreeSet<_>>();
    let mut dimensions = Vec::new();
    let mut available_names = if query.source_syntax == ExploreTransitionSyntax::Explicit {
        query
            .inputs
            .iter()
            .map(|input| input.name.clone())
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut dimension_names = BTreeSet::new();
    let mut derived_names = BTreeSet::new();
    let mut facts = Vec::new();
    let mut bound_fact_indices = BTreeMap::new();
    let mut constraints = Vec::new();
    let mut diagnostics = Vec::new();
    let mut bound_roles = BTreeMap::new();
    for (field_index, field) in query.transition.state_schema.fields.iter().enumerate() {
        if let TypedExploreProductFieldBinding::Bound { bound_index } = &field.binding {
            if bound_roles
                .insert(
                    *bound_index,
                    (ExploreGeneratorAxisRole::Before, field_index),
                )
                .is_some()
            {
                diagnostics.push(Diagnostic::error_at(
                    field.span,
                    format!(
                        "normalized bound {} is assigned to more than one transition field",
                        bound_index
                    ),
                ));
            }
        }
    }
    for (field_index, field) in query.transition.context_schema.fields.iter().enumerate() {
        if let TypedExploreProductFieldBinding::Bound { bound_index } = &field.binding {
            if bound_roles
                .insert(
                    *bound_index,
                    (ExploreGeneratorAxisRole::Context, field_index),
                )
                .is_some()
            {
                diagnostics.push(Diagnostic::error_at(
                    field.span,
                    format!(
                        "normalized bound {} is assigned to more than one transition field",
                        bound_index
                    ),
                ));
            }
        }
    }
    for field in &query.transition.after_fields {
        if let TypedExploreAfterFieldSource::IndependentDomain { bound_index } = &field.source {
            if bound_roles
                .insert(
                    *bound_index,
                    (
                        ExploreGeneratorAxisRole::AfterIndependent,
                        field.field_index,
                    ),
                )
                .is_some()
            {
                diagnostics.push(Diagnostic::error_at(
                    field.span,
                    format!(
                        "normalized bound {} is assigned to more than one transition field",
                        bound_index
                    ),
                ));
            }
        }
    }
    for (bound_index, bound) in query.bounds.iter().enumerate() {
        let (target, span) = match bound {
            TypedExploreBound::Domain { target, span, .. }
            | TypedExploreBound::Value { target, span, .. } => (target, *span),
            TypedExploreBound::Where { .. } => continue,
        };
        let Some((expected_role, expected_field_index)) = bound_roles.get(&bound_index).copied()
        else {
            diagnostics.push(Diagnostic::error_at(
                span,
                format!(
                    "exploration bound {} is not owned by the normalized transition",
                    bound_index
                ),
            ));
            continue;
        };
        let target_matches = match target {
            TypedExploreBoundTarget::CompactScalar => {
                query.source_syntax == ExploreTransitionSyntax::FlatSugar
            }
            TypedExploreBoundTarget::BeforeField { field_index } => {
                expected_role == ExploreGeneratorAxisRole::Before
                    && *field_index == expected_field_index
            }
            TypedExploreBoundTarget::ContextField { field_index } => {
                expected_role == ExploreGeneratorAxisRole::Context
                    && *field_index == expected_field_index
            }
            TypedExploreBoundTarget::AfterIndependent { field_index } => {
                expected_role == ExploreGeneratorAxisRole::AfterIndependent
                    && *field_index == expected_field_index
            }
        };
        if !target_matches {
            diagnostics.push(Diagnostic::error_at(
                span,
                format!(
                    "exploration bound {} target disagrees with its normalized {:?} field {} role",
                    bound_index, expected_role, expected_field_index
                ),
            ));
        }
    }
    for (bound_index, bound) in query.bounds.iter().enumerate() {
        match bound {
            TypedExploreBound::Domain {
                name,
                value_ty,
                domain,
                span,
                ..
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
                        let Some((role, role_field_index)) = bound_roles.get(&bound_index).copied()
                        else {
                            diagnostics.push(Diagnostic::error_at(
                                *span,
                                format!(
                                    "exploration dimension `{name}` has no normalized transition role"
                                ),
                            ));
                            available_names.insert(name.clone());
                            continue;
                        };
                        dimension_names.insert(name.clone());
                        dimensions.push(ExploreDimensionIr {
                            bound_index,
                            name: name.clone(),
                            value_ty: value_ty.clone(),
                            domain,
                            role,
                            role_field_index,
                            span: *span,
                        });
                    }
                    Err(message) => diagnostics.push(Diagnostic::error_at(*span, message)),
                }
                available_names.insert(name.clone());
            }
            TypedExploreBound::Value {
                target,
                name,
                value_ty,
                value,
                span,
                ..
            } => {
                let Some((role, role_field_index)) = bound_roles.get(&bound_index).copied() else {
                    diagnostics.push(Diagnostic::error_at(
                        *span,
                        format!("exploration value `{name}` has no normalized transition role"),
                    ));
                    available_names.insert(name.clone());
                    continue;
                };
                if role == ExploreGeneratorAxisRole::AfterIndependent {
                    diagnostics.push(Diagnostic::error_at(
                        *span,
                        format!("independent after field `{name}` must use a finite domain"),
                    ));
                    available_names.insert(name.clone());
                    continue;
                }
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
                let depends_on_transition_role = query.source_syntax
                    == ExploreTransitionSyntax::Explicit
                    && !matches!(target, TypedExploreBoundTarget::CompactScalar)
                    && dependencies.iter().any(|dependency| {
                        query.inputs.iter().any(|input| input.name == *dependency)
                    });
                let varies = depends_on_transition_role
                    || dependencies.iter().any(|dependency| {
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
                bound_fact_indices.insert(bound_index, facts.len());
                facts.push(ExploreFactIr {
                    bound_index,
                    role,
                    role_field_index,
                    name: name.clone(),
                    value_ty: value_ty.clone(),
                    value: fact,
                    span: *span,
                });
            }
            TypedExploreBound::Where {
                predicate,
                scope,
                span,
            } => {
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
                    scope: *scope,
                    span: *span,
                });
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // CaseId coordinates are semantic generator coordinates, not parser
    // order. Keep them stable across harmless source reordering by sorting
    // each axis into the declared Context, Before, AfterIndependent product
    // order before any cardinality or membership index is derived.
    dimensions.sort_by_key(|dimension| {
        let role_order = match dimension.role {
            ExploreGeneratorAxisRole::Context => 0_u8,
            ExploreGeneratorAxisRole::Before => 1,
            ExploreGeneratorAxisRole::AfterIndependent => 2,
        };
        (
            role_order,
            dimension.role_field_index,
            dimension.bound_index,
        )
    });
    let bound_dimension_indices = dimensions
        .iter()
        .enumerate()
        .map(|(dimension_index, dimension)| (dimension.bound_index, dimension_index))
        .collect::<BTreeMap<_, _>>();

    let mut cartesian_count_before_constraints = ExploreCardinality::one();
    for dimension in &dimensions {
        cartesian_count_before_constraints =
            cartesian_count_before_constraints.multiply(dimension.domain.cardinality());
    }

    let mut transition_sensitive_names =
        if query.source_syntax == ExploreTransitionSyntax::FlatSugar {
            query
                .transition
                .after_fields
                .iter()
                .filter_map(|field| {
                    matches!(&field.source, TypedExploreAfterFieldSource::Derived { .. })
                        .then(|| field.name.clone())
                })
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
    let mut transition_recomputed_fact_indices = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let ExploreFactValue::Derived { dependencies, .. } = &fact.value else {
            continue;
        };
        if dependencies
            .iter()
            .any(|dependency| transition_sensitive_names.contains(dependency))
        {
            transition_recomputed_fact_indices.push(index);
            transition_sensitive_names.insert(fact.name.clone());
        }
    }

    let boundary_hint = query.boundary_hint().and_then(|boundary| {
        let axis_bound_index = boundary.axis_bound_index;
        let state_field_index = query
            .transition
            .state_schema
            .fields
            .iter()
            .position(|field| {
                matches!(
                    &field.binding,
                    TypedExploreProductFieldBinding::Bound { bound_index }
                        if *bound_index == axis_bound_index
                )
            });
        let Some(state_field_index) = state_field_index else {
            diagnostics.push(Diagnostic::error_at(
                boundary.span,
                "exploration boundary does not target a canonical Before field",
            ));
            return None;
        };
        let Some(axis_dimension_index) = bound_dimension_indices.get(&axis_bound_index).copied()
        else {
            diagnostics.push(Diagnostic::error_at(
                boundary.span,
                "exploration boundary does not target a finite generator dimension",
            ));
            return None;
        };
        let Some(dimension) = dimensions.get(axis_dimension_index) else {
            diagnostics.push(Diagnostic::error_at(
                boundary.span,
                "exploration boundary generator dimension is absent",
            ));
            return None;
        };
        if dimension.role != ExploreGeneratorAxisRole::Before
            || dimension.role_field_index != state_field_index
        {
            diagnostics.push(Diagnostic::error_at(
                boundary.span,
                "exploration boundary target is not the resolved Before generator axis",
            ));
            return None;
        }
        let mut step_symbol_uses = FreeSymbolUses::default();
        collect_true_free_symbol_uses(
            &boundary.step,
            &mut step_symbol_uses,
            &BTreeSet::new(),
            &BTreeMap::new(),
        );
        if step_symbol_uses.values.contains("context")
            || step_symbol_uses.calls.contains("context")
        {
            let required_context_fields = step_symbol_uses
                .member_values
                .iter()
                .filter_map(|(receiver, field)| {
                    (receiver == "context").then_some(field.clone())
                })
                .collect::<BTreeSet<_>>();
            let context_value_uses = step_symbol_uses
                .value_occurrences
                .get("context")
                .copied()
                .unwrap_or_default();
            let context_projection_uses = step_symbol_uses
                .member_value_occurrences
                .iter()
                .filter_map(|((receiver, _), count)| {
                    (receiver == "context").then_some(*count)
                })
                .sum::<usize>();
            if context_value_uses != context_projection_uses {
                diagnostics.push(Diagnostic::error_at(
                    boundary.span,
                    "exploration boundary step may reference Context only through fixed `context.FIELD` projections",
                ));
                return None;
            }
            match fixed_boundary_context(
                query,
                &facts,
                &bound_fact_indices,
                &required_context_fields,
            ) {
                Ok(Some((canonical, runtime))) => {
                    evaluator.set_local("context", canonical);
                    runtime_evaluator.set_local("context", runtime);
                }
                Ok(None) => {
                    diagnostics.push(Diagnostic::error_at(
                        boundary.span,
                        "exploration boundary step references a Context field that is not coordinate-invariant",
                    ));
                    return None;
                }
                Err(message) => {
                    diagnostics.push(Diagnostic::error_at(boundary.span, message));
                    return None;
                }
            }
        }
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
        for (dimension_index, other) in dimensions.iter().enumerate() {
            if dimension_index == axis_dimension_index {
                continue;
            }
            eligible_unconstrained_pairs =
                eligible_unconstrained_pairs.multiply(other.domain.cardinality());
        }
        Some(ExploreBoundaryIr {
            axis: dimension.name.clone(),
            axis_dimension_index,
            step,
            requires_both_endpoints_in_domain: true,
            recomputed_fact_indices: transition_recomputed_fact_indices.clone(),
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
    for field in &query.output.extrema {
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
                    "exploration extrema `{}` depends on value(s) that are not yet available: {}",
                    field.name,
                    unavailable.join(", ")
                ),
            ));
        }
    }
    output_available_names.extend(query.output.extrema.iter().map(|field| field.name.clone()));
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

    let recomputed_fact_names = transition_recomputed_fact_indices
        .iter()
        .filter_map(|fact_index| facts.get(*fact_index))
        .map(|fact| fact.name.as_str())
        .collect::<BTreeSet<_>>();
    let state_field_indices = query
        .transition
        .state_schema
        .fields
        .iter()
        .enumerate()
        .map(|(field_index, field)| (field.name.as_str(), field_index))
        .collect::<BTreeMap<_, _>>();
    let mut after_fields = Vec::with_capacity(query.transition.after_fields.len());
    for field in &query.transition.after_fields {
        let source = match &field.source {
            TypedExploreAfterFieldSource::FrameBefore { .. }
                if recomputed_fact_names.contains(field.name.as_str()) =>
            {
                let Some(ExploreFactValue::Derived {
                    expression,
                    dependencies,
                }) = facts
                    .iter()
                    .find(|fact| fact.name == field.name)
                    .map(|fact| &fact.value)
                else {
                    diagnostics.push(Diagnostic::error_at(
                        field.span,
                        format!(
                            "transition field `{}` is marked for recomputation without a derived fact",
                            field.name
                        ),
                    ));
                    continue;
                };
                ExploreAfterFieldSourceIr::Derived {
                    expression: expression.clone(),
                    environment: TypedExploreDerivedEnvironment::TransitionFrameV1,
                    after_dependencies: dependencies
                        .iter()
                        .filter(|dependency| {
                            recomputed_fact_names.contains(dependency.as_str())
                                || query.transition.after_fields.iter().any(|candidate| {
                                    candidate.name == dependency.as_str()
                                        && matches!(
                                            &candidate.source,
                                            TypedExploreAfterFieldSource::Derived { .. }
                                        )
                                })
                        })
                        .filter_map(|dependency| {
                            state_field_indices
                                .get(dependency.as_str())
                                .map(|field_index| ExploreAfterDependencyIr {
                                    field_index: *field_index,
                                    binding_name: dependency.clone(),
                                })
                        })
                        .collect(),
                }
            }
            TypedExploreAfterFieldSource::FrameBefore { before_field_index } => {
                ExploreAfterFieldSourceIr::FrameBefore {
                    before_field_index: *before_field_index,
                }
            }
            TypedExploreAfterFieldSource::Derived {
                expression,
                environment,
                after_dependencies,
                ..
            } => ExploreAfterFieldSourceIr::Derived {
                expression: expression.clone(),
                environment: *environment,
                after_dependencies: after_dependencies
                    .iter()
                    .map(|dependency| ExploreAfterDependencyIr {
                        field_index: dependency.field_index,
                        binding_name: dependency.binding_name.clone(),
                    })
                    .collect(),
            },
            TypedExploreAfterFieldSource::IndependentDomain { bound_index } => {
                let Some(dimension_index) = bound_dimension_indices.get(bound_index).copied()
                else {
                    diagnostics.push(Diagnostic::error_at(
                        field.span,
                        format!(
                            "independent after field `{}` does not name a finite generator dimension",
                            field.name
                        ),
                    ));
                    continue;
                };
                let Some(dimension) = dimensions.get(dimension_index) else {
                    diagnostics.push(Diagnostic::error_at(
                        field.span,
                        format!(
                            "independent after field `{}` references absent generator dimension {}",
                            field.name, dimension_index
                        ),
                    ));
                    continue;
                };
                if dimension.role != ExploreGeneratorAxisRole::AfterIndependent
                    || dimension.role_field_index != field.field_index
                {
                    diagnostics.push(Diagnostic::error_at(
                        field.span,
                        format!(
                            "independent after field `{}` does not own its normalized generator axis",
                            field.name
                        ),
                    ));
                    continue;
                }
                ExploreAfterFieldSourceIr::IndependentDomain { dimension_index }
            }
        };
        after_fields.push(ExploreAfterFieldIr {
            field_index: field.field_index,
            name: field.name.clone(),
            value_ty: field.value_ty.clone(),
            source,
            span: field.span,
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if let Err(message) = validate_after_construction_dag(&after_fields) {
        return Err(vec![Diagnostic::error_at(query.span, message)]);
    }

    let mut after_membership = Vec::with_capacity(query.transition.after_membership.len());
    for membership in &query.transition.after_membership {
        let Some(before_dimension_index) = bound_dimension_indices
            .get(&membership.before_bound_index)
            .copied()
        else {
            diagnostics.push(Diagnostic::error_at(
                query.span,
                format!(
                    "transition membership for after field {} does not name a finite before dimension",
                    membership.after_field_index
                ),
            ));
            continue;
        };
        let Some(boundary) = boundary_hint.as_ref().filter(|boundary| {
            boundary.axis_dimension_index == before_dimension_index
                && dimensions
                    .get(before_dimension_index)
                    .is_some_and(|dimension| {
                        dimension.role_field_index == membership.after_field_index
                    })
        }) else {
            diagnostics.push(Diagnostic::error_at(
                query.span,
                format!(
                    "transition membership for after field {} has no matching closed boundary construction",
                    membership.after_field_index
                ),
            ));
            continue;
        };
        after_membership.push(ExploreAfterMembershipIr {
            after_field_index: membership.after_field_index,
            before_dimension_index,
            preconstruction: ExploreAfterMembershipPreconstructionIr::RelativeIntStep {
                step: boundary.step,
            },
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let state_schema = close_product_schema(
        &query.transition.state_schema,
        &bound_dimension_indices,
        &bound_fact_indices,
    )
    .map_err(|message| vec![Diagnostic::error_at(query.span, message)])?;
    let context_schema = close_product_schema(
        &query.transition.context_schema,
        &bound_dimension_indices,
        &bound_fact_indices,
    )
    .map_err(|message| vec![Diagnostic::error_at(query.span, message)])?;
    let mut flat_aliases = Vec::new();
    for (bound_index, bound) in query.bounds.iter().enumerate() {
        let (target, name) = match bound {
            TypedExploreBound::Domain { target, name, .. }
            | TypedExploreBound::Value { target, name, .. } => (target, name),
            TypedExploreBound::Where { .. } => continue,
        };
        if !matches!(target, TypedExploreBoundTarget::CompactScalar) {
            continue;
        }
        let Some((generator_role, field_index)) = bound_roles.get(&bound_index).copied() else {
            return Err(vec![Diagnostic::error_at(
                query.span,
                format!("compact alias `{name}` has no normalized transition role"),
            )]);
        };
        let role = match generator_role {
            ExploreGeneratorAxisRole::Context => ExploreFlatAliasRole::Context { field_index },
            ExploreGeneratorAxisRole::Before => ExploreFlatAliasRole::State { field_index },
            ExploreGeneratorAxisRole::AfterIndependent => {
                return Err(vec![Diagnostic::error_at(
                    query.span,
                    format!("compact alias `{name}` cannot own an independent after axis"),
                )])
            }
        };
        let source = match (
            bound_dimension_indices.get(&bound_index),
            bound_fact_indices.get(&bound_index),
        ) {
            (Some(dimension_index), None) => ExploreFlatAliasSource::Dimension {
                dimension_index: *dimension_index,
            },
            (None, Some(fact_index)) => ExploreFlatAliasSource::Fact {
                fact_index: *fact_index,
            },
            _ => {
                return Err(vec![Diagnostic::error_at(
                    query.span,
                    format!("compact alias `{name}` has no unique closed value slot"),
                )])
            }
        };
        flat_aliases.push(ExploreFlatAliasIr {
            name: name.clone(),
            role,
            source,
        });
    }

    let transition = ExploreTransitionIr {
        normalization_version: query.transition.normalization_version,
        mode: query.transition.mode,
        state_schema,
        context_schema,
        after_fields,
        after_membership,
        flat_aliases,
        boundary_hint,
    };
    let universe = ExploreUniverseIr {
        dimensions,
        facts,
        constraints,
        sliced_inputs: query.sliced_inputs.clone(),
        cartesian_count_before_constraints,
    };
    Ok((universe, transition))
}

/// Render a small result through the one canonical exact evaluator. This
/// hidden command is a presentation adapter only: it owns no transition,
/// eligibility, question, or output semantics.
pub fn execute_exhaustive_preview(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    case_limit: usize,
) -> Result<ExplorePreviewReport, String> {
    if case_limit == 0 {
        return Err("exploration preview limit must be positive".to_string());
    }
    let budget = report::ExploreExecutionBudget::new(
        Some(case_limit as u128),
        report::DEFAULT_EXPLORE_STEP_LIMIT,
        report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
    )?;
    let exact = exact::execute_exact_finite(
        statements,
        source_dir,
        artifacts,
        accepted_query_index,
        report::ExploreReportRequest {
            case_graph: report::ExploreCaseGraphRequest::Omit,
            ledger: report::ExploreLedgerRequest::MatchingConfigurations,
        },
        budget,
    )?;
    let report::ExploreExactReport {
        query_name,
        polarity,
        outcome,
        ..
    } = exact;
    let evidence = match outcome {
        report::ExploreExactOutcome::Complete { evidence, .. } => evidence,
        report::ExploreExactOutcome::Partial { stop, .. } => {
            return Err(format!(
                "exploration did not complete within preview limit {case_limit}: {stop:?}"
            ))
        }
        report::ExploreExactOutcome::Unknown { reason, .. } => return Err(reason),
        report::ExploreExactOutcome::Unsupported { diagnostic } => return Err(diagnostic),
        report::ExploreExactOutcome::Error { diagnostics } => {
            return Err(diagnostics.into_vec().join("; "))
        }
    };
    let exact_u64 = |name: &str, count: report::ExploreCount| {
        count
            .exact()
            .ok_or_else(|| format!("complete exploration has non-exact {name}"))
            .and_then(|value| {
                u64::try_from(value).map_err(|_| format!("exploration {name} exceeds u64::MAX"))
            })
    };
    let declared_assignments = exact_u64(
        "declared assignment count",
        evidence.counts.declared_assignments,
    )?;
    let eligible_configurations = exact_u64(
        "admissible configuration count",
        evidence.counts.admissible_configurations,
    )?;
    let matching_configurations = exact_u64(
        "matching configuration count",
        evidence.counts.matching_configurations,
    )?;
    let distinct_keys = exact_u64(
        "distinct result-key count",
        evidence.counts.distinct_result_keys,
    )?;
    let rows = match evidence.ledger {
        report::ExploreLedgerEvidence::MatchingConfigurations { rows } => rows
            .into_vec()
            .into_iter()
            .map(|row| ExplorePreviewRow {
                inputs: evidence
                    .schema
                    .dimensions
                    .iter()
                    .map(report::ExploreReportDimension::qualified_label)
                    .zip(row.dimensions.into_vec())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
                key: evidence
                    .schema
                    .key_names
                    .iter()
                    .cloned()
                    .zip(row.key.values().iter().cloned())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
                shown: evidence
                    .schema
                    .shown_names
                    .iter()
                    .cloned()
                    .zip(row.shown.into_vec())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
            })
            .collect::<Vec<_>>(),
        report::ExploreLedgerEvidence::Omitted => {
            return Err("canonical preview execution omitted its requested matching ledger".into())
        }
    };
    let mut rows = rows;
    rows.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.inputs.cmp(&right.inputs))
    });
    Ok(ExplorePreviewReport {
        query_name,
        polarity,
        declared_assignments,
        eligible_configurations,
        evaluated_configurations: eligible_configurations,
        matching_configurations,
        distinct_keys,
        rows,
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
    fn exact_evaluator_constructs_explicit_after_fields_in_dag_order() {
        let source = r#"
# OrderedState = OrderedState(earlier: Int, later: Int)

> later(value: Int) -> Int { value + 1 }

| changed(before: OrderedState, after: OrderedState, context: ()) ->
    after.earlier > before.earlier

? explore forward_dependency {
    over changed(before, after, context)
    find matches
    bounds {
        before.earlier = 0
        before.later in range(0, 2)
    }
    transition as OrderedState context () {
        relative
        after.earlier = after.later + later(before.earlier)
        after.later = later(before.later)
    }
    output {
        key [later = before.later]
        show [earlier_after = after.earlier, later_after = after.later]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse explicit after-DAG fixture");
        let report = execute_checked_exact(
            &statements,
            None,
            source,
            Some("forward_dependency"),
            ExploreExactOptions {
                case_limit: NonZeroU128::new(2).unwrap(),
            },
        )
        .expect("execute the two-case explicit transition");
        let evidence = match report.outcome {
            ExploreExecutionOutcome::Complete { evidence, .. } => evidence,
            outcome => panic!("explicit after-DAG fixture did not close: {outcome:?}"),
        };

        assert_eq!(evidence.dimensions.len(), 1);
        assert_eq!(evidence.dimensions[0].qualified_label(), "before.later");
        assert_eq!(evidence.dimensions[0].bound_index, 1);
        assert_eq!(
            evidence.dimensions[0].role,
            ExploreGeneratorAxisRole::Before
        );
        assert_eq!(evidence.dimensions[0].role_field_index, 1);
        assert_eq!(evidence.axis_cardinalities, [2]);
        assert_eq!(
            evidence.counts,
            ExploreExecutionCounts {
                declared_assignments: ExploreCountEvidence::Exact(2),
                admissible_configurations: ExploreCountEvidence::Exact(2),
                matching_configurations: ExploreCountEvidence::Exact(2),
                distinct_result_keys: ExploreCountEvidence::Exact(2),
            }
        );
        assert_eq!(evidence.results.len(), 2);
        assert_eq!(evidence.results[0].key[0].value, ExploreValue::Int(0));
        assert_eq!(
            evidence.results[0]
                .shown
                .iter()
                .map(|field| field.value.clone())
                .collect::<Vec<_>>(),
            [ExploreValue::Int(2), ExploreValue::Int(1)]
        );
        assert_eq!(evidence.results[1].key[0].value, ExploreValue::Int(1));
        assert_eq!(
            evidence.results[1]
                .shown
                .iter()
                .map(|field| field.value.clone())
                .collect::<Vec<_>>(),
            [ExploreValue::Int(3), ExploreValue::Int(2)]
        );
    }

    #[test]
    fn explicit_boundary_context_step_excludes_overflow_before_after_construction() {
        let source = r#"
# BoundaryState = BoundaryState(income: Int)
# BoundaryContext = BoundaryContext(step: Int)

| changed(before: BoundaryState, after: BoundaryState, context: BoundaryContext) ->
    after.income > before.income under context.step > 0

? explore overflow_guard {
    over changed(before, after, context)
    find matches
    bounds {
        context.step = 1
        before.income in [9223372036854775807]
    }
    transition as BoundaryState context BoundaryContext {
        relative
        after.income = before.income + context.step
    }
    boundaries on before.income by context.step
    output {
        key [income = before.income]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse explicit boundary-overflow fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let mut closed_without_hint = artifacts.exploration_universes[0].clone();
        closed_without_hint.transition.boundary_hint = None;
        assert_eq!(
            exact::endpoint_memberships_are_structurally_eligible(
                &closed_without_hint,
                &[ExploreValue::Int(i64::MAX)],
            ),
            Ok(false),
            "canonical membership, not optimizer metadata, must close overflow"
        );
        let report = execute_checked_exact(
            &statements,
            None,
            source,
            Some("overflow_guard"),
            ExploreExactOptions {
                case_limit: NonZeroU128::new(1).unwrap(),
            },
        )
        .expect("close the one-coordinate boundary-overflow fixture");
        let evidence = match report.outcome {
            ExploreExecutionOutcome::Complete { evidence, .. } => evidence,
            outcome => panic!("boundary-overflow fixture did not close: {outcome:?}"),
        };

        assert_eq!(
            evidence.counts,
            ExploreExecutionCounts {
                declared_assignments: ExploreCountEvidence::Exact(1),
                admissible_configurations: ExploreCountEvidence::Exact(0),
                matching_configurations: ExploreCountEvidence::Exact(0),
                distinct_result_keys: ExploreCountEvidence::Exact(0),
            }
        );
        assert!(evidence.results.is_empty());
    }

    #[test]
    fn mechanism_stream_mints_checkpoints_and_replays_the_same_case_graph() {
        let source = r#"
> net_income(income: Int) -> Int {
    if income >= 200 { income - 20 } else { income }
}
| eligible(income: Int, step: Int) -> True
? explore mechanism_stream_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = net_income(income),
            after = net_income(income + step),
            loss_ore = (before - after) * 100
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse mechanism stream fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let selected = 0;
        let plan = mechanism_runtime::CheckedSingleIfMechanismRuntimePlanV1::from_trace_selection(
            &artifacts,
            selected,
            mechanism_request::MechanismTraceSelectionV1 {
                before_show_index: 0,
                after_show_index: 1,
                bin_fields: vec![mechanism_request::MechanismBinShowSelectionV1 {
                    show_index: 2,
                    bins: vec![
                        mechanism::MechanismNumericBin::new(-5_000, 0)
                            .expect("negative 50-DKK bin"),
                        mechanism::MechanismNumericBin::new(0, 5_000).expect("positive 50-DKK bin"),
                    ]
                    .into_boxed_slice(),
                }]
                .into_boxed_slice(),
                retained_examples_per_signature: 1,
            },
        )
        .expect("check single-if mechanism runtime plan with bins");
        let directory = std::env::temp_dir().join(format!(
            "futuruna_mechanism_stream_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let report_request = report::ExploreReportRequest {
            case_graph: report::ExploreCaseGraphRequest::Omit,
            ledger: report::ExploreLedgerRequest::Omit,
        };
        let mut coordinator =
            stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
                &directory,
                run_store::RunStoreLimits::default(),
                &statements,
                None,
                &artifacts,
                selected,
                report_request,
                plan.request().clone(),
            )
            .expect("create mechanism-enabled stream");
        coordinator
            .persist_probe_fallback_manifest()
            .expect("persist bounded canonical probe fallback");
        coordinator
            .accept_prepared_probe_coverage(NonZeroU64::new(1).expect("one is nonzero"))
            .expect("accept bounded probe coverage");
        assert!(coordinator
            .complete_prepared_probe()
            .expect("complete source-probe milestone")
            .complete());

        let initial_checkpoint = coordinator
            .prepare_mechanism_checkpoint_publication_for_test()
            .expect("prepare zero-evidence mechanism checkpoint");
        let initial_json = std::str::from_utf8(initial_checkpoint.canonical_json_line())
            .expect("initial mechanism checkpoint is UTF-8");
        assert!(initial_json.contains("\"status\":\"scope_open\""));
        assert!(initial_json.contains(
            "\"mechanism_signatures\":{\"certainty\":\"unknown\",\"value\":null,\"confirmed_lower_bound\":\"0\"}"
        ));
        coordinator
            .publish_prepared_mechanism_checkpoint(&initial_checkpoint)
            .expect("publish zero-evidence mechanism checkpoint");

        let mut traced_ranks = Vec::new();
        loop {
            while coordinator
                .next_mechanism_rank_hint()
                .expect("select confirmed mechanism backlog")
                .is_some()
            {
                match coordinator
                    .advance_one_single_if_mechanism_case(&plan)
                    .expect("fresh-replay one mechanism case")
                {
                    stream_coordinator::MechanismStreamAdvanceV1::Committed { rank, .. } => {
                        traced_ranks.push(rank);
                    }
                    stream_coordinator::MechanismStreamAdvanceV1::NoConfirmedTargetBacklog => {
                        panic!("mechanism backlog disappeared before its selected replay")
                    }
                    stream_coordinator::MechanismStreamAdvanceV1::CaseOpen { reason, .. } => {
                        panic!("tiny mechanism replay hit an operational limit: {reason:?}")
                    }
                }
            }
            if coordinator.next_open_rank_hint().is_none() {
                break;
            }
            assert!(matches!(
                coordinator
                    .advance_one_case()
                    .expect("classify one exact mechanism target case"),
                stream_coordinator::ExactStreamAdvance::Committed { .. }
            ));
        }
        while coordinator
            .next_mechanism_rank_hint()
            .expect("select final confirmed mechanism backlog")
            .is_some()
        {
            match coordinator
                .advance_one_single_if_mechanism_case(&plan)
                .expect("fresh-replay final mechanism case")
            {
                stream_coordinator::MechanismStreamAdvanceV1::Committed { rank, .. } => {
                    traced_ranks.push(rank)
                }
                stream_coordinator::MechanismStreamAdvanceV1::NoConfirmedTargetBacklog => {
                    panic!("final mechanism backlog disappeared before replay")
                }
                stream_coordinator::MechanismStreamAdvanceV1::CaseOpen { reason, .. } => {
                    panic!("tiny final mechanism replay hit an operational limit: {reason:?}")
                }
            }
        }

        let final_checkpoint = coordinator
            .prepare_mechanism_checkpoint_publication_for_test()
            .expect("prepare closed mechanism checkpoint");
        let final_json = final_checkpoint.canonical_json_line().to_vec();
        let final_text = std::str::from_utf8(&final_json).expect("final checkpoint is UTF-8");
        assert!(final_text.contains("\"status\":\"matching_closed\""));
        assert!(final_text.contains("\"target_cases\":{\"certainty\":\"exact\",\"value\":\"3\"}"));
        assert!(final_text.contains("\"traced_cases\":\"3\""));
        assert!(final_text.contains(
            "\"known_target_untraced\":{\"total\":\"0\",\"pending\":\"0\",\"replay_unavailable\":\"0\",\"observation_unsupported\":\"0\"}"
        ));
        assert!(final_text
            .contains("\"mechanism_signatures\":{\"certainty\":\"exact\",\"value\":\"3\"}"));
        assert!(final_text.contains(
            "\"coverage\":{\"binned_cases\":\"3\",\"outside_declared_bins_cases\":\"0\",\"unavailable_cases\":\"0\",\"replay_unavailable_cases\":\"0\",\"observation_unsupported_cases\":\"0\"}"
        ));
        assert!(final_text.contains(
            "\"lower_inclusive\":\"-5000\",\"upper_exclusive\":\"0\",\"confirmed_case_support\":\"2\",\"mechanism_count\":{\"certainty\":\"exact\",\"value\":\"2\"}"
        ));
        assert!(final_text.contains(
            "\"lower_inclusive\":\"0\",\"upper_exclusive\":\"5000\",\"confirmed_case_support\":\"1\",\"mechanism_count\":{\"certainty\":\"exact\",\"value\":\"1\"}"
        ));
        coordinator
            .publish_prepared_mechanism_checkpoint(&final_checkpoint)
            .expect("publish closed mechanism checkpoint");
        assert_eq!(traced_ranks, vec![0, 1, 2]);
        let evidence_before_recovery = coordinator
            .mechanism_snapshot()
            .expect("materialize pre-recovery mechanism evidence")
            .expect("mechanism evidence is enabled");
        drop(coordinator);

        let mut recovered =
            stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
                &directory,
                run_store::RunStoreLimits::default(),
                &statements,
                None,
                &artifacts,
                selected,
                report_request,
                plan.request().clone(),
            )
            .expect("recover mechanism-enabled stream");
        assert!(recovered
            .next_mechanism_rank_hint()
            .expect("inspect recovered mechanism backlog")
            .is_none());
        let evidence_after_recovery = recovered
            .mechanism_snapshot()
            .expect("materialize recovered mechanism evidence")
            .expect("recovered mechanism evidence is enabled");
        assert_eq!(evidence_after_recovery, evidence_before_recovery);
        drop(recovered);
        std::fs::remove_dir_all(&directory).expect("remove mechanism stream fixture directory");
    }

    #[test]
    fn fixed_mechanism_capacity_is_not_reported_as_transient_resource_pressure() {
        let (pause_reason, stop) =
            fixed_mechanism_limit_stop_v1(37, "fixed signature ceiling reached".to_string());

        assert_eq!(pause_reason, run_stream::PauseReason::StorageLimit);
        assert_eq!(
            stop,
            ExploreStreamSliceStop::MechanismLimit {
                blocked_rank: 37,
                detail: "fixed signature ceiling reached".to_string(),
            }
        );
    }

    #[test]
    fn nested_mechanism_stream_traces_actual_helper_if_and_recovers() {
        let source = r#"
> adjustment(income: Int) -> Int {
    if income >= 200 { 20 } else { 0 }
}
> net_income(income: Int) -> Int {
    income - adjustment(income)
}
| eligible(income: Int, step: Int) -> True
? explore nested_mechanism_stream_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = net_income(income),
            after = net_income(income + step)
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse nested mechanism stream fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let selected = 0;
        let plan = mechanism_runtime::CheckedNestedIfMechanismRuntimePlanV1::from_show_call_roots(
            &artifacts, selected, 0, 1,
        )
        .expect("check nested-if mechanism runtime plan");
        let directory = std::env::temp_dir().join(format!(
            "futuruna_nested_mechanism_stream_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let report_request = report::ExploreReportRequest {
            case_graph: report::ExploreCaseGraphRequest::Omit,
            ledger: report::ExploreLedgerRequest::Omit,
        };
        let mechanism_request = plan.request().clone();
        let open = || {
            stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
                &directory,
                run_store::RunStoreLimits::default(),
                &statements,
                None,
                &artifacts,
                selected,
                report_request,
                mechanism_request.clone(),
            )
        };

        let mut coordinator = open().expect("create pre-probe nested mechanism stream");
        let pre_probe_pause = pause_exact_stream_slice_without_snapshot(
            &mut coordinator,
            run_stream::PauseReason::TimeLimit,
            ExploreStreamSliceStop::TimeLimit,
            ExploreStreamObserverDeferral::ProbeIncomplete,
            0,
            0,
        )
        .expect("commit journal-only pre-probe pause");
        assert!(!pre_probe_pause.probe_milestone_complete);
        assert!(matches!(
            pre_probe_pause.artifact,
            ExploreStreamArtifact::MechanismJournalOnlyCheckpoint {
                observer_deferral: ExploreStreamObserverDeferral::ProbeIncomplete,
            }
        ));
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        drop(coordinator);

        let coordinator = open().expect("resume journal-only pre-probe mechanism stream");
        assert!(!coordinator.probe_phase_complete());
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        drop(coordinator);

        let probe_report = execute_checked_stream_slice_v1(
            &statements,
            None,
            source,
            Some("nested_mechanism_stream_fixture"),
            ExploreStreamSliceOptions {
                run_state: directory.clone(),
                max_runtime: None,
                pause_after: Some(ExploreStreamPauseAfter::Probes),
                case_graph: ExploreStreamCaseGraphRequest::Omit,
                finalize: false,
            },
            CheckedStreamExecutionProfileV1::NestedIfMechanism {
                before_show_index: 0,
                after_show_index: 1,
            },
            Some(
                stream_resource::ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
                    .expect("create deterministic scheduler-test resource authority"),
            ),
        )
        .expect("run the bounded mechanism stream through its probe milestone");
        assert_eq!(probe_report.stop, ExploreStreamSliceStop::ProbeMilestone);
        assert!(probe_report.probe_milestone_complete);
        assert_eq!(probe_report.singleton_cases_evaluated_this_slice, 0);
        let initial_json = match &probe_report.artifact {
            ExploreStreamArtifact::MechanismCheckpointJsonLine {
                canonical_json_line,
                ..
            } => std::str::from_utf8(canonical_json_line)
                .expect("initial mechanism checkpoint is UTF-8"),
            other => panic!("expected initial mechanism checkpoint, got {other:?}"),
        };
        assert!(initial_json.contains("\"schema\":\"futuruna.explore.mechanism-checkpoint.v1\""));
        assert!(initial_json.contains("\"status\":\"scope_open\""));
        assert!(initial_json.contains(
            "\"mechanism_signatures\":{\"certainty\":\"unknown\",\"value\":null,\"confirmed_lower_bound\":\"0\"}"
        ));

        // Simulate an abrupt process stop after exact classification but before
        // its newly confirmed target receives mechanism replay. The recovered
        // scheduler must service that durable backlog before classifying rank 1.
        let mut coordinator = open().expect("resume the probe-paused nested mechanism stream");
        assert_eq!(
            next_mechanism_stream_work_v1(&mut coordinator).expect("select first post-probe work"),
            MechanismStreamWorkV1::ClassifyCase { rank: 0 }
        );

        assert!(matches!(
            coordinator
                .advance_one_case()
                .expect("classify first nested mechanism case"),
            stream_coordinator::ExactStreamAdvance::Committed { rank: 0, .. }
        ));
        assert_eq!(
            next_mechanism_stream_work_v1(&mut coordinator)
                .expect("prioritize the newly confirmed mechanism target"),
            MechanismStreamWorkV1::ReplayConfirmedMechanism { rank: 0 }
        );
        let partial_before_recovery = coordinator
            .mechanism_snapshot()
            .expect("materialize partial nested mechanism evidence")
            .expect("nested mechanism evidence is enabled");
        drop(coordinator);

        let final_report = execute_checked_stream_slice_v1(
            &statements,
            None,
            source,
            Some("nested_mechanism_stream_fixture"),
            ExploreStreamSliceOptions {
                run_state: directory.clone(),
                max_runtime: Some(Duration::from_secs(10)),
                pause_after: None,
                case_graph: ExploreStreamCaseGraphRequest::Omit,
                finalize: false,
            },
            CheckedStreamExecutionProfileV1::NestedIfMechanism {
                before_show_index: 0,
                after_show_index: 1,
            },
            Some(
                stream_resource::ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
                    .expect("create resumed scheduler-test resource authority"),
            ),
        )
        .expect("resume and close the bounded mechanism stream");
        assert_eq!(
            final_report.stop,
            ExploreStreamSliceStop::MechanismObservationClosedTerminalUnavailable
        );
        assert!(final_report.probe_milestone_complete);
        assert_eq!(final_report.singleton_cases_evaluated_this_slice, 3);
        assert_eq!(final_report.closed_cases_this_slice, 3);
        let final_json = match &final_report.artifact {
            ExploreStreamArtifact::MechanismCheckpointJsonLine {
                canonical_json_line,
                ..
            } => std::str::from_utf8(canonical_json_line)
                .expect("closed mechanism checkpoint is UTF-8"),
            other => panic!("expected closed mechanism checkpoint, got {other:?}"),
        };
        assert!(final_json.contains("\"status\":\"matching_closed\""));
        assert!(final_json.contains("\"target_cases\":{\"certainty\":\"exact\",\"value\":\"3\"}"));
        assert!(final_json.contains("\"traced_cases\":\"3\""));
        assert!(final_json
            .contains("\"mechanism_signatures\":{\"certainty\":\"exact\",\"value\":\"3\"}"));

        let mut coordinator = open().expect("recover the closed nested mechanism stream");
        assert!(coordinator
            .next_mechanism_rank_hint()
            .expect("inspect recovered nested mechanism backlog")
            .is_none());
        assert_eq!(coordinator.closed_case_count(), 4);

        let evidence = coordinator
            .mechanism_snapshot()
            .expect("materialize complete nested mechanism evidence")
            .expect("complete nested mechanism evidence is enabled");
        assert_eq!(
            evidence.population.status,
            mechanism::MechanismEvidenceStatus::MatchingClosed
        );
        assert_eq!(
            evidence.population.requested_target,
            mechanism::MechanismCount::Exact(3)
        );
        assert_eq!(evidence.population.traced, 3);
        assert_eq!(evidence.population.known_target_untraced, 0);
        assert_eq!(evidence.signatures.len(), 3);
        assert!(evidence.bin_fields.is_empty());
        assert_ne!(evidence, partial_before_recovery);

        let mut outcomes = BTreeSet::new();
        let mut stable_shape = None;
        for signature in evidence.signatures.values() {
            assert_eq!(signature.observed_support, 1);
            assert_eq!(signature.signature.nodes.len(), 1);
            assert_eq!(signature.signature.before_roots.len(), 1);
            assert_eq!(signature.signature.after_roots.len(), 1);
            let node = signature
                .signature
                .nodes
                .values()
                .next()
                .expect("one nested mechanism node");
            assert_eq!(node.slot.kind, mechanism::DynamicEventKind::IfDecision);
            assert_eq!(node.slot.visit_ordinal, 0);
            assert_eq!(node.slot.activation_path.len(), 1);
            assert!(node.before_dependencies.is_empty());
            assert!(node.after_dependencies.is_empty());
            let activation = &node.slot.activation_path[0];
            assert_eq!(activation.invocation_ordinal, 0);
            let before = match node.before.as_ref() {
                Some(mechanism::DynamicEventOutcome::IfDecision(outcome)) => *outcome,
                other => panic!("unexpected before nested mechanism outcome: {other:?}"),
            };
            let after = match node.after.as_ref() {
                Some(mechanism::DynamicEventOutcome::IfDecision(outcome)) => *outcome,
                other => panic!("unexpected after nested mechanism outcome: {other:?}"),
            };
            outcomes.insert((before, after));
            let shape = (
                activation.call_site.clone(),
                activation.callee.clone(),
                node.slot.site.clone(),
            );
            if let Some(expected) = &stable_shape {
                assert_eq!(&shape, expected);
            } else {
                stable_shape = Some(shape);
            }
        }
        assert_eq!(
            outcomes,
            BTreeSet::from([
                (
                    mechanism::IfDecisionOutcome::Else,
                    mechanism::IfDecisionOutcome::Else,
                ),
                (
                    mechanism::IfDecisionOutcome::Else,
                    mechanism::IfDecisionOutcome::Then,
                ),
                (
                    mechanism::IfDecisionOutcome::Then,
                    mechanism::IfDecisionOutcome::Then,
                ),
            ])
        );
        drop(coordinator);
        std::fs::remove_dir_all(&directory)
            .expect("remove nested mechanism stream fixture directory");
    }

    #[test]
    fn rule_dispatch_mechanism_stream_traces_equal_results_through_distinct_candidates() {
        let source = r#"
| route(0) -> True
| route(value: Int) -> True
| eligible(income: Int, step: Int) -> True
? explore rule_dispatch_mechanism_stream_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(0, 3)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = route(income),
            after = route(income + step)
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse rule-dispatch mechanism stream fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let selected = 0;
        let plan =
            mechanism_runtime::CheckedRuleDispatchMechanismRuntimePlanV1::from_show_call_roots(
                &artifacts, selected, 0, 1,
            )
            .expect("check direct rule-dispatch mechanism runtime plan");
        let route_family = artifacts
            .checked_resolutions
            .rule_families
            .get(&RuleDispatchKey {
                scope: None,
                name: "route".to_string(),
                arity: 1,
            })
            .expect("checked route family");
        let [literal_candidate, fallback_candidate] = route_family.candidates.as_ref() else {
            panic!("route fixture must retain exactly two checked candidates")
        };
        let literal_site = mechanism::MechanismSiteId::from_rule_candidate(
            &artifacts.analysis_program.id,
            literal_candidate,
        )
        .expect("literal rule candidate site");
        let fallback_site = mechanism::MechanismSiteId::from_rule_candidate(
            &artifacts.analysis_program.id,
            fallback_candidate,
        )
        .expect("fallback rule candidate site");
        let selection_site = mechanism::MechanismSiteId::from_rule_family(
            &artifacts.analysis_program.id,
            &route_family.key,
        )
        .expect("route selection site");
        let directory = std::env::temp_dir().join(format!(
            "futuruna_rule_dispatch_mechanism_stream_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let report_request = report::ExploreReportRequest {
            case_graph: report::ExploreCaseGraphRequest::Omit,
            ledger: report::ExploreLedgerRequest::Omit,
        };

        let final_report = execute_checked_stream_slice_v1(
            &statements,
            None,
            source,
            Some("rule_dispatch_mechanism_stream_fixture"),
            ExploreStreamSliceOptions {
                run_state: directory.clone(),
                max_runtime: Some(Duration::from_secs(10)),
                pause_after: None,
                case_graph: ExploreStreamCaseGraphRequest::Omit,
                finalize: false,
            },
            CheckedStreamExecutionProfileV1::RuleDispatchMechanism {
                before_show_index: 0,
                after_show_index: 1,
            },
            Some(
                stream_resource::ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
                    .expect("create deterministic rule scheduler-test resource authority"),
            ),
        )
        .expect("close the tiny rule-dispatch mechanism stream");
        assert_eq!(
            final_report.stop,
            ExploreStreamSliceStop::MechanismObservationClosedTerminalUnavailable
        );
        assert!(final_report.probe_milestone_complete);
        assert_eq!(final_report.singleton_cases_evaluated_this_slice, 3);
        let final_json = match &final_report.artifact {
            ExploreStreamArtifact::MechanismCheckpointJsonLine {
                canonical_json_line,
                ..
            } => std::str::from_utf8(canonical_json_line)
                .expect("closed rule mechanism checkpoint is UTF-8"),
            other => panic!("expected closed rule mechanism checkpoint, got {other:?}"),
        };
        assert!(
            final_json.contains("\"target_cases\":{\"certainty\":\"exact\",\"value\":\"2\"}"),
            "{final_json}"
        );
        assert!(
            final_json.contains("\"traced_cases\":\"2\""),
            "{final_json}"
        );
        assert!(
            final_json
                .contains("\"mechanism_signatures\":{\"certainty\":\"exact\",\"value\":\"2\"}"),
            "{final_json}"
        );

        let mut recovered =
            stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
                &directory,
                run_store::RunStoreLimits::default(),
                &statements,
                None,
                &artifacts,
                selected,
                report_request,
                plan.request().clone(),
            )
            .expect("recover closed rule-dispatch mechanism stream");
        let evidence = recovered
            .mechanism_snapshot()
            .expect("materialize recovered rule mechanism evidence")
            .expect("rule mechanism evidence is enabled");
        assert_eq!(
            evidence.population.requested_target,
            mechanism::MechanismCount::Exact(2)
        );
        assert_eq!(evidence.population.traced, 2);
        assert_eq!(evidence.signatures.len(), 2);

        let mut saw_candidate_transition = false;
        let mut saw_stable_fallback = false;
        for signature in evidence.signatures.values() {
            assert_eq!(signature.observed_support, 1);
            let selection = signature
                .signature
                .nodes
                .values()
                .find(|node| node.slot.site == selection_site)
                .expect("rule signature has a selection root");
            assert_eq!(
                selection.slot.kind,
                mechanism::DynamicEventKind::RuleSelection
            );
            assert!(signature.signature.before_roots.contains(&selection.id));
            assert!(signature.signature.after_roots.contains(&selection.id));
            assert!(selection.slot.activation_path.is_empty());
            let literal = signature
                .signature
                .nodes
                .values()
                .find(|node| node.slot.site == literal_site)
                .expect("rule signature retains literal candidate attempt");
            let fallback = signature
                .signature
                .nodes
                .values()
                .find(|node| node.slot.site == fallback_site)
                .expect("rule signature retains fallback candidate attempt");
            assert_eq!(literal.slot.kind, mechanism::DynamicEventKind::RuleAttempt);
            assert_eq!(fallback.slot.kind, mechanism::DynamicEventKind::RuleAttempt);
            assert!(literal.slot.activation_path.is_empty());
            assert!(fallback.slot.activation_path.is_empty());
            assert!(literal.before_dependencies.is_empty());
            assert!(literal.after_dependencies.is_empty());

            let applicable = Some(mechanism::DynamicEventOutcome::RuleAttempt(
                mechanism::RuleAttemptOutcome::Applicable,
            ));
            let head_mismatch = Some(mechanism::DynamicEventOutcome::RuleAttempt(
                mechanism::RuleAttemptOutcome::HeadMismatch,
            ));
            let selected_literal = Some(mechanism::DynamicEventOutcome::RuleSelection(
                mechanism::RuleSelectionOutcome::Selected(literal_site.clone()),
            ));
            let selected_fallback = Some(mechanism::DynamicEventOutcome::RuleSelection(
                mechanism::RuleSelectionOutcome::Selected(fallback_site.clone()),
            ));
            if literal.before == applicable {
                assert_eq!(literal.after, head_mismatch);
                assert_eq!(fallback.before, None);
                assert_eq!(fallback.after, applicable);
                assert!(fallback.before_dependencies.is_empty());
                assert_eq!(
                    fallback.after_dependencies,
                    BTreeSet::from([literal.id.clone()])
                );
                assert_eq!(selection.before, selected_literal);
                assert_eq!(selection.after, selected_fallback);
                assert_eq!(
                    selection.before_dependencies,
                    BTreeSet::from([literal.id.clone()])
                );
                assert_eq!(
                    selection.after_dependencies,
                    BTreeSet::from([fallback.id.clone()])
                );
                saw_candidate_transition = true;
            } else {
                assert_eq!(literal.before, head_mismatch);
                assert_eq!(literal.after, head_mismatch);
                assert_eq!(fallback.before, applicable);
                assert_eq!(fallback.after, applicable);
                assert_eq!(
                    fallback.before_dependencies,
                    BTreeSet::from([literal.id.clone()])
                );
                assert_eq!(
                    fallback.after_dependencies,
                    BTreeSet::from([literal.id.clone()])
                );
                assert_eq!(selection.before, selected_fallback);
                assert_eq!(selection.after, selected_fallback);
                assert_eq!(
                    selection.before_dependencies,
                    BTreeSet::from([fallback.id.clone()])
                );
                assert_eq!(
                    selection.after_dependencies,
                    BTreeSet::from([fallback.id.clone()])
                );
                saw_stable_fallback = true;
            }
        }
        assert!(saw_candidate_transition && saw_stable_fallback);
        drop(recovered);
        std::fs::remove_dir_all(&directory)
            .expect("remove rule-dispatch mechanism stream fixture directory");
    }

    #[test]
    fn rule_dispatch_mechanism_profile_rejects_non_pattern_head_before_replay() {
        let source = r#"
| route(1 + 1) -> True
| eligible(income: Int, step: Int) -> True
? explore rule_dispatch_non_pattern_head_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(0, 2)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = route(income),
            after = route(income + step)
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse non-pattern rule-head fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let error =
            mechanism_runtime::CheckedRuleDispatchMechanismRuntimePlanV1::from_show_call_roots(
                &artifacts, 0, 0, 1,
            )
            .err()
            .expect("non-pattern rule head must be rejected before replay");
        assert!(
            error.contains("outside the direct matcher subset"),
            "{error}"
        );
    }

    #[test]
    fn nested_mechanism_profile_rejects_unsupported_show_argument_before_replay() {
        let source = r#"
> adjustment(income: Int) -> Int {
    if income >= 200 { 20 } else { 0 }
}
> net_income(values: List(Int)) -> Int {
    adjustment(200)
}
| eligible(income: Int, step: Int) -> True
? explore nested_mechanism_show_subset_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = net_income([income]),
            after = net_income([income + step])
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse unsupported nested mechanism show fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let error = mechanism_runtime::CheckedNestedIfMechanismRuntimePlanV1::from_show_call_roots(
            &artifacts, 0, 0, 1,
        )
        .err()
        .expect("unsupported show argument must fail during static profile selection");
        assert!(error.contains("outside the nested trace subset"), "{error}");
    }

    #[test]
    fn nested_mechanism_runtime_refuses_live_import_graph_before_stream_creation() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_nested_mechanism_import_drift_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create import-drift fixture directory");
        let helper_path = directory.join("adjustment.runa");
        let checked_helper = r#"
> adjustment(income: Int) -> Int {
    if income >= 200 { 20 } else { 0 }
}
"#;
        std::fs::write(&helper_path, checked_helper).expect("write checked helper module");
        let source = r#"
@ import ./adjustment
> net_income(income: Int) -> Int {
    income - adjustment(income)
}
| eligible(income: Int, step: Int) -> True
? explore nested_mechanism_import_drift_fixture {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [
            before = net_income(income),
            after = net_income(income + step)
        ]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse import-drift mechanism fixture");
        let source_dir = directory.to_string_lossy().to_string();
        let artifacts =
            TypeChecker::check_with_artifacts(&statements, Some(source_dir.clone()), source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let error = mechanism_runtime::CheckedNestedIfMechanismRuntimePlanV1::from_show_call_roots(
            &artifacts, 0, 0, 1,
        )
        .err()
        .expect("a live imported helper must not authorize durable mechanism replay");
        assert!(error.contains("frozen module graph"), "{error}");

        let request = mechanism_request::build_checked_mechanism_request_v1(
            &artifacts,
            0,
            mechanism_request::MechanismTraceSelectionV1 {
                before_show_index: 0,
                after_show_index: 1,
                bin_fields: Box::default(),
                retained_examples_per_signature: 1,
            },
        )
        .expect("build identity-only request independently of runtime profile");
        let stream_directory = directory.join("mechanism-stream");
        let coordinator_error =
            stream_coordinator::ExactStreamCoordinator::open_or_create_with_mechanism(
                &stream_directory,
                run_store::RunStoreLimits::default(),
                &statements,
                Some(&source_dir),
                &artifacts,
                0,
                report::ExploreReportRequest {
                    case_graph: report::ExploreCaseGraphRequest::Omit,
                    ledger: report::ExploreLedgerRequest::Omit,
                },
                request,
            )
            .err()
            .expect("coordinator must refuse imports before opening its run store");
        assert!(
            coordinator_error
                .to_string()
                .contains("frozen module graph"),
            "{coordinator_error}"
        );
        assert!(
            !stream_directory.exists(),
            "rejected mechanism source created a durable run directory"
        );

        std::fs::remove_dir_all(&directory)
            .expect("remove live-import mechanism fixture directory");
    }

    #[test]
    fn exact_stream_evaluator_rejects_caller_root_drift_after_checking() {
        let checked_source = r#"
> score(value: Int) -> Int { value }
| eligible(value: Int) -> True
? explore immutable_runtime_root_fixture {
    over eligible(value)
    find matches
    bounds { value in [1] }
    output { key [value] show [result = score(value)] representative first }
}
"#;
        let mut lexer = Lexer::new(checked_source);
        let checked_statements = Parser::new(lexer.tokenize(), checked_source)
            .parse_program()
            .expect("parse checked immutable-root fixture");
        let artifacts =
            TypeChecker::check_with_artifacts(&checked_statements, None, checked_source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );

        let drifted_source = checked_source.replace("{ value }", "{ value + 1 }");
        let mut lexer = Lexer::new(&drifted_source);
        let drifted_statements = Parser::new(lexer.tokenize(), &drifted_source)
            .parse_program()
            .expect("parse drifted immutable-root fixture");
        let error = exact::ExactStreamEvaluator::prepare(
            &drifted_statements,
            None,
            &artifacts,
            0,
            report::DEFAULT_EXPLORE_STEP_LIMIT,
            report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
        )
        .err()
        .expect("caller root drift must not replace the checked runtime snapshot");
        assert!(error.contains("runtime entry syntax differs"), "{error}");
    }

    #[test]
    fn mechanism_runtime_rejects_equal_shape_query_identity_drift() {
        let source = r#"
> net_income(income: Int) -> Int {
    if income >= 200 { income - 20 } else { income }
}
| eligible(income: Int, step: Int) -> True
? explore first_query {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [before = net_income(income), after = net_income(income + step)]
        representative first
    }
}
? explore second_query {
    over eligible(income, step)
    find matches
    bounds {
        income in range(198, 202)
        step = 1
    }
    boundaries on income by step
    output {
        key [income]
        show [before = net_income(income), after = net_income(income + step)]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse equal-shape mechanism queries");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let plan = mechanism_runtime::CheckedSingleIfMechanismRuntimePlanV1::from_show_call_roots(
            &artifacts, 0, 0, 1,
        )
        .expect("check first mechanism plan");
        let evaluator = exact::ExactStreamEvaluator::prepare(
            &statements,
            None,
            &artifacts,
            1,
            report::DEFAULT_EXPLORE_STEP_LIMIT,
            report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
        )
        .expect("prepare second query evaluator");
        let error = match mechanism_runtime::mint_single_if_mechanism_observation_v1(
            &plan, &evaluator, 0,
        ) {
            Ok(_) => panic!("another checked query minted this plan's evidence"),
            Err(mechanism_runtime::MechanismRuntimeMintErrorV1::Failure(error)) => error,
            Err(mechanism_runtime::MechanismRuntimeMintErrorV1::OperationalLimit(reason)) => {
                panic!("equal-shape identity test hit an operational limit: {reason:?}")
            }
        };
        assert!(error.contains("different checked queries"), "{error}");
    }

    #[test]
    fn durable_checkpoint_pause_resume_finalize_and_reopen_is_idempotent() {
        let source = r#"
| condition(value: Int) -> True
? explore one_case_stream {
    over condition(value)
    find matches
    bounds { value in [7] }
    output { key [value] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse one-case durable-stream fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let selected = 0;
        let query = &artifacts.exploration_universes[selected];
        let graph_request = report::ExploreReportRequest {
            case_graph: report::ExploreCaseGraphRequest::Include,
            ledger: report::ExploreLedgerRequest::Omit,
        };
        assert_eq!(
            query.universe.cartesian_count_before_constraints,
            ExploreCardinality::Exact(1)
        );

        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_durable_lifecycle_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("create one-case durable stream");

        match source_proof_plan::prepare_source_proof_plan(
            &artifacts,
            selected,
            source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
        ) {
            Ok(plan) => {
                coordinator
                    .persist_source_probe_manifest(&plan)
                    .expect("persist source-probe manifest");
            }
            Err(error) if error.permits_canonical_fallback() => {
                coordinator
                    .persist_probe_fallback_manifest()
                    .expect("persist canonical probe fallback");
            }
            Err(error) => panic!("one-case source probe failed closed: {error:?}"),
        }
        coordinator
            .accept_prepared_probe_coverage(NonZeroU64::new(1).expect("one is nonzero"))
            .expect("accept one-case probe coverage");
        let probe_progress = coordinator
            .complete_prepared_probe()
            .expect("complete one-case source probe");
        assert!(probe_progress.complete());

        let prepared_checkpoint = coordinator
            .prepare_observable_snapshot_publication_for_test()
            .expect("prepare one-case checkpoint");
        let checkpoint = publish_prepared_snapshot_and_pause_exact_stream_slice(
            &mut coordinator,
            prepared_checkpoint,
            run_stream::PauseReason::ProbeMilestone,
            ExploreStreamSliceStop::ProbeMilestone,
            0,
            0,
        )
        .expect("publish and pause one-case checkpoint");
        assert_eq!(checkpoint.stop, ExploreStreamSliceStop::ProbeMilestone);
        assert_eq!(
            checkpoint.final_cursor.lifecycle,
            ExploreStreamLifecycle::Paused
        );
        assert!(checkpoint.probe_milestone_complete);
        let (checkpoint_cursor, publication_cursor) = match &checkpoint.artifact {
            ExploreStreamArtifact::CheckpointSnapshotJsonLine {
                canonical_json_line,
                checkpoint_cursor,
                publication_cursor,
                ..
            } => {
                assert!(canonical_json_line.ends_with(b"\n"));
                assert_eq!(
                    canonical_json_line
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count(),
                    1
                );
                let rendered =
                    std::str::from_utf8(canonical_json_line).expect("checkpoint JSON is UTF-8");
                assert!(rendered.contains("\"case_graph\":\"full\""));
                assert!(rendered.contains("\"status\":\"included\""));
                assert!(rendered.contains("\"classification\":\"eligibility_open\""));
                (checkpoint_cursor, publication_cursor)
            }
            ExploreStreamArtifact::TerminalResultJson { .. } => {
                panic!("probe milestone returned a terminal artifact")
            }
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine { .. } => {
                panic!("one-case probe checkpoint unexpectedly hit snapshot capacity")
            }
            ExploreStreamArtifact::MechanismCheckpointJsonLine { .. }
            | ExploreStreamArtifact::MechanismCheckpointUnavailableJsonLine { .. }
            | ExploreStreamArtifact::MechanismJournalOnlyCheckpoint { .. } => {
                panic!("exact-only probe returned a mechanism checkpoint")
            }
            ExploreStreamArtifact::JournalOnlyCheckpoint { .. } => {
                panic!("direct graph-bearing checkpoint publication was deferred")
            }
        };
        assert_eq!(checkpoint_cursor.lifecycle, ExploreStreamLifecycle::Running);
        assert_eq!(
            publication_cursor.lifecycle,
            ExploreStreamLifecycle::Running
        );
        assert_eq!(
            publication_cursor.sequence,
            checkpoint_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_eq!(
            checkpoint.final_cursor.sequence,
            publication_cursor
                .sequence
                .checked_add(1)
                .expect("sequence")
        );
        assert_eq!(checkpoint_cursor.run_id, publication_cursor.run_id);
        assert_eq!(checkpoint_cursor.run_id, checkpoint.final_cursor.run_id);
        assert_ne!(
            checkpoint_cursor.journal_head,
            publication_cursor.journal_head
        );
        assert_ne!(
            publication_cursor.journal_head,
            checkpoint.final_cursor.journal_head
        );
        assert_eq!(
            checkpoint_cursor.evidence_root,
            publication_cursor.evidence_root
        );
        assert_eq!(
            publication_cursor.evidence_root,
            checkpoint.final_cursor.evidence_root
        );
        let paused_cursor = checkpoint.final_cursor.clone();
        drop(coordinator);

        let mismatch = match stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            report::ExploreReportRequest::baseline(),
        ) {
            Ok(_) => panic!("case-graph authorization is immutable run identity"),
            Err(error) => error,
        };
        assert!(mismatch
            .to_string()
            .contains("stored Explore stream header does not match"));

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume one-case durable stream");
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        let resumed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(resumed_cursor.lifecycle, ExploreStreamLifecycle::Running);
        assert_eq!(resumed_cursor.run_id, paused_cursor.run_id);
        assert_eq!(
            resumed_cursor.sequence,
            paused_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_ne!(resumed_cursor.journal_head, paused_cursor.journal_head);
        assert_eq!(resumed_cursor.evidence_root, paused_cursor.evidence_root);

        let journal_only = pause_exact_stream_slice_without_snapshot(
            &mut coordinator,
            run_stream::PauseReason::TimeLimit,
            ExploreStreamSliceStop::TimeLimit,
            ExploreStreamObserverDeferral::TimeLimit,
            0,
            0,
        )
        .expect("pause one-case stream without materializing another snapshot");
        assert_eq!(
            journal_only.final_cursor.sequence,
            resumed_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert!(matches!(
            journal_only.artifact,
            ExploreStreamArtifact::JournalOnlyCheckpoint {
                observer_deferral: ExploreStreamObserverDeferral::TimeLimit,
            }
        ));
        assert!(coordinator.pending_observable_snapshot_on_resume());
        let journal_only_cursor = journal_only.final_cursor;
        drop(coordinator);

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume journal-only one-case checkpoint");
        let resumed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(
            resumed_cursor.sequence,
            journal_only_cursor
                .sequence
                .checked_add(1)
                .expect("sequence")
        );
        assert_eq!(
            resumed_cursor.evidence_root,
            journal_only_cursor.evidence_root
        );
        assert!(coordinator.pending_observable_snapshot_on_resume());
        let debt_resume_cursor = coordinator.stream().cursor();
        drop(coordinator);

        // Simulate process loss after Resumed but before materialization. A
        // Recovery record must preserve observer debt rather than letting
        // semantic work outrun it.
        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("recover pending observer debt after an interrupted resume");
        assert!(coordinator.pending_observable_snapshot_on_resume());
        assert_eq!(
            coordinator.stream().cursor().sequence(),
            debt_resume_cursor
                .sequence()
                .checked_add(1)
                .expect("sequence")
        );

        let prepared_catch_up = coordinator
            .prepare_observable_snapshot_unavailable_for_test(
                "forced admitted-capacity outcome for lifecycle coverage",
            )
            .expect("prepare pending observer-unavailable receipt");
        let catch_up = publish_prepared_snapshot_and_pause_exact_stream_slice(
            &mut coordinator,
            prepared_catch_up,
            run_stream::PauseReason::Explicit,
            ExploreStreamSliceStop::SnapshotCatchUp,
            0,
            0,
        )
        .expect("materialize the pending observer view before further search");
        assert_eq!(catch_up.stop, ExploreStreamSliceStop::SnapshotCatchUp);
        match &catch_up.artifact {
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine {
                canonical_json_line,
                checkpoint_cursor,
                publication_cursor,
                detail,
                ..
            } => {
                let rendered = std::str::from_utf8(canonical_json_line)
                    .expect("snapshot-unavailable JSON is UTF-8");
                assert!(rendered.contains("\"status\":\"unavailable\""));
                assert!(rendered.contains("\"reason\":{\"kind\":\"capacity\"}"));
                assert!(!rendered.contains("\"configuration\""));
                assert!(!rendered.contains("\"answer\""));
                assert!(!rendered.contains("\"case_graph\""));
                assert_eq!(
                    canonical_json_line
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count(),
                    1
                );
                assert!(
                    canonical_json_line.len()
                        <= stream_snapshot::EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1
                );
                assert_eq!(
                    checkpoint_cursor.evidence_root,
                    publication_cursor.evidence_root
                );
                assert_eq!(
                    publication_cursor.evidence_root,
                    catch_up.final_cursor.evidence_root
                );
                assert_eq!(
                    detail,
                    "forced admitted-capacity outcome for lifecycle coverage"
                );
            }
            _ => panic!("catch-up did not publish the bounded snapshot-unavailable receipt"),
        }
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        let catch_up_cursor = catch_up.final_cursor;
        drop(coordinator);

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume after materializing the pending observer view");
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        assert_eq!(
            coordinator.stream().cursor().sequence(),
            catch_up_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_eq!(
            coordinator
                .stream()
                .cursor()
                .evidence_root()
                .to_lowercase_hex(),
            catch_up_cursor.evidence_root
        );

        let case_cap = NonZeroU16::new(1).expect("one is nonzero");
        while let Some(rank) = coordinator.next_open_rank_hint() {
            match coordinator
                .advance_bounded_case_batch(case_cap)
                .expect("classify one-case durable frontier")
            {
                stream_coordinator::ExactStreamBatchAdvance::Committed { ranks, .. } => {
                    assert_eq!(ranks.as_ref(), &[rank]);
                }
                stream_coordinator::ExactStreamBatchAdvance::ClassificationClosedFinalizationPending => {
                    panic!("open-rank hint disagreed with the exact frontier")
                }
                stream_coordinator::ExactStreamBatchAdvance::CaseOpen { .. } => {
                    panic!("one-case fixture hit an evaluation limit")
                }
            }
        }
        assert_eq!(coordinator.closed_case_count(), 1);
        assert!(coordinator.exact_snapshot().result_group_scan_complete);
        let final_case_graph = coordinator
            .prepare_case_graph_publication()
            .expect("prepare final one-case graph");
        let terminal_result_json =
            match attempt_atomic_exact_stream_finalization(&mut coordinator, &final_case_graph)
                .expect("finalize one-case durable stream")
            {
                ExactStreamFinalizationAttempt::Sealed(bytes) => bytes,
                ExactStreamFinalizationAttempt::WitnessOpen { .. } => {
                    panic!("one-case finalization left a replay witness open")
                }
                ExactStreamFinalizationAttempt::LimitReached { .. } => {
                    panic!("one-case finalization exceeded an atomic limit")
                }
            };
        let terminal_rendered =
            std::str::from_utf8(&terminal_result_json).expect("terminal JSON is UTF-8");
        assert!(terminal_rendered.contains("\"case_graph\":\"full\""));
        assert!(terminal_rendered.contains("\"status\":\"included\""));
        assert!(terminal_rendered.contains("\"classification\":\"admissible_match\""));
        assert!(terminal_rendered.contains("\"views\":\"closed\""));
        let sealed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(sealed_cursor.lifecycle, ExploreStreamLifecycle::Sealed);
        let terminal_blob_digest = coordinator
            .published_terminal_result()
            .expect("terminal publication receipt")
            .blob_digest()
            .to_lowercase_hex();
        drop(coordinator);

        let coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("reopen sealed one-case durable stream");
        assert_eq!(
            public_exact_stream_cursor(coordinator.stream().cursor()),
            sealed_cursor
        );
        let already_sealed =
            render_already_sealed_exact_stream(&coordinator, coordinator.closed_case_count())
                .expect("render already-sealed one-case receipt");
        assert_eq!(
            already_sealed.stop,
            ExploreStreamSliceStop::AlreadySealed(ExploreStreamTerminalStatus::Completed)
        );
        assert_eq!(already_sealed.final_cursor, sealed_cursor);
        assert_eq!(already_sealed.singleton_cases_evaluated_this_slice, 0);
        assert_eq!(already_sealed.closed_cases_this_slice, 0);
        match already_sealed.artifact {
            ExploreStreamArtifact::TerminalResultJson {
                canonical_json,
                blob_digest,
            } => {
                assert_eq!(canonical_json, terminal_result_json);
                assert_eq!(blob_digest, terminal_blob_digest);
            }
            ExploreStreamArtifact::CheckpointSnapshotJsonLine { .. } => {
                panic!("already-sealed reopen returned a checkpoint artifact")
            }
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine { .. } => {
                panic!("already-sealed reopen returned a snapshot-capacity artifact")
            }
            ExploreStreamArtifact::MechanismCheckpointJsonLine { .. }
            | ExploreStreamArtifact::MechanismCheckpointUnavailableJsonLine { .. }
            | ExploreStreamArtifact::MechanismJournalOnlyCheckpoint { .. } => {
                panic!("already-sealed exact stream returned a mechanism checkpoint")
            }
            ExploreStreamArtifact::JournalOnlyCheckpoint { .. } => {
                panic!("already-sealed reopen returned a journal-only checkpoint")
            }
        }
        drop(coordinator);
        std::fs::remove_dir_all(&directory).expect("remove one-case durable-stream fixture");
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
        let closed_query = &artifacts.exploration_universes[0];
        let universe = &closed_query.universe;
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
        let boundary = closed_query.boundary_hint().expect("boundary");
        assert_eq!(boundary.eligible_axis_pairs, ExploreCardinality::Exact(4));
        assert_eq!(
            boundary.eligible_unconstrained_pairs,
            ExploreCardinality::Exact(8)
        );
        assert_eq!(boundary.axis_dimension_index, 1);
        assert!(boundary.requires_both_endpoints_in_domain);
        assert_eq!(boundary.recomputed_fact_indices, vec![1, 2]);
        assert_eq!(
            closed_query.transition.mode,
            ExploreTransitionMode::Relative
        );
        assert!(universe
            .dimensions
            .iter()
            .all(|dimension| dimension.role == ExploreGeneratorAxisRole::Before));
        assert!(universe
            .constraints
            .iter()
            .all(|constraint| { constraint.scope == ExploreConstraintScope::BothEndpoints }));
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
        let closed_query = &artifacts.exploration_universes[0];
        let universe = &closed_query.universe;
        assert_eq!(
            universe.cartesian_count_before_constraints,
            ExploreCardinality::Exact(0)
        );
        assert!(universe
            .dimensions
            .iter()
            .all(|dimension| dimension.domain.cardinality() == ExploreCardinality::Exact(0)));
        assert_eq!(
            closed_query.transition.mode,
            ExploreTransitionMode::Identity
        );
        assert!(closed_query.transition.context_schema.fields.is_empty());
        assert!(closed_query
            .transition
            .after_fields
            .iter()
            .all(|field| matches!(&field.source, ExploreAfterFieldSourceIr::FrameBefore { .. })));
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
            .boundary_hint()
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
    fn replay_identity_gate_rejects_runtime_declarations_that_preempt_rules() {
        let fixtures = [
            (
                r#"
# Flag = On | Off | choose(value: Int)
| choose(value: Int) -> On
| condition(value: Int) -> choose(value) == On
? explore rule_constructor_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "constructor `choose(1 argument)` takes precedence over the rule",
            ),
            (
                r#"
# Flag = On | Off
= choose = |flag: Flag| flag == Off
| choose(flag: Flag) -> flag == On
| condition(flag: Flag) -> choose(flag)
? explore rule_closure_binding_collision {
    over condition(flag)
    find matches
    bounds { flag in values(Flag) }
    output { key [flag] representative first }
}
"#,
                "rule call `choose` is shadowed by a top-level binding",
            ),
            (
                r#"
# Flag = On | Off
# trait Choice {
    > choose(self) -> Bool
}
# impl Choice for Flag {
    > choose(self) -> Bool { False }
}
| choose(flag: Flag) -> flag == On
| condition(flag: Flag) -> choose(flag)
? explore rule_impl_method_collision {
    over condition(flag)
    find matches
    bounds { flag in values(Flag) }
    output { key [flag] representative first }
}
"#,
                "rule call `choose` collides with an unsupported callable",
            ),
        ];

        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
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
    fn replay_identity_gate_allows_rule_clauses_and_arities_without_sibling_collisions() {
        let source = r#"
# Flag = On | Off
| choose(flag: Flag) -> True under flag == On
| choose(flag: Flag) -> False
| choose() -> True
| condition(flag: Flag) -> choose(flag)
? explore unique_rule_family {
    over condition(flag)
    find matches
    bounds { flag in values(Flag) }
    output { key [flag] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert_eq!(
            artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(2)
        );
    }

    #[test]
    fn replay_identity_gate_rejects_special_dispatch_and_builtin_value_shadowing() {
        let fixtures = [
            (
                r#"
| findall(template: Int, goal: Int) -> [template]
| condition(template: Int, goal: Int) -> length(findall(template, goal)) > 0
? explore findall_rule_collision {
    over condition(template, goal)
    find matches
    bounds { template in [1]; goal in [1] }
    output { key [template, goal] representative first }
}
"#,
                "runtime special form `findall(2 arguments)`",
            ),
            (
                r#"
| search(template: Int, goal: Int) -> Some(template)
| condition(template: Int, goal: Int) -> search(template, goal) == Some(template)
? explore search_rule_collision {
    over condition(template, goal)
    find matches
    bounds { template in [1]; goal in [1] }
    output { key [template, goal] representative first }
}
"#,
                "runtime special form `search(2 arguments)`",
            ),
            (
                r#"
= abs: Int = 0
| condition(value: Int) -> abs(value) == value
? explore builtin_value_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "call `abs` is shadowed by a top-level binding",
            ),
            (
                r#"
# Weird = abs(left: Int, right: Int)
| condition(value: Int) -> show(abs(value)) == show(value)
? explore direct_builtin_constructor_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "direct built-in call `abs(1 argument)` is shadowed at runtime by a different-arity constructor",
            ),
            (
                r#"
| condition(template: Int, goal: Int) -> length(template |> findall(goal)) > 0
? explore pipe_findall_special {
    over condition(template, goal)
    find matches
    bounds { template in [1]; goal in [1] }
    output { key [template, goal] representative first }
}
"#,
                "runtime special form `findall(2 arguments)`",
            ),
            (
                r#"
| condition(template: Int, goal: Int) -> (template |> search(goal)) == Some(template)
? explore pipe_search_special {
    over condition(template, goal)
    find matches
    bounds { template in [1]; goal in [1] }
    output { key [template, goal] representative first }
}
"#,
                "runtime special form `search(2 arguments)`",
            ),
        ];

        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
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

        for special in ["findall", "search"] {
            let source = format!(
                r#"
| {special}(template: Int, goal: Int) -> True
? explore direct_special_target {{
    over {special}(template, goal)
    find matches
    bounds {{ template in [1]; goal in [1] }}
    output {{ key [template, goal] representative first }}
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
                    .any(|diagnostic| diagnostic.message.contains("runtime special form")),
                "missing direct special-form target diagnostic: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn replay_identity_gate_allows_exact_arity_rule_fallbacks_and_builtin_fallthrough() {
        let source = r#"
| mixed(1)
| mixed(1, 1) -> 1
| mixed_condition(value: Int) -> mixed(value)

| not(left: Bool, right: Bool) -> left && right
| builtin_condition(value: Bool) -> not(value)

? explore mixed_rule_arities {
    over mixed_condition(value)
    find matches
    bounds { value in [0] }
    output { key [value] representative first }
}

? explore direct_builtin_fallthrough {
    over builtin_condition(value)
    find matches
    bounds { value in values(Bool) }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        assert_eq!(artifacts.exploration_queries.len(), 2);
        assert_eq!(artifacts.exploration_universes.len(), 2);
        assert_eq!(
            artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(1)
        );
        assert_eq!(
            artifacts.exploration_universes[1]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(2)
        );
    }

    #[test]
    fn explore_replay_runtime_uses_canonical_boolean_rule_misses() {
        let source = r#"
| conditional(value: Int) -> True under value > 0
| exception positive exception_only(value: Int) -> True under value > 0
| combined(value: Int) -> conditional(value) || exception_only(value)

? explore boolean_misses {
    over combined(value)
    find matches
    bounds { value in [0, 1] }
    output { key [value] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse Boolean replay fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            artifacts.diagnostics
        );
        assert_eq!(
            artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(2)
        );

        let mut definitions =
            collect_ground_bindings(&statements, None).expect("ground declarations");
        definitions.rule_dispatch_return_types = artifacts.rule_dispatch_return_types.clone();
        definitions.rule_dispatch_return_issues = artifacts.rule_dispatch_return_issues.clone();
        definitions.rule_dispatch_boolean_miss_safe_keys =
            artifacts.rule_dispatch_boolean_miss_safe_keys.clone();
        let mut runtime = ExploreRuntimeGroundEvaluator::new(&definitions);
        for name in ["conditional_miss", "exception_miss"] {
            let rule = if name == "conditional_miss" {
                "conditional"
            } else {
                "exception_only"
            };
            let expression: Expr = ExprKind::App(
                Box::new(ExprKind::Var(rule.to_string()).into()),
                vec![ExprKind::Lit(Literal::Int(0)).into()],
            )
            .into();
            match runtime.eval(&expression, &[]) {
                Ok(Value::Bool(false)) => {}
                other => panic!("{name} must be the canonical replay false value: {other:?}"),
            }
        }
    }

    #[test]
    fn explore_replay_fails_closed_for_missing_or_conflicting_rule_classification() {
        let source = r#"
| conditional(value: Int) -> True under value > 0
| condition(value: Int) -> conditional(value)

? explore classification_gate {
    over condition(value)
    find matches
    bounds { value in [0, 1] }
    output { key [value] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse replay classification fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_queries[0];
        let target_key = ("condition".to_string(), 1);
        let helper_key = ("conditional".to_string(), 1);

        let mut missing = collect_ground_bindings(&statements, None).expect("ground declarations");
        missing
            .explore_rule_return_types_by_arity
            .insert(target_key.clone(), Ty::Name("Bool".to_string()));
        missing
            .explore_rule_return_types_by_arity
            .insert(helper_key.clone(), Ty::Name("Bool".to_string()));
        missing
            .explore_rule_return_types_by_arity
            .remove(&helper_key);
        let diagnostics = validate_query_replay_callable_identities(query, &missing);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot classify the exact return type of reachable rule `conditional(1 argument)`")),
            "missing classification must fail closed: {diagnostics:?}"
        );

        let mut conflicting =
            collect_ground_bindings(&statements, None).expect("ground declarations");
        conflicting
            .explore_rule_return_types_by_arity
            .insert(target_key, Ty::Name("Bool".to_string()));
        conflicting
            .explore_rule_return_types_by_arity
            .insert(helper_key.clone(), Ty::Name("Bool".to_string()));
        conflicting
            .explore_rule_return_issues
            .insert(helper_key, "synthetic conflicting return types".to_string());
        let diagnostics = validate_query_replay_callable_identities(query, &conflicting);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("synthetic conflicting return types")),
            "conflicting classification must fail closed: {diagnostics:?}"
        );
    }

    #[test]
    fn replay_identity_gate_rejects_pipe_source_runtime_identity_drift() {
        let fixtures = [
            (
                r#"
| concat(items: List(Int)) -> items
| condition(value: Int) -> length([value] |> concat([1])) > 0
? explore pipe_rule_builtin_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "pipe call `concat` resolves its source form at 1 argument but executes at 2 arguments",
            ),
            (
                r#"
# Built = Build(value: Int)
| condition(value: Int) -> (value |> Build(1)) == Build(value)
? explore pipe_constructor_arity_drift {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "must resolve through pure exploration-supported operations",
            ),
            (
                r#"
| choose(value: Int) -> value > 0
| choose(left: Int, right: Int) -> left < right
| condition(value: Int) -> value |> choose(2)
? explore pipe_rule_overload_drift {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "pipe call `choose` resolves its source form at 1 argument but executes at 2 arguments",
            ),
        ];

        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
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

        let unique_effective_rule = r#"
| choose(left: Int, right: Int) -> left < right
| condition(value: Int) -> value |> choose(2)
? explore unique_pipe_rule {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let unique_artifacts = artifacts(unique_effective_rule);
        assert!(
            unique_artifacts.diagnostics.is_empty(),
            "{:?}",
            unique_artifacts.diagnostics
        );
        assert_eq!(
            unique_artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(1)
        );

        let unique_function_over_builtin = r#"
> abs(value: Int) -> Int { value + 1 }
| condition(value: Int) -> (value |> abs) == value + 1
? explore pipe_function_over_builtin {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let function_artifacts = artifacts(unique_function_over_builtin);
        assert!(
            function_artifacts.diagnostics.is_empty(),
            "{:?}",
            function_artifacts.diagnostics
        );
        assert_eq!(
            function_artifacts.exploration_universes[0]
                .universe
                .cartesian_count_before_constraints,
            ExploreCardinality::Exact(1)
        );

        let non_ground_builtin_collision = r#"
| abs(value: Int) -> value > 0
| condition(value: Int) -> (value |> abs) == value
? explore pipe_rule_default_builtin_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let builtin_collision_artifacts = artifacts(non_ground_builtin_collision);
        assert!(builtin_collision_artifacts.exploration_queries.is_empty());
        assert!(builtin_collision_artifacts.exploration_universes.is_empty());
        assert!(
            builtin_collision_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("executes the built-in intrinsic instead of the exact rule")),
            "missing pipe rule/default-builtin collision: {:?}",
            builtin_collision_artifacts.diagnostics
        );

        let builtin_arity_drift = r#"
| abs(left: Int, right: Int) -> left > right
| condition(value: Int) -> (value |> abs(1)) == True
? explore pipe_builtin_arity_drift {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let builtin_drift_artifacts = artifacts(builtin_arity_drift);
        assert!(builtin_drift_artifacts.exploration_queries.is_empty());
        assert!(builtin_drift_artifacts.exploration_universes.is_empty());
        assert!(
            builtin_drift_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "pipe built-in `abs` is declared for 1 argument but receives 2 arguments"
                )),
            "missing pipe builtin arity drift: {:?}",
            builtin_drift_artifacts.diagnostics
        );

        let interpreter_builtin_collision = r#"
| format_f(value: Int, decimals: Int) -> True
| condition(value: Int) -> (value |> format_f(2)) == True
? explore pipe_interpreter_builtin_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(interpreter_builtin_collision);
        assert!(artifacts.exploration_queries.is_empty());
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("executes the built-in intrinsic instead of the exact rule")),
            "missing interpreter-only builtin collision: {:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn replay_identity_gate_rejects_computed_pipe_transforms() {
        let source = r#"
| condition(value: Int) -> value |> (|item: Int| item > 0)
? explore computed_pipe_transform {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#;
        let artifacts = artifacts(source);
        assert!(artifacts.exploration_queries.is_empty());
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("pure exploration-supported operations")),
            "missing computed-pipe rejection: {:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn replay_identity_gate_rejects_nested_static_declarations() {
        let fixtures = [
            r#"
| condition(value: Int) -> {
    > abs(item: Int) -> Int { item + 1 }
    abs(value) == value + 1
}
? explore nested_function_shadow {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
            r#"
| condition(value: Int) -> {
    # Weird = abs(left: Int, right: Int)
    show(abs(value)) == show(value)
}
? explore nested_constructor_shadow {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
        ];

        for source in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_queries.is_empty());
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("must resolve through pure exploration-supported operations")),
                "missing nested static-declaration rejection: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn replay_identity_gate_rejects_named_argument_pipe_transforms() {
        let source = r#"
> choose(left: Int, right: Int) -> Bool { left < right }
| condition(value: Int) -> value |> choose(right = 2)
? explore named_argument_pipe_transform {
    over condition(value)
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
                diagnostic.message.contains("named argument")
                    || diagnostic.message.contains(NAMED_ARG_MARKER)
            }),
            "missing named-argument pipe rejection: {:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn replay_identity_gate_rejects_calls_through_lexical_values() {
        let fixtures = [
            (
                r#"
> helper(value: Int) -> Int { value + 1 }
> apply(helper: Int -> Int, value: Int) -> Int { helper(value) }
| condition(value: Int) -> apply(|item: Int| item, value) == value
? explore parameter_callable_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "call `helper` resolves through a lexical value",
            ),
            (
                r#"
> helper(value: Int) -> Int { value + 1 }
| condition(value: Int) -> {
    = helper = 0
    helper(value) == value
}
? explore block_callable_collision {
    over condition(value)
    find matches
    bounds { value in [1] }
    output { key [value] representative first }
}
"#,
                "call `helper` resolves through a lexical value",
            ),
            (
                r#"
> helper(value: Int) -> Bool { value > 0 }
| condition(value: Int, helper: Int) -> True
? explore query_callable_collision {
    over condition(value, helper)
    find matches
    bounds {
        value in [1]
        helper = 0
        where helper(value)
    }
    output { key [value] show [helper] representative first }
}
"#,
                "call `helper` resolves through a lexical value",
            ),
        ];

        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_queries.is_empty());
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(expected)
                        || diagnostic
                            .message
                            .contains("must resolve through pure exploration-supported operations")
                        || diagnostic
                            .message
                            .contains("exploration expressions must use only pure")
                }),
                "missing lexical-callee rejection for {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
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
            .boundary_hint()
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
