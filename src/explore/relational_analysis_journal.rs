//! Replayable semantic events for the plan-bound relational analysis DAG.
//!
//! This is deliberately a subordinate event protocol, not a second journal:
//! it has no sequence number, previous head, file framing, or scheduler
//! cursor. One outer append-only journal must hash [`RelationalAnalysisEvidenceEvent::digest`]
//! into its evidence frames and keep invocation/scheduler checkpoints in its
//! separate checkpoint vocabulary. Replaying the evidence frames in causal
//! order rebuilds the same [`RelationalAnalysisCatalogBuilder`].
//!
//! Small content-bearing events carry complete values plus claimed typed
//! identities. Potentially trace-sized mechanism evidence instead crosses as
//! one non-interleaved sequence of bounded chunks under a typed full-payload
//! digest. Apply validates and privately restores that sequence at its compact
//! closure before calling the ordinary catalog mutation. No event applies by
//! cloning the whole analysis builder. The only linear materializations are
//! intentional immutable mechanism/result artifacts at closure boundaries and
//! the final closed analysis snapshot.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::{
    MechanismIncidenceRoot, MechanismPublicationDiscovery, MechanismPublicationDiscoveryRef,
    MechanismSignatureDefinition, MechanismSignatureId, MechanismTargetCaseSetCommitment,
    MechanismUnavailableReasonDefinition, MechanismUnavailableReasonId,
};
use super::mechanism_support::{
    MechanismSupportCatalogBuilder, MechanismSupportCheckpointCursor,
    MechanismSupportClosureReceipt, MechanismSupportClosureRoot, MechanismSupportError,
    MechanismSupportFrontierSummary,
};
use super::relation::{
    ClosedQuestionCatalogRef, MechanismRequestId, QuestionCatalog, QuestionContentRoot, QuestionId,
    RelationalCaseId, RelationalCaseRef, ViewId,
};
use super::relational_analysis_catalog::{
    ClosedRelationalAnalysisCatalog, RelationalAnalysisCatalogBuilder,
    RelationalAnalysisCatalogError, RelationalAnalysisCatalogRoot,
    RelationalAnalysisCatalogSnapshot, RelationalMechanismClosureReceipt,
    RelationalMechanismEvidenceContract,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanRoot, RelationalMechanismObservationDigest,
    RelationalMechanismObservationId, RelationalResolvedResultInput,
};
use super::relational_certified_source_summary::{
    RelationalCertifiedSourceSummaryArtifact, RelationalCertifiedSourceSummaryArtifactId,
};
use super::relational_mechanism_executor::{
    derive_relational_structural_mechanism_v1, RelationalMechanismReplayError,
    RelationalMechanismReplayEvidence, RelationalMechanismReplayObservationId,
    RelationalMechanismReplayReceiptId, RelationalMechanismSignatureDagIndex,
    RelationalMechanismUnavailableEvidence, RelationalStructuralMechanismError,
};
use super::relational_population::{
    CertifiedSelectedPopulationRoot, ClosedCertifiedSelectedPopulation,
};
use super::result_evidence::{
    RelationalResultEvidenceId, RelationalResultEvidenceRecord, RelationalResultEvidenceRoot,
    RelationalResultInputSeal, ResultEvidenceError, ResultEvidenceUpstreamRoot,
};
use super::result_projection::{
    IndexedResultProjectionRecord, ResultProjectionClosure, ResultProjectionRecordId,
    ResultProjectionRoot,
};
use super::result_view::{
    ClosedResultView, ResultViewCount, ResultViewCounts, ResultViewInputKind, ResultViewRoot,
    ResultViewSpec, ResultViewSpecRoot,
};
use super::structural_mechanism::{
    relational_structural_derivation_budget, ExecutionProfileId, StructuralMechanismCatalogBuilder,
    StructuralMechanismId, StructuralQuotientClosureReceipt, StructuralQuotientClosureRoot,
    StructuralSignatureQuotientArtifact, RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES,
};
use super::transition::TransitionId;

pub(crate) const RELATIONAL_ANALYSIS_EVENT_SCHEMA_VERSION: u32 = 8;
pub(crate) const RELATIONAL_SELECTED_QUESTION_SEAL_VERSION: u32 = 2;
pub(crate) const RELATIONAL_MECHANISM_ARTIFACT_VERSION: u32 = 1;
pub(crate) const RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES: usize = 32 << 10;
pub(crate) const RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES: usize = 64 << 10;
pub(crate) const RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS: usize = 65_536;
pub(crate) const RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES: usize = 512 << 20;

const ANALYSIS_EVENT_HASH_V8: &[u8] = b"futuruna.explore.relational-analysis-event.v8";
const ANALYSIS_SCOPE_ROOT_HASH_V1: &[u8] = b"futuruna.explore.relational-analysis-journal-scope.v1";
const ANALYSIS_CLOSURE_SET_ROOT_HASH_V1: &[u8] =
    b"futuruna.explore.relational-analysis-closure-set-root.v1";
const SELECTED_QUESTION_SEAL_ID_HASH_V2: &[u8] =
    b"futuruna.explore.relational-selected-question-seal.v2";
const MECHANISM_ARTIFACT_ID_HASH_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-artifact-id.v1";
const MECHANISM_ARTIFACT_CHUNK_HASH_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-artifact-chunk.v1";
const MECHANISM_ARTIFACT_CHUNK_ROOT_HASH_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-artifact-chunk-root.v1";

/// Canonical digest of one answer-defining analysis event. The outer journal
/// includes this digest in its single ordered hash chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalAnalysisEvidenceEventDigest([u8; 32]);

impl RelationalAnalysisEvidenceEventDigest {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of the compact selected-question closure receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalSelectedQuestionSealId([u8; 32]);

impl RelationalSelectedQuestionSealId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Compact private-mint receipt retained after the exact FIND catalog closes.
///
/// Result-input and mechanism-target domains deliberately use different set
/// roots. Both commitments are minted from the same immutable selected-case
/// iterator here. Later replay independently compares each local accumulated
/// set against the commitment appropriate to that layer; a serialized count
/// can never close a frontier by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedQuestionSeal {
    version: u32,
    id: RelationalSelectedQuestionSealId,
    question_id: QuestionId,
    authority: RelationalSelectedPopulationAuthority,
    result_input_seal: RelationalResultInputSeal,
    mechanism_target: MechanismTargetCaseSetCommitment,
}

/// Typed authority for an exact selected population. Support-certified
/// populations are not coerced into an extensional [`QuestionContentRoot`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSelectedPopulationAuthority {
    ExtensionalQuestion {
        content_root: QuestionContentRoot,
    },
    CertifiedSupport {
        population_root: CertifiedSelectedPopulationRoot,
        exact_cardinality: u128,
    },
}

impl RelationalSelectedQuestionSeal {
    pub(super) fn restore_from_journal_codec(
        version: u32,
        question_id: QuestionId,
        authority: RelationalSelectedPopulationAuthority,
        result_input_seal: RelationalResultInputSeal,
        mechanism_target: MechanismTargetCaseSetCommitment,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let id = derive_selected_question_seal_id(
            version,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        );
        let restored = Self {
            version,
            id,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        };
        restored.validate_identity()?;
        Ok(restored)
    }

    pub(crate) fn from_closed_question(
        question: &QuestionCatalog,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let version = RELATIONAL_SELECTED_QUESTION_SEAL_VERSION;
        let question_id = question.question_id();
        let authority = RelationalSelectedPopulationAuthority::ExtensionalQuestion {
            content_root: question.content_root(),
        };
        let result_input_seal = RelationalResultInputSeal::from_selected(question)?;
        let mechanism_target =
            MechanismTargetCaseSetCommitment::from_cases(question.selected_case_ids());
        let id = derive_selected_question_seal_id(
            version,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        );
        Ok(Self {
            version,
            id,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        })
    }

    /// Mint the same extensional receipt from a validated closure over the
    /// journal's borrowed builders. No relation, admission, question, or
    /// selected-case catalog is cloned or moved across this boundary.
    pub(crate) fn from_borrowed_closed_question(
        question: &ClosedQuestionCatalogRef<'_>,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let version = RELATIONAL_SELECTED_QUESTION_SEAL_VERSION;
        let question_id = question.question_id();
        let authority = RelationalSelectedPopulationAuthority::ExtensionalQuestion {
            content_root: question.content_root(),
        };
        let result_input_seal = RelationalResultInputSeal::from_borrowed_selected(question);
        let mechanism_target = MechanismTargetCaseSetCommitment::from_borrowed_selected(question);
        let id = derive_selected_question_seal_id(
            version,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        );
        Ok(Self {
            version,
            id,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        })
    }

    /// Bind a closed support proof to the exact concrete selected cases that
    /// sparse run materialization retained. The proof remains the population
    /// authority; the real CaseIds are independently committed for result and
    /// mechanism consumers. A support cell is never substituted for a case.
    pub(crate) fn from_certified_population(
        population: &ClosedCertifiedSelectedPopulation,
        selected_case_ids: impl IntoIterator<Item = RelationalCaseId>,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let selected_case_ids = selected_case_ids.into_iter().collect::<BTreeSet<_>>();
        if selected_case_ids.len() as u128 != population.exact_cardinality() {
            return Err(RelationalAnalysisJournalError::InvalidCertifiedSupportPopulation);
        }
        let version = RELATIONAL_SELECTED_QUESTION_SEAL_VERSION;
        let question_id = population.question_id();
        let authority = RelationalSelectedPopulationAuthority::CertifiedSupport {
            population_root: population.root(),
            exact_cardinality: population.exact_cardinality(),
        };
        let result_input_seal = RelationalResultInputSeal::from_certified_selected_population(
            population,
            selected_case_ids.iter().copied(),
        )?;
        let mechanism_target =
            MechanismTargetCaseSetCommitment::from_cases(selected_case_ids.iter().copied());
        let id = derive_selected_question_seal_id(
            version,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        );
        Ok(Self {
            version,
            id,
            question_id,
            authority,
            result_input_seal,
            mechanism_target,
        })
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn id(self) -> RelationalSelectedQuestionSealId {
        self.id
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn authority(self) -> RelationalSelectedPopulationAuthority {
        self.authority
    }

    pub(crate) const fn result_input_seal(self) -> RelationalResultInputSeal {
        self.result_input_seal
    }

    pub(crate) const fn mechanism_target(self) -> MechanismTargetCaseSetCommitment {
        self.mechanism_target
    }

    pub(crate) fn validate_identity(self) -> Result<(), RelationalAnalysisJournalError> {
        if self.version != RELATIONAL_SELECTED_QUESTION_SEAL_VERSION {
            return Err(
                RelationalAnalysisJournalError::UnsupportedSelectedQuestionSealVersion {
                    actual: self.version,
                    expected: RELATIONAL_SELECTED_QUESTION_SEAL_VERSION,
                },
            );
        }
        let coverage = self.result_input_seal.coverage();
        let upstream_matches = match (self.authority, self.result_input_seal.upstream()) {
            (
                RelationalSelectedPopulationAuthority::ExtensionalQuestion {
                    content_root: expected_root,
                },
                ResultEvidenceUpstreamRoot::Selected {
                    question_id,
                    content_root,
                },
            ) => question_id == self.question_id && content_root == expected_root,
            (
                RelationalSelectedPopulationAuthority::CertifiedSupport {
                    population_root: expected_population_root,
                    exact_cardinality: expected_cardinality,
                },
                ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
                    question_id,
                    population_root,
                    exact_cardinality,
                },
            ) => {
                question_id == self.question_id
                    && population_root == expected_population_root
                    && exact_cardinality == expected_cardinality
                    && exact_cardinality == coverage.row_count()
            }
            _ => false,
        };
        if !upstream_matches
            || coverage.input_kind() != ResultViewInputKind::Case
            || coverage.row_count() != self.mechanism_target.count()
        {
            return Err(RelationalAnalysisJournalError::InvalidSelectedQuestionSeal);
        }
        let derived = derive_selected_question_seal_id(
            self.version,
            self.question_id,
            self.authority,
            self.result_input_seal,
            self.mechanism_target,
        );
        if derived != self.id {
            return Err(
                RelationalAnalysisJournalError::SelectedQuestionSealIdMismatch {
                    claimed: self.id,
                    derived,
                },
            );
        }
        Ok(())
    }
}

/// Semantic scope an outer journal must bind once when analysis replay starts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalAnalysisJournalScopeRoot([u8; 32]);

impl RelationalAnalysisJournalScopeRoot {
    fn derive(
        plan_root: RelationalAnalysisPlanRoot,
        question_seal_id: RelationalSelectedQuestionSealId,
    ) -> Self {
        let mut hasher = AnalysisEventHasher::new(ANALYSIS_SCOPE_ROOT_HASH_V1);
        hasher.u32(RELATIONAL_ANALYSIS_EVENT_SCHEMA_VERSION);
        hasher.digest(plan_root.bytes());
        hasher.digest(question_seal_id.bytes());
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical request -> closure commitment for every mechanism layer in one
/// analysis plan. The raw-incidence, structural-quotient, and factorized
/// support roots stay independently typed; this root only binds their exact
/// request-local composition into the terminal analysis evidence claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalAnalysisClosureSetRoot([u8; 32]);

impl RelationalAnalysisClosureSetRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Typed identity of one private mechanism payload. Chunk size and chunk
/// count are deliberately absent: the complete canonical payload digest and
/// semantic claim define the artifact, while chunking remains an operational
/// transport choice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismArtifactId([u8; 32]);

impl RelationalMechanismArtifactId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismArtifactClaim {
    Signature {
        request_id: MechanismRequestId,
        signature_id: MechanismSignatureId,
    },
    Incidence {
        request_id: MechanismRequestId,
        observation_id: RelationalMechanismObservationId,
        observation_digest: RelationalMechanismObservationDigest,
        replay_observation_id: RelationalMechanismReplayObservationId,
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        signature_id: MechanismSignatureId,
        replay_receipt_id: RelationalMechanismReplayReceiptId,
    },
    Unavailable {
        request_id: MechanismRequestId,
        observation_id: RelationalMechanismObservationId,
        observation_digest: RelationalMechanismObservationDigest,
        replay_observation_id: RelationalMechanismReplayObservationId,
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        reason_id: MechanismUnavailableReasonId,
    },
    StructuralQuotient {
        request_id: MechanismRequestId,
        raw_signature_id: MechanismSignatureId,
        structural_mechanism_id: StructuralMechanismId,
        execution_profile_id: ExecutionProfileId,
    },
}

impl RelationalMechanismArtifactClaim {
    pub(crate) const fn request_id(self) -> MechanismRequestId {
        match self {
            Self::Signature { request_id, .. }
            | Self::Incidence { request_id, .. }
            | Self::Unavailable { request_id, .. }
            | Self::StructuralQuotient { request_id, .. } => request_id,
        }
    }

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Signature { .. } => 0x01,
            Self::Incidence { .. } => 0x02,
            Self::Unavailable { .. } => 0x03,
            Self::StructuralQuotient { .. } => 0x04,
        }
    }
}

/// Fixed-size opening record for one non-interleaved artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismArtifactHeader {
    version: u32,
    id: RelationalMechanismArtifactId,
    claim: RelationalMechanismArtifactClaim,
    payload_digest: [u8; 32],
    total_bytes: u64,
}

impl RelationalMechanismArtifactHeader {
    pub(super) fn restore_from_journal_codec(
        version: u32,
        id: RelationalMechanismArtifactId,
        claim: RelationalMechanismArtifactClaim,
        payload_digest: [u8; 32],
        total_bytes: u64,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let header = Self {
            version,
            id,
            claim,
            payload_digest,
            total_bytes,
        };
        header.validate_identity()?;
        Ok(header)
    }

