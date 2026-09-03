//! Query-relative totality proof for mechanism endpoint observers.
//!
//! This is an authorization pass, not an evaluator. It interprets the exact
//! checked FROM/TO closure into a finite abstract domain and proves that the
//! canonical runtime observer returns for every declared Before and After
//! endpoint before admission filtering. Loss of precision is harmless until it reaches a
//! partial operation; at that point the query is rejected before a journal can
//! be created. There is deliberately no concrete fallback in this module.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::mechanism::MechanismSiteId;
use super::relational_endpoint_totality::{
    RelationalEndpointAbstractProofRoot, RelationalEndpointProofDomainRoot, RelationalEndpointRole,
    RelationalEndpointTotalityCertificate, RelationalEndpointTotalityIssue,
    RelationalEndpointTotalityIssueReason, RelationalEndpointTotalityObligationCount,
};
use super::{
    ExploreExactDomain, ExploreFiniteDomainIr, ExploreFiniteTypePlan, ExploreQueryIr,
    ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr, ExploreSuccessorKindIr, ExploreValue,
    MechanismObservationIr, MechanismRequestId, RelationId,
};
use crate::{
    checked_explore_projection_binder_digest, checked_explore_projection_constructor_digest,
    CheckedBinderKind, CheckedBinderSiteId, CheckedCallTarget, CheckedCallableId,
    CheckedConstructorIdentity, CheckedConstructorLayout, CheckedDataTypeId,
    CheckedExploreQuerySites, CheckedExploreSemanticIndex, CheckedExpressionResolution,
    CheckedExpressionType, CheckedFieldResolution, CheckedNamedArgumentOrder, CheckedPatternSiteId,
    CheckedResolutionArtifacts, CheckedRuleCandidateResolution, CheckedTopLevelBindingId,
    CheckedValueBinding, CheckedVariantField, Expr, ExprKind, ExprSiteId, Literal, Pat,
    RuleDispatchKey, RuleDispatchTier, Stmt, Ty, TypeDecl,
};

const PROOF_ROOT_V1: &[u8] = b"futuruna.explore.endpoint-totality.abstract-proof.v1\0";
const PROOF_DOMAIN_ROOT_V1: &[u8] =
    b"futuruna.explore.endpoint-totality.endpoint-proof-domain.v1\0";
const OBLIGATION_V1: &[u8] = b"futuruna.explore.endpoint-totality.obligation.v1\0";
const ABSTRACT_VALUE_V1: &[u8] = b"futuruna.explore.endpoint-totality.abstract-value.v1\0";
const DISPATCH_SCALAR_TERM_V1: &[u8] =
    b"futuruna.explore.endpoint-totality.dispatch-scalar-term.v1\0";
const DISPATCH_FIELD_PROJECTION_V1: &[u8] =
    b"futuruna.explore.endpoint-totality.dispatch-field-projection.v1\0";

// These are proof-format limits. Exhaustion rejects the request as unproved;
// it never narrows the semantic domain and never changes runtime scheduling.
const MAX_ABSTRACT_STEPS: usize = 4_000_000;
const MAX_CALL_DEPTH: usize = 256;
const MAX_EXACT_COLLECTION_ITEMS: usize = 4_096;
const MAX_CONSTRUCTOR_VARIANTS: usize = 512;
const MAX_ABSTRACT_VALUE_NODES: usize = 262_144;
const MAX_ABSTRACT_VALUE_DEPTH: usize = 256;
const MAX_EXACT_STRING_BYTES: usize = 1024 * 1024;
// One machine frame may intentionally retain several independently validated
// values (for example canonical call input plus original arguments).
const MAX_RETAINED_FRAME_VALUE_NODES: usize = MAX_ABSTRACT_VALUE_NODES * 4;
const MAX_RETAINED_FRAME_EXACT_STRING_BYTES: usize = 32 * 1024 * 1024;
// The canonical ground evaluator applies this smaller operand-local boundary
// before structural `==`/`!=`. A certificate must prove that replay cannot
// reach that resource refusal, even when the value is otherwise small enough
// for the abstract domain.
const MAX_RUNTIME_EQUALITY_VALUE_NODES: usize = 512;
const MAX_DISPATCH_BDD_NODES: usize = 65_536;
const MAX_DISPATCH_BDD_CACHE_ENTRIES: usize = MAX_DISPATCH_BDD_NODES * 8;
const MAX_DISPATCH_BDD_WORK_ITEMS: usize = MAX_DISPATCH_BDD_NODES * 24 + 1;
const MAX_DISPATCH_SCALAR_TERM_NODES: usize = MAX_EXACT_COLLECTION_ITEMS;
const MAX_DISPATCH_FIELD_VARIANTS: usize = MAX_CONSTRUCTOR_VARIANTS;
const MAX_DISPATCH_FIELD_METADATA_SEGMENTS: usize = MAX_EXACT_COLLECTION_ITEMS * 4;
const MAX_DISPATCH_FIELD_METADATA_BYTES: usize = MAX_EXACT_STRING_BYTES;
// Aggregate live-proof limits complement the much smaller value/frame limits.
// They bound the graph retained by the prover rather than cumulative work;
// released frames immediately return their leases. The supervised Explore
// child allocator/RSS envelope remains the physical-byte authority.
const MAX_RETAINED_PROOF_VALUE_NODES: usize = 8_388_608;
const MAX_RETAINED_PROOF_EXACT_STRING_BYTES: usize = 512 * 1024 * 1024;
const MAX_RETAINED_PROOF_SLOTS: usize = 8_388_608;
const MAX_RETAINED_PROOF_FRAMES: usize = 65_536;
const MAX_RETAINED_PROOF_CACHE_ENTRIES: usize = 65_536;
const MAX_PROOF_OBLIGATIONS: usize = 262_144;
// Until abstract values become interned IDs, continuations may still own
// nontrivial values. Keep this heap boundary deliberately conservative for
// ordinary developer machines; exhaustion is a typed refusal, never a panic.
const MAX_EVALUATION_CONTINUATIONS: usize = 16_384;
const MAX_EVALUATION_TRANSITIONS: usize = 16_000_000;

const BINDER_PARAMETER: u32 = 0;
const BINDER_PATTERN: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IntInterval {
    minimum: i128,
    maximum: i128,
}

impl IntInterval {
    fn singleton(value: i64) -> Self {
        Self {
            minimum: i128::from(value),
            maximum: i128::from(value),
        }
    }

    fn new(minimum: i128, maximum: i128) -> Option<Self> {
        (minimum <= maximum).then_some(Self { minimum, maximum })
    }

    fn runtime_int(self) -> Option<Self> {
        (self.minimum >= i128::from(i64::MIN) && self.maximum <= i128::from(i64::MAX))
            .then_some(self)
    }

    fn singleton_value(self) -> Option<i64> {
        (self.minimum == self.maximum)
            .then(|| i64::try_from(self.minimum).ok())
            .flatten()
    }

    fn contains(self, value: i128) -> bool {
        self.minimum <= value && value <= self.maximum
    }

    fn hull(self, other: Self) -> Self {
        Self {
            minimum: self.minimum.min(other.minimum),
            maximum: self.maximum.max(other.maximum),
        }
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Self::new(
            self.minimum.checked_add(other.minimum)?,
            self.maximum.checked_add(other.maximum)?,
        )?
        .runtime_int()
    }

    fn checked_sub(self, other: Self) -> Option<Self> {
        Self::new(
            self.minimum.checked_sub(other.maximum)?,
            self.maximum.checked_sub(other.minimum)?,
        )?
        .runtime_int()
    }

    fn checked_mul(self, other: Self) -> Option<Self> {
        let products = [
            self.minimum.checked_mul(other.minimum)?,
            self.minimum.checked_mul(other.maximum)?,
            self.maximum.checked_mul(other.minimum)?,
            self.maximum.checked_mul(other.maximum)?,
        ];
        Self::new(*products.iter().min()?, *products.iter().max()?)?.runtime_int()
    }

    fn checked_neg(self) -> Option<Self> {
        Self::new(self.maximum.checked_neg()?, self.minimum.checked_neg()?)?.runtime_int()
    }

    fn checked_div(self, other: Self) -> Option<Self> {
        if other.contains(0) || (self.contains(i128::from(i64::MIN)) && other.contains(-1)) {
            return None;
        }
        let quotients = [
            self.minimum.checked_div(other.minimum)?,
            self.minimum.checked_div(other.maximum)?,
            self.maximum.checked_div(other.minimum)?,
            self.maximum.checked_div(other.maximum)?,
        ];
        Self::new(*quotients.iter().min()?, *quotients.iter().max()?)?.runtime_int()
    }

