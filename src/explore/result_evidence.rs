//! Durable, content-addressed evidence for evaluated relational result rows.
//!
//! This catalog sits between row-local expression evaluation and the pure
//! result reducer. It retains exactly the owned values needed to replay an
//! [`EvaluatedResultContribution`] plus cached row-local SELECT/objective
//! values. Relation-owned `context`/`before`/`after` bindings are deliberately
//! absent; a later closed projection must rehydrate those from the canonical
//! relation or incidence catalog instead of copying them into every layer.
//!
//! Every catalog is scoped to one [`ViewId`]. Its closed state binds an exact,
//! typed upstream content root and the canonical set of input-row identities.
//! Merely observing no more arrivals is never a seal. Weighted support-cell
//! contributions remain unsupported until a certified reducer algebra exists.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::choice_relation::ChoiceContentRoot;
use super::mechanism_incidence::{
    ClosedMechanismIncidence, MechanismCaseTerminal, MechanismCaseTerminalRecord,
    MechanismIncidenceRoot,
};
use super::relation::{
    ChoiceId, ClosedQuestionCatalogRef, MechanismRequestId, QuestionCatalog, QuestionContentRoot,
    QuestionId, RelationId, RelationalCaseId, SourceKey, SourceKeySetRoot, ViewId,
};
use super::relational_certified_source_summary::{
    RelationalCertifiedSourceSummaryArtifact, RelationalCertifiedSourceSummaryArtifactId,
};
use super::relational_population::{
    CertifiedSelectedPopulationRoot, ClosedCertifiedSelectedPopulation,
};
use super::relational_result_executor::RelationalResultEvidence;
use super::relational_source_image_exactness::CertifiedSourcePopulationRoot;
use super::result_view::{
    CertifiedResultInputRoot, EvaluatedResultContribution, MechanismIncidenceRowId, ResultValue,
    ResultViewBuilder, ResultViewError, ResultViewInputKind, ResultViewInputRowId, ResultViewSpec,
    ResultViewSpecRoot,
};
use super::structural_mechanism::StructuralQuotientClosureRoot;
use super::support_cell::SupportCellId;
use super::transition::canonical_explore_value_digest;

const RESULT_EVIDENCE_ID_V1: &[u8] = b"futuruna.explore.relational-result-evidence-id.v1";
const RESULT_INPUT_COVERAGE_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-result-input-coverage-root.v1";
const CERTIFIED_SOURCE_INPUT_COVERAGE_ROOT_V1: &[u8] =
    b"futuruna.explore.certified-source-result-input-coverage-root.v1";
const RESULT_EVIDENCE_ROOT_V3: &[u8] = b"futuruna.explore.relational-result-evidence-root.v3";

pub(crate) const RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION: u32 = 3;

/// Content identity of one evaluated singleton-row result contribution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalResultEvidenceId([u8; 32]);

impl RelationalResultEvidenceId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent identity of one open or sealed per-view catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalResultEvidenceRoot([u8; 32]);

impl RelationalResultEvidenceRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact identity-set commitment for the rows consumed by one result view.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultInputCoverageRoot([u8; 32]);

impl ResultInputCoverageRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed upstream relation whose exact row set feeds this result view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultEvidenceUpstreamRoot {
    Sources {
        relation_id: RelationId,
        source_key_root: SourceKeySetRoot,
    },
    /// Exact logical source population consumed through a proof-specialized
    /// reducer. No SourceKey or representative row is fabricated. The
    /// summary artifact commits the checked uniform group values, while the
    /// certified input root commits the complete source population and N.
    CertifiedSources {
        relation_id: RelationId,
        population_root: CertifiedSourcePopulationRoot,
        summary_artifact_id: RelationalCertifiedSourceSummaryArtifactId,
        certified_input_root: CertifiedResultInputRoot,
        exact_cardinality: u128,
    },
    Selected {
        question_id: QuestionId,
        content_root: QuestionContentRoot,
    },
    /// Exact selected population certified by the support/proof DAG without
    /// materializing (or inventing) an extensional question-content root.
    CertifiedSelectedSupport {
        question_id: QuestionId,
        population_root: CertifiedSelectedPopulationRoot,
        exact_cardinality: u128,
    },
    Choice {
        choice_id: ChoiceId,
        content_root: ChoiceContentRoot,
    },
    MechanismIncidence {
        request_id: MechanismRequestId,
        completed_root: MechanismIncidenceRoot,
    },
    /// The same exact incidence row set, enriched by the durable quotient
    /// assignment that supplies structural mechanism and execution-profile
    /// identities to every row.
    StructuralMechanismIncidence {
        request_id: MechanismRequestId,
        completed_root: MechanismIncidenceRoot,
        structural_root: StructuralQuotientClosureRoot,
    },
}

impl ResultEvidenceUpstreamRoot {
    pub(crate) const fn input_kind(self) -> ResultViewInputKind {
        match self {
            Self::Sources { .. } | Self::CertifiedSources { .. } => ResultViewInputKind::Source,
            Self::Selected { .. } | Self::CertifiedSelectedSupport { .. } | Self::Choice { .. } => {
                ResultViewInputKind::Case
            }
            Self::MechanismIncidence { .. } | Self::StructuralMechanismIncidence { .. } => {
                ResultViewInputKind::Incidence
            }
        }
    }
}

/// Exact input-set coverage accompanying a typed upstream content root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResultInputCoverageCommitment {
    input_kind: ResultViewInputKind,
    row_count: u128,
    row_set_root: ResultInputCoverageRoot,
}

impl ResultInputCoverageCommitment {
    pub(crate) const fn input_kind(self) -> ResultViewInputKind {
        self.input_kind
    }

    pub(crate) const fn row_count(self) -> u128 {
        self.row_count
    }

    pub(crate) const fn row_set_root(self) -> ResultInputCoverageRoot {
        self.row_set_root
    }
}

/// Explicit closure receipt for a result view's exact upstream population.
///
/// Constructors accept only already closed upstream catalogs. Journal/catalog
/// integration remains responsible for checking that the view's resolved
/// input semantic ID names the same question or mechanism request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultInputSeal {
    upstream: ResultEvidenceUpstreamRoot,
    coverage: ResultInputCoverageCommitment,
}

impl RelationalResultInputSeal {
    pub(super) fn restore_from_journal_codec(
        upstream: ResultEvidenceUpstreamRoot,
        input_kind: ResultViewInputKind,
        row_count: u128,
        row_set_root: ResultInputCoverageRoot,
    ) -> Result<Self, ResultEvidenceError> {
        let restored = Self {
            upstream,
            coverage: ResultInputCoverageCommitment {
                input_kind,
                row_count,
                row_set_root,
            },
        };
        restored.validate()?;
        Ok(restored)
    }

    pub(crate) fn from_selected(question: &QuestionCatalog) -> Result<Self, ResultEvidenceError> {
        let upstream = ResultEvidenceUpstreamRoot::Selected {
            question_id: question.question_id(),
            content_root: question.content_root(),
        };
        Self::derive(
            upstream,
            question.selected_case_ids().map(ResultViewInputRowId::Case),
        )
    }

    pub(crate) fn from_choice(
        choice_id: ChoiceId,
        content_root: ChoiceContentRoot,
        member_case_ids: impl IntoIterator<Item = RelationalCaseId>,
    ) -> Result<Self, ResultEvidenceError> {
        Self::derive(
            ResultEvidenceUpstreamRoot::Choice {
                choice_id,
                content_root,
            },
            member_case_ids.into_iter().map(ResultViewInputRowId::Case),
        )
    }

    pub(crate) fn from_sources(
        relation_id: RelationId,
        source_key_root: SourceKeySetRoot,
        source_keys: impl ExactSizeIterator<Item = SourceKey>,
    ) -> Self {
        Self {
            upstream: ResultEvidenceUpstreamRoot::Sources {
                relation_id,
                source_key_root,
            },
            coverage: derive_canonical_source_coverage(source_keys),
        }
    }

