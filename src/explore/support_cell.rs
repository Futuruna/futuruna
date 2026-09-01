//! Exact finite support cells and proof-carrying summaries for relational Explore.
//!
//! A support cell is a content-addressed description of a finite set and, when
//! applicable, the canonical mapping from producer coordinates to extensional
//! rows or cases.  It deliberately contains no admission, FIND, view,
//! mechanism, scheduling, or example-retention state.  Those facts attach to a
//! stable cell through typed proof obligations, so a new question can reuse the
//! same support partition.
//!
//! Producer coordinates and mapped images are different cardinality domains.
//! In particular, an exact Cartesian product count does not become an exact
//! row or case count when the materializer may collapse duplicate images.  An
//! image count becomes exact only through an accepted count proof or accepted
//! injectivity evidence.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU128;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::MechanismSignatureId;
use super::relation::{
    AdmissionDecision, AdmissionId, MechanismRequestId, QuestionId, RelationId, SelectionDecision,
};
use super::{transition::canonical_explore_value_digest, ExploreValue};

const SUPPORT_EXPR_HASH_V1: &[u8] = b"futuruna.explore.support-expr.v1";
const SUPPORT_PRODUCER_HASH_V1: &[u8] = b"futuruna.explore.support-producer.v1";
const SUPPORT_MATERIALIZER_HASH_V1: &[u8] = b"futuruna.explore.support-materializer.v1";
const SUPPORT_OBSERVER_HASH_V1: &[u8] = b"futuruna.explore.support-observer.v1";
const SUPPORT_CELL_HASH_V1: &[u8] = b"futuruna.explore.support-cell.v1";
const SUPPORT_OBLIGATION_HASH_V1: &[u8] = b"futuruna.explore.support-obligation.v1";
const SUPPORT_PARTITION_OBLIGATION_HASH_V1: &[u8] =
    b"futuruna.explore.support-partition-obligation.v1";
const SUPPORT_PROOF_VERIFIER_HASH_V1: &[u8] = b"futuruna.explore.support-proof-verifier.v1";
const SUPPORT_PROOF_RECEIPT_HASH_V1: &[u8] = b"futuruna.explore.support-proof-receipt.v1";
const SUPPORT_EVIDENCE_HASH_V1: &[u8] = b"futuruna.explore.support-evidence.v1";
const SUPPORT_PARTITION_HASH_V1: &[u8] = b"futuruna.explore.support-partition.v1";
const SUPPORT_CURSOR_HASH_V1: &[u8] = b"futuruna.explore.support-cursor.v1";
const SUPPORT_EXAMPLE_HASH_V1: &[u8] = b"futuruna.explore.support-example.v1";

pub(crate) const SUPPORT_MATERIALIZATION_CURSOR_VERSION: u32 = 1;

/// Exactness state for a finite support count.
///
/// `Open` does not mean infinite.  The enclosing support contract is finite;
/// it means that only the stated lower bound has so far been certified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportCardinality {
    Exact(u128),
    Open { confirmed_lower_bound: u128 },
}

impl SupportCardinality {
    pub(crate) const fn exact(self) -> Option<u128> {
        match self {
            Self::Exact(count) => Some(count),
            Self::Open { .. } => None,
        }
    }

    pub(crate) const fn lower_bound(self) -> u128 {
        match self {
            Self::Exact(count) => count,
            Self::Open {
                confirmed_lower_bound,
            } => confirmed_lower_bound,
        }
    }

    fn hash_into(self, hasher: &mut CanonicalHasher) {
        match self {
            Self::Exact(count) => {
                hasher.tag(0x01);
                hasher.u128(count);
            }
            Self::Open {
                confirmed_lower_bound,
            } => {
                hasher.tag(0x02);
                hasher.u128(confirmed_lower_bound);
            }
        }
    }
}

/// Content identity of a canonical finite-support expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportExprId([u8; 32]);

impl SupportExprId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of a checked finite producer or dependent join contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportProducerId([u8; 32]);

impl SupportProducerId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_scoped_digest(SUPPORT_PRODUCER_HASH_V1, preimage))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of the checked semantic coordinate-to-image mapping and its
/// checkpoint-codec contract.
///
/// Physical enumeration order, eager versus symbolic execution, solver choice,
/// worker layout, and other scheduling strategy are excluded. Equivalent
/// implementations of the same mapping/codec therefore do not rename a cell.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportMaterializerId([u8; 32]);

impl SupportMaterializerId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_scoped_digest(SUPPORT_MATERIALIZER_HASH_V1, preimage))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of a checked value observer used by a uniform-value obligation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportObserverId([u8; 32]);

impl SupportObserverId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_scoped_digest(SUPPORT_OBSERVER_HASH_V1, preimage))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical finite support algebra.
///
/// Product factors are ordered because they are producer coordinates.  Union
/// operands are canonicalized by content identity.  Join references name a
/// checked producer contract and its ordered input cells without embedding a
/// solver backend in this layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportExprKind {
    Singleton(ExploreValue),
    FiniteEnum(Box<[ExploreValue]>),
    OrdinalInterval {
        start: u128,
        end_exclusive: u128,
    },
    OrdinalCongruence {
        start: u128,
        end_exclusive: u128,
        modulus: NonZeroU128,
        residue: u128,
    },
    Product(Box<[SupportExpr]>),
    /// One nonempty half-open mixed-radix rank interval inside an ordered
    /// product of zero-based ordinal factors. The final factor is the
    /// least-significant (fastest-varying) coordinate.
    ProductRankInterval {
        factors: Box<[SupportExpr]>,
        rank_start: u128,
        rank_end_exclusive: u128,
    },
    JoinReference {
        producer_id: SupportProducerId,
        inputs: Box<[SupportCellId]>,
    },
    Union(Box<[SupportExpr]>),
    Difference {
        minuend: Box<SupportExpr>,
        subtrahend: Box<SupportExpr>,
    },
}

/// A canonical, content-addressed exact-finite support description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportExpr {
    id: SupportExprId,
    kind: SupportExprKind,
    intrinsic_cardinality: SupportCardinality,
}

impl SupportExpr {
    pub(crate) fn singleton(value: ExploreValue) -> Self {
        Self::from_canonical_kind(
            SupportExprKind::Singleton(value),
            SupportCardinality::Exact(1),
        )
    }

    /// Construct a set-valued finite enumeration. Input order and duplicates
    /// do not affect identity; a one-element enumeration canonicalizes to a
    /// singleton. Empty cells are rejected rather than retained as work.
    pub(crate) fn finite_enum(mut values: Vec<ExploreValue>) -> Result<Self, SupportCellError> {
        values.sort();
        values.dedup();
        match values.len() {
            0 => Err(SupportCellError::EmptySupport("finite enumeration")),
            1 => Ok(Self::singleton(values.pop().expect("length checked"))),
            count => {
                let count = u128::try_from(count)
                    .map_err(|_| SupportCellError::CardinalityOverflow("finite enumeration"))?;
                Ok(Self::from_canonical_kind(
                    SupportExprKind::FiniteEnum(values.into_boxed_slice()),
                    SupportCardinality::Exact(count),
                ))
            }
        }
    }

    pub(crate) fn ordinal_interval(
        start: u128,
        end_exclusive: u128,
    ) -> Result<Self, SupportCellError> {
        if start >= end_exclusive {
            return Err(SupportCellError::InvalidInterval {
                start,
                end_exclusive,
            });
        }
        Ok(Self::from_canonical_kind(
            SupportExprKind::OrdinalInterval {
                start,
                end_exclusive,
            },
            SupportCardinality::Exact(end_exclusive - start),
        ))
    }

    pub(crate) fn ordinal_congruence(
        start: u128,
        end_exclusive: u128,
        modulus: NonZeroU128,
        residue: u128,
    ) -> Result<Self, SupportCellError> {
        if start >= end_exclusive {
            return Err(SupportCellError::InvalidInterval {
                start,
                end_exclusive,
            });
        }
        if residue >= modulus.get() {
            return Err(SupportCellError::InvalidCongruenceResidue {
                residue,
                modulus: modulus.get(),
            });
        }
        let count = congruence_cardinality(start, end_exclusive, modulus.get(), residue)?;
        if count == 0 {
            return Err(SupportCellError::EmptySupport("ordinal congruence"));
        }
        Ok(Self::from_canonical_kind(
            SupportExprKind::OrdinalCongruence {
                start,
                end_exclusive,
                modulus,
                residue,
            },
            SupportCardinality::Exact(count),
        ))
    }

    /// Ordered Cartesian product of producer coordinates.
    ///
    /// Its cardinality is the number of assignments, not necessarily the
    /// number of distinct rows obtained after a materializer maps assignments
    /// into an extensional relation.
    pub(crate) fn product(factors: Vec<Self>) -> Result<Self, SupportCellError> {
        let mut flattened = Vec::new();
        for factor in factors {
            factor.validate()?;
            match factor {
                Self {
                    kind: SupportExprKind::Product(nested),
                    ..
                } => flattened.extend(nested.into_vec()),
                other => flattened.push(other),
            }
        }
        match flattened.len() {
            0 => Err(SupportCellError::EmptyProduct),
            1 => Ok(flattened.pop().expect("length checked")),
            _ => {
                let cardinality = product_cardinality(&flattened)?;
                Ok(Self::from_canonical_kind(
                    SupportExprKind::Product(flattened.into_boxed_slice()),
                    cardinality,
                ))
            }
        }
    }

    /// Restrict an exact independent ordinal product to one canonical
    /// mixed-radix rank interval.
    ///
    /// A full-rank interval canonicalizes back to [`Self::product`]. This
    /// keeps the original product identity stable and reserves the ranked form
    /// for proper nonempty restrictions only.
    pub(crate) fn product_rank_interval(
        factors: Vec<Self>,
        rank_start: u128,
        rank_end_exclusive: u128,
    ) -> Result<Self, SupportCellError> {
        let product = Self::product(factors)?;
        let SupportExprKind::Product(canonical_factors) = product.kind() else {
            return Err(SupportCellError::InvalidProductRankInterval(
                "ranked product requires at least two factors",
            ));
        };
        for factor in canonical_factors.iter() {
            let SupportExprKind::OrdinalInterval {
                start: 0,
                end_exclusive: _,
            } = factor.kind()
            else {
                return Err(SupportCellError::InvalidProductRankInterval(
                    "ranked product factors must be zero-based ordinal intervals",
                ));
            };
        }
        let product_cardinality = product.intrinsic_cardinality().exact().ok_or(
            SupportCellError::InvalidProductRankInterval("ranked product is not exact"),
        )?;
        if rank_start >= rank_end_exclusive || rank_end_exclusive > product_cardinality {
            return Err(SupportCellError::InvalidProductRankInterval(
                "rank interval is empty, reversed, or outside its product",
            ));
        }
        if rank_start == 0 && rank_end_exclusive == product_cardinality {
            return Ok(product);
        }
        let SupportExprKind::Product(canonical_factors) = product.kind else {
            unreachable!("ranked product shape checked")
        };
        Ok(Self::from_canonical_kind(
            SupportExprKind::ProductRankInterval {
                factors: canonical_factors,
                rank_start,
                rank_end_exclusive,
            },
            SupportCardinality::Exact(rank_end_exclusive - rank_start),
        ))
    }

    pub(crate) fn join_reference(
        producer_id: SupportProducerId,
        inputs: Vec<SupportCellId>,
    ) -> Self {
        Self::from_canonical_kind(
            SupportExprKind::JoinReference {
                producer_id,
                inputs: inputs.into_boxed_slice(),
            },
            SupportCardinality::Open {
                confirmed_lower_bound: 0,
            },
        )
    }

    /// Canonical set union. Enumerations and ordinal intervals are normalized
    /// extensionally (including interval coalescing) and counted exactly.
    /// Other distinct operands may overlap, so they retain only a safe lower
    /// bound until partition or exact-count evidence is attached.
    pub(crate) fn union(operands: Vec<Self>) -> Result<Self, SupportCellError> {
        let mut flattened = Vec::new();
        for operand in operands {
            operand.validate()?;
            match operand {
                Self {
                    kind: SupportExprKind::Union(nested),
                    ..
                } => flattened.extend(nested.into_vec()),
                other => flattened.push(other),
            }
        }

        if flattened.iter().all(|operand| {
            matches!(
                &operand.kind,
                SupportExprKind::Singleton(_) | SupportExprKind::FiniteEnum(_)
            )
        }) {
            let mut values = Vec::new();
            for operand in flattened {
                match operand.kind {
                    SupportExprKind::Singleton(value) => values.push(value),
                    SupportExprKind::FiniteEnum(items) => values.extend(items.into_vec()),
                    _ => unreachable!("enumerated union shape checked"),
                }
            }
            return Self::finite_enum(values);
        }

        if flattened
            .iter()
            .all(|operand| ordinal_interval_bounds(operand).is_some())
        {
            let mut intervals = flattened
                .iter()
                .map(|operand| {
                    ordinal_interval_bounds(operand).expect("interval union shape checked")
                })
                .collect::<Vec<_>>();
            intervals.sort();
            let mut merged = Vec::<(u128, u128)>::new();
            for (start, end_exclusive) in intervals {
                match merged.last_mut() {
                    Some((_, previous_end)) if start <= *previous_end => {
                        *previous_end = (*previous_end).max(end_exclusive);
                    }
                    _ => merged.push((start, end_exclusive)),
                }
            }
            if merged.len() == 1 {
                let (start, end_exclusive) = merged[0];
                return Self::ordinal_interval(start, end_exclusive);
            }
            if !merged.is_empty() {
                let mut exact_count = 0_u128;
                let mut canonical_intervals = Vec::with_capacity(merged.len());
                for (start, end_exclusive) in merged {
                    exact_count = exact_count.checked_add(end_exclusive - start).ok_or(
                        SupportCellError::CardinalityOverflow("adding disjoint interval union"),
                    )?;
                    canonical_intervals.push(Self::ordinal_interval(start, end_exclusive)?);
                }
                return Ok(Self::from_canonical_kind(
                    SupportExprKind::Union(canonical_intervals.into_boxed_slice()),
                    SupportCardinality::Exact(exact_count),
                ));
            }
        }

        flattened.sort_by_key(|operand| operand.id);
        flattened.dedup_by_key(|operand| operand.id);
        match flattened.len() {
            0 => Err(SupportCellError::EmptySupport("union")),
            1 => Ok(flattened.pop().expect("length checked")),
            _ => {
                let confirmed_lower_bound = flattened
                    .iter()
                    .map(|operand| operand.intrinsic_cardinality.lower_bound())
                    .max()
                    .unwrap_or(0);
                Ok(Self::from_canonical_kind(
                    SupportExprKind::Union(flattened.into_boxed_slice()),
                    SupportCardinality::Open {
                        confirmed_lower_bound,
                    },
                ))
            }
        }
    }

