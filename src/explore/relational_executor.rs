//! Lazy, resumable source-relation execution for canonical Explore IR.
//!
//! One cursor expands exactly one ordered FROM binding under an already
//! materialized canonical prefix. It never owns a Cartesian rank, recursively
//! drains the source relation, or batches completed rows. A scheduler may pause
//! after any yielded member, persist the cursor snapshot, and publish child
//! prefix work immediately while the parent fiber remains open.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{
    RelationId, RelationLineageId, RelationProvenance, RelationSupportId, SourceKey, SourceRow,
};
use super::relational_frontier::{CanonicalSourcePrefix, WorkFrontierError, WorkNodeSpec};
use super::relational_ir::{
    relational_tys_equivalent, ExploreFiniteDomainIr, ExploreSourceBindingKindIr,
    ExploreSourceBindingRoleIr, ExploreSourceRelationIr,
};
use super::support_cell::{
    SupportCell, SupportCellSpace, SupportExpr, SupportMaterializerId, SupportProducerId,
};
use super::transition::canonical_explore_value_digest;
use super::{
    ExploreCardinality, ExploreEnumeratedSource, ExploreExactDomain, ExploreFiniteTypePlan,
    ExploreValue,
};
use crate::{
    runtime_nominal_declared_type_name, ExploreRelationMultiplicity, Expr, Ty,
    EXPLORE_RELATION_NORMALIZATION_VERSION,
};

pub(crate) const RELATIONAL_SOURCE_CURSOR_VERSION: u32 = 1;
pub(crate) const SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION: u32 = 1;

const SOURCE_LINEAGE_PREIMAGE_V1: &[u8] = b"futuruna.explore.source-lineage-preimage.v1";
const SOURCE_SUPPORT_PREIMAGE_V1: &[u8] = b"futuruna.explore.source-support-preimage.v1";
const SOURCE_FIBER_MEMBER_COMMITMENT_V1: &[u8] =
    b"futuruna.explore.source-fiber-member-commitment.v1";
const SOURCE_BINDING_EXHAUSTION_RECEIPT_HASH_V1: &[u8] =
    b"futuruna.explore.source-binding-exhaustion-receipt.v1";

/// Content identity of one executor-issued source-binding exhaustion receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceBindingExhaustionReceiptId([u8; 32]);

impl SourceBindingExhaustionReceiptId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One earlier binding visible while evaluating a dependent source domain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalBoundValue<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: &'a ExploreValue,
}

/// Checked expression-runtime boundary used by relational enumeration.
///
/// The executor owns dependency order, domain shape, canonicalization, and
/// cursors. The surrounding checked runtime owns ordinary Futuruna expression
/// semantics and conversion to the declared first-order [`ExploreValue`].
pub(crate) trait RelationalExpressionRuntime {
    fn evaluate(
        &mut self,
        expression: &Expr,
        expected_ty: &Ty,
        earlier_bindings: &[RelationalBoundValue<'_>],
    ) -> Result<ExploreValue, String>;
}

/// Canonical member of one set-normalized binding fiber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalFiberMember {
    value: ExploreValue,
    canonical_ordinal: u128,
    /// Source-list or enumerated-domain positions collapsed into this member.
    /// Intrinsically set-valued and lazy unique domains use their canonical
    /// ordinal as their sole support coordinate.
    raw_support_ordinals: Box<[u128]>,
}

impl RelationalFiberMember {
    pub(super) fn restore_from_journal_codec(
        value: ExploreValue,
        canonical_ordinal: u128,
        raw_support_ordinals: Box<[u128]>,
    ) -> Result<Self, RelationalSourceExecutorError> {
        if raw_support_ordinals.is_empty()
            || raw_support_ordinals
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(RelationalSourceExecutorError::NonCanonicalSupportOrdinals(
                0,
            ));
        }
        Ok(Self {
            value,
            canonical_ordinal,
            raw_support_ordinals,
        })
    }

    pub(crate) fn value(&self) -> &ExploreValue {
        &self.value
    }

    pub(crate) const fn canonical_ordinal(&self) -> u128 {
        self.canonical_ordinal
    }

    pub(crate) fn raw_support_ordinals(&self) -> &[u128] {
        &self.raw_support_ordinals
    }
}

#[derive(Clone, Debug)]
enum RelationalFiniteFiberKind {
    Materialized(Box<[MaterializedFiberMember]>),
    IntRange { start: i64 },
    FiniteType { plan: ExploreFiniteTypePlan },
}

#[derive(Clone, Debug)]
struct MaterializedFiberMember {
    value: ExploreValue,
    raw_support_ordinals: Box<[u128]>,
}

/// One evaluated binding fiber. Ranges and finite-type products retain only
/// their compact plans and decode members directly by ordinal.
#[derive(Clone, Debug)]
pub(crate) struct RelationalFiniteFiber {
    cardinality: u128,
    kind: RelationalFiniteFiberKind,
    origin: Option<RelationalFiberOrigin>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalFiberOrigin {
    relation_id: RelationId,
    binding_index: u32,
    prefix_digest: [u8; 32],
}

impl RelationalFiniteFiber {
    pub(crate) const fn cardinality(&self) -> u128 {
        self.cardinality
    }