    /// Seal a proof-specialized source result without pretending its logical
    /// members are zero rows or inventing one representative SourceKey.
    pub(crate) fn from_certified_source_summary(
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Self {
        let upstream = ResultEvidenceUpstreamRoot::CertifiedSources {
            relation_id: artifact.relation_id(),
            population_root: artifact.source_population_root(),
            summary_artifact_id: artifact.artifact_id(),
            certified_input_root: artifact.certified_input_root(),
            exact_cardinality: artifact.exact_cardinality(),
        };
        Self {
            upstream,
            coverage: derive_certified_source_coverage(upstream),
        }
    }

    /// Mint a selected-input seal directly from a validated borrowed FIND
    /// closure. Its selected IDs already arrive in unique canonical CaseId
    /// order, so deriving the coverage root needs no intermediate `BTreeSet`.
    pub(crate) fn from_borrowed_selected(question: &ClosedQuestionCatalogRef<'_>) -> Self {
        let upstream = ResultEvidenceUpstreamRoot::Selected {
            question_id: question.question_id(),
            content_root: question.content_root(),
        };
        Self {
            upstream,
            coverage: derive_canonical_selected_coverage(question),
        }
    }

    pub(crate) fn from_mechanism_incidence(
        incidence: &ClosedMechanismIncidence,
    ) -> Result<Self, ResultEvidenceError> {
        let expected_row_count = incidence
            .snapshot()
            .terminals()
            .iter()
            .filter(|record| matches!(record.terminal(), MechanismCaseTerminal::Incidence { .. }))
            .count() as u128;
        Self::from_canonical_mechanism_terminals(
            incidence.request_id(),
            incidence.root(),
            expected_row_count,
            incidence.snapshot().terminals().iter().copied(),
        )
    }

    /// Mint an incidence-input seal from a validated complete mechanism
    /// frontier without collecting its rows into another ordered set. The
    /// caller supplies terminals in canonical CaseId order and the exact
    /// successful-row count from the same borrowed frontier.
    pub(crate) fn from_canonical_mechanism_terminals(
        request_id: MechanismRequestId,
        completed_root: MechanismIncidenceRoot,
        expected_row_count: u128,
        terminals: impl IntoIterator<Item = MechanismCaseTerminalRecord>,
    ) -> Result<Self, ResultEvidenceError> {
        let upstream = ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id,
            completed_root,
        };
        let coverage = derive_canonical_mechanism_coverage(expected_row_count, terminals)?;
        Ok(Self { upstream, coverage })
    }