    fn issue(claim: RelationalMechanismArtifactClaim, payload: &[u8]) -> Self {
        let payload_digest = Sha256::digest(payload).into();
        let total_bytes = payload.len() as u64;
        let version = RELATIONAL_MECHANISM_ARTIFACT_VERSION;
        let id = derive_mechanism_artifact_id(version, claim, payload_digest, total_bytes);
        Self {
            version,
            id,
            claim,
            payload_digest,
            total_bytes,
        }
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn id(self) -> RelationalMechanismArtifactId {
        self.id
    }

    pub(crate) const fn claim(self) -> RelationalMechanismArtifactClaim {
        self.claim
    }

    pub(crate) const fn payload_digest(self) -> [u8; 32] {
        self.payload_digest
    }

    pub(crate) const fn total_bytes(self) -> u64 {
        self.total_bytes
    }

    fn validate_identity(self) -> Result<(), RelationalAnalysisJournalError> {
        if self.version != RELATIONAL_MECHANISM_ARTIFACT_VERSION {
            return Err(
                RelationalAnalysisJournalError::UnsupportedMechanismArtifactVersion {
                    actual: self.version,
                    expected: RELATIONAL_MECHANISM_ARTIFACT_VERSION,
                },
            );
        }
        let max_bytes = mechanism_artifact_max_bytes(self.claim);
        let total_bytes = usize::try_from(self.total_bytes).map_err(|_| {
            RelationalAnalysisJournalError::MechanismArtifactCapacity {
                actual: usize::MAX,
                limit: max_bytes,
            }
        })?;
        if total_bytes == 0 || total_bytes > max_bytes {
            return Err(RelationalAnalysisJournalError::MechanismArtifactCapacity {
                actual: total_bytes,
                limit: max_bytes,
            });
        }
        match self.claim {
            RelationalMechanismArtifactClaim::Signature {
                request_id,
                signature_id,
            }
            | RelationalMechanismArtifactClaim::Incidence {
                request_id,
                signature_id,
                ..
            } if signature_id.request_id() != request_id => {
                return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "mechanism artifact signature scope",
                ));
            }
            RelationalMechanismArtifactClaim::StructuralQuotient {
                request_id,
                raw_signature_id,
                ..
            } if raw_signature_id.request_id() != request_id => {
                return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "structural quotient raw-signature scope",
                ));
            }
            _ => {}
        }
        let derived = derive_mechanism_artifact_id(
            self.version,
            self.claim,
            self.payload_digest,
            self.total_bytes,
        );
        if derived != self.id {
            return Err(RelationalAnalysisJournalError::MechanismArtifactIdMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismArtifactChunk {
    artifact_id: RelationalMechanismArtifactId,
    ordinal: u32,
    offset: u64,
    chunk_digest: [u8; 32],
    bytes: Box<[u8]>,
}

impl RelationalMechanismArtifactChunk {
    pub(super) fn restore_from_journal_codec(
        artifact_id: RelationalMechanismArtifactId,
        ordinal: u32,
        offset: u64,
        chunk_digest: [u8; 32],
        bytes: impl Into<Box<[u8]>>,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let chunk = Self {
            artifact_id,
            ordinal,
            offset,
            chunk_digest,
            bytes: bytes.into(),
        };
        chunk.validate_identity()?;
        Ok(chunk)
    }

    fn issue(
        artifact_id: RelationalMechanismArtifactId,
        ordinal: u32,
        offset: u64,
        bytes: &[u8],
    ) -> Self {
        let chunk_digest =
            derive_mechanism_artifact_chunk_digest(artifact_id, ordinal, offset, bytes);
        Self {
            artifact_id,
            ordinal,
            offset,
            chunk_digest,
            bytes: bytes.to_vec().into_boxed_slice(),
        }
    }

    pub(crate) const fn artifact_id(&self) -> RelationalMechanismArtifactId {
        self.artifact_id
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) const fn chunk_digest(&self) -> [u8; 32] {
        self.chunk_digest
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn validate_identity(&self) -> Result<(), RelationalAnalysisJournalError> {
        if self.bytes.is_empty() || self.bytes.len() > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES
        {
            return Err(
                RelationalAnalysisJournalError::MechanismArtifactChunkCapacity {
                    actual: self.bytes.len(),
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES,
                },
            );
        }
        let derived = derive_mechanism_artifact_chunk_digest(
            self.artifact_id,
            self.ordinal,
            self.offset,
            &self.bytes,
        );
        if derived != self.chunk_digest {
            return Err(RelationalAnalysisJournalError::MechanismArtifactChunkDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismArtifactChunkRoot([u8; 32]);

impl RelationalMechanismArtifactChunkRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismArtifactClosure {
    artifact_id: RelationalMechanismArtifactId,
    chunk_count: u32,
    chunk_root: RelationalMechanismArtifactChunkRoot,
}

impl RelationalMechanismArtifactClosure {
    pub(super) fn restore_from_journal_codec(
        artifact_id: RelationalMechanismArtifactId,
        chunk_count: u32,
        chunk_root: RelationalMechanismArtifactChunkRoot,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let closure = Self {
            artifact_id,
            chunk_count,
            chunk_root,
        };
        closure.validate_shape()?;
        Ok(closure)
    }

    fn validate_shape(self) -> Result<(), RelationalAnalysisJournalError> {
        let chunk_count = usize::try_from(self.chunk_count).map_err(|_| {
            RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                actual: usize::MAX,
                limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
            }
        })?;
        if chunk_count == 0 || chunk_count > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS {
            return Err(
                RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                    actual: chunk_count,
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                },
            );
        }
        Ok(())
    }

    pub(crate) const fn artifact_id(self) -> RelationalMechanismArtifactId {
        self.artifact_id
    }

    pub(crate) const fn chunk_count(self) -> u32 {
        self.chunk_count
    }

    pub(crate) const fn chunk_root(self) -> RelationalMechanismArtifactChunkRoot {
        self.chunk_root
    }
}

/// One answer-defining mutation of the post-FIND analysis DAG.
///
/// Scheduler cursors, resource decisions, retry state, retained examples, and
/// invocation deadlines are intentionally not variants of this enum. They
/// belong to the outer journal's checkpoint event class and do not affect
/// [`RelationalAnalysisCatalogRoot`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisEvidenceEvent {
    /// Bind the exact closed FIND result before post-FIND evidence. Keeping
    /// the compact receipt in the semantic stream makes durable replay
    /// independent of already-consumed base-stage builders.
    SelectedQuestionBound {
        seal_id: RelationalSelectedQuestionSealId,
        seal: RelationalSelectedQuestionSeal,
    },
    ResultSpecRegistered {
        view_id: ViewId,
        resolved_input: RelationalResolvedResultInput,
        spec_root: ResultViewSpecRoot,
        spec: ResultViewSpec,
    },
    ResultEvidenceAccepted {
        view_id: ViewId,
        evidence_id: RelationalResultEvidenceId,
        record: RelationalResultEvidenceRecord,
    },
    ResultInputSealedFromSources {
        view_id: ViewId,
        seal: RelationalResultInputSeal,
    },
    /// Bind one proof-specialized source result and atomically seal its exact
    /// logical input without materializing or inventing SourceKeys.
    CertifiedSourceSummaryAccepted {
        view_id: ViewId,
        artifact_id: RelationalCertifiedSourceSummaryArtifactId,
        artifact: Box<RelationalCertifiedSourceSummaryArtifact>,
    },
    ResultInputSealedFromSelected {
        view_id: ViewId,
        question_seal_id: RelationalSelectedQuestionSealId,
    },
    ResultInputSealedFromMechanisms {
        view_id: ViewId,
        request_id: MechanismRequestId,
        incidence_root: MechanismIncidenceRoot,
        structural_root: StructuralQuotientClosureRoot,
    },
    ResultProjectionRecordAccepted {
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        ordinal: u128,
        record_id: ResultProjectionRecordId,
        record: Box<IndexedResultProjectionRecord>,
    },
    ResultViewPublished {
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        evidence_root: RelationalResultEvidenceRoot,
        projection_root: ResultProjectionRoot,
        result_root: ResultViewRoot,
        closure: ResultProjectionClosure,
    },
    MechanismTargetCaseAccepted {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    MechanismTargetSealedFromSelected {
        request_id: MechanismRequestId,
        question_seal_id: RelationalSelectedQuestionSealId,
    },
    MechanismTargetSealedFromResult {
        request_id: MechanismRequestId,
        view_id: ViewId,
        result_root: ResultViewRoot,
    },
    MechanismArtifactOpened {
        header: RelationalMechanismArtifactHeader,
    },
    MechanismArtifactChunkAccepted {
        chunk: RelationalMechanismArtifactChunk,
    },
    MechanismArtifactClosed {
        closure: RelationalMechanismArtifactClosure,
    },
    MechanismIncidenceClosed {
        request_id: MechanismRequestId,
        incidence_root: MechanismIncidenceRoot,
    },
    StructuralQuotientClosed {
        request_id: MechanismRequestId,
        structural_root: StructuralQuotientClosureRoot,
    },
    SupportClosed {
        request_id: MechanismRequestId,
        support_root: MechanismSupportClosureRoot,
    },
    AnalysisClosed {
        catalog_root: RelationalAnalysisCatalogRoot,
        closure_set_root: RelationalAnalysisClosureSetRoot,
    },
}

impl RelationalAnalysisEvidenceEvent {
    pub(crate) const fn selected_question_bound(seal: RelationalSelectedQuestionSeal) -> Self {
        Self::SelectedQuestionBound {
            seal_id: seal.id(),
            seal,
        }
    }

    pub(crate) fn result_spec_registered(
        resolved_input: RelationalResolvedResultInput,
        spec: ResultViewSpec,
    ) -> Self {
        Self::ResultSpecRegistered {
            view_id: spec.view_id(),
            resolved_input,
            spec_root: spec.spec_root(),
            spec,
        }
    }

    pub(crate) fn result_evidence_accepted(record: RelationalResultEvidenceRecord) -> Self {
        Self::ResultEvidenceAccepted {
            view_id: record.view_id(),
            evidence_id: record.id(),
            record,
        }
    }

    pub(crate) const fn result_input_sealed_from_sources(
        view_id: ViewId,
        seal: RelationalResultInputSeal,
    ) -> Self {
        Self::ResultInputSealedFromSources { view_id, seal }
    }

    pub(crate) fn certified_source_summary_accepted(
        artifact: RelationalCertifiedSourceSummaryArtifact,
    ) -> Self {
        Self::CertifiedSourceSummaryAccepted {
            view_id: artifact.view_id(),
            artifact_id: artifact.artifact_id(),
            artifact: Box::new(artifact),
        }
    }

    pub(crate) const fn result_input_sealed_from_selected(
        view_id: ViewId,
        question: RelationalSelectedQuestionSeal,
    ) -> Self {
        Self::ResultInputSealedFromSelected {
            view_id,
            question_seal_id: question.id(),
        }
    }

    pub(crate) const fn result_input_sealed_from_mechanisms(
        view_id: ViewId,
        closure: RelationalMechanismClosureReceipt,
        structural_closure: StructuralQuotientClosureReceipt,
    ) -> Self {
        Self::ResultInputSealedFromMechanisms {
            view_id,
            request_id: closure.request_id(),
            incidence_root: closure.incidence_root(),
            structural_root: structural_closure.root(),
        }
    }

    pub(crate) fn result_projection_record_accepted(
        view_id: ViewId,
        record: IndexedResultProjectionRecord,
    ) -> Self {
        Self::ResultProjectionRecordAccepted {
            view_id,
            spec_root: record.spec_root(),
            ordinal: record.ordinal(),
            record_id: record.id(),
            record: Box::new(record),
        }
    }

    pub(crate) fn result_view_published(
        catalog: &RelationalAnalysisCatalogBuilder,
        view: ClosedResultView,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let view_id = view.view_id();
        let spec_root = catalog.result_spec(view_id)?.spec_root();
        let evidence_root = catalog.result_evidence_root(view_id)?;
        let closure = catalog.prepare_result_projection_closure(&view)?;
        Ok(Self::ResultViewPublished {
            view_id,
            spec_root,
            evidence_root,
            projection_root: closure.projection_root(),
            result_root: closure.result_root(),
            closure,
        })
    }

    /// Close directly from the complete bounded durable projection prefix.
    /// The catalog hashes borrowed exact evidence plus the materialized output
    /// without invoking an expression runtime or retaining a full view.
    pub(crate) fn durable_result_view_published(
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let spec_root = catalog.result_spec(view_id)?.spec_root();
        let evidence_root = catalog.result_evidence_root(view_id)?;
        let closure = catalog.prepare_durable_result_projection_closure(view_id)?;
        Ok(Self::ResultViewPublished {
            view_id,
            spec_root,
            evidence_root,
            projection_root: closure.projection_root(),
            result_root: closure.result_root(),
            closure,
        })
    }

    pub(crate) fn certified_source_result_view_published(
        catalog: &RelationalAnalysisCatalogBuilder,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let view_id = artifact.view_id();
        let spec_root = catalog.result_spec(view_id)?.spec_root();
        let evidence_root = catalog.result_evidence_root(view_id)?;
        let closure = catalog.prepare_certified_source_projection_closure(artifact)?;
        Ok(Self::ResultViewPublished {
            view_id,
            spec_root,
            evidence_root,
            projection_root: closure.projection_root(),
            result_root: closure.result_root(),
            closure,
        })
    }

    pub(crate) const fn mechanism_target_case_accepted(
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    ) -> Self {
        Self::MechanismTargetCaseAccepted {
            request_id,
            case_id,
        }
    }

    pub(crate) const fn mechanism_target_sealed_from_selected(
        request_id: MechanismRequestId,
        question: RelationalSelectedQuestionSeal,
    ) -> Self {
        Self::MechanismTargetSealedFromSelected {
            request_id,
            question_seal_id: question.id(),
        }
    }

    pub(crate) const fn mechanism_target_sealed_from_result(
        request_id: MechanismRequestId,
        view: &ClosedResultView,
    ) -> Self {
        Self::MechanismTargetSealedFromResult {
            request_id,
            view_id: view.view_id(),
            result_root: view.root(),
        }
    }

    pub(crate) fn mechanism_signature_artifact_events(
        definition: &MechanismSignatureDefinition,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        Self::mechanism_signature_artifact_events_with_chunk_bytes(
            definition,
            RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES,
        )
    }

    pub(crate) fn mechanism_signature_artifact_events_with_chunk_bytes(
        definition: &MechanismSignatureDefinition,
        chunk_bytes: usize,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        build_mechanism_artifact_events(
            RelationalMechanismArtifactClaim::Signature {
                request_id: definition.id().request_id(),
                signature_id: definition.id(),
            },
            definition.canonical_definition(),
            chunk_bytes,
        )
    }

    pub(crate) fn mechanism_incidence_artifact_events(
        contract: RelationalMechanismEvidenceContract,
        evidence: &RelationalMechanismReplayEvidence,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        Self::mechanism_incidence_artifact_events_with_chunk_bytes(
            contract,
            evidence,
            RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES,
        )
    }

    pub(crate) fn mechanism_incidence_artifact_events_with_chunk_bytes(
        contract: RelationalMechanismEvidenceContract,
        evidence: &RelationalMechanismReplayEvidence,
        chunk_bytes: usize,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        evidence.validate_identity()?;
        if evidence.scope() != contract.scope() {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism replay scope",
            ));
        }
        let claim = RelationalMechanismArtifactClaim::Incidence {
            request_id: contract.scope().request_id(),
            observation_id: contract.observation_id(),
            observation_digest: contract.observation_digest(),
            replay_observation_id: evidence.observation_id(),
            case_id: evidence.case_id(),
            transition_id: evidence.transition_id(),
            signature_id: evidence.signature_id(),
            replay_receipt_id: evidence.receipt().id(),
        };
        let payload = evidence.canonical_durable_payload()?;
        build_mechanism_artifact_events(claim, &payload, chunk_bytes)
    }

    pub(crate) fn mechanism_compact_incidence_artifact_events_with_chunk_bytes(
        contract: RelationalMechanismEvidenceContract,
        evidence: &RelationalMechanismReplayEvidence,
        chunk_bytes: usize,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        evidence.validate_identity()?;
        if evidence.scope() != contract.scope() {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism replay scope",
            ));
        }
        let claim = RelationalMechanismArtifactClaim::Incidence {
            request_id: contract.scope().request_id(),
            observation_id: contract.observation_id(),
            observation_digest: contract.observation_digest(),
            replay_observation_id: evidence.observation_id(),
            case_id: evidence.case_id(),
            transition_id: evidence.transition_id(),
            signature_id: evidence.signature_id(),
            replay_receipt_id: evidence.receipt().id(),
        };
        let payload = evidence.canonical_compact_incidence_durable_payload()?;
        build_mechanism_artifact_events(claim, &payload, chunk_bytes)
    }

    pub(crate) fn mechanism_unavailable_artifact_events(
        contract: RelationalMechanismEvidenceContract,
        evidence: &RelationalMechanismUnavailableEvidence,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        Self::mechanism_unavailable_artifact_events_with_chunk_bytes(
            contract,
            evidence,
            RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES,
        )
    }

    pub(crate) fn mechanism_unavailable_artifact_events_with_chunk_bytes(
        contract: RelationalMechanismEvidenceContract,
        evidence: &RelationalMechanismUnavailableEvidence,
        chunk_bytes: usize,
    ) -> Result<Box<[Self]>, RelationalAnalysisJournalError> {
        evidence.validate_identity()?;
        if evidence.scope() != contract.scope() {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism unavailable scope",
            ));
        }
        build_mechanism_artifact_events(
            RelationalMechanismArtifactClaim::Unavailable {
                request_id: contract.scope().request_id(),
                observation_id: contract.observation_id(),
                observation_digest: contract.observation_digest(),
                replay_observation_id: evidence.observation_id(),
                case_id: evidence.case_id(),
                transition_id: evidence.transition_id(),
                reason_id: evidence.reason_id(),
            },
            evidence.canonical_reason(),
            chunk_bytes,
        )
    }

    pub(crate) const fn mechanism_incidence_closed(
        closure: RelationalMechanismClosureReceipt,
    ) -> Self {
        Self::MechanismIncidenceClosed {
            request_id: closure.request_id(),
            incidence_root: closure.incidence_root(),
        }
    }

    pub(crate) const fn structural_quotient_closed(
        closure: StructuralQuotientClosureReceipt,
    ) -> Self {
        Self::StructuralQuotientClosed {
            request_id: closure.request_id(),
            structural_root: closure.root(),
        }
    }

    pub(crate) const fn support_closed(closure: MechanismSupportClosureReceipt) -> Self {
        Self::SupportClosed {
            request_id: closure.request_id(),
            support_root: closure.root(),
        }
    }

    pub(crate) const fn analysis_closed(
        catalog_root: RelationalAnalysisCatalogRoot,
        closure_set_root: RelationalAnalysisClosureSetRoot,
    ) -> Self {
        Self::AnalysisClosed {
            catalog_root,
            closure_set_root,
        }
    }

    /// Hash one answer-defining analysis record. Mechanism payload chunks are
    /// independently collision-checkable; their compact header binds the full
    /// typed payload digest while their closure binds the exact chunk stream.
    pub(crate) fn digest(&self) -> RelationalAnalysisEvidenceEventDigest {
        let mut hasher = AnalysisEventHasher::new(ANALYSIS_EVENT_HASH_V8);
        hasher.u32(RELATIONAL_ANALYSIS_EVENT_SCHEMA_VERSION);
        match self {
            Self::SelectedQuestionBound { seal_id, .. } => {
                hasher.tag(0x00);
                hasher.digest(seal_id.bytes());
            }
            Self::ResultSpecRegistered {
                view_id,
                resolved_input,
                spec_root,
                ..
            } => {
                hasher.tag(0x01);
                hasher.digest(view_id.bytes());
                hash_result_input(&mut hasher, *resolved_input);
                hasher.digest(spec_root.bytes());
            }
            Self::ResultEvidenceAccepted {
                view_id,
                evidence_id,
                ..
            } => {
                hasher.tag(0x02);
                hasher.digest(view_id.bytes());
                hasher.digest(evidence_id.bytes());
            }
            Self::ResultInputSealedFromSources { view_id, seal } => {
                hasher.tag(0x0f);
                hasher.digest(view_id.bytes());
                hash_result_input_seal(&mut hasher, *seal);
            }
            Self::CertifiedSourceSummaryAccepted {
                view_id,
                artifact_id,
                ..
            } => {
                hasher.tag(0x10);
                hasher.digest(view_id.bytes());
                hasher.digest(artifact_id.bytes());
            }
            Self::ResultInputSealedFromSelected {
                view_id,
                question_seal_id,
            } => {
                hasher.tag(0x03);
                hasher.digest(view_id.bytes());
                hasher.digest(question_seal_id.bytes());
            }
            Self::ResultInputSealedFromMechanisms {
                view_id,
                request_id,
                incidence_root,
                structural_root,
            } => {
                hasher.tag(0x04);
                hasher.digest(view_id.bytes());
                hasher.digest(request_id.bytes());
                hasher.digest(incidence_root.bytes());
                hasher.digest(structural_root.bytes());
            }
            Self::ResultProjectionRecordAccepted {
                view_id,
                spec_root,
                ordinal,
                record_id,
                ..
            } => {
                hasher.tag(0x0e);
                hasher.digest(view_id.bytes());
                hasher.digest(spec_root.bytes());
                hasher.u128(*ordinal);
                hasher.digest(record_id.bytes());
            }
            Self::ResultViewPublished {
                view_id,
                spec_root,
                evidence_root,
                projection_root,
                result_root,
                closure,
            } => {
                hasher.tag(0x05);
                hasher.digest(view_id.bytes());
                hasher.digest(spec_root.bytes());
                hasher.digest(evidence_root.bytes());
                hasher.digest(projection_root.bytes());
                hasher.digest(result_root.bytes());
                hasher.u128(closure.record_count());
                hash_result_counts(&mut hasher, closure.counts());
            }
            Self::MechanismTargetCaseAccepted {
                request_id,
                case_id,
            } => {
                hasher.tag(0x06);
                hasher.digest(request_id.bytes());
                hasher.digest(case_id.bytes());
            }
            Self::MechanismTargetSealedFromSelected {
                request_id,
                question_seal_id,
            } => {
                hasher.tag(0x07);
                hasher.digest(request_id.bytes());
                hasher.digest(question_seal_id.bytes());
            }
            Self::MechanismTargetSealedFromResult {
                request_id,
                view_id,
                result_root,
            } => {
                hasher.tag(0x08);
                hasher.digest(request_id.bytes());
                hasher.digest(view_id.bytes());
                hasher.digest(result_root.bytes());
            }
            Self::MechanismArtifactOpened { header } => {
                hasher.tag(0x09);
                hash_mechanism_artifact_header(&mut hasher, *header);
            }
            Self::MechanismArtifactChunkAccepted { chunk } => {
                hasher.tag(0x0a);
                hasher.digest(chunk.artifact_id().bytes());
                hasher.u32(chunk.ordinal());
                hasher.u128(u128::from(chunk.offset()));
                hasher.digest(chunk.chunk_digest());
                hasher.bytes(chunk.bytes());
            }
            Self::MechanismArtifactClosed { closure } => {
                hasher.tag(0x0b);
                hasher.digest(closure.artifact_id().bytes());
                hasher.u32(closure.chunk_count());
                hasher.digest(closure.chunk_root().bytes());
            }
            Self::MechanismIncidenceClosed {
                request_id,
                incidence_root,
            } => {
                hasher.tag(0x0c);
                hasher.digest(request_id.bytes());
                hasher.digest(incidence_root.bytes());
            }
            Self::StructuralQuotientClosed {
                request_id,
                structural_root,
            } => {
                hasher.tag(0x11);
                hasher.digest(request_id.bytes());
                hasher.digest(structural_root.bytes());
            }
            Self::SupportClosed {
                request_id,
                support_root,
            } => {
                hasher.tag(0x12);
                hasher.digest(request_id.bytes());
                hasher.digest(support_root.bytes());
            }
            Self::AnalysisClosed {
                catalog_root,
                closure_set_root,
            } => {
                hasher.tag(0x0d);
                hasher.digest(catalog_root.bytes());
                hasher.digest(closure_set_root.bytes());
            }
        }
        RelationalAnalysisEvidenceEventDigest(hasher.finish())
    }

    fn validate_claimed_content(&self) -> Result<(), RelationalAnalysisJournalError> {
        match self {
            Self::SelectedQuestionBound { seal_id, seal } if *seal_id != seal.id() => Err(
                RelationalAnalysisJournalError::EventClaimMismatch("selected-question seal"),
            ),
            Self::ResultSpecRegistered {
                view_id,
                spec_root,
                spec,
                ..
            } if *view_id != spec.view_id() || *spec_root != spec.spec_root() => Err(
                RelationalAnalysisJournalError::EventClaimMismatch("result spec"),
            ),
            Self::ResultEvidenceAccepted {
                view_id,
                evidence_id,
                record,
            } if *view_id != record.view_id() || *evidence_id != record.id() => Err(
                RelationalAnalysisJournalError::EventClaimMismatch("result evidence"),
            ),
            Self::CertifiedSourceSummaryAccepted {
                view_id,
                artifact_id,
                artifact,
            } if *view_id != artifact.view_id()
                || *artifact_id != artifact.artifact_id()
                || !artifact.validate_identity() =>
            {
                Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "certified source summary",
                ))
            }
            Self::ResultProjectionRecordAccepted {
                view_id,
                spec_root,
                ordinal,
                record_id,
                record,
                ..
            } if *view_id != record.view_id()
                || *spec_root != record.spec_root()
                || *ordinal != record.ordinal()
                || *record_id != record.id() =>
            {
                Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "result projection record",
                ))
            }
            Self::ResultViewPublished {
                view_id,
                spec_root,
                projection_root,
                result_root,
                closure,
                ..
            } if *view_id != closure.view_id()
                || *spec_root != closure.spec_root()
                || *projection_root != closure.projection_root()
                || *result_root != closure.result_root() =>
            {
                Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "result projection closure",
                ))
            }
            Self::MechanismArtifactOpened { header } => header.validate_identity(),
            Self::MechanismArtifactChunkAccepted { chunk } => chunk.validate_identity(),
            Self::MechanismArtifactClosed { closure } => closure.validate_shape(),
            _ => Ok(()),
        }
    }
}