    /// Describe this checked binding fiber as one exact producer-coordinate
    /// cell without enumerating its values. Empty fibers have no support cell.
    ///
    /// The producer identity includes the semantic relation, binding position,
    /// and canonical dependent prefix. The materializer identity additionally
    /// seals the cursor/coordinate decoding contract, but excludes whether a
    /// worker later uses concrete enumeration, interval reasoning, or a solver.
    pub(crate) fn coordinate_support_cell(
        &self,
    ) -> Result<Option<SupportCell>, RelationalSourceExecutorError> {
        let Some(origin) = self.origin else {
            return Err(RelationalSourceExecutorError::UnboundFiberSupport);
        };
        if self.cardinality == 0 {
            return Ok(None);
        }

        let mut producer_preimage = Vec::with_capacity(32 + 4 + 32);
        producer_preimage.extend_from_slice(&origin.relation_id.bytes());
        producer_preimage.extend_from_slice(&origin.binding_index.to_be_bytes());
        producer_preimage.extend_from_slice(&origin.prefix_digest);
        let producer_id = SupportProducerId::from_canonical_preimage(&producer_preimage);

        let mut materializer_preimage = Vec::with_capacity(4 + producer_preimage.len());
        materializer_preimage.extend_from_slice(&RELATIONAL_SOURCE_CURSOR_VERSION.to_be_bytes());
        materializer_preimage.extend_from_slice(&producer_preimage);
        let materializer_id =
            SupportMaterializerId::from_canonical_preimage(&materializer_preimage);
        let expression = SupportExpr::ordinal_interval(0, self.cardinality)
            .map_err(|error| RelationalSourceExecutorError::SupportCell(error.to_string()))?;
        SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer_id),
            expression,
            materializer_id,
        )
        .map(Some)
        .map_err(|error| RelationalSourceExecutorError::SupportCell(error.to_string()))
    }

    /// Decode one canonical member. `ordinal == cardinality` is the canonical
    /// exhaustion cursor; a greater ordinal is corrupt resume state.
    pub(crate) fn member_at_ordinal(
        &self,
        ordinal: u128,
    ) -> Result<Option<RelationalFiberMember>, RelationalSourceExecutorError> {
        if ordinal == self.cardinality {
            return Ok(None);
        }
        if ordinal > self.cardinality {
            return Err(RelationalSourceExecutorError::OrdinalBeyondCardinality {
                ordinal,
                cardinality: self.cardinality,
            });
        }

        let (value, raw_support_ordinals) = match &self.kind {
            RelationalFiniteFiberKind::Materialized(members) => {
                let index = usize::try_from(ordinal)
                    .map_err(|_| RelationalSourceExecutorError::OrdinalDoesNotFitUsize(ordinal))?;
                let member = members.get(index).ok_or(
                    RelationalSourceExecutorError::OrdinalBeyondCardinality {
                        ordinal,
                        cardinality: self.cardinality,
                    },
                )?;
                (member.value.clone(), member.raw_support_ordinals.clone())
            }
            RelationalFiniteFiberKind::IntRange { start } => {
                let offset = i128::try_from(ordinal)
                    .map_err(|_| RelationalSourceExecutorError::IntegerRangeValueOverflow)?;
                let value = i128::from(*start)
                    .checked_add(offset)
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or(RelationalSourceExecutorError::IntegerRangeValueOverflow)?;
                (ExploreValue::Int(value), vec![ordinal].into_boxed_slice())
            }
            RelationalFiniteFiberKind::FiniteType { plan } => (
                finite_type_value_at(plan, ordinal)?,
                vec![ordinal].into_boxed_slice(),
            ),
        };

        Ok(Some(RelationalFiberMember {
            value,
            canonical_ordinal: ordinal,
            raw_support_ordinals,
        }))
    }

    fn member_for_value(
        &self,
        value: &ExploreValue,
    ) -> Result<Option<RelationalFiberMember>, RelationalSourceExecutorError> {
        let ordinal = match &self.kind {
            RelationalFiniteFiberKind::Materialized(members) => members
                .binary_search_by(|member| member.value.cmp(value))
                .ok()
                .map(|index| {
                    u128::try_from(index).map_err(|_| {
                        RelationalSourceExecutorError::CanonicalLengthOverflow(
                            "canonical fiber members",
                        )
                    })
                })
                .transpose()?,
            RelationalFiniteFiberKind::IntRange { start } => match value {
                ExploreValue::Int(value) if *value >= *start => {
                    let ordinal = u128::try_from(i128::from(*value) - i128::from(*start))
                        .map_err(|_| RelationalSourceExecutorError::IntegerRangeValueOverflow)?;
                    (ordinal < self.cardinality).then_some(ordinal)
                }
                _ => None,
            },
            RelationalFiniteFiberKind::FiniteType { plan } => finite_type_ordinal_of(plan, value)?,
        };
        ordinal
            .map(|ordinal| {
                self.member_at_ordinal(ordinal)?.ok_or(
                    RelationalSourceExecutorError::OrdinalBeyondCardinality {
                        ordinal,
                        cardinality: self.cardinality,
                    },
                )
            })
            .transpose()
    }

    /// Construct the one-member fiber used by checked singleton producers.
    /// Successor execution uses this same representation so singleton cells
    /// and ordinary concrete enumeration cannot diverge semantically.
    pub(crate) fn singleton(value: ExploreValue) -> Self {
        Self {
            cardinality: 1,
            kind: RelationalFiniteFiberKind::Materialized(
                vec![MaterializedFiberMember {
                    value,
                    raw_support_ordinals: vec![0].into_boxed_slice(),
                }]
                .into_boxed_slice(),
            ),
            origin: None,
        }
    }

    fn ordered_collection(
        values: Vec<ExploreValue>,
    ) -> Result<Self, RelationalSourceExecutorError> {
        Self::canonicalized_materialized(values, true)
    }

    fn semantic_set(values: Vec<ExploreValue>) -> Result<Self, RelationalSourceExecutorError> {
        Self::canonicalized_materialized(values, false)
    }

    /// Normalize one already evaluated List or Set with the same canonical
    /// set semantics used by dependent FROM bindings. List occurrence
    /// positions remain exact support coordinates behind each unique member;
    /// an intrinsically set-valued collection has one canonical coordinate.
    pub(crate) fn from_collection_value(
        collection_ty: &Ty,
        value: ExploreValue,
    ) -> Result<Self, RelationalSourceExecutorError> {
        match (collection_type_kind(collection_ty), value) {
            (Some(CollectionTypeKind::List), ExploreValue::List(values)) => {
                Self::ordered_collection(values)
            }
            (Some(CollectionTypeKind::Set), ExploreValue::Set(values)) => {
                Self::semantic_set(values)
            }
            (Some(CollectionTypeKind::List), _) => {
                Err(RelationalSourceExecutorError::ExpectedCollection("List"))
            }
            (Some(CollectionTypeKind::Set), _) => {
                Err(RelationalSourceExecutorError::ExpectedCollection("Set"))
            }
            (None, _) => Err(RelationalSourceExecutorError::InvalidExactDomain(format!(
                "finite collection has non-collection type `{collection_ty}`"
            ))),
        }
    }

    fn canonicalized_materialized(
        values: Vec<ExploreValue>,
        preserve_occurrence_support: bool,
    ) -> Result<Self, RelationalSourceExecutorError> {
        let mut unique = BTreeMap::<ExploreValue, Vec<u128>>::new();
        for (index, value) in values.into_iter().enumerate() {
            let ordinal = u128::try_from(index).map_err(|_| {
                RelationalSourceExecutorError::CanonicalLengthOverflow("fiber members")
            })?;
            unique.entry(value).or_default().push(ordinal);
        }

        let mut members = Vec::with_capacity(unique.len());
        for (canonical_index, (value, occurrence_ordinals)) in unique.into_iter().enumerate() {
            let canonical_ordinal = u128::try_from(canonical_index).map_err(|_| {
                RelationalSourceExecutorError::CanonicalLengthOverflow("canonical fiber members")
            })?;
            let raw_support_ordinals = if preserve_occurrence_support {
                occurrence_ordinals.into_boxed_slice()
            } else {
                vec![canonical_ordinal].into_boxed_slice()
            };
            members.push(MaterializedFiberMember {
                value,
                raw_support_ordinals,
            });
        }
        let cardinality = u128::try_from(members.len()).map_err(|_| {
            RelationalSourceExecutorError::CanonicalLengthOverflow("canonical fiber members")
        })?;
        Ok(Self {
            cardinality,
            kind: RelationalFiniteFiberKind::Materialized(members.into_boxed_slice()),
            origin: None,
        })
    }

    pub(crate) fn int_range(
        start: i64,
        end_exclusive: i64,
    ) -> Result<Self, RelationalSourceExecutorError> {
        if start > end_exclusive {
            return Err(RelationalSourceExecutorError::RangeStartAfterEnd {
                start,
                end_exclusive,
            });
        }
        let cardinality = u128::try_from(i128::from(end_exclusive) - i128::from(start))
            .map_err(|_| RelationalSourceExecutorError::IntegerRangeValueOverflow)?;
        Ok(Self {
            cardinality,
            kind: RelationalFiniteFiberKind::IntRange { start },
            origin: None,
        })
    }

    pub(crate) fn exact(
        domain: &ExploreExactDomain,
    ) -> Result<Self, RelationalSourceExecutorError> {
        match domain {
            ExploreExactDomain::Enumerated { values, source } => match source {
                ExploreEnumeratedSource::ExplicitList
                | ExploreEnumeratedSource::NamedList { .. } => {
                    Self::ordered_collection(values.clone())
                }
                ExploreEnumeratedSource::NamedSet { .. } => Self::semantic_set(values.clone()),
            },
            ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                cardinality,
            } => {
                let fiber = Self::int_range(*start, *end_exclusive)?;
                if fiber.cardinality != u128::from(*cardinality) {
                    return Err(RelationalSourceExecutorError::InvalidExactDomain(format!(
                        "integer range declares cardinality {} but its endpoints imply {}",
                        cardinality, fiber.cardinality
                    )));
                }
                Ok(fiber)
            }
            ExploreExactDomain::FiniteType { plan, .. } => {
                let cardinality = finite_type_exact_cardinality(plan)?;
                Ok(Self {
                    cardinality,
                    kind: RelationalFiniteFiberKind::FiniteType { plan: plan.clone() },
                    origin: None,
                })
            }
        }
    }

    fn with_origin(mut self, origin: RelationalFiberOrigin) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Compact commitment to the exact canonical members this fiber emits.
    ///
    /// Materialized members are hashed in their set-normalized value order.
    /// Ranges and finite-type universes commit their compact canonical decoder
    /// plans, so issuing an exhaustion receipt does not re-enumerate a large
    /// domain. Worker scheduling and journal arrival order are absent.
    pub(crate) fn canonical_member_commitment(
        &self,
    ) -> Result<[u8; 32], RelationalSourceExecutorError> {
        let mut hasher = ExhaustionHasher::new(SOURCE_FIBER_MEMBER_COMMITMENT_V1);
        hasher.u128(self.cardinality);
        match &self.kind {
            RelationalFiniteFiberKind::Materialized(members) => {
                hasher.tag(0x01);
                hasher.len(members.len());
                for member in members {
                    hasher.digest(canonical_explore_value_digest(&member.value));
                    hasher.len(member.raw_support_ordinals.len());
                    for ordinal in &member.raw_support_ordinals {
                        hasher.u128(*ordinal);
                    }
                }
            }
            RelationalFiniteFiberKind::IntRange { start } => {
                hasher.tag(0x02);
                hasher.i64(*start);
            }
            RelationalFiniteFiberKind::FiniteType { plan } => {
                hasher.tag(0x03);
                hash_finite_type_plan(&mut hasher, plan)?;
            }
        }
        Ok(hasher.finish())
    }
}

/// Semantic proof that the checked source executor reached the terminal member
/// of one exact binding fiber.
///
/// Fields are private and there is deliberately no general constructor. The
/// executor issues this value only after reopening the bound fiber and
/// observing its canonical exhaustion transition; a persisted cursor by itself
/// cannot mint semantic completion authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceBindingExhaustionReceipt {
    version: u32,
    id: SourceBindingExhaustionReceiptId,
    relation_id: RelationId,
    binding_index: u32,
    prefix_digest: [u8; 32],
    terminal_ordinal: u128,
    emitted_member_count: u128,
    emitted_members_commitment: [u8; 32],
}