    /// Bind an already certified incidence row set to the exact durable
    /// signature-to-structure quotient used to enrich those rows.
    pub(crate) fn with_structural_quotient(
        self,
        structural_root: StructuralQuotientClosureRoot,
    ) -> Result<Self, ResultEvidenceError> {
        let ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id,
            completed_root,
        } = self.upstream
        else {
            return Err(ResultEvidenceError::WrongInputKind {
                expected: ResultViewInputKind::Incidence,
                actual: self.upstream.input_kind(),
            });
        };
        Ok(Self {
            upstream: ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
                request_id,
                completed_root,
                structural_root,
            },
            coverage: self.coverage,
        })
    }

    /// Bind a support-certified selected population to its exact concrete
    /// cases. The proof DAG supplies the independent cardinality/root
    /// authority; the supplied real CaseIds are the rows consumed by result
    /// expressions. No support cell is treated as a representative row and no
    /// synthetic CaseId is minted.
    pub(crate) fn from_certified_selected_population(
        population: &ClosedCertifiedSelectedPopulation,
        selected_case_ids: impl IntoIterator<Item = RelationalCaseId>,
    ) -> Result<Self, ResultEvidenceError> {
        let seal = Self::derive(
            ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
                question_id: population.question_id(),
                population_root: population.root(),
                exact_cardinality: population.exact_cardinality(),
            },
            selected_case_ids
                .into_iter()
                .map(ResultViewInputRowId::Case),
        )?;
        if seal.coverage.row_count != population.exact_cardinality() {
            return Err(
                ResultEvidenceError::CertifiedPopulationConcreteCoverageMismatch {
                    expected: population.exact_cardinality(),
                    actual: seal.coverage.row_count,
                },
            );
        }
        Ok(seal)
    }

    fn derive(
        upstream: ResultEvidenceUpstreamRoot,
        rows: impl IntoIterator<Item = ResultViewInputRowId>,
    ) -> Result<Self, ResultEvidenceError> {
        let coverage = derive_coverage(upstream.input_kind(), rows)?;
        Ok(Self { upstream, coverage })
    }

    pub(crate) const fn upstream(self) -> ResultEvidenceUpstreamRoot {
        self.upstream
    }

    pub(crate) const fn coverage(self) -> ResultInputCoverageCommitment {
        self.coverage
    }

    pub(crate) const fn certified_source_summary_artifact_id(
        self,
    ) -> Option<RelationalCertifiedSourceSummaryArtifactId> {
        match self.upstream {
            ResultEvidenceUpstreamRoot::CertifiedSources {
                summary_artifact_id,
                ..
            } => Some(summary_artifact_id),
            _ => None,
        }
    }

    fn validate(self) -> Result<(), ResultEvidenceError> {
        if self.upstream.input_kind() != self.coverage.input_kind {
            return Err(ResultEvidenceError::InputSealKindMismatch {
                expected: self.upstream.input_kind(),
                actual: self.coverage.input_kind,
            });
        }
        match self.upstream {
            ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
                exact_cardinality, ..
            } => {
                // A positive proof population is consumed only through the
                // exact concrete cases independently checked by the base
                // journal.
                if exact_cardinality != self.coverage.row_count {
                    return Err(
                        ResultEvidenceError::CertifiedPopulationConcreteCoverageMismatch {
                            expected: exact_cardinality,
                            actual: self.coverage.row_count,
                        },
                    );
                }
            }
            upstream @ ResultEvidenceUpstreamRoot::CertifiedSources {
                population_root,
                certified_input_root,
                exact_cardinality,
                ..
            } => {
                if exact_cardinality == 0
                    || exact_cardinality > i64::MAX as u128
                    || certified_input_root
                        != CertifiedResultInputRoot::from_certified_source_population(
                            population_root.bytes(),
                            exact_cardinality,
                        )
                    || self.coverage != derive_certified_source_coverage(upstream)
                {
                    return Err(ResultEvidenceError::CertifiedSourceCoverageMismatch);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Owned canonical payload for one concrete singleton row.
///
/// `contribution` owns the row identity, evaluated grain, measures, and
/// distinct arguments. The two staged arrays retain row-local SELECT and
/// objective results. Full base bindings stay in the relation catalog.
///
/// Group-closed projections commonly stage only `None` values for every row.
/// [`CanonicalOptionalSlots`] retains that exact logical array as a length
/// without allocating a population-sized collection of identical empty
/// boxes. Mixed and present arrays keep the ordinary dense representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultEvidenceRecord {
    id: RelationalResultEvidenceId,
    contribution: EvaluatedResultContribution,
    early_select: CanonicalOptionalSlots<ResultValue>,
    early_objectives: CanonicalOptionalSlots<i64>,
}

/// Canonical owned storage for a fixed-length array of optional staged values.
///
/// The dense variant is inhabited only when at least one slot is present.
/// This invariant makes derived equality representation-independent for every
/// value constructible through this module: an all-`None` dense input always
/// normalizes to `AllNone`.
#[derive(Clone, Debug, Eq, PartialEq)]
enum CanonicalOptionalSlots<T> {
    AllNone(usize),
    Dense(Box<[Option<T>]>),
}

impl<T> CanonicalOptionalSlots<T> {
    fn from_dense(values: Box<[Option<T>]>) -> Self {
        if values.iter().all(Option::is_none) {
            Self::AllNone(values.len())
        } else {
            Self::Dense(values)
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::AllNone(len) => *len,
            Self::Dense(values) => values.len(),
        }
    }

    fn get(&self, index: usize) -> Option<&T> {
        match self {
            Self::AllNone(_) => None,
            Self::Dense(values) => values.get(index).and_then(Option::as_ref),
        }
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = Option<&T>> + '_ {
        (0..self.len()).map(move |index| self.get(index))
    }

    fn semantically_eq_dense(&self, values: &[Option<T>]) -> bool
    where
        T: PartialEq,
    {
        self.len() == values.len()
            && self
                .iter()
                .zip(values)
                .all(|(left, right)| left == right.as_ref())
    }

    fn for_each_present_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        if let Self::Dense(values) = self {
            for value in values.iter_mut().flatten() {
                visit(value);
            }
        }
    }
}

impl<T: Clone> CanonicalOptionalSlots<T> {
    fn from_borrowed(values: &[Option<T>]) -> Self {
        if values.iter().all(Option::is_none) {
            Self::AllNone(values.len())
        } else {
            Self::Dense(values.to_vec().into_boxed_slice())
        }
    }

    fn materialize(&self) -> Box<[Option<T>]> {
        match self {
            Self::AllNone(len) => std::iter::repeat_with(|| None)
                .take(*len)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Self::Dense(values) => values.clone(),
        }
    }
}

impl RelationalResultEvidenceRecord {
    pub(super) fn restore_from_journal_codec(
        contribution: EvaluatedResultContribution,
        early_select: Box<[Option<ResultValue>]>,
        early_objectives: Box<[Option<i64>]>,
    ) -> Self {
        Self::derive(contribution, early_select, early_objectives)
    }

    pub(crate) fn from_evaluated(evidence: &RelationalResultEvidence) -> Self {
        let contribution = evidence.contribution().clone();
        let early_select = CanonicalOptionalSlots::from_borrowed(evidence.early_select());
        let early_objectives = CanonicalOptionalSlots::from_borrowed(evidence.early_objectives());
        debug_assert!(early_select.semantically_eq_dense(evidence.early_select()));
        debug_assert!(early_objectives.semantically_eq_dense(evidence.early_objectives()));
        Self::derive_canonical(contribution, early_select, early_objectives)
    }

    fn derive(
        contribution: EvaluatedResultContribution,
        early_select: Box<[Option<ResultValue>]>,
        early_objectives: Box<[Option<i64>]>,
    ) -> Self {
        Self::derive_canonical(
            contribution,
            CanonicalOptionalSlots::from_dense(early_select),
            CanonicalOptionalSlots::from_dense(early_objectives),
        )
    }

    fn derive_canonical(
        contribution: EvaluatedResultContribution,
        early_select: CanonicalOptionalSlots<ResultValue>,
        early_objectives: CanonicalOptionalSlots<i64>,
    ) -> Self {
        let id = derive_evidence_id(&contribution, &early_select, &early_objectives);
        Self {
            id,
            contribution,
            early_select,
            early_objectives,
        }
    }

    pub(crate) const fn id(&self) -> RelationalResultEvidenceId {
        self.id
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.contribution.view_id()
    }

    pub(crate) const fn row_id(&self) -> ResultViewInputRowId {
        self.contribution.row_id()
    }

    pub(crate) fn grain_values(&self) -> &[ResultValue] {
        self.contribution.group_values()
    }

    pub(crate) fn measures(&self) -> &[ResultValue] {
        self.contribution.measures()
    }

    pub(crate) fn distinct_arguments(&self) -> &[ResultValue] {
        self.contribution.distinct_arguments()
    }

    pub(crate) fn early_select_len(&self) -> usize {
        self.early_select.len()
    }

    pub(crate) fn early_select_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = Option<&ResultValue>> + '_ {
        self.early_select.iter()
    }

    /// Materialize staged SELECT slots only for the legacy row-state reducer,
    /// whose owned compatibility state genuinely requires a dense array.
    pub(crate) fn materialize_early_select(&self) -> Box<[Option<ResultValue>]> {
        self.early_select.materialize()
    }

    pub(crate) fn early_objectives_len(&self) -> usize {
        self.early_objectives.len()
    }

    pub(crate) fn early_objectives_iter(&self) -> impl ExactSizeIterator<Item = Option<&i64>> + '_ {
        self.early_objectives.iter()
    }

    pub(crate) const fn contribution(&self) -> &EvaluatedResultContribution {
        &self.contribution
    }

    /// Canonicalize process-local constructor backing without changing the
    /// evidence payload or its content-derived identity.
    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut super::ExploreValue),
    ) {
        self.contribution.canonicalize_value_storage(visitor);
        self.early_select
            .for_each_present_mut(|value| value.canonicalize_value_storage(visitor));
    }

    /// Replay exactly one contribution through the ordinary reducer's checked
    /// boundary. Cached projection values are consumed later by executor
    /// integration and do not alter reducer ingestion.
    pub(crate) fn replay_into(
        &self,
        reducer: &mut ResultViewBuilder,
    ) -> Result<bool, ResultEvidenceError> {
        reducer
            .insert(self.contribution.clone())
            .map_err(ResultEvidenceError::Reducer)
    }

    fn validate(&self) -> Result<(), ResultEvidenceError> {
        let derived = derive_evidence_id(
            &self.contribution,
            &self.early_select,
            &self.early_objectives,
        );
        if derived != self.id {
            return Err(ResultEvidenceError::EvidenceIdentityMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Canonically ordered owned checkpoint for one result-evidence catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultEvidenceSnapshot {
    pub(crate) version: u32,
    view_id: ViewId,
    input_kind: ResultViewInputKind,
    spec_root: ResultViewSpecRoot,
    root: RelationalResultEvidenceRoot,
    input_seal: Option<RelationalResultInputSeal>,
    records: Box<[RelationalResultEvidenceRecord]>,
}

impl RelationalResultEvidenceSnapshot {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn input_kind(&self) -> ResultViewInputKind {
        self.input_kind
    }

    pub(crate) const fn root(&self) -> RelationalResultEvidenceRoot {
        self.root
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn input_seal(&self) -> Option<RelationalResultInputSeal> {
        self.input_seal
    }

    pub(crate) const fn input_is_sealed(&self) -> bool {
        self.input_seal.is_some()
    }

    pub(crate) fn logical_len(&self) -> u128 {
        self.input_seal
            .map_or(self.records.len() as u128, |seal| seal.coverage.row_count)
    }

    pub(crate) fn records(&self) -> &[RelationalResultEvidenceRecord] {
        &self.records
    }

    pub(crate) fn record(
        &self,
        row_id: ResultViewInputRowId,
    ) -> Option<&RelationalResultEvidenceRecord> {
        self.records
            .binary_search_by_key(&row_id, RelationalResultEvidenceRecord::row_id)
            .ok()
            .map(|index| &self.records[index])
    }
}

/// Mutable, set-semantic result evidence for exactly one result view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultEvidenceCatalogBuilder {
    view_id: ViewId,
    input_kind: ResultViewInputKind,
    spec_root: ResultViewSpecRoot,
    input_seal: Option<RelationalResultInputSeal>,
    /// Process-local acceleration for the immutable sealed catalog. Open
    /// roots remain derived from their current row set; once the input seal
    /// lands no further row can be accepted, so the exact canonical root is
    /// stable and may be retained without entering the snapshot contract.
    sealed_root: Option<RelationalResultEvidenceRoot>,
    records: BTreeMap<ResultViewInputRowId, RelationalResultEvidenceRecord>,
    /// Operational collision index. The canonical row map already owns every
    /// row identity, so this secondary index retains only evidence-ID
    /// membership rather than another population-sized copy of each row ID.
    evidence_ids: BTreeSet<RelationalResultEvidenceId>,
}

impl RelationalResultEvidenceCatalogBuilder {
    pub(crate) fn new(spec: &ResultViewSpec) -> Result<Self, ResultEvidenceError> {
        spec.validate_spec_root()
            .map_err(|_| ResultEvidenceError::ResultSpecRootMismatch)?;
        Ok(Self {
            view_id: spec.view_id(),
            input_kind: spec.input_kind(),
            spec_root: spec.spec_root(),
            input_seal: None,
            sealed_root: None,
            records: BTreeMap::new(),
            evidence_ids: BTreeSet::new(),
        })
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn input_kind(&self) -> ResultViewInputKind {
        self.input_kind
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Logical population size. Proof-specialized source evidence deliberately
    /// has zero physical singleton records but retains its exact N in the seal.
    pub(crate) fn logical_len(&self) -> u128 {
        self.input_seal
            .map_or(self.records.len() as u128, |seal| seal.coverage.row_count)
    }

    pub(crate) const fn input_is_sealed(&self) -> bool {
        self.input_seal.is_some()
    }

    pub(crate) const fn input_seal(&self) -> Option<RelationalResultInputSeal> {
        self.input_seal
    }

    pub(crate) fn record(
        &self,
        row_id: ResultViewInputRowId,
    ) -> Option<&RelationalResultEvidenceRecord> {
        self.records.get(&row_id)
    }

    pub(crate) fn records(&self) -> impl Iterator<Item = &RelationalResultEvidenceRecord> + '_ {
        self.records.values()
    }

    /// Seek a canonical row suffix without walking the already-published
    /// prefix. The cursor is operational; the sealed catalog owns membership.
    pub(crate) fn records_after(
        &self,
        after: Option<ResultViewInputRowId>,
    ) -> impl Iterator<Item = &RelationalResultEvidenceRecord> + '_ {
        use std::ops::Bound::{Excluded, Unbounded};
        self.records
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .map(|(_, record)| record)
    }

    pub(crate) fn insert_evaluated(
        &mut self,
        evidence: &RelationalResultEvidence,
    ) -> Result<(RelationalResultEvidenceId, bool), ResultEvidenceError> {
        self.insert(RelationalResultEvidenceRecord::from_evaluated(evidence))
    }

    /// Equal rediscovery is idempotent, including after sealing. A different
    /// payload for an existing row or content identity fails before mutation.
    pub(crate) fn insert(
        &mut self,
        record: RelationalResultEvidenceRecord,
    ) -> Result<(RelationalResultEvidenceId, bool), ResultEvidenceError> {
        record.validate()?;
        if record.view_id() != self.view_id {
            return Err(ResultEvidenceError::WrongView {
                expected: self.view_id,
                actual: record.view_id(),
            });
        }
        if record.row_id().kind() != self.input_kind {
            return Err(ResultEvidenceError::WrongInputKind {
                expected: self.input_kind,
                actual: record.row_id().kind(),
            });
        }

        let id = record.id();
        let row_id = record.row_id();
        if self.evidence_ids.contains(&id) {
            return if self.records.get(&row_id) == Some(&record) {
                Ok((id, false))
            } else {
                Err(ResultEvidenceError::EvidenceIdentityCollision { id })
            };
        }
        if self.records.contains_key(&row_id) {
            return Err(ResultEvidenceError::RowEvidenceConflict { row_id });
        }
        if self.input_seal.is_some() {
            return Err(ResultEvidenceError::InputAlreadySealed);
        }

        self.evidence_ids.insert(id);
        self.records.insert(row_id, record);
        Ok((id, true))
    }

    /// Weighted/certified cells need a reducer algebra for multiplicity,
    /// distinctness, staging, and projection. Until that exists, callers must
    /// materialize exact singleton rows rather than submit a representative.
    pub(crate) fn insert_weighted_support_cell(
        &mut self,
        cell_id: SupportCellId,
        _exact_weight: u128,
    ) -> Result<bool, ResultEvidenceError> {
        Err(ResultEvidenceError::WeightedSupportCellUnsupported { cell_id })
    }

    /// Seal only against an explicit exact upstream/root-set commitment. The
    /// row count and canonical row-set root must match all accepted evidence.
    pub(crate) fn seal_input(
        &mut self,
        seal: RelationalResultInputSeal,
    ) -> Result<bool, ResultEvidenceError> {
        seal.validate()?;
        if seal.upstream.input_kind() != self.input_kind {
            return Err(ResultEvidenceError::InputSealKindMismatch {
                expected: self.input_kind,
                actual: seal.upstream.input_kind(),
            });
        }
        if matches!(
            seal.upstream,
            ResultEvidenceUpstreamRoot::CertifiedSources { .. }
        ) {
            if !self.records.is_empty() {
                return Err(ResultEvidenceError::CertifiedSourceEvidenceRowsForbidden);
            }
        } else {
            let actual = derive_coverage(self.input_kind, self.records.keys().copied())?;
            if seal.coverage != actual {
                return Err(ResultEvidenceError::InputCoverageMismatch {
                    expected_count: seal.coverage.row_count,
                    actual_count: actual.row_count,
                    expected_root: seal.coverage.row_set_root,
                    actual_root: actual.row_set_root,
                });
            }
        }
        match self.input_seal {
            Some(existing) if existing == seal => Ok(false),
            Some(_) => Err(ResultEvidenceError::InputSealConflict),
            None => {
                let sealed_root = derive_catalog_root(
                    self.view_id,
                    self.input_kind,
                    self.spec_root,
                    Some(seal),
                    self.records.values(),
                );
                self.input_seal = Some(seal);
                self.sealed_root = Some(sealed_root);
                Ok(true)
            }
        }
    }

    pub(crate) fn root(&self) -> RelationalResultEvidenceRoot {
        if let Some(root) = self.sealed_root {
            return root;
        }
        derive_catalog_root(
            self.view_id,
            self.input_kind,
            self.spec_root,
            self.input_seal,
            self.records.values(),
        )
    }

    pub(crate) fn snapshot(&self) -> RelationalResultEvidenceSnapshot {
        RelationalResultEvidenceSnapshot {
            version: RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION,
            view_id: self.view_id,
            input_kind: self.input_kind,
            spec_root: self.spec_root,
            root: self.root(),
            input_seal: self.input_seal,
            records: self
                .records
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    /// Consume this mutable catalog into its canonical checkpoint without
    /// cloning any retained evidence payload. The secondary evidence-id index
    /// is operational state only, so it can be dropped before the canonical
    /// row map is moved into snapshot order.
    pub(crate) fn into_snapshot(self) -> RelationalResultEvidenceSnapshot {
        let root = self.root();
        let Self {
            view_id,
            input_kind,
            spec_root,
            input_seal,
            sealed_root: _,
            records,
            evidence_ids,
        } = self;
        drop(evidence_ids);
        RelationalResultEvidenceSnapshot {
            version: RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION,
            view_id,
            input_kind,
            spec_root,
            root,
            input_seal,
            records: records.into_values().collect::<Vec<_>>().into_boxed_slice(),
        }
    }

    pub(crate) fn from_snapshot(
        snapshot: RelationalResultEvidenceSnapshot,
        spec: &ResultViewSpec,
    ) -> Result<Self, ResultEvidenceError> {
        let RelationalResultEvidenceSnapshot {
            version,
            view_id,
            input_kind,
            spec_root,
            root,
            input_seal,
            records,
        } = snapshot;
        if version != RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION {
            return Err(ResultEvidenceError::UnsupportedSnapshotVersion {
                actual: version,
                expected: RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION,
            });
        }

        spec.validate_spec_root()
            .map_err(|_| ResultEvidenceError::ResultSpecRootMismatch)?;
        if view_id != spec.view_id() {
            return Err(ResultEvidenceError::WrongView {
                expected: spec.view_id(),
                actual: view_id,
            });
        }
        if input_kind != spec.input_kind() {
            return Err(ResultEvidenceError::WrongInputKind {
                expected: spec.input_kind(),
                actual: input_kind,
            });
        }
        if spec_root != spec.spec_root() {
            return Err(ResultEvidenceError::ResultSpecRootMismatch);
        }

        let mut builder = Self::new(spec)?;
        let mut previous = None;
        for record in records.into_vec() {
            if previous.is_some_and(|row_id| record.row_id() <= row_id) {
                return Err(ResultEvidenceError::NonCanonicalSnapshotOrder);
            }
            previous = Some(record.row_id());
            builder.insert(record)?;
        }
        if let Some(input_seal) = input_seal {
            builder.seal_input(input_seal)?;
        }
        if builder.root() != root {
            return Err(ResultEvidenceError::SnapshotRootMismatch);
        }
        Ok(builder)
    }

    pub(crate) fn finish(self) -> Result<RelationalResultEvidenceCatalog, ResultEvidenceError> {
        self.validate_complete()?;
        Ok(RelationalResultEvidenceCatalog {
            snapshot: self.into_snapshot(),
        })
    }

    /// Materialize one immutable closed artifact without cloning this mutable
    /// builder and all of its indexes first.
    pub(crate) fn materialize_closed(
        &self,
    ) -> Result<RelationalResultEvidenceCatalog, ResultEvidenceError> {
        self.validate_complete()?;
        Ok(RelationalResultEvidenceCatalog {
            snapshot: self.snapshot(),
        })
    }

    /// Validate terminal closure without cloning the accumulated row maps.
    /// This is the preflight used by an append-only journal before consuming
    /// the enclosing analysis builder at its one terminal event.
    pub(crate) fn validate_complete(&self) -> Result<(), ResultEvidenceError> {
        if self.input_seal.is_none() {
            return Err(ResultEvidenceError::InputFrontierOpen);
        }
        Ok(())
    }
}

/// Immutable closed result evidence for one exact input population.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultEvidenceCatalog {
    snapshot: RelationalResultEvidenceSnapshot,
}

impl RelationalResultEvidenceCatalog {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.snapshot.view_id
    }

    pub(crate) const fn root(&self) -> RelationalResultEvidenceRoot {
        self.snapshot.root
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.snapshot.spec_root
    }

    pub(crate) fn input_seal(&self) -> RelationalResultInputSeal {
        match self.snapshot.input_seal {
            Some(seal) => seal,
            None => unreachable!("closed result evidence always has an input seal"),
        }
    }

    pub(crate) const fn snapshot(&self) -> &RelationalResultEvidenceSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultEvidenceError {
    WrongView {
        expected: ViewId,
        actual: ViewId,
    },
    WrongInputKind {
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    ResultSpecRootMismatch,
    EvidenceIdentityMismatch {
        claimed: RelationalResultEvidenceId,
        derived: RelationalResultEvidenceId,
    },
    EvidenceIdentityCollision {
        id: RelationalResultEvidenceId,
    },
    RowEvidenceConflict {
        row_id: ResultViewInputRowId,
    },
    InputAlreadySealed,
    InputSealKindMismatch {
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    InputCoverageRowKindMismatch {
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    InputCoverageMismatch {
        expected_count: u128,
        actual_count: u128,
        expected_root: ResultInputCoverageRoot,
        actual_root: ResultInputCoverageRoot,
    },
    InputSealConflict,
    InputFrontierOpen,
    WeightedSupportCellUnsupported {
        cell_id: SupportCellId,
    },
    CertifiedPopulationConcreteCoverageMismatch {
        expected: u128,
        actual: u128,
    },
    CertifiedSourceCoverageMismatch,
    CertifiedSourceEvidenceRowsForbidden,
    NonCanonicalSnapshotOrder,
    SnapshotRootMismatch,
    UnsupportedSnapshotVersion {
        actual: u32,
        expected: u32,
    },
    Reducer(ResultViewError),
}

impl fmt::Display for ResultEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongView { .. } => {
                formatter.write_str("result evidence belongs to another view")
            }
            Self::WrongInputKind { .. } => {
                formatter.write_str("result evidence has the wrong input-row identity kind")
            }
            Self::ResultSpecRootMismatch => formatter.write_str(
                "result evidence snapshot disagrees with the checked result-view spec root",
            ),
            Self::EvidenceIdentityMismatch { .. } => formatter
                .write_str("result evidence ID does not match its canonical evaluated payload"),
            Self::EvidenceIdentityCollision { .. } => formatter.write_str(
                "result evidence content-ID collision has unequal canonical payloads",
            ),
            Self::RowEvidenceConflict { .. } => {
                formatter.write_str("result input row has contradictory evaluated evidence")
            }
            Self::InputAlreadySealed => {
                formatter.write_str("result evidence input cannot grow after sealing")
            }
            Self::InputSealKindMismatch { .. } => formatter
                .write_str("result evidence seal names the wrong typed upstream relation"),
            Self::InputCoverageRowKindMismatch { .. } => formatter
                .write_str("result input coverage contains the wrong row identity kind"),
            Self::InputCoverageMismatch { .. } => formatter.write_str(
                "result evidence rows do not match the explicit upstream coverage commitment",
            ),
            Self::InputSealConflict => {
                formatter.write_str("result evidence has conflicting upstream closure receipts")
            }
            Self::InputFrontierOpen => formatter
                .write_str("result evidence cannot finish without an upstream closure receipt"),
            Self::WeightedSupportCellUnsupported { .. } => formatter.write_str(
                "weighted support-cell result contributions are not yet modeled and cannot be accepted",
            ),
            Self::CertifiedPopulationConcreteCoverageMismatch { .. } => formatter.write_str(
                "concrete selected cases do not exactly cover the certified support population",
            ),
            Self::CertifiedSourceCoverageMismatch => formatter.write_str(
                "certified source result coverage does not match its proof artifact and exact population",
            ),
            Self::CertifiedSourceEvidenceRowsForbidden => formatter.write_str(
                "certified source result input cannot be mixed with concrete singleton-row evidence",
            ),
            Self::NonCanonicalSnapshotOrder => {
                formatter.write_str("result evidence snapshot rows are not strictly ordered")
            }
            Self::SnapshotRootMismatch => formatter.write_str(
                "result evidence snapshot root does not authenticate its canonical catalog",
            ),
            Self::UnsupportedSnapshotVersion { actual, expected } => write!(
                formatter,
                "unsupported result evidence snapshot version {actual}; expected {expected}"
            ),
            Self::Reducer(error) => error.fmt(formatter),
        }
    }
}

impl Error for ResultEvidenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Reducer(error) => Some(error),
            _ => None,
        }
    }
}

fn derive_evidence_id(
    contribution: &EvaluatedResultContribution,
    early_select: &CanonicalOptionalSlots<ResultValue>,
    early_objectives: &CanonicalOptionalSlots<i64>,
) -> RelationalResultEvidenceId {
    let mut hasher = CanonicalHasher::new(RESULT_EVIDENCE_ID_V1);
    hasher.digest(contribution.view_id().bytes());
    hash_row_id(&mut hasher, contribution.row_id());
    hash_values(&mut hasher, contribution.group_values());
    hash_values(&mut hasher, contribution.measures());
    hash_values(&mut hasher, contribution.distinct_arguments());
    hash_optional_values(&mut hasher, early_select);
    hash_optional_i64s(&mut hasher, early_objectives);
    RelationalResultEvidenceId(hasher.finish())
}

fn derive_coverage(
    input_kind: ResultViewInputKind,
    rows: impl IntoIterator<Item = ResultViewInputRowId>,
) -> Result<ResultInputCoverageCommitment, ResultEvidenceError> {
    let rows = rows.into_iter().collect::<BTreeSet<_>>();
    let mut hasher = CanonicalHasher::new(RESULT_INPUT_COVERAGE_ROOT_V1);
    hash_input_kind(&mut hasher, input_kind);
    hasher.u128(rows.len() as u128);
    for row_id in &rows {
        if row_id.kind() != input_kind {
            return Err(ResultEvidenceError::InputCoverageRowKindMismatch {
                expected: input_kind,
                actual: row_id.kind(),
            });
        }
        hash_row_id(&mut hasher, *row_id);
    }
    Ok(ResultInputCoverageCommitment {
        input_kind,
        row_count: rows.len() as u128,
        row_set_root: ResultInputCoverageRoot(hasher.finish()),
    })
}

fn derive_canonical_selected_coverage(
    question: &ClosedQuestionCatalogRef<'_>,
) -> ResultInputCoverageCommitment {
    let input_kind = ResultViewInputKind::Case;
    let mut hasher = CanonicalHasher::new(RESULT_INPUT_COVERAGE_ROOT_V1);
    hash_input_kind(&mut hasher, input_kind);
    hasher.u128(question.selected_count());
    for case_id in question.selected_case_ids() {
        hash_row_id(&mut hasher, ResultViewInputRowId::Case(case_id));
    }
    ResultInputCoverageCommitment {
        input_kind,
        row_count: question.selected_count(),
        row_set_root: ResultInputCoverageRoot(hasher.finish()),
    }
}

fn derive_canonical_mechanism_coverage(
    expected_row_count: u128,
    terminals: impl IntoIterator<Item = MechanismCaseTerminalRecord>,
) -> Result<ResultInputCoverageCommitment, ResultEvidenceError> {
    let input_kind = ResultViewInputKind::Incidence;
    let mut hasher = CanonicalHasher::new(RESULT_INPUT_COVERAGE_ROOT_V1);
    hash_input_kind(&mut hasher, input_kind);
    hasher.u128(expected_row_count);
    let mut previous = None;
    let mut actual_row_count = 0u128;
    for terminal in terminals {
        let MechanismCaseTerminal::Incidence {
            transition_id,
            signature_id,
        } = terminal.terminal()
        else {
            continue;
        };
        let row_id = MechanismIncidenceRowId::new(terminal.case_id(), transition_id, signature_id);
        if previous.is_some_and(|previous| previous >= row_id) {
            return Err(ResultEvidenceError::NonCanonicalSnapshotOrder);
        }
        hash_row_id(&mut hasher, ResultViewInputRowId::Incidence(row_id));
        previous = Some(row_id);
        actual_row_count += 1;
    }
    if actual_row_count != expected_row_count {
        return Err(ResultEvidenceError::NonCanonicalSnapshotOrder);
    }
    Ok(ResultInputCoverageCommitment {
        input_kind,
        row_count: actual_row_count,
        row_set_root: ResultInputCoverageRoot(hasher.finish()),
    })
}

fn derive_canonical_source_coverage(
    source_keys: impl ExactSizeIterator<Item = SourceKey>,
) -> ResultInputCoverageCommitment {
    let input_kind = ResultViewInputKind::Source;
    let row_count = source_keys.len() as u128;
    let mut hasher = CanonicalHasher::new(RESULT_INPUT_COVERAGE_ROOT_V1);
    hash_input_kind(&mut hasher, input_kind);
    hasher.u128(row_count);
    for source_key in source_keys {
        hash_row_id(&mut hasher, ResultViewInputRowId::Source(source_key));
    }
    ResultInputCoverageCommitment {
        input_kind,
        row_count,
        row_set_root: ResultInputCoverageRoot(hasher.finish()),
    }
}

fn derive_certified_source_coverage(
    upstream: ResultEvidenceUpstreamRoot,
) -> ResultInputCoverageCommitment {
    let ResultEvidenceUpstreamRoot::CertifiedSources {
        relation_id,
        population_root,
        summary_artifact_id,
        certified_input_root,
        exact_cardinality,
    } = upstream
    else {
        unreachable!("certified source coverage is derived only for its typed upstream")
    };
    let mut hasher = CanonicalHasher::new(CERTIFIED_SOURCE_INPUT_COVERAGE_ROOT_V1);
    hash_input_kind(&mut hasher, ResultViewInputKind::Source);
    hasher.digest(relation_id.bytes());
    hasher.digest(population_root.bytes());
    hasher.digest(summary_artifact_id.bytes());
    hasher.digest(certified_input_root.bytes());
    hasher.u128(exact_cardinality);
    ResultInputCoverageCommitment {
        input_kind: ResultViewInputKind::Source,
        row_count: exact_cardinality,
        row_set_root: ResultInputCoverageRoot(hasher.finish()),
    }
}

fn derive_catalog_root<'a>(
    view_id: ViewId,
    input_kind: ResultViewInputKind,
    spec_root: ResultViewSpecRoot,
    input_seal: Option<RelationalResultInputSeal>,
    records: impl IntoIterator<Item = &'a RelationalResultEvidenceRecord>,
) -> RelationalResultEvidenceRoot {
    let records = records.into_iter().collect::<Vec<_>>();
    let mut hasher = CanonicalHasher::new(RESULT_EVIDENCE_ROOT_V3);
    hasher.u32(RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION);
    hasher.digest(view_id.bytes());
    hash_input_kind(&mut hasher, input_kind);
    hasher.digest(spec_root.bytes());
    match input_seal {
        None => hasher.tag(0x00),
        Some(seal) => {
            hasher.tag(0x01);
            hash_input_seal(&mut hasher, seal);
        }
    }
    hasher.u128(records.len() as u128);
    for record in records {
        hash_row_id(&mut hasher, record.row_id());
        hasher.digest(record.id().bytes());
    }
    RelationalResultEvidenceRoot(hasher.finish())
}