/// Causal replay state for one plan and one exact selected-question closure.
///
/// Bounded projection records remain in their plan-bound catalog so a later
/// chosen-view target and final report are reconstructed from exact durable
/// output rather than a caller count. Completed mechanism payload remains in
/// the live catalog until final closure; only compact closure receipts are
/// retained beside it.
#[derive(Clone, Debug)]
struct PendingMechanismArtifact {
    header: RelationalMechanismArtifactHeader,
    chunks: Vec<RelationalMechanismArtifactChunk>,
    accepted_bytes: u64,
    /// Ephemeral producer cache only. Replay resets this bit and rechecks the
    /// complete deterministic structural payload against the durable header
    /// once before minting the next bounded suffix event. The cursor cells
    /// then authenticate each newly appended chunk exactly once.
    structural_producer_verified: Cell<bool>,
    structural_verified_chunk_count: Cell<usize>,
    structural_verified_bytes: Cell<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct RelationalAnalysisJournalState {
    plan_root: RelationalAnalysisPlanRoot,
    scope_root: Option<RelationalAnalysisJournalScopeRoot>,
    selected_question: Option<RelationalSelectedQuestionSeal>,
    open: Option<RelationalAnalysisCatalogBuilder>,
    /// Replay-derived exact result closures retained across final analysis
    /// closure for bounded reporting. The durable event is the authority; this
    /// index is not hashed into the catalog or journal identity.
    result_projection_closures: BTreeMap<ViewId, ResultProjectionClosure>,
    mechanism_closures: BTreeMap<MechanismRequestId, RelationalMechanismClosureReceipt>,
    /// Request-local structural quotient interners. These remain subordinate
    /// to raw incidence and become independently publishable only through an
    /// exact structural-closure event.
    structural_mechanisms: BTreeMap<MechanismRequestId, StructuralMechanismCatalogBuilder>,
    /// Request-local factorized support joins. Coordinates enter only through
    /// bounded or exact-cursor catch-up from checked relation cases; no
    /// semantic event manufactures source/successor coordinates from CaseIds.
    mechanism_supports: BTreeMap<MechanismRequestId, MechanismSupportCatalogBuilder>,
    support_closures: BTreeMap<MechanismRequestId, MechanismSupportClosureReceipt>,
    closed_closure_set_root: Option<RelationalAnalysisClosureSetRoot>,
    /// Replay-derived publication addressing retained across final analysis
    /// closure, but kept outside the semantic catalog snapshot and its root.
    closed_mechanism_publication_discoveries:
        Box<[(MechanismRequestId, MechanismPublicationDiscovery)]>,
    pending_mechanism_artifact: Option<PendingMechanismArtifact>,
    closed: Option<ClosedRelationalAnalysisCatalog>,
}

impl RelationalAnalysisJournalState {
    pub(crate) fn new(
        plan: &RelationalAnalysisPlan,
    ) -> Result<Self, RelationalAnalysisJournalError> {
        let open = RelationalAnalysisCatalogBuilder::new(plan)?;
        Ok(Self {
            plan_root: plan.root(),
            scope_root: None,
            selected_question: None,
            open: Some(open),
            result_projection_closures: BTreeMap::new(),
            mechanism_closures: BTreeMap::new(),
            structural_mechanisms: BTreeMap::new(),
            mechanism_supports: BTreeMap::new(),
            support_closures: BTreeMap::new(),
            closed_closure_set_root: None,
            closed_mechanism_publication_discoveries: Box::default(),
            pending_mechanism_artifact: None,
            closed: None,
        })
    }

    pub(crate) const fn scope_root(&self) -> Option<RelationalAnalysisJournalScopeRoot> {
        self.scope_root
    }

    pub(crate) const fn selected_question(&self) -> Option<RelationalSelectedQuestionSeal> {
        self.selected_question
    }

    pub(crate) fn certified_source_summary(
        &self,
        view_id: ViewId,
    ) -> Option<&RelationalCertifiedSourceSummaryArtifact> {
        match (&self.open, &self.closed) {
            (Some(open), None) => open.certified_source_summary(view_id).ok().flatten(),
            (None, Some(closed)) => closed.certified_source_summary(view_id),
            _ => None,
        }
    }

    pub(crate) const fn open_catalog(&self) -> Option<&RelationalAnalysisCatalogBuilder> {
        self.open.as_ref()
    }

    pub(crate) fn result_projection_closure(
        &self,
        view_id: ViewId,
    ) -> Option<ResultProjectionClosure> {
        self.result_projection_closures.get(&view_id).copied()
    }