    fn checked_rem(self, other: Self) -> Option<Self> {
        if other.contains(0) || (self.contains(i128::from(i64::MIN)) && other.contains(-1)) {
            return None;
        }
        let maximum_abs_divisor = [other.minimum, other.maximum]
            .into_iter()
            .map(i128::checked_abs)
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .max()?;
        let magnitude = maximum_abs_divisor.checked_sub(1)?;
        let (minimum, maximum) = if self.maximum < 0 {
            (-magnitude, 0)
        } else if self.minimum > 0 {
            (0, magnitude)
        } else {
            (-magnitude, magnitude)
        };
        Self::new(minimum, maximum)?.runtime_int()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TruthDomain(u8);

impl TruthDomain {
    const FALSE: Self = Self(0b01);
    const TRUE: Self = Self(0b10);
    const BOTH: Self = Self(0b11);

    fn from_bool(value: bool) -> Self {
        if value {
            Self::TRUE
        } else {
            Self::FALSE
        }
    }

    fn may_be_false(self) -> bool {
        self.0 & Self::FALSE.0 != 0
    }

    fn may_be_true(self) -> bool {
        self.0 & Self::TRUE.0 != 0
    }

    fn singleton(self) -> Option<bool> {
        match self {
            Self::FALSE => Some(false),
            Self::TRUE => Some(true),
            _ => None,
        }
    }

    fn join(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn not(self) -> Self {
        Self(((self.0 & 0b01) << 1) | ((self.0 & 0b10) >> 1))
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AbstractConstructor {
    identity: CheckedConstructorIdentity,
    fields: Box<[AbstractValue]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AbstractSequence {
    Exact(Box<[AbstractValue]>),
    Summary {
        element: Box<AbstractValue>,
        minimum_length: u64,
        maximum_length: u64,
    },
}

impl AbstractSequence {
    fn lengths(&self) -> (u64, u64) {
        match self {
            Self::Exact(values) => (values.len() as u64, values.len() as u64),
            Self::Summary {
                minimum_length,
                maximum_length,
                ..
            } => (*minimum_length, *maximum_length),
        }
    }

    fn joined_element(&self) -> Option<AbstractValue> {
        match self {
            Self::Exact(values) => join_values(values.iter().cloned()),
            Self::Summary { element, .. } => Some((**element).clone()),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AbstractCallable {
    Function(CheckedCallableId),
    RuleFamily(RuleDispatchKey),
    Lambda {
        body_site: ExprSiteId,
        parameters: Box<[CheckedBinderSiteId]>,
        captured: Arc<[(CheckedBinderSiteId, AbstractValue)]>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AbstractValue {
    Unreachable,
    Unknown,
    Int(IntInterval),
    Bool(TruthDomain),
    Float(Option<u64>),
    String(Option<Box<str>>),
    Character(Option<char>),
    Unit,
    Constructors(BTreeMap<[u8; 32], AbstractConstructor>),
    List(AbstractSequence),
    Set(AbstractSequence),
    Map(Box<[(AbstractValue, AbstractValue)]>),
    Tuple(Box<[AbstractValue]>),
    Callable(AbstractCallable),
}

#[derive(Clone, Copy, Debug, Default)]
struct AbstractValueShapeBounds {
    nodes: usize,
    depth: usize,
    exact_string_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProofRetentionTotals {
    nodes: usize,
    exact_string_bytes: usize,
    slots: usize,
    frames: usize,
    cache_entries: usize,
}

#[derive(Debug, Default)]
struct ProofRetentionLedger {
    totals: Cell<ProofRetentionTotals>,
}

impl ProofRetentionLedger {
    fn try_reserve(
        &self,
        nodes: usize,
        exact_string_bytes: usize,
        slots: usize,
        frames: usize,
        cache_entries: usize,
    ) -> bool {
        let current = self.totals.get();
        let Some(next) = (|| {
            Some(ProofRetentionTotals {
                nodes: current.nodes.checked_add(nodes)?,
                exact_string_bytes: current.exact_string_bytes.checked_add(exact_string_bytes)?,
                slots: current.slots.checked_add(slots)?,
                frames: current.frames.checked_add(frames)?,
                cache_entries: current.cache_entries.checked_add(cache_entries)?,
            })
        })() else {
            return false;
        };
        if next.nodes > MAX_RETAINED_PROOF_VALUE_NODES
            || next.exact_string_bytes > MAX_RETAINED_PROOF_EXACT_STRING_BYTES
            || next.slots > MAX_RETAINED_PROOF_SLOTS
            || next.frames > MAX_RETAINED_PROOF_FRAMES
            || next.cache_entries > MAX_RETAINED_PROOF_CACHE_ENTRIES
        {
            return false;
        }
        self.totals.set(next);
        true
    }

    fn release(
        &self,
        nodes: usize,
        exact_string_bytes: usize,
        slots: usize,
        frames: usize,
        cache_entries: usize,
    ) {
        let current = self.totals.get();
        self.totals.set(ProofRetentionTotals {
            nodes: current
                .nodes
                .checked_sub(nodes)
                .expect("proof-retention node lease balances"),
            exact_string_bytes: current
                .exact_string_bytes
                .checked_sub(exact_string_bytes)
                .expect("proof-retention string lease balances"),
            slots: current
                .slots
                .checked_sub(slots)
                .expect("proof-retention slot lease balances"),
            frames: current
                .frames
                .checked_sub(frames)
                .expect("proof-retention frame lease balances"),
            cache_entries: current
                .cache_entries
                .checked_sub(cache_entries)
                .expect("proof-retention cache lease balances"),
        });
    }
}

#[derive(Debug, Default)]
struct RetainedValueBudget {
    nodes: usize,
    exact_string_bytes: usize,
    slots: usize,
    ledger: Option<Rc<ProofRetentionLedger>>,
}

// This budget follows monotonically growing stores while they are being
// constructed. Stable inputs and replacement slots are checked separately as
// complete abstract values; charging replacements cumulatively would make a
// long fold fail because of work already released rather than memory retained.
impl RetainedValueBudget {
    fn attached(ledger: Rc<ProofRetentionLedger>) -> Option<Self> {
        ledger.try_reserve(0, 0, 0, 1, 0).then_some(Self {
            nodes: 0,
            exact_string_bytes: 0,
            slots: 0,
            ledger: Some(ledger),
        })
    }

    fn try_retain(&mut self, bounds: AbstractValueShapeBounds) -> bool {
        let Some(nodes) = self.nodes.checked_add(bounds.nodes) else {
            return false;
        };
        let Some(exact_string_bytes) = self
            .exact_string_bytes
            .checked_add(bounds.exact_string_bytes)
        else {
            return false;
        };
        if nodes > MAX_RETAINED_FRAME_VALUE_NODES
            || exact_string_bytes > MAX_RETAINED_FRAME_EXACT_STRING_BYTES
        {
            return false;
        }
        if self.ledger.as_ref().is_some_and(|ledger| {
            !ledger.try_reserve(bounds.nodes, bounds.exact_string_bytes, 0, 0, 0)
        }) {
            return false;
        }
        self.nodes = nodes;
        self.exact_string_bytes = exact_string_bytes;
        true
    }

    fn try_retain_slots(&mut self, slots: usize) -> bool {
        let Some(total) = self.slots.checked_add(slots) else {
            return false;
        };
        if total > MAX_RETAINED_FRAME_VALUE_NODES {
            return false;
        }
        if self
            .ledger
            .as_ref()
            .is_some_and(|ledger| !ledger.try_reserve(0, 0, slots, 0, 0))
        {
            return false;
        }
        self.slots = total;
        true
    }

    fn release_value(&mut self, bounds: AbstractValueShapeBounds) {
        self.nodes = self
            .nodes
            .checked_sub(bounds.nodes)
            .expect("retained frame node lease balances");
        self.exact_string_bytes = self
            .exact_string_bytes
            .checked_sub(bounds.exact_string_bytes)
            .expect("retained frame string lease balances");
        if let Some(ledger) = &self.ledger {
            ledger.release(bounds.nodes, bounds.exact_string_bytes, 0, 0, 0);
        }
    }

    #[cfg(test)]
    fn checked_retaining(mut self, bounds: AbstractValueShapeBounds) -> Option<Self> {
        self.try_retain(bounds).then_some(self)
    }
}

impl Drop for RetainedValueBudget {
    fn drop(&mut self) {
        if let Some(ledger) = &self.ledger {
            ledger.release(self.nodes, self.exact_string_bytes, self.slots, 1, 0);
        }
    }
}

fn checked_data_type_identity_retained_bytes(identity: &CheckedDataTypeId) -> usize {
    match identity {
        CheckedDataTypeId::Intrinsic { canonical_name } => canonical_name.len(),
        CheckedDataTypeId::Declared(owner) => {
            let declaration = &owner.declaration;
            declaration
                .module
                .interface_hash
                .len()
                .saturating_add(
                    declaration
                        .module
                        .internal_path
                        .iter()
                        .map(String::len)
                        .fold(0_usize, usize::saturating_add),
                )
                .saturating_add(declaration.owner.as_deref().map_or(0, str::len))
                .saturating_add(declaration.name.len())
        }
    }
}

fn constructor_identity_retained_bytes(identity: &CheckedConstructorIdentity) -> usize {
    identity
        .owner_type
        .len()
        .saturating_add(identity.variant.len())
        .saturating_add(checked_data_type_identity_retained_bytes(&identity.owner))
        .saturating_add(identity.fields.iter().fold(0_usize, |total, field| {
            total
                .saturating_add(field.name.len())
                .saturating_add(checked_data_type_identity_retained_bytes(&field.owner))
        }))
}

impl AbstractValue {
    fn int(&self) -> Option<IntInterval> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    fn truth(&self) -> Option<TruthDomain> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable)
    }

    fn shape_bounds(&self) -> AbstractValueShapeBounds {
        let mut pending = vec![(self, 1_usize, true)];
        let mut counted_captures = BTreeSet::new();
        let mut deepest_capture_visit = BTreeMap::new();
        let mut nodes = 0_usize;
        let mut maximum_depth = 0_usize;
        let mut exact_string_bytes = 0_usize;
        while let Some((value, depth, count_node)) = pending.pop() {
            if count_node {
                nodes = nodes.saturating_add(1);
                match value {
                    Self::String(Some(value)) => {
                        exact_string_bytes = exact_string_bytes.saturating_add(value.len());
                    }
                    Self::Constructors(variants) => {
                        // Each alternative is a separately owned BTreeMap
                        // entry with a deep constructor identity, even though
                        // only one alternative can materialize at runtime.
                        nodes = nodes.saturating_add(variants.len());
                        for variant in variants.values() {
                            exact_string_bytes = exact_string_bytes.saturating_add(
                                constructor_identity_retained_bytes(&variant.identity),
                            );
                        }
                    }
                    _ => {}
                }
            }
            maximum_depth = maximum_depth.max(depth);
            if nodes > MAX_ABSTRACT_VALUE_NODES
                || maximum_depth > MAX_ABSTRACT_VALUE_DEPTH
                || exact_string_bytes > MAX_RETAINED_FRAME_EXACT_STRING_BYTES
            {
                return AbstractValueShapeBounds {
                    nodes,
                    depth: maximum_depth,
                    exact_string_bytes,
                };
            }
            match value {
                Self::Constructors(variants) => {
                    for child in variants
                        .values()
                        .rev()
                        .flat_map(|variant| variant.fields.iter().rev())
                    {
                        pending.push((child, depth + 1, count_node));
                    }
                }
                Self::List(AbstractSequence::Exact(values))
                | Self::Set(AbstractSequence::Exact(values))
                | Self::Tuple(values) => {
                    for child in values.iter().rev() {
                        pending.push((child, depth + 1, count_node));
                    }
                }
                Self::List(AbstractSequence::Summary { element, .. })
                | Self::Set(AbstractSequence::Summary { element, .. }) => {
                    pending.push((element, depth + 1, count_node))
                }
                Self::Map(entries) => {
                    for (key, value) in entries.iter().rev() {
                        pending.push((value, depth + 1, count_node));
                        pending.push((key, depth + 1, count_node));
                    }
                }
                Self::Callable(AbstractCallable::Lambda { captured, .. }) => {
                    let identity = Arc::as_ptr(captured) as *const () as usize;
                    let prior_depth = deepest_capture_visit.get(&identity).copied().unwrap_or(0);
                    if depth > prior_depth {
                        deepest_capture_visit.insert(identity, depth);
                        let count_children = counted_captures.insert(identity);
                        for (_, child) in captured.iter().rev() {
                            pending.push((child, depth + 1, count_children));
                        }
                    }
                }
                _ => {}
            }
        }
        AbstractValueShapeBounds {
            nodes,
            depth: maximum_depth,
            exact_string_bytes,
        }
    }

    /// Worst-case number of runtime `Value` nodes materialized by any one
    /// concrete value represented here, capped one past the evaluator limit.
    ///
    /// This deliberately differs from `shape_bounds`: an abstract summary
    /// stores one joined element, while replay can materialize that element
    /// `maximum_length` times; canonical Explore lists additionally lower to
    /// a Cons/Nil chain.
    fn runtime_equality_materialization_nodes(&self) -> Option<usize> {
        const OVER_LIMIT: usize = MAX_RUNTIME_EQUALITY_VALUE_NODES + 1;

        fn add(left: usize, right: usize) -> usize {
            left.saturating_add(right).min(OVER_LIMIT)
        }

        fn multiply(left: usize, right: usize) -> usize {
            left.saturating_mul(right).min(OVER_LIMIT)
        }

        fn sequence_sum<'a>(
            initial: usize,
            values: impl IntoIterator<Item = &'a AbstractValue>,
        ) -> Option<usize> {
            values.into_iter().try_fold(initial, |total, value| {
                value
                    .runtime_equality_materialization_nodes()
                    .map(|nodes| add(total, nodes))
            })
        }

        match self {
            // No concrete value is represented, so this branch cannot make a
            // runtime equality invocation larger.
            Self::Unreachable => Some(0),
            // Unknown can represent an arbitrarily large runtime value.
            Self::Unknown => None,
            Self::Int(_)
            | Self::Bool(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Character(_)
            | Self::Unit
            | Self::Callable(_) => Some(1),
            Self::Constructors(variants) => variants
                .values()
                .map(|variant| sequence_sum(1, variant.fields.iter()))
                .try_fold(0, |maximum, nodes| nodes.map(|nodes| maximum.max(nodes))),
            Self::List(AbstractSequence::Exact(values)) => {
                // One Nil plus one Cons per item, as well as every item value.
                sequence_sum(add(1, values.len().min(OVER_LIMIT)), values.iter())
            }
            Self::List(AbstractSequence::Summary {
                element,
                maximum_length,
                ..
            }) => {
                let length = usize::try_from(*maximum_length)
                    .unwrap_or(OVER_LIMIT)
                    .min(OVER_LIMIT);
                if length == 0 {
                    return Some(1);
                }
                let element_nodes = element.runtime_equality_materialization_nodes()?;
                Some(add(add(1, length), multiply(length, element_nodes)))
            }
            Self::Set(AbstractSequence::Exact(values)) | Self::Tuple(values) => {
                sequence_sum(1, values.iter())
            }
            Self::Set(AbstractSequence::Summary {
                element,
                maximum_length,
                ..
            }) => {
                let length = usize::try_from(*maximum_length)
                    .unwrap_or(OVER_LIMIT)
                    .min(OVER_LIMIT);
                if length == 0 {
                    return Some(1);
                }
                let element_nodes = element.runtime_equality_materialization_nodes()?;
                Some(add(1, multiply(length, element_nodes)))
            }
            Self::Map(entries) => entries.iter().try_fold(1, |total, (key, value)| {
                let key_nodes = key.runtime_equality_materialization_nodes()?;
                let value_nodes = value.runtime_equality_materialization_nodes()?;
                Some(add(add(total, key_nodes), value_nodes))
            }),
        }
    }
}

type AbstractEnv = BTreeMap<CheckedBinderSiteId, AbstractValue>;

struct BudgetedAbstractEnv {
    bindings: AbstractEnv,
    _retained: RetainedValueBudget,
}

impl Deref for BudgetedAbstractEnv {
    type Target = AbstractEnv;

    fn deref(&self) -> &Self::Target {
        &self.bindings
    }
}

type SharedAbstractEnv = Arc<BudgetedAbstractEnv>;

struct CacheEntryLease(Rc<ProofRetentionLedger>);

impl CacheEntryLease {
    fn try_new(ledger: Rc<ProofRetentionLedger>) -> Option<Self> {
        ledger.try_reserve(0, 0, 0, 0, 1).then_some(Self(ledger))
    }
}

impl Drop for CacheEntryLease {
    fn drop(&mut self) {
        self.0.release(0, 0, 0, 0, 1);
    }
}

struct CachedAbstractValue {
    value: AbstractValue,
    _retained: RetainedValueBudget,
    _entry: CacheEntryLease,
}

struct BudgetedAbstractValue {
    value: AbstractValue,
    _retained: RetainedValueBudget,
}

impl BudgetedAbstractValue {
    fn into_parts(self) -> (AbstractValue, RetainedValueBudget) {
        (self.value, self._retained)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ActiveDefinition {
    Callable(CheckedCallableId),
    Rule(RuleDispatchKey),
    TopLevel(CheckedTopLevelBindingId),
    Lambda(ExprSiteId),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ObligationKind {
    Endpoint,
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Remainder,
    Negation,
    Index,
    Equality,
    Dispatch,
    Callable,
    Collection,
    Match,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProofObligation {
    role: RelationalEndpointRole,
    site: [u8; 32],
    kind: ObligationKind,
    input_root: [u8; 32],
    result_root: [u8; 32],
}

#[derive(Clone, Debug)]
struct PatternPartition {
    matched: AbstractValue,
    unmatched: AbstractValue,
    definitely_all: bool,
}

#[derive(Clone, Debug)]
struct AbstractIntPlace {
    binder: CheckedBinderSiteId,
    fields: Vec<Box<[CheckedVariantField]>>,
}

impl PatternPartition {
    fn all(value: AbstractValue) -> Self {
        Self {
            matched: value,
            unmatched: AbstractValue::Unreachable,
            definitely_all: true,
        }
    }

    fn none(value: AbstractValue) -> Self {
        Self {
            matched: AbstractValue::Unreachable,
            unmatched: value,
            definitely_all: false,
        }
    }

    fn overlapping(value: AbstractValue) -> Self {
        Self {
            matched: value.clone(),
            unmatched: value,
            definitely_all: false,
        }
    }

    fn may_match(&self) -> bool {
        !self.matched.is_unreachable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeadMatch {
    No,
    Yes,
    Maybe,
}

/// Candidate-local rule-head binders are deliberately absent from this term
/// language. Every such binder is substituted with its canonical dispatch
/// argument slot before a predicate atom is minted, allowing independently
/// authored candidates to prove that their guards are complementary.
type DispatchScalarTermId = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DispatchArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DispatchComparisonOperator {
    Equal,
    Less,
    LessOrEqual,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DispatchPredicateAtom {
    operator: DispatchComparisonOperator,
    left: DispatchScalarTermId,
    right: DispatchScalarTermId,
}

type DispatchArgumentOrigins = BTreeMap<[u8; 32], DispatchScalarTermId>;
type DispatchBddId = usize;

#[derive(Clone, Copy, Debug)]
enum DispatchBddNode {
    Terminal,
    Branch {
        atom: DispatchPredicateAtom,
        when_false: DispatchBddId,
        when_true: DispatchBddId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchBddError {
    NodeLimit,
    CacheLimit,
    RetentionLimit,
    WorkLimit,
    ScalarTermLimit,
    FieldVariantLimit,
    FieldMetadataLimit,
    InvalidState,
}

impl DispatchBddError {
    fn detail(self) -> Box<str> {
        match self {
            Self::NodeLimit => format!(
                "dispatch predicate BDD exceeds {MAX_DISPATCH_BDD_NODES} canonical nodes"
            )
            .into_boxed_str(),
            Self::CacheLimit => format!(
                "dispatch predicate BDD exceeds {MAX_DISPATCH_BDD_CACHE_ENTRIES} retained cache entries"
            )
            .into_boxed_str(),
            Self::RetentionLimit => format!(
                "dispatch predicate BDD exceeds the aggregate proof-retention limit of {MAX_RETAINED_PROOF_SLOTS} structural slots"
            )
            .into_boxed_str(),
            Self::WorkLimit => format!(
                "dispatch predicate BDD exceeds {MAX_DISPATCH_BDD_WORK_ITEMS} bounded work items"
            )
            .into_boxed_str(),
            Self::ScalarTermLimit => format!(
                "dispatch scalar term exceeds {MAX_DISPATCH_SCALAR_TERM_NODES} canonical nodes"
            )
            .into_boxed_str(),
            Self::FieldVariantLimit => format!(
                "dispatch field projection exceeds {MAX_DISPATCH_FIELD_VARIANTS} checked variants"
            )
            .into_boxed_str(),
            Self::FieldMetadataLimit => format!(
                "dispatch field projection exceeds {MAX_DISPATCH_FIELD_METADATA_SEGMENTS} metadata segments or {MAX_DISPATCH_FIELD_METADATA_BYTES} metadata bytes"
            )
            .into_boxed_str(),
            Self::InvalidState =>
                "dispatch predicate BDD contains an invalid internal node reference".into(),
        }
    }
}

#[derive(Debug, Default)]
struct DispatchCanonicalizationBudget {
    scalar_nodes: usize,
    field_metadata_segments: usize,
    field_metadata_bytes: usize,
}

impl DispatchCanonicalizationBudget {
    fn charge_scalar_node(&mut self) -> Result<(), DispatchBddError> {
        self.scalar_nodes = self
            .scalar_nodes
            .checked_add(1)
            .ok_or(DispatchBddError::ScalarTermLimit)?;
        if self.scalar_nodes > MAX_DISPATCH_SCALAR_TERM_NODES {
            return Err(DispatchBddError::ScalarTermLimit);
        }
        Ok(())
    }

    fn hash_field_segment(
        &mut self,
        hasher: &mut Sha256,
        bytes: &[u8],
    ) -> Result<(), DispatchBddError> {
        let segments = self
            .field_metadata_segments
            .checked_add(1)
            .ok_or(DispatchBddError::FieldMetadataLimit)?;
        let byte_count = self
            .field_metadata_bytes
            .checked_add(bytes.len())
            .ok_or(DispatchBddError::FieldMetadataLimit)?;
        if segments > MAX_DISPATCH_FIELD_METADATA_SEGMENTS
            || byte_count > MAX_DISPATCH_FIELD_METADATA_BYTES
        {
            return Err(DispatchBddError::FieldMetadataLimit);
        }
        self.field_metadata_segments = segments;
        self.field_metadata_bytes = byte_count;
        hash_segment(hasher, bytes);
        Ok(())
    }
}

/// Slot-only RAII accounting for fixed-size dispatch records. The BDD owns one
/// lease for its durable stores and each Boolean operation owns a short-lived
/// sibling lease for work frames. Reservations always precede allocation.
#[derive(Debug)]
struct DispatchRetentionLease {
    ledger: Rc<ProofRetentionLedger>,
    slots: usize,
}

impl DispatchRetentionLease {
    fn empty(ledger: Rc<ProofRetentionLedger>) -> Self {
        Self { ledger, slots: 0 }
    }

    fn sibling(&self) -> Self {
        Self::empty(Rc::clone(&self.ledger))
    }

    fn try_grow(&mut self, slots: usize) -> bool {
        let Some(total) = self.slots.checked_add(slots) else {
            return false;
        };
        if !self.ledger.try_reserve(0, 0, slots, 0, 0) {
            return false;
        }
        self.slots = total;
        true
    }
}

impl Drop for DispatchRetentionLease {
    fn drop(&mut self) {
        self.ledger.release(0, 0, self.slots, 0, 0);
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DispatchBddBinaryOperator {
    And,
    Or,
}

/// A small request-local reduced ordered BDD. The terminals are fixed at 0
/// (empty) and 1 (all). Unknown/noncanonical predicates never enter the BDD;
/// callers retain their previous residual domain instead of weakening it.
struct DispatchPredicateBdd {
    nodes: Vec<DispatchBddNode>,
    unique: BTreeMap<(DispatchPredicateAtom, DispatchBddId, DispatchBddId), DispatchBddId>,
    negation_cache: BTreeMap<DispatchBddId, DispatchBddId>,
    binary_cache:
        BTreeMap<(DispatchBddBinaryOperator, DispatchBddId, DispatchBddId), DispatchBddId>,
    // Keep this field last so the stores are dropped before their lease.
    retained: DispatchRetentionLease,
}

impl DispatchPredicateBdd {
    const EMPTY: DispatchBddId = 0;
    const ALL: DispatchBddId = 1;

    fn new(ledger: Rc<ProofRetentionLedger>) -> Result<Self, DispatchBddError> {
        let mut retained = DispatchRetentionLease::empty(ledger);
        if !retained.try_grow(4) {
            return Err(DispatchBddError::RetentionLimit);
        }
        Ok(Self {
            nodes: vec![DispatchBddNode::Terminal, DispatchBddNode::Terminal],
            unique: BTreeMap::new(),
            negation_cache: BTreeMap::from([(Self::EMPTY, Self::ALL), (Self::ALL, Self::EMPTY)]),
            binary_cache: BTreeMap::new(),
            retained,
        })
    }

    fn retained_slots(&self) -> usize {
        self.nodes
            .len()
            .saturating_add(self.unique.len())
            .saturating_add(self.negation_cache.len())
            .saturating_add(self.binary_cache.len())
    }

    fn atom(&mut self, atom: DispatchPredicateAtom) -> Result<DispatchBddId, DispatchBddError> {
        self.branch(atom, Self::EMPTY, Self::ALL)
    }

    fn branch(
        &mut self,
        atom: DispatchPredicateAtom,
        when_false: DispatchBddId,
        when_true: DispatchBddId,
    ) -> Result<DispatchBddId, DispatchBddError> {
        if when_false == when_true {
            return Ok(when_false);
        }
        let key = (atom, when_false, when_true);
        if let Some(existing) = self.unique.get(&key).copied() {
            return Ok(existing);
        }
        if self.nodes.len() >= MAX_DISPATCH_BDD_NODES {
            return Err(DispatchBddError::NodeLimit);
        }
        if !self.retained.try_grow(2) {
            return Err(DispatchBddError::RetentionLimit);
        }
        let id = self.nodes.len();
        self.nodes.push(DispatchBddNode::Branch {
            atom,
            when_false,
            when_true,
        });
        self.unique.insert(key, id);
        debug_assert_eq!(self.retained.slots, self.retained_slots());
        Ok(id)
    }

    fn negate(&mut self, node: DispatchBddId) -> Result<DispatchBddId, DispatchBddError> {
        enum Frame {
            Visit(DispatchBddId),
            Assemble {
                node: DispatchBddId,
                atom: DispatchPredicateAtom,
                when_false: DispatchBddId,
                when_true: DispatchBddId,
            },
        }

        let mut work = self.retained.sibling();
        let mut frames = Vec::new();
        Self::push_work(&mut frames, &mut work, Frame::Visit(node))?;
        let mut steps = 0_usize;
        while let Some(frame) = frames.pop() {
            steps = steps.checked_add(1).ok_or(DispatchBddError::WorkLimit)?;
            if steps > MAX_DISPATCH_BDD_NODES.saturating_mul(4) {
                return Err(DispatchBddError::WorkLimit);
            }
            match frame {
                Frame::Visit(node) => {
                    if self.negation_cache.contains_key(&node) {
                        continue;
                    }
                    let DispatchBddNode::Branch {
                        atom,
                        when_false,
                        when_true,
                    } = self
                        .nodes
                        .get(node)
                        .copied()
                        .ok_or(DispatchBddError::InvalidState)?
                    else {
                        return Err(DispatchBddError::InvalidState);
                    };
                    Self::push_work(
                        &mut frames,
                        &mut work,
                        Frame::Assemble {
                            node,
                            atom,
                            when_false,
                            when_true,
                        },
                    )?;
                    Self::push_work(&mut frames, &mut work, Frame::Visit(when_true))?;
                    Self::push_work(&mut frames, &mut work, Frame::Visit(when_false))?;
                }
                Frame::Assemble {
                    node,
                    atom,
                    when_false,
                    when_true,
                } => {
                    if self.negation_cache.contains_key(&node) {
                        continue;
                    }
                    let when_false = self
                        .negation_cache
                        .get(&when_false)
                        .copied()
                        .ok_or(DispatchBddError::InvalidState)?;
                    let when_true = self
                        .negation_cache
                        .get(&when_true)
                        .copied()
                        .ok_or(DispatchBddError::InvalidState)?;
                    let negated = self.branch(atom, when_false, when_true)?;
                    self.insert_negation_pair(node, negated)?;
                }
            }
        }
        self.negation_cache
            .get(&node)
            .copied()
            .ok_or(DispatchBddError::InvalidState)
    }

    fn and(
        &mut self,
        left: DispatchBddId,
        right: DispatchBddId,
    ) -> Result<DispatchBddId, DispatchBddError> {
        self.binary(DispatchBddBinaryOperator::And, left, right)
    }

    fn or(
        &mut self,
        left: DispatchBddId,
        right: DispatchBddId,
    ) -> Result<DispatchBddId, DispatchBddError> {
        self.binary(DispatchBddBinaryOperator::Or, left, right)
    }

    fn binary(
        &mut self,
        operator: DispatchBddBinaryOperator,
        left: DispatchBddId,
        right: DispatchBddId,
    ) -> Result<DispatchBddId, DispatchBddError> {
        enum Frame {
            Visit {
                left: DispatchBddId,
                right: DispatchBddId,
            },
            Assemble {
                left: DispatchBddId,
                right: DispatchBddId,
                atom: DispatchPredicateAtom,
                left_false: DispatchBddId,
                left_true: DispatchBddId,
                right_false: DispatchBddId,
                right_true: DispatchBddId,
            },
        }

        let (left, right) = Self::canonical_binary_operands(left, right);
        if let Some(result) = self.known_binary_result(operator, left, right) {
            return Ok(result);
        }
        let mut work = self.retained.sibling();
        let mut frames = Vec::new();
        Self::push_work(&mut frames, &mut work, Frame::Visit { left, right })?;
        let mut steps = 0_usize;
        while let Some(frame) = frames.pop() {
            steps = steps.checked_add(1).ok_or(DispatchBddError::WorkLimit)?;
            if steps > MAX_DISPATCH_BDD_NODES.saturating_mul(8) {
                return Err(DispatchBddError::WorkLimit);
            }
            match frame {
                Frame::Visit { left, right } => {
                    let (left, right) = Self::canonical_binary_operands(left, right);
                    if self.known_binary_result(operator, left, right).is_some() {
                        continue;
                    }
                    let left_atom = self.node_atom(left);
                    let right_atom = self.node_atom(right);
                    let atom = match (left_atom, right_atom) {
                        (Some(left), Some(right)) => left.min(right),
                        (Some(left), None) => left,
                        (None, Some(right)) => right,
                        (None, None) => return Err(DispatchBddError::InvalidState),
                    };
                    let (left_false, left_true) = self.cofactors(left, atom)?;
                    let (right_false, right_true) = self.cofactors(right, atom)?;
                    Self::push_work(
                        &mut frames,
                        &mut work,
                        Frame::Assemble {
                            left,
                            right,
                            atom,
                            left_false,
                            left_true,
                            right_false,
                            right_true,
                        },
                    )?;
                    Self::push_work(
                        &mut frames,
                        &mut work,
                        Frame::Visit {
                            left: left_true,
                            right: right_true,
                        },
                    )?;
                    Self::push_work(
                        &mut frames,
                        &mut work,
                        Frame::Visit {
                            left: left_false,
                            right: right_false,
                        },
                    )?;
                }
                Frame::Assemble {
                    left,
                    right,
                    atom,
                    left_false,
                    left_true,
                    right_false,
                    right_true,
                } => {
                    if self.known_binary_result(operator, left, right).is_some() {
                        continue;
                    }
                    let (left_false, right_false) =
                        Self::canonical_binary_operands(left_false, right_false);
                    let (left_true, right_true) =
                        Self::canonical_binary_operands(left_true, right_true);
                    let when_false = self
                        .known_binary_result(operator, left_false, right_false)
                        .ok_or(DispatchBddError::InvalidState)?;
                    let when_true = self
                        .known_binary_result(operator, left_true, right_true)
                        .ok_or(DispatchBddError::InvalidState)?;
                    let result = self.branch(atom, when_false, when_true)?;
                    self.insert_binary_cache((operator, left, right), result)?;
                }
            }
        }
        self.known_binary_result(operator, left, right)
            .ok_or(DispatchBddError::InvalidState)
    }

    fn canonical_binary_operands(
        mut left: DispatchBddId,
        mut right: DispatchBddId,
    ) -> (DispatchBddId, DispatchBddId) {
        if right < left {
            std::mem::swap(&mut left, &mut right);
        }
        (left, right)
    }

    fn known_binary_result(
        &self,
        operator: DispatchBddBinaryOperator,
        left: DispatchBddId,
        right: DispatchBddId,
    ) -> Option<DispatchBddId> {
        let terminal = match operator {
            DispatchBddBinaryOperator::And => match (left, right) {
                (Self::EMPTY, _) => Some(Self::EMPTY),
                (Self::ALL, value) => Some(value),
                (left, right) if left == right => Some(left),
                _ => None,
            },
            DispatchBddBinaryOperator::Or => match (left, right) {
                (Self::ALL, _) => Some(Self::ALL),
                (Self::EMPTY, value) => Some(value),
                (left, right) if left == right => Some(left),
                _ => None,
            },
        };
        terminal.or_else(|| self.binary_cache.get(&(operator, left, right)).copied())
    }

    fn node_atom(&self, node: DispatchBddId) -> Option<DispatchPredicateAtom> {
        match self.nodes.get(node)? {
            DispatchBddNode::Terminal => None,
            DispatchBddNode::Branch { atom, .. } => Some(*atom),
        }
    }

    fn cofactors(
        &self,
        node: DispatchBddId,
        atom: DispatchPredicateAtom,
    ) -> Result<(DispatchBddId, DispatchBddId), DispatchBddError> {
        match self.nodes.get(node).ok_or(DispatchBddError::InvalidState)? {
            DispatchBddNode::Branch {
                atom: candidate,
                when_false,
                when_true,
            } if *candidate == atom => Ok((*when_false, *when_true)),
            _ => Ok((node, node)),
        }
    }

    fn push_work<T>(
        frames: &mut Vec<T>,
        retained: &mut DispatchRetentionLease,
        frame: T,
    ) -> Result<(), DispatchBddError> {
        if frames.len() >= MAX_DISPATCH_BDD_WORK_ITEMS {
            return Err(DispatchBddError::WorkLimit);
        }
        // The Vec retains its high-water storage when frames are popped. Lease
        // that maximum live length once; later pushes into an already reached
        // depth reuse the same bounded storage instead of charging cumulative
        // work as though it were still resident.
        if frames.len() >= retained.slots && !retained.try_grow(1) {
            return Err(DispatchBddError::RetentionLimit);
        }
        frames.push(frame);
        Ok(())
    }

    fn insert_negation_pair(
        &mut self,
        node: DispatchBddId,
        negated: DispatchBddId,
    ) -> Result<(), DispatchBddError> {
        let additions = if node == negated {
            usize::from(!self.negation_cache.contains_key(&node))
        } else {
            usize::from(!self.negation_cache.contains_key(&node))
                + usize::from(!self.negation_cache.contains_key(&negated))
        };
        self.reserve_cache_entries(additions)?;
        self.negation_cache.insert(node, negated);
        self.negation_cache.insert(negated, node);
        debug_assert_eq!(self.retained.slots, self.retained_slots());
        Ok(())
    }

    fn insert_binary_cache(
        &mut self,
        key: (DispatchBddBinaryOperator, DispatchBddId, DispatchBddId),
        value: DispatchBddId,
    ) -> Result<(), DispatchBddError> {
        let additions = usize::from(!self.binary_cache.contains_key(&key));
        self.reserve_cache_entries(additions)?;
        self.binary_cache.insert(key, value);
        debug_assert_eq!(self.retained.slots, self.retained_slots());
        Ok(())
    }

    fn reserve_cache_entries(&mut self, additions: usize) -> Result<(), DispatchBddError> {
        let entries = self
            .negation_cache
            .len()
            .checked_add(self.binary_cache.len())
            .and_then(|entries| entries.checked_add(additions))
            .ok_or(DispatchBddError::CacheLimit)?;
        if entries > MAX_DISPATCH_BDD_CACHE_ENTRIES {
            return Err(DispatchBddError::CacheLimit);
        }
        if !self.retained.try_grow(additions) {
            return Err(DispatchBddError::RetentionLimit);
        }
        Ok(())
    }
}

fn dispatch_scalar_argument_id(index: u32) -> DispatchScalarTermId {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_SCALAR_TERM_V1);
    hash_segment(&mut hasher, &[0x01]);
    hash_segment(&mut hasher, &index.to_le_bytes());
    hasher.finalize().into()
}

fn dispatch_scalar_integer_id(value: i64) -> DispatchScalarTermId {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_SCALAR_TERM_V1);
    hash_segment(&mut hasher, &[0x02]);
    hash_segment(&mut hasher, &value.to_le_bytes());
    hasher.finalize().into()
}

fn dispatch_scalar_field_id(
    base: DispatchScalarTermId,
    fields: &[CheckedVariantField],
    budget: &mut DispatchCanonicalizationBudget,
) -> Result<DispatchScalarTermId, DispatchBddError> {
    let projection = dispatch_field_projection_id(fields, budget)?;
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_SCALAR_TERM_V1);
    hash_segment(&mut hasher, &[0x03]);
    hash_segment(&mut hasher, &base);
    hash_segment(&mut hasher, &projection);
    Ok(hasher.finalize().into())
}

fn dispatch_scalar_negation_id(inner: DispatchScalarTermId) -> DispatchScalarTermId {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_SCALAR_TERM_V1);
    hash_segment(&mut hasher, &[0x04]);
    hash_segment(&mut hasher, &inner);
    hasher.finalize().into()
}

fn dispatch_scalar_arithmetic_id(
    operator: DispatchArithmeticOperator,
    left: DispatchScalarTermId,
    right: DispatchScalarTermId,
) -> DispatchScalarTermId {
    let operator = match operator {
        DispatchArithmeticOperator::Add => 0x01,
        DispatchArithmeticOperator::Subtract => 0x02,
        DispatchArithmeticOperator::Multiply => 0x03,
        DispatchArithmeticOperator::Divide => 0x04,
        DispatchArithmeticOperator::Remainder => 0x05,
    };
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_SCALAR_TERM_V1);
    hash_segment(&mut hasher, &[0x05, operator]);
    hash_segment(&mut hasher, &left);
    hash_segment(&mut hasher, &right);
    hasher.finalize().into()
}

fn dispatch_field_projection_id(
    fields: &[CheckedVariantField],
    budget: &mut DispatchCanonicalizationBudget,
) -> Result<[u8; 32], DispatchBddError> {
    if fields.len() > MAX_DISPATCH_FIELD_VARIANTS {
        return Err(DispatchBddError::FieldVariantLimit);
    }
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, DISPATCH_FIELD_PROJECTION_V1);
    dispatch_hash_field_usize(&mut hasher, budget, fields.len())?;
    for field in fields {
        budget.hash_field_segment(&mut hasher, field.variant.as_bytes())?;
        dispatch_hash_field_usize(&mut hasher, budget, field.variant_index)?;
        dispatch_hash_field_usize(&mut hasher, budget, field.field_index)?;
        budget.hash_field_segment(
            &mut hasher,
            &[match field.layout {
                CheckedConstructorLayout::Positional => 0x01,
                CheckedConstructorLayout::Named => 0x02,
            }],
        )?;
        dispatch_hash_field_owner(&mut hasher, budget, &field.identity.owner)?;
        dispatch_hash_field_usize(&mut hasher, budget, field.identity.variant_index)?;
        dispatch_hash_field_usize(&mut hasher, budget, field.identity.field_index)?;
        budget.hash_field_segment(&mut hasher, field.identity.name.as_bytes())?;
    }
    Ok(hasher.finalize().into())
}

fn dispatch_hash_field_owner(
    hasher: &mut Sha256,
    budget: &mut DispatchCanonicalizationBudget,
    owner: &CheckedDataTypeId,
) -> Result<(), DispatchBddError> {
    match owner {
        CheckedDataTypeId::Intrinsic { canonical_name } => {
            budget.hash_field_segment(hasher, &[0x01])?;
            budget.hash_field_segment(hasher, canonical_name.as_bytes())?;
        }
        CheckedDataTypeId::Declared(owner) => {
            budget.hash_field_segment(hasher, &[0x02])?;
            let declaration = &owner.declaration;
            budget.hash_field_segment(hasher, declaration.module.interface_hash.as_bytes())?;
            dispatch_hash_field_usize(hasher, budget, declaration.module.internal_path.len())?;
            for component in declaration.module.internal_path.iter() {
                budget.hash_field_segment(hasher, component.as_bytes())?;
            }
            budget.hash_field_segment(hasher, &[declaration.kind as u8])?;
            match declaration.owner.as_deref() {
                Some(declaration_owner) => {
                    budget.hash_field_segment(hasher, &[0x01])?;
                    budget.hash_field_segment(hasher, declaration_owner.as_bytes())?;
                }
                None => budget.hash_field_segment(hasher, &[0x00])?,
            }
            budget.hash_field_segment(hasher, declaration.name.as_bytes())?;
            match declaration.arity {
                Some(arity) => {
                    budget.hash_field_segment(hasher, &[0x01])?;
                    dispatch_hash_field_usize(hasher, budget, arity)?;
                }
                None => budget.hash_field_segment(hasher, &[0x00])?,
            }
            dispatch_hash_field_usize(hasher, budget, declaration.ordinal)?;
            dispatch_hash_field_usize(hasher, budget, owner.declaration_occurrence_ordinal)?;
        }
    }
    Ok(())
}

fn dispatch_hash_field_usize(
    hasher: &mut Sha256,
    budget: &mut DispatchCanonicalizationBudget,
    value: usize,
) -> Result<(), DispatchBddError> {
    let value = u64::try_from(value).map_err(|_| DispatchBddError::FieldMetadataLimit)?;
    budget.hash_field_segment(hasher, &value.to_le_bytes())
}

#[derive(Debug)]
struct AbstractEndpointDomains {
    before: AbstractValue,
    after: AbstractValue,
    context: AbstractValue,
    before_root: RelationalEndpointProofDomainRoot,
    after_root: RelationalEndpointProofDomainRoot,
    _retained: RetainedValueBudget,
}

#[derive(Default)]
struct ProofInputBudget {
    nodes: usize,
    exact_string_bytes: usize,
}

/// Explicit CEK-style abstract evaluator. Rust frames only drive one machine
/// transition at a time; Futuruna expression/call/rule depth lives in these
/// bounded heap structures and therefore forms a deterministic yield boundary.
struct EndpointEvalMachine {
    control: Option<EvalControl>,
    control_retained: Option<RetainedValueBudget>,
    continuations: Vec<BudgetedContinuation>,
    active: BTreeSet<ActiveDefinition>,
    transitions: usize,
    guard_site: ExprSiteId,
}

struct BudgetedContinuation {
    inner: EvalContinuation,
    _retained: RetainedValueBudget,
}

enum EvalControl {
    Site {
        site: ExprSiteId,
        env: SharedAbstractEnv,
    },
    TopLevel {
        binding: CheckedTopLevelBindingId,
        use_site: ExprSiteId,
    },
    Callable {
        callable: CheckedCallableId,
        arguments: Vec<AbstractValue>,
        call_site: ExprSiteId,
    },
    RuleFamily {
        family: RuleDispatchKey,
        arguments: Vec<AbstractValue>,
        captures: SharedAbstractEnv,
        call_site: ExprSiteId,
    },
    Apply {
        callable: AbstractValue,
        arguments: Vec<AbstractValue>,
        call_site: ExprSiteId,
    },
    CollectChildren(Box<ChildCollectionState>),
    IfBranches(Box<IfBranchState>),
    LogicalNext(Box<LogicalState>),
    MatchNext(Box<MatchState>),
    BlockNext(Box<BlockState>),
    RuleNext(Box<RuleState>),
    BuiltinNext(Box<BuiltinState>),
    Value(AbstractValue),
}

enum EvalContinuation {
    CheckSite {
        site: ExprSiteId,
        resolved_type: CheckedExpressionType,
    },
    CollectedChild(Box<ChildCollectionState>),
    ShortCircuitLeft {
        site: ExprSiteId,
        operator: String,
        env: SharedAbstractEnv,
    },
    ShortCircuitRight {
        site: ExprSiteId,
        operator: String,
        left: TruthDomain,
    },
    BinaryLeft {
        site: ExprSiteId,
        operator: String,
        env: SharedAbstractEnv,
    },
    BinaryRight {
        site: ExprSiteId,
        operator: String,
        left: AbstractValue,
    },
    Unary {
        site: ExprSiteId,
        operator: String,
    },
    IfCondition {
        site: ExprSiteId,
        env: SharedAbstractEnv,
    },
    IfBranch(Box<IfBranchState>),
    LogicalPart(Box<LogicalState>),
    Field {
        site: ExprSiteId,
        resolution: CheckedExpressionResolution,
    },
    IndexCollection {
        site: ExprSiteId,
        env: SharedAbstractEnv,
    },
    IndexValue {
        site: ExprSiteId,
        collection: AbstractValue,
    },
    BoundCallable {
        site: ExprSiteId,
        arguments: Vec<AbstractValue>,
    },
    ScopedReceiver {
        site: ExprSiteId,
        family: RuleDispatchKey,
        arguments: Vec<AbstractValue>,
    },
    FinishTopLevel {
        active: ActiveDefinition,
        cache_key: (RelationalEndpointRole, CheckedTopLevelBindingId),
    },
    FinishCallable(Box<CallableFinishState>),
    FinishLambda {
        active: ActiveDefinition,
        call_site: ExprSiteId,
        call_input: AbstractValue,
        _retained: RetainedValueBudget,
    },
    RuleCondition {
        state: Box<RuleState>,
        candidate: Box<RuleCandidateState>,
    },
    RuleValue {
        state: Box<RuleState>,
        candidate: Box<RuleCandidateState>,
    },
    MatchScrutinee(Box<MatchState>),
    MatchGuard {
        state: Box<MatchState>,
        arm: Box<MatchArmState>,
    },
    MatchBody(Box<MatchState>),
    BlockBind {
        state: Box<BlockState>,
        statement_site: ExprSiteId,
        pattern: Pat,
    },
    BlockExpression(Box<BlockState>),
    BuiltinCallback(Box<BuiltinState>),
}

enum ChildCollectionKind {
    List,
    Tuple,
    Application(CheckedExpressionResolution),
}

struct ChildCollectionState {
    site: ExprSiteId,
    env: SharedAbstractEnv,
    next_index: usize,
    child_count: usize,
    values: Vec<AbstractValue>,
    retained: RetainedValueBudget,
    kind: ChildCollectionKind,
}

struct IfBranchState {
    site: ExprSiteId,
    branches: Vec<(ExprSiteId, SharedAbstractEnv)>,
    next_index: usize,
    results: Vec<AbstractValue>,
    retained: RetainedValueBudget,
}

struct LogicalState {
    site: ExprSiteId,
    env: SharedAbstractEnv,
    part_count: usize,
    next_index: usize,
    conjunction: bool,
    result: TruthDomain,
}

struct MatchState {
    site: ExprSiteId,
    env: SharedAbstractEnv,
    arm_count: usize,
    next_arm: usize,
    next_child: usize,
    allow_bare_fielded_tag: bool,
    scrutinee: AbstractValue,
    remaining: AbstractValue,
    results: Vec<AbstractValue>,
    retained: RetainedValueBudget,
}

struct MatchArmState {
    guard_site: Option<ExprSiteId>,
    body_site: ExprSiteId,
    env: SharedAbstractEnv,
    partition: PatternPartition,
    _retained: RetainedValueBudget,
}

struct BlockState {
    site: ExprSiteId,
    statement_count: usize,
    next_statement: usize,
    env: SharedAbstractEnv,
    result: AbstractValue,
    retained: RetainedValueBudget,
}

enum ShallowExpression {
    Variable,
    Literal(Literal),
    Application {
        argument_count: usize,
    },
    Lambda {
        parameter_count: usize,
    },
    Binary {
        operator: String,
    },
    Unary {
        operator: String,
    },
    If,
    Match {
        arm_count: usize,
        allow_bare_fielded_tag: bool,
    },
    Block {
        statement_count: usize,
    },
    Field,
    Index,
    List {
        item_count: usize,
    },
    Tuple {
        item_count: usize,
    },
    Conjunction {
        part_count: usize,
    },
    Disjunction {
        part_count: usize,
    },
    Unit,
    Effectful,
    Try,
    Pipe,
}

enum ShallowBlockStatement {
    Bind(Pat),
    Expression,
    EffectfulOrControl,
    Declaration,
}

enum ShallowRuleHeadArgument {
    Wildcard,
    Variable,
    Literal(Literal),
    Application { argument_count: usize },
    Tuple { item_count: usize },
    Unsupported,
}

struct CallableFinishState {
    active: ActiveDefinition,
    callable: CheckedCallableId,
    argument_root: [u8; 32],
    call_site: ExprSiteId,
    body_site: ExprSiteId,
    call_input: AbstractValue,
    runtime_name: String,
    expected_result: Option<Ty>,
    substitutions: BTreeMap<String, Ty>,
    _retained: RetainedValueBudget,
}

struct RuleState {
    active: ActiveDefinition,
    family: RuleDispatchKey,
    arguments: Vec<AbstractValue>,
    captures: SharedAbstractEnv,
    call_site: ExprSiteId,
    call_input: AbstractValue,
    argument_root: [u8; 32],
    result_type: Ty,
    substitutions: BTreeMap<String, Ty>,
    boolean_miss_safe: bool,
    runtime_irrefutable: bool,
    next_candidate: usize,
    results: Vec<AbstractValue>,
    retained: RetainedValueBudget,
    predicate_bdd: DispatchPredicateBdd,
    residual: DispatchBddId,
    /// Residual inputs for which a reachable Bool clause has already proved
    /// a total result: `true` returns immediately, while `false` backtracks
    /// and becomes the runtime fallback if no later candidate succeeds.
    ///
    /// This is a domain, rather than a family-wide bit. A false clause whose
    /// head covers only `0` must not authorize `false` for an unrelated `1`.
    false_backtrack_coverage: DispatchBddId,
}

struct RuleCandidateState {
    candidate: CheckedRuleCandidateResolution,
    env: SharedAbstractEnv,
    head_match: HeadMatch,
    origins: DispatchArgumentOrigins,
    guard_domain: Option<DispatchBddId>,
    _retained: RetainedValueBudget,
}

struct BuiltinState {
    site: ExprSiteId,
    callback_site: ExprSiteId,
    input: AbstractValue,
    callable: AbstractValue,
    kind: BuiltinStateKind,
    // Keep the lease last so every value it accounts is dropped first.
    retained: RetainedValueBudget,
}

enum BuiltinStateKind {
    MapExact {
        values: Box<[AbstractValue]>,
        next: usize,
        output: Vec<AbstractValue>,
    },
    MapSummary {
        element: AbstractValue,
        minimum_length: u64,
        maximum_length: u64,
    },
    FilterExact {
        values: Box<[AbstractValue]>,
        next: usize,
        retained: Vec<AbstractValue>,
        possible: Vec<AbstractValue>,
        exact: bool,
    },
    FilterSummary {
        element: AbstractValue,
        maximum_length: u64,
    },
    SortByExact {
        values: Box<[AbstractValue]>,
        next: usize,
        keys: Vec<AbstractValue>,
    },
    SortBySummary {
        element: AbstractValue,
        minimum_length: u64,
        maximum_length: u64,
    },
    FoldLeft {
        values: Box<[AbstractValue]>,
        next: usize,
        accumulator: Option<AbstractValue>,
        accumulator_bounds: AbstractValueShapeBounds,
    },
    AllAny {
        values: Box<[AbstractValue]>,
        next: usize,
        truth: TruthDomain,
        is_all: bool,
    },
    FindExact {
        values: Box<[AbstractValue]>,
        next: usize,
        possible_matches: Vec<AbstractValue>,
    },
    FindSummary {
        element: AbstractValue,
        minimum_length: u64,
    },
    FlatMap {
        values: Box<[AbstractValue]>,
        next: usize,
        output: Vec<AbstractValue>,
    },
}

pub(crate) fn prove_relational_endpoint_totality(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    query: &ExploreQueryIr,
    sites: &CheckedExploreQuerySites,
    relation_id: RelationId,
    request_id: MechanismRequestId,
    observation: &MechanismObservationIr,
) -> Result<RelationalEndpointTotalityCertificate, RelationalEndpointTotalityIssue> {
    let mut prover = EndpointTotalityProver::new(index, resolutions, relation_id);
    let domains = prover.endpoint_domains(query, sites)?;

    for role in [
        RelationalEndpointRole::Before,
        RelationalEndpointRole::After,
    ] {
        let state = match role {
            RelationalEndpointRole::Before => &domains.before,
            RelationalEndpointRole::After => &domains.after,
        };
        prover.switch_role(role);
        if state.is_unreachable() || domains.context.is_unreachable() {
            continue;
        }
        let mut observation_substitutions = BTreeMap::new();
        prover.require_value_type(
            &observation.template_site,
            "mechanism endpoint state",
            &observation.state_type,
            state,
            &mut observation_substitutions,
        )?;
        prover.require_value_type(
            &observation.template_site,
            "mechanism endpoint context",
            &observation.context_type,
            &domains.context,
            &mut observation_substitutions,
        )?;
        let value = prover.eval_callable(
            &observation.endpoint_template,
            &[state, &domains.context],
            &observation.template_site,
        )?;
        prover.require_value_type(
            &observation.template_site,
            "mechanism endpoint observation",
            &observation.observation_type,
            &value.value,
            &mut observation_substitutions,
        )?;
        prover.require_bounded_value(&value.value, &observation.template_site)?;
        // Reserve the owned tuple representation before cloning either input.
        let _endpoint_inputs_retained = prover.new_tuple_clone_budget(
            [state, &domains.context].into_iter(),
            &observation.template_site,
        )?;
        let endpoint_inputs =
            AbstractValue::Tuple(vec![state.clone(), domains.context.clone()].into_boxed_slice());
        prover.record(
            &observation.template_site,
            ObligationKind::Endpoint,
            &endpoint_inputs,
            &value.value,
        )?;
    }

    let proof_root = prover.proof_root();
    let obligation_count =
        RelationalEndpointTotalityObligationCount::try_from_usize(prover.obligations.len())
            .map_err(|error| {
                prover.issue(
                    &observation.template_site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    error.to_string(),
                )
            })?;
    RelationalEndpointTotalityCertificate::new(
        request_id,
        relation_id,
        domains.before_root,
        domains.after_root,
        proof_root,
        obligation_count,
    )
    .map_err(|error| {
        prover.issue(
            &observation.template_site,
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
            error.to_string(),
        )
    })
}

struct EndpointTotalityProver<'a, 'program> {
    index: &'a CheckedExploreSemanticIndex<'program>,
    resolutions: &'a CheckedResolutionArtifacts,
    relation_id: RelationId,
    role: RelationalEndpointRole,
    steps: usize,
    obligations: BTreeSet<ProofObligation>,
    retention: Rc<ProofRetentionLedger>,
    callable_cache: RefCell<
        BTreeMap<(RelationalEndpointRole, CheckedCallableId, [u8; 32]), CachedAbstractValue>,
    >,
    rule_family_cache:
        RefCell<BTreeMap<(RelationalEndpointRole, RuleDispatchKey, [u8; 32]), CachedAbstractValue>>,
    top_level_cache:
        RefCell<BTreeMap<(RelationalEndpointRole, CheckedTopLevelBindingId), CachedAbstractValue>>,
    cache_admission_enabled: Cell<bool>,
}

impl<'a, 'program> EndpointTotalityProver<'a, 'program> {
    fn new(
        index: &'a CheckedExploreSemanticIndex<'program>,
        resolutions: &'a CheckedResolutionArtifacts,
        relation_id: RelationId,
    ) -> Self {
        Self {
            index,
            resolutions,
            relation_id,
            role: RelationalEndpointRole::Before,
            steps: 0,
            obligations: BTreeSet::new(),
            retention: Rc::new(ProofRetentionLedger::default()),
            callable_cache: RefCell::new(BTreeMap::new()),
            rule_family_cache: RefCell::new(BTreeMap::new()),
            top_level_cache: RefCell::new(BTreeMap::new()),
            cache_admission_enabled: Cell::new(true),
        }
    }

    fn issue(
        &self,
        site: &ExprSiteId,
        reason: RelationalEndpointTotalityIssueReason,
        detail: impl Into<Box<str>>,
    ) -> RelationalEndpointTotalityIssue {
        RelationalEndpointTotalityIssue::new(self.role, site.clone(), reason, detail.into())
    }

    fn dispatch_issue(
        &self,
        site: &ExprSiteId,
        error: DispatchBddError,
    ) -> RelationalEndpointTotalityIssue {
        let reason = if error == DispatchBddError::InvalidState {
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable
        } else {
            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded
        };
        self.issue(site, reason, error.detail())
    }

    fn retry_dispatch_after_cache_shed<T>(
        &self,
        mut operation: impl FnMut() -> Result<T, DispatchBddError>,
    ) -> Result<T, DispatchBddError> {
        match operation() {
            Err(DispatchBddError::RetentionLimit) => {
                // Endpoint result caches are an optional acceleration. A BDD
                // operation may have installed a valid partial memo prefix
                // before encountering aggregate pressure, so clearing those
                // outer caches and retrying the idempotent operation is safe.
                self.shed_optional_caches();
                operation()
            }
            result => result,
        }
    }

    fn new_retained_budget(
        &self,
        site: &ExprSiteId,
    ) -> Result<RetainedValueBudget, RelationalEndpointTotalityIssue> {
        let retained = RetainedValueBudget::attached(Rc::clone(&self.retention)).or_else(|| {
            self.shed_optional_caches();
            RetainedValueBudget::attached(Rc::clone(&self.retention))
        });
        retained.ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint proof exceeds its aggregate live-retention envelope of {MAX_RETAINED_PROOF_VALUE_NODES} value nodes, {MAX_RETAINED_PROOF_EXACT_STRING_BYTES} exact string bytes, {MAX_RETAINED_PROOF_SLOTS} structural slots, or {MAX_RETAINED_PROOF_FRAMES} retained frames"
                ),
            )
        })
    }

    fn shed_optional_caches(&self) {
        self.callable_cache.borrow_mut().clear();
        self.rule_family_cache.borrow_mut().clear();
        self.top_level_cache.borrow_mut().clear();
        // Once optional state has pressured a mandatory lease, do not refill
        // it and oscillate between eviction and admission.
        self.cache_admission_enabled.set(false);
    }

    fn shared_env(
        &self,
        bindings: AbstractEnv,
        site: &ExprSiteId,
    ) -> Result<SharedAbstractEnv, RelationalEndpointTotalityIssue> {
        let mut retained = self.new_retained_budget(site)?;
        self.retain_slots(&mut retained, bindings.len(), site)?;
        for value in bindings.values() {
            self.retain_value(&mut retained, value, site)?;
        }
        Ok(Arc::new(BudgetedAbstractEnv {
            bindings,
            _retained: retained,
        }))
    }

    fn shared_env_clone(
        &self,
        bindings: &AbstractEnv,
        site: &ExprSiteId,
    ) -> Result<SharedAbstractEnv, RelationalEndpointTotalityIssue> {
        let mut retained = self.new_retained_budget(site)?;
        self.retain_slots(&mut retained, bindings.len(), site)?;
        for value in bindings.values() {
            self.retain_value(&mut retained, value, site)?;
        }
        // The complete clone is prospectively leased above.
        let bindings = bindings.clone();
        Ok(Arc::new(BudgetedAbstractEnv {
            bindings,
            _retained: retained,
        }))
    }

    fn switch_role(&mut self, role: RelationalEndpointRole) {
        // Cache keys include the endpoint role. Keep both partitions alive:
        // the proof visits Before, then After, and later revisits both roles
        // for the observer. The bounded cache-entry ledger remains the
        // retention guard across those phases.
        self.role = role;
    }

    fn cached_value(
        &self,
        value: &AbstractValue,
        site: &ExprSiteId,
    ) -> Option<CachedAbstractValue> {
        if !self.cache_admission_enabled.get() {
            return None;
        }
        let entry = CacheEntryLease::try_new(Rc::clone(&self.retention))?;
        let mut retained = RetainedValueBudget::attached(Rc::clone(&self.retention))?;
        if !retained.try_retain(value.shape_bounds()) || !retained.try_retain_slots(1) {
            return None;
        }
        debug_assert!(self.require_bounded_value(value, site).is_ok());
        Some(CachedAbstractValue {
            value: value.clone(),
            _retained: retained,
            _entry: entry,
        })
    }

    fn charge(&mut self, site: &ExprSiteId) -> Result<(), RelationalEndpointTotalityIssue> {
        self.steps = self.steps.checked_add(1).ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "endpoint proof step counter overflowed",
            )
        })?;
        if self.steps > MAX_ABSTRACT_STEPS {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!("endpoint proof exceeded {MAX_ABSTRACT_STEPS} abstract steps"),
            ));
        }
        Ok(())
    }

    fn require_bounded_value(
        &self,
        value: &AbstractValue,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let bounds = value.shape_bounds();
        self.require_shape_bounds(bounds, site)
    }

    fn require_shape_bounds(
        &self,
        bounds: AbstractValueShapeBounds,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if bounds.depth > MAX_ABSTRACT_VALUE_DEPTH {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint abstract value depth {} exceeds proof limit {MAX_ABSTRACT_VALUE_DEPTH}",
                    bounds.depth
                ),
            ));
        }
        if bounds.nodes > MAX_ABSTRACT_VALUE_NODES {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint abstract value needs {} nodes; proof limit is {MAX_ABSTRACT_VALUE_NODES}",
                    bounds.nodes
                ),
            ));
        }
        if bounds.exact_string_bytes > MAX_RETAINED_FRAME_EXACT_STRING_BYTES {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint abstract value retains {} owned string/identity bytes; proof limit is {MAX_RETAINED_FRAME_EXACT_STRING_BYTES}",
                    bounds.exact_string_bytes
                ),
            ));
        }
        Ok(())
    }

    fn retain_value(
        &self,
        budget: &mut RetainedValueBudget,
        value: &AbstractValue,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let bounds = value.shape_bounds();
        self.require_shape_bounds(bounds, site)?;
        self.retain_shape_bounds(budget, bounds, site)
    }

    fn tuple_shape_bounds<'value>(
        &self,
        values: impl IntoIterator<Item = &'value AbstractValue>,
        site: &ExprSiteId,
    ) -> Result<AbstractValueShapeBounds, RelationalEndpointTotalityIssue> {
        let mut result = AbstractValueShapeBounds {
            nodes: 1,
            depth: 1,
            exact_string_bytes: 0,
        };
        for value in values {
            let child = value.shape_bounds();
            self.require_shape_bounds(child, site)?;
            result.nodes = result.nodes.checked_add(child.nodes).ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "prospective tuple node count overflowed",
                )
            })?;
            result.depth = result.depth.max(child.depth.checked_add(1).ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "prospective tuple depth overflowed",
                )
            })?);
            result.exact_string_bytes = result
                .exact_string_bytes
                .checked_add(child.exact_string_bytes)
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "prospective tuple owned-byte count overflowed",
                    )
                })?;
        }
        self.require_shape_bounds(result, site)?;
        Ok(result)
    }

    fn new_tuple_clone_budget<'value>(
        &self,
        values: impl ExactSizeIterator<Item = &'value AbstractValue>,
        site: &ExprSiteId,
    ) -> Result<RetainedValueBudget, RelationalEndpointTotalityIssue> {
        let item_count = values.len();
        let bounds = self.tuple_shape_bounds(values, site)?;
        let mut retained = self.new_retained_budget(site)?;
        // Reserve both the tuple value and its backing element slots before
        // constructing a deep clone.
        self.retain_slots(&mut retained, item_count, site)?;
        self.retain_shape_bounds(&mut retained, bounds, site)?;
        Ok(retained)
    }

    fn new_pair_of_tuples_clone_budget<'left, 'right>(
        &self,
        left: impl ExactSizeIterator<Item = &'left AbstractValue>,
        right: impl ExactSizeIterator<Item = &'right AbstractValue>,
        site: &ExprSiteId,
    ) -> Result<RetainedValueBudget, RelationalEndpointTotalityIssue> {
        let left_count = left.len();
        let right_count = right.len();
        let left = self.tuple_shape_bounds(left, site)?;
        let right = self.tuple_shape_bounds(right, site)?;
        let outer = AbstractValueShapeBounds {
            nodes: 1_usize
                .checked_add(left.nodes)
                .and_then(|nodes| nodes.checked_add(right.nodes))
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "prospective nested tuple node count overflowed",
                    )
                })?,
            depth: 1_usize
                .checked_add(left.depth.max(right.depth))
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "prospective nested tuple depth overflowed",
                    )
                })?,
            exact_string_bytes: left
                .exact_string_bytes
                .checked_add(right.exact_string_bytes)
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "prospective nested tuple owned-byte count overflowed",
                    )
                })?,
        };
        self.require_shape_bounds(outer, site)?;
        let element_slots = left_count
            .checked_add(right_count)
            .and_then(|slots| slots.checked_add(2))
            .ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "prospective nested tuple slot count overflowed",
                )
            })?;
        let mut retained = self.new_retained_budget(site)?;
        self.retain_slots(&mut retained, element_slots, site)?;
        self.retain_shape_bounds(&mut retained, outer, site)?;
        Ok(retained)
    }

    fn budget_owned_value(
        &self,
        value: AbstractValue,
        site: &ExprSiteId,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        let mut retained = self.new_retained_budget(site)?;
        self.retain_value(&mut retained, &value, site)?;
        Ok(BudgetedAbstractValue {
            value,
            _retained: retained,
        })
    }

    fn try_budgeted_cache_clone(&self, value: &AbstractValue) -> Option<BudgetedAbstractValue> {
        // Raw, non-evicting preflight: callers hold a RefCell cache borrow and
        // must release it before optional caches can be shed on pressure.
        let mut retained = RetainedValueBudget::attached(Rc::clone(&self.retention))?;
        if !retained.try_retain(value.shape_bounds()) || !retained.try_retain_slots(1) {
            return None;
        }
        Some(BudgetedAbstractValue {
            value: value.clone(),
            _retained: retained,
        })
    }

    fn retain_value_copies(
        &self,
        budget: &mut RetainedValueBudget,
        value: &AbstractValue,
        copies: usize,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if copies == 0 {
            return Ok(());
        }
        let bounds = value.shape_bounds();
        self.require_shape_bounds(bounds, site)?;
        let repeated = AbstractValueShapeBounds {
            nodes: bounds.nodes.checked_mul(copies).ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "retained abstract-value node count overflowed",
                )
            })?,
            depth: bounds.depth,
            exact_string_bytes: bounds
                .exact_string_bytes
                .checked_mul(copies)
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "retained exact-string byte count overflowed",
                    )
                })?,
        };
        self.retain_shape_bounds(budget, repeated, site)
    }

    fn retain_shape_bounds(
        &self,
        budget: &mut RetainedValueBudget,
        bounds: AbstractValueShapeBounds,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let retained = if budget.try_retain(bounds) {
            true
        } else {
            self.shed_optional_caches();
            budget.try_retain(bounds)
        };
        if !retained {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint retained graph exceeds its per-frame or aggregate proof budget (frame: {MAX_RETAINED_FRAME_VALUE_NODES} nodes/{MAX_RETAINED_FRAME_EXACT_STRING_BYTES} owned string/identity bytes; aggregate: {MAX_RETAINED_PROOF_VALUE_NODES} nodes/{MAX_RETAINED_PROOF_EXACT_STRING_BYTES} owned string/identity bytes)"
                ),
            ));
        }
        let slot_retained = if budget.try_retain_slots(1) {
            true
        } else {
            self.shed_optional_caches();
            budget.try_retain_slots(1)
        };
        if !slot_retained {
            budget.release_value(bounds);
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint retained graph exceeds its structural-slot budget of {MAX_RETAINED_PROOF_SLOTS} aggregate slots"
                ),
            ));
        }
        Ok(())
    }

    fn retain_slots(
        &self,
        budget: &mut RetainedValueBudget,
        slots: usize,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let retained = if budget.try_retain_slots(slots) {
            true
        } else {
            self.shed_optional_caches();
            budget.try_retain_slots(slots)
        };
        if !retained {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint retained graph exceeds its structural-slot budget of {MAX_RETAINED_PROOF_SLOTS} aggregate slots"
                ),
            ));
        }
        Ok(())
    }

    fn replace_retained_value(
        &self,
        budget: &mut RetainedValueBudget,
        old: &AbstractValue,
        new: &AbstractValue,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.replace_retained_shape(budget, old.shape_bounds(), new.shape_bounds(), site)
    }

    fn replace_retained_shape(
        &self,
        budget: &mut RetainedValueBudget,
        old_bounds: AbstractValueShapeBounds,
        new_bounds: AbstractValueShapeBounds,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.require_shape_bounds(new_bounds, site)?;
        self.retain_shape_bounds(budget, new_bounds, site)?;
        budget.release_value(old_bounds);
        // The old and replacement values each occupy one retained slot, so
        // only their node/string payloads change.
        budget.slots = budget
            .slots
            .checked_sub(1)
            .expect("retained replacement slot balances");
        if let Some(ledger) = &budget.ledger {
            ledger.release(0, 0, 1, 0, 0);
        }
        Ok(())
    }

    /// Recheck the abstract result against the producer-owned expression type.
    ///
    /// The abstract interpreter deliberately joins control-flow alternatives.
    /// A join that crosses runtime type shapes becomes `Unknown`; accepting
    /// that value merely because the source checker assigned one branch a type
    /// would let a mismatched reachable branch hide inside a totality proof.
    /// `Unreachable` is the sole exception: it denotes no concrete value on
    /// this path and is therefore compatible with every result type.
    fn require_checked_expression_type(
        &self,
        site: &ExprSiteId,
        checked: &CheckedExpressionType,
        value: &AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let compatible = match checked {
            CheckedExpressionType::Resolved(expected) if !matches!(expected, Ty::Hole) => {
                self.abstract_value_conforms_to_type(value, expected, &mut BTreeMap::new(), 0)
            }
            CheckedExpressionType::Callable { .. } => {
                value.is_unreachable() || matches!(value, AbstractValue::Callable(_))
            }
            CheckedExpressionType::CallableReference => {
                value.is_unreachable()
                    || matches!(
                        value,
                        AbstractValue::Callable(
                            AbstractCallable::Function(_) | AbstractCallable::RuleFamily(_)
                        )
                    )
            }
            CheckedExpressionType::PolymorphicEmptyList => {
                value.is_unreachable()
                    || matches!(
                        value,
                        AbstractValue::List(AbstractSequence::Exact(items)) if items.is_empty()
                    )
            }
            CheckedExpressionType::Resolved(_) | CheckedExpressionType::Unsupported => false,
        };
        if compatible {
            return Ok(());
        }
        Err(self.issue(
            site,
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
            format!(
                "abstract endpoint result `{}` is incompatible with checked expression type `{}`",
                abstract_value_shape(value),
                checked_expression_type_name(checked),
            ),
        ))
    }

    fn require_value_type(
        &self,
        site: &ExprSiteId,
        description: &str,
        expected: &Ty,
        value: &AbstractValue,
        substitutions: &mut BTreeMap<String, Ty>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if self.abstract_value_conforms_to_type(value, expected, substitutions, 0) {
            return Ok(());
        }
        Err(self.issue(
            site,
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
            format!(
                "{description} has abstract runtime shape `{}`, incompatible with `{expected}`",
                abstract_value_shape(value),
            ),
        ))
    }

    /// Decide whether every concrete value represented by `value` inhabits
    /// `expected`. Type variables are instantiated from concrete call inputs
    /// and then reused for the result, so a generic `a -> a` cannot certify an
    /// unrelated result shape. Empty containers retain an internal `_`
    /// element placeholder while preserving their proved outer container.
    fn abstract_value_conforms_to_type(
        &self,
        value: &AbstractValue,
        expected: &Ty,
        substitutions: &mut BTreeMap<String, Ty>,
        depth: usize,
    ) -> bool {
        if value.is_unreachable() {
            return true;
        }
        if depth >= MAX_CALL_DEPTH || matches!(value, AbstractValue::Unknown) {
            return false;
        }
        match expected {
            Ty::Var(name) => {
                if let Some(bound) = substitutions.get(name).cloned() {
                    self.abstract_value_conforms_to_type(value, &bound, substitutions, depth + 1)
                } else if let Some(inferred) = self.abstract_value_type(value, depth + 1) {
                    substitutions.insert(name.clone(), inferred);
                    true
                } else {
                    false
                }
            }
            Ty::Name(name) => match name.as_str() {
                "Int" => matches!(value, AbstractValue::Int(_)),
                "Bool" => matches!(value, AbstractValue::Bool(_)),
                "Float" => matches!(value, AbstractValue::Float(_)),
                "String" => matches!(value, AbstractValue::String(_)),
                "Char" | "Character" => matches!(value, AbstractValue::Character(_)),
                "Unit" => matches!(value, AbstractValue::Unit),
                "List" => matches!(value, AbstractValue::List(_)),
                "Set" => matches!(value, AbstractValue::Set(_)),
                "Map" => matches!(value, AbstractValue::Map(_)),
                "Tuple" => matches!(value, AbstractValue::Tuple(_)),
                _ => self.abstract_constructors_conform_to_nominal(
                    value,
                    name,
                    &[],
                    substitutions,
                    depth + 1,
                ),
            },
            Ty::App(constructor, arguments) => {
                let Ty::Name(name) = constructor.as_ref() else {
                    return false;
                };
                match (name.as_str(), arguments.as_slice(), value) {
                    ("List", [item], AbstractValue::List(sequence)) => self
                        .abstract_sequence_conforms_to_type(
                            sequence,
                            item,
                            substitutions,
                            depth + 1,
                        ),
                    ("Set", [item], AbstractValue::Set(sequence)) => self
                        .abstract_sequence_conforms_to_type(
                            sequence,
                            item,
                            substitutions,
                            depth + 1,
                        ),
                    ("Map", [key, item], AbstractValue::Map(entries)) => {
                        entries.iter().all(|(stored_key, stored_value)| {
                            self.abstract_value_conforms_to_type(
                                stored_key,
                                key,
                                substitutions,
                                depth + 1,
                            ) && self.abstract_value_conforms_to_type(
                                stored_value,
                                item,
                                substitutions,
                                depth + 1,
                            )
                        })
                    }
                    ("Tuple", items, AbstractValue::Tuple(values)) => {
                        values.len() == items.len()
                            && values.iter().zip(items).all(|(value, expected)| {
                                self.abstract_value_conforms_to_type(
                                    value,
                                    expected,
                                    substitutions,
                                    depth + 1,
                                )
                            })
                    }
                    ("Pair", [left, right], AbstractValue::Tuple(values)) => {
                        matches!(values.as_ref(), [left_value, right_value]
                        if self.abstract_value_conforms_to_type(
                            left_value,
                            left,
                            substitutions,
                            depth + 1,
                        ) && self.abstract_value_conforms_to_type(
                            right_value,
                            right,
                            substitutions,
                            depth + 1,
                        ))
                    }
                    _ => self.abstract_constructors_conform_to_nominal(
                        value,
                        name,
                        arguments,
                        substitutions,
                        depth + 1,
                    ),
                }
            }
            Ty::Arrow(_, _) => matches!(value, AbstractValue::Callable(_)),
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                self.abstract_value_conforms_to_type(value, inner, substitutions, depth + 1)
            }
            Ty::Optional(inner) => self.abstract_constructors_conform_to_nominal(
                value,
                "Option",
                std::slice::from_ref(inner.as_ref()),
                substitutions,
                depth + 1,
            ),
            Ty::Unit => matches!(value, AbstractValue::Unit),
            // `_` is used only inside an inferred empty-container shape. A
            // producer-resolved root `Hole` is rejected by the caller above.
            Ty::Hole => true,
        }
    }

    fn abstract_sequence_conforms_to_type(
        &self,
        sequence: &AbstractSequence,
        expected: &Ty,
        substitutions: &mut BTreeMap<String, Ty>,
        depth: usize,
    ) -> bool {
        match sequence {
            AbstractSequence::Exact(values) => values.iter().all(|value| {
                self.abstract_value_conforms_to_type(value, expected, substitutions, depth + 1)
            }),
            AbstractSequence::Summary {
                maximum_length: 0, ..
            } => true,
            AbstractSequence::Summary { element, .. } => {
                self.abstract_value_conforms_to_type(element, expected, substitutions, depth + 1)
            }
        }
    }

    fn abstract_constructors_conform_to_nominal(
        &self,
        value: &AbstractValue,
        expected_owner: &str,
        expected_arguments: &[Ty],
        substitutions: &mut BTreeMap<String, Ty>,
        depth: usize,
    ) -> bool {
        let AbstractValue::Constructors(variants) = value else {
            return false;
        };
        !variants.is_empty()
            && variants.values().all(|variant| {
                self.abstract_constructor_conforms_to_nominal(
                    variant,
                    expected_owner,
                    expected_arguments,
                    substitutions,
                    depth + 1,
                )
            })
    }

    fn abstract_constructor_conforms_to_nominal(
        &self,
        constructor: &AbstractConstructor,
        expected_owner: &str,
        expected_arguments: &[Ty],
        substitutions: &mut BTreeMap<String, Ty>,
        depth: usize,
    ) -> bool {
        if depth >= MAX_CALL_DEPTH || constructor.identity.owner_type.as_ref() != expected_owner {
            return false;
        }
        let CheckedDataTypeId::Declared(owner) = &constructor.identity.owner else {
            // Intrinsic identities are producer-minted nominal evidence. Their
            // payload expressions have already passed their own checked-type
            // obligations, while no authored declaration exists to replay.
            return true;
        };
        let Some(occurrence) = self
            .resolutions
            .data_owner_to_analysis_occurrence
            .get(owner)
        else {
            return false;
        };
        let Some(declaration) = self.index.type_declarations.get(occurrence).copied() else {
            return false;
        };
        match declaration {
            TypeDecl::ADT {
                name,
                params,
                variants,
                ..
            } => {
                if name != expected_owner
                    || (!expected_arguments.is_empty() && params.len() != expected_arguments.len())
                {
                    return false;
                }
                let Some(variant) = variants.get(constructor.identity.variant_index) else {
                    return false;
                };
                if variant.name != constructor.identity.variant.as_ref()
                    || variant.fields.len() != constructor.fields.len()
                    || constructor.identity.fields.len() != constructor.fields.len()
                {
                    return false;
                }
                let type_arguments = params
                    .iter()
                    .zip(expected_arguments)
                    .map(|(parameter, argument)| (parameter.name.clone(), argument.clone()))
                    .collect::<BTreeMap<_, _>>();
                variant
                    .fields
                    .iter()
                    .zip(constructor.fields.iter())
                    .all(|(field, value)| {
                        let expected = substitute_type_parameters(&field.ty, &type_arguments);
                        self.abstract_value_conforms_to_type(
                            value,
                            &expected,
                            substitutions,
                            depth + 1,
                        )
                    })
            }
            TypeDecl::WhenType { name, variants, .. } => {
                if name != expected_owner || !expected_arguments.is_empty() {
                    return false;
                }
                let Some(variant) = variants.get(constructor.identity.variant_index) else {
                    return false;
                };
                variant.name == constructor.identity.variant.as_ref()
                    && variant.fields.len() == constructor.fields.len()
                    && variant
                        .fields
                        .iter()
                        .zip(constructor.fields.iter())
                        .all(|(field, value)| {
                            self.abstract_value_conforms_to_type(
                                value,
                                &field.ty,
                                substitutions,
                                depth + 1,
                            )
                        })
            }
            TypeDecl::RuleScope { name, params, .. } => {
                name == expected_owner
                    && expected_arguments.is_empty()
                    && params.len() == constructor.fields.len()
                    && params
                        .iter()
                        .zip(constructor.fields.iter())
                        .all(|(parameter, value)| {
                            parameter.ty.as_ref().is_some_and(|expected| {
                                self.abstract_value_conforms_to_type(
                                    value,
                                    expected,
                                    substitutions,
                                    depth + 1,
                                )
                            })
                        })
            }
            _ => false,
        }
    }

    /// Infer just enough structural type information to instantiate a generic
    /// call contract. This is not a second source typechecker: every nominal
    /// owner comes from a checked constructor identity, and `_` is retained
    /// only where an empty container reveals no element type.
    fn abstract_value_type(&self, value: &AbstractValue, depth: usize) -> Option<Ty> {
        if depth >= MAX_CALL_DEPTH {
            return None;
        }
        match value {
            AbstractValue::Unreachable | AbstractValue::Unknown => None,
            AbstractValue::Int(_) => Some(Ty::Name("Int".into())),
            AbstractValue::Bool(_) => Some(Ty::Name("Bool".into())),
            AbstractValue::Float(_) => Some(Ty::Name("Float".into())),
            AbstractValue::String(_) => Some(Ty::Name("String".into())),
            AbstractValue::Character(_) => Some(Ty::Name("Char".into())),
            AbstractValue::Unit => Some(Ty::Unit),
            AbstractValue::List(sequence) => Some(Ty::App(
                Box::new(Ty::Name("List".into())),
                vec![self.abstract_sequence_type(sequence, depth + 1)],
            )),
            AbstractValue::Set(sequence) => Some(Ty::App(
                Box::new(Ty::Name("Set".into())),
                vec![self.abstract_sequence_type(sequence, depth + 1)],
            )),
            AbstractValue::Map(entries) => {
                let keys = join_inferred_types(
                    entries
                        .iter()
                        .map(|(key, _)| self.abstract_value_type(key, depth + 1)),
                )?;
                let values = join_inferred_types(
                    entries
                        .iter()
                        .map(|(_, value)| self.abstract_value_type(value, depth + 1)),
                )?;
                Some(Ty::App(
                    Box::new(Ty::Name("Map".into())),
                    vec![keys, values],
                ))
            }
            AbstractValue::Tuple(values) => Some(Ty::App(
                Box::new(Ty::Name("Tuple".into())),
                values
                    .iter()
                    .map(|value| self.abstract_value_type(value, depth + 1))
                    .collect::<Option<Vec<_>>>()?,
            )),
            AbstractValue::Constructors(variants) => {
                let mut owners = variants
                    .values()
                    .map(|variant| variant.identity.owner_type.as_ref());
                let owner = owners.next()?;
                if owners.any(|candidate| candidate != owner) {
                    return None;
                }
                // Constructor identity proves the nominal owner. Generic
                // arguments are recovered only when field evidence fixes them;
                // otherwise preserve an explicit `_` inside the known owner.
                let first = variants.values().next()?;
                let parameter_names = self.constructor_type_parameter_names(&first.identity);
                if parameter_names.is_empty() {
                    return Some(Ty::Name(owner.to_string()));
                }
                let mut inferred_arguments = vec![Ty::Hole; parameter_names.len()];
                for variant in variants.values() {
                    let Some(candidate) =
                        self.infer_constructor_type_arguments(variant, &parameter_names, depth + 1)
                    else {
                        return None;
                    };
                    for (known, candidate) in inferred_arguments.iter_mut().zip(candidate) {
                        *known = merge_inferred_type(known.clone(), candidate)?;
                    }
                }
                Some(Ty::App(
                    Box::new(Ty::Name(owner.to_string())),
                    inferred_arguments,
                ))
            }
            AbstractValue::Callable(_) => None,
        }
    }

    fn abstract_sequence_type(&self, sequence: &AbstractSequence, depth: usize) -> Ty {
        match sequence {
            AbstractSequence::Exact(values) => join_inferred_types(
                values
                    .iter()
                    .map(|value| self.abstract_value_type(value, depth + 1)),
            )
            .unwrap_or(Ty::Hole),
            AbstractSequence::Summary {
                maximum_length: 0, ..
            } => Ty::Hole,
            AbstractSequence::Summary { element, .. } => self
                .abstract_value_type(element, depth + 1)
                .unwrap_or(Ty::Hole),
        }
    }

    fn constructor_type_parameter_names(
        &self,
        identity: &CheckedConstructorIdentity,
    ) -> Vec<String> {
        let CheckedDataTypeId::Declared(owner) = &identity.owner else {
            return Vec::new();
        };
        let Some(occurrence) = self
            .resolutions
            .data_owner_to_analysis_occurrence
            .get(owner)
        else {
            return Vec::new();
        };
        match self.index.type_declarations.get(occurrence).copied() {
            Some(TypeDecl::ADT { params, .. }) => params
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn infer_constructor_type_arguments(
        &self,
        constructor: &AbstractConstructor,
        parameter_names: &[String],
        depth: usize,
    ) -> Option<Vec<Ty>> {
        let CheckedDataTypeId::Declared(owner) = &constructor.identity.owner else {
            return Some(vec![Ty::Hole; parameter_names.len()]);
        };
        let occurrence = self
            .resolutions
            .data_owner_to_analysis_occurrence
            .get(owner)?;
        let TypeDecl::ADT { variants, .. } =
            self.index.type_declarations.get(occurrence).copied()?
        else {
            return Some(vec![Ty::Hole; parameter_names.len()]);
        };
        let variant = variants.get(constructor.identity.variant_index)?;
        if variant.fields.len() != constructor.fields.len() {
            return None;
        }
        let mut substitutions = BTreeMap::new();
        for (field, value) in variant.fields.iter().zip(constructor.fields.iter()) {
            if !self.abstract_value_conforms_to_type(
                value,
                &field.ty,
                &mut substitutions,
                depth + 1,
            ) {
                return None;
            }
        }
        Some(
            parameter_names
                .iter()
                .map(|parameter| substitutions.remove(parameter).unwrap_or(Ty::Hole))
                .collect(),
        )
    }

    fn charge_proof_input_node(
        &self,
        budget: &mut ProofInputBudget,
        depth: usize,
        site: &ExprSiteId,
        description: &str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if depth >= MAX_CALL_DEPTH {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "{description} exceeds the endpoint proof input depth limit of {MAX_CALL_DEPTH}"
                ),
            ));
        }
        self.charge_proof_input_nodes(budget, 1, site, description)
    }

    fn charge_proof_input_nodes(
        &self,
        budget: &mut ProofInputBudget,
        additional: usize,
        site: &ExprSiteId,
        description: &str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let nodes = budget.nodes.checked_add(additional).ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!("{description} proof input node count overflowed"),
            )
        })?;
        if nodes > MAX_ABSTRACT_VALUE_NODES {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "{description} needs {nodes} proof input nodes; limit is {MAX_ABSTRACT_VALUE_NODES}"
                ),
            ));
        }
        budget.nodes = nodes;
        Ok(())
    }

    fn require_proof_input_items(
        &self,
        items: usize,
        site: &ExprSiteId,
        description: &str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if items > MAX_EXACT_COLLECTION_ITEMS {
            Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "{description} has {items} items; endpoint proof input limit is {MAX_EXACT_COLLECTION_ITEMS}"
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn require_proof_input_variants(
        &self,
        variants: usize,
        site: &ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if variants > MAX_CONSTRUCTOR_VARIANTS {
            Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "finite endpoint type has {variants} variants; endpoint proof input limit is {MAX_CONSTRUCTOR_VARIANTS}"
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn reserve_exact_string_bytes(&self, budget: &mut ProofInputBudget, bytes: usize) -> bool {
        let Some(total) = budget.exact_string_bytes.checked_add(bytes) else {
            return false;
        };
        if total > MAX_EXACT_STRING_BYTES {
            return false;
        }
        budget.exact_string_bytes = total;
        true
    }

    fn charge_proof_input_owned_bytes(
        &self,
        budget: &mut ProofInputBudget,
        bytes: usize,
        site: &ExprSiteId,
        description: &str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if !self.reserve_exact_string_bytes(budget, bytes) {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "{description} exceeds the endpoint proof input owned-byte limit of {MAX_EXACT_STRING_BYTES}"
                ),
            ));
        }
        Ok(())
    }

    fn endpoint_domains(
        &mut self,
        query: &ExploreQueryIr,
        sites: &CheckedExploreQuerySites,
    ) -> Result<AbstractEndpointDomains, RelationalEndpointTotalityIssue> {
        if !self.resolutions.source_snapshot_coherent
            || self.resolutions.analysis_program != self.index.program.id
        {
            return Err(self.issue(
                &sites.successor,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "endpoint proof inputs do not share one coherent checked source snapshot",
            ));
        }
        if query.source.bindings.len() != sites.source_bindings.len() {
            return Err(self.issue(
                &sites.successor,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked FROM binding/site counts disagree",
            ));
        }
        let mut env = AbstractEnv::new();
        let mut env_retained = self.new_retained_budget(&sites.successor)?;
        let mut role_retained = self.new_retained_budget(&sites.successor)?;
        let mut context = None;
        let mut before = None;
        let mut source_empty = false;
        for (binding, binding_sites) in query
            .source
            .bindings
            .iter()
            .zip(sites.source_bindings.iter())
        {
            let budgeted_value = if source_empty {
                self.budget_owned_value(AbstractValue::Unreachable, &binding_sites.expression)?
            } else {
                match &binding.kind {
                    ExploreSourceBindingKindIr::Singleton { .. } => {
                        self.eval_site(&binding_sites.expression, &env)?
                    }
                    ExploreSourceBindingKindIr::Finite { domain } => {
                        self.eval_domain(domain, &binding_sites.expression, &env)?
                    }
                }
            };
            let (value, value_retained) = budgeted_value.into_parts();
            source_empty |= value.is_unreachable();
            self.require_bounded_value(&value, &binding_sites.expression)?;
            self.retain_value(&mut env_retained, &value, &binding_sites.expression)?;
            env.insert(binding_sites.binder.clone(), value.clone());
            match binding.role {
                ExploreSourceBindingRoleIr::Context => {
                    self.retain_value(&mut role_retained, &value, &binding_sites.expression)?;
                    context = Some(value);
                }
                ExploreSourceBindingRoleIr::Before => {
                    self.retain_value(&mut role_retained, &value, &binding_sites.expression)?;
                    before = Some(value);
                }
                ExploreSourceBindingRoleIr::Auxiliary => {}
            }
            drop(value_retained);
        }
        let mut context = context.ok_or_else(|| {
            self.issue(
                &sites.successor,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked FROM relation has no Context value",
            )
        })?;
        let mut before = before.ok_or_else(|| {
            self.issue(
                &sites.successor,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked FROM relation has no Before value",
            )
        })?;
        self.switch_role(RelationalEndpointRole::After);
        let after = if source_empty {
            self.budget_owned_value(AbstractValue::Unreachable, &sites.successor)?
        } else {
            match &query.successor.kind {
                ExploreSuccessorKindIr::Singleton { .. } => {
                    self.eval_site(&sites.successor, &env)?
                }
                ExploreSuccessorKindIr::Finite { domain } => {
                    self.eval_domain(domain, &sites.successor, &env)?
                }
            }
        };
        let (after, after_retained) = after.into_parts();
        if source_empty {
            context = AbstractValue::Unreachable;
            before = AbstractValue::Unreachable;
        }
        self.require_bounded_value(&after, &sites.successor)?;
        // The source environment is no longer consulted once the successor
        // has been evaluated. Release its duplicate values before retaining
        // the durable endpoint-domain bundle.
        drop(env);
        drop(env_retained);
        let before_root = endpoint_domain_root(
            self.relation_id,
            RelationalEndpointRole::Before,
            &before,
            &context,
        );
        let after_root = endpoint_domain_root(
            self.relation_id,
            RelationalEndpointRole::After,
            &after,
            &context,
        );
        let mut retained = self.new_retained_budget(&sites.successor)?;
        self.retain_value(&mut retained, &before, &sites.successor)?;
        self.retain_value(&mut retained, &after, &sites.successor)?;
        self.retain_value(&mut retained, &context, &sites.successor)?;
        drop(role_retained);
        drop(after_retained);
        Ok(AbstractEndpointDomains {
            before,
            after,
            context,
            before_root,
            after_root,
            _retained: retained,
        })
    }

    fn eval_domain(
        &mut self,
        domain: &ExploreFiniteDomainIr,
        site: &ExprSiteId,
        env: &AbstractEnv,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        match domain {
            ExploreFiniteDomainIr::Exact(domain) => {
                let value = self.exact_domain_value(domain, site)?;
                self.budget_owned_value(value, site)
            }
            ExploreFiniteDomainIr::Collection { .. } => {
                let value = self.eval_site(site, env)?;
                let (value, source_retained) = value.into_parts();
                // A joined element cannot exceed the complete source
                // collection shape. Reserve that conservative destination
                // allowance before cloning/joining its elements.
                let mut retained = self.new_retained_budget(site)?;
                self.retain_value(&mut retained, &value, site)?;
                let result = match value {
                    AbstractValue::List(sequence) | AbstractValue::Set(sequence) => sequence
                        .joined_element()
                        .unwrap_or(AbstractValue::Unreachable),
                    _ => Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "finite collection domain did not abstract to a List or Set",
                    ))?,
                };
                drop(source_retained);
                Ok(BudgetedAbstractValue {
                    value: result,
                    _retained: retained,
                })
            }
            ExploreFiniteDomainIr::IntRange { .. } => {
                // The checked source site is the original `range(start, end)`
                // application: child zero is the callable and endpoints are
                // application arguments one and two.
                let start_site = child_site(site, 1);
                let end_site = child_site(site, 2);
                let start_value = self.eval_site(&start_site, env)?;
                let start = start_value.value.int().ok_or_else(|| {
                    self.issue(
                        &start_site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "integer range start is not a proved Int interval",
                    )
                })?;
                let end_value = self.eval_site(&end_site, env)?;
                let end = end_value.value.int().ok_or_else(|| {
                    self.issue(
                        &end_site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "integer range end is not a proved Int interval",
                    )
                })?;
                if start.maximum > end.minimum {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "integer range start is not proved at or before its end for every source prefix",
                    ));
                }
                let maximum = end.maximum.checked_sub(1).ok_or_else(|| {
                    self.issue(
                        &end_site,
                        RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                        "integer range end cannot be decremented safely",
                    )
                })?;
                if start.minimum > maximum {
                    return self.budget_owned_value(AbstractValue::Unreachable, site);
                }
                self.budget_owned_value(
                    AbstractValue::Int(
                    IntInterval::new(start.minimum, maximum)
                        .and_then(IntInterval::runtime_int)
                        .ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                                "integer range endpoint hull does not fit Futuruna Int",
                            )
                        })?,
                    ),
                    site,
                )
            }
        }
    }

    fn exact_domain_value(
        &self,
        domain: &ExploreExactDomain,
        site: &ExprSiteId,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        match domain {
            ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                cardinality,
            } => {
                if start > end_exclusive {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "exact integer range starts after its end",
                    ));
                }
                let expected = if end_exclusive == start {
                    0
                } else {
                    u64::try_from(i128::from(*end_exclusive) - i128::from(*start)).map_err(
                        |_| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                                "exact integer range cardinality exceeds the proof format",
                            )
                        },
                    )?
                };
                if expected != *cardinality {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "exact integer range bounds and cardinality disagree",
                    ));
                }
                if *cardinality == 0 {
                    return Ok(AbstractValue::Unreachable);
                }
                let maximum = end_exclusive.checked_sub(1).ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                        "exact integer range end cannot be decremented safely",
                    )
                })?;
                Ok(AbstractValue::Int(IntInterval {
                    minimum: i128::from(*start),
                    maximum: i128::from(maximum),
                }))
            }
            ExploreExactDomain::Enumerated { values, .. } => {
                self.require_proof_input_items(values.len(), site, "enumerated endpoint domain")?;
                let mut budget = ProofInputBudget::default();
                Ok(join_values(
                    values
                        .iter()
                        .map(|value| self.abstract_explore_value(value, site, 0, &mut budget))
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .unwrap_or(AbstractValue::Unreachable))
            }
            ExploreExactDomain::FiniteType { plan, .. } => {
                let mut budget = ProofInputBudget::default();
                self.abstract_finite_plan(plan, site, 0, &mut budget)
            }
        }
    }

    fn abstract_finite_plan(
        &self,
        plan: &ExploreFiniteTypePlan,
        site: &ExprSiteId,
        depth: usize,
        budget: &mut ProofInputBudget,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        self.charge_proof_input_node(budget, depth, site, "finite endpoint type")?;
        match plan {
            ExploreFiniteTypePlan::Unit => Ok(AbstractValue::Unit),
            ExploreFiniteTypePlan::Bool => Ok(AbstractValue::Bool(TruthDomain::BOTH)),
            ExploreFiniteTypePlan::Tuple { elements, .. } => {
                self.require_proof_input_items(elements.len(), site, "finite endpoint tuple type")?;
                Ok(AbstractValue::Tuple(
                    elements
                        .iter()
                        .map(|element| self.abstract_finite_plan(element, site, depth + 1, budget))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                ))
            }
            ExploreFiniteTypePlan::Sum {
                type_name,
                variants,
                ..
            } => {
                self.require_proof_input_variants(variants.len(), site)?;
                // Each abstract constructor occupies storage in the result but
                // is not represented by a recursive field-plan node.
                self.charge_proof_input_nodes(
                    budget,
                    variants.len(),
                    site,
                    "finite endpoint type variants",
                )?;
                let mut result = BTreeMap::new();
                for variant in variants {
                    self.require_proof_input_items(
                        variant.fields.len(),
                        site,
                        "finite endpoint constructor fields",
                    )?;
                    let identity = self
                        .resolutions
                        .constructor_identities
                        .get(&(type_name.as_str().into(), variant.name.as_str().into()))
                        .ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                format!(
                                    "finite endpoint constructor {type_name}::{} has no exact checked identity",
                                    variant.name
                                ),
                            )
                        })?
                        .as_ref();
                    self.charge_proof_input_owned_bytes(
                        budget,
                        constructor_identity_retained_bytes(identity),
                        site,
                        "finite endpoint constructor identities",
                    )?;
                    let identity = identity.clone();
                    let fields = variant
                        .fields
                        .iter()
                        .map(|field| {
                            self.abstract_finite_plan(&field.plan, site, depth + 1, budget)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice();
                    result.insert(
                        checked_explore_projection_constructor_digest(&identity),
                        AbstractConstructor { identity, fields },
                    );
                }
                Ok(AbstractValue::Constructors(result))
            }
        }
    }

    fn abstract_explore_value(
        &self,
        value: &ExploreValue,
        site: &ExprSiteId,
        depth: usize,
        budget: &mut ProofInputBudget,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        self.charge_proof_input_node(budget, depth, site, "enumerated endpoint value")?;
        match value {
            ExploreValue::Int(value) => Ok(AbstractValue::Int(IntInterval::singleton(*value))),
            ExploreValue::FloatBits(value) => Ok(AbstractValue::Float(Some(*value))),
            ExploreValue::String(value) => {
                let exact = self
                    .reserve_exact_string_bytes(budget, value.len())
                    .then(|| value.clone().into_boxed_str());
                Ok(AbstractValue::String(exact))
            }
            ExploreValue::Character(value) => Ok(AbstractValue::Character(Some(*value))),
            ExploreValue::Boolean(value) => Ok(AbstractValue::Bool(TruthDomain::from_bool(*value))),
            ExploreValue::Unit => Ok(AbstractValue::Unit),
            ExploreValue::List(values) => {
                self.require_proof_input_items(values.len(), site, "enumerated endpoint List")?;
                Ok(AbstractValue::List(AbstractSequence::Exact(
                    values
                        .iter()
                        .map(|value| self.abstract_explore_value(value, site, depth + 1, budget))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                )))
            }
            ExploreValue::Set(values) => {
                self.require_proof_input_items(values.len(), site, "enumerated endpoint Set")?;
                Ok(AbstractValue::Set(AbstractSequence::Exact(
                    values
                        .iter()
                        .map(|value| self.abstract_explore_value(value, site, depth + 1, budget))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                )))
            }
            ExploreValue::Tuple(values) => {
                self.require_proof_input_items(values.len(), site, "enumerated endpoint tuple")?;
                Ok(AbstractValue::Tuple(
                    values
                        .iter()
                        .map(|value| self.abstract_explore_value(value, site, depth + 1, budget))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                ))
            }
            ExploreValue::Constructor {
                type_name,
                variant,
                positional,
                fields,
            } => {
                self.require_proof_input_items(
                    fields.len(),
                    site,
                    "enumerated endpoint constructor fields",
                )?;
                let identity = self
                    .resolutions
                    .constructor_identities
                    .get(&(type_name.as_str().into(), variant.as_str().into()))
                    .ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            format!(
                                "enumerated endpoint constructor {type_name}::{variant} has no exact checked identity"
                            ),
                        )
                    })?
                    .as_ref();
                self.charge_proof_input_owned_bytes(
                    budget,
                    constructor_identity_retained_bytes(identity),
                    site,
                    "enumerated endpoint constructor identities",
                )?;
                if (*positional && identity.layout != CheckedConstructorLayout::Positional)
                    || (!*positional && identity.layout != CheckedConstructorLayout::Named)
                    || fields.len() != identity.fields.len()
                {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "enumerated endpoint constructor layout disagrees with checked identity",
                    ));
                }
                let mut canonical = vec![AbstractValue::Unknown; identity.fields.len()];
                for (source_index, (name, value)) in fields.iter().enumerate() {
                    let canonical_index = if *positional {
                        source_index
                    } else {
                        identity
                            .fields
                            .iter()
                            .position(|field| field.name.as_ref() == name)
                            .ok_or_else(|| {
                                self.issue(
                                    site,
                                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                    "enumerated endpoint field is absent from checked constructor",
                                )
                            })?
                    };
                    canonical[canonical_index] =
                        self.abstract_explore_value(value, site, depth + 1, budget)?;
                }
                let digest = checked_explore_projection_constructor_digest(identity);
                Ok(AbstractValue::Constructors(BTreeMap::from([(
                    digest,
                    AbstractConstructor {
                        identity: identity.clone(),
                        fields: canonical.into_boxed_slice(),
                    },
                )])))
            }
        }
    }

    fn eval_site(
        &mut self,
        site: &ExprSiteId,
        env: &AbstractEnv,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        self.run_evaluation(EvalControl::Site {
            site: site.clone(),
            env: self.shared_env_clone(env, site)?,
        })
    }

    fn eval_callable(
        &mut self,
        callable: &CheckedCallableId,
        arguments: &[&AbstractValue],
        call_site: &ExprSiteId,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        // Lease the destination argument vector before cloning its values.
        let mut retained = self.new_retained_budget(call_site)?;
        self.retain_slots(&mut retained, arguments.len(), call_site)?;
        for value in arguments {
            self.retain_value(&mut retained, value, call_site)?;
        }
        let arguments = arguments.iter().map(|value| (*value).clone()).collect();
        self.run_evaluation_with_retained(
            EvalControl::Callable {
                callable: callable.clone(),
                arguments,
                call_site: call_site.clone(),
            },
            retained,
        )
    }

    fn run_evaluation(
        &mut self,
        initial: EvalControl,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        let guard_site = match &initial {
            EvalControl::Site { site, .. }
            | EvalControl::TopLevel { use_site: site, .. }
            | EvalControl::Callable {
                call_site: site, ..
            }
            | EvalControl::RuleFamily {
                call_site: site, ..
            }
            | EvalControl::Apply {
                call_site: site, ..
            } => site.clone(),
            _ => unreachable!("root evaluation begins at a checked site or callable"),
        };
        let retained = self.budget_control_direct(&initial, &guard_site)?;
        self.run_evaluation_with_retained(initial, retained)
    }

    fn run_evaluation_with_retained(
        &mut self,
        initial: EvalControl,
        retained: RetainedValueBudget,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        let guard_site = match &initial {
            EvalControl::Site { site, .. }
            | EvalControl::TopLevel { use_site: site, .. }
            | EvalControl::Callable {
                call_site: site, ..
            }
            | EvalControl::RuleFamily {
                call_site: site, ..
            }
            | EvalControl::Apply {
                call_site: site, ..
            } => site.clone(),
            _ => unreachable!("root evaluation begins at a checked site or callable"),
        };
        let mut machine = EndpointEvalMachine {
            control_retained: Some(retained),
            control: Some(initial),
            continuations: Vec::new(),
            active: BTreeSet::new(),
            transitions: 0,
            guard_site,
        };
        loop {
            machine.transitions = machine.transitions.checked_add(1).ok_or_else(|| {
                self.issue(
                    &machine.guard_site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "endpoint evaluator transition counter overflowed",
                )
            })?;
            if machine.transitions > MAX_EVALUATION_TRANSITIONS {
                return Err(self.issue(
                    &machine.guard_site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    format!(
                        "endpoint evaluator exceeded {MAX_EVALUATION_TRANSITIONS} heap transitions"
                    ),
                ));
            }
            let control = machine.control.take().ok_or_else(|| {
                self.issue(
                    &machine.guard_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "endpoint evaluator lost its next heap work item",
                )
            })?;
            let _control_retained = machine.control_retained.take().ok_or_else(|| {
                self.issue(
                    &machine.guard_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "endpoint evaluator lost its retained control lease",
                )
            })?;
            match control {
                EvalControl::Site { site, env } => self.begin_site(&mut machine, site, env)?,
                EvalControl::TopLevel { binding, use_site } => {
                    self.begin_top_level(&mut machine, binding, use_site)?
                }
                EvalControl::Callable {
                    callable,
                    arguments,
                    call_site,
                } => self.begin_callable(&mut machine, callable, arguments, call_site)?,
                EvalControl::RuleFamily {
                    family,
                    arguments,
                    captures,
                    call_site,
                } => {
                    self.begin_rule_family(&mut machine, family, arguments, captures, call_site)?
                }
                EvalControl::Apply {
                    callable,
                    arguments,
                    call_site,
                } => self.begin_apply(&mut machine, callable, arguments, call_site)?,
                EvalControl::CollectChildren(state) => {
                    self.continue_child_collection(&mut machine, state)?
                }
                EvalControl::IfBranches(state) => self.continue_if_branches(&mut machine, state)?,
                EvalControl::LogicalNext(state) => self.continue_logical(&mut machine, state)?,
                EvalControl::MatchNext(state) => self.continue_match(&mut machine, state)?,
                EvalControl::BlockNext(state) => self.continue_block(&mut machine, state)?,
                EvalControl::RuleNext(state) => self.continue_rule(&mut machine, state)?,
                EvalControl::BuiltinNext(state) => self.continue_builtin(&mut machine, state)?,
                EvalControl::Value(value) => {
                    let Some(continuation) = machine.continuations.pop() else {
                        return Ok(BudgetedAbstractValue {
                            value,
                            _retained: _control_retained,
                        });
                    };
                    self.resume_evaluation(&mut machine, continuation.inner, value)?;
                }
            }
        }
    }

    fn budget_control_direct(
        &self,
        control: &EvalControl,
        site: &ExprSiteId,
    ) -> Result<RetainedValueBudget, RelationalEndpointTotalityIssue> {
        let mut retained = self.new_retained_budget(site)?;
        match control {
            EvalControl::Callable { arguments, .. } | EvalControl::RuleFamily { arguments, .. } => {
                self.retain_slots(&mut retained, arguments.len(), site)?;
                for value in arguments {
                    self.retain_value(&mut retained, value, site)?;
                }
            }
            EvalControl::Apply {
                callable,
                arguments,
                ..
            } => {
                self.retain_value(&mut retained, callable, site)?;
                self.retain_slots(&mut retained, arguments.len(), site)?;
                for value in arguments {
                    self.retain_value(&mut retained, value, site)?;
                }
            }
            EvalControl::Value(value) => self.retain_value(&mut retained, value, site)?,
            EvalControl::Site { .. }
            | EvalControl::TopLevel { .. }
            | EvalControl::CollectChildren(_)
            | EvalControl::IfBranches(_)
            | EvalControl::LogicalNext(_)
            | EvalControl::MatchNext(_)
            | EvalControl::BlockNext(_)
            | EvalControl::RuleNext(_)
            | EvalControl::BuiltinNext(_) => {}
        }
        Ok(retained)
    }

    fn budget_continuation_direct(
        &self,
        continuation: &EvalContinuation,
        site: &ExprSiteId,
    ) -> Result<RetainedValueBudget, RelationalEndpointTotalityIssue> {
        let mut retained = self.new_retained_budget(site)?;
        match continuation {
            EvalContinuation::BinaryRight { left, .. } => {
                self.retain_value(&mut retained, left, site)?;
            }
            EvalContinuation::IndexValue { collection, .. } => {
                self.retain_value(&mut retained, collection, site)?;
            }
            EvalContinuation::BoundCallable { arguments, .. }
            | EvalContinuation::ScopedReceiver { arguments, .. } => {
                self.retain_slots(&mut retained, arguments.len(), site)?;
                for value in arguments {
                    self.retain_value(&mut retained, value, site)?;
                }
            }
            EvalContinuation::CheckSite { .. }
            | EvalContinuation::CollectedChild(_)
            | EvalContinuation::ShortCircuitLeft { .. }
            | EvalContinuation::ShortCircuitRight { .. }
            | EvalContinuation::BinaryLeft { .. }
            | EvalContinuation::Unary { .. }
            | EvalContinuation::IfCondition { .. }
            | EvalContinuation::IfBranch(_)
            | EvalContinuation::LogicalPart(_)
            | EvalContinuation::Field { .. }
            | EvalContinuation::IndexCollection { .. }
            | EvalContinuation::FinishTopLevel { .. }
            | EvalContinuation::FinishCallable(_)
            | EvalContinuation::FinishLambda { .. }
            | EvalContinuation::RuleCondition { .. }
            | EvalContinuation::RuleValue { .. }
            | EvalContinuation::MatchScrutinee(_)
            | EvalContinuation::MatchGuard { .. }
            | EvalContinuation::MatchBody(_)
            | EvalContinuation::BlockBind { .. }
            | EvalContinuation::BlockExpression(_)
            | EvalContinuation::BuiltinCallback(_) => {}
        }
        Ok(retained)
    }

    fn push_continuation(
        &self,
        machine: &mut EndpointEvalMachine,
        site: &ExprSiteId,
        continuation: EvalContinuation,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if machine.continuations.len() >= MAX_EVALUATION_CONTINUATIONS {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!(
                    "endpoint evaluator exceeded {MAX_EVALUATION_CONTINUATIONS} heap continuations"
                ),
            ));
        }
        machine.guard_site = site.clone();
        let retained = self.budget_continuation_direct(&continuation, site)?;
        machine.continuations.push(BudgetedContinuation {
            inner: continuation,
            _retained: retained,
        });
        Ok(())
    }

    fn enter_active(
        &self,
        machine: &mut EndpointEvalMachine,
        active: ActiveDefinition,
        site: &ExprSiteId,
        detail: &'static str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.require_inactive(machine, &active, site, detail)?;
        if machine.active.len() >= MAX_CALL_DEPTH {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!("endpoint proof exceeded {MAX_CALL_DEPTH} active definitions"),
            ));
        }
        let inserted = machine.active.insert(active);
        debug_assert!(inserted, "inactive endpoint definition must be insertable");
        Ok(())
    }

    fn require_inactive(
        &self,
        machine: &EndpointEvalMachine,
        active: &ActiveDefinition,
        site: &ExprSiteId,
        detail: &'static str,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if machine.active.contains(active) {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::RecursiveCall,
                detail,
            ));
        }
        Ok(())
    }

    fn schedule_site(
        &self,
        machine: &mut EndpointEvalMachine,
        site: ExprSiteId,
        env: SharedAbstractEnv,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        machine.guard_site = site.clone();
        self.set_control(machine, EvalControl::Site { site, env })
    }

    fn deliver_value(
        &self,
        machine: &mut EndpointEvalMachine,
        value: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.set_control(machine, EvalControl::Value(value))
    }

    fn deliver_budgeted_value(
        &self,
        machine: &mut EndpointEvalMachine,
        value: BudgetedAbstractValue,
    ) {
        let (value, retained) = value.into_parts();
        machine.control = Some(EvalControl::Value(value));
        machine.control_retained = Some(retained);
    }

    fn set_control(
        &self,
        machine: &mut EndpointEvalMachine,
        control: EvalControl,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let retained = self.budget_control_direct(&control, &machine.guard_site)?;
        machine.control = Some(control);
        machine.control_retained = Some(retained);
        Ok(())
    }

    fn begin_site(
        &mut self,
        machine: &mut EndpointEvalMachine,
        site: ExprSiteId,
        env: SharedAbstractEnv,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.charge(&site)?;
        if let Some(issues) = self.resolutions.unsupported_sites.get(&site) {
            return Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                format!("checked endpoint expression has unresolved decisions: {issues:?}"),
            ));
        }
        let expression = self.index.expression(&site).ok_or_else(|| {
            self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "endpoint expression is absent from the checked semantic index",
            )
        })?;
        let expression = match &expression.kind {
            ExprKind::Var(_) => ShallowExpression::Variable,
            ExprKind::Lit(literal) => ShallowExpression::Literal(literal.clone()),
            ExprKind::App(_, arguments) => ShallowExpression::Application {
                argument_count: arguments.len(),
            },
            ExprKind::Lambda(parameters, _) => ShallowExpression::Lambda {
                parameter_count: parameters.len(),
            },
            ExprKind::BinOp(operator, _, _) => ShallowExpression::Binary {
                operator: operator.clone(),
            },
            ExprKind::UnOp(operator, _) => ShallowExpression::Unary {
                operator: operator.clone(),
            },
            ExprKind::If(_, _, _) => ShallowExpression::If,
            ExprKind::Match(scrutinee, arms) => ShallowExpression::Match {
                arm_count: arms.len(),
                allow_bare_fielded_tag: matches!(scrutinee.kind, ExprKind::Var(_))
                    && arms
                        .iter()
                        .any(|arm| matches!(&arm.pat, Pat::Con(_, fields) if fields.is_empty())),
            },
            ExprKind::Block(statements) => ShallowExpression::Block {
                statement_count: statements.len(),
            },
            ExprKind::Field(_, _) => ShallowExpression::Field,
            ExprKind::Index(_, _) => ShallowExpression::Index,
            ExprKind::List(items) => ShallowExpression::List {
                item_count: items.len(),
            },
            ExprKind::Tuple(items) => ShallowExpression::Tuple {
                item_count: items.len(),
            },
            ExprKind::Conjunction(parts) => ShallowExpression::Conjunction {
                part_count: parts.len(),
            },
            ExprKind::Disjunction(parts) => ShallowExpression::Disjunction {
                part_count: parts.len(),
            },
            ExprKind::Unit => ShallowExpression::Unit,
            ExprKind::Effect(_, _) | ExprKind::Handle { .. } => ShallowExpression::Effectful,
            ExprKind::Try(_) => ShallowExpression::Try,
            ExprKind::Pipe(_, _) => ShallowExpression::Pipe,
        };
        let resolution = self
            .resolutions
            .expressions
            .get(&site)
            .cloned()
            .ok_or_else(|| {
                self.issue(
                    &site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "endpoint expression has no checked resolution",
                )
            })?;
        self.push_continuation(
            machine,
            &site,
            EvalContinuation::CheckSite {
                site: site.clone(),
                resolved_type: resolution.resolved_type.clone(),
            },
        )?;
        self.begin_expression(machine, site, expression, resolution, env)
    }

    fn begin_expression(
        &mut self,
        machine: &mut EndpointEvalMachine,
        site: ExprSiteId,
        expression: ShallowExpression,
        resolution: CheckedExpressionResolution,
        env: SharedAbstractEnv,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        match expression {
            ShallowExpression::Variable => self.begin_variable(machine, site, resolution, env),
            ShallowExpression::Literal(literal) => {
                self.deliver_value(machine, abstract_literal(&literal))?;
                Ok(())
            }
            ShallowExpression::Application { argument_count } => {
                if argument_count > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "application argument list exceeds the endpoint proof limit",
                    ));
                }
                let mut retained = self.new_retained_budget(&site)?;
                // Reserve the backing-vector slots before allocating them.
                // Value-shape slots are charged separately as children arrive.
                self.retain_slots(&mut retained, argument_count, &site)?;
                let values = Vec::with_capacity(argument_count);
                self.set_control(
                    machine,
                    EvalControl::CollectChildren(Box::new(ChildCollectionState {
                        site: site.clone(),
                        env,
                        next_index: 0,
                        child_count: argument_count,
                        values,
                        retained,
                        kind: ChildCollectionKind::Application(resolution),
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Lambda { parameter_count } => {
                if parameter_count > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "lambda parameter list exceeds the endpoint proof limit",
                    ));
                }
                let parameters = (0..parameter_count)
                    .map(|index| {
                        structural_binder_site(&site, vec![BINDER_PARAMETER, index as u32])
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let value = AbstractValue::Callable(AbstractCallable::Lambda {
                    body_site: child_site(&site, 0),
                    parameters,
                    captured: env
                        .iter()
                        .map(|(binder, value)| (binder.clone(), value.clone()))
                        .collect::<Vec<_>>()
                        .into(),
                });
                self.deliver_value(machine, value)?;
                Ok(())
            }
            ShallowExpression::Binary { operator } if operator == "&&" || operator == "||" => {
                let left_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::ShortCircuitLeft {
                        site: site.clone(),
                        operator,
                        env: Arc::clone(&env),
                    },
                )?;
                self.schedule_site(machine, left_site, env)?;
                Ok(())
            }
            ShallowExpression::Binary { operator } => {
                let left_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::BinaryLeft {
                        site: site.clone(),
                        operator,
                        env: Arc::clone(&env),
                    },
                )?;
                self.schedule_site(machine, left_site, env)?;
                Ok(())
            }
            ShallowExpression::Unary { operator } => {
                let value_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::Unary {
                        site: site.clone(),
                        operator,
                    },
                )?;
                self.schedule_site(machine, value_site, env)?;
                Ok(())
            }
            ShallowExpression::If => {
                let condition_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::IfCondition {
                        site: site.clone(),
                        env: Arc::clone(&env),
                    },
                )?;
                self.schedule_site(machine, condition_site, env)?;
                Ok(())
            }
            ShallowExpression::Match {
                arm_count,
                allow_bare_fielded_tag,
            } => {
                if arm_count > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "match arm count exceeds the endpoint proof limit",
                    ));
                }
                let scrutinee_site = child_site(&site, 0);
                let scrutinee = AbstractValue::Unreachable;
                let remaining = AbstractValue::Unreachable;
                let mut retained = self.new_retained_budget(&site)?;
                self.retain_value(&mut retained, &scrutinee, &site)?;
                self.retain_value(&mut retained, &remaining, &site)?;
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::MatchScrutinee(Box::new(MatchState {
                        site: site.clone(),
                        env: Arc::clone(&env),
                        arm_count,
                        next_arm: 0,
                        next_child: 1,
                        allow_bare_fielded_tag,
                        scrutinee,
                        remaining,
                        results: Vec::new(),
                        retained,
                    })),
                )?;
                self.schedule_site(machine, scrutinee_site, env)?;
                Ok(())
            }
            ShallowExpression::Block { statement_count } => {
                let result = AbstractValue::Unit;
                let mut retained = self.new_retained_budget(&site)?;
                self.retain_value(&mut retained, &result, &site)?;
                self.set_control(
                    machine,
                    EvalControl::BlockNext(Box::new(BlockState {
                        site,
                        statement_count,
                        next_statement: 0,
                        env,
                        result,
                        retained,
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Field => {
                let base_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::Field {
                        site: site.clone(),
                        resolution,
                    },
                )?;
                self.schedule_site(machine, base_site, env)?;
                Ok(())
            }
            ShallowExpression::Index => {
                let collection_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::IndexCollection {
                        site: site.clone(),
                        env: Arc::clone(&env),
                    },
                )?;
                self.schedule_site(machine, collection_site, env)?;
                Ok(())
            }
            ShallowExpression::List { item_count } => {
                if item_count > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "list literal exceeds the exact endpoint proof limit",
                    ));
                }
                let mut retained = self.new_retained_budget(&site)?;
                self.retain_slots(&mut retained, item_count, &site)?;
                let values = Vec::with_capacity(item_count);
                self.set_control(
                    machine,
                    EvalControl::CollectChildren(Box::new(ChildCollectionState {
                        site: site.clone(),
                        env,
                        next_index: 0,
                        child_count: item_count,
                        values,
                        retained,
                        kind: ChildCollectionKind::List,
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Tuple { item_count } => {
                if item_count > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "tuple literal exceeds the exact endpoint proof limit",
                    ));
                }
                let mut retained = self.new_retained_budget(&site)?;
                self.retain_slots(&mut retained, item_count, &site)?;
                let values = Vec::with_capacity(item_count);
                self.set_control(
                    machine,
                    EvalControl::CollectChildren(Box::new(ChildCollectionState {
                        site: site.clone(),
                        env,
                        next_index: 0,
                        child_count: item_count,
                        values,
                        retained,
                        kind: ChildCollectionKind::Tuple,
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Conjunction { part_count } => {
                self.set_control(
                    machine,
                    EvalControl::LogicalNext(Box::new(LogicalState {
                        site,
                        env,
                        part_count,
                        next_index: 0,
                        conjunction: true,
                        result: TruthDomain::TRUE,
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Disjunction { part_count } => {
                self.set_control(
                    machine,
                    EvalControl::LogicalNext(Box::new(LogicalState {
                        site,
                        env,
                        part_count,
                        next_index: 0,
                        conjunction: false,
                        result: TruthDomain::FALSE,
                    })),
                )?;
                Ok(())
            }
            ShallowExpression::Unit => {
                self.deliver_value(machine, AbstractValue::Unit)?;
                Ok(())
            }
            ShallowExpression::Effectful => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::EffectfulCall,
                "effectful expression is not admissible in a mechanism endpoint proof",
            )),
            ShallowExpression::Try => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                "propagating try requires a separate success-variant proof",
            )),
            ShallowExpression::Pipe => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::UnknownCall,
                "pipe dispatch has no exact checked endpoint definition",
            )),
        }
    }

    fn begin_variable(
        &mut self,
        machine: &mut EndpointEvalMachine,
        site: ExprSiteId,
        resolution: CheckedExpressionResolution,
        env: SharedAbstractEnv,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let value = match resolution.value_binding.as_ref() {
            Some(CheckedValueBinding::Binder { site: binder, .. }) => {
                env.get(binder).cloned().ok_or_else(|| {
                    self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "checked binder has no abstract endpoint value",
                    )
                })?
            }
            Some(CheckedValueBinding::TopLevel(binding)) => {
                self.set_control(
                    machine,
                    EvalControl::TopLevel {
                        binding: binding.clone(),
                        use_site: site,
                    },
                )?;
                return Ok(());
            }
            Some(CheckedValueBinding::Callable(callable)) => {
                AbstractValue::Callable(AbstractCallable::Function(callable.clone()))
            }
            Some(CheckedValueBinding::RuleFamily(family)) => {
                AbstractValue::Callable(AbstractCallable::RuleFamily(family.clone()))
            }
            Some(CheckedValueBinding::Constructor { .. }) => {
                let identity = resolution.exact_constructor.clone().ok_or_else(|| {
                    self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "constructor value has no exact checked identity",
                    )
                })?;
                if !identity.fields.is_empty() {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::UnknownCall,
                        "non-nullary constructor used without a checked application",
                    ));
                }
                single_constructor(identity, Box::new([]))
            }
            Some(CheckedValueBinding::OpaqueQualifiedOwner(_)) | None => {
                return Err(self.issue(
                    &site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "value has no exact checked endpoint binding",
                ))
            }
        };
        self.deliver_value(machine, value)?;
        Ok(())
    }

    fn resume_evaluation(
        &mut self,
        machine: &mut EndpointEvalMachine,
        continuation: EvalContinuation,
        value: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        match continuation {
            EvalContinuation::CheckSite {
                site,
                resolved_type,
            } => {
                // Every evaluated syntax node establishes this postcondition
                // before its value can enter caches, joins, derived ordering,
                // or recursive destruction. Call boundaries alone are too
                // late for deeply nested local/constructor values.
                self.require_bounded_value(&value, &site)?;
                self.require_checked_expression_type(&site, &resolved_type, &value)?;
                self.deliver_value(machine, value)?;
            }
            EvalContinuation::CollectedChild(mut state) => {
                let offset = usize::from(matches!(state.kind, ChildCollectionKind::Application(_)));
                let value_site = child_site(&state.site, state.next_index + offset);
                self.retain_value(&mut state.retained, &value, &value_site)?;
                state.values.push(value);
                state.next_index += 1;
                self.set_control(machine, EvalControl::CollectChildren(state))?;
            }
            EvalContinuation::ShortCircuitLeft {
                site,
                operator,
                env,
            } => {
                let left_site = child_site(&site, 0);
                let left = value.truth().ok_or_else(|| {
                    self.issue(
                        &left_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "short-circuit left operand is not a proved Bool",
                    )
                })?;
                if (operator == "&&" && left == TruthDomain::FALSE)
                    || (operator == "||" && left == TruthDomain::TRUE)
                {
                    self.deliver_value(machine, AbstractValue::Bool(left))?;
                } else {
                    let right_site = child_site(&site, 1);
                    let right_env = self.shared_env(
                        self.refined_env_for_condition(&left_site, operator == "&&", &env),
                        &left_site,
                    )?;
                    if env_is_unreachable(&right_env) {
                        self.deliver_value(
                            machine,
                            AbstractValue::Bool(if operator == "&&" {
                                TruthDomain::FALSE
                            } else {
                                TruthDomain::TRUE
                            }),
                        )?;
                    } else {
                        self.push_continuation(
                            machine,
                            &site,
                            EvalContinuation::ShortCircuitRight {
                                site: site.clone(),
                                operator,
                                left,
                            },
                        )?;
                        self.schedule_site(machine, right_site, right_env)?;
                    }
                }
            }
            EvalContinuation::ShortCircuitRight {
                site,
                operator,
                left,
            } => {
                let right_site = child_site(&site, 1);
                let right = value.truth().ok_or_else(|| {
                    self.issue(
                        &right_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "short-circuit right operand is not a proved Bool",
                    )
                })?;
                let result = if operator == "&&" {
                    match (left.singleton(), right.singleton()) {
                        (Some(false), _) | (_, Some(false)) => TruthDomain::FALSE,
                        (Some(true), Some(true)) => TruthDomain::TRUE,
                        _ => TruthDomain::BOTH,
                    }
                } else {
                    match (left.singleton(), right.singleton()) {
                        (Some(true), _) | (_, Some(true)) => TruthDomain::TRUE,
                        (Some(false), Some(false)) => TruthDomain::FALSE,
                        _ => TruthDomain::BOTH,
                    }
                };
                self.deliver_value(machine, AbstractValue::Bool(result))?;
            }
            EvalContinuation::BinaryLeft {
                site,
                operator,
                env,
            } => {
                let right_site = child_site(&site, 1);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::BinaryRight {
                        site: site.clone(),
                        operator,
                        left: value,
                    },
                )?;
                self.schedule_site(machine, right_site, env)?;
            }
            EvalContinuation::BinaryRight {
                site,
                operator,
                left,
            } => {
                let result = self.eval_binop(&operator, left, value, &site)?;
                self.deliver_value(machine, result)?;
            }
            EvalContinuation::Unary { site, operator } => {
                let result = self.eval_unop(&operator, value, &site)?;
                self.deliver_value(machine, result)?;
            }
            EvalContinuation::IfCondition { site, env } => {
                let condition_site = child_site(&site, 0);
                let truth = value.truth().ok_or_else(|| {
                    self.issue(
                        &condition_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "if condition is not a proved Bool",
                    )
                })?;
                let mut branches = Vec::with_capacity(2);
                if truth.may_be_true() {
                    let then_env = self.shared_env(
                        self.refined_env_for_condition(&condition_site, true, &env),
                        &condition_site,
                    )?;
                    if !env_is_unreachable(&then_env) {
                        branches.push((child_site(&site, 1), then_env));
                    }
                }
                if truth.may_be_false() {
                    let else_env = self.shared_env(
                        self.refined_env_for_condition(&condition_site, false, &env),
                        &condition_site,
                    )?;
                    if !env_is_unreachable(&else_env) {
                        branches.push((child_site(&site, 2), else_env));
                    }
                }
                self.set_control(
                    machine,
                    EvalControl::IfBranches(Box::new(IfBranchState {
                        site: site.clone(),
                        branches,
                        next_index: 0,
                        results: Vec::new(),
                        retained: self.new_retained_budget(&site)?,
                    })),
                )?;
            }
            EvalContinuation::IfBranch(mut state) => {
                self.retain_value(&mut state.retained, &value, &state.site)?;
                state.results.push(value);
                self.set_control(machine, EvalControl::IfBranches(state))?;
            }
            EvalContinuation::LogicalPart(mut state) => {
                let part_index = state.next_index.saturating_sub(1);
                let part_site = child_site(&state.site, part_index);
                let part = value.truth().ok_or_else(|| {
                    self.issue(
                        &part_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "logical aggregate member is not a proved Bool",
                    )
                })?;
                state.result = if state.conjunction {
                    if state.result == TruthDomain::TRUE {
                        part
                    } else if part == TruthDomain::FALSE {
                        TruthDomain::FALSE
                    } else {
                        TruthDomain::BOTH
                    }
                } else if state.result == TruthDomain::FALSE {
                    part
                } else if part == TruthDomain::TRUE {
                    TruthDomain::TRUE
                } else {
                    TruthDomain::BOTH
                };
                state.env = self.shared_env(
                    self.refined_env_for_condition(&part_site, state.conjunction, &state.env),
                    &part_site,
                )?;
                self.set_control(machine, EvalControl::LogicalNext(state))?;
            }
            EvalContinuation::Field { site, resolution } => {
                let result = self.project_field(&site, &resolution, value)?;
                self.deliver_value(machine, result)?;
            }
            EvalContinuation::IndexCollection { site, env } => {
                let index_site = child_site(&site, 1);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::IndexValue {
                        site: site.clone(),
                        collection: value,
                    },
                )?;
                self.schedule_site(machine, index_site, env)?;
            }
            EvalContinuation::IndexValue { site, collection } => {
                let result = self.finish_index(&site, collection, value)?;
                self.deliver_budgeted_value(machine, result);
            }
            EvalContinuation::BoundCallable { site, arguments } => {
                self.set_control(
                    machine,
                    EvalControl::Apply {
                        callable: value,
                        arguments,
                        call_site: site,
                    },
                )?;
            }
            EvalContinuation::ScopedReceiver {
                site,
                family,
                arguments,
            } => {
                let captures = self.shared_env(
                    self.scoped_receiver_captures_from_value(&site, &family, value)?,
                    &site,
                )?;
                self.set_control(
                    machine,
                    EvalControl::RuleFamily {
                        family,
                        arguments,
                        captures,
                        call_site: site,
                    },
                )?;
            }
            EvalContinuation::FinishTopLevel { active, cache_key } => {
                machine.active.remove(&active);
                if let Some(cached) = self.cached_value(&value, &machine.guard_site) {
                    self.top_level_cache.borrow_mut().insert(cache_key, cached);
                }
                self.deliver_value(machine, value)?;
            }
            EvalContinuation::FinishCallable(mut state) => {
                machine.active.remove(&state.active);
                if let Some(expected_result) = state.expected_result.as_ref() {
                    self.require_value_type(
                        &state.body_site,
                        &format!("result of endpoint helper `{}`", state.runtime_name),
                        expected_result,
                        &value,
                        &mut state.substitutions,
                    )?;
                }
                self.record(
                    &state.call_site,
                    ObligationKind::Callable,
                    &state.call_input,
                    &value,
                )?;
                if let Some(cached) = self.cached_value(&value, &state.call_site) {
                    self.callable_cache
                        .borrow_mut()
                        .insert((self.role, state.callable, state.argument_root), cached);
                }
                self.deliver_value(machine, value)?;
            }
            EvalContinuation::FinishLambda {
                active,
                call_site,
                call_input,
                ..
            } => {
                machine.active.remove(&active);
                self.record(&call_site, ObligationKind::Callable, &call_input, &value)?;
                self.deliver_value(machine, value)?;
            }
            EvalContinuation::RuleCondition { state, candidate } => {
                self.resume_rule_condition(machine, state, candidate, value)?;
            }
            EvalContinuation::RuleValue { state, candidate } => {
                self.resume_rule_value(machine, state, candidate, value)?;
            }
            EvalContinuation::MatchScrutinee(mut state) => {
                if value.is_unreachable() {
                    self.deliver_value(machine, AbstractValue::Unreachable)?;
                } else {
                    // The matcher keeps both the original scrutinee and a
                    // destructively refined remainder.
                    self.replace_retained_value(
                        &mut state.retained,
                        &state.scrutinee,
                        &value,
                        &state.site,
                    )?;
                    self.replace_retained_value(
                        &mut state.retained,
                        &state.remaining,
                        &value,
                        &state.site,
                    )?;
                    state.scrutinee = value.clone();
                    state.remaining = value;
                    self.set_control(machine, EvalControl::MatchNext(state))?;
                }
            }
            EvalContinuation::MatchGuard { state, arm } => {
                let guard_site = arm
                    .guard_site
                    .as_ref()
                    .expect("match guard continuation has a guard site");
                let truth = value.truth().ok_or_else(|| {
                    self.issue(
                        guard_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "match guard is not a proved Bool",
                    )
                })?;
                self.continue_match_after_guard(machine, state, arm, truth)?;
            }
            EvalContinuation::MatchBody(mut state) => {
                self.retain_value(&mut state.retained, &value, &state.site)?;
                state.results.push(value);
                self.set_control(machine, EvalControl::MatchNext(state))?;
            }
            EvalContinuation::BlockBind {
                mut state,
                statement_site,
                pattern,
            } => {
                // `as` and destructuring patterns may retain several clones
                // of one source. Charge each actual binder before the binding
                // operation can allocate those clones.
                self.retain_value_copies(
                    &mut state.retained,
                    &value,
                    pattern_binder_count(&pattern),
                    &statement_site,
                )?;
                let mut bindings = state.env.bindings.clone();
                let matched =
                    self.bind_pattern(&statement_site, &pattern, &value, &[], &mut bindings)?;
                if !matched.definitely_all {
                    return Err(self.issue(
                        &statement_site,
                        RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                        "local binding pattern is not irrefutable over its endpoint value",
                    ));
                }
                state.env = self.shared_env(bindings, &statement_site)?;
                self.set_control(machine, EvalControl::BlockNext(state))?;
            }
            EvalContinuation::BlockExpression(mut state) => {
                self.replace_retained_value(
                    &mut state.retained,
                    &state.result,
                    &value,
                    &state.site,
                )?;
                state.result = value;
                self.set_control(machine, EvalControl::BlockNext(state))?;
            }
            EvalContinuation::BuiltinCallback(state) => {
                self.resume_builtin(machine, state, value)?;
            }
        }
        Ok(())
    }

    fn continue_child_collection(
        &mut self,
        machine: &mut EndpointEvalMachine,
        state: Box<ChildCollectionState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if state.next_index < state.child_count {
            let offset = usize::from(matches!(state.kind, ChildCollectionKind::Application(_)));
            let child = child_site(&state.site, state.next_index + offset);
            let env = Arc::clone(&state.env);
            let site = state.site.clone();
            self.push_continuation(machine, &site, EvalContinuation::CollectedChild(state))?;
            self.schedule_site(machine, child, env)?;
            return Ok(());
        }
        let ChildCollectionState {
            site,
            env,
            mut values,
            kind,
            ..
        } = *state;
        match kind {
            ChildCollectionKind::List => self.deliver_value(
                machine,
                AbstractValue::List(AbstractSequence::Exact(values.into_boxed_slice())),
            )?,
            ChildCollectionKind::Tuple => {
                self.deliver_value(machine, AbstractValue::Tuple(values.into_boxed_slice()))?
            }
            ChildCollectionKind::Application(resolution) => {
                let argument_count = values.len();
                let arguments = canonical_arguments(
                    &mut values,
                    resolution.named_arguments.as_ref(),
                    &site,
                    self.role,
                )?;
                let argument_sites = canonical_argument_sites(
                    argument_count,
                    resolution.named_arguments.as_ref(),
                    &site,
                    self.role,
                )?;
                self.begin_application_dispatch(
                    machine,
                    site,
                    resolution,
                    arguments,
                    argument_sites,
                    env,
                )?;
            }
        }
        Ok(())
    }

    fn continue_if_branches(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<IfBranchState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if let Some((site, env)) = state.branches.get(state.next_index).cloned() {
            state.next_index += 1;
            let parent_site = state.site.clone();
            self.push_continuation(machine, &parent_site, EvalContinuation::IfBranch(state))?;
            self.schedule_site(machine, site, env)?;
            return Ok(());
        }
        let result = join_values(state.results).ok_or_else(|| {
            self.issue(
                &state.site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "if expression has no abstractly reachable branch",
            )
        })?;
        self.deliver_value(machine, result)?;
        Ok(())
    }

    fn continue_logical(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<LogicalState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let short_circuited = (state.conjunction && state.result == TruthDomain::FALSE)
            || (!state.conjunction && state.result == TruthDomain::TRUE);
        if short_circuited || state.next_index >= state.part_count || env_is_unreachable(&state.env)
        {
            self.deliver_value(machine, AbstractValue::Bool(state.result))?;
            return Ok(());
        }
        let part_site = child_site(&state.site, state.next_index);
        state.next_index += 1;
        let env = Arc::clone(&state.env);
        let parent_site = state.site.clone();
        self.push_continuation(machine, &parent_site, EvalContinuation::LogicalPart(state))?;
        self.schedule_site(machine, part_site, env)?;
        Ok(())
    }

    fn begin_application_dispatch(
        &mut self,
        machine: &mut EndpointEvalMachine,
        site: ExprSiteId,
        resolution: CheckedExpressionResolution,
        arguments: Vec<AbstractValue>,
        argument_sites: Vec<ExprSiteId>,
        env: SharedAbstractEnv,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        match resolution.call_target.clone() {
            Some(CheckedCallTarget::Builtin {
                canonical_name,
                arity,
            }) if arity == arguments.len() => {
                self.begin_builtin(
                    machine,
                    canonical_name.as_ref(),
                    arguments,
                    argument_sites,
                    site,
                )
            }
            Some(CheckedCallTarget::Constructor { arity, .. })
                if arity == arguments.len() =>
            {
                let identity = resolution.exact_constructor.ok_or_else(|| {
                    self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "constructor application has no exact checked identity",
                    )
                })?;
                if identity.fields.len() != arguments.len() {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "constructor application arity disagrees with checked identity",
                    ));
                }
                self.deliver_value(
                    machine,
                    single_constructor(identity, arguments.into_boxed_slice()),
                )?;
                Ok(())
            }
            Some(CheckedCallTarget::Function { callable, arity })
                if arity == arguments.len() =>
            {
                self.set_control(machine, EvalControl::Callable {
                    callable,
                    arguments,
                    call_site: site,
                })?;
                Ok(())
            }
            Some(CheckedCallTarget::RuleFamily(family)) if family.arity == arguments.len() => {
                let captures = self.shared_env(
                    self.ambient_scoped_captures(&family, &env, &site)?,
                    &site,
                )?;
                self.set_control(machine, EvalControl::RuleFamily {
                    family,
                    arguments,
                    captures,
                    call_site: site,
                })?;
                Ok(())
            }
            Some(CheckedCallTarget::ScopedMember {
                owner_type,
                member,
                arity,
                rule_family: Some(family),
            }) if arity == arguments.len() && family.arity == arguments.len() => {
                let _ = (owner_type, member);
                let function_site = child_site(&site, 0);
                let function = self.index.expression(&function_site).ok_or_else(|| {
                    self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "scoped call function expression is absent from the checked index",
                    )
                })?;
                if !matches!(&function.kind, ExprKind::Field(_, _)) {
                    return Err(self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "checked scoped call target is not rooted at a field expression",
                    ));
                }
                let receiver_site = child_site(&function_site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::ScopedReceiver {
                        site: site.clone(),
                        family,
                        arguments,
                    },
                )?;
                self.schedule_site(machine, receiver_site, env)?;
                Ok(())
            }
            Some(CheckedCallTarget::ScopedMember {
                owner_type,
                member,
                arity,
                rule_family: None,
            }) if arity == arguments.len() => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::UnknownCall,
                format!(
                    "ordinary scoped endpoint call {owner_type}::{member}/{arity} has no checked receiver-closure identity"
                ),
            )),
            Some(CheckedCallTarget::BoundCallable { arity, .. })
                if arity == arguments.len() =>
            {
                let callable_site = child_site(&site, 0);
                self.push_continuation(
                    machine,
                    &site,
                    EvalContinuation::BoundCallable {
                        site: site.clone(),
                        arguments,
                    },
                )?;
                self.schedule_site(machine, callable_site, env)?;
                Ok(())
            }
            Some(target) => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::UnknownCall,
                format!("checked endpoint call target has incompatible arity: {target:?}"),
            )),
            None => Err(self.issue(
                &site,
                RelationalEndpointTotalityIssueReason::UnknownCall,
                "endpoint application has no exact checked call target",
            )),
        }
    }

    fn begin_top_level(
        &mut self,
        machine: &mut EndpointEvalMachine,
        binding: CheckedTopLevelBindingId,
        use_site: ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let cache_key = (self.role, binding.clone());
        let active = ActiveDefinition::TopLevel(binding.clone());
        self.require_inactive(
            machine,
            &active,
            &use_site,
            "recursive top-level endpoint binding",
        )?;
        let cached_value = {
            let cache = self.top_level_cache.borrow();
            cache
                .get(&cache_key)
                .map(|cached| self.try_budgeted_cache_clone(&cached.value))
        };
        match cached_value {
            Some(Some(value)) => {
                self.deliver_budgeted_value(machine, value);
                return Ok(());
            }
            Some(None) => self.shed_optional_caches(),
            None => {}
        }
        self.enter_active(
            machine,
            active.clone(),
            &use_site,
            "recursive top-level endpoint binding",
        )?;
        if binding.binder_path.as_ref() != [BINDER_PATTERN] {
            return Err(self.issue(
                &use_site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                "destructuring top-level endpoint bindings are not yet normalized",
            ));
        }
        let declaration = self
            .index
            .declarations
            .get(&binding.declaration)
            .copied()
            .ok_or_else(|| {
                self.issue(
                    &use_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "top-level endpoint binding declaration is absent",
                )
            })?;
        if !matches!(
            declaration.statement.as_ref(),
            Stmt::Bind(Pat::Var(_), _, _)
        ) {
            return Err(self.issue(
                &use_site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                "top-level endpoint binding is not one simple immutable value",
            ));
        }
        let initializer_site = ExprSiteId {
            analysis_program: use_site.analysis_program.clone(),
            declaration: binding.declaration.declaration.clone(),
            normalized_declaration_ordinal: binding.declaration.normalized_ordinal,
            ast_path: vec![0].into_boxed_slice(),
        };
        self.push_continuation(
            machine,
            &use_site,
            EvalContinuation::FinishTopLevel { active, cache_key },
        )?;
        let initializer_env = self.shared_env(AbstractEnv::new(), &initializer_site)?;
        self.schedule_site(machine, initializer_site, initializer_env)?;
        Ok(())
    }

    fn begin_callable(
        &mut self,
        machine: &mut EndpointEvalMachine,
        callable_id: CheckedCallableId,
        arguments: Vec<AbstractValue>,
        call_site: ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let call_input_retained = self.new_tuple_clone_budget(arguments.iter(), &call_site)?;
        let call_input = AbstractValue::Tuple(arguments.clone().into_boxed_slice());
        self.require_bounded_value(&call_input, &call_site)?;
        let argument_root = abstract_value_root(&call_input);
        let callable = self.index.callables.get(&callable_id).ok_or_else(|| {
            self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "endpoint helper has no exact checked callable definition",
            )
        })?;
        if callable.parameter_sites.len() != arguments.len()
            || callable.parameters.len() != arguments.len()
        {
            return Err(self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "endpoint helper argument count disagrees with checked parameters",
            ));
        }

        let contract = self
            .resolutions
            .callable_type_contracts
            .get(&callable_id)
            .cloned();
        if let Some(contract) = contract.as_ref() {
            if contract.parameter_types.len() != callable.parameters.len() {
                return Err(self.issue(
                    &call_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "checked endpoint helper type contract has incompatible arity",
                ));
            }
            if callable.return_type != Some(&contract.result_type)
                || callable
                    .parameters
                    .iter()
                    .zip(contract.parameter_types.iter())
                    .any(|(parameter, expected)| parameter.ty.as_ref() != Some(expected))
            {
                return Err(self.issue(
                    &call_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "checked endpoint helper type contract disagrees with its source declaration",
                ));
            }
        }

        let mut substitutions = BTreeMap::new();
        for (index, (argument, parameter)) in
            arguments.iter().zip(callable.parameters.iter()).enumerate()
        {
            let expected = contract
                .as_ref()
                .and_then(|contract| contract.parameter_types.get(index))
                .or(parameter.ty.as_ref());
            if let Some(expected) = expected.filter(|ty| !matches!(ty, Ty::Hole)) {
                self.require_value_type(
                    &call_site,
                    &format!(
                        "argument {} of endpoint helper `{}`",
                        index + 1,
                        callable.runtime_name
                    ),
                    expected,
                    argument,
                    &mut substitutions,
                )?;
            }
        }
        let expected_result = contract
            .as_ref()
            .map(|contract| contract.result_type.clone())
            .or_else(|| callable.return_type.cloned())
            .filter(|ty| !matches!(ty, Ty::Hole));
        let runtime_name = callable.runtime_name.to_owned();
        let body_site = callable.body_site.clone();
        let parameter_sites = callable.parameter_sites.clone();
        let effects_empty = callable.effects.is_empty();
        let active = ActiveDefinition::Callable(callable_id.clone());
        self.require_inactive(
            machine,
            &active,
            &call_site,
            "recursive endpoint helper call",
        )?;

        let cached_value = {
            let cache = self.callable_cache.borrow();
            cache
                .get(&(self.role, callable_id.clone(), argument_root))
                .map(|cached| self.try_budgeted_cache_clone(&cached.value))
        };
        match cached_value {
            Some(Some(value)) => {
                if let Some(expected_result) = expected_result.as_ref() {
                    self.require_value_type(
                        &call_site,
                        &format!("cached result of endpoint helper `{runtime_name}`"),
                        expected_result,
                        &value.value,
                        &mut substitutions,
                    )?;
                }
                self.record(
                    &call_site,
                    ObligationKind::Callable,
                    &call_input,
                    &value.value,
                )?;
                self.deliver_budgeted_value(machine, value);
                return Ok(());
            }
            Some(None) => self.shed_optional_caches(),
            None => {}
        }

        self.enter_active(
            machine,
            active.clone(),
            &call_site,
            "recursive endpoint helper call",
        )?;
        if !effects_empty {
            return Err(self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::EffectfulCall,
                format!("endpoint helper `{runtime_name}` declares effects"),
            ));
        }
        let env = parameter_sites
            .into_vec()
            .into_iter()
            .zip(arguments)
            .collect::<AbstractEnv>();
        self.push_continuation(
            machine,
            &call_site,
            EvalContinuation::FinishCallable(Box::new(CallableFinishState {
                active,
                callable: callable_id,
                argument_root,
                call_site: call_site.clone(),
                body_site: body_site.clone(),
                call_input,
                runtime_name,
                expected_result,
                substitutions,
                _retained: call_input_retained,
            })),
        )?;
        let body_env = self.shared_env(env, &body_site)?;
        self.schedule_site(machine, body_site, body_env)?;
        Ok(())
    }

    fn begin_apply(
        &mut self,
        machine: &mut EndpointEvalMachine,
        callable: AbstractValue,
        arguments: Vec<AbstractValue>,
        call_site: ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        match callable {
            AbstractValue::Callable(AbstractCallable::Function(callable)) => {
                self.set_control(
                    machine,
                    EvalControl::Callable {
                        callable,
                        arguments,
                        call_site,
                    },
                )?;
            }
            AbstractValue::Callable(AbstractCallable::RuleFamily(family)) => {
                self.set_control(
                    machine,
                    EvalControl::RuleFamily {
                        family,
                        arguments,
                        captures: self.shared_env(AbstractEnv::new(), &call_site)?,
                        call_site,
                    },
                )?;
            }
            AbstractValue::Callable(AbstractCallable::Lambda {
                body_site,
                parameters,
                captured,
            }) => {
                if parameters.len() != arguments.len() {
                    return Err(self.issue(
                        &call_site,
                        RelationalEndpointTotalityIssueReason::UnknownCall,
                        "lambda argument count disagrees with checked parameters",
                    ));
                }
                let active = ActiveDefinition::Lambda(body_site.clone());
                self.enter_active(
                    machine,
                    active.clone(),
                    &call_site,
                    "recursive endpoint lambda call",
                )?;
                let call_input_retained =
                    self.new_tuple_clone_budget(arguments.iter(), &call_site)?;
                let call_input = AbstractValue::Tuple(arguments.clone().into_boxed_slice());
                let mut env = captured.iter().cloned().collect::<AbstractEnv>();
                env.extend(parameters.into_vec().into_iter().zip(arguments));
                self.push_continuation(
                    machine,
                    &call_site,
                    EvalContinuation::FinishLambda {
                        active,
                        call_site: call_site.clone(),
                        call_input,
                        _retained: call_input_retained,
                    },
                )?;
                let body_env = self.shared_env(env, &body_site)?;
                self.schedule_site(machine, body_site, body_env)?;
            }
            _ => {
                return Err(self.issue(
                    &call_site,
                    RelationalEndpointTotalityIssueReason::UnknownCall,
                    "higher-order endpoint call has no exact callable identity",
                ))
            }
        }
        Ok(())
    }

    fn begin_rule_family(
        &mut self,
        machine: &mut EndpointEvalMachine,
        family: RuleDispatchKey,
        arguments: Vec<AbstractValue>,
        captures: SharedAbstractEnv,
        call_site: ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if family.arity != arguments.len() {
            return Err(self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule-family arity disagrees with canonical arguments",
            ));
        }
        let contract = self
            .resolutions
            .rule_dispatch_type_contracts
            .get(&family)
            .cloned()
            .ok_or_else(|| {
                self.issue(
                    &call_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "checked rule family has no type-only dispatch contract",
                )
            })?;
        if contract.parameter_types.len() != arguments.len() {
            return Err(self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule-family type contract has incompatible arity",
            ));
        }
        let mut substitutions = BTreeMap::new();
        for (index, (argument, expected)) in arguments
            .iter()
            .zip(contract.parameter_types.iter())
            .enumerate()
        {
            if let Some(expected) = expected.as_ref().filter(|ty| !matches!(ty, Ty::Hole)) {
                self.require_value_type(
                    &call_site,
                    &format!(
                        "argument {} of rule family `{}/{}`",
                        index + 1,
                        family.name,
                        family.arity
                    ),
                    expected,
                    argument,
                    &mut substitutions,
                )?;
            }
        }
        let mut retained =
            self.new_pair_of_tuples_clone_budget(arguments.iter(), captures.values(), &call_site)?;
        let capture_values = captures.values().cloned().collect::<Vec<_>>();
        let call_input = AbstractValue::Tuple(
            vec![
                AbstractValue::Tuple(arguments.clone().into_boxed_slice()),
                AbstractValue::Tuple(capture_values.into_boxed_slice()),
            ]
            .into_boxed_slice(),
        );
        self.require_bounded_value(&call_input, &call_site)?;
        let argument_root = abstract_value_root(&call_input);
        let active = ActiveDefinition::Rule(family.clone());
        self.require_inactive(
            machine,
            &active,
            &call_site,
            "recursive endpoint rule-family call",
        )?;
        let cached_value = {
            let cache = self.rule_family_cache.borrow();
            cache
                .get(&(self.role, family.clone(), argument_root))
                .map(|cached| self.try_budgeted_cache_clone(&cached.value))
        };
        match cached_value {
            Some(Some(value)) => {
                self.require_value_type(
                    &call_site,
                    "cached rule-family result",
                    &contract.result_type,
                    &value.value,
                    &mut substitutions,
                )?;
                self.record(
                    &call_site,
                    ObligationKind::Dispatch,
                    &call_input,
                    &value.value,
                )?;
                self.deliver_budgeted_value(machine, value);
                return Ok(());
            }
            Some(None) => self.shed_optional_caches(),
            None => {}
        }

        let resolution = self.resolutions.rule_families.get(&family).ok_or_else(|| {
            self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule-family target is absent from the resolution snapshot",
            )
        })?;
        if resolution.key != family {
            return Err(self.issue(
                &call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule-family resolution carries a different dispatch key",
            ));
        }

        self.enter_active(
            machine,
            active.clone(),
            &call_site,
            "recursive endpoint rule-family call",
        )?;
        self.retain_slots(&mut retained, arguments.len(), &call_site)?;
        for argument in &arguments {
            self.retain_value(&mut retained, argument, &call_site)?;
        }
        let predicate_bdd = match DispatchPredicateBdd::new(Rc::clone(&self.retention)) {
            Ok(predicate_bdd) => predicate_bdd,
            Err(DispatchBddError::RetentionLimit) => {
                self.shed_optional_caches();
                DispatchPredicateBdd::new(Rc::clone(&self.retention))
                    .map_err(|error| self.dispatch_issue(&call_site, error))?
            }
            Err(error) => return Err(self.dispatch_issue(&call_site, error)),
        };
        self.set_control(
            machine,
            EvalControl::RuleNext(Box::new(RuleState {
                active,
                family: family.clone(),
                arguments,
                captures,
                call_site,
                call_input,
                argument_root,
                result_type: contract.result_type,
                substitutions,
                boolean_miss_safe: self
                    .resolutions
                    .rule_dispatch_boolean_miss_safe_keys
                    .contains(&family),
                runtime_irrefutable: self
                    .resolutions
                    .rule_dispatch_runtime_irrefutable_keys
                    .contains(&family),
                next_candidate: 0,
                results: Vec::new(),
                retained,
                predicate_bdd,
                residual: DispatchPredicateBdd::ALL,
                false_backtrack_coverage: DispatchPredicateBdd::EMPTY,
            })),
        )?;
        Ok(())
    }

    fn continue_rule(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<RuleState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let candidate = self
            .resolutions
            .rule_families
            .get(&state.family)
            .and_then(|resolution| resolution.candidates.get(state.next_candidate))
            .cloned();
        if state.residual == DispatchPredicateBdd::EMPTY || candidate.is_none() {
            return self.finish_rule(machine, state);
        }
        let candidate = candidate.expect("candidate checked above");
        state.next_candidate += 1;
        self.charge(&candidate.head_site)?;
        let mut candidate_env = state.captures.bindings.clone();
        let head_match = self.bind_rule_head(
            &candidate.head_site,
            &state.arguments,
            &mut candidate_env,
            &state.call_site,
        )?;
        if head_match == HeadMatch::No {
            self.set_control(machine, EvalControl::RuleNext(state))?;
            return Ok(());
        }
        let mut candidate_retained = self.new_retained_budget(&candidate.head_site)?;
        // At most one fixed-size origin entry can be minted per canonical
        // argument. Reserve that upper bound before the map allocates.
        self.retain_slots(
            &mut candidate_retained,
            state.arguments.len(),
            &candidate.head_site,
        )?;
        let origins = self.dispatch_argument_origins(&candidate.head_site, state.arguments.len());
        let candidate_site = candidate.head_site.clone();
        let candidate_state = Box::new(RuleCandidateState {
            candidate,
            env: self.shared_env(candidate_env, &candidate_site)?,
            head_match,
            origins,
            guard_domain: Some(DispatchPredicateBdd::ALL),
            _retained: candidate_retained,
        });
        if let Some(condition_site) = candidate_state.candidate.condition_site.clone() {
            let candidate_env = Arc::clone(&candidate_state.env);
            self.push_continuation(
                machine,
                &condition_site,
                EvalContinuation::RuleCondition {
                    state,
                    candidate: candidate_state,
                },
            )?;
            self.schedule_site(machine, condition_site, candidate_env)?;
        } else {
            self.begin_rule_value(machine, state, candidate_state)?;
        }
        Ok(())
    }

    fn resume_rule_condition(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<RuleState>,
        mut candidate: Box<RuleCandidateState>,
        value: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let condition_site = candidate
            .candidate
            .condition_site
            .as_ref()
            .expect("condition continuation has a condition site");
        let truth = value.truth().ok_or_else(|| {
            self.issue(
                condition_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "rule condition is not a proved Bool",
            )
        })?;
        if !truth.may_be_true() {
            self.set_control(machine, EvalControl::RuleNext(state))?;
            return Ok(());
        }
        let guard_domain = match truth {
            TruthDomain::TRUE => Some(DispatchPredicateBdd::ALL),
            TruthDomain::FALSE => Some(DispatchPredicateBdd::EMPTY),
            _ => self.canonical_dispatch_condition(
                condition_site,
                &candidate.origins,
                &mut state.predicate_bdd,
            )?,
        };
        if let Some(guard_domain) = guard_domain {
            let selected = self
                .retry_dispatch_after_cache_shed(|| {
                    state.predicate_bdd.and(state.residual, guard_domain)
                })
                .map_err(|error| self.dispatch_issue(condition_site, error))?;
            if selected == DispatchPredicateBdd::EMPTY {
                self.set_control(machine, EvalControl::RuleNext(state))?;
                return Ok(());
            }
        }
        candidate.guard_domain = guard_domain;
        candidate.env = self.shared_env(
            self.refined_env_for_condition(condition_site, true, &candidate.env),
            condition_site,
        )?;
        if env_is_unreachable(&candidate.env) {
            self.set_control(machine, EvalControl::RuleNext(state))?;
            return Ok(());
        }
        self.begin_rule_value(machine, state, candidate)
    }

    fn begin_rule_value(
        &mut self,
        machine: &mut EndpointEvalMachine,
        state: Box<RuleState>,
        candidate: Box<RuleCandidateState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if let Some(value_site) = candidate.candidate.value_site.clone() {
            let env = Arc::clone(&candidate.env);
            self.push_continuation(
                machine,
                &value_site,
                EvalContinuation::RuleValue { state, candidate },
            )?;
            self.schedule_site(machine, value_site, env)?;
            Ok(())
        } else if candidate.candidate.tier == RuleDispatchTier::Clause {
            self.resume_rule_value(
                machine,
                state,
                candidate,
                AbstractValue::Bool(TruthDomain::TRUE),
            )
        } else {
            Err(self.issue(
                &candidate.candidate.head_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "non-clause rule candidate has no checked value site",
            ))
        }
    }

    fn resume_rule_value(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<RuleState>,
        candidate: Box<RuleCandidateState>,
        value: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let result_site = candidate
            .candidate
            .value_site
            .as_ref()
            .unwrap_or(&candidate.candidate.head_site);
        self.require_value_type(
            result_site,
            "reachable rule-candidate result",
            &state.result_type,
            &value,
            &mut state.substitutions,
        )?;

        let mut covers_guard_domain = candidate.candidate.tier != RuleDispatchTier::Clause;
        if candidate.candidate.tier == RuleDispatchTier::Clause {
            match value {
                AbstractValue::Bool(truth) => {
                    if truth.may_be_true() {
                        let result = AbstractValue::Bool(TruthDomain::TRUE);
                        self.retain_value(&mut state.retained, &result, result_site)?;
                        state.results.push(result);
                    }
                    if truth.may_be_false() && candidate.head_match == HeadMatch::Yes {
                        if let Some(guard_domain) = candidate.guard_domain {
                            let selected = self
                                .retry_dispatch_after_cache_shed(|| {
                                    state.predicate_bdd.and(state.residual, guard_domain)
                                })
                                .map_err(|error| self.dispatch_issue(result_site, error))?;
                            state.false_backtrack_coverage = self
                                .retry_dispatch_after_cache_shed(|| {
                                    state
                                        .predicate_bdd
                                        .or(state.false_backtrack_coverage, selected)
                                })
                                .map_err(|error| self.dispatch_issue(result_site, error))?;
                        }
                    }
                    covers_guard_domain = truth == TruthDomain::TRUE;
                }
                AbstractValue::Unknown => {
                    return Err(self.issue(
                        result_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "clause result lost the precision needed to distinguish false backtracking",
                    ))
                }
                other => {
                    self.retain_value(&mut state.retained, &other, result_site)?;
                    state.results.push(other);
                    covers_guard_domain = true;
                }
            }
        } else {
            self.retain_value(&mut state.retained, &value, result_site)?;
            state.results.push(value);
        }

        if candidate.head_match == HeadMatch::Yes && covers_guard_domain {
            if let Some(guard_domain) = candidate.guard_domain {
                let capacity_site = candidate
                    .candidate
                    .condition_site
                    .as_ref()
                    .unwrap_or(&candidate.candidate.head_site);
                let guard_false = self
                    .retry_dispatch_after_cache_shed(|| state.predicate_bdd.negate(guard_domain))
                    .map_err(|error| self.dispatch_issue(capacity_site, error))?;
                state.residual = self
                    .retry_dispatch_after_cache_shed(|| {
                        state.predicate_bdd.and(state.residual, guard_false)
                    })
                    .map_err(|error| self.dispatch_issue(capacity_site, error))?;
            }
        }
        self.set_control(machine, EvalControl::RuleNext(state))?;
        Ok(())
    }

    fn finish_rule(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<RuleState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if state.residual != DispatchPredicateBdd::EMPTY {
            if state.boolean_miss_safe {
                let result = AbstractValue::Bool(TruthDomain::FALSE);
                self.retain_value(&mut state.retained, &result, &state.call_site)?;
                state.results.push(result);
            } else {
                let false_fallback = self
                    .retry_dispatch_after_cache_shed(|| {
                        state
                            .predicate_bdd
                            .and(state.residual, state.false_backtrack_coverage)
                    })
                    .map_err(|error| self.dispatch_issue(&state.call_site, error))?;
                if false_fallback != DispatchPredicateBdd::EMPTY {
                    let result = AbstractValue::Bool(TruthDomain::FALSE);
                    self.retain_value(&mut state.retained, &result, &state.call_site)?;
                    state.results.push(result);
                }
                let false_fallback_complement = self
                    .retry_dispatch_after_cache_shed(|| {
                        state.predicate_bdd.negate(state.false_backtrack_coverage)
                    })
                    .map_err(|error| self.dispatch_issue(&state.call_site, error))?;
                let uncovered = self
                    .retry_dispatch_after_cache_shed(|| {
                        state
                            .predicate_bdd
                            .and(state.residual, false_fallback_complement)
                    })
                    .map_err(|error| self.dispatch_issue(&state.call_site, error))?;
                if uncovered != DispatchPredicateBdd::EMPTY {
                    let (reason, detail) = if state.runtime_irrefutable {
                        (
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            format!(
                                "abstract dispatch for `{} / {}` failed to reach the producer-certified irrefutable candidate",
                                state.family.name, state.family.arity
                            ),
                        )
                    } else {
                        (
                            RelationalEndpointTotalityIssueReason::PartialRuleDispatch,
                            format!(
                                "abstract rule dispatch for `{} / {}` retains a reachable fallthrough path",
                                state.family.name, state.family.arity
                            ),
                        )
                    };
                    return Err(self.issue(&state.call_site, reason, detail));
                }
            }
        }
        let value = join_values(state.results).ok_or_else(|| {
            self.issue(
                &state.call_site,
                RelationalEndpointTotalityIssueReason::PartialRuleDispatch,
                "rule family has no abstractly reachable return value",
            )
        })?;
        self.require_bounded_value(&value, &state.call_site)?;
        self.require_value_type(
            &state.call_site,
            "joined rule-family result",
            &state.result_type,
            &value,
            &mut state.substitutions,
        )?;
        self.record(
            &state.call_site,
            ObligationKind::Dispatch,
            &state.call_input,
            &value,
        )?;
        machine.active.remove(&state.active);
        if let Some(cached) = self.cached_value(&value, &state.call_site) {
            self.rule_family_cache
                .borrow_mut()
                .insert((self.role, state.family, state.argument_root), cached);
        }
        self.deliver_value(machine, value)?;
        Ok(())
    }

    fn continue_match(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<MatchState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if state.remaining.is_unreachable() || state.next_arm >= state.arm_count {
            if !state.remaining.is_unreachable() {
                return Err(self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                    "match is not proved exhaustive over the endpoint domain",
                ));
            }
            let result = join_values(state.results).ok_or_else(|| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                    "match has no reachable result arm",
                )
            })?;
            self.require_bounded_value(&result, &state.site)?;
            self.record(
                &state.site,
                ObligationKind::Match,
                &state.scrutinee,
                &result,
            )?;
            self.deliver_value(machine, result)?;
            return Ok(());
        }

        let arm_index = state.next_arm;
        let (pattern, has_guard) = {
            let expression = self.index.expression(&state.site).ok_or_else(|| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "match expression disappeared from the checked semantic index",
                )
            })?;
            let ExprKind::Match(_, arms) = &expression.kind else {
                return Err(self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "match state no longer addresses a checked match expression",
                ));
            };
            let arm = arms.get(arm_index).ok_or_else(|| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "match state arm count disagrees with the checked expression",
                )
            })?;
            (
                self.clone_pattern_bounded(&state.site, &arm.pat)?,
                arm.guard.is_some(),
            )
        };
        state.next_arm += 1;
        let guard_site = has_guard.then(|| {
            let guard_site = child_site(&state.site, state.next_child);
            state.next_child += 1;
            guard_site
        });
        let body_site = child_site(&state.site, state.next_child);
        state.next_child += 1;
        let pattern_index = u32::try_from(arm_index).map_err(|_| {
            self.issue(
                &state.site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "match arm index exceeds the checked structural address space",
            )
        })?;
        let mut arm_env = state.env.bindings.clone();
        let partition = self.bind_pattern_with_options(
            &state.site,
            &pattern,
            &state.remaining,
            &[pattern_index],
            &mut arm_env,
            state.allow_bare_fielded_tag,
        )?;
        if !partition.may_match() {
            self.replace_retained_value(
                &mut state.retained,
                &state.remaining,
                &partition.unmatched,
                &state.site,
            )?;
            state.remaining = partition.unmatched;
            self.set_control(machine, EvalControl::MatchNext(state))?;
            return Ok(());
        }
        let mut arm_retained = self.new_retained_budget(&state.site)?;
        self.retain_value(&mut arm_retained, &partition.matched, &state.site)?;
        self.retain_value(&mut arm_retained, &partition.unmatched, &state.site)?;
        let arm_state = Box::new(MatchArmState {
            guard_site: guard_site.clone(),
            body_site,
            env: self.shared_env(arm_env, &state.site)?,
            partition,
            _retained: arm_retained,
        });
        if let Some(guard_site) = guard_site {
            let env = Arc::clone(&arm_state.env);
            self.push_continuation(
                machine,
                &guard_site,
                EvalContinuation::MatchGuard {
                    state,
                    arm: arm_state,
                },
            )?;
            self.schedule_site(machine, guard_site, env)?;
        } else {
            self.continue_match_after_guard(machine, state, arm_state, TruthDomain::TRUE)?;
        }
        Ok(())
    }

    fn continue_match_after_guard(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<MatchState>,
        arm: Box<MatchArmState>,
        truth: TruthDomain,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let PatternPartition {
            matched, unmatched, ..
        } = arm.partition;
        let remaining = if truth == TruthDomain::TRUE {
            unmatched
        } else if truth == TruthDomain::FALSE {
            join_value(matched, unmatched)
        } else {
            join_value(unmatched, matched)
        };
        self.replace_retained_value(
            &mut state.retained,
            &state.remaining,
            &remaining,
            &state.site,
        )?;
        state.remaining = remaining;
        if truth.may_be_true() {
            let body_env = if let Some(guard_site) = arm.guard_site.as_ref() {
                self.shared_env(
                    self.refined_env_for_condition(guard_site, true, &arm.env),
                    guard_site,
                )?
            } else {
                arm.env
            };
            if !env_is_unreachable(&body_env) {
                let body_site = arm.body_site;
                self.push_continuation(machine, &body_site, EvalContinuation::MatchBody(state))?;
                self.schedule_site(machine, body_site, body_env)?;
                return Ok(());
            }
        }
        self.set_control(machine, EvalControl::MatchNext(state))?;
        Ok(())
    }

    fn continue_block(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<BlockState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        if state.next_statement >= state.statement_count {
            self.deliver_value(machine, state.result)?;
            return Ok(());
        }
        let statement_index = state.next_statement;
        let statement_site = child_site(&state.site, statement_index);
        let statement = {
            let expression = self.index.expression(&state.site).ok_or_else(|| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "block expression disappeared from the checked semantic index",
                )
            })?;
            let ExprKind::Block(statements) = &expression.kind else {
                return Err(self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "block state no longer addresses a checked block expression",
                ));
            };
            let statement = statements.get(statement_index).ok_or_else(|| {
                self.issue(
                    &statement_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "block state statement count disagrees with the checked expression",
                )
            })?;
            match statement {
                Stmt::Bind(pattern, _, _) => ShallowBlockStatement::Bind(
                    self.clone_pattern_bounded(&statement_site, pattern)?,
                ),
                Stmt::Expr(_) => ShallowBlockStatement::Expression,
                Stmt::MonadicBind(_, _, _)
                | Stmt::For(_, _, _)
                | Stmt::While(_, _)
                | Stmt::Send(_, _)
                | Stmt::StreamBind(_, _)
                | Stmt::StreamSub(_, _)
                | Stmt::Invariant { .. }
                | Stmt::Prove { .. }
                | Stmt::Assert(_, _)
                | Stmt::Retract(_, _)
                | Stmt::Abort => ShallowBlockStatement::EffectfulOrControl,
                Stmt::Defn(_)
                | Stmt::TypeDecl(_)
                | Stmt::Rule(_)
                | Stmt::Use(_)
                | Stmt::Import(_)
                | Stmt::QualifiedImport(_, _)
                | Stmt::HashImport(_, _)
                | Stmt::Depend(_, _)
                | Stmt::RustBlock(_)
                | Stmt::Annot(_, _)
                | Stmt::Explore(_) => ShallowBlockStatement::Declaration,
            }
        };
        state.next_statement += 1;
        match statement {
            ShallowBlockStatement::Bind(pattern) => {
                let initializer_site = child_site(&statement_site, 0);
                let env = Arc::clone(&state.env);
                self.push_continuation(
                    machine,
                    &statement_site,
                    EvalContinuation::BlockBind {
                        state,
                        statement_site: statement_site.clone(),
                        pattern,
                    },
                )?;
                self.schedule_site(machine, initializer_site, env)?;
            }
            ShallowBlockStatement::Expression => {
                let expression_site = child_site(&statement_site, 0);
                let env = Arc::clone(&state.env);
                self.push_continuation(
                    machine,
                    &statement_site,
                    EvalContinuation::BlockExpression(state),
                )?;
                self.schedule_site(machine, expression_site, env)?;
            }
            ShallowBlockStatement::EffectfulOrControl => {
                return Err(self.issue(
                    &statement_site,
                    RelationalEndpointTotalityIssueReason::EffectfulCall,
                    "effectful or control statement occurs inside an endpoint helper",
                ))
            }
            ShallowBlockStatement::Declaration => {
                return Err(self.issue(
                    &statement_site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    "declaration statement occurs inside an endpoint expression block",
                ))
            }
        }
        Ok(())
    }

    fn begin_builtin(
        &mut self,
        machine: &mut EndpointEvalMachine,
        name: &str,
        mut arguments: Vec<AbstractValue>,
        argument_sites: Vec<ExprSiteId>,
        site: ExprSiteId,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let callback_index = match (name, arguments.len()) {
            ("foldl", 3) => Some(2),
            ("map" | "filter" | "sort_by" | "all" | "any" | "find" | "flat_map", 2) => Some(1),
            _ => None,
        };
        if callback_index.is_none() {
            let result = self.eval_builtin_leaf(name, arguments, &site)?;
            self.deliver_value(machine, result)?;
            return Ok(());
        }
        let mut retained = self.new_tuple_clone_budget(arguments.iter(), &site)?;
        let input = AbstractValue::Tuple(arguments.clone().into_boxed_slice());
        let callback_site = callback_index
            .map(|index| {
                argument_sites.get(index).cloned().ok_or_else(|| {
                    self.issue(
                        &site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "higher-order builtin has no canonical callback argument site",
                    )
                })
            })
            .transpose()?;
        let retained_argument_bounds = callback_index
            .is_some()
            .then(|| {
                arguments
                    .iter()
                    .map(AbstractValue::shape_bounds)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut state =
            match (name, arguments.len()) {
                ("map", 2) => {
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "map requires a proved finite List",
                            )),
                        };
                    let kind = match sequence {
                        AbstractSequence::Exact(values) => BuiltinStateKind::MapExact {
                            output: Vec::with_capacity(values.len()),
                            values,
                            next: 0,
                        },
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Exact(Box::new([]))),
                            )
                        }
                        AbstractSequence::Summary {
                            element,
                            minimum_length,
                            maximum_length,
                        } => BuiltinStateKind::MapSummary {
                            element: *element,
                            minimum_length,
                            maximum_length,
                        },
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("map callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind,
                    }
                }
                ("filter", 2) => {
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "filter requires a proved finite List",
                            )),
                        };
                    let kind = match sequence {
                        AbstractSequence::Exact(values) => BuiltinStateKind::FilterExact {
                            values,
                            next: 0,
                            retained: Vec::new(),
                            possible: Vec::new(),
                            exact: true,
                        },
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Exact(Box::new([]))),
                            )
                        }
                        AbstractSequence::Summary {
                            element,
                            maximum_length,
                            ..
                        } => BuiltinStateKind::FilterSummary {
                            element: *element,
                            maximum_length,
                        },
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("filter callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind,
                    }
                }
                ("sort_by", 2) => {
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "sort_by requires a proved finite List",
                            )),
                        };
                    let kind = match sequence {
                        AbstractSequence::Exact(values) if values.len() <= 1 => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Exact(values)),
                            )
                        }
                        AbstractSequence::Exact(values) => BuiltinStateKind::SortByExact {
                            keys: Vec::with_capacity(values.len()),
                            values,
                            next: 0,
                        },
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Exact(Box::new([]))),
                            )
                        }
                        AbstractSequence::Summary {
                            element,
                            minimum_length,
                            maximum_length,
                        } if maximum_length <= 1 => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Summary {
                                    element,
                                    minimum_length,
                                    maximum_length,
                                }),
                            )
                        }
                        AbstractSequence::Summary {
                            element,
                            minimum_length,
                            maximum_length,
                        } => BuiltinStateKind::SortBySummary {
                            element: *element,
                            minimum_length,
                            maximum_length,
                        },
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("sort_by callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind,
                    }
                }
                ("foldl", 3) => {
                    let callable = arguments.pop().expect("checked arity");
                    let accumulator = arguments.pop().expect("checked arity");
                    let accumulator_bounds = accumulator.shape_bounds();
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "foldl requires a proved finite List",
                            )),
                        };
                    let values = match sequence {
                        AbstractSequence::Exact(values) => values,
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                accumulator,
                            )
                        }
                        _ => return Err(self.issue(
                            &site,
                            RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                            "foldl requires an exact List unless its proved maximum length is zero",
                        )),
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("foldl callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind: BuiltinStateKind::FoldLeft {
                            values,
                            next: 0,
                            accumulator: Some(accumulator),
                            accumulator_bounds,
                        },
                    }
                }
                ("all", 2) | ("any", 2) => {
                    let is_all = name == "all";
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                format!("{name} requires a proved finite List"),
                            )),
                        };
                    let values = match sequence {
                        AbstractSequence::Exact(values) => values,
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::Bool(if is_all {
                                    TruthDomain::TRUE
                                } else {
                                    TruthDomain::FALSE
                                }),
                            )
                        }
                        _ => {
                            return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                                format!("{name} requires an exact List unless it is proved empty"),
                            ))
                        }
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("all/any callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind: BuiltinStateKind::AllAny {
                            values,
                            next: 0,
                            truth: if is_all {
                                TruthDomain::TRUE
                            } else {
                                TruthDomain::FALSE
                            },
                            is_all,
                        },
                    }
                }
                ("find", 2) => {
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "find requires a proved finite List",
                            )),
                        };
                    let kind = match sequence {
                        AbstractSequence::Exact(values) if values.is_empty() => {
                            let result = self.abstract_option_value(&site, None)?;
                            return self.finish_collection_builtin(machine, &site, input, result);
                        }
                        AbstractSequence::Exact(values) => BuiltinStateKind::FindExact {
                            values,
                            next: 0,
                            possible_matches: Vec::new(),
                        },
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            let result = self.abstract_option_value(&site, None)?;
                            return self.finish_collection_builtin(machine, &site, input, result);
                        }
                        AbstractSequence::Summary {
                            element,
                            minimum_length,
                            ..
                        } => BuiltinStateKind::FindSummary {
                            element: *element,
                            minimum_length,
                        },
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("find callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind,
                    }
                }
                ("flat_map", 2) => {
                    let callable = arguments.pop().expect("checked arity");
                    let sequence =
                        match arguments.pop().expect("checked arity") {
                            AbstractValue::List(sequence) => sequence,
                            _ => return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "flat_map requires a proved finite List",
                            )),
                        };
                    let values = match sequence {
                        AbstractSequence::Exact(values) => values,
                        AbstractSequence::Summary {
                            maximum_length: 0, ..
                        } => {
                            return self.finish_collection_builtin(
                                machine,
                                &site,
                                input,
                                AbstractValue::List(AbstractSequence::Exact(Box::new([]))),
                            )
                        }
                        _ => {
                            return Err(self.issue(
                                &site,
                                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                                "flat_map requires an exact List unless it is proved empty",
                            ))
                        }
                    };
                    BuiltinState {
                        site,
                        callback_site: callback_site
                            .clone()
                            .expect("flat_map callback site is indexed above"),
                        input,
                        callable,
                        retained,
                        kind: BuiltinStateKind::FlatMap {
                            values,
                            next: 0,
                            output: Vec::new(),
                        },
                    }
                }
                _ => {
                    // Non-higher-order builtins returned before the retained
                    // callback state and canonical input were constructed.
                    let result = self.eval_builtin_leaf(name, arguments, &site)?;
                    self.deliver_value(machine, result)?;
                    return Ok(());
                }
            };
        // Higher-order state retains both the canonical call input used by
        // the proof obligation and the original collection/callable values it
        // iterates. Account for both owned representations. Empty/terminal
        // paths return above and therefore never reserve state they do not
        // retain.
        for bounds in retained_argument_bounds {
            self.retain_shape_bounds(&mut state.retained, bounds, &state.site)?;
        }
        self.set_control(machine, EvalControl::BuiltinNext(Box::new(state)))?;
        Ok(())
    }

    fn continue_builtin(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<BuiltinState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let arguments = match &mut state.kind {
            BuiltinStateKind::MapExact {
                values,
                next,
                output,
            } => {
                let Some(value) = values.get(*next).cloned() else {
                    let result = AbstractValue::List(AbstractSequence::Exact(
                        std::mem::take(output).into_boxed_slice(),
                    ));
                    return self.finish_collection_builtin(
                        machine,
                        &state.site,
                        state.input,
                        result,
                    );
                };
                *next += 1;
                vec![value]
            }
            BuiltinStateKind::MapSummary { element, .. } => vec![element.clone()],
            BuiltinStateKind::FilterExact { values, next, .. } => {
                let Some(value) = values.get(*next).cloned() else {
                    return self.finish_filter_exact(machine, state);
                };
                *next += 1;
                vec![value]
            }
            BuiltinStateKind::FilterSummary { element, .. } => vec![element.clone()],
            BuiltinStateKind::SortByExact { values, next, keys } => {
                let Some(value) = values.get(*next).cloned() else {
                    return self.finish_sort_by_exact(machine, state);
                };
                debug_assert_eq!(*next, keys.len());
                *next += 1;
                vec![value]
            }
            BuiltinStateKind::SortBySummary { element, .. } => vec![element.clone()],
            BuiltinStateKind::FoldLeft {
                values,
                next,
                accumulator,
                ..
            } => {
                let Some(value) = values.get(*next).cloned() else {
                    let result = accumulator.take().expect("fold accumulator is present");
                    return self.finish_collection_builtin(
                        machine,
                        &state.site,
                        state.input,
                        result,
                    );
                };
                *next += 1;
                vec![
                    accumulator.take().expect("fold accumulator is present"),
                    value,
                ]
            }
            BuiltinStateKind::AllAny {
                values,
                next,
                truth,
                is_all,
            } => {
                let done = (*is_all && *truth == TruthDomain::FALSE)
                    || (!*is_all && *truth == TruthDomain::TRUE);
                let Some(value) = (!done).then(|| values.get(*next).cloned()).flatten() else {
                    let result = AbstractValue::Bool(*truth);
                    return self.finish_collection_builtin(
                        machine,
                        &state.site,
                        state.input,
                        result,
                    );
                };
                *next += 1;
                vec![value]
            }
            BuiltinStateKind::FindExact {
                values,
                next,
                possible_matches,
            } => {
                let Some(value) = values.get(*next).cloned() else {
                    let result = self.finish_find_value(
                        &state.site,
                        std::mem::take(possible_matches),
                        true,
                    )?;
                    return self.finish_collection_builtin(
                        machine,
                        &state.site,
                        state.input,
                        result,
                    );
                };
                *next += 1;
                vec![value]
            }
            BuiltinStateKind::FindSummary { element, .. } => vec![element.clone()],
            BuiltinStateKind::FlatMap {
                values,
                next,
                output,
            } => {
                let Some(value) = values.get(*next).cloned() else {
                    let result = AbstractValue::List(AbstractSequence::Exact(
                        std::mem::take(output).into_boxed_slice(),
                    ));
                    return self.finish_collection_builtin(
                        machine,
                        &state.site,
                        state.input,
                        result,
                    );
                };
                *next += 1;
                vec![value]
            }
        };
        let callback_site = state.callback_site.clone();
        let callable = state.callable.clone();
        self.push_continuation(
            machine,
            &callback_site,
            EvalContinuation::BuiltinCallback(state),
        )?;
        self.set_control(
            machine,
            EvalControl::Apply {
                callable,
                arguments,
                call_site: callback_site,
            },
        )?;
        Ok(())
    }

    fn resume_builtin(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<BuiltinState>,
        value: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let retention_site = state.site.clone();
        match &mut state.kind {
            BuiltinStateKind::MapExact { output, .. } => {
                self.retain_value(&mut state.retained, &value, &retention_site)?;
                output.push(value);
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
            BuiltinStateKind::MapSummary {
                minimum_length,
                maximum_length,
                ..
            } => {
                let result = AbstractValue::List(AbstractSequence::Summary {
                    element: Box::new(value),
                    minimum_length: *minimum_length,
                    maximum_length: *maximum_length,
                });
                self.finish_collection_builtin(machine, &state.site, state.input, result)?;
            }
            BuiltinStateKind::FilterExact {
                values,
                next,
                retained,
                possible,
                exact,
            } => {
                let predicate = value.truth().ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "filter callback is not a proved Bool",
                    )
                })?;
                let item = values
                    .get(next.saturating_sub(1))
                    .cloned()
                    .expect("filter callback has one current item");
                if predicate == TruthDomain::TRUE {
                    self.retain_value(&mut state.retained, &item, &retention_site)?;
                    retained.push(item.clone());
                } else if predicate == TruthDomain::BOTH {
                    *exact = false;
                }
                if predicate.may_be_true() {
                    self.retain_value(&mut state.retained, &item, &retention_site)?;
                    possible.push(item);
                }
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
            BuiltinStateKind::FilterSummary {
                element,
                maximum_length,
            } => {
                let predicate = value.truth().ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "filter callback is not a proved Bool",
                    )
                })?;
                let result = if predicate == TruthDomain::FALSE {
                    AbstractValue::List(AbstractSequence::Exact(Box::new([])))
                } else {
                    AbstractValue::List(AbstractSequence::Summary {
                        element: Box::new(element.clone()),
                        minimum_length: 0,
                        maximum_length: *maximum_length,
                    })
                };
                self.finish_collection_builtin(machine, &state.site, state.input, result)?;
            }
            BuiltinStateKind::SortByExact { keys, .. } => {
                self.retain_value(&mut state.retained, &value, &retention_site)?;
                keys.push(value);
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
            BuiltinStateKind::SortBySummary {
                element,
                minimum_length,
                maximum_length,
            } => {
                if total_sort_key_family(&value).is_none() {
                    return Err(self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                        "sort_by callback result does not have a proved total runtime ordering",
                    ));
                }
                let result = AbstractValue::List(AbstractSequence::Summary {
                    element: Box::new(element.clone()),
                    minimum_length: *minimum_length,
                    maximum_length: *maximum_length,
                });
                self.finish_collection_builtin(machine, &state.site, state.input, result)?;
            }
            BuiltinStateKind::FoldLeft {
                accumulator,
                accumulator_bounds,
                ..
            } => {
                let next_bounds = value.shape_bounds();
                self.replace_retained_shape(
                    &mut state.retained,
                    *accumulator_bounds,
                    next_bounds,
                    &retention_site,
                )?;
                *accumulator = Some(value);
                *accumulator_bounds = next_bounds;
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
            BuiltinStateKind::AllAny { truth, is_all, .. } => {
                let item = value.truth().ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        format!(
                            "{} callback is not a proved Bool",
                            if *is_all { "all" } else { "any" }
                        ),
                    )
                })?;
                *truth = if *is_all {
                    abstract_and(*truth, item)
                } else {
                    abstract_or(*truth, item)
                };
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
            BuiltinStateKind::FindExact {
                values,
                next,
                possible_matches,
            } => {
                let predicate = value.truth().ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "find callback is not a proved Bool",
                    )
                })?;
                let item = values
                    .get(next.saturating_sub(1))
                    .cloned()
                    .expect("find callback has one current item");
                if predicate.may_be_true() {
                    self.retain_value(&mut state.retained, &item, &retention_site)?;
                    possible_matches.push(item);
                }
                if predicate == TruthDomain::TRUE {
                    let result = self.finish_find_value(
                        &state.site,
                        std::mem::take(possible_matches),
                        false,
                    )?;
                    self.finish_collection_builtin(machine, &state.site, state.input, result)?;
                } else {
                    self.set_control(machine, EvalControl::BuiltinNext(state))?;
                }
            }
            BuiltinStateKind::FindSummary {
                element,
                minimum_length,
            } => {
                let predicate = value.truth().ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "find callback is not a proved Bool",
                    )
                })?;
                let possible_matches = predicate
                    .may_be_true()
                    .then(|| vec![element.clone()])
                    .unwrap_or_default();
                let none_possible = *minimum_length == 0 || predicate.may_be_false();
                let result =
                    self.finish_find_value(&state.site, possible_matches, none_possible)?;
                self.finish_collection_builtin(machine, &state.site, state.input, result)?;
            }
            BuiltinStateKind::FlatMap { output, .. } => {
                self.retain_value(&mut state.retained, &value, &retention_site)?;
                let AbstractValue::List(AbstractSequence::Exact(mapped)) = value else {
                    return Err(self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                        "flat_map callback must return an exact finite List",
                    ));
                };
                let new_length = output.len().checked_add(mapped.len()).ok_or_else(|| {
                    self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "flat_map result length overflowed",
                    )
                })?;
                if new_length > MAX_EXACT_COLLECTION_ITEMS {
                    return Err(self.issue(
                        &state.site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        "flat_map result exceeds the exact collection proof limit",
                    ));
                }
                output.extend(mapped.into_vec());
                self.set_control(machine, EvalControl::BuiltinNext(state))?;
            }
        }
        Ok(())
    }

    fn abstract_option_value(
        &self,
        site: &ExprSiteId,
        value: Option<AbstractValue>,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        let variant = if value.is_some() { "Some" } else { "None" };
        let identity = self
            .resolutions
            .constructor_identities
            .get(&("Option".into(), variant.into()))
            .ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    format!("find result has no checked Option::{variant} identity"),
                )
            })?
            .as_ref()
            .clone();
        let fields = value.into_iter().collect::<Vec<_>>().into_boxed_slice();
        if identity.fields.len() != fields.len() {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "find result disagrees with the checked Option constructor layout",
            ));
        }
        Ok(single_constructor(identity, fields))
    }

    fn finish_find_value(
        &self,
        site: &ExprSiteId,
        possible_matches: Vec<AbstractValue>,
        none_possible: bool,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        let mut outcomes = possible_matches
            .into_iter()
            .map(|value| self.abstract_option_value(site, Some(value)))
            .collect::<Result<Vec<_>, _>>()?;
        if none_possible {
            outcomes.push(self.abstract_option_value(site, None)?);
        }
        join_values(outcomes).ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "find has no abstractly reachable Option result",
            )
        })
    }

    fn finish_sort_by_exact(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<BuiltinState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let BuiltinStateKind::SortByExact { values, keys, .. } = &mut state.kind else {
            unreachable!("sort_by finalizer receives sort_by state")
        };
        debug_assert_eq!(values.len(), keys.len());

        let total_family = keys.first().and_then(total_sort_key_family);
        if total_family.is_none()
            || !keys
                .iter()
                .all(|key| total_sort_key_family(key) == total_family)
        {
            return Err(self.issue(
                &state.site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                "sort_by callback results do not share one proved total runtime ordering",
            ));
        }

        let exact_family = keys.first().and_then(exact_sort_key_family);
        let result = if exact_family.is_some()
            && keys
                .iter()
                .all(|key| exact_sort_key_family(key) == exact_family)
        {
            let mut keyed = std::mem::take(values)
                .into_vec()
                .into_iter()
                .zip(std::mem::take(keys))
                .collect::<Vec<_>>();
            keyed.sort_by(|(_, left), (_, right)| {
                compare_exact_sort_keys(left, right)
                    .expect("keys in one exact family have a runtime ordering")
            });
            AbstractValue::List(AbstractSequence::Exact(
                keyed
                    .into_iter()
                    .map(|(value, _)| value)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ))
        } else {
            let values = std::mem::take(values).into_vec();
            let length = u64::try_from(values.len()).map_err(|_| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "sort_by result length exceeds the proof format",
                )
            })?;
            let element = join_values(values).unwrap_or(AbstractValue::Unknown);
            // The ordering is unknown, but the length is not. A summary keeps
            // exactly that information without cloning a potentially large
            // joined element once per input item.
            AbstractValue::List(AbstractSequence::Summary {
                element: Box::new(element),
                minimum_length: length,
                maximum_length: length,
            })
        };
        self.finish_collection_builtin(machine, &state.site, state.input, result)
    }

    fn finish_filter_exact(
        &mut self,
        machine: &mut EndpointEvalMachine,
        mut state: Box<BuiltinState>,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        let BuiltinStateKind::FilterExact {
            retained,
            possible,
            exact,
            ..
        } = &mut state.kind
        else {
            unreachable!("filter finalizer receives filter state")
        };
        let result = if *exact {
            AbstractValue::List(AbstractSequence::Exact(
                std::mem::take(retained).into_boxed_slice(),
            ))
        } else if possible.is_empty() {
            AbstractValue::List(AbstractSequence::Exact(Box::new([])))
        } else {
            let minimum_length = u64::try_from(retained.len()).map_err(|_| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "filter minimum length exceeds the proof format",
                )
            })?;
            let maximum_length = u64::try_from(possible.len()).map_err(|_| {
                self.issue(
                    &state.site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "filter maximum length exceeds the proof format",
                )
            })?;
            AbstractValue::List(AbstractSequence::Summary {
                element: Box::new(
                    join_values(std::mem::take(possible)).unwrap_or(AbstractValue::Unknown),
                ),
                minimum_length,
                maximum_length,
            })
        };
        self.finish_collection_builtin(machine, &state.site, state.input, result)
    }

    fn finish_collection_builtin(
        &mut self,
        machine: &mut EndpointEvalMachine,
        site: &ExprSiteId,
        input: AbstractValue,
        result: AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.require_bounded_value(&result, site)?;
        self.record(site, ObligationKind::Collection, &input, &result)?;
        self.deliver_value(machine, result)?;
        Ok(())
    }

    fn dispatch_argument_origins(
        &self,
        head_site: &ExprSiteId,
        argument_count: usize,
    ) -> DispatchArgumentOrigins {
        let mut origins = DispatchArgumentOrigins::new();
        let Some(Expr {
            kind: ExprKind::App(_, arguments),
            ..
        }) = self.index.expression(head_site)
        else {
            return origins;
        };
        if arguments.len() != argument_count {
            return origins;
        }
        let source_order = self
            .resolutions
            .expressions
            .get(head_site)
            .and_then(|resolution| {
                canonical_source_indices(argument_count, resolution.named_arguments.as_ref())
            });
        let Some(source_order) = source_order else {
            return origins;
        };
        for (canonical_index, source_index) in source_order.into_iter().enumerate() {
            let Ok(canonical_index) = u32::try_from(canonical_index) else {
                return DispatchArgumentOrigins::new();
            };
            if !self.record_dispatch_argument_origin(
                &child_site(head_site, source_index + 1),
                dispatch_scalar_argument_id(canonical_index),
                &mut origins,
                0,
            ) {
                return DispatchArgumentOrigins::new();
            }
        }
        origins
    }

    fn record_dispatch_argument_origin(
        &self,
        site: &ExprSiteId,
        origin: DispatchScalarTermId,
        origins: &mut DispatchArgumentOrigins,
        depth: usize,
    ) -> bool {
        if depth >= MAX_CALL_DEPTH {
            return false;
        }
        let Some(expression) = self.index.expression(site) else {
            return false;
        };
        let resolution = self.resolutions.expressions.get(site);
        match &expression.kind {
            ExprKind::Var(name) if name == "_" => true,
            ExprKind::Var(_) => {
                let Some(CheckedValueBinding::Binder {
                    kind: CheckedBinderKind::RuleHead,
                    site: binder,
                }) = resolution.and_then(|resolution| resolution.value_binding.as_ref())
                else {
                    return false;
                };
                let binder = checked_explore_projection_binder_digest(binder);
                match origins.get(&binder) {
                    Some(existing) => existing == &origin,
                    None => {
                        origins.insert(binder, origin);
                        true
                    }
                }
            }
            ExprKind::App(_, _) => {
                match resolution.and_then(|resolution| resolution.call_target.as_ref()) {
                    Some(CheckedCallTarget::Builtin { canonical_name, .. })
                        if canonical_name.as_ref() == "__typed" =>
                    {
                        self.record_dispatch_argument_origin(
                            &child_site(site, 1),
                            origin,
                            origins,
                            depth + 1,
                        )
                    }
                    Some(CheckedCallTarget::Builtin { canonical_name, .. })
                        if canonical_name.as_ref() == "__named_arg" =>
                    {
                        self.record_dispatch_argument_origin(
                            &child_site(site, 2),
                            origin,
                            origins,
                            depth + 1,
                        )
                    }
                    _ => false,
                }
            }
            // Literal heads introduce no binder. Their exact applicability is
            // still governed by `HeadMatch`; no symbolic origin is needed.
            ExprKind::Lit(_) => true,
            _ => false,
        }
    }

    fn canonical_dispatch_condition(
        &self,
        site: &ExprSiteId,
        origins: &DispatchArgumentOrigins,
        bdd: &mut DispatchPredicateBdd,
    ) -> Result<Option<DispatchBddId>, RelationalEndpointTotalityIssue> {
        self.canonical_dispatch_condition_inner(site, origins, bdd, 0)
    }

    fn canonical_dispatch_condition_inner(
        &self,
        site: &ExprSiteId,
        origins: &DispatchArgumentOrigins,
        bdd: &mut DispatchPredicateBdd,
        depth: usize,
    ) -> Result<Option<DispatchBddId>, RelationalEndpointTotalityIssue> {
        if depth >= MAX_CALL_DEPTH {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "dispatch predicate exceeds the endpoint proof depth limit",
            ));
        }
        let Some(expression) = self.index.expression(site) else {
            return Ok(None);
        };
        let capacity = |error| self.dispatch_issue(site, error);
        match &expression.kind {
            ExprKind::Lit(Literal::Bool(value)) => Ok(Some(if *value {
                DispatchPredicateBdd::ALL
            } else {
                DispatchPredicateBdd::EMPTY
            })),
            ExprKind::UnOp(operator, _) if operator == "!" => {
                let Some(inner) = self.canonical_dispatch_condition_inner(
                    &child_site(site, 0),
                    origins,
                    bdd,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                Ok(Some(
                    self.retry_dispatch_after_cache_shed(|| bdd.negate(inner))
                        .map_err(capacity)?,
                ))
            }
            ExprKind::BinOp(operator, _, _) if operator == "&&" || operator == "||" => {
                let Some(left) = self.canonical_dispatch_condition_inner(
                    &child_site(site, 0),
                    origins,
                    bdd,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                let Some(right) = self.canonical_dispatch_condition_inner(
                    &child_site(site, 1),
                    origins,
                    bdd,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                let result = self
                    .retry_dispatch_after_cache_shed(|| {
                        if operator == "&&" {
                            bdd.and(left, right)
                        } else {
                            bdd.or(left, right)
                        }
                    })
                    .map_err(capacity)?;
                Ok(Some(result))
            }
            ExprKind::Conjunction(parts) | ExprKind::Disjunction(parts) => {
                let conjunction = matches!(&expression.kind, ExprKind::Conjunction(_));
                let mut result = if conjunction {
                    DispatchPredicateBdd::ALL
                } else {
                    DispatchPredicateBdd::EMPTY
                };
                for index in 0..parts.len() {
                    let Some(part) = self.canonical_dispatch_condition_inner(
                        &child_site(site, index),
                        origins,
                        bdd,
                        depth + 1,
                    )?
                    else {
                        return Ok(None);
                    };
                    result = self
                        .retry_dispatch_after_cache_shed(|| {
                            if conjunction {
                                bdd.and(result, part)
                            } else {
                                bdd.or(result, part)
                            }
                        })
                        .map_err(capacity)?;
                }
                Ok(Some(result))
            }
            ExprKind::BinOp(operator, _, _)
                if matches!(operator.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=") =>
            {
                let left_site = child_site(site, 0);
                let right_site = child_site(site, 1);
                let operands_are_int =
                    self.checked_site_is_int(&left_site) && self.checked_site_is_int(&right_site);
                if matches!(operator.as_str(), "<" | "<=" | ">" | ">=") && !operands_are_int {
                    // Ordered Float comparisons are not complementary in the
                    // presence of NaN (`x > y` is not `!(x <= y)`). Keep
                    // non-Int predicates outside the Boolean algebra unless a
                    // future producer certificate proves a total order.
                    return Ok(None);
                }
                let mut canonicalization = DispatchCanonicalizationBudget::default();
                let Some(mut left) = self
                    .canonical_dispatch_scalar_term(
                        &left_site,
                        origins,
                        &mut canonicalization,
                        depth + 1,
                    )
                    .map_err(capacity)?
                else {
                    return Ok(None);
                };
                let Some(mut right) = self
                    .canonical_dispatch_scalar_term(
                        &right_site,
                        origins,
                        &mut canonicalization,
                        depth + 1,
                    )
                    .map_err(capacity)?
                else {
                    return Ok(None);
                };
                let (mut comparison, mut negated) = match operator.as_str() {
                    "==" => (DispatchComparisonOperator::Equal, false),
                    "!=" => (DispatchComparisonOperator::Equal, true),
                    "<" => (DispatchComparisonOperator::Less, false),
                    "<=" => (DispatchComparisonOperator::LessOrEqual, false),
                    ">" => (DispatchComparisonOperator::LessOrEqual, true),
                    ">=" => (DispatchComparisonOperator::Less, true),
                    _ => unreachable!("guarded comparison operator"),
                };
                if right < left {
                    std::mem::swap(&mut left, &mut right);
                    match comparison {
                        DispatchComparisonOperator::Equal => {}
                        DispatchComparisonOperator::Less => {
                            comparison = DispatchComparisonOperator::LessOrEqual;
                            negated = !negated;
                        }
                        DispatchComparisonOperator::LessOrEqual => {
                            comparison = DispatchComparisonOperator::Less;
                            negated = !negated;
                        }
                    }
                }
                if left == right && operands_are_int {
                    let true_before_negation = match comparison {
                        DispatchComparisonOperator::Equal
                        | DispatchComparisonOperator::LessOrEqual => true,
                        DispatchComparisonOperator::Less => false,
                    };
                    return Ok(Some(if true_before_negation ^ negated {
                        DispatchPredicateBdd::ALL
                    } else {
                        DispatchPredicateBdd::EMPTY
                    }));
                }
                let atom = self
                    .retry_dispatch_after_cache_shed(|| {
                        bdd.atom(DispatchPredicateAtom {
                            operator: comparison,
                            left,
                            right,
                        })
                    })
                    .map_err(capacity)?;
                Ok(Some(if negated {
                    self.retry_dispatch_after_cache_shed(|| bdd.negate(atom))
                        .map_err(capacity)?
                } else {
                    atom
                }))
            }
            _ => Ok(None),
        }
    }

    fn checked_site_is_int(&self, site: &ExprSiteId) -> bool {
        matches!(
            self.resolutions
                .expressions
                .get(site)
                .map(|resolution| &resolution.resolved_type),
            Some(CheckedExpressionType::Resolved(Ty::Name(name))) if name == "Int"
        )
    }

    fn canonical_dispatch_scalar_term(
        &self,
        site: &ExprSiteId,
        origins: &DispatchArgumentOrigins,
        budget: &mut DispatchCanonicalizationBudget,
        depth: usize,
    ) -> Result<Option<DispatchScalarTermId>, DispatchBddError> {
        if depth >= MAX_CALL_DEPTH {
            return Err(DispatchBddError::ScalarTermLimit);
        }
        let Some(expression) = self.index.expression(site) else {
            return Ok(None);
        };
        match &expression.kind {
            ExprKind::Var(_) => {
                let Some(resolution) = self.resolutions.expressions.get(site) else {
                    return Ok(None);
                };
                let Some(CheckedValueBinding::Binder { site: binder, .. }) =
                    resolution.value_binding.as_ref()
                else {
                    return Ok(None);
                };
                let origin = origins
                    .get(&checked_explore_projection_binder_digest(binder))
                    .copied();
                if origin.is_some() {
                    budget.charge_scalar_node()?;
                }
                Ok(origin)
            }
            ExprKind::Lit(Literal::Int(value)) => {
                budget.charge_scalar_node()?;
                Ok(Some(dispatch_scalar_integer_id(*value)))
            }
            ExprKind::UnOp(operator, _) if operator == "-" => {
                let Some(inner) = self.canonical_dispatch_scalar_term(
                    &child_site(site, 0),
                    origins,
                    budget,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                budget.charge_scalar_node()?;
                Ok(Some(dispatch_scalar_negation_id(inner)))
            }
            ExprKind::BinOp(operator, _, _)
                if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%") =>
            {
                let Some(mut left) = self.canonical_dispatch_scalar_term(
                    &child_site(site, 0),
                    origins,
                    budget,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                let Some(mut right) = self.canonical_dispatch_scalar_term(
                    &child_site(site, 1),
                    origins,
                    budget,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                let operator = match operator.as_str() {
                    "+" => DispatchArithmeticOperator::Add,
                    "-" => DispatchArithmeticOperator::Subtract,
                    "*" => DispatchArithmeticOperator::Multiply,
                    "/" => DispatchArithmeticOperator::Divide,
                    "%" => DispatchArithmeticOperator::Remainder,
                    _ => unreachable!("guarded arithmetic operator"),
                };
                if matches!(
                    operator,
                    DispatchArithmeticOperator::Add | DispatchArithmeticOperator::Multiply
                ) && self.checked_site_is_int(site)
                    && right < left
                {
                    std::mem::swap(&mut left, &mut right);
                }
                budget.charge_scalar_node()?;
                Ok(Some(dispatch_scalar_arithmetic_id(operator, left, right)))
            }
            ExprKind::Field(_, _) => {
                let Some(base) = self.canonical_dispatch_scalar_term(
                    &child_site(site, 0),
                    origins,
                    budget,
                    depth + 1,
                )?
                else {
                    return Ok(None);
                };
                let Some(CheckedFieldResolution::Data { fields, .. }) = self
                    .resolutions
                    .expressions
                    .get(site)
                    .and_then(|resolution| resolution.field.as_ref())
                else {
                    return Ok(None);
                };
                budget.charge_scalar_node()?;
                Ok(Some(dispatch_scalar_field_id(base, fields, budget)?))
            }
            _ => Ok(None),
        }
    }

    /// Narrow one branch environment from simple checked integer predicates.
    /// Failure to recognize a predicate deliberately leaves the environment
    /// unchanged; recognized bounds only remove values excluded by the branch.
    fn refined_env_for_condition(
        &self,
        condition_site: &ExprSiteId,
        assume_true: bool,
        env: &AbstractEnv,
    ) -> AbstractEnv {
        let mut refined = env.clone();
        self.refine_condition_into(condition_site, assume_true, &mut refined);
        refined
    }

    fn refine_condition_into(
        &self,
        condition_site: &ExprSiteId,
        assume_true: bool,
        env: &mut AbstractEnv,
    ) {
        let mut pending = vec![(condition_site.clone(), assume_true, 0_usize)];
        let mut steps = 0_usize;
        while let Some((condition_site, assume_true, depth)) = pending.pop() {
            steps = steps.saturating_add(1);
            if depth >= MAX_CALL_DEPTH || steps > MAX_EXACT_COLLECTION_ITEMS {
                // Refinement is an optional precision step. Stopping here
                // leaves a wider environment and therefore remains sound.
                return;
            }
            let Some(expression) = self.index.expression(&condition_site) else {
                continue;
            };
            match &expression.kind {
                ExprKind::UnOp(operator, _) if operator == "!" => {
                    pending.push((child_site(&condition_site, 0), !assume_true, depth + 1))
                }
                ExprKind::BinOp(operator, _, _) if operator == "&&" && assume_true => {
                    pending.push((child_site(&condition_site, 1), true, depth + 1));
                    pending.push((child_site(&condition_site, 0), true, depth + 1));
                }
                ExprKind::BinOp(operator, _, _) if operator == "||" && !assume_true => {
                    pending.push((child_site(&condition_site, 1), false, depth + 1));
                    pending.push((child_site(&condition_site, 0), false, depth + 1));
                }
                ExprKind::Conjunction(parts) if assume_true => {
                    for index in (0..parts.len()).rev() {
                        pending.push((child_site(&condition_site, index), true, depth + 1));
                    }
                }
                ExprKind::Disjunction(parts) if !assume_true => {
                    for index in (0..parts.len()).rev() {
                        pending.push((child_site(&condition_site, index), false, depth + 1));
                    }
                }
                ExprKind::BinOp(operator, _, _)
                    if matches!(operator.as_str(), "<" | "<=" | ">" | ">=" | "==" | "!=") =>
                {
                    let left_site = child_site(&condition_site, 0);
                    let right_site = child_site(&condition_site, 1);
                    if let Some(constant) = exact_int_literal(self.index.expression(&right_site)) {
                        if let Some(place) = self.abstract_int_place(&left_site) {
                            self.refine_place_comparison(
                                env,
                                &place,
                                operator,
                                constant,
                                assume_true,
                            );
                        }
                    } else if let Some(constant) =
                        exact_int_literal(self.index.expression(&left_site))
                    {
                        if let Some(place) = self.abstract_int_place(&right_site) {
                            self.refine_place_comparison(
                                env,
                                &place,
                                reverse_comparison(operator),
                                constant,
                                assume_true,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn abstract_int_place(&self, site: &ExprSiteId) -> Option<AbstractIntPlace> {
        let mut cursor = site.clone();
        let mut reversed_fields = Vec::new();
        for _ in 0..MAX_CALL_DEPTH {
            let expression = self.index.expression(&cursor)?;
            match &expression.kind {
                ExprKind::Var(_) => {
                    let resolution = self.resolutions.expressions.get(&cursor)?;
                    let CheckedValueBinding::Binder { site: binder, .. } =
                        resolution.value_binding.as_ref()?
                    else {
                        return None;
                    };
                    reversed_fields.reverse();
                    return Some(AbstractIntPlace {
                        binder: binder.clone(),
                        fields: reversed_fields,
                    });
                }
                ExprKind::Field(_, _) => {
                    let resolution = self.resolutions.expressions.get(&cursor)?;
                    let CheckedFieldResolution::Data { fields, .. } = resolution.field.as_ref()?
                    else {
                        return None;
                    };
                    reversed_fields.push(fields.clone());
                    cursor = child_site(&cursor, 0);
                }
                _ => return None,
            }
        }
        None
    }

    fn refine_place_comparison(
        &self,
        env: &mut AbstractEnv,
        place: &AbstractIntPlace,
        operator: &str,
        constant: i128,
        assume_true: bool,
    ) {
        let operator = if assume_true {
            Some(operator)
        } else {
            negated_comparison(operator)
        };
        let Some(operator) = operator else {
            return;
        };
        let runtime_min = i128::from(i64::MIN);
        let runtime_max = i128::from(i64::MAX);
        let bounds = match operator {
            "<" => constant
                .checked_sub(1)
                .map(|maximum| (runtime_min, maximum)),
            "<=" => Some((runtime_min, constant)),
            ">" => constant
                .checked_add(1)
                .map(|minimum| (minimum, runtime_max)),
            ">=" => Some((constant, runtime_max)),
            "==" => Some((constant, constant)),
            _ => None,
        };
        let Some((minimum, maximum)) = bounds else {
            return;
        };
        let Some(value) = env.get_mut(&place.binder) else {
            return;
        };
        let mut candidate = value.clone();
        if refine_abstract_int_value(&mut candidate, &place.fields, minimum, maximum) {
            *value = candidate;
        }
    }

    fn scoped_receiver_captures_from_value(
        &self,
        app_site: &ExprSiteId,
        family: &RuleDispatchKey,
        receiver: AbstractValue,
    ) -> Result<AbstractEnv, RelationalEndpointTotalityIssue> {
        let receiver_site = child_site(&child_site(app_site, 0), 0);
        let AbstractValue::Constructors(receiver_variants) = receiver else {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule receiver is not one exact checked constructor domain",
            ));
        };
        let mut receivers = receiver_variants.values();
        let Some(receiver) = receivers.next() else {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule receiver has no reachable constructor identity",
            ));
        };
        if receivers.next().is_some() {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule receiver has more than one possible constructor identity",
            ));
        }
        let resolution = self.resolutions.rule_families.get(family).ok_or_else(|| {
            self.issue(
                app_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule family is absent from the checked resolution snapshot",
            )
        })?;
        let declaration_id = resolution
            .candidates
            .first()
            .map(|candidate| candidate.declaration.clone())
            .ok_or_else(|| {
                self.issue(
                    app_site,
                    RelationalEndpointTotalityIssueReason::PartialRuleDispatch,
                    "scoped rule family has no checked candidates",
                )
            })?;
        if resolution
            .candidates
            .iter()
            .any(|candidate| candidate.declaration != declaration_id)
        {
            return Err(self.issue(
                app_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule family candidates disagree on their owning declaration",
            ));
        }
        let model_owner = self
            .resolutions
            .analysis_occurrence_to_data_owner
            .get(&declaration_id)
            .ok_or_else(|| {
                self.issue(
                    app_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "scoped rule declaration has no checked data-owner bridge",
                )
            })?;
        if receiver.identity.owner != CheckedDataTypeId::Declared(model_owner.clone()) {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule receiver identity differs from the checked rule owner",
            ));
        }
        let declaration = self
            .index
            .declarations
            .get(&declaration_id)
            .ok_or_else(|| {
                self.issue(
                    app_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "scoped rule owner declaration is absent from the semantic index",
                )
            })?;
        let Stmt::TypeDecl(TypeDecl::RuleScope { params, .. }) = declaration.statement.as_ref()
        else {
            return Err(self.issue(
                app_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule owner is not a RuleScope declaration",
            ));
        };
        if params.len() != receiver.fields.len()
            || receiver.identity.fields.len() != receiver.fields.len()
        {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule receiver payload disagrees with its checked parameters",
            ));
        }
        if params.len() > MAX_EXACT_COLLECTION_ITEMS {
            return Err(self.issue(
                &receiver_site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "scoped rule receiver exceeds the capture proof limit",
            ));
        }
        let anchor = ExprSiteId {
            analysis_program: app_site.analysis_program.clone(),
            declaration: declaration_id.declaration.clone(),
            normalized_declaration_ordinal: declaration_id.normalized_ordinal,
            ast_path: Box::new([]),
        };
        let mut captures = AbstractEnv::new();
        for (index, value) in receiver.fields.iter().cloned().enumerate() {
            let index = u32::try_from(index).map_err(|_| {
                self.issue(
                    &receiver_site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "scoped receiver field index exceeds the checked address space",
                )
            })?;
            captures.insert(
                structural_binder_site(&anchor, vec![BINDER_PARAMETER, index]),
                value,
            );
        }
        Ok(captures)
    }

    /// Recover the exact RuleScope payload already present in the current
    /// evaluation environment. A bare sibling call such as `leaf()` has no
    /// receiver expression to reevaluate, but closes over the same
    /// declaration-bound parameters as the scoped family that invoked it.
    fn ambient_scoped_captures(
        &self,
        family: &RuleDispatchKey,
        env: &AbstractEnv,
        call_site: &ExprSiteId,
    ) -> Result<AbstractEnv, RelationalEndpointTotalityIssue> {
        let Some(expected_scope) = family.scope.as_deref() else {
            return Ok(AbstractEnv::new());
        };
        let resolution = self.resolutions.rule_families.get(family).ok_or_else(|| {
            self.issue(
                call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule family is absent from the checked resolution snapshot",
            )
        })?;
        let declaration_id = resolution
            .candidates
            .first()
            .map(|candidate| candidate.declaration.clone())
            .ok_or_else(|| {
                self.issue(
                    call_site,
                    RelationalEndpointTotalityIssueReason::PartialRuleDispatch,
                    "scoped rule family has no checked candidates",
                )
            })?;
        if resolution
            .candidates
            .iter()
            .any(|candidate| candidate.declaration != declaration_id)
        {
            return Err(self.issue(
                call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule family candidates disagree on their owning declaration",
            ));
        }
        let declaration = self
            .index
            .declarations
            .get(&declaration_id)
            .ok_or_else(|| {
                self.issue(
                    call_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "scoped rule owner declaration is absent from the semantic index",
                )
            })?;
        let Stmt::TypeDecl(TypeDecl::RuleScope { name, params, .. }) =
            declaration.statement.as_ref()
        else {
            return Err(self.issue(
                call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule owner is not a RuleScope declaration",
            ));
        };
        if name != expected_scope {
            return Err(self.issue(
                call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "scoped rule owner name differs from its exact dispatch key",
            ));
        }
        let anchor = ExprSiteId {
            analysis_program: call_site.analysis_program.clone(),
            declaration: declaration_id.declaration.clone(),
            normalized_declaration_ordinal: declaration_id.normalized_ordinal,
            ast_path: Box::new([]),
        };
        let mut captures = AbstractEnv::new();
        for index in 0..params.len() {
            let index = u32::try_from(index).map_err(|_| {
                self.issue(
                    call_site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "scoped capture index exceeds the checked address space",
                )
            })?;
            let binder = structural_binder_site(&anchor, vec![BINDER_PARAMETER, index]);
            let value = env.get(&binder).cloned().ok_or_else(|| {
                self.issue(
                    call_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "bare scoped rule call escaped its exact receiver captures",
                )
            })?;
            captures.insert(binder, value);
        }
        Ok(captures)
    }

    fn bind_rule_head(
        &mut self,
        head_site: &ExprSiteId,
        arguments: &[AbstractValue],
        env: &mut AbstractEnv,
        call_site: &ExprSiteId,
    ) -> Result<HeadMatch, RelationalEndpointTotalityIssue> {
        let head = self.index.expression(head_site).ok_or_else(|| {
            self.issue(
                call_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule candidate head is absent from the semantic index",
            )
        })?;
        let ExprKind::App(_, head_arguments) = &head.kind else {
            return Err(self.issue(
                head_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule candidate head is not an application",
            ));
        };
        if head_arguments.len() != arguments.len() {
            return Ok(HeadMatch::No);
        }
        let head_resolution = self.resolutions.expressions.get(head_site).ok_or_else(|| {
            self.issue(
                head_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule head has no expression resolution",
            )
        })?;
        let source_order = canonical_source_indices(
            head_arguments.len(),
            head_resolution.named_arguments.as_ref(),
        )
        .ok_or_else(|| {
            self.issue(
                head_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule head has an invalid named-argument order",
            )
        })?;
        let mut result = HeadMatch::Yes;
        for (canonical_index, source_index) in source_order.into_iter().enumerate() {
            match self.bind_rule_argument(
                &child_site(head_site, source_index + 1),
                &arguments[canonical_index],
                env,
            )? {
                HeadMatch::No => return Ok(HeadMatch::No),
                HeadMatch::Maybe => result = HeadMatch::Maybe,
                HeadMatch::Yes => {}
            }
        }
        Ok(result)
    }

    fn bind_rule_argument(
        &mut self,
        argument_site: &ExprSiteId,
        value: &AbstractValue,
        env: &mut AbstractEnv,
    ) -> Result<HeadMatch, RelationalEndpointTotalityIssue> {
        self.bind_rule_argument_inner(argument_site, value, env, 0)
    }

    fn bind_rule_argument_inner(
        &mut self,
        argument_site: &ExprSiteId,
        value: &AbstractValue,
        env: &mut AbstractEnv,
        depth: usize,
    ) -> Result<HeadMatch, RelationalEndpointTotalityIssue> {
        if depth >= MAX_CALL_DEPTH {
            return Err(self.issue(
                argument_site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "rule-head pattern exceeds the endpoint proof depth limit",
            ));
        }
        let expression = self.index.expression(argument_site).ok_or_else(|| {
            self.issue(
                argument_site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked rule-head argument is absent from the semantic index",
            )
        })?;
        let expression = match &expression.kind {
            ExprKind::Var(name) if name == "_" => ShallowRuleHeadArgument::Wildcard,
            ExprKind::Var(_) => ShallowRuleHeadArgument::Variable,
            ExprKind::Lit(literal) => ShallowRuleHeadArgument::Literal(literal.clone()),
            ExprKind::App(_, arguments) => ShallowRuleHeadArgument::Application {
                argument_count: arguments.len(),
            },
            ExprKind::Tuple(items) => ShallowRuleHeadArgument::Tuple {
                item_count: items.len(),
            },
            _ => ShallowRuleHeadArgument::Unsupported,
        };
        let resolution = self.resolutions.expressions.get(argument_site).cloned();
        match expression {
            ShallowRuleHeadArgument::Wildcard => Ok(HeadMatch::Yes),
            ShallowRuleHeadArgument::Variable => match resolution
                .as_ref()
                .ok_or_else(|| {
                    self.issue(
                        argument_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "checked rule-head variable has no exact resolution",
                    )
                })?
                .value_binding
                .as_ref()
            {
                Some(CheckedValueBinding::Binder {
                    kind: CheckedBinderKind::RuleHead,
                    site: binder,
                }) => {
                    if let Some(existing) = env.get(binder) {
                        Ok(match abstract_equality(existing, value) {
                            TruthDomain::TRUE => HeadMatch::Yes,
                            TruthDomain::FALSE => HeadMatch::No,
                            _ => HeadMatch::Maybe,
                        })
                    } else {
                        env.insert(binder.clone(), value.clone());
                        Ok(HeadMatch::Yes)
                    }
                }
                Some(CheckedValueBinding::Constructor { .. }) => {
                    let identity = resolution
                        .as_ref()
                        .and_then(|resolution| resolution.exact_constructor.clone())
                        .ok_or_else(|| {
                            self.issue(
                                argument_site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "rule-head constructor has no exact checked identity",
                            )
                        })?;
                    if !identity.fields.is_empty() {
                        return Ok(HeadMatch::No);
                    }
                    match value {
                        AbstractValue::Constructors(variants) => {
                            let key = checked_explore_projection_constructor_digest(&identity);
                            if !variants.contains_key(&key) {
                                Ok(HeadMatch::No)
                            } else if variants.len() == 1 {
                                Ok(HeadMatch::Yes)
                            } else {
                                Ok(HeadMatch::Maybe)
                            }
                        }
                        AbstractValue::Unknown => Ok(HeadMatch::Maybe),
                        _ => Ok(HeadMatch::No),
                    }
                }
                _ => Err(self.issue(
                    argument_site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "rule-head variable has neither a RuleHead binder nor constructor identity",
                )),
            },
            ShallowRuleHeadArgument::Literal(literal) => Ok(
                match abstract_equality(&abstract_literal(&literal), value) {
                    TruthDomain::TRUE => HeadMatch::Yes,
                    TruthDomain::FALSE => HeadMatch::No,
                    _ => HeadMatch::Maybe,
                },
            ),
            ShallowRuleHeadArgument::Application { argument_count } => match resolution
                .as_ref()
                .ok_or_else(|| {
                    self.issue(
                        argument_site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "checked rule-head application has no exact resolution",
                    )
                })?
                .call_target
                .as_ref()
            {
                Some(CheckedCallTarget::Builtin { canonical_name, .. })
                    if canonical_name.as_ref() == "__typed" =>
                {
                    self.bind_rule_argument_inner(
                        &child_site(argument_site, 1),
                        value,
                        env,
                        depth + 1,
                    )
                }
                Some(CheckedCallTarget::Builtin { canonical_name, .. })
                    if canonical_name.as_ref() == "__named_arg" =>
                {
                    self.bind_rule_argument_inner(
                        &child_site(argument_site, 2),
                        value,
                        env,
                        depth + 1,
                    )
                }
                Some(CheckedCallTarget::Constructor { .. }) => {
                    let identity = resolution
                        .as_ref()
                        .and_then(|resolution| resolution.exact_constructor.clone())
                        .ok_or_else(|| {
                            self.issue(
                                argument_site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "rule-head constructor application has no exact identity",
                            )
                        })?;
                    let AbstractValue::Constructors(variants) = value else {
                        return Ok(if matches!(value, AbstractValue::Unknown) {
                            HeadMatch::Maybe
                        } else {
                            HeadMatch::No
                        });
                    };
                    let key = checked_explore_projection_constructor_digest(&identity);
                    let Some(variant) = variants.get(&key) else {
                        return Ok(HeadMatch::No);
                    };
                    if variant.identity != identity {
                        return Err(self.issue(
                            argument_site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "rule-head and endpoint constructor identities disagree",
                        ));
                    }
                    let order = canonical_source_indices(
                        argument_count,
                        resolution
                            .as_ref()
                            .and_then(|resolution| resolution.named_arguments.as_ref()),
                    )
                    .ok_or_else(|| {
                        self.issue(
                            argument_site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "rule-head constructor has an invalid named-argument order",
                        )
                    })?;
                    if order.len() != variant.fields.len() {
                        return Ok(HeadMatch::No);
                    }
                    let mut result = if variants.len() == 1 {
                        HeadMatch::Yes
                    } else {
                        HeadMatch::Maybe
                    };
                    for (canonical_index, source_index) in order.into_iter().enumerate() {
                        match self.bind_rule_argument_inner(
                            &child_site(argument_site, source_index + 1),
                            &variant.fields[canonical_index],
                            env,
                            depth + 1,
                        )? {
                            HeadMatch::No => return Ok(HeadMatch::No),
                            HeadMatch::Maybe => result = HeadMatch::Maybe,
                            HeadMatch::Yes => {}
                        }
                    }
                    Ok(result)
                }
                _ => Err(self.issue(
                    argument_site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    "rule-head application is neither a checked wrapper nor constructor",
                )),
            },
            ShallowRuleHeadArgument::Tuple { item_count } => {
                let AbstractValue::Tuple(values) = value else {
                    return Ok(if matches!(value, AbstractValue::Unknown) {
                        HeadMatch::Maybe
                    } else {
                        HeadMatch::No
                    });
                };
                if item_count != values.len() {
                    return Ok(HeadMatch::No);
                }
                let mut result = HeadMatch::Yes;
                for (index, value) in values.iter().enumerate() {
                    match self.bind_rule_argument_inner(
                        &child_site(argument_site, index),
                        value,
                        env,
                        depth + 1,
                    )? {
                        HeadMatch::No => return Ok(HeadMatch::No),
                        HeadMatch::Maybe => result = HeadMatch::Maybe,
                        HeadMatch::Yes => {}
                    }
                }
                Ok(result)
            }
            ShallowRuleHeadArgument::Unsupported => Err(self.issue(
                argument_site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                "rule-head shape is outside the endpoint proof language",
            )),
        }
    }

    fn clone_pattern_bounded(
        &self,
        site: &ExprSiteId,
        pattern: &Pat,
    ) -> Result<Pat, RelationalEndpointTotalityIssue> {
        let mut pending = vec![(pattern, 0_usize)];
        let mut nodes = 0_usize;
        while let Some((pattern, depth)) = pending.pop() {
            nodes = nodes.saturating_add(1);
            if depth >= MAX_CALL_DEPTH || nodes > MAX_EXACT_COLLECTION_ITEMS {
                return Err(self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "pattern exceeds the bounded endpoint proof syntax budget",
                ));
            }
            match pattern {
                Pat::Con(_, children) => pending.extend(
                    children
                        .iter()
                        .rev()
                        .map(|child| (child, depth.saturating_add(1))),
                ),
                Pat::NamedCon(_, fields) => pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|(_, child)| (child, depth.saturating_add(1))),
                ),
                Pat::As(child, _) => pending.push((child, depth.saturating_add(1))),
                Pat::Wild | Pat::Var(_) | Pat::Lit(_) => {}
            }
        }
        Ok(pattern.clone())
    }

    fn bind_pattern(
        &mut self,
        pattern_site_root: &ExprSiteId,
        pattern: &Pat,
        value: &AbstractValue,
        pattern_path: &[u32],
        env: &mut AbstractEnv,
    ) -> Result<PatternPartition, RelationalEndpointTotalityIssue> {
        self.bind_pattern_with_options(pattern_site_root, pattern, value, pattern_path, env, false)
    }

    fn bind_pattern_with_options(
        &mut self,
        pattern_site_root: &ExprSiteId,
        pattern: &Pat,
        value: &AbstractValue,
        pattern_path: &[u32],
        env: &mut AbstractEnv,
        allow_bare_fielded_tag: bool,
    ) -> Result<PatternPartition, RelationalEndpointTotalityIssue> {
        self.bind_pattern_inner(
            pattern_site_root,
            pattern,
            value,
            pattern_path,
            env,
            allow_bare_fielded_tag,
            0,
        )
    }

    fn bind_pattern_inner(
        &mut self,
        pattern_site_root: &ExprSiteId,
        pattern: &Pat,
        value: &AbstractValue,
        pattern_path: &[u32],
        env: &mut AbstractEnv,
        allow_bare_fielded_tag: bool,
        depth: usize,
    ) -> Result<PatternPartition, RelationalEndpointTotalityIssue> {
        if depth >= MAX_CALL_DEPTH {
            return Err(self.issue(
                pattern_site_root,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                "pattern depth exceeds the endpoint proof limit",
            ));
        }
        if value.is_unreachable() {
            return Ok(PatternPartition::all(AbstractValue::Unreachable));
        }
        match pattern {
            Pat::Wild => Ok(PatternPartition::all(value.clone())),
            Pat::Var(_) => {
                let mut binder_path = vec![BINDER_PATTERN];
                binder_path.extend_from_slice(pattern_path);
                env.insert(
                    structural_binder_site(pattern_site_root, binder_path),
                    value.clone(),
                );
                Ok(PatternPartition::all(value.clone()))
            }
            Pat::As(inner, _) => {
                let partition = self.bind_pattern_inner(
                    pattern_site_root,
                    inner,
                    value,
                    pattern_path,
                    env,
                    allow_bare_fielded_tag,
                    depth + 1,
                )?;
                if partition.may_match() {
                    let mut binder_path = vec![BINDER_PATTERN];
                    binder_path.extend_from_slice(pattern_path);
                    binder_path.push(u32::MAX);
                    env.insert(
                        structural_binder_site(pattern_site_root, binder_path),
                        partition.matched.clone(),
                    );
                }
                Ok(partition)
            }
            Pat::Lit(literal) => match abstract_equality(&abstract_literal(literal), value) {
                TruthDomain::TRUE => Ok(PatternPartition::all(value.clone())),
                TruthDomain::FALSE => Ok(PatternPartition::none(value.clone())),
                _ => Ok(PatternPartition::overlapping(value.clone())),
            },
            Pat::Con(_, _) | Pat::NamedCon(_, _) => {
                let checked_site = CheckedPatternSiteId {
                    analysis_program: pattern_site_root.analysis_program.clone(),
                    declaration: pattern_site_root.declaration.clone(),
                    normalized_declaration_ordinal: pattern_site_root
                        .normalized_declaration_ordinal,
                    ast_path: pattern_site_root.ast_path.clone(),
                    pattern_path: pattern_path.to_vec().into_boxed_slice(),
                };
                let checked = self
                    .resolutions
                    .constructor_patterns
                    .get(&checked_site)
                    .cloned()
                    .ok_or_else(|| {
                        self.issue(
                            pattern_site_root,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "constructor pattern has no exact checked identity",
                        )
                    })?;
                let AbstractValue::Constructors(variants) = value else {
                    return match value {
                        AbstractValue::Unknown => Ok(PatternPartition::overlapping(value.clone())),
                        _ => Ok(PatternPartition::none(value.clone())),
                    };
                };
                let constructor_key =
                    checked_explore_projection_constructor_digest(&checked.constructor);
                let Some(variant) = variants.get(&constructor_key) else {
                    return Ok(PatternPartition::none(value.clone()));
                };
                if variant.identity != checked.constructor {
                    return Err(self.issue(
                        pattern_site_root,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "constructor pattern identity disagrees with the endpoint value",
                    ));
                }

                let child_patterns = match pattern {
                    Pat::Con(_, children) => children.iter().collect::<Vec<_>>(),
                    Pat::NamedCon(_, fields) => {
                        fields.iter().map(|(_, child)| child).collect::<Vec<_>>()
                    }
                    _ => unreachable!("guarded constructor pattern"),
                };
                if let Pat::Con(_, children) = pattern {
                    if children.is_empty() && !allow_bare_fielded_tag && !variant.fields.is_empty()
                    {
                        return Ok(PatternPartition::none(value.clone()));
                    }
                    if !children.is_empty() && children.len() != variant.fields.len() {
                        return Ok(PatternPartition::none(value.clone()));
                    }
                }
                if checked.source_fields.len() != child_patterns.len() {
                    return Err(self.issue(
                        pattern_site_root,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "constructor pattern field count disagrees with its checked layout",
                    ));
                }

                let mut fields_are_total = true;
                for (source_index, (field, child_pattern)) in
                    checked.source_fields.iter().zip(child_patterns).enumerate()
                {
                    let canonical_index = variant
                        .identity
                        .fields
                        .iter()
                        .position(|candidate| candidate == field)
                        .ok_or_else(|| {
                            self.issue(
                                pattern_site_root,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "constructor pattern field is absent from its checked layout",
                            )
                        })?;
                    let child_value = variant.fields.get(canonical_index).ok_or_else(|| {
                        self.issue(
                            pattern_site_root,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "constructor endpoint payload is shorter than its checked layout",
                        )
                    })?;
                    let source_index = u32::try_from(source_index).map_err(|_| {
                        self.issue(
                            pattern_site_root,
                            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                            "constructor pattern index exceeds the checked address space",
                        )
                    })?;
                    let mut child_path = pattern_path.to_vec();
                    child_path.push(source_index);
                    let child_partition = self.bind_pattern_inner(
                        pattern_site_root,
                        child_pattern,
                        child_value,
                        &child_path,
                        env,
                        false,
                        depth + 1,
                    )?;
                    if !child_partition.may_match() {
                        return Ok(PatternPartition::none(value.clone()));
                    }
                    fields_are_total &= child_partition.definitely_all;
                }

                let matched = AbstractValue::Constructors(BTreeMap::from([(
                    constructor_key,
                    variant.clone(),
                )]));
                let mut unmatched_variants = variants.clone();
                if fields_are_total {
                    unmatched_variants.remove(&constructor_key);
                }
                let unmatched = if unmatched_variants.is_empty() {
                    AbstractValue::Unreachable
                } else {
                    AbstractValue::Constructors(unmatched_variants)
                };
                Ok(PatternPartition {
                    definitely_all: fields_are_total && unmatched.is_unreachable(),
                    matched,
                    unmatched: if fields_are_total {
                        unmatched
                    } else {
                        value.clone()
                    },
                })
            }
        }
    }

    fn eval_builtin_leaf(
        &mut self,
        name: &str,
        mut arguments: Vec<AbstractValue>,
        site: &ExprSiteId,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        let _input_retained = self.new_tuple_clone_budget(arguments.iter(), site)?;
        let input = AbstractValue::Tuple(arguments.clone().into_boxed_slice());
        let arity = arguments.len();
        let result = match (name, arity) {
            ("__named_arg", 2) => arguments.pop().expect("checked arity"),
            ("__typed", 2) => arguments.remove(0),
            ("map_new", 0) => AbstractValue::Map(Box::new([])),
            ("not", 1) => {
                let truth = arguments.remove(0).truth().ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "not requires one proved Bool",
                    )
                })?;
                AbstractValue::Bool(truth.not())
            }
            ("length", 1) => match arguments.remove(0) {
                AbstractValue::List(sequence) => {
                    let (minimum, maximum) = sequence.lengths();
                    let interval = IntInterval::new(i128::from(minimum), i128::from(maximum))
                        .and_then(IntInterval::runtime_int)
                        .ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                                "collection length may exceed Futuruna Int",
                            )
                        })?;
                    AbstractValue::Int(interval)
                }
                AbstractValue::String(Some(value)) => {
                    let length = i64::try_from(value.chars().count()).map_err(|_| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                            "string length exceeds Futuruna Int",
                        )
                    })?;
                    AbstractValue::Int(IntInterval::singleton(length))
                }
                AbstractValue::String(None) => AbstractValue::Int(IntInterval {
                    minimum: i128::from(i64::MIN),
                    maximum: i128::from(i64::MAX),
                }),
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "length requires a proved List or String",
                    ))
                }
            },
            ("sum_list", 1) => match arguments.remove(0) {
                AbstractValue::List(AbstractSequence::Exact(values)) => {
                    let mut total = IntInterval::singleton(0);
                    for value in values.iter() {
                        let item = value.int().ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "sum_list requires a proved List(Int)",
                            )
                        })?;
                        total = total.checked_add(item).ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                                "sum_list may overflow Futuruna Int",
                            )
                        })?;
                    }
                    AbstractValue::Int(total)
                }
                AbstractValue::List(AbstractSequence::Summary {
                    element,
                    minimum_length,
                    maximum_length,
                }) => {
                    let item = element.int().ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "sum_list requires a proved List(Int)",
                        )
                    })?;
                    let lengths =
                        IntInterval::new(i128::from(minimum_length), i128::from(maximum_length))
                            .expect("sequence summaries preserve ordered length bounds");
                    let total = item.checked_mul(lengths).ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                            "sum_list may overflow Futuruna Int",
                        )
                    })?;
                    AbstractValue::Int(total)
                }
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "sum_list requires a proved List(Int)",
                    ))
                }
            },
            ("string_length", 1) => match arguments.remove(0) {
                AbstractValue::String(Some(value)) => {
                    let length = i64::try_from(value.chars().count()).map_err(|_| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                            "string length exceeds Futuruna Int",
                        )
                    })?;
                    AbstractValue::Int(IntInterval::singleton(length))
                }
                AbstractValue::String(None) => AbstractValue::Int(IntInterval {
                    minimum: i128::from(i64::MIN),
                    maximum: i128::from(i64::MAX),
                }),
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "string_length requires a proved String",
                    ))
                }
            },
            ("concat", 2) => {
                let right = arguments.pop().expect("checked arity");
                let left = arguments.pop().expect("checked arity");
                match (left, right) {
                    (AbstractValue::String(Some(left)), AbstractValue::String(Some(right))) => {
                        concat_abstract_strings(left, right)
                    }
                    (AbstractValue::String(_), AbstractValue::String(_)) => {
                        AbstractValue::String(None)
                    }
                    (AbstractValue::List(left), AbstractValue::List(right)) => {
                        AbstractValue::List(concat_sequences(left, right, site, self.role)?)
                    }
                    _ => {
                        return Err(self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "concat operands have no proved common List or String shape",
                        ))
                    }
                }
            }
            ("head", 1) => match arguments.remove(0) {
                AbstractValue::List(AbstractSequence::Exact(values)) => {
                    values.first().cloned().ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                            "head is not total on an empty List",
                        )
                    })?
                }
                AbstractValue::List(AbstractSequence::Summary {
                    element,
                    minimum_length,
                    ..
                }) if minimum_length > 0 => *element,
                AbstractValue::List(_) => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                        "head input is not proved nonempty",
                    ))
                }
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "head requires a proved finite List",
                    ))
                }
            },
            ("distinct", 1) => match arguments.remove(0) {
                AbstractValue::List(AbstractSequence::Exact(values)) => {
                    let mut output = Vec::<AbstractValue>::new();
                    let mut exact = true;
                    for value in values.into_vec() {
                        let equalities = output
                            .iter()
                            .map(|existing| abstract_equality(existing, &value))
                            .collect::<Vec<_>>();
                        if equalities.iter().any(|truth| *truth == TruthDomain::TRUE) {
                            continue;
                        }
                        exact &= equalities.iter().all(|truth| *truth == TruthDomain::FALSE);
                        output.push(value);
                    }
                    if exact || output.is_empty() {
                        AbstractValue::List(AbstractSequence::Exact(output.into_boxed_slice()))
                    } else {
                        let maximum_length = u64::try_from(output.len()).map_err(|_| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                                "distinct result length exceeds the proof format",
                            )
                        })?;
                        AbstractValue::List(AbstractSequence::Summary {
                            element: Box::new(
                                join_values(output).unwrap_or(AbstractValue::Unknown),
                            ),
                            minimum_length: 1,
                            maximum_length,
                        })
                    }
                }
                AbstractValue::List(AbstractSequence::Summary {
                    maximum_length: 0, ..
                }) => AbstractValue::List(AbstractSequence::Exact(Box::new([]))),
                AbstractValue::List(AbstractSequence::Summary {
                    element,
                    minimum_length,
                    maximum_length,
                }) => AbstractValue::List(AbstractSequence::Summary {
                    element,
                    minimum_length: u64::from(minimum_length > 0),
                    maximum_length,
                }),
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "distinct requires a proved finite List",
                    ))
                }
            },
            ("contains", 2) => {
                let needle = arguments.pop().expect("checked arity");
                match arguments.pop().expect("checked arity") {
                    AbstractValue::String(Some(haystack)) => {
                        let AbstractValue::String(Some(needle)) = needle else {
                            return Err(self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                                "String contains requires an exact String needle",
                            ));
                        };
                        AbstractValue::Bool(TruthDomain::from_bool(
                            haystack.contains(needle.as_ref()),
                        ))
                    }
                    AbstractValue::List(AbstractSequence::Exact(values)) => {
                        let mut truth = TruthDomain::FALSE;
                        for value in values.iter() {
                            truth = abstract_or(truth, abstract_equality(value, &needle));
                            if truth == TruthDomain::TRUE {
                                break;
                            }
                        }
                        AbstractValue::Bool(truth)
                    }
                    AbstractValue::List(AbstractSequence::Summary { .. })
                    | AbstractValue::String(None) => AbstractValue::Bool(TruthDomain::BOTH),
                    _ => {
                        return Err(self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "contains requires a proved List or String",
                        ))
                    }
                }
            }
            ("abs", 1) => match arguments.remove(0) {
                AbstractValue::Int(interval) => {
                    if interval.contains(i128::from(i64::MIN)) {
                        return Err(self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                            "abs input interval contains the minimum Futuruna Int",
                        ));
                    }
                    let endpoints = [interval.minimum.abs(), interval.maximum.abs()];
                    AbstractValue::Int(IntInterval {
                        minimum: if interval.contains(0) {
                            0
                        } else {
                            *endpoints.iter().min().expect("two endpoints")
                        },
                        maximum: *endpoints.iter().max().expect("two endpoints"),
                    })
                }
                AbstractValue::Float(_) => AbstractValue::Float(None),
                _ => {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "abs requires a proved Int or Float",
                    ))
                }
            },
            ("range", 2) => {
                let end = arguments
                    .pop()
                    .expect("checked arity")
                    .int()
                    .and_then(IntInterval::singleton_value)
                    .ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                            "range end must be one exact Int in an endpoint helper",
                        )
                    })?;
                let start = arguments
                    .pop()
                    .expect("checked arity")
                    .int()
                    .and_then(IntInterval::singleton_value)
                    .ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                            "range start must be one exact Int in an endpoint helper",
                        )
                    })?;
                if end < start {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "range start is greater than its end in a reachable endpoint helper",
                    ));
                }
                if end == start {
                    AbstractValue::List(AbstractSequence::Exact(Box::new([])))
                } else {
                    let length =
                        u64::try_from(i128::from(end) - i128::from(start)).map_err(|_| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                                "range length exceeds the endpoint proof format",
                            )
                        })?;
                    if usize::try_from(length)
                        .ok()
                        .is_some_and(|length| length <= MAX_EXACT_COLLECTION_ITEMS)
                    {
                        AbstractValue::List(AbstractSequence::Exact(
                            (start..end)
                                .map(|value| AbstractValue::Int(IntInterval::singleton(value)))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ))
                    } else {
                        let maximum = end.checked_sub(1).ok_or_else(|| {
                            self.issue(
                                site,
                                RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                                "nonempty range end cannot be decremented safely",
                            )
                        })?;
                        AbstractValue::List(AbstractSequence::Summary {
                            element: Box::new(AbstractValue::Int(IntInterval {
                                minimum: i128::from(start),
                                maximum: i128::from(maximum),
                            })),
                            minimum_length: length,
                            maximum_length: length,
                        })
                    }
                }
            }
            _ => {
                return Err(self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::UnknownCall,
                    format!("builtin `{name}`/{arity} is outside the endpoint proof whitelist"),
                ))
            }
        };
        self.require_bounded_value(&result, site)?;
        if matches!(
            name,
            "head" | "distinct" | "concat" | "contains" | "length" | "sum_list" | "range"
        ) {
            self.record(site, ObligationKind::Collection, &input, &result)?;
        }
        Ok(result)
    }

    fn eval_unop(
        &mut self,
        operator: &str,
        value: AbstractValue,
        site: &ExprSiteId,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        match operator {
            "!" => value
                .truth()
                .map(|truth| AbstractValue::Bool(truth.not()))
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "logical negation operand is not a proved Bool",
                    )
                }),
            "-" => {
                let interval = value.int().ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "integer negation operand is not a proved Int interval",
                    )
                })?;
                let result = interval.checked_neg().ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                        "integer negation may overflow on this endpoint domain",
                    )
                })?;
                let result = AbstractValue::Int(result);
                self.record(site, ObligationKind::Negation, &value, &result)?;
                Ok(result)
            }
            "+" | "&" | "&mut" => Ok(value),
            _ => Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                format!("unary operator `{operator}` is not in the endpoint proof language"),
            )),
        }
    }

    fn eval_binop(
        &mut self,
        operator: &str,
        left: AbstractValue,
        right: AbstractValue,
        site: &ExprSiteId,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        if matches!(operator, "==" | "!=") {
            for (side, value) in [("left", &left), ("right", &right)] {
                let bounds = value.shape_bounds();
                self.require_shape_bounds(bounds, site)?;
                let Some(runtime_nodes) = value.runtime_equality_materialization_nodes() else {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        format!(
                            "{side} structural equality operand has no finite runtime-materialization bound"
                        ),
                    ));
                };
                if runtime_nodes > MAX_RUNTIME_EQUALITY_VALUE_NODES {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                        format!(
                            "{side} structural equality operand needs {} runtime value nodes; canonical replay limit is {MAX_RUNTIME_EQUALITY_VALUE_NODES}",
                            runtime_nodes
                        ),
                    ));
                }
            }
            if !self.runtime_direct_equality_returns_bool(&left, &right) {
                return Err(self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    "direct equality is not proved to return Bool for these runtime value shapes",
                ));
            }
            let _input_retained = self.new_tuple_clone_budget([&left, &right].into_iter(), site)?;
            let input = AbstractValue::Tuple(vec![left.clone(), right.clone()].into_boxed_slice());
            let mut result = abstract_equality(&left, &right);
            if operator == "!=" {
                result = result.not();
            }
            let result = AbstractValue::Bool(result);
            self.record(site, ObligationKind::Equality, &input, &result)?;
            return Ok(result);
        }
        let _input_retained = self.new_tuple_clone_budget([&left, &right].into_iter(), site)?;
        let input = AbstractValue::Tuple(vec![left.clone(), right.clone()].into_boxed_slice());
        if matches!(operator, "<" | "<=" | ">" | ">=") {
            if let (Some(left), Some(right)) = (left.int(), right.int()) {
                return Ok(AbstractValue::Bool(compare_intervals(
                    operator, left, right,
                )));
            }
            if matches!(
                (&left, &right),
                (AbstractValue::Float(_), AbstractValue::Float(_))
                    | (AbstractValue::Float(_), AbstractValue::Int(_))
                    | (AbstractValue::Int(_), AbstractValue::Float(_))
            ) {
                return Ok(AbstractValue::Bool(TruthDomain::BOTH));
            }
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "ordered comparison operands have no proved runtime-compatible shape",
            ));
        }
        if operator == "+"
            && (matches!(left, AbstractValue::String(_))
                || matches!(right, AbstractValue::String(_)))
        {
            return Ok(AbstractValue::String(None));
        }
        if matches!(
            (&left, &right),
            (AbstractValue::Float(_), AbstractValue::Float(_))
                | (AbstractValue::Float(_), AbstractValue::Int(_))
                | (AbstractValue::Int(_), AbstractValue::Float(_))
        ) && matches!(operator, "+" | "-" | "*" | "/")
        {
            return Ok(AbstractValue::Float(None));
        }
        let (Some(left_interval), Some(right_interval)) = (left.int(), right.int()) else {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                format!("binary operator `{operator}` operands are not proved Int intervals"),
            ));
        };
        let (kind, result) = match operator {
            "+" => (
                ObligationKind::Addition,
                left_interval.checked_add(right_interval),
            ),
            "-" => (
                ObligationKind::Subtraction,
                left_interval.checked_sub(right_interval),
            ),
            "*" => (
                ObligationKind::Multiplication,
                left_interval.checked_mul(right_interval),
            ),
            "/" => {
                if right_interval.contains(0) {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded,
                        format!(
                            "integer divisor interval {}..={} contains zero",
                            right_interval.minimum, right_interval.maximum
                        ),
                    ));
                }
                (
                    ObligationKind::Division,
                    left_interval.checked_div(right_interval),
                )
            }
            "%" => {
                if right_interval.contains(0) {
                    return Err(self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded,
                        format!(
                            "integer remainder divisor interval {}..={} contains zero",
                            right_interval.minimum, right_interval.maximum
                        ),
                    ));
                }
                (
                    ObligationKind::Remainder,
                    left_interval.checked_rem(right_interval),
                )
            }
            _ => {
                return Err(self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    format!("binary operator `{operator}` is not in the endpoint proof language"),
                ))
            }
        };
        let result = result.ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded,
                format!("integer operator `{operator}` may overflow on this endpoint domain"),
            )
        })?;
        let result = AbstractValue::Int(result);
        self.record(site, kind, &input, &result)?;
        Ok(result)
    }

    fn runtime_direct_equality_returns_bool(
        &self,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> bool {
        let constructors_supported = |variants: &BTreeMap<[u8; 32], AbstractConstructor>| {
            variants
                .values()
                .all(|variant| match &variant.identity.owner {
                    CheckedDataTypeId::Intrinsic { .. } => true,
                    CheckedDataTypeId::Declared(owner) => self
                        .resolutions
                        .data_owner_to_analysis_occurrence
                        .get(owner)
                        .and_then(|occurrence| self.index.declarations.get(occurrence))
                        .is_some_and(|declaration| {
                            !matches!(
                                declaration.statement.as_ref(),
                                Stmt::TypeDecl(TypeDecl::RuleScope { .. })
                            )
                        }),
                })
        };
        use AbstractValue::*;
        match (left, right) {
            (Int(_), Int(_))
            | (Float(_), Float(_))
            | (String(_), String(_))
            | (Bool(_), Bool(_))
            | (List(_), List(_)) => true,
            (Constructors(left), Constructors(right)) => {
                constructors_supported(left) && constructors_supported(right)
            }
            (Constructors(variants), _) | (_, Constructors(variants)) => {
                constructors_supported(variants)
            }
            _ => false,
        }
    }

    fn project_field(
        &mut self,
        site: &ExprSiteId,
        resolution: &CheckedExpressionResolution,
        base: AbstractValue,
    ) -> Result<AbstractValue, RelationalEndpointTotalityIssue> {
        let CheckedFieldResolution::Data { fields, .. } =
            resolution.field.as_ref().ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                    "field access has no exact checked field resolution",
                )
            })?
        else {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::UnknownCall,
                "first-class scoped member has no exact endpoint callable value",
            ));
        };
        let AbstractValue::Constructors(variants) = base else {
            return Err(self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "field base lost the constructor precision required for total projection",
            ));
        };
        let mut values = Vec::new();
        for variant in variants.values() {
            let checked = fields
                .iter()
                .find(|field| {
                    field.identity.owner == variant.identity.owner
                        && field.variant_index == variant.identity.variant_index
                })
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "field is not present on every possible endpoint constructor variant",
                    )
                })?;
            let canonical_index = variant
                .identity
                .fields
                .iter()
                .position(|field| field == &checked.identity)
                .ok_or_else(|| {
                    self.issue(
                        site,
                        RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                        "checked field identity is absent from its constructor layout",
                    )
                })?;
            values.push(
                variant
                    .fields
                    .get(canonical_index)
                    .cloned()
                    .ok_or_else(|| {
                        self.issue(
                            site,
                            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                            "endpoint constructor payload is shorter than its checked layout",
                        )
                    })?,
            );
        }
        join_values(values).ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "endpoint field projection has no reachable constructor variant",
            )
        })
    }

    fn finish_index(
        &mut self,
        site: &ExprSiteId,
        collection: AbstractValue,
        index_value: AbstractValue,
    ) -> Result<BudgetedAbstractValue, RelationalEndpointTotalityIssue> {
        let index = index_value
            .int()
            .and_then(IntInterval::singleton_value)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    "endpoint indexing requires one exact nonnegative index",
                )
            })?;
        let sequence = match &collection {
            AbstractValue::List(sequence) => sequence,
            _ => {
                return Err(self.issue(
                    site,
                    RelationalEndpointTotalityIssueReason::UnsupportedExpression,
                    "endpoint indexing requires a proved finite List",
                ))
            }
        };
        let result = match sequence {
            AbstractSequence::Exact(values) => values.get(index),
            AbstractSequence::Summary {
                element,
                minimum_length,
                ..
            } if u64::try_from(index)
                .ok()
                .is_some_and(|index| index < *minimum_length) =>
            {
                Some(element.as_ref())
            }
            _ => None,
        }
        .ok_or_else(|| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
                "endpoint index is not proved in bounds for every possible sequence",
            )
        })?;
        let mut result_retained = self.new_retained_budget(site)?;
        self.retain_value(&mut result_retained, result, site)?;
        let result = result.clone();
        let _input_retained =
            self.new_tuple_clone_budget([&collection, &index_value].into_iter(), site)?;
        let input = AbstractValue::Tuple(vec![collection, index_value].into_boxed_slice());
        self.record(site, ObligationKind::Index, &input, &result)?;
        Ok(BudgetedAbstractValue {
            value: result,
            _retained: result_retained,
        })
    }

    fn proof_root(&self) -> RelationalEndpointAbstractProofRoot {
        let mut hasher = Sha256::new();
        hash_segment(&mut hasher, PROOF_ROOT_V1);
        hash_segment(&mut hasher, &self.relation_id.bytes());
        hash_segment(&mut hasher, &(self.obligations.len() as u64).to_le_bytes());
        for obligation in &self.obligations {
            hash_segment(&mut hasher, &[endpoint_role_tag(obligation.role)]);
            hash_segment(&mut hasher, &obligation.site);
            hash_segment(&mut hasher, &[obligation.kind as u8]);
            hash_segment(&mut hasher, &obligation.input_root);
            hash_segment(&mut hasher, &obligation.result_root);
        }
        RelationalEndpointAbstractProofRoot::from_canonical_bytes(hasher.finalize().into())
    }

    fn record(
        &mut self,
        site: &ExprSiteId,
        kind: ObligationKind,
        input: &AbstractValue,
        result: &AbstractValue,
    ) -> Result<(), RelationalEndpointTotalityIssue> {
        self.require_bounded_value(input, site)?;
        self.require_bounded_value(result, site)?;
        let expression_site = site.clone();
        let mechanism_site = MechanismSiteId::from_expression_site(site).map_err(|error| {
            self.issue(
                site,
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                error.to_string(),
            )
        })?;
        let input_root = abstract_value_root(input);
        let result_root = abstract_value_root(result);
        let obligation = ProofObligation {
            role: self.role,
            site: mechanism_site.digest_bytes(),
            kind,
            input_root,
            result_root,
        };
        if !self.obligations.contains(&obligation)
            && self.obligations.len() >= MAX_PROOF_OBLIGATIONS
        {
            return Err(self.issue(
                &expression_site,
                RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                format!("endpoint proof exceeds {MAX_PROOF_OBLIGATIONS} distinct obligations"),
            ));
        }
        self.obligations.insert(obligation);
        Ok(())
    }
}

