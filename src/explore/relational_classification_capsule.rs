//! Canonical semantic identity for relational Explore classification.
//!
//! A classification graph is program semantics: it can be shared by several
//! bounded requests over the same checked FROM/TO/WHERE/FIND program. A
//! capsule binds that graph to one exact support/provenance boundary. Runtime
//! cache shape, scheduling and result materialization are deliberately absent
//! from both identities.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionId, QuestionId, RelationId};
use super::relational_support_planner::RelationalSupportPlanRoot;
use super::support_cell::SupportCellId;

pub(crate) const CLASSIFICATION_GRAPH_VERSION: u32 = 1;
pub(crate) const CLASSIFICATION_CAPSULE_VERSION: u32 = 1;

const TYPE_ID_V1: &[u8] = b"futuruna.explore.classification-type-id.v1\0";
const CALLABLE_ID_V1: &[u8] = b"futuruna.explore.classification-callable-id.v1\0";
const NODE_ID_V1: &[u8] = b"futuruna.explore.classification-node-id.v1\0";
const GRAPH_ROOT_V1: &[u8] = b"futuruna.explore.classification-graph-root.v1\0";
const CAPSULE_ID_V1: &[u8] = b"futuruna.explore.classification-capsule-id.v1\0";
const NO_SPECIALIZATION_V1: &[u8] = b"futuruna.explore.classification-no-specialization.v1\0";
const EXACT_SPECIALIZATION_V1: &[u8] = b"futuruna.explore.classification-exact-specialization.v1\0";
const PROVENANCE_ROOT_V1: &[u8] = b"futuruna.explore.classification-provenance-root.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationTypeId([u8; 32]);