    pub(crate) fn structural_mechanism_catalog(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<&StructuralMechanismCatalogBuilder> {
        self.structural_mechanisms.get(&request_id)
    }

    pub(crate) fn structural_quotient_closure(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<StructuralQuotientClosureReceipt> {
        self.structural_mechanisms
            .get(&request_id)
            .and_then(StructuralMechanismCatalogBuilder::closure)
    }

    pub(crate) fn mechanism_support_catalog(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<&MechanismSupportCatalogBuilder> {
        self.mechanism_supports.get(&request_id)
    }

    pub(crate) fn mechanism_support_closure(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismSupportClosureReceipt> {
        self.support_closures.get(&request_id).copied()
    }

    pub(crate) const fn closed_closure_set_root(&self) -> Option<RelationalAnalysisClosureSetRoot> {
        self.closed_closure_set_root
    }

    /// Return the exact derived-cache cursor and the currently visible
    /// upstream lane limits. Support is allowed to trail open raw and
    /// structural streams; an empty structural catalog is installed lazily so
    /// its zero-length prefix has the same authenticated resume semantics as
    /// every later assignment prefix.
    pub(crate) fn support_checkpoint_cursors(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<
        (
            MechanismSupportCheckpointCursor,
            MechanismSupportCheckpointCursor,
        ),
        RelationalAnalysisJournalError,
    > {
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        let (scope, target_count, terminal_count) = {
            let catalog = self
                .open
                .as_ref()
                .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
            let incidence = catalog.mechanism_incidence(request_id)?;
            (
                incidence.scope(),
                incidence.target_discovery_count(),
                incidence.terminal_discovery_count(),
            )
        };
        let structural = self
            .structural_mechanisms
            .entry(request_id)
            .or_insert_with(|| StructuralMechanismCatalogBuilder::new(request_id));
        let current = self
            .mechanism_supports
            .get(&request_id)
            .map_or_else(MechanismSupportCheckpointCursor::default, |support| {
                support.checkpoint_cursor()
            });
        let available = MechanismSupportCheckpointCursor::new(
            target_count as u128,
            terminal_count as u128,
            structural.assignment_discovery_count() as u128,
        );
        if self
            .mechanism_supports
            .get(&request_id)
            .is_some_and(|support| support.scope() != scope)
        {
            return Err(RelationalAnalysisJournalError::MechanismSupport(
                MechanismSupportError::RequestMismatch,
            ));
        }
        if current.target_discovery() > available.target_discovery() {
            return Err(RelationalAnalysisJournalError::MechanismSupport(
                MechanismSupportError::TargetDiscoveryCursorRegression,
            ));
        }
        if current.terminal_discovery() > available.terminal_discovery() {
            return Err(RelationalAnalysisJournalError::MechanismSupport(
                MechanismSupportError::TerminalDiscoveryCursorRegression,
            ));
        }
        if current.structural_assignment() > available.structural_assignment() {
            return Err(RelationalAnalysisJournalError::MechanismSupport(
                MechanismSupportError::StructuralAssignmentCursorRegression,
            ));
        }
        Ok((current, available))
    }

    /// Read-only scheduler hint: a request is ready when an imported lane
    /// trails its live upstream or when both upstream closures make the final
    /// checkpoint/close sequence possible. The lifecycle method remains the
    /// authority and revalidates every cursor/root before emitting anything.
    pub(crate) fn support_checkpoint_has_ready_work(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let current = self
            .mechanism_supports
            .get(&request_id)
            .map_or_else(MechanismSupportCheckpointCursor::default, |support| {
                support.checkpoint_cursor()
            });
        let available = MechanismSupportCheckpointCursor::new(
            incidence.target_discovery_count() as u128,
            incidence.terminal_discovery_count() as u128,
            self.structural_mechanisms
                .get(&request_id)
                .map_or(0, |structural| structural.assignment_discovery_count())
                as u128,
        );
        Ok(current != available
            || (self.mechanism_closures.contains_key(&request_id)
                && self.structural_quotient_closure(request_id).is_some()))
    }

    /// Advance each support lane by at most one bounded delta. Target
    /// coordinates are resolved only through the outer checked relation;
    /// terminal replay stops at the first coordinate not yet imported.
    pub(crate) fn advance_support_checkpoint_bounded<'case>(
        &mut self,
        request_id: MechanismRequestId,
        maximum_cases: NonZeroU16,
        mut resolve_case: impl FnMut(RelationalCaseId) -> Option<RelationalCaseRef<'case>>,
    ) -> Result<
        (
            usize,
            MechanismSupportCheckpointCursor,
            MechanismSupportCheckpointCursor,
        ),
        RelationalAnalysisJournalError,
    > {
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let structural = self
            .structural_mechanisms
            .entry(request_id)
            .or_insert_with(|| StructuralMechanismCatalogBuilder::new(request_id));
        let support = self
            .mechanism_supports
            .entry(request_id)
            .or_insert_with(|| MechanismSupportCatalogBuilder::new(incidence.scope()));
        let maximum_delta = usize::from(maximum_cases.get());
        let requested_target = support
            .target_discovery_cursor()
            .saturating_add(maximum_delta)
            .min(incidence.target_discovery_count());
        let mut accepted_targets = 0usize;
        for ordinal in support.target_discovery_cursor()..requested_target {
            let case_id = incidence
                .target_discovery_at(ordinal)
                .expect("target discovery count bounds its indexed prefix");
            let case = resolve_case(case_id).ok_or(
                RelationalAnalysisJournalError::SupportTargetCaseMissing {
                    request_id,
                    case_id,
                },
            )?;
            if case.case_id() != case_id {
                return Err(
                    RelationalAnalysisJournalError::SupportTargetCaseResolutionMismatch {
                        request_id,
                        expected: case_id,
                        actual: case.case_id(),
                    },
                );
            }
            if support.accept_target_case(incidence, case)? {
                accepted_targets = accepted_targets.checked_add(1).ok_or(
                    RelationalAnalysisJournalError::MechanismSupport(
                        MechanismSupportError::CountOverflow,
                    ),
                )?;
            }
        }

        let requested_structural = (support.checkpoint_cursor().structural_assignment())
            .saturating_add(maximum_delta as u128)
            .min(structural.assignment_discovery_count() as u128);
        support.sync_structural_assignments_through(structural, requested_structural)?;
        let requested_terminal =
            support.bounded_terminal_discovery_cursor(incidence, maximum_delta)?;
        support.sync_incidence_terminals_through(incidence, structural, requested_terminal)?;

        let current = support.checkpoint_cursor();
        let available = MechanismSupportCheckpointCursor::new(
            incidence.target_discovery_count() as u128,
            incidence.terminal_discovery_count() as u128,
            structural.assignment_discovery_count() as u128,
        );
        Ok((accepted_targets, current, available))
    }

    /// Rebuild exactly the target prefix named by a durable checkpoint. A
    /// replayed event may never pull in a later raw target merely because it
    /// is already visible in the surrounding incidence catalog.
    pub(crate) fn catch_up_support_targets_through<'case>(
        &mut self,
        request_id: MechanismRequestId,
        target_discovery_cursor: u128,
        mut resolve_case: impl FnMut(RelationalCaseId) -> Option<RelationalCaseRef<'case>>,
    ) -> Result<usize, RelationalAnalysisJournalError> {
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let support = self
            .mechanism_supports
            .entry(request_id)
            .or_insert_with(|| MechanismSupportCatalogBuilder::new(incidence.scope()));
        let current = support.target_discovery_cursor() as u128;
        let available = incidence.target_discovery_count() as u128;
        if target_discovery_cursor < current || target_discovery_cursor > available {
            return Err(
                RelationalAnalysisJournalError::SupportTargetCursorOutOfRange {
                    request_id,
                    requested: target_discovery_cursor,
                    current,
                    available,
                },
            );
        }
        let target_discovery_cursor = usize::try_from(target_discovery_cursor)
            .expect("a cursor bounded by an in-memory incidence count fits usize");
        let mut accepted = 0usize;
        for ordinal in support.target_discovery_cursor()..target_discovery_cursor {
            let case_id = incidence
                .target_discovery_at(ordinal)
                .expect("target discovery count bounds its indexed prefix");
            let case = resolve_case(case_id).ok_or(
                RelationalAnalysisJournalError::SupportTargetCaseMissing {
                    request_id,
                    case_id,
                },
            )?;
            if case.case_id() != case_id {
                return Err(
                    RelationalAnalysisJournalError::SupportTargetCaseResolutionMismatch {
                        request_id,
                        expected: case_id,
                        actual: case.case_id(),
                    },
                );
            }
            if support.accept_target_case(incidence, case)? {
                accepted = accepted.checked_add(1).ok_or(
                    RelationalAnalysisJournalError::MechanismSupport(
                        MechanismSupportError::CountOverflow,
                    ),
                )?;
            }
        }
        Ok(accepted)
    }

    /// Replay all three derived support lanes through exactly the authenticated
    /// checkpoint cursor. The outer journal validates per-lane deltas before
    /// calling this method, so no crafted event can turn exact replay into an
    /// unbounded catch-up.
    pub(crate) fn restore_support_checkpoint_through<'case>(
        &mut self,
        request_id: MechanismRequestId,
        cursor: MechanismSupportCheckpointCursor,
        resolve_case: impl FnMut(RelationalCaseId) -> Option<RelationalCaseRef<'case>>,
    ) -> Result<usize, RelationalAnalysisJournalError> {
        // Establish the request-local open upstreams before importing any
        // lane. Closure is deliberately not a checkpoint prerequisite.
        {
            let catalog = self
                .open
                .as_ref()
                .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
            catalog.mechanism_incidence(request_id)?;
        }
        self.structural_mechanisms
            .entry(request_id)
            .or_insert_with(|| StructuralMechanismCatalogBuilder::new(request_id));
        let accepted_targets = self.catch_up_support_targets_through(
            request_id,
            cursor.target_discovery(),
            resolve_case,
        )?;
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let structural = self
            .structural_mechanisms
            .get(&request_id)
            .expect("open support checkpoint restore installed structural state");
        let support = self
            .mechanism_supports
            .get_mut(&request_id)
            .expect("target checkpoint restore installs request-local support state");
        support.sync_structural_assignments_through(structural, cursor.structural_assignment())?;
        support.sync_incidence_terminals_through(
            incidence,
            structural,
            cursor.terminal_discovery(),
        )?;
        if support.checkpoint_cursor() != cursor {
            return Err(RelationalAnalysisJournalError::MechanismSupport(
                MechanismSupportError::ClosurePrerequisite("exact support checkpoint cursors"),
            ));
        }
        Ok(accepted_targets)
    }

    /// Mint the resumable request-level support frontier for the exact
    /// three-lane cursor already imported into derived state. This never
    /// advances a lane; the outer journal trusts the returned root only after
    /// it matches the authenticated multi-lane checkpoint.
    pub(crate) fn checkpoint_support_frontier(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<MechanismSupportFrontierSummary, RelationalAnalysisJournalError> {
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let closed_incidence_root = self
            .mechanism_closures
            .get(&request_id)
            .copied()
            .map(|closure| closure.incidence_root());
        let structural = self
            .structural_mechanisms
            .entry(request_id)
            .or_insert_with(|| StructuralMechanismCatalogBuilder::new(request_id));
        let structural_closure_root = structural.closure().map(|closure| closure.root());
        let support = self
            .mechanism_supports
            .entry(request_id)
            .or_insert_with(|| MechanismSupportCatalogBuilder::new(incidence.scope()));
        Ok(support.checkpoint_frontier(
            incidence,
            closed_incidence_root,
            structural,
            structural_closure_root,
        )?)
    }

    /// Derive a structural-close claim without mutating semantic replay state.
    /// Applying the returned event repeats the derivation and commits the
    /// closed candidate atomically.
    pub(crate) fn structural_quotient_closure_event(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError> {
        let (_, receipt) = self.prepare_structural_quotient_closure(request_id)?;
        Ok(RelationalAnalysisEvidenceEvent::structural_quotient_closed(
            receipt,
        ))
    }

    /// Derive a support-close claim without committing semantic closure.
    /// Bounded or exact-cursor catch-up must first import the complete target,
    /// terminal, and structural-assignment prefixes; the outer journal
    /// separately requires that full frontier to have a durable checkpoint.
    pub(crate) fn support_closure_event(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError> {
        let receipt = self.derive_support_closure(request_id)?;
        Ok(RelationalAnalysisEvidenceEvent::support_closed(receipt))
    }

    pub(crate) const fn closed_catalog(&self) -> Option<&ClosedRelationalAnalysisCatalog> {
        self.closed.as_ref()
    }

    pub(crate) fn catalog_root(&self) -> RelationalAnalysisCatalogRoot {
        match &self.closed {
            Some(closed) => closed.root(),
            None => self
                .open
                .as_ref()
                .expect("analysis state retains either an open or closed catalog")
                .root(),
        }
    }

    pub(crate) const fn is_closed(&self) -> bool {
        self.closed.is_some()
    }

    pub(crate) const fn has_pending_mechanism_artifact(&self) -> bool {
        self.pending_mechanism_artifact.is_some()
    }

    pub(crate) fn pending_mechanism_artifact_request_id(&self) -> Option<MechanismRequestId> {
        self.pending_mechanism_artifact
            .as_ref()
            .map(|pending| pending.header.claim().request_id())
    }

    pub(crate) fn pending_mechanism_artifact_claim(
        &self,
    ) -> Option<RelationalMechanismArtifactClaim> {
        self.pending_mechanism_artifact
            .as_ref()
            .map(|pending| pending.header.claim())
    }

    /// Mint exactly one next event for a deterministic structural quotient
    /// artifact. Opening, each bounded data chunk, and closure therefore live
    /// in separate stream quanta. After replay, the complete regenerated
    /// payload is hashed once against the durable header before any suffix is
    /// continued; subsequent chunks copy only their own bounded byte range.
    pub(crate) fn next_structural_quotient_artifact_event(
        &self,
        artifact: &StructuralSignatureQuotientArtifact,
        chunk_bytes: usize,
    ) -> Result<RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError> {
        if chunk_bytes == 0 || chunk_bytes > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES {
            return Err(
                RelationalAnalysisJournalError::MechanismArtifactChunkCapacity {
                    actual: chunk_bytes,
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES,
                },
            );
        }
        let claim = structural_quotient_artifact_claim(artifact);
        let payload = artifact.canonical_payload();
        if payload.is_empty() || payload.len() > RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES {
            return Err(RelationalAnalysisJournalError::MechanismArtifactCapacity {
                actual: payload.len(),
                limit: RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES,
            });
        }

        let Some(pending) = self.pending_mechanism_artifact.as_ref() else {
            let chunk_count = payload.len().div_ceil(chunk_bytes);
            if chunk_count > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS {
                return Err(
                    RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                        actual: chunk_count,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                    },
                );
            }
            let header = RelationalMechanismArtifactHeader::issue(claim, payload);
            header.validate_identity()?;
            return Ok(RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header });
        };
        if pending.header.claim() != claim
            || pending.header.total_bytes() != payload.len() as u64
            || pending.accepted_bytes > pending.header.total_bytes()
        {
            return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
        }
        if !pending.structural_producer_verified.get() {
            let expected = RelationalMechanismArtifactHeader::issue(claim, payload);
            if expected != pending.header {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }
            pending.structural_producer_verified.set(true);
        }
        let verified_chunk_count = pending.structural_verified_chunk_count.get();
        if verified_chunk_count > pending.chunks.len() {
            return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
        }
        let mut verified_prefix_bytes = usize::try_from(pending.structural_verified_bytes.get())
            .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
        for chunk in &pending.chunks[verified_chunk_count..] {
            let end = verified_prefix_bytes
                .checked_add(chunk.bytes().len())
                .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            let verified_offset = u64::try_from(verified_prefix_bytes)
                .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            if end > payload.len()
                || chunk.offset() != verified_offset
                || chunk.bytes() != &payload[verified_prefix_bytes..end]
            {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }
            verified_prefix_bytes = end;
        }
        if u64::try_from(verified_prefix_bytes) != Ok(pending.accepted_bytes) {
            return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
        }
        pending
            .structural_verified_chunk_count
            .set(pending.chunks.len());
        pending
            .structural_verified_bytes
            .set(pending.accepted_bytes);

        let offset = usize::try_from(pending.accepted_bytes)
            .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
        if offset < payload.len() {
            let remaining_chunk_count = (payload.len() - offset).div_ceil(chunk_bytes);
            let completed_chunk_count = pending.chunks.len();
            let total_chunk_count = completed_chunk_count
                .checked_add(remaining_chunk_count)
                .ok_or(
                    RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                        actual: usize::MAX,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                    },
                )?;
            if total_chunk_count > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS {
                return Err(
                    RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                        actual: total_chunk_count,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                    },
                );
            }
            let end = offset.saturating_add(chunk_bytes).min(payload.len());
            let ordinal = u32::try_from(pending.chunks.len()).map_err(|_| {
                RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                    actual: pending.chunks.len(),
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                }
            })?;
            return Ok(
                RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted {
                    chunk: RelationalMechanismArtifactChunk::issue(
                        pending.header.id(),
                        ordinal,
                        pending.accepted_bytes,
                        &payload[offset..end],
                    ),
                },
            );
        }

        let closure = RelationalMechanismArtifactClosure {
            artifact_id: pending.header.id(),
            chunk_count: u32::try_from(pending.chunks.len()).map_err(|_| {
                RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                    actual: pending.chunks.len(),
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                }
            })?,
            chunk_root: derive_mechanism_artifact_chunk_root(pending.header.id(), &pending.chunks),
        };
        closure.validate_shape()?;
        Ok(RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { closure })
    }

    /// Compare a freshly reproduced artifact with the exact durable prefix
    /// after a crash, returning only records that have not already reached the
    /// journal. Chunk boundaries are operational: a resumed producer may use a
    /// different chunk size, but the typed header, complete payload digest,
    /// and every already-durable byte must still agree exactly.
    pub(crate) fn resume_mechanism_artifact_events(
        &self,
        events: Box<[RelationalAnalysisEvidenceEvent]>,
    ) -> Result<Box<[RelationalAnalysisEvidenceEvent]>, RelationalAnalysisJournalError> {
        let Some(pending) = &self.pending_mechanism_artifact else {
            return Ok(events);
        };
        let Some(RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header }) =
            events.first()
        else {
            return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
        };
        let header = *header;
        if header != pending.header {
            return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
        }
        let mut events = events.into_vec();
        let (candidate_closure_index, resumed_chunks, resumed_closure) = {
            let mut candidate_chunks = Vec::new();
            let mut candidate_bytes = 0u64;
            let mut candidate_hasher = Sha256::new();
            let mut candidate_closure = None;
            let mut candidate_closure_index = 0usize;
            for (event_index, event) in events.iter().enumerate().skip(1) {
                match event {
                    RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { chunk } => {
                        chunk.validate_identity()?;
                        if chunk.artifact_id() != header.id()
                            || usize::try_from(chunk.ordinal()) != Ok(candidate_chunks.len())
                            || chunk.offset() != candidate_bytes
                        {
                            return Err(
                                RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                            );
                        }
                        candidate_bytes = candidate_bytes
                            .checked_add(chunk.bytes().len() as u64)
                            .ok_or(
                            RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                        )?;
                        if candidate_bytes > header.total_bytes() {
                            return Err(
                                RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                            );
                        }
                        candidate_hasher.update(chunk.bytes());
                        candidate_chunks.push(chunk);
                    }
                    RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { closure } => {
                        candidate_closure = Some(*closure);
                        candidate_closure_index = event_index;
                        break;
                    }
                    _ => {
                        return Err(
                            RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                        );
                    }
                }
            }
            let candidate_closure = candidate_closure
                .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            candidate_closure.validate_shape()?;
            if candidate_chunks.is_empty()
                || candidate_bytes != header.total_bytes()
                || candidate_closure.artifact_id() != header.id()
                || usize::try_from(candidate_closure.chunk_count()) != Ok(candidate_chunks.len())
                || candidate_closure.chunk_root()
                    != derive_mechanism_artifact_chunk_root_from_iter(
                        header.id(),
                        candidate_chunks.len(),
                        candidate_chunks.iter().copied(),
                    )
                || <[u8; 32]>::from(candidate_hasher.finalize()) != header.payload_digest()
            {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }

            let mut durable_bytes = 0u64;
            for (ordinal, chunk) in pending.chunks.iter().enumerate() {
                chunk.validate_identity()?;
                if chunk.artifact_id() != pending.header.id()
                    || usize::try_from(chunk.ordinal()) != Ok(ordinal)
                    || chunk.offset() != durable_bytes
                {
                    return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
                }
                durable_bytes = durable_bytes
                    .checked_add(chunk.bytes().len() as u64)
                    .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            }
            if durable_bytes != pending.accepted_bytes
                || durable_bytes > pending.header.total_bytes()
            {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }
            let durable_prefix = pending
                .chunks
                .iter()
                .flat_map(|chunk| chunk.bytes().iter().copied());
            let candidate_prefix = candidate_chunks
                .iter()
                .flat_map(|chunk| chunk.bytes().iter().copied())
                .take(usize::try_from(durable_bytes).map_err(|_| {
                    RelationalAnalysisJournalError::MechanismArtifactResumeMismatch
                })?);
            if !durable_prefix.eq(candidate_prefix) {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }

            let remaining_bytes = usize::try_from(
                header
                    .total_bytes()
                    .checked_sub(durable_bytes)
                    .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?,
            )
            .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            let resumed_chunk_count =
                remaining_bytes.div_ceil(RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES);
            let total_chunk_count = pending
                .chunks
                .len()
                .checked_add(resumed_chunk_count)
                .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            if total_chunk_count == 0
                || total_chunk_count > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS
            {
                return Err(
                    RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                        actual: total_chunk_count,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                    },
                );
            }