fn checked_expression_type_name(checked: &CheckedExpressionType) -> String {
    match checked {
        CheckedExpressionType::Resolved(ty) => ty.to_string(),
        CheckedExpressionType::Callable { parameters, result } => {
            let parameters = parameters
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({parameters}) -> {result}")
        }
        CheckedExpressionType::CallableReference => "callable reference".into(),
        CheckedExpressionType::PolymorphicEmptyList => "empty List(_)".into(),
        CheckedExpressionType::Unsupported => "unresolved".into(),
    }
}

fn abstract_value_shape(value: &AbstractValue) -> &'static str {
    match value {
        AbstractValue::Unreachable => "unreachable",
        AbstractValue::Unknown => "unknown",
        AbstractValue::Int(_) => "Int",
        AbstractValue::Bool(_) => "Bool",
        AbstractValue::Float(_) => "Float",
        AbstractValue::String(_) => "String",
        AbstractValue::Character(_) => "Char",
        AbstractValue::Unit => "Unit",
        AbstractValue::Constructors(_) => "constructor",
        AbstractValue::List(_) => "List",
        AbstractValue::Set(_) => "Set",
        AbstractValue::Map(_) => "Map",
        AbstractValue::Tuple(_) => "Tuple",
        AbstractValue::Callable(_) => "callable",
    }
}