impl SourceBindingExhaustionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        version: u32,
        relation_id: RelationId,
        binding_index: u32,
        prefix_digest: [u8; 32],
        terminal_ordinal: u128,
        emitted_member_count: u128,
        emitted_members_commitment: [u8; 32],
    ) -> Result<Self, RelationalSourceExecutorError> {
        let id = derive_source_binding_exhaustion_receipt_id(
            version,
            relation_id,
            binding_index,
            prefix_digest,
            terminal_ordinal,
            emitted_member_count,
            emitted_members_commitment,
        );
        let restored = Self {
            version,
            id,
            relation_id,
            binding_index,
            prefix_digest,
            terminal_ordinal,
            emitted_member_count,
            emitted_members_commitment,
        };
        restored.validate_identity()?;
        Ok(restored)
    }

    fn issue(
        relation_id: RelationId,
        cursor: &RelationalSourceCursor,
        fiber: &RelationalFiniteFiber,
    ) -> Result<Self, RelationalSourceExecutorError> {
        let expected_origin = RelationalFiberOrigin {
            relation_id,
            binding_index: cursor.binding_index,
            prefix_digest: cursor.prefix.canonical.digest(),
        };
        if fiber.origin != Some(expected_origin) {
            return Err(RelationalSourceExecutorError::FiberCursorMismatch);
        }
        let terminal_ordinal = cursor.next_member_ordinal;
        let emitted_member_count = fiber.cardinality;
        if terminal_ordinal != emitted_member_count {
            return Err(RelationalSourceExecutorError::ExhaustionBeforeTerminal {
                terminal_ordinal,
                cardinality: emitted_member_count,
            });
        }
        let origin = fiber
            .origin
            .expect("a source-bound fiber origin was checked above");
        let version = SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION;
        let emitted_members_commitment = fiber.canonical_member_commitment()?;
        let id = derive_source_binding_exhaustion_receipt_id(
            version,
            origin.relation_id,
            origin.binding_index,
            origin.prefix_digest,
            terminal_ordinal,
            emitted_member_count,
            emitted_members_commitment,
        );
        Ok(Self {
            version,
            id,
            relation_id: origin.relation_id,
            binding_index: origin.binding_index,
            prefix_digest: origin.prefix_digest,
            terminal_ordinal,
            emitted_member_count,
            emitted_members_commitment,
        })
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn id(&self) -> SourceBindingExhaustionReceiptId {
        self.id
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        self.binding_index
    }

    pub(crate) const fn prefix_digest(&self) -> [u8; 32] {
        self.prefix_digest
    }

    pub(crate) const fn terminal_ordinal(&self) -> u128 {
        self.terminal_ordinal
    }

    pub(crate) const fn emitted_member_count(&self) -> u128 {
        self.emitted_member_count
    }

    pub(crate) const fn emitted_members_commitment(&self) -> [u8; 32] {
        self.emitted_members_commitment
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RelationalSourceExecutorError> {
        if self.version != SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION {
            return Err(
                RelationalSourceExecutorError::UnsupportedExhaustionReceiptVersion {
                    actual: self.version,
                    expected: SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION,
                },
            );
        }
        if self.terminal_ordinal != self.emitted_member_count {
            return Err(
                RelationalSourceExecutorError::ExhaustionReceiptCountMismatch {
                    terminal_ordinal: self.terminal_ordinal,
                    emitted_member_count: self.emitted_member_count,
                },
            );
        }
        let derived = derive_source_binding_exhaustion_receipt_id(
            self.version,
            self.relation_id,
            self.binding_index,
            self.prefix_digest,
            self.terminal_ordinal,
            self.emitted_member_count,
            self.emitted_members_commitment,
        );
        if derived != self.id {
            return Err(RelationalSourceExecutorError::ExhaustionReceiptIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Authenticated selection of one canonical member in one binding fiber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalBindingSelection {
    pub(crate) binding_index: u32,
    pub(crate) canonical_ordinal: u128,
    pub(crate) parent_prefix_digest: [u8; 32],
    pub(crate) raw_support_ordinals: Box<[u128]>,
}

/// Codec-ready canonical source prefix. Values are the semantic evaluation
/// environment; selections retain duplicate support without changing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourcePrefixSnapshot {
    pub(crate) version: u32,
    pub(crate) values: Box<[ExploreValue]>,
    pub(crate) digest: [u8; 32],
    pub(crate) selections: Box<[RelationalBindingSelection]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalSourcePrefix {
    canonical: CanonicalSourcePrefix,
    selections: Box<[RelationalBindingSelection]>,
}

impl RelationalSourcePrefix {
    fn empty() -> Result<Self, RelationalSourceExecutorError> {
        Ok(Self {
            canonical: CanonicalSourcePrefix::from_values(Vec::new())?,
            selections: Box::new([]),
        })
    }

    fn snapshot(&self) -> RelationalSourcePrefixSnapshot {
        RelationalSourcePrefixSnapshot {
            version: RELATIONAL_SOURCE_CURSOR_VERSION,
            values: self.canonical.values().to_vec().into_boxed_slice(),
            digest: self.canonical.digest(),
            selections: self.selections.clone(),
        }
    }

    fn from_snapshot(
        snapshot: RelationalSourcePrefixSnapshot,
    ) -> Result<Self, RelationalSourceExecutorError> {
        if snapshot.version != RELATIONAL_SOURCE_CURSOR_VERSION {
            return Err(RelationalSourceExecutorError::UnsupportedCursorVersion {
                actual: snapshot.version,
                expected: RELATIONAL_SOURCE_CURSOR_VERSION,
            });
        }
        if snapshot.values.len() != snapshot.selections.len() {
            return Err(
                RelationalSourceExecutorError::PrefixSelectionCountMismatch {
                    values: snapshot.values.len(),
                    selections: snapshot.selections.len(),
                },
            );
        }
        let canonical = CanonicalSourcePrefix::from_values(snapshot.values.into_vec())?;
        if canonical.digest() != snapshot.digest {
            return Err(RelationalSourceExecutorError::PrefixDigestMismatch);
        }

        for (index, selection) in snapshot.selections.iter().enumerate() {
            let expected_index = u32::try_from(index).map_err(|_| {
                RelationalSourceExecutorError::CanonicalLengthOverflow("source selections")
            })?;
            if selection.binding_index != expected_index {
                return Err(
                    RelationalSourceExecutorError::SelectionBindingIndexMismatch {
                        actual: selection.binding_index,
                        expected: expected_index,
                    },
                );
            }
            if selection.raw_support_ordinals.is_empty()
                || selection
                    .raw_support_ordinals
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(RelationalSourceExecutorError::NonCanonicalSupportOrdinals(
                    selection.binding_index,
                ));
            }
            let parent = CanonicalSourcePrefix::from_values(canonical.values()[..index].to_vec())?;
            if selection.parent_prefix_digest != parent.digest() {
                return Err(RelationalSourceExecutorError::ParentPrefixDigestMismatch(
                    selection.binding_index,
                ));
            }
        }

        Ok(Self {
            canonical,
            selections: snapshot.selections,
        })
    }

    fn extend(
        &self,
        binding_index: u32,
        member: &RelationalFiberMember,
    ) -> Result<Self, RelationalSourceExecutorError> {
        let expected_index = u32::try_from(self.canonical.values().len())
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("source prefix"))?;
        if binding_index != expected_index {
            return Err(RelationalSourceExecutorError::CursorBindingIndexMismatch {
                cursor: binding_index,
                prefix_len: expected_index,
            });
        }

        let mut values = self.canonical.values().to_vec();
        values.push(member.value.clone());
        let canonical = CanonicalSourcePrefix::from_values(values)?;
        let mut selections = self.selections.to_vec();
        selections.push(RelationalBindingSelection {
            binding_index,
            canonical_ordinal: member.canonical_ordinal,
            parent_prefix_digest: self.canonical.digest(),
            raw_support_ordinals: member.raw_support_ordinals.clone(),
        });
        Ok(Self {
            canonical,
            selections: selections.into_boxed_slice(),
        })
    }

    fn canonicalize_value_storage(&mut self, visitor: &mut impl FnMut(&mut ExploreValue)) {
        let expected_digest = self.canonical.digest();
        let mut values = self.canonical.values().to_vec();
        for value in &mut values {
            visitor(value);
        }
        let canonical = CanonicalSourcePrefix::from_values(values)
            .expect("storage canonicalization preserves valid source-prefix values");
        debug_assert_eq!(canonical.digest(), expected_digest);
        self.canonical = canonical;
    }
}

/// Durable cursor for one `ExpandSourceBinding` work node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceCursor {
    binding_index: u32,
    prefix: RelationalSourcePrefix,
    next_member_ordinal: u128,
}

/// Codec-ready cursor state. It contains no evaluator cache, worker identity,
/// resource limit, or scheduler rank.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceCursorSnapshot {
    pub(crate) version: u32,
    pub(crate) binding_index: u32,
    pub(crate) prefix: RelationalSourcePrefixSnapshot,
    pub(crate) next_member_ordinal: u128,
}

impl RelationalSourceCursor {
    pub(super) fn restore_from_journal_codec(
        snapshot: RelationalSourceCursorSnapshot,
    ) -> Result<Self, RelationalSourceExecutorError> {
        Self::from_snapshot(snapshot)
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        self.binding_index
    }

    pub(crate) const fn next_member_ordinal(&self) -> u128 {
        self.next_member_ordinal
    }

    pub(crate) fn canonical_prefix(&self) -> &CanonicalSourcePrefix {
        &self.prefix.canonical
    }

    pub(crate) fn snapshot(&self) -> RelationalSourceCursorSnapshot {
        RelationalSourceCursorSnapshot {
            version: RELATIONAL_SOURCE_CURSOR_VERSION,
            binding_index: self.binding_index,
            prefix: self.prefix.snapshot(),
            next_member_ordinal: self.next_member_ordinal,
        }
    }

    fn from_snapshot(
        snapshot: RelationalSourceCursorSnapshot,
    ) -> Result<Self, RelationalSourceExecutorError> {
        if snapshot.version != RELATIONAL_SOURCE_CURSOR_VERSION {
            return Err(RelationalSourceExecutorError::UnsupportedCursorVersion {
                actual: snapshot.version,
                expected: RELATIONAL_SOURCE_CURSOR_VERSION,
            });
        }
        let prefix = RelationalSourcePrefix::from_snapshot(snapshot.prefix)?;
        let prefix_len = u32::try_from(prefix.canonical.values().len())
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("source prefix"))?;
        if snapshot.binding_index != prefix_len {
            return Err(RelationalSourceExecutorError::CursorBindingIndexMismatch {
                cursor: snapshot.binding_index,
                prefix_len,
            });
        }
        Ok(Self {
            binding_index: snapshot.binding_index,
            prefix,
            next_member_ordinal: snapshot.next_member_ordinal,
        })
    }

    fn with_next_member_ordinal(&self, next_member_ordinal: u128) -> Self {
        Self {
            binding_index: self.binding_index,
            prefix: self.prefix.clone(),
            next_member_ordinal,
        }
    }

    fn canonicalize_value_storage(&mut self, visitor: &mut impl FnMut(&mut ExploreValue)) {
        self.prefix.canonicalize_value_storage(visitor);
    }
}

/// One completed semantic source row and the full authenticated producer
/// prefix that yielded it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCompletedSource {
    source_key: SourceKey,
    row: SourceRow,
    prefix: RelationalSourcePrefixSnapshot,
}

impl RelationalCompletedSource {
    pub(super) fn restore_from_journal_codec(
        relation_id: RelationId,
        row: SourceRow,
        prefix: RelationalSourcePrefixSnapshot,
    ) -> Result<Self, RelationalSourceExecutorError> {
        let prefix = RelationalSourcePrefix::from_snapshot(prefix)?.snapshot();
        let source_key = SourceKey::derive(relation_id, &row);
        Ok(Self {
            source_key,
            row,
            prefix,
        })
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) fn row(&self) -> &SourceRow {
        &self.row
    }

    pub(crate) fn prefix(&self) -> &RelationalSourcePrefixSnapshot {
        &self.prefix
    }

    fn canonicalize_value_storage(
        &mut self,
        relation_id: RelationId,
        visitor: &mut impl FnMut(&mut ExploreValue),
    ) {
        let mut context = self.row.context().clone();
        let mut before = self.row.before().clone();
        visitor(&mut context);
        visitor(&mut before);
        let row = SourceRow::new(context, before, self.row.provenance().clone());
        let derived = SourceKey::derive(relation_id, &row);
        debug_assert_eq!(derived, self.source_key);
        for value in &mut self.prefix.values {
            visitor(value);
        }
        let reconstructed = RelationalSourcePrefix::from_snapshot(self.prefix.clone())
            .expect("storage canonicalization preserves a valid completed source prefix")
            .snapshot();
        self.row = row;
        self.prefix = reconstructed;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSourceContinuation {
    Expand(RelationalSourceCursor),
    Source(RelationalCompletedSource),
}

/// Result of one ordinal step. `resume` advances only the current binding;
/// `continuation` is independently publishable child work or a source row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSourceAdvance {
    Yielded {
        member: RelationalFiberMember,
        resume: RelationalSourceCursor,
        continuation: RelationalSourceContinuation,
    },
    Exhausted {
        cursor: RelationalSourceCursor,
        cardinality: u128,
        receipt: SourceBindingExhaustionReceipt,
    },
}

impl RelationalSourceAdvance {
    /// Rewrite only process-local value storage while preserving every
    /// semantic cursor, row, and content identity. The journal fold uses this
    /// to recover constructor sharing after decoding independent frames.
    pub(crate) fn canonicalize_value_storage(
        &mut self,
        relation_id: RelationId,
        visitor: &mut impl FnMut(&mut ExploreValue),
    ) {
        match self {
            Self::Yielded {
                member,
                resume,
                continuation,
            } => {
                visitor(&mut member.value);
                resume.canonicalize_value_storage(visitor);
                match continuation {
                    RelationalSourceContinuation::Expand(cursor) => {
                        cursor.canonicalize_value_storage(visitor);
                    }
                    RelationalSourceContinuation::Source(source) => {
                        source.canonicalize_value_storage(relation_id, visitor);
                    }
                }
            }
            Self::Exhausted { cursor, .. } => {
                cursor.canonicalize_value_storage(visitor);
            }
        }
    }
}

/// Lazy ordered dependent-FROM enumerator.
pub(crate) struct RelationalSourceEnumerator<'a> {
    relation_id: RelationId,
    relation: &'a ExploreSourceRelationIr,
}

impl<'a> RelationalSourceEnumerator<'a> {
    pub(crate) fn new(
        relation_id: RelationId,
        relation: &'a ExploreSourceRelationIr,
    ) -> Result<Self, RelationalSourceExecutorError> {
        validate_source_relation(relation)?;
        Ok(Self {
            relation_id,
            relation,
        })
    }

