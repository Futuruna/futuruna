//! Canonical binary codec for authenticated relational-journal entries.
//!
//! The outer segmented store treats these bytes as opaque. This layer owns
//! semantic schema tags, bounded allocation, and checked reconstruction. A
//! decoder is always given the expected sequence and previous journal head;
//! [`RelationalJournalEntry::restore_from_journal_codec`] then recomputes the
//! claimed terminal head from the decoded event and contract.
//!
//! Proof receipts are intentionally different from content identities. Raw
//! support events that would deserialize a `SupportProofReceipt` still fail
//! with [`RelationalJournalCodecError::ProofPolicyRequired`]. The narrow
//! producer-proof events instead retain canonical structural artifacts;
//! journal replay re-verifies each artifact against the installed plan and
//! privately remints receipts through their bound gateways.

use std::error::Error;
use std::fmt;
use std::num::{NonZeroU128, NonZeroU32};

use super::mechanism_incidence::{
    MechanismIncidenceRoot, MechanismSignatureId, MechanismTargetCaseSetCommitment,
    MechanismUnavailableReasonId,
};
use super::mechanism_support::{
    MechanismSupportCheckpointCursor, MechanismSupportClosureRoot, MechanismSupportFrontierRoot,
};
use super::relation::{
    AdmissionDecision, AdmissionId, MechanismRequestId, QuestionContentRoot, QuestionId,
    RelationId, RelationLineageId, RelationProvenance, RelationSupportId, RelationalCaseId,
    SelectionDecision, SourceKey, SourceKeySetRoot, SourceRow, SuccessorKey, SuccessorRow, ViewId,
};
use super::relational_analysis_catalog::RelationalAnalysisCatalogRoot;
use super::relational_analysis_journal::{
    RelationalAnalysisClosureSetRoot, RelationalAnalysisEvidenceEvent,
    RelationalAnalysisJournalError, RelationalMechanismArtifactChunk,
    RelationalMechanismArtifactChunkRoot, RelationalMechanismArtifactClaim,
    RelationalMechanismArtifactClosure, RelationalMechanismArtifactHeader,
    RelationalMechanismArtifactId, RelationalSelectedPopulationAuthority,
    RelationalSelectedQuestionSeal, RelationalSelectedQuestionSealId,
};
use super::relational_analysis_plan::{
    RelationalAnalysisDependencyId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanError, RelationalAnalysisPlanRoot, RelationalCheckedAnalysisGraphDigest,
    RelationalMechanismLayerRegistration, RelationalMechanismObservationDigest,
    RelationalMechanismObservationId, RelationalResolvedMechanismTarget,
    RelationalResolvedResultInput, RelationalResultLayerRegistration, RelationalResultSpecDigest,
};
use super::relational_bounded_chunk_partition::{
    RelationalCaseChunkDescriptor, RelationalCaseChunkId, RelationalCaseChunkPartitionArtifact,
    RelationalCaseChunkPartitionArtifactId, RelationalCaseChunkPartitionError,
    RelationalCaseChunkShape, RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1,
};
use super::relational_case_executor::{
    RelationalCaseExecutorError, SuccessorFiberExhaustionReceipt, SuccessorFiberExhaustionReceiptId,
};
use super::relational_certified_source_summary::{
    RelationalCertifiedSourceSummaryArtifact, RelationalCertifiedSourceSummaryArtifactId,
    RelationalCertifiedSourceSummaryError, RELATIONAL_CERTIFIED_SOURCE_SUMMARY_MAX_GROUPS,
    RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION, RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1,
};
use super::relational_classified_sweep::{
    RelationalClassifiedCaseOutcome, RelationalClassifiedChunkArtifact,
    RelationalClassifiedChunkArtifactId, RelationalClassifiedChunkSliceArtifact,
    RelationalClassifiedChunkSliceId, RelationalClassifiedChunkSliceRun,
    RelationalClassifiedChunkTranscriptRoot, RelationalClassifiedRunDescriptor,
    RelationalClassifiedRunId, RelationalClassifiedSweepError,
};
use super::relational_executor::{
    RelationalBindingSelection, RelationalCompletedSource, RelationalFiberMember,
    RelationalSourceAdvance, RelationalSourceContinuation, RelationalSourceCursor,
    RelationalSourceCursorSnapshot, RelationalSourceExecutorError, RelationalSourcePrefixSnapshot,
    SourceBindingExhaustionReceipt, SourceBindingExhaustionReceiptId,
};
use super::relational_frontier::{
    CanonicalSourcePrefix, MechanismEndpoint, WorkCompletionRef, WorkFrontierCompaction,
    WorkFrontierError, WorkNodeId, WorkNodeSpec,
};
use super::relational_ir::ExploreSourceBindingRoleIr;
use super::relational_journal::{
    RelationalCheckpointEvent, RelationalEvidenceEvent, RelationalJournalContract,
    RelationalJournalEntry, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RELATIONAL_JOURNAL_SCHEMA_VERSION,
};
use super::relational_mechanism_executor::{
    RelationalMechanismReplayObservationId, RelationalMechanismReplayReceiptId,
};
use super::relational_population::CertifiedSelectedPopulationRoot;
use super::relational_selected_run_materialization::{
    RelationalSelectedCaseRecord, RelationalSelectedRunMaterializationArtifact,
    RelationalSelectedRunMaterializationArtifactId, RelationalSelectedRunMaterializationError,
};
use super::relational_source_closure::{
    SourceFiberReceiptSetRoot, SourceRelationExhaustionReceipt, SourceTraversalAdvanceId,
    SourceTraversalClosureError, SourceTraversalEdgeRoot,
};
use super::relational_source_image_exactness::{
    CertifiedSourcePopulationRoot, RelationalSourceImageExactnessProofArtifact,
    RelationalSourceImageExactnessProofError, RelationalSourceImageExactnessProofShape,
    RelationalSourceImageFactorBinding, RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION,
    RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1,
};
use super::relational_support_planner::{
    RelationalBindingStage, RelationalBindingStageId, RelationalCaseImageAssignmentKind,
    RelationalCaseImageInjectivityProofArtifact, RelationalCaseImageInjectivityProofError,
    RelationalCaseImagePreimageKind, RelationalCaseSourceImageProofReference,
    RelationalCoverageQualifier, RelationalCoverageStatus, RelationalDependencyKeyRecipe,
    RelationalDimensionId, RelationalExactEmptyReason, RelationalFactorSchema,
    RelationalFiniteDomainRecipeKind, RelationalFiniteFactorRecipe, RelationalFiniteFactorStage,
    RelationalLiteralAdmissionPredicate, RelationalObligationActivation,
    RelationalPlannedPopulation, RelationalPlannedSupport, RelationalRootObligationPlan,
    RelationalSingletonMapStage, RelationalSourceAssignmentImageProof,
    RelationalStagedObligationDescriptor, RelationalSuccessorRecipeKind,
    RelationalSupportExactness, RelationalSupportOpenReason, RelationalSupportPlan,
    RelationalSupportPlanRoot, RelationalSupportPlannerError, RelationalSupportPopulationKind,
    RelationalSupportPopulationRecipe, RelationalUniformAdmissionProofRecipe,
    RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION,
};
use super::relational_uniform_admission_proof::{
    RelationalUniformAdmissionProofArtifact, RelationalUniformAdmissionProofError,
};
use super::result_evidence::{
    RelationalResultEvidenceRecord, RelationalResultEvidenceRoot, RelationalResultInputSeal,
    ResultEvidenceError, ResultEvidenceUpstreamRoot, ResultInputCoverageRoot,
};
use super::result_projection::{
    IndexedResultProjectionRecord, ResultProjectionClosure, ResultProjectionError,
    ResultProjectionGroup, ResultProjectionRecord, ResultProjectionRecordId, ResultProjectionRoot,
};
use super::result_view::{
    CertifiedResultGroupSummary, CertifiedResultInputRoot, EvaluatedResultContribution,
    MechanismIncidenceRowId, ResultCountDistinctSnapshot, ResultGroupDisposition, ResultGroupKey,
    ResultOutputRow, ResultValue, ResultViewChoice, ResultViewCount, ResultViewCounts,
    ResultViewError, ResultViewGrain, ResultViewHaving, ResultViewInputKind, ResultViewInputRowId,
    ResultViewRoot, ResultViewSpec, ResultViewSpecRoot,
};
use super::structural_mechanism::{
    ExecutionProfileId, StructuralMechanismId, StructuralQuotientClosureRoot,
};
use super::support_cell::{
    AdmissionClassificationClaim, ExactCardinalityClaim, InjectiveMappingClaim,
    SelectionClassificationClaim, SupportCell, SupportCellError, SupportCellEvidenceId,
    SupportCellId, SupportCellObligation, SupportCellSpace, SupportExpr, SupportExprKind,
    SupportExtensionalTarget, SupportMaterializationCursor, SupportMaterializerId,
    SupportObserverId, SupportPartitionId, SupportProducerId, SupportProofObligationId,
    UniformMechanismClaim, UniformValueClaim,
};
use super::support_evidence::{
    SupportEvidenceError, SupportObligationRecord, SupportObligationRefinement,
    SupportObligationRefinementId,
};
use super::support_journal::{SupportJournalError, SupportJournalEvent};
use super::transition::TransitionId;
use super::ExploreValue;
use crate::{
    CheckedExploreSourceImageProjectionCertificate, CheckedExploreSourceProjectionEndpoint,
    CheckedExploreSourceProjectionFactor, CheckedExploreSourceProjectionField,
    CheckedExploreSourceProjectionWitness, ExploreAdmissionScope, ExploreChooseCardinality,
    ExploreOptimizeDirection,
};

pub(crate) const RELATIONAL_JOURNAL_CODEC_SCHEMA_VERSION: u32 = 11;

// Stable family marker; the following two u32 fields carry the independently
// checked codec and semantic-journal schema generations.
const ENTRY_MAGIC: &[u8; 8] = b"FTRJEVNT";
const ENTRY_FIXED_BYTES: usize = ENTRY_MAGIC.len() + 4 + 4 + 8 + 32 + 32 + 8;

/// A packed physical journal frame is the concatenation of canonical entry
/// encodings, each preceded by its canonical big-endian byte length. The
/// physical store authenticates the semantic range and terminal head; replay
/// uses this delimiter to recover every intermediate entry without allocating
/// the whole semantic batch.
pub(crate) const RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES: usize = 8;

pub(crate) const RELATIONAL_JOURNAL_CODEC_HARD_MAX_ENTRY_BYTES: usize = 64 << 20;
pub(crate) const RELATIONAL_JOURNAL_CODEC_HARD_MAX_COLLECTION_ITEMS: usize = 1_000_000;
pub(crate) const RELATIONAL_JOURNAL_CODEC_HARD_MAX_VALUE_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalCodecLimits {
    max_entry_bytes: usize,
    max_blob_bytes: usize,
    max_string_bytes: usize,
    max_collection_items: usize,
    max_value_depth: usize,
    max_value_nodes: usize,
}

impl RelationalJournalCodecLimits {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        max_entry_bytes: usize,
        max_blob_bytes: usize,
        max_string_bytes: usize,
        max_collection_items: usize,
        max_value_depth: usize,
        max_value_nodes: usize,
    ) -> Result<Self, RelationalJournalCodecError> {
        if !(ENTRY_FIXED_BYTES..=RELATIONAL_JOURNAL_CODEC_HARD_MAX_ENTRY_BYTES)
            .contains(&max_entry_bytes)
            || max_blob_bytes > max_entry_bytes
            || max_string_bytes > max_blob_bytes
            || max_collection_items == 0
            || max_collection_items > RELATIONAL_JOURNAL_CODEC_HARD_MAX_COLLECTION_ITEMS
            || max_value_depth == 0
            || max_value_depth > RELATIONAL_JOURNAL_CODEC_HARD_MAX_VALUE_DEPTH
            || max_value_nodes == 0
            || max_value_nodes > RELATIONAL_JOURNAL_CODEC_HARD_MAX_COLLECTION_ITEMS
        {
            return Err(RelationalJournalCodecError::InvalidLimits);
        }
        Ok(Self {
            max_entry_bytes,
            max_blob_bytes,
            max_string_bytes,
            max_collection_items,
            max_value_depth,
            max_value_nodes,
        })
    }

    pub(crate) const fn max_entry_bytes(self) -> usize {
        self.max_entry_bytes
    }

    pub(crate) const fn max_blob_bytes(self) -> usize {
        self.max_blob_bytes
    }
}

impl Default for RelationalJournalCodecLimits {
    fn default() -> Self {
        Self {
            max_entry_bytes: 16 << 20,
            max_blob_bytes: 8 << 20,
            max_string_bytes: 1 << 20,
            max_collection_items: 1_000_000,
            max_value_depth: 64,
            max_value_nodes: 1_000_000,
        }
    }
}

/// One bounded physical-frame accumulator. It deliberately owns only the
/// current frame: callers install or copy `bytes()` before `clear()` reuses the
/// allocation for the next frame.
pub(crate) struct RelationalJournalPackedFrameBuilder {
    bytes: Vec<u8>,
    max_frame_bytes: usize,
    semantic_event_count: u64,
}

impl RelationalJournalPackedFrameBuilder {
    pub(crate) fn new(max_frame_bytes: usize) -> Result<Self, RelationalJournalCodecError> {
        if max_frame_bytes < RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES + ENTRY_FIXED_BYTES {
            return Err(RelationalJournalCodecError::InvalidPackedFrameLimit {
                limit: max_frame_bytes,
            });
        }
        Ok(Self {
            bytes: Vec::new(),
            max_frame_bytes,
            semantic_event_count: 0,
        })
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.semantic_event_count == 0
    }

    pub(crate) const fn semantic_event_count(&self) -> u64 {
        self.semantic_event_count
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Append one canonical entry. `Ok(false)` means the nonempty current
    /// frame must be installed first; an entry that cannot fit an empty frame
    /// is a policy error rather than an invitation to exceed the bound.
    pub(crate) fn try_append(&mut self, entry: &[u8]) -> Result<bool, RelationalJournalCodecError> {
        let encoded_bytes = RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES
            .checked_add(entry.len())
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        if encoded_bytes > self.max_frame_bytes {
            return Err(RelationalJournalCodecError::PackedEntryTooLarge {
                bytes: encoded_bytes,
                limit: self.max_frame_bytes,
            });
        }
        let final_len = self
            .bytes
            .len()
            .checked_add(encoded_bytes)
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        if final_len > self.max_frame_bytes {
            debug_assert!(!self.is_empty());
            return Ok(false);
        }
        self.bytes.try_reserve_exact(encoded_bytes).map_err(|_| {
            RelationalJournalCodecError::AllocationFailed {
                requested: final_len,
            }
        })?;
        self.bytes.extend_from_slice(
            &u64::try_from(entry.len())
                .map_err(|_| RelationalJournalCodecError::LengthNotRepresentable {
                    component: "packed journal entry",
                })?
                .to_be_bytes(),
        );
        self.bytes.extend_from_slice(entry);
        self.semantic_event_count = self
            .semantic_event_count
            .checked_add(1)
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        Ok(true)
    }

    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.semantic_event_count = 0;
    }
}

/// Allocation-free reader for the length-delimited canonical entries inside
/// one already bounded physical frame.
pub(crate) struct RelationalJournalPackedFrameReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    remaining: u64,
    max_entry_bytes: usize,
}

impl<'a> RelationalJournalPackedFrameReader<'a> {
    pub(crate) fn new(
        bytes: &'a [u8],
        semantic_event_count: u64,
        max_frame_bytes: usize,
        limits: RelationalJournalCodecLimits,
    ) -> Result<Self, RelationalJournalCodecError> {
        if bytes.len() > max_frame_bytes {
            return Err(RelationalJournalCodecError::PackedFrameTooLarge {
                bytes: bytes.len(),
                limit: max_frame_bytes,
            });
        }
        if semantic_event_count == 0 {
            return Err(RelationalJournalCodecError::Malformed(
                "packed physical frame has no semantic entries",
            ));
        }
        Ok(Self {
            bytes,
            cursor: 0,
            remaining: semantic_event_count,
            max_entry_bytes: limits.max_entry_bytes(),
        })
    }

    pub(crate) fn next_entry(&mut self) -> Result<Option<&'a [u8]>, RelationalJournalCodecError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        let prefix_end = self
            .cursor
            .checked_add(RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(RelationalJournalCodecError::Truncated)?;
        let claimed = u64::from_be_bytes(
            self.bytes[self.cursor..prefix_end]
                .try_into()
                .expect("fixed packed-entry length bytes"),
        );
        let entry_bytes = usize::try_from(claimed).map_err(|_| {
            RelationalJournalCodecError::LengthNotRepresentable {
                component: "packed journal entry",
            }
        })?;
        if entry_bytes > self.max_entry_bytes {
            return Err(RelationalJournalCodecError::DeclaredLengthTooLarge {
                component: "packed journal entry",
                claimed,
                limit: self.max_entry_bytes,
            });
        }
        let entry_end = prefix_end
            .checked_add(entry_bytes)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(RelationalJournalCodecError::Truncated)?;
        self.cursor = entry_end;
        self.remaining -= 1;
        Ok(Some(&self.bytes[prefix_end..entry_end]))
    }

    pub(crate) fn finish(self) -> Result<(), RelationalJournalCodecError> {
        if self.remaining != 0 {
            return Err(RelationalJournalCodecError::PackedEventCountMismatch {
                remaining: self.remaining,
            });
        }
        if self.cursor != self.bytes.len() {
            return Err(RelationalJournalCodecError::TrailingBytes {
                bytes: self.bytes.len() - self.cursor,
            });
        }
        Ok(())
    }
}

/// Coordinator-facing canonical entry encoder using the maintained bounded
/// defaults. Callers that deliberately need tighter limits may use
/// [`encode_relational_journal_entry`] directly.
pub(crate) fn encode_entry(
    entry: &RelationalJournalEntry,
) -> Result<Vec<u8>, RelationalJournalCodecError> {
    encode_relational_journal_entry(entry, RelationalJournalCodecLimits::default())
}

/// Coordinator-facing incremental decoder. The expected cursor is supplied
/// by the already replayed prefix; neither serialized sequence nor serialized
/// chain anchors are accepted as authority.
pub(crate) fn decode_entry(
    contract: RelationalJournalContract,
    expected_sequence: u64,
    expected_previous: RelationalJournalHead,
    bytes: &[u8],
) -> Result<RelationalJournalEntry, RelationalJournalCodecError> {
    decode_relational_journal_entry(
        contract,
        expected_sequence,
        expected_previous,
        bytes,
        RelationalJournalCodecLimits::default(),
    )
}

/// Encode one already validated semantic entry. At peak this retains the
/// bounded event payload plus the bounded final entry buffer.
pub(crate) fn encode_relational_journal_entry(
    entry: &RelationalJournalEntry,
    limits: RelationalJournalCodecLimits,
) -> Result<Vec<u8>, RelationalJournalCodecError> {
    let mut payload = Encoder::new(limits);
    encode_journal_event(&mut payload, entry.event())?;
    let payload = payload.finish();

    let final_len = ENTRY_FIXED_BYTES
        .checked_add(payload.len())
        .ok_or(RelationalJournalCodecError::LengthOverflow)?;
    if final_len > limits.max_entry_bytes {
        return Err(RelationalJournalCodecError::EntryTooLarge {
            bytes: final_len,
            limit: limits.max_entry_bytes,
        });
    }
    let mut encoded = Encoder::new(limits);
    encoded.raw(ENTRY_MAGIC)?;
    encoded.u32(RELATIONAL_JOURNAL_CODEC_SCHEMA_VERSION)?;
    encoded.u32(RELATIONAL_JOURNAL_SCHEMA_VERSION)?;
    encoded.u64(entry.sequence())?;
    encoded.digest(entry.previous().bytes())?;
    encoded.digest(entry.head().bytes())?;
    encoded.len(payload.len())?;
    encoded.raw(&payload)?;
    Ok(encoded.finish())
}

/// Decode one entry against the caller's expected journal cursor. The result
/// is re-encoded and compared byte-for-byte, rejecting any alternative wire
/// representation that happens to decode to the same semantic value.
pub(crate) fn decode_relational_journal_entry(
    contract: RelationalJournalContract,
    expected_sequence: u64,
    expected_previous: RelationalJournalHead,
    bytes: &[u8],
    limits: RelationalJournalCodecLimits,
) -> Result<RelationalJournalEntry, RelationalJournalCodecError> {
    if bytes.len() > limits.max_entry_bytes {
        return Err(RelationalJournalCodecError::EntryTooLarge {
            bytes: bytes.len(),
            limit: limits.max_entry_bytes,
        });
    }
    let mut reader = Reader::new(bytes, limits);
    if reader.take(ENTRY_MAGIC.len())? != ENTRY_MAGIC {
        return Err(RelationalJournalCodecError::Malformed("wrong entry magic"));
    }
    let codec_schema = reader.u32()?;
    if codec_schema != RELATIONAL_JOURNAL_CODEC_SCHEMA_VERSION {
        return Err(RelationalJournalCodecError::UnsupportedCodecSchema {
            actual: codec_schema,
            expected: RELATIONAL_JOURNAL_CODEC_SCHEMA_VERSION,
        });
    }
    let journal_schema = reader.u32()?;
    if journal_schema != RELATIONAL_JOURNAL_SCHEMA_VERSION {
        return Err(RelationalJournalCodecError::UnsupportedJournalSchema {
            actual: journal_schema,
            expected: RELATIONAL_JOURNAL_SCHEMA_VERSION,
        });
    }
    let sequence = reader.u64()?;
    let previous = reader.digest()?;
    let claimed_head = reader.digest()?;
    let payload_len = reader.bounded_len(limits.max_entry_bytes, "event payload")?;
    let payload_bytes = reader.take(payload_len)?;
    reader.finish()?;

    let mut payload = Reader::new(payload_bytes, limits);
    let event = decode_journal_event(&mut payload, contract)?;
    payload.finish()?;
    let entry = RelationalJournalEntry::restore_from_journal_codec(
        contract,
        expected_sequence,
        expected_previous,
        sequence,
        previous,
        event,
        claimed_head,
    )?;
    let canonical = encode_relational_journal_entry(&entry, limits)?;
    if canonical != bytes {
        return Err(RelationalJournalCodecError::NonCanonicalEncoding);
    }
    Ok(entry)
}

struct Encoder {
    bytes: Vec<u8>,
    limits: RelationalJournalCodecLimits,
    value_nodes: usize,
}

impl Encoder {
    fn new(limits: RelationalJournalCodecLimits) -> Self {
        Self {
            bytes: Vec::new(),
            limits,
            value_nodes: 0,
        }
    }

    fn reserve(&mut self, additional: usize) -> Result<(), RelationalJournalCodecError> {
        let requested = self
            .bytes
            .len()
            .checked_add(additional)
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        if requested > self.limits.max_entry_bytes {
            return Err(RelationalJournalCodecError::EntryTooLarge {
                bytes: requested,
                limit: self.limits.max_entry_bytes,
            });
        }
        self.bytes
            .try_reserve_exact(additional)
            .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested })
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), RelationalJournalCodecError> {
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn tag(&mut self, value: u8) -> Result<(), RelationalJournalCodecError> {
        self.raw(&[value])
    }

    fn bool(&mut self, value: bool) -> Result<(), RelationalJournalCodecError> {
        self.tag(if value { 0x01 } else { 0x00 })
    }

    fn u32(&mut self, value: u32) -> Result<(), RelationalJournalCodecError> {
        self.raw(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), RelationalJournalCodecError> {
        self.raw(&value.to_be_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), RelationalJournalCodecError> {
        self.raw(&value.to_be_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<(), RelationalJournalCodecError> {
        self.raw(&value.to_be_bytes())
    }

    fn digest(&mut self, value: [u8; 32]) -> Result<(), RelationalJournalCodecError> {
        self.raw(&value)
    }

    fn len(&mut self, value: usize) -> Result<(), RelationalJournalCodecError> {
        self.u64(u64::try_from(value).map_err(|_| RelationalJournalCodecError::LengthOverflow)?)
    }

    fn usize(&mut self, value: usize) -> Result<(), RelationalJournalCodecError> {
        self.len(value)
    }

    fn collection_len(&mut self, value: usize) -> Result<(), RelationalJournalCodecError> {
        if value > self.limits.max_collection_items {
            return Err(RelationalJournalCodecError::CollectionTooLarge {
                items: value,
                limit: self.limits.max_collection_items,
            });
        }
        self.len(value)
    }

    fn blob(&mut self, value: &[u8]) -> Result<(), RelationalJournalCodecError> {
        if value.len() > self.limits.max_blob_bytes {
            return Err(RelationalJournalCodecError::BlobTooLarge {
                bytes: value.len(),
                limit: self.limits.max_blob_bytes,
            });
        }
        self.len(value.len())?;
        self.raw(value)
    }

    fn string(&mut self, value: &str) -> Result<(), RelationalJournalCodecError> {
        if value.len() > self.limits.max_string_bytes {
            return Err(RelationalJournalCodecError::StringTooLarge {
                bytes: value.len(),
                limit: self.limits.max_string_bytes,
            });
        }
        self.len(value.len())?;
        self.raw(value.as_bytes())
    }

    fn value_node(&mut self, depth: usize) -> Result<(), RelationalJournalCodecError> {
        if depth > self.limits.max_value_depth {
            return Err(RelationalJournalCodecError::ValueDepthExceeded {
                depth,
                limit: self.limits.max_value_depth,
            });
        }
        self.value_nodes = self
            .value_nodes
            .checked_add(1)
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        if self.value_nodes > self.limits.max_value_nodes {
            return Err(RelationalJournalCodecError::ValueNodeLimitExceeded {
                limit: self.limits.max_value_nodes,
            });
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    limits: RelationalJournalCodecLimits,
    value_nodes: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8], limits: RelationalJournalCodecLimits) -> Self {
        Self {
            bytes,
            position: 0,
            limits,
            value_nodes: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RelationalJournalCodecError> {
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(RelationalJournalCodecError::Truncated)?;
        let value = &self.bytes[self.position..end];
        self.position = end;
        Ok(value)
    }

    fn tag(&mut self) -> Result<u8, RelationalJournalCodecError> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, RelationalJournalCodecError> {
        match self.tag()? {
            0x00 => Ok(false),
            0x01 => Ok(true),
            tag => Err(RelationalJournalCodecError::UnknownTag {
                component: "boolean",
                tag,
            }),
        }
    }

    fn u32(&mut self) -> Result<u32, RelationalJournalCodecError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().expect("fixed u32 bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, RelationalJournalCodecError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("fixed u64 bytes"),
        ))
    }

    fn u128(&mut self) -> Result<u128, RelationalJournalCodecError> {
        Ok(u128::from_be_bytes(
            self.take(16)?.try_into().expect("fixed u128 bytes"),
        ))
    }

    fn i64(&mut self) -> Result<i64, RelationalJournalCodecError> {
        Ok(i64::from_be_bytes(
            self.take(8)?.try_into().expect("fixed i64 bytes"),
        ))
    }

    fn digest(&mut self) -> Result<[u8; 32], RelationalJournalCodecError> {
        Ok(self.take(32)?.try_into().expect("fixed digest bytes"))
    }

    fn bounded_len(
        &mut self,
        limit: usize,
        component: &'static str,
    ) -> Result<usize, RelationalJournalCodecError> {
        let claimed = self.u64()?;
        let value = usize::try_from(claimed)
            .map_err(|_| RelationalJournalCodecError::LengthNotRepresentable { component })?;
        if value > limit {
            return Err(RelationalJournalCodecError::DeclaredLengthTooLarge {
                component,
                claimed,
                limit,
            });
        }
        Ok(value)
    }

    fn usize(&mut self, component: &'static str) -> Result<usize, RelationalJournalCodecError> {
        usize::try_from(self.u64()?)
            .map_err(|_| RelationalJournalCodecError::LengthNotRepresentable { component })
    }

    fn collection_len(
        &mut self,
        component: &'static str,
    ) -> Result<usize, RelationalJournalCodecError> {
        self.bounded_len(self.limits.max_collection_items, component)
    }

    fn blob(&mut self) -> Result<Box<[u8]>, RelationalJournalCodecError> {
        let length = self.bounded_len(self.limits.max_blob_bytes, "blob")?;
        let source = self.take(length)?;
        let mut value = Vec::new();
        value
            .try_reserve_exact(length)
            .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: length })?;
        value.extend_from_slice(source);
        Ok(value.into_boxed_slice())
    }

    fn string(&mut self) -> Result<Box<str>, RelationalJournalCodecError> {
        let length = self.bounded_len(self.limits.max_string_bytes, "string")?;
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes).map_err(|_| RelationalJournalCodecError::Utf8)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(length)
            .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: length })?;
        owned.push_str(value);
        Ok(owned.into_boxed_str())
    }

    fn value_node(&mut self, depth: usize) -> Result<(), RelationalJournalCodecError> {
        if depth > self.limits.max_value_depth {
            return Err(RelationalJournalCodecError::ValueDepthExceeded {
                depth,
                limit: self.limits.max_value_depth,
            });
        }
        self.value_nodes = self
            .value_nodes
            .checked_add(1)
            .ok_or(RelationalJournalCodecError::LengthOverflow)?;
        if self.value_nodes > self.limits.max_value_nodes {
            return Err(RelationalJournalCodecError::ValueNodeLimitExceeded {
                limit: self.limits.max_value_nodes,
            });
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), RelationalJournalCodecError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(RelationalJournalCodecError::TrailingBytes {
                bytes: self.bytes.len() - self.position,
            })
        }
    }
}

