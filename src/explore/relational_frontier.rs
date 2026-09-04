//! Content-addressed work DAG for resumable relational Explore execution.
//!
//! This module models semantic work and monotone progress only. It contains no
//! global scheduler phase, worker assignment, priority, resource budget, retry
//! timing, or other scheduler policy. Those choices may select an open node,
//! but they may never rename one or alter its durable cursor.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use sha2::{Digest, Sha256};

use super::relation::{
    AdmissionDecision, AdmissionId, MechanismRequestId, QuestionId, RelationId, RelationalCaseId,
    SelectionDecision, SourceKey, ViewId,
};
use super::relational_case_executor::SuccessorFiberExhaustionReceiptId;
use super::relational_executor::SourceBindingExhaustionReceiptId;
use super::support_cell::{SupportCellEvidenceId, SupportCellId, SupportProofObligationId};
use super::support_evidence::SupportObligationRefinementId;
use super::transition::canonical_explore_value_digest;
use super::ExploreValue;

const WORK_NODE_ID_V2: &[u8] = b"futuruna.explore.relational-work-node.v2";
const SOURCE_PREFIX_ID_V1: &[u8] = b"futuruna.explore.source-prefix.v1";
const WORK_COMPLETION_REF_ID_V1: &[u8] = b"futuruna.explore.work-completion-ref.v1";
const WORK_FRONTIER_ROOT_V2: &[u8] = b"futuruna.explore.relational-frontier-root.v2";
const WORK_FRONTIER_COMPACTION_ID_V1: &[u8] = b"futuruna.explore.relational-frontier-compaction.v1";
const WORK_FRONTIER_COMPACTED_IDS_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-frontier-compacted-ids.v1";

/// One checkpoint frame may discard at most this many dead work records. The
/// hard bound keeps receipt preparation and replay allocation independent of
/// an attacker-controlled journal field.
pub(crate) const WORK_FRONTIER_MAX_COMPACTION_NODES: u32 = 65_536;

pub(crate) const RELATIONAL_FRONTIER_SNAPSHOT_VERSION: u32 = 3;

/// Content identity of one semantic work node and its dependency set.
///
/// Progress cursors are deliberately absent: advancing a node must not rename
/// it. Scheduler priority and resource policy do not appear anywhere in this
/// module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkNodeId([u8; 32]);

impl WorkNodeId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity derived internally from one accepted typed completion.
/// Arbitrary bytes cannot mint completion authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkEvidenceId([u8; 32]);

impl WorkEvidenceId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated content of a canonical work-frontier snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkFrontierRoot([u8; 32]);

impl WorkFrontierRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Deterministic receipt for removing a bounded prefix of completed leaf work.
///
/// Compaction changes only the operational checkpoint projection. Every
/// answer-defining row, classification, proof, and analysis event remains in
/// its semantic catalog and in the outer journal chain. The removed IDs are
/// not accepted from the caller: replay derives the same canonical leaf set
/// from the current frontier and compares this receipt before mutating it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkFrontierCompaction {
    id: [u8; 32],
    maximum_nodes: NonZeroU32,
    removed_nodes: u32,
    removed_ids_root: [u8; 32],
    before_root: WorkFrontierRoot,
    after_root: WorkFrontierRoot,
}

impl WorkFrontierCompaction {
    pub(super) fn restore_from_journal_codec(
        maximum_nodes: NonZeroU32,
        removed_nodes: u32,
        removed_ids_root: [u8; 32],
        before_root: [u8; 32],
        after_root: [u8; 32],
    ) -> Result<Self, WorkFrontierError> {
        if maximum_nodes.get() > WORK_FRONTIER_MAX_COMPACTION_NODES
            || removed_nodes == 0
            || removed_nodes > maximum_nodes.get()
        {
            return Err(WorkFrontierError::InvalidCompactionLimit(
                maximum_nodes.get(),
            ));
        }
        let before_root = WorkFrontierRoot(before_root);
        let after_root = WorkFrontierRoot(after_root);
        let id = derive_compaction_id(
            maximum_nodes,
            removed_nodes,
            removed_ids_root,
            before_root,
            after_root,
        );
        Ok(Self {
            id,
            maximum_nodes,
            removed_nodes,
            removed_ids_root,
            before_root,
            after_root,
        })
    }

    pub(crate) const fn id(self) -> [u8; 32] {
        self.id
    }

    pub(crate) const fn maximum_nodes(self) -> NonZeroU32 {
        self.maximum_nodes
    }

    pub(crate) const fn removed_nodes(self) -> u32 {
        self.removed_nodes
    }

    pub(crate) const fn removed_ids_root(self) -> [u8; 32] {
        self.removed_ids_root
    }

    pub(crate) const fn before_root(self) -> WorkFrontierRoot {
        self.before_root
    }

    pub(crate) const fn after_root(self) -> WorkFrontierRoot {
        self.after_root
    }
}

/// Ordered source values already bound before expanding the next dependent
/// binding. Values are retained for deterministic resume; the digest is a
/// collision-checked compact identity, not a lossy replacement for them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalSourcePrefix {
    values: Box<[ExploreValue]>,
    digest: [u8; 32],
}

impl CanonicalSourcePrefix {
    pub(crate) fn empty() -> Self {
        Self::from_values(Vec::new()).expect("an empty source prefix has canonical length")
    }

    pub(crate) fn from_values(values: Vec<ExploreValue>) -> Result<Self, WorkFrontierError> {
        let count = u64::try_from(values.len())
            .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("source prefix"))?;
        let mut hasher = CanonicalHasher::new(SOURCE_PREFIX_ID_V1);
        hasher.u64(count);
        for value in &values {
            hasher.digest(canonical_explore_value_digest(value));
        }
        Ok(Self {
            values: values.into_boxed_slice(),
            digest: hasher.finish(),
        })
    }

    pub(crate) fn values(&self) -> &[ExploreValue] {
        &self.values
    }

    pub(crate) const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    fn validate(&self) -> Result<(), WorkFrontierError> {
        let reconstructed = Self::from_values(self.values.to_vec())?;
        if reconstructed.digest != self.digest {
            return Err(WorkFrontierError::SourcePrefixDigestMismatch);
        }
        Ok(())
    }
}

/// Typed durable reference that can close one supported semantic work kind.
///
/// Every variant repeats the scheduled subject fields, so the frontier can
/// reject a valid evidence ID attached to the wrong layer, case, source, cell,
/// or obligation. Referenced catalog records are validated by journal/catalog
/// integration; this module validates only reference and cursor shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkCompletionRef {
    SourcePrefixReady {
        relation_id: RelationId,
        binding_index: u32,
        prefix: CanonicalSourcePrefix,
    },
    SourceRowReady {
        relation_id: RelationId,
        source_key: SourceKey,
    },
    CaseReady {
        case_id: RelationalCaseId,
    },
    SupportCellReady {
        cell_id: SupportCellId,
    },
    SourceBindingExhausted {
        relation_id: RelationId,
        binding_index: u32,
        prefix: CanonicalSourcePrefix,
        terminal_ordinal: u128,
        receipt_id: SourceBindingExhaustionReceiptId,
    },
    SuccessorsSealed {
        relation_id: RelationId,
        source_key: SourceKey,
        terminal_ordinal: u128,
        receipt_id: SuccessorFiberExhaustionReceiptId,
    },
    AdmissionDecided {
        admission_id: AdmissionId,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    },
    FindDecided {
        question_id: QuestionId,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    },
    DirectSupportEvidence {
        cell_id: SupportCellId,
        obligation_id: SupportProofObligationId,
        evidence_id: SupportCellEvidenceId,
    },
    SupportObligationRefined {
        cell_id: SupportCellId,
        obligation_id: SupportProofObligationId,
        refinement_id: SupportObligationRefinementId,
    },
    /// Concrete fallback exhausted the producer and published accepted exact
    /// cardinality evidence. The operational cursor is intentionally absent:
    /// a checkpoint cannot certify semantic exhaustion by itself.
    SupportMaterializationExhausted {
        cell_id: SupportCellId,
        cardinality_obligation_id: SupportProofObligationId,
        evidence_id: SupportCellEvidenceId,
    },
}

impl WorkCompletionRef {
    fn for_readiness(spec: &WorkNodeSpec) -> Result<Self, WorkFrontierError> {
        match spec {
            WorkNodeSpec::SourcePrefixReady {
                relation_id,
                binding_index,
                prefix,
            } => Ok(Self::SourcePrefixReady {
                relation_id: *relation_id,
                binding_index: *binding_index,
                prefix: prefix.clone(),
            }),
            WorkNodeSpec::SourceRowReady {
                relation_id,
                source_key,
            } => Ok(Self::SourceRowReady {
                relation_id: *relation_id,
                source_key: *source_key,
            }),
            WorkNodeSpec::CaseReady { case_id } => Ok(Self::CaseReady { case_id: *case_id }),
            WorkNodeSpec::SupportCellReady { cell_id } => {
                Ok(Self::SupportCellReady { cell_id: *cell_id })
            }
            _ => Err(WorkFrontierError::NotReadinessNode),
        }
    }

