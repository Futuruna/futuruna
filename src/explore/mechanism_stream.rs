//! Canonical durable mechanism observations and their replay reducer.
//!
//! Decoding produces proposals only. A fresh-replay adapter must confirm the
//! checked sites, endpoint traces, outcomes, bin values, and receipt before a
//! batch crosses the private validation seam. The reducer then treats the
//! validated batch as an atomic journal block and derives an
//! arrival-order-independent materialized view.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{
    exact_stream::ExactClosedMatchSupportV1,
    mechanism::{
        CanonicalSignatureInterner, CheckedMechanismObservationRequestV1,
        CheckedMechanismRequestId, DynamicEventKind, DynamicEventOutcome,
        DynamicMechanismSignature, ExactMatchingTargetMembership, IfDecisionOutcome,
        KnownTargetUntracedReason, MechanismActivationStepV1, MechanismBinField,
        MechanismBinFieldEvidence, MechanismBinUnavailableReason, MechanismBinUnavailableSupport,
        MechanismCallableSiteId, MechanismCount, MechanismEvidenceStatus,
        MechanismIncidenceDisclosure, MechanismIncidenceTerminal, MechanismNumericBin,
        MechanismObservationRequest, MechanismObservedEvidence, MechanismOccurrenceId,
        MechanismOccurrenceSlotV1, MechanismPopulationEvidence, MechanismSignatureBinIncidence,
        MechanismSignatureId, MechanismSiteId, MechanismSiteKind, MechanismTargetMembership,
        PairedOccurrenceNode, RuleAttemptOutcome, RuleSelectionOutcome, ShortCircuitOutcome,
    },
    report::ExploreCaseId,
    run_stream::{ExactCaseSupport, ExploreCaseUniverse},
};

const MECHANISM_BATCH_MAGIC_V1: &[u8; 8] = b"FXMEB001";
pub(crate) const MECHANISM_OBSERVATION_BLOB_KIND_V1: &str = "mechanism-observations-v1";

// Wire and reducer limits are deliberately conservative and platform
// independent. They bound one journal proposal; they do not authorize a
// producer to truncate semantic evidence silently.
const MAX_BATCH_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_AXES: usize = 256;
const MAX_OBSERVATIONS_PER_BATCH: usize = 4_096;
const MAX_SIGNATURES_PER_BATCH: usize = 4_096;
const MAX_SIGNATURE_NODES_PER_BATCH: usize = 65_536;
const MAX_SIGNATURE_EDGES_PER_BATCH: usize = 262_144;
const MAX_INCIDENCE_OVERRIDE_INTERVALS: usize = 262_144;
pub(crate) const MAX_NORMALIZED_SEMANTIC_FACTS_PER_BATCH: usize = 262_144;
const MAX_ACTIVATION_DEPTH: usize = 256;
pub(crate) const MAX_BIN_FIELDS: usize = 256;
pub(crate) const MAX_BINS_PER_FIELD: usize = 65_536;
pub(crate) const MAX_TOTAL_BINS: usize = 262_144;
pub(crate) const MAX_RETAINED_EXAMPLES_PER_SIGNATURE: usize = 1_024;
pub(crate) const MAX_SELECTED_SAMPLING_CASES: usize = 65_536;
pub(crate) const MAX_REDUCER_UNIQUE_SIGNATURES: usize = 65_536;
pub(crate) const MAX_REDUCER_SIGNATURE_NODES: usize = 1_048_576;
pub(crate) const MAX_REDUCER_SIGNATURE_ACTIVATION_STEPS: usize = 1_048_576;
pub(crate) const MAX_REDUCER_SIGNATURE_EDGES: usize = 4_194_304;
pub(crate) const MAX_REDUCER_SUPPORT_MAP_ENTRIES: usize = 262_144;
pub(crate) const MAX_REDUCER_SUPPORT_INTERVALS: usize = 1_048_576;
pub(crate) const MAX_REDUCER_RETAINED_EXAMPLES: usize = 262_144;
const MAX_TEXT_BYTES: usize = 4_096;

/// Identity of the canonical mechanism-batch wire schema and every fixed
/// resource ceiling enforced by its decoder/reducer.
///
/// Operational budgets such as jobs, elapsed time, and scheduling order are
/// deliberately absent. Changing any wire tag or fixed ceiling must therefore
/// produce a different Explore stream identity before replay begins.
pub(crate) fn mechanism_stream_contract_digest_v1() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"futuruna.explore.mechanism-stream-contract.v1");
    for segment in [
        b"canonical-mechanism-observation-batch-v1".as_slice(),
        MECHANISM_BATCH_MAGIC_V1.as_slice(),
        MECHANISM_OBSERVATION_BLOB_KIND_V1.as_bytes(),
        b"complete-signature-definitions-v1".as_slice(),
        b"fresh-replay-validation-receipt-v1".as_slice(),
        b"normalized-mechanism-semantic-facts-v1".as_slice(),
        b"fixed-resource-limits-v1".as_slice(),
    ] {
        hasher.update((segment.len() as u64).to_le_bytes());
        hasher.update(segment);
    }
    for (name, limit) in [
        (b"batch-bytes".as_slice(), MAX_BATCH_BYTES),
        (b"axes".as_slice(), MAX_AXES),
        (
            b"observations-per-batch".as_slice(),
            MAX_OBSERVATIONS_PER_BATCH,
        ),
        (b"signatures-per-batch".as_slice(), MAX_SIGNATURES_PER_BATCH),
        (
            b"signature-nodes-per-batch".as_slice(),
            MAX_SIGNATURE_NODES_PER_BATCH,
        ),
        (
            b"signature-edges-per-batch".as_slice(),
            MAX_SIGNATURE_EDGES_PER_BATCH,
        ),
        (
            b"incidence-override-intervals".as_slice(),
            MAX_INCIDENCE_OVERRIDE_INTERVALS,
        ),
        (
            b"incidence-rank-interval-dimension-steps".as_slice(),
            MAX_INCIDENCE_OVERRIDE_INTERVALS,
        ),
        (
            b"target-rank-interval-dimension-steps".as_slice(),
            MAX_INCIDENCE_OVERRIDE_INTERVALS,
        ),
        (
            b"normalized-semantic-facts-per-batch".as_slice(),
            MAX_NORMALIZED_SEMANTIC_FACTS_PER_BATCH,
        ),
        (b"activation-depth".as_slice(), MAX_ACTIVATION_DEPTH),
        (b"bin-fields".as_slice(), MAX_BIN_FIELDS),
        (b"bins-per-field".as_slice(), MAX_BINS_PER_FIELD),
        (b"total-bins".as_slice(), MAX_TOTAL_BINS),
        (
            b"retained-examples-per-signature".as_slice(),
            MAX_RETAINED_EXAMPLES_PER_SIGNATURE,
        ),
        (
            b"selected-sampling-cases".as_slice(),
            MAX_SELECTED_SAMPLING_CASES,
        ),
        (
            b"reducer-unique-signatures".as_slice(),
            MAX_REDUCER_UNIQUE_SIGNATURES,
        ),
        (
            b"reducer-signature-nodes".as_slice(),
            MAX_REDUCER_SIGNATURE_NODES,
        ),
        (
            b"reducer-signature-activation-steps".as_slice(),
            MAX_REDUCER_SIGNATURE_ACTIVATION_STEPS,
        ),
        (
            b"reducer-signature-edges".as_slice(),
            MAX_REDUCER_SIGNATURE_EDGES,
        ),
        (
            b"reducer-support-map-entries".as_slice(),
            MAX_REDUCER_SUPPORT_MAP_ENTRIES,
        ),
        (
            b"reducer-support-intervals".as_slice(),
            MAX_REDUCER_SUPPORT_INTERVALS,
        ),
        (
            b"reducer-retained-examples".as_slice(),
            MAX_REDUCER_RETAINED_EXAMPLES,
        ),
        (b"text-bytes".as_slice(), MAX_TEXT_BYTES),
    ] {
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name);
        hasher.update((limit as u128).to_le_bytes());
    }
    hasher.finalize().into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismStreamError {
    Invalid(Box<str>),
    ReducerCapacity {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    SnapshotCapacity {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
}

impl MechanismStreamError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into().into_boxed_str())
    }

    fn snapshot_capacity(resource: &'static str, actual: usize, limit: usize) -> Self {
        Self::SnapshotCapacity {
            resource,
            actual,
            limit,
        }
    }

    fn reducer_capacity(resource: &'static str, actual: usize, limit: usize) -> Self {
        Self::ReducerCapacity {
            resource,
            actual,
            limit,
        }
    }

    pub(crate) fn is_reducer_capacity(&self) -> bool {
        matches!(self, Self::ReducerCapacity { .. })
    }

    pub(crate) fn reducer_capacity_details(&self) -> Option<(&'static str, usize, usize)> {
        match self {
            Self::ReducerCapacity {
                resource,
                actual,
                limit,
            } => Some((*resource, *actual, *limit)),
            Self::Invalid(_) | Self::SnapshotCapacity { .. } => None,
        }
    }

    pub(crate) fn is_snapshot_capacity(&self) -> bool {
        matches!(self, Self::SnapshotCapacity { .. })
    }

    pub(crate) fn snapshot_capacity_details(&self) -> Option<(&'static str, usize, usize)> {
        match self {
            Self::SnapshotCapacity {
                resource,
                actual,
                limit,
            } => Some((*resource, *actual, *limit)),
            Self::Invalid(_) | Self::ReducerCapacity { .. } => None,
        }
    }
}

impl fmt::Display for MechanismStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::ReducerCapacity {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "mechanism reducer needs {actual} {resource}; capacity is {limit}"
            ),
            Self::SnapshotCapacity {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "mechanism snapshot needs {actual} {resource}; capacity is {limit}"
            ),
        }
    }
}

impl Error for MechanismStreamError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismValidationReceiptDigestV1([u8; 32]);