    pub(crate) fn root_cursor(
        &self,
    ) -> Result<RelationalSourceCursor, RelationalSourceExecutorError> {
        Ok(RelationalSourceCursor {
            binding_index: 0,
            prefix: RelationalSourcePrefix::empty()?,
            next_member_ordinal: 0,
        })
    }

    /// Materialize one exact independent assignment without publishing the
    /// ordinary source-traversal path.
    ///
    /// `finite_ordinals` names one canonical member ordinal for every finite
    /// FROM binding, in binding order. Singleton bindings are replayed through
    /// the checked runtime at their sole member. This is the narrow producer
    /// seam used by an exhaustive support-cell sweep: the caller still has to
    /// prove that its coordinate tuple belongs to the installed support plan,
    /// and this method deliberately issues no traversal or exhaustion receipt.
    pub(crate) fn completed_source_at_independent_finite_ordinals<
        R: RelationalExpressionRuntime,
    >(
        &self,
        finite_ordinals: &[u128],
        runtime: &mut R,
    ) -> Result<RelationalCompletedSource, RelationalSourceExecutorError> {
        let expected_finite = self
            .relation
            .bindings
            .iter()
            .filter(|binding| matches!(&binding.kind, ExploreSourceBindingKindIr::Finite { .. }))
            .count();
        if finite_ordinals.len() != expected_finite {
            return Err(
                RelationalSourceExecutorError::IndependentAssignmentArityMismatch {
                    expected: expected_finite,
                    actual: finite_ordinals.len(),
                },
            );
        }

        let mut cursor = self.root_cursor()?;
        let mut finite_index = 0usize;
        loop {
            let binding_index = usize::try_from(cursor.binding_index).map_err(|_| {
                RelationalSourceExecutorError::CanonicalLengthOverflow("binding index")
            })?;
            let binding = self.relation.bindings.get(binding_index).ok_or(
                RelationalSourceExecutorError::CursorHasNoBinding(cursor.binding_index),
            )?;
            let member_ordinal = match &binding.kind {
                ExploreSourceBindingKindIr::Singleton { .. } => 0,
                ExploreSourceBindingKindIr::Finite { .. } => {
                    let ordinal = *finite_ordinals.get(finite_index).ok_or(
                        RelationalSourceExecutorError::IndependentAssignmentArityMismatch {
                            expected: expected_finite,
                            actual: finite_ordinals.len(),
                        },
                    )?;
                    finite_index = finite_index.checked_add(1).ok_or(
                        RelationalSourceExecutorError::CanonicalLengthOverflow(
                            "finite assignment index",
                        ),
                    )?;
                    ordinal
                }
            };

            let positioned = cursor.with_next_member_ordinal(member_ordinal);
            let fiber = self.binding_fiber(&positioned, runtime)?;
            let RelationalSourceAdvance::Yielded { continuation, .. } =
                self.advance_in_fiber(&positioned, &fiber)?
            else {
                return Err(
                    RelationalSourceExecutorError::IndependentAssignmentMemberAbsent {
                        binding_index: cursor.binding_index,
                        ordinal: member_ordinal,
                    },
                );
            };
            match continuation {
                RelationalSourceContinuation::Expand(next) => cursor = next,
                RelationalSourceContinuation::Source(source) => {
                    if finite_index != finite_ordinals.len() {
                        return Err(
                            RelationalSourceExecutorError::IndependentAssignmentArityMismatch {
                                expected: finite_index,
                                actual: finite_ordinals.len(),
                            },
                        );
                    }
                    return Ok(source);
                }
            }
        }
    }

    pub(crate) fn work_spec(
        &self,
        cursor: &RelationalSourceCursor,
    ) -> Result<WorkNodeSpec, RelationalSourceExecutorError> {
        self.validate_cursor(cursor)?;
        Ok(WorkNodeSpec::ExpandSourceBinding {
            relation_id: self.relation_id,
            binding_index: cursor.binding_index,
            prefix: cursor.prefix.canonical.clone(),
        })
    }

    /// Restore a codec snapshot only after replaying its prefix through the
    /// checked domains. Structurally plausible but semantically invented
    /// lineage/support is rejected rather than trusted as resume authority.
    pub(crate) fn resume_snapshot<R: RelationalExpressionRuntime>(
        &self,
        snapshot: RelationalSourceCursorSnapshot,
        runtime: &mut R,
    ) -> Result<RelationalSourceCursor, RelationalSourceExecutorError> {
        let claimed = RelationalSourceCursor::from_snapshot(snapshot.clone())?;
        let work_spec = self.work_spec(&claimed)?;
        let reconstructed = self.resume_cursor(&work_spec, claimed.next_member_ordinal, runtime)?;
        if reconstructed.snapshot() != snapshot {
            return Err(RelationalSourceExecutorError::CursorSnapshotProvenanceMismatch);
        }
        Ok(reconstructed)
    }

    /// Reconstruct a durable cursor from exactly the semantic prefix stored in
    /// `ExpandSourceBinding` plus the frontier's next-member ordinal. Any
    /// duplicate support hidden behind an earlier set member is deterministically
    /// recovered by reopening that member's checked fiber.
    pub(crate) fn resume_cursor<R: RelationalExpressionRuntime>(
        &self,
        work_spec: &WorkNodeSpec,
        next_member_ordinal: u128,
        runtime: &mut R,
    ) -> Result<RelationalSourceCursor, RelationalSourceExecutorError> {
        self.resume_cursor_with_fiber(work_spec, next_member_ordinal, runtime)
            .map(|(cursor, _)| cursor)
    }

    /// Reconstruct and validate one durable cursor while retaining the exact
    /// current fiber opened for that validation. A bounded worker can advance
    /// several ordinals through the returned fiber without re-evaluating or
    /// re-canonicalizing its domain; neither value is durable authority beyond
    /// the checked work subject and cursor supplied here.
    pub(crate) fn resume_cursor_with_fiber<R: RelationalExpressionRuntime>(
        &self,
        work_spec: &WorkNodeSpec,
        next_member_ordinal: u128,
        runtime: &mut R,
    ) -> Result<(RelationalSourceCursor, RelationalFiniteFiber), RelationalSourceExecutorError>
    {
        let WorkNodeSpec::ExpandSourceBinding {
            relation_id,
            binding_index,
            prefix,
        } = work_spec
        else {
            return Err(RelationalSourceExecutorError::NotSourceBindingWork);
        };
        if *relation_id != self.relation_id {
            return Err(RelationalSourceExecutorError::WorkRelationMismatch);
        }
        let prefix_len = u32::try_from(prefix.values().len())
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("source prefix"))?;
        if *binding_index != prefix_len {
            return Err(RelationalSourceExecutorError::CursorBindingIndexMismatch {
                cursor: *binding_index,
                prefix_len,
            });
        }