fn substitute_type_parameters(ty: &Ty, substitutions: &BTreeMap<String, Ty>) -> Ty {
    match ty {
        Ty::Name(name) | Ty::Var(name) if substitutions.contains_key(name) => {
            substitutions[name].clone()
        }
        Ty::App(constructor, arguments) => Ty::App(
            Box::new(substitute_type_parameters(constructor, substitutions)),
            arguments
                .iter()
                .map(|argument| substitute_type_parameters(argument, substitutions))
                .collect(),
        ),
        Ty::Arrow(parameter, result) => Ty::Arrow(
            Box::new(substitute_type_parameters(parameter, substitutions)),
            Box::new(substitute_type_parameters(result, substitutions)),
        ),
        Ty::Ref(inner) => Ty::Ref(Box::new(substitute_type_parameters(inner, substitutions))),
        Ty::MutRef(inner) => Ty::MutRef(Box::new(substitute_type_parameters(inner, substitutions))),
        Ty::Shared(inner) => Ty::Shared(Box::new(substitute_type_parameters(inner, substitutions))),
        Ty::Optional(inner) => {
            Ty::Optional(Box::new(substitute_type_parameters(inner, substitutions)))
        }
        Ty::Name(_) | Ty::Var(_) | Ty::Unit | Ty::Hole => ty.clone(),
    }
}