impl MechanismValidationReceiptDigestV1 {
    pub(crate) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical mixed-radix identity retained redundantly as rank and ordinals.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MechanismCanonicalCaseIdV1 {
    pub(crate) rank: u128,
    pub(crate) ordinals: Box<[u128]>,
}

impl MechanismCanonicalCaseIdV1 {
    pub(crate) fn new(rank: u128, ordinals: impl Into<Box<[u128]>>) -> Self {
        Self {
            rank,
            ordinals: ordinals.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MechanismBinAssignmentOutcomeV1 {
    Binned(MechanismNumericBin),
    OutsideDeclaredBins,
    ReplayUnavailable,
    ObservationUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MechanismBinAssignmentV1 {
    pub(crate) field_name: Box<str>,
    pub(crate) outcome: MechanismBinAssignmentOutcomeV1,
}

impl MechanismBinAssignmentV1 {
    pub(crate) fn binned(field_name: impl Into<Box<str>>, bin: MechanismNumericBin) -> Self {
        Self {
            field_name: field_name.into(),
            outcome: MechanismBinAssignmentOutcomeV1::Binned(bin),
        }
    }

    pub(crate) fn unavailable(
        field_name: impl Into<Box<str>>,
        outcome: MechanismBinAssignmentOutcomeV1,
    ) -> Result<Self, MechanismStreamError> {
        if matches!(
            outcome,
            MechanismBinAssignmentOutcomeV1::Binned(_)
                | MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins
        ) {
            return Err(MechanismStreamError::invalid(
                "successful mechanism bin assignments must use a successful assignment constructor",
            ));
        }
        Ok(Self {
            field_name: field_name.into(),
            outcome,
        })
    }

    pub(crate) fn outside_declared_bins(field_name: impl Into<Box<str>>) -> Self {
        Self {
            field_name: field_name.into(),
            outcome: MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MechanismPermanentUntracedReasonV1 {
    ReplayUnavailable,
    ObservationUnsupported,
}

impl MechanismPermanentUntracedReasonV1 {
    fn incidence_reason(self) -> KnownTargetUntracedReason {
        match self {
            Self::ReplayUnavailable => KnownTargetUntracedReason::ReplayUnavailable,
            Self::ObservationUnsupported => KnownTargetUntracedReason::ObservationUnsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismCaseObservationOutcomeProposalV1 {
    Observed {
        signature: MechanismSignatureId,
        bin_assignments: Box<[MechanismBinAssignmentV1]>,
    },
    PermanentlyUntraced(MechanismPermanentUntracedReasonV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismCaseObservationProposalV1 {
    pub(crate) case_id: MechanismCanonicalCaseIdV1,
    pub(crate) outcome: MechanismCaseObservationOutcomeProposalV1,
    pub(crate) validation_receipt_digest: MechanismValidationReceiptDigestV1,
}

impl MechanismCaseObservationProposalV1 {
    pub(crate) fn observed(
        case_id: MechanismCanonicalCaseIdV1,
        signature: MechanismSignatureId,
        bin_assignments: impl Into<Box<[MechanismBinAssignmentV1]>>,
        validation_receipt_digest: MechanismValidationReceiptDigestV1,
    ) -> Self {
        Self {
            case_id,
            outcome: MechanismCaseObservationOutcomeProposalV1::Observed {
                signature,
                bin_assignments: bin_assignments.into(),
            },
            validation_receipt_digest,
        }
    }

    pub(crate) fn permanently_untraced(
        case_id: MechanismCanonicalCaseIdV1,
        reason: MechanismPermanentUntracedReasonV1,
        validation_receipt_digest: MechanismValidationReceiptDigestV1,
    ) -> Self {
        Self {
            case_id,
            outcome: MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(reason),
            validation_receipt_digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MechanismSignatureDefinitionProposalV1 {
    id: MechanismSignatureId,
    signature: DynamicMechanismSignature,
}

/// Decoded, untrusted, nonempty mechanism replay slice.
///
/// Every observed signature is defined in the same block. Definitions are
/// canonicalized by content ID, while cases are canonicalized by rank. This
/// makes each persisted block independently inspectable and replayable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismObservationBatchProposalV1 {
    checked_request_id: CheckedMechanismRequestId,
    definitions: Box<[MechanismSignatureDefinitionProposalV1]>,
    observations: Box<[MechanismCaseObservationProposalV1]>,
}

impl MechanismObservationBatchProposalV1 {
    pub(crate) fn new(
        request: &CheckedMechanismObservationRequestV1,
        definitions: impl Into<Box<[DynamicMechanismSignature]>>,
        observations: impl Into<Box<[MechanismCaseObservationProposalV1]>>,
    ) -> Result<Self, MechanismStreamError> {
        validate_mechanism_stream_request_v1(request)?;
        let definitions = normalize_definitions(&request.observation, definitions.into())?;
        let mut observations = observations.into().into_vec();
        validate_nonempty_len(
            "mechanism observation",
            observations.len(),
            MAX_OBSERVATIONS_PER_BATCH,
        )?;
        for observation in observations.iter_mut() {
            normalize_observation(request, observation)?;
        }
        observations.sort_by_key(|observation| observation.case_id.rank);
        validate_observation_sequence(&observations)?;
        validate_definition_references(&definitions, &observations)?;
        validate_signature_resource_bounds(&definitions)?;
        Ok(Self {
            checked_request_id: request.id.clone(),
            definitions,
            observations: observations.into_boxed_slice(),
        })
    }

    pub(crate) fn observations(&self) -> &[MechanismCaseObservationProposalV1] {
        &self.observations
    }

    /// Complete canonical definitions available to the trusted replay
    /// confirmer. The iterator order is the signature content-ID order used
    /// by the wire format.
    pub(crate) fn definitions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&MechanismSignatureId, &DynamicMechanismSignature)> {
        self.definitions
            .iter()
            .map(|definition| (&definition.id, &definition.signature))
    }
}

/// Fresh-replay-confirmed, atomic mechanism observation slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedMechanismObservationBatchV1 {
    proposal: MechanismObservationBatchProposalV1,
}

impl ValidatedMechanismObservationBatchV1 {
    pub(crate) fn proposal(&self) -> &MechanismObservationBatchProposalV1 {
        &self.proposal
    }
}

mod validation_boundary {
    use super::*;

    pub(super) fn seal_fresh_replay_confirmed_batch<Confirm>(
        request: &CheckedMechanismObservationRequestV1,
        proposal: MechanismObservationBatchProposalV1,
        confirm: Confirm,
    ) -> Result<ValidatedMechanismObservationBatchV1, MechanismStreamError>
    where
        Confirm: FnOnce(&MechanismObservationBatchProposalV1) -> Result<(), MechanismStreamError>,
    {
        validate_batch_for_request(request, &proposal)?;
        confirm(&proposal)?;
        Ok(ValidatedMechanismObservationBatchV1 { proposal })
    }
}

/// Trusted mint seam. `confirm` must fresh-replay every case and source-check
/// every decoded semantic site, endpoint event, bin value, and receipt.
pub(super) fn seal_fresh_replay_confirmed_mechanism_batch_v1<Confirm>(
    request: &CheckedMechanismObservationRequestV1,
    proposal: MechanismObservationBatchProposalV1,
    confirm: Confirm,
) -> Result<ValidatedMechanismObservationBatchV1, MechanismStreamError>
where
    Confirm: FnOnce(&MechanismObservationBatchProposalV1) -> Result<(), String>,
{
    validation_boundary::seal_fresh_replay_confirmed_batch(request, proposal, |proposal| {
        confirm(proposal).map_err(MechanismStreamError::invalid)
    })
}

/// Restore an already authenticated local journal block without rerunning its
/// expensive trace. The caller must verify the blob digest, journal envelope,
/// writer fence, and original validation receipt before returning success.
pub(super) fn restore_committed_mechanism_batch_v1<Confirm>(
    request: &CheckedMechanismObservationRequestV1,
    proposal: MechanismObservationBatchProposalV1,
    confirm_commitment: Confirm,
) -> Result<ValidatedMechanismObservationBatchV1, MechanismStreamError>
where
    Confirm: FnOnce(&ValidatedMechanismObservationBatchV1) -> Result<(), String>,
{
    let validated =
        validation_boundary::seal_fresh_replay_confirmed_batch(request, proposal, |_| Ok(()))?;
    confirm_commitment(&validated).map_err(MechanismStreamError::invalid)?;
    Ok(validated)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MechanismReducerResourceUsageV1 {
    unique_signatures: usize,
    signature_nodes: usize,
    signature_activation_steps: usize,
    signature_edges: usize,
    support_map_entries: usize,
    support_intervals: usize,
    retained_examples: usize,
}

#[derive(Debug, Clone, Copy)]
struct MechanismReducerResourceLimitsV1 {
    unique_signatures: usize,
    signature_nodes: usize,
    signature_activation_steps: usize,
    signature_edges: usize,
    support_map_entries: usize,
    support_intervals: usize,
    retained_examples: usize,
}

const MECHANISM_REDUCER_RESOURCE_LIMITS_V1: MechanismReducerResourceLimitsV1 =
    MechanismReducerResourceLimitsV1 {
        unique_signatures: MAX_REDUCER_UNIQUE_SIGNATURES,
        signature_nodes: MAX_REDUCER_SIGNATURE_NODES,
        signature_activation_steps: MAX_REDUCER_SIGNATURE_ACTIVATION_STEPS,
        signature_edges: MAX_REDUCER_SIGNATURE_EDGES,
        support_map_entries: MAX_REDUCER_SUPPORT_MAP_ENTRIES,
        support_intervals: MAX_REDUCER_SUPPORT_INTERVALS,
        retained_examples: MAX_REDUCER_RETAINED_EXAMPLES,
    };

pub(crate) struct PreparedMechanismObservationBatchV1 {
    prior_revision: u64,
    next_processed: ExactCaseSupport,
    new_definitions: BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
    next_signature_supports: BTreeMap<MechanismSignatureId, ExactCaseSupport>,
    next_untraced_supports: BTreeMap<MechanismPermanentUntracedReasonV1, ExactCaseSupport>,
    next_signature_bin_supports: BTreeMap<MechanismSignatureBinIncidence, ExactCaseSupport>,
    next_field_signature_binned_supports:
        BTreeMap<(Box<str>, MechanismSignatureId), ExactCaseSupport>,
    next_field_signature_outside_supports:
        BTreeMap<(Box<str>, MechanismSignatureId), ExactCaseSupport>,
    next_field_unavailable_supports: BTreeMap<
        (
            Box<str>,
            MechanismSignatureId,
            MechanismBinAssignmentOutcomeV1,
        ),
        ExactCaseSupport,
    >,
    next_retained_examples: BTreeMap<MechanismSignatureId, BTreeMap<u128, ExploreCaseId>>,
    next_traced: u128,
    next_permanently_untraced: u128,
    next_resource_usage: MechanismReducerResourceUsageV1,
}

pub(crate) struct PreparedKnownMechanismTargetSupportV1 {
    prior_revision: u64,
    next_known_target_support: ExactCaseSupport,
}

pub(crate) struct PreparedExactMechanismTargetV1 {
    prior_revision: u64,
    target: ExactMatchingTargetMembership,
}

/// Certainty attached to one count in a lightweight observable checkpoint.
///
/// `Unknown` is deliberately distinct from a zero lower bound. It means the
/// current evidence has not confirmed any member, while still refusing to
/// publish zero as a mathematical lower-bound result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MechanismCheckpointCountV1 {
    Exact(u128),
    LowerBound(u128),
    Unknown { confirmed_lower_bound: u128 },
}

impl MechanismCheckpointCountV1 {
    pub(crate) const fn confirmed_lower_bound(self) -> u128 {
        match self {
            Self::Exact(value) | Self::LowerBound(value) => value,
            Self::Unknown {
                confirmed_lower_bound,
            } => confirmed_lower_bound,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MechanismCheckpointUntracedV1 {
    pub(crate) total: u128,
    pub(crate) pending: u128,
    pub(crate) replay_unavailable: u128,
    pub(crate) observation_unsupported: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MechanismCheckpointBinV1 {
    pub(crate) bin: MechanismNumericBin,
    pub(crate) confirmed_case_support: u128,
    pub(crate) mechanism_count: MechanismCheckpointCountV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismCheckpointBinFieldV1 {
    pub(crate) name: Box<str>,
    pub(crate) binned_cases: u128,
    pub(crate) outside_declared_bins_cases: u128,
    pub(crate) unavailable_cases: u128,
    pub(crate) replay_unavailable_cases: u128,
    pub(crate) observation_unsupported_cases: u128,
    pub(crate) bins: Box<[MechanismCheckpointBinV1]>,
}

/// Count-only projection of the mechanism reducer at one durable cursor.
///
/// This intentionally omits signature definitions, retained examples and both
/// incidence DAGs. It is therefore cheap enough to expose during a running
/// stream, while its certainty tags prevent open scope or unavailable bin
/// values from being mistaken for exact counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismCheckpointSummaryV1 {
    pub(crate) checked_request_hash: [u8; 32],
    pub(crate) observation_spec_hash: [u8; 32],
    pub(crate) status: MechanismEvidenceStatus,
    pub(crate) target_cases: MechanismCount,
    pub(crate) traced_cases: u128,
    pub(crate) known_target_untraced: MechanismCheckpointUntracedV1,
    pub(crate) mechanism_signatures: MechanismCheckpointCountV1,
    pub(crate) bin_fields: Box<[MechanismCheckpointBinFieldV1]>,
}

/// Pure, non-Clone replay reducer for mechanism definitions and incidence.
pub(crate) struct MechanismEvidenceReducerV1 {
    request: CheckedMechanismObservationRequestV1,
    case_universe: ExploreCaseUniverse,
    revision: u64,
    known_target_support: ExactCaseSupport,
    exact_target: Option<ExactMatchingTargetMembership>,
    signatures: BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
    processed: ExactCaseSupport,
    signature_supports: BTreeMap<MechanismSignatureId, ExactCaseSupport>,
    untraced_supports: BTreeMap<MechanismPermanentUntracedReasonV1, ExactCaseSupport>,
    signature_bin_supports: BTreeMap<MechanismSignatureBinIncidence, ExactCaseSupport>,
    field_signature_binned_supports: BTreeMap<(Box<str>, MechanismSignatureId), ExactCaseSupport>,
    field_signature_outside_supports: BTreeMap<(Box<str>, MechanismSignatureId), ExactCaseSupport>,
    field_unavailable_supports: BTreeMap<
        (
            Box<str>,
            MechanismSignatureId,
            MechanismBinAssignmentOutcomeV1,
        ),
        ExactCaseSupport,
    >,
    retained_examples: BTreeMap<MechanismSignatureId, BTreeMap<u128, ExploreCaseId>>,
    traced: u128,
    permanently_untraced: u128,
    resource_usage: MechanismReducerResourceUsageV1,
}

impl MechanismEvidenceReducerV1 {
    pub(crate) fn new(
        request: CheckedMechanismObservationRequestV1,
    ) -> Result<Self, MechanismStreamError> {
        validate_mechanism_stream_request_v1(&request)?;
        let case_universe =
            ExploreCaseUniverse::new(request.observation.axis_cardinalities.clone())
                .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        let processed = ExactCaseSupport::empty(&case_universe);
        let known_target_support = ExactCaseSupport::empty(&case_universe);
        Ok(Self {
            request,
            case_universe,
            revision: 0,
            known_target_support,
            exact_target: None,
            signatures: BTreeMap::new(),
            processed,
            signature_supports: BTreeMap::new(),
            untraced_supports: BTreeMap::new(),
            signature_bin_supports: BTreeMap::new(),
            field_signature_binned_supports: BTreeMap::new(),
            field_signature_outside_supports: BTreeMap::new(),
            field_unavailable_supports: BTreeMap::new(),
            retained_examples: BTreeMap::new(),
            traced: 0,
            permanently_untraced: 0,
            resource_usage: MechanismReducerResourceUsageV1::default(),
        })
    }

    /// Prepare a monotone coordinator-certified lower bound on matching cases.
    /// Observations may cross the durable reducer boundary only after their
    /// ranks appear in this support.
    pub(crate) fn prepare_known_target_support(
        &self,
        next_known_target_support: ExactCaseSupport,
    ) -> Result<PreparedKnownMechanismTargetSupportV1, MechanismStreamError> {
        next_known_target_support
            .subtract_exact(&self.known_target_support)
            .map_err(|error| {
                MechanismStreamError::invalid(format!(
                    "known mechanism target support must grow monotonically in one case universe: {error}"
                ))
            })?;
        next_known_target_support
            .subtract_exact(&self.processed)
            .map_err(|error| {
                MechanismStreamError::invalid(format!(
                    "known mechanism target support does not cover already processed cases: {error}"
                ))
            })?;
        if self.exact_target.is_some() && next_known_target_support != self.known_target_support {
            return Err(MechanismStreamError::invalid(
                "known mechanism target support cannot grow after exact target closure",
            ));
        }
        Ok(PreparedKnownMechanismTargetSupportV1 {
            prior_revision: self.revision,
            next_known_target_support,
        })
    }

    pub(crate) fn known_target_support(&self) -> &ExactCaseSupport {
        &self.known_target_support
    }

    pub(crate) fn first_known_unprocessed_rank(
        &self,
    ) -> Result<Option<u128>, MechanismStreamError> {
        self.known_target_support
            .first_rank_excluding(&self.processed)
            .map_err(|error| {
                MechanismStreamError::invalid(format!(
                    "processed mechanism support is not a subset of known target support: {error}"
                ))
            })
    }

    pub(crate) fn has_exact_target(&self) -> bool {
        self.exact_target.is_some()
    }

    pub(crate) fn apply_prepared_known_target_support(
        &mut self,
        prepared: PreparedKnownMechanismTargetSupportV1,
    ) {
        assert_eq!(
            self.revision, prepared.prior_revision,
            "prepared known mechanism target support is stale"
        );
        self.known_target_support = prepared.next_known_target_support;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("bounded journal revision");
    }

    pub(crate) fn prepare_observation_batch(
        &self,
        batch: ValidatedMechanismObservationBatchV1,
    ) -> Result<PreparedMechanismObservationBatchV1, MechanismStreamError> {
        self.prepare_observation_batch_with_limits(batch, MECHANISM_REDUCER_RESOURCE_LIMITS_V1)
    }

    fn prepare_observation_batch_with_limits(
        &self,
        batch: ValidatedMechanismObservationBatchV1,
        resource_limits: MechanismReducerResourceLimitsV1,
    ) -> Result<PreparedMechanismObservationBatchV1, MechanismStreamError> {
        validate_batch_for_request(&self.request, &batch.proposal)?;

        for definition in batch.proposal.definitions.iter() {
            if let Some(existing) = self.signatures.get(&definition.id) {
                if existing != &definition.signature {
                    return Err(MechanismStreamError::invalid(
                        "mechanism signature content-hash collision rejected",
                    ));
                }
            }
        }

        let mut traced_delta = 0_u128;
        let mut untraced_delta = 0_u128;
        let mut processed_ranks = Vec::new();
        let mut signature_ranks = BTreeMap::<MechanismSignatureId, Vec<u128>>::new();
        let mut untraced_ranks = BTreeMap::<MechanismPermanentUntracedReasonV1, Vec<u128>>::new();
        let mut bin_ranks = BTreeMap::<MechanismSignatureBinIncidence, Vec<u128>>::new();
        let mut field_ranks = BTreeMap::<(Box<str>, MechanismSignatureId), Vec<u128>>::new();
        let mut outside_ranks = BTreeMap::<(Box<str>, MechanismSignatureId), Vec<u128>>::new();
        let mut unavailable_ranks = BTreeMap::<
            (
                Box<str>,
                MechanismSignatureId,
                MechanismBinAssignmentOutcomeV1,
            ),
            Vec<u128>,
        >::new();
        let mut example_candidates =
            BTreeMap::<MechanismSignatureId, Vec<(u128, ExploreCaseId)>>::new();
        for observation in batch.proposal.observations.iter() {
            validate_case_id(
                &self.request.observation.axis_cardinalities,
                self.case_universe.case_count(),
                &observation.case_id,
            )?;
            if self.processed.contains_rank(observation.case_id.rank) {
                return Err(MechanismStreamError::invalid(format!(
                    "mechanism CaseId rank {} was already observed",
                    observation.case_id.rank
                )));
            }
            if !self
                .known_target_support
                .contains_rank(observation.case_id.rank)
            {
                return Err(MechanismStreamError::invalid(format!(
                    "mechanism CaseId rank {} is not inside coordinator-confirmed known matching support",
                    observation.case_id.rank
                )));
            }
            if let Some(exact_target) = self.exact_target.as_ref() {
                require_inside_target(exact_target, &observation.case_id)?;
            }
            processed_ranks.push(observation.case_id.rank);
            match &observation.outcome {
                MechanismCaseObservationOutcomeProposalV1::Observed {
                    signature,
                    bin_assignments,
                } => {
                    traced_delta = checked_add(traced_delta, 1, "traced mechanism batch")?;
                    signature_ranks
                        .entry(signature.clone())
                        .or_default()
                        .push(observation.case_id.rank);
                    example_candidates
                        .entry(signature.clone())
                        .or_default()
                        .push((
                            observation.case_id.rank,
                            ExploreCaseId::new(observation.case_id.ordinals.clone()),
                        ));
                    for assignment in bin_assignments.iter() {
                        match assignment.outcome {
                            MechanismBinAssignmentOutcomeV1::Binned(bin) => {
                                bin_ranks
                                    .entry(MechanismSignatureBinIncidence {
                                        signature: signature.clone(),
                                        field_name: assignment.field_name.clone(),
                                        bin,
                                    })
                                    .or_default()
                                    .push(observation.case_id.rank);
                                field_ranks
                                    .entry((assignment.field_name.clone(), signature.clone()))
                                    .or_default()
                                    .push(observation.case_id.rank);
                            }
                            MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => {
                                outside_ranks
                                    .entry((assignment.field_name.clone(), signature.clone()))
                                    .or_default()
                                    .push(observation.case_id.rank);
                            }
                            unavailable @ (MechanismBinAssignmentOutcomeV1::ReplayUnavailable
                            | MechanismBinAssignmentOutcomeV1::ObservationUnsupported) => {
                                unavailable_ranks
                                    .entry((
                                        assignment.field_name.clone(),
                                        signature.clone(),
                                        unavailable,
                                    ))
                                    .or_default()
                                    .push(observation.case_id.rank);
                            }
                        }
                    }
                }
                MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(reason) => {
                    untraced_delta =
                        checked_add(untraced_delta, 1, "permanently untraced mechanism batch")?;
                    untraced_ranks
                        .entry(*reason)
                        .or_default()
                        .push(observation.case_id.rank);
                }
            }
        }
        let next_traced = checked_add(self.traced, traced_delta, "traced mechanism")?;
        let next_permanently_untraced = checked_add(
            self.permanently_untraced,
            untraced_delta,
            "permanently untraced mechanism",
        )?;
        let processed_delta = support_from_ranks(&self.case_universe, processed_ranks)?;
        let next_processed = self
            .processed
            .merge_disjoint(&processed_delta)
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        let next_signature_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.signature_supports,
            signature_ranks,
        )?;
        let next_untraced_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.untraced_supports,
            untraced_ranks,
        )?;
        let next_signature_bin_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.signature_bin_supports,
            bin_ranks,
        )?;
        let next_field_signature_binned_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.field_signature_binned_supports,
            field_ranks,
        )?;
        let next_field_signature_outside_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.field_signature_outside_supports,
            outside_ranks,
        )?;
        let next_field_unavailable_supports = merge_support_rank_deltas(
            &self.case_universe,
            &self.field_unavailable_supports,
            unavailable_ranks,
        )?;
        let next_retained_examples = self.prepare_retained_examples(example_candidates);
        let new_definitions = batch
            .proposal
            .definitions
            .iter()
            .filter(|definition| !self.signatures.contains_key(&definition.id))
            .map(|definition| (definition.id.clone(), definition.signature.clone()))
            .collect::<BTreeMap<_, _>>();
        let next_resource_usage = self.prepare_next_resource_usage(
            &new_definitions,
            &next_processed,
            &next_signature_supports,
            &next_untraced_supports,
            &next_signature_bin_supports,
            &next_field_signature_binned_supports,
            &next_field_signature_outside_supports,
            &next_field_unavailable_supports,
            &next_retained_examples,
            resource_limits,
        )?;

        Ok(PreparedMechanismObservationBatchV1 {
            prior_revision: self.revision,
            next_processed,
            new_definitions,
            next_signature_supports,
            next_untraced_supports,
            next_signature_bin_supports,
            next_field_signature_binned_supports,
            next_field_signature_outside_supports,
            next_field_unavailable_supports,
            next_retained_examples,
            next_traced,
            next_permanently_untraced,
            next_resource_usage,
        })
    }

    /// Apply only after the corresponding canonical block is durable.
    pub(crate) fn apply_prepared_observation_batch(
        &mut self,
        prepared: PreparedMechanismObservationBatchV1,
    ) {
        assert_eq!(
            self.revision, prepared.prior_revision,
            "prepared mechanism observation block is stale"
        );
        self.processed = prepared.next_processed;
        self.signatures.extend(prepared.new_definitions);
        self.signature_supports
            .extend(prepared.next_signature_supports);
        self.untraced_supports
            .extend(prepared.next_untraced_supports);
        self.signature_bin_supports
            .extend(prepared.next_signature_bin_supports);
        self.field_signature_binned_supports
            .extend(prepared.next_field_signature_binned_supports);
        self.field_signature_outside_supports
            .extend(prepared.next_field_signature_outside_supports);
        self.field_unavailable_supports
            .extend(prepared.next_field_unavailable_supports);
        self.retained_examples
            .extend(prepared.next_retained_examples);
        self.traced = prepared.next_traced;
        self.permanently_untraced = prepared.next_permanently_untraced;
        self.resource_usage = prepared.next_resource_usage;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("bounded journal revision");
    }

    /// Seal the coordinator's accumulated known-matching support as complete.
    /// The membership DAG is constructed from canonical support intervals, so
    /// the caller never enumerates ranks or supplies a second target shape.
    pub(crate) fn prepare_exact_target_from_known_support(
        &self,
        authoritative_target: &ExactClosedMatchSupportV1,
    ) -> Result<PreparedExactMechanismTargetV1, MechanismStreamError> {
        if self.exact_target.is_some() {
            return Err(MechanismStreamError::invalid(
                "exact mechanism target membership was already installed",
            ));
        }
        if authoritative_target.support() != &self.known_target_support {
            return Err(MechanismStreamError::invalid(
                "closure-gated exact target disagrees with known matching support",
            ));
        }
        validate_exact_target_lowering_capacity(
            &self.known_target_support,
            self.request.observation.axis_cardinalities.len(),
            MAX_INCIDENCE_OVERRIDE_INTERVALS,
        )?;
        let target = ExactMatchingTargetMembership::from_exact_case_support(
            &self.request.observation,
            &self.known_target_support,
        )
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        if target
            .inside_count()
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?
            != self.known_target_support.case_count()
        {
            return Err(MechanismStreamError::invalid(
                "exact mechanism target does not conserve the complete known matching support",
            ));
        }
        for selected in self.request.observation.sampling.selected_case_ids() {
            let terminal = target
                .membership
                .terminal_for_path(selected.ordinals())
                .map_err(|error| MechanismStreamError::invalid(error.to_string()))?
                .ok_or_else(|| {
                    MechanismStreamError::invalid(
                        "mechanism sampling case has no target membership terminal",
                    )
                })?;
            if terminal != &MechanismTargetMembership::InsideTarget {
                return Err(MechanismStreamError::invalid(
                    "mechanism sampling plan contains a case outside the exact matching target",
                ));
            }
        }
        Ok(PreparedExactMechanismTargetV1 {
            prior_revision: self.revision,
            target,
        })
    }

    /// Install authoritative target closure independently of answer sealing.
    pub(crate) fn apply_prepared_exact_target(&mut self, prepared: PreparedExactMechanismTargetV1) {
        assert_eq!(
            self.revision, prepared.prior_revision,
            "prepared exact mechanism target is stale"
        );
        assert!(
            self.exact_target.is_none(),
            "prepared exact mechanism target was already installed"
        );
        self.exact_target = Some(prepared.target);
        self.revision = self
            .revision
            .checked_add(1)
            .expect("bounded journal revision");
    }

    /// Project the current reducer into a bounded, count-only checkpoint.
    ///
    /// The projection repeats all population and bin conservation checks at
    /// the publication boundary. It never constructs or clones the target or
    /// incidence DAG, and it never discloses signatures or case identities.
    pub(crate) fn checkpoint_summary(
        &self,
    ) -> Result<MechanismCheckpointSummaryV1, MechanismStreamError> {
        self.checkpoint_summary_with_authoritative_target(None)
    }

    /// Count-only checkpoint projection with optional closure-gated target
    /// support from the exact reducer. Supplying authoritative support lets a
    /// checkpoint become exact without constructing the much larger target or
    /// incidence DAG first.
    pub(crate) fn checkpoint_summary_with_authoritative_target(
        &self,
        authoritative_target: Option<&ExactClosedMatchSupportV1>,
    ) -> Result<MechanismCheckpointSummaryV1, MechanismStreamError> {
        validate_mechanism_stream_request_v1(&self.request)?;

        let processed_count = checked_add(
            self.traced,
            self.permanently_untraced,
            "processed mechanism checkpoint population",
        )?;
        if self.processed.case_count() != processed_count {
            return Err(MechanismStreamError::invalid(
                "processed mechanism support disagrees with checkpoint population accounting",
            ));
        }
        let pending_support = self
            .known_target_support
            .subtract_exact(&self.processed)
            .map_err(|error| {
                MechanismStreamError::invalid(format!(
                    "processed mechanism support is not a subset of known target support: {error}"
                ))
            })?;

        if !self.signatures.keys().eq(self.signature_supports.keys()) {
            return Err(MechanismStreamError::invalid(
                "retained mechanism signatures and signature supports have different identities",
            ));
        }
        let traced_from_signatures =
            self.signature_supports
                .iter()
                .try_fold(0_u128, |total, (_, support)| {
                    if support.is_empty() {
                        return Err(MechanismStreamError::invalid(
                            "mechanism signature has empty retained support",
                        ));
                    }
                    checked_add(total, support.case_count(), "checkpoint signature support")
                })?;
        if traced_from_signatures != self.traced {
            return Err(MechanismStreamError::invalid(
                "checkpoint signature supports do not conserve the traced population",
            ));
        }

        let replay_unavailable = self
            .untraced_supports
            .get(&MechanismPermanentUntracedReasonV1::ReplayUnavailable)
            .map(ExactCaseSupport::case_count)
            .unwrap_or(0);
        let observation_unsupported = self
            .untraced_supports
            .get(&MechanismPermanentUntracedReasonV1::ObservationUnsupported)
            .map(ExactCaseSupport::case_count)
            .unwrap_or(0);
        if self
            .untraced_supports
            .values()
            .any(ExactCaseSupport::is_empty)
        {
            return Err(MechanismStreamError::invalid(
                "checkpoint permanently-untraced support must not retain empty entries",
            ));
        }
        let permanent_from_reasons = checked_add(
            replay_unavailable,
            observation_unsupported,
            "checkpoint permanently-untraced reasons",
        )?;
        if permanent_from_reasons != self.permanently_untraced {
            return Err(MechanismStreamError::invalid(
                "checkpoint untraced reasons do not conserve the permanently-untraced population",
            ));
        }

        let pending = pending_support.case_count();
        let known_target_untraced = checked_add(
            pending,
            permanent_from_reasons,
            "checkpoint known-target untraced population",
        )?;
        let known_target_count = self.known_target_support.case_count();
        if checked_add(
            self.traced,
            known_target_untraced,
            "checkpoint known target population",
        )? != known_target_count
        {
            return Err(MechanismStreamError::invalid(
                "checkpoint traced and untraced populations do not conserve known target support",
            ));
        }

        let authoritative_target_count = authoritative_target
            .map(|closed| {
                if closed.support() != &self.known_target_support {
                    return Err(MechanismStreamError::invalid(
                        "authoritative checkpoint target disagrees with confirmed matching support",
                    ));
                }
                Ok(closed.case_count())
            })
            .transpose()?;
        let materialized_target_count = self
            .exact_target
            .as_ref()
            .map(|target| {
                target
                    .inside_count()
                    .map_err(|error| MechanismStreamError::invalid(error.to_string()))
            })
            .transpose()?;
        if authoritative_target_count.is_some()
            && materialized_target_count.is_some()
            && authoritative_target_count != materialized_target_count
        {
            return Err(MechanismStreamError::invalid(
                "authoritative and materialized checkpoint targets disagree",
            ));
        }
        let exact_target_count = authoritative_target_count.or(materialized_target_count);
        let (status, target_cases) = match exact_target_count {
            None => (
                MechanismEvidenceStatus::ScopeOpen,
                MechanismCount::LowerBound(known_target_count),
            ),
            Some(exact_target_count) => {
                if exact_target_count != known_target_count {
                    return Err(MechanismStreamError::invalid(
                        "exact mechanism target disagrees with checkpoint known target support",
                    ));
                }
                let status = if known_target_untraced == 0 {
                    MechanismEvidenceStatus::MatchingClosed
                } else {
                    MechanismEvidenceStatus::IncidenceOpen
                };
                (status, MechanismCount::Exact(exact_target_count))
            }
        };

        let signature_count = self.signatures.len() as u128;
        let mechanism_signatures = checkpoint_count_for_open_population(status, signature_count);
        let bin_fields = self.checkpoint_bin_fields(status)?;
        Ok(MechanismCheckpointSummaryV1 {
            checked_request_hash: self.request.id.digest_bytes(),
            observation_spec_hash: self.request.observation.id.digest_bytes(),
            status,
            target_cases,
            traced_cases: self.traced,
            known_target_untraced: MechanismCheckpointUntracedV1 {
                total: known_target_untraced,
                pending,
                replay_unavailable,
                observation_unsupported,
            },
            mechanism_signatures,
            bin_fields,
        })
    }

    fn checkpoint_bin_fields(
        &self,
        status: MechanismEvidenceStatus,
    ) -> Result<Box<[MechanismCheckpointBinFieldV1]>, MechanismStreamError> {
        let requested_fields = self
            .request
            .observation
            .bin_fields
            .iter()
            .map(|field| (field.name.as_ref(), field))
            .collect::<BTreeMap<_, _>>();
        let mut bin_stats =
            BTreeMap::<(&str, MechanismNumericBin), MechanismCheckpointBinAccumulatorV1>::new();
        let mut binned_by_field_signature = BTreeMap::<(&str, &MechanismSignatureId), u128>::new();
        for (incidence, support) in &self.signature_bin_supports {
            let field = requested_fields
                .get(incidence.field_name.as_ref())
                .ok_or_else(|| {
                    MechanismStreamError::invalid(format!(
                        "checkpoint bin incidence references unknown field `{}`",
                        incidence.field_name
                    ))
                })?;
            if field.bins.binary_search(&incidence.bin).is_err() {
                return Err(MechanismStreamError::invalid(format!(
                    "checkpoint bin incidence for `{}` references an undeclared bin",
                    incidence.field_name
                )));
            }
            if !self.signature_supports.contains_key(&incidence.signature) {
                return Err(MechanismStreamError::invalid(
                    "checkpoint bin incidence references an unknown mechanism signature",
                ));
            }
            if support.is_empty() {
                return Err(MechanismStreamError::invalid(
                    "checkpoint bin incidence retains empty support",
                ));
            }
            let cases = support.case_count();
            add_checkpoint_map_count(
                &mut binned_by_field_signature,
                (incidence.field_name.as_ref(), &incidence.signature),
                cases,
                "checkpoint binned support by field and signature",
            )?;
            let stats = bin_stats
                .entry((incidence.field_name.as_ref(), incidence.bin))
                .or_default();
            stats.confirmed_case_support = checked_add(
                stats.confirmed_case_support,
                cases,
                "checkpoint bin case support",
            )?;
            stats.confirmed_signatures = checked_add(
                stats.confirmed_signatures,
                1,
                "checkpoint bin signature support",
            )?;
        }

        let mut classified_by_field_signature =
            BTreeMap::<(&str, &MechanismSignatureId), u128>::new();
        let mut field_totals = BTreeMap::<&str, MechanismCheckpointFieldAccumulatorV1>::new();
        for ((field_name, signature), support) in &self.field_signature_binned_supports {
            validate_checkpoint_field_signature(
                &requested_fields,
                &self.signature_supports,
                field_name,
                signature,
                support,
                "binned",
            )?;
            let cases = support.case_count();
            if binned_by_field_signature
                .get(&(field_name.as_ref(), signature))
                .copied()
                != Some(cases)
            {
                return Err(MechanismStreamError::invalid(
                    "checkpoint bin incidences do not conserve binned support by field and signature",
                ));
            }
            add_checkpoint_map_count(
                &mut classified_by_field_signature,
                (field_name.as_ref(), signature),
                cases,
                "checkpoint classified support",
            )?;
            let totals = field_totals.entry(field_name.as_ref()).or_default();
            totals.binned = checked_add(totals.binned, cases, "checkpoint binned cases")?;
        }
        if binned_by_field_signature.len() != self.field_signature_binned_supports.len() {
            return Err(MechanismStreamError::invalid(
                "checkpoint binned support maps retain different field/signature identities",
            ));
        }

        for ((field_name, signature), support) in &self.field_signature_outside_supports {
            validate_checkpoint_field_signature(
                &requested_fields,
                &self.signature_supports,
                field_name,
                signature,
                support,
                "outside-declared-bins",
            )?;
            let cases = support.case_count();
            add_checkpoint_map_count(
                &mut classified_by_field_signature,
                (field_name.as_ref(), signature),
                cases,
                "checkpoint classified support",
            )?;
            let totals = field_totals.entry(field_name.as_ref()).or_default();
            totals.outside = checked_add(
                totals.outside,
                cases,
                "checkpoint outside-declared-bins cases",
            )?;
        }

        for ((field_name, signature, outcome), support) in &self.field_unavailable_supports {
            validate_checkpoint_field_signature(
                &requested_fields,
                &self.signature_supports,
                field_name,
                signature,
                support,
                "unavailable",
            )?;
            let cases = support.case_count();
            add_checkpoint_map_count(
                &mut classified_by_field_signature,
                (field_name.as_ref(), signature),
                cases,
                "checkpoint classified support",
            )?;
            let totals = field_totals.entry(field_name.as_ref()).or_default();
            match outcome {
                MechanismBinAssignmentOutcomeV1::ReplayUnavailable => {
                    totals.replay_unavailable = checked_add(
                        totals.replay_unavailable,
                        cases,
                        "checkpoint replay-unavailable bin cases",
                    )?;
                }
                MechanismBinAssignmentOutcomeV1::ObservationUnsupported => {
                    totals.observation_unsupported = checked_add(
                        totals.observation_unsupported,
                        cases,
                        "checkpoint observation-unsupported bin cases",
                    )?;
                }
                MechanismBinAssignmentOutcomeV1::Binned(_)
                | MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => {
                    return Err(MechanismStreamError::invalid(
                        "checkpoint unavailable support contains a successful bin outcome",
                    ));
                }
            }
        }

        let expected_classifications = requested_fields
            .len()
            .checked_mul(self.signature_supports.len())
            .ok_or_else(|| {
                MechanismStreamError::invalid(
                    "checkpoint field/signature classification count exceeds usize::MAX",
                )
            })?;
        if classified_by_field_signature.len() != expected_classifications {
            return Err(MechanismStreamError::invalid(
                "checkpoint fields do not classify every traced signature support exactly once",
            ));
        }
        for ((_, signature), classified_cases) in &classified_by_field_signature {
            let signature_cases = self
                .signature_supports
                .get(*signature)
                .expect("checkpoint classification signatures were validated")
                .case_count();
            if *classified_cases != signature_cases {
                return Err(MechanismStreamError::invalid(
                    "checkpoint field classification does not conserve one signature support",
                ));
            }
        }

        let mut result = Vec::with_capacity(self.request.observation.bin_fields.len());
        for field in self.request.observation.bin_fields.iter() {
            let totals = field_totals
                .get(field.name.as_ref())
                .copied()
                .unwrap_or_default();
            let unavailable = checked_add(
                totals.replay_unavailable,
                totals.observation_unsupported,
                "checkpoint unavailable bin cases",
            )?;
            let classified = checked_add(
                checked_add(
                    totals.binned,
                    totals.outside,
                    "checkpoint classified bin cases",
                )?,
                unavailable,
                "checkpoint classified bin cases",
            )?;
            if classified != self.traced {
                return Err(MechanismStreamError::invalid(format!(
                    "checkpoint bin field `{}` does not conserve the traced population",
                    field.name
                )));
            }

            let mut binned_from_bins = 0_u128;
            let mut bins = Vec::with_capacity(field.bins.len());
            for bin in field.bins.iter().copied() {
                let stats = bin_stats
                    .get(&(field.name.as_ref(), bin))
                    .copied()
                    .unwrap_or_default();
                binned_from_bins = checked_add(
                    binned_from_bins,
                    stats.confirmed_case_support,
                    "checkpoint declared-bin support",
                )?;
                bins.push(MechanismCheckpointBinV1 {
                    bin,
                    confirmed_case_support: stats.confirmed_case_support,
                    mechanism_count: checkpoint_bin_count(
                        status,
                        unavailable,
                        stats.confirmed_signatures,
                    ),
                });
            }
            if binned_from_bins != totals.binned {
                return Err(MechanismStreamError::invalid(format!(
                    "checkpoint declared bins for `{}` do not conserve binned case support",
                    field.name
                )));
            }
            result.push(MechanismCheckpointBinFieldV1 {
                name: field.name.clone(),
                binned_cases: totals.binned,
                outside_declared_bins_cases: totals.outside,
                unavailable_cases: unavailable,
                replay_unavailable_cases: totals.replay_unavailable,
                observation_unsupported_cases: totals.observation_unsupported,
                bins: bins.into_boxed_slice(),
            });
        }
        Ok(result.into_boxed_slice())
    }

    pub(crate) fn snapshot(&self) -> Result<MechanismObservedEvidence, MechanismStreamError> {
        let processed_count = checked_add(
            self.traced,
            self.permanently_untraced,
            "processed mechanism population",
        )?;
        if self.processed.case_count() != processed_count {
            return Err(MechanismStreamError::invalid(
                "processed mechanism support disagrees with traced and permanently-untraced accounting",
            ));
        }
        let (status, requested_target, known_target_untraced, incidence, exact_target) =
            match self.exact_target.as_ref() {
                None => {
                    let known_target = self.known_target_support.case_count();
                    if self.traced > known_target {
                        return Err(MechanismStreamError::invalid(
                            "traced mechanism population exceeds known matching support",
                        ));
                    }
                    (
                        MechanismEvidenceStatus::ScopeOpen,
                        MechanismCount::LowerBound(known_target),
                        known_target - self.traced,
                        None,
                        None,
                    )
                }
                Some(target) => {
                    let target_count = target
                        .inside_count()
                        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
                    if target_count != self.known_target_support.case_count() {
                        return Err(MechanismStreamError::invalid(
                            "exact mechanism target disagrees with complete known matching support",
                        ));
                    }
                    if self.traced > target_count {
                        return Err(MechanismStreamError::invalid(
                            "traced mechanism population exceeds exact target",
                        ));
                    }
                    let known_target_untraced = target_count - self.traced;
                    let status = if known_target_untraced == 0 {
                        MechanismEvidenceStatus::MatchingClosed
                    } else {
                        MechanismEvidenceStatus::IncidenceOpen
                    };
                    let incidence = self.build_incidence(target)?;
                    (
                        status,
                        MechanismCount::Exact(target_count),
                        known_target_untraced,
                        Some(incidence),
                        Some(target.clone()),
                    )
                }
            };

        let population = MechanismPopulationEvidence::new(
            status,
            requested_target,
            self.traced,
            known_target_untraced,
            incidence,
        )
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        let sampled_traces = self.retained_examples();
        let bin_fields = self.snapshot_bin_fields();
        MechanismObservedEvidence::new(
            self.request.clone(),
            population,
            exact_target,
            self.signatures.clone(),
            self.signature_supports
                .iter()
                .map(|(signature, support)| (signature.clone(), support.case_count()))
                .collect(),
            sampled_traces,
            bin_fields,
            self.signature_bin_supports
                .iter()
                .map(|(incidence, support)| (incidence.clone(), support.case_count()))
                .collect(),
        )
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))
    }

    fn build_incidence(
        &self,
        target: &ExactMatchingTargetMembership,
    ) -> Result<super::mechanism::MechanismIncidenceDag, MechanismStreamError> {
        let override_intervals = validate_incidence_override_capacity(
            self.signature_supports
                .values()
                .chain(self.untraced_supports.values()),
            MAX_INCIDENCE_OVERRIDE_INTERVALS,
        )?;
        let dimension_steps = override_intervals
            .checked_mul(self.request.observation.axis_cardinalities.len().max(1))
            .unwrap_or(usize::MAX);
        if dimension_steps > MAX_INCIDENCE_OVERRIDE_INTERVALS {
            return Err(MechanismStreamError::snapshot_capacity(
                "incidence rank-interval dimension steps",
                dimension_steps,
                MAX_INCIDENCE_OVERRIDE_INTERVALS,
            ));
        }
        let base = target
            .membership
            .project_terminals(|terminal| match terminal {
                MechanismTargetMembership::OutsideTarget => {
                    MechanismIncidenceTerminal::OutsideTarget
                }
                MechanismTargetMembership::InsideTarget => {
                    MechanismIncidenceTerminal::KnownTargetUntraced(
                        KnownTargetUntracedReason::Pending,
                    )
                }
            })
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        let signature_overrides =
            self.signature_supports
                .iter()
                .flat_map(|(signature, support)| {
                    support.iter_intervals().map(move |interval| {
                        (
                            interval.start(),
                            interval.end_exclusive(),
                            MechanismIncidenceTerminal::Signature(signature.clone()),
                        )
                    })
                });
        let untraced_overrides = self.untraced_supports.iter().flat_map(|(reason, support)| {
            support.iter_intervals().map(move |interval| {
                (
                    interval.start(),
                    interval.end_exclusive(),
                    MechanismIncidenceTerminal::KnownTargetUntraced(reason.incidence_reason()),
                )
            })
        });
        base.with_rank_interval_overrides(signature_overrides.chain(untraced_overrides))
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))
    }

    fn retained_examples(&self) -> BTreeMap<ExploreCaseId, MechanismSignatureId> {
        let cap = self.request.disclosure.retained_examples_per_signature as usize;
        if cap == 0 {
            return BTreeMap::new();
        }
        let mut retained = BTreeMap::new();
        for (signature, examples) in &self.retained_examples {
            for case_id in examples.values().take(cap) {
                retained.insert(case_id.clone(), signature.clone());
            }
        }
        retained
    }

    fn snapshot_bin_fields(&self) -> BTreeMap<Box<str>, MechanismBinFieldEvidence> {
        let mut result = BTreeMap::new();
        for field in self.request.observation.bin_fields.iter() {
            let observed_supports = self
                .field_signature_binned_supports
                .iter()
                .filter(|((name, _), _)| name == &field.name)
                .map(|((_, signature), support)| (signature.clone(), support.case_count()))
                .collect::<BTreeMap<_, _>>();
            let outside_declared_bins_supports = self
                .field_signature_outside_supports
                .iter()
                .filter(|((name, _), _)| name == &field.name)
                .map(|((_, signature), support)| (signature.clone(), support.case_count()))
                .collect::<BTreeMap<_, _>>();
            let unavailable_supports = self
                .field_unavailable_supports
                .iter()
                .filter(|((name, _, _), _)| name == &field.name)
                .map(|((_, signature, outcome), support)| {
                    (
                        MechanismBinUnavailableSupport {
                            signature: signature.clone(),
                            reason: match outcome {
                                MechanismBinAssignmentOutcomeV1::ReplayUnavailable => {
                                    MechanismBinUnavailableReason::ValueReplayUnavailable
                                }
                                MechanismBinAssignmentOutcomeV1::ObservationUnsupported => {
                                    MechanismBinUnavailableReason::ValueUnsupported
                                }
                                MechanismBinAssignmentOutcomeV1::Binned(_)
                                | MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => {
                                    unreachable!(
                                        "only unavailable outcomes enter unavailable support"
                                    )
                                }
                            },
                        },
                        support.case_count(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let evidence = MechanismBinFieldEvidence::Observed {
                observed_supports,
                outside_declared_bins_supports,
                unavailable_supports,
            };
            result.insert(field.name.clone(), evidence);
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_next_resource_usage(
        &self,
        new_definitions: &BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
        next_processed: &ExactCaseSupport,
        next_signature_supports: &BTreeMap<MechanismSignatureId, ExactCaseSupport>,
        next_untraced_supports: &BTreeMap<MechanismPermanentUntracedReasonV1, ExactCaseSupport>,
        next_signature_bin_supports: &BTreeMap<MechanismSignatureBinIncidence, ExactCaseSupport>,
        next_field_signature_binned_supports: &BTreeMap<
            (Box<str>, MechanismSignatureId),
            ExactCaseSupport,
        >,
        next_field_signature_outside_supports: &BTreeMap<
            (Box<str>, MechanismSignatureId),
            ExactCaseSupport,
        >,
        next_field_unavailable_supports: &BTreeMap<
            (
                Box<str>,
                MechanismSignatureId,
                MechanismBinAssignmentOutcomeV1,
            ),
            ExactCaseSupport,
        >,
        next_retained_examples: &BTreeMap<MechanismSignatureId, BTreeMap<u128, ExploreCaseId>>,
        limits: MechanismReducerResourceLimitsV1,
    ) -> Result<MechanismReducerResourceUsageV1, MechanismStreamError> {
        let mut next = self.resource_usage;
        next.unique_signatures = checked_reducer_usage_add(
            next.unique_signatures,
            new_definitions.len(),
            "unique signatures",
            limits.unique_signatures,
        )?;
        let (added_nodes, added_activation_steps, added_edges) =
            signature_collection_resource_counts(new_definitions.values())?;
        next.signature_nodes = checked_reducer_usage_add(
            next.signature_nodes,
            added_nodes,
            "retained signature nodes",
            limits.signature_nodes,
        )?;
        next.signature_activation_steps = checked_reducer_usage_add(
            next.signature_activation_steps,
            added_activation_steps,
            "retained signature activation steps",
            limits.signature_activation_steps,
        )?;
        next.signature_edges = checked_reducer_usage_add(
            next.signature_edges,
            added_edges,
            "retained signature edges",
            limits.signature_edges,
        )?;

        let added_support_entries = [
            count_new_support_keys(&self.signature_supports, next_signature_supports),
            count_new_support_keys(&self.untraced_supports, next_untraced_supports),
            count_new_support_keys(&self.signature_bin_supports, next_signature_bin_supports),
            count_new_support_keys(
                &self.field_signature_binned_supports,
                next_field_signature_binned_supports,
            ),
            count_new_support_keys(
                &self.field_signature_outside_supports,
                next_field_signature_outside_supports,
            ),
            count_new_support_keys(
                &self.field_unavailable_supports,
                next_field_unavailable_supports,
            ),
        ]
        .into_iter()
        .try_fold(0_usize, |total, count| {
            total.checked_add(count).ok_or_else(|| {
                MechanismStreamError::reducer_capacity(
                    "support-map entries",
                    usize::MAX,
                    limits.support_map_entries,
                )
            })
        })?;
        next.support_map_entries = checked_reducer_usage_add(
            next.support_map_entries,
            added_support_entries,
            "support-map entries",
            limits.support_map_entries,
        )?;

        next.support_intervals = replace_reducer_usage_count(
            next.support_intervals,
            self.processed.interval_count(),
            next_processed.interval_count(),
            "support intervals",
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.signature_supports,
            next_signature_supports,
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.untraced_supports,
            next_untraced_supports,
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.signature_bin_supports,
            next_signature_bin_supports,
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.field_signature_binned_supports,
            next_field_signature_binned_supports,
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.field_signature_outside_supports,
            next_field_signature_outside_supports,
            limits.support_intervals,
        )?;
        next.support_intervals = adjusted_support_interval_usage(
            next.support_intervals,
            &self.field_unavailable_supports,
            next_field_unavailable_supports,
            limits.support_intervals,
        )?;

        for (signature, examples) in next_retained_examples {
            next.retained_examples = replace_reducer_usage_count(
                next.retained_examples,
                self.retained_examples
                    .get(signature)
                    .map(BTreeMap::len)
                    .unwrap_or(0),
                examples.len(),
                "retained examples",
                limits.retained_examples,
            )?;
        }
        validate_reducer_resource_usage(next, limits)?;
        Ok(next)
    }

    fn prepare_retained_examples(
        &self,
        candidates: BTreeMap<MechanismSignatureId, Vec<(u128, ExploreCaseId)>>,
    ) -> BTreeMap<MechanismSignatureId, BTreeMap<u128, ExploreCaseId>> {
        let cap = self.request.disclosure.retained_examples_per_signature as usize;
        if cap == 0 {
            return BTreeMap::new();
        }
        candidates
            .into_iter()
            .map(|(signature, candidates)| {
                let mut examples = self
                    .retained_examples
                    .get(&signature)
                    .cloned()
                    .unwrap_or_default();
                examples.extend(candidates);
                while examples.len() > cap {
                    examples.pop_last();
                }
                (signature, examples)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct MechanismCheckpointBinAccumulatorV1 {
    confirmed_case_support: u128,
    confirmed_signatures: u128,
}

#[derive(Debug, Clone, Copy, Default)]
struct MechanismCheckpointFieldAccumulatorV1 {
    binned: u128,
    outside: u128,
    replay_unavailable: u128,
    observation_unsupported: u128,
}

fn checkpoint_count_for_open_population(
    status: MechanismEvidenceStatus,
    confirmed: u128,
) -> MechanismCheckpointCountV1 {
    if status == MechanismEvidenceStatus::MatchingClosed {
        MechanismCheckpointCountV1::Exact(confirmed)
    } else if confirmed == 0 {
        MechanismCheckpointCountV1::Unknown {
            confirmed_lower_bound: 0,
        }
    } else {
        MechanismCheckpointCountV1::LowerBound(confirmed)
    }
}

fn checkpoint_bin_count(
    status: MechanismEvidenceStatus,
    unavailable_cases: u128,
    confirmed: u128,
) -> MechanismCheckpointCountV1 {
    if status == MechanismEvidenceStatus::MatchingClosed && unavailable_cases == 0 {
        MechanismCheckpointCountV1::Exact(confirmed)
    } else if confirmed == 0 {
        MechanismCheckpointCountV1::Unknown {
            confirmed_lower_bound: 0,
        }
    } else {
        MechanismCheckpointCountV1::LowerBound(confirmed)
    }
}

fn add_checkpoint_map_count<K: Ord>(
    counts: &mut BTreeMap<K, u128>,
    key: K,
    value: u128,
    what: &str,
) -> Result<(), MechanismStreamError> {
    let total = counts.entry(key).or_insert(0);
    *total = checked_add(*total, value, what)?;
    Ok(())
}

fn validate_checkpoint_field_signature(
    requested_fields: &BTreeMap<&str, &MechanismBinField>,
    signature_supports: &BTreeMap<MechanismSignatureId, ExactCaseSupport>,
    field_name: &str,
    signature: &MechanismSignatureId,
    support: &ExactCaseSupport,
    support_kind: &str,
) -> Result<(), MechanismStreamError> {
    if !requested_fields.contains_key(field_name) {
        return Err(MechanismStreamError::invalid(format!(
            "checkpoint {support_kind} support references unknown field `{field_name}`"
        )));
    }
    if !signature_supports.contains_key(signature) {
        return Err(MechanismStreamError::invalid(format!(
            "checkpoint {support_kind} support references an unknown mechanism signature"
        )));
    }
    if support.is_empty() {
        return Err(MechanismStreamError::invalid(format!(
            "checkpoint {support_kind} support must not be empty"
        )));
    }
    Ok(())
}

fn signature_collection_resource_counts<'a>(
    signatures: impl IntoIterator<Item = &'a DynamicMechanismSignature>,
) -> Result<(usize, usize, usize), MechanismStreamError> {
    let mut nodes = 0_usize;
    let mut activation_steps = 0_usize;
    let mut edges = 0_usize;
    for signature in signatures {
        nodes = nodes
            .checked_add(signature.nodes.len())
            .ok_or_else(|| MechanismStreamError::invalid("signature node count overflow"))?;
        edges = edges
            .checked_add(signature.before_roots.len())
            .and_then(|count| count.checked_add(signature.after_roots.len()))
            .ok_or_else(|| MechanismStreamError::invalid("signature edge count overflow"))?;
        for node in signature.nodes.values() {
            activation_steps = activation_steps
                .checked_add(node.slot.activation_path.len())
                .ok_or_else(|| {
                    MechanismStreamError::invalid("signature activation-step count overflow")
                })?;
            edges = edges
                .checked_add(node.before_dependencies.len())
                .and_then(|count| count.checked_add(node.after_dependencies.len()))
                .ok_or_else(|| MechanismStreamError::invalid("signature edge count overflow"))?;
        }
    }
    Ok((nodes, activation_steps, edges))
}

fn count_new_support_keys<K: Ord>(
    existing: &BTreeMap<K, ExactCaseSupport>,
    replacements: &BTreeMap<K, ExactCaseSupport>,
) -> usize {
    replacements
        .keys()
        .filter(|key| !existing.contains_key(*key))
        .count()
}

fn adjusted_support_interval_usage<K: Ord>(
    mut current: usize,
    existing: &BTreeMap<K, ExactCaseSupport>,
    replacements: &BTreeMap<K, ExactCaseSupport>,
    limit: usize,
) -> Result<usize, MechanismStreamError> {
    for (key, replacement) in replacements {
        current = replace_reducer_usage_count(
            current,
            existing
                .get(key)
                .map(ExactCaseSupport::interval_count)
                .unwrap_or(0),
            replacement.interval_count(),
            "support intervals",
            limit,
        )?;
    }
    Ok(current)
}

fn checked_reducer_usage_add(
    current: usize,
    added: usize,
    resource: &'static str,
    limit: usize,
) -> Result<usize, MechanismStreamError> {
    let actual = current.checked_add(added).unwrap_or(usize::MAX);
    if actual > limit {
        return Err(MechanismStreamError::reducer_capacity(
            resource, actual, limit,
        ));
    }
    Ok(actual)
}

fn replace_reducer_usage_count(
    current: usize,
    removed: usize,
    added: usize,
    resource: &'static str,
    limit: usize,
) -> Result<usize, MechanismStreamError> {
    // Do not compare one replacement with `limit`: later touched supports may
    // coalesce intervals and reduce the aggregate. The exact final usage is
    // checked once by `validate_reducer_resource_usage`.
    let without_removed = current.checked_sub(removed).ok_or_else(|| {
        MechanismStreamError::invalid(format!("mechanism reducer {resource} counter underflow"))
    })?;
    without_removed
        .checked_add(added)
        .ok_or_else(|| MechanismStreamError::reducer_capacity(resource, usize::MAX, limit))
}

fn validate_reducer_resource_usage(
    usage: MechanismReducerResourceUsageV1,
    limits: MechanismReducerResourceLimitsV1,
) -> Result<(), MechanismStreamError> {
    for (resource, actual, limit) in [
        (
            "unique signatures",
            usage.unique_signatures,
            limits.unique_signatures,
        ),
        (
            "retained signature nodes",
            usage.signature_nodes,
            limits.signature_nodes,
        ),
        (
            "retained signature activation steps",
            usage.signature_activation_steps,
            limits.signature_activation_steps,
        ),
        (
            "retained signature edges",
            usage.signature_edges,
            limits.signature_edges,
        ),
        (
            "support-map entries",
            usage.support_map_entries,
            limits.support_map_entries,
        ),
        (
            "support intervals",
            usage.support_intervals,
            limits.support_intervals,
        ),
        (
            "retained examples",
            usage.retained_examples,
            limits.retained_examples,
        ),
    ] {
        if actual > limit {
            return Err(MechanismStreamError::reducer_capacity(
                resource, actual, limit,
            ));
        }
    }
    Ok(())
}

fn validate_incidence_override_capacity<'a>(
    supports: impl IntoIterator<Item = &'a ExactCaseSupport>,
    limit: usize,
) -> Result<usize, MechanismStreamError> {
    let mut total = 0_usize;
    for support in supports {
        total = total.checked_add(support.interval_count()).ok_or_else(|| {
            MechanismStreamError::snapshot_capacity(
                "incidence override intervals",
                usize::MAX,
                limit,
            )
        })?;
        if total > limit {
            return Err(MechanismStreamError::snapshot_capacity(
                "incidence override intervals",
                total,
                limit,
            ));
        }
    }
    Ok(total)
}

fn validate_exact_target_lowering_capacity(
    support: &ExactCaseSupport,
    axis_count: usize,
    limit: usize,
) -> Result<(), MechanismStreamError> {
    let actual = support
        .interval_count()
        .checked_mul(axis_count.max(1))
        .unwrap_or(usize::MAX);
    if actual > limit {
        return Err(MechanismStreamError::snapshot_capacity(
            "target rank-interval dimension steps",
            actual,
            limit,
        ));
    }
    Ok(())
}

fn require_inside_target(
    target: &ExactMatchingTargetMembership,
    case_id: &MechanismCanonicalCaseIdV1,
) -> Result<(), MechanismStreamError> {
    let terminal = target
        .membership
        .terminal_for_path(&case_id.ordinals)
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?
        .ok_or_else(|| MechanismStreamError::invalid("case has no target membership terminal"))?;
    if terminal != &MechanismTargetMembership::InsideTarget {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism CaseId rank {} is outside the exact matching target",
            case_id.rank
        )));
    }
    Ok(())
}

fn support_from_ranks(
    universe: &ExploreCaseUniverse,
    mut ranks: Vec<u128>,
) -> Result<ExactCaseSupport, MechanismStreamError> {
    ranks.sort_unstable();
    for pair in ranks.windows(2) {
        if pair[0] == pair[1] {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism support contains duplicate rank {}",
                pair[0]
            )));
        }
    }
    ExactCaseSupport::new(universe, ranks.into_iter().map(|rank| (rank, rank + 1)))
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))
}

fn merge_support_rank_deltas<K: Clone + Ord>(
    universe: &ExploreCaseUniverse,
    existing: &BTreeMap<K, ExactCaseSupport>,
    deltas: BTreeMap<K, Vec<u128>>,
) -> Result<BTreeMap<K, ExactCaseSupport>, MechanismStreamError> {
    deltas
        .into_iter()
        .map(|(key, ranks)| {
            let delta = support_from_ranks(universe, ranks)?;
            let next = match existing.get(&key) {
                Some(current) => current
                    .merge_disjoint(&delta)
                    .map_err(|error| MechanismStreamError::invalid(error.to_string()))?,
                None => delta,
            };
            Ok((key, next))
        })
        .collect()
}

fn normalize_definitions(
    request: &MechanismObservationRequest,
    definitions: Box<[DynamicMechanismSignature]>,
) -> Result<Box<[MechanismSignatureDefinitionProposalV1]>, MechanismStreamError> {
    if definitions.len() > MAX_SIGNATURES_PER_BATCH {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism signature batch length {} exceeds limit {MAX_SIGNATURES_PER_BATCH}",
            definitions.len()
        )));
    }
    let mut interner = CanonicalSignatureInterner::new(request);
    for definition in definitions.into_vec() {
        interner
            .intern(definition)
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
    }
    Ok(interner
        .into_signatures()
        .into_iter()
        .map(|(id, signature)| MechanismSignatureDefinitionProposalV1 { id, signature })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn normalize_observation(
    request: &CheckedMechanismObservationRequestV1,
    observation: &mut MechanismCaseObservationProposalV1,
) -> Result<(), MechanismStreamError> {
    validate_case_id(
        &request.observation.axis_cardinalities,
        checked_case_count(&request.observation.axis_cardinalities)?,
        &observation.case_id,
    )?;
    match &mut observation.outcome {
        MechanismCaseObservationOutcomeProposalV1::Observed {
            bin_assignments, ..
        } => {
            let mut assignments = std::mem::take(bin_assignments).into_vec();
            assignments.sort_by(|left, right| left.field_name.cmp(&right.field_name));
            validate_bin_assignments(&request.observation, &assignments)?;
            *bin_assignments = assignments.into_boxed_slice();
        }
        MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(_) => {}
    }
    Ok(())
}

fn validate_bin_assignments(
    request: &MechanismObservationRequest,
    assignments: &[MechanismBinAssignmentV1],
) -> Result<(), MechanismStreamError> {
    if assignments.len() != request.bin_fields.len() {
        return Err(MechanismStreamError::invalid(format!(
            "observed mechanism case has {} bin assignments; request requires {}",
            assignments.len(),
            request.bin_fields.len()
        )));
    }
    if assignments.len() > MAX_BIN_FIELDS {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism bin assignment count {} exceeds limit {MAX_BIN_FIELDS}",
            assignments.len()
        )));
    }
    for pair in assignments.windows(2) {
        if pair[0].field_name >= pair[1].field_name {
            return Err(MechanismStreamError::invalid(
                "mechanism bin assignments have duplicate or noncanonical field names",
            ));
        }
    }
    let requested = request
        .bin_fields
        .iter()
        .map(|field| (field.name.as_ref(), field))
        .collect::<BTreeMap<_, _>>();
    for assignment in assignments {
        validate_text(&assignment.field_name, "mechanism bin field")?;
        let field = requested
            .get(assignment.field_name.as_ref())
            .ok_or_else(|| {
                MechanismStreamError::invalid(format!(
                    "mechanism bin assignment references unknown field `{}`",
                    assignment.field_name
                ))
            })?;
        if let MechanismBinAssignmentOutcomeV1::Binned(bin) = assignment.outcome {
            MechanismNumericBin::new(bin.lower_inclusive, bin.upper_exclusive)
                .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
            if field.bins.binary_search(&bin).is_err() {
                return Err(MechanismStreamError::invalid(format!(
                    "mechanism bin assignment for `{}` references an undeclared bin",
                    assignment.field_name
                )));
            }
        }
    }
    Ok(())
}

fn validate_definition_references(
    definitions: &[MechanismSignatureDefinitionProposalV1],
    observations: &[MechanismCaseObservationProposalV1],
) -> Result<(), MechanismStreamError> {
    let defined = definitions
        .iter()
        .map(|definition| definition.id.clone())
        .collect::<BTreeSet<_>>();
    let referenced = observations
        .iter()
        .filter_map(|observation| match &observation.outcome {
            MechanismCaseObservationOutcomeProposalV1::Observed { signature, .. } => {
                Some(signature.clone())
            }
            MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(_) => None,
        })
        .collect::<BTreeSet<_>>();
    if defined != referenced {
        return Err(MechanismStreamError::invalid(
            "mechanism batch must include exactly the signature definitions referenced by its observed cases",
        ));
    }
    Ok(())
}

fn validate_signature_resource_bounds(
    definitions: &[MechanismSignatureDefinitionProposalV1],
) -> Result<(), MechanismStreamError> {
    let mut nodes = 0_usize;
    let mut edges = 0_usize;
    for definition in definitions {
        definition
            .signature
            .validate()
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        nodes = nodes
            .checked_add(definition.signature.nodes.len())
            .ok_or_else(|| MechanismStreamError::invalid("mechanism node count overflow"))?;
        for node in definition.signature.nodes.values() {
            if node.slot.activation_path.len() > MAX_ACTIVATION_DEPTH {
                return Err(MechanismStreamError::invalid(format!(
                    "mechanism activation depth {} exceeds limit {MAX_ACTIVATION_DEPTH}",
                    node.slot.activation_path.len()
                )));
            }
            edges = edges
                .checked_add(node.before_dependencies.len())
                .and_then(|value| value.checked_add(node.after_dependencies.len()))
                .ok_or_else(|| MechanismStreamError::invalid("mechanism edge count overflow"))?;
        }
        edges = edges
            .checked_add(definition.signature.before_roots.len())
            .and_then(|value| value.checked_add(definition.signature.after_roots.len()))
            .ok_or_else(|| MechanismStreamError::invalid("mechanism edge count overflow"))?;
    }
    if nodes > MAX_SIGNATURE_NODES_PER_BATCH {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism signature node count {nodes} exceeds limit {MAX_SIGNATURE_NODES_PER_BATCH}"
        )));
    }
    if edges > MAX_SIGNATURE_EDGES_PER_BATCH {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism signature edge count {edges} exceeds limit {MAX_SIGNATURE_EDGES_PER_BATCH}"
        )));
    }
    Ok(())
}