            let mut resumed_chunks = Vec::new();
            resumed_chunks
                .try_reserve_exact(resumed_chunk_count)
                .map_err(|_| {
                    RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                        actual: resumed_chunk_count,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                    }
                })?;
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES)
                .map_err(
                    |_| RelationalAnalysisJournalError::MechanismArtifactCapacity {
                        actual: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES,
                        limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES,
                    },
                )?;
            let mut skipped = 0u64;
            let mut next_offset = durable_bytes;
            for candidate in &candidate_chunks {
                let mut bytes = candidate.bytes();
                if skipped < durable_bytes {
                    let skip = usize::try_from(
                        durable_bytes.checked_sub(skipped).ok_or(
                            RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                        )?,
                    )
                    .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?
                    .min(bytes.len());
                    bytes = &bytes[skip..];
                    skipped = skipped
                        .checked_add(skip as u64)
                        .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
                }
                while !bytes.is_empty() {
                    let take = (RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES - buffer.len())
                        .min(bytes.len());
                    buffer.extend_from_slice(&bytes[..take]);
                    bytes = &bytes[take..];
                    if buffer.len() == RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES {
                        let ordinal = pending
                            .chunks
                            .len()
                            .checked_add(resumed_chunks.len())
                            .ok_or(
                                RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                            )?;
                        resumed_chunks.push(RelationalMechanismArtifactChunk::issue(
                            header.id(),
                            u32::try_from(ordinal).map_err(|_| {
                                RelationalAnalysisJournalError::MechanismArtifactResumeMismatch
                            })?,
                            next_offset,
                            &buffer,
                        ));
                        next_offset = next_offset.checked_add(buffer.len() as u64).ok_or(
                            RelationalAnalysisJournalError::MechanismArtifactResumeMismatch,
                        )?;
                        buffer.clear();
                    }
                }
                skipped = candidate_bytes.min(
                    candidate
                        .offset()
                        .checked_add(candidate.bytes().len() as u64)
                        .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?,
                );
            }
            if !buffer.is_empty() {
                let ordinal = pending
                    .chunks
                    .len()
                    .checked_add(resumed_chunks.len())
                    .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
                resumed_chunks.push(RelationalMechanismArtifactChunk::issue(
                    header.id(),
                    u32::try_from(ordinal).map_err(|_| {
                        RelationalAnalysisJournalError::MechanismArtifactResumeMismatch
                    })?,
                    next_offset,
                    &buffer,
                ));
                next_offset = next_offset
                    .checked_add(buffer.len() as u64)
                    .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
            }
            if resumed_chunks.len() != resumed_chunk_count || next_offset != header.total_bytes() {
                return Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch);
            }
            let resumed_closure = RelationalMechanismArtifactClosure {
                artifact_id: header.id(),
                chunk_count: u32::try_from(total_chunk_count)
                    .map_err(|_| RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?,
                chunk_root: derive_mechanism_artifact_chunk_root_from_iter(
                    header.id(),
                    total_chunk_count,
                    pending.chunks.iter().chain(resumed_chunks.iter()),
                ),
            };
            resumed_closure.validate_shape()?;
            (candidate_closure_index, resumed_chunks, resumed_closure)
        };

        let trailing_event_count = events
            .len()
            .checked_sub(candidate_closure_index + 1)
            .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?;
        let mut suffix = Vec::new();
        suffix
            .try_reserve_exact(
                resumed_chunks
                    .len()
                    .checked_add(1)
                    .and_then(|count| count.checked_add(trailing_event_count))
                    .ok_or(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)?,
            )
            .map_err(
                |_| RelationalAnalysisJournalError::MechanismArtifactCapacity {
                    actual: resumed_chunks.len().saturating_add(1),
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES,
                },
            )?;
        suffix.extend(resumed_chunks.into_iter().map(|chunk| {
            RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { chunk }
        }));
        suffix.push(RelationalAnalysisEvidenceEvent::MechanismArtifactClosed {
            closure: resumed_closure,
        });
        suffix.extend(events.drain(candidate_closure_index + 1..));
        Ok(suffix.into_boxed_slice())
    }

    /// Canonical observation of the analysis evidence DAG. This may rebuild
    /// result/mechanism layer snapshots and therefore belongs at an explicit
    /// journal checkpoint or publication boundary, never in the base quantum
    /// scheduler.
    pub(crate) fn snapshot(&self) -> RelationalAnalysisCatalogSnapshot {
        match &self.closed {
            Some(closed) => closed.snapshot().clone(),
            None => self
                .open
                .as_ref()
                .expect("analysis state retains either an open or closed catalog")
                .snapshot(),
        }
    }

    pub(crate) fn materialize_published_view(
        &self,
        view_id: ViewId,
    ) -> Result<ClosedResultView, RelationalAnalysisJournalError> {
        match (&self.open, &self.closed) {
            (Some(open), None) => Ok(open.materialize_published_result(view_id)?),
            (None, Some(closed)) => Ok(closed.materialize_published_result(view_id)?),
            _ => Err(RelationalAnalysisJournalError::AnalysisStateDiverged),
        }
    }

    pub(crate) fn mechanism_closure(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<RelationalMechanismClosureReceipt> {
        self.mechanism_closures.get(&request_id).copied()
    }

    /// Resolve a missing canonical raw signature after durable incidence
    /// closure. Open scheduling follows raw discovery order; this separate
    /// close-time pass validates exact canonical set equality without
    /// requiring the operational structural-assignment order to be sorted.
    pub(crate) fn next_closed_structural_signature_id(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<Option<MechanismSignatureId>, RelationalAnalysisJournalError> {
        self.mechanism_closures
            .get(&request_id)
            .ok_or(RelationalAnalysisJournalError::MechanismClosureMissing { request_id })?;
        let catalog = self.open_catalog_or_error()?;
        let incidence = catalog.mechanism_incidence(request_id)?;
        let signature_count = incidence
            .closed_signature_count()
            .map_err(RelationalAnalysisCatalogError::Mechanism)?;
        let assignment_count = self
            .structural_mechanisms
            .get(&request_id)
            .map_or(0, StructuralMechanismCatalogBuilder::assignment_count);
        if assignment_count > signature_count {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural assignment count exceeds closed raw signatures",
            ));
        }
        let mut assigned_canonical = 0usize;
        let mut first_missing = None;
        if let Some(structural) = self.structural_mechanisms.get(&request_id) {
            if structural.assignment_discovery_count() != assignment_count {
                return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "structural assignment discovery count",
                ));
            }
            for signature_id in structural.assignment_discovery_suffix(0).iter().copied() {
                if incidence.signature_definition(signature_id).is_none() {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "structural assignment outside closed raw signatures",
                    ));
                }
            }
        }
        for ordinal in 0..signature_count {
            let signature_id = incidence
                .closed_signature_id_at(ordinal)
                .map_err(RelationalAnalysisCatalogError::Mechanism)?
                .ok_or(RelationalAnalysisJournalError::EventClaimMismatch(
                    "closed structural signature set",
                ))?;
            if self
                .structural_mechanisms
                .get(&request_id)
                .is_some_and(|structural| structural.assignment(signature_id).is_some())
            {
                assigned_canonical = assigned_canonical.checked_add(1).ok_or(
                    RelationalAnalysisJournalError::EventClaimMismatch(
                        "structural assignment count overflow",
                    ),
                )?;
            } else if first_missing.is_none() {
                first_missing = Some(signature_id);
            }
        }
        if assigned_canonical != assignment_count {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural assignment canonical set",
            ));
        }
        Ok(first_missing)
    }

    /// Borrow one request's append-only publication order before or after the
    /// final analysis-close event. Cold journal replay reconstructs the live
    /// order; analysis closure moves it here without adding scheduler order to
    /// any semantic snapshot or hash.
    pub(crate) fn mechanism_publication_discovery(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismPublicationDiscoveryRef<'_>> {
        if let Some(open) = &self.open {
            return open.mechanism_publication_discovery(request_id).ok();
        }
        self.closed_mechanism_publication_discoveries
            .binary_search_by_key(&request_id, |(stored_request_id, _)| *stored_request_id)
            .ok()
            .map(|index| {
                MechanismPublicationDiscoveryRef::Closed(
                    &self.closed_mechanism_publication_discoveries[index].1,
                )
            })
    }

    /// Derive the terminal event only after every declared mechanism layer has
    /// an explicit immutable closure artifact and the catalog itself validates
    /// all result publications and cross-layer seals. No caller count enters
    /// the event.
    pub(crate) fn terminal_event(
        &self,
    ) -> Result<RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError> {
        if self.pending_mechanism_artifact.is_some() {
            return Err(RelationalAnalysisJournalError::MechanismArtifactPending);
        }
        self.selected_question
            .ok_or(RelationalAnalysisJournalError::SelectedQuestionSealMissing)?;
        let catalog = self.open_catalog_or_error()?;
        let closure_set_root = self.validate_and_derive_closure_set_root(catalog)?;
        catalog.validate_complete()?;
        Ok(RelationalAnalysisEvidenceEvent::analysis_closed(
            catalog.root(),
            closure_set_root,
        ))
    }

    /// Apply one causal semantic event. Equal rediscovery is idempotent while
    /// its request remains open, and an equal request-close event stays
    /// idempotent afterward. Other request payload is rejected at that close
    /// barrier before any map mutation. Scheduler checkpoint application
    /// intentionally has no entry point here. A rejected support-close claim
    /// may retain independently checked request-local prefix caches, but it
    /// cannot install either the builder closure or semantic closure receipt
    /// until the derived root matches.
    pub(crate) fn apply(
        &mut self,
        event: &RelationalAnalysisEvidenceEvent,
    ) -> Result<RelationalAnalysisJournalApply, RelationalAnalysisJournalError> {
        event.validate_claimed_content()?;
        if self.pending_mechanism_artifact.is_some()
            && !matches!(
                event,
                RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { .. }
                    | RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { .. }
            )
        {
            return Err(RelationalAnalysisJournalError::MechanismArtifactInterleaving);
        }
        if let RelationalAnalysisEvidenceEvent::AnalysisClosed {
            catalog_root,
            closure_set_root,
        } = event
        {
            return self.apply_terminal(*catalog_root, *closure_set_root);
        }
        if self.closed.is_some() {
            return Err(RelationalAnalysisJournalError::EventAfterAnalysisClosure);
        }
        self.reject_mechanism_payload_after_closure(event)?;

        if let RelationalAnalysisEvidenceEvent::SelectedQuestionBound { seal, .. } = event {
            let changed = self.bind_selected_question(*seal)?;
            return Ok(if changed {
                RelationalAnalysisJournalApply::Applied
            } else {
                RelationalAnalysisJournalApply::AlreadyAccepted
            });
        }
        let changed = match event {
            RelationalAnalysisEvidenceEvent::SelectedQuestionBound { .. } => unreachable!(),
            RelationalAnalysisEvidenceEvent::ResultSpecRegistered {
                view_id,
                resolved_input,
                spec,
                ..
            } => self.open_catalog_mut_or_error()?.register_result_spec(
                *view_id,
                *resolved_input,
                spec.clone(),
            )?,
            RelationalAnalysisEvidenceEvent::ResultEvidenceAccepted {
                view_id, record, ..
            } => {
                self.open_catalog_mut_or_error()?
                    .insert_result_evidence(*view_id, record.clone())?
                    .1
            }
            RelationalAnalysisEvidenceEvent::CertifiedSourceSummaryAccepted {
                artifact, ..
            } => self.accept_certified_source_summary(artifact)?,
            RelationalAnalysisEvidenceEvent::ResultInputSealedFromSources { view_id, seal } => self
                .open_catalog_mut_or_error()?
                .seal_result_input_with_receipt(*view_id, *seal)?,
            RelationalAnalysisEvidenceEvent::ResultInputSealedFromSelected {
                view_id,
                question_seal_id,
            } => {
                let selected_question = self
                    .selected_question
                    .ok_or(RelationalAnalysisJournalError::SelectedQuestionSealMissing)?;
                if *question_seal_id != selected_question.id() {
                    return Err(RelationalAnalysisJournalError::SelectedQuestionSealMismatch);
                }
                let seal = selected_question.result_input_seal();
                self.open_catalog_mut_or_error()?
                    .seal_result_input_with_receipt(*view_id, seal)?
            }
            RelationalAnalysisEvidenceEvent::ResultInputSealedFromMechanisms {
                view_id,
                request_id,
                incidence_root,
                structural_root,
            } => {
                let closure = self.mechanism_closures.get(request_id).copied().ok_or(
                    RelationalAnalysisJournalError::MechanismClosureMissing {
                        request_id: *request_id,
                    },
                )?;
                if closure.incidence_root() != *incidence_root {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "closed mechanism incidence",
                    ));
                }
                let structural_closure = self.structural_quotient_closure(*request_id).ok_or(
                    RelationalAnalysisJournalError::StructuralQuotientClosureMissing {
                        request_id: *request_id,
                    },
                )?;
                if structural_closure.root() != *structural_root {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "closed structural quotient",
                    ));
                }
                let open = self
                    .open
                    .as_mut()
                    .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
                open.seal_result_input_from_mechanisms(
                    *view_id,
                    closure,
                    structural_closure.root(),
                )?
            }
            RelationalAnalysisEvidenceEvent::ResultProjectionRecordAccepted {
                view_id,
                record,
                ..
            } => self
                .open_catalog_mut_or_error()?
                .insert_result_projection_record(*view_id, (**record).clone())?,
            RelationalAnalysisEvidenceEvent::ResultViewPublished {
                view_id,
                evidence_root,
                projection_root,
                closure,
                ..
            } => {
                let open = self.open_catalog_or_error()?;
                if open.result_evidence_root(*view_id)? != *evidence_root
                    || open.result_projection_root(*view_id)? != *projection_root
                {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "published result evidence or projection root",
                    ));
                }
                let certified = open.certified_source_summary(*view_id)?.cloned();
                let (_, changed) = match certified {
                    Some(artifact) => self
                        .open_catalog_mut_or_error()?
                        .publish_certified_source_projection(*closure, &artifact)?,
                    None => self
                        .open_catalog_mut_or_error()?
                        .publish_result_projection(*closure)?,
                };
                let closure_changed = match self.result_projection_closures.get(view_id) {
                    Some(existing) if existing == closure => false,
                    Some(_) => {
                        return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                            "published result closure",
                        ))
                    }
                    None => {
                        self.result_projection_closures.insert(*view_id, *closure);
                        true
                    }
                };
                if changed != closure_changed {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "published result closure replay state",
                    ));
                }
                changed
            }
            RelationalAnalysisEvidenceEvent::MechanismTargetCaseAccepted {
                request_id,
                case_id,
            } => self
                .open_catalog_mut_or_error()?
                .insert_mechanism_target_case(*request_id, *case_id)?,
            RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromSelected {
                request_id,
                question_seal_id,
            } => {
                let selected_question = self
                    .selected_question
                    .ok_or(RelationalAnalysisJournalError::SelectedQuestionSealMissing)?;
                if *question_seal_id != selected_question.id() {
                    return Err(RelationalAnalysisJournalError::SelectedQuestionSealMismatch);
                }
                let selected = selected_question;
                match selected.authority() {
                    RelationalSelectedPopulationAuthority::ExtensionalQuestion { content_root } => {
                        self.open_catalog_mut_or_error()?
                            .seal_mechanism_target_from_selected_commitment(
                                *request_id,
                                selected.question_id(),
                                content_root,
                                selected.mechanism_target(),
                            )?
                    }
                    RelationalSelectedPopulationAuthority::CertifiedSupport {
                        population_root,
                        exact_cardinality,
                    } => self
                        .open_catalog_mut_or_error()?
                        .seal_mechanism_target_from_certified_selected_commitment(
                            *request_id,
                            selected.question_id(),
                            population_root,
                            exact_cardinality,
                            selected.mechanism_target(),
                        )?,
                }
            }
            RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromResult {
                request_id,
                view_id,
                result_root,
            } => {
                let view = self
                    .open_catalog_or_error()?
                    .materialize_published_result(*view_id)
                    .map_err(|error| match error {
                        RelationalAnalysisCatalogError::ResultNotPublished { .. } => {
                            RelationalAnalysisJournalError::PublishedViewMissing {
                                view_id: *view_id,
                            }
                        }
                        error => RelationalAnalysisJournalError::Catalog(error),
                    })?;
                if view.root() != *result_root {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "chosen result root",
                    ));
                }
                let open = self
                    .open
                    .as_mut()
                    .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
                open.seal_mechanism_target_from_result(*request_id, &view)?
            }
            RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header } => {
                self.open_mechanism_artifact(*header)?;
                true
            }
            RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { chunk } => {
                self.append_mechanism_artifact_chunk(chunk.clone())?;
                true
            }
            RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { closure } => {
                self.close_mechanism_artifact(*closure)?
            }
            RelationalAnalysisEvidenceEvent::MechanismIncidenceClosed {
                request_id,
                incidence_root,
            } => self.apply_mechanism_closure(*request_id, *incidence_root)?,
            RelationalAnalysisEvidenceEvent::StructuralQuotientClosed {
                request_id,
                structural_root,
            } => self.apply_structural_quotient_closure(*request_id, *structural_root)?,
            RelationalAnalysisEvidenceEvent::SupportClosed {
                request_id,
                support_root,
            } => self.apply_support_closure(*request_id, *support_root)?,
            RelationalAnalysisEvidenceEvent::AnalysisClosed { .. } => unreachable!(),
        };
        Ok(if changed {
            RelationalAnalysisJournalApply::Applied
        } else {
            RelationalAnalysisJournalApply::AlreadyAccepted
        })
    }

    fn accept_certified_source_summary(
        &mut self,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        artifact.validate_identity().then_some(()).ok_or(
            RelationalAnalysisJournalError::EventClaimMismatch("certified source summary"),
        )?;
        let catalog = self.open_catalog_or_error()?;
        let registration = catalog
            .plan()
            .registration(RelationalAnalysisLayerId::Result(artifact.view_id()))
            .ok_or(
                RelationalAnalysisJournalError::CertifiedSourceSummaryScopeMismatch {
                    view_id: artifact.view_id(),
                },
            )?;
        let RelationalAnalysisLayerRegistration::Result(registration) = registration else {
            return Err(
                RelationalAnalysisJournalError::CertifiedSourceSummaryScopeMismatch {
                    view_id: artifact.view_id(),
                },
            );
        };
        if artifact.analysis_plan_root() != self.plan_root
            || registration.input()
                != RelationalResolvedResultInput::Sources(artifact.relation_id())
            || registration.semantic_spec_digest() != artifact.semantic_spec_digest()
            || catalog.result_spec(artifact.view_id())?.spec_root() != artifact.spec_root()
        {
            return Err(
                RelationalAnalysisJournalError::CertifiedSourceSummaryScopeMismatch {
                    view_id: artifact.view_id(),
                },
            );
        }

        self.open_catalog_mut_or_error()?
            .accept_certified_source_summary(artifact)
            .map_err(RelationalAnalysisJournalError::Catalog)
    }

    fn apply_mechanism_closure(
        &mut self,
        request_id: MechanismRequestId,
        claimed_root: MechanismIncidenceRoot,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        if let Some(existing) = self.mechanism_closures.get(&request_id).copied() {
            if existing.incidence_root() != claimed_root {
                return Err(RelationalAnalysisJournalError::MechanismClosureConflict {
                    request_id,
                });
            }
            let current = self
                .open_catalog_or_error()?
                .mechanism_closure_receipt(request_id)?;
            if current != existing {
                return Err(RelationalAnalysisJournalError::MechanismClosureConflict {
                    request_id,
                });
            }
            return Ok(false);
        }
        let closure = self
            .open_catalog_or_error()?
            .mechanism_closure_receipt(request_id)?;
        if closure.incidence_root() != claimed_root {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism incidence root",
            ));
        }
        self.mechanism_closures.insert(request_id, closure);
        Ok(true)
    }

    fn prepare_structural_quotient_closure(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<
        (
            StructuralMechanismCatalogBuilder,
            StructuralQuotientClosureReceipt,
        ),
        RelationalAnalysisJournalError,
    > {
        let stored_incidence = self
            .mechanism_closures
            .get(&request_id)
            .copied()
            .ok_or(RelationalAnalysisJournalError::MechanismClosureMissing { request_id })?;
        let catalog = self.open_catalog_or_error()?;
        let current_incidence = catalog.mechanism_closure_receipt(request_id)?;
        if current_incidence != stored_incidence {
            return Err(RelationalAnalysisJournalError::MechanismClosureConflict { request_id });
        }
        let incidence = catalog.mechanism_incidence(request_id)?;
        let closed_incidence = incidence.closed_ref().map_err(|error| {
            RelationalAnalysisJournalError::Catalog(RelationalAnalysisCatalogError::Mechanism(
                error,
            ))
        })?;
        if closed_incidence.root() != stored_incidence.incidence_root() {
            return Err(RelationalAnalysisJournalError::MechanismClosureConflict { request_id });
        }
        let mut candidate = self
            .structural_mechanisms
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| StructuralMechanismCatalogBuilder::new(request_id));
        let receipt = candidate
            .close_against_expected_signatures(
                closed_incidence.signature_definition_count() as u128,
                closed_incidence.signature_ids(),
            )
            .map_err(RelationalStructuralMechanismError::from)?;
        Ok((candidate, receipt))
    }

    fn apply_structural_quotient_closure(
        &mut self,
        request_id: MechanismRequestId,
        claimed_root: StructuralQuotientClosureRoot,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        if let Some(existing) = self.structural_quotient_closure(request_id) {
            if existing.root() != claimed_root {
                return Err(
                    RelationalAnalysisJournalError::StructuralQuotientClosureConflict {
                        request_id,
                    },
                );
            }
        }
        let (candidate, receipt) = self.prepare_structural_quotient_closure(request_id)?;
        if receipt.root() != claimed_root {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural quotient closure root",
            ));
        }
        let changed = self.structural_quotient_closure(request_id) != Some(receipt);
        self.structural_mechanisms.insert(request_id, candidate);
        Ok(changed)
    }

    fn derive_support_closure(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<MechanismSupportClosureReceipt, RelationalAnalysisJournalError> {
        let stored_incidence = self
            .mechanism_closures
            .get(&request_id)
            .copied()
            .ok_or(RelationalAnalysisJournalError::MechanismClosureMissing { request_id })?;
        let catalog = self
            .open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)?;
        let current_incidence = catalog.mechanism_closure_receipt(request_id)?;
        if current_incidence != stored_incidence {
            return Err(RelationalAnalysisJournalError::MechanismClosureConflict { request_id });
        }
        let incidence = catalog.mechanism_incidence(request_id)?;
        let closed_incidence = incidence.closed_ref().map_err(|error| {
            RelationalAnalysisJournalError::Catalog(RelationalAnalysisCatalogError::Mechanism(
                error,
            ))
        })?;
        if closed_incidence.root() != stored_incidence.incidence_root() {
            return Err(RelationalAnalysisJournalError::MechanismClosureConflict { request_id });
        }
        let structural = self.structural_mechanisms.get(&request_id).ok_or(
            RelationalAnalysisJournalError::StructuralQuotientClosureMissing { request_id },
        )?;
        if structural.closure().is_none() {
            return Err(
                RelationalAnalysisJournalError::StructuralQuotientClosureMissing { request_id },
            );
        }
        let support = self
            .mechanism_supports
            .entry(request_id)
            .or_insert_with(|| MechanismSupportCatalogBuilder::new(incidence.scope()));
        if support.target_discovery_cursor() != incidence.target_discovery_count() {
            return Err(
                RelationalAnalysisJournalError::SupportTargetCatchUpIncomplete { request_id },
            );
        }
        Ok(support.derive_closure(closed_incidence, structural)?)
    }

    fn apply_support_closure(
        &mut self,
        request_id: MechanismRequestId,
        claimed_root: MechanismSupportClosureRoot,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        if let Some(existing) = self.support_closures.get(&request_id).copied() {
            if existing.root() != claimed_root {
                return Err(RelationalAnalysisJournalError::SupportClosureConflict { request_id });
            }
        }
        let receipt = self.derive_support_closure(request_id)?;
        if receipt.root() != claimed_root {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism support closure root",
            ));
        }
        let changed = self.support_closures.get(&request_id).copied() != Some(receipt);
        self.mechanism_supports
            .get_mut(&request_id)
            .expect("support closure derivation installs its request-local builder")
            .commit_derived_closure(receipt)?;
        self.support_closures.insert(request_id, receipt);
        Ok(changed)
    }

    /// A raw mechanism-close event is a request-level write barrier for target,
    /// signature, replay-incidence, and unavailable evidence. A structural
    /// quotient is derived only from an already-interned raw signature, so its
    /// subordinate artifact may arrive after raw closure. Final analysis
    /// closure remains the outer write barrier for both layers.
    fn reject_mechanism_payload_after_closure(
        &self,
        event: &RelationalAnalysisEvidenceEvent,
    ) -> Result<(), RelationalAnalysisJournalError> {
        let request_payload = match event {
            RelationalAnalysisEvidenceEvent::MechanismTargetCaseAccepted { request_id, .. }
            | RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromSelected {
                request_id,
                ..
            }
            | RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromResult {
                request_id, ..
            } => Some((*request_id, false)),
            RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header } => Some((
                header.claim().request_id(),
                matches!(
                    header.claim(),
                    RelationalMechanismArtifactClaim::StructuralQuotient { .. }
                ),
            )),
            RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { .. }
            | RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { .. } => {
                self.pending_mechanism_artifact.as_ref().map(|pending| {
                    (
                        pending.header.claim().request_id(),
                        matches!(
                            pending.header.claim(),
                            RelationalMechanismArtifactClaim::StructuralQuotient { .. }
                        ),
                    )
                })
            }
            _ => None,
        };
        if let Some((request_id, structural_quotient)) = request_payload {
            if structural_quotient && self.structural_quotient_closure(request_id).is_some() {
                return Err(
                    RelationalAnalysisJournalError::StructuralQuotientPayloadAfterClosure {
                        request_id,
                    },
                );
            }
            if !structural_quotient && self.mechanism_closures.contains_key(&request_id) {
                return Err(
                    RelationalAnalysisJournalError::MechanismPayloadAfterClosure { request_id },
                );
            }
        }
        Ok(())
    }

    fn open_mechanism_artifact(
        &mut self,
        header: RelationalMechanismArtifactHeader,
    ) -> Result<(), RelationalAnalysisJournalError> {
        header.validate_identity()?;
        if self.pending_mechanism_artifact.is_some() {
            return Err(RelationalAnalysisJournalError::MechanismArtifactInterleaving);
        }
        let claim = header.claim();
        self.open_catalog_or_error()?
            .mechanism_evidence_contract(claim.request_id())?;
        if let RelationalMechanismArtifactClaim::StructuralQuotient {
            request_id,
            raw_signature_id,
            structural_mechanism_id,
            execution_profile_id,
        } = claim
        {
            let incidence = self
                .open_catalog_or_error()?
                .mechanism_incidence(request_id)?;
            if raw_signature_id.request_id() != request_id
                || incidence.signature_definition(raw_signature_id).is_none()
            {
                return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "structural quotient existing raw signature",
                ));
            }
            if self
                .structural_mechanisms
                .get(&request_id)
                .and_then(|structural| structural.assignment(raw_signature_id))
                .is_some_and(|assignment| {
                    assignment.mechanism_id() != structural_mechanism_id
                        || assignment.profile_id() != execution_profile_id
                })
            {
                return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                    "structural quotient conflicting raw-signature assignment",
                ));
            }
        }
        self.pending_mechanism_artifact = Some(PendingMechanismArtifact {
            header,
            chunks: Vec::new(),
            accepted_bytes: 0,
            structural_producer_verified: Cell::new(false),
            structural_verified_chunk_count: Cell::new(0),
            structural_verified_bytes: Cell::new(0),
        });
        Ok(())
    }

    fn append_mechanism_artifact_chunk(
        &mut self,
        chunk: RelationalMechanismArtifactChunk,
    ) -> Result<(), RelationalAnalysisJournalError> {
        chunk.validate_identity()?;
        let pending = self
            .pending_mechanism_artifact
            .as_mut()
            .ok_or(RelationalAnalysisJournalError::MechanismArtifactNotOpen)?;
        if pending.chunks.len() >= RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS {
            return Err(
                RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                    actual: pending.chunks.len().saturating_add(1),
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                },
            );
        }
        if chunk.artifact_id() != pending.header.id()
            || usize::try_from(chunk.ordinal()) != Ok(pending.chunks.len())
            || chunk.offset() != pending.accepted_bytes
        {
            return Err(RelationalAnalysisJournalError::MechanismArtifactChunkOrderMismatch);
        }
        let accepted_bytes = pending
            .accepted_bytes
            .checked_add(chunk.bytes().len() as u64)
            .ok_or(RelationalAnalysisJournalError::MechanismArtifactCapacity {
                actual: usize::MAX,
                limit: mechanism_artifact_max_bytes(pending.header.claim()),
            })?;
        if accepted_bytes > pending.header.total_bytes() {
            return Err(RelationalAnalysisJournalError::MechanismArtifactLengthMismatch);
        }
        pending.chunks.try_reserve(1).map_err(|_| {
            RelationalAnalysisJournalError::MechanismArtifactCapacity {
                actual: pending.chunks.len().saturating_add(1),
                limit: mechanism_artifact_max_bytes(pending.header.claim()),
            }
        })?;
        pending.chunks.push(chunk);
        pending.accepted_bytes = accepted_bytes;
        Ok(())
    }

    fn close_mechanism_artifact(
        &mut self,
        closure: RelationalMechanismArtifactClosure,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        let header = {
            let pending = self
                .pending_mechanism_artifact
                .as_ref()
                .ok_or(RelationalAnalysisJournalError::MechanismArtifactNotOpen)?;
            if closure.artifact_id() != pending.header.id()
                || usize::try_from(closure.chunk_count()) != Ok(pending.chunks.len())
                || closure.chunk_root()
                    != derive_mechanism_artifact_chunk_root(pending.header.id(), &pending.chunks)
                || pending.accepted_bytes != pending.header.total_bytes()
            {
                return Err(RelationalAnalysisJournalError::MechanismArtifactClosureMismatch);
            }
            pending.header
        };

        let structural_artifact = match header.claim() {
            RelationalMechanismArtifactClaim::StructuralQuotient {
                request_id,
                raw_signature_id,
                ..
            } => {
                let pending = self
                    .pending_mechanism_artifact
                    .as_ref()
                    .expect("validated pending structural artifact remains present");
                let mut payload_hasher = Sha256::new();
                for chunk in &pending.chunks {
                    payload_hasher.update(chunk.bytes());
                }
                if <[u8; 32]>::from(payload_hasher.finalize()) != header.payload_digest() {
                    return Err(RelationalAnalysisJournalError::MechanismArtifactPayloadMismatch);
                }
                let artifact =
                    self.rederive_structural_quotient_artifact(request_id, raw_signature_id)?;
                self.validate_structural_quotient_artifact_claim(header.claim(), &artifact)?;
                if RelationalMechanismArtifactHeader::issue(
                    header.claim(),
                    artifact.canonical_payload(),
                ) != header
                {
                    return Err(RelationalAnalysisJournalError::MechanismArtifactPayloadMismatch);
                }
                Some(artifact)
            }
            RelationalMechanismArtifactClaim::Signature { .. }
            | RelationalMechanismArtifactClaim::Incidence { .. }
            | RelationalMechanismArtifactClaim::Unavailable { .. } => None,
        };

        let changed = if let Some(artifact) = structural_artifact {
            self.intern_structural_quotient_artifact(header.claim(), &artifact)?
        } else {
            let pending = self
                .pending_mechanism_artifact
                .as_ref()
                .expect("validated pending mechanism artifact remains present");
            let total_bytes = usize::try_from(pending.header.total_bytes()).map_err(|_| {
                RelationalAnalysisJournalError::MechanismArtifactCapacity {
                    actual: usize::MAX,
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES,
                }
            })?;
            let mut payload = Vec::new();
            payload.try_reserve_exact(total_bytes).map_err(|_| {
                RelationalAnalysisJournalError::MechanismArtifactCapacity {
                    actual: total_bytes,
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES,
                }
            })?;
            for chunk in &pending.chunks {
                payload.extend_from_slice(chunk.bytes());
            }
            if payload.len() != total_bytes
                || <[u8; 32]>::from(Sha256::digest(&payload)) != pending.header.payload_digest()
            {
                return Err(RelationalAnalysisJournalError::MechanismArtifactPayloadMismatch);
            }
            self.apply_closed_mechanism_artifact(header.claim(), &payload)?
        };
        self.pending_mechanism_artifact = None;
        Ok(changed)
    }

    fn rederive_structural_quotient_artifact(
        &self,
        request_id: MechanismRequestId,
        raw_signature_id: MechanismSignatureId,
    ) -> Result<StructuralSignatureQuotientArtifact, RelationalAnalysisJournalError> {
        let catalog = self.open_catalog_or_error()?;
        let expected_scope = catalog.mechanism_evidence_contract(request_id)?.scope();
        let definition = catalog
            .mechanism_incidence(request_id)?
            .signature_definition(raw_signature_id)
            .ok_or(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural quotient raw signature",
            ))?;
        Ok(derive_relational_structural_mechanism_v1(
            definition,
            expected_scope,
            relational_structural_derivation_budget(),
        )?)
    }

    fn validate_structural_quotient_artifact_claim(
        &self,
        claim: RelationalMechanismArtifactClaim,
        artifact: &StructuralSignatureQuotientArtifact,
    ) -> Result<(), RelationalAnalysisJournalError> {
        let RelationalMechanismArtifactClaim::StructuralQuotient {
            raw_signature_id,
            structural_mechanism_id,
            execution_profile_id,
            ..
        } = claim
        else {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural quotient artifact kind",
            ));
        };
        if artifact.signature_id() != raw_signature_id
            || artifact.mechanism().id() != structural_mechanism_id
            || artifact.profile().id() != execution_profile_id
        {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural quotient artifact",
            ));
        }
        Ok(())
    }

    fn intern_structural_quotient_artifact(
        &mut self,
        claim: RelationalMechanismArtifactClaim,
        artifact: &StructuralSignatureQuotientArtifact,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        let RelationalMechanismArtifactClaim::StructuralQuotient { request_id, .. } = claim else {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "structural quotient artifact kind",
            ));
        };
        if let Some(structural_catalog) = self.structural_mechanisms.get_mut(&request_id) {
            return Ok(structural_catalog
                .intern_artifact(artifact)
                .map_err(RelationalStructuralMechanismError::from)?);
        }
        let mut structural_catalog = StructuralMechanismCatalogBuilder::new(request_id);
        let changed = structural_catalog
            .intern_artifact(artifact)
            .map_err(RelationalStructuralMechanismError::from)?;
        debug_assert!(
            changed,
            "a fresh structural catalog accepts its first assignment"
        );
        self.structural_mechanisms
            .insert(request_id, structural_catalog);
        Ok(changed)
    }

    fn apply_closed_mechanism_artifact(
        &mut self,
        claim: RelationalMechanismArtifactClaim,
        payload: &[u8],
    ) -> Result<bool, RelationalAnalysisJournalError> {
        match claim {
            RelationalMechanismArtifactClaim::Signature {
                request_id,
                signature_id,
            } => {
                let definition =
                    MechanismSignatureDefinition::from_canonical_definition(request_id, payload);
                if definition.id() != signature_id {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "mechanism signature artifact",
                    ));
                }
                let expected_scope = self
                    .open_catalog_or_error()?
                    .mechanism_evidence_contract(request_id)?
                    .scope();
                let _validated_definition = RelationalMechanismSignatureDagIndex::from_definition(
                    &definition,
                    expected_scope,
                )?;
                Ok(self
                    .open_catalog_mut_or_error()?
                    .intern_mechanism_signature(request_id, &definition)?)
            }
            claim @ RelationalMechanismArtifactClaim::StructuralQuotient {
                request_id,
                raw_signature_id,
                ..
            } => {
                let artifact =
                    self.rederive_structural_quotient_artifact(request_id, raw_signature_id)?;
                self.validate_structural_quotient_artifact_claim(claim, &artifact)?;
                if artifact.canonical_payload() != payload {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "structural quotient artifact",
                    ));
                }
                self.intern_structural_quotient_artifact(claim, &artifact)
            }
            RelationalMechanismArtifactClaim::Incidence {
                request_id,
                observation_id,
                observation_digest,
                replay_observation_id,
                case_id,
                transition_id,
                signature_id,
                replay_receipt_id,
            } => {
                let interned_definition = self
                    .open_catalog_or_error()?
                    .mechanism_incidence(request_id)?
                    .signature_definition(signature_id)
                    .cloned();
                let evidence =
                    RelationalMechanismReplayEvidence::restore_incidence_from_durable_payload(
                        payload,
                        interned_definition.as_ref(),
                    )?;
                if evidence.scope().request_id() != request_id
                    || evidence.observation_id() != replay_observation_id
                    || evidence.case_id() != case_id
                    || evidence.transition_id() != transition_id
                    || evidence.transition().id() != transition_id
                    || evidence.signature_id() != signature_id
                    || evidence.definition().id() != signature_id
                    || evidence.receipt().id() != replay_receipt_id
                {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "mechanism replay artifact",
                    ));
                }
                self.require_mechanism_evidence_contract(
                    request_id,
                    observation_id,
                    observation_digest,
                    evidence.scope(),
                )?;
                let insertion = self
                    .open_catalog_mut_or_error()?
                    .record_mechanism_incidence(
                        request_id,
                        case_id,
                        transition_id,
                        evidence.definition(),
                    )?;
                Ok(insertion.signature_inserted() || insertion.terminal_inserted())
            }
            RelationalMechanismArtifactClaim::Unavailable {
                request_id,
                observation_id,
                observation_digest,
                replay_observation_id,
                case_id,
                transition_id,
                reason_id,
            } => {
                let evidence =
                    RelationalMechanismUnavailableEvidence::restore_from_canonical_reason(payload)?;
                if evidence.scope().request_id() != request_id
                    || evidence.observation_id() != replay_observation_id
                    || evidence.case_id() != case_id
                    || evidence.transition_id() != transition_id
                    || evidence.reason_id() != reason_id
                {
                    return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                        "mechanism unavailable artifact",
                    ));
                }
                self.apply_mechanism_unavailable(
                    request_id,
                    observation_id,
                    observation_digest,
                    case_id,
                    reason_id,
                    &evidence,
                )
            }
        }
    }

    fn apply_mechanism_unavailable(
        &mut self,
        request_id: MechanismRequestId,
        observation_id: RelationalMechanismObservationId,
        observation_digest: RelationalMechanismObservationDigest,
        case_id: RelationalCaseId,
        reason_id: MechanismUnavailableReasonId,
        evidence: &RelationalMechanismUnavailableEvidence,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        self.require_mechanism_evidence_contract(
            request_id,
            observation_id,
            observation_digest,
            evidence.scope(),
        )?;
        let definition = MechanismUnavailableReasonDefinition::from_canonical_reason(
            evidence.canonical_reason(),
        );
        if definition.id() != reason_id {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism unavailable reason",
            ));
        }
        self.open_catalog_mut_or_error()?
            .record_mechanism_unavailable(request_id, case_id, &definition)
            .map_err(RelationalAnalysisJournalError::Catalog)
    }

    fn require_mechanism_evidence_contract(
        &self,
        request_id: MechanismRequestId,
        observation_id: RelationalMechanismObservationId,
        observation_digest: RelationalMechanismObservationDigest,
        scope: super::mechanism_incidence::MechanismRequestScope,
    ) -> Result<(), RelationalAnalysisJournalError> {
        let expected = self
            .open_catalog_or_error()?
            .mechanism_evidence_contract(request_id)?;
        if expected.scope() != scope
            || expected.observation_id() != observation_id
            || expected.observation_digest() != observation_digest
        {
            return Err(RelationalAnalysisJournalError::EventClaimMismatch(
                "mechanism plan/observation contract",
            ));
        }
        Ok(())
    }

    fn bind_selected_question(
        &mut self,
        seal: RelationalSelectedQuestionSeal,
    ) -> Result<bool, RelationalAnalysisJournalError> {
        seal.validate_identity()?;
        let expected_question_id = self.open_catalog_or_error()?.plan().question_id();
        if seal.question_id() != expected_question_id {
            return Err(
                RelationalAnalysisJournalError::SelectedQuestionScopeMismatch {
                    expected: expected_question_id,
                    actual: seal.question_id(),
                },
            );
        }
        match self.selected_question {
            Some(existing) if existing == seal => Ok(false),
            Some(_) => Err(RelationalAnalysisJournalError::SelectedQuestionSealReplacement),
            None => {
                self.scope_root = Some(RelationalAnalysisJournalScopeRoot::derive(
                    self.plan_root,
                    seal.id(),
                ));
                self.selected_question = Some(seal);
                Ok(true)
            }
        }
    }

    fn apply_terminal(
        &mut self,
        claimed_root: RelationalAnalysisCatalogRoot,
        claimed_closure_set_root: RelationalAnalysisClosureSetRoot,
    ) -> Result<RelationalAnalysisJournalApply, RelationalAnalysisJournalError> {
        if let Some(closed) = &self.closed {
            return if closed.root() == claimed_root
                && self.closed_closure_set_root == Some(claimed_closure_set_root)
            {
                Ok(RelationalAnalysisJournalApply::AlreadyAccepted)
            } else {
                Err(RelationalAnalysisJournalError::AnalysisClosureConflict)
            };
        }

        self.selected_question
            .ok_or(RelationalAnalysisJournalError::SelectedQuestionSealMissing)?;

        let open = self.open_catalog_or_error()?;
        let derived_closure_set_root = self.validate_and_derive_closure_set_root(open)?;
        open.validate_complete()?;
        let derived = open.root();
        if claimed_root != derived {
            return Err(RelationalAnalysisJournalError::AnalysisRootClaimMismatch {
                claimed: claimed_root,
                derived,
            });
        }
        if claimed_closure_set_root != derived_closure_set_root {
            return Err(
                RelationalAnalysisJournalError::AnalysisClosureSetRootClaimMismatch {
                    claimed: claimed_closure_set_root,
                    derived: derived_closure_set_root,
                },
            );
        }

        let builder = self
            .open
            .take()
            .expect("validated open analysis catalog remains present until terminal consume");
        let (closed, publication_discoveries) = builder
            .finish_with_mechanism_publication_discovery()
            .expect("terminal preflight and finish use the same immutable completeness checks");
        debug_assert_eq!(closed.root(), claimed_root);
        self.closed_mechanism_publication_discoveries = publication_discoveries;
        self.closed_closure_set_root = Some(derived_closure_set_root);
        self.closed = Some(closed);
        Ok(RelationalAnalysisJournalApply::Applied)
    }

    fn validate_and_derive_closure_set_root(
        &self,
        catalog: &RelationalAnalysisCatalogBuilder,
    ) -> Result<RelationalAnalysisClosureSetRoot, RelationalAnalysisJournalError> {
        let mechanism_count = catalog
            .plan()
            .layer_registrations()
            .iter()
            .filter(|registration| {
                matches!(
                    registration,
                    RelationalAnalysisLayerRegistration::Mechanisms(_)
                )
            })
            .count();
        if self.mechanism_closures.len() != mechanism_count
            || self.structural_mechanisms.len() != mechanism_count
            || self.mechanism_supports.len() != mechanism_count
            || self.support_closures.len() != mechanism_count
        {
            return Err(RelationalAnalysisJournalError::AnalysisClosureSetIncomplete);
        }

        let mut hasher = AnalysisEventHasher::new(ANALYSIS_CLOSURE_SET_ROOT_HASH_V1);
        hasher.u32(RELATIONAL_ANALYSIS_EVENT_SCHEMA_VERSION);
        hasher.digest(self.plan_root.bytes());
        hasher.u128(mechanism_count as u128);
        for registration in catalog.plan().layer_registrations() {
            if let RelationalAnalysisLayerRegistration::Mechanisms(mechanism) = registration {
                let request_id = mechanism.request_id();
                let stored = self.mechanism_closures.get(&request_id).copied().ok_or(
                    RelationalAnalysisJournalError::MechanismClosureMissing {
                        request_id: mechanism.request_id(),
                    },
                )?;
                let current = catalog.mechanism_closure_receipt(request_id)?;
                if current != stored {
                    return Err(RelationalAnalysisJournalError::MechanismClosureConflict {
                        request_id,
                    });
                }
                let incidence = catalog.mechanism_incidence(request_id)?;
                let closed_incidence = incidence.closed_ref().map_err(|error| {
                    RelationalAnalysisJournalError::Catalog(
                        RelationalAnalysisCatalogError::Mechanism(error),
                    )
                })?;
                if closed_incidence.root() != stored.incidence_root() {
                    return Err(RelationalAnalysisJournalError::MechanismClosureConflict {
                        request_id,
                    });
                }
                let structural = self.structural_mechanisms.get(&request_id).ok_or(
                    RelationalAnalysisJournalError::StructuralQuotientClosureMissing { request_id },
                )?;
                let structural_receipt = structural
                    .validate_closure_against_expected_signatures(
                        closed_incidence.signature_definition_count() as u128,
                        closed_incidence.signature_ids(),
                    )
                    .map_err(RelationalStructuralMechanismError::from)?;
                let support = self
                    .mechanism_supports
                    .get(&request_id)
                    .ok_or(RelationalAnalysisJournalError::SupportClosureMissing { request_id })?;
                let support_receipt =
                    self.support_closures.get(&request_id).copied().ok_or(
                        RelationalAnalysisJournalError::SupportClosureMissing { request_id },
                    )?;
                if support.closure() != Some(support_receipt)
                    || support_receipt.request_id() != request_id
                    || support_receipt.incidence_root() != stored.incidence_root()
                    || support_receipt.structural_root() != structural_receipt.root()
                {
                    return Err(RelationalAnalysisJournalError::SupportClosureConflict {
                        request_id,
                    });
                }
                hasher.digest(request_id.bytes());
                hasher.digest(stored.incidence_root().bytes());
                hasher.digest(structural_receipt.root().bytes());
                hasher.digest(support_receipt.root().bytes());
            }
        }
        Ok(RelationalAnalysisClosureSetRoot(hasher.finish()))
    }

    fn open_catalog_or_error(
        &self,
    ) -> Result<&RelationalAnalysisCatalogBuilder, RelationalAnalysisJournalError> {
        self.open
            .as_ref()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)
    }

    fn open_catalog_mut_or_error(
        &mut self,
    ) -> Result<&mut RelationalAnalysisCatalogBuilder, RelationalAnalysisJournalError> {
        self.open
            .as_mut()
            .ok_or(RelationalAnalysisJournalError::EventAfterAnalysisClosure)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisJournalApply {
    Applied,
    AlreadyAccepted,
}

impl RelationalAnalysisJournalApply {
    pub(crate) const fn changed(self) -> bool {
        matches!(self, Self::Applied)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisJournalError {
    Catalog(RelationalAnalysisCatalogError),
    ResultEvidence(ResultEvidenceError),
    MechanismReplay(RelationalMechanismReplayError),
    StructuralMechanism(RelationalStructuralMechanismError),
    MechanismSupport(MechanismSupportError),
    UnsupportedSelectedQuestionSealVersion {
        actual: u32,
        expected: u32,
    },
    InvalidCertifiedSupportPopulation,
    InvalidSelectedQuestionSeal,
    SelectedQuestionSealIdMismatch {
        claimed: RelationalSelectedQuestionSealId,
        derived: RelationalSelectedQuestionSealId,
    },
    SelectedQuestionScopeMismatch {
        expected: QuestionId,
        actual: QuestionId,
    },
    SelectedQuestionSealMissing,
    SelectedQuestionSealReplacement,
    SelectedQuestionSealMismatch,
    CertifiedSourceSummaryScopeMismatch {
        view_id: ViewId,
    },
    UnsupportedMechanismArtifactVersion {
        actual: u32,
        expected: u32,
    },
    MechanismArtifactCapacity {
        actual: usize,
        limit: usize,
    },
    MechanismArtifactChunkCapacity {
        actual: usize,
        limit: usize,
    },
    MechanismArtifactChunkCountCapacity {
        actual: usize,
        limit: usize,
    },
    MechanismArtifactIdMismatch,
    MechanismArtifactChunkDigestMismatch,
    MechanismArtifactInterleaving,
    MechanismArtifactNotOpen,
    MechanismArtifactChunkOrderMismatch,
    MechanismArtifactLengthMismatch,
    MechanismArtifactClosureMismatch,
    MechanismArtifactPayloadMismatch,
    MechanismArtifactResumeMismatch,
    MechanismArtifactPending,
    EventClaimMismatch(&'static str),
    PublishedViewMissing {
        view_id: ViewId,
    },
    PublishedViewConflict {
        view_id: ViewId,
    },
    MechanismClosureMissing {
        request_id: MechanismRequestId,
    },
    MechanismClosureConflict {
        request_id: MechanismRequestId,
    },
    MechanismPayloadAfterClosure {
        request_id: MechanismRequestId,
    },
    StructuralQuotientClosureMissing {
        request_id: MechanismRequestId,
    },
    StructuralQuotientClosureConflict {
        request_id: MechanismRequestId,
    },
    StructuralQuotientPayloadAfterClosure {
        request_id: MechanismRequestId,
    },
    SupportTargetCaseMissing {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    SupportTargetCaseResolutionMismatch {
        request_id: MechanismRequestId,
        expected: RelationalCaseId,
        actual: RelationalCaseId,
    },
    SupportTargetCursorOutOfRange {
        request_id: MechanismRequestId,
        requested: u128,
        current: u128,
        available: u128,
    },
    SupportTargetCatchUpIncomplete {
        request_id: MechanismRequestId,
    },
    SupportClosureMissing {
        request_id: MechanismRequestId,
    },
    SupportClosureConflict {
        request_id: MechanismRequestId,
    },
    AnalysisClosureSetIncomplete,
    AnalysisRootClaimMismatch {
        claimed: RelationalAnalysisCatalogRoot,
        derived: RelationalAnalysisCatalogRoot,
    },
    AnalysisClosureSetRootClaimMismatch {
        claimed: RelationalAnalysisClosureSetRoot,
        derived: RelationalAnalysisClosureSetRoot,
    },
    AnalysisClosureConflict,
    AnalysisStateDiverged,
    EventAfterAnalysisClosure,
}

impl From<RelationalAnalysisCatalogError> for RelationalAnalysisJournalError {
    fn from(error: RelationalAnalysisCatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<ResultEvidenceError> for RelationalAnalysisJournalError {
    fn from(error: ResultEvidenceError) -> Self {
        Self::ResultEvidence(error)
    }
}

impl From<RelationalMechanismReplayError> for RelationalAnalysisJournalError {
    fn from(error: RelationalMechanismReplayError) -> Self {
        Self::MechanismReplay(error)
    }
}

impl From<RelationalStructuralMechanismError> for RelationalAnalysisJournalError {
    fn from(error: RelationalStructuralMechanismError) -> Self {
        Self::StructuralMechanism(error)
    }
}

impl From<MechanismSupportError> for RelationalAnalysisJournalError {
    fn from(error: MechanismSupportError) -> Self {
        Self::MechanismSupport(error)
    }
}

impl fmt::Display for RelationalAnalysisJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(error) => error.fmt(formatter),
            Self::ResultEvidence(error) => error.fmt(formatter),
            Self::MechanismReplay(error) => error.fmt(formatter),
            Self::StructuralMechanism(error) => error.fmt(formatter),
            Self::MechanismSupport(error) => error.fmt(formatter),
            Self::UnsupportedSelectedQuestionSealVersion { actual, expected } => write!(
                formatter,
                "unsupported selected-question seal version {actual}; expected {expected}"
            ),
            Self::InvalidCertifiedSupportPopulation => formatter.write_str(
                "support evidence and concrete cases do not certify the same closed selected population",
            ),
            Self::InvalidSelectedQuestionSeal => formatter.write_str(
                "selected-question seal has inconsistent typed upstream or set commitments",
            ),
            Self::SelectedQuestionSealIdMismatch { .. } => formatter.write_str(
                "selected-question seal ID does not match its canonical semantic content",
            ),
            Self::SelectedQuestionScopeMismatch { .. } => formatter
                .write_str("selected-question seal belongs to another analysis plan question"),
            Self::SelectedQuestionSealMissing => formatter.write_str(
                "analysis evidence arrived before the exact selected-question seal was bound",
            ),
            Self::SelectedQuestionSealReplacement => formatter
                .write_str("the bound selected-question closure receipt cannot be replaced"),
            Self::SelectedQuestionSealMismatch => formatter
                .write_str("analysis event names a different selected-question closure receipt"),
            Self::CertifiedSourceSummaryScopeMismatch { .. } => formatter.write_str(
                "certified source summary does not match its registered result layer",
            ),
            Self::UnsupportedMechanismArtifactVersion { actual, expected } => write!(
                formatter,
                "unsupported mechanism artifact version {actual}; expected {expected}"
            ),
            Self::MechanismArtifactCapacity { actual, limit } => write!(
                formatter,
                "mechanism artifact needs {actual} bytes; durable limit is {limit}"
            ),
            Self::MechanismArtifactChunkCapacity { actual, limit } => write!(
                formatter,
                "mechanism artifact chunk needs {actual} bytes; frame payload limit is {limit}"
            ),
            Self::MechanismArtifactChunkCountCapacity { actual, limit } => write!(
                formatter,
                "mechanism artifact needs {actual} chunks; durable chunk-count limit is {limit}"
            ),
            Self::MechanismArtifactIdMismatch => formatter.write_str(
                "mechanism artifact ID does not match its typed claim and payload digest",
            ),
            Self::MechanismArtifactChunkDigestMismatch => formatter
                .write_str("mechanism artifact chunk digest does not match its bounded bytes"),
            Self::MechanismArtifactInterleaving => formatter.write_str(
                "another answer-defining event cannot interleave an open mechanism artifact",
            ),
            Self::MechanismArtifactNotOpen => formatter
                .write_str("mechanism artifact chunk or closure arrived without an open header"),
            Self::MechanismArtifactChunkOrderMismatch => formatter.write_str(
                "mechanism artifact chunk is not the next contiguous ordinal and byte offset",
            ),
            Self::MechanismArtifactLengthMismatch => formatter
                .write_str("mechanism artifact chunks exceed the header's exact payload length"),
            Self::MechanismArtifactClosureMismatch => formatter
                .write_str("mechanism artifact closure does not match its exact chunk prefix"),
            Self::MechanismArtifactPayloadMismatch => formatter
                .write_str("mechanism artifact bytes do not reproduce the header payload digest"),
            Self::MechanismArtifactResumeMismatch => formatter.write_str(
                "fresh mechanism replay does not reproduce the durable open-artifact prefix",
            ),
            Self::MechanismArtifactPending => formatter
                .write_str("analysis cannot close while a mechanism artifact is still open"),
            Self::EventClaimMismatch(component) => write!(
                formatter,
                "analysis journal {component} claim does not match its complete payload"
            ),
            Self::PublishedViewMissing { .. } => formatter.write_str(
                "chosen-view mechanism target arrived before its result publication event",
            ),
            Self::PublishedViewConflict { .. } => formatter
                .write_str("result view was republished with a different immutable payload"),
            Self::MechanismClosureMissing { .. } => formatter.write_str(
                "analysis dependency requires an explicit exact mechanism-closure event",
            ),
            Self::MechanismClosureConflict { .. } => formatter.write_str(
                "mechanism incidence was closed with a different authenticated content root",
            ),
            Self::MechanismPayloadAfterClosure { .. } => formatter.write_str(
                "raw request target or replay payload cannot arrive after incidence closure",
            ),
            Self::StructuralQuotientClosureMissing { .. } => formatter.write_str(
                "analysis dependency requires an explicit exact structural-quotient closure event",
            ),
            Self::StructuralQuotientClosureConflict { .. } => formatter.write_str(
                "structural quotient was closed with a different authenticated content root",
            ),
            Self::StructuralQuotientPayloadAfterClosure { .. } => formatter.write_str(
                "structural quotient assignments cannot arrive after request closure",
            ),
            Self::SupportTargetCaseMissing { .. } => formatter.write_str(
                "the outer relation catalog could not resolve a support target CaseId",
            ),
            Self::SupportTargetCaseResolutionMismatch { .. } => formatter.write_str(
                "the outer relation catalog resolved a different support target CaseId",
            ),
            Self::SupportTargetCursorOutOfRange { .. } => formatter.write_str(
                "mechanism support checkpoint cursor is behind its durable prefix or beyond raw incidence",
            ),
            Self::SupportTargetCatchUpIncomplete { .. } => formatter.write_str(
                "mechanism support cannot close before checked relation coordinates catch up to the raw target prefix",
            ),
            Self::SupportClosureMissing { .. } => formatter.write_str(
                "analysis dependency requires an explicit exact mechanism-support closure event",
            ),
            Self::SupportClosureConflict { .. } => formatter.write_str(
                "mechanism support was closed with different authenticated evidence",
            ),
            Self::AnalysisClosureSetIncomplete => formatter.write_str(
                "terminal analysis closure requires raw, structural, and support closure receipts for every mechanism request",
            ),
            Self::AnalysisRootClaimMismatch { .. } => formatter.write_str(
                "terminal analysis root claim does not match the fully rederived catalog root",
            ),
            Self::AnalysisClosureSetRootClaimMismatch { .. } => formatter.write_str(
                "terminal analysis closure-set claim does not match the rederived request closure roots",
            ),
            Self::AnalysisClosureConflict => {
                formatter.write_str("analysis was already closed under a different catalog root")
            }
            Self::AnalysisStateDiverged => formatter
                .write_str("analysis journal does not contain exactly one open or closed catalog"),
            Self::EventAfterAnalysisClosure => formatter.write_str(
                "answer-defining analysis evidence cannot change after terminal closure",
            ),
        }
    }
}

impl Error for RelationalAnalysisJournalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Catalog(error) => Some(error),
            Self::ResultEvidence(error) => Some(error),
            Self::MechanismReplay(error) => Some(error),
            Self::StructuralMechanism(error) => Some(error),
            Self::MechanismSupport(error) => Some(error),
            _ => None,
        }
    }
}

fn structural_quotient_artifact_claim(
    artifact: &StructuralSignatureQuotientArtifact,
) -> RelationalMechanismArtifactClaim {
    RelationalMechanismArtifactClaim::StructuralQuotient {
        request_id: artifact.signature_id().request_id(),
        raw_signature_id: artifact.signature_id(),
        structural_mechanism_id: artifact.mechanism().id(),
        execution_profile_id: artifact.profile().id(),
    }
}

const fn mechanism_artifact_max_bytes(claim: RelationalMechanismArtifactClaim) -> usize {
    match claim {
        RelationalMechanismArtifactClaim::StructuralQuotient { .. } => {
            RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES
        }
        RelationalMechanismArtifactClaim::Signature { .. }
        | RelationalMechanismArtifactClaim::Incidence { .. }
        | RelationalMechanismArtifactClaim::Unavailable { .. } => {
            RELATIONAL_MECHANISM_ARTIFACT_MAX_BYTES
        }
    }
}

fn build_mechanism_artifact_events(
    claim: RelationalMechanismArtifactClaim,
    payload: &[u8],
    chunk_bytes: usize,
) -> Result<Box<[RelationalAnalysisEvidenceEvent]>, RelationalAnalysisJournalError> {
    let max_bytes = mechanism_artifact_max_bytes(claim);
    if payload.is_empty() || payload.len() > max_bytes {
        return Err(RelationalAnalysisJournalError::MechanismArtifactCapacity {
            actual: payload.len(),
            limit: max_bytes,
        });
    }
    if chunk_bytes == 0 || chunk_bytes > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES {
        return Err(
            RelationalAnalysisJournalError::MechanismArtifactChunkCapacity {
                actual: chunk_bytes,
                limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES,
            },
        );
    }
    let header = RelationalMechanismArtifactHeader::issue(claim, payload);
    let chunk_count = payload.len().div_ceil(chunk_bytes);
    if chunk_count > RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS {
        return Err(
            RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                actual: chunk_count,
                limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
            },
        );
    }
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(chunk_count).map_err(|_| {
        RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
            actual: chunk_count,
            limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
        }
    })?;
    for (ordinal, bytes) in payload.chunks(chunk_bytes).enumerate() {
        chunks.push(RelationalMechanismArtifactChunk::issue(
            header.id(),
            u32::try_from(ordinal).map_err(|_| {
                RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                    actual: ordinal,
                    limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
                }
            })?,
            (ordinal * chunk_bytes) as u64,
            bytes,
        ));
    }
    let closure = RelationalMechanismArtifactClosure {
        artifact_id: header.id(),
        chunk_count: u32::try_from(chunks.len()).map_err(|_| {
            RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
                actual: chunks.len(),
                limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS,
            }
        })?,
        chunk_root: derive_mechanism_artifact_chunk_root(header.id(), &chunks),
    };
    closure.validate_shape()?;
    let mut events = Vec::new();
    events.try_reserve_exact(chunks.len() + 2).map_err(|_| {
        RelationalAnalysisJournalError::MechanismArtifactChunkCountCapacity {
            actual: chunks.len() + 2,
            limit: RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNKS + 2,
        }
    })?;
    events.push(RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header });
    events.extend(
        chunks
            .into_iter()
            .map(|chunk| RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { chunk }),
    );
    events.push(RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { closure });
    Ok(events.into_boxed_slice())
}