fn join_inferred_types(types: impl IntoIterator<Item = Option<Ty>>) -> Option<Ty> {
    let mut types = types.into_iter();
    let mut result = match types.next() {
        Some(Some(ty)) => ty,
        Some(None) => return None,
        None => return Some(Ty::Hole),
    };
    for ty in types {
        result = merge_inferred_type(result, ty?)?;
    }
    Some(result)
}

fn merge_inferred_type(left: Ty, right: Ty) -> Option<Ty> {
    match (left, right) {
        (Ty::Hole, value) | (value, Ty::Hole) => Some(value),
        (
            Ty::App(left_constructor, left_arguments),
            Ty::App(right_constructor, right_arguments),
        ) if left_constructor == right_constructor
            && left_arguments.len() == right_arguments.len() =>
        {
            Some(Ty::App(
                left_constructor,
                left_arguments
                    .into_iter()
                    .zip(right_arguments)
                    .map(|(left, right)| merge_inferred_type(left, right))
                    .collect::<Option<Vec<_>>>()?,
            ))
        }
        (Ty::Arrow(left_parameter, left_result), Ty::Arrow(right_parameter, right_result)) => {
            Some(Ty::Arrow(
                Box::new(merge_inferred_type(*left_parameter, *right_parameter)?),
                Box::new(merge_inferred_type(*left_result, *right_result)?),
            ))
        }
        (Ty::Ref(left), Ty::Ref(right)) => {
            Some(Ty::Ref(Box::new(merge_inferred_type(*left, *right)?)))
        }
        (Ty::MutRef(left), Ty::MutRef(right)) => {
            Some(Ty::MutRef(Box::new(merge_inferred_type(*left, *right)?)))
        }
        (Ty::Shared(left), Ty::Shared(right)) => {
            Some(Ty::Shared(Box::new(merge_inferred_type(*left, *right)?)))
        }
        (Ty::Optional(left), Ty::Optional(right)) => {
            Some(Ty::Optional(Box::new(merge_inferred_type(*left, *right)?)))
        }
        (left, right) if left == right => Some(left),
        _ => None,
    }
}