    pub(crate) fn difference(minuend: Self, subtrahend: Self) -> Result<Self, SupportCellError> {
        minuend.validate()?;
        subtrahend.validate()?;
        if minuend.id == subtrahend.id {
            return Err(SupportCellError::EmptySupport("self difference"));
        }

        if matches!(
            &minuend.kind,
            SupportExprKind::Singleton(_) | SupportExprKind::FiniteEnum(_)
        ) && matches!(
            &subtrahend.kind,
            SupportExprKind::Singleton(_) | SupportExprKind::FiniteEnum(_)
        ) {
            let mut minuend_values = match &minuend.kind {
                SupportExprKind::Singleton(value) => vec![value.clone()],
                SupportExprKind::FiniteEnum(values) => values.to_vec(),
                _ => unreachable!("enumerated difference shape checked"),
            };
            let subtrahend_values = match &subtrahend.kind {
                SupportExprKind::Singleton(value) => vec![value.clone()],
                SupportExprKind::FiniteEnum(values) => values.to_vec(),
                _ => unreachable!("enumerated difference shape checked"),
            };
            minuend_values.retain(|value| subtrahend_values.binary_search(value).is_err());
            return Self::finite_enum(minuend_values);
        }

        if let (Some((left_start, left_end)), Some((right_start, right_end))) = (
            ordinal_interval_bounds(&minuend),
            ordinal_interval_bounds(&subtrahend),
        ) {
            if right_end <= left_start || right_start >= left_end {
                return Ok(minuend);
            }
            if right_start <= left_start && right_end >= left_end {
                return Err(SupportCellError::EmptySupport("interval difference"));
            }
            if right_start <= left_start {
                return Self::ordinal_interval(right_end, left_end);
            }
            if right_end >= left_end {
                return Self::ordinal_interval(left_start, right_start);
            }
            return Self::union(vec![
                Self::ordinal_interval(left_start, right_start)?,
                Self::ordinal_interval(right_end, left_end)?,
            ]);
        }

        Ok(Self::from_canonical_kind(
            SupportExprKind::Difference {
                minuend: Box::new(minuend),
                subtrahend: Box::new(subtrahend),
            },
            SupportCardinality::Open {
                confirmed_lower_bound: 0,
            },
        ))
    }

    pub(crate) const fn id(&self) -> SupportExprId {
        self.id
    }

    pub(crate) const fn kind(&self) -> &SupportExprKind {
        &self.kind
    }

    /// Cardinality of expression elements before any cell mapping is applied.
    pub(crate) const fn intrinsic_cardinality(&self) -> SupportCardinality {
        self.intrinsic_cardinality
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        let canonical = match &self.kind {
            SupportExprKind::Singleton(value) => Self::singleton(value.clone()),
            SupportExprKind::FiniteEnum(values) => Self::finite_enum(values.to_vec())?,
            SupportExprKind::OrdinalInterval {
                start,
                end_exclusive,
            } => Self::ordinal_interval(*start, *end_exclusive)?,
            SupportExprKind::OrdinalCongruence {
                start,
                end_exclusive,
                modulus,
                residue,
            } => Self::ordinal_congruence(*start, *end_exclusive, *modulus, *residue)?,
            SupportExprKind::Product(factors) => Self::product(factors.to_vec())?,
            SupportExprKind::ProductRankInterval {
                factors,
                rank_start,
                rank_end_exclusive,
            } => Self::product_rank_interval(factors.to_vec(), *rank_start, *rank_end_exclusive)?,
            SupportExprKind::JoinReference {
                producer_id,
                inputs,
            } => Self::join_reference(*producer_id, inputs.to_vec()),
            SupportExprKind::Union(operands) => Self::union(operands.to_vec())?,
            SupportExprKind::Difference {
                minuend,
                subtrahend,
            } => Self::difference((**minuend).clone(), (**subtrahend).clone())?,
        };
        if canonical != *self {
            return Err(SupportCellError::NonCanonicalExpression);
        }
        Ok(())
    }

    fn from_canonical_kind(
        kind: SupportExprKind,
        intrinsic_cardinality: SupportCardinality,
    ) -> Self {
        let id = SupportExprId(hash_support_expr(&kind));
        Self {
            id,
            kind,
            intrinsic_cardinality,
        }
    }
}

/// Extensional population represented by a support cell.
///
/// Questions, admissions, result views, and mechanism observers do not appear
/// here: they are reusable evidence layers over these populations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SupportExtensionalTarget {
    SourceRows(RelationId),
    SuccessorRows(RelationId),
    Cases(RelationId),
    Derived(SupportProducerId),
}

impl SupportExtensionalTarget {
    fn hash_into(self, hasher: &mut CanonicalHasher) {
        match self {
            Self::SourceRows(relation_id) => {
                hasher.tag(0x01);
                hasher.digest(relation_id.bytes());
            }
            Self::SuccessorRows(relation_id) => {
                hasher.tag(0x02);
                hasher.digest(relation_id.bytes());
            }
            Self::Cases(relation_id) => {
                hasher.tag(0x03);
                hasher.digest(relation_id.bytes());
            }
            Self::Derived(producer_id) => {
                hasher.tag(0x04);
                hasher.digest(producer_id.bytes());
            }
        }
    }
}

/// Whether expression elements are coordinates, already canonical extensional
/// values, or a preimage whose materializer may collapse equal images.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SupportCellSpace {
    ProducerCoordinates(SupportProducerId),
    ExtensionalValues(SupportExtensionalTarget),
    MappedImage {
        producer_id: SupportProducerId,
        target: SupportExtensionalTarget,
    },
}

impl SupportCellSpace {
    fn hash_into(self, hasher: &mut CanonicalHasher) {
        match self {
            Self::ProducerCoordinates(producer_id) => {
                hasher.tag(0x01);
                hasher.digest(producer_id.bytes());
            }
            Self::ExtensionalValues(target) => {
                hasher.tag(0x02);
                target.hash_into(hasher);
            }
            Self::MappedImage {
                producer_id,
                target,
            } => {
                hasher.tag(0x03);
                hasher.digest(producer_id.bytes());
                target.hash_into(hasher);
            }
        }
    }

    const fn is_mapped_image(self) -> bool {
        matches!(self, Self::MappedImage { .. })
    }
}

/// Content identity of support plus its canonical mapping contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportCellId([u8; 32]);

impl SupportCellId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable support unit consumed by proof, classification, view, and mechanism
/// layers without first expanding it into per-case identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportCell {
    id: SupportCellId,
    space: SupportCellSpace,
    expression: SupportExpr,
    materializer_id: SupportMaterializerId,
}

impl SupportCell {
    pub(crate) fn new(
        space: SupportCellSpace,
        expression: SupportExpr,
        materializer_id: SupportMaterializerId,
    ) -> Result<Self, SupportCellError> {
        expression.validate()?;
        let id = derive_support_cell_id(space, expression.id, materializer_id);
        Ok(Self {
            id,
            space,
            expression,
            materializer_id,
        })
    }

    pub(crate) const fn id(&self) -> SupportCellId {
        self.id
    }

    pub(crate) const fn space(&self) -> SupportCellSpace {
        self.space
    }

    pub(crate) const fn expression(&self) -> &SupportExpr {
        &self.expression
    }

    pub(crate) const fn materializer_id(&self) -> SupportMaterializerId {
        self.materializer_id
    }

    /// Count of producer coordinates visited by a materialization cursor.
    pub(crate) const fn coordinate_cardinality(&self) -> SupportCardinality {
        self.expression.intrinsic_cardinality
    }

    /// Best currently structural count of this cell's represented population.
    /// A mapped image deliberately does not inherit its coordinate count.
    pub(crate) const fn cardinality(&self) -> SupportCardinality {
        match self.space {
            SupportCellSpace::ProducerCoordinates(_) | SupportCellSpace::ExtensionalValues(_) => {
                self.expression.intrinsic_cardinality
            }
            SupportCellSpace::MappedImage { .. } => {
                let lower_bound = self.expression.intrinsic_cardinality.lower_bound();
                SupportCardinality::Open {
                    confirmed_lower_bound: if lower_bound == 0 { 0 } else { 1 },
                }
            }
        }
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        self.expression.validate()?;
        let derived = derive_support_cell_id(self.space, self.expression.id, self.materializer_id);
        if derived != self.id {
            return Err(SupportCellError::CellIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }

    /// Derive accepted structural cardinality evidence when no mapping can
    /// collapse expression elements. Mapped images must use a proof receipt.
    pub(crate) fn structural_cardinality_evidence(
        &self,
    ) -> Result<Option<SupportCellEvidence<ExactCardinalityClaim>>, SupportCellError> {
        let Some(count) = self.cardinality().exact() else {
            return Ok(None);
        };
        let obligation = SupportCellObligation::new(self, ExactCardinalityClaim)?;
        let conclusion_digest = obligation.claim().conclusion_digest(&count);
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.structural-cardinality-check.v1",
        );
        let proof_digest = derive_scoped_digest(
            b"futuruna.explore.structural-cardinality-proof.v1",
            &self.id.bytes(),
        );
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            verifier,
            conclusion_digest,
            proof_digest,
        );
        Ok(Some(SupportCellEvidence::from_accepted_proof(
            obligation, count, receipt,
        )?))
    }

    /// Recover the coordinate count for a mapped image after its materializer
    /// has been certified injective over this exact cell.
    pub(crate) fn cardinality_with_injectivity(
        &self,
        evidence: &SupportCellEvidence<InjectiveMappingClaim>,
    ) -> Result<SupportCardinality, SupportCellError> {
        if !self.space.is_mapped_image() {
            return Err(SupportCellError::InjectivityForUnmappedCell);
        }
        self.validate_evidence(evidence)?;
        if evidence.obligation().claim().materializer_id() != self.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        Ok(self.coordinate_cardinality())
    }

    pub(crate) fn validate_evidence<C: SupportCellClaim>(
        &self,
        evidence: &SupportCellEvidence<C>,
    ) -> Result<(), SupportCellError> {
        evidence.validate()?;
        if evidence.obligation().cell_id() != self.id {
            return Err(SupportCellError::EvidenceCellMismatch);
        }
        Ok(())
    }
}

/// Stable identity of a proof obligation, independent of proof strategy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportProofObligationId([u8; 32]);

impl SupportProofObligationId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable identity of a trusted proof checker and proof format contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportProofVerifierId([u8; 32]);

impl SupportProofVerifierId {
    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_scoped_digest(
            SUPPORT_PROOF_VERIFIER_HASH_V1,
            preimage,
        ))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of a proof checker accepting one exact conclusion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportProofReceiptId([u8; 32]);

impl SupportProofReceiptId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Receipt emitted only after `verifier_id` accepts a proof with the supplied
/// digest for the exact obligation and conclusion digests.
///
/// This type authenticates acceptance; it is not itself a proof checker. A
/// durable decoder must revalidate the receipt identity and trust or replay the
/// named verifier according to the run's proof policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupportProofReceipt {
    id: SupportProofReceiptId,
    obligation_id: SupportProofObligationId,
    verifier_id: SupportProofVerifierId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl SupportProofReceipt {
    /// Module-private issuance boundary. Hash consistency is not proof
    /// validity; future solver integrations must add an explicit verifier
    /// gateway here rather than exposing a receipt constructor to callers.
    fn from_accepted_proof(
        obligation_id: SupportProofObligationId,
        verifier_id: SupportProofVerifierId,
        conclusion_digest: [u8; 32],
        proof_digest: [u8; 32],
    ) -> Self {
        let id =
            derive_proof_receipt_id(obligation_id, verifier_id, conclusion_digest, proof_digest);
        Self {
            id,
            obligation_id,
            verifier_id,
            conclusion_digest,
            proof_digest,
        }
    }

    pub(crate) const fn id(self) -> SupportProofReceiptId {
        self.id
    }

    pub(crate) const fn obligation_id(self) -> SupportProofObligationId {
        self.obligation_id
    }

    pub(crate) const fn verifier_id(self) -> SupportProofVerifierId {
        self.verifier_id
    }

    pub(crate) const fn conclusion_digest(self) -> [u8; 32] {
        self.conclusion_digest
    }

    pub(crate) const fn proof_digest(self) -> [u8; 32] {
        self.proof_digest
    }

    pub(crate) fn validate(self) -> Result<(), SupportCellError> {
        let derived = derive_proof_receipt_id(
            self.obligation_id,
            self.verifier_id,
            self.conclusion_digest,
            self.proof_digest,
        );
        if derived != self.id {
            return Err(SupportCellError::ReceiptIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Typed claim implemented by support-cell proof layers.
///
/// The associated conclusion prevents, for example, attaching a mechanism
/// signature to an admission obligation. Layer identities live in the claim,
/// not in [`SupportCellId`].
pub(crate) trait SupportCellClaim: Clone + fmt::Debug + Eq {
    type Conclusion: Clone + fmt::Debug + Eq;

    fn claim_digest(&self) -> [u8; 32];
    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32];
    fn validate_conclusion(&self, conclusion: &Self::Conclusion) -> Result<(), SupportCellError>;
}

/// Exact extensional cardinality of one cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExactCardinalityClaim;

impl SupportCellClaim for ExactCardinalityClaim {
    type Conclusion = u128;

    fn claim_digest(&self) -> [u8; 32] {
        derive_scoped_digest(b"futuruna.explore.claim.exact-cardinality.v1", b"")
    }

    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.conclusion.exact-cardinality.v1",
            &conclusion.to_be_bytes(),
        )
    }

    fn validate_conclusion(&self, _conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        Ok(())
    }
}

/// Claim that a mapped-image materializer is injective on this exact cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InjectiveMappingClaim {
    materializer_id: SupportMaterializerId,
}