    pub(crate) fn evidence_id(&self) -> WorkEvidenceId {
        let mut hasher = CanonicalHasher::new(WORK_COMPLETION_REF_ID_V1);
        match self {
            Self::SourcePrefixReady {
                relation_id,
                binding_index,
                prefix,
            } => {
                hasher.tag(0x01);
                hasher.digest(relation_id.bytes());
                hasher.u32(*binding_index);
                hasher.digest(prefix.digest());
            }
            Self::SourceRowReady {
                relation_id,
                source_key,
            } => {
                hasher.tag(0x02);
                hasher.digest(relation_id.bytes());
                hasher.digest(source_key.bytes());
            }
            Self::CaseReady { case_id } => {
                hasher.tag(0x03);
                hasher.digest(case_id.bytes());
            }
            Self::SupportCellReady { cell_id } => {
                hasher.tag(0x04);
                hasher.digest(cell_id.bytes());
            }
            Self::SourceBindingExhausted {
                relation_id,
                binding_index,
                prefix,
                terminal_ordinal,
                receipt_id,
            } => {
                hasher.tag(0x05);
                hasher.digest(relation_id.bytes());
                hasher.u32(*binding_index);
                hasher.digest(prefix.digest());
                hasher.u128(*terminal_ordinal);
                hasher.digest(receipt_id.bytes());
            }
            Self::SuccessorsSealed {
                relation_id,
                source_key,
                terminal_ordinal,
                receipt_id,
            } => {
                hasher.tag(0x06);
                hasher.digest(relation_id.bytes());
                hasher.digest(source_key.bytes());
                hasher.u128(*terminal_ordinal);
                hasher.digest(receipt_id.bytes());
            }
            Self::AdmissionDecided {
                admission_id,
                case_id,
                decision,
            } => {
                hasher.tag(0x07);
                hasher.digest(admission_id.bytes());
                hasher.digest(case_id.bytes());
                hasher.tag(match decision {
                    AdmissionDecision::Rejected => 0x01,
                    AdmissionDecision::Admitted => 0x02,
                });
            }
            Self::FindDecided {
                question_id,
                case_id,
                decision,
            } => {
                hasher.tag(0x08);
                hasher.digest(question_id.bytes());
                hasher.digest(case_id.bytes());
                hasher.tag(match decision {
                    SelectionDecision::NotSelected => 0x01,
                    SelectionDecision::Selected => 0x02,
                });
            }
            Self::DirectSupportEvidence {
                cell_id,
                obligation_id,
                evidence_id,
            } => {
                hasher.tag(0x09);
                hasher.digest(cell_id.bytes());
                hasher.digest(obligation_id.bytes());
                hasher.digest(evidence_id.bytes());
            }
            Self::SupportObligationRefined {
                cell_id,
                obligation_id,
                refinement_id,
            } => {
                hasher.tag(0x0a);
                hasher.digest(cell_id.bytes());
                hasher.digest(obligation_id.bytes());
                hasher.digest(refinement_id.bytes());
            }
            Self::SupportMaterializationExhausted {
                cell_id,
                cardinality_obligation_id,
                evidence_id,
            } => {
                hasher.tag(0x0b);
                hasher.digest(cell_id.bytes());
                hasher.digest(cardinality_obligation_id.bytes());
                hasher.digest(evidence_id.bytes());
            }
        }
        WorkEvidenceId(hasher.finish())
    }

    fn validate_for(
        &self,
        id: WorkNodeId,
        spec: &WorkNodeSpec,
        final_cursor: WorkCursor,
    ) -> Result<(), WorkFrontierError> {
        if matches!(
            spec,
            WorkNodeSpec::ReduceCaseView { .. }
                | WorkNodeSpec::ReplayMechanismEndpoint { .. }
                | WorkNodeSpec::BuildMechanismIncidence { .. }
                | WorkNodeSpec::ReduceMechanismIncidenceView { .. }
        ) {
            return Err(WorkFrontierError::UnsupportedCompletionKind { id });
        }

        let subject_matches = match (spec, self) {
            (
                WorkNodeSpec::SourcePrefixReady {
                    relation_id: expected_relation,
                    binding_index: expected_index,
                    prefix: expected_prefix,
                },
                Self::SourcePrefixReady {
                    relation_id,
                    binding_index,
                    prefix,
                },
            ) => {
                prefix.validate()?;
                relation_id == expected_relation
                    && binding_index == expected_index
                    && prefix == expected_prefix
            }
            (
                WorkNodeSpec::SourceRowReady {
                    relation_id: expected_relation,
                    source_key: expected_source,
                },
                Self::SourceRowReady {
                    relation_id,
                    source_key,
                },
            ) => relation_id == expected_relation && source_key == expected_source,
            (WorkNodeSpec::CaseReady { case_id: expected }, Self::CaseReady { case_id }) => {
                case_id == expected
            }
            (
                WorkNodeSpec::SupportCellReady { cell_id: expected },
                Self::SupportCellReady { cell_id },
            ) => cell_id == expected,
            (
                WorkNodeSpec::ExpandSourceBinding {
                    relation_id: expected_relation,
                    binding_index: expected_index,
                    prefix: expected_prefix,
                },
                Self::SourceBindingExhausted {
                    relation_id,
                    binding_index,
                    prefix,
                    terminal_ordinal,
                    ..
                },
            ) => {
                prefix.validate()?;
                relation_id == expected_relation
                    && binding_index == expected_index
                    && prefix == expected_prefix
                    && cursor_ordinal_matches(id, final_cursor, *terminal_ordinal)?
            }
            (
                WorkNodeSpec::ExpandSuccessors {
                    relation_id: expected_relation,
                    source_key: expected_source,
                },
                Self::SuccessorsSealed {
                    relation_id,
                    source_key,
                    terminal_ordinal,
                    ..
                },
            ) => {
                relation_id == expected_relation
                    && source_key == expected_source
                    && cursor_ordinal_matches(id, final_cursor, *terminal_ordinal)?
            }
            (
                WorkNodeSpec::EvaluateAdmission {
                    admission_id: expected_admission,
                    case_id: expected_case,
                },
                Self::AdmissionDecided {
                    admission_id,
                    case_id,
                    ..
                },
            ) => admission_id == expected_admission && case_id == expected_case,
            (
                WorkNodeSpec::EvaluateFind {
                    question_id: expected_question,
                    case_id: expected_case,
                },
                Self::FindDecided {
                    question_id,
                    case_id,
                    ..
                },
            ) => question_id == expected_question && case_id == expected_case,
            (
                WorkNodeSpec::ResolveSupportObligation {
                    cell_id: expected_cell,
                    obligation_id: expected_obligation,
                },
                Self::DirectSupportEvidence {
                    cell_id,
                    obligation_id,
                    ..
                }
                | Self::SupportObligationRefined {
                    cell_id,
                    obligation_id,
                    ..
                },
            ) => cell_id == expected_cell && obligation_id == expected_obligation,
            (
                WorkNodeSpec::MaterializeSupportCell {
                    cell_id: expected_cell,
                },
                Self::SupportMaterializationExhausted { cell_id, .. },
            ) => cell_id == expected_cell,
            _ => false,
        };
        if !subject_matches {
            return Err(WorkFrontierError::CompletionSubjectMismatch { id });
        }
        if !matches!(
            spec,
            WorkNodeSpec::ExpandSourceBinding { .. } | WorkNodeSpec::ExpandSuccessors { .. }
        ) && final_cursor != WorkCursor::Atomic
        {
            return Err(WorkFrontierError::CursorShapeMismatch(id));
        }
        Ok(())
    }
}