        let mut reconstructed = RelationalSourcePrefix::empty()?;
        for (index, expected_value) in prefix.values().iter().enumerate() {
            let binding_index = u32::try_from(index).map_err(|_| {
                RelationalSourceExecutorError::CanonicalLengthOverflow("binding index")
            })?;
            let cursor = RelationalSourceCursor {
                binding_index,
                prefix: reconstructed.clone(),
                next_member_ordinal: 0,
            };
            let fiber = self.binding_fiber(&cursor, runtime)?;
            let member = fiber.member_for_value(expected_value)?.ok_or(
                RelationalSourceExecutorError::PrefixMemberAbsent(binding_index),
            )?;
            reconstructed = reconstructed.extend(binding_index, &member)?;
        }
        if reconstructed.canonical.digest() != prefix.digest() {
            return Err(RelationalSourceExecutorError::PrefixDigestMismatch);
        }
        let cursor = RelationalSourceCursor {
            binding_index: *binding_index,
            prefix: reconstructed,
            next_member_ordinal,
        };
        self.validate_cursor(&cursor)?;
        let fiber = self.binding_fiber(&cursor, runtime)?;
        let cardinality = fiber.cardinality();
        if next_member_ordinal > cardinality {
            return Err(RelationalSourceExecutorError::OrdinalBeyondCardinality {
                ordinal: next_member_ordinal,
                cardinality,
            });
        }
        Ok((cursor, fiber))
    }

    pub(crate) fn binding_fiber<R: RelationalExpressionRuntime>(
        &self,
        cursor: &RelationalSourceCursor,
        runtime: &mut R,
    ) -> Result<RelationalFiniteFiber, RelationalSourceExecutorError> {
        self.validate_cursor(cursor)?;
        let binding_index = usize::try_from(cursor.binding_index)
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("binding index"))?;
        let binding = &self.relation.bindings[binding_index];
        let earlier_bindings = self.relation.bindings[..binding_index]
            .iter()
            .zip(cursor.prefix.canonical.values())
            .map(|(binding, value)| RelationalBoundValue {
                name: binding.name.as_str(),
                value,
            })
            .collect::<Vec<_>>();

        let fiber = match &binding.kind {
            ExploreSourceBindingKindIr::Singleton { value } => runtime
                .evaluate(value, &binding.value_ty, &earlier_bindings)
                .map(RelationalFiniteFiber::singleton)
                .map_err(|message| RelationalSourceExecutorError::Evaluation {
                    binding_index: cursor.binding_index,
                    binding_name: binding.name.clone(),
                    phase: "singleton",
                    message,
                }),
            ExploreSourceBindingKindIr::Finite { domain } => self.evaluate_finite_domain(
                cursor.binding_index,
                &binding.name,
                domain,
                &earlier_bindings,
                runtime,
            ),
        }?;
        Ok(fiber.with_origin(RelationalFiberOrigin {
            relation_id: self.relation_id,
            binding_index: cursor.binding_index,
            prefix_digest: cursor.prefix.canonical.digest(),
        }))
    }

    /// Advance exactly one canonical member, or report canonical exhaustion.
    pub(crate) fn advance<R: RelationalExpressionRuntime>(
        &self,
        cursor: &RelationalSourceCursor,
        runtime: &mut R,
    ) -> Result<RelationalSourceAdvance, RelationalSourceExecutorError> {
        let fiber = self.binding_fiber(cursor, runtime)?;
        self.advance_in_fiber(cursor, &fiber)
    }

    /// Advance using one already-opened fiber so a worker can enumerate many
    /// members without re-evaluating or re-canonicalizing a collection. The
    /// fiber is bound to its relation, binding index, and prefix and cannot be
    /// replayed against different work.
    pub(crate) fn advance_in_fiber(
        &self,
        cursor: &RelationalSourceCursor,
        fiber: &RelationalFiniteFiber,
    ) -> Result<RelationalSourceAdvance, RelationalSourceExecutorError> {
        self.validate_cursor(cursor)?;
        let expected_origin = RelationalFiberOrigin {
            relation_id: self.relation_id,
            binding_index: cursor.binding_index,
            prefix_digest: cursor.prefix.canonical.digest(),
        };
        if fiber.origin != Some(expected_origin) {
            return Err(RelationalSourceExecutorError::FiberCursorMismatch);
        }
        let Some(member) = fiber.member_at_ordinal(cursor.next_member_ordinal)? else {
            let receipt = SourceBindingExhaustionReceipt::issue(self.relation_id, cursor, fiber)?;
            return Ok(RelationalSourceAdvance::Exhausted {
                cursor: cursor.clone(),
                cardinality: fiber.cardinality(),
                receipt,
            });
        };
        let next_member_ordinal = cursor
            .next_member_ordinal
            .checked_add(1)
            .ok_or(RelationalSourceExecutorError::CursorOrdinalOverflow)?;
        let resume = cursor.with_next_member_ordinal(next_member_ordinal);
        let next_prefix = cursor.prefix.extend(cursor.binding_index, &member)?;
        let next_binding_index = cursor.binding_index.checked_add(1).ok_or(
            RelationalSourceExecutorError::CanonicalLengthOverflow("binding index"),
        )?;
        let continuation = if usize::try_from(next_binding_index)
            .ok()
            .is_some_and(|index| index == self.relation.bindings.len())
        {
            RelationalSourceContinuation::Source(self.completed_source(next_prefix)?)
        } else {
            RelationalSourceContinuation::Expand(RelationalSourceCursor {
                binding_index: next_binding_index,
                prefix: next_prefix,
                next_member_ordinal: 0,
            })
        };
        Ok(RelationalSourceAdvance::Yielded {
            member,
            resume,
            continuation,
        })
    }

    fn validate_cursor(
        &self,
        cursor: &RelationalSourceCursor,
    ) -> Result<(), RelationalSourceExecutorError> {
        let prefix_len = u32::try_from(cursor.prefix.canonical.values().len())
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("source prefix"))?;
        if cursor.binding_index != prefix_len {
            return Err(RelationalSourceExecutorError::CursorBindingIndexMismatch {
                cursor: cursor.binding_index,
                prefix_len,
            });
        }
        let binding_index = usize::try_from(cursor.binding_index)
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("binding index"))?;
        if binding_index >= self.relation.bindings.len() {
            return Err(RelationalSourceExecutorError::CursorHasNoBinding(
                cursor.binding_index,
            ));
        }
        Ok(())
    }

    fn evaluate_finite_domain<R: RelationalExpressionRuntime>(
        &self,
        binding_index: u32,
        binding_name: &str,
        domain: &ExploreFiniteDomainIr,
        earlier_bindings: &[RelationalBoundValue<'_>],
        runtime: &mut R,
    ) -> Result<RelationalFiniteFiber, RelationalSourceExecutorError> {
        match domain {
            ExploreFiniteDomainIr::Exact(domain) => RelationalFiniteFiber::exact(domain),
            ExploreFiniteDomainIr::Collection {
                expression,
                collection_ty,
                ..
            } => {
                let value = runtime
                    .evaluate(expression, collection_ty, earlier_bindings)
                    .map_err(|message| RelationalSourceExecutorError::Evaluation {
                        binding_index,
                        binding_name: binding_name.to_string(),
                        phase: "finite collection",
                        message,
                    })?;
                RelationalFiniteFiber::from_collection_value(collection_ty, value).map_err(
                    |error| match error {
                        RelationalSourceExecutorError::InvalidExactDomain(message) => {
                            RelationalSourceExecutorError::InvalidExactDomain(format!(
                                "binding `{binding_name}` {message}"
                            ))
                        }
                        error => error,
                    },
                )
            }
            ExploreFiniteDomainIr::IntRange {
                start,
                end_exclusive,
            } => {
                let int_ty = Ty::Name("Int".to_string());
                let start =
                    runtime
                        .evaluate(start, &int_ty, earlier_bindings)
                        .map_err(|message| RelationalSourceExecutorError::Evaluation {
                            binding_index,
                            binding_name: binding_name.to_string(),
                            phase: "range start",
                            message,
                        })?;
                let end_exclusive = runtime
                    .evaluate(end_exclusive, &int_ty, earlier_bindings)
                    .map_err(|message| RelationalSourceExecutorError::Evaluation {
                        binding_index,
                        binding_name: binding_name.to_string(),
                        phase: "range end",
                        message,
                    })?;
                let ExploreValue::Int(start) = start else {
                    return Err(RelationalSourceExecutorError::ExpectedInt("range start"));
                };
                let ExploreValue::Int(end_exclusive) = end_exclusive else {
                    return Err(RelationalSourceExecutorError::ExpectedInt("range end"));
                };
                RelationalFiniteFiber::int_range(start, end_exclusive)
            }
        }
    }

    fn completed_source(
        &self,
        prefix: RelationalSourcePrefix,
    ) -> Result<RelationalCompletedSource, RelationalSourceExecutorError> {
        let context = prefix
            .canonical
            .values()
            .get(self.relation.context_binding_index)
            .cloned()
            .ok_or(RelationalSourceExecutorError::MissingContextValue)?;
        let before = prefix
            .canonical
            .values()
            .get(self.relation.before_binding_index)
            .cloned()
            .ok_or(RelationalSourceExecutorError::MissingBeforeValue)?;
        let provenance = source_provenance(self.relation_id, &prefix)?;
        let row = SourceRow::new(context, before, provenance);
        let source_key = SourceKey::derive(self.relation_id, &row);
        Ok(RelationalCompletedSource {
            source_key,
            row,
            prefix: prefix.snapshot(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectionTypeKind {
    List,
    Set,
}

fn collection_type_kind(ty: &Ty) -> Option<CollectionTypeKind> {
    let Ty::App(base, arguments) = ty else {
        return None;
    };
    if arguments.len() != 1 {
        return None;
    }
    match base.as_ref() {
        Ty::Name(name) if name == "List" => Some(CollectionTypeKind::List),
        Ty::Name(name) if name == "Set" => Some(CollectionTypeKind::Set),
        _ => None,
    }
}

fn validate_source_relation(
    relation: &ExploreSourceRelationIr,
) -> Result<(), RelationalSourceExecutorError> {
    if relation.normalization_version != EXPLORE_RELATION_NORMALIZATION_VERSION {
        return Err(RelationalSourceExecutorError::InvalidSourceRelation(
            format!(
                "normalization version {} is unsupported",
                relation.normalization_version
            ),
        ));
    }
    if !matches!(
        relation.multiplicity,
        ExploreRelationMultiplicity::SetNormalized
    ) {
        return Err(RelationalSourceExecutorError::InvalidSourceRelation(
            "source multiplicity is not set-normalized".to_string(),
        ));
    }
    if relation.bindings.is_empty() {
        return Err(RelationalSourceExecutorError::InvalidSourceRelation(
            "source relation has no bindings".to_string(),
        ));
    }
    let _ = u32::try_from(relation.bindings.len())
        .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("source bindings"))?;

    let mut names = BTreeSet::new();
    let mut context_count = 0usize;
    let mut before_count = 0usize;
    for (expected, binding) in relation.bindings.iter().enumerate() {
        if binding.binding_index != expected {
            return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                format!(
                    "binding `{}` has index {}, expected {}",
                    binding.name, binding.binding_index, expected
                ),
            ));
        }
        if !names.insert(binding.name.as_str()) {
            return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                format!("duplicate source binding `{}`", binding.name),
            ));
        }
        let mut previous = None;
        for dependency in binding.dependencies.iter() {
            if dependency.binding_index >= expected
                || previous.is_some_and(|index| dependency.binding_index <= index)
            {
                return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                    format!(
                        "binding `{}` has a non-canonical dependency index {}",
                        binding.name, dependency.binding_index
                    ),
                ));
            }
            if relation.bindings[dependency.binding_index].name != dependency.binding_name {
                return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                    format!(
                        "binding `{}` dependency `{}` resolves to a different binding",
                        binding.name, dependency.binding_name
                    ),
                ));
            }
            previous = Some(dependency.binding_index);
        }
        match binding.role {
            ExploreSourceBindingRoleIr::Auxiliary => {}
            ExploreSourceBindingRoleIr::Context => {
                context_count += 1;
                if expected != relation.context_binding_index
                    || !relational_tys_equivalent(&binding.value_ty, &relation.context_ty)
                {
                    return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                        "Context role does not match its resolved index/type".to_string(),
                    ));
                }
            }
            ExploreSourceBindingRoleIr::Before => {
                before_count += 1;
                if expected != relation.before_binding_index
                    || !relational_tys_equivalent(&binding.value_ty, &relation.before_ty)
                {
                    return Err(RelationalSourceExecutorError::InvalidSourceRelation(
                        "Before role does not match its resolved index/type".to_string(),
                    ));
                }
            }
        }
    }
    if context_count != 1 || before_count != 1 {
        return Err(RelationalSourceExecutorError::InvalidSourceRelation(
            format!(
                "source relation requires one Context and one Before role; found {context_count} and {before_count}"
            ),
        ));
    }
    Ok(())
}

fn source_provenance(
    relation_id: RelationId,
    prefix: &RelationalSourcePrefix,
) -> Result<RelationProvenance, RelationalSourceExecutorError> {
    let mut lineage_preimage = CanonicalBytes::new(SOURCE_LINEAGE_PREIMAGE_V1);
    lineage_preimage.u32(RELATIONAL_SOURCE_CURSOR_VERSION);
    lineage_preimage.digest(relation_id.bytes());
    lineage_preimage.digest(prefix.canonical.digest());
    let lineage = RelationLineageId::from_canonical_preimage(lineage_preimage.as_slice());

    let mut support = BTreeSet::new();
    for selection in prefix.selections.iter() {
        let value_index = usize::try_from(selection.binding_index)
            .map_err(|_| RelationalSourceExecutorError::CanonicalLengthOverflow("binding index"))?;
        let value = prefix.canonical.values().get(value_index).ok_or(
            RelationalSourceExecutorError::SelectionValueMissing(selection.binding_index),
        )?;
        for raw_ordinal in selection.raw_support_ordinals.iter().copied() {
            let mut preimage = CanonicalBytes::new(SOURCE_SUPPORT_PREIMAGE_V1);
            preimage.u32(RELATIONAL_SOURCE_CURSOR_VERSION);
            preimage.digest(relation_id.bytes());
            preimage.u32(selection.binding_index);
            preimage.digest(selection.parent_prefix_digest);
            preimage.u128(selection.canonical_ordinal);
            preimage.u128(raw_ordinal);
            preimage.digest(canonical_explore_value_digest(value));
            support.insert(RelationSupportId::from_canonical_preimage(
                preimage.as_slice(),
            ));
        }
    }
    Ok(RelationProvenance::new([lineage], support))
}