fn encode_explore_value(
    encoder: &mut Encoder,
    value: &ExploreValue,
    depth: usize,
) -> Result<(), RelationalJournalCodecError> {
    encoder.value_node(depth)?;
    match value {
        ExploreValue::Int(value) => {
            encoder.tag(0x01)?;
            encoder.i64(*value)
        }
        ExploreValue::FloatBits(bits) => {
            encoder.tag(0x02)?;
            encoder.u64(*bits)
        }
        ExploreValue::String(value) => {
            encoder.tag(0x03)?;
            encoder.string(value)
        }
        ExploreValue::Character(value) => {
            encoder.tag(0x04)?;
            encoder.u32(u32::from(*value))
        }
        ExploreValue::Boolean(value) => {
            encoder.tag(0x05)?;
            encoder.bool(*value)
        }
        ExploreValue::Unit => encoder.tag(0x06),
        ExploreValue::List(values) => {
            encoder.tag(0x07)?;
            encode_explore_values(encoder, values, depth)
        }
        ExploreValue::Set(values) => {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RelationalJournalCodecError::NonCanonicalValue(
                    "set values must be strictly ordered",
                ));
            }
            encoder.tag(0x08)?;
            encode_explore_values(encoder, values, depth)
        }
        ExploreValue::Tuple(values) => {
            encoder.tag(0x09)?;
            encode_explore_values(encoder, values, depth)
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            encoder.tag(0x0a)?;
            encoder.string(type_name)?;
            encoder.string(variant)?;
            encoder.bool(*positional)?;
            encoder.collection_len(fields.len())?;
            for (name, value) in fields.iter() {
                encoder.string(name)?;
                encode_explore_value(encoder, value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn encode_explore_values(
    encoder: &mut Encoder,
    values: &[ExploreValue],
    depth: usize,
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(values.len())?;
    for value in values {
        encode_explore_value(encoder, value, depth + 1)?;
    }
    Ok(())
}

fn decode_explore_value(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ExploreValue, RelationalJournalCodecError> {
    reader.value_node(depth)?;
    match reader.tag()? {
        0x01 => Ok(ExploreValue::Int(reader.i64()?)),
        0x02 => Ok(ExploreValue::FloatBits(reader.u64()?)),
        0x03 => Ok(ExploreValue::String(reader.string()?.into_string())),
        0x04 => {
            let scalar = reader.u32()?;
            char::from_u32(scalar)
                .map(ExploreValue::Character)
                .ok_or(RelationalJournalCodecError::InvalidCharacter(scalar))
        }
        0x05 => Ok(ExploreValue::Boolean(reader.bool()?)),
        0x06 => Ok(ExploreValue::Unit),
        0x07 => Ok(ExploreValue::List(decode_explore_values(reader, depth)?)),
        0x08 => {
            let values = decode_explore_values(reader, depth)?;
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RelationalJournalCodecError::NonCanonicalValue(
                    "set values must be strictly ordered",
                ));
            }
            Ok(ExploreValue::Set(values))
        }
        0x09 => Ok(ExploreValue::Tuple(decode_explore_values(reader, depth)?)),
        0x0a => {
            let type_name = reader.string()?.into_string();
            let variant = reader.string()?.into_string();
            let positional = reader.bool()?;
            let count = reader.collection_len("constructor fields")?;
            let mut fields = Vec::new();
            fields
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                fields.push((
                    reader.string()?.into_string(),
                    decode_explore_value(reader, depth + 1)?,
                ));
            }
            Ok(ExploreValue::Constructor {
                type_name,
                variant,
                positional,
                fields: fields.into(),
            })
        }
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "ExploreValue",
            tag,
        }),
    }
}

fn decode_explore_values(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<Vec<ExploreValue>, RelationalJournalCodecError> {
    let count = reader.collection_len("ExploreValue collection")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(decode_explore_value(reader, depth + 1)?);
    }
    Ok(values)
}

fn encode_provenance(
    encoder: &mut Encoder,
    provenance: &RelationProvenance,
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(provenance.lineage().len())?;
    for id in provenance.lineage() {
        encoder.digest(id.bytes())?;
    }
    encoder.collection_len(provenance.support().len())?;
    for id in provenance.support() {
        encoder.digest(id.bytes())?;
    }
    Ok(())
}

fn decode_provenance(
    reader: &mut Reader<'_>,
) -> Result<RelationProvenance, RelationalJournalCodecError> {
    let lineage_count = reader.collection_len("relation lineage")?;
    let mut lineage = Vec::new();
    lineage.try_reserve_exact(lineage_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: lineage_count,
        }
    })?;
    for _ in 0..lineage_count {
        lineage.push(RelationLineageId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    let support_count = reader.collection_len("relation support")?;
    let mut support = Vec::new();
    support.try_reserve_exact(support_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: support_count,
        }
    })?;
    for _ in 0..support_count {
        support.push(RelationSupportId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    Ok(RelationProvenance::new(lineage, support))
}

fn encode_source_row(
    encoder: &mut Encoder,
    row: &SourceRow,
) -> Result<(), RelationalJournalCodecError> {
    encode_explore_value(encoder, row.context(), 0)?;
    encode_explore_value(encoder, row.before(), 0)?;
    encode_provenance(encoder, row.provenance())
}

fn decode_source_row(reader: &mut Reader<'_>) -> Result<SourceRow, RelationalJournalCodecError> {
    Ok(SourceRow::new(
        decode_explore_value(reader, 0)?,
        decode_explore_value(reader, 0)?,
        decode_provenance(reader)?,
    ))
}

fn encode_successor_row(
    encoder: &mut Encoder,
    row: &SuccessorRow,
) -> Result<(), RelationalJournalCodecError> {
    encode_explore_value(encoder, row.after(), 0)?;
    encode_provenance(encoder, row.provenance())
}

fn decode_successor_row(
    reader: &mut Reader<'_>,
) -> Result<SuccessorRow, RelationalJournalCodecError> {
    Ok(SuccessorRow::new(
        decode_explore_value(reader, 0)?,
        decode_provenance(reader)?,
    ))
}

fn encode_admission_decision(
    encoder: &mut Encoder,
    decision: AdmissionDecision,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match decision {
        AdmissionDecision::Rejected => 0x01,
        AdmissionDecision::Admitted => 0x02,
    })
}

fn decode_admission_decision(
    reader: &mut Reader<'_>,
) -> Result<AdmissionDecision, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(AdmissionDecision::Rejected),
        0x02 => Ok(AdmissionDecision::Admitted),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "admission decision",
            tag,
        }),
    }
}

fn encode_selection_decision(
    encoder: &mut Encoder,
    decision: SelectionDecision,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match decision {
        SelectionDecision::NotSelected => 0x01,
        SelectionDecision::Selected => 0x02,
    })
}

fn decode_selection_decision(
    reader: &mut Reader<'_>,
) -> Result<SelectionDecision, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(SelectionDecision::NotSelected),
        0x02 => Ok(SelectionDecision::Selected),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "selection decision",
            tag,
        }),
    }
}

fn encode_support_expr(
    encoder: &mut Encoder,
    expression: &SupportExpr,
    depth: usize,
) -> Result<(), RelationalJournalCodecError> {
    encoder.value_node(depth)?;
    match expression.kind() {
        SupportExprKind::Singleton(value) => {
            encoder.tag(0x01)?;
            encode_explore_value(encoder, value, depth + 1)
        }
        SupportExprKind::FiniteEnum(values) => {
            encoder.tag(0x02)?;
            encoder.collection_len(values.len())?;
            for value in values {
                encode_explore_value(encoder, value, depth + 1)?;
            }
            Ok(())
        }
        SupportExprKind::OrdinalInterval {
            start,
            end_exclusive,
        } => {
            encoder.tag(0x03)?;
            encoder.u128(*start)?;
            encoder.u128(*end_exclusive)
        }
        SupportExprKind::OrdinalCongruence {
            start,
            end_exclusive,
            modulus,
            residue,
        } => {
            encoder.tag(0x04)?;
            encoder.u128(*start)?;
            encoder.u128(*end_exclusive)?;
            encoder.u128(modulus.get())?;
            encoder.u128(*residue)
        }
        SupportExprKind::Product(factors) => {
            encoder.tag(0x05)?;
            encoder.collection_len(factors.len())?;
            for factor in factors {
                encode_support_expr(encoder, factor, depth + 1)?;
            }
            Ok(())
        }
        SupportExprKind::JoinReference {
            producer_id,
            inputs,
        } => {
            encoder.tag(0x06)?;
            encoder.digest(producer_id.bytes())?;
            encoder.collection_len(inputs.len())?;
            for input in inputs {
                encoder.digest(input.bytes())?;
            }
            Ok(())
        }
        SupportExprKind::Union(operands) => {
            encoder.tag(0x07)?;
            encoder.collection_len(operands.len())?;
            for operand in operands {
                encode_support_expr(encoder, operand, depth + 1)?;
            }
            Ok(())
        }
        SupportExprKind::Difference {
            minuend,
            subtrahend,
        } => {
            encoder.tag(0x08)?;
            encode_support_expr(encoder, minuend, depth + 1)?;
            encode_support_expr(encoder, subtrahend, depth + 1)
        }
        SupportExprKind::ProductRankInterval {
            factors,
            rank_start,
            rank_end_exclusive,
        } => {
            encoder.tag(0x09)?;
            encoder.collection_len(factors.len())?;
            for factor in factors {
                encode_support_expr(encoder, factor, depth + 1)?;
            }
            encoder.u128(*rank_start)?;
            encoder.u128(*rank_end_exclusive)
        }
    }
}

fn decode_support_expr(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<SupportExpr, RelationalJournalCodecError> {
    reader.value_node(depth)?;
    match reader.tag()? {
        0x01 => Ok(SupportExpr::singleton(decode_explore_value(
            reader,
            depth + 1,
        )?)),
        0x02 => {
            let count = reader.collection_len("finite support enumeration")?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                values.push(decode_explore_value(reader, depth + 1)?);
            }
            Ok(SupportExpr::finite_enum(values)?)
        }
        0x03 => Ok(SupportExpr::ordinal_interval(
            reader.u128()?,
            reader.u128()?,
        )?),
        0x04 => {
            let start = reader.u128()?;
            let end_exclusive = reader.u128()?;
            let modulus = NonZeroU128::new(reader.u128()?).ok_or(
                RelationalJournalCodecError::Malformed("zero congruence modulus"),
            )?;
            let residue = reader.u128()?;
            Ok(SupportExpr::ordinal_congruence(
                start,
                end_exclusive,
                modulus,
                residue,
            )?)
        }
        0x05 => {
            let count = reader.collection_len("support product")?;
            let mut factors = Vec::new();
            factors
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                factors.push(decode_support_expr(reader, depth + 1)?);
            }
            Ok(SupportExpr::product(factors)?)
        }
        0x06 => {
            let producer_id = SupportProducerId::from_journal_codec_bytes(reader.digest()?);
            let count = reader.collection_len("support join inputs")?;
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                inputs.push(SupportCellId::from_journal_codec_bytes(reader.digest()?));
            }
            Ok(SupportExpr::join_reference(producer_id, inputs))
        }
        0x07 => {
            let count = reader.collection_len("support union")?;
            let mut operands = Vec::new();
            operands
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                operands.push(decode_support_expr(reader, depth + 1)?);
            }
            Ok(SupportExpr::union(operands)?)
        }
        0x08 => Ok(SupportExpr::difference(
            decode_support_expr(reader, depth + 1)?,
            decode_support_expr(reader, depth + 1)?,
        )?),
        0x09 => {
            let count = reader.collection_len("ranked support product")?;
            let mut factors = Vec::new();
            factors
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                factors.push(decode_support_expr(reader, depth + 1)?);
            }
            Ok(SupportExpr::product_rank_interval(
                factors,
                reader.u128()?,
                reader.u128()?,
            )?)
        }
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support expression",
            tag,
        }),
    }
}

fn encode_support_target(
    encoder: &mut Encoder,
    target: SupportExtensionalTarget,
) -> Result<(), RelationalJournalCodecError> {
    match target {
        SupportExtensionalTarget::SourceRows(relation_id) => {
            encoder.tag(0x01)?;
            encoder.digest(relation_id.bytes())
        }
        SupportExtensionalTarget::SuccessorRows(relation_id) => {
            encoder.tag(0x02)?;
            encoder.digest(relation_id.bytes())
        }
        SupportExtensionalTarget::Cases(relation_id) => {
            encoder.tag(0x03)?;
            encoder.digest(relation_id.bytes())
        }
        SupportExtensionalTarget::Derived(producer_id) => {
            encoder.tag(0x04)?;
            encoder.digest(producer_id.bytes())
        }
    }
}

fn decode_support_target(
    reader: &mut Reader<'_>,
) -> Result<SupportExtensionalTarget, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(SupportExtensionalTarget::SourceRows(
            RelationId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x02 => Ok(SupportExtensionalTarget::SuccessorRows(
            RelationId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x03 => Ok(SupportExtensionalTarget::Cases(
            RelationId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x04 => Ok(SupportExtensionalTarget::Derived(
            SupportProducerId::from_journal_codec_bytes(reader.digest()?),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support extensional target",
            tag,
        }),
    }
}

fn encode_support_cell_space(
    encoder: &mut Encoder,
    space: SupportCellSpace,
) -> Result<(), RelationalJournalCodecError> {
    match space {
        SupportCellSpace::ProducerCoordinates(producer_id) => {
            encoder.tag(0x01)?;
            encoder.digest(producer_id.bytes())
        }
        SupportCellSpace::ExtensionalValues(target) => {
            encoder.tag(0x02)?;
            encode_support_target(encoder, target)
        }
        SupportCellSpace::MappedImage {
            producer_id,
            target,
        } => {
            encoder.tag(0x03)?;
            encoder.digest(producer_id.bytes())?;
            encode_support_target(encoder, target)
        }
    }
}

fn decode_support_cell_space(
    reader: &mut Reader<'_>,
) -> Result<SupportCellSpace, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(SupportCellSpace::ProducerCoordinates(
            SupportProducerId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x02 => Ok(SupportCellSpace::ExtensionalValues(decode_support_target(
            reader,
        )?)),
        0x03 => Ok(SupportCellSpace::MappedImage {
            producer_id: SupportProducerId::from_journal_codec_bytes(reader.digest()?),
            target: decode_support_target(reader)?,
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support cell space",
            tag,
        }),
    }
}

fn encode_support_cell(
    encoder: &mut Encoder,
    cell: &SupportCell,
) -> Result<(), RelationalJournalCodecError> {
    encode_support_cell_space(encoder, cell.space())?;
    encode_support_expr(encoder, cell.expression(), 0)?;
    encoder.digest(cell.materializer_id().bytes())
}

fn decode_support_cell(
    reader: &mut Reader<'_>,
) -> Result<SupportCell, RelationalJournalCodecError> {
    Ok(SupportCell::new(
        decode_support_cell_space(reader)?,
        decode_support_expr(reader, 0)?,
        SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
    )?)
}

fn encode_support_obligation(
    encoder: &mut Encoder,
    obligation: &SupportObligationRecord,
) -> Result<(), RelationalJournalCodecError> {
    match obligation {
        SupportObligationRecord::Cardinality(obligation) => {
            encoder.tag(0x01)?;
            encoder.digest(obligation.cell_id().bytes())
        }
        SupportObligationRecord::Injectivity(obligation) => {
            encoder.tag(0x02)?;
            encoder.digest(obligation.cell_id().bytes())?;
            encoder.digest(obligation.claim().materializer_id().bytes())
        }
        SupportObligationRecord::Admission(obligation) => {
            encoder.tag(0x03)?;
            encoder.digest(obligation.cell_id().bytes())?;
            encoder.digest(obligation.claim().admission_id().bytes())
        }
        SupportObligationRecord::Selection(obligation) => {
            encoder.tag(0x04)?;
            encoder.digest(obligation.cell_id().bytes())?;
            encoder.digest(obligation.claim().question_id().bytes())
        }
        SupportObligationRecord::UniformValue(obligation) => {
            encoder.tag(0x05)?;
            encoder.digest(obligation.cell_id().bytes())?;
            encoder.digest(obligation.claim().observer_id().bytes())?;
            encoder.digest(obligation.claim().value_schema_digest())
        }
        SupportObligationRecord::UniformMechanism(obligation) => {
            encoder.tag(0x06)?;
            encoder.digest(obligation.cell_id().bytes())?;
            encoder.digest(obligation.claim().request_id().bytes())
        }
    }
}

fn decode_support_obligation(
    reader: &mut Reader<'_>,
) -> Result<SupportObligationRecord, RelationalJournalCodecError> {
    let tag = reader.tag()?;
    let cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    match tag {
        0x01 => Ok(SupportObligationRecord::Cardinality(
            SupportCellObligation::restore_from_journal_codec(cell_id, ExactCardinalityClaim),
        )),
        0x02 => Ok(SupportObligationRecord::Injectivity(
            SupportCellObligation::restore_from_journal_codec(
                cell_id,
                InjectiveMappingClaim::new(SupportMaterializerId::from_journal_codec_bytes(
                    reader.digest()?,
                )),
            ),
        )),
        0x03 => Ok(SupportObligationRecord::Admission(
            SupportCellObligation::restore_from_journal_codec(
                cell_id,
                AdmissionClassificationClaim::new(AdmissionId::from_journal_codec_bytes(
                    reader.digest()?,
                )),
            ),
        )),
        0x04 => Ok(SupportObligationRecord::Selection(
            SupportCellObligation::restore_from_journal_codec(
                cell_id,
                SelectionClassificationClaim::new(QuestionId::from_journal_codec_bytes(
                    reader.digest()?,
                )),
            ),
        )),
        0x05 => Ok(SupportObligationRecord::UniformValue(
            SupportCellObligation::restore_from_journal_codec(
                cell_id,
                UniformValueClaim::new(
                    SupportObserverId::from_journal_codec_bytes(reader.digest()?),
                    reader.digest()?,
                ),
            ),
        )),
        0x06 => Ok(SupportObligationRecord::UniformMechanism(
            SupportCellObligation::restore_from_journal_codec(
                cell_id,
                UniformMechanismClaim::new(MechanismRequestId::from_journal_codec_bytes(
                    reader.digest()?,
                )),
            ),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support obligation",
            tag,
        }),
    }
}

fn encode_obligation_refinement(
    encoder: &mut Encoder,
    refinement: &SupportObligationRefinement,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(refinement.parent_obligation_id().bytes())?;
    encoder.digest(refinement.partition_id().bytes())?;
    encoder.collection_len(refinement.child_obligation_ids().len())?;
    for child in refinement.child_obligation_ids() {
        encoder.digest(child.bytes())?;
    }
    Ok(())
}

fn decode_obligation_refinement(
    reader: &mut Reader<'_>,
) -> Result<SupportObligationRefinement, RelationalJournalCodecError> {
    let parent = SupportProofObligationId::from_journal_codec_bytes(reader.digest()?);
    let partition = SupportPartitionId::from_journal_codec_bytes(reader.digest()?);
    let count = reader.collection_len("obligation refinement children")?;
    let mut children = Vec::new();
    children
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        children.push(SupportProofObligationId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    Ok(SupportObligationRefinement::restore_from_journal_codec(
        parent,
        partition,
        children.into_boxed_slice(),
    )?)
}

fn encode_analysis_dependency(
    encoder: &mut Encoder,
    dependency: RelationalAnalysisDependencyId,
) -> Result<(), RelationalJournalCodecError> {
    match dependency {
        RelationalAnalysisDependencyId::Relation(id) => {
            encoder.tag(0x04)?;
            encoder.digest(id.bytes())
        }
        RelationalAnalysisDependencyId::Question(id) => {
            encoder.tag(0x01)?;
            encoder.digest(id.bytes())
        }
        RelationalAnalysisDependencyId::Result(id) => {
            encoder.tag(0x02)?;
            encoder.digest(id.bytes())
        }
        RelationalAnalysisDependencyId::Mechanisms(id) => {
            encoder.tag(0x03)?;
            encoder.digest(id.bytes())
        }
    }
}

fn decode_analysis_dependency(
    reader: &mut Reader<'_>,
) -> Result<RelationalAnalysisDependencyId, RelationalJournalCodecError> {
    match reader.tag()? {
        0x04 => Ok(RelationalAnalysisDependencyId::Relation(
            RelationId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x01 => Ok(RelationalAnalysisDependencyId::Question(
            QuestionId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x02 => Ok(RelationalAnalysisDependencyId::Result(
            ViewId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x03 => Ok(RelationalAnalysisDependencyId::Mechanisms(
            MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "analysis dependency",
            tag,
        }),
    }
}

fn encode_analysis_dependencies(
    encoder: &mut Encoder,
    dependencies: &[RelationalAnalysisDependencyId],
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(dependencies.len())?;
    for dependency in dependencies {
        encode_analysis_dependency(encoder, *dependency)?;
    }
    Ok(())
}

fn decode_analysis_dependencies(
    reader: &mut Reader<'_>,
) -> Result<Box<[RelationalAnalysisDependencyId]>, RelationalJournalCodecError> {
    let count = reader.collection_len("analysis dependencies")?;
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        dependencies.push(decode_analysis_dependency(reader)?);
    }
    Ok(dependencies.into_boxed_slice())
}

fn encode_analysis_plan(
    encoder: &mut Encoder,
    plan: &RelationalAnalysisPlan,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(plan.question_id().bytes())?;
    encoder.digest(plan.producer_graph_digest().bytes())?;
    encoder.collection_len(plan.layer_registrations().len())?;
    for registration in plan.layer_registrations() {
        match registration {
            RelationalAnalysisLayerRegistration::Result(result) => {
                encoder.tag(0x01)?;
                encoder.digest(result.view_id().bytes())?;
                match result.input() {
                    RelationalResolvedResultInput::Sources(relation_id) => {
                        encoder.tag(0x03)?;
                        encoder.digest(relation_id.bytes())?;
                    }
                    RelationalResolvedResultInput::Selected(question_id) => {
                        encoder.tag(0x01)?;
                        encoder.digest(question_id.bytes())?;
                    }
                    RelationalResolvedResultInput::MechanismIncidence(request_id) => {
                        encoder.tag(0x02)?;
                        encoder.digest(request_id.bytes())?;
                    }
                }
                encoder.digest(result.semantic_spec_digest().bytes())?;
                encode_analysis_dependencies(encoder, result.dependencies())?;
            }
            RelationalAnalysisLayerRegistration::Mechanisms(mechanism) => {
                encoder.tag(0x02)?;
                encoder.digest(mechanism.request_id().bytes())?;
                match mechanism.target() {
                    RelationalResolvedMechanismTarget::Selected(question_id) => {
                        encoder.tag(0x01)?;
                        encoder.digest(question_id.bytes())?;
                    }
                    RelationalResolvedMechanismTarget::ChosenView(view_id) => {
                        encoder.tag(0x02)?;
                        encoder.digest(view_id.bytes())?;
                    }
                }
                encoder.digest(mechanism.observation_id().bytes())?;
                encoder.digest(mechanism.observation_digest().bytes())?;
                encode_analysis_dependencies(encoder, mechanism.dependencies())?;
            }
        }
    }
    Ok(())
}

fn decode_analysis_plan(
    reader: &mut Reader<'_>,
) -> Result<RelationalAnalysisPlan, RelationalJournalCodecError> {
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let graph_digest =
        RelationalCheckedAnalysisGraphDigest::from_journal_codec_bytes(reader.digest()?);
    let count = reader.collection_len("analysis registrations")?;
    let mut registrations = Vec::new();
    registrations
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        let registration = match reader.tag()? {
            0x01 => {
                let view_id = ViewId::from_journal_codec_bytes(reader.digest()?);
                let input = match reader.tag()? {
                    0x03 => RelationalResolvedResultInput::Sources(
                        RelationId::from_journal_codec_bytes(reader.digest()?),
                    ),
                    0x01 => RelationalResolvedResultInput::Selected(
                        QuestionId::from_journal_codec_bytes(reader.digest()?),
                    ),
                    0x02 => RelationalResolvedResultInput::MechanismIncidence(
                        MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
                    ),
                    tag => {
                        return Err(RelationalJournalCodecError::UnknownTag {
                            component: "resolved result input",
                            tag,
                        });
                    }
                };
                let spec_digest =
                    RelationalResultSpecDigest::from_journal_codec_bytes(reader.digest()?);
                let dependencies = decode_analysis_dependencies(reader)?;
                RelationalAnalysisLayerRegistration::Result(
                    RelationalResultLayerRegistration::restore_from_journal_codec(
                        view_id,
                        input,
                        spec_digest,
                        dependencies,
                    ),
                )
            }
            0x02 => {
                let request_id = MechanismRequestId::from_journal_codec_bytes(reader.digest()?);
                let target = match reader.tag()? {
                    0x01 => RelationalResolvedMechanismTarget::Selected(
                        QuestionId::from_journal_codec_bytes(reader.digest()?),
                    ),
                    0x02 => RelationalResolvedMechanismTarget::ChosenView(
                        ViewId::from_journal_codec_bytes(reader.digest()?),
                    ),
                    tag => {
                        return Err(RelationalJournalCodecError::UnknownTag {
                            component: "resolved mechanism target",
                            tag,
                        });
                    }
                };
                let observation_id =
                    RelationalMechanismObservationId::from_journal_codec_bytes(reader.digest()?);
                let observation_digest =
                    RelationalMechanismObservationDigest::from_journal_codec_bytes(
                        reader.digest()?,
                    );
                let dependencies = decode_analysis_dependencies(reader)?;
                RelationalAnalysisLayerRegistration::Mechanisms(
                    RelationalMechanismLayerRegistration::restore_from_journal_codec(
                        request_id,
                        target,
                        observation_id,
                        observation_digest,
                        dependencies,
                    ),
                )
            }
            tag => {
                return Err(RelationalJournalCodecError::UnknownTag {
                    component: "analysis registration",
                    tag,
                });
            }
        };
        registrations.push(registration);
    }
    Ok(RelationalAnalysisPlan::restore_from_journal_codec(
        question_id,
        graph_digest,
        registrations,
    )?)
}

fn encode_binding_role(
    encoder: &mut Encoder,
    role: ExploreSourceBindingRoleIr,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match role {
        ExploreSourceBindingRoleIr::Auxiliary => 0x01,
        ExploreSourceBindingRoleIr::Context => 0x02,
        ExploreSourceBindingRoleIr::Before => 0x03,
    })
}

fn decode_binding_role(
    reader: &mut Reader<'_>,
) -> Result<ExploreSourceBindingRoleIr, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ExploreSourceBindingRoleIr::Auxiliary),
        0x02 => Ok(ExploreSourceBindingRoleIr::Context),
        0x03 => Ok(ExploreSourceBindingRoleIr::Before),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "source binding role",
            tag,
        }),
    }
}

fn encode_dimension_ids(
    encoder: &mut Encoder,
    dimensions: &[RelationalDimensionId],
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(dimensions.len())?;
    for dimension in dimensions {
        encoder.digest(dimension.bytes())?;
    }
    Ok(())
}

fn decode_dimension_ids(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[RelationalDimensionId]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut dimensions = Vec::new();
    dimensions
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        dimensions.push(RelationalDimensionId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    Ok(dimensions.into_boxed_slice())
}

fn encode_dependency_key(
    encoder: &mut Encoder,
    recipe: &RelationalDependencyKeyRecipe,
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(recipe.binding_indices().len())?;
    for index in recipe.binding_indices() {
        encoder.u32(*index)?;
    }
    encoder.collection_len(recipe.binding_stage_ids().len())?;
    for stage_id in recipe.binding_stage_ids() {
        encoder.digest(stage_id.bytes())?;
    }
    Ok(())
}

fn decode_dependency_key(
    reader: &mut Reader<'_>,
) -> Result<RelationalDependencyKeyRecipe, RelationalJournalCodecError> {
    let index_count = reader.collection_len("dependency binding indices")?;
    let mut indices = Vec::new();
    indices.try_reserve_exact(index_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: index_count,
        }
    })?;
    for _ in 0..index_count {
        indices.push(reader.u32()?);
    }
    let stage_count = reader.collection_len("dependency stage IDs")?;
    let mut stage_ids = Vec::new();
    stage_ids.try_reserve_exact(stage_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: stage_count,
        }
    })?;
    for _ in 0..stage_count {
        stage_ids.push(RelationalBindingStageId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    Ok(RelationalDependencyKeyRecipe::restore_from_journal_codec(
        indices.into_boxed_slice(),
        stage_ids.into_boxed_slice(),
    )?)
}

fn encode_support_exactness(
    encoder: &mut Encoder,
    exactness: RelationalSupportExactness,
) -> Result<(), RelationalJournalCodecError> {
    match exactness {
        RelationalSupportExactness::StructuralExact(value) => {
            encoder.tag(0x01)?;
            encoder.u128(value)
        }
        RelationalSupportExactness::Open {
            confirmed_lower_bound,
            reason,
        } => {
            encoder.tag(0x02)?;
            encoder.u128(confirmed_lower_bound)?;
            encode_support_open_reason(encoder, reason)
        }
    }
}

fn decode_support_exactness(
    reader: &mut Reader<'_>,
) -> Result<RelationalSupportExactness, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSupportExactness::StructuralExact(reader.u128()?)),
        0x02 => Ok(RelationalSupportExactness::Open {
            confirmed_lower_bound: reader.u128()?,
            reason: decode_support_open_reason(reader)?,
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support exactness",
            tag,
        }),
    }
}