fn derive_mechanism_artifact_id(
    version: u32,
    claim: RelationalMechanismArtifactClaim,
    payload_digest: [u8; 32],
    total_bytes: u64,
) -> RelationalMechanismArtifactId {
    let mut hasher = AnalysisEventHasher::new(MECHANISM_ARTIFACT_ID_HASH_V1);
    hasher.u32(version);
    hash_mechanism_artifact_claim(&mut hasher, claim);
    hasher.digest(payload_digest);
    hasher.u128(u128::from(total_bytes));
    RelationalMechanismArtifactId(hasher.finish())
}

fn derive_mechanism_artifact_chunk_digest(
    artifact_id: RelationalMechanismArtifactId,
    ordinal: u32,
    offset: u64,
    bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = AnalysisEventHasher::new(MECHANISM_ARTIFACT_CHUNK_HASH_V1);
    hasher.digest(artifact_id.bytes());
    hasher.u32(ordinal);
    hasher.u128(u128::from(offset));
    hasher.bytes(bytes);
    hasher.finish()
}

fn derive_mechanism_artifact_chunk_root(
    artifact_id: RelationalMechanismArtifactId,
    chunks: &[RelationalMechanismArtifactChunk],
) -> RelationalMechanismArtifactChunkRoot {
    derive_mechanism_artifact_chunk_root_from_iter(artifact_id, chunks.len(), chunks.iter())
}

