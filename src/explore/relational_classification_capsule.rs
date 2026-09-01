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
pub(crate) const CLASSIFICATION_RUNTIME_SHAPE_VERSION: u32 = 1;
pub(crate) const CLASSIFICATION_CAPSULE_VERSION: u32 = 1;

const TYPE_ID_V1: &[u8] = b"futuruna.explore.classification-type-id.v1\0";
const CALLABLE_ID_V1: &[u8] = b"futuruna.explore.classification-callable-id.v1\0";
const NODE_ID_V1: &[u8] = b"futuruna.explore.classification-node-id.v1\0";
const GRAPH_ROOT_V1: &[u8] = b"futuruna.explore.classification-graph-root.v1\0";
const RUNTIME_SHAPE_ROOT_V1: &[u8] = b"futuruna.explore.classification-runtime-shape-root.v1\0";
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
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

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

/// Identity of the checked runtime spelling/layout adapter for one graph.
///
/// This root is intentionally separate from [`ClassificationGraphRoot`]: the
/// graph remains name-free program semantics, while a capsule commits the
/// exact host representation needed to execute constructors and projections.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeShapeRoot([u8; 32]);

impl RuntimeShapeRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationRuntimeLayout {
    Positional,
    Named,
}

impl ClassificationRuntimeLayout {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Positional => 0x01,
            Self::Named => 0x02,
        }
    }

    pub(crate) const fn is_positional(self) -> bool {
        matches!(self, Self::Positional)
    }
}

/// Name-free semantic address of one declared constructor variant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeConstructorKey {
    pub(crate) owner_id: [u8; 32],
    pub(crate) variant_ordinal: u32,
}

/// Capsule-authenticated adapter from a checked constructor identity to the
/// current [`super::ExploreValue::Constructor`] representation.
///
/// Names are representation payload only. They never enter node or graph
/// identity, and evaluators may use them only after resolving this entry by a
/// semantic constructor/key digest.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeConstructorShape {
    pub(crate) owner_id: [u8; 32],
    pub(crate) variant_ordinal: u32,
    pub(crate) constructor_id: [u8; 32],
    pub(crate) type_name: Box<str>,
    pub(crate) variant_name: Box<str>,
    pub(crate) layout: ClassificationRuntimeLayout,
    pub(crate) field_names: Box<[Box<str>]>,
}

impl RuntimeConstructorShape {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        owner_id: [u8; 32],
        variant_ordinal: u32,
        constructor_id: [u8; 32],
        type_name: Box<str>,
        variant_name: Box<str>,
        layout: ClassificationRuntimeLayout,
        field_names: Box<[Box<str>]>,
    ) -> Self {
        Self {
            owner_id,
            variant_ordinal,
            constructor_id,
            type_name,
            variant_name,
            layout,
            field_names,
        }
    }

    pub(crate) const fn key(&self) -> RuntimeConstructorKey {
        RuntimeConstructorKey {
            owner_id: self.owner_id,
            variant_ordinal: self.variant_ordinal,
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update(self.owner_id);
        hasher.update(self.variant_ordinal.to_le_bytes());
        hasher.update(self.constructor_id);
        hash_bytes(hasher, self.type_name.as_bytes());
        hash_bytes(hasher, self.variant_name.as_bytes());
        hasher.update([self.layout.canonical_tag()]);
        hash_len(hasher, self.field_names.len());
        for field_name in self.field_names.iter() {
            hash_bytes(hasher, field_name.as_bytes());
        }
    }
}

/// Minimal checked runtime adapter retained beside, never inside, the
/// reusable name-free classification program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenClassificationRuntimeShapes {
    version: u32,
    shapes: Box<[RuntimeConstructorShape]>,
    runtime_shape_root: RuntimeShapeRoot,
}

impl FrozenClassificationRuntimeShapes {
    pub(crate) fn freeze(
        shapes: impl IntoIterator<Item = RuntimeConstructorShape>,
    ) -> Result<Self, RelationalClassificationCapsuleError> {
        let mut by_key = BTreeMap::<RuntimeConstructorKey, RuntimeConstructorShape>::new();
        let mut by_constructor = BTreeMap::<[u8; 32], RuntimeConstructorKey>::new();
        for shape in shapes {
            let key = shape.key();
            match by_key.entry(key) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(shape.clone());
                }
                std::collections::btree_map::Entry::Occupied(slot) if slot.get() == &shape => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(
                        RelationalClassificationCapsuleError::RuntimeShapeKeyCollision(key),
                    );
                }
            }
            match by_constructor.entry(shape.constructor_id) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(key);
                }
                std::collections::btree_map::Entry::Occupied(slot) if *slot.get() == key => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(
                        RelationalClassificationCapsuleError::RuntimeConstructorDigestCollision(
                            shape.constructor_id,
                        ),
                    );
                }
            }
        }
        let shapes = by_key.into_values().collect::<Vec<_>>();
        let runtime_shape_root = derive_runtime_shape_root(&shapes);
        Ok(Self {
            version: CLASSIFICATION_RUNTIME_SHAPE_VERSION,
            shapes: shapes.into_boxed_slice(),
            runtime_shape_root,
        })
    }

    pub(crate) const fn runtime_shape_root(&self) -> RuntimeShapeRoot {
        self.runtime_shape_root
    }

    pub(crate) fn shapes(&self) -> &[RuntimeConstructorShape] {
        &self.shapes
    }

    pub(crate) fn shape_for_variant(
        &self,
        key: RuntimeConstructorKey,
    ) -> Option<&RuntimeConstructorShape> {
        self.shapes
            .binary_search_by_key(&key, RuntimeConstructorShape::key)
            .ok()
            .map(|index| &self.shapes[index])
    }

    pub(crate) fn shape_for_constructor(
        &self,
        constructor_id: [u8; 32],
    ) -> Option<&RuntimeConstructorShape> {
        self.shapes
            .iter()
            .find(|shape| shape.constructor_id == constructor_id)
    }

    pub(crate) fn validate_identity(&self) -> bool {
        self.version == CLASSIFICATION_RUNTIME_SHAPE_VERSION
            && self
                .shapes
                .windows(2)
                .all(|pair| pair[0].key() < pair[1].key())
            && self
                .shapes
                .iter()
                .map(|shape| shape.constructor_id)
                .collect::<BTreeSet<_>>()
                .len()
                == self.shapes.len()
            && self.runtime_shape_root == derive_runtime_shape_root(&self.shapes)
    }

    /// Require a one-to-one, reachable-only adapter for every retained
    /// constructor operation in `graph`, including semantic node dependencies
    /// committed by a residual lane.
    pub(crate) fn validate_for_program(
        &self,
        graph: &FrozenClassificationProgram,
    ) -> Result<(), RelationalClassificationCapsuleError> {
        if !self.validate_identity() {
            return Err(RelationalClassificationCapsuleError::InvalidRuntimeShapeIdentity);
        }
        let mut used = BTreeSet::new();
        for (node_id, node) in graph.nodes() {
            match &node.kind {
                ClassificationNodeKind::Construct {
                    constructor_id,
                    fields,
                } => {
                    let shape = self.shape_for_constructor(*constructor_id).ok_or(
                        RelationalClassificationCapsuleError::MissingRuntimeConstructorShape(
                            *constructor_id,
                        ),
                    )?;
                    if fields.len() != shape.field_names.len() {
                        return Err(
                            RelationalClassificationCapsuleError::RuntimeConstructArityMismatch {
                                node_id: *node_id,
                                expected: shape.field_names.len(),
                                actual: fields.len(),
                            },
                        );
                    }
                    used.insert(shape.key());
                }
                ClassificationNodeKind::Project {
                    owner_id,
                    variant_ordinal,
                    field_ordinal,
                    ..
                } => {
                    let key = RuntimeConstructorKey {
                        owner_id: *owner_id,
                        variant_ordinal: *variant_ordinal,
                    };
                    let shape = self.shape_for_variant(key).ok_or(
                        RelationalClassificationCapsuleError::MissingRuntimeVariantShape(key),
                    )?;
                    let field_index = usize::try_from(*field_ordinal).map_err(|_| {
                        RelationalClassificationCapsuleError::RuntimeProjectionFieldOutOfBounds {
                            node_id: *node_id,
                            field_ordinal: *field_ordinal,
                            field_count: shape.field_names.len(),
                        }
                    })?;
                    if field_index >= shape.field_names.len() {
                        return Err(
                            RelationalClassificationCapsuleError::RuntimeProjectionFieldOutOfBounds {
                                node_id: *node_id,
                                field_ordinal: *field_ordinal,
                                field_count: shape.field_names.len(),
                            },
                        );
                    }
                    used.insert(key);
                }
                ClassificationNodeKind::IsVariant {
                    owner_id,
                    variant_ordinal,
                    ..
                } => {
                    let key = RuntimeConstructorKey {
                        owner_id: *owner_id,
                        variant_ordinal: *variant_ordinal,
                    };
                    if self.shape_for_variant(key).is_none() {
                        return Err(
                            RelationalClassificationCapsuleError::MissingRuntimeVariantShape(key),
                        );
                    }
                    used.insert(key);
                }
                ClassificationNodeKind::Constant(_)
                | ClassificationNodeKind::Input(_)
                | ClassificationNodeKind::SourceParameter(_)
                | ClassificationNodeKind::CallableParameter { .. }
                | ClassificationNodeKind::Unary { .. }
                | ClassificationNodeKind::Binary { .. }
                | ClassificationNodeKind::If { .. }
                | ClassificationNodeKind::Call { .. } => {}
            }
        }
        if let Some(unused) = self
            .shapes
            .iter()
            .map(RuntimeConstructorShape::key)
            .find(|key| !used.contains(key))
        {
            return Err(RelationalClassificationCapsuleError::UnusedRuntimeShape(
                unused,
            ));
        }
        Ok(())
    }
}