fn encode_support_open_reason(
    encoder: &mut Encoder,
    reason: RelationalSupportOpenReason,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match reason {
        RelationalSupportOpenReason::RuntimeDomain => 0x01,
        RelationalSupportOpenReason::DependentFiberJoin => 0x02,
        RelationalSupportOpenReason::NaturalJoin => 0x03,
        RelationalSupportOpenReason::CoordinateCardinalityExceedsU128 => 0x04,
        RelationalSupportOpenReason::CoordinateCardinalityOverflow => 0x05,
        RelationalSupportOpenReason::MappedImageNeedsEvidence => 0x06,
        RelationalSupportOpenReason::SuccessorFiberSum => 0x07,
    })
}

fn decode_support_open_reason(
    reader: &mut Reader<'_>,
) -> Result<RelationalSupportOpenReason, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSupportOpenReason::RuntimeDomain),
        0x02 => Ok(RelationalSupportOpenReason::DependentFiberJoin),
        0x03 => Ok(RelationalSupportOpenReason::NaturalJoin),
        0x04 => Ok(RelationalSupportOpenReason::CoordinateCardinalityExceedsU128),
        0x05 => Ok(RelationalSupportOpenReason::CoordinateCardinalityOverflow),
        0x06 => Ok(RelationalSupportOpenReason::MappedImageNeedsEvidence),
        0x07 => Ok(RelationalSupportOpenReason::SuccessorFiberSum),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support open reason",
            tag,
        }),
    }
}

fn encode_population_kind(
    encoder: &mut Encoder,
    kind: RelationalSupportPopulationKind,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match kind {
        RelationalSupportPopulationKind::SourceAssignments => 0x01,
        RelationalSupportPopulationKind::SourceRows => 0x02,
        RelationalSupportPopulationKind::SuccessorCoordinates => 0x03,
        RelationalSupportPopulationKind::Cases => 0x04,
    })
}

fn decode_population_kind(
    reader: &mut Reader<'_>,
) -> Result<RelationalSupportPopulationKind, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSupportPopulationKind::SourceAssignments),
        0x02 => Ok(RelationalSupportPopulationKind::SourceRows),
        0x03 => Ok(RelationalSupportPopulationKind::SuccessorCoordinates),
        0x04 => Ok(RelationalSupportPopulationKind::Cases),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support population kind",
            tag,
        }),
    }
}

fn encode_exact_empty_reason(
    encoder: &mut Encoder,
    reason: RelationalExactEmptyReason,
) -> Result<(), RelationalJournalCodecError> {
    match reason {
        RelationalExactEmptyReason::StaticFiniteDomain { stage_id } => {
            encoder.tag(0x01)?;
            encoder.digest(stage_id.bytes())
        }
        RelationalExactEmptyReason::EmptyDependencyKeySpace {
            stage_id,
            empty_input_dimension,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(stage_id.bytes())?;
            encoder.digest(empty_input_dimension.bytes())
        }
        RelationalExactEmptyReason::EmptyAssignmentFactor { stage_id } => {
            encoder.tag(0x03)?;
            encoder.digest(stage_id.bytes())
        }
        RelationalExactEmptyReason::StaticSuccessorDomain => encoder.tag(0x04),
        RelationalExactEmptyReason::UpstreamPopulation(kind) => {
            encoder.tag(0x05)?;
            encode_population_kind(encoder, kind)
        }
    }
}

fn decode_exact_empty_reason(
    reader: &mut Reader<'_>,
) -> Result<RelationalExactEmptyReason, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalExactEmptyReason::StaticFiniteDomain {
            stage_id: RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x02 => Ok(RelationalExactEmptyReason::EmptyDependencyKeySpace {
            stage_id: RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
            empty_input_dimension: RelationalDimensionId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x03 => Ok(RelationalExactEmptyReason::EmptyAssignmentFactor {
            stage_id: RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x04 => Ok(RelationalExactEmptyReason::StaticSuccessorDomain),
        0x05 => Ok(RelationalExactEmptyReason::UpstreamPopulation(
            decode_population_kind(reader)?,
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "exact-empty reason",
            tag,
        }),
    }
}

fn encode_planned_support(
    encoder: &mut Encoder,
    support: &RelationalPlannedSupport,
) -> Result<(), RelationalJournalCodecError> {
    match support {
        RelationalPlannedSupport::Cell { cell, exactness } => {
            encoder.tag(0x01)?;
            encode_support_cell(encoder, cell)?;
            encode_support_exactness(encoder, *exactness)
        }
        RelationalPlannedSupport::ExactEmpty { reason } => {
            encoder.tag(0x02)?;
            encode_exact_empty_reason(encoder, *reason)
        }
    }
}

fn decode_planned_support(
    reader: &mut Reader<'_>,
) -> Result<RelationalPlannedSupport, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalPlannedSupport::restore_from_journal_codec(
            Some(decode_support_cell(reader)?),
            decode_support_exactness(reader)?,
            None,
        )?),
        0x02 => Ok(RelationalPlannedSupport::restore_from_journal_codec(
            None,
            RelationalSupportExactness::StructuralExact(0),
            Some(decode_exact_empty_reason(reader)?),
        )?),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "planned support",
            tag,
        }),
    }
}

fn encode_factor_schema(
    encoder: &mut Encoder,
    schema: &RelationalFactorSchema,
) -> Result<(), RelationalJournalCodecError> {
    encode_dimension_ids(encoder, schema.key_dimensions())?;
    encoder.digest(schema.output_dimension().bytes())
}

fn decode_factor_schema(
    reader: &mut Reader<'_>,
) -> Result<RelationalFactorSchema, RelationalJournalCodecError> {
    Ok(RelationalFactorSchema::restore_from_journal_codec(
        decode_dimension_ids(reader, "factor key dimensions")?,
        RelationalDimensionId::from_journal_codec_bytes(reader.digest()?),
    ))
}

fn encode_finite_domain_kind(
    encoder: &mut Encoder,
    kind: RelationalFiniteDomainRecipeKind,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match kind {
        RelationalFiniteDomainRecipeKind::CheckedExact => 0x01,
        RelationalFiniteDomainRecipeKind::CheckedCollection => 0x02,
        RelationalFiniteDomainRecipeKind::CheckedIntRange => 0x03,
    })
}

fn decode_finite_domain_kind(
    reader: &mut Reader<'_>,
) -> Result<RelationalFiniteDomainRecipeKind, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalFiniteDomainRecipeKind::CheckedExact),
        0x02 => Ok(RelationalFiniteDomainRecipeKind::CheckedCollection),
        0x03 => Ok(RelationalFiniteDomainRecipeKind::CheckedIntRange),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "finite domain recipe kind",
            tag,
        }),
    }
}

fn encode_optional_u128(
    encoder: &mut Encoder,
    value: Option<u128>,
) -> Result<(), RelationalJournalCodecError> {
    encoder.bool(value.is_some())?;
    if let Some(value) = value {
        encoder.u128(value)?;
    }
    Ok(())
}

fn decode_optional_u128(
    reader: &mut Reader<'_>,
) -> Result<Option<u128>, RelationalJournalCodecError> {
    Ok(if reader.bool()? {
        Some(reader.u128()?)
    } else {
        None
    })
}

fn encode_finite_factor_recipe(
    encoder: &mut Encoder,
    recipe: &RelationalFiniteFactorRecipe,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(recipe.binding_index())?;
    encode_dependency_key(encoder, recipe.dependency_key())?;
    encode_finite_domain_kind(encoder, recipe.domain_kind())?;
    encoder.digest(recipe.producer_id().bytes())?;
    encoder.digest(recipe.materializer_id().bytes())?;
    encode_optional_u128(encoder, recipe.known_local_cardinality())
}

fn decode_finite_factor_recipe(
    reader: &mut Reader<'_>,
) -> Result<RelationalFiniteFactorRecipe, RelationalJournalCodecError> {
    Ok(RelationalFiniteFactorRecipe::restore_from_journal_codec(
        reader.u32()?,
        decode_dependency_key(reader)?,
        decode_finite_domain_kind(reader)?,
        SupportProducerId::from_journal_codec_bytes(reader.digest()?),
        SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
        decode_optional_u128(reader)?,
    ))
}

fn encode_binding_stage(
    encoder: &mut Encoder,
    stage: &RelationalBindingStage,
) -> Result<(), RelationalJournalCodecError> {
    match stage {
        RelationalBindingStage::Finite(stage) => {
            encoder.tag(0x01)?;
            encoder.digest(stage.stage_id().bytes())?;
            encode_binding_role(encoder, stage.role())?;
            encoder.digest(stage.dimension_id().bytes())?;
            encode_factor_schema(encoder, stage.schema())?;
            encode_planned_support(encoder, stage.support())?;
            encode_finite_factor_recipe(encoder, stage.recipe())
        }
        RelationalBindingStage::Singleton(stage) => {
            encoder.tag(0x02)?;
            encoder.digest(stage.stage_id().bytes())?;
            encoder.u32(stage.binding_index())?;
            encode_binding_role(encoder, stage.role())?;
            encode_dependency_key(encoder, stage.dependency_key())?;
            encode_dimension_ids(encoder, stage.input_dimensions())
        }
    }
}

fn decode_binding_stage(
    reader: &mut Reader<'_>,
) -> Result<RelationalBindingStage, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalBindingStage::Finite(
            RelationalFiniteFactorStage::restore_from_journal_codec(
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                decode_binding_role(reader)?,
                RelationalDimensionId::from_journal_codec_bytes(reader.digest()?),
                decode_factor_schema(reader)?,
                decode_planned_support(reader)?,
                decode_finite_factor_recipe(reader)?,
            )?,
        )),
        0x02 => Ok(RelationalBindingStage::Singleton(
            RelationalSingletonMapStage::restore_from_journal_codec(
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                reader.u32()?,
                decode_binding_role(reader)?,
                decode_dependency_key(reader)?,
                decode_dimension_ids(reader, "singleton input dimensions")?,
            ),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "binding stage",
            tag,
        }),
    }
}

fn encode_cell_ids(
    encoder: &mut Encoder,
    cells: &[SupportCellId],
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(cells.len())?;
    for cell in cells {
        encoder.digest(cell.bytes())?;
    }
    Ok(())
}

fn decode_cell_ids(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[SupportCellId]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        cells.push(SupportCellId::from_journal_codec_bytes(reader.digest()?));
    }
    Ok(cells.into_boxed_slice())
}

fn encode_successor_recipe_kind(
    encoder: &mut Encoder,
    kind: RelationalSuccessorRecipeKind,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match kind {
        RelationalSuccessorRecipeKind::Singleton => 0x01,
        RelationalSuccessorRecipeKind::FiniteExact => 0x02,
        RelationalSuccessorRecipeKind::FiniteCollection => 0x03,
        RelationalSuccessorRecipeKind::FiniteIntRange => 0x04,
    })
}

fn decode_successor_recipe_kind(
    reader: &mut Reader<'_>,
) -> Result<RelationalSuccessorRecipeKind, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSuccessorRecipeKind::Singleton),
        0x02 => Ok(RelationalSuccessorRecipeKind::FiniteExact),
        0x03 => Ok(RelationalSuccessorRecipeKind::FiniteCollection),
        0x04 => Ok(RelationalSuccessorRecipeKind::FiniteIntRange),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "successor recipe kind",
            tag,
        }),
    }
}

fn encode_population_recipe(
    encoder: &mut Encoder,
    recipe: &RelationalSupportPopulationRecipe,
) -> Result<(), RelationalJournalCodecError> {
    match recipe {
        RelationalSupportPopulationRecipe::ExactEmpty { reason } => {
            encoder.tag(0x01)?;
            encode_exact_empty_reason(encoder, *reason)
        }
        RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells } => {
            encoder.tag(0x02)?;
            encode_cell_ids(encoder, factor_cells)
        }
        RelationalSupportPopulationRecipe::DependentAssignmentJoin { factor_cells } => {
            encoder.tag(0x03)?;
            encode_cell_ids(encoder, factor_cells)
        }
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell } => {
            encoder.tag(0x04)?;
            encoder.digest(assignment_cell.bytes())
        }
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell,
            successor_kind,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(source_row_cell.bytes())?;
            encode_successor_recipe_kind(encoder, *successor_kind)
        }
        RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell,
        } => {
            encoder.tag(0x06)?;
            encoder.digest(successor_coordinate_cell.bytes())
        }
    }
}

fn decode_population_recipe(
    reader: &mut Reader<'_>,
) -> Result<RelationalSupportPopulationRecipe, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSupportPopulationRecipe::ExactEmpty {
            reason: decode_exact_empty_reason(reader)?,
        }),
        0x02 => Ok(
            RelationalSupportPopulationRecipe::IndependentAssignmentProduct {
                factor_cells: decode_cell_ids(reader, "independent assignment factors")?,
            },
        ),
        0x03 => Ok(RelationalSupportPopulationRecipe::DependentAssignmentJoin {
            factor_cells: decode_cell_ids(reader, "dependent assignment factors")?,
        }),
        0x04 => Ok(RelationalSupportPopulationRecipe::SourceRowImage {
            assignment_cell: SupportCellId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x05 => Ok(RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell: SupportCellId::from_journal_codec_bytes(reader.digest()?),
            successor_kind: decode_successor_recipe_kind(reader)?,
        }),
        0x06 => Ok(RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell: SupportCellId::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support population recipe",
            tag,
        }),
    }
}

fn encode_planned_population(
    encoder: &mut Encoder,
    population: &RelationalPlannedPopulation,
) -> Result<(), RelationalJournalCodecError> {
    encode_population_kind(encoder, population.kind())?;
    encode_planned_support(encoder, population.support())?;
    encode_population_recipe(encoder, population.recipe())
}

fn decode_planned_population(
    reader: &mut Reader<'_>,
) -> Result<RelationalPlannedPopulation, RelationalJournalCodecError> {
    Ok(RelationalPlannedPopulation::restore_from_journal_codec(
        decode_population_kind(reader)?,
        decode_planned_support(reader)?,
        decode_population_recipe(reader)?,
    )?)
}

fn encode_obligation_activation(
    encoder: &mut Encoder,
    activation: RelationalObligationActivation,
) -> Result<(), RelationalJournalCodecError> {
    match activation {
        RelationalObligationActivation::RootCasePopulation => encoder.tag(0x01),
        RelationalObligationActivation::AdmissionDecision(decision) => {
            encoder.tag(0x02)?;
            encode_admission_decision(encoder, decision)
        }
        RelationalObligationActivation::SelectionDecision(decision) => {
            encoder.tag(0x03)?;
            encode_selection_decision(encoder, decision)
        }
    }
}

fn decode_obligation_activation(
    reader: &mut Reader<'_>,
) -> Result<RelationalObligationActivation, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalObligationActivation::RootCasePopulation),
        0x02 => Ok(RelationalObligationActivation::AdmissionDecision(
            decode_admission_decision(reader)?,
        )),
        0x03 => Ok(RelationalObligationActivation::SelectionDecision(
            decode_selection_decision(reader)?,
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "obligation activation",
            tag,
        }),
    }
}

fn encode_staged_obligation(
    encoder: &mut Encoder,
    descriptor: &RelationalStagedObligationDescriptor,
) -> Result<(), RelationalJournalCodecError> {
    match descriptor {
        RelationalStagedObligationDescriptor::Root {
            activation,
            obligation,
        } => {
            encoder.tag(0x01)?;
            encode_obligation_activation(encoder, *activation)?;
            encode_support_obligation(encoder, obligation)
        }
        RelationalStagedObligationDescriptor::SelectionOnAdmitted {
            activation,
            question_id,
        } => {
            encoder.tag(0x02)?;
            encode_obligation_activation(encoder, *activation)?;
            encoder.digest(question_id.bytes())
        }
    }
}

fn decode_staged_obligation(
    reader: &mut Reader<'_>,
) -> Result<RelationalStagedObligationDescriptor, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalStagedObligationDescriptor::Root {
            activation: decode_obligation_activation(reader)?,
            obligation: decode_support_obligation(reader)?,
        }),
        0x02 => Ok(RelationalStagedObligationDescriptor::SelectionOnAdmitted {
            activation: decode_obligation_activation(reader)?,
            question_id: QuestionId::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "staged obligation",
            tag,
        }),
    }
}

fn encode_root_obligation_plan(
    encoder: &mut Encoder,
    plan: &RelationalRootObligationPlan,
) -> Result<(), RelationalJournalCodecError> {
    match plan {
        RelationalRootObligationPlan::ResolvedExactEmpty { admission_id } => {
            encoder.tag(0x01)?;
            encoder.digest(admission_id.bytes())
        }
        RelationalRootObligationPlan::CellBacked {
            root_cell_id,
            descriptors,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(root_cell_id.bytes())?;
            encoder.collection_len(descriptors.len())?;
            for descriptor in descriptors {
                encode_staged_obligation(encoder, descriptor)?;
            }
            Ok(())
        }
    }
}

fn decode_root_obligation_plan(
    reader: &mut Reader<'_>,
) -> Result<RelationalRootObligationPlan, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalRootObligationPlan::ResolvedExactEmpty {
            admission_id: AdmissionId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x02 => {
            let root_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
            let count = reader.collection_len("root obligation descriptors")?;
            let mut descriptors = Vec::new();
            descriptors
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                descriptors.push(decode_staged_obligation(reader)?);
            }
            Ok(RelationalRootObligationPlan::CellBacked {
                root_cell_id,
                descriptors: descriptors.into_boxed_slice(),
            })
        }
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "root obligation plan",
            tag,
        }),
    }
}

fn encode_coverage(
    encoder: &mut Encoder,
    coverage: &RelationalCoverageQualifier,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match coverage.status() {
        RelationalCoverageStatus::NoKnownGaps => 0x01,
        RelationalCoverageStatus::HasCoverageGaps => 0x02,
    })?;
    encoder.string(coverage.manifest_digest())?;
    encoder.digest(coverage.semantic_dependency_digest())?;
    encoder.usize(coverage.varied_dimensions())?;
    encoder.usize(coverage.derived_subjects())?;
    encoder.usize(coverage.conditioned_subjects())?;
    encoder.usize(coverage.irrelevance_certificates())?;
    encoder.usize(coverage.coverage_gaps())
}

fn decode_coverage(
    reader: &mut Reader<'_>,
) -> Result<RelationalCoverageQualifier, RelationalJournalCodecError> {
    let status = match reader.tag()? {
        0x01 => RelationalCoverageStatus::NoKnownGaps,
        0x02 => RelationalCoverageStatus::HasCoverageGaps,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "coverage status",
                tag,
            });
        }
    };
    Ok(RelationalCoverageQualifier::restore_from_journal_codec(
        status,
        reader.string()?,
        reader.digest()?,
        reader.usize("varied dimensions")?,
        reader.usize("derived subjects")?,
        reader.usize("conditioned subjects")?,
        reader.usize("irrelevance certificates")?,
        reader.usize("coverage gaps")?,
    )?)
}

fn encode_support_plan(
    encoder: &mut Encoder,
    plan: &RelationalSupportPlan,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(plan.relation_id().bytes())?;
    encoder.digest(plan.admission_id().bytes())?;
    encoder.digest(plan.question_id().bytes())?;
    encode_uniform_admission_proof_recipe(encoder, plan.uniform_admission_proof())?;
    encode_source_image_projection_certificate(encoder, plan.source_image_projection())?;
    encoder.collection_len(plan.stages().len())?;
    for stage in plan.stages() {
        encode_binding_stage(encoder, stage)?;
    }
    encode_planned_population(encoder, plan.source_assignments())?;
    encode_planned_population(encoder, plan.source_rows())?;
    encode_planned_population(encoder, plan.successor_coordinates())?;
    encode_planned_population(encoder, plan.cases())?;
    encode_root_obligation_plan(encoder, plan.root_obligations())?;
    encode_coverage(encoder, plan.coverage())
}

fn decode_support_plan(
    reader: &mut Reader<'_>,
) -> Result<RelationalSupportPlan, RelationalJournalCodecError> {
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let admission_id = AdmissionId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let uniform_admission_proof = decode_uniform_admission_proof_recipe(reader)?;
    let source_image_projection = decode_source_image_projection_certificate(reader)?;
    let count = reader.collection_len("support binding stages")?;
    let mut stages = Vec::new();
    stages
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        stages.push(decode_binding_stage(reader)?);
    }
    Ok(RelationalSupportPlan::restore_from_journal_codec(
        relation_id,
        admission_id,
        question_id,
        uniform_admission_proof,
        stages.into_boxed_slice(),
        decode_planned_population(reader)?,
        decode_planned_population(reader)?,
        decode_planned_population(reader)?,
        decode_planned_population(reader)?,
        decode_root_obligation_plan(reader)?,
        decode_coverage(reader)?,
        source_image_projection,
    )?)
}