impl InjectiveMappingClaim {
    pub(crate) const fn new(materializer_id: SupportMaterializerId) -> Self {
        Self { materializer_id }
    }

    pub(crate) const fn materializer_id(self) -> SupportMaterializerId {
        self.materializer_id
    }
}

/// Typed conclusion marker for an accepted injectivity proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedInjective;

impl SupportCellClaim for InjectiveMappingClaim {
    type Conclusion = CertifiedInjective;

    fn claim_digest(&self) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.claim.injective-mapping.v1",
            &self.materializer_id.bytes(),
        )
    }

    fn conclusion_digest(&self, _conclusion: &Self::Conclusion) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.conclusion.injective-mapping.v1",
            &self.materializer_id.bytes(),
        )
    }

    fn validate_conclusion(&self, _conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        Ok(())
    }
}

/// Uniform admission classification for one admission layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionClassificationClaim {
    admission_id: AdmissionId,
}

impl AdmissionClassificationClaim {
    pub(crate) const fn new(admission_id: AdmissionId) -> Self {
        Self { admission_id }
    }

    pub(crate) const fn admission_id(self) -> AdmissionId {
        self.admission_id
    }
}

impl SupportCellClaim for AdmissionClassificationClaim {
    type Conclusion = AdmissionDecision;

    fn claim_digest(&self) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.claim.admission-classification.v1",
            &self.admission_id.bytes(),
        )
    }

    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32] {
        let tag = match conclusion {
            AdmissionDecision::Rejected => 0x01,
            AdmissionDecision::Admitted => 0x02,
        };
        derive_scoped_digest(
            b"futuruna.explore.conclusion.admission-classification.v1",
            &[tag],
        )
    }

    fn validate_conclusion(&self, _conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        Ok(())
    }
}

/// Uniform FIND classification for one question layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectionClassificationClaim {
    question_id: QuestionId,
}

impl SelectionClassificationClaim {
    pub(crate) const fn new(question_id: QuestionId) -> Self {
        Self { question_id }
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }
}

impl SupportCellClaim for SelectionClassificationClaim {
    type Conclusion = SelectionDecision;

    fn claim_digest(&self) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.claim.selection-classification.v1",
            &self.question_id.bytes(),
        )
    }

    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32] {
        let tag = match conclusion {
            SelectionDecision::NotSelected => 0x01,
            SelectionDecision::Selected => 0x02,
        };
        derive_scoped_digest(
            b"futuruna.explore.conclusion.selection-classification.v1",
            &[tag],
        )
    }

    fn validate_conclusion(&self, _conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        Ok(())
    }
}

/// Claim that a checked observer has one value over every member of a cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UniformValueClaim {
    observer_id: SupportObserverId,
    value_schema_digest: [u8; 32],
}

impl UniformValueClaim {
    pub(crate) const fn new(observer_id: SupportObserverId, value_schema_digest: [u8; 32]) -> Self {
        Self {
            observer_id,
            value_schema_digest,
        }
    }

    pub(crate) const fn observer_id(self) -> SupportObserverId {
        self.observer_id
    }

    pub(crate) const fn value_schema_digest(self) -> [u8; 32] {
        self.value_schema_digest
    }
}

impl SupportCellClaim for UniformValueClaim {
    type Conclusion = ExploreValue;

    fn claim_digest(&self) -> [u8; 32] {
        let mut hasher = CanonicalHasher::new(b"futuruna.explore.claim.uniform-value.v1");
        hasher.digest(self.observer_id.bytes());
        hasher.digest(self.value_schema_digest);
        hasher.finish()
    }

    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32] {
        let mut hasher = CanonicalHasher::new(b"futuruna.explore.conclusion.uniform-value.v1");
        hasher.digest(self.observer_id.bytes());
        hasher.digest(self.value_schema_digest);
        hasher.digest(canonical_explore_value_digest(conclusion));
        hasher.finish()
    }

    fn validate_conclusion(&self, _conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        // Type checking produced `value_schema_digest`; runtime values already
        // use canonical ExploreValue representation at this boundary.
        Ok(())
    }
}

/// Claim that every case in a cell has one normalized differential mechanism.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UniformMechanismClaim {
    request_id: MechanismRequestId,
}

impl UniformMechanismClaim {
    pub(crate) const fn new(request_id: MechanismRequestId) -> Self {
        Self { request_id }
    }

    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }
}

impl SupportCellClaim for UniformMechanismClaim {
    type Conclusion = MechanismSignatureId;

    fn claim_digest(&self) -> [u8; 32] {
        derive_scoped_digest(
            b"futuruna.explore.claim.uniform-mechanism.v1",
            &self.request_id.bytes(),
        )
    }

    fn conclusion_digest(&self, conclusion: &Self::Conclusion) -> [u8; 32] {
        let mut hasher = CanonicalHasher::new(b"futuruna.explore.conclusion.uniform-mechanism.v1");
        hasher.digest(self.request_id.bytes());
        hasher.digest(conclusion.bytes());
        hasher.finish()
    }

    fn validate_conclusion(&self, conclusion: &Self::Conclusion) -> Result<(), SupportCellError> {
        if conclusion.request_id() != self.request_id {
            return Err(SupportCellError::MechanismRequestMismatch);
        }
        Ok(())
    }
}

/// One typed proof obligation over a stable cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportCellObligation<C: SupportCellClaim> {
    id: SupportProofObligationId,
    cell_id: SupportCellId,
    claim: C,
}

impl<C: SupportCellClaim> SupportCellObligation<C> {
    pub(crate) fn new(cell: &SupportCell, claim: C) -> Result<Self, SupportCellError> {
        cell.validate()?;
        let id = derive_cell_obligation_id(cell.id, claim.claim_digest());
        Ok(Self {
            id,
            cell_id: cell.id,
            claim,
        })
    }

    pub(super) fn restore_from_journal_codec(cell_id: SupportCellId, claim: C) -> Self {
        let id = derive_cell_obligation_id(cell_id, claim.claim_digest());
        Self { id, cell_id, claim }
    }