fn derive_runtime_shape_root(shapes: &[RuntimeConstructorShape]) -> RuntimeShapeRoot {
    let mut hasher = Sha256::new();
    hasher.update(RUNTIME_SHAPE_ROOT_V1);
    hasher.update(CLASSIFICATION_RUNTIME_SHAPE_VERSION.to_le_bytes());
    hash_len(&mut hasher, shapes.len());
    for shape in shapes {
        shape.hash_into(&mut hasher);
    }
    RuntimeShapeRoot(hasher.finalize().into())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationCapsuleId([u8; 32]);

impl ClassificationCapsuleId {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical whole-value inputs of a reusable transition-classification graph.
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

impl ClassificationInputSlot {
    /// Whole checked Context value at transition roots.
    pub(crate) const CONTEXT: Self = Self {
        lane: ClassificationInputLane::Context,
        ordinal: 0,
    };
    /// Whole checked Before value at transition roots.
    pub(crate) const BEFORE: Self = Self {
        lane: ClassificationInputLane::State,
        ordinal: 0,
    };
    /// Whole checked After value at transition roots.
    pub(crate) const AFTER: Self = Self {
        lane: ClassificationInputLane::State,
        ordinal: 1,
    };
}

/// Self-describing exact constant admitted by the executable V1 graph.
///
/// Request-fixed source values remain graph inputs and belong in the capsule's
/// exact specialization root. Floats and collection constants deliberately do
/// not enter this vocabulary: their equality/rounding or traversal semantics
/// require a later explicit proof and remain producer residuals for now.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationConstant {
    Integer(i64),
    Boolean(bool),
    Unit,
    Character(char),
    String(Box<str>),
}

impl ClassificationConstant {
    pub(crate) fn to_explore_value(&self) -> super::ExploreValue {
        match self {
            Self::Integer(value) => super::ExploreValue::Int(*value),
            Self::Boolean(value) => super::ExploreValue::Boolean(*value),
            Self::Unit => super::ExploreValue::Unit,
            Self::Character(value) => super::ExploreValue::Character(*value),
            Self::String(value) => super::ExploreValue::String(value.to_string()),
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        match self {
            Self::Integer(value) => {
                hasher.update([0x01]);
                hasher.update(value.to_le_bytes());
            }
            Self::Boolean(value) => hasher.update([0x02, u8::from(*value)]),
            Self::Unit => hasher.update([0x03]),
            Self::Character(value) => {
                hasher.update([0x04]);
                hasher.update(u32::from(*value).to_le_bytes());
            }
            Self::String(value) => {
                hasher.update([0x05]);
                hash_bytes(hasher, value.as_bytes());
            }
        }
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
    /// One exact finite/source binding. Its producer binding index, not its
    /// authored name or collection representation, is the semantic slot.
    SourceParameter(u32),
    /// Lexically owned pure-call parameter. Ownership prevents two unrelated
    /// callable bodies from aliasing merely because both use ordinal zero.
    CallableParameter {
        callable_id: ClassificationCallableId,
        ordinal: u32,
    },
    /// Reserved executable representation for a checked constructor. A
    /// producer may lower this only when the bound capsule also supplies the
    /// checked runtime shape needed to create the corresponding ExploreValue.
    Construct {
        constructor_id: [u8; 32],
        fields: Box<[ClassificationNodeId]>,
    },
    /// Reserved executable representation for a checked field projection. A
    /// producer may lower this only when the bound capsule can validate the
    /// runtime constructor owner, variant and field ordering exactly.
    Project {
        owner_id: [u8; 32],
        variant_ordinal: u32,
        field_ordinal: u32,
        base: ClassificationNodeId,
    },
    /// Exact checked constructor-tag test used by normalized match arms.
    /// Runtime spellings are resolved only through the capsule shape table.
    IsVariant {
        owner_id: [u8; 32],
        variant_ordinal: u32,
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
    /// Acyclic pure call. The frozen callable definition table binds the exact
    /// ordered signature and body root; recursive SCCs remain residuals.
    Call {
        callable_id: ClassificationCallableId,
        arguments: Box<[ClassificationNodeId]>,
    },
}

impl ClassificationNodeKind {
    fn child_ids(&self) -> Box<[ClassificationNodeId]> {
        match self {
            Self::Constant(_)
            | Self::Input(_)
            | Self::SourceParameter(_)
            | Self::CallableParameter { .. } => Box::new([]),
            Self::Construct { fields, .. } => fields.clone(),
            Self::Project { base, .. } | Self::IsVariant { base, .. } => Box::new([*base]),
            Self::Unary { operand, .. } => Box::new([*operand]),
            Self::Binary { left, right, .. } => Box::new([*left, *right]),
            Self::If {
                condition,
                then_node,
                else_node,
            } => Box::new([*condition, *then_node, *else_node]),
            Self::Call { arguments, .. } => arguments.clone(),
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        match self {
            Self::Constant(value) => {
                hasher.update([0x01]);
                value.hash_into(hasher);
            }
            Self::Input(slot) => {
                hasher.update([0x02, slot.lane.canonical_tag()]);
                hasher.update(slot.ordinal.to_le_bytes());
            }
            Self::SourceParameter(ordinal) => {
                hasher.update([0x03]);
                hasher.update(ordinal.to_le_bytes());
            }
            Self::CallableParameter {
                callable_id,
                ordinal,
            } => {
                hasher.update([0x0c]);
                hasher.update(callable_id.bytes());
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
            Self::IsVariant {
                owner_id,
                variant_ordinal,
                base,
            } => {
                hasher.update([0x09]);
                hasher.update(owner_id);
                hasher.update(variant_ordinal.to_le_bytes());
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

/// Canonical executable definition of one safely resolved pure callable.
///
/// The callable identity commits checked declaration/dispatch semantics; the
/// ordered signature and body root additionally make the executable graph
/// independently closed. `CallableParameter` leaves in `body` use this exact
/// callable identity and ordinal namespace.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationCallableDefinition {
    pub(crate) callable_id: ClassificationCallableId,
    pub(crate) parameter_types: Box<[ClassificationTypeId]>,
    pub(crate) return_type: ClassificationTypeId,
    pub(crate) body: ClassificationNodeId,
}

impl ClassificationCallableDefinition {
    fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update(self.callable_id.bytes());
        hash_len(hasher, self.parameter_types.len());
        for parameter_type in self.parameter_types.iter().copied() {
            hasher.update(parameter_type.bytes());
        }
        hasher.update(self.return_type.bytes());
        hasher.update(self.body.bytes());
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
    /// Final normalized selection decision: Matches lowers `p`, Violations
    /// lowers `!p`, and Find All lowers `true`. Backends never reinterpret the
    /// authored polarity independently of this root.
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

/// Whether one expected semantic lane was lowered into the executable graph
/// or must be evaluated by the concrete fallback. Completeness is deliberately
/// lane-local: a residual source binding does not disable lowered admission or
/// FIND lanes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClassificationLaneStatus {
    Lowered,
    Residual,
}

impl ClassificationLaneStatus {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Lowered => 0x01,
            Self::Residual => 0x02,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClassificationLaneManifestEntry {
    pub(crate) lane: ClassificationSemanticLane,
    pub(crate) status: ClassificationLaneStatus,
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
    MatchNormalizationRequired,
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
            Self::MatchNormalizationRequired => 0x0b,
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
    callables: Box<[ClassificationCallableDefinition]>,
    lanes: Box<[ClassificationLaneManifestEntry]>,
    roots: Box<[ClassificationLaneRoot]>,
    residuals: Box<[ClassificationResidual]>,
    graph_root: ClassificationGraphRoot,
}

impl FrozenClassificationProgram {
    pub(crate) fn freeze(
        interner: ClassificationInterner,
        expected_lanes: impl IntoIterator<Item = ClassificationSemanticLane>,
        roots: impl IntoIterator<Item = ClassificationLaneRoot>,
        residuals: impl IntoIterator<Item = ClassificationResidual>,
    ) -> Result<Self, RelationalClassificationCapsuleError> {
        Self::freeze_with_callables(interner, [], expected_lanes, roots, residuals)
    }

    pub(crate) fn freeze_with_callables(
        interner: ClassificationInterner,
        callables: impl IntoIterator<Item = ClassificationCallableDefinition>,
        expected_lanes: impl IntoIterator<Item = ClassificationSemanticLane>,
        roots: impl IntoIterator<Item = ClassificationLaneRoot>,
        residuals: impl IntoIterator<Item = ClassificationResidual>,
    ) -> Result<Self, RelationalClassificationCapsuleError> {
        let mut expected_lanes = expected_lanes.into_iter().collect::<Vec<_>>();
        expected_lanes.sort_unstable();
        if expected_lanes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
        }
        if let Some(ordinal) = duplicate_admission_ordinal(expected_lanes.iter().copied()) {
            return Err(RelationalClassificationCapsuleError::DuplicateAdmissionOrdinal(ordinal));
        }

        let mut roots = roots.into_iter().collect::<Vec<_>>();
        roots.sort_unstable();
        if roots.windows(2).any(|pair| pair[0].lane == pair[1].lane) {
            return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
        }

        let mut residuals = residuals.into_iter().collect::<Vec<_>>();
        residuals.sort_unstable();
        residuals.dedup();
        let lanes = derive_lane_manifest(&expected_lanes, &roots, &residuals)?;

        let mut callable_catalog = BTreeMap::new();
        for definition in callables {
            match callable_catalog.entry(definition.callable_id) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(definition);
                }
                std::collections::btree_map::Entry::Occupied(slot) if slot.get() == &definition => {
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(
                        RelationalClassificationCapsuleError::CallableDefinitionCollision(
                            definition.callable_id,
                        ),
                    );
                }
            }
        }

        let (reachable, reachable_callables) =
            collect_reachable_program(&interner.nodes, &callable_catalog, &roots, &residuals)?;
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
        let callables = reachable_callables
            .into_iter()
            .map(|callable_id| {
                callable_catalog.get(&callable_id).cloned().ok_or(
                    RelationalClassificationCapsuleError::MissingCallableDefinition(callable_id),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let graph_root = derive_graph_root(&lanes, &roots, &callables, &residuals);
        Ok(Self {
            version: CLASSIFICATION_GRAPH_VERSION,
            nodes: nodes.into_boxed_slice(),
            callables: callables.into_boxed_slice(),
            lanes: lanes.into_boxed_slice(),
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

    pub(crate) fn callables(&self) -> &[ClassificationCallableDefinition] {
        &self.callables
    }

    pub(crate) fn lane_manifest(&self) -> &[ClassificationLaneManifestEntry] {
        &self.lanes
    }

    pub(crate) fn lane_status(
        &self,
        lane: ClassificationSemanticLane,
    ) -> Option<ClassificationLaneStatus> {
        self.lanes
            .binary_search_by_key(&lane, |entry| entry.lane)
            .ok()
            .map(|index| self.lanes[index].status)
    }

    pub(crate) fn lane_is_complete(&self, lane: ClassificationSemanticLane) -> Option<bool> {
        self.lane_status(lane)
            .map(|status| status == ClassificationLaneStatus::Lowered)
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
        let expected_lanes = self
            .lanes
            .iter()
            .map(|entry| entry.lane)
            .collect::<Vec<_>>();
        let lane_manifest_valid =
            derive_lane_manifest(&expected_lanes, &self.roots, &self.residuals)
                .is_ok_and(|lanes| lanes.as_slice() == self.lanes.as_ref());
        if self.version != CLASSIFICATION_GRAPH_VERSION
            || !self.nodes.windows(2).all(|pair| pair[0].0 < pair[1].0)
            || !self.nodes.iter().all(|(id, key)| *id == key.derive_id())
            || !self
                .callables
                .windows(2)
                .all(|pair| pair[0].callable_id < pair[1].callable_id)
            || !self
                .lanes
                .windows(2)
                .all(|pair| pair[0].lane < pair[1].lane)
            || !self
                .roots
                .windows(2)
                .all(|pair| pair[0].lane < pair[1].lane)
            || !self.residuals.windows(2).all(|pair| pair[0] < pair[1])
            || duplicate_admission_ordinal(self.lanes.iter().map(|entry| entry.lane)).is_some()
            || !lane_manifest_valid
            || self.graph_root
                != derive_graph_root(&self.lanes, &self.roots, &self.callables, &self.residuals)
        {
            return false;
        }

        let nodes = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.clone()))
            .collect::<BTreeMap<_, _>>();
        let callables = self
            .callables
            .iter()
            .cloned()
            .map(|definition| (definition.callable_id, definition))
            .collect::<BTreeMap<_, _>>();
        collect_reachable_program(&nodes, &callables, &self.roots, &self.residuals).is_ok_and(
            |(reachable, reachable_callables)| {
                reachable.len() == self.nodes.len()
                    && reachable_callables.len() == self.callables.len()
            },
        )
    }
}

fn collect_reachable_program(
    nodes: &BTreeMap<ClassificationNodeId, ClassificationNodeKey>,
    callables: &BTreeMap<ClassificationCallableId, ClassificationCallableDefinition>,
    roots: &[ClassificationLaneRoot],
    residuals: &[ClassificationResidual],
) -> Result<
    (
        BTreeSet<ClassificationNodeId>,
        BTreeSet<ClassificationCallableId>,
    ),
    RelationalClassificationCapsuleError,
> {
    let mut reachable = BTreeSet::new();
    let mut reachable_callables = BTreeSet::new();
    let mut visited_frames = BTreeSet::new();
    let mut pending = roots
        .iter()
        .map(|root| (root.node, None))
        .chain(
            residuals
                .iter()
                .flat_map(ClassificationResidual::node_dependencies)
                .map(|node_id| (node_id, None)),
        )
        .collect::<Vec<_>>();

    while let Some((node_id, active_callable)) = pending.pop() {
        reachable.insert(node_id);
        if !visited_frames.insert((node_id, active_callable)) {
            continue;
        }
        let node = nodes
            .get(&node_id)
            .ok_or(RelationalClassificationCapsuleError::UnresolvedNodeReference(node_id))?;
        match &node.kind {
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal,
            } => {
                let definition = callables.get(callable_id).ok_or(
                    RelationalClassificationCapsuleError::MissingCallableDefinition(*callable_id),
                )?;
                let Some(parameter_type) = usize::try_from(*ordinal)
                    .ok()
                    .and_then(|ordinal| definition.parameter_types.get(ordinal))
                else {
                    return Err(
                        RelationalClassificationCapsuleError::InvalidCallableDefinition(
                            *callable_id,
                        ),
                    );
                };
                if active_callable != Some(*callable_id) || parameter_type != &node.ty {
                    return Err(
                        RelationalClassificationCapsuleError::InvalidCallableDefinition(
                            *callable_id,
                        ),
                    );
                }
            }
            ClassificationNodeKind::Input(_) | ClassificationNodeKind::SourceParameter(_)
                if active_callable.is_some() =>
            {
                return Err(
                    RelationalClassificationCapsuleError::InvalidCallableDefinition(
                        active_callable.expect("checked present callable"),
                    ),
                );
            }
            ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } => {
                let definition = callables.get(callable_id).ok_or(
                    RelationalClassificationCapsuleError::MissingCallableDefinition(*callable_id),
                )?;
                let body = nodes.get(&definition.body).ok_or(
                    RelationalClassificationCapsuleError::UnresolvedNodeReference(definition.body),
                )?;
                let valid_arguments = arguments.len() == definition.parameter_types.len()
                    && arguments.iter().zip(definition.parameter_types.iter()).all(
                        |(argument, parameter_type)| {
                            nodes
                                .get(argument)
                                .is_some_and(|argument| argument.ty == *parameter_type)
                        },
                    );
                if !valid_arguments
                    || node.ty != definition.return_type
                    || body.ty != definition.return_type
                {
                    return Err(
                        RelationalClassificationCapsuleError::InvalidCallableApplication(node_id),
                    );
                }
                reachable_callables.insert(*callable_id);
                pending.extend(
                    arguments
                        .iter()
                        .copied()
                        .map(|argument| (argument, active_callable)),
                );
                pending.push((definition.body, Some(*callable_id)));
            }
            _ => pending.extend(
                node.kind
                    .child_ids()
                    .iter()
                    .copied()
                    .map(|child| (child, active_callable)),
            ),
        }
    }

    validate_acyclic_callables(nodes, callables, &reachable_callables)?;
    Ok((reachable, reachable_callables))
}

fn validate_acyclic_callables(
    nodes: &BTreeMap<ClassificationNodeId, ClassificationNodeKey>,
    callables: &BTreeMap<ClassificationCallableId, ClassificationCallableDefinition>,
    reachable_callables: &BTreeSet<ClassificationCallableId>,
) -> Result<(), RelationalClassificationCapsuleError> {
    fn visit(
        callable_id: ClassificationCallableId,
        nodes: &BTreeMap<ClassificationNodeId, ClassificationNodeKey>,
        callables: &BTreeMap<ClassificationCallableId, ClassificationCallableDefinition>,
        visiting: &mut BTreeSet<ClassificationCallableId>,
        visited: &mut BTreeSet<ClassificationCallableId>,
    ) -> Result<(), RelationalClassificationCapsuleError> {
        if visited.contains(&callable_id) {
            return Ok(());
        }
        if !visiting.insert(callable_id) {
            return Err(
                RelationalClassificationCapsuleError::RecursiveCallableDefinition(callable_id),
            );
        }
        let definition = callables
            .get(&callable_id)
            .ok_or(RelationalClassificationCapsuleError::MissingCallableDefinition(callable_id))?;
        let mut seen_nodes = BTreeSet::new();
        let mut pending = vec![definition.body];
        let mut dependencies = BTreeSet::new();
        while let Some(node_id) = pending.pop() {
            if !seen_nodes.insert(node_id) {
                continue;
            }
            let node = nodes
                .get(&node_id)
                .ok_or(RelationalClassificationCapsuleError::UnresolvedNodeReference(node_id))?;
            if let ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } = &node.kind
            {
                dependencies.insert(*callable_id);
                pending.extend(arguments.iter().copied());
            } else {
                pending.extend(node.kind.child_ids());
            }
        }
        for dependency in dependencies {
            visit(dependency, nodes, callables, visiting, visited)?;
        }
        visiting.remove(&callable_id);
        visited.insert(callable_id);
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for callable_id in reachable_callables.iter().copied() {
        visit(callable_id, nodes, callables, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn derive_lane_manifest(
    expected_lanes: &[ClassificationSemanticLane],
    roots: &[ClassificationLaneRoot],
    residuals: &[ClassificationResidual],
) -> Result<Vec<ClassificationLaneManifestEntry>, RelationalClassificationCapsuleError> {
    let expected = expected_lanes.iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != expected_lanes.len() {
        return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
    }

    let mut statuses = BTreeMap::new();
    for root in roots {
        if !expected.contains(&root.lane) {
            return Err(RelationalClassificationCapsuleError::UnexpectedSemanticLane(root.lane));
        }
        if statuses
            .insert(root.lane, ClassificationLaneStatus::Lowered)
            .is_some()
        {
            return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
        }
    }
    for residual in residuals {
        if !expected.contains(&residual.lane) {
            return Err(
                RelationalClassificationCapsuleError::UnexpectedSemanticLane(residual.lane),
            );
        }
        if statuses
            .insert(residual.lane, ClassificationLaneStatus::Residual)
            .is_some()
        {
            return Err(RelationalClassificationCapsuleError::DuplicateSemanticLane);
        }
    }

    expected_lanes
        .iter()
        .copied()
        .map(|lane| {
            statuses
                .get(&lane)
                .copied()
                .map(|status| ClassificationLaneManifestEntry { lane, status })
                .ok_or(RelationalClassificationCapsuleError::MissingSemanticLane(
                    lane,
                ))
        })
        .collect()
}

fn duplicate_admission_ordinal(
    lanes: impl IntoIterator<Item = ClassificationSemanticLane>,
) -> Option<u32> {
    let mut ordinals = BTreeSet::new();
    lanes.into_iter().find_map(|lane| {
        let ClassificationSemanticLane::Admission { ordinal, .. } = lane else {
            return None;
        };
        (!ordinals.insert(ordinal)).then_some(ordinal)
    })
}

fn derive_graph_root(
    lanes: &[ClassificationLaneManifestEntry],
    roots: &[ClassificationLaneRoot],
    callables: &[ClassificationCallableDefinition],
    residuals: &[ClassificationResidual],
) -> ClassificationGraphRoot {
    let mut hasher = Sha256::new();
    hasher.update(GRAPH_ROOT_V1);
    hasher.update(CLASSIFICATION_GRAPH_VERSION.to_le_bytes());
    hash_len(&mut hasher, lanes.len());
    for entry in lanes {
        entry.lane.hash_into(&mut hasher);
        hasher.update([entry.status.canonical_tag()]);
    }
    hash_len(&mut hasher, roots.len());
    for root in roots {
        root.lane.hash_into(&mut hasher);
        hasher.update(root.node.bytes());
    }
    hash_len(&mut hasher, callables.len());
    for callable in callables {
        callable.hash_into(&mut hasher);
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
    runtime_shapes: Arc<FrozenClassificationRuntimeShapes>,
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
        runtime_shapes: Arc<FrozenClassificationRuntimeShapes>,
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
        runtime_shapes.validate_for_program(graph.as_ref())?;
        let id = derive_capsule_id(
            graph.graph_root(),
            runtime_shapes.runtime_shape_root(),
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
            runtime_shapes,
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

    pub(crate) fn runtime_shape_root(&self) -> RuntimeShapeRoot {
        self.runtime_shapes.runtime_shape_root()
    }

    pub(crate) fn runtime_shapes(&self) -> &FrozenClassificationRuntimeShapes {
        self.runtime_shapes.as_ref()
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn root_cell_id(&self) -> Option<SupportCellId> {
        self.root_cell_id
    }

    pub(crate) const fn specialization_root(&self) -> ClassificationSpecializationRoot {
        self.specialization_root
    }

    pub(crate) const fn provenance_root(&self) -> ClassificationProvenanceRoot {
        self.provenance_root
    }

    /// Rebind a retained process-local evaluator only to the exact checked
    /// query and support scope that minted this capsule. Internal identity
    /// validity alone is insufficient: a different, self-consistent capsule
    /// must never classify this host's cases.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validates_binding(
        &self,
        checked_program: [u8; 32],
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        support_plan_root: RelationalSupportPlanRoot,
        root_cell_id: Option<SupportCellId>,
    ) -> bool {
        self.validate_identity()
            && self.checked_program == checked_program
            && self.relation_id == relation_id
            && self.admission_id == admission_id
            && self.question_id == question_id
            && self.support_plan_root == support_plan_root
            && self.root_cell_id == root_cell_id
    }

    pub(crate) fn validate_identity(&self) -> bool {
        self.graph.validate_identity()
            && self
                .runtime_shapes
                .validate_for_program(self.graph.as_ref())
                .is_ok()
            && self.id
                == derive_capsule_id(
                    self.graph.graph_root(),
                    self.runtime_shapes.runtime_shape_root(),
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
    runtime_shape_root: RuntimeShapeRoot,
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
    hasher.update(runtime_shape_root.bytes());
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
    CallableDefinitionCollision(ClassificationCallableId),
    RuntimeShapeKeyCollision(RuntimeConstructorKey),
    RuntimeConstructorDigestCollision([u8; 32]),
    UnresolvedNodeReference(ClassificationNodeId),
    MissingCallableDefinition(ClassificationCallableId),
    InvalidCallableDefinition(ClassificationCallableId),
    InvalidCallableApplication(ClassificationNodeId),
    RecursiveCallableDefinition(ClassificationCallableId),
    DuplicateSemanticLane,
    MissingSemanticLane(ClassificationSemanticLane),
    UnexpectedSemanticLane(ClassificationSemanticLane),
    DuplicateAdmissionOrdinal(u32),
    InvalidGraphIdentity,
    InvalidRuntimeShapeIdentity,
    MissingRuntimeConstructorShape([u8; 32]),
    MissingRuntimeVariantShape(RuntimeConstructorKey),
    RuntimeConstructArityMismatch {
        node_id: ClassificationNodeId,
        expected: usize,
        actual: usize,
    },
    RuntimeProjectionFieldOutOfBounds {
        node_id: ClassificationNodeId,
        field_ordinal: u32,
        field_count: usize,
    },
    UnusedRuntimeShape(RuntimeConstructorKey),
}

impl fmt::Display for RelationalClassificationCapsuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeDigestCollision(node_id) => write!(
                formatter,
                "classification node digest collision at {}",
                lowercase_hex(node_id.bytes())
            ),
            Self::CallableDefinitionCollision(callable_id) => write!(
                formatter,
                "classification callable definition collision at {}",
                lowercase_hex(callable_id.bytes())
            ),
            Self::RuntimeShapeKeyCollision(key) => write!(
                formatter,
                "classification runtime shape conflicts at owner {} variant {}",
                lowercase_hex(key.owner_id),
                key.variant_ordinal
            ),
            Self::RuntimeConstructorDigestCollision(constructor_id) => write!(
                formatter,
                "classification runtime constructor digest collision at {}",
                lowercase_hex(*constructor_id)
            ),
            Self::UnresolvedNodeReference(node_id) => write!(
                formatter,
                "classification graph references absent node {}",
                lowercase_hex(node_id.bytes())
            ),
            Self::MissingCallableDefinition(callable_id) => write!(
                formatter,
                "classification graph references absent callable {}",
                lowercase_hex(callable_id.bytes())
            ),
            Self::InvalidCallableDefinition(callable_id) => write!(
                formatter,
                "classification callable {} has an invalid closed definition",
                lowercase_hex(callable_id.bytes())
            ),
            Self::InvalidCallableApplication(node_id) => write!(
                formatter,
                "classification call node {} does not match its frozen definition",
                lowercase_hex(node_id.bytes())
            ),
            Self::RecursiveCallableDefinition(callable_id) => write!(
                formatter,
                "classification callable {} is recursive",
                lowercase_hex(callable_id.bytes())
            ),
            Self::DuplicateSemanticLane => {
                formatter.write_str("classification graph has duplicate semantic lane outcomes")
            }
            Self::MissingSemanticLane(lane) => write!(
                formatter,
                "classification graph has no lowered root or residual for expected lane {lane:?}"
            ),
            Self::UnexpectedSemanticLane(lane) => write!(
                formatter,
                "classification graph has an outcome for unexpected lane {lane:?}"
            ),
            Self::DuplicateAdmissionOrdinal(ordinal) => write!(
                formatter,
                "classification graph repeats admission ordinal {ordinal} across semantic lanes"
            ),
            Self::InvalidGraphIdentity => {
                formatter.write_str("classification graph identity is invalid")
            }
            Self::InvalidRuntimeShapeIdentity => {
                formatter.write_str("classification runtime-shape identity is invalid")
            }
            Self::MissingRuntimeConstructorShape(constructor_id) => write!(
                formatter,
                "classification graph has no runtime shape for constructor {}",
                lowercase_hex(*constructor_id)
            ),
            Self::MissingRuntimeVariantShape(key) => write!(
                formatter,
                "classification graph has no runtime shape for owner {} variant {}",
                lowercase_hex(key.owner_id),
                key.variant_ordinal
            ),
            Self::RuntimeConstructArityMismatch {
                node_id,
                expected,
                actual,
            } => write!(
                formatter,
                "classification construct node {} has {actual} fields but its runtime shape has {expected}",
                lowercase_hex(node_id.bytes())
            ),
            Self::RuntimeProjectionFieldOutOfBounds {
                node_id,
                field_ordinal,
                field_count,
            } => write!(
                formatter,
                "classification projection node {} selects field {field_ordinal} from a runtime shape with {field_count} fields",
                lowercase_hex(node_id.bytes())
            ),
            Self::UnusedRuntimeShape(key) => write!(
                formatter,
                "classification capsule carries unused runtime shape for owner {} variant {}",
                lowercase_hex(key.owner_id),
                key.variant_ordinal
            ),
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
        ClassificationNodeKind::Constant(ClassificationConstant::Integer(i64::from(tag)))
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
            [ClassificationSemanticLane::Find],
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
            [ClassificationSemanticLane::Find],
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

    #[test]
    fn exact_constants_are_self_describing_and_executable() {
        let values = [
            ClassificationConstant::Integer(-7),
            ClassificationConstant::Boolean(true),
            ClassificationConstant::Unit,
            ClassificationConstant::Character('x'),
            ClassificationConstant::String("tax".into()),
        ];
        assert_eq!(
            values[0].to_explore_value(),
            super::super::ExploreValue::Int(-7)
        );
        assert_eq!(
            values[1].to_explore_value(),
            super::super::ExploreValue::Boolean(true)
        );
        assert_eq!(
            values[2].to_explore_value(),
            super::super::ExploreValue::Unit
        );
        assert_eq!(
            values[3].to_explore_value(),
            super::super::ExploreValue::Character('x')
        );
        assert_eq!(
            values[4].to_explore_value(),
            super::super::ExploreValue::String("tax".into())
        );

        let mut interner = ClassificationInterner::default();
        let integer = interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: ClassificationNodeKind::Constant(values[0].clone()),
            })
            .unwrap();
        let boolean = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Constant(values[1].clone()),
            })
            .unwrap();
        assert_ne!(integer, boolean);
    }

    #[test]
    fn transition_endpoints_reuse_one_context_state_observation_dag() {
        let mut interner = ClassificationInterner::default();
        let integer_type = type_id(1);
        let boolean_type = type_id(2);
        let context_type = type_id(3);
        let state_type = type_id(4);
        let callable_id = ClassificationCallableId::from_checked_callable_digest([8; 32]);

        let context = interner
            .intern(ClassificationNodeKey {
                ty: context_type,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::CONTEXT),
            })
            .unwrap();
        let before = interner
            .intern(ClassificationNodeKey {
                ty: state_type,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::BEFORE),
            })
            .unwrap();
        let after = interner
            .intern(ClassificationNodeKey {
                ty: state_type,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::AFTER),
            })
            .unwrap();
        let context_parameter = interner
            .intern(ClassificationNodeKey {
                ty: context_type,
                kind: ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal: 0,
                },
            })
            .unwrap();
        let state_parameter = interner
            .intern(ClassificationNodeKey {
                ty: state_type,
                kind: ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal: 1,
                },
            })
            .unwrap();
        let context_amount = interner
            .intern(ClassificationNodeKey {
                ty: integer_type,
                kind: ClassificationNodeKind::Project {
                    owner_id: [3; 32],
                    variant_ordinal: 0,
                    field_ordinal: 0,
                    base: context_parameter,
                },
            })
            .unwrap();
        let state_amount = interner
            .intern(ClassificationNodeKey {
                ty: integer_type,
                kind: ClassificationNodeKind::Project {
                    owner_id: [4; 32],
                    variant_ordinal: 0,
                    field_ordinal: 0,
                    base: state_parameter,
                },
            })
            .unwrap();
        let observation = interner
            .intern(ClassificationNodeKey {
                ty: integer_type,
                kind: ClassificationNodeKind::Binary {
                    op: ClassificationBinaryOp::IntegerAddChecked,
                    left: context_amount,
                    right: state_amount,
                },
            })
            .unwrap();
        let before_observation = interner
            .intern(ClassificationNodeKey {
                ty: integer_type,
                kind: ClassificationNodeKind::Call {
                    callable_id,
                    arguments: Box::new([context, before]),
                },
            })
            .unwrap();
        let after_observation = interner
            .intern(ClassificationNodeKey {
                ty: integer_type,
                kind: ClassificationNodeKind::Call {
                    callable_id,
                    arguments: Box::new([context, after]),
                },
            })
            .unwrap();
        let selected = interner
            .intern(ClassificationNodeKey {
                ty: boolean_type,
                kind: ClassificationNodeKind::Binary {
                    op: ClassificationBinaryOp::LessThan,
                    left: after_observation,
                    right: before_observation,
                },
            })
            .unwrap();
        let graph = FrozenClassificationProgram::freeze_with_callables(
            interner,
            [ClassificationCallableDefinition {
                callable_id,
                parameter_types: Box::new([context_type, state_type]),
                return_type: integer_type,
                body: observation,
            }],
            [ClassificationSemanticLane::Find],
            [ClassificationLaneRoot {
                lane: ClassificationSemanticLane::Find,
                node: selected,
            }],
            [],
        )
        .unwrap();

        assert_ne!(before_observation, after_observation);
        assert_eq!(graph.callables().len(), 1);
        assert_eq!(graph.callables()[0].body, observation);
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|(_, node)| {
                    node.kind == ClassificationNodeKind::Input(ClassificationInputSlot::CONTEXT)
                })
                .count(),
            1
        );
        assert!(graph.validate_identity());
    }

    #[test]
    fn pure_call_definition_body_is_reachable_and_graph_semantic() {
        fn graph_for(op: ClassificationBinaryOp) -> FrozenClassificationProgram {
            let mut interner = ClassificationInterner::default();
            let integer_type = type_id(1);
            let boolean_type = type_id(2);
            let callable_id = ClassificationCallableId::from_checked_callable_digest([8; 32]);
            let parameter = interner
                .intern(ClassificationNodeKey {
                    ty: integer_type,
                    kind: ClassificationNodeKind::CallableParameter {
                        callable_id,
                        ordinal: 0,
                    },
                })
                .unwrap();
            let zero = interner
                .intern(ClassificationNodeKey {
                    ty: integer_type,
                    kind: constant(0),
                })
                .unwrap();
            let body = interner
                .intern(ClassificationNodeKey {
                    ty: boolean_type,
                    kind: ClassificationNodeKind::Binary {
                        op,
                        left: parameter,
                        right: zero,
                    },
                })
                .unwrap();
            let argument = interner
                .intern(ClassificationNodeKey {
                    ty: integer_type,
                    kind: ClassificationNodeKind::Input(ClassificationInputSlot::BEFORE),
                })
                .unwrap();
            let call = interner
                .intern(ClassificationNodeKey {
                    ty: boolean_type,
                    kind: ClassificationNodeKind::Call {
                        callable_id,
                        arguments: Box::new([argument]),
                    },
                })
                .unwrap();
            FrozenClassificationProgram::freeze_with_callables(
                interner,
                [ClassificationCallableDefinition {
                    callable_id,
                    parameter_types: Box::new([integer_type]),
                    return_type: boolean_type,
                    body,
                }],
                [ClassificationSemanticLane::Find],
                [ClassificationLaneRoot {
                    lane: ClassificationSemanticLane::Find,
                    node: call,
                }],
                [],
            )
            .unwrap()
        }

        let greater = graph_for(ClassificationBinaryOp::GreaterThan);
        let greater_or_equal = graph_for(ClassificationBinaryOp::GreaterThanOrEqual);
        assert_eq!(greater.roots(), greater_or_equal.roots());
        assert_ne!(
            greater.callables()[0].body,
            greater_or_equal.callables()[0].body
        );
        assert_ne!(greater.graph_root(), greater_or_equal.graph_root());
        assert!(greater
            .nodes()
            .iter()
            .any(|(node_id, _)| *node_id == greater.callables()[0].body));
        assert!(greater.validate_identity());
    }

    #[test]
    fn source_parameters_and_residuals_have_lane_local_completeness() {
        let source_zero_lane = ClassificationSemanticLane::SourceBinding(0);
        let source_one_lane = ClassificationSemanticLane::SourceBinding(1);
        let mut interner = ClassificationInterner::default();
        let source_zero = interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: ClassificationNodeKind::SourceParameter(0),
            })
            .unwrap();
        let find = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Constant(ClassificationConstant::Boolean(true)),
            })
            .unwrap();
        let graph = FrozenClassificationProgram::freeze(
            interner,
            [
                source_zero_lane,
                source_one_lane,
                ClassificationSemanticLane::Find,
            ],
            [
                ClassificationLaneRoot {
                    lane: source_zero_lane,
                    node: source_zero,
                },
                ClassificationLaneRoot {
                    lane: ClassificationSemanticLane::Find,
                    node: find,
                },
            ],
            [ClassificationResidual::new(
                ClassificationResidualReason::CollectionTraversal,
                source_one_lane,
                [9; 32],
                [],
            )],
        )
        .unwrap();

        assert_eq!(graph.lane_manifest().len(), 3);
        assert_eq!(
            graph.lane_status(source_zero_lane),
            Some(ClassificationLaneStatus::Lowered)
        );
        assert_eq!(
            graph.lane_status(source_one_lane),
            Some(ClassificationLaneStatus::Residual)
        );
        assert_eq!(
            graph.lane_status(ClassificationSemanticLane::Find),
            Some(ClassificationLaneStatus::Lowered)
        );
        assert_eq!(graph.lane_is_complete(source_one_lane), Some(false));
        assert_eq!(
            graph.lane_is_complete(ClassificationSemanticLane::Find),
            Some(true)
        );
        assert!(!graph.is_complete());
        assert!(graph.validate_identity());

        let mut incomplete_interner = ClassificationInterner::default();
        let find = incomplete_interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Constant(ClassificationConstant::Boolean(true)),
            })
            .unwrap();
        assert_eq!(
            FrozenClassificationProgram::freeze(
                incomplete_interner,
                [source_zero_lane, ClassificationSemanticLane::Find],
                [ClassificationLaneRoot {
                    lane: ClassificationSemanticLane::Find,
                    node: find,
                }],
                [],
            ),
            Err(RelationalClassificationCapsuleError::MissingSemanticLane(
                source_zero_lane
            ))
        );
    }

    fn frozen_admission(scope: ClassificationAdmissionScope) -> FrozenClassificationProgram {
        let mut interner = ClassificationInterner::default();
        let predicate = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Constant(ClassificationConstant::Boolean(true)),
            })
            .unwrap();
        FrozenClassificationProgram::freeze(
            interner,
            [ClassificationSemanticLane::Admission { ordinal: 0, scope }],
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
                kind: ClassificationNodeKind::Constant(ClassificationConstant::Boolean(true)),
            })
            .unwrap();
        assert_eq!(
            FrozenClassificationProgram::freeze(
                interner,
                [
                    ClassificationSemanticLane::Admission {
                        ordinal: 0,
                        scope: ClassificationAdmissionScope::Before,
                    },
                    ClassificationSemanticLane::Admission {
                        ordinal: 0,
                        scope: ClassificationAdmissionScope::After,
                    },
                ],
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
                ClassificationResidualDependency::Node(root.node),
                ClassificationResidualDependency::RuleFamily([3; 32]),
                ClassificationResidualDependency::RuleFamily([2; 32]),
                ClassificationResidualDependency::RuleFamily([3; 32]),
            ],
        );
        assert_eq!(
            residual.dependencies.as_ref(),
            &[
                ClassificationResidualDependency::Node(root.node),
                ClassificationResidualDependency::RuleFamily([2; 32]),
                ClassificationResidualDependency::RuleFamily([3; 32]),
            ]
        );
        let with_residual = FrozenClassificationProgram::freeze(
            ClassificationInterner { nodes },
            [ClassificationSemanticLane::Find],
            [],
            [residual],
        )
        .unwrap();
        assert_ne!(base.graph_root(), with_residual.graph_root());
        assert!(!with_residual.is_complete());
        assert_eq!(
            with_residual.lane_status(ClassificationSemanticLane::Find),
            Some(ClassificationLaneStatus::Residual)
        );

        let callable_residual = ClassificationResidual::new(
            ClassificationResidualReason::DynamicDispatch,
            ClassificationSemanticLane::Find,
            [7; 32],
            [ClassificationResidualDependency::Callable(
                ClassificationCallableId::from_checked_callable_digest([2; 32]),
            )],
        );
        let callable_graph = FrozenClassificationProgram::freeze(
            ClassificationInterner::default(),
            [ClassificationSemanticLane::Find],
            [],
            [callable_residual],
        )
        .unwrap();
        let rule_family_graph = FrozenClassificationProgram::freeze(
            ClassificationInterner::default(),
            [ClassificationSemanticLane::Find],
            [],
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
        capsule_with_shapes(
            graph,
            Arc::new(FrozenClassificationRuntimeShapes::freeze([]).unwrap()),
            tags,
        )
    }

    fn capsule_with_shapes(
        graph: Arc<FrozenClassificationProgram>,
        runtime_shapes: Arc<FrozenClassificationRuntimeShapes>,
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
            runtime_shapes,
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

    fn constructor_program() -> FrozenClassificationProgram {
        let mut interner = ClassificationInterner::default();
        let field = interner
            .intern(ClassificationNodeKey {
                ty: type_id(1),
                kind: constant(7),
            })
            .unwrap();
        let value = interner
            .intern(ClassificationNodeKey {
                ty: type_id(3),
                kind: ClassificationNodeKind::Construct {
                    constructor_id: [32; 32],
                    fields: Box::new([field]),
                },
            })
            .unwrap();
        let predicate = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::Binary {
                    op: ClassificationBinaryOp::Equal,
                    left: value,
                    right: value,
                },
            })
            .unwrap();
        FrozenClassificationProgram::freeze(
            interner,
            [ClassificationSemanticLane::Find],
            [ClassificationLaneRoot {
                lane: ClassificationSemanticLane::Find,
                node: predicate,
            }],
            [],
        )
        .unwrap()
    }

    fn constructor_shape(type_name: &str) -> RuntimeConstructorShape {
        RuntimeConstructorShape::new(
            [31; 32],
            0,
            [32; 32],
            type_name.into(),
            "Only".into(),
            ClassificationRuntimeLayout::Named,
            Box::new([Box::<str>::from("amount")]),
        )
    }

    #[test]
    fn runtime_spellings_are_capsule_bound_but_not_graph_semantics() {
        let graph = Arc::new(constructor_program());
        let first_shapes = Arc::new(
            FrozenClassificationRuntimeShapes::freeze([constructor_shape("State")]).unwrap(),
        );
        let renamed_shapes = Arc::new(
            FrozenClassificationRuntimeShapes::freeze([constructor_shape("RenamedState")]).unwrap(),
        );
        let tags = CapsuleTags::default();
        let first = capsule_with_shapes(Arc::clone(&graph), first_shapes, tags);
        let renamed = capsule_with_shapes(Arc::clone(&graph), renamed_shapes, tags);

        assert_eq!(first.graph_root(), renamed.graph_root());
        assert_ne!(first.runtime_shape_root(), renamed.runtime_shape_root());
        assert_ne!(first.id(), renamed.id());
        assert!(first.validate_identity());
        assert!(renamed.validate_identity());
    }

    #[test]
    fn capsule_requires_exact_reachable_runtime_shapes() {
        let graph = Arc::new(constructor_program());
        let empty = Arc::new(FrozenClassificationRuntimeShapes::freeze([]).unwrap());
        let tags = CapsuleTags::default();
        let relation_id = RelationId::from_canonical_semantic_digest([tags.relation; 32]);
        let admission_id =
            AdmissionId::from_canonical_admission_digest(relation_id, [tags.admission; 32]);
        let question_id = QuestionId::from_canonical_find_digest(
            admission_id,
            [tags.question; 32],
            FindPolarity::Matches,
        );
        assert!(matches!(
            RelationalClassificationCapsule::bind(
                Arc::clone(&graph),
                empty,
                [tags.checked_program; 32],
                relation_id,
                admission_id,
                question_id,
                RelationalSupportPlanRoot::from_journal_codec_bytes([tags.support_plan; 32]),
                tags.root_cell
                    .map(|tag| SupportCellId::from_journal_codec_bytes([tag; 32])),
                ClassificationSpecializationRoot::none(),
                ClassificationProvenanceRoot::from_checked_source_coverage_digest(
                    [tags.provenance; 32],
                ),
            ),
            Err(RelationalClassificationCapsuleError::MissingRuntimeConstructorShape(id))
                if id == [32; 32]
        ));

        let scalar_graph = Arc::new(frozen_program(ClassificationBinaryOp::LessThan));
        let unused = Arc::new(
            FrozenClassificationRuntimeShapes::freeze([constructor_shape("State")]).unwrap(),
        );
        assert_eq!(
            unused.validate_for_program(scalar_graph.as_ref()),
            Err(RelationalClassificationCapsuleError::UnusedRuntimeShape(
                RuntimeConstructorKey {
                    owner_id: [31; 32],
                    variant_ordinal: 0,
                }
            ))
        );
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