fn encode_source_image_projection_certificate(
    encoder: &mut Encoder,
    certificate: Option<&CheckedExploreSourceImageProjectionCertificate>,
) -> Result<(), RelationalJournalCodecError> {
    let Some(certificate) = certificate else {
        return encoder.tag(0x02);
    };
    encoder.tag(0x01)?;
    encoder.u32(certificate.version)?;
    encoder.digest(certificate.relation_id.bytes())?;
    encoder.digest(certificate.semantic_dependency_digest)?;
    encoder.digest(certificate.total_construction_digest)?;
    encoder.collection_len(certificate.factors.len())?;
    for factor in certificate.factors.iter() {
        encoder.u32(factor.binding_index)?;
        encoder.digest(factor.binder_digest)?;
        encoder.i64(factor.start)?;
        encoder.i64(factor.end_exclusive)?;
        encoder.u128(factor.exact_cardinality)?;
    }
    encoder.collection_len(certificate.witnesses.len())?;
    for witness in certificate.witnesses.iter() {
        encoder.u32(witness.factor_binding_index)?;
        encoder.tag(match witness.endpoint {
            CheckedExploreSourceProjectionEndpoint::Context => 0x01,
            CheckedExploreSourceProjectionEndpoint::Before => 0x02,
        })?;
        encoder.collection_len(witness.path.len())?;
        for field in witness.path.iter() {
            encoder.digest(field.owner_digest)?;
            encoder.u32(field.variant_index)?;
            encoder.u32(field.field_index)?;
        }
        encoder.i64(witness.coefficient)?;
        encoder.i64(witness.offset)?;
        encoder.i64(witness.output_min)?;
        encoder.i64(witness.output_max)?;
        encoder.digest(witness.affine_proof_digest)?;
        encoder.digest(witness.witness_id)?;
    }
    encoder.digest(certificate.certificate_id)
}

fn decode_source_image_projection_certificate(
    reader: &mut Reader<'_>,
) -> Result<Option<CheckedExploreSourceImageProjectionCertificate>, RelationalJournalCodecError> {
    match reader.tag()? {
        0x02 => Ok(None),
        0x01 => {
            let version = reader.u32()?;
            let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
            let semantic_dependency_digest = reader.digest()?;
            let total_construction_digest = reader.digest()?;
            let factor_count = reader.collection_len("source projection factors")?;
            let mut factors = Vec::new();
            factors.try_reserve_exact(factor_count).map_err(|_| {
                RelationalJournalCodecError::AllocationFailed {
                    requested: factor_count,
                }
            })?;
            for _ in 0..factor_count {
                factors.push(CheckedExploreSourceProjectionFactor {
                    binding_index: reader.u32()?,
                    binder_digest: reader.digest()?,
                    start: reader.i64()?,
                    end_exclusive: reader.i64()?,
                    exact_cardinality: reader.u128()?,
                });
            }
            let witness_count = reader.collection_len("source projection witnesses")?;
            let mut witnesses = Vec::new();
            witnesses.try_reserve_exact(witness_count).map_err(|_| {
                RelationalJournalCodecError::AllocationFailed {
                    requested: witness_count,
                }
            })?;
            for _ in 0..witness_count {
                let factor_binding_index = reader.u32()?;
                let endpoint = match reader.tag()? {
                    0x01 => CheckedExploreSourceProjectionEndpoint::Context,
                    0x02 => CheckedExploreSourceProjectionEndpoint::Before,
                    tag => {
                        return Err(RelationalJournalCodecError::UnknownTag {
                            component: "source projection endpoint",
                            tag,
                        });
                    }
                };
                let path_count = reader.collection_len("source projection field path")?;
                let mut path = Vec::new();
                path.try_reserve_exact(path_count).map_err(|_| {
                    RelationalJournalCodecError::AllocationFailed {
                        requested: path_count,
                    }
                })?;
                for _ in 0..path_count {
                    path.push(CheckedExploreSourceProjectionField {
                        owner_digest: reader.digest()?,
                        variant_index: reader.u32()?,
                        field_index: reader.u32()?,
                    });
                }
                witnesses.push(CheckedExploreSourceProjectionWitness {
                    factor_binding_index,
                    endpoint,
                    path: path.into_boxed_slice(),
                    coefficient: reader.i64()?,
                    offset: reader.i64()?,
                    output_min: reader.i64()?,
                    output_max: reader.i64()?,
                    affine_proof_digest: reader.digest()?,
                    witness_id: reader.digest()?,
                });
            }
            let certificate = CheckedExploreSourceImageProjectionCertificate {
                version,
                relation_id,
                semantic_dependency_digest,
                total_construction_digest,
                factors: factors.into_boxed_slice(),
                witnesses: witnesses.into_boxed_slice(),
                certificate_id: reader.digest()?,
            };
            if !certificate.validate_identity() {
                return Err(RelationalJournalCodecError::Malformed(
                    "source projection certificate identity is invalid",
                ));
            }
            Ok(Some(certificate))
        }
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "source projection certificate",
            tag,
        }),
    }
}

fn encode_uniform_admission_proof_recipe(
    encoder: &mut Encoder,
    recipe: &RelationalUniformAdmissionProofRecipe,
) -> Result<(), RelationalJournalCodecError> {
    let Some(predicates) = recipe.literal_predicates() else {
        return encoder.tag(0x02);
    };
    encoder.tag(0x01)?;
    encoder.collection_len(predicates.len())?;
    for predicate in predicates {
        encoder.u32(predicate.admission_index())?;
        encoder.tag(match predicate.scope() {
            ExploreAdmissionScope::Before => 0x01,
            ExploreAdmissionScope::After => 0x02,
            ExploreAdmissionScope::Transition => 0x03,
        })?;
        encoder.tag(if predicate.value() { 0x01 } else { 0x02 })?;
    }
    Ok(())
}

fn decode_uniform_admission_proof_recipe(
    reader: &mut Reader<'_>,
) -> Result<RelationalUniformAdmissionProofRecipe, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => {
            let count = reader.collection_len("literal admission proof predicates")?;
            let mut predicates = Vec::new();
            predicates
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                let admission_index = reader.u32()?;
                let scope = match reader.tag()? {
                    0x01 => ExploreAdmissionScope::Before,
                    0x02 => ExploreAdmissionScope::After,
                    0x03 => ExploreAdmissionScope::Transition,
                    tag => {
                        return Err(RelationalJournalCodecError::UnknownTag {
                            component: "literal admission predicate scope",
                            tag,
                        });
                    }
                };
                let value = match reader.tag()? {
                    0x01 => true,
                    0x02 => false,
                    tag => {
                        return Err(RelationalJournalCodecError::UnknownTag {
                            component: "literal admission predicate value",
                            tag,
                        });
                    }
                };
                predicates.push(
                    RelationalLiteralAdmissionPredicate::restore_from_journal_codec(
                        admission_index,
                        scope,
                        value,
                    ),
                );
            }
            Ok(
                RelationalUniformAdmissionProofRecipe::restore_literal_conjunction_from_journal_codec(
                    predicates.into_boxed_slice(),
                )?,
            )
        }
        0x02 => Ok(RelationalUniformAdmissionProofRecipe::restore_unsupported_from_journal_codec()),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "uniform admission proof recipe",
            tag,
        }),
    }
}

fn encode_case_chunk_partition_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalCaseChunkPartitionArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.id().bytes())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.admission_id().bytes())?;
    encoder.digest(artifact.question_id().bytes())?;
    encoder.digest(artifact.case_image_certificate_id())?;
    encoder.digest(artifact.injectivity_evidence_id().bytes())?;
    encoder.digest(artifact.root_cell_id().bytes())?;
    encoder.digest(artifact.root_materializer_id().bytes())?;
    encoder.tag(match artifact.shape() {
        RelationalCaseChunkShape::BareOrdinalInterval => 0x01,
        RelationalCaseChunkShape::ProductFactor => 0x02,
        RelationalCaseChunkShape::ProductRankInterval => 0x03,
    })?;
    match artifact.factor_index() {
        Some(factor_index) => {
            encoder.tag(0x01)?;
            encoder.u32(factor_index)?;
        }
        None => encoder.tag(0x02)?,
    }
    encoder.u128(artifact.interval_start())?;
    encoder.u128(artifact.interval_end_exclusive())?;
    encoder.u128(artifact.max_chunk_coordinates())?;
    encoder.collection_len(artifact.chunks().len())?;
    for chunk in artifact.chunks() {
        encoder.digest(chunk.id().bytes())?;
        encoder.u128(chunk.ordinal())?;
        encoder.digest(chunk.cell_id().bytes())?;
        encoder.u128(chunk.interval_start())?;
        encoder.u128(chunk.interval_end_exclusive())?;
    }
    encoder.digest(artifact.partition_id().bytes())
}

fn decode_case_chunk_partition_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalCaseChunkPartitionArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let artifact_id =
        RelationalCaseChunkPartitionArtifactId::from_canonical_bytes(reader.digest()?);
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let admission_id = AdmissionId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let case_image_certificate_id = reader.digest()?;
    let injectivity_evidence_id = SupportCellEvidenceId::from_journal_codec_bytes(reader.digest()?);
    let root_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let root_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);
    let shape = match reader.tag()? {
        0x01 => RelationalCaseChunkShape::BareOrdinalInterval,
        0x02 => RelationalCaseChunkShape::ProductFactor,
        0x03 => RelationalCaseChunkShape::ProductRankInterval,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "case-chunk partition shape",
                tag,
            });
        }
    };
    let factor_index = match reader.tag()? {
        0x01 => Some(reader.u32()?),
        0x02 => None,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "case-chunk partition factor index",
                tag,
            });
        }
    };
    let interval_start = reader.u128()?;
    let interval_end_exclusive = reader.u128()?;
    let max_chunk_coordinates = reader.u128()?;
    let chunk_count = reader.collection_len("case-chunk partition children")?;
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(chunk_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: chunk_count,
        }
    })?;
    for _ in 0..chunk_count {
        chunks.push(RelationalCaseChunkDescriptor::restore_from_canonical_parts(
            RelationalCaseChunkId::from_canonical_bytes(reader.digest()?),
            reader.u128()?,
            SupportCellId::from_journal_codec_bytes(reader.digest()?),
            reader.u128()?,
            reader.u128()?,
        )?);
    }
    RelationalCaseChunkPartitionArtifact::restore_from_canonical_parts(
        schema_version,
        artifact_id,
        plan_root,
        relation_id,
        admission_id,
        question_id,
        case_image_certificate_id,
        injectivity_evidence_id,
        root_cell_id,
        root_materializer_id,
        shape,
        factor_index,
        interval_start,
        interval_end_exclusive,
        max_chunk_coordinates,
        chunks.into_boxed_slice(),
        SupportPartitionId::from_journal_codec_bytes(reader.digest()?),
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_classified_chunk_slice_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalClassifiedChunkSliceArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.id().bytes())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.admission_id().bytes())?;
    encoder.digest(artifact.question_id().bytes())?;
    encoder.digest(artifact.chunk_partition_id().bytes())?;
    encoder.digest(artifact.chunk_id().bytes())?;
    encoder.u128(artifact.chunk_ordinal())?;
    encoder.digest(artifact.chunk_cell_id().bytes())?;
    encoder.digest(artifact.chunk_materializer_id().bytes())?;
    encoder.u128(artifact.chunk_interval_start())?;
    encoder.u128(artifact.chunk_interval_end_exclusive())?;
    encoder.u128(artifact.slice_interval_start())?;
    encoder.u128(artifact.slice_interval_end_exclusive())?;
    match artifact.predecessor_slice_id() {
        Some(predecessor) => {
            encoder.tag(0x01)?;
            encoder.digest(predecessor.bytes())?;
        }
        None => encoder.tag(0x02)?,
    }
    encoder.digest(artifact.transcript_root_before().bytes())?;
    encoder.digest(artifact.transcript_root_after().bytes())?;
    encoder.u128(artifact.rejected_count())?;
    encoder.u128(artifact.admitted_not_selected_count())?;
    encoder.u128(artifact.admitted_selected_count())?;
    encoder.collection_len(artifact.runs().len())?;
    for run in artifact.runs() {
        encoder.u128(run.interval_start())?;
        encoder.u128(run.interval_end_exclusive())?;
        encoder.tag(run.outcome().canonical_tag())?;
    }
    Ok(())
}

fn decode_classified_chunk_slice_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalClassifiedChunkSliceArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let artifact_id = RelationalClassifiedChunkSliceId::from_journal_codec_bytes(reader.digest()?);
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let admission_id = AdmissionId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let chunk_partition_id =
        RelationalCaseChunkPartitionArtifactId::from_canonical_bytes(reader.digest()?);
    let chunk_id = RelationalCaseChunkId::from_canonical_bytes(reader.digest()?);
    let chunk_ordinal = reader.u128()?;
    let chunk_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let chunk_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);
    let chunk_interval_start = reader.u128()?;
    let chunk_interval_end_exclusive = reader.u128()?;
    let slice_interval_start = reader.u128()?;
    let slice_interval_end_exclusive = reader.u128()?;
    let predecessor_slice_id = match reader.tag()? {
        0x01 => Some(RelationalClassifiedChunkSliceId::from_journal_codec_bytes(
            reader.digest()?,
        )),
        0x02 => None,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "classified chunk slice predecessor",
                tag,
            });
        }
    };
    let transcript_root_before =
        RelationalClassifiedChunkTranscriptRoot::from_journal_codec_bytes(reader.digest()?);
    let transcript_root_after =
        RelationalClassifiedChunkTranscriptRoot::from_journal_codec_bytes(reader.digest()?);
    let rejected_count = reader.u128()?;
    let admitted_not_selected_count = reader.u128()?;
    let admitted_selected_count = reader.u128()?;
    let run_count = reader.collection_len("classified chunk slice runs")?;
    if run_count > 256 {
        return Err(RelationalJournalCodecError::DeclaredLengthTooLarge {
            component: "classified chunk slice runs",
            claimed: run_count as u64,
            limit: 256,
        });
    }
    let mut runs = Vec::new();
    runs.try_reserve_exact(run_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: run_count,
        }
    })?;
    for _ in 0..run_count {
        let interval_start = reader.u128()?;
        let interval_end_exclusive = reader.u128()?;
        let outcome = RelationalClassifiedCaseOutcome::from_codec_tag(reader.tag()?).ok_or(
            RelationalJournalCodecError::Malformed("unknown classified slice run outcome"),
        )?;
        runs.push(
            RelationalClassifiedChunkSliceRun::restore_from_journal_codec(
                interval_start,
                interval_end_exclusive,
                outcome,
            )?,
        );
    }
    RelationalClassifiedChunkSliceArtifact::restore_from_journal_codec(
        schema_version,
        artifact_id,
        plan_root,
        relation_id,
        admission_id,
        question_id,
        chunk_partition_id,
        chunk_id,
        chunk_ordinal,
        chunk_cell_id,
        chunk_materializer_id,
        chunk_interval_start,
        chunk_interval_end_exclusive,
        slice_interval_start,
        slice_interval_end_exclusive,
        predecessor_slice_id,
        transcript_root_before,
        transcript_root_after,
        rejected_count,
        admitted_not_selected_count,
        admitted_selected_count,
        runs.into_boxed_slice(),
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_classified_chunk_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalClassifiedChunkArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.id().bytes())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.admission_id().bytes())?;
    encoder.digest(artifact.question_id().bytes())?;
    encoder.digest(artifact.chunk_partition_id().bytes())?;
    encoder.digest(artifact.chunk_id().bytes())?;
    encoder.u128(artifact.chunk_ordinal())?;
    encoder.digest(artifact.chunk_cell_id().bytes())?;
    encoder.digest(artifact.chunk_materializer_id().bytes())?;
    encoder.u128(artifact.interval_start())?;
    encoder.u128(artifact.interval_end_exclusive())?;
    encoder.u128(artifact.evaluated_case_count())?;
    encoder.digest(artifact.evaluated_cases_root())?;
    encoder.u128(artifact.rejected_count())?;
    encoder.u128(artifact.admitted_not_selected_count())?;
    encoder.u128(artifact.admitted_selected_count())?;
    encoder.collection_len(artifact.runs().len())?;
    for run in artifact.runs() {
        encoder.digest(run.id().bytes())?;
        encoder.u32(u32::from(run.ordinal()))?;
        encoder.digest(run.cell_id().bytes())?;
        encoder.u128(run.interval_start())?;
        encoder.u128(run.interval_end_exclusive())?;
        encoder.tag(run.outcome().canonical_tag())?;
    }
    match artifact.partition_id() {
        Some(partition_id) => {
            encoder.tag(0x01)?;
            encoder.digest(partition_id.bytes())
        }
        None => encoder.tag(0x02),
    }
}

fn decode_classified_chunk_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalClassifiedChunkArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let artifact_id =
        RelationalClassifiedChunkArtifactId::from_journal_codec_bytes(reader.digest()?);
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let admission_id = AdmissionId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let chunk_partition_id =
        RelationalCaseChunkPartitionArtifactId::from_canonical_bytes(reader.digest()?);
    let chunk_id = RelationalCaseChunkId::from_canonical_bytes(reader.digest()?);
    let chunk_ordinal = reader.u128()?;
    let chunk_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let chunk_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);
    let interval_start = reader.u128()?;
    let interval_end_exclusive = reader.u128()?;
    let evaluated_case_count = reader.u128()?;
    let evaluated_cases_root = reader.digest()?;
    let rejected_count = reader.u128()?;
    let admitted_not_selected_count = reader.u128()?;
    let admitted_selected_count = reader.u128()?;
    let run_count = reader.collection_len("classified chunk runs")?;
    if run_count > 256 {
        return Err(RelationalJournalCodecError::DeclaredLengthTooLarge {
            component: "classified chunk runs",
            claimed: run_count as u64,
            limit: 256,
        });
    }
    let mut runs = Vec::new();
    runs.try_reserve_exact(run_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: run_count,
        }
    })?;
    for _ in 0..run_count {
        let id = RelationalClassifiedRunId::from_journal_codec_bytes(reader.digest()?);
        let ordinal = u16::try_from(reader.u32()?).map_err(|_| {
            RelationalJournalCodecError::Malformed("classified run ordinal exceeds u16")
        })?;
        let cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
        let run_start = reader.u128()?;
        let run_end_exclusive = reader.u128()?;
        let outcome = RelationalClassifiedCaseOutcome::from_codec_tag(reader.tag()?).ok_or(
            RelationalJournalCodecError::Malformed("unknown classified run outcome"),
        )?;
        runs.push(
            RelationalClassifiedRunDescriptor::restore_from_journal_codec(
                id,
                ordinal,
                cell_id,
                run_start,
                run_end_exclusive,
                outcome,
            )?,
        );
    }
    let partition_id = match reader.tag()? {
        0x01 => Some(SupportPartitionId::from_journal_codec_bytes(
            reader.digest()?,
        )),
        0x02 => None,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "classified chunk partition",
                tag,
            });
        }
    };
    RelationalClassifiedChunkArtifact::restore_from_journal_codec(
        schema_version,
        artifact_id,
        plan_root,
        relation_id,
        admission_id,
        question_id,
        chunk_partition_id,
        chunk_id,
        chunk_ordinal,
        chunk_cell_id,
        chunk_materializer_id,
        interval_start,
        interval_end_exclusive,
        evaluated_case_count,
        evaluated_cases_root,
        rejected_count,
        admitted_not_selected_count,
        admitted_selected_count,
        runs.into_boxed_slice(),
        partition_id,
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_selected_run_materialization_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalSelectedRunMaterializationArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.id().bytes())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.admission_id().bytes())?;
    encoder.digest(artifact.question_id().bytes())?;
    encoder.digest(artifact.classified_chunk_artifact_id().bytes())?;
    encoder.digest(artifact.chunk_partition_id().bytes())?;
    encoder.digest(artifact.chunk_id().bytes())?;
    encoder.u128(artifact.chunk_ordinal())?;
    encoder.digest(artifact.chunk_cell_id().bytes())?;
    encoder.digest(artifact.chunk_materializer_id().bytes())?;
    encoder.digest(artifact.run_id().bytes())?;
    encoder.u32(u32::from(artifact.run_ordinal()))?;
    encoder.digest(artifact.run_cell_id().bytes())?;
    encoder.digest(artifact.run_materializer_id().bytes())?;
    encoder.u128(artifact.interval_start())?;
    encoder.u128(artifact.interval_end_exclusive())?;
    encoder.u128(artifact.materialized_case_count())?;
    encoder.digest(artifact.materialized_cases_root())?;
    encoder.collection_len(artifact.cases().len())?;
    for record in artifact.cases() {
        encoder.u128(record.coordinate_ordinal())?;
        encoder.digest(record.source_key().bytes())?;
        encode_source_row(encoder, record.source())?;
        encoder.digest(record.successor_key().bytes())?;
        encode_successor_row(encoder, record.successor())?;
        encoder.digest(record.case_id().bytes())?;
    }
    Ok(())
}

fn decode_selected_run_materialization_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalSelectedRunMaterializationArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let artifact_id =
        RelationalSelectedRunMaterializationArtifactId::from_journal_codec_bytes(reader.digest()?);
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let admission_id = AdmissionId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let classified_chunk_artifact_id =
        RelationalClassifiedChunkArtifactId::from_journal_codec_bytes(reader.digest()?);
    let chunk_partition_id =
        RelationalCaseChunkPartitionArtifactId::from_canonical_bytes(reader.digest()?);
    let chunk_id = RelationalCaseChunkId::from_canonical_bytes(reader.digest()?);
    let chunk_ordinal = reader.u128()?;
    let chunk_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let chunk_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);
    let run_id = RelationalClassifiedRunId::from_journal_codec_bytes(reader.digest()?);
    let run_ordinal = u16::try_from(reader.u32()?)
        .map_err(|_| RelationalJournalCodecError::Malformed("selected-run ordinal exceeds u16"))?;
    let run_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let run_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);
    let interval_start = reader.u128()?;
    let interval_end_exclusive = reader.u128()?;
    let materialized_case_count = reader.u128()?;
    let materialized_cases_root = reader.digest()?;
    let case_count = reader.collection_len("selected-run concrete cases")?;
    let maximum_cases = usize::try_from(RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1)
        .expect("the V1 selected-run bound fits usize");
    if case_count > maximum_cases {
        return Err(RelationalJournalCodecError::DeclaredLengthTooLarge {
            component: "selected-run concrete cases",
            claimed: case_count as u64,
            limit: maximum_cases,
        });
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(case_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: case_count,
        }
    })?;
    for _ in 0..case_count {
        cases.push(RelationalSelectedCaseRecord::restore_from_journal_codec(
            reader.u128()?,
            SourceKey::from_journal_codec_bytes(reader.digest()?),
            decode_source_row(reader)?,
            SuccessorKey::from_journal_codec_bytes(reader.digest()?),
            decode_successor_row(reader)?,
            RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        ));
    }
    RelationalSelectedRunMaterializationArtifact::restore_from_journal_codec(
        schema_version,
        artifact_id,
        plan_root,
        relation_id,
        admission_id,
        question_id,
        classified_chunk_artifact_id,
        chunk_partition_id,
        chunk_id,
        chunk_ordinal,
        chunk_cell_id,
        chunk_materializer_id,
        run_id,
        run_ordinal,
        run_cell_id,
        run_materializer_id,
        interval_start,
        interval_end_exclusive,
        materialized_case_count,
        materialized_cases_root,
        cases.into_boxed_slice(),
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_case_image_injectivity_proof_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalCaseImageInjectivityProofArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.certificate_id())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.collection_len(artifact.binding_stage_ids().len())?;
    for stage_id in artifact.binding_stage_ids() {
        encoder.digest(stage_id.bytes())?;
    }
    encode_cell_ids(encoder, artifact.finite_factor_cell_ids())?;
    encoder.tag(match artifact.assignment_kind() {
        RelationalCaseImageAssignmentKind::IndependentProduct => 0x01,
        RelationalCaseImageAssignmentKind::DependentJoin => 0x02,
    })?;
    encoder.tag(match artifact.source_assignment_image_proof() {
        RelationalSourceAssignmentImageProof::Unproven => 0x01,
        RelationalSourceAssignmentImageProof::DirectEndpointCoordinates => 0x02,
        RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate => 0x03,
    })?;
    if artifact.schema_version() == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION {
        let reference = artifact.source_image_proof_reference().ok_or(
            RelationalJournalCodecError::Malformed(
                "v2 case-image proof lacks its source-image proof reference",
            ),
        )?;
        encoder.digest(reference.compiler_certificate_id())?;
        encoder.digest(reference.source_exactness_certificate_id())?;
        encoder.digest(reference.source_injectivity_evidence_id().bytes())?;
        encoder.digest(reference.source_population_root().bytes())?;
    }
    encoder.digest(artifact.source_assignment_cell_id().bytes())?;
    encoder.digest(artifact.source_row_cell_id().bytes())?;
    encoder.digest(artifact.successor_coordinate_cell_id().bytes())?;
    encode_successor_recipe_kind(encoder, artifact.successor_kind())?;
    encoder.tag(match artifact.preimage_kind() {
        RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin => 0x01,
        RelationalCaseImagePreimageKind::ComposedSingletonAssignment => 0x02,
    })?;
    encoder.digest(artifact.case_cell_id().bytes())?;
    encoder.digest(artifact.case_materializer_id().bytes())?;
    encode_optional_u128(encoder, artifact.exact_case_cardinality())
}