fn finite_type_exact_cardinality(
    plan: &ExploreFiniteTypePlan,
) -> Result<u128, RelationalSourceExecutorError> {
    let computed = match plan {
        ExploreFiniteTypePlan::Unit => 1,
        ExploreFiniteTypePlan::Bool => 2,
        ExploreFiniteTypePlan::Tuple {
            elements,
            cardinality,
        } => {
            let computed = checked_product(
                elements.iter().map(finite_type_exact_cardinality),
                "finite tuple",
            )?;
            validate_declared_cardinality(cardinality, computed, "finite tuple")?;
            computed
        }
        ExploreFiniteTypePlan::Sum {
            variants,
            cardinality,
            ..
        } => {
            let mut computed = 0u128;
            for variant in variants {
                let variant_cardinality = checked_product(
                    variant
                        .fields
                        .iter()
                        .map(|field| finite_type_exact_cardinality(&field.plan)),
                    "finite variant",
                )?;
                computed = computed.checked_add(variant_cardinality).ok_or(
                    RelationalSourceExecutorError::CardinalityExceedsU128("finite sum"),
                )?;
            }
            validate_declared_cardinality(cardinality, computed, "finite sum")?;
            computed
        }
    };
    Ok(computed)
}

fn validate_declared_cardinality(
    declared: &ExploreCardinality,
    computed: u128,
    subject: &'static str,
) -> Result<(), RelationalSourceExecutorError> {
    match declared {
        ExploreCardinality::Exact(value) if *value == computed => Ok(()),
        ExploreCardinality::Exact(value) => Err(RelationalSourceExecutorError::InvalidExactDomain(
            format!("{subject} declares cardinality {value} but its plan implies {computed}"),
        )),
        ExploreCardinality::ExceedsU128 => {
            Err(RelationalSourceExecutorError::InvalidExactDomain(format!(
                "{subject} declares an overflowing cardinality but its plan implies {computed}"
            )))
        }
    }
}

fn checked_product<I>(
    cardinalities: I,
    subject: &'static str,
) -> Result<u128, RelationalSourceExecutorError>
where
    I: IntoIterator<Item = Result<u128, RelationalSourceExecutorError>>,
{
    let mut product = 1u128;
    for cardinality in cardinalities {
        product = product.checked_mul(cardinality?).ok_or(
            RelationalSourceExecutorError::CardinalityExceedsU128(subject),
        )?;
    }
    Ok(product)
}