fn derive_mechanism_artifact_chunk_root_from_iter<'a>(
    artifact_id: RelationalMechanismArtifactId,
    chunk_count: usize,
    chunks: impl IntoIterator<Item = &'a RelationalMechanismArtifactChunk>,
) -> RelationalMechanismArtifactChunkRoot {
    let mut hasher = AnalysisEventHasher::new(MECHANISM_ARTIFACT_CHUNK_ROOT_HASH_V1);
    hasher.digest(artifact_id.bytes());
    hasher.u128(chunk_count as u128);
    for chunk in chunks {
        hasher.u32(chunk.ordinal());
        hasher.u128(u128::from(chunk.offset()));
        hasher.digest(chunk.chunk_digest());
        hasher.u128(chunk.bytes().len() as u128);
    }
    RelationalMechanismArtifactChunkRoot(hasher.finish())
}

fn hash_mechanism_artifact_header(
    hasher: &mut AnalysisEventHasher,
    header: RelationalMechanismArtifactHeader,
) {
    hasher.u32(header.version());
    hasher.digest(header.id().bytes());
    hash_mechanism_artifact_claim(hasher, header.claim());
    hasher.digest(header.payload_digest());
    hasher.u128(u128::from(header.total_bytes()));
}

fn hash_mechanism_artifact_claim(
    hasher: &mut AnalysisEventHasher,
    claim: RelationalMechanismArtifactClaim,
) {
    hasher.tag(claim.canonical_tag());
    match claim {
        RelationalMechanismArtifactClaim::Signature {
            request_id,
            signature_id,
        } => {
            hasher.digest(request_id.bytes());
            hasher.digest(signature_id.bytes());
        }
        RelationalMechanismArtifactClaim::Incidence {
            request_id,
            observation_id,
            observation_digest,
            replay_observation_id,
            case_id,
            transition_id,
            signature_id,
            replay_receipt_id,
        } => {
            hasher.digest(request_id.bytes());
            hasher.digest(observation_id.bytes());
            hasher.digest(observation_digest.bytes());
            hasher.digest(replay_observation_id.bytes());
            hasher.digest(case_id.bytes());
            hasher.digest(transition_id.bytes());
            hasher.digest(signature_id.bytes());
            hasher.digest(replay_receipt_id.bytes());
        }
        RelationalMechanismArtifactClaim::Unavailable {
            request_id,
            observation_id,
            observation_digest,
            replay_observation_id,
            case_id,
            transition_id,
            reason_id,
        } => {
            hasher.digest(request_id.bytes());
            hasher.digest(observation_id.bytes());
            hasher.digest(observation_digest.bytes());
            hasher.digest(replay_observation_id.bytes());
            hasher.digest(case_id.bytes());
            hasher.digest(transition_id.bytes());
            hasher.digest(reason_id.bytes());
        }
        RelationalMechanismArtifactClaim::StructuralQuotient {
            request_id,
            raw_signature_id,
            structural_mechanism_id,
            execution_profile_id,
        } => {
            hasher.digest(request_id.bytes());
            hasher.digest(raw_signature_id.bytes());
            hasher.digest(structural_mechanism_id.bytes());
            hasher.digest(execution_profile_id.bytes());
        }
    }
}