fn decode_case_image_injectivity_proof_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalCaseImageInjectivityProofArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let certificate_id = reader.digest()?;
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let stage_count = reader.collection_len("case-image proof binding stages")?;
    let mut binding_stage_ids = Vec::new();
    binding_stage_ids
        .try_reserve_exact(stage_count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed {
            requested: stage_count,
        })?;
    for _ in 0..stage_count {
        binding_stage_ids.push(RelationalBindingStageId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    let finite_factor_cell_ids = decode_cell_ids(reader, "case-image proof finite factor cells")?;
    let assignment_kind = match reader.tag()? {
        0x01 => RelationalCaseImageAssignmentKind::IndependentProduct,
        0x02 => RelationalCaseImageAssignmentKind::DependentJoin,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "case-image proof assignment kind",
                tag,
            });
        }
    };
    let source_assignment_image_proof = match reader.tag()? {
        0x01 => RelationalSourceAssignmentImageProof::Unproven,
        0x02 => RelationalSourceAssignmentImageProof::DirectEndpointCoordinates,
        0x03 => RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "case-image source-assignment proof",
                tag,
            });
        }
    };
    let source_image_proof_reference = (schema_version
        == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION)
        .then(|| {
            Ok::<_, RelationalJournalCodecError>(
                RelationalCaseSourceImageProofReference::restore_from_journal_codec(
                    reader.digest()?,
                    reader.digest()?,
                    SupportCellEvidenceId::from_journal_codec_bytes(reader.digest()?),
                    CertifiedSourcePopulationRoot::from_journal_codec_bytes(reader.digest()?),
                ),
            )
        })
        .transpose()?;
    let source_assignment_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let source_row_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let successor_coordinate_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let successor_kind = decode_successor_recipe_kind(reader)?;
    let preimage_kind = match reader.tag()? {
        0x01 => RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin,
        0x02 => RelationalCaseImagePreimageKind::ComposedSingletonAssignment,
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "case-image proof preimage kind",
                tag,
            });
        }
    };
    RelationalCaseImageInjectivityProofArtifact::restore_from_journal_codec(
        schema_version,
        certificate_id,
        plan_root,
        relation_id,
        binding_stage_ids.into_boxed_slice(),
        finite_factor_cell_ids,
        assignment_kind,
        source_assignment_image_proof,
        source_image_proof_reference,
        source_assignment_cell_id,
        source_row_cell_id,
        successor_coordinate_cell_id,
        successor_kind,
        preimage_kind,
        SupportCellId::from_journal_codec_bytes(reader.digest()?),
        SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
        decode_optional_u128(reader)?,
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_source_image_exactness_proof_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalSourceImageExactnessProofArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.certificate_id())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.collection_len(artifact.binding_stage_ids().len())?;
    for stage_id in artifact.binding_stage_ids() {
        encoder.digest(stage_id.bytes())?;
    }
    match artifact.shape() {
        RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
            context_stage_id,
            before_stage_id,
            before_dimension_id,
            before_factor_cell_id,
        } => {
            encoder.digest(context_stage_id.bytes())?;
            encoder.digest(before_stage_id.bytes())?;
            encoder.digest(before_dimension_id.bytes())?;
            encoder.digest(before_factor_cell_id.bytes())?;
        }
        RelationalSourceImageExactnessProofShape::SeparatedProjection {
            compiler_certificate_id,
            factors,
            witness_ids,
        } => {
            encoder.digest(*compiler_certificate_id)?;
            encoder.collection_len(factors.len())?;
            for factor in factors.iter().copied() {
                encoder.digest(factor.stage_id().bytes())?;
                encoder.digest(factor.dimension_id().bytes())?;
                encoder.digest(factor.factor_cell_id().bytes())?;
                encoder.u128(factor.exact_cardinality())?;
            }
            encoder.collection_len(witness_ids.len())?;
            for witness_id in witness_ids.iter() {
                encoder.digest(*witness_id)?;
            }
        }
    }
    encoder.digest(artifact.source_assignment_cell_id().bytes())?;
    encoder.digest(artifact.source_assignment_producer_id().bytes())?;
    encoder.digest(artifact.source_assignment_materializer_id().bytes())?;
    encoder.digest(artifact.source_row_cell_id().bytes())?;
    encoder.digest(artifact.source_materializer_id().bytes())?;
    encoder.u128(artifact.exact_source_cardinality())
}

fn decode_source_image_exactness_proof_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalSourceImageExactnessProofArtifact, RelationalJournalCodecError> {
    let schema_version = reader.u32()?;
    let certificate_id = reader.digest()?;
    let plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let stage_count = reader.collection_len("source-image proof binding stages")?;
    if schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1 && stage_count != 2 {
        return Err(RelationalJournalCodecError::Malformed(
            "source-image proof must name exactly two binding stages",
        ));
    }
    let mut binding_stage_ids = Vec::new();
    binding_stage_ids
        .try_reserve_exact(stage_count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed {
            requested: stage_count,
        })?;
    for _ in 0..stage_count {
        binding_stage_ids.push(RelationalBindingStageId::from_journal_codec_bytes(
            reader.digest()?,
        ));
    }
    let binding_stage_ids = binding_stage_ids.into_boxed_slice();
    match schema_version {
        RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1 => {
            RelationalSourceImageExactnessProofArtifact::restore_v1_from_journal_codec(
                schema_version,
                certificate_id,
                plan_root,
                relation_id,
                binding_stage_ids,
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                RelationalDimensionId::from_journal_codec_bytes(reader.digest()?),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                SupportProducerId::from_journal_codec_bytes(reader.digest()?),
                SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
                reader.u128()?,
            )
            .map_err(RelationalJournalCodecError::from)
        }
        RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION => {
            let compiler_certificate_id = reader.digest()?;
            let factor_count = reader.collection_len("source-image proof factors")?;
            let mut factors = Vec::new();
            factors.try_reserve_exact(factor_count).map_err(|_| {
                RelationalJournalCodecError::AllocationFailed {
                    requested: factor_count,
                }
            })?;
            for _ in 0..factor_count {
                factors.push(
                    RelationalSourceImageFactorBinding::restore_from_journal_codec(
                        RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                        RelationalDimensionId::from_journal_codec_bytes(reader.digest()?),
                        SupportCellId::from_journal_codec_bytes(reader.digest()?),
                        reader.u128()?,
                    ),
                );
            }
            let witness_count = reader.collection_len("source-image proof witnesses")?;
            let mut witness_ids = Vec::new();
            witness_ids.try_reserve_exact(witness_count).map_err(|_| {
                RelationalJournalCodecError::AllocationFailed {
                    requested: witness_count,
                }
            })?;
            for _ in 0..witness_count {
                witness_ids.push(reader.digest()?);
            }
            RelationalSourceImageExactnessProofArtifact::restore_v2_from_journal_codec(
                schema_version,
                certificate_id,
                plan_root,
                relation_id,
                binding_stage_ids,
                compiler_certificate_id,
                factors.into_boxed_slice(),
                witness_ids.into_boxed_slice(),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                SupportProducerId::from_journal_codec_bytes(reader.digest()?),
                SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
                reader.u128()?,
            )
            .map_err(RelationalJournalCodecError::from)
        }
        _ => Err(RelationalJournalCodecError::Malformed(
            "unsupported source-image proof schema version",
        )),
    }
}

fn encode_uniform_admission_proof_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalUniformAdmissionProofArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.schema_version())?;
    encoder.digest(artifact.certificate_id())?;
    encoder.digest(artifact.plan_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.admission_id().bytes())?;
    encoder.digest(artifact.case_cell_id().bytes())?;
    encoder.u32(artifact.predicate_count())?;
    encoder.digest(artifact.recipe_digest())?;
    encode_admission_decision(encoder, artifact.decision())
}

fn decode_uniform_admission_proof_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalUniformAdmissionProofArtifact, RelationalJournalCodecError> {
    RelationalUniformAdmissionProofArtifact::restore_from_journal_codec(
        reader.u32()?,
        reader.digest()?,
        RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?),
        RelationId::from_journal_codec_bytes(reader.digest()?),
        AdmissionId::from_journal_codec_bytes(reader.digest()?),
        SupportCellId::from_journal_codec_bytes(reader.digest()?),
        reader.u32()?,
        reader.digest()?,
        decode_admission_decision(reader)?,
    )
    .map_err(RelationalJournalCodecError::from)
}

fn encode_u128s(encoder: &mut Encoder, values: &[u128]) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(values.len())?;
    for value in values {
        encoder.u128(*value)?;
    }
    Ok(())
}

fn decode_u128s(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[u128]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(reader.u128()?);
    }
    Ok(values.into_boxed_slice())
}

fn encode_fiber_member(
    encoder: &mut Encoder,
    member: &RelationalFiberMember,
) -> Result<(), RelationalJournalCodecError> {
    encode_explore_value(encoder, member.value(), 0)?;
    encoder.u128(member.canonical_ordinal())?;
    encode_u128s(encoder, member.raw_support_ordinals())
}

fn decode_fiber_member(
    reader: &mut Reader<'_>,
) -> Result<RelationalFiberMember, RelationalJournalCodecError> {
    Ok(RelationalFiberMember::restore_from_journal_codec(
        decode_explore_value(reader, 0)?,
        reader.u128()?,
        decode_u128s(reader, "fiber support ordinals")?,
    )?)
}

fn encode_binding_selection(
    encoder: &mut Encoder,
    selection: &RelationalBindingSelection,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(selection.binding_index)?;
    encoder.u128(selection.canonical_ordinal)?;
    encoder.digest(selection.parent_prefix_digest)?;
    encode_u128s(encoder, &selection.raw_support_ordinals)
}

fn decode_binding_selection(
    reader: &mut Reader<'_>,
) -> Result<RelationalBindingSelection, RelationalJournalCodecError> {
    Ok(RelationalBindingSelection {
        binding_index: reader.u32()?,
        canonical_ordinal: reader.u128()?,
        parent_prefix_digest: reader.digest()?,
        raw_support_ordinals: decode_u128s(reader, "binding selection support ordinals")?,
    })
}

fn encode_source_prefix_snapshot(
    encoder: &mut Encoder,
    prefix: &RelationalSourcePrefixSnapshot,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(prefix.version)?;
    encoder.collection_len(prefix.values.len())?;
    for value in &prefix.values {
        encode_explore_value(encoder, value, 0)?;
    }
    encoder.digest(prefix.digest)?;
    encoder.collection_len(prefix.selections.len())?;
    for selection in &prefix.selections {
        encode_binding_selection(encoder, selection)?;
    }
    Ok(())
}

fn decode_source_prefix_snapshot(
    reader: &mut Reader<'_>,
) -> Result<RelationalSourcePrefixSnapshot, RelationalJournalCodecError> {
    let version = reader.u32()?;
    let value_count = reader.collection_len("source prefix values")?;
    let mut values = Vec::new();
    values.try_reserve_exact(value_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: value_count,
        }
    })?;
    for _ in 0..value_count {
        values.push(decode_explore_value(reader, 0)?);
    }
    let digest = reader.digest()?;
    let selection_count = reader.collection_len("source prefix selections")?;
    let mut selections = Vec::new();
    selections.try_reserve_exact(selection_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: selection_count,
        }
    })?;
    for _ in 0..selection_count {
        selections.push(decode_binding_selection(reader)?);
    }
    Ok(RelationalSourcePrefixSnapshot {
        version,
        values: values.into_boxed_slice(),
        digest,
        selections: selections.into_boxed_slice(),
    })
}

fn encode_source_cursor(
    encoder: &mut Encoder,
    cursor: &RelationalSourceCursor,
) -> Result<(), RelationalJournalCodecError> {
    let snapshot = cursor.snapshot();
    encoder.u32(snapshot.version)?;
    encoder.u32(snapshot.binding_index)?;
    encode_source_prefix_snapshot(encoder, &snapshot.prefix)?;
    encoder.u128(snapshot.next_member_ordinal)
}

fn decode_source_cursor(
    reader: &mut Reader<'_>,
) -> Result<RelationalSourceCursor, RelationalJournalCodecError> {
    Ok(RelationalSourceCursor::restore_from_journal_codec(
        RelationalSourceCursorSnapshot {
            version: reader.u32()?,
            binding_index: reader.u32()?,
            prefix: decode_source_prefix_snapshot(reader)?,
            next_member_ordinal: reader.u128()?,
        },
    )?)
}

fn encode_source_binding_receipt(
    encoder: &mut Encoder,
    receipt: &SourceBindingExhaustionReceipt,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(receipt.version())?;
    encoder.digest(receipt.relation_id().bytes())?;
    encoder.u32(receipt.binding_index())?;
    encoder.digest(receipt.prefix_digest())?;
    encoder.u128(receipt.terminal_ordinal())?;
    encoder.u128(receipt.emitted_member_count())?;
    encoder.digest(receipt.emitted_members_commitment())
}

fn decode_source_binding_receipt(
    reader: &mut Reader<'_>,
) -> Result<SourceBindingExhaustionReceipt, RelationalJournalCodecError> {
    Ok(SourceBindingExhaustionReceipt::restore_from_journal_codec(
        reader.u32()?,
        RelationId::from_journal_codec_bytes(reader.digest()?),
        reader.u32()?,
        reader.digest()?,
        reader.u128()?,
        reader.u128()?,
        reader.digest()?,
    )?)
}

fn encode_completed_source(
    encoder: &mut Encoder,
    source: &RelationalCompletedSource,
) -> Result<(), RelationalJournalCodecError> {
    encode_source_row(encoder, source.row())?;
    encode_source_prefix_snapshot(encoder, source.prefix())
}

fn decode_completed_source(
    reader: &mut Reader<'_>,
    relation_id: RelationId,
) -> Result<RelationalCompletedSource, RelationalJournalCodecError> {
    Ok(RelationalCompletedSource::restore_from_journal_codec(
        relation_id,
        decode_source_row(reader)?,
        decode_source_prefix_snapshot(reader)?,
    )?)
}

fn encode_source_advance(
    encoder: &mut Encoder,
    advance: &RelationalSourceAdvance,
) -> Result<(), RelationalJournalCodecError> {
    match advance {
        RelationalSourceAdvance::Yielded {
            member,
            resume,
            continuation,
        } => {
            encoder.tag(0x01)?;
            encode_fiber_member(encoder, member)?;
            encode_source_cursor(encoder, resume)?;
            match continuation {
                RelationalSourceContinuation::Expand(cursor) => {
                    encoder.tag(0x01)?;
                    encode_source_cursor(encoder, cursor)
                }
                RelationalSourceContinuation::Source(source) => {
                    encoder.tag(0x02)?;
                    encode_completed_source(encoder, source)
                }
            }
        }
        RelationalSourceAdvance::Exhausted {
            cursor,
            cardinality,
            receipt,
        } => {
            encoder.tag(0x02)?;
            encode_source_cursor(encoder, cursor)?;
            encoder.u128(*cardinality)?;
            encode_source_binding_receipt(encoder, receipt)
        }
    }
}

fn decode_source_advance(
    reader: &mut Reader<'_>,
    relation_id: RelationId,
) -> Result<RelationalSourceAdvance, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => {
            let member = decode_fiber_member(reader)?;
            let resume = decode_source_cursor(reader)?;
            let continuation = match reader.tag()? {
                0x01 => RelationalSourceContinuation::Expand(decode_source_cursor(reader)?),
                0x02 => RelationalSourceContinuation::Source(decode_completed_source(
                    reader,
                    relation_id,
                )?),
                tag => {
                    return Err(RelationalJournalCodecError::UnknownTag {
                        component: "source continuation",
                        tag,
                    });
                }
            };
            Ok(RelationalSourceAdvance::Yielded {
                member,
                resume,
                continuation,
            })
        }
        0x02 => Ok(RelationalSourceAdvance::Exhausted {
            cursor: decode_source_cursor(reader)?,
            cardinality: reader.u128()?,
            receipt: decode_source_binding_receipt(reader)?,
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "source advance",
            tag,
        }),
    }
}

fn encode_source_relation_receipt(
    encoder: &mut Encoder,
    receipt: &SourceRelationExhaustionReceipt,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(receipt.relation_id().bytes())?;
    encoder.digest(receipt.support_plan_root().bytes())?;
    encoder.u32(receipt.binding_count())?;
    encoder.digest(receipt.fiber_receipt_root().bytes())?;
    encoder.u128(receipt.fiber_receipt_count())?;
    encoder.digest(receipt.source_key_root().bytes())?;
    encoder.u128(receipt.source_key_count())?;
    encoder.digest(receipt.traversal_edge_root().bytes())?;
    encoder.u128(receipt.traversal_edge_count())
}

fn decode_source_relation_receipt(
    reader: &mut Reader<'_>,
) -> Result<SourceRelationExhaustionReceipt, RelationalJournalCodecError> {
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let support_plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let binding_count = reader.u32()?;
    Ok(SourceRelationExhaustionReceipt::restore_from_journal_codec(
        relation_id,
        support_plan_root,
        binding_count,
        SourceFiberReceiptSetRoot::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
        SourceKeySetRoot::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
        SourceTraversalEdgeRoot::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
    )?)
}

fn encode_successor_receipt(
    encoder: &mut Encoder,
    receipt: &SuccessorFiberExhaustionReceipt,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(receipt.version())?;
    encoder.digest(receipt.relation_id().bytes())?;
    encoder.digest(receipt.source_key().bytes())?;
    encoder.u128(receipt.terminal_ordinal())?;
    encoder.u128(receipt.emitted_row_count())?;
    encoder.digest(receipt.emitted_rows_commitment())
}

fn decode_successor_receipt(
    reader: &mut Reader<'_>,
) -> Result<SuccessorFiberExhaustionReceipt, RelationalJournalCodecError> {
    Ok(SuccessorFiberExhaustionReceipt::restore_from_journal_codec(
        reader.u32()?,
        RelationId::from_journal_codec_bytes(reader.digest()?),
        SourceKey::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
        reader.u128()?,
        reader.digest()?,
    )?)
}

fn encode_canonical_prefix(
    encoder: &mut Encoder,
    prefix: &CanonicalSourcePrefix,
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(prefix.values().len())?;
    for value in prefix.values() {
        encode_explore_value(encoder, value, 0)?;
    }
    Ok(())
}

fn decode_canonical_prefix(
    reader: &mut Reader<'_>,
) -> Result<CanonicalSourcePrefix, RelationalJournalCodecError> {
    let count = reader.collection_len("canonical source prefix")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(decode_explore_value(reader, 0)?);
    }
    Ok(CanonicalSourcePrefix::from_values(values)?)
}

fn encode_mechanism_endpoint(
    encoder: &mut Encoder,
    endpoint: MechanismEndpoint,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match endpoint {
        MechanismEndpoint::Before => 0x01,
        MechanismEndpoint::After => 0x02,
    })
}

fn decode_mechanism_endpoint(
    reader: &mut Reader<'_>,
) -> Result<MechanismEndpoint, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(MechanismEndpoint::Before),
        0x02 => Ok(MechanismEndpoint::After),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "mechanism work endpoint",
            tag,
        }),
    }
}