fn hash_input_seal(hasher: &mut CanonicalHasher, seal: RelationalResultInputSeal) {
    match seal.upstream {
        ResultEvidenceUpstreamRoot::Sources {
            relation_id,
            source_key_root,
        } => {
            hasher.tag(0x04);
            hasher.digest(relation_id.bytes());
            hasher.digest(source_key_root.bytes());
        }
        ResultEvidenceUpstreamRoot::CertifiedSources {
            relation_id,
            population_root,
            summary_artifact_id,
            certified_input_root,
            exact_cardinality,
        } => {
            hasher.tag(0x05);
            hasher.digest(relation_id.bytes());
            hasher.digest(population_root.bytes());
            hasher.digest(summary_artifact_id.bytes());
            hasher.digest(certified_input_root.bytes());
            hasher.u128(exact_cardinality);
        }
        ResultEvidenceUpstreamRoot::Selected {
            question_id,
            content_root,
        } => {
            hasher.tag(0x01);
            hasher.digest(question_id.bytes());
            hasher.digest(content_root.bytes());
        }
        ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
            question_id,
            population_root,
            exact_cardinality,
        } => {
            hasher.tag(0x03);
            hasher.digest(question_id.bytes());
            hasher.digest(population_root.bytes());
            hasher.u128(exact_cardinality);
        }
        ResultEvidenceUpstreamRoot::Choice {
            choice_id,
            content_root,
        } => {
            hasher.tag(0x07);
            hasher.digest(choice_id.bytes());
            hasher.digest(content_root.bytes());
        }
        ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id,
            completed_root,
        } => {
            hasher.tag(0x02);
            hasher.digest(request_id.bytes());
            hasher.digest(completed_root.bytes());
        }
        ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
            request_id,
            completed_root,
            structural_root,
        } => {
            hasher.tag(0x06);
            hasher.digest(request_id.bytes());
            hasher.digest(completed_root.bytes());
            hasher.digest(structural_root.bytes());
        }
    }
    hash_input_kind(hasher, seal.coverage.input_kind);
    hasher.u128(seal.coverage.row_count);
    hasher.digest(seal.coverage.row_set_root.bytes());
}

