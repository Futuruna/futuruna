//! Arrival-order-independent evidence catalog for exact Explore support cells.
//!
//! The mutable catalog indexes content-addressed support, partitions, typed
//! obligations/evidence, and resumable materialization checkpoints without
//! rebuilding a canonical snapshot on every insertion. Dependencies may arrive
//! in any order; snapshot construction validates the complete referenced graph
//! and computes its canonical root. Contradictory conclusions and incompatible
//! parent replacements are rejected at ingestion.
//!
//! Retained examples are presentation metadata and materialization cursors are
//! operational resume progress. Both are stored beside the evidence snapshot
//! but deliberately excluded from [`SupportEvidenceRoot`]; neither can change
//! exact support, proof closure, or evidence identity. This catalog also never
//! constructs or claims a relational `RelationContentRoot`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{
    AdmissionId, ChoiceId, MechanismRequestId, QuestionId, RelationId, ViewId, ViewInputId,
};
use super::support_cell::{
    AdmissionClassificationClaim, ExactCardinalityClaim, InjectiveMappingClaim,
    RetainedSupportExamples, SelectionClassificationClaim, SupportCell, SupportCellError,
    SupportCellEvidence, SupportCellEvidenceId, SupportCellId, SupportCellObligation,
    SupportCellSpace, SupportExampleId, SupportExpr, SupportExprKind, SupportExtensionalTarget,
    SupportMaterializationCursor, SupportMaterializationCursorId, SupportMaterializerId,
    SupportObserverId, SupportPartitionCertificate, SupportPartitionId, SupportProducerId,
    SupportProofObligationId, UniformMechanismClaim, UniformValueClaim,
};

const SUPPORT_EVIDENCE_ROOT_HASH_V3: &[u8] = b"futuruna.explore.support-evidence-root.v3";
const SUPPORT_OBLIGATION_REFINEMENT_HASH_V1: &[u8] =
    b"futuruna.explore.support-obligation-refinement.v1";

pub(crate) const SUPPORT_EVIDENCE_SNAPSHOT_VERSION: u32 = 3;

/// Canonical commitment to support and accepted evidence at one frontier.
///
/// This is intentionally neither interchangeable with nor convertible into a
/// relation content root. It may describe an open support/proof frontier and
/// excludes retained-example presentation policy and materialization progress.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportEvidenceRoot([u8; 32]);

impl SupportEvidenceRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closure-aware record count. `Open` reports the current observation only;
/// unlike [`super::support_cell::SupportCardinality`], it does not claim that
/// the observation is a mathematical lower bound for every derived DAG view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportEvidenceCount {
    Open { observed: u128 },
    Exact(u128),
}

impl SupportEvidenceCount {
    pub(crate) const fn observed(self) -> u128 {
        match self {
            Self::Open { observed } | Self::Exact(observed) => observed,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Counts over semantic support/evidence records. `proved_obligations` and
/// `open_obligations` count active leaves only; a refined parent is counted as
/// superseded, never as a proved uniform result. Presentation-only examples
/// have their own count structure and do not affect these values or the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupportEvidenceCounts {
    cells: SupportEvidenceCount,
    roots: SupportEvidenceCount,
    active_leaves: SupportEvidenceCount,
    open_leaves: SupportEvidenceCount,
    partitions: SupportEvidenceCount,
    obligations: SupportEvidenceCount,
    obligation_refinements: SupportEvidenceCount,
    root_obligations: SupportEvidenceCount,
    active_obligation_leaves: SupportEvidenceCount,
    superseded_obligations: SupportEvidenceCount,
    proved_obligations: SupportEvidenceCount,
    open_obligations: SupportEvidenceCount,
    evidence_records: SupportEvidenceCount,
    cardinality_records: SupportEvidenceCount,
    injectivity_records: SupportEvidenceCount,
    admission_records: SupportEvidenceCount,
    selection_records: SupportEvidenceCount,
    uniform_value_records: SupportEvidenceCount,
    uniform_mechanism_records: SupportEvidenceCount,
    cursor_records: SupportEvidenceCount,
    latest_cursors: SupportEvidenceCount,
}

macro_rules! evidence_count_accessors {
    ($($name:ident),+ $(,)?) => {
        $(pub(crate) const fn $name(self) -> SupportEvidenceCount { self.$name })+
    };
}

impl SupportEvidenceCounts {
    evidence_count_accessors!(
        cells,
        roots,
        active_leaves,
        open_leaves,
        partitions,
        obligations,
        obligation_refinements,
        root_obligations,
        active_obligation_leaves,
        superseded_obligations,
        proved_obligations,
        open_obligations,
        evidence_records,
        cardinality_records,
        injectivity_records,
        admission_records,
        selection_records,
        uniform_value_records,
        uniform_mechanism_records,
        cursor_records,
        latest_cursors,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupportPresentationCounts {
    cells_with_examples: u128,
    retained_examples: u128,
}

impl SupportPresentationCounts {
    pub(crate) const fn cells_with_examples(self) -> u128 {
        self.cells_with_examples
    }

    pub(crate) const fn retained_examples(self) -> u128 {
        self.retained_examples
    }
}

/// Declared semantic scope of a value observer. Registration lets the catalog
/// reject an otherwise well-hashed observer attached to an unrelated cell.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SupportObserverLayerScope {
    Relation(RelationId),
    Question(QuestionId),
    MechanismRequest(MechanismRequestId),
    View(ViewId),
    Producer(SupportProducerId),
    ExactCell(SupportCellId),
}

/// Heterogeneous typed proof obligation retained without erasing its claim
/// type at ingestion boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportObligationRecord {
    Cardinality(SupportCellObligation<ExactCardinalityClaim>),
    Injectivity(SupportCellObligation<InjectiveMappingClaim>),
    Admission(SupportCellObligation<AdmissionClassificationClaim>),
    Selection(SupportCellObligation<SelectionClassificationClaim>),
    UniformValue(SupportCellObligation<UniformValueClaim>),
    UniformMechanism(SupportCellObligation<UniformMechanismClaim>),
}

impl SupportObligationRecord {
    pub(crate) const fn id(&self) -> SupportProofObligationId {
        match self {
            Self::Cardinality(value) => value.id(),
            Self::Injectivity(value) => value.id(),
            Self::Admission(value) => value.id(),
            Self::Selection(value) => value.id(),
            Self::UniformValue(value) => value.id(),
            Self::UniformMechanism(value) => value.id(),
        }
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        match self {
            Self::Cardinality(value) => value.cell_id(),
            Self::Injectivity(value) => value.cell_id(),
            Self::Admission(value) => value.cell_id(),
            Self::Selection(value) => value.cell_id(),
            Self::UniformValue(value) => value.cell_id(),
            Self::UniformMechanism(value) => value.cell_id(),
        }
    }

    pub(crate) const fn kind(&self) -> SupportEvidenceKind {
        match self {
            Self::Cardinality(_) => SupportEvidenceKind::Cardinality,
            Self::Injectivity(_) => SupportEvidenceKind::Injectivity,
            Self::Admission(_) => SupportEvidenceKind::Admission,
            Self::Selection(_) => SupportEvidenceKind::Selection,
            Self::UniformValue(_) => SupportEvidenceKind::UniformValue,
            Self::UniformMechanism(_) => SupportEvidenceKind::UniformMechanism,
        }
    }

    fn validate(&self) -> Result<(), SupportCellError> {
        match self {
            Self::Cardinality(value) => value.validate(),
            Self::Injectivity(value) => value.validate(),
            Self::Admission(value) => value.validate(),
            Self::Selection(value) => value.validate(),
            Self::UniformValue(value) => value.validate(),
            Self::UniformMechanism(value) => value.validate(),
        }
    }

    fn has_same_claim(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Cardinality(left), Self::Cardinality(right)) => left.claim() == right.claim(),
            (Self::Injectivity(left), Self::Injectivity(right)) => left.claim() == right.claim(),
            (Self::Admission(left), Self::Admission(right)) => left.claim() == right.claim(),
            (Self::Selection(left), Self::Selection(right)) => left.claim() == right.claim(),
            (Self::UniformValue(left), Self::UniformValue(right)) => left.claim() == right.claim(),
            (Self::UniformMechanism(left), Self::UniformMechanism(right)) => {
                left.claim() == right.claim()
            }
            _ => false,
        }
    }
}

/// Heterogeneous storage for evidence whose public insertion methods remain
/// statically typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportEvidenceRecord {
    Cardinality(SupportCellEvidence<ExactCardinalityClaim>),
    Injectivity(SupportCellEvidence<InjectiveMappingClaim>),
    Admission(SupportCellEvidence<AdmissionClassificationClaim>),
    Selection(SupportCellEvidence<SelectionClassificationClaim>),
    UniformValue(SupportCellEvidence<UniformValueClaim>),
    UniformMechanism(SupportCellEvidence<UniformMechanismClaim>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SupportEvidenceKind {
    Cardinality,
    Injectivity,
    Admission,
    Selection,
    UniformValue,
    UniformMechanism,
}

impl SupportEvidenceKind {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Cardinality => 0x01,
            Self::Injectivity => 0x02,
            Self::Admission => 0x03,
            Self::Selection => 0x04,
            Self::UniformValue => 0x05,
            Self::UniformMechanism => 0x06,
        }
    }
}

impl SupportEvidenceRecord {
    pub(crate) const fn id(&self) -> SupportCellEvidenceId {
        match self {
            Self::Cardinality(value) => value.id(),
            Self::Injectivity(value) => value.id(),
            Self::Admission(value) => value.id(),
            Self::Selection(value) => value.id(),
            Self::UniformValue(value) => value.id(),
            Self::UniformMechanism(value) => value.id(),
        }
    }

    pub(crate) const fn obligation_id(&self) -> SupportProofObligationId {
        match self {
            Self::Cardinality(value) => value.obligation().id(),
            Self::Injectivity(value) => value.obligation().id(),
            Self::Admission(value) => value.obligation().id(),
            Self::Selection(value) => value.obligation().id(),
            Self::UniformValue(value) => value.obligation().id(),
            Self::UniformMechanism(value) => value.obligation().id(),
        }
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        match self {
            Self::Cardinality(value) => value.obligation().cell_id(),
            Self::Injectivity(value) => value.obligation().cell_id(),
            Self::Admission(value) => value.obligation().cell_id(),
            Self::Selection(value) => value.obligation().cell_id(),
            Self::UniformValue(value) => value.obligation().cell_id(),
            Self::UniformMechanism(value) => value.obligation().cell_id(),
        }
    }

    pub(crate) const fn kind(&self) -> SupportEvidenceKind {
        match self {
            Self::Cardinality(_) => SupportEvidenceKind::Cardinality,
            Self::Injectivity(_) => SupportEvidenceKind::Injectivity,
            Self::Admission(_) => SupportEvidenceKind::Admission,
            Self::Selection(_) => SupportEvidenceKind::Selection,
            Self::UniformValue(_) => SupportEvidenceKind::UniformValue,
            Self::UniformMechanism(_) => SupportEvidenceKind::UniformMechanism,
        }
    }

    pub(crate) const fn conclusion_digest(&self) -> [u8; 32] {
        match self {
            Self::Cardinality(value) => value.receipt().conclusion_digest(),
            Self::Injectivity(value) => value.receipt().conclusion_digest(),
            Self::Admission(value) => value.receipt().conclusion_digest(),
            Self::Selection(value) => value.receipt().conclusion_digest(),
            Self::UniformValue(value) => value.receipt().conclusion_digest(),
            Self::UniformMechanism(value) => value.receipt().conclusion_digest(),
        }
    }

    pub(crate) fn obligation_record(&self) -> SupportObligationRecord {
        match self {
            Self::Cardinality(value) => {
                SupportObligationRecord::Cardinality(value.obligation().clone())
            }
            Self::Injectivity(value) => {
                SupportObligationRecord::Injectivity(value.obligation().clone())
            }
            Self::Admission(value) => {
                SupportObligationRecord::Admission(value.obligation().clone())
            }
            Self::Selection(value) => {
                SupportObligationRecord::Selection(value.obligation().clone())
            }
            Self::UniformValue(value) => {
                SupportObligationRecord::UniformValue(value.obligation().clone())
            }
            Self::UniformMechanism(value) => {
                SupportObligationRecord::UniformMechanism(value.obligation().clone())
            }
        }
    }

    fn validate(&self) -> Result<(), SupportCellError> {
        match self {
            Self::Cardinality(value) => value.validate(),
            Self::Injectivity(value) => value.validate(),
            Self::Admission(value) => value.validate(),
            Self::Selection(value) => value.validate(),
            Self::UniformValue(value) => value.validate(),
            Self::UniformMechanism(value) => value.validate(),
        }
    }
}

/// Content identity of one exact refinement of a typed proof obligation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportObligationRefinementId([u8; 32]);

impl SupportObligationRefinementId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// A typed parent obligation replaced by one same-claim obligation for every
/// child of an accepted exact cell partition.
///
/// Refinement is not evidence that the parent has one uniform conclusion. It
/// only changes the obligation frontier: the parent becomes superseded and the
/// child obligations become the active proof leaves. The open parent obligation
/// remains cataloged for audit, but accepted direct parent evidence and a
/// refinement are mutually exclusive: a proved parent never needs refinement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportObligationRefinement {
    id: SupportObligationRefinementId,
    parent_obligation_id: SupportProofObligationId,
    partition_id: SupportPartitionId,
    child_obligation_ids: Box<[SupportProofObligationId]>,
}

impl SupportObligationRefinement {
    pub(super) fn restore_from_journal_codec(
        parent_obligation_id: SupportProofObligationId,
        partition_id: SupportPartitionId,
        child_obligation_ids: Box<[SupportProofObligationId]>,
    ) -> Result<Self, SupportEvidenceError> {
        let id = derive_obligation_refinement_id(
            parent_obligation_id,
            partition_id,
            &child_obligation_ids,
        );
        let restored = Self {
            id,
            parent_obligation_id,
            partition_id,
            child_obligation_ids,
        };
        restored.validate_identity()?;
        Ok(restored)
    }

    pub(crate) fn new<'a>(
        parent: &SupportObligationRecord,
        partition: &SupportPartitionCertificate,
        children: impl IntoIterator<Item = &'a SupportObligationRecord>,
    ) -> Result<Self, SupportEvidenceError> {
        parent
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidObligation {
                obligation_id: parent.id(),
                source,
            })?;
        partition
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidPartition {
                partition_id: partition.id(),
                source,
            })?;
        if parent.cell_id() != partition.parent_id() {
            return Err(SupportEvidenceError::RefinementParentCellMismatch {
                parent_obligation_id: parent.id(),
                parent_cell_id: parent.cell_id(),
                partition_id: partition.id(),
                partition_parent_id: partition.parent_id(),
            });
        }

        let mut children_by_cell = BTreeMap::new();
        for child in children {
            child
                .validate()
                .map_err(|source| SupportEvidenceError::InvalidObligation {
                    obligation_id: child.id(),
                    source,
                })?;
            if children_by_cell.insert(child.cell_id(), child).is_some() {
                return Err(SupportEvidenceError::DuplicateRefinementChildCell {
                    parent_obligation_id: parent.id(),
                    child_cell_id: child.cell_id(),
                });
            }
        }
        if children_by_cell.len() != partition.child_ids().len() {
            return Err(SupportEvidenceError::RefinementChildCountMismatch {
                parent_obligation_id: parent.id(),
                expected: partition.child_ids().len(),
                actual: children_by_cell.len(),
            });
        }

        let mut ordered_child_ids = Vec::with_capacity(partition.child_ids().len());
        for child_cell_id in partition.child_ids() {
            let child = children_by_cell.get(child_cell_id).ok_or(
                SupportEvidenceError::RefinementChildCellMismatch {
                    parent_obligation_id: parent.id(),
                    child_cell_id: *child_cell_id,
                },
            )?;
            if !parent.has_same_claim(child) {
                return Err(SupportEvidenceError::RefinementClaimMismatch {
                    parent_obligation_id: parent.id(),
                    child_obligation_id: child.id(),
                });
            }
            ordered_child_ids.push(child.id());
        }
        let child_obligation_ids = ordered_child_ids.into_boxed_slice();
        let id =
            derive_obligation_refinement_id(parent.id(), partition.id(), &child_obligation_ids);
        Ok(Self {
            id,
            parent_obligation_id: parent.id(),
            partition_id: partition.id(),
            child_obligation_ids,
        })
    }

    pub(crate) const fn id(&self) -> SupportObligationRefinementId {
        self.id
    }

    pub(crate) const fn parent_obligation_id(&self) -> SupportProofObligationId {
        self.parent_obligation_id
    }

    pub(crate) const fn partition_id(&self) -> SupportPartitionId {
        self.partition_id
    }

    pub(crate) fn child_obligation_ids(&self) -> &[SupportProofObligationId] {
        &self.child_obligation_ids
    }

    fn validate_identity(&self) -> Result<(), SupportEvidenceError> {
        if self.child_obligation_ids.is_empty() {
            return Err(SupportEvidenceError::EmptyObligationRefinement);
        }
        let derived = derive_obligation_refinement_id(
            self.parent_obligation_id,
            self.partition_id,
            &self.child_obligation_ids,
        );
        if derived != self.id {
            return Err(SupportEvidenceError::RefinementIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportCursorInsert {
    Existing,
    InsertedHistorical,
    InsertedAdvanced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportRetainedExampleInsert {
    Existing,
    IgnoredByCap,
    Inserted,
    Replaced,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedExampleState {
    cap: usize,
    examples: BTreeSet<SupportExampleId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportRetainedExamplesSnapshot {
    cell_id: SupportCellId,
    cap: usize,
    examples: Box<[SupportExampleId]>,
}

impl SupportRetainedExamplesSnapshot {
    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn cap(&self) -> usize {
        self.cap
    }

    pub(crate) fn examples(&self) -> &[SupportExampleId] {
        &self.examples
    }
}

/// Mutable keyed catalog. Inserts update only directly affected indexes;
/// canonical sorting, graph traversal, and root hashing happen in `snapshot`.
#[derive(Clone, Debug, Default)]
pub(crate) struct SupportEvidenceCatalogBuilder {
    catalog_sealed: bool,
    root_frontier_sealed: bool,
    obligation_frontier_sealed: bool,
    cells: BTreeMap<SupportCellId, SupportCell>,
    root_cells: BTreeSet<SupportCellId>,
    partitions: BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
    partition_by_parent: BTreeMap<SupportCellId, SupportPartitionId>,
    sealed_leaf_claims: BTreeSet<SupportCellId>,
    obligations: BTreeMap<SupportProofObligationId, SupportObligationRecord>,
    root_obligations: BTreeSet<SupportProofObligationId>,
    obligation_refinements: BTreeMap<SupportObligationRefinementId, SupportObligationRefinement>,
    refinement_by_parent: BTreeMap<SupportProofObligationId, SupportObligationRefinementId>,
    evidence: BTreeMap<SupportCellEvidenceId, SupportEvidenceRecord>,
    conclusion_by_obligation: BTreeMap<SupportProofObligationId, [u8; 32]>,
    evidence_by_obligation: BTreeMap<SupportProofObligationId, BTreeSet<SupportCellEvidenceId>>,
    cursor_records: BTreeMap<SupportMaterializationCursorId, SupportMaterializationCursor>,
    cursor_by_position: BTreeMap<(SupportCellId, u128), SupportMaterializationCursorId>,
    latest_cursor_by_cell: BTreeMap<SupportCellId, SupportMaterializationCursorId>,
    retained_examples: BTreeMap<SupportCellId, RetainedExampleState>,
    admissions: BTreeMap<AdmissionId, RelationId>,
    questions: BTreeMap<QuestionId, AdmissionId>,
    choices: BTreeMap<ChoiceId, QuestionId>,
    mechanism_requests: BTreeMap<MechanismRequestId, QuestionId>,
    views: BTreeMap<ViewId, ViewInputId>,
    observers: BTreeMap<SupportObserverId, SupportObserverLayerScope>,
}

/// Fully validated closure projection over one immutable catalog prefix.
///
/// The projection owns only the derived ID sets needed by validation and
/// borrows every semantic record from the builder. It therefore supplies the
/// same closure facts and canonical root as [`SupportEvidenceSnapshot`]
/// without cloning the support graph or its evidence payloads.
pub(crate) struct ValidatedSupportEvidenceClosure<'a> {
    catalog: &'a SupportEvidenceCatalogBuilder,
    validated: ValidatedState,
}

impl ValidatedSupportEvidenceClosure<'_> {
    pub(crate) fn catalog_is_sealed(&self) -> bool {
        self.catalog.catalog_sealed
    }

    pub(crate) fn root_frontier_is_sealed(&self) -> bool {
        self.catalog.root_frontier_sealed
    }

    pub(crate) fn obligation_frontier_is_sealed(&self) -> bool {
        self.catalog.obligation_frontier_sealed
    }

    pub(crate) fn open_leaf_count(&self) -> usize {
        self.validated.open_leaf_ids.len()
    }

    pub(crate) fn open_obligation_count(&self) -> usize {
        self.validated.open_obligation_ids.len()
    }

    pub(crate) fn support_frontier_is_complete(&self) -> bool {
        self.root_frontier_is_sealed() && self.open_leaf_count() == 0
    }

    /// Whether the current prefix satisfies the preconditions for catalog
    /// seal. This deliberately does not require the seal bit itself.
    pub(crate) fn catalog_seal_is_ready(&self) -> bool {
        self.support_frontier_is_complete()
            && self.obligation_frontier_is_sealed()
            && self.open_obligation_count() == 0
    }

    /// Match the final-completeness meaning recorded by
    /// [`SupportEvidenceSnapshot`]. Refinement mutation remains possible until
    /// the catalog seal, even after every current obligation leaf is proved.
    pub(crate) fn obligation_frontier_is_complete(&self) -> bool {
        self.catalog_is_sealed()
            && self.obligation_frontier_is_sealed()
            && self.open_obligation_count() == 0
    }

    pub(crate) fn has_open_obligation_kind(&self, kind: SupportEvidenceKind) -> bool {
        self.validated
            .open_obligation_ids
            .iter()
            .any(|obligation_id| {
                self.catalog
                    .obligations
                    .get(obligation_id)
                    .expect("validated open obligation exists in the catalog")
                    .kind()
                    == kind
            })
    }

    pub(crate) fn classification_view(&self) -> SupportEvidenceClassificationView<'_> {
        SupportEvidenceClassificationView {
            cells: &self.catalog.cells,
            root_cells: &self.catalog.root_cells,
            partitions: &self.catalog.partitions,
            partition_by_parent: &self.catalog.partition_by_parent,
            sealed_leaf_claims: &self.validated.sealed_leaf_ids,
            evidence: &self.catalog.evidence,
        }
    }

    pub(crate) fn active_leaf_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.validated.active_leaf_ids.iter().copied()
    }

    /// Hash the already validated immutable prefix in the existing canonical
    /// domain and order. No presentation or operational state is added.
    pub(crate) fn root(&self) -> SupportEvidenceRoot {
        support_evidence_root(self.catalog)
    }
}

/// Borrowed subset of support state needed to count the classified case-root
/// population. The view deliberately excludes owned snapshots and exposes no
/// mutation surface; callers walk only the installed case-root subtree while
/// ignoring auxiliary source-proof cells.
#[derive(Clone, Copy)]
pub(crate) struct SupportEvidenceClassificationView<'a> {
    cells: &'a BTreeMap<SupportCellId, SupportCell>,
    root_cells: &'a BTreeSet<SupportCellId>,
    partitions: &'a BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
    partition_by_parent: &'a BTreeMap<SupportCellId, SupportPartitionId>,
    sealed_leaf_claims: &'a BTreeSet<SupportCellId>,
    evidence: &'a BTreeMap<SupportCellEvidenceId, SupportEvidenceRecord>,
}

impl SupportEvidenceClassificationView<'_> {
    pub(crate) fn cell(&self, cell_id: SupportCellId) -> Option<&SupportCell> {
        self.cells.get(&cell_id)
    }

    pub(crate) fn root_cell_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.root_cells.iter().copied()
    }

    pub(crate) fn partitions(&self) -> impl ExactSizeIterator<Item = &SupportPartitionCertificate> {
        self.partitions.values()
    }

    pub(crate) fn partition_for_parent(
        &self,
        parent_id: SupportCellId,
    ) -> Option<&SupportPartitionCertificate> {
        self.partition_by_parent
            .get(&parent_id)
            .and_then(|partition_id| self.partitions.get(partition_id))
    }

    pub(crate) fn sealed_leaf_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.sealed_leaf_claims.iter().copied()
    }

    pub(crate) fn evidence(&self) -> impl ExactSizeIterator<Item = &SupportEvidenceRecord> {
        self.evidence.values()
    }
}