fn encode_work_spec(
    encoder: &mut Encoder,
    spec: &WorkNodeSpec,
) -> Result<(), RelationalJournalCodecError> {
    match spec {
        WorkNodeSpec::SourcePrefixReady {
            relation_id,
            binding_index,
            prefix,
        } => {
            encoder.tag(0x01)?;
            encoder.digest(relation_id.bytes())?;
            encoder.u32(*binding_index)?;
            encode_canonical_prefix(encoder, prefix)
        }
        WorkNodeSpec::SourceRowReady {
            relation_id,
            source_key,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(source_key.bytes())
        }
        WorkNodeSpec::CaseReady { case_id } => {
            encoder.tag(0x03)?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::SupportCellReady { cell_id } => {
            encoder.tag(0x04)?;
            encoder.digest(cell_id.bytes())
        }
        WorkNodeSpec::ExpandSourceBinding {
            relation_id,
            binding_index,
            prefix,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(relation_id.bytes())?;
            encoder.u32(*binding_index)?;
            encode_canonical_prefix(encoder, prefix)
        }
        WorkNodeSpec::ExpandSuccessors {
            relation_id,
            source_key,
        } => {
            encoder.tag(0x06)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(source_key.bytes())
        }
        WorkNodeSpec::EvaluateAdmission {
            admission_id,
            case_id,
        } => {
            encoder.tag(0x07)?;
            encoder.digest(admission_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::EvaluateFind {
            question_id,
            case_id,
        } => {
            encoder.tag(0x08)?;
            encoder.digest(question_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::ReduceCaseView { view_id, case_id } => {
            encoder.tag(0x09)?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::ReplayMechanismEndpoint {
            request_id,
            case_id,
            endpoint,
        } => {
            encoder.tag(0x0a)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(case_id.bytes())?;
            encode_mechanism_endpoint(encoder, *endpoint)
        }
        WorkNodeSpec::BuildMechanismIncidence {
            request_id,
            case_id,
        } => {
            encoder.tag(0x0b)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::ReduceMechanismIncidenceView {
            view_id,
            request_id,
            case_id,
        } => {
            encoder.tag(0x0c)?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        WorkNodeSpec::ResolveSupportObligation {
            cell_id,
            obligation_id,
        } => {
            encoder.tag(0x0d)?;
            encoder.digest(cell_id.bytes())?;
            encoder.digest(obligation_id.bytes())
        }
        WorkNodeSpec::MaterializeSupportCell { cell_id } => {
            encoder.tag(0x0e)?;
            encoder.digest(cell_id.bytes())
        }
    }
}

fn decode_work_spec(reader: &mut Reader<'_>) -> Result<WorkNodeSpec, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(WorkNodeSpec::SourcePrefixReady {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            binding_index: reader.u32()?,
            prefix: decode_canonical_prefix(reader)?,
        }),
        0x02 => Ok(WorkNodeSpec::SourceRowReady {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
        }),
        0x03 => Ok(WorkNodeSpec::CaseReady {
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x04 => Ok(WorkNodeSpec::SupportCellReady {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x05 => Ok(WorkNodeSpec::ExpandSourceBinding {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            binding_index: reader.u32()?,
            prefix: decode_canonical_prefix(reader)?,
        }),
        0x06 => Ok(WorkNodeSpec::ExpandSuccessors {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
        }),
        0x07 => Ok(WorkNodeSpec::EvaluateAdmission {
            admission_id: AdmissionId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x08 => Ok(WorkNodeSpec::EvaluateFind {
            question_id: QuestionId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x09 => Ok(WorkNodeSpec::ReduceCaseView {
            view_id: ViewId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0a => Ok(WorkNodeSpec::ReplayMechanismEndpoint {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            endpoint: decode_mechanism_endpoint(reader)?,
        }),
        0x0b => Ok(WorkNodeSpec::BuildMechanismIncidence {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0c => Ok(WorkNodeSpec::ReduceMechanismIncidenceView {
            view_id: ViewId::from_journal_codec_bytes(reader.digest()?),
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0d => Ok(WorkNodeSpec::ResolveSupportObligation {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
            obligation_id: SupportProofObligationId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0e => Ok(WorkNodeSpec::MaterializeSupportCell {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "work-node specification",
            tag,
        }),
    }
}

fn encode_work_completion(
    encoder: &mut Encoder,
    completion: &WorkCompletionRef,
) -> Result<(), RelationalJournalCodecError> {
    match completion {
        WorkCompletionRef::SourcePrefixReady {
            relation_id,
            binding_index,
            prefix,
        } => {
            encoder.tag(0x01)?;
            encoder.digest(relation_id.bytes())?;
            encoder.u32(*binding_index)?;
            encode_canonical_prefix(encoder, prefix)
        }
        WorkCompletionRef::SourceRowReady {
            relation_id,
            source_key,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(source_key.bytes())
        }
        WorkCompletionRef::CaseReady { case_id } => {
            encoder.tag(0x03)?;
            encoder.digest(case_id.bytes())
        }
        WorkCompletionRef::SupportCellReady { cell_id } => {
            encoder.tag(0x04)?;
            encoder.digest(cell_id.bytes())
        }
        WorkCompletionRef::SourceBindingExhausted {
            relation_id,
            binding_index,
            prefix,
            terminal_ordinal,
            receipt_id,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(relation_id.bytes())?;
            encoder.u32(*binding_index)?;
            encode_canonical_prefix(encoder, prefix)?;
            encoder.u128(*terminal_ordinal)?;
            encoder.digest(receipt_id.bytes())
        }
        WorkCompletionRef::SuccessorsSealed {
            relation_id,
            source_key,
            terminal_ordinal,
            receipt_id,
        } => {
            encoder.tag(0x06)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(source_key.bytes())?;
            encoder.u128(*terminal_ordinal)?;
            encoder.digest(receipt_id.bytes())
        }
        WorkCompletionRef::AdmissionDecided {
            admission_id,
            case_id,
            decision,
        } => {
            encoder.tag(0x07)?;
            encoder.digest(admission_id.bytes())?;
            encoder.digest(case_id.bytes())?;
            encode_admission_decision(encoder, *decision)
        }
        WorkCompletionRef::FindDecided {
            question_id,
            case_id,
            decision,
        } => {
            encoder.tag(0x08)?;
            encoder.digest(question_id.bytes())?;
            encoder.digest(case_id.bytes())?;
            encode_selection_decision(encoder, *decision)
        }
        WorkCompletionRef::DirectSupportEvidence {
            cell_id,
            obligation_id,
            evidence_id,
        } => {
            encoder.tag(0x09)?;
            encoder.digest(cell_id.bytes())?;
            encoder.digest(obligation_id.bytes())?;
            encoder.digest(evidence_id.bytes())
        }
        WorkCompletionRef::SupportObligationRefined {
            cell_id,
            obligation_id,
            refinement_id,
        } => {
            encoder.tag(0x0a)?;
            encoder.digest(cell_id.bytes())?;
            encoder.digest(obligation_id.bytes())?;
            encoder.digest(refinement_id.bytes())
        }
        WorkCompletionRef::SupportMaterializationExhausted {
            cell_id,
            cardinality_obligation_id,
            evidence_id,
        } => {
            encoder.tag(0x0b)?;
            encoder.digest(cell_id.bytes())?;
            encoder.digest(cardinality_obligation_id.bytes())?;
            encoder.digest(evidence_id.bytes())
        }
    }
}

fn decode_work_completion(
    reader: &mut Reader<'_>,
) -> Result<WorkCompletionRef, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(WorkCompletionRef::SourcePrefixReady {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            binding_index: reader.u32()?,
            prefix: decode_canonical_prefix(reader)?,
        }),
        0x02 => Ok(WorkCompletionRef::SourceRowReady {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
        }),
        0x03 => Ok(WorkCompletionRef::CaseReady {
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x04 => Ok(WorkCompletionRef::SupportCellReady {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x05 => Ok(WorkCompletionRef::SourceBindingExhausted {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            binding_index: reader.u32()?,
            prefix: decode_canonical_prefix(reader)?,
            terminal_ordinal: reader.u128()?,
            receipt_id: SourceBindingExhaustionReceiptId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x06 => Ok(WorkCompletionRef::SuccessorsSealed {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
            terminal_ordinal: reader.u128()?,
            receipt_id: SuccessorFiberExhaustionReceiptId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x07 => Ok(WorkCompletionRef::AdmissionDecided {
            admission_id: AdmissionId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            decision: decode_admission_decision(reader)?,
        }),
        0x08 => Ok(WorkCompletionRef::FindDecided {
            question_id: QuestionId::from_journal_codec_bytes(reader.digest()?),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            decision: decode_selection_decision(reader)?,
        }),
        0x09 => Ok(WorkCompletionRef::DirectSupportEvidence {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
            obligation_id: SupportProofObligationId::from_journal_codec_bytes(reader.digest()?),
            evidence_id: SupportCellEvidenceId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0a => Ok(WorkCompletionRef::SupportObligationRefined {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
            obligation_id: SupportProofObligationId::from_journal_codec_bytes(reader.digest()?),
            refinement_id: SupportObligationRefinementId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x0b => Ok(WorkCompletionRef::SupportMaterializationExhausted {
            cell_id: SupportCellId::from_journal_codec_bytes(reader.digest()?),
            cardinality_obligation_id: SupportProofObligationId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            evidence_id: SupportCellEvidenceId::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "work completion",
            tag,
        }),
    }
}

fn encode_materialization_cursor(
    encoder: &mut Encoder,
    cursor: &SupportMaterializationCursor,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(cursor.version())?;
    encoder.digest(cursor.cell_id().bytes())?;
    encoder.digest(cursor.materializer_id().bytes())?;
    encoder.u128(cursor.next_coordinate_ordinal())?;
    encoder.blob(cursor.checkpoint())
}

fn decode_materialization_cursor(
    reader: &mut Reader<'_>,
) -> Result<SupportMaterializationCursor, RelationalJournalCodecError> {
    Ok(SupportMaterializationCursor::restore_from_journal_codec(
        reader.u32()?,
        SupportCellId::from_journal_codec_bytes(reader.digest()?),
        SupportMaterializerId::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
        reader.blob()?,
    )?)
}

fn encode_compaction(
    encoder: &mut Encoder,
    receipt: WorkFrontierCompaction,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(receipt.maximum_nodes().get())?;
    encoder.u32(receipt.removed_nodes())?;
    encoder.digest(receipt.removed_ids_root())?;
    encoder.digest(receipt.before_root().bytes())?;
    encoder.digest(receipt.after_root().bytes())
}

fn decode_compaction(
    reader: &mut Reader<'_>,
) -> Result<WorkFrontierCompaction, RelationalJournalCodecError> {
    let maximum_nodes = NonZeroU32::new(reader.u32()?).ok_or(
        RelationalJournalCodecError::Malformed("zero work compaction limit"),
    )?;
    Ok(WorkFrontierCompaction::restore_from_journal_codec(
        maximum_nodes,
        reader.u32()?,
        reader.digest()?,
        reader.digest()?,
        reader.digest()?,
    )?)
}

fn encode_signature_id(
    encoder: &mut Encoder,
    id: MechanismSignatureId,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(id.request_id().bytes())?;
    encoder.digest(id.bytes())
}

fn decode_signature_id(
    reader: &mut Reader<'_>,
) -> Result<MechanismSignatureId, RelationalJournalCodecError> {
    Ok(MechanismSignatureId::from_journal_codec_parts(
        MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
        reader.digest()?,
    ))
}

fn encode_result_value(
    encoder: &mut Encoder,
    value: &ResultValue,
) -> Result<(), RelationalJournalCodecError> {
    match value {
        ResultValue::Value(value) => {
            encoder.tag(0x01)?;
            encode_explore_value(encoder, value, 0)
        }
        ResultValue::CaseId(id) => {
            encoder.tag(0x02)?;
            encoder.digest(id.bytes())
        }
        ResultValue::TransitionId(id) => {
            encoder.tag(0x03)?;
            encoder.digest(id.bytes())
        }
        ResultValue::SignatureId(id) => {
            encoder.tag(0x04)?;
            encode_signature_id(encoder, *id)
        }
        ResultValue::StructuralMechanismId(id) => {
            encoder.tag(0x05)?;
            encoder.digest(id.bytes())
        }
        ResultValue::ExecutionProfileId(id) => {
            encoder.tag(0x06)?;
            encoder.digest(id.bytes())
        }
    }
}

fn decode_result_value(
    reader: &mut Reader<'_>,
) -> Result<ResultValue, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ResultValue::Value(decode_explore_value(reader, 0)?)),
        0x02 => Ok(ResultValue::CaseId(
            RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x03 => Ok(ResultValue::TransitionId(TransitionId::from_bytes(
            reader.digest()?,
        ))),
        0x04 => Ok(ResultValue::SignatureId(decode_signature_id(reader)?)),
        0x05 => Ok(ResultValue::StructuralMechanismId(
            StructuralMechanismId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x06 => Ok(ResultValue::ExecutionProfileId(
            ExecutionProfileId::from_journal_codec_bytes(reader.digest()?),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result value",
            tag,
        }),
    }
}

fn encode_result_values(
    encoder: &mut Encoder,
    values: &[ResultValue],
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(values.len())?;
    for value in values {
        encode_result_value(encoder, value)?;
    }
    Ok(())
}

fn decode_result_values(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[ResultValue]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(decode_result_value(reader)?);
    }
    Ok(values.into_boxed_slice())
}

fn encode_certified_source_summary_artifact(
    encoder: &mut Encoder,
    artifact: &RelationalCertifiedSourceSummaryArtifact,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(artifact.version())?;
    encoder.digest(artifact.artifact_id().bytes())?;
    encoder.digest(artifact.analysis_plan_root().bytes())?;
    encoder.digest(artifact.semantic_spec_digest().bytes())?;
    encoder.digest(artifact.view_id().bytes())?;
    encoder.digest(artifact.spec_root().bytes())?;
    encoder.digest(artifact.relation_id().bytes())?;
    encoder.digest(artifact.source_plan_root().bytes())?;
    encoder.digest(artifact.source_certificate_id())?;
    encoder.digest(artifact.source_population_root().bytes())?;
    encoder.digest(artifact.source_cell_id().bytes())?;
    encoder.digest(artifact.source_materializer_id().bytes())?;
    match artifact.version() {
        RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 => {
            let shape = artifact
                .direct_shape()
                .ok_or(RelationalJournalCodecError::Malformed(
                    "v1 certified source summary lost its direct shape",
                ))?;
            encoder.digest(shape.context_stage_id().bytes())?;
            encoder.digest(shape.before_stage_id().bytes())?;
            encoder.digest(shape.before_dimension_id().bytes())?;
            encoder.digest(shape.before_factor_cell_id().bytes())?;
        }
        RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION => {
            let shape = artifact
                .product_shape()
                .ok_or(RelationalJournalCodecError::Malformed(
                    "v2 certified source summary lost its product shape",
                ))?;
            encoder.digest(shape.summary_certificate_id())?;
            encoder.digest(shape.compiler_projection_certificate_id())?;
            encoder.digest(shape.factor_binding_root())?;
        }
        _ => {
            return Err(RelationalJournalCodecError::Malformed(
                "unsupported certified source summary version",
            ));
        }
    }
    encoder.u128(artifact.exact_cardinality())?;
    encoder.digest(artifact.certified_input_root().bytes())?;
    if artifact.version() == RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 {
        let [group] = artifact.groups() else {
            return Err(RelationalJournalCodecError::Malformed(
                "v1 certified source summary has more than one group",
            ));
        };
        return encode_result_values(encoder, group.group_values());
    }
    encoder.collection_len(artifact.groups().len())?;
    for group in artifact.groups() {
        encode_result_values(encoder, group.group_values())?;
        encoder.u128(group.exact_member_count())?;
        encoder.collection_len(group.exact_distinct_counts().len())?;
        for count in group.exact_distinct_counts() {
            encoder.u128(*count)?;
        }
    }
    Ok(())
}

fn decode_certified_source_summary_artifact(
    reader: &mut Reader<'_>,
) -> Result<RelationalCertifiedSourceSummaryArtifact, RelationalJournalCodecError> {
    let version = reader.u32()?;
    let artifact_id =
        RelationalCertifiedSourceSummaryArtifactId::from_journal_codec_bytes(reader.digest()?);
    let analysis_plan_root = RelationalAnalysisPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let semantic_spec_digest =
        RelationalResultSpecDigest::from_journal_codec_bytes(reader.digest()?);
    let view_id = ViewId::from_journal_codec_bytes(reader.digest()?);
    let spec_root = ResultViewSpecRoot::from_journal_codec_bytes(reader.digest()?);
    let relation_id = RelationId::from_journal_codec_bytes(reader.digest()?);
    let source_plan_root = RelationalSupportPlanRoot::from_journal_codec_bytes(reader.digest()?);
    let source_certificate_id = reader.digest()?;
    let source_population_root =
        CertifiedSourcePopulationRoot::from_journal_codec_bytes(reader.digest()?);
    let source_cell_id = SupportCellId::from_journal_codec_bytes(reader.digest()?);
    let source_materializer_id = SupportMaterializerId::from_journal_codec_bytes(reader.digest()?);

    match version {
        RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 => Ok(
            RelationalCertifiedSourceSummaryArtifact::restore_v1_from_journal_codec(
                version,
                artifact_id,
                analysis_plan_root,
                semantic_spec_digest,
                view_id,
                spec_root,
                relation_id,
                source_plan_root,
                source_certificate_id,
                source_population_root,
                source_cell_id,
                source_materializer_id,
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                RelationalBindingStageId::from_journal_codec_bytes(reader.digest()?),
                RelationalDimensionId::from_journal_codec_bytes(reader.digest()?),
                SupportCellId::from_journal_codec_bytes(reader.digest()?),
                reader.u128()?,
                CertifiedResultInputRoot::from_journal_codec_bytes(reader.digest()?),
                decode_result_values(reader, "certified source summary group values")?,
            )?,
        ),
        RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION => {
            let summary_certificate_id = reader.digest()?;
            let compiler_projection_certificate_id = reader.digest()?;
            let factor_binding_root = reader.digest()?;
            let exact_cardinality = reader.u128()?;
            let certified_input_root =
                CertifiedResultInputRoot::from_journal_codec_bytes(reader.digest()?);
            let group_count = reader.collection_len("certified source summary groups")?;
            let compact_group_limit =
                usize::try_from(RELATIONAL_CERTIFIED_SOURCE_SUMMARY_MAX_GROUPS)
                    .map_err(|_| RelationalJournalCodecError::LengthOverflow)?;
            if group_count > compact_group_limit {
                return Err(RelationalJournalCodecError::CollectionTooLarge {
                    items: group_count,
                    limit: compact_group_limit,
                });
            }
            let mut groups = Vec::new();
            groups.try_reserve_exact(group_count).map_err(|_| {
                RelationalJournalCodecError::AllocationFailed {
                    requested: group_count,
                }
            })?;
            for _ in 0..group_count {
                let group_values =
                    decode_result_values(reader, "certified source summary group values")?;
                let exact_member_count = reader.u128()?;
                let distinct_count =
                    reader.collection_len("certified source summary distinct counts")?;
                let mut exact_distinct_counts = Vec::new();
                exact_distinct_counts
                    .try_reserve_exact(distinct_count)
                    .map_err(|_| RelationalJournalCodecError::AllocationFailed {
                        requested: distinct_count,
                    })?;
                for _ in 0..distinct_count {
                    exact_distinct_counts.push(reader.u128()?);
                }
                groups.push(CertifiedResultGroupSummary::new(
                    group_values,
                    exact_member_count,
                    exact_distinct_counts.into_boxed_slice(),
                ));
            }
            Ok(
                RelationalCertifiedSourceSummaryArtifact::restore_v2_from_journal_codec(
                    version,
                    artifact_id,
                    analysis_plan_root,
                    semantic_spec_digest,
                    view_id,
                    spec_root,
                    relation_id,
                    source_plan_root,
                    source_certificate_id,
                    source_population_root,
                    source_cell_id,
                    source_materializer_id,
                    summary_certificate_id,
                    compiler_projection_certificate_id,
                    factor_binding_root,
                    exact_cardinality,
                    certified_input_root,
                    groups.into_boxed_slice(),
                )?,
            )
        }
        _ => Err(RelationalJournalCodecError::Malformed(
            "unsupported certified source summary version",
        )),
    }
}

fn encode_input_row_id(
    encoder: &mut Encoder,
    row_id: ResultViewInputRowId,
) -> Result<(), RelationalJournalCodecError> {
    match row_id {
        ResultViewInputRowId::Source(source_key) => {
            encoder.tag(0x03)?;
            encoder.digest(source_key.bytes())
        }
        ResultViewInputRowId::Case(case_id) => {
            encoder.tag(0x01)?;
            encoder.digest(case_id.bytes())
        }
        ResultViewInputRowId::Incidence(incidence) => {
            encoder.tag(0x02)?;
            encoder.digest(incidence.case_id().bytes())?;
            encoder.digest(incidence.transition_id().bytes())?;
            encode_signature_id(encoder, incidence.signature_id())
        }
    }
}

fn decode_input_row_id(
    reader: &mut Reader<'_>,
) -> Result<ResultViewInputRowId, RelationalJournalCodecError> {
    match reader.tag()? {
        0x03 => Ok(ResultViewInputRowId::Source(
            SourceKey::from_journal_codec_bytes(reader.digest()?),
        )),
        0x01 => Ok(ResultViewInputRowId::Case(
            RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x02 => Ok(ResultViewInputRowId::Incidence(
            MechanismIncidenceRowId::new(
                RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
                TransitionId::from_bytes(reader.digest()?),
                decode_signature_id(reader)?,
            ),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result input row ID",
            tag,
        }),
    }
}

fn encode_input_kind(
    encoder: &mut Encoder,
    kind: ResultViewInputKind,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match kind {
        ResultViewInputKind::Source => 0x03,
        ResultViewInputKind::Case => 0x01,
        ResultViewInputKind::Incidence => 0x02,
    })
}

fn decode_input_kind(
    reader: &mut Reader<'_>,
) -> Result<ResultViewInputKind, RelationalJournalCodecError> {
    match reader.tag()? {
        0x03 => Ok(ResultViewInputKind::Source),
        0x01 => Ok(ResultViewInputKind::Case),
        0x02 => Ok(ResultViewInputKind::Incidence),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result input kind",
            tag,
        }),
    }
}

fn encode_strings(
    encoder: &mut Encoder,
    strings: &[Box<str>],
) -> Result<(), RelationalJournalCodecError> {
    encoder.collection_len(strings.len())?;
    for string in strings {
        encoder.string(string)?;
    }
    Ok(())
}

fn decode_strings(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[Box<str>]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut strings = Vec::new();
    strings
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        strings.push(reader.string()?);
    }
    Ok(strings.into_boxed_slice())
}

fn encode_direction(
    encoder: &mut Encoder,
    direction: ExploreOptimizeDirection,
) -> Result<(), RelationalJournalCodecError> {
    encoder.tag(match direction {
        ExploreOptimizeDirection::Minimize => 0x01,
        ExploreOptimizeDirection::Maximize => 0x02,
    })
}

fn decode_direction(
    reader: &mut Reader<'_>,
) -> Result<ExploreOptimizeDirection, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ExploreOptimizeDirection::Minimize),
        0x02 => Ok(ExploreOptimizeDirection::Maximize),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "optimization direction",
            tag,
        }),
    }
}

fn encode_result_spec(
    encoder: &mut Encoder,
    spec: &ResultViewSpec,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(spec.view_id().bytes())?;
    encode_input_kind(encoder, spec.input_kind())?;
    match spec.grain() {
        ResultViewGrain::EachCase => encoder.tag(0x01)?,
        ResultViewGrain::EachIncidence => encoder.tag(0x02)?,
        ResultViewGrain::GroupAll => encoder.tag(0x03)?,
        ResultViewGrain::GroupBy { field_names } => {
            encoder.tag(0x04)?;
            encode_strings(encoder, field_names)?;
        }
    }
    encode_strings(encoder, spec.measure_names())?;
    encode_strings(encoder, spec.aggregate_names())?;
    encode_strings(encoder, spec.projection_names())?;
    match spec.having() {
        None => encoder.tag(0x00)?,
        Some(ResultViewHaving::Varies { measure_index }) => {
            encoder.tag(0x01)?;
            encoder.usize(measure_index)?;
        }
    }
    match spec.choice() {
        None => encoder.tag(0x00)?,
        Some(ResultViewChoice::Optimize {
            cardinality,
            direction,
        }) => {
            encoder.tag(0x01)?;
            encoder.tag(match cardinality {
                ExploreChooseCardinality::One => 0x01,
                ExploreChooseCardinality::All => 0x02,
            })?;
            encode_direction(encoder, *direction)?;
        }
        Some(ResultViewChoice::Pareto { directions }) => {
            encoder.tag(0x02)?;
            encoder.collection_len(directions.len())?;
            for direction in directions {
                encode_direction(encoder, *direction)?;
            }
        }
    }
    Ok(())
}

fn decode_result_spec(
    reader: &mut Reader<'_>,
) -> Result<ResultViewSpec, RelationalJournalCodecError> {
    let view_id = ViewId::from_journal_codec_bytes(reader.digest()?);
    let input_kind = decode_input_kind(reader)?;
    let grain = match reader.tag()? {
        0x01 => ResultViewGrain::EachCase,
        0x02 => ResultViewGrain::EachIncidence,
        0x03 => ResultViewGrain::GroupAll,
        0x04 => ResultViewGrain::GroupBy {
            field_names: decode_strings(reader, "result group fields")?,
        },
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "result grain",
                tag,
            });
        }
    };
    let measure_names = decode_strings(reader, "result measures")?;
    let aggregate_names = decode_strings(reader, "result aggregates")?;
    let projection_names = decode_strings(reader, "result projections")?;
    let having = match reader.tag()? {
        0x00 => None,
        0x01 => Some(ResultViewHaving::Varies {
            measure_index: reader.usize("having measure index")?,
        }),
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "result having",
                tag,
            });
        }
    };
    let choice = match reader.tag()? {
        0x00 => None,
        0x01 => {
            let cardinality = match reader.tag()? {
                0x01 => ExploreChooseCardinality::One,
                0x02 => ExploreChooseCardinality::All,
                tag => {
                    return Err(RelationalJournalCodecError::UnknownTag {
                        component: "choice cardinality",
                        tag,
                    });
                }
            };
            Some(ResultViewChoice::Optimize {
                cardinality,
                direction: decode_direction(reader)?,
            })
        }
        0x02 => {
            let count = reader.collection_len("Pareto directions")?;
            let mut directions = Vec::new();
            directions
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                directions.push(decode_direction(reader)?);
            }
            Some(ResultViewChoice::Pareto {
                directions: directions.into_boxed_slice(),
            })
        }
        tag => {
            return Err(RelationalJournalCodecError::UnknownTag {
                component: "result choice",
                tag,
            });
        }
    };
    Ok(ResultViewSpec::new(
        view_id,
        input_kind,
        grain,
        measure_names,
        aggregate_names,
        projection_names,
        having,
        choice,
    )?)
}

fn encode_contribution(
    encoder: &mut Encoder,
    contribution: &EvaluatedResultContribution,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(contribution.view_id().bytes())?;
    encode_input_row_id(encoder, contribution.row_id())?;
    encode_result_values(encoder, contribution.group_values())?;
    encode_result_values(encoder, contribution.measures())?;
    encode_result_values(encoder, contribution.distinct_arguments())
}

fn decode_contribution(
    reader: &mut Reader<'_>,
) -> Result<EvaluatedResultContribution, RelationalJournalCodecError> {
    Ok(EvaluatedResultContribution::new(
        ViewId::from_journal_codec_bytes(reader.digest()?),
        decode_input_row_id(reader)?,
        decode_result_values(reader, "result group values")?,
        decode_result_values(reader, "result measures")?,
        decode_result_values(reader, "result distinct arguments")?,
    ))
}

fn encode_optional_result_values<'a>(
    encoder: &mut Encoder,
    len: usize,
    values: impl ExactSizeIterator<Item = Option<&'a ResultValue>>,
) -> Result<(), RelationalJournalCodecError> {
    debug_assert_eq!(len, values.len());
    encoder.collection_len(len)?;
    for value in values {
        encoder.bool(value.is_some())?;
        if let Some(value) = value {
            encode_result_value(encoder, value)?;
        }
    }
    Ok(())
}

fn decode_optional_result_values(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[Option<ResultValue>]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(if reader.bool()? {
            Some(decode_result_value(reader)?)
        } else {
            None
        });
    }
    Ok(values.into_boxed_slice())
}

fn encode_optional_i64s<'a>(
    encoder: &mut Encoder,
    len: usize,
    values: impl ExactSizeIterator<Item = Option<&'a i64>>,
) -> Result<(), RelationalJournalCodecError> {
    debug_assert_eq!(len, values.len());
    encoder.collection_len(len)?;
    for value in values {
        encoder.bool(value.is_some())?;
        if let Some(value) = value {
            encoder.i64(*value)?;
        }
    }
    Ok(())
}

fn decode_optional_i64s(
    reader: &mut Reader<'_>,
    component: &'static str,
) -> Result<Box<[Option<i64>]>, RelationalJournalCodecError> {
    let count = reader.collection_len(component)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
    for _ in 0..count {
        values.push(if reader.bool()? {
            Some(reader.i64()?)
        } else {
            None
        });
    }
    Ok(values.into_boxed_slice())
}

fn encode_result_evidence_record(
    encoder: &mut Encoder,
    record: &RelationalResultEvidenceRecord,
) -> Result<(), RelationalJournalCodecError> {
    encode_contribution(encoder, record.contribution())?;
    encode_optional_result_values(
        encoder,
        record.early_select_len(),
        record.early_select_iter(),
    )?;
    encode_optional_i64s(
        encoder,
        record.early_objectives_len(),
        record.early_objectives_iter(),
    )
}

fn decode_result_evidence_record(
    reader: &mut Reader<'_>,
) -> Result<RelationalResultEvidenceRecord, RelationalJournalCodecError> {
    Ok(RelationalResultEvidenceRecord::restore_from_journal_codec(
        decode_contribution(reader)?,
        decode_optional_result_values(reader, "early SELECT values")?,
        decode_optional_i64s(reader, "early objective values")?,
    ))
}

fn encode_result_count(
    encoder: &mut Encoder,
    count: ResultViewCount,
) -> Result<(), RelationalJournalCodecError> {
    match count {
        ResultViewCount::LowerBound(value) => {
            encoder.tag(0x01)?;
            encoder.u128(value)
        }
        ResultViewCount::Provisional(value) => {
            encoder.tag(0x02)?;
            encoder.u128(value)
        }
        ResultViewCount::Exact(value) => {
            encoder.tag(0x03)?;
            encoder.u128(value)
        }
    }
}

fn decode_result_count(
    reader: &mut Reader<'_>,
) -> Result<ResultViewCount, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ResultViewCount::LowerBound(reader.u128()?)),
        0x02 => Ok(ResultViewCount::Provisional(reader.u128()?)),
        0x03 => Ok(ResultViewCount::Exact(reader.u128()?)),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result count",
            tag,
        }),
    }
}

fn encode_output_row(
    encoder: &mut Encoder,
    row: &ResultOutputRow,
) -> Result<(), RelationalJournalCodecError> {
    encode_input_row_id(encoder, row.row_id())?;
    encode_result_values(encoder, row.values())
}

fn decode_output_row(
    reader: &mut Reader<'_>,
) -> Result<ResultOutputRow, RelationalJournalCodecError> {
    Ok(ResultOutputRow::from_journal_codec_parts(
        decode_input_row_id(reader)?,
        decode_result_values(reader, "result output values")?,
    ))
}

fn encode_group_disposition(
    encoder: &mut Encoder,
    disposition: ResultGroupDisposition,
) -> Result<(), RelationalJournalCodecError> {
    match disposition {
        ResultGroupDisposition::Provisional {
            currently_passes_having,
        } => {
            encoder.tag(0x01)?;
            encoder.bool(currently_passes_having)
        }
        ResultGroupDisposition::ExactIncluded => encoder.tag(0x02),
        ResultGroupDisposition::ExactExcluded => encoder.tag(0x03),
    }
}

fn decode_group_disposition(
    reader: &mut Reader<'_>,
) -> Result<ResultGroupDisposition, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ResultGroupDisposition::Provisional {
            currently_passes_having: reader.bool()?,
        }),
        0x02 => Ok(ResultGroupDisposition::ExactIncluded),
        0x03 => Ok(ResultGroupDisposition::ExactExcluded),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result group disposition",
            tag,
        }),
    }
}

fn encode_result_counts(
    encoder: &mut Encoder,
    counts: ResultViewCounts,
) -> Result<(), RelationalJournalCodecError> {
    encode_result_count(encoder, counts.input_rows())?;
    encoder.bool(counts.groups().is_some())?;
    if let Some(groups) = counts.groups() {
        encode_result_count(encoder, groups)?;
    }
    encoder.bool(counts.output_groups().is_some())?;
    if let Some(output_groups) = counts.output_groups() {
        encode_result_count(encoder, output_groups)?;
    }
    encode_result_count(encoder, counts.output_rows())
}

fn decode_result_counts(
    reader: &mut Reader<'_>,
) -> Result<ResultViewCounts, RelationalJournalCodecError> {
    let input_rows = decode_result_count(reader)?;
    let groups = if reader.bool()? {
        Some(decode_result_count(reader)?)
    } else {
        None
    };
    let output_groups = if reader.bool()? {
        Some(decode_result_count(reader)?)
    } else {
        None
    };
    Ok(ResultViewCounts::from_journal_codec_parts(
        input_rows,
        groups,
        output_groups,
        decode_result_count(reader)?,
    ))
}

fn encode_projection_group(
    encoder: &mut Encoder,
    group: &ResultProjectionGroup,
) -> Result<(), RelationalJournalCodecError> {
    encode_result_values(encoder, group.key().values())?;
    encode_result_count(encoder, group.member_count())?;
    encoder.bool(group.observed_having_varies().is_some())?;
    if let Some(value) = group.observed_having_varies() {
        encoder.bool(value)?;
    }
    encode_group_disposition(encoder, group.disposition())?;
    encoder.collection_len(group.aggregates().len())?;
    for aggregate in group.aggregates() {
        encoder.string(aggregate.name())?;
        encode_result_count(encoder, aggregate.count())?;
    }
    encoder.bool(group.projected_values().is_some())?;
    if let Some(values) = group.projected_values() {
        encode_result_values(encoder, values)?;
    }
    encoder.u128(group.chosen_row_count())
}

fn decode_projection_group(
    reader: &mut Reader<'_>,
) -> Result<ResultProjectionGroup, RelationalJournalCodecError> {
    let key = ResultGroupKey::from_journal_codec_values(decode_result_values(
        reader,
        "result projection group key",
    )?);
    let member_count = decode_result_count(reader)?;
    let observed_having_varies = if reader.bool()? {
        Some(reader.bool()?)
    } else {
        None
    };
    let disposition = decode_group_disposition(reader)?;
    let aggregate_count = reader.collection_len("result projection group aggregates")?;
    let mut aggregates = Vec::new();
    aggregates.try_reserve_exact(aggregate_count).map_err(|_| {
        RelationalJournalCodecError::AllocationFailed {
            requested: aggregate_count,
        }
    })?;
    for _ in 0..aggregate_count {
        aggregates.push(ResultCountDistinctSnapshot::from_journal_codec_parts(
            reader.string()?,
            decode_result_count(reader)?,
        ));
    }
    let projected_values = if reader.bool()? {
        Some(decode_result_values(
            reader,
            "result projection group values",
        )?)
    } else {
        None
    };
    Ok(ResultProjectionGroup::restore_from_journal_codec(
        key,
        member_count,
        observed_having_varies,
        disposition,
        aggregates.into_boxed_slice(),
        projected_values,
        reader.u128()?,
    ))
}

fn encode_projection_record(
    encoder: &mut Encoder,
    record: &ResultProjectionRecord,
) -> Result<(), RelationalJournalCodecError> {
    match record {
        ResultProjectionRecord::Row(row) => {
            encoder.tag(0x01)?;
            encode_output_row(encoder, row)
        }
        ResultProjectionRecord::Group(group) => {
            encoder.tag(0x02)?;
            encode_projection_group(encoder, group)
        }
        ResultProjectionRecord::ChosenRow { group_key, row } => {
            encoder.tag(0x03)?;
            encode_result_values(encoder, group_key.values())?;
            encode_output_row(encoder, row)
        }
    }
}

fn decode_projection_record(
    reader: &mut Reader<'_>,
) -> Result<ResultProjectionRecord, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(ResultProjectionRecord::Row(decode_output_row(reader)?)),
        0x02 => Ok(ResultProjectionRecord::Group(decode_projection_group(
            reader,
        )?)),
        0x03 => Ok(ResultProjectionRecord::ChosenRow {
            group_key: ResultGroupKey::from_journal_codec_values(decode_result_values(
                reader,
                "result projection chosen-row group key",
            )?),
            row: decode_output_row(reader)?,
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result projection record",
            tag,
        }),
    }
}

fn encode_projection_closure(
    encoder: &mut Encoder,
    closure: ResultProjectionClosure,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(closure.view_id().bytes())?;
    encoder.digest(closure.spec_root().bytes())?;
    encoder.digest(closure.projection_root().bytes())?;
    encoder.u128(closure.record_count())?;
    encode_result_counts(encoder, closure.counts())?;
    encoder.digest(closure.result_root().bytes())
}

fn decode_projection_closure(
    reader: &mut Reader<'_>,
) -> Result<ResultProjectionClosure, RelationalJournalCodecError> {
    Ok(ResultProjectionClosure::restore_from_journal_codec(
        ViewId::from_journal_codec_bytes(reader.digest()?),
        ResultViewSpecRoot::from_journal_codec_bytes(reader.digest()?),
        ResultProjectionRoot::from_journal_codec_bytes(reader.digest()?),
        reader.u128()?,
        decode_result_counts(reader)?,
        ResultViewRoot::from_journal_codec_bytes(reader.digest()?),
    ))
}

fn encode_result_upstream(
    encoder: &mut Encoder,
    upstream: ResultEvidenceUpstreamRoot,
) -> Result<(), RelationalJournalCodecError> {
    match upstream {
        ResultEvidenceUpstreamRoot::Sources {
            relation_id,
            source_key_root,
        } => {
            encoder.tag(0x04)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(source_key_root.bytes())
        }
        ResultEvidenceUpstreamRoot::CertifiedSources {
            relation_id,
            population_root,
            summary_artifact_id,
            certified_input_root,
            exact_cardinality,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(relation_id.bytes())?;
            encoder.digest(population_root.bytes())?;
            encoder.digest(summary_artifact_id.bytes())?;
            encoder.digest(certified_input_root.bytes())?;
            encoder.u128(exact_cardinality)
        }
        ResultEvidenceUpstreamRoot::Selected {
            question_id,
            content_root,
        } => {
            encoder.tag(0x01)?;
            encoder.digest(question_id.bytes())?;
            encoder.digest(content_root.bytes())
        }
        ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
            question_id,
            population_root,
            exact_cardinality,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(question_id.bytes())?;
            encoder.digest(population_root.bytes())?;
            encoder.u128(exact_cardinality)
        }
        ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id,
            completed_root,
        } => {
            encoder.tag(0x03)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(completed_root.bytes())
        }
        ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
            request_id,
            completed_root,
            structural_root,
        } => {
            encoder.tag(0x06)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(completed_root.bytes())?;
            encoder.digest(structural_root.bytes())
        }
    }
}