fn hash_input_kind(hasher: &mut CanonicalHasher, input_kind: ResultViewInputKind) {
    hasher.tag(match input_kind {
        ResultViewInputKind::Source => 0x03,
        ResultViewInputKind::Case => 0x01,
        ResultViewInputKind::Incidence => 0x02,
    });
}

fn hash_row_id(hasher: &mut CanonicalHasher, row_id: ResultViewInputRowId) {
    match row_id {
        ResultViewInputRowId::Source(source_key) => {
            hasher.tag(0x03);
            hasher.digest(source_key.bytes());
        }
        ResultViewInputRowId::Case(case_id) => {
            hasher.tag(0x01);
            hasher.digest(case_id.bytes());
        }
        ResultViewInputRowId::Incidence(incidence) => {
            hasher.tag(0x02);
            hasher.digest(incidence.case_id().bytes());
            hasher.digest(incidence.transition_id().bytes());
            hasher.digest(incidence.signature_id().request_id().bytes());
            hasher.digest(incidence.signature_id().bytes());
        }
    }
}

fn hash_values(hasher: &mut CanonicalHasher, values: &[ResultValue]) {
    hasher.u128(values.len() as u128);
    for value in values.iter() {
        hash_value(hasher, value);
    }
}

fn hash_value(hasher: &mut CanonicalHasher, value: &ResultValue) {
    match value {
        ResultValue::Value(value) => {
            hasher.tag(0x01);
            hasher.digest(canonical_explore_value_digest(value));
        }
        ResultValue::CaseId(case_id) => {
            hasher.tag(0x02);
            hasher.digest(case_id.bytes());
        }
        ResultValue::TransitionId(transition_id) => {
            hasher.tag(0x03);
            hasher.digest(transition_id.bytes());
        }
        ResultValue::SignatureId(signature_id) => {
            hasher.tag(0x04);
            hasher.digest(signature_id.request_id().bytes());
            hasher.digest(signature_id.bytes());
        }
        ResultValue::StructuralMechanismId(mechanism_id) => {
            hasher.tag(0x05);
            hasher.digest(mechanism_id.bytes());
        }
        ResultValue::ExecutionProfileId(profile_id) => {
            hasher.tag(0x06);
            hasher.digest(profile_id.bytes());
        }
    }
}