    pub(crate) const fn id(&self) -> SupportProofObligationId {
        self.id
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn claim(&self) -> &C {
        &self.claim
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        let derived = derive_cell_obligation_id(self.cell_id, self.claim.claim_digest());
        if derived != self.id {
            return Err(SupportCellError::ObligationIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Content identity of typed cell evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportCellEvidenceId([u8; 32]);

impl SupportCellEvidenceId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Accepted, typed conclusion over one support cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportCellEvidence<C: SupportCellClaim> {
    id: SupportCellEvidenceId,
    obligation: SupportCellObligation<C>,
    conclusion: C::Conclusion,
    receipt: SupportProofReceipt,
}

impl<C: SupportCellClaim> SupportCellEvidence<C> {
    pub(crate) fn from_accepted_proof(
        obligation: SupportCellObligation<C>,
        conclusion: C::Conclusion,
        receipt: SupportProofReceipt,
    ) -> Result<Self, SupportCellError> {
        obligation.validate()?;
        obligation.claim.validate_conclusion(&conclusion)?;
        receipt.validate()?;
        if receipt.obligation_id != obligation.id {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim.conclusion_digest(&conclusion);
        if receipt.conclusion_digest != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let id = derive_evidence_id(obligation.id, conclusion_digest, receipt.id);
        Ok(Self {
            id,
            obligation,
            conclusion,
            receipt,
        })
    }

    pub(crate) const fn id(&self) -> SupportCellEvidenceId {
        self.id
    }

    pub(crate) const fn obligation(&self) -> &SupportCellObligation<C> {
        &self.obligation
    }

    pub(crate) const fn conclusion(&self) -> &C::Conclusion {
        &self.conclusion
    }

    pub(crate) const fn receipt(&self) -> SupportProofReceipt {
        self.receipt
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        self.obligation.validate()?;
        self.obligation
            .claim
            .validate_conclusion(&self.conclusion)?;
        self.receipt.validate()?;
        if self.receipt.obligation_id != self.obligation.id {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = self.obligation.claim.conclusion_digest(&self.conclusion);
        if self.receipt.conclusion_digest != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let derived = derive_evidence_id(self.obligation.id, conclusion_digest, self.receipt.id);
        if derived != self.id {
            return Err(SupportCellError::EvidenceIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Narrow issuance gateway for the checked relational-region verifier.
///
/// The sibling proof module can provide only an opaque token returned by its
/// producer-bound verifier.  This child module retains access to the private
/// receipt constructor, checks the exact typed obligation/conclusion binding,
/// and never accepts a caller-supplied verifier or proof digest.
pub(crate) mod relational_region_proof_gateway {
    use super::*;
    use crate::explore::relational_region_proof::{
        RelationalRegionEvidenceRole, VerifiedRelationalRegionProof,
    };

    const CARDINALITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-region.cardinality-verifier.v1";
    const ADMISSION_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-region.admission-verifier.v1";
    const SELECTION_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-region.selection-verifier.v1";

    pub(crate) fn cardinality(
        proof: &VerifiedRelationalRegionProof,
        obligation: SupportCellObligation<ExactCardinalityClaim>,
        conclusion: u128,
    ) -> Result<SupportCellEvidence<ExactCardinalityClaim>, SupportCellError> {
        accepted(
            proof,
            RelationalRegionEvidenceRole::Cardinality,
            obligation,
            conclusion,
            CARDINALITY_VERIFIER_V1,
        )
    }

    pub(crate) fn admission(
        proof: &VerifiedRelationalRegionProof,
        obligation: SupportCellObligation<AdmissionClassificationClaim>,
        conclusion: AdmissionDecision,
    ) -> Result<SupportCellEvidence<AdmissionClassificationClaim>, SupportCellError> {
        accepted(
            proof,
            RelationalRegionEvidenceRole::Admission,
            obligation,
            conclusion,
            ADMISSION_VERIFIER_V1,
        )
    }

    pub(crate) fn selection(
        proof: &VerifiedRelationalRegionProof,
        obligation: SupportCellObligation<SelectionClassificationClaim>,
        conclusion: SelectionDecision,
    ) -> Result<SupportCellEvidence<SelectionClassificationClaim>, SupportCellError> {
        accepted(
            proof,
            RelationalRegionEvidenceRole::Selection,
            obligation,
            conclusion,
            SELECTION_VERIFIER_V1,
        )
    }

    fn accepted<C: SupportCellClaim>(
        proof: &VerifiedRelationalRegionProof,
        role: RelationalRegionEvidenceRole,
        obligation: SupportCellObligation<C>,
        conclusion: C::Conclusion,
        verifier_contract: &[u8],
    ) -> Result<SupportCellEvidence<C>, SupportCellError> {
        obligation.validate()?;
        obligation.claim.validate_conclusion(&conclusion)?;
        let binding = proof.evidence_binding(role);
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim.conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(verifier_contract),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

/// Narrow issuance gateway for the producer-bound relational case-image
/// verifier. A hash-consistent artifact is not authority: callers must supply
/// the opaque token returned only after the support plan's complete producer
/// chain has been replayed.
pub(crate) mod relational_case_image_proof_gateway {
    use super::*;
    use crate::explore::relational_support_planner::VerifiedRelationalCaseImageInjectivityProof;

    const INJECTIVITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-case-image.injectivity-verifier.v1";
    const CARDINALITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-case-image.cardinality-verifier.v1";

    pub(crate) fn injectivity(
        proof: &VerifiedRelationalCaseImageInjectivityProof,
        obligation: SupportCellObligation<InjectiveMappingClaim>,
    ) -> Result<SupportCellEvidence<InjectiveMappingClaim>, SupportCellError> {
        if obligation.cell_id() != proof.artifact().case_cell_id() {
            return Err(SupportCellError::EvidenceCellMismatch);
        }
        if obligation.claim().materializer_id() != proof.artifact().case_materializer_id() {
            return Err(SupportCellError::MaterializerMismatch);
        }
        accepted(
            obligation,
            CertifiedInjective,
            proof.injectivity_binding(),
            INJECTIVITY_VERIFIER_V1,
        )
    }

    pub(crate) fn cardinality(
        proof: &VerifiedRelationalCaseImageInjectivityProof,
        obligation: SupportCellObligation<ExactCardinalityClaim>,
        conclusion: u128,
    ) -> Result<SupportCellEvidence<ExactCardinalityClaim>, SupportCellError> {
        if obligation.cell_id() != proof.artifact().case_cell_id() {
            return Err(SupportCellError::EvidenceCellMismatch);
        }
        if proof.artifact().exact_case_cardinality() != Some(conclusion) {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let binding = proof
            .cardinality_binding()
            .ok_or(SupportCellError::ReceiptConclusionMismatch)?;
        accepted(obligation, conclusion, binding, CARDINALITY_VERIFIER_V1)
    }

    fn accepted<C: SupportCellClaim>(
        obligation: SupportCellObligation<C>,
        conclusion: C::Conclusion,
        binding: crate::explore::relational_support_planner::RelationalCaseImageEvidenceBinding,
        verifier_contract: &[u8],
    ) -> Result<SupportCellEvidence<C>, SupportCellError> {
        obligation.validate()?;
        obligation.claim.validate_conclusion(&conclusion)?;
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim.conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(verifier_contract),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

/// Narrow issuance gateway for the plan-bound source-image exactness verifier.
/// A decoded artifact or population-root digest cannot mint evidence: callers
/// must supply the opaque token returned only after replaying the canonical
/// assignment product and `SourceRowImage` against the retained support plan.
pub(crate) mod relational_source_image_exactness_gateway {
    use super::*;
    use crate::explore::relational_source_image_exactness::{
        RelationalSourceImageEvidenceBinding, VerifiedRelationalSourceImageExactnessProof,
    };

    const INJECTIVITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-source-image-exactness.injectivity-verifier.v1";
    const CARDINALITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-source-image-exactness.cardinality-verifier.v1";

    pub(crate) fn injectivity(
        proof: &VerifiedRelationalSourceImageExactnessProof,
    ) -> Result<SupportCellEvidence<InjectiveMappingClaim>, SupportCellError> {
        let source_cell = checked_source_cell(proof)?;
        let obligation = SupportCellObligation::new(
            source_cell,
            InjectiveMappingClaim::new(source_cell.materializer_id()),
        )?;
        accepted(
            obligation,
            CertifiedInjective,
            proof.injectivity_binding(),
            INJECTIVITY_VERIFIER_V1,
        )
    }

    pub(crate) fn cardinality(
        proof: &VerifiedRelationalSourceImageExactnessProof,
    ) -> Result<SupportCellEvidence<ExactCardinalityClaim>, SupportCellError> {
        let obligation =
            SupportCellObligation::new(checked_source_cell(proof)?, ExactCardinalityClaim)?;
        let conclusion = proof.artifact().exact_source_cardinality();
        accepted(
            obligation,
            conclusion,
            proof.cardinality_binding(),
            CARDINALITY_VERIFIER_V1,
        )
    }

    fn checked_source_cell(
        proof: &VerifiedRelationalSourceImageExactnessProof,
    ) -> Result<&SupportCell, SupportCellError> {
        let source_cell = proof.source_cell();
        if source_cell.id() != proof.artifact().source_row_cell_id() {
            return Err(SupportCellError::EvidenceCellMismatch);
        }
        if source_cell.materializer_id() != proof.artifact().source_materializer_id() {
            return Err(SupportCellError::MaterializerMismatch);
        }
        Ok(source_cell)
    }

    fn accepted<C: SupportCellClaim>(
        obligation: SupportCellObligation<C>,
        conclusion: C::Conclusion,
        binding: RelationalSourceImageEvidenceBinding<C>,
        verifier_contract: &[u8],
    ) -> Result<SupportCellEvidence<C>, SupportCellError> {
        obligation.validate()?;
        obligation.claim().validate_conclusion(&conclusion)?;
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim().conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(verifier_contract),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

/// Narrow issuance gateway for injectivity restricted from one durably proved
/// mapped-image root to the exact children of its accepted partition. The
/// opaque token is available only after journal replay has matched the named
/// durable root evidence and independently rebuilt the complete partition.
pub(crate) mod relational_case_chunk_partition_gateway {
    use super::*;
    use crate::explore::relational_bounded_chunk_partition::VerifiedRelationalCaseChunkPartition;

    const RESTRICTED_INJECTIVITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-case-chunk.restricted-injectivity-verifier.v1";

    pub(crate) fn injectivity(
        proof: &VerifiedRelationalCaseChunkPartition,
        child_ordinal: usize,
    ) -> Result<SupportCellEvidence<InjectiveMappingClaim>, SupportCellError> {
        let (chunk, binding) = proof
            .child_and_injectivity_binding(child_ordinal)
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        let child = chunk.cell();
        let ordinal =
            u128::try_from(child_ordinal).map_err(|_| SupportCellError::EvidenceCellMismatch)?;
        if !child.space().is_mapped_image()
            || binding.ordinal() != ordinal
            || binding.chunk_id() != chunk.descriptor().id()
            || binding.child_cell_id() != child.id()
            || binding.child_materializer_id() != child.materializer_id()
        {
            return Err(SupportCellError::EvidenceCellMismatch);
        }

        let claim = InjectiveMappingClaim::new(child.materializer_id());
        let obligation = SupportCellObligation::new(child, claim)?;
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion = CertifiedInjective;
        let conclusion_digest = claim.conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(RESTRICTED_INJECTIVITY_VERIFIER_V1),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

/// Narrow issuance gateway for one exhaustively evaluated bounded chunk.
/// Structural replay yields the opaque token; decoded outcome bytes alone can
/// never mint cardinality, admission, selection, or restricted-injectivity
/// evidence. A multi-run mapped image receives fresh injectivity evidence for
/// every proper child restriction. A homogeneous one-run chunk keeps the
/// injectivity evidence already installed by the enclosing chunk partition.
pub(crate) mod relational_classified_sweep_gateway {
    use super::*;
    use crate::explore::relational_classified_sweep::VerifiedRelationalClassifiedChunk;

    const INJECTIVITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-classified-sweep.injectivity-verifier.v1";
    const CARDINALITY_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-classified-sweep.cardinality-verifier.v1";
    const ADMISSION_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-classified-sweep.admission-verifier.v1";
    const SELECTION_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-classified-sweep.selection-verifier.v1";

    pub(crate) fn injectivity(
        proof: &VerifiedRelationalClassifiedChunk,
        run_ordinal: usize,
    ) -> Result<SupportCellEvidence<InjectiveMappingClaim>, SupportCellError> {
        let (run, bindings) = proof
            .run_and_bindings(run_ordinal)
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        let binding = bindings
            .injectivity()
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        let claim = InjectiveMappingClaim::new(run.cell().materializer_id());
        accepted(
            run.cell(),
            claim,
            CertifiedInjective,
            binding,
            INJECTIVITY_VERIFIER_V1,
        )
    }

    pub(crate) fn cardinality(
        proof: &VerifiedRelationalClassifiedChunk,
        run_ordinal: usize,
    ) -> Result<SupportCellEvidence<ExactCardinalityClaim>, SupportCellError> {
        let (run, bindings) = proof
            .run_and_bindings(run_ordinal)
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        accepted(
            run.cell(),
            ExactCardinalityClaim,
            run.descriptor().cardinality(),
            bindings.cardinality(),
            CARDINALITY_VERIFIER_V1,
        )
    }

    pub(crate) fn admission(
        proof: &VerifiedRelationalClassifiedChunk,
        run_ordinal: usize,
    ) -> Result<SupportCellEvidence<AdmissionClassificationClaim>, SupportCellError> {
        let (run, bindings) = proof
            .run_and_bindings(run_ordinal)
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        accepted(
            run.cell(),
            AdmissionClassificationClaim::new(proof.artifact().admission_id()),
            run.descriptor().outcome().admission(),
            bindings.admission(),
            ADMISSION_VERIFIER_V1,
        )
    }

    pub(crate) fn selection(
        proof: &VerifiedRelationalClassifiedChunk,
        run_ordinal: usize,
    ) -> Result<SupportCellEvidence<SelectionClassificationClaim>, SupportCellError> {
        let (run, bindings) = proof
            .run_and_bindings(run_ordinal)
            .ok_or(SupportCellError::EvidenceCellMismatch)?;
        let conclusion = run
            .descriptor()
            .outcome()
            .selection()
            .ok_or(SupportCellError::ReceiptConclusionMismatch)?;
        let binding = bindings
            .selection()
            .ok_or(SupportCellError::ReceiptConclusionMismatch)?;
        accepted(
            run.cell(),
            SelectionClassificationClaim::new(proof.artifact().question_id()),
            conclusion,
            binding,
            SELECTION_VERIFIER_V1,
        )
    }

    fn accepted<C: SupportCellClaim>(
        cell: &SupportCell,
        claim: C,
        conclusion: C::Conclusion,
        binding: crate::explore::relational_classified_sweep::RelationalClassifiedEvidenceBinding,
        verifier_contract: &[u8],
    ) -> Result<SupportCellEvidence<C>, SupportCellError> {
        let obligation = SupportCellObligation::new(cell, claim)?;
        obligation.claim().validate_conclusion(&conclusion)?;
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim().conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(verifier_contract),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

/// Narrow issuance gateway for the plan-bound uniform-admission verifier.
/// Decoded artifacts cannot call this gateway: only the opaque token returned
/// after replaying the installed plan's complete recognized recipe is
/// accepted.
pub(crate) mod relational_uniform_admission_proof_gateway {
    use super::*;
    use crate::explore::relational_uniform_admission_proof::VerifiedRelationalUniformAdmissionProof;

    const ADMISSION_VERIFIER_V1: &[u8] =
        b"futuruna.explore.relational-uniform-admission.verifier.v1";

    pub(crate) fn admission(
        proof: &VerifiedRelationalUniformAdmissionProof,
        obligation: SupportCellObligation<AdmissionClassificationClaim>,
        conclusion: AdmissionDecision,
    ) -> Result<SupportCellEvidence<AdmissionClassificationClaim>, SupportCellError> {
        obligation.validate()?;
        obligation.claim().validate_conclusion(&conclusion)?;
        if obligation.cell_id() != proof.artifact().case_cell_id() {
            return Err(SupportCellError::EvidenceCellMismatch);
        }
        if obligation.claim().admission_id() != proof.artifact().admission_id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        if conclusion != proof.artifact().decision() {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let binding = proof.evidence_binding();
        if binding.obligation_id() != obligation.id() {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        let conclusion_digest = obligation.claim().conclusion_digest(&conclusion);
        if binding.conclusion_digest() != conclusion_digest {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(ADMISSION_VERIFIER_V1),
            conclusion_digest,
            binding.proof_digest(),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt)
    }
}

impl SupportCellEvidence<ExactCardinalityClaim> {
    pub(crate) const fn exact_cardinality(&self) -> u128 {
        self.conclusion
    }
}

impl SupportCell {
    /// Use accepted exact image/count evidence without confusing it with the
    /// expression's coordinate count.
    pub(crate) fn cardinality_from_evidence(
        &self,
        evidence: &SupportCellEvidence<ExactCardinalityClaim>,
    ) -> Result<SupportCardinality, SupportCellError> {
        self.validate_evidence(evidence)?;
        let actual = evidence.exact_cardinality();
        let structural_lower_bound = self.cardinality().lower_bound();
        if actual < structural_lower_bound {
            return Err(SupportCellError::CardinalityBelowLowerBound {
                lower_bound: structural_lower_bound,
                actual,
            });
        }
        if let Some(structural) = self.cardinality().exact() {
            if structural != actual {
                return Err(SupportCellError::CardinalityMismatch {
                    expected: structural,
                    actual,
                });
            }
        }
        if self.space.is_mapped_image() {
            if let Some(coordinate_count) = self.coordinate_cardinality().exact() {
                if actual > coordinate_count {
                    return Err(SupportCellError::ImageCardinalityExceedsCoordinates {
                        image_count: actual,
                        coordinate_count,
                    });
                }
            }
        }
        Ok(SupportCardinality::Exact(actual))
    }
}

/// Canonical obligation that named children form a nonempty, pairwise-disjoint
/// union exactly equal to their parent cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportPartitionObligation {
    id: SupportProofObligationId,
    parent_id: SupportCellId,
    child_ids: Box<[SupportCellId]>,
}

impl SupportPartitionObligation {
    pub(crate) fn new(
        parent: &SupportCell,
        children: &[SupportCell],
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if children.is_empty() {
            return Err(SupportCellError::EmptyPartition);
        }
        let mut child_ids = Vec::with_capacity(children.len());
        for child in children {
            child.validate()?;
            if child.space != parent.space {
                return Err(SupportCellError::PartitionSpaceMismatch);
            }
            if child.materializer_id != parent.materializer_id {
                return Err(SupportCellError::MaterializerMismatch);
            }
            child_ids.push(child.id);
        }
        child_ids.sort();
        if child_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SupportCellError::DuplicatePartitionChild);
        }
        let id = derive_partition_obligation_id(parent.id, &child_ids);
        Ok(Self {
            id,
            parent_id: parent.id,
            child_ids: child_ids.into_boxed_slice(),
        })
    }

    pub(crate) const fn id(&self) -> SupportProofObligationId {
        self.id
    }

    pub(crate) const fn parent_id(&self) -> SupportCellId {
        self.parent_id
    }

    pub(crate) fn child_ids(&self) -> &[SupportCellId] {
        &self.child_ids
    }

    /// Digest of the exact disjoint-union conclusion checked by a partition
    /// proof. Nonempty children are part of this conclusion.
    pub(crate) fn conclusion_digest(&self) -> [u8; 32] {
        let mut hasher = CanonicalHasher::new(b"futuruna.explore.conclusion.disjoint-union.v1");
        hasher.digest(self.id.bytes());
        hasher.finish()
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        if self.child_ids.is_empty() {
            return Err(SupportCellError::EmptyPartition);
        }
        if self.child_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SupportCellError::NonCanonicalPartitionChildren);
        }
        let derived = derive_partition_obligation_id(self.parent_id, &self.child_ids);
        if derived != self.id {
            return Err(SupportCellError::ObligationIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// How a disjoint-union certificate was established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportPartitionKind {
    /// Checked structurally from a complete half-open ordinal interval cover.
    OrdinalIntervalCover,
    /// Accepted by an explicitly named external proof verifier.
    AcceptedDisjointUnion,
    /// A structural coordinate interval cover lifted through a materializer
    /// proved injective over the mapped-image parent.
    MappedInjectiveOrdinalCover,
    /// A structural interval cover of one factor, lifted through the unchanged
    /// remainder of a Cartesian coordinate product.
    ProductFactorCover,
    /// A product-factor coordinate cover additionally lifted through an
    /// injective mapped-image materializer.
    MappedInjectiveProductFactorCover,
    /// A structural mixed-radix rank-interval cover lifted through an
    /// injective mapped-image materializer.
    MappedInjectiveProductRankIntervalCover,
}

impl SupportPartitionKind {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::OrdinalIntervalCover => 0x01,
            Self::AcceptedDisjointUnion => 0x02,
            Self::MappedInjectiveOrdinalCover => 0x03,
            Self::ProductFactorCover => 0x04,
            Self::MappedInjectiveProductFactorCover => 0x05,
            Self::MappedInjectiveProductRankIntervalCover => 0x06,
        }
    }
}

/// Content identity of one accepted parent-to-children partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportPartitionId([u8; 32]);

impl SupportPartitionId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Certificate that children are nonempty, pairwise disjoint, and exactly
/// cover a parent. Downstream consumers may aggregate evidence by cells using
/// this certificate without enumerating retained examples or per-case IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportPartitionCertificate {
    id: SupportPartitionId,
    kind: SupportPartitionKind,
    obligation: SupportPartitionObligation,
    cardinality: SupportCardinality,
    receipt: SupportProofReceipt,
}

impl SupportPartitionCertificate {
    /// Prove a complete partition by structural half-open interval checks.
    ///
    /// This constructor is intentionally unavailable for mapped images:
    /// disjoint coordinate intervals can still map to overlapping rows. Such a
    /// partition must use [`Self::from_accepted_disjoint_union`] or first carry
    /// accepted injectivity evidence.
    pub(crate) fn ordinal_interval_cover(
        parent: &SupportCell,
        mut children: Vec<SupportCell>,
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if parent.space.is_mapped_image() {
            return Err(SupportCellError::MappedImageNeedsPartitionProof);
        }
        let (parent_start, parent_end) = validate_ordinal_interval_cover(parent, &mut children)?;

        let obligation = SupportPartitionObligation::new(parent, &children)?;
        let cardinality = SupportCardinality::Exact(parent_end - parent_start);
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.ordinal-interval-cover-check.v1",
        );
        let proof_digest = ordinal_partition_proof_digest(parent, &children);
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id,
            verifier,
            obligation.conclusion_digest(),
            proof_digest,
        );
        Self::finish(
            SupportPartitionKind::OrdinalIntervalCover,
            obligation,
            cardinality,
            receipt,
        )
    }

    /// Lift a complete coordinate interval cover into a mapped-image
    /// partition using accepted injectivity evidence for the whole parent.
    ///
    /// Injectivity on the parent implies injectivity on each child and makes
    /// images of disjoint child coordinate sets disjoint. This is the checked
    /// bridge needed to refine an income interval without materializing every
    /// case merely because the case population is represented as an image.
    pub(crate) fn mapped_injective_ordinal_cover(
        parent: &SupportCell,
        mut children: Vec<SupportCell>,
        injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if !parent.space.is_mapped_image() {
            return Err(SupportCellError::InjectivityForUnmappedCell);
        }
        parent.validate_evidence(injectivity)?;
        if injectivity.obligation().claim().materializer_id() != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        let (parent_start, parent_end) = validate_ordinal_interval_cover(parent, &mut children)?;

        let obligation = SupportPartitionObligation::new(parent, &children)?;
        let cardinality = SupportCardinality::Exact(parent_end - parent_start);
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.mapped-injective-ordinal-cover-check.v1",
        );
        let mut proof =
            CanonicalHasher::new(b"futuruna.explore.mapped-injective-ordinal-cover-proof.v1");
        proof.digest(injectivity.id().bytes());
        proof.digest(injectivity.receipt().id().bytes());
        proof.digest(ordinal_partition_proof_digest(parent, &children));
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id,
            verifier,
            obligation.conclusion_digest(),
            proof.finish(),
        );
        Self::finish(
            SupportPartitionKind::MappedInjectiveOrdinalCover,
            obligation,
            cardinality,
            receipt,
        )
    }

    /// Split exactly one interval factor while preserving every other factor
    /// in a non-image Cartesian support cell.
    pub(crate) fn product_factor_cover(
        parent: &SupportCell,
        mut children: Vec<SupportCell>,
        factor_index: usize,
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if parent.space.is_mapped_image() {
            return Err(SupportCellError::MappedImageNeedsPartitionProof);
        }
        validate_product_factor_cover(parent, &mut children, factor_index)?;
        let obligation = SupportPartitionObligation::new(parent, &children)?;
        let cardinality = partition_cardinality(parent, &children)?;
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.product-factor-cover-check.v1",
        );
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id,
            verifier,
            obligation.conclusion_digest(),
            product_factor_partition_proof_digest(parent, &children, factor_index),
        );
        Self::finish(
            SupportPartitionKind::ProductFactorCover,
            obligation,
            cardinality,
            receipt,
        )
    }

    /// Lift a one-factor Cartesian split through an injective mapped-image
    /// materializer. This lets a refiner bisect income while retaining the
    /// full independent commune/profile product in each child.
    pub(crate) fn mapped_injective_product_factor_cover(
        parent: &SupportCell,
        mut children: Vec<SupportCell>,
        factor_index: usize,
        injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if !parent.space.is_mapped_image() {
            return Err(SupportCellError::InjectivityForUnmappedCell);
        }
        parent.validate_evidence(injectivity)?;
        if injectivity.obligation().claim().materializer_id() != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        validate_product_factor_cover(parent, &mut children, factor_index)?;
        let obligation = SupportPartitionObligation::new(parent, &children)?;
        let cardinality = parent.coordinate_cardinality();
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.mapped-injective-product-factor-cover-check.v1",
        );
        let mut proof = CanonicalHasher::new(
            b"futuruna.explore.mapped-injective-product-factor-cover-proof.v1",
        );
        proof.digest(injectivity.id().bytes());
        proof.digest(injectivity.receipt().id().bytes());
        proof.digest(product_factor_partition_proof_digest(
            parent,
            &children,
            factor_index,
        ));
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id,
            verifier,
            obligation.conclusion_digest(),
            proof.finish(),
        );
        Self::finish(
            SupportPartitionKind::MappedInjectiveProductFactorCover,
            obligation,
            cardinality,
            receipt,
        )
    }

    /// Lift a complete mixed-radix rank-interval cover through an injective
    /// mapped-image materializer. The parent may be the full product or an
    /// already ranked proper restriction; every child retains the identical
    /// ordered factor basis.
    pub(crate) fn mapped_injective_product_rank_interval_cover(
        parent: &SupportCell,
        mut children: Vec<SupportCell>,
        injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    ) -> Result<Self, SupportCellError> {
        parent.validate()?;
        if !parent.space.is_mapped_image() {
            return Err(SupportCellError::InjectivityForUnmappedCell);
        }
        parent.validate_evidence(injectivity)?;
        if injectivity.obligation().claim().materializer_id() != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        let (rank_start, rank_end_exclusive) =
            validate_product_rank_interval_cover(parent, &mut children)?;
        let obligation = SupportPartitionObligation::new(parent, &children)?;
        let cardinality = SupportCardinality::Exact(rank_end_exclusive - rank_start);
        let verifier = SupportProofVerifierId::from_canonical_preimage(
            b"futuruna.explore.mapped-injective-product-rank-interval-cover-check.v1",
        );
        let mut proof = CanonicalHasher::new(
            b"futuruna.explore.mapped-injective-product-rank-interval-cover-proof.v1",
        );
        proof.digest(injectivity.id().bytes());
        proof.digest(injectivity.receipt().id().bytes());
        proof.digest(product_rank_interval_partition_proof_digest(
            parent, &children,
        ));
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id,
            verifier,
            obligation.conclusion_digest(),
            proof.finish(),
        );
        Self::finish(
            SupportPartitionKind::MappedInjectiveProductRankIntervalCover,
            obligation,
            cardinality,
            receipt,
        )
    }

    /// Attach the output of a proof checker capable of establishing a general
    /// nonempty disjoint union (including unions, differences, congruences, or
    /// mapped images). The caller must invoke this only after the named verifier
    /// has accepted the proof committed by `receipt.proof_digest()`.
    pub(crate) fn from_accepted_disjoint_union(
        parent: &SupportCell,
        children: &[SupportCell],
        receipt: SupportProofReceipt,
    ) -> Result<Self, SupportCellError> {
        let obligation = SupportPartitionObligation::new(parent, children)?;
        let cardinality = partition_cardinality(parent, children)?;
        Self::finish(
            SupportPartitionKind::AcceptedDisjointUnion,
            obligation,
            cardinality,
            receipt,
        )
    }

    fn finish(
        kind: SupportPartitionKind,
        obligation: SupportPartitionObligation,
        cardinality: SupportCardinality,
        receipt: SupportProofReceipt,
    ) -> Result<Self, SupportCellError> {
        obligation.validate()?;
        receipt.validate()?;
        if receipt.obligation_id != obligation.id {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        if receipt.conclusion_digest != obligation.conclusion_digest() {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let id = derive_partition_id(kind, obligation.id, cardinality, receipt.id);
        Ok(Self {
            id,
            kind,
            obligation,
            cardinality,
            receipt,
        })
    }

    pub(crate) const fn id(&self) -> SupportPartitionId {
        self.id
    }

    pub(crate) const fn kind(&self) -> SupportPartitionKind {
        self.kind
    }

    pub(crate) const fn parent_id(&self) -> SupportCellId {
        self.obligation.parent_id
    }

    pub(crate) fn child_ids(&self) -> &[SupportCellId] {
        &self.obligation.child_ids
    }

    pub(crate) const fn cardinality(&self) -> SupportCardinality {
        self.cardinality
    }

    pub(crate) const fn receipt(&self) -> SupportProofReceipt {
        self.receipt
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        self.obligation.validate()?;
        self.receipt.validate()?;
        if self.receipt.obligation_id != self.obligation.id {
            return Err(SupportCellError::ReceiptObligationMismatch);
        }
        if self.receipt.conclusion_digest != self.obligation.conclusion_digest() {
            return Err(SupportCellError::ReceiptConclusionMismatch);
        }
        let derived = derive_partition_id(
            self.kind,
            self.obligation.id,
            self.cardinality,
            self.receipt.id,
        );
        if derived != self.id {
            return Err(SupportCellError::PartitionIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Content identity of one resumable materializer checkpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportMaterializationCursorId([u8; 32]);

impl SupportMaterializationCursorId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Resumable cursor over a cell's producer coordinates.
///
/// `next_coordinate_ordinal` is local progress inside the materializer. It is
/// not a case identity and never enters [`SupportCellId`]. The opaque checkpoint
/// is authenticated so a backend can resume dependent joins or other
/// non-rank-based enumeration without this foundation knowing its format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportMaterializationCursor {
    version: u32,
    id: SupportMaterializationCursorId,
    cell_id: SupportCellId,
    materializer_id: SupportMaterializerId,
    next_coordinate_ordinal: u128,
    checkpoint: Box<[u8]>,
}

impl SupportMaterializationCursor {
    pub(super) fn restore_from_journal_codec(
        version: u32,
        cell_id: SupportCellId,
        materializer_id: SupportMaterializerId,
        next_coordinate_ordinal: u128,
        checkpoint: Box<[u8]>,
    ) -> Result<Self, SupportCellError> {
        if version != SUPPORT_MATERIALIZATION_CURSOR_VERSION {
            return Err(SupportCellError::UnsupportedCursorVersion(version));
        }
        Self::derive(
            cell_id,
            materializer_id,
            next_coordinate_ordinal,
            checkpoint,
        )
    }

    pub(crate) fn at_start(cell: &SupportCell) -> Result<Self, SupportCellError> {
        cell.validate()?;
        Self::derive(cell.id, cell.materializer_id, 0, Box::new([]))
    }

    pub(crate) fn advance(
        &self,
        cell: &SupportCell,
        next_coordinate_ordinal: u128,
        checkpoint: impl Into<Box<[u8]>>,
    ) -> Result<Self, SupportCellError> {
        self.validate_for(cell)?;
        if next_coordinate_ordinal < self.next_coordinate_ordinal {
            return Err(SupportCellError::CursorRegressed {
                previous: self.next_coordinate_ordinal,
                next: next_coordinate_ordinal,
            });
        }
        Self::derive(
            self.cell_id,
            self.materializer_id,
            next_coordinate_ordinal,
            checkpoint.into(),
        )
        .and_then(|next| {
            next.validate_for(cell)?;
            Ok(next)
        })
    }

    /// Validate and adopt a durable checkpoint for this exact support/mapping.
    pub(crate) fn resume(cell: &SupportCell, checkpoint: Self) -> Result<Self, SupportCellError> {
        checkpoint.validate_for(cell)?;
        Ok(checkpoint)
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn id(&self) -> SupportMaterializationCursorId {
        self.id
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn materializer_id(&self) -> SupportMaterializerId {
        self.materializer_id
    }

    pub(crate) const fn next_coordinate_ordinal(&self) -> u128 {
        self.next_coordinate_ordinal
    }

    pub(crate) fn checkpoint(&self) -> &[u8] {
        &self.checkpoint
    }

    pub(crate) const fn is_complete(&self, cell: &SupportCell) -> bool {
        matches!(
            cell.coordinate_cardinality(),
            SupportCardinality::Exact(count) if self.next_coordinate_ordinal == count
        )
    }

    pub(crate) fn validate_for(&self, cell: &SupportCell) -> Result<(), SupportCellError> {
        cell.validate()?;
        self.validate()?;
        if self.cell_id != cell.id {
            return Err(SupportCellError::CursorCellMismatch);
        }
        if self.materializer_id != cell.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        if let Some(count) = cell.coordinate_cardinality().exact() {
            if self.next_coordinate_ordinal > count {
                return Err(SupportCellError::CursorBeyondSupport {
                    next: self.next_coordinate_ordinal,
                    coordinate_count: count,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), SupportCellError> {
        if self.version != SUPPORT_MATERIALIZATION_CURSOR_VERSION {
            return Err(SupportCellError::UnsupportedCursorVersion(self.version));
        }
        let derived = derive_cursor_id(
            self.version,
            self.cell_id,
            self.materializer_id,
            self.next_coordinate_ordinal,
            &self.checkpoint,
        );
        if derived != self.id {
            return Err(SupportCellError::CursorIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }

    fn derive(
        cell_id: SupportCellId,
        materializer_id: SupportMaterializerId,
        next_coordinate_ordinal: u128,
        checkpoint: Box<[u8]>,
    ) -> Result<Self, SupportCellError> {
        let version = SUPPORT_MATERIALIZATION_CURSOR_VERSION;
        let id = derive_cursor_id(
            version,
            cell_id,
            materializer_id,
            next_coordinate_ordinal,
            &checkpoint,
        );
        Ok(Self {
            version,
            id,
            cell_id,
            materializer_id,
            next_coordinate_ordinal,
            checkpoint,
        })
    }
}

/// Content identity of one privacy-safe retained example reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportExampleId {
    cell_id: SupportCellId,
    digest: [u8; 32],
}

impl SupportExampleId {
    pub(crate) fn from_canonical_example_digest(
        cell_id: SupportCellId,
        canonical_example_digest: [u8; 32],
    ) -> Self {
        let mut hasher = CanonicalHasher::new(SUPPORT_EXAMPLE_HASH_V1);
        hasher.digest(cell_id.bytes());
        hasher.digest(canonical_example_digest);
        Self {
            cell_id,
            digest: hasher.finish(),
        }
    }

    pub(crate) const fn cell_id(self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportExampleRetention {
    Inserted,
    AlreadyRetained,
    CapReached,
}

/// Bounded display/debug examples attached to a cell.
///
/// This collection is intentionally not support evidence: changing its cap or
/// members cannot change cell identity, cardinality, partitions, or proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetainedSupportExamples {
    cell_id: SupportCellId,
    cap: usize,
    examples: BTreeSet<SupportExampleId>,
}

impl RetainedSupportExamples {
    pub(crate) fn new(cell: &SupportCell, cap: usize) -> Result<Self, SupportCellError> {
        cell.validate()?;
        Ok(Self {
            cell_id: cell.id,
            cap,
            examples: BTreeSet::new(),
        })
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn cap(&self) -> usize {
        self.cap
    }

    pub(crate) fn examples(&self) -> impl ExactSizeIterator<Item = SupportExampleId> + '_ {
        self.examples.iter().copied()
    }

    pub(crate) fn retain(
        &mut self,
        example: SupportExampleId,
    ) -> Result<SupportExampleRetention, SupportCellError> {
        if example.cell_id != self.cell_id {
            return Err(SupportCellError::ExampleCellMismatch);
        }
        if self.examples.contains(&example) {
            return Ok(SupportExampleRetention::AlreadyRetained);
        }
        if self.examples.len() >= self.cap {
            return Ok(SupportExampleRetention::CapReached);
        }
        self.examples.insert(example);
        Ok(SupportExampleRetention::Inserted)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportCellError {
    EmptySupport(&'static str),
    EmptyProduct,
    InvalidInterval {
        start: u128,
        end_exclusive: u128,
    },
    InvalidProductRankInterval(&'static str),
    InvalidCongruenceResidue {
        residue: u128,
        modulus: u128,
    },
    CardinalityOverflow(&'static str),
    NonCanonicalExpression,
    CellIdMismatch {
        claimed: SupportCellId,
        derived: SupportCellId,
    },
    ObligationIdMismatch {
        claimed: SupportProofObligationId,
        derived: SupportProofObligationId,
    },
    ReceiptIdMismatch {
        claimed: SupportProofReceiptId,
        derived: SupportProofReceiptId,
    },
    ReceiptObligationMismatch,
    ReceiptConclusionMismatch,
    EvidenceIdMismatch {
        claimed: SupportCellEvidenceId,
        derived: SupportCellEvidenceId,
    },
    EvidenceCellMismatch,
    MechanismRequestMismatch,
    InjectivityForUnmappedCell,
    MaterializerMismatch,
    CardinalityMismatch {
        expected: u128,
        actual: u128,
    },
    CardinalityBelowLowerBound {
        lower_bound: u128,
        actual: u128,
    },
    ImageCardinalityExceedsCoordinates {
        image_count: u128,
        coordinate_count: u128,
    },
    EmptyPartition,
    EmptyPartitionChild {
        child_id: SupportCellId,
    },
    DuplicatePartitionChild,
    NonCanonicalPartitionChildren,
    PartitionSpaceMismatch,
    MappedImageNeedsPartitionProof,
    ParentNotOrdinalInterval,
    ParentNotProduct,
    ProductFactorOutOfBounds {
        factor_index: usize,
        factor_count: usize,
    },
    ChildNotMatchingProduct {
        child_id: SupportCellId,
    },
    ChildNotOrdinalInterval {
        child_id: SupportCellId,
    },
    PartitionChildOutOfBounds {
        child_id: SupportCellId,
        parent_start: u128,
        parent_end_exclusive: u128,
        child_start: u128,
        child_end_exclusive: u128,
    },
    PartitionGap {
        expected_start: u128,
        actual_start: u128,
    },
    PartitionOverlap {
        expected_start: u128,
        actual_start: u128,
    },
    PartitionIdMismatch {
        claimed: SupportPartitionId,
        derived: SupportPartitionId,
    },
    PartitionLowerBoundExceedsParent {
        lower_bound: u128,
        parent_count: u128,
    },
    UnsupportedCursorVersion(u32),
    CursorIdMismatch {
        claimed: SupportMaterializationCursorId,
        derived: SupportMaterializationCursorId,
    },
    CursorCellMismatch,
    CursorRegressed {
        previous: u128,
        next: u128,
    },
    CursorBeyondSupport {
        next: u128,
        coordinate_count: u128,
    },
    ExampleCellMismatch,
}

impl fmt::Display for SupportCellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySupport(kind) => write!(formatter, "{kind} has empty support"),
            Self::EmptyProduct => write!(formatter, "a support product needs at least one factor"),
            Self::InvalidInterval {
                start,
                end_exclusive,
            } => write!(
                formatter,
                "support interval [{start}, {end_exclusive}) is empty or reversed"
            ),
            Self::InvalidProductRankInterval(message) => {
                write!(formatter, "invalid product rank interval: {message}")
            }
            Self::InvalidCongruenceResidue { residue, modulus } => write!(
                formatter,
                "congruence residue {residue} is outside modulus {modulus}"
            ),
            Self::CardinalityOverflow(context) => {
                write!(formatter, "support cardinality overflow while {context}")
            }
            Self::NonCanonicalExpression => {
                write!(formatter, "support expression is not canonical")
            }
            Self::CellIdMismatch { .. } => write!(formatter, "support cell identity mismatch"),
            Self::ObligationIdMismatch { .. } => {
                write!(formatter, "support proof-obligation identity mismatch")
            }
            Self::ReceiptIdMismatch { .. } => {
                write!(formatter, "support proof-receipt identity mismatch")
            }
            Self::ReceiptObligationMismatch => {
                write!(formatter, "proof receipt belongs to another obligation")
            }
            Self::ReceiptConclusionMismatch => {
                write!(formatter, "proof receipt commits to another conclusion")
            }
            Self::EvidenceIdMismatch { .. } => {
                write!(formatter, "support evidence identity mismatch")
            }
            Self::EvidenceCellMismatch => {
                write!(formatter, "support evidence belongs to another cell")
            }
            Self::MechanismRequestMismatch => write!(
                formatter,
                "mechanism signature belongs to another mechanism request"
            ),
            Self::InjectivityForUnmappedCell => {
                write!(
                    formatter,
                    "injectivity evidence was applied to an unmapped cell"
                )
            }
            Self::MaterializerMismatch => write!(formatter, "support materializer mismatch"),
            Self::CardinalityMismatch { expected, actual } => write!(
                formatter,
                "support cardinality {actual} does not equal structural cardinality {expected}"
            ),
            Self::CardinalityBelowLowerBound {
                lower_bound,
                actual,
            } => write!(
                formatter,
                "support cardinality {actual} is below certified lower bound {lower_bound}"
            ),
            Self::ImageCardinalityExceedsCoordinates {
                image_count,
                coordinate_count,
            } => write!(
                formatter,
                "mapped-image cardinality {image_count} exceeds coordinate count {coordinate_count}"
            ),
            Self::EmptyPartition => write!(formatter, "support partition has no children"),
            Self::EmptyPartitionChild { .. } => {
                write!(formatter, "support partition contains an empty child")
            }
            Self::DuplicatePartitionChild => {
                write!(formatter, "support partition repeats a child")
            }
            Self::NonCanonicalPartitionChildren => {
                write!(formatter, "support partition children are not canonical")
            }
            Self::PartitionSpaceMismatch => {
                write!(formatter, "support partition crosses support spaces")
            }
            Self::MappedImageNeedsPartitionProof => write!(
                formatter,
                "coordinate intervals do not prove disjointness of a mapped image"
            ),
            Self::ParentNotOrdinalInterval => {
                write!(
                    formatter,
                    "support partition parent is not an ordinal interval"
                )
            }
            Self::ParentNotProduct => {
                write!(formatter, "support partition parent is not a product")
            }
            Self::ProductFactorOutOfBounds {
                factor_index,
                factor_count,
            } => write!(
                formatter,
                "support product factor {factor_index} is outside {factor_count} factors"
            ),
            Self::ChildNotMatchingProduct { .. } => write!(
                formatter,
                "support partition child changes a non-selected product factor"
            ),
            Self::ChildNotOrdinalInterval { .. } => {
                write!(
                    formatter,
                    "support partition child is not an ordinal interval"
                )
            }
            Self::PartitionChildOutOfBounds { .. } => {
                write!(formatter, "support partition child lies outside its parent")
            }
            Self::PartitionGap {
                expected_start,
                actual_start,
            } => write!(
                formatter,
                "support partition gap: expected {expected_start}, found {actual_start}"
            ),
            Self::PartitionOverlap {
                expected_start,
                actual_start,
            } => write!(
                formatter,
                "support partition overlap: expected {expected_start}, found {actual_start}"
            ),
            Self::PartitionIdMismatch { .. } => {
                write!(formatter, "support partition identity mismatch")
            }
            Self::PartitionLowerBoundExceedsParent {
                lower_bound,
                parent_count,
            } => write!(
                formatter,
                "partition child lower bound {lower_bound} exceeds parent count {parent_count}"
            ),
            Self::UnsupportedCursorVersion(version) => {
                write!(formatter, "unsupported support cursor version {version}")
            }
            Self::CursorIdMismatch { .. } => {
                write!(formatter, "support cursor identity mismatch")
            }
            Self::CursorCellMismatch => write!(formatter, "support cursor belongs to another cell"),
            Self::CursorRegressed { previous, next } => write!(
                formatter,
                "support cursor regressed from {previous} to {next}"
            ),
            Self::CursorBeyondSupport {
                next,
                coordinate_count,
            } => write!(
                formatter,
                "support cursor position {next} exceeds coordinate count {coordinate_count}"
            ),
            Self::ExampleCellMismatch => {
                write!(
                    formatter,
                    "retained example belongs to another support cell"
                )
            }
        }
    }
}

impl Error for SupportCellError {}

fn congruence_cardinality(
    start: u128,
    end_exclusive: u128,
    modulus: u128,
    residue: u128,
) -> Result<u128, SupportCellError> {
    let start_residue = start % modulus;
    let delta = if start_residue <= residue {
        residue - start_residue
    } else {
        modulus - (start_residue - residue)
    };
    if delta >= end_exclusive - start {
        return Ok(0);
    }
    let first = start + delta;
    Ok(((end_exclusive - 1 - first) / modulus) + 1)
}

fn product_cardinality(factors: &[SupportExpr]) -> Result<SupportCardinality, SupportCellError> {
    let all_exact = factors
        .iter()
        .all(|factor| factor.intrinsic_cardinality.exact().is_some());
    let lower_bound = factors.iter().try_fold(1_u128, |product, factor| {
        product
            .checked_mul(factor.intrinsic_cardinality.lower_bound())
            .ok_or(SupportCellError::CardinalityOverflow(
                "multiplying product factors",
            ))
    })?;
    Ok(if all_exact {
        SupportCardinality::Exact(lower_bound)
    } else {
        SupportCardinality::Open {
            confirmed_lower_bound: lower_bound,
        }
    })
}

fn partition_cardinality(
    parent: &SupportCell,
    children: &[SupportCell],
) -> Result<SupportCardinality, SupportCellError> {
    let all_children_exact = children
        .iter()
        .all(|child| child.cardinality().exact().is_some());
    let child_lower_bound = children.iter().try_fold(0_u128, |sum, child| {
        // The accepted partition conclusion itself certifies every child
        // nonempty even when a child's standalone count was open at zero.
        let cardinality = child.cardinality();
        if cardinality.exact() == Some(0) {
            return Err(SupportCellError::EmptyPartitionChild { child_id: child.id });
        }
        let child_lower_bound = cardinality.lower_bound().max(1);
        sum.checked_add(child_lower_bound)
            .ok_or(SupportCellError::CardinalityOverflow(
                "adding disjoint partition children",
            ))
    })?;

    match parent.cardinality() {
        SupportCardinality::Exact(parent_count) => {
            if child_lower_bound > parent_count {
                return Err(SupportCellError::PartitionLowerBoundExceedsParent {
                    lower_bound: child_lower_bound,
                    parent_count,
                });
            }
            if all_children_exact && child_lower_bound != parent_count {
                return Err(SupportCellError::CardinalityMismatch {
                    expected: parent_count,
                    actual: child_lower_bound,
                });
            }
            Ok(SupportCardinality::Exact(parent_count))
        }
        SupportCardinality::Open { .. } if all_children_exact => {
            Ok(SupportCardinality::Exact(child_lower_bound))
        }
        SupportCardinality::Open { .. } => Ok(SupportCardinality::Open {
            confirmed_lower_bound: child_lower_bound,
        }),
    }
}

fn ordinal_interval_bounds(expression: &SupportExpr) -> Option<(u128, u128)> {
    match &expression.kind {
        SupportExprKind::OrdinalInterval {
            start,
            end_exclusive,
        } => Some((*start, *end_exclusive)),
        _ => None,
    }
}

fn validate_ordinal_interval_cover(
    parent: &SupportCell,
    children: &mut [SupportCell],
) -> Result<(u128, u128), SupportCellError> {
    let (parent_start, parent_end) = ordinal_interval_bounds(&parent.expression)
        .ok_or(SupportCellError::ParentNotOrdinalInterval)?;
    if children.is_empty() {
        return Err(SupportCellError::EmptyPartition);
    }
    for child in children.iter() {
        child.validate()?;
        if child.space != parent.space {
            return Err(SupportCellError::PartitionSpaceMismatch);
        }
        if child.materializer_id != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        if ordinal_interval_bounds(&child.expression).is_none() {
            return Err(SupportCellError::ChildNotOrdinalInterval { child_id: child.id });
        }
    }
    children.sort_by_key(|child| {
        ordinal_interval_bounds(&child.expression)
            .expect("child interval checked")
            .0
    });

    let mut expected_start = parent_start;
    for child in children.iter() {
        let (start, end_exclusive) =
            ordinal_interval_bounds(&child.expression).expect("child interval checked");
        if start < parent_start || end_exclusive > parent_end {
            return Err(SupportCellError::PartitionChildOutOfBounds {
                child_id: child.id,
                parent_start,
                parent_end_exclusive: parent_end,
                child_start: start,
                child_end_exclusive: end_exclusive,
            });
        }
        if start < expected_start {
            return Err(SupportCellError::PartitionOverlap {
                expected_start,
                actual_start: start,
            });
        }
        if start > expected_start {
            return Err(SupportCellError::PartitionGap {
                expected_start,
                actual_start: start,
            });
        }
        expected_start = end_exclusive;
    }
    if expected_start != parent_end {
        return Err(SupportCellError::PartitionGap {
            expected_start,
            actual_start: parent_end,
        });
    }
    Ok((parent_start, parent_end))
}

fn validate_product_factor_cover(
    parent: &SupportCell,
    children: &mut [SupportCell],
    factor_index: usize,
) -> Result<(), SupportCellError> {
    let SupportExprKind::Product(parent_factors) = parent.expression().kind() else {
        return Err(SupportCellError::ParentNotProduct);
    };
    let Some(parent_factor) = parent_factors.get(factor_index) else {
        return Err(SupportCellError::ProductFactorOutOfBounds {
            factor_index,
            factor_count: parent_factors.len(),
        });
    };
    let (parent_start, parent_end) =
        ordinal_interval_bounds(parent_factor).ok_or(SupportCellError::ParentNotOrdinalInterval)?;
    if children.is_empty() {
        return Err(SupportCellError::EmptyPartition);
    }

    for child in children.iter() {
        child.validate()?;
        if child.space != parent.space {
            return Err(SupportCellError::PartitionSpaceMismatch);
        }
        if child.materializer_id != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        let SupportExprKind::Product(child_factors) = child.expression().kind() else {
            return Err(SupportCellError::ChildNotMatchingProduct {
                child_id: child.id(),
            });
        };
        if child_factors.len() != parent_factors.len()
            || child_factors
                .iter()
                .enumerate()
                .any(|(index, factor)| index != factor_index && factor != &parent_factors[index])
            || ordinal_interval_bounds(&child_factors[factor_index]).is_none()
        {
            return Err(SupportCellError::ChildNotMatchingProduct {
                child_id: child.id(),
            });
        }
    }
    children.sort_by_key(|child| {
        let SupportExprKind::Product(factors) = child.expression().kind() else {
            unreachable!("child product checked")
        };
        ordinal_interval_bounds(&factors[factor_index])
            .expect("selected child interval checked")
            .0
    });

    let mut expected_start = parent_start;
    for child in children.iter() {
        let SupportExprKind::Product(factors) = child.expression().kind() else {
            unreachable!("child product checked")
        };
        let (start, end_exclusive) = ordinal_interval_bounds(&factors[factor_index])
            .expect("selected child interval checked");
        if start < parent_start || end_exclusive > parent_end {
            return Err(SupportCellError::PartitionChildOutOfBounds {
                child_id: child.id(),
                parent_start,
                parent_end_exclusive: parent_end,
                child_start: start,
                child_end_exclusive: end_exclusive,
            });
        }
        if start < expected_start {
            return Err(SupportCellError::PartitionOverlap {
                expected_start,
                actual_start: start,
            });
        }
        if start > expected_start {
            return Err(SupportCellError::PartitionGap {
                expected_start,
                actual_start: start,
            });
        }
        expected_start = end_exclusive;
    }
    if expected_start != parent_end {
        return Err(SupportCellError::PartitionGap {
            expected_start,
            actual_start: parent_end,
        });
    }
    Ok(())
}

fn product_rank_basis_and_bounds(
    expression: &SupportExpr,
) -> Result<(&[SupportExpr], u128, u128), SupportCellError> {
    match expression.kind() {
        SupportExprKind::Product(factors) => {
            let cardinality = expression.intrinsic_cardinality().exact().ok_or(
                SupportCellError::InvalidProductRankInterval(
                    "full ranked parent product is not exact",
                ),
            )?;
            Ok((factors, 0, cardinality))
        }
        SupportExprKind::ProductRankInterval {
            factors,
            rank_start,
            rank_end_exclusive,
        } => Ok((factors, *rank_start, *rank_end_exclusive)),
        _ => Err(SupportCellError::ParentNotProduct),
    }
}

fn validate_product_rank_interval_cover(
    parent: &SupportCell,
    children: &mut [SupportCell],
) -> Result<(u128, u128), SupportCellError> {
    let (parent_factors, parent_start, parent_end) =
        product_rank_basis_and_bounds(parent.expression())?;
    if children.is_empty() {
        return Err(SupportCellError::EmptyPartition);
    }
    for child in children.iter() {
        child.validate()?;
        if child.space != parent.space {
            return Err(SupportCellError::PartitionSpaceMismatch);
        }
        if child.materializer_id != parent.materializer_id {
            return Err(SupportCellError::MaterializerMismatch);
        }
        let SupportExprKind::ProductRankInterval {
            factors,
            rank_start,
            rank_end_exclusive,
        } = child.expression().kind()
        else {
            return Err(SupportCellError::ChildNotMatchingProduct {
                child_id: child.id(),
            });
        };
        if factors.as_ref() != parent_factors
            || *rank_start < parent_start
            || *rank_end_exclusive > parent_end
        {
            return Err(SupportCellError::ChildNotMatchingProduct {
                child_id: child.id(),
            });
        }
    }
    children.sort_by_key(|child| {
        let SupportExprKind::ProductRankInterval { rank_start, .. } = child.expression().kind()
        else {
            unreachable!("ranked product child checked")
        };
        *rank_start
    });

    let mut expected_start = parent_start;
    for child in children.iter() {
        let SupportExprKind::ProductRankInterval {
            rank_start,
            rank_end_exclusive,
            ..
        } = child.expression().kind()
        else {
            unreachable!("ranked product child checked")
        };
        if *rank_start < expected_start {
            return Err(SupportCellError::PartitionOverlap {
                expected_start,
                actual_start: *rank_start,
            });
        }
        if *rank_start > expected_start {
            return Err(SupportCellError::PartitionGap {
                expected_start,
                actual_start: *rank_start,
            });
        }
        expected_start = *rank_end_exclusive;
    }
    if expected_start != parent_end {
        return Err(SupportCellError::PartitionGap {
            expected_start,
            actual_start: parent_end,
        });
    }
    Ok((parent_start, parent_end))
}

fn ordinal_partition_proof_digest(parent: &SupportCell, children: &[SupportCell]) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(b"futuruna.explore.ordinal-interval-cover-proof.v1");
    hasher.digest(parent.id.bytes());
    hasher.u128(children.len() as u128);
    for child in children {
        let (start, end_exclusive) = ordinal_interval_bounds(&child.expression)
            .expect("ordinal partition children checked before proof receipt");
        hasher.digest(child.id.bytes());
        hasher.u128(start);
        hasher.u128(end_exclusive);
    }
    hasher.finish()
}

fn product_factor_partition_proof_digest(
    parent: &SupportCell,
    children: &[SupportCell],
    factor_index: usize,
) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(b"futuruna.explore.product-factor-cover-proof.v1");
    hasher.digest(parent.id.bytes());
    hasher.u128(factor_index as u128);
    hasher.u128(children.len() as u128);
    for child in children {
        let SupportExprKind::Product(factors) = child.expression().kind() else {
            unreachable!("product partition children checked before proof receipt")
        };
        let (start, end_exclusive) = ordinal_interval_bounds(&factors[factor_index])
            .expect("selected product factor checked before proof receipt");
        hasher.digest(child.id.bytes());
        hasher.u128(start);
        hasher.u128(end_exclusive);
    }
    hasher.finish()
}

fn product_rank_interval_partition_proof_digest(
    parent: &SupportCell,
    children: &[SupportCell],
) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(b"futuruna.explore.product-rank-interval-cover-proof.v1");
    hasher.digest(parent.id.bytes());
    hasher.u128(children.len() as u128);
    for child in children {
        let SupportExprKind::ProductRankInterval {
            rank_start,
            rank_end_exclusive,
            ..
        } = child.expression().kind()
        else {
            unreachable!("ranked product partition children checked before proof receipt")
        };
        hasher.digest(child.id.bytes());
        hasher.u128(*rank_start);
        hasher.u128(*rank_end_exclusive);
    }
    hasher.finish()
}

fn hash_support_expr(kind: &SupportExprKind) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(SUPPORT_EXPR_HASH_V1);
    match kind {
        SupportExprKind::Singleton(value) => {
            hasher.tag(0x01);
            hasher.digest(canonical_explore_value_digest(value));
        }
        SupportExprKind::FiniteEnum(values) => {
            hasher.tag(0x02);
            hasher.u128(values.len() as u128);
            for value in values {
                hasher.digest(canonical_explore_value_digest(value));
            }
        }
        SupportExprKind::OrdinalInterval {
            start,
            end_exclusive,
        } => {
            hasher.tag(0x03);
            hasher.u128(*start);
            hasher.u128(*end_exclusive);
        }
        SupportExprKind::OrdinalCongruence {
            start,
            end_exclusive,
            modulus,
            residue,
        } => {
            hasher.tag(0x04);
            hasher.u128(*start);
            hasher.u128(*end_exclusive);
            hasher.u128(modulus.get());
            hasher.u128(*residue);
        }
        SupportExprKind::Product(factors) => {
            hasher.tag(0x05);
            hasher.u128(factors.len() as u128);
            for factor in factors {
                hasher.digest(factor.id.bytes());
            }
        }
        SupportExprKind::ProductRankInterval {
            factors,
            rank_start,
            rank_end_exclusive,
        } => {
            hasher.tag(0x09);
            hasher.u128(factors.len() as u128);
            for factor in factors {
                hasher.digest(factor.id.bytes());
            }
            hasher.u128(*rank_start);
            hasher.u128(*rank_end_exclusive);
        }
        SupportExprKind::JoinReference {
            producer_id,
            inputs,
        } => {
            hasher.tag(0x06);
            hasher.digest(producer_id.bytes());
            hasher.u128(inputs.len() as u128);
            for input in inputs {
                hasher.digest(input.bytes());
            }
        }
        SupportExprKind::Union(operands) => {
            hasher.tag(0x07);
            hasher.u128(operands.len() as u128);
            for operand in operands {
                hasher.digest(operand.id.bytes());
            }
        }
        SupportExprKind::Difference {
            minuend,
            subtrahend,
        } => {
            hasher.tag(0x08);
            hasher.digest(minuend.id.bytes());
            hasher.digest(subtrahend.id.bytes());
        }
    }
    hasher.finish()
}

fn derive_support_cell_id(
    space: SupportCellSpace,
    expression_id: SupportExprId,
    materializer_id: SupportMaterializerId,
) -> SupportCellId {
    let mut hasher = CanonicalHasher::new(SUPPORT_CELL_HASH_V1);
    space.hash_into(&mut hasher);
    hasher.digest(expression_id.bytes());
    hasher.digest(materializer_id.bytes());
    SupportCellId(hasher.finish())
}

fn derive_cell_obligation_id(
    cell_id: SupportCellId,
    claim_digest: [u8; 32],
) -> SupportProofObligationId {
    let mut hasher = CanonicalHasher::new(SUPPORT_OBLIGATION_HASH_V1);
    hasher.digest(cell_id.bytes());
    hasher.digest(claim_digest);
    SupportProofObligationId(hasher.finish())
}

fn derive_partition_obligation_id(
    parent_id: SupportCellId,
    child_ids: &[SupportCellId],
) -> SupportProofObligationId {
    let mut hasher = CanonicalHasher::new(SUPPORT_PARTITION_OBLIGATION_HASH_V1);
    hasher.digest(parent_id.bytes());
    hasher.u128(child_ids.len() as u128);
    for child_id in child_ids {
        hasher.digest(child_id.bytes());
    }
    SupportProofObligationId(hasher.finish())
}

fn derive_proof_receipt_id(
    obligation_id: SupportProofObligationId,
    verifier_id: SupportProofVerifierId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
) -> SupportProofReceiptId {
    let mut hasher = CanonicalHasher::new(SUPPORT_PROOF_RECEIPT_HASH_V1);
    hasher.digest(obligation_id.bytes());
    hasher.digest(verifier_id.bytes());
    hasher.digest(conclusion_digest);
    hasher.digest(proof_digest);
    SupportProofReceiptId(hasher.finish())
}

fn derive_evidence_id(
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    receipt_id: SupportProofReceiptId,
) -> SupportCellEvidenceId {
    let mut hasher = CanonicalHasher::new(SUPPORT_EVIDENCE_HASH_V1);
    hasher.digest(obligation_id.bytes());
    hasher.digest(conclusion_digest);
    hasher.digest(receipt_id.bytes());
    SupportCellEvidenceId(hasher.finish())
}

fn derive_partition_id(
    kind: SupportPartitionKind,
    obligation_id: SupportProofObligationId,
    cardinality: SupportCardinality,
    receipt_id: SupportProofReceiptId,
) -> SupportPartitionId {
    let mut hasher = CanonicalHasher::new(SUPPORT_PARTITION_HASH_V1);
    hasher.tag(kind.canonical_tag());
    hasher.digest(obligation_id.bytes());
    cardinality.hash_into(&mut hasher);
    hasher.digest(receipt_id.bytes());
    SupportPartitionId(hasher.finish())
}

fn derive_cursor_id(
    version: u32,
    cell_id: SupportCellId,
    materializer_id: SupportMaterializerId,
    next_coordinate_ordinal: u128,
    checkpoint: &[u8],
) -> SupportMaterializationCursorId {
    let mut hasher = CanonicalHasher::new(SUPPORT_CURSOR_HASH_V1);
    hasher.u32(version);
    hasher.digest(cell_id.bytes());
    hasher.digest(materializer_id.bytes());
    hasher.u128(next_coordinate_ordinal);
    hasher.bytes(checkpoint);
    SupportMaterializationCursorId(hasher.finish())
}

fn derive_scoped_digest(scope: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(scope);
    hasher.bytes(preimage);
    hasher.finish()
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, tag: u8) {
        self.0.update([tag]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.u128(bytes.len() as u128);
        self.0.update(bytes);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

// Focused contract fixtures for the support/evidence boundary.
#[cfg(test)]
mod tests {
    use super::super::relation::{FindPolarity, MechanismTargetId};
    use super::*;

    fn producer() -> SupportProducerId {
        SupportProducerId::from_canonical_preimage(b"fixture-producer")
    }

    fn materializer() -> SupportMaterializerId {
        SupportMaterializerId::from_canonical_preimage(b"fixture-mapping-and-codec")
    }

    fn coordinate_interval(start: u128, end_exclusive: u128) -> SupportCell {
        SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer()),
            SupportExpr::ordinal_interval(start, end_exclusive).unwrap(),
            materializer(),
        )
        .unwrap()
    }

    fn mapped_interval(relation: RelationId, start: u128, end_exclusive: u128) -> SupportCell {
        SupportCell::new(
            SupportCellSpace::MappedImage {
                producer_id: producer(),
                target: SupportExtensionalTarget::Cases(relation),
            },
            SupportExpr::ordinal_interval(start, end_exclusive).unwrap(),
            materializer(),
        )
        .unwrap()
    }

    fn product_cell(
        space: SupportCellSpace,
        income_start: u128,
        income_end_exclusive: u128,
    ) -> SupportCell {
        SupportCell::new(
            space,
            SupportExpr::product(vec![
                SupportExpr::finite_enum(vec![ExploreValue::Int(101), ExploreValue::Int(202)])
                    .unwrap(),
                SupportExpr::ordinal_interval(income_start, income_end_exclusive).unwrap(),
            ])
            .unwrap(),
            materializer(),
        )
        .unwrap()
    }

    fn accepted_injectivity_evidence(
        cell: &SupportCell,
    ) -> SupportCellEvidence<InjectiveMappingClaim> {
        let claim = InjectiveMappingClaim::new(cell.materializer_id());
        let obligation = SupportCellObligation::new(cell, claim).unwrap();
        let conclusion = CertifiedInjective;
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(b"fixture-injectivity-verifier"),
            obligation.claim().conclusion_digest(&conclusion),
            derive_scoped_digest(b"fixture-injectivity-proof", &cell.id().bytes()),
        );
        SupportCellEvidence::from_accepted_proof(obligation, conclusion, receipt).unwrap()
    }

    fn accepted_cardinality_evidence(
        cell: &SupportCell,
        count: u128,
    ) -> SupportCellEvidence<ExactCardinalityClaim> {
        let obligation = SupportCellObligation::new(cell, ExactCardinalityClaim).unwrap();
        let receipt = SupportProofReceipt::from_accepted_proof(
            obligation.id(),
            SupportProofVerifierId::from_canonical_preimage(b"fixture-count-verifier"),
            obligation.claim().conclusion_digest(&count),
            derive_scoped_digest(b"fixture-count-proof", &count.to_be_bytes()),
        );
        SupportCellEvidence::from_accepted_proof(obligation, count, receipt).unwrap()
    }

    #[test]
    fn finite_enum_and_set_union_have_canonical_identity() {
        let left = SupportExpr::finite_enum(vec![
            ExploreValue::Int(2),
            ExploreValue::Int(1),
            ExploreValue::Int(2),
        ])
        .unwrap();
        let right =
            SupportExpr::finite_enum(vec![ExploreValue::Int(1), ExploreValue::Int(2)]).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.intrinsic_cardinality(), SupportCardinality::Exact(2));

        let first = SupportExpr::singleton(ExploreValue::Int(10));
        let second = SupportExpr::singleton(ExploreValue::Int(20));
        let union_a =
            SupportExpr::union(vec![first.clone(), second.clone(), first.clone()]).unwrap();
        let union_b = SupportExpr::union(vec![second, first]).unwrap();
        assert_eq!(union_a, union_b);
        assert_eq!(
            union_a.intrinsic_cardinality(),
            SupportCardinality::Exact(2)
        );
    }

    #[test]
    fn interval_congruence_and_product_count_coordinates_exactly() {
        let odd = SupportExpr::ordinal_congruence(0, 10, NonZeroU128::new(2).unwrap(), 1).unwrap();
        assert_eq!(odd.intrinsic_cardinality(), SupportCardinality::Exact(5));

        let product = SupportExpr::product(vec![
            SupportExpr::ordinal_interval(0, 3).unwrap(),
            SupportExpr::ordinal_interval(10, 14).unwrap(),
        ])
        .unwrap();
        assert_eq!(
            product.intrinsic_cardinality(),
            SupportCardinality::Exact(12)
        );

        let coalesced = SupportExpr::union(vec![
            SupportExpr::ordinal_interval(5, 10).unwrap(),
            SupportExpr::ordinal_interval(0, 5).unwrap(),
        ])
        .unwrap();
        assert_eq!(coalesced, SupportExpr::ordinal_interval(0, 10).unwrap());
    }

    #[test]
    fn mapped_image_does_not_inherit_product_cardinality() {
        let relation = RelationId::from_canonical_semantic_preimage(b"fixture-relation");
        let product = SupportExpr::product(vec![
            SupportExpr::ordinal_interval(0, 2).unwrap(),
            SupportExpr::ordinal_interval(0, 3).unwrap(),
        ])
        .unwrap();
        let image = SupportCell::new(
            SupportCellSpace::MappedImage {
                producer_id: producer(),
                target: SupportExtensionalTarget::Cases(relation),
            },
            product,
            materializer(),
        )
        .unwrap();

        assert_eq!(image.coordinate_cardinality(), SupportCardinality::Exact(6));
        assert_eq!(
            image.cardinality(),
            SupportCardinality::Open {
                confirmed_lower_bound: 1
            }
        );

        let evidence = accepted_cardinality_evidence(&image, 4);
        assert_eq!(
            image.cardinality_from_evidence(&evidence).unwrap(),
            SupportCardinality::Exact(4)
        );
    }

    #[test]
    fn interval_partition_rejects_gap_overlap_and_mapped_image_shortcut() {
        let parent = coordinate_interval(0, 10);
        let certificate = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![coordinate_interval(0, 4), coordinate_interval(4, 10)],
        )
        .unwrap();
        assert_eq!(certificate.cardinality(), SupportCardinality::Exact(10));
        certificate.validate().unwrap();

        assert!(matches!(
            SupportPartitionCertificate::ordinal_interval_cover(
                &parent,
                vec![coordinate_interval(0, 4), coordinate_interval(5, 10)]
            ),
            Err(SupportCellError::PartitionGap { .. })
        ));
        assert!(matches!(
            SupportPartitionCertificate::ordinal_interval_cover(
                &parent,
                vec![coordinate_interval(0, 6), coordinate_interval(5, 10)]
            ),
            Err(SupportCellError::PartitionOverlap { .. })
        ));

        let relation = RelationId::from_canonical_semantic_preimage(b"fixture-image-relation");
        let image_parent = SupportCell::new(
            SupportCellSpace::MappedImage {
                producer_id: producer(),
                target: SupportExtensionalTarget::Cases(relation),
            },
            SupportExpr::ordinal_interval(0, 10).unwrap(),
            materializer(),
        )
        .unwrap();
        assert_eq!(
            SupportPartitionCertificate::ordinal_interval_cover(
                &image_parent,
                vec![coordinate_interval(0, 10)]
            ),
            Err(SupportCellError::MappedImageNeedsPartitionProof)
        );

        let injectivity = accepted_injectivity_evidence(&image_parent);
        let lifted = SupportPartitionCertificate::mapped_injective_ordinal_cover(
            &image_parent,
            vec![
                mapped_interval(relation, 0, 4),
                mapped_interval(relation, 4, 10),
            ],
            &injectivity,
        )
        .unwrap();
        assert_eq!(
            lifted.kind(),
            SupportPartitionKind::MappedInjectiveOrdinalCover
        );
        assert_eq!(lifted.cardinality(), SupportCardinality::Exact(10));
        lifted.validate().unwrap();
    }

    #[test]
    fn product_factor_cover_splits_income_without_splitting_other_dimensions() {
        let coordinate_space = SupportCellSpace::ProducerCoordinates(producer());
        let parent = product_cell(coordinate_space, 0, 10);
        let certificate = SupportPartitionCertificate::product_factor_cover(
            &parent,
            vec![
                product_cell(coordinate_space, 0, 4),
                product_cell(coordinate_space, 4, 10),
            ],
            1,
        )
        .unwrap();
        assert_eq!(certificate.cardinality(), SupportCardinality::Exact(20));
        assert_eq!(certificate.kind(), SupportPartitionKind::ProductFactorCover);

        let relation = RelationId::from_canonical_semantic_preimage(b"fixture-product-image");
        let image_space = SupportCellSpace::MappedImage {
            producer_id: producer(),
            target: SupportExtensionalTarget::Cases(relation),
        };
        let image_parent = product_cell(image_space, 0, 10);
        let injectivity = accepted_injectivity_evidence(&image_parent);
        let lifted = SupportPartitionCertificate::mapped_injective_product_factor_cover(
            &image_parent,
            vec![
                product_cell(image_space, 0, 4),
                product_cell(image_space, 4, 10),
            ],
            1,
            &injectivity,
        )
        .unwrap();
        assert_eq!(lifted.cardinality(), SupportCardinality::Exact(20));
        assert_eq!(
            lifted.kind(),
            SupportPartitionKind::MappedInjectiveProductFactorCover
        );
        lifted.validate().unwrap();
    }

    #[test]
    fn cursor_resumes_local_coordinates_without_becoming_case_identity() {
        let cell = coordinate_interval(10, 15);
        let start = SupportMaterializationCursor::at_start(&cell).unwrap();
        let middle = start.advance(&cell, 3, b"backend-state".to_vec()).unwrap();
        let resumed = SupportMaterializationCursor::resume(&cell, middle.clone()).unwrap();
        assert_eq!(resumed, middle);
        assert_eq!(resumed.next_coordinate_ordinal(), 3);
        assert!(!resumed.is_complete(&cell));

        let complete = resumed.advance(&cell, 5, Vec::new()).unwrap();
        assert!(complete.is_complete(&cell));
        assert!(matches!(
            complete.advance(&cell, 6, Vec::new()),
            Err(SupportCellError::CursorBeyondSupport { .. })
        ));
        assert!(matches!(
            complete.advance(&cell, 4, Vec::new()),
            Err(SupportCellError::CursorRegressed { .. })
        ));
    }

    #[test]
    fn retained_examples_never_change_exact_support() {
        let cell = coordinate_interval(0, 100);
        let original_id = cell.id();
        let original_cardinality = cell.cardinality();
        let mut retained = RetainedSupportExamples::new(&cell, 1).unwrap();
        let first = SupportExampleId::from_canonical_example_digest(cell.id(), [1; 32]);
        let second = SupportExampleId::from_canonical_example_digest(cell.id(), [2; 32]);
        assert_eq!(
            retained.retain(first).unwrap(),
            SupportExampleRetention::Inserted
        );
        assert_eq!(
            retained.retain(second).unwrap(),
            SupportExampleRetention::CapReached
        );
        assert_eq!(cell.id(), original_id);
        assert_eq!(cell.cardinality(), original_cardinality);
    }

    #[test]
    fn questions_and_mechanism_observers_attach_without_renaming_support() {
        let relation = RelationId::from_canonical_semantic_preimage(b"shared-case-relation");
        let cell = SupportCell::new(
            SupportCellSpace::ExtensionalValues(SupportExtensionalTarget::Cases(relation)),
            SupportExpr::finite_enum(vec![ExploreValue::Int(1), ExploreValue::Int(2)]).unwrap(),
            materializer(),
        )
        .unwrap();
        let admission =
            AdmissionId::from_canonical_admission_preimage(relation, b"shared-admission");
        let first_question = QuestionId::from_canonical_find_preimage(
            admission,
            b"first-question",
            FindPolarity::Matches,
        );
        let second_question = QuestionId::from_canonical_find_preimage(
            admission,
            b"second-question",
            FindPolarity::Matches,
        );
        let first_obligation =
            SupportCellObligation::new(&cell, SelectionClassificationClaim::new(first_question))
                .unwrap();
        let second_obligation =
            SupportCellObligation::new(&cell, SelectionClassificationClaim::new(second_question))
                .unwrap();
        assert_eq!(first_obligation.cell_id(), cell.id());
        assert_eq!(second_obligation.cell_id(), cell.id());
        assert_ne!(first_obligation.id(), second_obligation.id());

        let first_request = MechanismRequestId::from_canonical_request_preimages(
            first_question,
            MechanismTargetId::Selected,
            b"observer",
            b"normalizer",
        );
        let second_request = MechanismRequestId::from_canonical_request_preimages(
            second_question,
            MechanismTargetId::Selected,
            b"observer",
            b"normalizer",
        );
        let first_signature = MechanismSignatureId::from_canonical_differential_signature_digest(
            first_request,
            [7; 32],
        );
        assert!(matches!(
            UniformMechanismClaim::new(second_request).validate_conclusion(&first_signature),
            Err(SupportCellError::MechanismRequestMismatch)
        ));
    }
}