fn validate_observation_sequence(
    observations: &[MechanismCaseObservationProposalV1],
) -> Result<(), MechanismStreamError> {
    validate_nonempty_len(
        "mechanism observation",
        observations.len(),
        MAX_OBSERVATIONS_PER_BATCH,
    )?;
    for pair in observations.windows(2) {
        if pair[0].case_id.rank >= pair[1].case_id.rank {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism observation batch has duplicate or noncanonical ranks {} and {}",
                pair[0].case_id.rank, pair[1].case_id.rank
            )));
        }
    }
    Ok(())
}

fn validate_batch_for_request(
    request: &CheckedMechanismObservationRequestV1,
    batch: &MechanismObservationBatchProposalV1,
) -> Result<(), MechanismStreamError> {
    validate_mechanism_stream_request_v1(request)?;
    if batch.checked_request_id != request.id {
        return Err(MechanismStreamError::invalid(
            "mechanism observation batch belongs to another checked request",
        ));
    }
    if batch.definitions.len() > MAX_SIGNATURES_PER_BATCH {
        return Err(MechanismStreamError::invalid(
            "mechanism signature batch exceeds its fixed limit",
        ));
    }
    for pair in batch.definitions.windows(2) {
        if pair[0].id >= pair[1].id {
            return Err(MechanismStreamError::invalid(
                "mechanism signature definitions are duplicate or not sorted by content ID",
            ));
        }
    }
    let mut interner = CanonicalSignatureInterner::new(&request.observation);
    for definition in batch.definitions.iter() {
        let derived = interner
            .intern(definition.signature.clone())
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        if derived != definition.id {
            return Err(MechanismStreamError::invalid(
                "mechanism signature definition ID disagrees with its canonical content",
            ));
        }
    }
    let universe = checked_case_count(&request.observation.axis_cardinalities)?;
    for observation in batch.observations.iter() {
        validate_case_id(
            &request.observation.axis_cardinalities,
            universe,
            &observation.case_id,
        )?;
        if let MechanismCaseObservationOutcomeProposalV1::Observed {
            bin_assignments, ..
        } = &observation.outcome
        {
            validate_bin_assignments(&request.observation, bin_assignments)?;
        }
    }
    validate_observation_sequence(&batch.observations)?;
    validate_definition_references(&batch.definitions, &batch.observations)?;
    validate_signature_resource_bounds(&batch.definitions)
}