fn decode_result_upstream(
    reader: &mut Reader<'_>,
) -> Result<ResultEvidenceUpstreamRoot, RelationalJournalCodecError> {
    match reader.tag()? {
        0x04 => Ok(ResultEvidenceUpstreamRoot::Sources {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            source_key_root: SourceKeySetRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x05 => Ok(ResultEvidenceUpstreamRoot::CertifiedSources {
            relation_id: RelationId::from_journal_codec_bytes(reader.digest()?),
            population_root: CertifiedSourcePopulationRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
            summary_artifact_id:
                RelationalCertifiedSourceSummaryArtifactId::from_journal_codec_bytes(
                    reader.digest()?,
                ),
            certified_input_root: CertifiedResultInputRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
            exact_cardinality: reader.u128()?,
        }),
        0x01 => Ok(ResultEvidenceUpstreamRoot::Selected {
            question_id: QuestionId::from_journal_codec_bytes(reader.digest()?),
            content_root: QuestionContentRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x02 => Ok(ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
            question_id: QuestionId::from_journal_codec_bytes(reader.digest()?),
            population_root: CertifiedSelectedPopulationRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
            exact_cardinality: reader.u128()?,
        }),
        0x03 => Ok(ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            completed_root: MechanismIncidenceRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x06 => Ok(ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            completed_root: MechanismIncidenceRoot::from_journal_codec_bytes(reader.digest()?),
            structural_root: StructuralQuotientClosureRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "result upstream root",
            tag,
        }),
    }
}

fn encode_result_input_seal(
    encoder: &mut Encoder,
    seal: RelationalResultInputSeal,
) -> Result<(), RelationalJournalCodecError> {
    encode_result_upstream(encoder, seal.upstream())?;
    let coverage = seal.coverage();
    encode_input_kind(encoder, coverage.input_kind())?;
    encoder.u128(coverage.row_count())?;
    encoder.digest(coverage.row_set_root().bytes())
}

fn decode_result_input_seal(
    reader: &mut Reader<'_>,
) -> Result<RelationalResultInputSeal, RelationalJournalCodecError> {
    Ok(RelationalResultInputSeal::restore_from_journal_codec(
        decode_result_upstream(reader)?,
        decode_input_kind(reader)?,
        reader.u128()?,
        ResultInputCoverageRoot::from_journal_codec_bytes(reader.digest()?),
    )?)
}

fn encode_selected_authority(
    encoder: &mut Encoder,
    authority: RelationalSelectedPopulationAuthority,
) -> Result<(), RelationalJournalCodecError> {
    match authority {
        RelationalSelectedPopulationAuthority::ExtensionalQuestion { content_root } => {
            encoder.tag(0x01)?;
            encoder.digest(content_root.bytes())
        }
        RelationalSelectedPopulationAuthority::CertifiedSupport {
            population_root,
            exact_cardinality,
        } => {
            encoder.tag(0x02)?;
            encoder.digest(population_root.bytes())?;
            encoder.u128(exact_cardinality)
        }
    }
}

fn decode_selected_authority(
    reader: &mut Reader<'_>,
) -> Result<RelationalSelectedPopulationAuthority, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalSelectedPopulationAuthority::ExtensionalQuestion {
            content_root: QuestionContentRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x02 => Ok(RelationalSelectedPopulationAuthority::CertifiedSupport {
            population_root: CertifiedSelectedPopulationRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
            exact_cardinality: reader.u128()?,
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "selected population authority",
            tag,
        }),
    }
}

fn encode_selected_question_seal(
    encoder: &mut Encoder,
    seal: RelationalSelectedQuestionSeal,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(seal.version())?;
    encoder.digest(seal.question_id().bytes())?;
    encode_selected_authority(encoder, seal.authority())?;
    encode_result_input_seal(encoder, seal.result_input_seal())?;
    encoder.digest(seal.mechanism_target().root().bytes())?;
    encoder.u128(seal.mechanism_target().count())
}

fn decode_selected_question_seal(
    reader: &mut Reader<'_>,
) -> Result<RelationalSelectedQuestionSeal, RelationalJournalCodecError> {
    Ok(RelationalSelectedQuestionSeal::restore_from_journal_codec(
        reader.u32()?,
        QuestionId::from_journal_codec_bytes(reader.digest()?),
        decode_selected_authority(reader)?,
        decode_result_input_seal(reader)?,
        MechanismTargetCaseSetCommitment::restore_from_journal_codec(
            reader.digest()?,
            reader.u128()?,
        ),
    )?)
}

fn encode_mechanism_artifact_claim(
    encoder: &mut Encoder,
    claim: RelationalMechanismArtifactClaim,
) -> Result<(), RelationalJournalCodecError> {
    match claim {
        RelationalMechanismArtifactClaim::Signature {
            request_id,
            signature_id,
        } => {
            encoder.tag(0x01)?;
            encoder.digest(request_id.bytes())?;
            encode_signature_id(encoder, signature_id)
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
            encoder.tag(0x02)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(observation_id.bytes())?;
            encoder.digest(observation_digest.bytes())?;
            encoder.digest(replay_observation_id.bytes())?;
            encoder.digest(case_id.bytes())?;
            encoder.digest(transition_id.bytes())?;
            encode_signature_id(encoder, signature_id)?;
            encoder.digest(replay_receipt_id.bytes())
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
            encoder.tag(0x03)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(observation_id.bytes())?;
            encoder.digest(observation_digest.bytes())?;
            encoder.digest(replay_observation_id.bytes())?;
            encoder.digest(case_id.bytes())?;
            encoder.digest(transition_id.bytes())?;
            encoder.digest(reason_id.bytes())
        }
        RelationalMechanismArtifactClaim::StructuralQuotient {
            request_id,
            raw_signature_id,
            structural_mechanism_id,
            execution_profile_id,
        } => {
            encoder.tag(0x04)?;
            encoder.digest(request_id.bytes())?;
            encode_signature_id(encoder, raw_signature_id)?;
            encoder.digest(structural_mechanism_id.bytes())?;
            encoder.digest(execution_profile_id.bytes())
        }
    }
}

fn decode_mechanism_artifact_claim(
    reader: &mut Reader<'_>,
) -> Result<RelationalMechanismArtifactClaim, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalMechanismArtifactClaim::Signature {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            signature_id: decode_signature_id(reader)?,
        }),
        0x02 => Ok(RelationalMechanismArtifactClaim::Incidence {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            observation_id: RelationalMechanismObservationId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            observation_digest: RelationalMechanismObservationDigest::from_journal_codec_bytes(
                reader.digest()?,
            ),
            replay_observation_id: RelationalMechanismReplayObservationId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            transition_id: TransitionId::from_bytes(reader.digest()?),
            signature_id: decode_signature_id(reader)?,
            replay_receipt_id: RelationalMechanismReplayReceiptId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x03 => Ok(RelationalMechanismArtifactClaim::Unavailable {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            observation_id: RelationalMechanismObservationId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            observation_digest: RelationalMechanismObservationDigest::from_journal_codec_bytes(
                reader.digest()?,
            ),
            replay_observation_id: RelationalMechanismReplayObservationId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            transition_id: TransitionId::from_bytes(reader.digest()?),
            reason_id: MechanismUnavailableReasonId::from_journal_codec_bytes(reader.digest()?),
        }),
        0x04 => Ok(RelationalMechanismArtifactClaim::StructuralQuotient {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            raw_signature_id: decode_signature_id(reader)?,
            structural_mechanism_id: StructuralMechanismId::from_journal_codec_bytes(
                reader.digest()?,
            ),
            execution_profile_id: ExecutionProfileId::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "mechanism artifact claim",
            tag,
        }),
    }
}

fn encode_mechanism_artifact_header(
    encoder: &mut Encoder,
    header: RelationalMechanismArtifactHeader,
) -> Result<(), RelationalJournalCodecError> {
    encoder.u32(header.version())?;
    encoder.digest(header.id().bytes())?;
    encode_mechanism_artifact_claim(encoder, header.claim())?;
    encoder.digest(header.payload_digest())?;
    encoder.u64(header.total_bytes())
}

fn decode_mechanism_artifact_header(
    reader: &mut Reader<'_>,
) -> Result<RelationalMechanismArtifactHeader, RelationalJournalCodecError> {
    Ok(
        RelationalMechanismArtifactHeader::restore_from_journal_codec(
            reader.u32()?,
            RelationalMechanismArtifactId::from_journal_codec_bytes(reader.digest()?),
            decode_mechanism_artifact_claim(reader)?,
            reader.digest()?,
            reader.u64()?,
        )?,
    )
}

fn encode_mechanism_artifact_chunk(
    encoder: &mut Encoder,
    chunk: &RelationalMechanismArtifactChunk,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(chunk.artifact_id().bytes())?;
    encoder.u32(chunk.ordinal())?;
    encoder.u64(chunk.offset())?;
    encoder.digest(chunk.chunk_digest())?;
    encoder.blob(chunk.bytes())
}

fn decode_mechanism_artifact_chunk(
    reader: &mut Reader<'_>,
) -> Result<RelationalMechanismArtifactChunk, RelationalJournalCodecError> {
    Ok(
        RelationalMechanismArtifactChunk::restore_from_journal_codec(
            RelationalMechanismArtifactId::from_journal_codec_bytes(reader.digest()?),
            reader.u32()?,
            reader.u64()?,
            reader.digest()?,
            reader.blob()?,
        )?,
    )
}

fn encode_mechanism_artifact_closure(
    encoder: &mut Encoder,
    closure: RelationalMechanismArtifactClosure,
) -> Result<(), RelationalJournalCodecError> {
    encoder.digest(closure.artifact_id().bytes())?;
    encoder.u32(closure.chunk_count())?;
    encoder.digest(closure.chunk_root().bytes())
}

fn decode_mechanism_artifact_closure(
    reader: &mut Reader<'_>,
) -> Result<RelationalMechanismArtifactClosure, RelationalJournalCodecError> {
    Ok(
        RelationalMechanismArtifactClosure::restore_from_journal_codec(
            RelationalMechanismArtifactId::from_journal_codec_bytes(reader.digest()?),
            reader.u32()?,
            RelationalMechanismArtifactChunkRoot::from_journal_codec_bytes(reader.digest()?),
        )?,
    )
}

fn encode_resolved_result_input(
    encoder: &mut Encoder,
    input: RelationalResolvedResultInput,
) -> Result<(), RelationalJournalCodecError> {
    match input {
        RelationalResolvedResultInput::Sources(relation_id) => {
            encoder.tag(0x03)?;
            encoder.digest(relation_id.bytes())
        }
        RelationalResolvedResultInput::Selected(question_id) => {
            encoder.tag(0x01)?;
            encoder.digest(question_id.bytes())
        }
        RelationalResolvedResultInput::MechanismIncidence(request_id) => {
            encoder.tag(0x02)?;
            encoder.digest(request_id.bytes())
        }
    }
}

fn decode_resolved_result_input(
    reader: &mut Reader<'_>,
) -> Result<RelationalResolvedResultInput, RelationalJournalCodecError> {
    match reader.tag()? {
        0x03 => Ok(RelationalResolvedResultInput::Sources(
            RelationId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x01 => Ok(RelationalResolvedResultInput::Selected(
            QuestionId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x02 => Ok(RelationalResolvedResultInput::MechanismIncidence(
            MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
        )),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "resolved result input",
            tag,
        }),
    }
}

fn encode_analysis_event(
    encoder: &mut Encoder,
    event: &RelationalAnalysisEvidenceEvent,
) -> Result<(), RelationalJournalCodecError> {
    match event {
        RelationalAnalysisEvidenceEvent::SelectedQuestionBound { seal, .. } => {
            encoder.tag(0x01)?;
            encode_selected_question_seal(encoder, *seal)
        }
        RelationalAnalysisEvidenceEvent::ResultSpecRegistered {
            resolved_input,
            spec,
            ..
        } => {
            encoder.tag(0x02)?;
            encode_resolved_result_input(encoder, *resolved_input)?;
            encode_result_spec(encoder, spec)
        }
        RelationalAnalysisEvidenceEvent::ResultEvidenceAccepted { record, .. } => {
            encoder.tag(0x03)?;
            encode_result_evidence_record(encoder, record)
        }
        RelationalAnalysisEvidenceEvent::ResultInputSealedFromSources { view_id, seal } => {
            encoder.tag(0x10)?;
            encoder.digest(view_id.bytes())?;
            encode_result_input_seal(encoder, *seal)
        }
        RelationalAnalysisEvidenceEvent::CertifiedSourceSummaryAccepted { artifact, .. } => {
            encoder.tag(0x11)?;
            encode_certified_source_summary_artifact(encoder, artifact)
        }
        RelationalAnalysisEvidenceEvent::ResultInputSealedFromSelected {
            view_id,
            question_seal_id,
        } => {
            encoder.tag(0x04)?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(question_seal_id.bytes())
        }
        RelationalAnalysisEvidenceEvent::ResultInputSealedFromMechanisms {
            view_id,
            request_id,
            incidence_root,
            structural_root,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(incidence_root.bytes())?;
            encoder.digest(structural_root.bytes())
        }
        RelationalAnalysisEvidenceEvent::ResultProjectionRecordAccepted {
            view_id,
            spec_root,
            ordinal,
            record_id,
            record,
        } => {
            encoder.tag(0x0f)?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(spec_root.bytes())?;
            encoder.u128(*ordinal)?;
            encoder.digest(record_id.bytes())?;
            encode_projection_record(encoder, record.record())
        }
        RelationalAnalysisEvidenceEvent::ResultViewPublished {
            evidence_root,
            closure,
            ..
        } => {
            encoder.tag(0x06)?;
            encoder.digest(evidence_root.bytes())?;
            encode_projection_closure(encoder, *closure)
        }
        RelationalAnalysisEvidenceEvent::MechanismTargetCaseAccepted {
            request_id,
            case_id,
        } => {
            encoder.tag(0x07)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(case_id.bytes())
        }
        RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromSelected {
            request_id,
            question_seal_id,
        } => {
            encoder.tag(0x08)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(question_seal_id.bytes())
        }
        RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromResult {
            request_id,
            view_id,
            result_root,
        } => {
            encoder.tag(0x09)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(view_id.bytes())?;
            encoder.digest(result_root.bytes())
        }
        RelationalAnalysisEvidenceEvent::MechanismArtifactOpened { header } => {
            encoder.tag(0x0a)?;
            encode_mechanism_artifact_header(encoder, *header)
        }
        RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { chunk } => {
            encoder.tag(0x0b)?;
            encode_mechanism_artifact_chunk(encoder, chunk)
        }
        RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { closure } => {
            encoder.tag(0x0c)?;
            encode_mechanism_artifact_closure(encoder, *closure)
        }
        RelationalAnalysisEvidenceEvent::MechanismIncidenceClosed {
            request_id,
            incidence_root,
        } => {
            encoder.tag(0x0d)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(incidence_root.bytes())
        }
        RelationalAnalysisEvidenceEvent::StructuralQuotientClosed {
            request_id,
            structural_root,
        } => {
            encoder.tag(0x12)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(structural_root.bytes())
        }
        RelationalAnalysisEvidenceEvent::SupportClosed {
            request_id,
            support_root,
        } => {
            encoder.tag(0x13)?;
            encoder.digest(request_id.bytes())?;
            encoder.digest(support_root.bytes())
        }
        RelationalAnalysisEvidenceEvent::AnalysisClosed {
            catalog_root,
            closure_set_root,
        } => {
            encoder.tag(0x0e)?;
            encoder.digest(catalog_root.bytes())?;
            encoder.digest(closure_set_root.bytes())
        }
    }
}

fn decode_analysis_event(
    reader: &mut Reader<'_>,
) -> Result<RelationalAnalysisEvidenceEvent, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalAnalysisEvidenceEvent::selected_question_bound(
            decode_selected_question_seal(reader)?,
        )),
        0x02 => Ok(RelationalAnalysisEvidenceEvent::result_spec_registered(
            decode_resolved_result_input(reader)?,
            decode_result_spec(reader)?,
        )),
        0x03 => Ok(RelationalAnalysisEvidenceEvent::result_evidence_accepted(
            decode_result_evidence_record(reader)?,
        )),
        0x10 => Ok(
            RelationalAnalysisEvidenceEvent::result_input_sealed_from_sources(
                ViewId::from_journal_codec_bytes(reader.digest()?),
                decode_result_input_seal(reader)?,
            ),
        ),
        0x11 => Ok(
            RelationalAnalysisEvidenceEvent::certified_source_summary_accepted(
                decode_certified_source_summary_artifact(reader)?,
            ),
        ),
        0x04 => Ok(
            RelationalAnalysisEvidenceEvent::ResultInputSealedFromSelected {
                view_id: ViewId::from_journal_codec_bytes(reader.digest()?),
                question_seal_id: RelationalSelectedQuestionSealId::from_journal_codec_bytes(
                    reader.digest()?,
                ),
            },
        ),
        0x05 => Ok(
            RelationalAnalysisEvidenceEvent::ResultInputSealedFromMechanisms {
                view_id: ViewId::from_journal_codec_bytes(reader.digest()?),
                request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
                incidence_root: MechanismIncidenceRoot::from_journal_codec_bytes(reader.digest()?),
                structural_root: StructuralQuotientClosureRoot::from_journal_codec_bytes(
                    reader.digest()?,
                ),
            },
        ),
        0x06 => {
            let evidence_root =
                RelationalResultEvidenceRoot::from_journal_codec_bytes(reader.digest()?);
            let closure = decode_projection_closure(reader)?;
            Ok(RelationalAnalysisEvidenceEvent::ResultViewPublished {
                view_id: closure.view_id(),
                spec_root: closure.spec_root(),
                evidence_root,
                projection_root: closure.projection_root(),
                result_root: closure.result_root(),
                closure,
            })
        }
        0x07 => Ok(
            RelationalAnalysisEvidenceEvent::MechanismTargetCaseAccepted {
                request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
                case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            },
        ),
        0x08 => Ok(
            RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromSelected {
                request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
                question_seal_id: RelationalSelectedQuestionSealId::from_journal_codec_bytes(
                    reader.digest()?,
                ),
            },
        ),
        0x09 => Ok(
            RelationalAnalysisEvidenceEvent::MechanismTargetSealedFromResult {
                request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
                view_id: ViewId::from_journal_codec_bytes(reader.digest()?),
                result_root: ResultViewRoot::from_journal_codec_bytes(reader.digest()?),
            },
        ),
        0x0a => Ok(RelationalAnalysisEvidenceEvent::MechanismArtifactOpened {
            header: decode_mechanism_artifact_header(reader)?,
        }),
        0x0b => Ok(
            RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted {
                chunk: decode_mechanism_artifact_chunk(reader)?,
            },
        ),
        0x0c => Ok(RelationalAnalysisEvidenceEvent::MechanismArtifactClosed {
            closure: decode_mechanism_artifact_closure(reader)?,
        }),
        0x0d => Ok(RelationalAnalysisEvidenceEvent::MechanismIncidenceClosed {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            incidence_root: MechanismIncidenceRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x12 => Ok(RelationalAnalysisEvidenceEvent::StructuralQuotientClosed {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            structural_root: StructuralQuotientClosureRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x13 => Ok(RelationalAnalysisEvidenceEvent::SupportClosed {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            support_root: MechanismSupportClosureRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        0x0e => Ok(RelationalAnalysisEvidenceEvent::AnalysisClosed {
            catalog_root: RelationalAnalysisCatalogRoot::from_journal_codec_bytes(reader.digest()?),
            closure_set_root: RelationalAnalysisClosureSetRoot::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x0f => {
            let view_id = ViewId::from_journal_codec_bytes(reader.digest()?);
            let spec_root = ResultViewSpecRoot::from_journal_codec_bytes(reader.digest()?);
            let ordinal = reader.u128()?;
            let record_id = ResultProjectionRecordId::from_journal_codec_bytes(reader.digest()?);
            let record = IndexedResultProjectionRecord::restore_from_journal_codec(
                view_id,
                spec_root,
                ordinal,
                record_id,
                decode_projection_record(reader)?,
            )?;
            Ok(
                RelationalAnalysisEvidenceEvent::ResultProjectionRecordAccepted {
                    view_id,
                    spec_root,
                    ordinal,
                    record_id,
                    record: Box::new(record),
                },
            )
        }
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "analysis evidence event",
            tag,
        }),
    }
}

fn encode_support_event(
    encoder: &mut Encoder,
    event: &SupportJournalEvent,
) -> Result<(), RelationalJournalCodecError> {
    match event {
        SupportJournalEvent::CellInserted { cell, .. } => {
            encoder.tag(0x01)?;
            encode_support_cell(encoder, cell)
        }
        SupportJournalEvent::RootCellDeclared { cell_id } => {
            encoder.tag(0x02)?;
            encoder.digest(cell_id.bytes())
        }
        SupportJournalEvent::RootFrontierSealed => encoder.tag(0x03),
        SupportJournalEvent::PartitionAccepted { .. } => {
            Err(RelationalJournalCodecError::ProofPolicyRequired {
                variant: "support.partition-accepted",
            })
        }
        SupportJournalEvent::LeafSealed { cell_id } => {
            encoder.tag(0x05)?;
            encoder.digest(cell_id.bytes())
        }
        SupportJournalEvent::RootObligationDeclared { obligation, .. } => {
            encoder.tag(0x06)?;
            encode_support_obligation(encoder, obligation)
        }
        SupportJournalEvent::ObligationRefined {
            refinement,
            child_obligations,
            ..
        } => {
            encoder.tag(0x07)?;
            encode_obligation_refinement(encoder, refinement)?;
            encoder.collection_len(child_obligations.len())?;
            for child in child_obligations {
                encode_support_obligation(encoder, child)?;
            }
            Ok(())
        }
        SupportJournalEvent::EvidenceAccepted { .. } => {
            Err(RelationalJournalCodecError::ProofPolicyRequired {
                variant: "support.evidence-accepted",
            })
        }
        SupportJournalEvent::ObligationFrontierSealed => encoder.tag(0x09),
        SupportJournalEvent::CatalogSealed => encoder.tag(0x0a),
    }
}

fn decode_support_event(
    reader: &mut Reader<'_>,
) -> Result<SupportJournalEvent, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(SupportJournalEvent::cell_inserted(decode_support_cell(
            reader,
        )?)),
        0x02 => Ok(SupportJournalEvent::root_cell_declared(
            SupportCellId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x03 => Ok(SupportJournalEvent::RootFrontierSealed),
        0x04 => Err(RelationalJournalCodecError::ProofPolicyRequired {
            variant: "support.partition-accepted",
        }),
        0x05 => Ok(SupportJournalEvent::leaf_sealed(
            SupportCellId::from_journal_codec_bytes(reader.digest()?),
        )),
        0x06 => Ok(SupportJournalEvent::root_obligation_declared(
            decode_support_obligation(reader)?,
        )),
        0x07 => {
            let refinement = decode_obligation_refinement(reader)?;
            let count = reader.collection_len("refinement child obligations")?;
            let mut children = Vec::new();
            children
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                children.push(decode_support_obligation(reader)?);
            }
            Ok(SupportJournalEvent::obligation_refined(
                refinement, children,
            )?)
        }
        0x08 => Err(RelationalJournalCodecError::ProofPolicyRequired {
            variant: "support.evidence-accepted",
        }),
        0x09 => Ok(SupportJournalEvent::ObligationFrontierSealed),
        0x0a => Ok(SupportJournalEvent::CatalogSealed),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "support evidence event",
            tag,
        }),
    }
}

fn encode_evidence_event(
    encoder: &mut Encoder,
    event: &RelationalEvidenceEvent,
) -> Result<(), RelationalJournalCodecError> {
    match event {
        RelationalEvidenceEvent::AnalysisPlanRegistered { plan, .. } => {
            encoder.tag(0x01)?;
            encode_analysis_plan(encoder, plan)
        }
        RelationalEvidenceEvent::SupportPlanRegistered { plan, .. } => {
            encoder.tag(0x02)?;
            encode_support_plan(encoder, plan)
        }
        RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted { artifact } => {
            encoder.tag(0x0c)?;
            encode_case_image_injectivity_proof_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted { artifact } => {
            encoder.tag(0x10)?;
            encode_source_image_exactness_proof_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted { artifact } => {
            encoder.tag(0x0e)?;
            encode_case_chunk_partition_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::RelationalClassifiedChunkAccepted { artifact } => {
            encoder.tag(0x0f)?;
            encode_classified_chunk_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted { artifact } => {
            encoder.tag(0x11)?;
            encode_selected_run_materialization_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted { artifact } => {
            encoder.tag(0x0d)?;
            encode_uniform_admission_proof_artifact(encoder, artifact)
        }
        RelationalEvidenceEvent::SourceTraversalObserved {
            advance_id,
            advance,
        } => {
            encoder.tag(0x03)?;
            encoder.digest(advance_id.bytes())?;
            encode_source_advance(encoder, advance)
        }
        RelationalEvidenceEvent::SourceEnumerationSealed { receipt, .. } => {
            encoder.tag(0x04)?;
            encode_source_relation_receipt(encoder, receipt)
        }
        RelationalEvidenceEvent::SuccessorDiscovered {
            source_key, row, ..
        } => {
            encoder.tag(0x05)?;
            encoder.digest(source_key.bytes())?;
            encode_successor_row(encoder, row)
        }
        RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted { receipt, .. } => {
            encoder.tag(0x06)?;
            encode_successor_receipt(encoder, receipt)
        }
        RelationalEvidenceEvent::SuccessorEnumerationSealed {
            source_key,
            receipt_id,
        } => {
            encoder.tag(0x07)?;
            encoder.digest(source_key.bytes())?;
            encoder.digest(receipt_id.bytes())
        }
        RelationalEvidenceEvent::AdmissionClassified { case_id, decision } => {
            encoder.tag(0x08)?;
            encoder.digest(case_id.bytes())?;
            encode_admission_decision(encoder, *decision)
        }
        RelationalEvidenceEvent::QuestionClassified { case_id, decision } => {
            encoder.tag(0x09)?;
            encoder.digest(case_id.bytes())?;
            encode_selection_decision(encoder, *decision)
        }
        RelationalEvidenceEvent::Support(event) => {
            encoder.tag(0x0a)?;
            encode_support_event(encoder, event)
        }
        RelationalEvidenceEvent::Analysis(event) => {
            encoder.tag(0x0b)?;
            encode_analysis_event(encoder, event)
        }
    }
}

fn decode_evidence_event(
    reader: &mut Reader<'_>,
    contract: RelationalJournalContract,
) -> Result<RelationalEvidenceEvent, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => {
            let plan = decode_analysis_plan(reader)?;
            Ok(RelationalEvidenceEvent::AnalysisPlanRegistered {
                plan_root: plan.root(),
                plan: Box::new(plan),
            })
        }
        0x02 => {
            let plan = decode_support_plan(reader)?;
            Ok(RelationalEvidenceEvent::SupportPlanRegistered {
                plan_root: plan.root(),
                plan: Box::new(plan),
            })
        }
        0x03 => Ok(RelationalEvidenceEvent::SourceTraversalObserved {
            advance_id: SourceTraversalAdvanceId::from_journal_codec_bytes(reader.digest()?),
            advance: Box::new(decode_source_advance(reader, contract.relation_id())?),
        }),
        0x04 => {
            let receipt = decode_source_relation_receipt(reader)?;
            Ok(RelationalEvidenceEvent::SourceEnumerationSealed {
                receipt_id: receipt.id(),
                receipt: Box::new(receipt),
            })
        }
        0x05 => {
            let source_key = SourceKey::from_journal_codec_bytes(reader.digest()?);
            let event = RelationalJournalEvent::successor_discovered(
                contract.relation_id(),
                source_key,
                decode_successor_row(reader)?,
            );
            let RelationalJournalEvent::Evidence(event) = event else {
                unreachable!("successor constructor returns evidence")
            };
            Ok(event)
        }
        0x06 => {
            let receipt = decode_successor_receipt(reader)?;
            Ok(RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted {
                receipt_id: receipt.id(),
                receipt,
            })
        }
        0x07 => Ok(RelationalEvidenceEvent::SuccessorEnumerationSealed {
            source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
            receipt_id: SuccessorFiberExhaustionReceiptId::from_journal_codec_bytes(
                reader.digest()?,
            ),
        }),
        0x08 => Ok(RelationalEvidenceEvent::AdmissionClassified {
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            decision: decode_admission_decision(reader)?,
        }),
        0x09 => Ok(RelationalEvidenceEvent::QuestionClassified {
            case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
            decision: decode_selection_decision(reader)?,
        }),
        0x0a => Ok(RelationalEvidenceEvent::Support(decode_support_event(
            reader,
        )?)),
        0x0b => Ok(RelationalEvidenceEvent::Analysis(decode_analysis_event(
            reader,
        )?)),
        0x0c => Ok(
            RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted {
                artifact: Box::new(decode_case_image_injectivity_proof_artifact(reader)?),
            },
        ),
        0x0d => Ok(
            RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted {
                artifact: Box::new(decode_uniform_admission_proof_artifact(reader)?),
            },
        ),
        0x0e => Ok(
            RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted {
                artifact: Box::new(decode_case_chunk_partition_artifact(reader)?),
            },
        ),
        0x0f => Ok(RelationalEvidenceEvent::RelationalClassifiedChunkAccepted {
            artifact: Box::new(decode_classified_chunk_artifact(reader)?),
        }),
        0x11 => Ok(
            RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted {
                artifact: Box::new(decode_selected_run_materialization_artifact(reader)?),
            },
        ),
        0x10 => Ok(
            RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted {
                artifact: Box::new(decode_source_image_exactness_proof_artifact(reader)?),
            },
        ),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "relational evidence event",
            tag,
        }),
    }
}

fn encode_checkpoint_event(
    encoder: &mut Encoder,
    event: &RelationalCheckpointEvent,
) -> Result<(), RelationalJournalCodecError> {
    match event {
        RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { artifact } => {
            encoder.tag(0x07)?;
            encode_classified_chunk_slice_artifact(encoder, artifact)
        }
        RelationalCheckpointEvent::WorkNodeInserted {
            spec, dependencies, ..
        } => {
            encoder.tag(0x01)?;
            encode_work_spec(encoder, spec)?;
            encoder.collection_len(dependencies.len())?;
            for dependency in dependencies {
                encoder.digest(dependency.bytes())?;
            }
            Ok(())
        }
        RelationalCheckpointEvent::WorkReadinessMaterialized { spec, .. } => {
            encoder.tag(0x02)?;
            encode_work_spec(encoder, spec)
        }
        RelationalCheckpointEvent::WorkCursorAdvanced {
            node_id,
            next_member_ordinal,
        } => {
            encoder.tag(0x03)?;
            encoder.digest(node_id.bytes())?;
            encoder.u128(*next_member_ordinal)
        }
        RelationalCheckpointEvent::SupportMaterializationCheckpointed { cursor } => {
            encoder.tag(0x04)?;
            encode_materialization_cursor(encoder, cursor)
        }
        RelationalCheckpointEvent::SupportFrontierCheckpointed {
            request_id,
            cursor,
            frontier_root,
        } => {
            encoder.tag(0x08)?;
            encoder.digest(request_id.bytes())?;
            encoder.u128(cursor.target_discovery())?;
            encoder.u128(cursor.terminal_discovery())?;
            encoder.u128(cursor.structural_assignment())?;
            encoder.digest(frontier_root.bytes())
        }
        RelationalCheckpointEvent::WorkNodeCompleted {
            node_id,
            completion,
        } => {
            encoder.tag(0x05)?;
            encoder.digest(node_id.bytes())?;
            encode_work_completion(encoder, completion)
        }
        RelationalCheckpointEvent::WorkFrontierCompacted { receipt } => {
            encoder.tag(0x06)?;
            encode_compaction(encoder, *receipt)
        }
    }
}

fn decode_checkpoint_event(
    reader: &mut Reader<'_>,
) -> Result<RelationalCheckpointEvent, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => {
            let spec = decode_work_spec(reader)?;
            let count = reader.collection_len("work dependencies")?;
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(count)
                .map_err(|_| RelationalJournalCodecError::AllocationFailed { requested: count })?;
            for _ in 0..count {
                dependencies.push(WorkNodeId::from_journal_codec_bytes(reader.digest()?));
            }
            let event = RelationalJournalEvent::work_node_inserted(spec, dependencies)?;
            let RelationalJournalEvent::Checkpoint(event) = event else {
                unreachable!("work insertion constructor returns checkpoint")
            };
            Ok(event)
        }
        0x02 => {
            let event =
                RelationalJournalEvent::work_readiness_materialized(decode_work_spec(reader)?)?;
            let RelationalJournalEvent::Checkpoint(event) = event else {
                unreachable!("readiness constructor returns checkpoint")
            };
            Ok(event)
        }
        0x03 => Ok(RelationalCheckpointEvent::WorkCursorAdvanced {
            node_id: WorkNodeId::from_journal_codec_bytes(reader.digest()?),
            next_member_ordinal: reader.u128()?,
        }),
        0x04 => Ok(
            RelationalCheckpointEvent::SupportMaterializationCheckpointed {
                cursor: decode_materialization_cursor(reader)?,
            },
        ),
        0x05 => Ok(RelationalCheckpointEvent::WorkNodeCompleted {
            node_id: WorkNodeId::from_journal_codec_bytes(reader.digest()?),
            completion: decode_work_completion(reader)?,
        }),
        0x06 => Ok(RelationalCheckpointEvent::WorkFrontierCompacted {
            receipt: decode_compaction(reader)?,
        }),
        0x07 => Ok(
            RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed {
                artifact: Box::new(decode_classified_chunk_slice_artifact(reader)?),
            },
        ),
        0x08 => Ok(RelationalCheckpointEvent::SupportFrontierCheckpointed {
            request_id: MechanismRequestId::from_journal_codec_bytes(reader.digest()?),
            cursor: MechanismSupportCheckpointCursor::new(
                reader.u128()?,
                reader.u128()?,
                reader.u128()?,
            ),
            frontier_root: MechanismSupportFrontierRoot::from_journal_codec_bytes(reader.digest()?),
        }),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "relational checkpoint event",
            tag,
        }),
    }
}