fn derive_selected_question_seal_id(
    version: u32,
    question_id: QuestionId,
    authority: RelationalSelectedPopulationAuthority,
    result_input_seal: RelationalResultInputSeal,
    mechanism_target: MechanismTargetCaseSetCommitment,
) -> RelationalSelectedQuestionSealId {
    let mut hasher = AnalysisEventHasher::new(SELECTED_QUESTION_SEAL_ID_HASH_V2);
    hasher.u32(version);
    hasher.digest(question_id.bytes());
    hash_selected_population_authority(&mut hasher, authority);
    hash_result_input_seal(&mut hasher, result_input_seal);
    hasher.digest(mechanism_target.root().bytes());
    hasher.u128(mechanism_target.count());
    RelationalSelectedQuestionSealId(hasher.finish())
}

fn hash_result_input_seal(hasher: &mut AnalysisEventHasher, seal: RelationalResultInputSeal) {
    match seal.upstream() {
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
    let coverage = seal.coverage();
    hash_input_kind(hasher, coverage.input_kind());
    hasher.u128(coverage.row_count());
    hasher.digest(coverage.row_set_root().bytes());
}

fn hash_selected_population_authority(
    hasher: &mut AnalysisEventHasher,
    authority: RelationalSelectedPopulationAuthority,
) {
    match authority {
        RelationalSelectedPopulationAuthority::ExtensionalQuestion { content_root } => {
            hasher.tag(0x01);
            hasher.digest(content_root.bytes());
        }
        RelationalSelectedPopulationAuthority::CertifiedSupport {
            population_root,
            exact_cardinality,
        } => {
            hasher.tag(0x02);
            hasher.digest(population_root.bytes());
            hasher.u128(exact_cardinality);
        }
    }
}

fn hash_result_input(hasher: &mut AnalysisEventHasher, input: RelationalResolvedResultInput) {
    match input {
        RelationalResolvedResultInput::Sources(relation_id) => {
            hasher.tag(0x03);
            hasher.digest(relation_id.bytes());
        }
        RelationalResolvedResultInput::Selected(question_id) => {
            hasher.tag(0x01);
            hasher.digest(question_id.bytes());
        }
        RelationalResolvedResultInput::MechanismIncidence(request_id) => {
            hasher.tag(0x02);
            hasher.digest(request_id.bytes());
        }
    }
}

fn hash_input_kind(hasher: &mut AnalysisEventHasher, input_kind: ResultViewInputKind) {
    hasher.tag(match input_kind {
        ResultViewInputKind::Source => 0x03,
        ResultViewInputKind::Case => 0x01,
        ResultViewInputKind::Incidence => 0x02,
    });
}

fn hash_result_counts(hasher: &mut AnalysisEventHasher, counts: ResultViewCounts) {
    hash_result_count(hasher, counts.input_rows());
    match counts.groups() {
        None => hasher.tag(0x00),
        Some(count) => {
            hasher.tag(0x01);
            hash_result_count(hasher, count);
        }
    }
    match counts.output_groups() {
        None => hasher.tag(0x00),
        Some(count) => {
            hasher.tag(0x01);
            hash_result_count(hasher, count);
        }
    }
    hash_result_count(hasher, counts.output_rows());
}

fn hash_result_count(hasher: &mut AnalysisEventHasher, count: ResultViewCount) {
    match count {
        ResultViewCount::LowerBound(value) => {
            hasher.tag(0x01);
            hasher.u128(value);
        }
        ResultViewCount::Provisional(value) => {
            hasher.tag(0x02);
            hasher.u128(value);
        }
        ResultViewCount::Exact(value) => {
            hasher.tag(0x03);
            hasher.u128(value);
        }
    }
}

struct AnalysisEventHasher(Sha256);

impl AnalysisEventHasher {
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

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