fn validate_full_incidence_authorization(
    request: &CheckedMechanismObservationRequestV1,
) -> Result<(), MechanismStreamError> {
    if request.disclosure.incidence != MechanismIncidenceDisclosure::FullMatchingIncidence {
        return Err(MechanismStreamError::invalid(
            "case-ranked durable mechanism batches require full matching-incidence disclosure; summary-only needs a redacted aggregate protocol",
        ));
    }
    Ok(())
}

pub(crate) fn validate_mechanism_stream_request_v1(
    request: &CheckedMechanismObservationRequestV1,
) -> Result<(), MechanismStreamError> {
    validate_full_incidence_authorization(request)?;
    if request.observation.axis_cardinalities.len() > MAX_AXES {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism universe has {} axes; limit is {MAX_AXES}",
            request.observation.axis_cardinalities.len()
        )));
    }
    checked_case_count(&request.observation.axis_cardinalities)?;
    if request.observation.bin_fields.len() > MAX_BIN_FIELDS {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism request has {} bin fields; limit is {MAX_BIN_FIELDS}",
            request.observation.bin_fields.len()
        )));
    }
    for field in request.observation.bin_fields.iter() {
        validate_text(&field.name, "mechanism bin field")?;
    }
    validate_bin_count_limits(
        request
            .observation
            .bin_fields
            .iter()
            .map(|field| field.bins.len()),
        MAX_BINS_PER_FIELD,
        MAX_TOTAL_BINS,
    )?;
    if request.disclosure.retained_examples_per_signature as usize
        > MAX_RETAINED_EXAMPLES_PER_SIGNATURE
    {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism retained examples per signature {} exceeds limit {MAX_RETAINED_EXAMPLES_PER_SIGNATURE}",
            request.disclosure.retained_examples_per_signature
        )));
    }
    validate_selected_sampling_count(&request.observation, MAX_SELECTED_SAMPLING_CASES)?;
    request
        .validate()
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
    Ok(())
}