fn hash_optional_values(
    hasher: &mut CanonicalHasher,
    values: &CanonicalOptionalSlots<ResultValue>,
) {
    hasher.u128(values.len() as u128);
    for value in values.iter() {
        match value {
            None => hasher.tag(0x00),
            Some(value) => {
                hasher.tag(0x01);
                hash_value(hasher, value);
            }
        }
    }
}

fn hash_optional_i64s(hasher: &mut CanonicalHasher, values: &CanonicalOptionalSlots<i64>) {
    hasher.u128(values.len() as u128);
    for value in values.iter() {
        match value {
            None => hasher.tag(0x00),
            Some(value) => {
                hasher.tag(0x01);
                hasher.i64(*value);
            }
        }
    }
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
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

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relation::{
        AdmissionCatalogBuilder, AdmissionDecision, AdmissionId, FindPolarity, MechanismTargetId,
        QuestionCatalogBuilder, RelationCatalogBuilder, RelationId, RelationProvenance,
        RelationalCaseId, SelectionDecision, SourceKey, SourceRow, SuccessorKey, SuccessorRow,
        ViewInputId,
    };
    use crate::explore::result_view::{ResultViewGrain, ResultViewSpec};
    use crate::explore::support_cell::{
        SupportCell, SupportCellSpace, SupportExpr, SupportMaterializerId, SupportProducerId,
    };
    use crate::explore::ExploreValue;

    fn identities(label: &[u8]) -> (RelationId, AdmissionId, QuestionId, ViewId) {
        let relation_id = RelationId::from_canonical_semantic_preimage(label);
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"admission");
        let question_id =
            QuestionId::from_canonical_find_preimage(admission_id, b"question", FindPolarity::All);
        let view_id =
            ViewId::from_canonical_view_preimage(ViewInputId::Selected(question_id), b"view");
        (relation_id, admission_id, question_id, view_id)
    }

    fn case_id(relation_id: RelationId, label: &str, after: i64) -> RelationalCaseId {
        let source_row = SourceRow::new(
            ExploreValue::String(label.to_string()),
            ExploreValue::Int(0),
            RelationProvenance::new([], []),
        );
        let source_key = SourceKey::derive(relation_id, &source_row);
        let successor_row =
            SuccessorRow::new(ExploreValue::Int(after), RelationProvenance::new([], []));
        let successor_key = SuccessorKey::derive(relation_id, source_key, &successor_row);
        RelationalCaseId::derive(relation_id, source_key, successor_key)
    }

    fn record(
        view_id: ViewId,
        case_id: RelationalCaseId,
        value: i64,
    ) -> RelationalResultEvidenceRecord {
        let value = ResultValue::Value(ExploreValue::Int(value));
        let objective = value_as_i64(value.clone());
        RelationalResultEvidenceRecord::derive(
            EvaluatedResultContribution::new(
                view_id,
                ResultViewInputRowId::Case(case_id),
                Box::<[ResultValue]>::default(),
                vec![value.clone()].into_boxed_slice(),
                Box::<[ResultValue]>::default(),
            ),
            vec![Some(value)].into_boxed_slice(),
            vec![Some(objective)].into_boxed_slice(),
        )
    }

    fn value_as_i64(value: ResultValue) -> i64 {
        match value {
            ResultValue::Value(ExploreValue::Int(value)) => value,
            _ => unreachable!(),
        }
    }

    fn legacy_dense_evidence_id(
        contribution: &EvaluatedResultContribution,
        early_select: &[Option<ResultValue>],
        early_objectives: &[Option<i64>],
    ) -> RelationalResultEvidenceId {
        let mut hasher = CanonicalHasher::new(RESULT_EVIDENCE_ID_V1);
        hasher.digest(contribution.view_id().bytes());
        hash_row_id(&mut hasher, contribution.row_id());
        hash_values(&mut hasher, contribution.group_values());
        hash_values(&mut hasher, contribution.measures());
        hash_values(&mut hasher, contribution.distinct_arguments());
        hasher.u128(early_select.len() as u128);
        for value in early_select {
            match value {
                None => hasher.tag(0x00),
                Some(value) => {
                    hasher.tag(0x01);
                    hash_value(&mut hasher, value);
                }
            }
        }
        hasher.u128(early_objectives.len() as u128);
        for value in early_objectives {
            match value {
                None => hasher.tag(0x00),
                Some(value) => {
                    hasher.tag(0x01);
                    hasher.i64(*value);
                }
            }
        }
        RelationalResultEvidenceId(hasher.finish())
    }

    fn case_spec(view_id: ViewId) -> ResultViewSpec {
        case_spec_with_projection(view_id, "selected")
    }

    fn case_spec_with_projection(view_id: ViewId, projection_name: &str) -> ResultViewSpec {
        ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::EachCase,
            vec![Box::<str>::from("measure")].into_boxed_slice(),
            Box::new([]),
            vec![Box::<str>::from(projection_name)].into_boxed_slice(),
            None,
            None,
        )
        .unwrap()
    }

    fn closed_question() -> (QuestionCatalog, Box<[RelationalCaseId]>) {
        let (relation_id, admission_id, question_id, _) = identities(b"closed-question");
        let mut relation = RelationCatalogBuilder::new(relation_id);
        let source_key = relation
            .insert_source(SourceRow::new(
                ExploreValue::String("profile".to_string()),
                ExploreValue::Int(0),
                RelationProvenance::new([], []),
            ))
            .unwrap();
        let mut cases = Vec::new();
        for after in [1, 2] {
            cases.push(
                relation
                    .insert_successor(
                        source_key,
                        SuccessorRow::new(
                            ExploreValue::Int(after),
                            RelationProvenance::new([], []),
                        ),
                    )
                    .unwrap()
                    .1,
            );
        }
        let open_relation = relation.snapshot();
        let mut admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        let mut question = QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        for case_id in &cases {
            admission
                .classify(&open_relation, *case_id, AdmissionDecision::Admitted)
                .unwrap();
            question
                .classify(
                    &open_relation,
                    &admission,
                    *case_id,
                    SelectionDecision::Selected,
                )
                .unwrap();
        }
        relation.seal_successor_enumeration(source_key).unwrap();
        relation.seal_source_enumeration();
        let relation = relation.finish().unwrap();
        let admission = admission.finish(&relation).unwrap();
        let question = question.finish(&relation, &admission).unwrap();
        (question, cases.into_boxed_slice())
    }

    #[test]
    fn structural_incidence_seal_identity_binds_the_quotient_root() {
        let (_, _, question_id, _) = identities(b"structural-incidence-seal");
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"observation",
            b"normalization",
        );
        let raw = RelationalResultInputSeal::from_canonical_mechanism_terminals(
            request_id,
            MechanismIncidenceRoot::from_journal_codec_bytes([0x11; 32]),
            0,
            std::iter::empty::<MechanismCaseTerminalRecord>(),
        )
        .expect("empty exact incidence seal");
        let left = raw
            .with_structural_quotient(StructuralQuotientClosureRoot::from_journal_codec_bytes(
                [0x21; 32],
            ))
            .expect("left structural seal");
        let right = raw
            .with_structural_quotient(StructuralQuotientClosureRoot::from_journal_codec_bytes(
                [0x22; 32],
            ))
            .expect("right structural seal");

        let digest = |seal| {
            let mut hasher = CanonicalHasher::new(b"structural-incidence-seal-test");
            hash_input_seal(&mut hasher, seal);
            hasher.finish()
        };
        assert_ne!(left, right);
        assert_ne!(digest(left), digest(right));
        assert_ne!(digest(raw), digest(left));
    }

    #[test]
    fn optional_staging_is_canonical_without_changing_dense_identity_semantics() {
        let (relation_id, _, _, view_id) = identities(b"canonical-optional-staging");
        let contribution = EvaluatedResultContribution::new(
            view_id,
            ResultViewInputRowId::Case(case_id(relation_id, "row", 1)),
            Box::<[ResultValue]>::default(),
            Box::<[ResultValue]>::default(),
            Box::<[ResultValue]>::default(),
        );
        let all_none_select = vec![None, None, None, None, None, None].into_boxed_slice();
        let all_none_objectives = vec![None, None].into_boxed_slice();
        let all_none = RelationalResultEvidenceRecord::derive(
            contribution.clone(),
            all_none_select.clone(),
            all_none_objectives.clone(),
        );
        assert!(matches!(
            &all_none.early_select,
            CanonicalOptionalSlots::AllNone(6)
        ));
        assert!(matches!(
            &all_none.early_objectives,
            CanonicalOptionalSlots::AllNone(2)
        ));
        assert!(all_none
            .early_select
            .semantically_eq_dense(&all_none_select));
        assert_eq!(all_none.early_select.materialize(), all_none_select);
        assert_eq!(
            all_none.id(),
            legacy_dense_evidence_id(&contribution, &all_none_select, &all_none_objectives,)
        );

        let present = ResultValue::Value(ExploreValue::Int(42));
        let dense_select = vec![None, Some(present.clone()), None].into_boxed_slice();
        let dense = RelationalResultEvidenceRecord::derive(
            contribution.clone(),
            dense_select.clone(),
            Box::new([]),
        );
        assert!(matches!(
            &dense.early_select,
            CanonicalOptionalSlots::Dense(_)
        ));
        assert_eq!(dense.early_select.get(1), Some(&present));
        assert!(dense.early_select.semantically_eq_dense(&dense_select));
        assert_eq!(
            dense.id(),
            legacy_dense_evidence_id(&contribution, &dense_select, &[])
        );

        let restored = RelationalResultEvidenceRecord::restore_from_journal_codec(
            contribution,
            all_none_select,
            all_none_objectives,
        );
        assert!(matches!(
            &restored.early_select,
            CanonicalOptionalSlots::AllNone(6)
        ));
        assert_eq!(restored, all_none);
    }

    #[test]
    fn arrival_order_converges_but_equal_counts_do_not_hide_membership() {
        let (relation_id, _, _, view_id) = identities(b"arrival-order");
        let first_case = case_id(relation_id, "first", 1);
        let second_case = case_id(relation_id, "second", 2);
        let third_case = case_id(relation_id, "third", 3);
        let first_record = record(view_id, first_case, 10);
        let second_record = record(view_id, second_case, 20);
        let third_record = record(view_id, third_case, 20);
        let spec = case_spec(view_id);

        let mut forward = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        forward.insert(first_record.clone()).unwrap();
        forward.insert(second_record.clone()).unwrap();
        let divergent_spec = case_spec_with_projection(view_id, "other_selected");
        let mut divergent = RelationalResultEvidenceCatalogBuilder::new(&divergent_spec).unwrap();
        divergent.insert(first_record.clone()).unwrap();
        divergent.insert(second_record.clone()).unwrap();
        assert_ne!(forward.root(), divergent.root());
        let mut reverse = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        reverse.insert(second_record).unwrap();
        reverse.insert(first_record.clone()).unwrap();
        assert_eq!(forward.root(), reverse.root());
        assert_eq!(forward.snapshot(), reverse.snapshot());

        let mut different_membership = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        different_membership.insert(first_record).unwrap();
        different_membership.insert(third_record).unwrap();
        assert_eq!(forward.len(), different_membership.len());
        assert_ne!(forward.root(), different_membership.root());
    }

    #[test]
    fn exact_repeat_is_idempotent_conflict_is_rejected_and_payload_replays() {
        let (relation_id, _, _, view_id) = identities(b"idempotence");
        let case_id = case_id(relation_id, "row", 1);
        let evidence = record(view_id, case_id, 10);
        let spec = case_spec(view_id);
        let mut catalog = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        assert!(catalog.insert(evidence.clone()).unwrap().1);
        assert!(!catalog.insert(evidence.clone()).unwrap().1);
        assert!(matches!(
            catalog.insert(record(view_id, case_id, 11)),
            Err(ResultEvidenceError::RowEvidenceConflict { row_id })
                if row_id == ResultViewInputRowId::Case(case_id)
        ));

        let mut reducer = ResultViewBuilder::new(spec);
        assert!(evidence.replay_into(&mut reducer).unwrap());
        assert!(!evidence.replay_into(&mut reducer).unwrap());
    }

    #[test]
    fn seal_requires_exact_closed_upstream_coverage_and_survives_resume() {
        let (question, cases) = closed_question();
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question.question_id()),
            b"sealed-view",
        );
        let seal = RelationalResultInputSeal::from_selected(&question).unwrap();
        let spec = case_spec(view_id);
        let mut catalog = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        catalog.insert(record(view_id, cases[0], 1)).unwrap();
        assert!(matches!(
            catalog.seal_input(seal),
            Err(ResultEvidenceError::InputCoverageMismatch {
                expected_count: 2,
                actual_count: 1,
                ..
            })
        ));
        catalog.insert(record(view_id, cases[1], 2)).unwrap();
        assert!(catalog.seal_input(seal).unwrap());
        assert!(!catalog.seal_input(seal).unwrap());
        assert!(matches!(
            catalog.insert(record(
                view_id,
                case_id(question.relation_id(), "late", 3),
                3
            )),
            Err(ResultEvidenceError::InputAlreadySealed)
        ));

        let snapshot = catalog.snapshot();
        let divergent_spec = case_spec_with_projection(view_id, "different_selected");
        assert!(matches!(
            RelationalResultEvidenceCatalogBuilder::from_snapshot(
                snapshot.clone(),
                &divergent_spec,
            ),
            Err(ResultEvidenceError::ResultSpecRootMismatch)
        ));
        let resumed =
            RelationalResultEvidenceCatalogBuilder::from_snapshot(snapshot.clone(), &spec).unwrap();
        assert_eq!(resumed.snapshot(), snapshot);
        assert_eq!(catalog.finish().unwrap().root(), snapshot.root());
    }

    #[test]
    fn claimed_record_id_and_weighted_support_cells_fail_closed() {
        let (relation_id, _, _, view_id) = identities(b"fail-closed");
        let mut forged = record(view_id, case_id(relation_id, "row", 1), 10);
        forged.id = RelationalResultEvidenceId([0x55; 32]);
        let spec = case_spec(view_id);
        let mut catalog = RelationalResultEvidenceCatalogBuilder::new(&spec).unwrap();
        assert!(matches!(
            catalog.insert(forged),
            Err(ResultEvidenceError::EvidenceIdentityMismatch { .. })
        ));

        let cell = SupportCell::new(
            SupportCellSpace::ProducerCoordinates(SupportProducerId::from_canonical_preimage(
                b"producer",
            )),
            SupportExpr::ordinal_interval(0, 2).unwrap(),
            SupportMaterializerId::from_canonical_preimage(b"materializer"),
        )
        .unwrap();
        assert!(matches!(
            catalog.insert_weighted_support_cell(cell.id(), 2),
            Err(ResultEvidenceError::WeightedSupportCellUnsupported { cell_id })
                if cell_id == cell.id()
        ));
    }
}