/// Commit the normalized abstract over-approximation on which the endpoint
/// proof ran. This is a relation-scoped `(state, context)` domain commitment,
/// not a claim that the concrete Cartesian domain was materialized.
fn endpoint_domain_root(
    relation_id: RelationId,
    role: RelationalEndpointRole,
    state: &AbstractValue,
    context: &AbstractValue,
) -> RelationalEndpointProofDomainRoot {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, PROOF_DOMAIN_ROOT_V1);
    hash_segment(&mut hasher, &relation_id.bytes());
    hash_segment(&mut hasher, &[endpoint_role_tag(role)]);
    hash_segment(&mut hasher, &abstract_value_root(state));
    hash_segment(&mut hasher, &abstract_value_root(context));
    RelationalEndpointProofDomainRoot::from_canonical_bytes(hasher.finalize().into())
}

fn endpoint_role_tag(role: RelationalEndpointRole) -> u8 {
    match role {
        RelationalEndpointRole::Before => 0x01,
        RelationalEndpointRole::After => 0x02,
    }
}

fn child_site(site: &ExprSiteId, child: usize) -> ExprSiteId {
    let mut path = site.ast_path.to_vec();
    path.push(u32::try_from(child).unwrap_or(u32::MAX));
    ExprSiteId {
        analysis_program: site.analysis_program.clone(),
        declaration: site.declaration.clone(),
        normalized_declaration_ordinal: site.normalized_declaration_ordinal,
        ast_path: path.into_boxed_slice(),
    }
}