fn validate_bin_count_limits(
    field_bin_counts: impl IntoIterator<Item = usize>,
    per_field_limit: usize,
    total_limit: usize,
) -> Result<usize, MechanismStreamError> {
    let mut total = 0_usize;
    for count in field_bin_counts {
        if count > per_field_limit {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism bin field has {count} bins; limit is {per_field_limit}"
            )));
        }
        total = total
            .checked_add(count)
            .ok_or_else(|| MechanismStreamError::invalid("mechanism total bin count overflow"))?;
        if total > total_limit {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism request has {total} total bins; limit is {total_limit}"
            )));
        }
    }
    Ok(total)
}

fn validate_selected_sampling_count(
    request: &MechanismObservationRequest,
    limit: usize,
) -> Result<usize, MechanismStreamError> {
    let mut selected = BTreeSet::new();
    for case_id in request
        .sampling
        .result_representatives
        .iter()
        .chain(&request.sampling.extrema_witnesses)
        .chain(&request.sampling.required_case_ids)
    {
        selected.insert(case_id);
        if selected.len() > limit {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism request has more than {limit} unique selected sampling cases"
            )));
        }
    }
    Ok(selected.len())
}

fn checked_case_count(cardinalities: &[u128]) -> Result<u128, MechanismStreamError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities
        .iter()
        .copied()
        .try_fold(1_u128, |total, cardinality| {
            total.checked_mul(cardinality).ok_or_else(|| {
                MechanismStreamError::invalid("mechanism case universe exceeds u128::MAX")
            })
        })
}

fn validate_case_id(
    cardinalities: &[u128],
    case_count: u128,
    case_id: &MechanismCanonicalCaseIdV1,
) -> Result<(), MechanismStreamError> {
    if case_id.rank >= case_count {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism CaseId rank {} is outside universe cardinality {case_count}",
            case_id.rank
        )));
    }
    if case_id.ordinals.len() != cardinalities.len() {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism CaseId rank {} has {} ordinals for {} axes",
            case_id.rank,
            case_id.ordinals.len(),
            cardinalities.len()
        )));
    }
    let mut computed_rank = 0_u128;
    for (axis, (&ordinal, &cardinality)) in case_id.ordinals.iter().zip(cardinalities).enumerate() {
        if ordinal >= cardinality {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism CaseId ordinal {ordinal} at axis {axis} is outside cardinality {cardinality}"
            )));
        }
        computed_rank = computed_rank
            .checked_mul(cardinality)
            .and_then(|prefix| prefix.checked_add(ordinal))
            .ok_or_else(|| MechanismStreamError::invalid("mechanism CaseId rank overflow"))?;
    }
    if computed_rank != case_id.rank {
        return Err(MechanismStreamError::invalid(format!(
            "mechanism CaseId rank {} disagrees with canonical mixed-radix rank {computed_rank}",
            case_id.rank
        )));
    }
    Ok(())
}