impl ClassificationTypeId {
    /// Bind the capsule type vocabulary to a producer-owned checked type
    /// identity. Authored type spellings are not accepted at this boundary.
    pub(crate) fn from_checked_type_digest(digest: [u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(TYPE_ID_V1);
        hasher.update(digest);
        Self(hasher.finalize().into())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationNodeId([u8; 32]);

impl ClassificationNodeId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact checked callable semantics, independent of its authored spelling or
/// source occurrence address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationCallableId([u8; 32]);

impl ClassificationCallableId {
    pub(crate) fn from_checked_callable_digest(digest: [u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(CALLABLE_ID_V1);
        hasher.update(digest);
        Self(hasher.finalize().into())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationGraphRoot([u8; 32]);

impl ClassificationGraphRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationCapsuleId([u8; 32]);

impl ClassificationCapsuleId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// The two parameters of a reusable endpoint-observation graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationInputLane {
    Context,
    State,
}

impl ClassificationInputLane {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Context => 0x01,
            Self::State => 0x02,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationInputSlot {
    pub(crate) lane: ClassificationInputLane,
    pub(crate) ordinal: u32,
}

/// Producer-owned canonical encoding of one checked program constant. The
/// lowerer supplies the same collision-free value preimage used by exact
/// runtime identity; callers cannot construct an arbitrary tuple field.
/// Request-fixed source values remain graph inputs and belong in the capsule's
/// exact specialization root, not in this node.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationConstant(Box<[u8]>);

impl ClassificationConstant {
    pub(crate) fn from_canonical_checked_value_bytes(bytes: Box<[u8]>) -> Self {
        Self(bytes)
    }

    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationUnaryOp {
    BooleanNot,
    IntegerNegateChecked,
}

impl ClassificationUnaryOp {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::BooleanNot => 0x01,
            Self::IntegerNegateChecked => 0x02,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationBinaryOp {
    IntegerAddChecked,
    IntegerSubtractChecked,
    IntegerMultiplyChecked,
    IntegerDivideChecked,
    IntegerRemainderChecked,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    BooleanAndShortCircuit,
    BooleanOrShortCircuit,
}

impl ClassificationBinaryOp {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::IntegerAddChecked => 0x01,
            Self::IntegerSubtractChecked => 0x02,
            Self::IntegerMultiplyChecked => 0x03,
            Self::IntegerDivideChecked => 0x04,
            Self::IntegerRemainderChecked => 0x05,
            Self::Equal => 0x06,
            Self::NotEqual => 0x07,
            Self::LessThan => 0x08,
            Self::LessThanOrEqual => 0x09,
            Self::GreaterThan => 0x0a,
            Self::GreaterThanOrEqual => 0x0b,
            Self::BooleanAndShortCircuit => 0x0c,
            Self::BooleanOrShortCircuit => 0x0d,
        }
    }
}

/// Canonical, checked node vocabulary for the strict total/pure V1 subset.
///
/// Owner/callable/pattern digests come from the checked producer. Local
/// binders use ordinal parameters; names, spans and AST addresses never enter
/// this value.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationNodeKind {
    Constant(ClassificationConstant),
    Input(ClassificationInputSlot),
    Parameter(u32),
    Construct {
        constructor_id: [u8; 32],
        fields: Box<[ClassificationNodeId]>,
    },
    Project {
        owner_id: [u8; 32],
        variant_ordinal: u32,
        field_ordinal: u32,
        base: ClassificationNodeId,
    },
    Unary {
        op: ClassificationUnaryOp,
        operand: ClassificationNodeId,
    },
    Binary {
        op: ClassificationBinaryOp,
        left: ClassificationNodeId,
        right: ClassificationNodeId,
    },
    If {
        condition: ClassificationNodeId,
        then_node: ClassificationNodeId,
        else_node: ClassificationNodeId,
    },
    Match {
        scrutinee: ClassificationNodeId,
        /// Each checked pattern digest commits constructor/literal/binder
        /// semantics; authored pattern text is not an identity input.
        arms: Box<[([u8; 32], ClassificationNodeId)]>,
    },
    Call {
        callable_id: ClassificationCallableId,
        arguments: Box<[ClassificationNodeId]>,
    },
}

impl ClassificationNodeKind {
    fn child_ids(&self) -> Box<[ClassificationNodeId]> {
        match self {
            Self::Constant(_) | Self::Input(_) | Self::Parameter(_) => Box::new([]),
            Self::Construct { fields, .. } => fields.clone(),
            Self::Project { base, .. } => Box::new([*base]),
            Self::Unary { operand, .. } => Box::new([*operand]),
            Self::Binary { left, right, .. } => Box::new([*left, *right]),
            Self::If {
                condition,
                then_node,
                else_node,
            } => Box::new([*condition, *then_node, *else_node]),
            Self::Match { scrutinee, arms } => std::iter::once(*scrutinee)
                .chain(arms.iter().map(|(_, node)| *node))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Self::Call { arguments, .. } => arguments.clone(),
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        match self {
            Self::Constant(value) => {
                hasher.update([0x01]);
                hash_bytes(hasher, value.bytes());
            }
            Self::Input(slot) => {
                hasher.update([0x02, slot.lane.canonical_tag()]);
                hasher.update(slot.ordinal.to_le_bytes());
            }
            Self::Parameter(ordinal) => {
                hasher.update([0x03]);
                hasher.update(ordinal.to_le_bytes());
            }
            Self::Construct {
                constructor_id,
                fields,
            } => {
                hasher.update([0x04]);
                hasher.update(constructor_id);
                hash_node_ids(hasher, fields);
            }
            Self::Project {
                owner_id,
                variant_ordinal,
                field_ordinal,
                base,
            } => {
                hasher.update([0x05]);
                hasher.update(owner_id);
                hasher.update(variant_ordinal.to_le_bytes());
                hasher.update(field_ordinal.to_le_bytes());
                hasher.update(base.bytes());
            }
            Self::Unary { op, operand } => {
                hasher.update([0x06, op.canonical_tag()]);
                hasher.update(operand.bytes());
            }
            Self::Binary { op, left, right } => {
                hasher.update([0x07, op.canonical_tag()]);
                hasher.update(left.bytes());
                hasher.update(right.bytes());
            }
            Self::If {
                condition,
                then_node,
                else_node,
            } => {
                hasher.update([0x08]);
                hasher.update(condition.bytes());
                hasher.update(then_node.bytes());
                hasher.update(else_node.bytes());
            }
            Self::Match { scrutinee, arms } => {
                hasher.update([0x09]);
                hasher.update(scrutinee.bytes());
                hash_len(hasher, arms.len());
                for (pattern_digest, node) in arms.iter() {
                    hasher.update(pattern_digest);
                    hasher.update(node.bytes());
                }
            }
            Self::Call {
                callable_id,
                arguments,
            } => {
                hasher.update([0x0a]);
                hasher.update(callable_id.bytes());
                hash_node_ids(hasher, arguments);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationNodeKey {
    pub(crate) ty: ClassificationTypeId,
    pub(crate) kind: ClassificationNodeKind,
}

impl ClassificationNodeKey {
    fn derive_id(&self) -> ClassificationNodeId {
        let mut hasher = Sha256::new();
        hasher.update(NODE_ID_V1);
        hasher.update(CLASSIFICATION_GRAPH_VERSION.to_le_bytes());
        hasher.update(self.ty.bytes());
        self.kind.hash_into(&mut hasher);
        ClassificationNodeId(hasher.finalize().into())
    }
}

/// Collision-checking Merkle interner. Full keys remain available until the
/// frozen program is complete; a digest is never treated as equality alone.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClassificationInterner {
    nodes: BTreeMap<ClassificationNodeId, ClassificationNodeKey>,
}

impl ClassificationInterner {
    pub(crate) fn intern(
        &mut self,
        key: ClassificationNodeKey,
    ) -> Result<ClassificationNodeId, RelationalClassificationCapsuleError> {
        let id = key.derive_id();
        self.intern_with_id(id, key)
    }

    fn intern_with_id(
        &mut self,
        id: ClassificationNodeId,
        key: ClassificationNodeKey,
    ) -> Result<ClassificationNodeId, RelationalClassificationCapsuleError> {
        match self.nodes.entry(id) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(key);
                Ok(id)
            }
            std::collections::btree_map::Entry::Occupied(slot) if slot.get() == &key => Ok(id),
            std::collections::btree_map::Entry::Occupied(_) => Err(
                RelationalClassificationCapsuleError::NodeDigestCollision(id),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationSemanticLane {
    /// Parameterized source-construction function. Exact request-fixed values
    /// remain parameters even when the support plan proves them singleton.
    SourceBinding(u32),
    Successor,
    Admission {
        ordinal: u32,
        scope: ClassificationAdmissionScope,
    },
    Find,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationAdmissionScope {
    Before,
    After,
    Transition,
}

impl ClassificationAdmissionScope {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Before => 0x01,
            Self::After => 0x02,
            Self::Transition => 0x03,
        }
    }
}

impl ClassificationSemanticLane {
    fn hash_into(self, hasher: &mut Sha256) {
        match self {
            Self::SourceBinding(ordinal) => {
                hasher.update([0x01]);
                hasher.update(ordinal.to_le_bytes());
            }
            Self::Successor => hasher.update([0x02]),
            Self::Admission { ordinal, scope } => {
                hasher.update([0x03, scope.canonical_tag()]);
                hasher.update(ordinal.to_le_bytes());
            }
            Self::Find => hasher.update([0x04]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationLaneRoot {
    pub(crate) lane: ClassificationSemanticLane,
    pub(crate) node: ClassificationNodeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationResidualReason {
    RecursiveCall,
    EffectfulExpression,
    DynamicDispatch,
    HigherOrderCall,
    CollectionTraversal,
    OpenCapture,
    UnresolvedMember,
    UncertainArithmetic,
    UnsupportedType,
    UnsupportedExpression,
}

/// Typed semantic dependency retained by a concrete-fallback residual.
/// Node dependencies participate in the frozen reachable closure; external
/// dependencies live in a separate checked-semantic namespace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationResidualDependency {
    Node(ClassificationNodeId),
    Callable(ClassificationCallableId),
    DeclaredType([u8; 32]),
    Field([u8; 32]),
    RuleFamily([u8; 32]),
    Capture([u8; 32]),
    TopLevelConstant([u8; 32]),
    Constructor([u8; 32]),
}

impl ClassificationResidualDependency {
    fn hash_into(self, hasher: &mut Sha256) {
        match self {
            Self::Node(node_id) => {
                hasher.update([0x01]);
                hasher.update(node_id.bytes());
            }
            Self::Callable(digest) => {
                hasher.update([0x02]);
                hasher.update(digest.bytes());
            }
            Self::DeclaredType(digest) => {
                hasher.update([0x03]);
                hasher.update(digest);
            }
            Self::Field(digest) => {
                hasher.update([0x04]);
                hasher.update(digest);
            }
            Self::RuleFamily(digest) => {
                hasher.update([0x05]);
                hasher.update(digest);
            }
            Self::Capture(digest) => {
                hasher.update([0x06]);
                hasher.update(digest);
            }
            Self::TopLevelConstant(digest) => {
                hasher.update([0x07]);
                hasher.update(digest);
            }
            Self::Constructor(digest) => {
                hasher.update([0x08]);
                hasher.update(digest);
            }
        }
    }
}

impl ClassificationResidualReason {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::RecursiveCall => 0x01,
            Self::EffectfulExpression => 0x02,
            Self::DynamicDispatch => 0x03,
            Self::HigherOrderCall => 0x04,
            Self::CollectionTraversal => 0x05,
            Self::OpenCapture => 0x06,
            Self::UnresolvedMember => 0x07,
            Self::UncertainArithmetic => 0x08,
            Self::UnsupportedType => 0x09,
            Self::UnsupportedExpression => 0x0a,
        }
    }
}

/// Explicit concrete-fallback boundary. `site_digest` and dependency digests
/// are producer-owned semantic identities, never source positions or names.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationResidual {
    pub(crate) reason: ClassificationResidualReason,
    pub(crate) lane: ClassificationSemanticLane,
    pub(crate) site_digest: [u8; 32],
    pub(crate) dependencies: Box<[ClassificationResidualDependency]>,
}

impl ClassificationResidual {
    pub(crate) fn new(
        reason: ClassificationResidualReason,
        lane: ClassificationSemanticLane,
        site_digest: [u8; 32],
        dependencies: impl IntoIterator<Item = ClassificationResidualDependency>,
    ) -> Self {
        let mut dependencies = dependencies.into_iter().collect::<Vec<_>>();
        dependencies.sort_unstable();
        dependencies.dedup();
        Self {
            reason,
            lane,
            site_digest,
            dependencies: dependencies.into_boxed_slice(),
        }
    }

    fn node_dependencies(&self) -> impl Iterator<Item = ClassificationNodeId> + '_ {
        self.dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                ClassificationResidualDependency::Node(node_id) => Some(*node_id),
                ClassificationResidualDependency::Callable(_)
                | ClassificationResidualDependency::DeclaredType(_)
                | ClassificationResidualDependency::Field(_)
                | ClassificationResidualDependency::RuleFamily(_)
                | ClassificationResidualDependency::Capture(_)
                | ClassificationResidualDependency::TopLevelConstant(_)
                | ClassificationResidualDependency::Constructor(_) => None,
            })
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update([self.reason.canonical_tag()]);
        self.lane.hash_into(hasher);
        hasher.update(self.site_digest);
        hash_len(hasher, self.dependencies.len());
        for dependency in self.dependencies.iter().copied() {
            dependency.hash_into(hasher);
        }
    }
}

/// Immutable canonical graph for one checked classification program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenClassificationProgram {
    version: u32,
    nodes: Box<[(ClassificationNodeId, ClassificationNodeKey)]>,
    roots: Box<[ClassificationLaneRoot]>,
    residuals: Box<[ClassificationResidual]>,
    graph_root: ClassificationGraphRoot,
}

impl FrozenClassificationProgram {
    pub(crate) fn freeze(
        interner: ClassificationInterner,
        roots: impl IntoIterator<Item = ClassificationLaneRoot>,
        residuals: impl IntoIterator<Item = ClassificationResidual>,
    ) -> Result<Self, RelationalClassificationCapsuleError> {
        let mut roots = roots.into_iter().collect::<Vec<_>>();
        roots.sort_unstable();
        if roots.windows(2).any(|pair| pair[0].lane == pair[1].lane) {
            return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
        }
        if let Some(ordinal) = duplicate_admission_ordinal(&roots) {
            return Err(RelationalClassificationCapsuleError::DuplicateAdmissionOrdinal(ordinal));
        }

        let mut residuals = residuals.into_iter().collect::<Vec<_>>();
        residuals.sort_unstable();
        residuals.dedup();

        let mut reachable = BTreeSet::new();
        let mut pending = roots
            .iter()
            .map(|root| root.node)
            .chain(
                residuals
                    .iter()
                    .flat_map(ClassificationResidual::node_dependencies),
            )
            .collect::<Vec<_>>();
        while let Some(node_id) = pending.pop() {
            if !reachable.insert(node_id) {
                continue;
            }
            let node = interner
                .nodes
                .get(&node_id)
                .ok_or(RelationalClassificationCapsuleError::UnresolvedNodeReference(node_id))?;
            pending.extend(node.kind.child_ids());
        }
        let nodes = reachable
            .into_iter()
            .map(|id| {
                interner
                    .nodes
                    .get(&id)
                    .cloned()
                    .map(|key| (id, key))
                    .ok_or(RelationalClassificationCapsuleError::UnresolvedNodeReference(id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let graph_root = derive_graph_root(&roots, &residuals);
        Ok(Self {
            version: CLASSIFICATION_GRAPH_VERSION,
            nodes: nodes.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            residuals: residuals.into_boxed_slice(),
            graph_root,
        })
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn graph_root(&self) -> ClassificationGraphRoot {
        self.graph_root
    }

    pub(crate) fn nodes(&self) -> &[(ClassificationNodeId, ClassificationNodeKey)] {
        &self.nodes
    }

    pub(crate) fn roots(&self) -> &[ClassificationLaneRoot] {
        &self.roots
    }

    pub(crate) fn residuals(&self) -> &[ClassificationResidual] {
        &self.residuals
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.residuals.is_empty()
    }

    pub(crate) fn validate_identity(&self) -> bool {
        if self.version != CLASSIFICATION_GRAPH_VERSION
            || !self.nodes.windows(2).all(|pair| pair[0].0 < pair[1].0)
            || !self.nodes.iter().all(|(id, key)| *id == key.derive_id())
            || !self
                .roots
                .windows(2)
                .all(|pair| pair[0].lane < pair[1].lane)
            || duplicate_admission_ordinal(&self.roots).is_some()
            || !self.residuals.windows(2).all(|pair| pair[0] < pair[1])
            || self.graph_root != derive_graph_root(&self.roots, &self.residuals)
        {
            return false;
        }

        let nodes = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, node))
            .collect::<BTreeMap<_, _>>();
        let mut reachable = BTreeSet::new();
        let mut pending = self
            .roots
            .iter()
            .map(|root| root.node)
            .chain(
                self.residuals
                    .iter()
                    .flat_map(ClassificationResidual::node_dependencies),
            )
            .collect::<Vec<_>>();
        while let Some(node_id) = pending.pop() {
            if !reachable.insert(node_id) {
                continue;
            }
            let Some(node) = nodes.get(&node_id) else {
                return false;
            };
            pending.extend(node.kind.child_ids());
        }
        reachable.len() == self.nodes.len()
    }
}

fn duplicate_admission_ordinal(roots: &[ClassificationLaneRoot]) -> Option<u32> {
    let mut ordinals = BTreeSet::new();
    roots.iter().find_map(|root| {
        let ClassificationSemanticLane::Admission { ordinal, .. } = root.lane else {
            return None;
        };
        (!ordinals.insert(ordinal)).then_some(ordinal)
    })
}

fn derive_graph_root(
    roots: &[ClassificationLaneRoot],
    residuals: &[ClassificationResidual],
) -> ClassificationGraphRoot {
    let mut hasher = Sha256::new();
    hasher.update(GRAPH_ROOT_V1);
    hasher.update(CLASSIFICATION_GRAPH_VERSION.to_le_bytes());
    hash_len(&mut hasher, roots.len());
    for root in roots {
        root.lane.hash_into(&mut hasher);
        hasher.update(root.node.bytes());
    }
    hash_len(&mut hasher, residuals.len());
    for residual in residuals {
        residual.hash_into(&mut hasher);
    }
    ClassificationGraphRoot(hasher.finalize().into())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationSpecializationRoot([u8; 32]);

impl ClassificationSpecializationRoot {
    /// The default is an unspecialized parameterized graph, not an unknown or
    /// sampled specialization.
    pub(crate) fn none() -> Self {
        Self(Sha256::digest(NO_SPECIALIZATION_V1).into())
    }

    /// Only compiler-checked exact singleton/support evidence may call this
    /// constructor. Runtime sample values are not specialization authority.
    pub(crate) fn from_exact_witness_digest(witness: [u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(EXACT_SPECIALIZATION_V1);
        hasher.update(witness);
        Self(hasher.finalize().into())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationProvenanceRoot([u8; 32]);

impl ClassificationProvenanceRoot {
    pub(crate) fn from_checked_source_coverage_digest(digest: [u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(PROVENANCE_ROOT_V1);
        hasher.update(digest);
        Self(hasher.finalize().into())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Request/support-bound replay capsule around one reusable semantic graph.
#[derive(Clone, Debug)]
pub(crate) struct RelationalClassificationCapsule {
    id: ClassificationCapsuleId,
    graph: Arc<FrozenClassificationProgram>,
    checked_program: [u8; 32],
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan_root: RelationalSupportPlanRoot,
    root_cell_id: Option<SupportCellId>,
    specialization_root: ClassificationSpecializationRoot,
    provenance_root: ClassificationProvenanceRoot,
}

impl RelationalClassificationCapsule {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind(
        graph: Arc<FrozenClassificationProgram>,
        checked_program: [u8; 32],
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        support_plan_root: RelationalSupportPlanRoot,
        root_cell_id: Option<SupportCellId>,
        specialization_root: ClassificationSpecializationRoot,
        provenance_root: ClassificationProvenanceRoot,
    ) -> Result<Self, RelationalClassificationCapsuleError> {
        if !graph.validate_identity() {
            return Err(RelationalClassificationCapsuleError::InvalidGraphIdentity);
        }
        let id = derive_capsule_id(
            graph.graph_root(),
            checked_program,
            relation_id,
            admission_id,
            question_id,
            support_plan_root,
            root_cell_id,
            specialization_root,
            provenance_root,
        );
        Ok(Self {
            id,
            graph,
            checked_program,
            relation_id,
            admission_id,
            question_id,
            support_plan_root,
            root_cell_id,
            specialization_root,
            provenance_root,
        })
    }

    pub(crate) const fn id(&self) -> ClassificationCapsuleId {
        self.id
    }

    pub(crate) fn graph_root(&self) -> ClassificationGraphRoot {
        self.graph.graph_root()
    }

    pub(crate) fn graph(&self) -> &FrozenClassificationProgram {
        self.graph.as_ref()
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn root_cell_id(&self) -> Option<SupportCellId> {
        self.root_cell_id
    }

    pub(crate) fn validate_identity(&self) -> bool {
        self.graph.validate_identity()
            && self.id
                == derive_capsule_id(
                    self.graph.graph_root(),
                    self.checked_program,
                    self.relation_id,
                    self.admission_id,
                    self.question_id,
                    self.support_plan_root,
                    self.root_cell_id,
                    self.specialization_root,
                    self.provenance_root,
                )
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_capsule_id(
    graph_root: ClassificationGraphRoot,
    checked_program: [u8; 32],
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan_root: RelationalSupportPlanRoot,
    root_cell_id: Option<SupportCellId>,
    specialization_root: ClassificationSpecializationRoot,
    provenance_root: ClassificationProvenanceRoot,
) -> ClassificationCapsuleId {
    let mut hasher = Sha256::new();
    hasher.update(CAPSULE_ID_V1);
    hasher.update(CLASSIFICATION_CAPSULE_VERSION.to_le_bytes());
    hasher.update(graph_root.bytes());
    hasher.update(checked_program);
    hasher.update(relation_id.bytes());
    hasher.update(admission_id.bytes());
    hasher.update(question_id.bytes());
    hasher.update(support_plan_root.bytes());
    match root_cell_id {
        Some(root_cell_id) => {
            hasher.update([0x01]);
            hasher.update(root_cell_id.bytes());
        }
        None => hasher.update([0x00]),
    }
    hasher.update(specialization_root.bytes());
    hasher.update(provenance_root.bytes());
    ClassificationCapsuleId(hasher.finalize().into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassificationCapsuleError {
    NodeDigestCollision(ClassificationNodeId),
    UnresolvedNodeReference(ClassificationNodeId),
    DuplicateSemanticLane,
    DuplicateAdmissionOrdinal(u32),
    InvalidGraphIdentity,
}

impl fmt::Display for RelationalClassificationCapsuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeDigestCollision(node_id) => write!(
                formatter,
                "classification node digest collision at {}",
                lowercase_hex(node_id.bytes())
            ),
            Self::UnresolvedNodeReference(node_id) => write!(
                formatter,
                "classification graph references absent node {}",
                lowercase_hex(node_id.bytes())
            ),
            Self::DuplicateSemanticLane => {
                formatter.write_str("classification graph has duplicate semantic lane roots")
            }
            Self::DuplicateAdmissionOrdinal(ordinal) => write!(
                formatter,
                "classification graph repeats admission ordinal {ordinal} across semantic lanes"
            ),
            Self::InvalidGraphIdentity => {
                formatter.write_str("classification graph identity is invalid")
            }
        }
    }
}

impl Error for RelationalClassificationCapsuleError {}

fn hash_len(hasher: &mut Sha256, len: usize) {
    hasher.update((len as u64).to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_len(hasher, bytes.len());
    hasher.update(bytes);
}

fn hash_node_ids(hasher: &mut Sha256, ids: &[ClassificationNodeId]) {
    hash_len(hasher, ids.len());
    for id in ids {
        hasher.update(id.bytes());
    }
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::super::relation::FindPolarity;
    use super::*;

    fn type_id(tag: u8) -> ClassificationTypeId {
        ClassificationTypeId::from_checked_type_digest([tag; 32])
    }

    fn constant(tag: u8) -> ClassificationNodeKind {
        ClassificationNodeKind::Constant(
            ClassificationConstant::from_canonical_checked_value_bytes(Box::new([tag])),
        )
    }

    fn frozen_program(op: ClassificationBinaryOp) -> FrozenClassificationProgram {
        let mut interner = ClassificationInterner::default();
        let state = interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: ClassificationNodeKind::Input(ClassificationInputSlot {
                    lane: ClassificationInputLane::State,
                    ordinal: 0,
                }),
            })
            .unwrap();
        let zero = interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: constant(0),
            })
            .unwrap();
        let predicate = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Binary {
                    op,
                    left: state,
                    right: zero,
                },
            })
            .unwrap();
        FrozenClassificationProgram::freeze(
            interner,
            [ClassificationLaneRoot {
                lane: ClassificationSemanticLane::Find,
                node: predicate,
            }],
            [],
        )
        .unwrap()
    }

    #[test]
    fn canonical_graph_is_stable_and_discards_unreachable_nodes() {
        let left = frozen_program(ClassificationBinaryOp::LessThan);
        let mut right_interner = ClassificationInterner::default();
        let _unreachable = right_interner
            .intern(ClassificationNodeKey {
                ty: type_id(9),
                kind: constant(99),
            })
            .unwrap();
        let state = right_interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: ClassificationNodeKind::Input(ClassificationInputSlot {
                    lane: ClassificationInputLane::State,
                    ordinal: 0,
                }),
            })
            .unwrap();
        let zero = right_interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: constant(0),
            })
            .unwrap();
        let predicate = right_interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Binary {
                    op: ClassificationBinaryOp::LessThan,
                    left: state,
                    right: zero,
                },
            })
            .unwrap();
        let right = FrozenClassificationProgram::freeze(
            right_interner,
            [ClassificationLaneRoot {
                lane: ClassificationSemanticLane::Find,
                node: predicate,
            }],
            [],
        )
        .unwrap();

        assert_eq!(left.graph_root(), right.graph_root());
        assert_eq!(left.nodes(), right.nodes());
        assert_eq!(right.nodes().len(), 3);
        assert!(right.validate_identity());
    }

    #[test]
    fn semantic_operation_changes_graph_identity() {
        assert_ne!(
            frozen_program(ClassificationBinaryOp::LessThan).graph_root(),
            frozen_program(ClassificationBinaryOp::GreaterThan).graph_root(),
        );
    }

    fn frozen_admission(scope: ClassificationAdmissionScope) -> FrozenClassificationProgram {
        let mut interner = ClassificationInterner::default();
        let predicate = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: constant(1),
            })
            .unwrap();
        FrozenClassificationProgram::freeze(
            interner,
            [ClassificationLaneRoot {
                lane: ClassificationSemanticLane::Admission { ordinal: 0, scope },
                node: predicate,
            }],
            [],
        )
        .unwrap()
    }

    #[test]
    fn admission_scope_changes_graph_identity() {
        let before = frozen_admission(ClassificationAdmissionScope::Before).graph_root();
        let after = frozen_admission(ClassificationAdmissionScope::After).graph_root();
        let transition = frozen_admission(ClassificationAdmissionScope::Transition).graph_root();
        assert_ne!(before, after);
        assert_ne!(before, transition);
        assert_ne!(after, transition);
    }

    #[test]
    fn one_checked_admission_ordinal_cannot_appear_under_two_scopes() {
        let mut interner = ClassificationInterner::default();
        let predicate = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: constant(1),
            })
            .unwrap();
        assert_eq!(
            FrozenClassificationProgram::freeze(
                interner,
                [
                    ClassificationLaneRoot {
                        lane: ClassificationSemanticLane::Admission {
                            ordinal: 0,
                            scope: ClassificationAdmissionScope::Before,
                        },
                        node: predicate,
                    },
                    ClassificationLaneRoot {
                        lane: ClassificationSemanticLane::Admission {
                            ordinal: 0,
                            scope: ClassificationAdmissionScope::After,
                        },
                        node: predicate,
                    },
                ],
                [],
            ),
            Err(RelationalClassificationCapsuleError::DuplicateAdmissionOrdinal(0))
        );
    }

    #[test]
    fn residual_manifest_is_canonical_and_changes_graph_identity() {
        let base = frozen_program(ClassificationBinaryOp::LessThan);
        let root = base.roots()[0];
        let nodes = base.nodes().iter().cloned().collect::<BTreeMap<_, _>>();
        let residual = ClassificationResidual::new(
            ClassificationResidualReason::DynamicDispatch,
            ClassificationSemanticLane::Find,
            [7; 32],
            [
                ClassificationResidualDependency::RuleFamily([3; 32]),
                ClassificationResidualDependency::RuleFamily([2; 32]),
                ClassificationResidualDependency::RuleFamily([3; 32]),
            ],
        );
        assert_eq!(
            residual.dependencies.as_ref(),
            &[
                ClassificationResidualDependency::RuleFamily([2; 32]),
                ClassificationResidualDependency::RuleFamily([3; 32]),
            ]
        );
        let with_residual = FrozenClassificationProgram::freeze(
            ClassificationInterner { nodes },
            [root],
            [residual],
        )
        .unwrap();
        assert_ne!(base.graph_root(), with_residual.graph_root());
        assert!(!with_residual.is_complete());

        let callable_residual = ClassificationResidual::new(
            ClassificationResidualReason::DynamicDispatch,
            ClassificationSemanticLane::Find,
            [7; 32],
            [ClassificationResidualDependency::Callable(
                ClassificationCallableId::from_checked_callable_digest([2; 32]),
            )],
        );
        let callable_graph = FrozenClassificationProgram::freeze(
            ClassificationInterner {
                nodes: base.nodes().iter().cloned().collect(),
            },
            [root],
            [callable_residual],
        )
        .unwrap();
        let rule_family_graph = FrozenClassificationProgram::freeze(
            ClassificationInterner {
                nodes: base.nodes().iter().cloned().collect(),
            },
            [root],
            [ClassificationResidual::new(
                ClassificationResidualReason::DynamicDispatch,
                ClassificationSemanticLane::Find,
                [7; 32],
                [ClassificationResidualDependency::RuleFamily([2; 32])],
            )],
        )
        .unwrap();
        assert_ne!(rule_family_graph.graph_root(), callable_graph.graph_root());
    }

    #[test]
    fn interner_rejects_a_full_key_mismatch_at_one_digest() {
        let mut interner = ClassificationInterner::default();
        let id = ClassificationNodeId([4; 32]);
        interner
            .intern_with_id(
                id,
                ClassificationNodeKey {
                    ty: type_id(1),
                    kind: constant(1),
                },
            )
            .unwrap();
        assert_eq!(
            interner.intern_with_id(
                id,
                ClassificationNodeKey {
                    ty: type_id(1),
                    kind: constant(2),
                },
            ),
            Err(RelationalClassificationCapsuleError::NodeDigestCollision(
                id
            ))
        );
    }

    #[derive(Clone, Copy)]
    struct CapsuleTags {
        checked_program: u8,
        relation: u8,
        admission: u8,
        question: u8,
        support_plan: u8,
        root_cell: Option<u8>,
        specialization: Option<u8>,
        provenance: u8,
    }

    impl Default for CapsuleTags {
        fn default() -> Self {
            Self {
                checked_program: 13,
                relation: 10,
                admission: 11,
                question: 12,
                support_plan: 1,
                root_cell: Some(14),
                specialization: None,
                provenance: 2,
            }
        }
    }

    fn capsule(
        graph: Arc<FrozenClassificationProgram>,
        tags: CapsuleTags,
    ) -> RelationalClassificationCapsule {
        let relation_id = RelationId::from_canonical_semantic_digest([tags.relation; 32]);
        let admission_id =
            AdmissionId::from_canonical_admission_digest(relation_id, [tags.admission; 32]);
        let question_id = QuestionId::from_canonical_find_digest(
            admission_id,
            [tags.question; 32],
            FindPolarity::Matches,
        );
        RelationalClassificationCapsule::bind(
            graph,
            [tags.checked_program; 32],
            relation_id,
            admission_id,
            question_id,
            RelationalSupportPlanRoot::from_journal_codec_bytes([tags.support_plan; 32]),
            tags.root_cell
                .map(|tag| SupportCellId::from_journal_codec_bytes([tag; 32])),
            tags.specialization
                .map_or_else(ClassificationSpecializationRoot::none, |tag| {
                    ClassificationSpecializationRoot::from_exact_witness_digest([tag; 32])
                }),
            ClassificationProvenanceRoot::from_checked_source_coverage_digest(
                [tags.provenance; 32],
            ),
        )
        .unwrap()
    }

    #[test]
    fn capsule_binding_does_not_rename_the_semantic_graph() {
        let graph = Arc::new(frozen_program(ClassificationBinaryOp::LessThan));
        let base = CapsuleTags::default();
        let first = capsule(Arc::clone(&graph), base);
        let independently_rebuilt = capsule(
            Arc::new(frozen_program(ClassificationBinaryOp::LessThan)),
            base,
        );
        let variants = [
            CapsuleTags {
                checked_program: 21,
                ..base
            },
            CapsuleTags {
                relation: 22,
                ..base
            },
            CapsuleTags {
                admission: 23,
                ..base
            },
            CapsuleTags {
                question: 24,
                ..base
            },
            CapsuleTags {
                support_plan: 25,
                ..base
            },
            CapsuleTags {
                root_cell: None,
                ..base
            },
            CapsuleTags {
                specialization: Some(26),
                ..base
            },
            CapsuleTags {
                provenance: 27,
                ..base
            },
        ];

        assert_eq!(first.graph_root(), independently_rebuilt.graph_root());
        assert_eq!(first.id(), independently_rebuilt.id());
        for variant in variants {
            let rebound = capsule(Arc::clone(&graph), variant);
            assert_eq!(first.graph_root(), rebound.graph_root());
            assert_ne!(first.id(), rebound.id());
        }
        assert!(first.validate_identity());
    }
}