fn finite_type_value_at(
    plan: &ExploreFiniteTypePlan,
    ordinal: u128,
) -> Result<ExploreValue, RelationalSourceExecutorError> {
    let cardinality = finite_type_exact_cardinality(plan)?;
    if ordinal >= cardinality {
        return Err(RelationalSourceExecutorError::OrdinalBeyondCardinality {
            ordinal,
            cardinality,
        });
    }
    match plan {
        ExploreFiniteTypePlan::Unit => Ok(ExploreValue::Unit),
        ExploreFiniteTypePlan::Bool => Ok(ExploreValue::Boolean(ordinal == 1)),
        ExploreFiniteTypePlan::Tuple { elements, .. } => {
            let cardinalities = elements
                .iter()
                .map(finite_type_exact_cardinality)
                .collect::<Result<Vec<_>, _>>()?;
            let ordinals = unrank_product(&cardinalities, ordinal)?;
            elements
                .iter()
                .zip(ordinals)
                .map(|(element, ordinal)| finite_type_value_at(element, ordinal))
                .collect::<Result<Vec<_>, _>>()
                .map(ExploreValue::Tuple)
        }
        ExploreFiniteTypePlan::Sum {
            type_name,
            variants,
            ..
        } => {
            let mut remainder = ordinal;
            for variant in variants {
                let cardinalities = variant
                    .fields
                    .iter()
                    .map(|field| finite_type_exact_cardinality(&field.plan))
                    .collect::<Result<Vec<_>, _>>()?;
                let variant_cardinality =
                    cardinalities.iter().try_fold(1u128, |product, value| {
                        product.checked_mul(*value).ok_or(
                            RelationalSourceExecutorError::CardinalityExceedsU128("finite variant"),
                        )
                    })?;
                if remainder >= variant_cardinality {
                    remainder -= variant_cardinality;
                    continue;
                }
                let ordinals = unrank_product(&cardinalities, remainder)?;
                let fields = variant
                    .fields
                    .iter()
                    .zip(ordinals)
                    .map(|(field, ordinal)| {
                        finite_type_value_at(&field.plan, ordinal)
                            .map(|value| (field.name.clone(), value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(ExploreValue::Constructor {
                    type_name: type_name.clone(),
                    variant: variant.name.clone(),
                    positional: variant.positional,
                    fields: fields.into(),
                });
            }
            Err(RelationalSourceExecutorError::InvalidExactDomain(format!(
                "finite sum ordinal {ordinal} selected no variant"
            )))
        }
    }
}

fn finite_type_ordinal_of(
    plan: &ExploreFiniteTypePlan,
    value: &ExploreValue,
) -> Result<Option<u128>, RelationalSourceExecutorError> {
    let _ = finite_type_exact_cardinality(plan)?;
    match (plan, value) {
        (ExploreFiniteTypePlan::Unit, ExploreValue::Unit) => Ok(Some(0)),
        (ExploreFiniteTypePlan::Unit, _) => Ok(None),
        (ExploreFiniteTypePlan::Bool, ExploreValue::Boolean(false)) => Ok(Some(0)),
        (ExploreFiniteTypePlan::Bool, ExploreValue::Boolean(true)) => Ok(Some(1)),
        (ExploreFiniteTypePlan::Bool, _) => Ok(None),
        (ExploreFiniteTypePlan::Tuple { elements, .. }, ExploreValue::Tuple(values))
            if elements.len() == values.len() =>
        {
            let cardinalities = elements
                .iter()
                .map(finite_type_exact_cardinality)
                .collect::<Result<Vec<_>, _>>()?;
            let mut ordinals = Vec::with_capacity(elements.len());
            for (element, value) in elements.iter().zip(values) {
                let Some(ordinal) = finite_type_ordinal_of(element, value)? else {
                    return Ok(None);
                };
                ordinals.push(ordinal);
            }
            rank_product(&cardinalities, &ordinals).map(Some)
        }
        (ExploreFiniteTypePlan::Tuple { .. }, _) => Ok(None),
        (
            ExploreFiniteTypePlan::Sum {
                type_name,
                variants,
                ..
            },
            ExploreValue::Constructor {
                type_name: value_type,
                variant: value_variant,
                positional,
                fields: value_fields,
            },
        ) if type_name == runtime_nominal_declared_type_name(value_type) => {
            let mut preceding = 0u128;
            for variant in variants {
                let cardinalities = variant
                    .fields
                    .iter()
                    .map(|field| finite_type_exact_cardinality(&field.plan))
                    .collect::<Result<Vec<_>, _>>()?;
                let cardinality = cardinalities.iter().try_fold(1u128, |product, value| {
                    product.checked_mul(*value).ok_or(
                        RelationalSourceExecutorError::CardinalityExceedsU128("finite variant"),
                    )
                })?;
                if variant.name.as_str() != value_variant.as_str() {
                    preceding = preceding.checked_add(cardinality).ok_or(
                        RelationalSourceExecutorError::CardinalityExceedsU128("finite sum"),
                    )?;
                    continue;
                }
                if variant.positional != *positional || variant.fields.len() != value_fields.len() {
                    return Ok(None);
                }
                let mut ordinals = Vec::with_capacity(variant.fields.len());
                for (field, (value_name, value)) in variant.fields.iter().zip(value_fields.iter()) {
                    if !variant.positional && field.name.as_str() != value_name.as_str() {
                        return Ok(None);
                    }
                    let Some(ordinal) = finite_type_ordinal_of(&field.plan, value)? else {
                        return Ok(None);
                    };
                    ordinals.push(ordinal);
                }
                return preceding
                    .checked_add(rank_product(&cardinalities, &ordinals)?)
                    .ok_or(RelationalSourceExecutorError::CardinalityExceedsU128(
                        "finite sum",
                    ))
                    .map(Some);
            }
            Ok(None)
        }
        (ExploreFiniteTypePlan::Sum { .. }, _) => Ok(None),
    }
}

fn rank_product(
    cardinalities: &[u128],
    ordinals: &[u128],
) -> Result<u128, RelationalSourceExecutorError> {
    if cardinalities.len() != ordinals.len() {
        return Err(RelationalSourceExecutorError::InvalidExactDomain(
            "finite product rank has a different coordinate count".to_string(),
        ));
    }
    let mut rank = 0u128;
    for (cardinality, ordinal) in cardinalities.iter().zip(ordinals) {
        if *ordinal >= *cardinality {
            return Err(RelationalSourceExecutorError::OrdinalBeyondCardinality {
                ordinal: *ordinal,
                cardinality: *cardinality,
            });
        }
        rank = rank
            .checked_mul(*cardinality)
            .and_then(|rank| rank.checked_add(*ordinal))
            .ok_or(RelationalSourceExecutorError::CardinalityExceedsU128(
                "finite product",
            ))?;
    }
    Ok(rank)
}

fn unrank_product(
    cardinalities: &[u128],
    ordinal: u128,
) -> Result<Vec<u128>, RelationalSourceExecutorError> {
    let product = cardinalities
        .iter()
        .try_fold(1u128, |product, cardinality| {
            product.checked_mul(*cardinality).ok_or(
                RelationalSourceExecutorError::CardinalityExceedsU128("finite product"),
            )
        })?;
    if ordinal >= product {
        return Err(RelationalSourceExecutorError::OrdinalBeyondCardinality {
            ordinal,
            cardinality: product,
        });
    }
    let mut remainder = ordinal;
    let mut ordinals = vec![0u128; cardinalities.len()];
    for index in (0..cardinalities.len()).rev() {
        let cardinality = cardinalities[index];
        if cardinality == 0 {
            return Err(RelationalSourceExecutorError::InvalidExactDomain(
                "finite product contains a zero-cardinality component".to_string(),
            ));
        }
        ordinals[index] = remainder % cardinality;
        remainder /= cardinality;
    }
    Ok(ordinals)
}

fn hash_finite_type_plan(
    hasher: &mut ExhaustionHasher,
    plan: &ExploreFiniteTypePlan,
) -> Result<(), RelationalSourceExecutorError> {
    match plan {
        ExploreFiniteTypePlan::Unit => hasher.tag(0x01),
        ExploreFiniteTypePlan::Bool => hasher.tag(0x02),
        ExploreFiniteTypePlan::Tuple { elements, .. } => {
            hasher.tag(0x03);
            hasher.len(elements.len());
            for element in elements {
                hash_finite_type_plan(hasher, element)?;
            }
            hasher.u128(finite_type_exact_cardinality(plan)?);
        }
        ExploreFiniteTypePlan::Sum {
            type_name,
            variants,
            ..
        } => {
            hasher.tag(0x04);
            hasher.bytes(type_name.as_bytes());
            hasher.len(variants.len());
            for variant in variants {
                hasher.bytes(variant.name.as_bytes());
                hasher.boolean(variant.positional);
                hasher.len(variant.fields.len());
                for field in &variant.fields {
                    hasher.bytes(field.name.as_bytes());
                    hash_finite_type_plan(hasher, &field.plan)?;
                }
            }
            hasher.u128(finite_type_exact_cardinality(plan)?);
        }
    }
    Ok(())
}

fn derive_source_binding_exhaustion_receipt_id(
    version: u32,
    relation_id: RelationId,
    binding_index: u32,
    prefix_digest: [u8; 32],
    terminal_ordinal: u128,
    emitted_member_count: u128,
    emitted_members_commitment: [u8; 32],
) -> SourceBindingExhaustionReceiptId {
    let mut hasher = ExhaustionHasher::new(SOURCE_BINDING_EXHAUSTION_RECEIPT_HASH_V1);
    hasher.u32(version);
    hasher.digest(relation_id.bytes());
    hasher.u32(binding_index);
    hasher.digest(prefix_digest);
    hasher.u128(terminal_ordinal);
    hasher.u128(emitted_member_count);
    hasher.digest(emitted_members_commitment);
    SourceBindingExhaustionReceiptId(hasher.finish())
}

struct ExhaustionHasher(Sha256);

impl ExhaustionHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn boolean(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn len(&mut self, value: usize) {
        self.0.update((value as u128).to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    fn new(domain: &[u8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(domain.len() as u64).to_be_bytes());
        bytes.extend_from_slice(domain);
        Self(bytes)
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.extend_from_slice(&digest);
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSourceExecutorError {
    InvalidSourceRelation(String),
    InvalidExactDomain(String),
    Evaluation {
        binding_index: u32,
        binding_name: String,
        phase: &'static str,
        message: String,
    },
    ExpectedCollection(&'static str),
    ExpectedInt(&'static str),
    RangeStartAfterEnd {
        start: i64,
        end_exclusive: i64,
    },
    CardinalityExceedsU128(&'static str),
    OrdinalBeyondCardinality {
        ordinal: u128,
        cardinality: u128,
    },
    OrdinalDoesNotFitUsize(u128),
    IntegerRangeValueOverflow,
    CursorOrdinalOverflow,
    CursorHasNoBinding(u32),
    IndependentAssignmentArityMismatch {
        expected: usize,
        actual: usize,
    },
    IndependentAssignmentMemberAbsent {
        binding_index: u32,
        ordinal: u128,
    },
    NotSourceBindingWork,
    WorkRelationMismatch,
    PrefixMemberAbsent(u32),
    CursorSnapshotProvenanceMismatch,
    FiberCursorMismatch,
    ExhaustionBeforeTerminal {
        terminal_ordinal: u128,
        cardinality: u128,
    },
    UnsupportedExhaustionReceiptVersion {
        actual: u32,
        expected: u32,
    },
    ExhaustionReceiptCountMismatch {
        terminal_ordinal: u128,
        emitted_member_count: u128,
    },
    ExhaustionReceiptIdMismatch {
        claimed: SourceBindingExhaustionReceiptId,
        derived: SourceBindingExhaustionReceiptId,
    },
    UnboundFiberSupport,
    SupportCell(String),
    CursorBindingIndexMismatch {
        cursor: u32,
        prefix_len: u32,
    },
    UnsupportedCursorVersion {
        actual: u32,
        expected: u32,
    },
    PrefixDigestMismatch,
    PrefixSelectionCountMismatch {
        values: usize,
        selections: usize,
    },
    SelectionBindingIndexMismatch {
        actual: u32,
        expected: u32,
    },
    NonCanonicalSupportOrdinals(u32),
    ParentPrefixDigestMismatch(u32),
    SelectionValueMissing(u32),
    MissingContextValue,
    MissingBeforeValue,
    CanonicalLengthOverflow(&'static str),
    Frontier(WorkFrontierError),
}

impl From<WorkFrontierError> for RelationalSourceExecutorError {
    fn from(error: WorkFrontierError) -> Self {
        Self::Frontier(error)
    }
}

impl fmt::Display for RelationalSourceExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSourceRelation(message) => {
                write!(formatter, "invalid relational source: {message}")
            }
            Self::InvalidExactDomain(message) => {
                write!(formatter, "invalid finite relational domain: {message}")
            }
            Self::Evaluation {
                binding_index,
                binding_name,
                phase,
                message,
            } => write!(
                formatter,
                "source binding {binding_index} `{binding_name}` {phase} evaluation failed: {message}"
            ),
            Self::ExpectedCollection(kind) => {
                write!(
                    formatter,
                    "finite-domain evaluation did not produce a {kind}"
                )
            }
            Self::ExpectedInt(subject) => {
                write!(formatter, "{subject} evaluation did not produce an Int")
            }
            Self::RangeStartAfterEnd {
                start,
                end_exclusive,
            } => write!(
                formatter,
                "source range start {start} is greater than end {end_exclusive}"
            ),
            Self::CardinalityExceedsU128(subject) => {
                write!(
                    formatter,
                    "{subject} cardinality exceeds the durable u128 cursor"
                )
            }
            Self::OrdinalBeyondCardinality {
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "source member ordinal {ordinal} exceeds cardinality {cardinality}"
            ),
            Self::OrdinalDoesNotFitUsize(ordinal) => {
                write!(
                    formatter,
                    "materialized member ordinal {ordinal} does not fit usize"
                )
            }
            Self::IntegerRangeValueOverflow => {
                formatter.write_str("source integer-range value cannot be represented")
            }
            Self::CursorOrdinalOverflow => {
                formatter.write_str("source member cursor cannot advance beyond u128")
            }
            Self::CursorHasNoBinding(index) => {
                write!(
                    formatter,
                    "source cursor binding index {index} is complete or absent"
                )
            }
            Self::IndependentAssignmentArityMismatch { expected, actual } => write!(
                formatter,
                "independent source assignment supplied {actual} finite ordinals; expected {expected}"
            ),
            Self::IndependentAssignmentMemberAbsent {
                binding_index,
                ordinal,
            } => write!(
                formatter,
                "independent source assignment ordinal {ordinal} is absent from binding {binding_index}"
            ),
            Self::NotSourceBindingWork => {
                formatter.write_str("work node is not dependent source-binding expansion")
            }
            Self::WorkRelationMismatch => {
                formatter.write_str("source work node belongs to a different relation")
            }
            Self::PrefixMemberAbsent(index) => write!(
                formatter,
                "source prefix value at binding {index} is absent from its reconstructed fiber"
            ),
            Self::CursorSnapshotProvenanceMismatch => formatter
                .write_str("source cursor snapshot lineage/support does not match checked replay"),
            Self::FiberCursorMismatch => formatter
                .write_str("source binding fiber belongs to a different relation or prefix"),
            Self::ExhaustionBeforeTerminal {
                terminal_ordinal,
                cardinality,
            } => write!(
                formatter,
                "source exhaustion ordinal {terminal_ordinal} does not equal fiber cardinality {cardinality}"
            ),
            Self::UnsupportedExhaustionReceiptVersion { actual, expected } => write!(
                formatter,
                "unsupported source exhaustion receipt version {actual}; expected {expected}"
            ),
            Self::ExhaustionReceiptCountMismatch {
                terminal_ordinal,
                emitted_member_count,
            } => write!(
                formatter,
                "source exhaustion ordinal {terminal_ordinal} does not equal emitted member count {emitted_member_count}"
            ),
            Self::ExhaustionReceiptIdMismatch { .. } => formatter
                .write_str("source exhaustion receipt ID does not match its semantic content"),
            Self::UnboundFiberSupport => {
                formatter.write_str("source support cell requires a relation-bound binding fiber")
            }
            Self::SupportCell(message) => {
                write!(formatter, "source support cell is invalid: {message}")
            }
            Self::CursorBindingIndexMismatch { cursor, prefix_len } => write!(
                formatter,
                "source cursor binding index {cursor} does not match prefix length {prefix_len}"
            ),
            Self::UnsupportedCursorVersion { actual, expected } => write!(
                formatter,
                "unsupported relational source cursor version {actual}; expected {expected}"
            ),
            Self::PrefixDigestMismatch => {
                formatter.write_str("source prefix digest does not match its values")
            }
            Self::PrefixSelectionCountMismatch { values, selections } => write!(
                formatter,
                "source prefix has {values} values but {selections} selections"
            ),
            Self::SelectionBindingIndexMismatch { actual, expected } => write!(
                formatter,
                "source selection has binding index {actual}, expected {expected}"
            ),
            Self::NonCanonicalSupportOrdinals(index) => write!(
                formatter,
                "source binding {index} support ordinals are empty, duplicated, or unsorted"
            ),
            Self::ParentPrefixDigestMismatch(index) => write!(
                formatter,
                "source binding {index} selection names a different parent prefix"
            ),
            Self::SelectionValueMissing(index) => {
                write!(
                    formatter,
                    "source binding {index} selection has no prefix value"
                )
            }
            Self::MissingContextValue => formatter.write_str("completed source has no Context"),
            Self::MissingBeforeValue => formatter.write_str("completed source has no Before"),
            Self::CanonicalLengthOverflow(subject) => {
                write!(
                    formatter,
                    "{subject} length cannot be canonically represented"
                )
            }
            Self::Frontier(error) => write!(formatter, "source frontier value is invalid: {error}"),
        }
    }
}

impl Error for RelationalSourceExecutorError {}

#[cfg(test)]
mod tests {
    use super::super::relational_ir::{ExploreSourceBindingIr, ExploreSourceDependencyIr};
    use super::*;
    use crate::{
        ExploreRelationMultiplicity, ExprKind, Literal, Span,
        EXPLORE_RELATION_NORMALIZATION_VERSION,
    };

    struct TestRuntime;

    impl RelationalExpressionRuntime for TestRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            earlier_bindings: &[RelationalBoundValue<'_>],
        ) -> Result<ExploreValue, String> {
            fn evaluate(
                expression: &Expr,
                earlier_bindings: &[RelationalBoundValue<'_>],
            ) -> Result<ExploreValue, String> {
                match &expression.kind {
                    ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
                    ExprKind::Var(name) => earlier_bindings
                        .iter()
                        .find(|binding| binding.name == name.as_str())
                        .map(|binding| binding.value.clone())
                        .ok_or_else(|| format!("unbound test name {name}")),
                    ExprKind::List(values) => values
                        .iter()
                        .map(|value| evaluate(value, earlier_bindings))
                        .collect::<Result<Vec<_>, _>>()
                        .map(ExploreValue::List),
                    _ => Err("unsupported test expression".to_string()),
                }
            }
            evaluate(expression, earlier_bindings)
        }
    }

    fn int_ty() -> Ty {
        Ty::Name("Int".to_string())
    }

    fn list_int_ty() -> Ty {
        Ty::App(Box::new(Ty::Name("List".to_string())), vec![int_ty()])
    }

    fn int(value: i64) -> Expr {
        Expr::unspanned(ExprKind::Lit(Literal::Int(value)))
    }

    fn var(name: &str) -> Expr {
        Expr::unspanned(ExprKind::Var(name.to_string()))
    }

    fn dependency(index: usize, name: &str) -> ExploreSourceDependencyIr {
        ExploreSourceDependencyIr {
            binding_index: index,
            binding_name: name.to_string(),
        }
    }

    fn binding(
        binding_index: usize,
        name: &str,
        value_ty: Ty,
        role: ExploreSourceBindingRoleIr,
        dependencies: Vec<ExploreSourceDependencyIr>,
        kind: ExploreSourceBindingKindIr,
    ) -> ExploreSourceBindingIr {
        ExploreSourceBindingIr {
            binding_index,
            name: name.to_string(),
            value_ty,
            role,
            dependencies: dependencies.into_boxed_slice(),
            kind,
            span: Span::dummy(),
        }
    }

    fn relation(bindings: Vec<ExploreSourceBindingIr>) -> ExploreSourceRelationIr {
        ExploreSourceRelationIr {
            normalization_version: EXPLORE_RELATION_NORMALIZATION_VERSION,
            multiplicity: ExploreRelationMultiplicity::SetNormalized,
            bindings: bindings.into_boxed_slice(),
            context_binding_index: 2,
            before_binding_index: 1,
            context_ty: int_ty(),
            before_ty: int_ty(),
        }
    }

    fn dependent_relation() -> ExploreSourceRelationIr {
        relation(vec![
            binding(
                0,
                "limit",
                int_ty(),
                ExploreSourceBindingRoleIr::Auxiliary,
                vec![],
                ExploreSourceBindingKindIr::Singleton { value: int(3) },
            ),
            binding(
                1,
                "before",
                int_ty(),
                ExploreSourceBindingRoleIr::Before,
                vec![dependency(0, "limit")],
                ExploreSourceBindingKindIr::Finite {
                    domain: ExploreFiniteDomainIr::IntRange {
                        start: int(0),
                        end_exclusive: var("limit"),
                    },
                },
            ),
            binding(
                2,
                "context",
                int_ty(),
                ExploreSourceBindingRoleIr::Context,
                vec![dependency(1, "before")],
                ExploreSourceBindingKindIr::Singleton {
                    value: var("before"),
                },
            ),
        ])
    }

    fn enumerator(relation: &ExploreSourceRelationIr) -> RelationalSourceEnumerator<'_> {
        RelationalSourceEnumerator::new(
            RelationId::from_canonical_semantic_preimage(b"source-executor-test"),
            relation,
        )
        .unwrap()
    }

    fn yielded_continuation(advance: RelationalSourceAdvance) -> RelationalSourceContinuation {
        match advance {
            RelationalSourceAdvance::Yielded { continuation, .. } => continuation,
            RelationalSourceAdvance::Exhausted { .. } => panic!("expected yielded member"),
        }
    }

    #[test]
    fn dependent_range_uses_the_earlier_prefix_and_yields_a_source_row() {
        let relation = dependent_relation();
        let executor = enumerator(&relation);
        let mut runtime = TestRuntime;
        let root = executor.root_cursor().unwrap();
        let RelationalSourceContinuation::Expand(before_cursor) =
            yielded_continuation(executor.advance(&root, &mut runtime).unwrap())
        else {
            panic!("limit must yield the dependent before binding")
        };
        let before_fiber = executor
            .binding_fiber(&before_cursor, &mut runtime)
            .unwrap();
        assert_eq!(before_fiber.cardinality(), 3);
        assert_eq!(
            before_fiber.member_at_ordinal(2).unwrap().unwrap().value(),
            &ExploreValue::Int(2)
        );

        let mut selected_before = before_cursor.snapshot();
        selected_before.next_member_ordinal = 2;
        let selected_before = executor
            .resume_snapshot(selected_before, &mut runtime)
            .unwrap();
        let RelationalSourceContinuation::Expand(context_cursor) =
            yielded_continuation(executor.advance(&selected_before, &mut runtime).unwrap())
        else {
            panic!("before must yield the dependent context binding")
        };
        let RelationalSourceContinuation::Source(source) =
            yielded_continuation(executor.advance(&context_cursor, &mut runtime).unwrap())
        else {
            panic!("context must complete one source row")
        };
        assert_eq!(source.row().before(), &ExploreValue::Int(2));
        assert_eq!(source.row().context(), &ExploreValue::Int(2));
        assert_eq!(source.row().provenance().lineage().len(), 1);
        assert_eq!(source.row().provenance().support().len(), 3);
    }

    #[test]
    fn collection_fiber_deduplicates_canonically_and_retains_occurrence_support() {
        let choices = Expr::unspanned(ExprKind::List(vec![int(2), int(1), int(2)]));
        let relation = relation(vec![
            binding(
                0,
                "choices",
                list_int_ty(),
                ExploreSourceBindingRoleIr::Auxiliary,
                vec![],
                ExploreSourceBindingKindIr::Singleton { value: choices },
            ),
            binding(
                1,
                "before",
                int_ty(),
                ExploreSourceBindingRoleIr::Before,
                vec![dependency(0, "choices")],
                ExploreSourceBindingKindIr::Finite {
                    domain: ExploreFiniteDomainIr::Collection {
                        expression: var("choices"),
                        collection_ty: list_int_ty(),
                        element_ty: int_ty(),
                    },
                },
            ),
            binding(
                2,
                "context",
                int_ty(),
                ExploreSourceBindingRoleIr::Context,
                vec![],
                ExploreSourceBindingKindIr::Singleton { value: int(0) },
            ),
        ]);
        let executor = enumerator(&relation);
        let mut runtime = TestRuntime;
        let root = executor.root_cursor().unwrap();
        let RelationalSourceContinuation::Expand(before_cursor) =
            yielded_continuation(executor.advance(&root, &mut runtime).unwrap())
        else {
            panic!("choices must yield before work")
        };
        let fiber = executor
            .binding_fiber(&before_cursor, &mut runtime)
            .unwrap();
        assert_eq!(fiber.cardinality(), 2);
        let first = fiber.member_at_ordinal(0).unwrap().unwrap();
        let second = fiber.member_at_ordinal(1).unwrap().unwrap();
        assert_eq!(first.value(), &ExploreValue::Int(1));
        assert_eq!(first.raw_support_ordinals(), &[1]);
        assert_eq!(second.value(), &ExploreValue::Int(2));
        assert_eq!(second.raw_support_ordinals(), &[0, 2]);
    }

    #[test]
    fn finite_type_product_is_ordinal_addressed_without_materialization() {
        let plan = ExploreFiniteTypePlan::Tuple {
            elements: vec![ExploreFiniteTypePlan::Bool, ExploreFiniteTypePlan::Bool],
            cardinality: ExploreCardinality::Exact(4),
        };
        let fiber = RelationalFiniteFiber::exact(&ExploreExactDomain::FiniteType {
            ty: Ty::App(
                Box::new(Ty::Name("Tuple".to_string())),
                vec![Ty::Name("Bool".to_string()), Ty::Name("Bool".to_string())],
            ),
            plan,
        })
        .unwrap();
        assert_eq!(fiber.cardinality(), 4);
        assert_eq!(
            fiber.member_at_ordinal(3).unwrap().unwrap().value(),
            &ExploreValue::Tuple(vec![
                ExploreValue::Boolean(true),
                ExploreValue::Boolean(true)
            ])
        );
    }

    #[test]
    fn cursor_snapshot_round_trip_preserves_pause_resume_identity() {
        let relation = dependent_relation();
        let executor = enumerator(&relation);
        let mut runtime = TestRuntime;
        let root = executor.root_cursor().unwrap();
        let RelationalSourceAdvance::Yielded {
            resume,
            continuation: RelationalSourceContinuation::Expand(child),
            ..
        } = executor.advance(&root, &mut runtime).unwrap()
        else {
            panic!("root must yield")
        };
        let resumed_parent = executor
            .resume_snapshot(resume.snapshot(), &mut runtime)
            .unwrap();
        let resumed_child = executor
            .resume_snapshot(child.snapshot(), &mut runtime)
            .unwrap();
        assert_eq!(resumed_parent, resume);
        assert_eq!(resumed_child, child);
        let parent_work = executor.work_spec(&root).unwrap();
        let child_work = executor.work_spec(&child).unwrap();
        assert_eq!(
            executor
                .resume_cursor(&parent_work, resume.next_member_ordinal(), &mut runtime)
                .unwrap(),
            resume
        );
        assert_eq!(
            executor
                .resume_cursor(&child_work, child.next_member_ordinal(), &mut runtime)
                .unwrap(),
            child
        );
        assert_eq!(
            executor
                .binding_fiber(&resumed_child, &mut runtime)
                .unwrap()
                .member_at_ordinal(0)
                .unwrap(),
            executor
                .binding_fiber(&child, &mut runtime)
                .unwrap()
                .member_at_ordinal(0)
                .unwrap()
        );
    }

    #[test]
    fn actual_terminal_transition_issues_content_stable_source_exhaustion_receipt() {
        let relation = dependent_relation();
        let relation_id = RelationId::from_canonical_semantic_preimage(b"source-executor-test");
        let executor = enumerator(&relation);
        let mut runtime = TestRuntime;
        let cursor = executor.root_cursor().unwrap();
        let fiber = executor.binding_fiber(&cursor, &mut runtime).unwrap();

        let RelationalSourceAdvance::Yielded { resume, .. } =
            executor.advance_in_fiber(&cursor, &fiber).unwrap()
        else {
            panic!("the root singleton must yield before it can exhaust")
        };
        let RelationalSourceAdvance::Exhausted {
            cursor: terminal,
            cardinality,
            receipt,
        } = executor.advance_in_fiber(&resume, &fiber).unwrap()
        else {
            panic!("the resumed singleton must reach actual exhaustion")
        };

        assert_eq!(terminal, resume);
        assert_eq!(cardinality, 1);
        assert_eq!(receipt.version(), SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION);
        assert_eq!(receipt.relation_id(), relation_id);
        assert_eq!(receipt.binding_index(), 0);
        assert_eq!(receipt.prefix_digest(), cursor.canonical_prefix().digest());
        assert_eq!(receipt.terminal_ordinal(), 1);
        assert_eq!(receipt.emitted_member_count(), 1);
        assert_eq!(
            receipt.emitted_members_commitment(),
            fiber.canonical_member_commitment().unwrap()
        );
        receipt.validate_identity().unwrap();

        let RelationalSourceAdvance::Exhausted {
            receipt: replayed, ..
        } = executor.advance_in_fiber(&resume, &fiber).unwrap()
        else {
            panic!("replaying the terminal transition must remain exhausted")
        };
        assert_eq!(receipt, replayed);
        assert_eq!(receipt.id(), replayed.id());
        assert_eq!(receipt.id().bytes(), replayed.id().bytes());
    }
}