fn validate_nonempty_len(
    name: &str,
    actual: usize,
    limit: usize,
) -> Result<(), MechanismStreamError> {
    if actual == 0 {
        return Err(MechanismStreamError::invalid(format!(
            "{name} batch must not be empty"
        )));
    }
    if actual > limit {
        return Err(MechanismStreamError::invalid(format!(
            "{name} batch length {actual} exceeds limit {limit}"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, what: &str) -> Result<(), MechanismStreamError> {
    if value.is_empty() {
        return Err(MechanismStreamError::invalid(format!(
            "{what} must not be empty"
        )));
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(MechanismStreamError::invalid(format!(
            "{what} has {} UTF-8 bytes; limit is {MAX_TEXT_BYTES}",
            value.len()
        )));
    }
    Ok(())
}

fn checked_add(left: u128, right: u128, what: &str) -> Result<u128, MechanismStreamError> {
    left.checked_add(right)
        .ok_or_else(|| MechanismStreamError::invalid(format!("{what} exceeds u128::MAX")))
}

/// Encode one independently replayable mechanism block. No serde or Rust
/// layout participates in this format.
pub(crate) fn encode_mechanism_observation_batch_v1(
    request: &CheckedMechanismObservationRequestV1,
    batch: &MechanismObservationBatchProposalV1,
) -> Result<Vec<u8>, MechanismStreamError> {
    validate_mechanism_stream_request_v1(request)?;
    validate_batch_for_request(request, batch)?;
    let mut writer = CanonicalWriter::new();
    writer.fixed(MECHANISM_BATCH_MAGIC_V1)?;
    writer.fixed(&request.id.digest_bytes())?;
    writer.len(
        batch.definitions.len(),
        MAX_SIGNATURES_PER_BATCH,
        "mechanism signatures",
    )?;
    for definition in batch.definitions.iter() {
        write_signature(&mut writer, &definition.signature)?;
    }
    let definition_indexes = batch
        .definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    writer.len(
        batch.observations.len(),
        MAX_OBSERVATIONS_PER_BATCH,
        "mechanism observations",
    )?;
    for observation in batch.observations.iter() {
        write_case_id(&mut writer, &observation.case_id)?;
        writer.fixed(&observation.validation_receipt_digest.0)?;
        match &observation.outcome {
            MechanismCaseObservationOutcomeProposalV1::Observed {
                signature,
                bin_assignments,
            } => {
                writer.u8(0)?;
                let index = definition_indexes.get(signature).ok_or_else(|| {
                    MechanismStreamError::invalid(
                        "observed mechanism signature lacks its block definition",
                    )
                })?;
                writer.u32(usize_to_u32(*index, "mechanism signature index")?)?;
                writer.len(
                    bin_assignments.len(),
                    MAX_BIN_FIELDS,
                    "mechanism bin assignments",
                )?;
                for assignment in bin_assignments.iter() {
                    writer.text(&assignment.field_name, "mechanism bin field")?;
                    match assignment.outcome {
                        MechanismBinAssignmentOutcomeV1::Binned(bin) => {
                            writer.u8(0)?;
                            writer.i64(bin.lower_inclusive)?;
                            writer.i64(bin.upper_exclusive)?;
                        }
                        MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => writer.u8(1)?,
                        MechanismBinAssignmentOutcomeV1::ReplayUnavailable => writer.u8(2)?,
                        MechanismBinAssignmentOutcomeV1::ObservationUnsupported => writer.u8(3)?,
                    }
                }
            }
            MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(reason) => {
                writer.u8(match reason {
                    MechanismPermanentUntracedReasonV1::ReplayUnavailable => 1,
                    MechanismPermanentUntracedReasonV1::ObservationUnsupported => 2,
                })?;
            }
        }
    }
    Ok(writer.finish())
}

/// Decode only the unique bounded v1 representation. The returned value is
/// still a proposal and cannot enter a reducer before trusted confirmation.
pub(crate) fn decode_mechanism_observation_batch_v1(
    request: &CheckedMechanismObservationRequestV1,
    bytes: &[u8],
) -> Result<MechanismObservationBatchProposalV1, MechanismStreamError> {
    validate_mechanism_stream_request_v1(request)?;
    let mut reader = CanonicalReader::new(bytes)?;
    reader.magic(MECHANISM_BATCH_MAGIC_V1, "mechanism observation batch")?;
    if reader.array_32()? != request.id.digest_bytes() {
        return Err(MechanismStreamError::invalid(
            "mechanism observation bytes belong to another checked request",
        ));
    }
    let definition_count = reader.len(MAX_SIGNATURES_PER_BATCH, "mechanism signatures")?;
    let mut definitions = Vec::new();
    definitions
        .try_reserve(definition_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism signatures"))?;
    for _ in 0..definition_count {
        definitions.push(read_signature(&mut reader, &request.observation)?);
    }
    let mut interner = CanonicalSignatureInterner::new(&request.observation);
    let mut definition_ids = Vec::new();
    for definition in definitions.iter().cloned() {
        definition_ids.push(
            interner
                .intern(definition)
                .map_err(|error| MechanismStreamError::invalid(error.to_string()))?,
        );
    }

    let observation_count = reader.len(MAX_OBSERVATIONS_PER_BATCH, "mechanism observations")?;
    if observation_count == 0 {
        return Err(MechanismStreamError::invalid(
            "mechanism observation batch must not be empty",
        ));
    }
    let mut observations = Vec::new();
    observations
        .try_reserve(observation_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism observations"))?;
    for _ in 0..observation_count {
        let case_id = read_case_id(&mut reader)?;
        let validation_receipt_digest = MechanismValidationReceiptDigestV1(reader.array_32()?);
        let outcome = match reader.u8()? {
            0 => {
                let definition_index = reader.u32()? as usize;
                let signature = definition_ids
                    .get(definition_index)
                    .cloned()
                    .ok_or_else(|| {
                        MechanismStreamError::invalid(format!(
                            "mechanism signature index {definition_index} is out of bounds"
                        ))
                    })?;
                let assignment_count = reader.len(MAX_BIN_FIELDS, "mechanism bin assignments")?;
                let mut assignments = Vec::new();
                assignments.try_reserve(assignment_count).map_err(|_| {
                    MechanismStreamError::invalid("cannot allocate mechanism bin assignments")
                })?;
                for _ in 0..assignment_count {
                    let field_name = reader.text("mechanism bin field")?;
                    let assignment = match reader.u8()? {
                        0 => MechanismBinAssignmentV1::binned(
                            field_name,
                            MechanismNumericBin::new(reader.i64()?, reader.i64()?).map_err(
                                |error| MechanismStreamError::invalid(error.to_string()),
                            )?,
                        ),
                        1 => MechanismBinAssignmentV1::outside_declared_bins(field_name),
                        2 => MechanismBinAssignmentV1::unavailable(
                            field_name,
                            MechanismBinAssignmentOutcomeV1::ReplayUnavailable,
                        )?,
                        3 => MechanismBinAssignmentV1::unavailable(
                            field_name,
                            MechanismBinAssignmentOutcomeV1::ObservationUnsupported,
                        )?,
                        tag => {
                            return Err(MechanismStreamError::invalid(format!(
                                "invalid mechanism bin outcome tag {tag}"
                            )))
                        }
                    };
                    assignments.push(assignment);
                }
                MechanismCaseObservationOutcomeProposalV1::Observed {
                    signature,
                    bin_assignments: assignments.into_boxed_slice(),
                }
            }
            1 => MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(
                MechanismPermanentUntracedReasonV1::ReplayUnavailable,
            ),
            2 => MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(
                MechanismPermanentUntracedReasonV1::ObservationUnsupported,
            ),
            tag => {
                return Err(MechanismStreamError::invalid(format!(
                    "invalid mechanism case outcome tag {tag}"
                )))
            }
        };
        observations.push(MechanismCaseObservationProposalV1 {
            case_id,
            outcome,
            validation_receipt_digest,
        });
    }
    reader.finish()?;
    let proposal = MechanismObservationBatchProposalV1::new(request, definitions, observations)?;
    let canonical = encode_mechanism_observation_batch_v1(request, &proposal)?;
    if canonical.as_slice() != bytes {
        return Err(MechanismStreamError::invalid(
            "mechanism observation bytes are not the canonical v1 encoding",
        ));
    }
    Ok(proposal)
}

fn write_case_id(
    writer: &mut CanonicalWriter,
    case_id: &MechanismCanonicalCaseIdV1,
) -> Result<(), MechanismStreamError> {
    writer.u128(case_id.rank)?;
    writer.len(
        case_id.ordinals.len(),
        MAX_AXES,
        "mechanism CaseId ordinals",
    )?;
    for ordinal in case_id.ordinals.iter().copied() {
        writer.u128(ordinal)?;
    }
    Ok(())
}

fn read_case_id(
    reader: &mut CanonicalReader<'_>,
) -> Result<MechanismCanonicalCaseIdV1, MechanismStreamError> {
    let rank = reader.u128()?;
    let ordinal_count = reader.len(MAX_AXES, "mechanism CaseId ordinals")?;
    let mut ordinals = Vec::new();
    ordinals
        .try_reserve(ordinal_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism CaseId"))?;
    for _ in 0..ordinal_count {
        ordinals.push(reader.u128()?);
    }
    Ok(MechanismCanonicalCaseIdV1::new(rank, ordinals))
}

fn write_signature(
    writer: &mut CanonicalWriter,
    signature: &DynamicMechanismSignature,
) -> Result<(), MechanismStreamError> {
    signature
        .validate()
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
    let mut nodes = signature.nodes.values().collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        left.slot
            .cmp(&right.slot)
            .then_with(|| left.id.cmp(&right.id))
    });
    let indexes = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    writer.len(
        nodes.len(),
        MAX_SIGNATURE_NODES_PER_BATCH,
        "mechanism signature nodes",
    )?;
    for node in nodes.iter() {
        write_slot(writer, &node.slot)?;
        write_optional_outcome(writer, node.slot.kind, node.before.as_ref())?;
        write_optional_outcome(writer, node.slot.kind, node.after.as_ref())?;
        write_occurrence_indexes(writer, &node.before_dependencies, &indexes)?;
        write_occurrence_indexes(writer, &node.after_dependencies, &indexes)?;
    }
    write_occurrence_indexes(writer, &signature.before_roots, &indexes)?;
    write_occurrence_indexes(writer, &signature.after_roots, &indexes)
}

fn write_occurrence_indexes(
    writer: &mut CanonicalWriter,
    occurrences: &BTreeSet<MechanismOccurrenceId>,
    indexes: &BTreeMap<MechanismOccurrenceId, usize>,
) -> Result<(), MechanismStreamError> {
    writer.len(
        occurrences.len(),
        MAX_SIGNATURE_EDGES_PER_BATCH,
        "mechanism occurrence references",
    )?;
    for occurrence in occurrences {
        let index = indexes.get(occurrence).ok_or_else(|| {
            MechanismStreamError::invalid("mechanism signature references an absent occurrence")
        })?;
        writer.u32(usize_to_u32(*index, "mechanism occurrence index")?)?;
    }
    Ok(())
}

#[derive(Debug)]
struct WireMechanismNodeV1 {
    slot: MechanismOccurrenceSlotV1,
    before: Option<DynamicEventOutcome>,
    after: Option<DynamicEventOutcome>,
    before_dependencies: Vec<usize>,
    after_dependencies: Vec<usize>,
}

fn read_signature(
    reader: &mut CanonicalReader<'_>,
    request: &MechanismObservationRequest,
) -> Result<DynamicMechanismSignature, MechanismStreamError> {
    let node_count = reader.len(MAX_SIGNATURE_NODES_PER_BATCH, "mechanism signature nodes")?;
    let mut wire_nodes = Vec::new();
    wire_nodes
        .try_reserve(node_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism nodes"))?;
    for _ in 0..node_count {
        let slot = read_slot(reader, request)?;
        let before = read_optional_outcome(reader, request, slot.kind)?;
        let after = read_optional_outcome(reader, request, slot.kind)?;
        let before_dependencies = read_occurrence_indexes(reader)?;
        let after_dependencies = read_occurrence_indexes(reader)?;
        wire_nodes.push(WireMechanismNodeV1 {
            slot,
            before,
            after,
            before_dependencies,
            after_dependencies,
        });
    }
    let before_root_indexes = read_occurrence_indexes(reader)?;
    let after_root_indexes = read_occurrence_indexes(reader)?;

    let mut occurrence_ids = Vec::new();
    occurrence_ids
        .try_reserve(node_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism occurrence IDs"))?;
    for wire in wire_nodes.iter() {
        let skeleton = PairedOccurrenceNode::new(
            request,
            wire.slot.clone(),
            wire.before.clone(),
            wire.after.clone(),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        occurrence_ids.push(skeleton.id);
    }

    let resolve =
        |indexes: &[usize]| -> Result<BTreeSet<MechanismOccurrenceId>, MechanismStreamError> {
            indexes
                .iter()
                .map(|index| {
                    occurrence_ids.get(*index).cloned().ok_or_else(|| {
                        MechanismStreamError::invalid(format!(
                            "mechanism occurrence index {index} is out of bounds"
                        ))
                    })
                })
                .collect()
        };
    let before_roots = resolve(&before_root_indexes)?;
    let after_roots = resolve(&after_root_indexes)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve(node_count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism nodes"))?;
    for wire in wire_nodes {
        nodes.push(
            PairedOccurrenceNode::new(
                request,
                wire.slot,
                wire.before,
                wire.after,
                resolve(&wire.before_dependencies)?,
                resolve(&wire.after_dependencies)?,
            )
            .map_err(|error| MechanismStreamError::invalid(error.to_string()))?,
        );
    }
    DynamicMechanismSignature::new(request, before_roots, after_roots, nodes)
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))
}

fn read_occurrence_indexes(
    reader: &mut CanonicalReader<'_>,
) -> Result<Vec<usize>, MechanismStreamError> {
    let count = reader.len(
        MAX_SIGNATURE_EDGES_PER_BATCH,
        "mechanism occurrence references",
    )?;
    let mut indexes = Vec::new();
    indexes
        .try_reserve(count)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism references"))?;
    for _ in 0..count {
        indexes.push(reader.u32()? as usize);
    }
    Ok(indexes)
}

fn write_slot(
    writer: &mut CanonicalWriter,
    slot: &MechanismOccurrenceSlotV1,
) -> Result<(), MechanismStreamError> {
    writer.u32(slot.root_index)?;
    writer.len(
        slot.activation_path.len(),
        MAX_ACTIVATION_DEPTH,
        "mechanism activation path",
    )?;
    for step in slot.activation_path.iter() {
        write_site(writer, &step.call_site)?;
        match &step.callee {
            MechanismCallableSiteId::Function(site) => {
                writer.u8(0)?;
                write_site(writer, site)?;
            }
            MechanismCallableSiteId::RuleFamily(site) => {
                writer.u8(1)?;
                write_site(writer, site)?;
            }
        }
        writer.u32(step.invocation_ordinal)?;
    }
    write_site(writer, &slot.site)?;
    writer.u8(event_kind_tag(slot.kind))?;
    writer.u32(slot.visit_ordinal)
}

fn read_slot(
    reader: &mut CanonicalReader<'_>,
    request: &MechanismObservationRequest,
) -> Result<MechanismOccurrenceSlotV1, MechanismStreamError> {
    let root_index = reader.u32()?;
    let path_len = reader.len(MAX_ACTIVATION_DEPTH, "mechanism activation path")?;
    let mut activation_path = Vec::new();
    activation_path
        .try_reserve(path_len)
        .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism activation path"))?;
    for _ in 0..path_len {
        let call_site = read_site(reader, request)?;
        let callee_tag = reader.u8()?;
        let callee_site = read_site_after_tag(reader, request, callee_tag)?;
        let callee = match callee_site.0 {
            0 => MechanismCallableSiteId::function(callee_site.1),
            1 => MechanismCallableSiteId::rule_family(callee_site.1),
            _ => unreachable!("read_site_after_tag accepts only known tags"),
        }
        .map_err(|error| MechanismStreamError::invalid(error.to_string()))?;
        let invocation_ordinal = reader.u32()?;
        activation_path.push(
            MechanismActivationStepV1::new(request, call_site, callee, invocation_ordinal)
                .map_err(|error| MechanismStreamError::invalid(error.to_string()))?,
        );
    }
    let site = read_site(reader, request)?;
    let kind = decode_event_kind(reader.u8()?)?;
    let visit_ordinal = reader.u32()?;
    MechanismOccurrenceSlotV1::new(
        request,
        root_index,
        activation_path,
        site,
        kind,
        visit_ordinal,
    )
    .map_err(|error| MechanismStreamError::invalid(error.to_string()))
}

fn read_site_after_tag(
    reader: &mut CanonicalReader<'_>,
    request: &MechanismObservationRequest,
    tag: u8,
) -> Result<(u8, MechanismSiteId), MechanismStreamError> {
    if tag > 1 {
        return Err(MechanismStreamError::invalid(format!(
            "invalid mechanism callee tag {tag}"
        )));
    }
    Ok((tag, read_site(reader, request)?))
}

fn write_site(
    writer: &mut CanonicalWriter,
    site: &MechanismSiteId,
) -> Result<(), MechanismStreamError> {
    writer.u8(site_kind_tag(site.kind()))?;
    writer.fixed(&site.digest_bytes())
}

fn read_site(
    reader: &mut CanonicalReader<'_>,
    request: &MechanismObservationRequest,
) -> Result<MechanismSiteId, MechanismStreamError> {
    let kind = decode_site_kind(reader.u8()?)?;
    MechanismSiteId::from_untrusted_digest(
        request.analysis_program.clone(),
        kind,
        reader.array_32()?,
    )
    .map_err(|error| MechanismStreamError::invalid(error.to_string()))
}

fn write_optional_outcome(
    writer: &mut CanonicalWriter,
    kind: DynamicEventKind,
    outcome: Option<&DynamicEventOutcome>,
) -> Result<(), MechanismStreamError> {
    let Some(outcome) = outcome else {
        writer.u8(0)?;
        return Ok(());
    };
    writer.u8(1)?;
    match (kind, outcome) {
        (DynamicEventKind::RuleAttempt, DynamicEventOutcome::RuleAttempt(outcome)) => {
            writer.u8(match outcome {
                RuleAttemptOutcome::HeadMismatch => 0,
                RuleAttemptOutcome::GuardFalse => 1,
                RuleAttemptOutcome::BodyFalse => 2,
                RuleAttemptOutcome::Applicable => 3,
            })?;
        }
        (DynamicEventKind::RuleSelection, DynamicEventOutcome::RuleSelection(outcome)) => {
            match outcome {
                RuleSelectionOutcome::NoApplicableRule => writer.u8(0)?,
                RuleSelectionOutcome::Selected(site) => {
                    writer.u8(1)?;
                    write_site(writer, site)?;
                }
            }
        }
        (DynamicEventKind::IfDecision, DynamicEventOutcome::IfDecision(outcome)) => {
            writer.u8(match outcome {
                IfDecisionOutcome::Then => 0,
                IfDecisionOutcome::Else => 1,
            })?;
        }
        (DynamicEventKind::MatchDecision, DynamicEventOutcome::MatchDecision { arm_index }) => {
            writer.u32(*arm_index)?
        }
        (
            DynamicEventKind::ShortCircuitAnd | DynamicEventKind::ShortCircuitOr,
            DynamicEventOutcome::ShortCircuit(outcome),
        ) => match outcome {
            ShortCircuitOutcome::SkippedRight { result } => {
                writer.u8(0)?;
                writer.bool(*result)?;
            }
            ShortCircuitOutcome::EvaluatedRight { result } => {
                writer.u8(1)?;
                writer.bool(*result)?;
            }
        },
        _ => {
            return Err(MechanismStreamError::invalid(
                "dynamic mechanism event outcome has the wrong kind",
            ))
        }
    }
    Ok(())
}

fn read_optional_outcome(
    reader: &mut CanonicalReader<'_>,
    request: &MechanismObservationRequest,
    kind: DynamicEventKind,
) -> Result<Option<DynamicEventOutcome>, MechanismStreamError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => {
            let outcome = match kind {
                DynamicEventKind::RuleAttempt => {
                    DynamicEventOutcome::RuleAttempt(match reader.u8()? {
                        0 => RuleAttemptOutcome::HeadMismatch,
                        1 => RuleAttemptOutcome::GuardFalse,
                        2 => RuleAttemptOutcome::BodyFalse,
                        3 => RuleAttemptOutcome::Applicable,
                        tag => {
                            return Err(MechanismStreamError::invalid(format!(
                                "invalid rule-attempt outcome tag {tag}"
                            )))
                        }
                    })
                }
                DynamicEventKind::RuleSelection => {
                    DynamicEventOutcome::RuleSelection(match reader.u8()? {
                        0 => RuleSelectionOutcome::NoApplicableRule,
                        1 => RuleSelectionOutcome::Selected(read_site(reader, request)?),
                        tag => {
                            return Err(MechanismStreamError::invalid(format!(
                                "invalid rule-selection outcome tag {tag}"
                            )))
                        }
                    })
                }
                DynamicEventKind::IfDecision => {
                    DynamicEventOutcome::IfDecision(match reader.u8()? {
                        0 => IfDecisionOutcome::Then,
                        1 => IfDecisionOutcome::Else,
                        tag => {
                            return Err(MechanismStreamError::invalid(format!(
                                "invalid if-decision outcome tag {tag}"
                            )))
                        }
                    })
                }
                DynamicEventKind::MatchDecision => DynamicEventOutcome::MatchDecision {
                    arm_index: reader.u32()?,
                },
                DynamicEventKind::ShortCircuitAnd | DynamicEventKind::ShortCircuitOr => {
                    DynamicEventOutcome::ShortCircuit(match reader.u8()? {
                        0 => ShortCircuitOutcome::SkippedRight {
                            result: reader.bool()?,
                        },
                        1 => ShortCircuitOutcome::EvaluatedRight {
                            result: reader.bool()?,
                        },
                        tag => {
                            return Err(MechanismStreamError::invalid(format!(
                                "invalid short-circuit outcome tag {tag}"
                            )))
                        }
                    })
                }
            };
            Ok(Some(outcome))
        }
        tag => Err(MechanismStreamError::invalid(format!(
            "invalid optional mechanism outcome tag {tag}"
        ))),
    }
}

fn site_kind_tag(kind: MechanismSiteKind) -> u8 {
    match kind {
        MechanismSiteKind::Expression => 0,
        MechanismSiteKind::Callable => 1,
        MechanismSiteKind::RuleFamily => 2,
        MechanismSiteKind::RuleCandidate => 3,
    }
}

fn decode_site_kind(tag: u8) -> Result<MechanismSiteKind, MechanismStreamError> {
    match tag {
        0 => Ok(MechanismSiteKind::Expression),
        1 => Ok(MechanismSiteKind::Callable),
        2 => Ok(MechanismSiteKind::RuleFamily),
        3 => Ok(MechanismSiteKind::RuleCandidate),
        _ => Err(MechanismStreamError::invalid(format!(
            "invalid mechanism site-kind tag {tag}"
        ))),
    }
}

fn event_kind_tag(kind: DynamicEventKind) -> u8 {
    match kind {
        DynamicEventKind::RuleAttempt => 0,
        DynamicEventKind::RuleSelection => 1,
        DynamicEventKind::IfDecision => 2,
        DynamicEventKind::MatchDecision => 3,
        DynamicEventKind::ShortCircuitAnd => 4,
        DynamicEventKind::ShortCircuitOr => 5,
    }
}

fn decode_event_kind(tag: u8) -> Result<DynamicEventKind, MechanismStreamError> {
    match tag {
        0 => Ok(DynamicEventKind::RuleAttempt),
        1 => Ok(DynamicEventKind::RuleSelection),
        2 => Ok(DynamicEventKind::IfDecision),
        3 => Ok(DynamicEventKind::MatchDecision),
        4 => Ok(DynamicEventKind::ShortCircuitAnd),
        5 => Ok(DynamicEventKind::ShortCircuitOr),
        _ => Err(MechanismStreamError::invalid(format!(
            "invalid dynamic event-kind tag {tag}"
        ))),
    }
}

fn usize_to_u32(value: usize, what: &str) -> Result<u32, MechanismStreamError> {
    u32::try_from(value).map_err(|_| {
        MechanismStreamError::invalid(format!("{what} {value} exceeds the v1 u32 boundary"))
    })
}

struct CanonicalWriter {
    bytes: Vec<u8>,
}

impl CanonicalWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), MechanismStreamError> {
        let next =
            self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
                MechanismStreamError::invalid("mechanism wire byte length overflow")
            })?;
        if next > MAX_BATCH_BYTES {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism batch has more than {MAX_BATCH_BYTES} bytes"
            )));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| MechanismStreamError::invalid("cannot allocate mechanism wire bytes"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn fixed(&mut self, bytes: &[u8]) -> Result<(), MechanismStreamError> {
        self.append(bytes)
    }

    fn u8(&mut self, value: u8) -> Result<(), MechanismStreamError> {
        self.append(&[value])
    }

    fn bool(&mut self, value: bool) -> Result<(), MechanismStreamError> {
        self.u8(u8::from(value))
    }

    fn u32(&mut self, value: u32) -> Result<(), MechanismStreamError> {
        self.append(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), MechanismStreamError> {
        self.append(&value.to_le_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<(), MechanismStreamError> {
        self.append(&value.to_le_bytes())
    }

    fn len(&mut self, value: usize, limit: usize, what: &str) -> Result<(), MechanismStreamError> {
        if value > limit {
            return Err(MechanismStreamError::invalid(format!(
                "{what} length {value} exceeds limit {limit}"
            )));
        }
        self.u32(usize_to_u32(value, what)?)
    }

    fn text(&mut self, value: &str, what: &str) -> Result<(), MechanismStreamError> {
        validate_text(value, what)?;
        self.len(value.len(), MAX_TEXT_BYTES, what)?;
        self.append(value.as_bytes())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct CanonicalReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> CanonicalReader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, MechanismStreamError> {
        if bytes.len() > MAX_BATCH_BYTES {
            return Err(MechanismStreamError::invalid(format!(
                "mechanism batch has {} bytes; limit is {MAX_BATCH_BYTES}",
                bytes.len()
            )));
        }
        Ok(Self { bytes, cursor: 0 })
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], MechanismStreamError> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or_else(|| MechanismStreamError::invalid("mechanism wire cursor overflow"))?;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            MechanismStreamError::invalid("mechanism wire payload ended unexpectedly")
        })?;
        self.cursor = end;
        Ok(bytes)
    }

    fn magic(&mut self, expected: &[u8], what: &str) -> Result<(), MechanismStreamError> {
        if self.take(expected.len())? != expected {
            return Err(MechanismStreamError::invalid(format!(
                "invalid {what} magic/version"
            )));
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, MechanismStreamError> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, MechanismStreamError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(MechanismStreamError::invalid(format!(
                "invalid canonical Boolean tag {tag}"
            ))),
        }
    }

    fn u32(&mut self) -> Result<u32, MechanismStreamError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn u128(&mut self) -> Result<u128, MechanismStreamError> {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(self.take(16)?);
        Ok(u128::from_le_bytes(bytes))
    }

    fn i64(&mut self) -> Result<i64, MechanismStreamError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(i64::from_le_bytes(bytes))
    }

    fn array_32(&mut self) -> Result<[u8; 32], MechanismStreamError> {
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(self.take(32)?);
        Ok(bytes)
    }

    fn len(&mut self, limit: usize, what: &str) -> Result<usize, MechanismStreamError> {
        let value = self.u32()? as usize;
        if value > limit {
            return Err(MechanismStreamError::invalid(format!(
                "{what} length {value} exceeds limit {limit}"
            )));
        }
        Ok(value)
    }

    fn text(&mut self, what: &str) -> Result<Box<str>, MechanismStreamError> {
        let len = self.len(MAX_TEXT_BYTES, what)?;
        let text = std::str::from_utf8(self.take(len)?)
            .map_err(|_| MechanismStreamError::invalid(format!("{what} is not valid UTF-8")))?;
        validate_text(text, what)?;
        Ok(text.into())
    }

    fn finish(self) -> Result<(), MechanismStreamError> {
        if self.cursor != self.bytes.len() {
            return Err(MechanismStreamError::invalid(
                "mechanism wire payload has trailing bytes",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::mechanism::{
        CheckedMechanismObservationRequestV1, MechanismBinField, MechanismDisclosureV1,
        MechanismIncidenceDisclosure, MechanismNormalization, MechanismObservationIr,
        MechanismObservationTarget, MechanismQueryId, MechanismSamplingPlan,
        MechanismSemanticRootId,
    };
    use super::*;
    use crate::{
        AnalysisProgramId, CheckedCallableId, CheckedDeclarationOccurrenceId, DeclarationId,
        DeclarationKind, ExprSiteId, ModuleId, Ty,
    };

    fn analysis_program() -> AnalysisProgramId {
        AnalysisProgramId("31".repeat(32).into_boxed_str())
    }

    fn declaration(name: &str) -> DeclarationId {
        DeclarationId {
            module: ModuleId {
                content_hash: "42".repeat(32).into_boxed_str(),
                internal_path: Box::default(),
            },
            kind: DeclarationKind::Function,
            owner: None,
            name: name.to_string().into_boxed_str(),
            arity: Some(1),
            ordinal: 0,
        }
    }

    fn site(program: &AnalysisProgramId, name: &str, path: u32) -> MechanismSiteId {
        MechanismSiteId::from_expression_site(&ExprSiteId {
            analysis_program: program.clone(),
            declaration: declaration(name),
            normalized_declaration_ordinal: 0,
            ast_path: vec![path].into_boxed_slice(),
        })
        .expect("expression site")
    }

    fn observation_template(program: &AnalysisProgramId) -> MechanismObservationIr {
        let template_site = ExprSiteId {
            analysis_program: program.clone(),
            declaration: declaration("income-policy"),
            normalized_declaration_ordinal: 0,
            ast_path: vec![30].into_boxed_slice(),
        };
        let template_root = MechanismSemanticRootId::from_site(
            MechanismSiteId::from_expression_site(&template_site).expect("template site"),
        )
        .expect("template root");
        MechanismObservationIr {
            endpoint_template: CheckedCallableId {
                declaration: CheckedDeclarationOccurrenceId {
                    declaration: declaration("income-policy"),
                    declaration_occurrence_ordinal: 0,
                    normalized_ordinal: 0,
                },
                structural_path: Box::default(),
            },
            template_site,
            template_root: template_root.clone(),
            state_type: Ty::Name("State".to_string()),
            context_type: Ty::Name("Context".to_string()),
            observation_type: Ty::Name("Observation".to_string()),
            dependency_roots: vec![template_root].into_boxed_slice(),
            normalization_version: 1,
        }
    }

    fn checked_request(
        axis_cardinalities: Vec<u128>,
        with_bins: bool,
    ) -> CheckedMechanismObservationRequestV1 {
        checked_request_with_disclosure(
            axis_cardinalities,
            with_bins,
            MechanismIncidenceDisclosure::FullMatchingIncidence,
        )
    }

    fn checked_request_with_disclosure(
        axis_cardinalities: Vec<u128>,
        with_bins: bool,
        incidence: MechanismIncidenceDisclosure,
    ) -> CheckedMechanismObservationRequestV1 {
        let program = analysis_program();
        let bin_fields = if with_bins {
            vec![MechanismBinField::new(
                "loss",
                MechanismSemanticRootId::from_site(site(&program, "loss", 3)).expect("loss root"),
                vec![
                    MechanismNumericBin::new(0, 50).expect("bin"),
                    MechanismNumericBin::new(50, 100).expect("bin"),
                ],
            )
            .expect("bin field")]
        } else {
            Vec::new()
        };
        checked_request_with_bin_fields(axis_cardinalities, bin_fields, incidence)
    }

    fn checked_request_with_bin_fields(
        axis_cardinalities: Vec<u128>,
        bin_fields: Vec<MechanismBinField>,
        incidence: MechanismIncidenceDisclosure,
    ) -> CheckedMechanismObservationRequestV1 {
        let program = analysis_program();
        let observation = MechanismObservationRequest::new(
            program.clone(),
            MechanismQueryId::from_checked_query_bytes(b"mechanism-stream-test-query"),
            MechanismObservationTarget::MatchingConfigurations,
            observation_template(&program),
            MechanismNormalization::DynamicControlV1,
            axis_cardinalities,
            MechanismSamplingPlan::empty(),
            bin_fields,
        )
        .expect("observation request");
        CheckedMechanismObservationRequestV1::new(
            observation,
            MechanismDisclosureV1::new(incidence, 2),
        )
        .expect("checked request")
    }

    fn signature(
        request: &MechanismObservationRequest,
        outcome: IfDecisionOutcome,
    ) -> (MechanismSignatureId, DynamicMechanismSignature) {
        signature_with_activation_path(request, outcome, Vec::new())
    }

    fn signature_with_activation_path(
        request: &MechanismObservationRequest,
        outcome: IfDecisionOutcome,
        activation_path: Vec<MechanismActivationStepV1>,
    ) -> (MechanismSignatureId, DynamicMechanismSignature) {
        let node = PairedOccurrenceNode::new(
            request,
            MechanismOccurrenceSlotV1::new(
                request,
                0,
                activation_path,
                site(&request.analysis_program, "branch", 4),
                DynamicEventKind::IfDecision,
                0,
            )
            .expect("slot"),
            Some(DynamicEventOutcome::IfDecision(outcome)),
            Some(DynamicEventOutcome::IfDecision(outcome)),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("node");
        let signature = DynamicMechanismSignature::new(
            request,
            BTreeSet::from([node.id.clone()]),
            BTreeSet::from([node.id.clone()]),
            [node],
        )
        .expect("signature");
        let mut interner = CanonicalSignatureInterner::new(request);
        let id = interner.intern(signature.clone()).expect("signature ID");
        (id, signature)
    }

    fn activation_step(request: &MechanismObservationRequest) -> MechanismActivationStepV1 {
        let callable = CheckedCallableId {
            declaration: CheckedDeclarationOccurrenceId {
                declaration: declaration("nested-income-policy"),
                declaration_occurrence_ordinal: 0,
                normalized_ordinal: 0,
            },
            structural_path: Box::default(),
        };
        let callable_site = MechanismSiteId::from_callable(&request.analysis_program, &callable)
            .expect("activation callable");
        MechanismActivationStepV1::new(
            request,
            site(&request.analysis_program, "nested-call", 12),
            MechanismCallableSiteId::function(callable_site).expect("activation callee"),
            0,
        )
        .expect("activation step")
    }

    fn case(rank: u128) -> MechanismCanonicalCaseIdV1 {
        MechanismCanonicalCaseIdV1::new(rank, vec![rank])
    }

    fn receipt(rank: u128) -> MechanismValidationReceiptDigestV1 {
        MechanismValidationReceiptDigestV1::new([rank as u8; 32])
    }

    fn observed(
        rank: u128,
        signature: MechanismSignatureId,
        assignments: Vec<MechanismBinAssignmentV1>,
    ) -> MechanismCaseObservationProposalV1 {
        MechanismCaseObservationProposalV1::observed(
            case(rank),
            signature,
            assignments,
            receipt(rank),
        )
    }

    fn validated_batch(
        request: &CheckedMechanismObservationRequestV1,
        definitions: Vec<DynamicMechanismSignature>,
        observations: Vec<MechanismCaseObservationProposalV1>,
    ) -> ValidatedMechanismObservationBatchV1 {
        let proposal = MechanismObservationBatchProposalV1::new(request, definitions, observations)
            .expect("proposal");
        seal_fresh_replay_confirmed_mechanism_batch_v1(request, proposal, |_| Ok(()))
            .expect("validated batch")
    }

    fn apply(
        reducer: &mut MechanismEvidenceReducerV1,
        batch: ValidatedMechanismObservationBatchV1,
    ) {
        let prepared = reducer
            .prepare_observation_batch(batch)
            .expect("prepared batch");
        reducer.apply_prepared_observation_batch(prepared);
    }

    fn known_support(
        reducer: &MechanismEvidenceReducerV1,
        intervals: impl IntoIterator<Item = (u128, u128)>,
    ) -> ExactCaseSupport {
        ExactCaseSupport::new(&reducer.case_universe, intervals).expect("known support")
    }

    fn sync_known_support(reducer: &mut MechanismEvidenceReducerV1, support: ExactCaseSupport) {
        let prepared = reducer
            .prepare_known_target_support(support)
            .expect("prepared known support");
        reducer.apply_prepared_known_target_support(prepared);
    }

    fn install_target(reducer: &mut MechanismEvidenceReducerV1) {
        let full = ExactCaseSupport::full(&reducer.case_universe);
        let authoritative = ExactClosedMatchSupportV1::from_support_for_test(full.clone());
        sync_known_support(reducer, full);
        let prepared = reducer
            .prepare_exact_target_from_known_support(&authoritative)
            .expect("prepared target");
        reducer.apply_prepared_exact_target(prepared);
    }

    #[test]
    fn shared_signature_support_is_exact_after_matching_closes() {
        let request = checked_request(vec![2], false);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![
                observed(1, id.clone(), vec![]),
                observed(0, id.clone(), vec![]),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).expect("reducer");
        install_target(&mut reducer);
        apply(&mut reducer, batch);

        let snapshot = reducer.snapshot().expect("snapshot");
        assert_eq!(
            snapshot.population.status,
            MechanismEvidenceStatus::MatchingClosed
        );
        assert_eq!(snapshot.distinct_signatures(), MechanismCount::Exact(1));
        assert_eq!(
            snapshot.signature_support(&id),
            Some(MechanismCount::Exact(2))
        );
        assert_eq!(reducer.processed.interval_count(), 1);
    }

    #[test]
    fn one_signature_can_span_two_bins_and_the_outside_population() {
        let request = checked_request(vec![3], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let low = MechanismNumericBin::new(0, 50).unwrap();
        let high = MechanismNumericBin::new(50, 100).unwrap();
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![
                observed(
                    0,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::binned("loss", low)],
                ),
                observed(
                    1,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::binned("loss", high)],
                ),
                observed(
                    2,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::outside_declared_bins("loss")],
                ),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).expect("reducer");
        install_target(&mut reducer);
        apply(&mut reducer, batch);
        let snapshot = reducer.snapshot().expect("snapshot");

        assert_eq!(
            snapshot.mechanisms_in_bin("loss", low),
            Some(MechanismCount::Exact(1))
        );
        assert_eq!(
            snapshot.mechanisms_in_bin("loss", high),
            Some(MechanismCount::Exact(1))
        );
        let MechanismBinFieldEvidence::Observed {
            outside_declared_bins_supports,
            ..
        } = &snapshot.bin_fields["loss"]
        else {
            panic!("loss bins should be observed")
        };
        assert_eq!(outside_declared_bins_supports.get(&id), Some(&1));
    }

    #[test]
    fn partial_bin_replay_remains_an_explicit_lower_bound() {
        let request = checked_request(vec![2], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let low = MechanismNumericBin::new(0, 50).unwrap();
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![
                observed(
                    0,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::binned("loss", low)],
                ),
                observed(
                    1,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::unavailable(
                        "loss",
                        MechanismBinAssignmentOutcomeV1::ReplayUnavailable,
                    )
                    .unwrap()],
                ),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);
        let snapshot = reducer.snapshot().unwrap();

        assert_eq!(
            snapshot.mechanisms_in_bin("loss", low),
            Some(MechanismCount::LowerBound(1))
        );
        let MechanismBinFieldEvidence::Observed {
            unavailable_supports,
            ..
        } = &snapshot.bin_fields["loss"]
        else {
            panic!("partially replayed field should retain observed evidence")
        };
        assert_eq!(
            unavailable_supports.get(&MechanismBinUnavailableSupport {
                signature: id,
                reason: MechanismBinUnavailableReason::ValueReplayUnavailable,
            }),
            Some(&1)
        );
    }

    #[test]
    fn all_unavailable_bin_values_retain_signature_reasons_and_supports() {
        let request = checked_request(vec![2], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![
                observed(
                    0,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::unavailable(
                        "loss",
                        MechanismBinAssignmentOutcomeV1::ReplayUnavailable,
                    )
                    .unwrap()],
                ),
                observed(
                    1,
                    id.clone(),
                    vec![MechanismBinAssignmentV1::unavailable(
                        "loss",
                        MechanismBinAssignmentOutcomeV1::ObservationUnsupported,
                    )
                    .unwrap()],
                ),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);
        let snapshot = reducer.snapshot().unwrap();

        let MechanismBinFieldEvidence::Observed {
            observed_supports,
            outside_declared_bins_supports,
            unavailable_supports,
        } = &snapshot.bin_fields["loss"]
        else {
            panic!("case-scoped unavailable values must remain observed evidence")
        };
        assert!(observed_supports.is_empty());
        assert!(outside_declared_bins_supports.is_empty());
        assert_eq!(
            unavailable_supports.get(&MechanismBinUnavailableSupport {
                signature: id.clone(),
                reason: MechanismBinUnavailableReason::ValueReplayUnavailable,
            }),
            Some(&1)
        );
        assert_eq!(
            unavailable_supports.get(&MechanismBinUnavailableSupport {
                signature: id,
                reason: MechanismBinUnavailableReason::ValueUnsupported,
            }),
            Some(&1)
        );
    }

    #[test]
    fn duplicate_conflict_rejects_the_whole_prepared_batch() {
        let request = checked_request(vec![2], false);
        let (then_id, then_definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let (else_id, else_definition) = signature(&request.observation, IfDecisionOutcome::Else);
        let first = validated_batch(
            &request,
            vec![then_definition.clone()],
            vec![observed(0, then_id.clone(), vec![])],
        );
        let conflicting = validated_batch(
            &request,
            vec![then_definition, else_definition],
            vec![observed(0, else_id, vec![]), observed(1, then_id, vec![])],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).expect("reducer");
        install_target(&mut reducer);
        apply(&mut reducer, first);
        let before = reducer.snapshot().expect("before");
        assert!(reducer.prepare_observation_batch(conflicting).is_err());
        assert_eq!(reducer.snapshot().expect("after"), before);
        assert_eq!(reducer.processed.case_count(), 1);
    }

    #[test]
    fn cumulative_reducer_capacity_rejects_atomically() {
        let request = checked_request(vec![1], false);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(&request, vec![definition], vec![observed(0, id, vec![])]);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        let known = ExactCaseSupport::full(&reducer.case_universe);
        sync_known_support(&mut reducer, known);
        let before = reducer.snapshot().unwrap();
        let before_usage = reducer.resource_usage;
        let before_revision = reducer.revision;
        let limits = MechanismReducerResourceLimitsV1 {
            unique_signatures: 0,
            ..MECHANISM_REDUCER_RESOURCE_LIMITS_V1
        };

        let error = reducer
            .prepare_observation_batch_with_limits(batch, limits)
            .err()
            .expect("cumulative signature cap");
        assert!(error.is_reducer_capacity());
        assert_eq!(
            error.reducer_capacity_details(),
            Some(("unique signatures", 1, 0))
        );
        assert_eq!(reducer.snapshot().unwrap(), before);
        assert_eq!(reducer.resource_usage, before_usage);
        assert_eq!(reducer.revision, before_revision);
        assert!(reducer.processed.is_empty());
        assert!(reducer.signatures.is_empty());
    }

    #[test]
    fn cumulative_activation_step_capacity_rejects_atomically() {
        let request = checked_request(vec![1], false);
        let step = activation_step(&request.observation);
        let (id, definition) = signature_with_activation_path(
            &request.observation,
            IfDecisionOutcome::Then,
            vec![step],
        );
        let batch = validated_batch(&request, vec![definition], vec![observed(0, id, vec![])]);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        let known = ExactCaseSupport::full(&reducer.case_universe);
        sync_known_support(&mut reducer, known);
        let before = reducer.snapshot().unwrap();
        let before_usage = reducer.resource_usage;
        let before_revision = reducer.revision;
        let limits = MechanismReducerResourceLimitsV1 {
            signature_activation_steps: 0,
            ..MECHANISM_REDUCER_RESOURCE_LIMITS_V1
        };

        let error = reducer
            .prepare_observation_batch_with_limits(batch, limits)
            .err()
            .expect("cumulative activation-step cap");
        assert_eq!(
            error.reducer_capacity_details(),
            Some(("retained signature activation steps", 1, 0))
        );
        assert_eq!(reducer.snapshot().unwrap(), before);
        assert_eq!(reducer.resource_usage, before_usage);
        assert_eq!(reducer.revision, before_revision);
        assert!(reducer.processed.is_empty());
        assert!(reducer.signatures.is_empty());
    }

    #[test]
    fn arrival_order_produces_the_same_partitioned_snapshot() {
        let request = checked_request(vec![2], false);
        let (then_id, then_definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let (else_id, else_definition) = signature(&request.observation, IfDecisionOutcome::Else);
        let then_batch = || {
            validated_batch(
                &request,
                vec![then_definition.clone()],
                vec![observed(0, then_id.clone(), vec![])],
            )
        };
        let else_batch = || {
            validated_batch(
                &request,
                vec![else_definition.clone()],
                vec![observed(1, else_id.clone(), vec![])],
            )
        };
        let mut left = MechanismEvidenceReducerV1::new(request.clone()).unwrap();
        install_target(&mut left);
        apply(&mut left, then_batch());
        apply(&mut left, else_batch());
        let mut right = MechanismEvidenceReducerV1::new(request.clone()).unwrap();
        install_target(&mut right);
        apply(&mut right, else_batch());
        apply(&mut right, then_batch());

        assert_eq!(left.snapshot().unwrap(), right.snapshot().unwrap());
        assert_eq!(left.processed, right.processed);
        assert_eq!(left.processed.interval_count(), 1);
    }

    #[test]
    fn permanently_unsupported_case_keeps_incidence_open() {
        let request = checked_request(vec![2], false);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![
                observed(0, id, vec![]),
                MechanismCaseObservationProposalV1::permanently_untraced(
                    case(1),
                    MechanismPermanentUntracedReasonV1::ObservationUnsupported,
                    receipt(1),
                ),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);
        let snapshot = reducer.snapshot().unwrap();

        assert_eq!(
            snapshot.population.status,
            MechanismEvidenceStatus::IncidenceOpen
        );
        assert_eq!(snapshot.population.known_target_untraced, 1);
        assert_eq!(
            snapshot
                .population
                .incidence
                .as_ref()
                .unwrap()
                .terminal_for_path(&[1])
                .unwrap(),
            Some(&MechanismIncidenceTerminal::KnownTargetUntraced(
                KnownTargetUntracedReason::ObservationUnsupported,
            ))
        );
    }

    #[test]
    fn empty_exact_target_closes_without_observation_rows() {
        let request = checked_request(vec![0], false);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        let snapshot = reducer.snapshot().unwrap();

        assert_eq!(
            snapshot.population.status,
            MechanismEvidenceStatus::MatchingClosed
        );
        assert_eq!(
            snapshot.population.requested_target,
            MechanismCount::Exact(0)
        );
        assert!(snapshot.signatures.is_empty());
        assert!(reducer.processed.is_empty());
    }

    #[test]
    fn early_evidence_moves_scope_open_to_incidence_open_to_matching_closed() {
        let request = checked_request(vec![2], false);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let first = validated_batch(
            &request,
            vec![definition.clone()],
            vec![observed(0, id.clone(), vec![])],
        );
        let second = validated_batch(&request, vec![definition], vec![observed(1, id, vec![])]);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        assert_eq!(
            reducer.snapshot().unwrap().population.status,
            MechanismEvidenceStatus::ScopeOpen
        );
        let first_known = known_support(&reducer, [(0, 1)]);
        sync_known_support(&mut reducer, first_known);
        apply(&mut reducer, first);
        let early = reducer.snapshot().unwrap();
        assert_eq!(early.population.status, MechanismEvidenceStatus::ScopeOpen);
        assert_eq!(
            early.population.requested_target,
            MechanismCount::LowerBound(1)
        );
        assert_eq!(early.population.traced, 1);
        assert_eq!(early.population.known_target_untraced, 0);
        assert!(early.population.incidence.is_none());

        let complete_known = ExactCaseSupport::full(&reducer.case_universe);
        let authoritative =
            ExactClosedMatchSupportV1::from_support_for_test(complete_known.clone());
        sync_known_support(&mut reducer, complete_known);
        assert_eq!(
            reducer.snapshot().unwrap().population.requested_target,
            MechanismCount::LowerBound(2)
        );
        let exact = reducer
            .prepare_exact_target_from_known_support(&authoritative)
            .expect("exact target from complete known support");
        reducer.apply_prepared_exact_target(exact);
        assert_eq!(
            reducer.snapshot().unwrap().population.status,
            MechanismEvidenceStatus::IncidenceOpen
        );
        let open = reducer.snapshot().unwrap();
        assert_eq!(
            open.population.status,
            MechanismEvidenceStatus::IncidenceOpen
        );
        assert_eq!(
            open.population
                .incidence
                .as_ref()
                .unwrap()
                .terminal_for_path(&[1])
                .unwrap(),
            Some(&MechanismIncidenceTerminal::KnownTargetUntraced(
                KnownTargetUntracedReason::Pending,
            ))
        );
        apply(&mut reducer, second);
        assert_eq!(
            reducer.snapshot().unwrap().population.status,
            MechanismEvidenceStatus::MatchingClosed
        );
    }

    #[test]
    fn scope_open_rejects_observations_outside_confirmed_known_support() {
        let request = checked_request(vec![2], false);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let unknown = validated_batch(&request, vec![definition], vec![observed(1, id, vec![])]);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        let first_known = known_support(&reducer, [(0, 1)]);
        sync_known_support(&mut reducer, first_known);
        let before = reducer.snapshot().unwrap();

        let error = reducer
            .prepare_observation_batch(unknown)
            .err()
            .expect("unknown rank must be rejected");
        assert!(error.to_string().contains("known matching support"));
        assert_eq!(reducer.snapshot().unwrap(), before);
        assert!(reducer.processed.is_empty());
    }

    #[test]
    fn coordinator_known_target_support_sync_is_monotone() {
        let request = checked_request(vec![4], false);
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        let first = known_support(&reducer, [(0, 1)]);
        sync_known_support(&mut reducer, first.clone());
        let grown = known_support(&reducer, [(0, 1), (2, 4)]);
        sync_known_support(&mut reducer, grown.clone());
        assert_eq!(reducer.known_target_support, grown);
        assert_eq!(
            reducer.snapshot().unwrap().population.requested_target,
            MechanismCount::LowerBound(3)
        );

        let before_revision = reducer.revision;
        assert!(reducer.prepare_known_target_support(first).is_err());
        assert_eq!(reducer.known_target_support.case_count(), 3);
        assert_eq!(reducer.revision, before_revision);
    }

    #[test]
    fn canonical_codec_roundtrips_complete_definitions_and_rejects_trailing_bytes() {
        let request = checked_request(vec![2], false);
        let (then_id, then_definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let (else_id, else_definition) = signature(&request.observation, IfDecisionOutcome::Else);
        let proposal = MechanismObservationBatchProposalV1::new(
            &request,
            vec![else_definition, then_definition],
            vec![observed(1, else_id, vec![]), observed(0, then_id, vec![])],
        )
        .unwrap();
        assert_eq!(proposal.definitions().len(), 2);
        assert!(proposal
            .definitions()
            .all(|(id, definition)| id.digest_bytes() == {
                let mut interner = CanonicalSignatureInterner::new(&request.observation);
                interner.intern(definition.clone()).unwrap().digest_bytes()
            }));
        let bytes = encode_mechanism_observation_batch_v1(&request, &proposal).unwrap();
        assert_eq!(
            decode_mechanism_observation_batch_v1(&request, &bytes).unwrap(),
            proposal
        );
        let mut noncanonical = bytes;
        noncanonical.push(0);
        assert!(decode_mechanism_observation_batch_v1(&request, &noncanonical).is_err());
    }

    #[test]
    fn malformed_rank_and_undeclared_bin_fail_before_sealing() {
        let request = checked_request(vec![2], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let wrong_rank = MechanismCaseObservationProposalV1::observed(
            MechanismCanonicalCaseIdV1::new(0, vec![1]),
            id.clone(),
            vec![MechanismBinAssignmentV1::binned(
                "loss",
                MechanismNumericBin::new(0, 50).unwrap(),
            )],
            receipt(0),
        );
        assert!(MechanismObservationBatchProposalV1::new(
            &request,
            vec![definition.clone()],
            vec![wrong_rank],
        )
        .is_err());
        assert!(MechanismObservationBatchProposalV1::new(
            &request,
            vec![definition],
            vec![observed(
                0,
                id,
                vec![MechanismBinAssignmentV1::binned(
                    "loss",
                    MechanismNumericBin::new(100, 150).unwrap(),
                )],
            )],
        )
        .is_err());
    }

    #[test]
    fn summary_only_request_cannot_authorize_case_ranked_storage() {
        let request = checked_request_with_disclosure(
            vec![1],
            false,
            MechanismIncidenceDisclosure::SummaryOnly,
        );
        assert!(MechanismEvidenceReducerV1::new(request).is_err());
    }

    #[test]
    fn stream_request_limits_bound_axes_fields_and_field_name_bytes() {
        let too_many_axes = checked_request(vec![1; MAX_AXES + 1], false);
        let axis_error = MechanismEvidenceReducerV1::new(too_many_axes.clone())
            .err()
            .expect("axis limit");
        assert!(axis_error.to_string().contains("axes"));
        assert!(MechanismObservationBatchProposalV1::new(
            &too_many_axes,
            Vec::<DynamicMechanismSignature>::new(),
            Vec::<MechanismCaseObservationProposalV1>::new(),
        )
        .unwrap_err()
        .to_string()
        .contains("axes"));

        let program = analysis_program();
        let root = MechanismSemanticRootId::from_site(site(&program, "limit-field", 20))
            .expect("field root");
        let bin = MechanismNumericBin::new(0, 1).expect("bin");
        let too_many_fields = (0..=MAX_BIN_FIELDS)
            .map(|index| {
                MechanismBinField::new(format!("field-{index}"), root.clone(), vec![bin])
                    .expect("bin field")
            })
            .collect::<Vec<_>>();
        let too_many_fields = checked_request_with_bin_fields(
            vec![1],
            too_many_fields,
            MechanismIncidenceDisclosure::FullMatchingIncidence,
        );
        assert!(MechanismEvidenceReducerV1::new(too_many_fields)
            .err()
            .expect("bin-field limit")
            .to_string()
            .contains("bin fields"));

        let long_name = "x".repeat(MAX_TEXT_BYTES + 1);
        let long_name_field =
            MechanismBinField::new(long_name, root, vec![bin]).expect("long-name field");
        let long_name_request = checked_request_with_bin_fields(
            vec![1],
            vec![long_name_field],
            MechanismIncidenceDisclosure::FullMatchingIncidence,
        );
        let name_error = MechanismEvidenceReducerV1::new(long_name_request.clone())
            .err()
            .expect("field-name limit");
        assert!(name_error.to_string().contains("UTF-8 bytes"));
        assert!(
            decode_mechanism_observation_batch_v1(&long_name_request, &[])
                .unwrap_err()
                .to_string()
                .contains("UTF-8 bytes")
        );
    }

    #[test]
    fn stream_request_collection_limits_are_inclusive_and_checked() {
        assert_eq!(validate_bin_count_limits([2, 2], 2, 4).unwrap(), 4);
        assert!(validate_bin_count_limits([3], 2, 4).is_err());
        assert!(validate_bin_count_limits([2, 2, 1], 2, 4).is_err());

        let observation = checked_request(vec![1], false).observation;
        let at_retention_limit = CheckedMechanismObservationRequestV1::new(
            observation.clone(),
            MechanismDisclosureV1::new(
                MechanismIncidenceDisclosure::FullMatchingIncidence,
                MAX_RETAINED_EXAMPLES_PER_SIGNATURE as u32,
            ),
        )
        .unwrap();
        validate_mechanism_stream_request_v1(&at_retention_limit).unwrap();
        let over_retention_limit = CheckedMechanismObservationRequestV1::new(
            observation,
            MechanismDisclosureV1::new(
                MechanismIncidenceDisclosure::FullMatchingIncidence,
                MAX_RETAINED_EXAMPLES_PER_SIGNATURE as u32 + 1,
            ),
        )
        .unwrap();
        assert!(validate_mechanism_stream_request_v1(&over_retention_limit).is_err());

        let mut sampled = checked_request(vec![3], false).observation;
        let shared = ExploreCaseId::new(vec![1]);
        sampled.sampling.result_representatives =
            BTreeSet::from([ExploreCaseId::new(vec![0]), shared.clone()]);
        sampled.sampling.extrema_witnesses = BTreeSet::from([shared]);
        sampled.sampling.required_case_ids = BTreeSet::from([ExploreCaseId::new(vec![2])]);
        assert_eq!(validate_selected_sampling_count(&sampled, 3).unwrap(), 3);
        assert!(validate_selected_sampling_count(&sampled, 2).is_err());
    }

    #[test]
    fn incidence_materialization_reports_typed_capacity_backpressure() {
        let universe = ExploreCaseUniverse::new(vec![8]).expect("universe");
        let first = ExactCaseSupport::new(&universe, [(0, 1), (2, 3)]).expect("support");
        let second = ExactCaseSupport::new(&universe, [(4, 5)]).expect("support");
        assert_eq!(
            validate_incidence_override_capacity([&first, &second], 3).unwrap(),
            3
        );
        let backpressure = validate_incidence_override_capacity([&first, &second], 2).unwrap_err();
        assert!(backpressure.is_snapshot_capacity());
        assert_eq!(
            backpressure.snapshot_capacity_details(),
            Some(("incidence override intervals", 3, 2))
        );
        let target_backpressure =
            validate_exact_target_lowering_capacity(&first, 3, 5).unwrap_err();
        assert!(target_backpressure.is_snapshot_capacity());
        assert_eq!(
            target_backpressure.snapshot_capacity_details(),
            Some(("target rank-interval dimension steps", 6, 5))
        );
    }

    #[test]
    fn count_only_checkpoint_closes_without_materializing_target_dag() {
        let reducer = MechanismEvidenceReducerV1::new(checked_request(vec![1], false)).unwrap();
        let authoritative =
            ExactClosedMatchSupportV1::from_support_for_test(reducer.known_target_support.clone());
        assert!(reducer.exact_target.is_none());

        let summary = reducer
            .checkpoint_summary_with_authoritative_target(Some(&authoritative))
            .expect("authoritative exact support is sufficient for count closure");
        assert_eq!(summary.status, MechanismEvidenceStatus::MatchingClosed);
        assert_eq!(summary.target_cases, MechanismCount::Exact(0));
        assert_eq!(
            summary.mechanism_signatures,
            MechanismCheckpointCountV1::Exact(0)
        );
        assert!(reducer.exact_target.is_none());
    }

    #[test]
    fn checkpoint_summary_conserves_closed_signature_and_bin_counts() {
        let request = checked_request(vec![2], true);
        let (then_id, then_definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let (else_id, else_definition) = signature(&request.observation, IfDecisionOutcome::Else);
        let batch = validated_batch(
            &request,
            vec![then_definition, else_definition],
            vec![
                observed(
                    0,
                    then_id,
                    vec![MechanismBinAssignmentV1::binned(
                        "loss",
                        MechanismNumericBin::new(0, 50).unwrap(),
                    )],
                ),
                observed(
                    1,
                    else_id,
                    vec![MechanismBinAssignmentV1::outside_declared_bins("loss")],
                ),
            ],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);

        let summary = reducer.checkpoint_summary().unwrap();
        assert_eq!(summary.status, MechanismEvidenceStatus::MatchingClosed);
        assert_eq!(summary.target_cases, MechanismCount::Exact(2));
        assert_eq!(summary.traced_cases, 2);
        assert_eq!(summary.known_target_untraced.total, 0);
        assert_eq!(
            summary.mechanism_signatures,
            MechanismCheckpointCountV1::Exact(2)
        );
        assert_eq!(summary.bin_fields.len(), 1);
        let field = &summary.bin_fields[0];
        assert_eq!(field.binned_cases, 1);
        assert_eq!(field.outside_declared_bins_cases, 1);
        assert_eq!(field.unavailable_cases, 0);
        assert_eq!(field.bins[0].confirmed_case_support, 1);
        assert_eq!(
            field.bins[0].mechanism_count,
            MechanismCheckpointCountV1::Exact(1)
        );
        assert_eq!(field.bins[1].confirmed_case_support, 0);
        assert_eq!(
            field.bins[1].mechanism_count,
            MechanismCheckpointCountV1::Exact(0)
        );
    }

    #[test]
    fn checkpoint_bin_count_is_unknown_when_a_closed_value_is_unavailable() {
        let request = checked_request(vec![1], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let assignment = MechanismBinAssignmentV1::unavailable(
            "loss",
            MechanismBinAssignmentOutcomeV1::ReplayUnavailable,
        )
        .unwrap();
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![observed(0, id, vec![assignment])],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);

        let summary = reducer.checkpoint_summary().unwrap();
        assert_eq!(summary.status, MechanismEvidenceStatus::MatchingClosed);
        assert_eq!(
            summary.mechanism_signatures,
            MechanismCheckpointCountV1::Exact(1)
        );
        let field = &summary.bin_fields[0];
        assert_eq!(field.unavailable_cases, 1);
        assert_eq!(field.replay_unavailable_cases, 1);
        assert!(field.bins.iter().all(|bin| {
            bin.mechanism_count
                == MechanismCheckpointCountV1::Unknown {
                    confirmed_lower_bound: 0,
                }
        }));
    }

    #[test]
    fn scope_open_checkpoint_never_promotes_confirmed_counts_to_exact() {
        let request = checked_request(vec![2], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![observed(
                0,
                id,
                vec![MechanismBinAssignmentV1::binned(
                    "loss",
                    MechanismNumericBin::new(0, 50).unwrap(),
                )],
            )],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        let known = known_support(&reducer, [(0, 1)]);
        sync_known_support(&mut reducer, known);
        apply(&mut reducer, batch);

        let summary = reducer.checkpoint_summary().unwrap();
        assert_eq!(summary.status, MechanismEvidenceStatus::ScopeOpen);
        assert_eq!(summary.target_cases, MechanismCount::LowerBound(1));
        assert_eq!(
            summary.mechanism_signatures,
            MechanismCheckpointCountV1::LowerBound(1)
        );
        assert_eq!(
            summary.bin_fields[0].bins[0].mechanism_count,
            MechanismCheckpointCountV1::LowerBound(1)
        );
        assert_eq!(
            summary.bin_fields[0].bins[1].mechanism_count,
            MechanismCheckpointCountV1::Unknown {
                confirmed_lower_bound: 0,
            }
        );
    }

    #[test]
    fn checkpoint_summary_rejects_nonconserving_internal_bin_support() {
        let request = checked_request(vec![1], true);
        let (id, definition) = signature(&request.observation, IfDecisionOutcome::Then);
        let batch = validated_batch(
            &request,
            vec![definition],
            vec![observed(
                0,
                id,
                vec![MechanismBinAssignmentV1::binned(
                    "loss",
                    MechanismNumericBin::new(0, 50).unwrap(),
                )],
            )],
        );
        let mut reducer = MechanismEvidenceReducerV1::new(request).unwrap();
        install_target(&mut reducer);
        apply(&mut reducer, batch);
        reducer.field_signature_binned_supports.clear();

        let error = reducer
            .checkpoint_summary()
            .expect_err("publication must fail closed on nonconserving support");
        assert!(error.to_string().contains("do not conserve"));
    }
}