fn cursor_ordinal_matches(
    id: WorkNodeId,
    cursor: WorkCursor,
    claimed: u128,
) -> Result<bool, WorkFrontierError> {
    match cursor {
        WorkCursor::NextMemberOrdinal(actual) if actual == claimed => Ok(true),
        WorkCursor::NextMemberOrdinal(actual) => Err(WorkFrontierError::CompletionCursorMismatch {
            id,
            actual,
            claimed,
        }),
        WorkCursor::Atomic => Err(WorkFrontierError::CursorShapeMismatch(id)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismEndpoint {
    Before,
    After,
}

/// Immutable semantic work. Every variant is independently content addressed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkNodeSpec {
    /// Immutable readiness token for one canonical dependent-source prefix.
    SourcePrefixReady {
        relation_id: RelationId,
        binding_index: u32,
        prefix: CanonicalSourcePrefix,
    },
    /// Immutable readiness token for one canonical set-normalized source row.
    SourceRowReady {
        relation_id: RelationId,
        source_key: SourceKey,
    },
    /// Immutable readiness token emitted as soon as one canonical case is
    /// discovered, independently of successor-frontier closure.
    CaseReady { case_id: RelationalCaseId },
    /// Immutable readiness token for one registered, validated support cell.
    /// The support-evidence catalog remains the authority for the cell body.
    SupportCellReady { cell_id: SupportCellId },
    /// Enumerate one dependent FROM binding for a canonical prefix. The cursor
    /// is the next member ordinal within that binding's finite domain.
    ExpandSourceBinding {
        relation_id: RelationId,
        binding_index: u32,
        prefix: CanonicalSourcePrefix,
    },
    /// Enumerate the finite TO relation for one canonical source row.
    ExpandSuccessors {
        relation_id: RelationId,
        source_key: SourceKey,
    },
    /// Evaluate all scoped WHERE admissions for one discovered case.
    EvaluateAdmission {
        admission_id: AdmissionId,
        case_id: RelationalCaseId,
    },
    /// Classify one admitted case under FIND.
    EvaluateFind {
        question_id: QuestionId,
        case_id: RelationalCaseId,
    },
    /// Apply one selected-case result view to one case.
    ReduceCaseView {
        view_id: ViewId,
        case_id: RelationalCaseId,
    },
    /// Replay one endpoint of a mechanism observation for one selected case.
    ReplayMechanismEndpoint {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        endpoint: MechanismEndpoint,
    },
    /// Join the two endpoint observations into one mechanism-incidence row.
    BuildMechanismIncidence {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    /// Apply a post-replay view to one mechanism-incidence row.
    ReduceMechanismIncidenceView {
        view_id: ViewId,
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    /// Resolve one typed proof/classification/partition obligation over an
    /// exact support cell. The resolution may be direct evidence or a proved
    /// partition into child obligations; solver choice and split heuristics are
    /// deliberately absent from semantic work identity.
    ResolveSupportObligation {
        cell_id: SupportCellId,
        obligation_id: SupportProofObligationId,
    },
    /// Exact fallback for a cell that cannot yet be discharged symbolically.
    /// Its resumable materializer cursor lives in the support-evidence catalog,
    /// so the work frontier does not duplicate that progress authority.
    MaterializeSupportCell { cell_id: SupportCellId },
}

impl WorkNodeSpec {
    fn is_readiness(&self) -> bool {
        matches!(
            self,
            Self::SourcePrefixReady { .. }
                | Self::SourceRowReady { .. }
                | Self::CaseReady { .. }
                | Self::SupportCellReady { .. }
        )
    }

    fn initial_cursor(&self) -> WorkCursor {
        match self {
            Self::ExpandSourceBinding { .. } | Self::ExpandSuccessors { .. } => {
                WorkCursor::NextMemberOrdinal(0)
            }
            Self::SourcePrefixReady { .. }
            | Self::SourceRowReady { .. }
            | Self::CaseReady { .. }
            | Self::SupportCellReady { .. }
            | Self::EvaluateAdmission { .. }
            | Self::EvaluateFind { .. }
            | Self::ReduceCaseView { .. }
            | Self::ReplayMechanismEndpoint { .. }
            | Self::BuildMechanismIncidence { .. }
            | Self::ReduceMechanismIncidenceView { .. }
            | Self::ResolveSupportObligation { .. }
            | Self::MaterializeSupportCell { .. } => WorkCursor::Atomic,
        }
    }

    fn validate(&self) -> Result<(), WorkFrontierError> {
        if let Self::ExpandSourceBinding {
            binding_index,
            prefix,
            ..
        }
        | Self::SourcePrefixReady {
            binding_index,
            prefix,
            ..
        } = self
        {
            prefix.validate()?;
            let prefix_len = u32::try_from(prefix.values.len())
                .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("source prefix"))?;
            if prefix_len != *binding_index {
                return Err(WorkFrontierError::SourcePrefixLengthMismatch {
                    binding_index: *binding_index,
                    prefix_len,
                });
            }
        }
        Ok(())
    }

    fn required_readiness(&self) -> Option<Self> {
        match self {
            Self::ExpandSourceBinding {
                relation_id,
                binding_index,
                prefix,
            } => Some(Self::SourcePrefixReady {
                relation_id: *relation_id,
                binding_index: *binding_index,
                prefix: prefix.clone(),
            }),
            Self::ExpandSuccessors {
                relation_id,
                source_key,
            } => Some(Self::SourceRowReady {
                relation_id: *relation_id,
                source_key: *source_key,
            }),
            Self::EvaluateAdmission { case_id, .. }
            | Self::EvaluateFind { case_id, .. }
            | Self::ReduceCaseView { case_id, .. }
            | Self::ReplayMechanismEndpoint { case_id, .. }
            | Self::BuildMechanismIncidence { case_id, .. }
            | Self::ReduceMechanismIncidenceView { case_id, .. } => {
                Some(Self::CaseReady { case_id: *case_id })
            }
            Self::ResolveSupportObligation { cell_id, .. }
            | Self::MaterializeSupportCell { cell_id } => {
                Some(Self::SupportCellReady { cell_id: *cell_id })
            }
            Self::SourcePrefixReady { .. }
            | Self::SourceRowReady { .. }
            | Self::CaseReady { .. }
            | Self::SupportCellReady { .. } => None,
        }
    }

    fn hash_into(&self, hasher: &mut CanonicalHasher) {
        match self {
            Self::SourcePrefixReady {
                relation_id,
                binding_index,
                prefix,
            } => {
                hasher.tag(0x0b);
                hasher.digest(relation_id.bytes());
                hasher.u32(*binding_index);
                hasher.digest(prefix.digest());
            }
            Self::SourceRowReady {
                relation_id,
                source_key,
            } => {
                hasher.tag(0x0c);
                hasher.digest(relation_id.bytes());
                hasher.digest(source_key.bytes());
            }
            Self::CaseReady { case_id } => {
                hasher.tag(0x0d);
                hasher.digest(case_id.bytes());
            }
            Self::SupportCellReady { cell_id } => {
                hasher.tag(0x0e);
                hasher.digest(cell_id.bytes());
            }
            Self::ExpandSourceBinding {
                relation_id,
                binding_index,
                prefix,
            } => {
                hasher.tag(0x01);
                hasher.digest(relation_id.bytes());
                hasher.u32(*binding_index);
                hasher.digest(prefix.digest());
            }
            Self::ExpandSuccessors {
                relation_id,
                source_key,
            } => {
                hasher.tag(0x02);
                hasher.digest(relation_id.bytes());
                hasher.digest(source_key.bytes());
            }
            Self::EvaluateAdmission {
                admission_id,
                case_id,
            } => {
                hasher.tag(0x03);
                hasher.digest(admission_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::EvaluateFind {
                question_id,
                case_id,
            } => {
                hasher.tag(0x04);
                hasher.digest(question_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::ReduceCaseView { view_id, case_id } => {
                hasher.tag(0x05);
                hasher.digest(view_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::ReplayMechanismEndpoint {
                request_id,
                case_id,
                endpoint,
            } => {
                hasher.tag(0x06);
                hasher.digest(request_id.bytes());
                hasher.digest(case_id.bytes());
                hasher.tag(match endpoint {
                    MechanismEndpoint::Before => 0x01,
                    MechanismEndpoint::After => 0x02,
                });
            }
            Self::BuildMechanismIncidence {
                request_id,
                case_id,
            } => {
                hasher.tag(0x07);
                hasher.digest(request_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::ReduceMechanismIncidenceView {
                view_id,
                request_id,
                case_id,
            } => {
                hasher.tag(0x08);
                hasher.digest(view_id.bytes());
                hasher.digest(request_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::ResolveSupportObligation {
                cell_id,
                obligation_id,
            } => {
                hasher.tag(0x0f);
                hasher.digest(cell_id.bytes());
                hasher.digest(obligation_id.bytes());
            }
            Self::MaterializeSupportCell { cell_id } => {
                hasher.tag(0x10);
                hasher.digest(cell_id.bytes());
            }
        }
    }
}

/// Durable progress for a semantic work node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkCursor {
    Atomic,
    NextMemberOrdinal(u128),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkNodeProgress {
    Open {
        cursor: WorkCursor,
    },
    Complete {
        final_cursor: WorkCursor,
        completion: WorkCompletionRef,
    },
}

impl WorkNodeProgress {
    pub(crate) const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    pub(crate) const fn cursor(&self) -> WorkCursor {
        match self {
            Self::Open { cursor } => *cursor,
            Self::Complete { final_cursor, .. } => *final_cursor,
        }
    }

    pub(crate) fn completion(&self) -> Option<&WorkCompletionRef> {
        match self {
            Self::Open { .. } => None,
            Self::Complete { completion, .. } => Some(completion),
        }
    }

    pub(crate) fn evidence_id(&self) -> Option<WorkEvidenceId> {
        self.completion().map(WorkCompletionRef::evidence_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkNodeSnapshot {
    pub(crate) id: WorkNodeId,
    pub(crate) spec: WorkNodeSpec,
    pub(crate) dependencies: Box<[WorkNodeId]>,
    pub(crate) progress: WorkNodeProgress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkFrontierSnapshot {
    pub(crate) version: u32,
    pub(crate) root: WorkFrontierRoot,
    /// Strictly ordered by WorkNodeId.
    pub(crate) nodes: Box<[WorkNodeSnapshot]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkNodeRecord {
    spec: WorkNodeSpec,
    dependencies: BTreeSet<WorkNodeId>,
    progress: WorkNodeProgress,
}

/// In-memory semantic frontier.
///
/// `nodes` is the sole snapshot/root authority. The remaining collections are
/// maintained derived indexes: they are omitted from snapshots and rebuilt
/// after snapshot validation. They make open/runnable lookup proportional to
/// the live frontier rather than to all completed work history.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RelationalWorkFrontier {
    nodes: BTreeMap<WorkNodeId, WorkNodeRecord>,
    open_nodes: BTreeSet<WorkNodeId>,
    runnable_nodes: BTreeSet<WorkNodeId>,
    dependents_by_dependency: BTreeMap<WorkNodeId, BTreeSet<WorkNodeId>>,
    incomplete_dependency_counts: BTreeMap<WorkNodeId, usize>,
}

impl RelationalWorkFrontier {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// O(log N) keyed lookup. Cloning the returned immutable descriptor is
    /// proportional only to that node's spec and dependency set.
    pub(crate) fn get(&self, id: WorkNodeId) -> Option<WorkNodeSnapshot> {
        self.nodes.get(&id).map(|record| snapshot_node(id, record))
    }

    pub(crate) fn open_node_ids(&self) -> impl Iterator<Item = WorkNodeId> + '_ {
        self.open_nodes.iter().copied()
    }

    /// Canonically ordered open nodes whose dependencies are all complete.
    /// Lookup does not scan completed history or re-walk dependency sets.
    pub(crate) fn runnable_node_ids(&self) -> impl Iterator<Item = WorkNodeId> + '_ {
        self.runnable_nodes.iter().copied()
    }

    pub(crate) fn open_len(&self) -> usize {
        self.open_nodes.len()
    }

    pub(crate) fn completed_len(&self) -> usize {
        self.nodes.len().saturating_sub(self.open_nodes.len())
    }

    /// Hash the current live checkpoint projection without cloning every work
    /// descriptor. Full snapshots use the same canonical node/progress
    /// encoding, but schedulers may use this root at an occasional compaction
    /// boundary without doubling frontier memory.
    pub(crate) fn root(&self) -> Result<WorkFrontierRoot, WorkFrontierError> {
        derive_frontier_root_records(
            self.nodes.len(),
            self.nodes.iter().map(|(id, record)| (*id, record)),
        )
    }

    /// Prepare a deterministic bounded garbage-collection receipt.
    ///
    /// Only completed leaves are eligible. A completed dependency of any live
    /// node is retained, so removing the selected records cannot make an open
    /// node runnable early or erase a dependency needed for resume. Repeated
    /// compaction naturally peels completed DAG layers from leaves toward
    /// roots while each event remains bounded.
    pub(crate) fn compaction_receipt(
        &self,
        maximum_nodes: NonZeroU32,
    ) -> Result<Option<WorkFrontierCompaction>, WorkFrontierError> {
        validate_compaction_limit(maximum_nodes)?;
        let removable = self.compactable_leaf_ids(maximum_nodes)?;
        if removable.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.derive_compaction(maximum_nodes, &removable)?))
    }

    /// Apply only the exact leaf set independently rederived from this
    /// frontier. The preflight performs every fallible consistency check before
    /// the first removal, leaving mutation itself allocation-free and
    /// infallible under the validated derived indexes.
    pub(crate) fn compact(
        &mut self,
        supplied: WorkFrontierCompaction,
    ) -> Result<u32, WorkFrontierError> {
        validate_compaction_limit(supplied.maximum_nodes)?;
        let removable = self.compactable_leaf_ids(supplied.maximum_nodes)?;
        if removable.is_empty() {
            return Err(WorkFrontierError::NothingToCompact);
        }
        let expected = self.derive_compaction(supplied.maximum_nodes, &removable)?;
        if supplied != expected {
            return Err(WorkFrontierError::CompactionReceiptMismatch);
        }

        for id in &removable {
            let record = self
                .nodes
                .get(id)
                .ok_or(WorkFrontierError::DerivedIndexIncoherent(*id))?;
            if !record.progress.is_complete()
                || self.open_nodes.contains(id)
                || self.runnable_nodes.contains(id)
                || self.incomplete_dependency_counts.get(id) != Some(&0)
                || self
                    .dependents_by_dependency
                    .get(id)
                    .is_some_and(|dependents| !dependents.is_empty())
            {
                return Err(WorkFrontierError::DerivedIndexIncoherent(*id));
            }
        }

        for id in removable {
            let record = self
                .nodes
                .remove(&id)
                .expect("compaction preflight retained the completed leaf");
            self.incomplete_dependency_counts.remove(&id);
            self.dependents_by_dependency.remove(&id);
            for dependency in record.dependencies {
                let remove_reverse_key = {
                    let dependents = self
                        .dependents_by_dependency
                        .get_mut(&dependency)
                        .expect("a retained dependency indexes its compacted dependent");
                    let removed = dependents.remove(&id);
                    debug_assert!(removed);
                    dependents.is_empty()
                };
                if remove_reverse_key {
                    self.dependents_by_dependency.remove(&dependency);
                }
            }
        }

        debug_assert_eq!(
            self.root()
                .expect("a preflighted compact frontier remains canonically hashable"),
            supplied.after_root
        );
        Ok(supplied.removed_nodes)
    }

    fn compactable_leaf_ids(
        &self,
        maximum_nodes: NonZeroU32,
    ) -> Result<BTreeSet<WorkNodeId>, WorkFrontierError> {
        let maximum = usize::try_from(maximum_nodes.get())
            .map_err(|_| WorkFrontierError::InvalidCompactionLimit(maximum_nodes.get()))?;
        let mut removable = BTreeSet::new();
        for (id, record) in &self.nodes {
            if removable.len() == maximum {
                break;
            }
            let has_dependents = self
                .dependents_by_dependency
                .get(id)
                .is_some_and(|dependents| !dependents.is_empty());
            if record.progress.is_complete() && !has_dependents {
                removable.insert(*id);
            }
        }
        Ok(removable)
    }

    fn derive_compaction(
        &self,
        maximum_nodes: NonZeroU32,
        removable: &BTreeSet<WorkNodeId>,
    ) -> Result<WorkFrontierCompaction, WorkFrontierError> {
        let removed_nodes = u32::try_from(removable.len())
            .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("compacted work nodes"))?;
        let remaining_nodes = self
            .nodes
            .len()
            .checked_sub(removable.len())
            .ok_or(WorkFrontierError::CompactionReceiptMismatch)?;
        let before_root = self.root()?;
        let after_root = derive_frontier_root_records(
            remaining_nodes,
            self.nodes
                .iter()
                .filter(|(id, _)| !removable.contains(id))
                .map(|(id, record)| (*id, record)),
        )?;
        let removed_ids_root = derive_compacted_ids_root(removable)?;
        let id = derive_compaction_id(
            maximum_nodes,
            removed_nodes,
            removed_ids_root,
            before_root,
            after_root,
        );
        Ok(WorkFrontierCompaction {
            id,
            maximum_nodes,
            removed_nodes,
            removed_ids_root,
            before_root,
            after_root,
        })
    }

    /// Derive the canonical ID a journal claim must carry without mutating a
    /// frontier. Dependency arrival order and duplicates are normalized away;
    /// semantic-spec validation and direct self-dependency still fail closed.
    pub(crate) fn derive_node_id(
        spec: &WorkNodeSpec,
        dependencies: impl IntoIterator<Item = WorkNodeId>,
    ) -> Result<WorkNodeId, WorkFrontierError> {
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        let id = derive_work_node_id(spec, &dependencies)?;
        if dependencies.contains(&id) {
            return Err(WorkFrontierError::DependencyCycle(id));
        }
        Ok(id)
    }

    /// Insert one immutable semantic node. Equal rediscovery is a no-op;
    /// conflicting content under an equal ID fails closed.
    pub(crate) fn insert(
        &mut self,
        spec: WorkNodeSpec,
        dependencies: impl IntoIterator<Item = WorkNodeId>,
    ) -> Result<(WorkNodeId, bool), WorkFrontierError> {
        spec.validate()?;
        if spec.is_readiness() {
            return Err(WorkFrontierError::ReadinessRequiresMaterialization);
        }
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        for dependency in &dependencies {
            if !self.nodes.contains_key(dependency) {
                return Err(WorkFrontierError::MissingDependency(*dependency));
            }
        }
        self.require_semantic_readiness(&spec, &dependencies)?;
        let id = Self::derive_node_id(&spec, dependencies.iter().copied())?;
        let record = WorkNodeRecord {
            progress: WorkNodeProgress::Open {
                cursor: spec.initial_cursor(),
            },
            spec,
            dependencies,
        };
        self.insert_with_id(id, record)
    }

    /// Atomically publish one immutable completed readiness token. Readiness
    /// has no open state and therefore cannot become a second cursor or wait
    /// for its still-open producer enumerator to close.
    pub(crate) fn materialize_ready(
        &mut self,
        spec: WorkNodeSpec,
    ) -> Result<(WorkNodeId, bool), WorkFrontierError> {
        spec.validate()?;
        if !spec.is_readiness() {
            return Err(WorkFrontierError::NotReadinessNode);
        }
        let dependencies = BTreeSet::new();
        let id = Self::derive_node_id(&spec, dependencies.iter().copied())?;
        let completion = WorkCompletionRef::for_readiness(&spec)?;
        completion.validate_for(id, &spec, WorkCursor::Atomic)?;
        let record = WorkNodeRecord {
            spec,
            dependencies,
            progress: WorkNodeProgress::Complete {
                final_cursor: WorkCursor::Atomic,
                completion,
            },
        };
        self.insert_with_id(id, record)
    }

    fn require_semantic_readiness(
        &self,
        spec: &WorkNodeSpec,
        dependencies: &BTreeSet<WorkNodeId>,
    ) -> Result<(), WorkFrontierError> {
        let Some(readiness_spec) = spec.required_readiness() else {
            return Ok(());
        };
        let readiness_id = derive_work_node_id(&readiness_spec, &BTreeSet::new())?;
        if !dependencies.contains(&readiness_id) {
            return Err(WorkFrontierError::MissingRequiredReadiness(readiness_id));
        }
        let readiness = self
            .nodes
            .get(&readiness_id)
            .ok_or(WorkFrontierError::MissingRequiredReadiness(readiness_id))?;
        validate_readiness_record(readiness_id, readiness)?;
        Ok(())
    }

    fn insert_with_id(
        &mut self,
        id: WorkNodeId,
        record: WorkNodeRecord,
    ) -> Result<(WorkNodeId, bool), WorkFrontierError> {
        match self.nodes.get(&id) {
            Some(existing)
                if existing.spec == record.spec && existing.dependencies == record.dependencies =>
            {
                // Rediscovery never resets a durable cursor or completion.
                Ok((id, false))
            }
            Some(_) => Err(WorkFrontierError::IdentityCollision(id)),
            None => {
                if self.open_nodes.contains(&id)
                    || self.runnable_nodes.contains(&id)
                    || self.incomplete_dependency_counts.contains_key(&id)
                {
                    return Err(WorkFrontierError::DerivedIndexIncoherent(id));
                }
                let mut incomplete_dependencies = 0usize;
                for dependency in &record.dependencies {
                    let dependency_record = self
                        .nodes
                        .get(dependency)
                        .ok_or(WorkFrontierError::MissingDependency(*dependency))?;
                    if !dependency_record.progress.is_complete() {
                        incomplete_dependencies += 1;
                    }
                }
                if record.progress.is_complete() && incomplete_dependencies != 0 {
                    let dependency = record
                        .dependencies
                        .iter()
                        .copied()
                        .find(|dependency| !self.nodes[dependency].progress.is_complete())
                        .expect("a positive incomplete count has one open dependency");
                    return Err(WorkFrontierError::DependencyStillOpen { id, dependency });
                }

                let is_open = !record.progress.is_complete();
                let dependencies = record.dependencies.iter().copied().collect::<Vec<_>>();
                // Every fallible check precedes these coordinated derived-index
                // mutations. Allocation failure aborts the process rather than
                // returning a partially applied semantic update.
                self.nodes.insert(id, record);
                self.incomplete_dependency_counts
                    .insert(id, incomplete_dependencies);
                for dependency in dependencies {
                    self.dependents_by_dependency
                        .entry(dependency)
                        .or_default()
                        .insert(id);
                }
                if is_open {
                    self.open_nodes.insert(id);
                    if incomplete_dependencies == 0 {
                        self.runnable_nodes.insert(id);
                    }
                }
                Ok((id, true))
            }
        }
    }

    /// Advance an enumerator to its next unconsumed member. Equality is an
    /// idempotent replay; regression, atomic work, or post-completion mutation
    /// fails closed.
    pub(crate) fn advance_next_member(
        &mut self,
        id: WorkNodeId,
        next_member_ordinal: u128,
    ) -> Result<bool, WorkFrontierError> {
        self.require_dependencies_complete(id)?;
        let record = self
            .nodes
            .get_mut(&id)
            .ok_or(WorkFrontierError::UnknownNode(id))?;
        match record.progress.clone() {
            WorkNodeProgress::Open {
                cursor: WorkCursor::NextMemberOrdinal(current),
            } if next_member_ordinal < current => Err(WorkFrontierError::CursorRegression {
                id,
                current,
                attempted: next_member_ordinal,
            }),
            WorkNodeProgress::Open {
                cursor: WorkCursor::NextMemberOrdinal(current),
            } if next_member_ordinal == current => Ok(false),
            WorkNodeProgress::Open {
                cursor: WorkCursor::NextMemberOrdinal(_),
            } => {
                record.progress = WorkNodeProgress::Open {
                    cursor: WorkCursor::NextMemberOrdinal(next_member_ordinal),
                };
                Ok(true)
            }
            WorkNodeProgress::Open {
                cursor: WorkCursor::Atomic,
            } => Err(WorkFrontierError::CursorNotSupported(id)),
            WorkNodeProgress::Complete { .. } => Err(WorkFrontierError::NodeAlreadyComplete(id)),
        }
    }

    /// Complete a node after all declared dependencies have completed. The
    /// typed reference must name exactly the scheduled semantic subject. Equal
    /// completion is idempotent; a different conclusion or durable evidence
    /// reference is rejected.
    pub(crate) fn complete(
        &mut self,
        id: WorkNodeId,
        completion: WorkCompletionRef,
    ) -> Result<bool, WorkFrontierError> {
        self.require_dependencies_complete(id)?;
        let record = self
            .nodes
            .get(&id)
            .ok_or(WorkFrontierError::UnknownNode(id))?;
        completion.validate_for(id, &record.spec, record.progress.cursor())?;
        let attempted = completion.evidence_id();
        match record.progress.clone() {
            WorkNodeProgress::Complete {
                completion: existing,
                ..
            } if existing == completion => Ok(false),
            WorkNodeProgress::Complete {
                completion: existing,
                ..
            } => Err(WorkFrontierError::CompletionConflict {
                id,
                existing: existing.evidence_id(),
                attempted,
            }),
            WorkNodeProgress::Open { cursor } => {
                if !self.open_nodes.contains(&id)
                    || !self.runnable_nodes.contains(&id)
                    || self.incomplete_dependency_counts.get(&id) != Some(&0)
                {
                    return Err(WorkFrontierError::DerivedIndexIncoherent(id));
                }
                let dependent_updates = self.completion_dependent_updates(id)?;

                // No fallible operation follows the first mutation. Removing
                // this node and unblocking its reverse dependents is therefore
                // one semantic/index update.
                self.nodes
                    .get_mut(&id)
                    .expect("completion preflight retained the work node")
                    .progress = WorkNodeProgress::Complete {
                    final_cursor: cursor,
                    completion,
                };
                self.open_nodes.remove(&id);
                self.runnable_nodes.remove(&id);
                for (dependent, remaining) in dependent_updates {
                    *self
                        .incomplete_dependency_counts
                        .get_mut(&dependent)
                        .expect("completion preflight retained dependent count") = remaining;
                    if remaining == 0 {
                        self.runnable_nodes.insert(dependent);
                    }
                }
                Ok(true)
            }
        }
    }

    fn require_dependencies_complete(&self, id: WorkNodeId) -> Result<(), WorkFrontierError> {
        let record = self
            .nodes
            .get(&id)
            .ok_or(WorkFrontierError::UnknownNode(id))?;
        let incomplete = self
            .incomplete_dependency_counts
            .get(&id)
            .copied()
            .ok_or(WorkFrontierError::DerivedIndexIncoherent(id))?;
        match &record.progress {
            WorkNodeProgress::Open { .. }
                if !self.open_nodes.contains(&id)
                    || self.runnable_nodes.contains(&id) != (incomplete == 0) =>
            {
                return Err(WorkFrontierError::DerivedIndexIncoherent(id));
            }
            WorkNodeProgress::Complete { .. }
                if self.open_nodes.contains(&id)
                    || self.runnable_nodes.contains(&id)
                    || incomplete != 0 =>
            {
                return Err(WorkFrontierError::DerivedIndexIncoherent(id));
            }
            WorkNodeProgress::Open { .. } | WorkNodeProgress::Complete { .. } => {}
        }
        if incomplete == 0 {
            return Ok(());
        }
        let dependency = record
            .dependencies
            .iter()
            .copied()
            .find(|dependency| {
                self.nodes
                    .get(dependency)
                    .is_some_and(|record| !record.progress.is_complete())
            })
            .ok_or(WorkFrontierError::DerivedIndexIncoherent(id))?;
        Err(WorkFrontierError::DependencyStillOpen { id, dependency })
    }

    fn completion_dependent_updates(
        &self,
        id: WorkNodeId,
    ) -> Result<Vec<(WorkNodeId, usize)>, WorkFrontierError> {
        let Some(dependents) = self.dependents_by_dependency.get(&id) else {
            return Ok(Vec::new());
        };
        let mut updates = Vec::with_capacity(dependents.len());
        for dependent in dependents {
            let dependent_record = self
                .nodes
                .get(dependent)
                .ok_or(WorkFrontierError::DerivedIndexIncoherent(*dependent))?;
            let incomplete = self
                .incomplete_dependency_counts
                .get(dependent)
                .copied()
                .ok_or(WorkFrontierError::DerivedIndexIncoherent(*dependent))?;
            if dependent_record.progress.is_complete()
                || !self.open_nodes.contains(dependent)
                || !dependent_record.dependencies.contains(&id)
                || incomplete == 0
            {
                return Err(WorkFrontierError::DerivedIndexIncoherent(*dependent));
            }
            updates.push((*dependent, incomplete - 1));
        }
        Ok(updates)
    }

    pub(crate) fn snapshot(&self) -> Result<WorkFrontierSnapshot, WorkFrontierError> {
        let nodes = self
            .nodes
            .iter()
            .map(|(id, record)| snapshot_node(*id, record))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let root = derive_frontier_root(&nodes)?;
        Ok(WorkFrontierSnapshot {
            version: RELATIONAL_FRONTIER_SNAPSHOT_VERSION,
            root,
            nodes,
        })
    }

    pub(crate) fn from_snapshot(snapshot: WorkFrontierSnapshot) -> Result<Self, WorkFrontierError> {
        if snapshot.version != RELATIONAL_FRONTIER_SNAPSHOT_VERSION {
            return Err(WorkFrontierError::UnsupportedSnapshotVersion {
                actual: snapshot.version,
                expected: RELATIONAL_FRONTIER_SNAPSHOT_VERSION,
            });
        }
        if derive_frontier_root(&snapshot.nodes)? != snapshot.root {
            return Err(WorkFrontierError::SnapshotRootMismatch);
        }

        let mut nodes = BTreeMap::new();
        let mut previous = None;
        for node in snapshot.nodes.into_vec() {
            if previous.is_some_and(|id| node.id <= id) {
                return Err(WorkFrontierError::NonCanonicalSnapshotOrder);
            }
            previous = Some(node.id);
            node.spec.validate()?;
            validate_cursor_for_spec(&node.spec, node.progress.cursor(), node.id)?;
            if let WorkNodeProgress::Complete {
                final_cursor,
                completion,
            } = &node.progress
            {
                completion.validate_for(node.id, &node.spec, *final_cursor)?;
            }
            if node.dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(WorkFrontierError::NonCanonicalSnapshotDependencyOrder(
                    node.id,
                ));
            }
            let dependencies = node.dependencies.iter().copied().collect::<BTreeSet<_>>();
            if dependencies.len() != node.dependencies.len() {
                return Err(WorkFrontierError::DuplicateSnapshotDependency(node.id));
            }
            let derived = derive_work_node_id(&node.spec, &dependencies)?;
            if derived != node.id {
                return Err(WorkFrontierError::NodeIdentityMismatch {
                    supplied: node.id,
                    derived,
                });
            }
            if nodes
                .insert(
                    node.id,
                    WorkNodeRecord {
                        spec: node.spec,
                        dependencies,
                        progress: node.progress,
                    },
                )
                .is_some()
            {
                return Err(WorkFrontierError::IdentityCollision(node.id));
            }
        }

        validate_snapshot_dependencies(&nodes)?;
        Ok(Self::with_rebuilt_indexes(nodes))
    }

    fn with_rebuilt_indexes(nodes: BTreeMap<WorkNodeId, WorkNodeRecord>) -> Self {
        let mut frontier = Self {
            nodes,
            open_nodes: BTreeSet::new(),
            runnable_nodes: BTreeSet::new(),
            dependents_by_dependency: BTreeMap::new(),
            incomplete_dependency_counts: BTreeMap::new(),
        };
        for (id, record) in &frontier.nodes {
            let incomplete = record
                .dependencies
                .iter()
                .filter(|dependency| !frontier.nodes[dependency].progress.is_complete())
                .count();
            frontier
                .incomplete_dependency_counts
                .insert(*id, incomplete);
            for dependency in &record.dependencies {
                frontier
                    .dependents_by_dependency
                    .entry(*dependency)
                    .or_default()
                    .insert(*id);
            }
            if !record.progress.is_complete() {
                frontier.open_nodes.insert(*id);
                if incomplete == 0 {
                    frontier.runnable_nodes.insert(*id);
                }
            }
        }
        frontier
    }
}

fn snapshot_node(id: WorkNodeId, record: &WorkNodeRecord) -> WorkNodeSnapshot {
    WorkNodeSnapshot {
        id,
        spec: record.spec.clone(),
        dependencies: record
            .dependencies
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        progress: record.progress.clone(),
    }
}

fn validate_cursor_for_spec(
    spec: &WorkNodeSpec,
    cursor: WorkCursor,
    id: WorkNodeId,
) -> Result<(), WorkFrontierError> {
    if spec.initial_cursor() == WorkCursor::Atomic && cursor != WorkCursor::Atomic
        || spec.initial_cursor() != WorkCursor::Atomic && cursor == WorkCursor::Atomic
    {
        return Err(WorkFrontierError::CursorShapeMismatch(id));
    }
    Ok(())
}

fn validate_snapshot_dependencies(
    nodes: &BTreeMap<WorkNodeId, WorkNodeRecord>,
) -> Result<(), WorkFrontierError> {
    for (id, record) in nodes {
        if record.spec.is_readiness() {
            validate_readiness_record(*id, record)?;
        } else if let Some(readiness_spec) = record.spec.required_readiness() {
            let readiness_id = derive_work_node_id(&readiness_spec, &BTreeSet::new())?;
            if !record.dependencies.contains(&readiness_id) {
                return Err(WorkFrontierError::MissingRequiredReadiness(readiness_id));
            }
            let readiness = nodes
                .get(&readiness_id)
                .ok_or(WorkFrontierError::MissingRequiredReadiness(readiness_id))?;
            validate_readiness_record(readiness_id, readiness)?;
        }
        for dependency in &record.dependencies {
            if dependency == id {
                return Err(WorkFrontierError::DependencyCycle(*id));
            }
            if !nodes.contains_key(dependency) {
                return Err(WorkFrontierError::MissingDependency(*dependency));
            }
        }
        if record.progress.is_complete() {
            for dependency in &record.dependencies {
                if !nodes[dependency].progress.is_complete() {
                    return Err(WorkFrontierError::DependencyStillOpen {
                        id: *id,
                        dependency: *dependency,
                    });
                }
            }
        } else if matches!(
            record.progress.cursor(),
            WorkCursor::NextMemberOrdinal(ordinal) if ordinal > 0
        ) {
            for dependency in &record.dependencies {
                if !nodes[dependency].progress.is_complete() {
                    return Err(WorkFrontierError::DependencyStillOpen {
                        id: *id,
                        dependency: *dependency,
                    });
                }
            }
        }
    }

    fn visit(
        id: WorkNodeId,
        nodes: &BTreeMap<WorkNodeId, WorkNodeRecord>,
        active: &mut BTreeSet<WorkNodeId>,
        closed: &mut BTreeSet<WorkNodeId>,
    ) -> Result<(), WorkFrontierError> {
        if closed.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(WorkFrontierError::DependencyCycle(id));
        }
        for dependency in &nodes[&id].dependencies {
            visit(*dependency, nodes, active, closed)?;
        }
        active.remove(&id);
        closed.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut closed = BTreeSet::new();
    for id in nodes.keys().copied() {
        visit(id, nodes, &mut active, &mut closed)?;
    }
    Ok(())
}

fn validate_readiness_record(
    id: WorkNodeId,
    record: &WorkNodeRecord,
) -> Result<(), WorkFrontierError> {
    if !record.spec.is_readiness() {
        return Err(WorkFrontierError::InvalidReadinessNode(id));
    }
    let expected_completion = WorkCompletionRef::for_readiness(&record.spec)?;
    let expected_progress = WorkNodeProgress::Complete {
        final_cursor: WorkCursor::Atomic,
        completion: expected_completion,
    };
    if !record.dependencies.is_empty() || record.progress != expected_progress {
        return Err(WorkFrontierError::InvalidReadinessNode(id));
    }
    Ok(())
}

fn derive_work_node_id(
    spec: &WorkNodeSpec,
    dependencies: &BTreeSet<WorkNodeId>,
) -> Result<WorkNodeId, WorkFrontierError> {
    spec.validate()?;
    let dependency_count = u64::try_from(dependencies.len())
        .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("work dependencies"))?;
    let mut hasher = CanonicalHasher::new(WORK_NODE_ID_V2);
    spec.hash_into(&mut hasher);
    hasher.u64(dependency_count);
    for dependency in dependencies {
        hasher.digest(dependency.bytes());
    }
    Ok(WorkNodeId(hasher.finish()))
}

fn derive_frontier_root(nodes: &[WorkNodeSnapshot]) -> Result<WorkFrontierRoot, WorkFrontierError> {
    let count = u64::try_from(nodes.len())
        .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("frontier nodes"))?;
    let mut hasher = CanonicalHasher::new(WORK_FRONTIER_ROOT_V2);
    hasher.u32(RELATIONAL_FRONTIER_SNAPSHOT_VERSION);
    hasher.u64(count);
    for node in nodes {
        hasher.digest(node.id.bytes());
        hash_progress(&node.progress, &mut hasher);
    }
    Ok(WorkFrontierRoot(hasher.finish()))
}

fn derive_frontier_root_records<'a>(
    count: usize,
    records: impl IntoIterator<Item = (WorkNodeId, &'a WorkNodeRecord)>,
) -> Result<WorkFrontierRoot, WorkFrontierError> {
    let count = u64::try_from(count)
        .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("frontier nodes"))?;
    let mut hasher = CanonicalHasher::new(WORK_FRONTIER_ROOT_V2);
    hasher.u32(RELATIONAL_FRONTIER_SNAPSHOT_VERSION);
    hasher.u64(count);
    let mut actual = 0u64;
    for (id, record) in records {
        actual = actual
            .checked_add(1)
            .ok_or(WorkFrontierError::CanonicalLengthOverflow("frontier nodes"))?;
        hasher.digest(id.bytes());
        hash_progress(&record.progress, &mut hasher);
    }
    if actual != count {
        return Err(WorkFrontierError::FrontierCountMismatch {
            expected: count,
            actual,
        });
    }
    Ok(WorkFrontierRoot(hasher.finish()))
}

fn hash_progress(progress: &WorkNodeProgress, hasher: &mut CanonicalHasher) {
    match progress {
        WorkNodeProgress::Open { cursor } => {
            hasher.tag(0x01);
            hash_cursor(*cursor, hasher);
        }
        WorkNodeProgress::Complete {
            final_cursor,
            completion,
        } => {
            hasher.tag(0x02);
            hash_cursor(*final_cursor, hasher);
            hasher.digest(completion.evidence_id().bytes());
        }
    }
}

fn validate_compaction_limit(maximum_nodes: NonZeroU32) -> Result<(), WorkFrontierError> {
    if maximum_nodes.get() > WORK_FRONTIER_MAX_COMPACTION_NODES {
        return Err(WorkFrontierError::InvalidCompactionLimit(
            maximum_nodes.get(),
        ));
    }
    Ok(())
}

fn derive_compacted_ids_root(
    removable: &BTreeSet<WorkNodeId>,
) -> Result<[u8; 32], WorkFrontierError> {
    let count = u32::try_from(removable.len())
        .map_err(|_| WorkFrontierError::CanonicalLengthOverflow("compacted work nodes"))?;
    let mut hasher = CanonicalHasher::new(WORK_FRONTIER_COMPACTED_IDS_ROOT_V1);
    hasher.u32(count);
    for id in removable {
        hasher.digest(id.bytes());
    }
    Ok(hasher.finish())
}

fn derive_compaction_id(
    maximum_nodes: NonZeroU32,
    removed_nodes: u32,
    removed_ids_root: [u8; 32],
    before_root: WorkFrontierRoot,
    after_root: WorkFrontierRoot,
) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(WORK_FRONTIER_COMPACTION_ID_V1);
    hasher.u32(maximum_nodes.get());
    hasher.u32(removed_nodes);
    hasher.digest(removed_ids_root);
    hasher.digest(before_root.bytes());
    hasher.digest(after_root.bytes());
    hasher.finish()
}

fn hash_cursor(cursor: WorkCursor, hasher: &mut CanonicalHasher) {
    match cursor {
        WorkCursor::Atomic => hasher.tag(0x01),
        WorkCursor::NextMemberOrdinal(ordinal) => {
            hasher.tag(0x02);
            hasher.u128(ordinal);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkFrontierError {
    UnknownNode(WorkNodeId),
    MissingDependency(WorkNodeId),
    DependencyStillOpen {
        id: WorkNodeId,
        dependency: WorkNodeId,
    },
    DependencyCycle(WorkNodeId),
    IdentityCollision(WorkNodeId),
    DerivedIndexIncoherent(WorkNodeId),
    ReadinessRequiresMaterialization,
    NotReadinessNode,
    MissingRequiredReadiness(WorkNodeId),
    InvalidReadinessNode(WorkNodeId),
    NodeIdentityMismatch {
        supplied: WorkNodeId,
        derived: WorkNodeId,
    },
    NodeAlreadyComplete(WorkNodeId),
    UnsupportedCompletionKind {
        id: WorkNodeId,
    },
    CompletionSubjectMismatch {
        id: WorkNodeId,
    },
    CompletionCursorMismatch {
        id: WorkNodeId,
        actual: u128,
        claimed: u128,
    },
    CursorNotSupported(WorkNodeId),
    CursorShapeMismatch(WorkNodeId),
    CursorRegression {
        id: WorkNodeId,
        current: u128,
        attempted: u128,
    },
    CompletionConflict {
        id: WorkNodeId,
        existing: WorkEvidenceId,
        attempted: WorkEvidenceId,
    },
    SourcePrefixDigestMismatch,
    SourcePrefixLengthMismatch {
        binding_index: u32,
        prefix_len: u32,
    },
    DuplicateSnapshotDependency(WorkNodeId),
    NonCanonicalSnapshotDependencyOrder(WorkNodeId),
    NonCanonicalSnapshotOrder,
    SnapshotRootMismatch,
    InvalidCompactionLimit(u32),
    NothingToCompact,
    CompactionReceiptMismatch,
    FrontierCountMismatch {
        expected: u64,
        actual: u64,
    },
    UnsupportedSnapshotVersion {
        actual: u32,
        expected: u32,
    },
    CanonicalLengthOverflow(&'static str),
}

impl fmt::Display for WorkFrontierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(_) => formatter.write_str("relational work node is absent"),
            Self::MissingDependency(_) => formatter.write_str("work dependency is absent"),
            Self::DependencyStillOpen { .. } => formatter.write_str("work dependency is open"),
            Self::DependencyCycle(_) => formatter.write_str("work dependencies contain a cycle"),
            Self::IdentityCollision(_) => {
                formatter.write_str("work-node identity collision has conflicting content")
            }
            Self::DerivedIndexIncoherent(_) => {
                formatter.write_str("relational work-frontier derived index is incoherent")
            }
            Self::ReadinessRequiresMaterialization => {
                formatter.write_str("readiness nodes must be atomically materialized complete")
            }
            Self::NotReadinessNode => {
                formatter.write_str("ordinary work cannot use readiness materialization")
            }
            Self::MissingRequiredReadiness(_) => {
                formatter.write_str("work node is missing its immutable readiness dependency")
            }
            Self::InvalidReadinessNode(_) => {
                formatter.write_str("readiness node is not immutable and canonically complete")
            }
            Self::NodeIdentityMismatch { .. } => {
                formatter.write_str("work-node identity does not match semantic content")
            }
            Self::NodeAlreadyComplete(_) => formatter.write_str("work node is already complete"),
            Self::UnsupportedCompletionKind { .. } => formatter.write_str(
                "work kind has no modeled durable completion reference and cannot close",
            ),
            Self::CompletionSubjectMismatch { .. } => formatter
                .write_str("work completion reference does not match the scheduled subject"),
            Self::CompletionCursorMismatch { .. } => formatter.write_str(
                "work completion exhaustion ordinal does not match the durable work cursor",
            ),
            Self::CursorNotSupported(_) => {
                formatter.write_str("atomic work does not have a member cursor")
            }
            Self::CursorShapeMismatch(_) => {
                formatter.write_str("work cursor does not match its semantic node kind")
            }
            Self::CursorRegression { .. } => {
                formatter.write_str("work cursor cannot move backwards")
            }
            Self::CompletionConflict { .. } => {
                formatter.write_str("work node has conflicting completion evidence")
            }
            Self::SourcePrefixDigestMismatch => {
                formatter.write_str("source prefix digest does not match its values")
            }
            Self::SourcePrefixLengthMismatch { .. } => {
                formatter.write_str("source prefix length does not match binding index")
            }
            Self::DuplicateSnapshotDependency(_) => {
                formatter.write_str("snapshot repeats one work dependency")
            }
            Self::NonCanonicalSnapshotDependencyOrder(_) => {
                formatter.write_str("snapshot work dependencies are not strictly ordered")
            }
            Self::NonCanonicalSnapshotOrder => {
                formatter.write_str("snapshot work nodes are not strictly ordered")
            }
            Self::SnapshotRootMismatch => {
                formatter.write_str("snapshot root does not authenticate its work frontier")
            }
            Self::InvalidCompactionLimit(limit) => write!(
                formatter,
                "work-frontier compaction bound {limit} exceeds the hard limit {WORK_FRONTIER_MAX_COMPACTION_NODES}"
            ),
            Self::NothingToCompact => {
                formatter.write_str("work frontier has no completed leaf nodes to compact")
            }
            Self::CompactionReceiptMismatch => formatter
                .write_str("work-frontier compaction receipt does not match the current leaf set"),
            Self::FrontierCountMismatch { expected, actual } => write!(
                formatter,
                "work-frontier hashing expected {expected} nodes but received {actual}"
            ),
            Self::UnsupportedSnapshotVersion { actual, expected } => write!(
                formatter,
                "unsupported relational frontier snapshot version {actual}; expected {expected}"
            ),
            Self::CanonicalLengthOverflow(label) => {
                write!(
                    formatter,
                    "{label} length cannot be canonically represented"
                )
            }
        }
    }
}

impl Error for WorkFrontierError {}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn tag(&mut self, tag: u8) {
        self.0.update([tag]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relation::{
        FindPolarity, MechanismTargetId, RelationProvenance, SourceRow, SuccessorKey, SuccessorRow,
        ViewInputId,
    };
    use crate::explore::relational_case_executor::{
        SuccessorFiberExhaustionReceipt, SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION,
    };
    use crate::explore::relational_executor::{
        SourceBindingExhaustionReceipt, SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION,
    };
    use crate::explore::support_cell::{
        ExactCardinalityClaim, SupportCell, SupportCellObligation, SupportCellSpace, SupportExpr,
        SupportMaterializerId, SupportPartitionCertificate, SupportProducerId,
    };
    use crate::explore::support_evidence::{SupportObligationRecord, SupportObligationRefinement};

    struct FixtureIds {
        relation: RelationId,
        source: SourceKey,
        case_id: RelationalCaseId,
        admission: AdmissionId,
        question: QuestionId,
        case_view: ViewId,
        request: MechanismRequestId,
        incidence_view: ViewId,
    }

    fn fixture_ids() -> FixtureIds {
        let relation = RelationId::from_canonical_semantic_preimage(b"relation");
        let source_row = SourceRow::new(
            ExploreValue::String("context".to_string()),
            ExploreValue::Int(100),
            RelationProvenance::new([], []),
        );
        let source = SourceKey::derive(relation, &source_row);
        let successor = SuccessorKey::derive(
            relation,
            source,
            &SuccessorRow::new(ExploreValue::Int(101), RelationProvenance::new([], [])),
        );
        let case_id = RelationalCaseId::derive(relation, source, successor);
        let admission = AdmissionId::from_canonical_admission_preimage(relation, b"where");
        let question =
            QuestionId::from_canonical_find_preimage(admission, b"find", FindPolarity::Matches);
        let case_view =
            ViewId::from_canonical_view_preimage(ViewInputId::Selected(question), b"case-view");
        let request = MechanismRequestId::from_canonical_request_preimages(
            question,
            MechanismTargetId::Selected,
            b"observe",
            b"normalize",
        );
        let incidence_view = ViewId::from_canonical_view_preimage(
            ViewInputId::MechanismIncidence(request),
            b"incidence-view",
        );
        FixtureIds {
            relation,
            source,
            case_id,
            admission,
            question,
            case_view,
            request,
            incidence_view,
        }
    }

    fn materialize_case(
        frontier: &mut RelationalWorkFrontier,
        case_id: RelationalCaseId,
    ) -> WorkNodeId {
        frontier
            .materialize_ready(WorkNodeSpec::CaseReady { case_id })
            .unwrap()
            .0
    }

    fn source_exhaustion_receipt_id(
        relation_id: RelationId,
        binding_index: u32,
        prefix: &CanonicalSourcePrefix,
        terminal_ordinal: u128,
    ) -> SourceBindingExhaustionReceiptId {
        let receipt = SourceBindingExhaustionReceipt::restore_from_journal_codec(
            SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION,
            relation_id,
            binding_index,
            prefix.digest(),
            terminal_ordinal,
            terminal_ordinal,
            Sha256::digest(b"frontier-source-fiber-members").into(),
        )
        .expect("content-authenticated source exhaustion receipt");
        receipt.validate_identity().unwrap();
        receipt.id()
    }

    fn successor_exhaustion_receipt_id(
        relation_id: RelationId,
        source_key: SourceKey,
        terminal_ordinal: u128,
    ) -> SuccessorFiberExhaustionReceiptId {
        let receipt = SuccessorFiberExhaustionReceipt::restore_from_journal_codec(
            SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION,
            relation_id,
            source_key,
            terminal_ordinal,
            terminal_ordinal,
            Sha256::digest(b"frontier-successor-fiber-rows").into(),
        )
        .expect("content-authenticated successor exhaustion receipt");
        receipt.validate_identity().unwrap();
        receipt.id()
    }

    #[test]
    fn source_cursor_is_monotone_and_node_identity_survives_pause_resume() {
        let ids = fixture_ids();
        let mut frontier = RelationalWorkFrontier::new();
        let prefix = CanonicalSourcePrefix::from_values(vec![ExploreValue::Int(7)]).unwrap();
        let (prefix_ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SourcePrefixReady {
                relation_id: ids.relation,
                binding_index: 1,
                prefix: prefix.clone(),
            })
            .unwrap();
        let spec = WorkNodeSpec::ExpandSourceBinding {
            relation_id: ids.relation,
            binding_index: 1,
            prefix,
        };
        let (id, inserted) = frontier.insert(spec.clone(), [prefix_ready]).unwrap();
        assert!(inserted);
        assert!(frontier.advance_next_member(id, 4).unwrap());
        assert_eq!(frontier.insert(spec, [prefix_ready]).unwrap(), (id, false));
        assert!(!frontier.advance_next_member(id, 4).unwrap());
        assert!(matches!(
            frontier.advance_next_member(id, 3),
            Err(WorkFrontierError::CursorRegression { .. })
        ));

        // One yielded value can materialize the child prefix and start its
        // dependent binding while this parent enumerator remains open.
        let child_prefix =
            CanonicalSourcePrefix::from_values(vec![ExploreValue::Int(7), ExploreValue::Int(11)])
                .unwrap();
        let (child_ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SourcePrefixReady {
                relation_id: ids.relation,
                binding_index: 2,
                prefix: child_prefix.clone(),
            })
            .unwrap();
        let (child, _) = frontier
            .insert(
                WorkNodeSpec::ExpandSourceBinding {
                    relation_id: ids.relation,
                    binding_index: 2,
                    prefix: child_prefix,
                },
                [child_ready],
            )
            .unwrap();
        frontier.advance_next_member(child, 1).unwrap();

        let resumed = RelationalWorkFrontier::from_snapshot(frontier.snapshot().unwrap()).unwrap();
        assert_eq!(resumed.get(id).unwrap().id, id);
        assert_eq!(
            resumed.get(id).unwrap().progress.cursor(),
            WorkCursor::NextMemberOrdinal(4)
        );
    }

    #[test]
    fn source_exhaustion_completion_binds_subject_and_terminal_cursor() {
        let ids = fixture_ids();
        let prefix = CanonicalSourcePrefix::empty();
        let mut frontier = RelationalWorkFrontier::new();
        let (ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SourcePrefixReady {
                relation_id: ids.relation,
                binding_index: 0,
                prefix: prefix.clone(),
            })
            .unwrap();
        let (expand, _) = frontier
            .insert(
                WorkNodeSpec::ExpandSourceBinding {
                    relation_id: ids.relation,
                    binding_index: 0,
                    prefix: prefix.clone(),
                },
                [ready],
            )
            .unwrap();
        frontier.advance_next_member(expand, 3).unwrap();
        assert!(matches!(
            frontier.complete(
                expand,
                WorkCompletionRef::SourceBindingExhausted {
                    relation_id: ids.relation,
                    binding_index: 0,
                    prefix: prefix.clone(),
                    terminal_ordinal: 2,
                    receipt_id: source_exhaustion_receipt_id(
                        ids.relation,
                        0,
                        &prefix,
                        2,
                    ),
                },
            ),
            Err(WorkFrontierError::CompletionCursorMismatch {
                id,
                actual: 3,
                claimed: 2,
            }) if id == expand
        ));
        frontier
            .complete(
                expand,
                WorkCompletionRef::SourceBindingExhausted {
                    relation_id: ids.relation,
                    binding_index: 0,
                    terminal_ordinal: 3,
                    receipt_id: source_exhaustion_receipt_id(ids.relation, 0, &prefix, 3),
                    prefix,
                },
            )
            .unwrap();
        let progress = frontier.get(expand).unwrap().progress;
        assert!(progress.is_complete());
        assert!(progress.evidence_id().is_some());
    }

    #[test]
    fn arrival_order_does_not_change_canonical_snapshot() {
        let ids = fixture_ids();
        let left = WorkNodeSpec::EvaluateAdmission {
            admission_id: ids.admission,
            case_id: ids.case_id,
        };
        let right = WorkNodeSpec::EvaluateFind {
            question_id: ids.question,
            case_id: ids.case_id,
        };
        let mut first = RelationalWorkFrontier::new();
        let first_ready = materialize_case(&mut first, ids.case_id);
        first.insert(left.clone(), [first_ready]).unwrap();
        first.insert(right.clone(), [first_ready]).unwrap();
        let mut second = RelationalWorkFrontier::new();
        let second_ready = materialize_case(&mut second, ids.case_id);
        second.insert(right, [second_ready]).unwrap();
        second.insert(left, [second_ready]).unwrap();
        assert_eq!(first.snapshot().unwrap(), second.snapshot().unwrap());
    }

    #[test]
    fn pure_node_id_derivation_canonicalizes_dependency_sets() {
        let ids = fixture_ids();
        let spec = WorkNodeSpec::EvaluateFind {
            question_id: ids.question,
            case_id: ids.case_id,
        };
        let first = WorkNodeId([0x11; 32]);
        let second = WorkNodeId([0x22; 32]);
        let canonical = RelationalWorkFrontier::derive_node_id(&spec, [first, second]).unwrap();
        let reordered_with_duplicate =
            RelationalWorkFrontier::derive_node_id(&spec, [second, first, second]).unwrap();
        assert_eq!(canonical, reordered_with_duplicate);
    }

    #[test]
    fn completion_requires_dependencies_and_is_idempotent_but_not_ambiguous() {
        let ids = fixture_ids();
        let mut frontier = RelationalWorkFrontier::new();
        let case_ready = materialize_case(&mut frontier, ids.case_id);
        let (admission, _) = frontier
            .insert(
                WorkNodeSpec::EvaluateAdmission {
                    admission_id: ids.admission,
                    case_id: ids.case_id,
                },
                [case_ready],
            )
            .unwrap();
        let (find, _) = frontier
            .insert(
                WorkNodeSpec::EvaluateFind {
                    question_id: ids.question,
                    case_id: ids.case_id,
                },
                [case_ready, admission],
            )
            .unwrap();
        assert!(matches!(
            frontier.complete(
                find,
                WorkCompletionRef::FindDecided {
                    question_id: ids.question,
                    case_id: ids.case_id,
                    decision: SelectionDecision::Selected,
                },
            ),
            Err(WorkFrontierError::DependencyStillOpen { .. })
        ));
        frontier
            .complete(
                admission,
                WorkCompletionRef::AdmissionDecided {
                    admission_id: ids.admission,
                    case_id: ids.case_id,
                    decision: AdmissionDecision::Admitted,
                },
            )
            .unwrap();
        let selected = WorkCompletionRef::FindDecided {
            question_id: ids.question,
            case_id: ids.case_id,
            decision: SelectionDecision::Selected,
        };
        assert!(frontier.complete(find, selected.clone()).unwrap());
        assert!(!frontier.complete(find, selected).unwrap());
        assert!(matches!(
            frontier.complete(
                find,
                WorkCompletionRef::FindDecided {
                    question_id: ids.question,
                    case_id: ids.case_id,
                    decision: SelectionDecision::NotSelected,
                },
            ),
            Err(WorkFrontierError::CompletionConflict { .. })
        ));
    }

    #[test]
    fn one_yielded_case_classifies_while_successor_enumerator_remains_open() {
        let ids = fixture_ids();
        let mut frontier = RelationalWorkFrontier::new();
        let (source_ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SourceRowReady {
                relation_id: ids.relation,
                source_key: ids.source,
            })
            .unwrap();
        let (source, _) = frontier
            .insert(
                WorkNodeSpec::ExpandSuccessors {
                    relation_id: ids.relation,
                    source_key: ids.source,
                },
                [source_ready],
            )
            .unwrap();
        frontier.advance_next_member(source, 1).unwrap();
        let case_ready = materialize_case(&mut frontier, ids.case_id);
        let (admission, _) = frontier
            .insert(
                WorkNodeSpec::EvaluateAdmission {
                    admission_id: ids.admission,
                    case_id: ids.case_id,
                },
                [case_ready],
            )
            .unwrap();
        frontier
            .complete(
                admission,
                WorkCompletionRef::AdmissionDecided {
                    admission_id: ids.admission,
                    case_id: ids.case_id,
                    decision: AdmissionDecision::Admitted,
                },
            )
            .unwrap();
        let (find, _) = frontier
            .insert(
                WorkNodeSpec::EvaluateFind {
                    question_id: ids.question,
                    case_id: ids.case_id,
                },
                [case_ready, admission],
            )
            .unwrap();
        frontier
            .complete(
                find,
                WorkCompletionRef::FindDecided {
                    question_id: ids.question,
                    case_id: ids.case_id,
                    decision: SelectionDecision::Selected,
                },
            )
            .unwrap();
        assert!(!frontier.get(source).unwrap().progress.is_complete());
        frontier
            .complete(
                source,
                WorkCompletionRef::SuccessorsSealed {
                    relation_id: ids.relation,
                    source_key: ids.source,
                    terminal_ordinal: 1,
                    receipt_id: successor_exhaustion_receipt_id(ids.relation, ids.source, 1),
                },
            )
            .unwrap();
        assert_eq!(frontier.open_node_ids().count(), 0);
        assert_eq!(frontier.len(), 5);
    }

    #[test]
    fn result_and_mechanism_work_fail_closed_without_typed_durable_evidence() {
        let ids = fixture_ids();
        let mut frontier = RelationalWorkFrontier::new();
        let case_ready = materialize_case(&mut frontier, ids.case_id);
        let specs = [
            WorkNodeSpec::ReduceCaseView {
                view_id: ids.case_view,
                case_id: ids.case_id,
            },
            WorkNodeSpec::ReplayMechanismEndpoint {
                request_id: ids.request,
                case_id: ids.case_id,
                endpoint: MechanismEndpoint::Before,
            },
            WorkNodeSpec::BuildMechanismIncidence {
                request_id: ids.request,
                case_id: ids.case_id,
            },
            WorkNodeSpec::ReduceMechanismIncidenceView {
                view_id: ids.incidence_view,
                request_id: ids.request,
                case_id: ids.case_id,
            },
        ];
        for spec in specs {
            let (node_id, _) = frontier.insert(spec, [case_ready]).unwrap();
            assert!(matches!(
                frontier.complete(
                    node_id,
                    WorkCompletionRef::CaseReady {
                        case_id: ids.case_id,
                    },
                ),
                Err(WorkFrontierError::UnsupportedCompletionKind { id }) if id == node_id
            ));
            assert!(!frontier.get(node_id).unwrap().progress.is_complete());
        }
    }

    #[test]
    fn support_resolution_is_semantic_work_not_optimizer_strategy() {
        let producer = SupportProducerId::from_canonical_preimage(b"producer");
        let materializer = SupportMaterializerId::from_canonical_preimage(b"materializer");
        let cell = SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer),
            SupportExpr::ordinal_interval(0, 10).unwrap(),
            materializer,
        )
        .unwrap();
        let obligation = SupportCellObligation::new(&cell, ExactCardinalityClaim).unwrap();
        let mut frontier = RelationalWorkFrontier::new();
        let (cell_ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SupportCellReady { cell_id: cell.id() })
            .unwrap();
        let (resolve, _) = frontier
            .insert(
                WorkNodeSpec::ResolveSupportObligation {
                    cell_id: cell.id(),
                    obligation_id: obligation.id(),
                },
                [cell_ready],
            )
            .unwrap();
        let (materialize, _) = frontier
            .insert(
                WorkNodeSpec::MaterializeSupportCell { cell_id: cell.id() },
                [cell_ready],
            )
            .unwrap();
        assert_ne!(resolve, materialize);
        let direct_evidence = cell.structural_cardinality_evidence().unwrap().unwrap();
        frontier
            .complete(
                resolve,
                WorkCompletionRef::DirectSupportEvidence {
                    cell_id: cell.id(),
                    obligation_id: obligation.id(),
                    evidence_id: direct_evidence.id(),
                },
            )
            .unwrap();
        assert!(frontier.get(resolve).unwrap().progress.is_complete());
        assert!(!frontier.get(materialize).unwrap().progress.is_complete());

        frontier
            .complete(
                materialize,
                WorkCompletionRef::SupportMaterializationExhausted {
                    cell_id: cell.id(),
                    cardinality_obligation_id: obligation.id(),
                    evidence_id: direct_evidence.id(),
                },
            )
            .unwrap();
        assert!(frontier.get(materialize).unwrap().progress.is_complete());
    }

    #[test]
    fn support_refinement_completion_must_match_cell_and_obligation_subject() {
        let producer = SupportProducerId::from_canonical_preimage(b"refinement-producer");
        let materializer =
            SupportMaterializerId::from_canonical_preimage(b"refinement-materializer");
        let make_cell = |start, end_exclusive| {
            SupportCell::new(
                SupportCellSpace::ProducerCoordinates(producer),
                SupportExpr::ordinal_interval(start, end_exclusive).unwrap(),
                materializer,
            )
            .unwrap()
        };
        let parent = make_cell(0, 10);
        let left = make_cell(0, 4);
        let right = make_cell(4, 10);
        let partition = SupportPartitionCertificate::ordinal_interval_cover(
            &parent,
            vec![left.clone(), right.clone()],
        )
        .unwrap();
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

        let mut frontier = RelationalWorkFrontier::new();
        let (ready, _) = frontier
            .materialize_ready(WorkNodeSpec::SupportCellReady {
                cell_id: parent.id(),
            })
            .unwrap();
        let (resolve, _) = frontier
            .insert(
                WorkNodeSpec::ResolveSupportObligation {
                    cell_id: parent.id(),
                    obligation_id: parent_obligation.id(),
                },
                [ready],
            )
            .unwrap();
        assert!(matches!(
            frontier.complete(
                resolve,
                WorkCompletionRef::SupportObligationRefined {
                    cell_id: left.id(),
                    obligation_id: parent_obligation.id(),
                    refinement_id: refinement.id(),
                },
            ),
            Err(WorkFrontierError::CompletionSubjectMismatch { id }) if id == resolve
        ));
        frontier
            .complete(
                resolve,
                WorkCompletionRef::SupportObligationRefined {
                    cell_id: parent.id(),
                    obligation_id: parent_obligation.id(),
                    refinement_id: refinement.id(),
                },
            )
            .unwrap();
        assert!(frontier.get(resolve).unwrap().progress.is_complete());
    }

    #[test]
    fn checked_insertion_rejects_conflicting_content_under_one_id() {
        let ids = fixture_ids();
        let mut frontier = RelationalWorkFrontier::new();
        let first = WorkNodeRecord {
            spec: WorkNodeSpec::EvaluateAdmission {
                admission_id: ids.admission,
                case_id: ids.case_id,
            },
            dependencies: BTreeSet::new(),
            progress: WorkNodeProgress::Open {
                cursor: WorkCursor::Atomic,
            },
        };
        let id = derive_work_node_id(&first.spec, &first.dependencies).unwrap();
        frontier.insert_with_id(id, first).unwrap();
        let conflicting = WorkNodeRecord {
            spec: WorkNodeSpec::EvaluateFind {
                question_id: ids.question,
                case_id: ids.case_id,
            },
            dependencies: BTreeSet::new(),
            progress: WorkNodeProgress::Open {
                cursor: WorkCursor::Atomic,
            },
        };
        assert_eq!(
            frontier.insert_with_id(id, conflicting),
            Err(WorkFrontierError::IdentityCollision(id))
        );
    }
}