/// Bounded append transaction over the keyed support catalog.
///
/// Only the mutation surface needed by one classified-chunk journal event is
/// exposed. Each wrapper records the exact prior values of the keys it may
/// touch before delegating to the established validator. Dropping an
/// uncommitted transaction restores those values in reverse order, so a late
/// semantic error cannot expose a partial durable fold and no complete catalog
/// clone is needed.
pub(crate) struct SupportEvidenceAppendTransaction<'a> {
    catalog: &'a mut SupportEvidenceCatalogBuilder,
    undo: Vec<SupportEvidenceUndo>,
    committed: bool,
}

#[derive(Debug)]
enum SupportEvidenceUndo {
    Cell {
        cell_id: SupportCellId,
        prior: Option<SupportCell>,
    },
    Partition {
        partition_id: SupportPartitionId,
        prior_partition: Option<SupportPartitionCertificate>,
        parent_id: SupportCellId,
        prior_parent_partition: Option<SupportPartitionId>,
    },
    RootObligation {
        obligation_id: SupportProofObligationId,
        prior_obligation: Option<SupportObligationRecord>,
        was_root: bool,
    },
    ObligationRefinement {
        refinement_id: SupportObligationRefinementId,
        prior_refinement: Option<SupportObligationRefinement>,
        parent_obligation_id: SupportProofObligationId,
        prior_parent_refinement: Option<SupportObligationRefinementId>,
        prior_children: Box<[(SupportProofObligationId, Option<SupportObligationRecord>)]>,
    },
    Evidence {
        evidence_id: SupportCellEvidenceId,
        prior_evidence: Option<SupportEvidenceRecord>,
        obligation_id: SupportProofObligationId,
        prior_conclusion: Option<[u8; 32]>,
        prior_obligation_evidence: Option<BTreeSet<SupportCellEvidenceId>>,
    },
    SealedLeaf {
        cell_id: SupportCellId,
        was_sealed: bool,
    },
    Cursor {
        cursor_id: SupportMaterializationCursorId,
        prior_cursor: Option<SupportMaterializationCursor>,
        position: (SupportCellId, u128),
        prior_position_cursor: Option<SupportMaterializationCursorId>,
        cell_id: SupportCellId,
        prior_latest_cursor: Option<SupportMaterializationCursorId>,
    },
}

impl SupportEvidenceAppendTransaction<'_> {
    fn record(&mut self, undo: SupportEvidenceUndo) -> Result<(), SupportEvidenceError> {
        if self.undo.len() == self.undo.capacity() {
            return Err(SupportEvidenceError::AtomicAppendReservationFailed);
        }
        self.undo.push(undo);
        Ok(())
    }

    pub(crate) fn insert_known_cell(
        &mut self,
        cell: SupportCell,
    ) -> Result<bool, SupportEvidenceError> {
        let cell_id = cell.id();
        let prior = self.catalog.cells.get(&cell_id).cloned();
        self.record(SupportEvidenceUndo::Cell { cell_id, prior })?;
        self.catalog.insert_known_cell(cell)
    }

    pub(crate) fn insert_known_partition(
        &mut self,
        certificate: SupportPartitionCertificate,
    ) -> Result<bool, SupportEvidenceError> {
        let partition_id = certificate.id();
        let parent_id = certificate.parent_id();
        let prior_partition = self.catalog.partitions.get(&partition_id).cloned();
        let prior_parent_partition = self.catalog.partition_by_parent.get(&parent_id).copied();
        self.record(SupportEvidenceUndo::Partition {
            partition_id,
            prior_partition,
            parent_id,
            prior_parent_partition,
        })?;
        self.catalog.insert_known_partition(certificate)
    }

    pub(crate) fn declare_root_obligation_record(
        &mut self,
        obligation: SupportObligationRecord,
    ) -> Result<bool, SupportEvidenceError> {
        let obligation_id = obligation.id();
        let prior_obligation = self.catalog.obligations.get(&obligation_id).cloned();
        let was_root = self.catalog.root_obligations.contains(&obligation_id);
        self.record(SupportEvidenceUndo::RootObligation {
            obligation_id,
            prior_obligation,
            was_root,
        })?;
        self.catalog
            .declare_causal_root_obligation_record(obligation)
    }

    pub(crate) fn insert_obligation_refinement_with_children(
        &mut self,
        refinement: SupportObligationRefinement,
        child_obligations: Box<[SupportObligationRecord]>,
    ) -> Result<bool, SupportEvidenceError> {
        let refinement_id = refinement.id();
        let parent_obligation_id = refinement.parent_obligation_id();
        let prior_refinement = self
            .catalog
            .obligation_refinements
            .get(&refinement_id)
            .cloned();
        let prior_parent_refinement = self
            .catalog
            .refinement_by_parent
            .get(&parent_obligation_id)
            .copied();
        let prior_children = child_obligations
            .iter()
            .map(|child| {
                (
                    child.id(),
                    self.catalog.obligations.get(&child.id()).cloned(),
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.record(SupportEvidenceUndo::ObligationRefinement {
            refinement_id,
            prior_refinement,
            parent_obligation_id,
            prior_parent_refinement,
            prior_children,
        })?;
        self.catalog
            .insert_obligation_refinement_with_children(refinement, child_obligations)
    }

    pub(crate) fn insert_declared_evidence_record(
        &mut self,
        evidence: SupportEvidenceRecord,
    ) -> Result<bool, SupportEvidenceError> {
        let evidence_id = evidence.id();
        let obligation_id = evidence.obligation_id();
        let prior_evidence = self.catalog.evidence.get(&evidence_id).cloned();
        let prior_conclusion = self
            .catalog
            .conclusion_by_obligation
            .get(&obligation_id)
            .copied();
        let prior_obligation_evidence = self
            .catalog
            .evidence_by_obligation
            .get(&obligation_id)
            .cloned();
        self.record(SupportEvidenceUndo::Evidence {
            evidence_id,
            prior_evidence,
            obligation_id,
            prior_conclusion,
            prior_obligation_evidence,
        })?;
        self.catalog.insert_declared_evidence_record(evidence)
    }

    pub(crate) fn seal_known_leaf(
        &mut self,
        cell_id: SupportCellId,
    ) -> Result<bool, SupportEvidenceError> {
        let was_sealed = self.catalog.sealed_leaf_claims.contains(&cell_id);
        self.record(SupportEvidenceUndo::SealedLeaf {
            cell_id,
            was_sealed,
        })?;
        self.catalog.seal_known_leaf(cell_id)
    }

    pub(crate) fn insert_cursor(
        &mut self,
        cursor: SupportMaterializationCursor,
    ) -> Result<SupportCursorInsert, SupportEvidenceError> {
        let cursor_id = cursor.id();
        let cell_id = cursor.cell_id();
        let position = (cell_id, cursor.next_coordinate_ordinal());
        let prior_cursor = self.catalog.cursor_records.get(&cursor_id).cloned();
        let prior_position_cursor = self.catalog.cursor_by_position.get(&position).copied();
        let prior_latest_cursor = self.catalog.latest_cursor_by_cell.get(&cell_id).copied();
        self.record(SupportEvidenceUndo::Cursor {
            cursor_id,
            prior_cursor,
            position,
            prior_position_cursor,
            cell_id,
            prior_latest_cursor,
        })?;
        self.catalog.insert_cursor(cursor)
    }

    pub(crate) fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for SupportEvidenceAppendTransaction<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        while let Some(undo) = self.undo.pop() {
            match undo {
                SupportEvidenceUndo::Cell { cell_id, prior } => {
                    restore_map_entry(&mut self.catalog.cells, cell_id, prior);
                }
                SupportEvidenceUndo::Partition {
                    partition_id,
                    prior_partition,
                    parent_id,
                    prior_parent_partition,
                } => {
                    restore_map_entry(
                        &mut self.catalog.partition_by_parent,
                        parent_id,
                        prior_parent_partition,
                    );
                    restore_map_entry(&mut self.catalog.partitions, partition_id, prior_partition);
                }
                SupportEvidenceUndo::RootObligation {
                    obligation_id,
                    prior_obligation,
                    was_root,
                } => {
                    restore_set_membership(
                        &mut self.catalog.root_obligations,
                        obligation_id,
                        was_root,
                    );
                    restore_map_entry(
                        &mut self.catalog.obligations,
                        obligation_id,
                        prior_obligation,
                    );
                }
                SupportEvidenceUndo::ObligationRefinement {
                    refinement_id,
                    prior_refinement,
                    parent_obligation_id,
                    prior_parent_refinement,
                    prior_children,
                } => {
                    restore_map_entry(
                        &mut self.catalog.refinement_by_parent,
                        parent_obligation_id,
                        prior_parent_refinement,
                    );
                    restore_map_entry(
                        &mut self.catalog.obligation_refinements,
                        refinement_id,
                        prior_refinement,
                    );
                    for (obligation_id, prior) in Vec::from(prior_children).into_iter().rev() {
                        restore_map_entry(&mut self.catalog.obligations, obligation_id, prior);
                    }
                }
                SupportEvidenceUndo::Evidence {
                    evidence_id,
                    prior_evidence,
                    obligation_id,
                    prior_conclusion,
                    prior_obligation_evidence,
                } => {
                    restore_map_entry(
                        &mut self.catalog.evidence_by_obligation,
                        obligation_id,
                        prior_obligation_evidence,
                    );
                    restore_map_entry(
                        &mut self.catalog.conclusion_by_obligation,
                        obligation_id,
                        prior_conclusion,
                    );
                    restore_map_entry(&mut self.catalog.evidence, evidence_id, prior_evidence);
                }
                SupportEvidenceUndo::SealedLeaf {
                    cell_id,
                    was_sealed,
                } => {
                    restore_set_membership(
                        &mut self.catalog.sealed_leaf_claims,
                        cell_id,
                        was_sealed,
                    );
                }
                SupportEvidenceUndo::Cursor {
                    cursor_id,
                    prior_cursor,
                    position,
                    prior_position_cursor,
                    cell_id,
                    prior_latest_cursor,
                } => {
                    restore_map_entry(
                        &mut self.catalog.latest_cursor_by_cell,
                        cell_id,
                        prior_latest_cursor,
                    );
                    restore_map_entry(
                        &mut self.catalog.cursor_by_position,
                        position,
                        prior_position_cursor,
                    );
                    restore_map_entry(&mut self.catalog.cursor_records, cursor_id, prior_cursor);
                }
            }
        }
    }
}

fn restore_map_entry<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, prior: Option<V>) {
    match prior {
        Some(value) => {
            map.insert(key, value);
        }
        None => {
            map.remove(&key);
        }
    }
}

fn restore_set_membership<K: Ord>(set: &mut BTreeSet<K>, key: K, was_member: bool) {
    if was_member {
        set.insert(key);
    } else {
        set.remove(&key);
    }
}

impl SupportEvidenceCatalogBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn begin_append_transaction(
        &mut self,
        undo_capacity: usize,
    ) -> Result<SupportEvidenceAppendTransaction<'_>, SupportEvidenceError> {
        let mut undo = Vec::new();
        undo.try_reserve_exact(undo_capacity)
            .map_err(|_| SupportEvidenceError::AtomicAppendReservationFailed)?;
        Ok(SupportEvidenceAppendTransaction {
            catalog: self,
            undo,
            committed: false,
        })
    }

    pub(crate) fn validated_closure(
        &self,
    ) -> Result<ValidatedSupportEvidenceClosure<'_>, SupportEvidenceError> {
        Ok(ValidatedSupportEvidenceClosure {
            catalog: self,
            validated: self.validate_state()?,
        })
    }

    pub(crate) const fn catalog_is_sealed(&self) -> bool {
        self.catalog_sealed
    }

    pub(crate) const fn root_frontier_is_sealed(&self) -> bool {
        self.root_frontier_sealed
    }

    pub(crate) const fn obligation_frontier_is_sealed(&self) -> bool {
        self.obligation_frontier_sealed
    }

    pub(crate) fn classification_view(&self) -> SupportEvidenceClassificationView<'_> {
        // `insert_partition` rejects sealed parents and `seal_leaf` rejects
        // partitioned cells. Expose the raw claims anyway so the borrowed
        // classification reduction retains its defensive subset check.
        SupportEvidenceClassificationView {
            cells: &self.cells,
            root_cells: &self.root_cells,
            partitions: &self.partitions,
            partition_by_parent: &self.partition_by_parent,
            sealed_leaf_claims: &self.sealed_leaf_claims,
            evidence: &self.evidence,
        }
    }

    pub(crate) fn cell(&self, cell_id: SupportCellId) -> Option<&SupportCell> {
        self.cells.get(&cell_id)
    }

    pub(crate) fn obligation(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<&SupportObligationRecord> {
        self.obligations.get(&obligation_id)
    }

    pub(crate) fn obligation_cell(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<SupportCellId> {
        self.obligation(obligation_id)
            .map(SupportObligationRecord::cell_id)
    }

    /// Whether a declared root obligation is still an active, unresolved
    /// proof leaf. `None` distinguishes an unknown or non-root obligation from
    /// a root that has already acquired evidence or been refined.
    pub(crate) fn root_obligation_is_open(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<bool> {
        self.obligations.get(&obligation_id)?;
        if !self.root_obligations.contains(&obligation_id) {
            return None;
        }
        Some(
            !self.refinement_by_parent.contains_key(&obligation_id)
                && !self.conclusion_by_obligation.contains_key(&obligation_id),
        )
    }

    pub(crate) fn evidence_record(
        &self,
        evidence_id: SupportCellEvidenceId,
    ) -> Option<&SupportEvidenceRecord> {
        self.evidence.get(&evidence_id)
    }

    pub(crate) fn cardinality_evidence_for_obligation(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<&SupportCellEvidence<ExactCardinalityClaim>> {
        let evidence_id = self.evidence_by_obligation.get(&obligation_id)?.first()?;
        match self.evidence.get(evidence_id)? {
            SupportEvidenceRecord::Cardinality(evidence) => Some(evidence),
            _ => None,
        }
    }

    pub(crate) fn admission_evidence_for_obligation(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<&SupportCellEvidence<AdmissionClassificationClaim>> {
        let evidence_id = self.evidence_by_obligation.get(&obligation_id)?.first()?;
        match self.evidence.get(evidence_id)? {
            SupportEvidenceRecord::Admission(evidence) => Some(evidence),
            _ => None,
        }
    }

    pub(crate) fn obligation_refinement(
        &self,
        refinement_id: SupportObligationRefinementId,
    ) -> Option<&SupportObligationRefinement> {
        self.obligation_refinements.get(&refinement_id)
    }

    pub(crate) fn refinement_for_parent(
        &self,
        parent_obligation_id: SupportProofObligationId,
    ) -> Option<&SupportObligationRefinement> {
        self.refinement_by_parent
            .get(&parent_obligation_id)
            .and_then(|refinement_id| self.obligation_refinements.get(refinement_id))
    }

    pub(crate) fn latest_cursor(
        &self,
        cell_id: SupportCellId,
    ) -> Option<&SupportMaterializationCursor> {
        self.latest_cursor_by_cell
            .get(&cell_id)
            .and_then(|cursor_id| self.cursor_records.get(cursor_id))
    }

    pub(crate) fn cursor(
        &self,
        cursor_id: SupportMaterializationCursorId,
    ) -> Option<&SupportMaterializationCursor> {
        self.cursor_records.get(&cursor_id)
    }

    pub(crate) fn cursor_at(
        &self,
        cell_id: SupportCellId,
        next_coordinate_ordinal: u128,
    ) -> Option<&SupportMaterializationCursor> {
        self.cursor_by_position
            .get(&(cell_id, next_coordinate_ordinal))
            .and_then(|cursor_id| self.cursor_records.get(cursor_id))
    }

    pub(crate) fn latest_cursors(
        &self,
    ) -> impl ExactSizeIterator<Item = &SupportMaterializationCursor> {
        self.latest_cursor_by_cell
            .values()
            .map(|cursor_id| &self.cursor_records[cursor_id])
    }

    pub(crate) fn insert_cell(&mut self, cell: SupportCell) -> Result<bool, SupportEvidenceError> {
        cell.validate()
            .map_err(|source| SupportEvidenceError::InvalidCell {
                cell_id: cell.id(),
                source,
            })?;
        let cell_id = cell.id();
        match self.cells.get(&cell_id) {
            Some(existing) if existing == &cell => return Ok(false),
            Some(_) => return Err(SupportEvidenceError::CellIdCollision { cell_id }),
            None => {}
        }
        self.require_catalog_open()?;
        self.cells.insert(cell_id, cell);
        Ok(true)
    }

    /// Insert one cell only after every cell named by a nested join expression
    /// is already durable. This is the causal single-event journal boundary;
    /// logical planning may instead register a complete catalog atomically.
    pub(crate) fn insert_known_cell(
        &mut self,
        cell: SupportCell,
    ) -> Result<bool, SupportEvidenceError> {
        let cell_id = cell.id();
        if self
            .cells
            .get(&cell_id)
            .is_some_and(|existing| existing == &cell)
        {
            return Ok(false);
        }
        for input_cell_id in expression_input_cell_ids(cell.expression()) {
            if !self.cells.contains_key(&input_cell_id) {
                return Err(SupportEvidenceError::UnknownExpressionInput {
                    cell_id,
                    input_cell_id,
                });
            }
        }
        self.insert_cell(cell)
    }

    /// Atomically register a complete logical-plan cell catalog. References
    /// may appear in any payload order; the combined catalog is validated for
    /// dangling inputs and cycles before it replaces the current state.
    pub(crate) fn insert_cell_catalog(
        &mut self,
        cells: impl IntoIterator<Item = SupportCell>,
    ) -> Result<bool, SupportEvidenceError> {
        let mut candidate = self.clone();
        let mut changed = false;
        for cell in cells {
            changed |= candidate.insert_cell(cell)?;
        }
        validate_expression_dependency_graph(&candidate.cells)?;
        if changed {
            *self = candidate;
        }
        Ok(changed)
    }

    pub(crate) fn declare_root_cell(
        &mut self,
        cell_id: SupportCellId,
    ) -> Result<bool, SupportEvidenceError> {
        if self.root_cells.contains(&cell_id) {
            return Ok(false);
        }
        if self.root_frontier_sealed {
            return Err(SupportEvidenceError::RootFrontierSealed);
        }
        self.require_catalog_open()?;
        self.root_cells.insert(cell_id);
        Ok(true)
    }

    /// Declare a root only after its complete cell record is available.
    ///
    /// The general catalog API permits arrival-order-independent ingestion;
    /// journal replay uses this causal entry point so every accepted prefix is
    /// independently snapshot-valid.
    pub(crate) fn declare_known_root_cell(
        &mut self,
        cell_id: SupportCellId,
    ) -> Result<bool, SupportEvidenceError> {
        self.require_cell(cell_id, SupportReferenceKind::Root)?;
        self.declare_root_cell(cell_id)
    }

    pub(crate) fn seal_root_frontier(&mut self) -> Result<bool, SupportEvidenceError> {
        if self.root_frontier_sealed {
            return Ok(false);
        }
        self.require_catalog_open()?;
        self.root_frontier_sealed = true;
        Ok(true)
    }

    pub(crate) fn insert_partition(
        &mut self,
        certificate: SupportPartitionCertificate,
    ) -> Result<bool, SupportEvidenceError> {
        certificate
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidPartition {
                partition_id: certificate.id(),
                source,
            })?;
        let partition_id = certificate.id();
        let parent_id = certificate.parent_id();
        match self.partitions.get(&partition_id) {
            Some(existing) if existing == &certificate => return Ok(false),
            Some(_) => {
                return Err(SupportEvidenceError::PartitionIdCollision { partition_id });
            }
            None => {}
        }
        if let Some(existing_id) = self.partition_by_parent.get(&parent_id) {
            return Err(SupportEvidenceError::IncompatibleParentReplacement {
                parent_id,
                first_partition_id: *existing_id,
                second_partition_id: partition_id,
            });
        }
        if self.sealed_leaf_claims.contains(&parent_id) {
            return Err(SupportEvidenceError::PartitionForSealedLeaf { parent_id });
        }
        self.require_catalog_open()?;
        self.partition_by_parent.insert(parent_id, partition_id);
        self.partitions.insert(partition_id, certificate);
        Ok(true)
    }

    /// Insert a partition only after its parent and children are cataloged and
    /// after proving that the new edge cannot close a partition cycle.
    pub(crate) fn insert_known_partition(
        &mut self,
        certificate: SupportPartitionCertificate,
    ) -> Result<bool, SupportEvidenceError> {
        certificate
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidPartition {
                partition_id: certificate.id(),
                source,
            })?;
        self.require_cell(
            certificate.parent_id(),
            SupportReferenceKind::PartitionParent,
        )?;
        for child_id in certificate.child_ids() {
            self.require_cell(*child_id, SupportReferenceKind::PartitionChild)?;
        }

        let is_existing = self
            .partitions
            .get(&certificate.id())
            .is_some_and(|existing| existing == &certificate);
        if !is_existing {
            ensure_partition_extension_is_acyclic(
                certificate.parent_id(),
                certificate.child_ids(),
                &self.partition_by_parent,
                &self.partitions,
            )?;
        }
        self.insert_partition(certificate)
    }

    /// Assert that one current or future active leaf needs no further split.
    /// A separately open proof obligation on that cell remains open.
    pub(crate) fn seal_leaf(
        &mut self,
        cell_id: SupportCellId,
    ) -> Result<bool, SupportEvidenceError> {
        if self.sealed_leaf_claims.contains(&cell_id) {
            return Ok(false);
        }
        if self.partition_by_parent.contains_key(&cell_id) {
            return Err(SupportEvidenceError::SealedLeafHasPartition { cell_id });
        }
        self.require_catalog_open()?;
        self.sealed_leaf_claims.insert(cell_id);
        Ok(true)
    }

    /// Seal a leaf only after its complete cell record is available.
    pub(crate) fn seal_known_leaf(
        &mut self,
        cell_id: SupportCellId,
    ) -> Result<bool, SupportEvidenceError> {
        self.require_cell(cell_id, SupportReferenceKind::SealedLeaf)?;
        self.seal_leaf(cell_id)
    }

    pub(crate) fn insert_obligation(
        &mut self,
        obligation: SupportObligationRecord,
    ) -> Result<bool, SupportEvidenceError> {
        obligation
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidObligation {
                obligation_id: obligation.id(),
                source,
            })?;
        let obligation_id = obligation.id();
        match self.obligations.get(&obligation_id) {
            Some(existing) if existing == &obligation => return Ok(false),
            Some(_) => {
                return Err(SupportEvidenceError::ObligationIdCollision { obligation_id });
            }
            None => {}
        }
        if self.obligation_frontier_sealed {
            return Err(SupportEvidenceError::ObligationFrontierSealed);
        }
        self.require_catalog_open()?;
        self.obligations.insert(obligation_id, obligation);
        Ok(true)
    }

    /// Declare one top-level question the obligation DAG must discharge.
    /// The obligation record itself may arrive before or after this event.
    pub(crate) fn declare_root_obligation(
        &mut self,
        obligation_id: SupportProofObligationId,
    ) -> Result<bool, SupportEvidenceError> {
        if self.root_obligations.contains(&obligation_id) {
            return Ok(false);
        }
        if self.obligation_frontier_sealed {
            return Err(SupportEvidenceError::ObligationFrontierSealed);
        }
        self.require_catalog_open()?;
        self.root_obligations.insert(obligation_id);
        Ok(true)
    }

    /// Atomically insert and declare one complete root obligation record.
    ///
    /// Unlike the arrival-order-independent pair of general catalog methods,
    /// this leaves no durable prefix in which a declared root dangles or an
    /// inserted obligation is unreachable from the declared DAG.
    pub(crate) fn declare_root_obligation_record(
        &mut self,
        obligation: SupportObligationRecord,
    ) -> Result<bool, SupportEvidenceError> {
        obligation
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidObligation {
                obligation_id: obligation.id(),
                source,
            })?;
        let obligation_id = obligation.id();
        let cell =
            self.require_cell(obligation.cell_id(), SupportReferenceKind::ProofObligation)?;
        self.validate_obligation_scope(&obligation, cell)?;

        let obligation_is_new = match self.obligations.get(&obligation_id) {
            Some(existing) if existing == &obligation => false,
            Some(_) => {
                return Err(SupportEvidenceError::ObligationIdCollision { obligation_id });
            }
            None => true,
        };
        if self
            .obligation_refinements
            .values()
            .any(|refinement| refinement.child_obligation_ids().contains(&obligation_id))
        {
            return Err(SupportEvidenceError::RootObligationIsRefinementChild { obligation_id });
        }
        let root_is_new = !self.root_obligations.contains(&obligation_id);
        if !obligation_is_new && !root_is_new {
            return Ok(false);
        }
        if self.obligation_frontier_sealed {
            return Err(SupportEvidenceError::ObligationFrontierSealed);
        }
        self.require_catalog_open()?;

        if obligation_is_new {
            self.obligations.insert(obligation_id, obligation);
        }
        self.root_obligations.insert(obligation_id);
        Ok(true)
    }

    /// Declare one root obligation against an independently valid causal
    /// journal prefix.
    ///
    /// Unlike [`Self::declare_root_obligation_record`], this boundary does not
    /// support promoting an earlier arrival-order-independent obligation into a
    /// root. Classified-chunk replay either introduces a fresh root obligation
    /// or repeats an exact root already installed by the same authenticated
    /// event. Consequently an exact existing non-root is necessarily a
    /// refinement child and can be rejected with indexed lookups, without
    /// scanning every retained refinement and child.
    fn declare_causal_root_obligation_record(
        &mut self,
        obligation: SupportObligationRecord,
    ) -> Result<bool, SupportEvidenceError> {
        obligation
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidObligation {
                obligation_id: obligation.id(),
                source,
            })?;
        let obligation_id = obligation.id();
        let cell =
            self.require_cell(obligation.cell_id(), SupportReferenceKind::ProofObligation)?;
        self.validate_obligation_scope(&obligation, cell)?;

        match self.obligations.get(&obligation_id) {
            Some(existing) if existing != &obligation => {
                return Err(SupportEvidenceError::ObligationIdCollision { obligation_id });
            }
            Some(_) if self.root_obligations.contains(&obligation_id) => return Ok(false),
            Some(_) => {
                return Err(SupportEvidenceError::RootObligationIsRefinementChild {
                    obligation_id,
                });
            }
            None => {}
        }
        if self.obligation_frontier_sealed {
            return Err(SupportEvidenceError::ObligationFrontierSealed);
        }
        self.require_catalog_open()?;
        self.obligations.insert(obligation_id, obligation);
        self.root_obligations.insert(obligation_id);
        Ok(true)
    }

    /// Seal both obligation-record discovery and explicit root declarations.
    /// Refinements between already declared records may still arrive until the
    /// semantic catalog itself is sealed.
    pub(crate) fn seal_obligation_frontier(&mut self) -> Result<bool, SupportEvidenceError> {
        if self.obligation_frontier_sealed {
            return Ok(false);
        }
        self.require_catalog_open()?;
        self.obligation_frontier_sealed = true;
        Ok(true)
    }

    pub(crate) fn insert_obligation_refinement(
        &mut self,
        refinement: SupportObligationRefinement,
    ) -> Result<bool, SupportEvidenceError> {
        refinement.validate_identity()?;
        let refinement_id = refinement.id();
        let parent_obligation_id = refinement.parent_obligation_id();
        match self.obligation_refinements.get(&refinement_id) {
            Some(existing) if existing == &refinement => return Ok(false),
            Some(_) => {
                return Err(SupportEvidenceError::RefinementIdCollision { refinement_id });
            }
            None => {}
        }
        if let Some(first_refinement_id) = self.refinement_by_parent.get(&parent_obligation_id) {
            return Err(SupportEvidenceError::IncompatibleObligationRefinement {
                parent_obligation_id,
                first_refinement_id: *first_refinement_id,
                second_refinement_id: refinement_id,
            });
        }
        if self
            .conclusion_by_obligation
            .contains_key(&parent_obligation_id)
        {
            return Err(SupportEvidenceError::RefinementForProvedObligation {
                parent_obligation_id,
                refinement_id,
            });
        }
        self.require_catalog_open()?;
        self.refinement_by_parent
            .insert(parent_obligation_id, refinement_id);
        self.obligation_refinements
            .insert(refinement_id, refinement);
        Ok(true)
    }

    /// Atomically refine an existing obligation and insert all complete child
    /// obligation records named by the refinement.
    ///
    /// Every validation happens before any index changes. This is the causal
    /// journal boundary: replay never exposes dangling child IDs, unreachable
    /// child records, or a partially installed replacement.
    pub(crate) fn insert_obligation_refinement_with_children(
        &mut self,
        refinement: SupportObligationRefinement,
        child_obligations: Box<[SupportObligationRecord]>,
    ) -> Result<bool, SupportEvidenceError> {
        refinement.validate_identity()?;
        let refinement_id = refinement.id();
        let parent_obligation_id = refinement.parent_obligation_id();
        let parent = self.obligations.get(&parent_obligation_id).ok_or(
            SupportEvidenceError::UnknownRefinementObligation {
                refinement_id,
                obligation_id: parent_obligation_id,
            },
        )?;
        let partition = self.partitions.get(&refinement.partition_id()).ok_or(
            SupportEvidenceError::UnknownRefinementPartition {
                refinement_id,
                partition_id: refinement.partition_id(),
            },
        )?;

        let supplied_child_ids = child_obligations
            .iter()
            .map(SupportObligationRecord::id)
            .collect::<Vec<_>>();
        if supplied_child_ids.as_slice() != refinement.child_obligation_ids() {
            return Err(SupportEvidenceError::NonCanonicalObligationRefinement { refinement_id });
        }
        let canonical =
            SupportObligationRefinement::new(parent, partition, child_obligations.iter())?;
        if canonical != refinement {
            return Err(SupportEvidenceError::NonCanonicalObligationRefinement { refinement_id });
        }

        let mut new_child_ids = Vec::new();
        for child in &child_obligations {
            let child_id = child.id();
            let cell = self.require_cell(child.cell_id(), SupportReferenceKind::ProofObligation)?;
            self.validate_obligation_scope(child, cell)?;
            match self.obligations.get(&child_id) {
                Some(existing) if existing == child => {}
                Some(_) => {
                    return Err(SupportEvidenceError::ObligationIdCollision {
                        obligation_id: child_id,
                    });
                }
                None => new_child_ids.push(child_id),
            }
            if self.root_obligations.contains(&child_id) {
                return Err(SupportEvidenceError::RootObligationIsRefinementChild {
                    obligation_id: child_id,
                });
            }
        }

        let refinement_is_new = match self.obligation_refinements.get(&refinement_id) {
            Some(existing) if existing == &refinement => false,
            Some(_) => {
                return Err(SupportEvidenceError::RefinementIdCollision { refinement_id });
            }
            None => true,
        };
        match self.refinement_by_parent.get(&parent_obligation_id) {
            Some(existing_id) if *existing_id == refinement_id => {
                if refinement_is_new {
                    return Err(SupportEvidenceError::RefinementIndexMismatch);
                }
            }
            Some(existing_id) => {
                return Err(SupportEvidenceError::IncompatibleObligationRefinement {
                    parent_obligation_id,
                    first_refinement_id: *existing_id,
                    second_refinement_id: refinement_id,
                });
            }
            None if !refinement_is_new => {
                return Err(SupportEvidenceError::RefinementIndexMismatch);
            }
            None => {}
        }
        if self
            .conclusion_by_obligation
            .contains_key(&parent_obligation_id)
        {
            return Err(SupportEvidenceError::RefinementForProvedObligation {
                parent_obligation_id,
                refinement_id,
            });
        }
        if refinement_is_new {
            ensure_obligation_refinement_extension_is_acyclic(
                parent_obligation_id,
                refinement.child_obligation_ids(),
                &self.refinement_by_parent,
                &self.obligation_refinements,
            )?;
        }
        if self.obligation_frontier_sealed && !new_child_ids.is_empty() {
            return Err(SupportEvidenceError::ObligationFrontierSealed);
        }

        let changed = refinement_is_new || !new_child_ids.is_empty();
        if !changed {
            return Ok(false);
        }
        self.require_catalog_open()?;

        for child in Vec::from(child_obligations) {
            self.obligations.entry(child.id()).or_insert(child);
        }
        if refinement_is_new {
            self.refinement_by_parent
                .insert(parent_obligation_id, refinement_id);
            self.obligation_refinements
                .insert(refinement_id, refinement);
        }
        Ok(true)
    }

    pub(crate) fn insert_cardinality_evidence(
        &mut self,
        evidence: SupportCellEvidence<ExactCardinalityClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::Cardinality(evidence))
    }

    pub(crate) fn insert_injectivity_evidence(
        &mut self,
        evidence: SupportCellEvidence<InjectiveMappingClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::Injectivity(evidence))
    }

    pub(crate) fn insert_admission_evidence(
        &mut self,
        evidence: SupportCellEvidence<AdmissionClassificationClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::Admission(evidence))
    }

    pub(crate) fn insert_selection_evidence(
        &mut self,
        evidence: SupportCellEvidence<SelectionClassificationClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::Selection(evidence))
    }

    pub(crate) fn insert_uniform_value_evidence(
        &mut self,
        evidence: SupportCellEvidence<UniformValueClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::UniformValue(evidence))
    }

    pub(crate) fn insert_uniform_mechanism_evidence(
        &mut self,
        evidence: SupportCellEvidence<UniformMechanismClaim>,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record(SupportEvidenceRecord::UniformMechanism(evidence))
    }

    /// Insert evidence only for an already cataloged, exactly matching
    /// obligation. Journal replay uses this stricter boundary so accepting
    /// evidence cannot implicitly invent an unreachable obligation record.
    pub(crate) fn insert_declared_evidence_record(
        &mut self,
        evidence: SupportEvidenceRecord,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record_inner(evidence, true)
    }

    fn insert_evidence_record(
        &mut self,
        evidence: SupportEvidenceRecord,
    ) -> Result<bool, SupportEvidenceError> {
        self.insert_evidence_record_inner(evidence, false)
    }

    fn insert_evidence_record_inner(
        &mut self,
        evidence: SupportEvidenceRecord,
        require_declared_obligation: bool,
    ) -> Result<bool, SupportEvidenceError> {
        evidence
            .validate()
            .map_err(|source| SupportEvidenceError::InvalidEvidence {
                evidence_id: evidence.id(),
                source,
            })?;
        let evidence_id = evidence.id();
        let obligation_id = evidence.obligation_id();
        let obligation = evidence.obligation_record();
        let conclusion_digest = evidence.conclusion_digest();

        let evidence_is_new = match self.evidence.get(&evidence_id) {
            Some(existing) if existing == &evidence => false,
            Some(_) => return Err(SupportEvidenceError::EvidenceIdCollision { evidence_id }),
            None => true,
        };
        match self.obligations.get(&obligation_id) {
            Some(existing) if existing == &obligation => {
                if require_declared_obligation {
                    let cell =
                        self.require_cell(obligation.cell_id(), SupportReferenceKind::Evidence)?;
                    self.validate_obligation_scope(&obligation, cell)?;
                    self.validate_evidence_against_cell(&evidence, cell)?;
                    self.validate_cardinality_injectivity_extension(&evidence, cell)?;
                }
            }
            Some(_) => {
                return Err(SupportEvidenceError::ObligationIdCollision { obligation_id });
            }
            None if require_declared_obligation => {
                return Err(SupportEvidenceError::MissingObligationForEvidence {
                    evidence_id,
                    obligation_id,
                });
            }
            None if self.obligation_frontier_sealed => {
                return Err(SupportEvidenceError::ObligationFrontierSealed);
            }
            None => {}
        }
        if let Some(existing) = self.conclusion_by_obligation.get(&obligation_id) {
            if existing != &conclusion_digest {
                return Err(SupportEvidenceError::ContradictoryConclusion { obligation_id });
            }
        }
        if let Some(refinement_id) = self.refinement_by_parent.get(&obligation_id) {
            return Err(SupportEvidenceError::EvidenceForRefinedObligation {
                obligation_id,
                refinement_id: *refinement_id,
                evidence_id,
            });
        }
        if !evidence_is_new {
            return Ok(false);
        }
        self.require_catalog_open()?;

        if !require_declared_obligation {
            self.obligations.entry(obligation_id).or_insert(obligation);
        }
        self.conclusion_by_obligation
            .entry(obligation_id)
            .or_insert(conclusion_digest);
        self.evidence_by_obligation
            .entry(obligation_id)
            .or_default()
            .insert(evidence_id);
        self.evidence.insert(evidence_id, evidence);
        Ok(true)
    }

    /// Record a validated operational checkpoint for a known cell. Semantic
    /// catalog sealing deliberately does not freeze cursor progress because
    /// cursors are excluded from [`SupportEvidenceRoot`].
    pub(crate) fn insert_cursor(
        &mut self,
        cursor: SupportMaterializationCursor,
    ) -> Result<SupportCursorInsert, SupportEvidenceError> {
        let cell = self.require_cell(cursor.cell_id(), SupportReferenceKind::Cursor)?;
        cursor
            .validate_for(cell)
            .map_err(|source| SupportEvidenceError::InvalidCursor {
                cursor_id: cursor.id(),
                source,
            })?;
        let cursor_id = cursor.id();
        let cell_id = cursor.cell_id();
        let position = (cell_id, cursor.next_coordinate_ordinal());
        match self.cursor_records.get(&cursor_id) {
            Some(existing) if existing == &cursor => return Ok(SupportCursorInsert::Existing),
            Some(_) => return Err(SupportEvidenceError::CursorIdCollision { cursor_id }),
            None => {}
        }

        if let Some(existing_id) = self.cursor_by_position.get(&position) {
            if *existing_id != cursor_id {
                return Err(SupportEvidenceError::CursorCheckpointConflict {
                    cell_id,
                    coordinate_ordinal: cursor.next_coordinate_ordinal(),
                });
            }
        }
        let latest_ordinal = self
            .latest_cursor(cell_id)
            .map(SupportMaterializationCursor::next_coordinate_ordinal);

        let advances =
            latest_ordinal.map_or(true, |ordinal| cursor.next_coordinate_ordinal() > ordinal);
        self.cursor_by_position.insert(position, cursor_id);
        self.cursor_records.insert(cursor_id, cursor);
        if advances {
            self.latest_cursor_by_cell.insert(cell_id, cursor_id);
            Ok(SupportCursorInsert::InsertedAdvanced)
        } else {
            Ok(SupportCursorInsert::InsertedHistorical)
        }
    }

    /// Merge presentation-only retained examples using a deterministic
    /// content-ID top-k policy. Input order therefore cannot choose the sample.
    pub(crate) fn merge_retained_examples(
        &mut self,
        metadata: &RetainedSupportExamples,
    ) -> Result<SupportRetainedExampleInsert, SupportEvidenceError> {
        let cell_id = metadata.cell_id();
        if self.catalog_sealed && !self.cells.contains_key(&cell_id) {
            return Err(SupportEvidenceError::UnknownCell {
                cell_id,
                referenced_by: SupportReferenceKind::RetainedExample,
            });
        }
        let cap = metadata.cap();
        if let Some(existing) = self.retained_examples.get(&cell_id) {
            if existing.cap != cap {
                return Err(SupportEvidenceError::RetainedExampleCapConflict {
                    cell_id,
                    first_cap: existing.cap,
                    second_cap: cap,
                });
            }
        }
        let existing = self.retained_examples.get(&cell_id);
        let mut candidate = existing.cloned().unwrap_or_else(|| RetainedExampleState {
            cap,
            examples: BTreeSet::new(),
        });
        let mut changed = false;
        let mut replaced = false;
        for example in metadata.examples() {
            if example.cell_id() != cell_id {
                return Err(SupportEvidenceError::RetainedExampleCellMismatch {
                    cell_id,
                    example_cell_id: example.cell_id(),
                });
            }
            if candidate.examples.contains(&example) || cap == 0 {
                continue;
            }
            if candidate.examples.len() < cap {
                candidate.examples.insert(example);
                changed = true;
                continue;
            }
            let largest = candidate
                .examples
                .last()
                .copied()
                .expect("positive full cap has a largest example");
            if example < largest {
                candidate.examples.remove(&largest);
                candidate.examples.insert(example);
                changed = true;
                replaced = true;
            }
        }

        let state_is_new = existing.is_none();
        if state_is_new || changed {
            self.retained_examples.insert(cell_id, candidate);
            return Ok(if replaced {
                SupportRetainedExampleInsert::Replaced
            } else {
                SupportRetainedExampleInsert::Inserted
            });
        }
        if metadata
            .examples()
            .any(|example| !candidate.examples.contains(&example))
        {
            Ok(SupportRetainedExampleInsert::IgnoredByCap)
        } else {
            Ok(SupportRetainedExampleInsert::Existing)
        }
    }

    pub(crate) fn register_admission(
        &mut self,
        admission_id: AdmissionId,
        relation_id: RelationId,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.admissions,
            admission_id,
            relation_id,
            self.catalog_sealed,
            SupportEvidenceError::AdmissionLayerCollision { admission_id },
        )
    }

    pub(crate) fn register_question(
        &mut self,
        question_id: QuestionId,
        admission_id: AdmissionId,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.questions,
            question_id,
            admission_id,
            self.catalog_sealed,
            SupportEvidenceError::QuestionLayerCollision { question_id },
        )
    }

    pub(crate) fn register_mechanism_request(
        &mut self,
        request_id: MechanismRequestId,
        question_id: QuestionId,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.mechanism_requests,
            request_id,
            question_id,
            self.catalog_sealed,
            SupportEvidenceError::MechanismLayerCollision { request_id },
        )
    }

    pub(crate) fn register_choice(
        &mut self,
        choice_id: ChoiceId,
        question_id: QuestionId,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.choices,
            choice_id,
            question_id,
            self.catalog_sealed,
            SupportEvidenceError::ChoiceLayerCollision { choice_id },
        )
    }

    pub(crate) fn register_view(
        &mut self,
        view_id: ViewId,
        input: ViewInputId,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.views,
            view_id,
            input,
            self.catalog_sealed,
            SupportEvidenceError::ViewLayerCollision { view_id },
        )
    }

    pub(crate) fn register_observer(
        &mut self,
        observer_id: SupportObserverId,
        scope: SupportObserverLayerScope,
    ) -> Result<bool, SupportEvidenceError> {
        insert_scoped_registration(
            &mut self.observers,
            observer_id,
            scope,
            self.catalog_sealed,
            SupportEvidenceError::ObserverLayerCollision { observer_id },
        )
    }

    /// Seal semantic mutation after both exact frontiers have closed. Equal
    /// semantic repeats remain idempotent; presentation-only retained examples
    /// may still change without changing this closure fact or the root.
    pub(crate) fn seal_catalog(&mut self) -> Result<bool, SupportEvidenceError> {
        if self.catalog_sealed {
            return Ok(false);
        }
        let closure = self.validated_closure()?;
        if !closure.support_frontier_is_complete() {
            return Err(SupportEvidenceError::SupportFrontierOpen {
                open_leaves: closure.open_leaf_count(),
                roots_open: !closure.root_frontier_is_sealed(),
            });
        }
        if !closure.obligation_frontier_is_sealed() || closure.open_obligation_count() != 0 {
            return Err(SupportEvidenceError::ProofFrontierOpen {
                open_obligations: closure.open_obligation_count(),
                obligations_open: !closure.obligation_frontier_is_sealed(),
            });
        }
        drop(closure);
        self.catalog_sealed = true;
        Ok(true)
    }

    pub(crate) fn snapshot(&self) -> Result<SupportEvidenceSnapshot, SupportEvidenceError> {
        let validated = self.validate_state()?;
        let support_frontier_complete =
            self.root_frontier_sealed && validated.open_leaf_ids.is_empty();
        // Until catalog seal, another accepted refinement may supersede a
        // currently proved leaf and expose child leaves. Final proof closure
        // therefore requires both the obligation frontier and refinement
        // mutation to be sealed.
        let obligation_frontier_complete = self.catalog_sealed
            && self.obligation_frontier_sealed
            && validated.open_obligation_ids.is_empty();
        let evidence_kind_counts = evidence_kind_counts(self.evidence.values());
        let counts = self.counts(
            &validated,
            support_frontier_complete,
            obligation_frontier_complete,
            &evidence_kind_counts,
        );
        let root = support_evidence_root(self);
        let retained_examples = self
            .retained_examples
            .iter()
            .map(|(cell_id, state)| {
                (
                    *cell_id,
                    SupportRetainedExamplesSnapshot {
                        cell_id: *cell_id,
                        cap: state.cap,
                        examples: state.examples.iter().copied().collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let presentation_counts = presentation_counts(&retained_examples);

        Ok(SupportEvidenceSnapshot {
            version: SUPPORT_EVIDENCE_SNAPSHOT_VERSION,
            root,
            catalog_sealed: self.catalog_sealed,
            root_frontier_sealed: self.root_frontier_sealed,
            obligation_frontier_sealed: self.obligation_frontier_sealed,
            support_frontier_complete,
            obligation_frontier_complete,
            counts,
            presentation_counts,
            cells: self.cells.clone(),
            root_cells: self.root_cells.clone(),
            partitions: self.partitions.clone(),
            partition_by_parent: self.partition_by_parent.clone(),
            active_leaf_ids: validated.active_leaf_ids,
            open_leaf_ids: validated.open_leaf_ids,
            sealed_leaf_ids: validated.sealed_leaf_ids,
            obligations: self.obligations.clone(),
            obligation_refinements: self.obligation_refinements.clone(),
            refinement_by_parent: self.refinement_by_parent.clone(),
            root_obligation_ids: validated.root_obligation_ids,
            active_obligation_leaf_ids: validated.active_obligation_leaf_ids,
            superseded_obligation_ids: validated.superseded_obligation_ids,
            proved_obligation_ids: validated.proved_obligation_ids,
            open_obligation_ids: validated.open_obligation_ids,
            evidence: self.evidence.clone(),
            evidence_by_obligation: self.evidence_by_obligation.clone(),
            cursor_records: self.cursor_records.clone(),
            cursor_by_position: self.cursor_by_position.clone(),
            latest_cursor_by_cell: self.latest_cursor_by_cell.clone(),
            retained_examples,
            admissions: self.admissions.clone(),
            questions: self.questions.clone(),
            choices: self.choices.clone(),
            mechanism_requests: self.mechanism_requests.clone(),
            views: self.views.clone(),
            observers: self.observers.clone(),
        })
    }

    fn require_catalog_open(&self) -> Result<(), SupportEvidenceError> {
        if self.catalog_sealed {
            Err(SupportEvidenceError::CatalogSealed)
        } else {
            Ok(())
        }
    }
}

// Focused contract fixtures. They are intentionally narrow and are left for
// the integration phase rather than invoking the repository test pipeline
// while the relational Explore architecture is still being assembled.
#[cfg(test)]
mod tests {
    use super::super::relation::FindPolarity;
    use super::super::support_cell::SupportExpr;
    use super::*;

    fn producer() -> SupportProducerId {
        SupportProducerId::from_canonical_preimage(b"catalog-fixture-producer")
    }

    fn materializer() -> SupportMaterializerId {
        SupportMaterializerId::from_canonical_preimage(b"catalog-fixture-materializer")
    }

    fn interval(start: u128, end_exclusive: u128) -> SupportCell {
        SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer()),
            SupportExpr::ordinal_interval(start, end_exclusive).unwrap(),
            materializer(),
        )
        .unwrap()
    }

    fn partition_fixture() -> (
        SupportCell,
        SupportCell,
        SupportCell,
        SupportPartitionCertificate,
    ) {
        let parent = interval(0, 10);
        let left = interval(0, 4);
        let right = interval(4, 10);
        let partition = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![left.clone(), right.clone()],
        )
        .unwrap();
        (parent, left, right, partition)
    }

    #[test]
    fn canonical_root_is_independent_of_arrival_order() {
        let (parent, left, right, partition) = partition_fixture();
        let evidence = parent.structural_cardinality_evidence().unwrap().unwrap();
        let obligation_id = evidence.obligation().id();
        let cursor0 = SupportMaterializationCursor::at_start(&parent).unwrap();
        let cursor4 = cursor0
            .advance(&parent, 4, b"position-four".to_vec())
            .unwrap();

        let mut forward = SupportEvidenceCatalogBuilder::new();
        for cell in [parent.clone(), left.clone(), right.clone()] {
            forward.insert_cell(cell).unwrap();
        }
        forward.declare_root_cell(parent.id()).unwrap();
        forward.insert_partition(partition.clone()).unwrap();
        forward.seal_leaf(left.id()).unwrap();
        forward.seal_leaf(right.id()).unwrap();
        forward
            .insert_cardinality_evidence(evidence.clone())
            .unwrap();
        forward.declare_root_obligation(obligation_id).unwrap();
        forward.insert_cursor(cursor0.clone()).unwrap();
        forward.insert_cursor(cursor4.clone()).unwrap();
        forward.seal_root_frontier().unwrap();
        forward.seal_obligation_frontier().unwrap();
        forward.seal_catalog().unwrap();

        let mut reverse = SupportEvidenceCatalogBuilder::new();
        reverse.insert_cursor(cursor4).unwrap();
        reverse.insert_cursor(cursor0).unwrap();
        reverse.declare_root_obligation(obligation_id).unwrap();
        reverse.insert_cardinality_evidence(evidence).unwrap();
        reverse.seal_leaf(right.id()).unwrap();
        reverse.seal_leaf(left.id()).unwrap();
        reverse.insert_partition(partition).unwrap();
        reverse.declare_root_cell(parent.id()).unwrap();
        for cell in [right, left, parent] {
            reverse.insert_cell(cell).unwrap();
        }
        reverse.seal_obligation_frontier().unwrap();
        reverse.seal_root_frontier().unwrap();
        reverse.seal_catalog().unwrap();

        let forward = forward.snapshot().unwrap();
        let reverse = reverse.snapshot().unwrap();
        assert_eq!(forward.root(), reverse.root());
        assert_eq!(forward.counts(), reverse.counts());
        assert!(forward.support_frontier_is_complete());
        assert!(forward.obligation_frontier_is_complete());
    }

    #[test]
    fn snapshot_rejects_partition_cycles() {
        let parent = interval(0, 10);
        let self_partition =
            SupportPartitionCertificate::ordinal_interval_cover(&parent, vec![parent.clone()])
                .unwrap();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(parent.clone()).unwrap();
        catalog.declare_root_cell(parent.id()).unwrap();
        catalog.insert_partition(self_partition).unwrap();

        assert!(matches!(
            catalog.snapshot(),
            Err(SupportEvidenceError::PartitionCycle { .. })
        ));
    }

    #[test]
    fn one_parent_cannot_have_incompatible_replacements() {
        let parent = interval(0, 10);
        let first = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![interval(0, 4), interval(4, 10)],
        )
        .unwrap();
        let second = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![interval(0, 5), interval(5, 10)],
        )
        .unwrap();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_partition(first).unwrap();
        assert!(matches!(
            catalog.insert_partition(second),
            Err(SupportEvidenceError::IncompatibleParentReplacement { .. })
        ));
    }

    #[test]
    fn sealed_leaf_and_open_proof_obligation_are_distinct() {
        let cell = interval(0, 10);
        let obligation = SupportCellObligation::new(&cell, ExactCardinalityClaim).unwrap();
        let obligation_id = obligation.id();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(cell.clone()).unwrap();
        catalog.declare_root_cell(cell.id()).unwrap();
        catalog.seal_leaf(cell.id()).unwrap();
        catalog
            .insert_obligation(SupportObligationRecord::Cardinality(obligation))
            .unwrap();
        catalog.declare_root_obligation(obligation_id).unwrap();
        catalog.seal_root_frontier().unwrap();
        catalog.seal_obligation_frontier().unwrap();

        let snapshot = catalog.snapshot().unwrap();
        assert!(snapshot.support_frontier_is_complete());
        assert!(!snapshot.obligation_frontier_is_complete());
        assert_eq!(
            snapshot.sealed_leaf_ids().collect::<Vec<_>>(),
            vec![cell.id()]
        );
        assert_eq!(
            snapshot.open_obligation_ids().collect::<Vec<_>>(),
            vec![obligation_id]
        );
    }

    #[test]
    fn retained_examples_do_not_change_the_semantic_root() {
        let cell = interval(0, 10);
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(cell.clone()).unwrap();
        catalog.declare_root_cell(cell.id()).unwrap();
        catalog.seal_leaf(cell.id()).unwrap();
        catalog.seal_root_frontier().unwrap();
        catalog.seal_obligation_frontier().unwrap();
        catalog.seal_catalog().unwrap();
        let before = catalog.snapshot().unwrap();

        let mut examples = RetainedSupportExamples::new(&cell, 2).unwrap();
        examples
            .retain(SupportExampleId::from_canonical_example_digest(
                cell.id(),
                [0x11; 32],
            ))
            .unwrap();
        catalog.merge_retained_examples(&examples).unwrap();
        let after = catalog.snapshot().unwrap();

        assert_eq!(before.root(), after.root());
        assert_ne!(before.presentation_counts(), after.presentation_counts());
        assert!(after.catalog_is_sealed());
    }

    #[test]
    fn historical_cursor_positions_reject_conflicting_checkpoints() {
        let cell = interval(0, 10);
        let start = SupportMaterializationCursor::at_start(&cell).unwrap();
        let first_at_four = start.advance(&cell, 4, b"first".to_vec()).unwrap();
        let later = first_at_four.advance(&cell, 8, b"later".to_vec()).unwrap();
        let conflicting_at_four = start.advance(&cell, 4, b"conflicting".to_vec()).unwrap();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(cell.clone()).unwrap();
        catalog.insert_cursor(first_at_four).unwrap();
        catalog.insert_cursor(later).unwrap();

        assert!(matches!(
            catalog.insert_cursor(conflicting_at_four),
            Err(SupportEvidenceError::CursorCheckpointConflict {
                cell_id,
                coordinate_ordinal: 4,
            }) if cell_id == cell.id()
        ));
    }

    #[test]
    fn materialization_progress_does_not_change_the_evidence_root() {
        let cell = interval(0, 10);
        let start = SupportMaterializationCursor::at_start(&cell).unwrap();
        let advanced = start.advance(&cell, 4, b"position-four".to_vec()).unwrap();

        let mut early = SupportEvidenceCatalogBuilder::new();
        early.insert_cell(cell.clone()).unwrap();
        early.declare_root_cell(cell.id()).unwrap();
        early.seal_leaf(cell.id()).unwrap();
        early.insert_cursor(start).unwrap();
        early.seal_root_frontier().unwrap();
        early.seal_obligation_frontier().unwrap();
        early.seal_catalog().unwrap();

        let mut later = SupportEvidenceCatalogBuilder::new();
        later.insert_cell(cell.clone()).unwrap();
        later.declare_root_cell(cell.id()).unwrap();
        later.seal_leaf(cell.id()).unwrap();
        later.insert_cursor(advanced).unwrap();
        later.seal_root_frontier().unwrap();
        later.seal_obligation_frontier().unwrap();
        later.seal_catalog().unwrap();

        let early = early.snapshot().unwrap();
        let later = later.snapshot().unwrap();
        assert_eq!(early.root(), later.root());
        assert_eq!(
            early
                .latest_cursor(cell.id())
                .unwrap()
                .next_coordinate_ordinal(),
            0
        );
        assert_eq!(
            later
                .latest_cursor(cell.id())
                .unwrap()
                .next_coordinate_ordinal(),
            4
        );
    }

    #[test]
    fn layer_registration_must_match_the_case_relation() {
        let actual_relation = RelationId::from_canonical_semantic_preimage(b"actual-case-relation");
        let other_relation = RelationId::from_canonical_semantic_preimage(b"other-case-relation");
        let cell = SupportCell::new(
            SupportCellSpace::ExtensionalValues(SupportExtensionalTarget::Cases(actual_relation)),
            SupportExpr::ordinal_interval(0, 10).unwrap(),
            materializer(),
        )
        .unwrap();
        let admission =
            AdmissionId::from_canonical_admission_preimage(other_relation, b"admission");
        let question = QuestionId::from_canonical_find_preimage(
            admission,
            b"selection",
            FindPolarity::Matches,
        );
        let obligation =
            SupportCellObligation::new(&cell, SelectionClassificationClaim::new(question)).unwrap();
        let obligation_id = obligation.id();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(cell.clone()).unwrap();
        catalog
            .register_admission(admission, other_relation)
            .unwrap();
        catalog.register_question(question, admission).unwrap();
        catalog
            .insert_obligation(SupportObligationRecord::Selection(obligation))
            .unwrap();
        catalog.declare_root_obligation(obligation_id).unwrap();

        assert!(matches!(
            catalog.snapshot(),
            Err(SupportEvidenceError::LayerCellRelationMismatch {
                cell_id,
                expected_relation_id,
                ..
            }) if cell_id == cell.id() && expected_relation_id == other_relation
        ));
    }

    #[test]
    fn typed_refinement_supersedes_parent_and_closes_through_child_leaves() {
        let (parent, left, right, partition) = partition_fixture();
        let parent_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&parent, ExactCardinalityClaim).unwrap(),
        );
        let left_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&left, ExactCardinalityClaim).unwrap(),
        );
        let right_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&right, ExactCardinalityClaim).unwrap(),
        );
        let refinement = SupportObligationRefinement::new(
            &parent_obligation,
            &partition,
            [&left_obligation, &right_obligation],
        )
        .unwrap();
        let parent_id = parent_obligation.id();
        let child_ids = [left_obligation.id(), right_obligation.id()]
            .into_iter()
            .collect::<BTreeSet<_>>();

        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.declare_root_obligation(parent_id).unwrap();
        // The link may arrive before the records it references.
        catalog.insert_obligation_refinement(refinement).unwrap();
        catalog.insert_partition(partition).unwrap();
        catalog.insert_obligation(parent_obligation).unwrap();
        for cell in [right.clone(), parent.clone(), left.clone()] {
            catalog.insert_cell(cell).unwrap();
        }
        catalog.declare_root_cell(parent.id()).unwrap();
        catalog.seal_leaf(left.id()).unwrap();
        catalog.seal_leaf(right.id()).unwrap();
        // The refined parent remains an auditable open obligation; accepted
        // evidence belongs on the active child leaves only.
        catalog
            .insert_cardinality_evidence(left.structural_cardinality_evidence().unwrap().unwrap())
            .unwrap();
        catalog
            .insert_cardinality_evidence(right.structural_cardinality_evidence().unwrap().unwrap())
            .unwrap();
        catalog.seal_root_frontier().unwrap();
        catalog.seal_obligation_frontier().unwrap();
        catalog.seal_catalog().unwrap();

        let snapshot = catalog.snapshot().unwrap();
        assert_eq!(
            snapshot.root_obligation_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([parent_id])
        );
        assert_eq!(
            snapshot
                .active_obligation_leaf_ids()
                .collect::<BTreeSet<_>>(),
            child_ids
        );
        assert_eq!(
            snapshot
                .superseded_obligation_ids()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([parent_id])
        );
        assert!(!snapshot.proved_obligation_ids().any(|id| id == parent_id));
        assert_eq!(snapshot.proved_obligation_ids().count(), 2);
        assert_eq!(snapshot.open_obligation_ids().count(), 0);
        assert!(snapshot.obligation_frontier_is_complete());
        assert_eq!(
            snapshot.counts().superseded_obligations(),
            SupportEvidenceCount::Exact(1)
        );
        assert_eq!(
            snapshot.counts().proved_obligations(),
            SupportEvidenceCount::Exact(2)
        );
    }

    #[test]
    fn refinement_requires_one_same_claim_child_per_partition_cell() {
        let (parent, left, right, partition) = partition_fixture();
        let parent_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&parent, ExactCardinalityClaim).unwrap(),
        );
        let wrong_claim = SupportObligationRecord::Injectivity(
            SupportCellObligation::new(&left, InjectiveMappingClaim::new(left.materializer_id()))
                .unwrap(),
        );
        let right_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&right, ExactCardinalityClaim).unwrap(),
        );
        assert!(matches!(
            SupportObligationRefinement::new(
                &parent_obligation,
                &partition,
                [&wrong_claim, &right_obligation],
            ),
            Err(SupportEvidenceError::RefinementClaimMismatch { .. })
        ));
    }

    #[test]
    fn direct_parent_evidence_and_refinement_are_mutually_exclusive() {
        let (parent, left, right, partition) = partition_fixture();
        let parent_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&parent, ExactCardinalityClaim).unwrap(),
        );
        let left_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&left, ExactCardinalityClaim).unwrap(),
        );
        let right_obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&right, ExactCardinalityClaim).unwrap(),
        );
        let refinement = SupportObligationRefinement::new(
            &parent_obligation,
            &partition,
            [&left_obligation, &right_obligation],
        )
        .unwrap();
        let direct = parent.structural_cardinality_evidence().unwrap().unwrap();
        let parent_id = parent_obligation.id();
        let refinement_id = refinement.id();
        let evidence_id = direct.id();

        let mut evidence_first = SupportEvidenceCatalogBuilder::new();
        evidence_first
            .insert_cardinality_evidence(direct.clone())
            .unwrap();
        assert!(matches!(
            evidence_first.insert_obligation_refinement(refinement.clone()),
            Err(SupportEvidenceError::RefinementForProvedObligation {
                parent_obligation_id,
                refinement_id: id,
            }) if parent_obligation_id == parent_id && id == refinement_id
        ));

        let mut refinement_first = SupportEvidenceCatalogBuilder::new();
        refinement_first
            .insert_obligation_refinement(refinement)
            .unwrap();
        assert!(matches!(
            refinement_first.insert_cardinality_evidence(direct),
            Err(SupportEvidenceError::EvidenceForRefinedObligation {
                obligation_id,
                refinement_id: id,
                evidence_id: actual_evidence_id,
            }) if obligation_id == parent_id
                && id == refinement_id
                && actual_evidence_id == evidence_id
        ));
    }

    #[test]
    fn snapshot_rejects_an_obligation_not_reachable_from_a_declared_root() {
        let cell = interval(0, 10);
        let obligation = SupportCellObligation::new(&cell, ExactCardinalityClaim).unwrap();
        let obligation_id = obligation.id();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_cell(cell).unwrap();
        catalog
            .insert_obligation(SupportObligationRecord::Cardinality(obligation))
            .unwrap();

        assert!(matches!(
            catalog.snapshot(),
            Err(SupportEvidenceError::UnreachableObligation { obligation_id: id })
                if id == obligation_id
        ));
    }

    #[test]
    fn refinement_graph_rejects_cycles_and_multiple_parent_replacements() {
        let parent = interval(0, 10);
        let obligation = SupportObligationRecord::Cardinality(
            SupportCellObligation::new(&parent, ExactCardinalityClaim).unwrap(),
        );
        let self_partition =
            SupportPartitionCertificate::ordinal_interval_cover(&parent, vec![parent.clone()])
                .unwrap();
        let self_refinement =
            SupportObligationRefinement::new(&obligation, &self_partition, [&obligation]).unwrap();
        let refinements = BTreeMap::from([(self_refinement.id(), self_refinement.clone())]);
        let by_parent = BTreeMap::from([(obligation.id(), self_refinement.id())]);
        assert!(matches!(
            validate_obligation_refinement_acyclic(&by_parent, &refinements),
            Err(SupportEvidenceError::ObligationRefinementCycle { .. })
        ));

        let first_partition = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![interval(0, 4), interval(4, 10)],
        )
        .unwrap();
        let first_children = first_partition
            .child_ids()
            .iter()
            .map(|cell_id| {
                let cell = if *cell_id == interval(0, 4).id() {
                    interval(0, 4)
                } else {
                    interval(4, 10)
                };
                SupportObligationRecord::Cardinality(
                    SupportCellObligation::new(&cell, ExactCardinalityClaim).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let second_partition = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![interval(0, 5), interval(5, 10)],
        )
        .unwrap();
        let second_children = second_partition
            .child_ids()
            .iter()
            .map(|cell_id| {
                let cell = if *cell_id == interval(0, 5).id() {
                    interval(0, 5)
                } else {
                    interval(5, 10)
                };
                SupportObligationRecord::Cardinality(
                    SupportCellObligation::new(&cell, ExactCardinalityClaim).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let first =
            SupportObligationRefinement::new(&obligation, &first_partition, first_children.iter())
                .unwrap();
        let second = SupportObligationRefinement::new(
            &obligation,
            &second_partition,
            second_children.iter(),
        )
        .unwrap();
        let mut catalog = SupportEvidenceCatalogBuilder::new();
        catalog.insert_obligation_refinement(first).unwrap();
        assert!(matches!(
            catalog.insert_obligation_refinement(second),
            Err(SupportEvidenceError::IncompatibleObligationRefinement { .. })
        ));
    }
}

fn insert_scoped_registration<K, V>(
    registrations: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    catalog_sealed: bool,
    collision: SupportEvidenceError,
) -> Result<bool, SupportEvidenceError>
where
    K: Copy + Ord,
    V: Copy + Eq,
{
    match registrations.get(&key) {
        Some(existing) if *existing == value => return Ok(false),
        Some(_) => return Err(collision),
        None => {}
    }
    if catalog_sealed {
        return Err(SupportEvidenceError::CatalogSealed);
    }
    registrations.insert(key, value);
    Ok(true)
}

fn expression_input_cell_ids(expression: &SupportExpr) -> BTreeSet<SupportCellId> {
    let mut inputs = BTreeSet::new();
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression.kind() {
            SupportExprKind::JoinReference {
                inputs: referenced, ..
            } => inputs.extend(referenced.iter().copied()),
            SupportExprKind::Product(children)
            | SupportExprKind::ProductRankInterval {
                factors: children, ..
            }
            | SupportExprKind::Union(children) => {
                pending.extend(children.iter());
            }
            SupportExprKind::Difference {
                minuend,
                subtrahend,
            } => {
                pending.push(subtrahend);
                pending.push(minuend);
            }
            SupportExprKind::Singleton(_)
            | SupportExprKind::FiniteEnum(_)
            | SupportExprKind::OrdinalInterval { .. }
            | SupportExprKind::OrdinalCongruence { .. } => {}
        }
    }
    inputs
}

/// Validate the compact producer DAG independently of the support-partition
/// tree. A root may legitimately depend on non-root factor/fiber cells, but a
/// content root must never authenticate a dangling or cyclic join recipe.
fn validate_expression_dependency_graph(
    cells: &BTreeMap<SupportCellId, SupportCell>,
) -> Result<(), SupportEvidenceError> {
    let mut dependencies = BTreeMap::<SupportCellId, BTreeSet<SupportCellId>>::new();
    for (cell_id, cell) in cells {
        let inputs = expression_input_cell_ids(cell.expression());
        if let Some(input_cell_id) = inputs
            .iter()
            .find(|input_cell_id| !cells.contains_key(input_cell_id))
            .copied()
        {
            return Err(SupportEvidenceError::UnknownExpressionInput {
                cell_id: *cell_id,
                input_cell_id,
            });
        }
        dependencies.insert(*cell_id, inputs);
    }

    let mut visiting = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for root_id in cells.keys().copied() {
        if complete.contains(&root_id) {
            continue;
        }
        let mut pending = vec![(root_id, false)];
        while let Some((cell_id, exiting)) = pending.pop() {
            if exiting {
                visiting.remove(&cell_id);
                complete.insert(cell_id);
                continue;
            }
            if complete.contains(&cell_id) {
                continue;
            }
            if !visiting.insert(cell_id) {
                return Err(SupportEvidenceError::ExpressionDependencyCycle { cell_id });
            }
            pending.push((cell_id, true));
            for input_cell_id in dependencies[&cell_id].iter().rev().copied() {
                if visiting.contains(&input_cell_id) {
                    return Err(SupportEvidenceError::ExpressionDependencyCycle {
                        cell_id: input_cell_id,
                    });
                }
                if !complete.contains(&input_cell_id) {
                    pending.push((input_cell_id, false));
                }
            }
        }
    }
    Ok(())
}

fn ensure_partition_extension_is_acyclic(
    parent_id: SupportCellId,
    child_ids: &[SupportCellId],
    partition_by_parent: &BTreeMap<SupportCellId, SupportPartitionId>,
    partitions: &BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
) -> Result<(), SupportEvidenceError> {
    let mut visited = BTreeSet::new();
    let mut pending = child_ids.iter().rev().copied().collect::<Vec<_>>();
    while let Some(cell_id) = pending.pop() {
        if cell_id == parent_id {
            return Err(SupportEvidenceError::PartitionCycle { cell_id });
        }
        if !visited.insert(cell_id) {
            continue;
        }
        let Some(partition_id) = partition_by_parent.get(&cell_id) else {
            continue;
        };
        let partition = partitions.get(partition_id).ok_or(
            SupportEvidenceError::PartitionParentIndexMismatch {
                parent_id: cell_id,
                partition_id: *partition_id,
            },
        )?;
        pending.extend(partition.child_ids().iter().rev().copied());
    }
    Ok(())
}

fn validate_partition_acyclic(
    partition_by_parent: &BTreeMap<SupportCellId, SupportPartitionId>,
    partitions: &BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
) -> Result<(), SupportEvidenceError> {
    // 1 = active DFS path, 2 = completely expanded. The explicit stack keeps
    // a deeply refined exact support from becoming a Rust call-stack limit.
    let mut color = BTreeMap::<SupportCellId, u8>::new();
    for start in partition_by_parent.keys().copied() {
        if color.get(&start) == Some(&2) {
            continue;
        }
        let mut stack = vec![(start, false)];
        while let Some((cell_id, exiting)) = stack.pop() {
            if exiting {
                color.insert(cell_id, 2);
                continue;
            }
            match color.get(&cell_id).copied() {
                Some(1) => return Err(SupportEvidenceError::PartitionCycle { cell_id }),
                Some(2) => continue,
                _ => {}
            }
            color.insert(cell_id, 1);
            stack.push((cell_id, true));

            let Some(partition_id) = partition_by_parent.get(&cell_id) else {
                continue;
            };
            let partition = partitions.get(partition_id).ok_or(
                SupportEvidenceError::PartitionParentIndexMismatch {
                    parent_id: cell_id,
                    partition_id: *partition_id,
                },
            )?;
            for child_id in partition.child_ids().iter().rev().copied() {
                if color.get(&child_id) == Some(&1) {
                    return Err(SupportEvidenceError::PartitionCycle { cell_id: child_id });
                }
                if partition_by_parent.contains_key(&child_id) && color.get(&child_id) != Some(&2) {
                    stack.push((child_id, false));
                }
            }
        }
    }
    Ok(())
}

fn active_leaves(
    roots: &BTreeSet<SupportCellId>,
    partition_by_parent: &BTreeMap<SupportCellId, SupportPartitionId>,
    partitions: &BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
) -> Result<BTreeSet<SupportCellId>, SupportEvidenceError> {
    validate_partition_acyclic(partition_by_parent, partitions)?;
    let mut leaves = BTreeSet::new();
    let mut expanded = BTreeSet::new();
    let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
    while let Some(cell_id) = pending.pop() {
        if !expanded.insert(cell_id) {
            continue;
        }
        let Some(partition_id) = partition_by_parent.get(&cell_id) else {
            leaves.insert(cell_id);
            continue;
        };
        let partition = partitions.get(partition_id).ok_or(
            SupportEvidenceError::PartitionParentIndexMismatch {
                parent_id: cell_id,
                partition_id: *partition_id,
            },
        )?;
        pending.extend(partition.child_ids().iter().rev().copied());
    }
    Ok(leaves)
}

fn ensure_obligation_refinement_extension_is_acyclic(
    parent_obligation_id: SupportProofObligationId,
    child_obligation_ids: &[SupportProofObligationId],
    refinement_by_parent: &BTreeMap<SupportProofObligationId, SupportObligationRefinementId>,
    refinements: &BTreeMap<SupportObligationRefinementId, SupportObligationRefinement>,
) -> Result<(), SupportEvidenceError> {
    let mut visited = BTreeSet::new();
    let mut pending = child_obligation_ids
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    while let Some(obligation_id) = pending.pop() {
        if obligation_id == parent_obligation_id {
            return Err(SupportEvidenceError::ObligationRefinementCycle { obligation_id });
        }
        if !visited.insert(obligation_id) {
            continue;
        }
        let Some(refinement_id) = refinement_by_parent.get(&obligation_id) else {
            continue;
        };
        let refinement = refinements
            .get(refinement_id)
            .ok_or(SupportEvidenceError::RefinementIndexMismatch)?;
        pending.extend(refinement.child_obligation_ids().iter().rev().copied());
    }
    Ok(())
}

fn validate_obligation_refinement_acyclic(
    refinement_by_parent: &BTreeMap<SupportProofObligationId, SupportObligationRefinementId>,
    refinements: &BTreeMap<SupportObligationRefinementId, SupportObligationRefinement>,
) -> Result<(), SupportEvidenceError> {
    let mut color = BTreeMap::<SupportProofObligationId, u8>::new();
    for start in refinement_by_parent.keys().copied() {
        if color.get(&start) == Some(&2) {
            continue;
        }
        let mut stack = vec![(start, false)];
        while let Some((obligation_id, exiting)) = stack.pop() {
            if exiting {
                color.insert(obligation_id, 2);
                continue;
            }
            match color.get(&obligation_id).copied() {
                Some(1) => {
                    return Err(SupportEvidenceError::ObligationRefinementCycle { obligation_id });
                }
                Some(2) => continue,
                _ => {}
            }
            color.insert(obligation_id, 1);
            stack.push((obligation_id, true));

            let Some(refinement_id) = refinement_by_parent.get(&obligation_id) else {
                continue;
            };
            let refinement = refinements
                .get(refinement_id)
                .ok_or(SupportEvidenceError::RefinementIndexMismatch)?;
            for child_id in refinement.child_obligation_ids().iter().rev().copied() {
                if color.get(&child_id) == Some(&1) {
                    return Err(SupportEvidenceError::ObligationRefinementCycle {
                        obligation_id: child_id,
                    });
                }
                if refinement_by_parent.contains_key(&child_id) && color.get(&child_id) != Some(&2)
                {
                    stack.push((child_id, false));
                }
            }
        }
    }
    Ok(())
}

struct ActiveObligationFrontier {
    root_ids: BTreeSet<SupportProofObligationId>,
    active_leaf_ids: BTreeSet<SupportProofObligationId>,
    superseded_ids: BTreeSet<SupportProofObligationId>,
    proved_leaf_ids: BTreeSet<SupportProofObligationId>,
    open_leaf_ids: BTreeSet<SupportProofObligationId>,
}

fn active_obligation_frontier(
    obligations: &BTreeMap<SupportProofObligationId, SupportObligationRecord>,
    declared_root_ids: &BTreeSet<SupportProofObligationId>,
    refinement_by_parent: &BTreeMap<SupportProofObligationId, SupportObligationRefinementId>,
    refinements: &BTreeMap<SupportObligationRefinementId, SupportObligationRefinement>,
    conclusion_by_obligation: &BTreeMap<SupportProofObligationId, [u8; 32]>,
) -> Result<ActiveObligationFrontier, SupportEvidenceError> {
    validate_obligation_refinement_acyclic(refinement_by_parent, refinements)?;
    let child_ids = refinements
        .values()
        .flat_map(|refinement| refinement.child_obligation_ids().iter().copied())
        .collect::<BTreeSet<_>>();
    for obligation_id in declared_root_ids {
        if !obligations.contains_key(obligation_id) {
            return Err(SupportEvidenceError::UnknownRootObligation {
                obligation_id: *obligation_id,
            });
        }
        if child_ids.contains(obligation_id) {
            return Err(SupportEvidenceError::RootObligationIsRefinementChild {
                obligation_id: *obligation_id,
            });
        }
    }
    let root_ids = declared_root_ids.clone();
    let superseded_ids = refinement_by_parent
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    let mut active_leaf_ids = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = root_ids.iter().rev().copied().collect::<Vec<_>>();
    while let Some(obligation_id) = pending.pop() {
        if !visited.insert(obligation_id) {
            continue;
        }
        let Some(refinement_id) = refinement_by_parent.get(&obligation_id) else {
            active_leaf_ids.insert(obligation_id);
            continue;
        };
        let refinement = refinements
            .get(refinement_id)
            .ok_or(SupportEvidenceError::RefinementIndexMismatch)?;
        pending.extend(refinement.child_obligation_ids().iter().rev().copied());
    }
    if let Some(obligation_id) = obligations
        .keys()
        .find(|obligation_id| !visited.contains(obligation_id))
        .copied()
    {
        return Err(SupportEvidenceError::UnreachableObligation { obligation_id });
    }

    let proved_leaf_ids = active_leaf_ids
        .iter()
        .filter(|obligation_id| conclusion_by_obligation.contains_key(obligation_id))
        .copied()
        .collect::<BTreeSet<_>>();
    let open_leaf_ids = active_leaf_ids
        .difference(&proved_leaf_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    Ok(ActiveObligationFrontier {
        root_ids,
        active_leaf_ids,
        superseded_ids,
        proved_leaf_ids,
        open_leaf_ids,
    })
}

fn require_case_relation(
    cell: &SupportCell,
    expected_relation_id: RelationId,
    obligation_id: SupportProofObligationId,
) -> Result<(), SupportEvidenceError> {
    if cell_case_relation(cell) == Some(expected_relation_id) {
        Ok(())
    } else {
        Err(SupportEvidenceError::LayerCellRelationMismatch {
            obligation_id,
            cell_id: cell.id(),
            expected_relation_id,
        })
    }
}

fn cell_case_relation(cell: &SupportCell) -> Option<RelationId> {
    match cell.space() {
        SupportCellSpace::ExtensionalValues(SupportExtensionalTarget::Cases(relation_id))
        | SupportCellSpace::MappedImage {
            target: SupportExtensionalTarget::Cases(relation_id),
            ..
        } => Some(relation_id),
        _ => None,
    }
}

fn cell_relation(cell: &SupportCell) -> Option<RelationId> {
    let target = match cell.space() {
        SupportCellSpace::ExtensionalValues(target)
        | SupportCellSpace::MappedImage { target, .. } => target,
        SupportCellSpace::ProducerCoordinates(_) => return None,
    };
    match target {
        SupportExtensionalTarget::SourceRows(relation_id)
        | SupportExtensionalTarget::SuccessorRows(relation_id)
        | SupportExtensionalTarget::Cases(relation_id) => Some(relation_id),
        SupportExtensionalTarget::Derived(_) => None,
    }
}

fn cell_producer(cell: &SupportCell) -> Option<SupportProducerId> {
    match cell.space() {
        SupportCellSpace::ProducerCoordinates(producer_id)
        | SupportCellSpace::MappedImage { producer_id, .. }
        | SupportCellSpace::ExtensionalValues(SupportExtensionalTarget::Derived(producer_id)) => {
            Some(producer_id)
        }
        _ => None,
    }
}

fn observer_scope_matches_cell(
    catalog: &SupportEvidenceCatalogBuilder,
    scope: SupportObserverLayerScope,
    cell: &SupportCell,
) -> bool {
    match scope {
        SupportObserverLayerScope::Relation(relation_id) => {
            cell_relation(cell) == Some(relation_id)
        }
        SupportObserverLayerScope::Question(question_id) => catalog
            .relation_for_question(question_id)
            .is_some_and(|relation_id| cell_case_relation(cell) == Some(relation_id)),
        SupportObserverLayerScope::MechanismRequest(request_id) => catalog
            .relation_for_mechanism(request_id)
            .is_some_and(|relation_id| cell_case_relation(cell) == Some(relation_id)),
        SupportObserverLayerScope::View(view_id) => catalog
            .relation_for_view(view_id)
            .is_some_and(|relation_id| cell_case_relation(cell) == Some(relation_id)),
        SupportObserverLayerScope::Producer(producer_id) => {
            cell_producer(cell) == Some(producer_id)
        }
        SupportObserverLayerScope::ExactCell(cell_id) => cell.id() == cell_id,
    }
}

fn count_evidence(count: usize, exact: bool) -> SupportEvidenceCount {
    let observed = count as u128;
    if exact {
        SupportEvidenceCount::Exact(observed)
    } else {
        SupportEvidenceCount::Open { observed }
    }
}

fn kind_count(counts: &BTreeMap<SupportEvidenceKind, usize>, kind: SupportEvidenceKind) -> usize {
    counts.get(&kind).copied().unwrap_or(0)
}

fn evidence_kind_counts<'a>(
    evidence: impl Iterator<Item = &'a SupportEvidenceRecord>,
) -> BTreeMap<SupportEvidenceKind, usize> {
    let mut counts = BTreeMap::new();
    for record in evidence {
        *counts.entry(record.kind()).or_insert(0) += 1;
    }
    counts
}

fn presentation_counts(
    retained: &BTreeMap<SupportCellId, SupportRetainedExamplesSnapshot>,
) -> SupportPresentationCounts {
    SupportPresentationCounts {
        cells_with_examples: retained
            .values()
            .filter(|examples| !examples.examples().is_empty())
            .count() as u128,
        retained_examples: retained
            .values()
            .map(|examples| examples.examples().len() as u128)
            .sum(),
    }
}

fn derive_obligation_refinement_id(
    parent_obligation_id: SupportProofObligationId,
    partition_id: SupportPartitionId,
    child_obligation_ids: &[SupportProofObligationId],
) -> SupportObligationRefinementId {
    let mut hasher = SupportEvidenceHasher::new(SUPPORT_OBLIGATION_REFINEMENT_HASH_V1);
    hasher.digest(parent_obligation_id.bytes());
    hasher.digest(partition_id.bytes());
    hasher.len(child_obligation_ids.len());
    for child_obligation_id in child_obligation_ids {
        hasher.digest(child_obligation_id.bytes());
    }
    SupportObligationRefinementId(hasher.finish())
}

fn support_evidence_root(catalog: &SupportEvidenceCatalogBuilder) -> SupportEvidenceRoot {
    let mut hasher = SupportEvidenceHasher::new(SUPPORT_EVIDENCE_ROOT_HASH_V3);
    hasher.u32(SUPPORT_EVIDENCE_SNAPSHOT_VERSION);
    hasher.boolean(catalog.catalog_sealed);
    hasher.boolean(catalog.root_frontier_sealed);
    hasher.boolean(catalog.obligation_frontier_sealed);

    hasher.tag(0x01);
    hasher.len(catalog.cells.len());
    for cell_id in catalog.cells.keys() {
        hasher.digest(cell_id.bytes());
    }

    hasher.tag(0x02);
    hasher.len(catalog.root_cells.len());
    for cell_id in &catalog.root_cells {
        hasher.digest(cell_id.bytes());
    }

    hasher.tag(0x03);
    hasher.len(catalog.partitions.len());
    for (partition_id, partition) in &catalog.partitions {
        hasher.digest(partition_id.bytes());
        hasher.digest(partition.parent_id().bytes());
        hasher.len(partition.child_ids().len());
        for child_id in partition.child_ids() {
            hasher.digest(child_id.bytes());
        }
    }

    hasher.tag(0x04);
    hasher.len(catalog.sealed_leaf_claims.len());
    for cell_id in &catalog.sealed_leaf_claims {
        hasher.digest(cell_id.bytes());
    }

    hasher.tag(0x05);
    hasher.len(catalog.obligations.len());
    for (obligation_id, obligation) in &catalog.obligations {
        hasher.tag(obligation.kind().canonical_tag());
        hasher.digest(obligation_id.bytes());
        hasher.digest(obligation.cell_id().bytes());
    }

    hasher.tag(0x06);
    hasher.len(catalog.root_obligations.len());
    for obligation_id in &catalog.root_obligations {
        hasher.digest(obligation_id.bytes());
    }

    hasher.tag(0x07);
    hasher.len(catalog.obligation_refinements.len());
    for (refinement_id, refinement) in &catalog.obligation_refinements {
        hasher.digest(refinement_id.bytes());
        hasher.digest(refinement.parent_obligation_id().bytes());
        hasher.digest(refinement.partition_id().bytes());
        hasher.len(refinement.child_obligation_ids().len());
        for child_obligation_id in refinement.child_obligation_ids() {
            hasher.digest(child_obligation_id.bytes());
        }
    }

    hasher.tag(0x08);
    hasher.len(catalog.evidence.len());
    for (evidence_id, evidence) in &catalog.evidence {
        hasher.tag(evidence.kind().canonical_tag());
        hasher.digest(evidence_id.bytes());
        hasher.digest(evidence.obligation_id().bytes());
        hasher.digest(evidence.conclusion_digest());
    }

    hasher.tag(0x09);
    hasher.len(catalog.admissions.len());
    for (admission_id, relation_id) in &catalog.admissions {
        hasher.digest(admission_id.bytes());
        hasher.digest(relation_id.bytes());
    }

    hasher.tag(0x0a);
    hasher.len(catalog.questions.len());
    for (question_id, admission_id) in &catalog.questions {
        hasher.digest(question_id.bytes());
        hasher.digest(admission_id.bytes());
    }

    hasher.tag(0x0b);
    hasher.len(catalog.choices.len());
    for (choice_id, question_id) in &catalog.choices {
        hasher.digest(choice_id.bytes());
        hasher.digest(question_id.bytes());
    }

    hasher.tag(0x0c);
    hasher.len(catalog.mechanism_requests.len());
    for (request_id, question_id) in &catalog.mechanism_requests {
        hasher.digest(request_id.bytes());
        hasher.digest(question_id.bytes());
    }

    hasher.tag(0x0d);
    hasher.len(catalog.views.len());
    for (view_id, input) in &catalog.views {
        hasher.digest(view_id.bytes());
        hash_view_input(&mut hasher, *input);
    }

    hasher.tag(0x0e);
    hasher.len(catalog.observers.len());
    for (observer_id, scope) in &catalog.observers {
        hasher.digest(observer_id.bytes());
        hash_observer_scope(&mut hasher, *scope);
    }

    SupportEvidenceRoot(hasher.finish())
}

fn hash_view_input(hasher: &mut SupportEvidenceHasher, input: ViewInputId) {
    match input {
        ViewInputId::Sources(relation_id) => {
            hasher.tag(0x03);
            hasher.digest(relation_id.bytes());
        }
        ViewInputId::Selected(question_id) => {
            hasher.tag(0x01);
            hasher.digest(question_id.bytes());
        }
        ViewInputId::Choice(choice_id) => {
            hasher.tag(0x04);
            hasher.digest(choice_id.bytes());
        }
        ViewInputId::MechanismIncidence(request_id) => {
            hasher.tag(0x02);
            hasher.digest(request_id.bytes());
        }
    }
}

fn hash_observer_scope(hasher: &mut SupportEvidenceHasher, scope: SupportObserverLayerScope) {
    match scope {
        SupportObserverLayerScope::Relation(relation_id) => {
            hasher.tag(0x01);
            hasher.digest(relation_id.bytes());
        }
        SupportObserverLayerScope::Question(question_id) => {
            hasher.tag(0x02);
            hasher.digest(question_id.bytes());
        }
        SupportObserverLayerScope::MechanismRequest(request_id) => {
            hasher.tag(0x03);
            hasher.digest(request_id.bytes());
        }
        SupportObserverLayerScope::View(view_id) => {
            hasher.tag(0x04);
            hasher.digest(view_id.bytes());
        }
        SupportObserverLayerScope::Producer(producer_id) => {
            hasher.tag(0x05);
            hasher.digest(producer_id.bytes());
        }
        SupportObserverLayerScope::ExactCell(cell_id) => {
            hasher.tag(0x06);
            hasher.digest(cell_id.bytes());
        }
    }
}

struct SupportEvidenceHasher(Sha256);

impl SupportEvidenceHasher {
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

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn len(&mut self, value: usize) {
        self.u128(value as u128);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.len(value.len());
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// Canonical validated catalog view. Every map is ordered by a content or
/// layer identity, so equal accepted record sets yield equal snapshots and
/// roots regardless of ingestion order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SupportEvidenceSnapshot {
    version: u32,
    root: SupportEvidenceRoot,
    catalog_sealed: bool,
    root_frontier_sealed: bool,
    obligation_frontier_sealed: bool,
    support_frontier_complete: bool,
    obligation_frontier_complete: bool,
    counts: SupportEvidenceCounts,
    presentation_counts: SupportPresentationCounts,
    cells: BTreeMap<SupportCellId, SupportCell>,
    root_cells: BTreeSet<SupportCellId>,
    partitions: BTreeMap<SupportPartitionId, SupportPartitionCertificate>,
    partition_by_parent: BTreeMap<SupportCellId, SupportPartitionId>,
    active_leaf_ids: BTreeSet<SupportCellId>,
    open_leaf_ids: BTreeSet<SupportCellId>,
    sealed_leaf_ids: BTreeSet<SupportCellId>,
    obligations: BTreeMap<SupportProofObligationId, SupportObligationRecord>,
    obligation_refinements: BTreeMap<SupportObligationRefinementId, SupportObligationRefinement>,
    refinement_by_parent: BTreeMap<SupportProofObligationId, SupportObligationRefinementId>,
    root_obligation_ids: BTreeSet<SupportProofObligationId>,
    active_obligation_leaf_ids: BTreeSet<SupportProofObligationId>,
    superseded_obligation_ids: BTreeSet<SupportProofObligationId>,
    proved_obligation_ids: BTreeSet<SupportProofObligationId>,
    open_obligation_ids: BTreeSet<SupportProofObligationId>,
    evidence: BTreeMap<SupportCellEvidenceId, SupportEvidenceRecord>,
    evidence_by_obligation: BTreeMap<SupportProofObligationId, BTreeSet<SupportCellEvidenceId>>,
    cursor_records: BTreeMap<SupportMaterializationCursorId, SupportMaterializationCursor>,
    cursor_by_position: BTreeMap<(SupportCellId, u128), SupportMaterializationCursorId>,
    latest_cursor_by_cell: BTreeMap<SupportCellId, SupportMaterializationCursorId>,
    retained_examples: BTreeMap<SupportCellId, SupportRetainedExamplesSnapshot>,
    admissions: BTreeMap<AdmissionId, RelationId>,
    questions: BTreeMap<QuestionId, AdmissionId>,
    choices: BTreeMap<ChoiceId, QuestionId>,
    mechanism_requests: BTreeMap<MechanismRequestId, QuestionId>,
    views: BTreeMap<ViewId, ViewInputId>,
    observers: BTreeMap<SupportObserverId, SupportObserverLayerScope>,
}

impl SupportEvidenceSnapshot {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn root(&self) -> SupportEvidenceRoot {
        self.root
    }

    pub(crate) const fn catalog_is_sealed(&self) -> bool {
        self.catalog_sealed
    }

    pub(crate) const fn root_frontier_is_sealed(&self) -> bool {
        self.root_frontier_sealed
    }

    pub(crate) const fn obligation_frontier_is_sealed(&self) -> bool {
        self.obligation_frontier_sealed
    }

    pub(crate) const fn support_frontier_is_complete(&self) -> bool {
        self.support_frontier_complete
    }

    pub(crate) const fn obligation_frontier_is_complete(&self) -> bool {
        self.obligation_frontier_complete
    }

    pub(crate) const fn counts(&self) -> SupportEvidenceCounts {
        self.counts
    }

    pub(crate) const fn presentation_counts(&self) -> SupportPresentationCounts {
        self.presentation_counts
    }

    pub(crate) fn classification_view(&self) -> SupportEvidenceClassificationView<'_> {
        SupportEvidenceClassificationView {
            cells: &self.cells,
            root_cells: &self.root_cells,
            partitions: &self.partitions,
            partition_by_parent: &self.partition_by_parent,
            sealed_leaf_claims: &self.sealed_leaf_ids,
            evidence: &self.evidence,
        }
    }

    pub(crate) fn cells(&self) -> impl ExactSizeIterator<Item = &SupportCell> {
        self.cells.values()
    }

    pub(crate) fn cell(&self, cell_id: SupportCellId) -> Option<&SupportCell> {
        self.cells.get(&cell_id)
    }

    pub(crate) fn root_cell_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.root_cells.iter().copied()
    }

    pub(crate) fn partitions(&self) -> impl ExactSizeIterator<Item = &SupportPartitionCertificate> {
        self.partitions.values()
    }

    pub(crate) fn partition_for_parent(
        &self,
        parent_id: SupportCellId,
    ) -> Option<&SupportPartitionCertificate> {
        self.partition_by_parent
            .get(&parent_id)
            .and_then(|partition_id| self.partitions.get(partition_id))
    }

    pub(crate) fn active_leaf_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.active_leaf_ids.iter().copied()
    }

    pub(crate) fn open_leaf_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.open_leaf_ids.iter().copied()
    }

    pub(crate) fn sealed_leaf_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.sealed_leaf_ids.iter().copied()
    }

    pub(crate) fn obligations(&self) -> impl ExactSizeIterator<Item = &SupportObligationRecord> {
        self.obligations.values()
    }

    pub(crate) fn obligation(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<&SupportObligationRecord> {
        self.obligations.get(&obligation_id)
    }

    pub(crate) fn obligation_cell(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> Option<SupportCellId> {
        self.obligation(obligation_id)
            .map(SupportObligationRecord::cell_id)
    }

    pub(crate) fn obligation_refinements(
        &self,
    ) -> impl ExactSizeIterator<Item = &SupportObligationRefinement> {
        self.obligation_refinements.values()
    }

    pub(crate) fn obligation_refinement(
        &self,
        refinement_id: SupportObligationRefinementId,
    ) -> Option<&SupportObligationRefinement> {
        self.obligation_refinements.get(&refinement_id)
    }

    pub(crate) fn refinement_for_parent(
        &self,
        parent_obligation_id: SupportProofObligationId,
    ) -> Option<&SupportObligationRefinement> {
        self.refinement_by_parent
            .get(&parent_obligation_id)
            .and_then(|refinement_id| self.obligation_refinements.get(refinement_id))
    }

    pub(crate) fn root_obligation_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProofObligationId> + '_ {
        self.root_obligation_ids.iter().copied()
    }

    pub(crate) fn active_obligation_leaf_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProofObligationId> + '_ {
        self.active_obligation_leaf_ids.iter().copied()
    }

    pub(crate) fn superseded_obligation_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProofObligationId> + '_ {
        self.superseded_obligation_ids.iter().copied()
    }

    pub(crate) fn proved_obligation_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProofObligationId> + '_ {
        self.proved_obligation_ids.iter().copied()
    }

    /// Active obligation leaves with no accepted conclusion. Refined parents
    /// are superseded and deliberately absent even if they have no evidence.
    pub(crate) fn open_obligation_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProofObligationId> + '_ {
        self.open_obligation_ids.iter().copied()
    }

    pub(crate) fn evidence(&self) -> impl ExactSizeIterator<Item = &SupportEvidenceRecord> {
        self.evidence.values()
    }

    pub(crate) fn evidence_record(
        &self,
        evidence_id: SupportCellEvidenceId,
    ) -> Option<&SupportEvidenceRecord> {
        self.evidence.get(&evidence_id)
    }

    pub(crate) fn evidence_for_obligation(
        &self,
        obligation_id: SupportProofObligationId,
    ) -> impl Iterator<Item = &SupportEvidenceRecord> {
        self.evidence_by_obligation
            .get(&obligation_id)
            .into_iter()
            .flatten()
            .filter_map(|evidence_id| self.evidence.get(evidence_id))
    }

    pub(crate) fn cursor_records(
        &self,
    ) -> impl ExactSizeIterator<Item = &SupportMaterializationCursor> {
        self.cursor_records.values()
    }

    pub(crate) fn cursor(
        &self,
        cursor_id: SupportMaterializationCursorId,
    ) -> Option<&SupportMaterializationCursor> {
        self.cursor_records.get(&cursor_id)
    }

    pub(crate) fn cursor_at(
        &self,
        cell_id: SupportCellId,
        next_coordinate_ordinal: u128,
    ) -> Option<&SupportMaterializationCursor> {
        self.cursor_by_position
            .get(&(cell_id, next_coordinate_ordinal))
            .and_then(|cursor_id| self.cursor_records.get(cursor_id))
    }

    pub(crate) fn latest_cursor(
        &self,
        cell_id: SupportCellId,
    ) -> Option<&SupportMaterializationCursor> {
        self.latest_cursor_by_cell
            .get(&cell_id)
            .and_then(|cursor_id| self.cursor_records.get(cursor_id))
    }

    pub(crate) fn latest_cursors(
        &self,
    ) -> impl ExactSizeIterator<Item = &SupportMaterializationCursor> {
        self.latest_cursor_by_cell
            .values()
            .map(|cursor_id| &self.cursor_records[cursor_id])
    }

    pub(crate) fn retained_examples(
        &self,
        cell_id: SupportCellId,
    ) -> Option<&SupportRetainedExamplesSnapshot> {
        self.retained_examples.get(&cell_id)
    }

    pub(crate) fn admission_relation(&self, admission_id: AdmissionId) -> Option<RelationId> {
        self.admissions.get(&admission_id).copied()
    }

    pub(crate) fn question_admission(&self, question_id: QuestionId) -> Option<AdmissionId> {
        self.questions.get(&question_id).copied()
    }

    pub(crate) fn mechanism_question(&self, request_id: MechanismRequestId) -> Option<QuestionId> {
        self.mechanism_requests.get(&request_id).copied()
    }

    pub(crate) fn choice_question(&self, choice_id: ChoiceId) -> Option<QuestionId> {
        self.choices.get(&choice_id).copied()
    }

    pub(crate) fn view_input(&self, view_id: ViewId) -> Option<ViewInputId> {
        self.views.get(&view_id).copied()
    }

    pub(crate) fn observer_scope(
        &self,
        observer_id: SupportObserverId,
    ) -> Option<SupportObserverLayerScope> {
        self.observers.get(&observer_id).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportReferenceKind {
    Root,
    PartitionParent,
    PartitionChild,
    SealedLeaf,
    ProofObligation,
    Evidence,
    Cursor,
    Observer,
    RetainedExample,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportLayerReference {
    Question(QuestionId),
    Choice(ChoiceId),
    MechanismRequest(MechanismRequestId),
    View(ViewId),
    Observer(SupportObserverId),
    Obligation(SupportProofObligationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportEvidenceError {
    AtomicAppendReservationFailed,
    CatalogSealed,
    RootFrontierSealed,
    ObligationFrontierSealed,
    CellIdCollision {
        cell_id: SupportCellId,
    },
    InvalidCell {
        cell_id: SupportCellId,
        source: SupportCellError,
    },
    UnknownCell {
        cell_id: SupportCellId,
        referenced_by: SupportReferenceKind,
    },
    UnknownExpressionInput {
        cell_id: SupportCellId,
        input_cell_id: SupportCellId,
    },
    ExpressionDependencyCycle {
        cell_id: SupportCellId,
    },
    PartitionIdCollision {
        partition_id: SupportPartitionId,
    },
    InvalidPartition {
        partition_id: SupportPartitionId,
        source: SupportCellError,
    },
    IncompatibleParentReplacement {
        parent_id: SupportCellId,
        first_partition_id: SupportPartitionId,
        second_partition_id: SupportPartitionId,
    },
    PartitionForSealedLeaf {
        parent_id: SupportCellId,
    },
    SealedLeafHasPartition {
        cell_id: SupportCellId,
    },
    PartitionParentIndexMismatch {
        parent_id: SupportCellId,
        partition_id: SupportPartitionId,
    },
    PartitionParentIndexCardinalityMismatch,
    PartitionCycle {
        cell_id: SupportCellId,
    },
    ObligationIdCollision {
        obligation_id: SupportProofObligationId,
    },
    InvalidObligation {
        obligation_id: SupportProofObligationId,
        source: SupportCellError,
    },
    UnknownRootObligation {
        obligation_id: SupportProofObligationId,
    },
    RootObligationIsRefinementChild {
        obligation_id: SupportProofObligationId,
    },
    UnreachableObligation {
        obligation_id: SupportProofObligationId,
    },
    EmptyObligationRefinement,
    RefinementIdMismatch {
        claimed: SupportObligationRefinementId,
        derived: SupportObligationRefinementId,
    },
    RefinementIdCollision {
        refinement_id: SupportObligationRefinementId,
    },
    NonCanonicalObligationRefinement {
        refinement_id: SupportObligationRefinementId,
    },
    IncompatibleObligationRefinement {
        parent_obligation_id: SupportProofObligationId,
        first_refinement_id: SupportObligationRefinementId,
        second_refinement_id: SupportObligationRefinementId,
    },
    RefinementForProvedObligation {
        parent_obligation_id: SupportProofObligationId,
        refinement_id: SupportObligationRefinementId,
    },
    EvidenceForRefinedObligation {
        obligation_id: SupportProofObligationId,
        refinement_id: SupportObligationRefinementId,
        evidence_id: SupportCellEvidenceId,
    },
    RefinementIndexMismatch,
    UnknownRefinementObligation {
        refinement_id: SupportObligationRefinementId,
        obligation_id: SupportProofObligationId,
    },
    UnknownRefinementPartition {
        refinement_id: SupportObligationRefinementId,
        partition_id: SupportPartitionId,
    },
    RefinementParentCellMismatch {
        parent_obligation_id: SupportProofObligationId,
        parent_cell_id: SupportCellId,
        partition_id: SupportPartitionId,
        partition_parent_id: SupportCellId,
    },
    RefinementChildCountMismatch {
        parent_obligation_id: SupportProofObligationId,
        expected: usize,
        actual: usize,
    },
    DuplicateRefinementChildCell {
        parent_obligation_id: SupportProofObligationId,
        child_cell_id: SupportCellId,
    },
    RefinementChildCellMismatch {
        parent_obligation_id: SupportProofObligationId,
        child_cell_id: SupportCellId,
    },
    RefinementClaimMismatch {
        parent_obligation_id: SupportProofObligationId,
        child_obligation_id: SupportProofObligationId,
    },
    ObligationRefinementCycle {
        obligation_id: SupportProofObligationId,
    },
    EvidenceIdCollision {
        evidence_id: SupportCellEvidenceId,
    },
    InvalidEvidence {
        evidence_id: SupportCellEvidenceId,
        source: SupportCellError,
    },
    MissingObligationForEvidence {
        evidence_id: SupportCellEvidenceId,
        obligation_id: SupportProofObligationId,
    },
    EvidenceObligationMismatch {
        evidence_id: SupportCellEvidenceId,
        obligation_id: SupportProofObligationId,
    },
    ContradictoryConclusion {
        obligation_id: SupportProofObligationId,
    },
    EvidenceIndexMismatch,
    InvalidEvidenceForCell {
        evidence_id: SupportCellEvidenceId,
        cell_id: SupportCellId,
        source: SupportCellError,
    },
    InjectivityForUnmappedCell {
        obligation_id: SupportProofObligationId,
        cell_id: SupportCellId,
    },
    ObligationMaterializerMismatch {
        obligation_id: SupportProofObligationId,
        expected: SupportMaterializerId,
        actual: SupportMaterializerId,
    },
    CardinalityInjectivityConflict {
        cell_id: SupportCellId,
        exact_count: u128,
        injective_count: u128,
    },
    CursorIdCollision {
        cursor_id: SupportMaterializationCursorId,
    },
    InvalidCursor {
        cursor_id: SupportMaterializationCursorId,
        source: SupportCellError,
    },
    CursorCheckpointConflict {
        cell_id: SupportCellId,
        coordinate_ordinal: u128,
    },
    CursorPositionIndexMismatch,
    LatestCursorIndexMismatch,
    RetainedExampleCapConflict {
        cell_id: SupportCellId,
        first_cap: usize,
        second_cap: usize,
    },
    RetainedExampleCellMismatch {
        cell_id: SupportCellId,
        example_cell_id: SupportCellId,
    },
    RetainedExampleCapExceeded {
        cell_id: SupportCellId,
        retained: usize,
        cap: usize,
    },
    AdmissionLayerCollision {
        admission_id: AdmissionId,
    },
    QuestionLayerCollision {
        question_id: QuestionId,
    },
    MechanismLayerCollision {
        request_id: MechanismRequestId,
    },
    ChoiceLayerCollision {
        choice_id: ChoiceId,
    },
    ViewLayerCollision {
        view_id: ViewId,
    },
    ObserverLayerCollision {
        observer_id: SupportObserverId,
    },
    UnknownAdmissionLayer {
        admission_id: AdmissionId,
        referenced_by: SupportLayerReference,
    },
    UnknownQuestionLayer {
        question_id: QuestionId,
        referenced_by: SupportLayerReference,
    },
    UnknownChoiceLayer {
        choice_id: ChoiceId,
        referenced_by: SupportLayerReference,
    },
    UnknownMechanismLayer {
        request_id: MechanismRequestId,
        referenced_by: SupportLayerReference,
    },
    UnknownViewLayer {
        view_id: ViewId,
        observer_id: SupportObserverId,
    },
    UnknownObserverLayer {
        observer_id: SupportObserverId,
        obligation_id: SupportProofObligationId,
    },
    LayerCellRelationMismatch {
        obligation_id: SupportProofObligationId,
        cell_id: SupportCellId,
        expected_relation_id: RelationId,
    },
    ObserverCellScopeMismatch {
        observer_id: SupportObserverId,
        obligation_id: SupportProofObligationId,
        cell_id: SupportCellId,
    },
    SupportFrontierOpen {
        roots_open: bool,
        open_leaves: usize,
    },
    ProofFrontierOpen {
        obligations_open: bool,
        open_obligations: usize,
    },
}

impl fmt::Display for SupportEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SupportEvidenceError as Error;
        match self {
            Error::AtomicAppendReservationFailed => {
                formatter.write_str("cannot reserve bounded support-catalog transaction metadata")
            }
            Error::CatalogSealed => formatter.write_str("support evidence catalog is sealed"),
            Error::RootFrontierSealed => {
                formatter.write_str("support root frontier is already sealed")
            }
            Error::ObligationFrontierSealed => {
                formatter.write_str("support obligation frontier is already sealed")
            }
            Error::CellIdCollision { .. } => {
                formatter.write_str("support cell ID has conflicting content")
            }
            Error::InvalidCell { source, .. } => {
                write!(formatter, "invalid support cell: {source}")
            }
            Error::UnknownCell { referenced_by, .. } => {
                write!(
                    formatter,
                    "{referenced_by:?} references an unknown support cell"
                )
            }
            Error::UnknownExpressionInput { .. } => formatter
                .write_str("support expression references an unknown upstream support cell"),
            Error::ExpressionDependencyCycle { .. } => {
                formatter.write_str("support expression dependency graph contains a cycle")
            }
            Error::PartitionIdCollision { .. } => {
                formatter.write_str("support partition ID has conflicting content")
            }
            Error::InvalidPartition { source, .. } => {
                write!(formatter, "invalid support partition: {source}")
            }
            Error::IncompatibleParentReplacement { .. } => formatter
                .write_str("one support parent has multiple incompatible partition replacements"),
            Error::PartitionForSealedLeaf { .. } | Error::SealedLeafHasPartition { .. } => {
                formatter.write_str("a sealed support leaf cannot also have a partition")
            }
            Error::PartitionParentIndexMismatch { .. }
            | Error::PartitionParentIndexCardinalityMismatch => {
                formatter.write_str("support partition parent index is inconsistent")
            }
            Error::PartitionCycle { .. } => {
                formatter.write_str("support partition graph contains a cycle")
            }
            Error::ObligationIdCollision { .. } => {
                formatter.write_str("support obligation ID has conflicting content")
            }
            Error::InvalidObligation { source, .. } => {
                write!(formatter, "invalid support obligation: {source}")
            }
            Error::UnknownRootObligation { .. } => {
                formatter.write_str("declared root references an unknown support obligation")
            }
            Error::RootObligationIsRefinementChild { .. } => formatter
                .write_str("a declared root support obligation cannot also be a refinement child"),
            Error::UnreachableObligation { .. } => formatter
                .write_str("support obligation is not reachable from an explicitly declared root"),
            Error::EmptyObligationRefinement => {
                formatter.write_str("support obligation refinement has no children")
            }
            Error::RefinementIdMismatch { .. } => {
                formatter.write_str("support obligation refinement identity mismatch")
            }
            Error::RefinementIdCollision { .. } => {
                formatter.write_str("support obligation refinement ID has conflicting content")
            }
            Error::NonCanonicalObligationRefinement { .. } => {
                formatter.write_str("support obligation refinement children are not canonical")
            }
            Error::IncompatibleObligationRefinement { .. } => formatter.write_str(
                "one support obligation has multiple incompatible refinement replacements",
            ),
            Error::RefinementForProvedObligation { .. }
            | Error::EvidenceForRefinedObligation { .. } => formatter.write_str(
                "accepted direct evidence and refinement cannot coexist for one obligation",
            ),
            Error::RefinementIndexMismatch => {
                formatter.write_str("support obligation refinement indexes are inconsistent")
            }
            Error::UnknownRefinementObligation { .. } => formatter
                .write_str("support obligation refinement references an unknown obligation"),
            Error::UnknownRefinementPartition { .. } => {
                formatter.write_str("support obligation refinement references an unknown partition")
            }
            Error::RefinementParentCellMismatch { .. } => formatter.write_str(
                "support obligation refinement parent does not match its cell partition",
            ),
            Error::RefinementChildCountMismatch { .. }
            | Error::DuplicateRefinementChildCell { .. }
            | Error::RefinementChildCellMismatch { .. } => formatter.write_str(
                "support obligation refinement children do not match the cell partition",
            ),
            Error::RefinementClaimMismatch { .. } => formatter
                .write_str("support obligation refinement child does not repeat the parent claim"),
            Error::ObligationRefinementCycle { .. } => {
                formatter.write_str("support obligation refinement graph contains a cycle")
            }
            Error::EvidenceIdCollision { .. } => {
                formatter.write_str("support evidence ID has conflicting content")
            }
            Error::InvalidEvidence { source, .. }
            | Error::InvalidEvidenceForCell { source, .. } => {
                write!(formatter, "invalid support evidence: {source}")
            }
            Error::MissingObligationForEvidence { .. }
            | Error::EvidenceObligationMismatch { .. } => {
                formatter.write_str("support evidence does not match its cataloged obligation")
            }
            Error::ContradictoryConclusion { .. } => {
                formatter.write_str("one support obligation has contradictory accepted conclusions")
            }
            Error::EvidenceIndexMismatch => {
                formatter.write_str("support evidence indexes are inconsistent")
            }
            Error::InjectivityForUnmappedCell { .. } => formatter
                .write_str("injectivity obligation is attached to an unmapped support cell"),
            Error::ObligationMaterializerMismatch { .. } => {
                formatter.write_str("injectivity obligation names another support materializer")
            }
            Error::CardinalityInjectivityConflict { .. } => formatter
                .write_str("exact image cardinality contradicts accepted materializer injectivity"),
            Error::CursorIdCollision { .. } => {
                formatter.write_str("support cursor ID has conflicting content")
            }
            Error::InvalidCursor { source, .. } => {
                write!(formatter, "invalid support cursor: {source}")
            }
            Error::CursorCheckpointConflict { .. } => {
                formatter.write_str("one support coordinate position has incompatible checkpoints")
            }
            Error::CursorPositionIndexMismatch | Error::LatestCursorIndexMismatch => {
                formatter.write_str("support cursor indexes are inconsistent")
            }
            Error::RetainedExampleCapConflict { .. } => {
                formatter.write_str("retained-example metadata has conflicting caps for one cell")
            }
            Error::RetainedExampleCellMismatch { .. } => {
                formatter.write_str("retained example metadata references another support cell")
            }
            Error::RetainedExampleCapExceeded { .. } => {
                formatter.write_str("retained example metadata exceeds its cap")
            }
            Error::AdmissionLayerCollision { .. }
            | Error::QuestionLayerCollision { .. }
            | Error::MechanismLayerCollision { .. }
            | Error::ChoiceLayerCollision { .. }
            | Error::ViewLayerCollision { .. }
            | Error::ObserverLayerCollision { .. } => {
                formatter.write_str("support layer ID has conflicting registration")
            }
            Error::UnknownAdmissionLayer { .. }
            | Error::UnknownQuestionLayer { .. }
            | Error::UnknownChoiceLayer { .. }
            | Error::UnknownMechanismLayer { .. }
            | Error::UnknownViewLayer { .. }
            | Error::UnknownObserverLayer { .. } => {
                formatter.write_str("support obligation references an unknown layer")
            }
            Error::LayerCellRelationMismatch { .. } => formatter
                .write_str("support obligation layer does not own the referenced case cell"),
            Error::ObserverCellScopeMismatch { .. } => {
                formatter.write_str("support observer scope does not include the referenced cell")
            }
            Error::SupportFrontierOpen {
                roots_open,
                open_leaves,
            } => write!(
                formatter,
                "support frontier remains open (roots_open={roots_open}, open_leaves={open_leaves})"
            ),
            Error::ProofFrontierOpen {
                obligations_open,
                open_obligations,
            } => write!(
                formatter,
                "proof frontier remains open (obligations_open={obligations_open}, open_obligations={open_obligations})"
            ),
        }
    }
}

impl Error for SupportEvidenceError {}

#[derive(Debug)]
struct ValidatedState {
    active_leaf_ids: BTreeSet<SupportCellId>,
    open_leaf_ids: BTreeSet<SupportCellId>,
    sealed_leaf_ids: BTreeSet<SupportCellId>,
    root_obligation_ids: BTreeSet<SupportProofObligationId>,
    active_obligation_leaf_ids: BTreeSet<SupportProofObligationId>,
    superseded_obligation_ids: BTreeSet<SupportProofObligationId>,
    proved_obligation_ids: BTreeSet<SupportProofObligationId>,
    open_obligation_ids: BTreeSet<SupportProofObligationId>,
}

impl SupportEvidenceCatalogBuilder {
    fn validate_state(&self) -> Result<ValidatedState, SupportEvidenceError> {
        self.validate_layer_registry()?;

        for (cell_id, cell) in &self.cells {
            if cell.id() != *cell_id {
                return Err(SupportEvidenceError::CellIdCollision { cell_id: *cell_id });
            }
            cell.validate()
                .map_err(|source| SupportEvidenceError::InvalidCell {
                    cell_id: *cell_id,
                    source,
                })?;
        }
        validate_expression_dependency_graph(&self.cells)?;
        for root_id in &self.root_cells {
            self.require_cell(*root_id, SupportReferenceKind::Root)?;
        }
        for sealed_leaf_id in &self.sealed_leaf_claims {
            self.require_cell(*sealed_leaf_id, SupportReferenceKind::SealedLeaf)?;
            if self.partition_by_parent.contains_key(sealed_leaf_id) {
                return Err(SupportEvidenceError::SealedLeafHasPartition {
                    cell_id: *sealed_leaf_id,
                });
            }
        }

        for (partition_id, partition) in &self.partitions {
            if partition.id() != *partition_id {
                return Err(SupportEvidenceError::PartitionIdCollision {
                    partition_id: *partition_id,
                });
            }
            partition
                .validate()
                .map_err(|source| SupportEvidenceError::InvalidPartition {
                    partition_id: *partition_id,
                    source,
                })?;
            self.require_cell(partition.parent_id(), SupportReferenceKind::PartitionParent)?;
            if self.partition_by_parent.get(&partition.parent_id()) != Some(partition_id) {
                return Err(SupportEvidenceError::PartitionParentIndexMismatch {
                    parent_id: partition.parent_id(),
                    partition_id: *partition_id,
                });
            }
            for child_id in partition.child_ids() {
                self.require_cell(*child_id, SupportReferenceKind::PartitionChild)?;
            }
        }
        if self.partition_by_parent.len() != self.partitions.len() {
            return Err(SupportEvidenceError::PartitionParentIndexCardinalityMismatch);
        }
        validate_partition_acyclic(&self.partition_by_parent, &self.partitions)?;

        for (obligation_id, obligation) in &self.obligations {
            if obligation.id() != *obligation_id {
                return Err(SupportEvidenceError::ObligationIdCollision {
                    obligation_id: *obligation_id,
                });
            }
            obligation
                .validate()
                .map_err(|source| SupportEvidenceError::InvalidObligation {
                    obligation_id: *obligation_id,
                    source,
                })?;
            let cell =
                self.require_cell(obligation.cell_id(), SupportReferenceKind::ProofObligation)?;
            self.validate_obligation_scope(obligation, cell)?;
        }
        for obligation_id in &self.root_obligations {
            if !self.obligations.contains_key(obligation_id) {
                return Err(SupportEvidenceError::UnknownRootObligation {
                    obligation_id: *obligation_id,
                });
            }
        }

        let mut recomputed_refinement_by_parent = BTreeMap::new();
        for (refinement_id, refinement) in &self.obligation_refinements {
            if refinement.id() != *refinement_id {
                return Err(SupportEvidenceError::RefinementIdCollision {
                    refinement_id: *refinement_id,
                });
            }
            refinement.validate_identity()?;
            let parent = self
                .obligations
                .get(&refinement.parent_obligation_id())
                .ok_or(SupportEvidenceError::UnknownRefinementObligation {
                    refinement_id: *refinement_id,
                    obligation_id: refinement.parent_obligation_id(),
                })?;
            let partition = self.partitions.get(&refinement.partition_id()).ok_or(
                SupportEvidenceError::UnknownRefinementPartition {
                    refinement_id: *refinement_id,
                    partition_id: refinement.partition_id(),
                },
            )?;
            let mut children = Vec::with_capacity(refinement.child_obligation_ids().len());
            for child_obligation_id in refinement.child_obligation_ids() {
                children.push(self.obligations.get(child_obligation_id).ok_or(
                    SupportEvidenceError::UnknownRefinementObligation {
                        refinement_id: *refinement_id,
                        obligation_id: *child_obligation_id,
                    },
                )?);
            }
            let canonical =
                SupportObligationRefinement::new(parent, partition, children.into_iter())?;
            if canonical != *refinement {
                return Err(SupportEvidenceError::NonCanonicalObligationRefinement {
                    refinement_id: *refinement_id,
                });
            }
            if let Some(first_refinement_id) = recomputed_refinement_by_parent
                .insert(refinement.parent_obligation_id(), *refinement_id)
            {
                return Err(SupportEvidenceError::IncompatibleObligationRefinement {
                    parent_obligation_id: refinement.parent_obligation_id(),
                    first_refinement_id,
                    second_refinement_id: *refinement_id,
                });
            }
        }
        if recomputed_refinement_by_parent != self.refinement_by_parent {
            return Err(SupportEvidenceError::RefinementIndexMismatch);
        }
        validate_obligation_refinement_acyclic(
            &self.refinement_by_parent,
            &self.obligation_refinements,
        )?;

        let mut recomputed_conclusions = BTreeMap::new();
        let mut recomputed_evidence_by_obligation =
            BTreeMap::<SupportProofObligationId, BTreeSet<SupportCellEvidenceId>>::new();
        for (evidence_id, evidence) in &self.evidence {
            if evidence.id() != *evidence_id {
                return Err(SupportEvidenceError::EvidenceIdCollision {
                    evidence_id: *evidence_id,
                });
            }
            evidence
                .validate()
                .map_err(|source| SupportEvidenceError::InvalidEvidence {
                    evidence_id: *evidence_id,
                    source,
                })?;
            let obligation_id = evidence.obligation_id();
            let stored_obligation = self.obligations.get(&obligation_id).ok_or(
                SupportEvidenceError::MissingObligationForEvidence {
                    evidence_id: *evidence_id,
                    obligation_id,
                },
            )?;
            if stored_obligation != &evidence.obligation_record() {
                return Err(SupportEvidenceError::EvidenceObligationMismatch {
                    evidence_id: *evidence_id,
                    obligation_id,
                });
            }
            if let Some(refinement_id) = self.refinement_by_parent.get(&obligation_id) {
                return Err(SupportEvidenceError::EvidenceForRefinedObligation {
                    obligation_id,
                    refinement_id: *refinement_id,
                    evidence_id: *evidence_id,
                });
            }
            let conclusion_digest = evidence.conclusion_digest();
            match recomputed_conclusions.get(&obligation_id) {
                Some(existing) if existing != &conclusion_digest => {
                    return Err(SupportEvidenceError::ContradictoryConclusion { obligation_id });
                }
                _ => {
                    recomputed_conclusions
                        .entry(obligation_id)
                        .or_insert(conclusion_digest);
                }
            }
            recomputed_evidence_by_obligation
                .entry(obligation_id)
                .or_default()
                .insert(*evidence_id);
            let cell = self.require_cell(evidence.cell_id(), SupportReferenceKind::Evidence)?;
            self.validate_evidence_against_cell(evidence, cell)?;
        }
        if recomputed_conclusions != self.conclusion_by_obligation
            || recomputed_evidence_by_obligation != self.evidence_by_obligation
        {
            return Err(SupportEvidenceError::EvidenceIndexMismatch);
        }
        self.validate_cardinality_injectivity_consistency()?;

        let mut recomputed_cursor_by_position = BTreeMap::new();
        let mut recomputed_latest_by_cell =
            BTreeMap::<SupportCellId, SupportMaterializationCursorId>::new();
        for (cursor_id, cursor) in &self.cursor_records {
            if cursor.id() != *cursor_id {
                return Err(SupportEvidenceError::CursorIdCollision {
                    cursor_id: *cursor_id,
                });
            }
            let cell = self.require_cell(cursor.cell_id(), SupportReferenceKind::Cursor)?;
            cursor
                .validate_for(cell)
                .map_err(|source| SupportEvidenceError::InvalidCursor {
                    cursor_id: *cursor_id,
                    source,
                })?;

            let position = (cursor.cell_id(), cursor.next_coordinate_ordinal());
            if let Some(first_id) = recomputed_cursor_by_position.insert(position, *cursor_id) {
                if first_id != *cursor_id {
                    return Err(SupportEvidenceError::CursorCheckpointConflict {
                        cell_id: cursor.cell_id(),
                        coordinate_ordinal: cursor.next_coordinate_ordinal(),
                    });
                }
            }

            match recomputed_latest_by_cell.get(&cursor.cell_id()).copied() {
                None => {
                    recomputed_latest_by_cell.insert(cursor.cell_id(), *cursor_id);
                }
                Some(previous_id) => {
                    let previous = self
                        .cursor_records
                        .get(&previous_id)
                        .expect("recomputed latest cursor came from the cursor catalog");
                    if cursor.next_coordinate_ordinal() > previous.next_coordinate_ordinal() {
                        recomputed_latest_by_cell.insert(cursor.cell_id(), *cursor_id);
                    } else if cursor.next_coordinate_ordinal() == previous.next_coordinate_ordinal()
                        && cursor.id() != previous.id()
                    {
                        return Err(SupportEvidenceError::CursorCheckpointConflict {
                            cell_id: cursor.cell_id(),
                            coordinate_ordinal: cursor.next_coordinate_ordinal(),
                        });
                    }
                }
            }
        }
        if recomputed_cursor_by_position != self.cursor_by_position {
            return Err(SupportEvidenceError::CursorPositionIndexMismatch);
        }
        if recomputed_latest_by_cell != self.latest_cursor_by_cell {
            return Err(SupportEvidenceError::LatestCursorIndexMismatch);
        }

        for (cell_id, retained) in &self.retained_examples {
            self.require_cell(*cell_id, SupportReferenceKind::RetainedExample)?;
            if retained.examples.len() > retained.cap {
                return Err(SupportEvidenceError::RetainedExampleCapExceeded {
                    cell_id: *cell_id,
                    retained: retained.examples.len(),
                    cap: retained.cap,
                });
            }
            if retained
                .examples
                .iter()
                .any(|example| example.cell_id() != *cell_id)
            {
                return Err(SupportEvidenceError::RetainedExampleCellMismatch {
                    cell_id: *cell_id,
                    example_cell_id: retained
                        .examples
                        .iter()
                        .find(|example| example.cell_id() != *cell_id)
                        .expect("mismatching example exists")
                        .cell_id(),
                });
            }
        }

        let active_leaf_ids = active_leaves(
            &self.root_cells,
            &self.partition_by_parent,
            &self.partitions,
        )?;
        let sealed_leaf_ids = active_leaf_ids
            .intersection(&self.sealed_leaf_claims)
            .copied()
            .collect::<BTreeSet<_>>();
        let open_leaf_ids = active_leaf_ids
            .difference(&sealed_leaf_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        let obligation_frontier = active_obligation_frontier(
            &self.obligations,
            &self.root_obligations,
            &self.refinement_by_parent,
            &self.obligation_refinements,
            &self.conclusion_by_obligation,
        )?;

        Ok(ValidatedState {
            active_leaf_ids,
            open_leaf_ids,
            sealed_leaf_ids,
            root_obligation_ids: obligation_frontier.root_ids,
            active_obligation_leaf_ids: obligation_frontier.active_leaf_ids,
            superseded_obligation_ids: obligation_frontier.superseded_ids,
            proved_obligation_ids: obligation_frontier.proved_leaf_ids,
            open_obligation_ids: obligation_frontier.open_leaf_ids,
        })
    }

    fn require_cell(
        &self,
        cell_id: SupportCellId,
        referenced_by: SupportReferenceKind,
    ) -> Result<&SupportCell, SupportEvidenceError> {
        self.cells
            .get(&cell_id)
            .ok_or(SupportEvidenceError::UnknownCell {
                cell_id,
                referenced_by,
            })
    }

    fn validate_layer_registry(&self) -> Result<(), SupportEvidenceError> {
        for (question_id, admission_id) in &self.questions {
            if !self.admissions.contains_key(admission_id) {
                return Err(SupportEvidenceError::UnknownAdmissionLayer {
                    admission_id: *admission_id,
                    referenced_by: SupportLayerReference::Question(*question_id),
                });
            }
        }
        for (request_id, question_id) in &self.mechanism_requests {
            if !self.questions.contains_key(question_id) {
                return Err(SupportEvidenceError::UnknownQuestionLayer {
                    question_id: *question_id,
                    referenced_by: SupportLayerReference::MechanismRequest(*request_id),
                });
            }
        }
        for (choice_id, question_id) in &self.choices {
            if !self.questions.contains_key(question_id) {
                return Err(SupportEvidenceError::UnknownQuestionLayer {
                    question_id: *question_id,
                    referenced_by: SupportLayerReference::Choice(*choice_id),
                });
            }
        }
        for (view_id, input) in &self.views {
            match input {
                ViewInputId::Sources(_) => {}
                ViewInputId::Choice(choice_id) if !self.choices.contains_key(choice_id) => {
                    return Err(SupportEvidenceError::UnknownChoiceLayer {
                        choice_id: *choice_id,
                        referenced_by: SupportLayerReference::View(*view_id),
                    });
                }
                ViewInputId::Selected(question_id) if !self.questions.contains_key(question_id) => {
                    return Err(SupportEvidenceError::UnknownQuestionLayer {
                        question_id: *question_id,
                        referenced_by: SupportLayerReference::View(*view_id),
                    });
                }
                ViewInputId::MechanismIncidence(request_id)
                    if !self.mechanism_requests.contains_key(request_id) =>
                {
                    return Err(SupportEvidenceError::UnknownMechanismLayer {
                        request_id: *request_id,
                        referenced_by: SupportLayerReference::View(*view_id),
                    });
                }
                _ => {}
            }
        }
        for (observer_id, scope) in &self.observers {
            self.validate_observer_scope_reference(*observer_id, *scope)?;
        }
        Ok(())
    }

    fn validate_observer_scope_reference(
        &self,
        observer_id: SupportObserverId,
        scope: SupportObserverLayerScope,
    ) -> Result<(), SupportEvidenceError> {
        match scope {
            SupportObserverLayerScope::Question(question_id)
                if !self.questions.contains_key(&question_id) =>
            {
                Err(SupportEvidenceError::UnknownQuestionLayer {
                    question_id,
                    referenced_by: SupportLayerReference::Observer(observer_id),
                })
            }
            SupportObserverLayerScope::MechanismRequest(request_id)
                if !self.mechanism_requests.contains_key(&request_id) =>
            {
                Err(SupportEvidenceError::UnknownMechanismLayer {
                    request_id,
                    referenced_by: SupportLayerReference::Observer(observer_id),
                })
            }
            SupportObserverLayerScope::View(view_id) if !self.views.contains_key(&view_id) => {
                Err(SupportEvidenceError::UnknownViewLayer {
                    view_id,
                    observer_id,
                })
            }
            SupportObserverLayerScope::ExactCell(cell_id) if !self.cells.contains_key(&cell_id) => {
                Err(SupportEvidenceError::UnknownCell {
                    cell_id,
                    referenced_by: SupportReferenceKind::Observer,
                })
            }
            _ => Ok(()),
        }
    }

    fn validate_obligation_scope(
        &self,
        obligation: &SupportObligationRecord,
        cell: &SupportCell,
    ) -> Result<(), SupportEvidenceError> {
        match obligation {
            SupportObligationRecord::Cardinality(_) => Ok(()),
            SupportObligationRecord::Injectivity(value) => {
                if !matches!(cell.space(), SupportCellSpace::MappedImage { .. }) {
                    return Err(SupportEvidenceError::InjectivityForUnmappedCell {
                        obligation_id: value.id(),
                        cell_id: cell.id(),
                    });
                }
                if value.claim().materializer_id() != cell.materializer_id() {
                    return Err(SupportEvidenceError::ObligationMaterializerMismatch {
                        obligation_id: value.id(),
                        expected: cell.materializer_id(),
                        actual: value.claim().materializer_id(),
                    });
                }
                Ok(())
            }
            SupportObligationRecord::Admission(value) => {
                let admission_id = value.claim().admission_id();
                let relation_id = *self.admissions.get(&admission_id).ok_or(
                    SupportEvidenceError::UnknownAdmissionLayer {
                        admission_id,
                        referenced_by: SupportLayerReference::Obligation(value.id()),
                    },
                )?;
                require_case_relation(cell, relation_id, value.id())
            }
            SupportObligationRecord::Selection(value) => {
                let question_id = value.claim().question_id();
                let relation_id = self.relation_for_question(question_id).ok_or(
                    SupportEvidenceError::UnknownQuestionLayer {
                        question_id,
                        referenced_by: SupportLayerReference::Obligation(value.id()),
                    },
                )?;
                require_case_relation(cell, relation_id, value.id())
            }
            SupportObligationRecord::UniformValue(value) => {
                let observer_id = value.claim().observer_id();
                let scope = *self.observers.get(&observer_id).ok_or(
                    SupportEvidenceError::UnknownObserverLayer {
                        observer_id,
                        obligation_id: value.id(),
                    },
                )?;
                if observer_scope_matches_cell(self, scope, cell) {
                    Ok(())
                } else {
                    Err(SupportEvidenceError::ObserverCellScopeMismatch {
                        observer_id,
                        obligation_id: value.id(),
                        cell_id: cell.id(),
                    })
                }
            }
            SupportObligationRecord::UniformMechanism(value) => {
                let request_id = value.claim().request_id();
                let relation_id = self.relation_for_mechanism(request_id).ok_or(
                    SupportEvidenceError::UnknownMechanismLayer {
                        request_id,
                        referenced_by: SupportLayerReference::Obligation(value.id()),
                    },
                )?;
                require_case_relation(cell, relation_id, value.id())
            }
        }
    }

    fn validate_evidence_against_cell(
        &self,
        evidence: &SupportEvidenceRecord,
        cell: &SupportCell,
    ) -> Result<(), SupportEvidenceError> {
        let result = match evidence {
            SupportEvidenceRecord::Cardinality(value) => {
                cell.cardinality_from_evidence(value).map(|_| ())
            }
            SupportEvidenceRecord::Injectivity(value) => {
                cell.cardinality_with_injectivity(value).map(|_| ())
            }
            SupportEvidenceRecord::Admission(value) => cell.validate_evidence(value),
            SupportEvidenceRecord::Selection(value) => cell.validate_evidence(value),
            SupportEvidenceRecord::UniformValue(value) => cell.validate_evidence(value),
            SupportEvidenceRecord::UniformMechanism(value) => cell.validate_evidence(value),
        };
        result.map_err(|source| SupportEvidenceError::InvalidEvidenceForCell {
            evidence_id: evidence.id(),
            cell_id: cell.id(),
            source,
        })
    }

    fn validate_cardinality_injectivity_extension(
        &self,
        evidence: &SupportEvidenceRecord,
        cell: &SupportCell,
    ) -> Result<(), SupportEvidenceError> {
        let evidence_id = evidence.id();
        let cell_id = evidence.cell_id();
        let invalid_opposite_obligation = |source| SupportEvidenceError::InvalidEvidenceForCell {
            evidence_id,
            cell_id,
            source,
        };
        let (prospective_count, compare_kind, opposite_obligation_id) = match evidence {
            SupportEvidenceRecord::Cardinality(value) => (
                value.exact_cardinality(),
                SupportEvidenceKind::Injectivity,
                SupportCellObligation::new(
                    cell,
                    InjectiveMappingClaim::new(cell.materializer_id()),
                )
                .map_err(invalid_opposite_obligation)?
                .id(),
            ),
            SupportEvidenceRecord::Injectivity(value) => {
                let Some(count) = cell
                    .cardinality_with_injectivity(value)
                    .map_err(|source| SupportEvidenceError::InvalidEvidenceForCell {
                        evidence_id: value.id(),
                        cell_id: cell.id(),
                        source,
                    })?
                    .exact()
                else {
                    return Ok(());
                };
                (
                    count,
                    SupportEvidenceKind::Cardinality,
                    SupportCellObligation::new(cell, ExactCardinalityClaim)
                        .map_err(invalid_opposite_obligation)?
                        .id(),
                )
            }
            _ => return Ok(()),
        };

        let Some(opposite_evidence_ids) = self.evidence_by_obligation.get(&opposite_obligation_id)
        else {
            return Ok(());
        };
        for opposite_evidence_id in opposite_evidence_ids {
            let existing = self
                .evidence
                .get(opposite_evidence_id)
                .ok_or(SupportEvidenceError::EvidenceIndexMismatch)?;
            if existing.obligation_id() != opposite_obligation_id
                || existing.kind() != compare_kind
                || existing.cell_id() != cell_id
            {
                return Err(SupportEvidenceError::EvidenceIndexMismatch);
            }
            let existing_count = match existing {
                SupportEvidenceRecord::Cardinality(value) => value.exact_cardinality(),
                SupportEvidenceRecord::Injectivity(value) => {
                    let Some(count) = cell
                        .cardinality_with_injectivity(value)
                        .map_err(|source| SupportEvidenceError::InvalidEvidenceForCell {
                            evidence_id: value.id(),
                            cell_id,
                            source,
                        })?
                        .exact()
                    else {
                        continue;
                    };
                    count
                }
                _ => unreachable!("opposite obligation index selected another evidence kind"),
            };
            if prospective_count != existing_count {
                let (exact_count, injective_count) = match evidence {
                    SupportEvidenceRecord::Cardinality(_) => (prospective_count, existing_count),
                    SupportEvidenceRecord::Injectivity(_) => (existing_count, prospective_count),
                    _ => unreachable!("non-count evidence returned before comparison"),
                };
                return Err(SupportEvidenceError::CardinalityInjectivityConflict {
                    cell_id,
                    exact_count,
                    injective_count,
                });
            }
        }
        Ok(())
    }

    fn validate_cardinality_injectivity_consistency(&self) -> Result<(), SupportEvidenceError> {
        let mut exact_counts = BTreeMap::<SupportCellId, u128>::new();
        let mut injectivity =
            BTreeMap::<SupportCellId, &SupportCellEvidence<InjectiveMappingClaim>>::new();
        for evidence in self.evidence.values() {
            match evidence {
                SupportEvidenceRecord::Cardinality(value) => {
                    exact_counts.insert(value.obligation().cell_id(), value.exact_cardinality());
                }
                SupportEvidenceRecord::Injectivity(value) => {
                    injectivity.insert(value.obligation().cell_id(), value);
                }
                _ => {}
            }
        }
        for (cell_id, injectivity_evidence) in injectivity {
            let Some(exact_count) = exact_counts.get(&cell_id).copied() else {
                continue;
            };
            let cell = self
                .cells
                .get(&cell_id)
                .expect("evidence cell presence checked before consistency validation");
            if let Some(injective_count) = cell
                .cardinality_with_injectivity(injectivity_evidence)
                .map_err(|source| SupportEvidenceError::InvalidEvidenceForCell {
                    evidence_id: injectivity_evidence.id(),
                    cell_id,
                    source,
                })?
                .exact()
            {
                if exact_count != injective_count {
                    return Err(SupportEvidenceError::CardinalityInjectivityConflict {
                        cell_id,
                        exact_count,
                        injective_count,
                    });
                }
            }
        }
        Ok(())
    }

    fn relation_for_question(&self, question_id: QuestionId) -> Option<RelationId> {
        let admission_id = self.questions.get(&question_id)?;
        self.admissions.get(admission_id).copied()
    }

    fn relation_for_mechanism(&self, request_id: MechanismRequestId) -> Option<RelationId> {
        let question_id = *self.mechanism_requests.get(&request_id)?;
        self.relation_for_question(question_id)
    }

    fn relation_for_choice(&self, choice_id: ChoiceId) -> Option<RelationId> {
        let question_id = *self.choices.get(&choice_id)?;
        self.relation_for_question(question_id)
    }

    fn relation_for_view(&self, view_id: ViewId) -> Option<RelationId> {
        match *self.views.get(&view_id)? {
            ViewInputId::Sources(relation_id) => Some(relation_id),
            ViewInputId::Selected(question_id) => self.relation_for_question(question_id),
            ViewInputId::Choice(choice_id) => self.relation_for_choice(choice_id),
            ViewInputId::MechanismIncidence(request_id) => self.relation_for_mechanism(request_id),
        }
    }

    fn counts(
        &self,
        validated: &ValidatedState,
        support_frontier_complete: bool,
        obligation_frontier_complete: bool,
        kinds: &BTreeMap<SupportEvidenceKind, usize>,
    ) -> SupportEvidenceCounts {
        let record_exact = self.catalog_sealed;
        SupportEvidenceCounts {
            cells: count_evidence(self.cells.len(), record_exact),
            roots: count_evidence(self.root_cells.len(), self.root_frontier_sealed),
            active_leaves: count_evidence(
                validated.active_leaf_ids.len(),
                support_frontier_complete,
            ),
            open_leaves: count_evidence(validated.open_leaf_ids.len(), support_frontier_complete),
            partitions: count_evidence(self.partitions.len(), record_exact),
            obligations: count_evidence(self.obligations.len(), self.obligation_frontier_sealed),
            obligation_refinements: count_evidence(self.obligation_refinements.len(), record_exact),
            root_obligations: count_evidence(
                validated.root_obligation_ids.len(),
                self.obligation_frontier_sealed,
            ),
            active_obligation_leaves: count_evidence(
                validated.active_obligation_leaf_ids.len(),
                record_exact,
            ),
            superseded_obligations: count_evidence(
                validated.superseded_obligation_ids.len(),
                record_exact,
            ),
            proved_obligations: count_evidence(
                validated.proved_obligation_ids.len(),
                obligation_frontier_complete,
            ),
            open_obligations: count_evidence(
                validated.open_obligation_ids.len(),
                obligation_frontier_complete,
            ),
            evidence_records: count_evidence(self.evidence.len(), record_exact),
            cardinality_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::Cardinality),
                record_exact,
            ),
            injectivity_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::Injectivity),
                record_exact,
            ),
            admission_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::Admission),
                record_exact,
            ),
            selection_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::Selection),
                record_exact,
            ),
            uniform_value_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::UniformValue),
                record_exact,
            ),
            uniform_mechanism_records: count_evidence(
                kind_count(kinds, SupportEvidenceKind::UniformMechanism),
                record_exact,
            ),
            cursor_records: count_evidence(self.cursor_records.len(), record_exact),
            latest_cursors: count_evidence(self.latest_cursor_by_cell.len(), record_exact),
        }
    }
}