fn hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn abstract_value_root(value: &AbstractValue) -> [u8; 32] {
    type Captures = Arc<[(CheckedBinderSiteId, AbstractValue)]>;

    enum Task<'a> {
        Value(&'a AbstractValue),
        FinishValue {
            value: &'a AbstractValue,
            child_count: usize,
        },
        Captures(&'a Captures),
        FinishCaptures {
            captures: &'a Captures,
            identity: usize,
        },
    }

    let mut tasks = vec![Task::Value(value)];
    let mut roots = Vec::<[u8; 32]>::new();
    let mut capture_roots = BTreeMap::<usize, [u8; 32]>::new();
    while let Some(task) = tasks.pop() {
        match task {
            Task::Value(value) => {
                let child_count = match value {
                    AbstractValue::Constructors(variants) => {
                        variants.values().map(|variant| variant.fields.len()).sum()
                    }
                    AbstractValue::List(AbstractSequence::Exact(values))
                    | AbstractValue::Set(AbstractSequence::Exact(values))
                    | AbstractValue::Tuple(values) => values.len(),
                    AbstractValue::List(AbstractSequence::Summary { .. })
                    | AbstractValue::Set(AbstractSequence::Summary { .. }) => 1,
                    AbstractValue::Map(entries) => entries.len().saturating_mul(2),
                    AbstractValue::Callable(AbstractCallable::Lambda { .. }) => 1,
                    _ => 0,
                };
                tasks.push(Task::FinishValue { value, child_count });
                match value {
                    AbstractValue::Constructors(variants) => {
                        for variant in variants.values().rev() {
                            for field in variant.fields.iter().rev() {
                                tasks.push(Task::Value(field));
                            }
                        }
                    }
                    AbstractValue::List(AbstractSequence::Exact(values))
                    | AbstractValue::Set(AbstractSequence::Exact(values))
                    | AbstractValue::Tuple(values) => {
                        for value in values.iter().rev() {
                            tasks.push(Task::Value(value));
                        }
                    }
                    AbstractValue::List(AbstractSequence::Summary { element, .. })
                    | AbstractValue::Set(AbstractSequence::Summary { element, .. }) => {
                        tasks.push(Task::Value(element));
                    }
                    AbstractValue::Map(entries) => {
                        for (key, value) in entries.iter().rev() {
                            tasks.push(Task::Value(value));
                            tasks.push(Task::Value(key));
                        }
                    }
                    AbstractValue::Callable(AbstractCallable::Lambda { captured, .. }) => {
                        tasks.push(Task::Captures(captured));
                    }
                    _ => {}
                }
            }
            Task::Captures(captures) => {
                let identity = Arc::as_ptr(captures) as *const () as usize;
                if let Some(root) = capture_roots.get(&identity).copied() {
                    roots.push(root);
                    continue;
                }
                tasks.push(Task::FinishCaptures { captures, identity });
                for (_, value) in captures.iter().rev() {
                    tasks.push(Task::Value(value));
                }
            }
            Task::FinishCaptures { captures, identity } => {
                let child_start = roots
                    .len()
                    .checked_sub(captures.len())
                    .expect("capture hash children are complete");
                let children = roots.split_off(child_start);
                let mut hasher = Sha256::new();
                hash_segment(
                    &mut hasher,
                    b"futuruna.explore.endpoint-totality.abstract-captures.v1\0",
                );
                hash_segment(&mut hasher, &(captures.len() as u64).to_le_bytes());
                for ((binder, _), child) in captures.iter().zip(children) {
                    hash_segment(
                        &mut hasher,
                        &checked_explore_projection_binder_digest(binder),
                    );
                    hash_segment(&mut hasher, &child);
                }
                let root = hasher.finalize().into();
                capture_roots.insert(identity, root);
                roots.push(root);
            }
            Task::FinishValue { value, child_count } => {
                let child_start = roots
                    .len()
                    .checked_sub(child_count)
                    .expect("abstract-value hash children are complete");
                let children = roots.split_off(child_start);
                let mut children = children.iter();
                let mut hasher = Sha256::new();
                hash_segment(&mut hasher, ABSTRACT_VALUE_V1);
                match value {
                    AbstractValue::Unreachable => hash_segment(&mut hasher, &[0x00]),
                    AbstractValue::Unknown => hash_segment(&mut hasher, &[0x01]),
                    AbstractValue::Int(interval) => {
                        hash_segment(&mut hasher, &[0x02]);
                        hash_segment(&mut hasher, &interval.minimum.to_le_bytes());
                        hash_segment(&mut hasher, &interval.maximum.to_le_bytes());
                    }
                    AbstractValue::Bool(truth) => {
                        hash_segment(&mut hasher, &[0x03, truth.0]);
                    }
                    AbstractValue::Float(value) => {
                        hash_segment(&mut hasher, &[0x04]);
                        match value {
                            Some(value) => hash_segment(&mut hasher, &value.to_le_bytes()),
                            None => hash_segment(&mut hasher, &[]),
                        }
                    }
                    AbstractValue::String(value) => {
                        hash_segment(&mut hasher, &[0x05]);
                        hash_segment(&mut hasher, value.as_deref().unwrap_or("").as_bytes());
                        hash_segment(&mut hasher, &[value.is_some() as u8]);
                    }
                    AbstractValue::Character(value) => {
                        hash_segment(&mut hasher, &[0x06]);
                        hash_segment(
                            &mut hasher,
                            &value.map(u32::from).unwrap_or(0).to_le_bytes(),
                        );
                        hash_segment(&mut hasher, &[value.is_some() as u8]);
                    }
                    AbstractValue::Unit => hash_segment(&mut hasher, &[0x07]),
                    AbstractValue::Constructors(variants) => {
                        hash_segment(&mut hasher, &[0x08]);
                        hash_segment(&mut hasher, &(variants.len() as u64).to_le_bytes());
                        for (identity, variant) in variants {
                            hash_segment(&mut hasher, identity);
                            hash_segment(&mut hasher, &(variant.fields.len() as u64).to_le_bytes());
                            for _ in variant.fields.iter() {
                                hash_segment(
                                    &mut hasher,
                                    children.next().expect("constructor field root"),
                                );
                            }
                        }
                    }
                    AbstractValue::List(sequence) | AbstractValue::Set(sequence) => {
                        hash_segment(
                            &mut hasher,
                            &[if matches!(value, AbstractValue::List(_)) {
                                0x09
                            } else {
                                0x0a
                            }],
                        );
                        match sequence {
                            AbstractSequence::Exact(values) => {
                                hash_segment(&mut hasher, &[0x01]);
                                hash_segment(&mut hasher, &(values.len() as u64).to_le_bytes());
                                for _ in values.iter() {
                                    hash_segment(
                                        &mut hasher,
                                        children.next().expect("sequence element root"),
                                    );
                                }
                            }
                            AbstractSequence::Summary {
                                minimum_length,
                                maximum_length,
                                ..
                            } => {
                                hash_segment(&mut hasher, &[0x02]);
                                hash_segment(&mut hasher, &minimum_length.to_le_bytes());
                                hash_segment(&mut hasher, &maximum_length.to_le_bytes());
                                hash_segment(
                                    &mut hasher,
                                    children.next().expect("summary element root"),
                                );
                            }
                        }
                    }
                    AbstractValue::Map(entries) => {
                        hash_segment(&mut hasher, &[0x0b]);
                        hash_segment(&mut hasher, &(entries.len() as u64).to_le_bytes());
                        for _ in entries.iter() {
                            hash_segment(&mut hasher, children.next().expect("map key root"));
                            hash_segment(&mut hasher, children.next().expect("map value root"));
                        }
                    }
                    AbstractValue::Tuple(values) => {
                        hash_segment(&mut hasher, &[0x0c]);
                        hash_segment(&mut hasher, &(values.len() as u64).to_le_bytes());
                        for _ in values.iter() {
                            hash_segment(&mut hasher, children.next().expect("tuple element root"));
                        }
                    }
                    AbstractValue::Callable(callable) => {
                        hash_segment(&mut hasher, &[0x0d]);
                        match callable {
                            AbstractCallable::Function(callable) => {
                                hash_segment(&mut hasher, &[0x01]);
                                hash_segment(
                                    &mut hasher,
                                    callable.declaration.declaration.semantic_key().as_bytes(),
                                );
                                hash_segment(
                                    &mut hasher,
                                    &(callable.declaration.normalized_ordinal as u64).to_le_bytes(),
                                );
                                hash_segment(
                                    &mut hasher,
                                    &(callable.structural_path.len() as u64).to_le_bytes(),
                                );
                                for component in callable.structural_path.iter().copied() {
                                    hash_segment(&mut hasher, &component.to_le_bytes());
                                }
                            }
                            AbstractCallable::RuleFamily(family) => {
                                hash_segment(&mut hasher, &[0x02]);
                                hash_segment(
                                    &mut hasher,
                                    family.scope.as_deref().unwrap_or("").as_bytes(),
                                );
                                hash_segment(&mut hasher, family.name.as_bytes());
                                hash_segment(&mut hasher, &(family.arity as u64).to_le_bytes());
                            }
                            AbstractCallable::Lambda {
                                body_site,
                                parameters,
                                ..
                            } => {
                                hash_segment(&mut hasher, &[0x03]);
                                let site = MechanismSiteId::from_expression_site(body_site)
                                    .map(|site| site.digest_bytes())
                                    .unwrap_or([0; 32]);
                                hash_segment(&mut hasher, &site);
                                hash_segment(&mut hasher, &(parameters.len() as u64).to_le_bytes());
                                hash_segment(
                                    &mut hasher,
                                    children.next().expect("lambda capture root"),
                                );
                            }
                        }
                    }
                }
                debug_assert!(children.next().is_none());
                roots.push(hasher.finalize().into());
            }
        }
    }
    let [root] = roots.as_slice() else {
        unreachable!("one abstract value produces one canonical root")
    };
    *root
}

fn join_values(values: impl IntoIterator<Item = AbstractValue>) -> Option<AbstractValue> {
    let mut values = values.into_iter();
    let mut result = values.next()?;
    for value in values {
        result = join_value(result, value);
    }
    Some(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactSortKeyFamily {
    Int,
    Float,
    String,
    Character,
    Bool,
}

fn total_sort_key_family(value: &AbstractValue) -> Option<ExactSortKeyFamily> {
    match value {
        AbstractValue::Int(_) => Some(ExactSortKeyFamily::Int),
        AbstractValue::Float(Some(bits)) if !f64::from_bits(*bits).is_nan() => {
            Some(ExactSortKeyFamily::Float)
        }
        AbstractValue::String(_) => Some(ExactSortKeyFamily::String),
        AbstractValue::Character(_) => Some(ExactSortKeyFamily::Character),
        AbstractValue::Bool(_) => Some(ExactSortKeyFamily::Bool),
        _ => None,
    }
}

fn exact_sort_key_family(value: &AbstractValue) -> Option<ExactSortKeyFamily> {
    match value {
        AbstractValue::Int(interval) if interval.singleton_value().is_some() => {
            Some(ExactSortKeyFamily::Int)
        }
        AbstractValue::Float(Some(bits)) if !f64::from_bits(*bits).is_nan() => {
            Some(ExactSortKeyFamily::Float)
        }
        AbstractValue::String(Some(_)) => Some(ExactSortKeyFamily::String),
        AbstractValue::Character(Some(_)) => Some(ExactSortKeyFamily::Character),
        AbstractValue::Bool(truth) if truth.singleton().is_some() => Some(ExactSortKeyFamily::Bool),
        _ => None,
    }
}

fn compare_exact_sort_keys(
    left: &AbstractValue,
    right: &AbstractValue,
) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    use AbstractValue::{Bool, Character, Float, Int, String};

    match (left, right) {
        (Int(left), Int(right)) => Some(left.singleton_value()?.cmp(&right.singleton_value()?)),
        (Float(Some(left)), Float(Some(right))) => Some(
            f64::from_bits(*left)
                .partial_cmp(&f64::from_bits(*right))
                .unwrap_or(Ordering::Equal),
        ),
        (String(Some(left)), String(Some(right))) => Some(left.cmp(right)),
        (Character(Some(left)), Character(Some(right))) => Some(left.cmp(right)),
        (Bool(left), Bool(right)) => Some(left.singleton()?.cmp(&right.singleton()?)),
        _ => None,
    }
}

fn join_value(left: AbstractValue, right: AbstractValue) -> AbstractValue {
    join_value_at_depth(left, right, 1)
}

fn join_value_at_depth(left: AbstractValue, right: AbstractValue, depth: usize) -> AbstractValue {
    use AbstractValue::*;
    match (left, right) {
        (Unreachable, value) | (value, Unreachable) => value,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Int(left), Int(right)) => Int(left.hull(right)),
        (Bool(left), Bool(right)) => Bool(left.join(right)),
        (Float(left), Float(right)) => Float((left == right).then_some(left).flatten()),
        (String(left), String(right)) => String((left == right).then_some(left).flatten()),
        (Character(left), Character(right)) => Character((left == right).then_some(left).flatten()),
        (Unit, Unit) => Unit,
        (Constructors(mut left), Constructors(right)) => {
            for (key, right) in right {
                match left.remove(&key) {
                    None => {
                        left.insert(key, right);
                    }
                    Some(existing) if existing.fields.len() == right.fields.len() => {
                        let identity = existing.identity;
                        let fields = existing
                            .fields
                            .into_vec()
                            .into_iter()
                            .zip(right.fields.into_vec())
                            .map(|(left, right)| join_value_at_depth(left, right, depth + 1))
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        left.insert(key, AbstractConstructor { identity, fields });
                    }
                    Some(_) => return Unknown,
                }
            }
            Constructors(left)
        }
        (List(left), List(right)) => List(join_sequence(left, right, depth + 1)),
        (Set(left), Set(right)) => Set(join_sequence(left, right, depth + 1)),
        (Map(left), Map(right)) if left == right => Map(left),
        (Tuple(left), Tuple(right)) if left.len() == right.len() => Tuple(
            left.into_vec()
                .into_iter()
                .zip(right.into_vec())
                .map(|(left, right)| join_value_at_depth(left, right, depth + 1))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        (Callable(left), Callable(right)) if left == right => Callable(left),
        _ => Unknown,
    }
}

fn join_sequence(
    left: AbstractSequence,
    right: AbstractSequence,
    depth: usize,
) -> AbstractSequence {
    match (left, right) {
        (AbstractSequence::Exact(left), AbstractSequence::Exact(right))
            if left.len() == right.len() =>
        {
            AbstractSequence::Exact(
                left.into_vec()
                    .into_iter()
                    .zip(right.into_vec())
                    .map(|(left, right)| join_value_at_depth(left, right, depth + 1))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )
        }
        (left, right) => {
            let (left_min, left_max) = left.lengths();
            let (right_min, right_max) = right.lengths();
            let element = join_values(
                [left.joined_element(), right.joined_element()]
                    .into_iter()
                    .flatten(),
            )
            .unwrap_or(AbstractValue::Unknown);
            AbstractSequence::Summary {
                element: Box::new(element),
                minimum_length: left_min.min(right_min),
                maximum_length: left_max.max(right_max),
            }
        }
    }
}

fn canonical_arguments(
    source: &mut Vec<AbstractValue>,
    order: Option<&CheckedNamedArgumentOrder>,
    site: &ExprSiteId,
    role: RelationalEndpointRole,
) -> Result<Vec<AbstractValue>, RelationalEndpointTotalityIssue> {
    let Some(order) = order else {
        return Ok(std::mem::take(source));
    };
    if order.canonical_source_indices.len() != source.len()
        || order.parameter_names.len() != source.len()
    {
        return Err(RelationalEndpointTotalityIssue::new(
            role,
            site.clone(),
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
            "checked named-argument permutation has the wrong arity",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut canonical = Vec::with_capacity(source.len());
    for source_index in order.canonical_source_indices.iter().copied() {
        if source_index >= source.len() || !seen.insert(source_index) {
            return Err(RelationalEndpointTotalityIssue::new(
                role,
                site.clone(),
                RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
                "checked named-argument order is not a complete permutation",
            ));
        }
        canonical.push(source[source_index].clone());
    }
    Ok(canonical)
}

fn canonical_argument_sites(
    argument_count: usize,
    order: Option<&CheckedNamedArgumentOrder>,
    site: &ExprSiteId,
    role: RelationalEndpointRole,
) -> Result<Vec<ExprSiteId>, RelationalEndpointTotalityIssue> {
    let indices = canonical_source_indices(argument_count, order).ok_or_else(|| {
        RelationalEndpointTotalityIssue::new(
            role,
            site.clone(),
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
            "checked named-argument sites are not a complete permutation",
        )
    })?;
    Ok(indices
        .into_iter()
        .map(|source_index| {
            let argument_site = child_site(site, source_index + 1);
            if order.is_some() {
                child_site(&argument_site, 2)
            } else {
                argument_site
            }
        })
        .collect())
}

fn canonical_source_indices(
    argument_count: usize,
    order: Option<&CheckedNamedArgumentOrder>,
) -> Option<Vec<usize>> {
    let Some(order) = order else {
        return Some((0..argument_count).collect());
    };
    if order.canonical_source_indices.len() != argument_count
        || order.parameter_names.len() != argument_count
    {
        return None;
    }
    let mut seen = BTreeSet::new();
    let indices = order
        .canonical_source_indices
        .iter()
        .copied()
        .inspect(|index| {
            seen.insert(*index);
        })
        .collect::<Vec<_>>();
    (indices.iter().all(|index| *index < argument_count) && seen.len() == argument_count)
        .then_some(indices)
}

fn abstract_literal(literal: &Literal) -> AbstractValue {
    match literal {
        Literal::Int(value) => AbstractValue::Int(IntInterval::singleton(*value)),
        Literal::Float(value) => AbstractValue::Float(Some(value.to_bits())),
        Literal::Str(value) if value.len() <= MAX_EXACT_STRING_BYTES => {
            AbstractValue::String(Some(value.clone().into_boxed_str()))
        }
        Literal::Str(_) => AbstractValue::String(None),
        Literal::Char(value) => AbstractValue::Character(Some(*value)),
        Literal::Bool(value) => AbstractValue::Bool(TruthDomain::from_bool(*value)),
    }
}

fn concat_abstract_strings(left: Box<str>, right: Box<str>) -> AbstractValue {
    let exact_length = left.len().checked_add(right.len());
    if exact_length.is_some_and(|length| length <= MAX_EXACT_STRING_BYTES) {
        let mut joined = String::with_capacity(exact_length.expect("bounded length"));
        joined.push_str(&left);
        joined.push_str(&right);
        AbstractValue::String(Some(joined.into_boxed_str()))
    } else {
        // Exact bytes are only a precision aid. Widen before allocating an
        // exponentially grown string.
        AbstractValue::String(None)
    }
}

fn single_constructor(
    identity: CheckedConstructorIdentity,
    fields: Box<[AbstractValue]>,
) -> AbstractValue {
    let digest = checked_explore_projection_constructor_digest(&identity);
    AbstractValue::Constructors(BTreeMap::from([(
        digest,
        AbstractConstructor { identity, fields },
    )]))
}

fn pattern_binder_count(pattern: &Pat) -> usize {
    let mut pending = vec![pattern];
    let mut binders = 0_usize;
    while let Some(pattern) = pending.pop() {
        match pattern {
            Pat::Var(_) => binders = binders.saturating_add(1),
            Pat::As(inner, _) => {
                binders = binders.saturating_add(1);
                pending.push(inner);
            }
            Pat::Con(_, children) => pending.extend(children.iter()),
            Pat::NamedCon(_, fields) => {
                pending.extend(fields.iter().map(|(_, child)| child));
            }
            Pat::Wild | Pat::Lit(_) => {}
        }
    }
    binders
}

fn compare_intervals(operator: &str, left: IntInterval, right: IntInterval) -> TruthDomain {
    let (always_true, always_false) = match operator {
        "<" => (left.maximum < right.minimum, left.minimum >= right.maximum),
        "<=" => (left.maximum <= right.minimum, left.minimum > right.maximum),
        ">" => (left.minimum > right.maximum, left.maximum <= right.minimum),
        ">=" => (left.minimum >= right.maximum, left.maximum < right.minimum),
        _ => (false, false),
    };
    if always_true {
        TruthDomain::TRUE
    } else if always_false {
        TruthDomain::FALSE
    } else {
        TruthDomain::BOTH
    }
}

fn constructor_runtime_discriminants_differ(
    left: &AbstractConstructor,
    right: &AbstractConstructor,
) -> bool {
    if left.identity.variant != right.identity.variant
        || left.identity.layout != right.identity.layout
        || left.fields.len() != right.fields.len()
    {
        return true;
    }
    if left.identity.layout == CheckedConstructorLayout::Named {
        let left_names = left.identity.fields.iter().map(|field| field.name.as_ref());
        let right_names = right
            .identity
            .fields
            .iter()
            .map(|field| field.name.as_ref());
        return left_names.ne(right_names);
    }
    false
}

fn abstract_equality(left: &AbstractValue, right: &AbstractValue) -> TruthDomain {
    use AbstractValue::*;
    match (left, right) {
        (Unreachable, _) | (_, Unreachable) | (Unknown, _) | (_, Unknown) => TruthDomain::BOTH,
        (Int(left), Int(right)) => {
            if left.maximum < right.minimum || right.maximum < left.minimum {
                TruthDomain::FALSE
            } else if left.singleton_value().is_some()
                && left.singleton_value() == right.singleton_value()
            {
                TruthDomain::TRUE
            } else {
                TruthDomain::BOTH
            }
        }
        (Bool(left), Bool(right)) => {
            if left.0 & right.0 == 0 {
                TruthDomain::FALSE
            } else if left.singleton().is_some() && left == right {
                TruthDomain::TRUE
            } else {
                TruthDomain::BOTH
            }
        }
        // Direct Float equality is IEEE while structural collection and named
        // constructor equality use an epsilon comparison. BOTH is the common
        // sound abstraction (including NaN and infinities).
        (Float(_), Float(_)) => TruthDomain::BOTH,
        (String(Some(left)), String(Some(right))) => TruthDomain::from_bool(left == right),
        (String(_), String(_)) => TruthDomain::BOTH,
        (Character(Some(left)), Character(Some(right))) => TruthDomain::from_bool(left == right),
        (Character(_), Character(_)) => TruthDomain::BOTH,
        // Runtime structural equality has no Unit arm. Keep collection
        // contains/distinct reasoning conservative instead of treating Unit
        // as reflexive here.
        (Unit, Unit) => TruthDomain::BOTH,
        (Constructors(left), Constructors(right)) => {
            if !left.is_empty()
                && !right.is_empty()
                && left.values().all(|left| {
                    right
                        .values()
                        .all(|right| constructor_runtime_discriminants_differ(left, right))
                })
            {
                return TruthDomain::FALSE;
            }
            let shared = left
                .keys()
                .filter(|key| right.contains_key(*key))
                .copied()
                .collect::<Vec<_>>();
            if shared.is_empty() {
                // Checked ownership is stronger than runtime constructor
                // equality: separately declared constructors can still have
                // the same runtime spelling and payload. Identity disjointness
                // therefore cannot by itself prove runtime inequality. The
                // runtime-visible discriminants above can.
                return TruthDomain::BOTH;
            }
            if left.len() == 1 && right.len() == 1 {
                let key = shared[0];
                let left = &left[&key];
                let right = &right[&key];
                if constructor_runtime_discriminants_differ(left, right) {
                    return TruthDomain::FALSE;
                }
                if left.identity == right.identity && left.fields.is_empty() {
                    return TruthDomain::TRUE;
                }
            }
            TruthDomain::BOTH
        }
        (List(AbstractSequence::Exact(left)), List(AbstractSequence::Exact(right))) => {
            if left.len() != right.len() {
                TruthDomain::FALSE
            } else if left.is_empty() {
                TruthDomain::TRUE
            } else {
                TruthDomain::BOTH
            }
        }
        (List(_), List(_)) | (Set(_), Set(_)) => TruthDomain::BOTH,
        // Runtime equality normalizes root Nil/Cons constructors and Value::List
        // through one list-like comparison. Keep the cross-representation case
        // conservative unless the abstract domain itself is normalized.
        (List(_), Constructors(_)) | (Constructors(_), List(_)) => TruthDomain::BOTH,
        (Map(left), Map(right)) => {
            if left.len() != right.len() {
                TruthDomain::FALSE
            } else {
                TruthDomain::BOTH
            }
        }
        (Tuple(left), Tuple(right)) => {
            if left.len() != right.len() {
                TruthDomain::FALSE
            } else {
                TruthDomain::BOTH
            }
        }
        (Callable(_), Callable(_)) => TruthDomain::BOTH,
        _ => TruthDomain::FALSE,
    }
}

fn abstract_and(left: TruthDomain, right: TruthDomain) -> TruthDomain {
    let may_be_true = left.may_be_true() && right.may_be_true();
    let may_be_false = left.may_be_false() || right.may_be_false();
    TruthDomain((u8::from(may_be_false)) | (u8::from(may_be_true) << 1))
}

fn abstract_or(left: TruthDomain, right: TruthDomain) -> TruthDomain {
    let may_be_true = left.may_be_true() || right.may_be_true();
    let may_be_false = left.may_be_false() && right.may_be_false();
    TruthDomain((u8::from(may_be_false)) | (u8::from(may_be_true) << 1))
}

fn structural_binder_site(site: &ExprSiteId, binder_path: Vec<u32>) -> CheckedBinderSiteId {
    CheckedBinderSiteId::Structural {
        analysis_program: site.analysis_program.clone(),
        declaration: site.declaration.clone(),
        normalized_declaration_ordinal: site.normalized_declaration_ordinal,
        ast_path: site.ast_path.clone(),
        binder_path: binder_path.into_boxed_slice(),
    }
}

fn env_is_unreachable(env: &AbstractEnv) -> bool {
    env.values().any(AbstractValue::is_unreachable)
}

fn exact_int_literal(expression: Option<&Expr>) -> Option<i128> {
    match &expression?.kind {
        ExprKind::Lit(Literal::Int(value)) => Some(i128::from(*value)),
        ExprKind::UnOp(operator, inner) if operator == "-" => match &inner.kind {
            ExprKind::Lit(Literal::Int(value)) => i128::from(*value).checked_neg(),
            _ => None,
        },
        _ => None,
    }
}

fn reverse_comparison(operator: &str) -> &str {
    match operator {
        "<" => ">",
        "<=" => ">=",
        ">" => "<",
        ">=" => "<=",
        other => other,
    }
}

fn negated_comparison(operator: &str) -> Option<&str> {
    match operator {
        "<" => Some(">="),
        "<=" => Some(">"),
        ">" => Some("<="),
        ">=" => Some("<"),
        "==" => None,
        "!=" => Some("=="),
        _ => None,
    }
}

fn refine_abstract_int_value(
    value: &mut AbstractValue,
    fields: &[Box<[CheckedVariantField]>],
    minimum: i128,
    maximum: i128,
) -> bool {
    if fields.is_empty() {
        return match value {
            AbstractValue::Int(interval) => {
                let minimum = interval.minimum.max(minimum);
                let maximum = interval.maximum.min(maximum);
                *value = match IntInterval::new(minimum, maximum) {
                    Some(interval) => AbstractValue::Int(interval),
                    None => AbstractValue::Unreachable,
                };
                true
            }
            AbstractValue::Unreachable => true,
            _ => false,
        };
    }
    let AbstractValue::Constructors(variants) = value else {
        return false;
    };
    let mut refined = BTreeMap::new();
    for (key, mut variant) in std::mem::take(variants) {
        let mut matching = fields[0].iter().filter(|field| {
            field.identity.owner == variant.identity.owner
                && field.variant_index == variant.identity.variant_index
                && field.layout == variant.identity.layout
                && variant.identity.fields.get(field.field_index) == Some(&field.identity)
        });
        let Some(field) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        let Some(field_value) = variant.fields.get_mut(field.field_index) else {
            return false;
        };
        if !refine_abstract_int_value(field_value, &fields[1..], minimum, maximum) {
            return false;
        }
        if !field_value.is_unreachable() {
            refined.insert(key, variant);
        }
    }
    *value = if refined.is_empty() {
        AbstractValue::Unreachable
    } else {
        AbstractValue::Constructors(refined)
    };
    true
}

fn concat_sequences(
    left: AbstractSequence,
    right: AbstractSequence,
    site: &ExprSiteId,
    role: RelationalEndpointRole,
) -> Result<AbstractSequence, RelationalEndpointTotalityIssue> {
    match (left, right) {
        (AbstractSequence::Exact(left), AbstractSequence::Exact(right)) => {
            let length = left.len().checked_add(right.len()).ok_or_else(|| {
                RelationalEndpointTotalityIssue::new(
                    role,
                    site.clone(),
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "concat result length overflowed",
                )
            })?;
            if length > MAX_EXACT_COLLECTION_ITEMS {
                return Err(RelationalEndpointTotalityIssue::new(
                    role,
                    site.clone(),
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "concat result exceeds the exact collection proof limit",
                ));
            }
            let mut values = left.into_vec();
            values.extend(right.into_vec());
            Ok(AbstractSequence::Exact(values.into_boxed_slice()))
        }
        (left, right) => {
            let (left_minimum, left_maximum) = left.lengths();
            let (right_minimum, right_maximum) = right.lengths();
            let minimum_length = left_minimum.checked_add(right_minimum).ok_or_else(|| {
                RelationalEndpointTotalityIssue::new(
                    role,
                    site.clone(),
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "concat minimum length overflowed",
                )
            })?;
            let maximum_length = left_maximum.checked_add(right_maximum).ok_or_else(|| {
                RelationalEndpointTotalityIssue::new(
                    role,
                    site.clone(),
                    RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                    "concat maximum length overflowed",
                )
            })?;
            if maximum_length == 0 {
                return Ok(AbstractSequence::Exact(Box::new([])));
            }
            let element = join_values(
                [left.joined_element(), right.joined_element()]
                    .into_iter()
                    .flatten(),
            )
            .unwrap_or(AbstractValue::Unknown);
            Ok(AbstractSequence::Summary {
                element: Box::new(element),
                minimum_length,
                maximum_length,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CheckedDataFieldId, CheckedExploreQueryAccessError, CheckedExploreQueryArtifactIssue,
        Lexer, Parser, TypeChecker,
    };

    #[test]
    fn remainder_hull_keeps_zero_when_a_possible_divisor_equals_the_dividend() {
        let dividend = IntInterval::singleton(5);
        let divisors = IntInterval {
            minimum: 2,
            maximum: 5,
        };
        let remainder = dividend
            .checked_rem(divisors)
            .expect("nonzero bounded divisors");

        assert!(remainder.contains(0));
        assert!(remainder.contains(2));
    }

    #[test]
    fn structural_equality_does_not_invent_runtime_unit_or_set_reflexivity() {
        assert_eq!(
            abstract_equality(&AbstractValue::Unit, &AbstractValue::Unit),
            TruthDomain::BOTH
        );
        let empty_set = AbstractValue::Set(AbstractSequence::Exact(Box::new([])));
        assert_eq!(abstract_equality(&empty_set, &empty_set), TruthDomain::BOTH);
    }

    #[test]
    fn equality_materialization_expands_list_summaries_to_cons_and_nil() {
        let summary = |maximum_length| {
            AbstractValue::List(AbstractSequence::Summary {
                element: Box::new(AbstractValue::Int(IntInterval::singleton(0))),
                minimum_length: 0,
                maximum_length,
            })
        };

        assert_eq!(
            summary(255).runtime_equality_materialization_nodes(),
            Some(511)
        );
        assert_eq!(
            summary(256).runtime_equality_materialization_nodes(),
            Some(MAX_RUNTIME_EQUALITY_VALUE_NODES + 1)
        );
        assert_eq!(
            AbstractValue::Unknown.runtime_equality_materialization_nodes(),
            None
        );
    }

    #[test]
    fn exact_string_concat_widens_before_allocating_past_its_byte_cap() {
        let left = "x".repeat(MAX_EXACT_STRING_BYTES / 2 + 1).into_boxed_str();
        let right = "y".repeat(MAX_EXACT_STRING_BYTES / 2 + 1).into_boxed_str();

        assert_eq!(
            concat_abstract_strings(left, right),
            AbstractValue::String(None)
        );
        assert_eq!(
            abstract_literal(&Literal::Str("z".repeat(MAX_EXACT_STRING_BYTES + 1))),
            AbstractValue::String(None)
        );
    }

    #[test]
    fn abstract_value_shape_tracks_depth_independently_from_node_count() {
        let mut value = AbstractValue::Unit;
        for _ in 0..MAX_ABSTRACT_VALUE_DEPTH {
            value = AbstractValue::Tuple(vec![value].into_boxed_slice());
        }

        let bounds = value.shape_bounds();
        assert_eq!(bounds.nodes, MAX_ABSTRACT_VALUE_DEPTH + 1);
        assert_eq!(bounds.depth, MAX_ABSTRACT_VALUE_DEPTH + 1);
    }

    #[test]
    fn retained_value_budget_refuses_aggregate_growth_past_either_cap() {
        let full = RetainedValueBudget::default()
            .checked_retaining(AbstractValueShapeBounds {
                nodes: MAX_RETAINED_FRAME_VALUE_NODES,
                depth: 1,
                exact_string_bytes: MAX_RETAINED_FRAME_EXACT_STRING_BYTES,
            })
            .expect("the exact aggregate budget is admissible");

        assert!(full
            .checked_retaining(AbstractValueShapeBounds {
                nodes: 1,
                depth: 1,
                exact_string_bytes: 0,
            })
            .is_none());
        assert!(RetainedValueBudget::default()
            .checked_retaining(AbstractValueShapeBounds {
                nodes: 1,
                depth: 1,
                exact_string_bytes: MAX_RETAINED_FRAME_EXACT_STRING_BYTES + 1,
            })
            .is_none());
    }

    #[test]
    fn live_proof_retention_is_released_instead_of_accumulating_work() {
        let ledger = Rc::new(ProofRetentionLedger::default());
        let mut retained =
            RetainedValueBudget::attached(Rc::clone(&ledger)).expect("one retained proof frame");
        assert!(retained.try_retain(AbstractValueShapeBounds {
            nodes: 7,
            depth: 1,
            exact_string_bytes: 11,
        }));
        assert!(retained.try_retain_slots(3));
        assert_eq!(
            ledger.totals.get(),
            ProofRetentionTotals {
                nodes: 7,
                exact_string_bytes: 11,
                slots: 3,
                frames: 1,
                cache_entries: 0,
            }
        );

        drop(retained);
        assert_eq!(ledger.totals.get(), ProofRetentionTotals::default());
    }

    #[test]
    fn shared_environment_clones_reuse_one_live_retention_lease() {
        let ledger = Rc::new(ProofRetentionLedger::default());
        let retained = RetainedValueBudget::attached(Rc::clone(&ledger))
            .expect("one retained environment frame");
        let env = Arc::new(BudgetedAbstractEnv {
            bindings: AbstractEnv::new(),
            _retained: retained,
        });
        let clones = (0..1_024).map(|_| Arc::clone(&env)).collect::<Vec<_>>();

        assert_eq!(ledger.totals.get().frames, 1);
        drop(clones);
        assert_eq!(ledger.totals.get().frames, 1);
        drop(env);
        assert_eq!(ledger.totals.get(), ProofRetentionTotals::default());
    }

    #[test]
    fn aggregate_proof_retention_refuses_before_crossing_its_live_cap() {
        let ledger = Rc::new(ProofRetentionLedger::default());
        assert!(ledger.try_reserve(MAX_RETAINED_PROOF_VALUE_NODES, 0, 0, 0, 0));
        let mut retained = RetainedValueBudget::attached(Rc::clone(&ledger))
            .expect("frame count remains available");
        assert!(!retained.try_retain(AbstractValueShapeBounds {
            nodes: 1,
            depth: 1,
            exact_string_bytes: 0,
        }));
        drop(retained);
        ledger.release(MAX_RETAINED_PROOF_VALUE_NODES, 0, 0, 0, 0);
        assert_eq!(ledger.totals.get(), ProofRetentionTotals::default());
    }

    #[test]
    fn dispatch_scalar_ids_are_canonical_and_distinguish_exact_field_identity() {
        let field = CheckedVariantField {
            variant: "Only".into(),
            variant_index: 0,
            field_index: 0,
            layout: CheckedConstructorLayout::Named,
            identity: CheckedDataFieldId {
                owner: CheckedDataTypeId::Intrinsic {
                    canonical_name: "DispatchFixture".into(),
                },
                variant_index: 0,
                field_index: 0,
                name: "amount".into(),
            },
        };
        let base = dispatch_scalar_argument_id(0);
        let first = dispatch_scalar_field_id(
            base,
            std::slice::from_ref(&field),
            &mut DispatchCanonicalizationBudget::default(),
        )
        .expect("one checked field projection");
        let repeated = dispatch_scalar_field_id(
            base,
            std::slice::from_ref(&field.clone()),
            &mut DispatchCanonicalizationBudget::default(),
        )
        .expect("equal checked field projection");
        let mut different = field.clone();
        different.field_index = 1;
        different.identity.field_index = 1;
        different.identity.name = "other_amount".into();
        let different = dispatch_scalar_field_id(
            base,
            std::slice::from_ref(&different),
            &mut DispatchCanonicalizationBudget::default(),
        )
        .expect("distinct checked field projection");

        assert_eq!(first, repeated);
        assert_ne!(first, different);
        assert_ne!(
            dispatch_scalar_argument_id(0),
            dispatch_scalar_argument_id(1)
        );
        assert_ne!(
            dispatch_scalar_argument_id(0),
            dispatch_scalar_integer_id(0)
        );

        let oversized = vec![field; MAX_DISPATCH_FIELD_VARIANTS + 1];
        assert_eq!(
            dispatch_field_projection_id(
                &oversized,
                &mut DispatchCanonicalizationBudget::default(),
            ),
            Err(DispatchBddError::FieldVariantLimit),
        );
    }

    #[test]
    fn dispatch_bdd_prospectively_accounts_and_releases_retained_and_work_slots() {
        let ledger = Rc::new(ProofRetentionLedger::default());
        let mut bdd =
            DispatchPredicateBdd::new(Rc::clone(&ledger)).expect("initial dispatch BDD retention");
        assert_eq!(ledger.totals.get().slots, bdd.retained_slots());

        let atom = DispatchPredicateAtom {
            operator: DispatchComparisonOperator::Equal,
            left: dispatch_scalar_argument_id(0),
            right: dispatch_scalar_integer_id(0),
        };
        let node = bdd.atom(atom).expect("one dispatch atom");
        assert_eq!(ledger.totals.get().slots, bdd.retained_slots());
        let negated = bdd.negate(node).expect("bounded BDD negation");
        assert_ne!(node, negated);
        // Transient work-frame leases are gone when the operation returns.
        assert_eq!(ledger.totals.get().slots, bdd.retained_slots());

        drop(bdd);
        assert_eq!(ledger.totals.get(), ProofRetentionTotals::default());
    }

    #[test]
    fn dispatch_bdd_refuses_before_unleased_storage_or_work_growth() {
        let ledger = Rc::new(ProofRetentionLedger::default());
        let mut bdd =
            DispatchPredicateBdd::new(Rc::clone(&ledger)).expect("initial dispatch BDD retention");
        let atom = DispatchPredicateAtom {
            operator: DispatchComparisonOperator::Less,
            left: dispatch_scalar_argument_id(0),
            right: dispatch_scalar_integer_id(10),
        };
        let node = bdd.atom(atom).expect("one retained dispatch atom");
        let retained_before = bdd.retained_slots();

        let fill = MAX_RETAINED_PROOF_SLOTS - ledger.totals.get().slots;
        assert!(ledger.try_reserve(0, 0, fill, 0, 0));
        assert_eq!(bdd.negate(node), Err(DispatchBddError::RetentionLimit),);
        assert_eq!(bdd.retained_slots(), retained_before);
        assert_eq!(ledger.totals.get().slots, MAX_RETAINED_PROOF_SLOTS);
        ledger.release(0, 0, fill, 0, 0);

        let fill = MAX_RETAINED_PROOF_SLOTS - ledger.totals.get().slots - 1;
        assert!(ledger.try_reserve(0, 0, fill, 0, 0));
        let different_atom = DispatchPredicateAtom {
            operator: DispatchComparisonOperator::LessOrEqual,
            left: dispatch_scalar_argument_id(0),
            right: dispatch_scalar_integer_id(11),
        };
        assert_eq!(
            bdd.atom(different_atom),
            Err(DispatchBddError::RetentionLimit),
        );
        assert_eq!(bdd.retained_slots(), retained_before);
        assert_eq!(ledger.totals.get().slots, MAX_RETAINED_PROOF_SLOTS - 1);

        drop(bdd);
        ledger.release(0, 0, fill, 0, 0);
        assert_eq!(ledger.totals.get(), ProofRetentionTotals::default());
    }

    #[test]
    fn declared_callable_result_type_is_rechecked_on_the_reachable_endpoint_path() {
        let source = r#"
> endpoint_bad_result(state: Int, context: Unit) -> Int {
    "not an Int"
}

? explore endpoint_bad_result_contract {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find all
    mechanisms paths for selected from endpoint_bad_result
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse result-conformance fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "ordinary source checking should leave endpoint authorization to the checked proof: {:?}",
            artifacts.diagnostics,
        );
        let issue = match artifacts.checked_exploration_query(0) {
            Err(CheckedExploreQueryAccessError::Producer(
                CheckedExploreQueryArtifactIssue::EndpointTotality(issue),
            )) => issue,
            Err(other) => panic!("unexpected checked-query rejection: {other:?}"),
            Ok(_) => panic!("a mismatched endpoint return must not mint a certificate"),
        };
        assert_eq!(
            issue.reason(),
            RelationalEndpointTotalityIssueReason::CheckedResolutionUnavailable,
        );
        assert!(issue.detail().contains("result of endpoint helper"));
    }
}