fn encode_journal_event(
    encoder: &mut Encoder,
    event: &RelationalJournalEvent,
) -> Result<(), RelationalJournalCodecError> {
    match event {
        RelationalJournalEvent::Evidence(event) => {
            encoder.tag(0x01)?;
            encode_evidence_event(encoder, event)
        }
        RelationalJournalEvent::Checkpoint(event) => {
            encoder.tag(0x02)?;
            encode_checkpoint_event(encoder, event)
        }
    }
}

fn decode_journal_event(
    reader: &mut Reader<'_>,
    contract: RelationalJournalContract,
) -> Result<RelationalJournalEvent, RelationalJournalCodecError> {
    match reader.tag()? {
        0x01 => Ok(RelationalJournalEvent::Evidence(decode_evidence_event(
            reader, contract,
        )?)),
        0x02 => Ok(RelationalJournalEvent::Checkpoint(decode_checkpoint_event(
            reader,
        )?)),
        tag => Err(RelationalJournalCodecError::UnknownTag {
            component: "relational journal event class",
            tag,
        }),
    }
}

#[derive(Debug)]
pub(crate) enum RelationalJournalCodecError {
    InvalidLimits,
    InvalidPackedFrameLimit {
        limit: usize,
    },
    EntryTooLarge {
        bytes: usize,
        limit: usize,
    },
    PackedEntryTooLarge {
        bytes: usize,
        limit: usize,
    },
    PackedFrameTooLarge {
        bytes: usize,
        limit: usize,
    },
    PackedEventCountMismatch {
        remaining: u64,
    },
    BlobTooLarge {
        bytes: usize,
        limit: usize,
    },
    StringTooLarge {
        bytes: usize,
        limit: usize,
    },
    CollectionTooLarge {
        items: usize,
        limit: usize,
    },
    ValueDepthExceeded {
        depth: usize,
        limit: usize,
    },
    ValueNodeLimitExceeded {
        limit: usize,
    },
    LengthOverflow,
    LengthNotRepresentable {
        component: &'static str,
    },
    DeclaredLengthTooLarge {
        component: &'static str,
        claimed: u64,
        limit: usize,
    },
    AllocationFailed {
        requested: usize,
    },
    Truncated,
    TrailingBytes {
        bytes: usize,
    },
    Utf8,
    InvalidCharacter(u32),
    UnknownTag {
        component: &'static str,
        tag: u8,
    },
    Malformed(&'static str),
    UnsupportedCodecSchema {
        actual: u32,
        expected: u32,
    },
    UnsupportedJournalSchema {
        actual: u32,
        expected: u32,
    },
    NonCanonicalEncoding,
    NonCanonicalValue(&'static str),
    /// The event carries only a proof receipt digest, not the canonical proof
    /// artifact required to invoke a trusted verifier and privately remint it.
    ProofPolicyRequired {
        variant: &'static str,
    },
    Journal(RelationalJournalError),
    AnalysisPlan(RelationalAnalysisPlanError),
    SupportPlan(RelationalSupportPlannerError),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    SourceImageProof(RelationalSourceImageExactnessProofError),
    CertifiedSourceSummary(RelationalCertifiedSourceSummaryError),
    CaseChunkPartition(RelationalCaseChunkPartitionError),
    ClassifiedSweep(RelationalClassifiedSweepError),
    SelectedRunMaterialization(RelationalSelectedRunMaterializationError),
    UniformAdmissionProof(RelationalUniformAdmissionProofError),
    SupportCell(SupportCellError),
    SupportEvidence(SupportEvidenceError),
    SupportJournal(SupportJournalError),
    SourceExecutor(RelationalSourceExecutorError),
    CaseExecutor(RelationalCaseExecutorError),
    SourceClosure(SourceTraversalClosureError),
    Work(WorkFrontierError),
    ResultProjection(ResultProjectionError),
    ResultView(ResultViewError),
    ResultEvidence(ResultEvidenceError),
    Analysis(RelationalAnalysisJournalError),
}

impl fmt::Display for RelationalJournalCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => write!(formatter, "invalid relational-journal codec limits"),
            Self::InvalidPackedFrameLimit { limit } => write!(
                formatter,
                "invalid packed relational-journal physical-frame limit {limit}"
            ),
            Self::EntryTooLarge { bytes, limit } => write!(
                formatter,
                "relational-journal entry has {bytes} bytes, exceeding limit {limit}"
            ),
            Self::PackedEntryTooLarge { bytes, limit } => write!(
                formatter,
                "length-delimited relational-journal entry has {bytes} bytes, exceeding physical-frame limit {limit}"
            ),
            Self::PackedFrameTooLarge { bytes, limit } => write!(
                formatter,
                "packed relational-journal physical frame has {bytes} bytes, exceeding limit {limit}"
            ),
            Self::PackedEventCountMismatch { remaining } => write!(
                formatter,
                "packed relational-journal physical frame is missing {remaining} semantic entries"
            ),
            Self::BlobTooLarge { bytes, limit } => {
                write!(formatter, "blob has {bytes} bytes, exceeding limit {limit}")
            }
            Self::StringTooLarge { bytes, limit } => {
                write!(
                    formatter,
                    "string has {bytes} bytes, exceeding limit {limit}"
                )
            }
            Self::CollectionTooLarge { items, limit } => write!(
                formatter,
                "collection has {items} items, exceeding limit {limit}"
            ),
            Self::ValueDepthExceeded { depth, limit } => write!(
                formatter,
                "canonical value depth {depth} exceeds limit {limit}"
            ),
            Self::ValueNodeLimitExceeded { limit } => {
                write!(
                    formatter,
                    "canonical value node count exceeds limit {limit}"
                )
            }
            Self::LengthOverflow => write!(formatter, "canonical length overflow"),
            Self::LengthNotRepresentable { component } => {
                write!(
                    formatter,
                    "{component} length is not representable on this host"
                )
            }
            Self::DeclaredLengthTooLarge {
                component,
                claimed,
                limit,
            } => write!(
                formatter,
                "declared {component} length {claimed} exceeds limit {limit}"
            ),
            Self::AllocationFailed { requested } => {
                write!(
                    formatter,
                    "failed bounded allocation for {requested} items or bytes"
                )
            }
            Self::Truncated => write!(formatter, "truncated relational-journal entry"),
            Self::TrailingBytes { bytes } => {
                write!(
                    formatter,
                    "relational-journal entry has {bytes} trailing bytes"
                )
            }
            Self::Utf8 => write!(formatter, "invalid UTF-8 in canonical string"),
            Self::InvalidCharacter(value) => {
                write!(formatter, "invalid Unicode scalar value {value:#x}")
            }
            Self::UnknownTag { component, tag } => {
                write!(formatter, "unknown {component} tag {tag:#x}")
            }
            Self::Malformed(message) => write!(formatter, "malformed journal entry: {message}"),
            Self::UnsupportedCodecSchema { actual, expected } => write!(
                formatter,
                "unsupported codec schema {actual}; expected {expected}"
            ),
            Self::UnsupportedJournalSchema { actual, expected } => write!(
                formatter,
                "unsupported relational-journal schema {actual}; expected {expected}"
            ),
            Self::NonCanonicalEncoding => write!(formatter, "noncanonical journal encoding"),
            Self::NonCanonicalValue(component) => {
                write!(formatter, "noncanonical {component}")
            }
            Self::ProofPolicyRequired { variant } => write!(
                formatter,
                "{variant} requires a bound proof-verifier policy and canonical proof artifact"
            ),
            Self::Journal(error) => write!(formatter, "invalid journal entry: {error}"),
            Self::AnalysisPlan(error) => write!(formatter, "invalid analysis plan: {error}"),
            Self::SupportPlan(error) => write!(formatter, "invalid support plan: {error}"),
            Self::CaseImageProof(error) => {
                write!(formatter, "invalid case-image proof artifact: {error}")
            }
            Self::SourceImageProof(error) => {
                write!(formatter, "invalid source-image proof artifact: {error}")
            }
            Self::CertifiedSourceSummary(error) => {
                write!(
                    formatter,
                    "invalid certified source-summary artifact: {error}"
                )
            }
            Self::CaseChunkPartition(error) => {
                write!(formatter, "invalid case-chunk partition artifact: {error}")
            }
            Self::ClassifiedSweep(error) => {
                write!(formatter, "invalid classified-chunk artifact: {error}")
            }
            Self::SelectedRunMaterialization(error) => {
                write!(
                    formatter,
                    "invalid selected-run materialization artifact: {error}"
                )
            }
            Self::UniformAdmissionProof(error) => {
                write!(
                    formatter,
                    "invalid uniform-admission proof artifact: {error}"
                )
            }
            Self::SupportCell(error) => write!(formatter, "invalid support value: {error}"),
            Self::SupportEvidence(error) => {
                write!(formatter, "invalid support evidence structure: {error}")
            }
            Self::SupportJournal(error) => write!(formatter, "invalid support event: {error}"),
            Self::SourceExecutor(error) => write!(formatter, "invalid source value: {error}"),
            Self::CaseExecutor(error) => write!(formatter, "invalid successor value: {error}"),
            Self::SourceClosure(error) => write!(formatter, "invalid source closure: {error}"),
            Self::Work(error) => write!(formatter, "invalid work checkpoint: {error}"),
            Self::ResultProjection(error) => {
                write!(formatter, "invalid result projection: {error}")
            }
            Self::ResultView(error) => write!(formatter, "invalid result view: {error}"),
            Self::ResultEvidence(error) => write!(formatter, "invalid result evidence: {error}"),
            Self::Analysis(error) => write!(formatter, "invalid analysis evidence: {error}"),
        }
    }
}

impl Error for RelationalJournalCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Journal(error) => Some(error),
            Self::AnalysisPlan(error) => Some(error),
            Self::SupportPlan(error) => Some(error),
            Self::CaseImageProof(error) => Some(error),
            Self::SourceImageProof(error) => Some(error),
            Self::CertifiedSourceSummary(error) => Some(error),
            Self::CaseChunkPartition(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::SelectedRunMaterialization(error) => Some(error),
            Self::UniformAdmissionProof(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            Self::SupportEvidence(error) => Some(error),
            Self::SupportJournal(error) => Some(error),
            Self::SourceExecutor(error) => Some(error),
            Self::CaseExecutor(error) => Some(error),
            Self::SourceClosure(error) => Some(error),
            Self::Work(error) => Some(error),
            Self::ResultProjection(error) => Some(error),
            Self::ResultView(error) => Some(error),
            Self::ResultEvidence(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::InvalidLimits
            | Self::InvalidPackedFrameLimit { .. }
            | Self::EntryTooLarge { .. }
            | Self::PackedEntryTooLarge { .. }
            | Self::PackedFrameTooLarge { .. }
            | Self::PackedEventCountMismatch { .. }
            | Self::BlobTooLarge { .. }
            | Self::StringTooLarge { .. }
            | Self::CollectionTooLarge { .. }
            | Self::ValueDepthExceeded { .. }
            | Self::ValueNodeLimitExceeded { .. }
            | Self::LengthOverflow
            | Self::LengthNotRepresentable { .. }
            | Self::DeclaredLengthTooLarge { .. }
            | Self::AllocationFailed { .. }
            | Self::Truncated
            | Self::TrailingBytes { .. }
            | Self::Utf8
            | Self::InvalidCharacter(_)
            | Self::UnknownTag { .. }
            | Self::Malformed(_)
            | Self::UnsupportedCodecSchema { .. }
            | Self::UnsupportedJournalSchema { .. }
            | Self::NonCanonicalEncoding
            | Self::NonCanonicalValue(_)
            | Self::ProofPolicyRequired { .. } => None,
        }
    }
}

macro_rules! codec_error_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for RelationalJournalCodecError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

codec_error_from!(RelationalJournalError, Journal);
codec_error_from!(RelationalAnalysisPlanError, AnalysisPlan);
codec_error_from!(RelationalSupportPlannerError, SupportPlan);
codec_error_from!(RelationalCaseImageInjectivityProofError, CaseImageProof);
codec_error_from!(RelationalSourceImageExactnessProofError, SourceImageProof);
codec_error_from!(
    RelationalCertifiedSourceSummaryError,
    CertifiedSourceSummary
);
codec_error_from!(RelationalCaseChunkPartitionError, CaseChunkPartition);
codec_error_from!(RelationalClassifiedSweepError, ClassifiedSweep);
codec_error_from!(
    RelationalSelectedRunMaterializationError,
    SelectedRunMaterialization
);
codec_error_from!(RelationalUniformAdmissionProofError, UniformAdmissionProof);
codec_error_from!(SupportCellError, SupportCell);
codec_error_from!(SupportEvidenceError, SupportEvidence);
codec_error_from!(SupportJournalError, SupportJournal);
codec_error_from!(RelationalSourceExecutorError, SourceExecutor);
codec_error_from!(RelationalCaseExecutorError, CaseExecutor);
codec_error_from!(SourceTraversalClosureError, SourceClosure);
codec_error_from!(WorkFrontierError, Work);
codec_error_from!(ResultProjectionError, ResultProjection);
codec_error_from!(ResultViewError, ResultView);
codec_error_from!(ResultEvidenceError, ResultEvidence);
codec_error_from!(RelationalAnalysisJournalError, Analysis);

#[cfg(test)]
mod tests {
    use super::super::relation::{AdmissionId, FindPolarity, QuestionId, RelationId};
    use super::super::relational_journal::{RelationalJournal, RelationalJournalContract};
    use super::*;

    #[test]
    fn codec_rejects_previous_semantic_journal_schema_before_payload_decode() {
        let relation = RelationId::from_canonical_semantic_preimage(b"old-schema relation");
        let admission =
            AdmissionId::from_canonical_admission_preimage(relation, b"old-schema admission");
        let question = QuestionId::from_canonical_find_preimage(
            admission,
            b"old-schema question",
            FindPolarity::All,
        );
        let contract = RelationalJournalContract::new(
            relation,
            admission,
            question,
            super::super::StateSchemaId::from_bytes([1; 32]),
            super::super::ContextSchemaId::from_bytes([2; 32]),
            super::super::TransitionTypeId::from_bytes([3; 32]),
            [0; 32],
        );
        let journal = RelationalJournal::new(contract);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(ENTRY_MAGIC);
        bytes.extend_from_slice(&RELATIONAL_JOURNAL_CODEC_SCHEMA_VERSION.to_be_bytes());
        bytes.extend_from_slice(&(RELATIONAL_JOURNAL_SCHEMA_VERSION - 1).to_be_bytes());

        let error = decode_relational_journal_entry(
            contract,
            journal.next_sequence(),
            journal.head(),
            &bytes,
            RelationalJournalCodecLimits::default(),
        )
        .expect_err("old semantic journal schema must fail closed");
        assert!(matches!(
            error,
            RelationalJournalCodecError::UnsupportedJournalSchema {
                actual: 15,
                expected: 16
            }
        ));
    }
}
