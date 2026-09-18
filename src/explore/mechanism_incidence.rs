//! Relational mechanism-signature incidence for Explore.
//!
//! This module is deliberately independent of the legacy Cartesian mechanism
//! stream. A request target grows as a set of content-stable relational
//! [`RelationalCaseId`] values, then seals. Every target case receives exactly
//! one terminal: either a transition/signature incidence or a permanent,
//! explicitly identified unavailability reason. Execution order, scheduling,
//! presentation, and retention policy do not enter this evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

use super::choice_relation::ChoiceContentRoot;
use super::relation::{
    ChoiceId, ClosedQuestionCatalogRef, MechanismRequestId, MechanismTargetId, QuestionCatalog,
    QuestionContentRoot, QuestionId, RelationalCaseId,
};
use super::relational_population::CertifiedSelectedPopulationRoot;
use super::result_view::ResultViewRoot;
use super::transition::TransitionId;

const MECHANISM_SIGNATURE_ID_HASH_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-signature-id.v1";
const MECHANISM_UNAVAILABLE_REASON_ID_HASH_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-unavailable-reason-id.v1";
const MECHANISM_TARGET_CASE_SET_ROOT_HASH_V1: &[u8] =
    b"futuruna.explore.mechanism-target-case-set-root.v1";
const MECHANISM_TARGET_SEAL_ID_HASH_V3: &[u8] = b"futuruna.explore.mechanism-target-seal-id.v3";
const MECHANISM_INCIDENCE_ROOT_HASH_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-incidence-root.v3";
const TERMINAL_DISCOVERY_REVISION_HASH_V1: &[u8] =
    b"futuruna.explore.mechanism-terminal-discovery-revision.v1";
const TARGET_DISCOVERY_REVISION_HASH_V1: &[u8] =
    b"futuruna.explore.mechanism-target-discovery-revision.v1";

pub(crate) const MECHANISM_TARGET_SEAL_VERSION: u32 = 3;

const SIGNATURE_REQUEST_ROLE: u8 = 0x01;
const SIGNATURE_DIFFERENTIAL_DIGEST_ROLE: u8 = 0x02;

/// Request-scoped content identity of one complete normalized differential
/// mechanism signature.
///
/// The supplied digest is produced by the checked signature normalizer. The
/// request scope prevents equal dynamic control graphs observed under
/// different endpoint observers or targets from being conflated.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSignatureId {
    request_id: MechanismRequestId,
    digest: [u8; 32],
}

impl MechanismSignatureId {
    pub(super) const fn from_journal_codec_parts(
        request_id: MechanismRequestId,
        digest: [u8; 32],
    ) -> Self {
        Self { request_id, digest }
    }

    pub(crate) fn from_canonical_differential_signature_digest(
        request_id: MechanismRequestId,
        differential_signature_digest: [u8; 32],
    ) -> Self {
        let mut hasher = CanonicalHasher::new(MECHANISM_SIGNATURE_ID_HASH_V1);
        hasher.tag(SIGNATURE_REQUEST_ROLE);
        hasher.digest(request_id.bytes());
        hasher.tag(SIGNATURE_DIFFERENTIAL_DIGEST_ROLE);
        hasher.digest(differential_signature_digest);
        Self {
            request_id,
            digest: hasher.finish(),
        }
    }

    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.digest
    }
}

/// Collision-checkable definition of one complete normalized differential
/// signature.
///
/// Keeping canonical bytes beside their digest lets the interner reject two
/// unequal definitions that claim the same content ID instead of silently
/// accepting a cryptographic collision or a malformed decoder claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSignatureDefinition {
    id: MechanismSignatureId,
    canonical_differential_digest: [u8; 32],
    canonical_definition: Box<[u8]>,
}

impl MechanismSignatureDefinition {
    pub(crate) fn from_canonical_definition(
        request_id: MechanismRequestId,
        canonical_definition: impl Into<Box<[u8]>>,
    ) -> Self {
        let canonical_definition = canonical_definition.into();
        let canonical_differential_digest = Sha256::digest(&canonical_definition).into();
        let id = MechanismSignatureId::from_canonical_differential_signature_digest(
            request_id,
            canonical_differential_digest,
        );
        Self {
            id,
            canonical_differential_digest,
            canonical_definition,
        }
    }

    pub(crate) const fn id(&self) -> MechanismSignatureId {
        self.id
    }

    pub(crate) const fn canonical_differential_digest(&self) -> [u8; 32] {
        self.canonical_differential_digest
    }

    pub(crate) fn canonical_definition(&self) -> &[u8] {
        &self.canonical_definition
    }

    fn validate_for_request(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<(), MechanismIncidenceError> {
        if self.id.request_id != request_id {
            return Err(MechanismIncidenceError::SignatureRequestMismatch);
        }
        let derived_differential_digest: [u8; 32] =
            Sha256::digest(&self.canonical_definition).into();
        if derived_differential_digest != self.canonical_differential_digest {
            return Err(MechanismIncidenceError::SignatureDefinitionDigestMismatch {
                signature_id: self.id,
            });
        }
        let derived_id = MechanismSignatureId::from_canonical_differential_signature_digest(
            request_id,
            self.canonical_differential_digest,
        );
        if derived_id != self.id {
            return Err(MechanismIncidenceError::SignatureIdMismatch {
                claimed: self.id,
                derived: derived_id,
            });
        }
        Ok(())
    }
}

/// Content identity of a permanent reason why endpoint replay cannot assign a
/// complete signature. The human-readable explanation and source provenance
/// may be retained elsewhere under the same canonical reason contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismUnavailableReasonId([u8; 32]);

impl MechanismUnavailableReasonId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_reason_preimage(preimage: &[u8]) -> Self {
        Self::from_canonical_reason_digest(Sha256::digest(preimage).into())
    }

    pub(crate) fn from_canonical_reason_digest(reason_digest: [u8; 32]) -> Self {
        let mut hasher = CanonicalHasher::new(MECHANISM_UNAVAILABLE_REASON_ID_HASH_V1);
        hasher.digest(reason_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Collision-checkable canonical payload for one permanent unavailability
/// reason. This value is descriptive content, not authority to close a case;
/// the analysis journal accepts it only as part of producer-minted replay
/// evidence and the incidence catalog retains it for checked restoration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismUnavailableReasonDefinition {
    id: MechanismUnavailableReasonId,
    canonical_reason: Box<[u8]>,
}

impl MechanismUnavailableReasonDefinition {
    pub(crate) fn from_canonical_reason(canonical_reason: impl Into<Box<[u8]>>) -> Self {
        let canonical_reason = canonical_reason.into();
        let id = MechanismUnavailableReasonId::from_canonical_reason_preimage(&canonical_reason);
        Self {
            id,
            canonical_reason,
        }
    }

    pub(crate) const fn id(&self) -> MechanismUnavailableReasonId {
        self.id
    }

    pub(crate) fn canonical_reason(&self) -> &[u8] {
        &self.canonical_reason
    }

    fn validate_identity(&self) -> Result<(), MechanismIncidenceError> {
        let derived =
            MechanismUnavailableReasonId::from_canonical_reason_preimage(&self.canonical_reason);
        if derived != self.id {
            return Err(MechanismIncidenceError::UnavailableReasonIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Checked semantic scope of one mechanism request.
///
/// [`MechanismRequestId`] is intentionally opaque, so the resolved question
/// and target are retained beside it at this evidence boundary. The checked
/// analysis planner is responsible for constructing this scope from the same
/// inputs that minted the request ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismRequestScope {
    request_id: MechanismRequestId,
    question_id: QuestionId,
    target: MechanismTargetId,
}

impl MechanismRequestScope {
    pub(crate) const fn new(
        request_id: MechanismRequestId,
        question_id: QuestionId,
        target: MechanismTargetId,
    ) -> Self {
        Self {
            request_id,
            question_id,
            target,
        }
    }

    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn target(self) -> MechanismTargetId {
        self.target
    }
}

/// Arrival-order-independent identity of the exact target case set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismTargetCaseSetRoot([u8; 32]);

impl MechanismTargetCaseSetRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical count and identity-set commitment for an exact mechanism target.
///
/// The constructor canonicalizes and deduplicates the supplied case IDs. This
/// compact value can therefore cross a durable journal boundary without
/// copying the entire selected-question catalog. Installing it as a target
/// seal still compares it with the independently accumulated local target
/// cases; neither the serialized count nor root is trusted on its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismTargetCaseSetCommitment {
    root: MechanismTargetCaseSetRoot,
    count: u128,
}

impl MechanismTargetCaseSetCommitment {
    pub(super) const fn restore_from_journal_codec(root: [u8; 32], count: u128) -> Self {
        Self {
            root: MechanismTargetCaseSetRoot(root),
            count,
        }
    }

    pub(crate) fn from_cases(cases: impl IntoIterator<Item = RelationalCaseId>) -> Self {
        let cases = cases.into_iter().collect::<BTreeSet<_>>();
        Self::from_canonical_cases(&cases)
    }

    /// Commit a validated borrowed FIND closure without recollecting its
    /// already unique, canonically ordered selected CaseIds.
    pub(crate) fn from_borrowed_selected(question: &ClosedQuestionCatalogRef<'_>) -> Self {
        let count = question.selected_count();
        let root = mechanism_target_case_set_root(question.selected_case_ids(), count);
        Self { root, count }
    }

    fn from_canonical_cases(cases: &BTreeSet<RelationalCaseId>) -> Self {
        let count = cases.len() as u128;
        let root = mechanism_target_case_set_root(cases.iter().copied(), count);
        Self { root, count }
    }

    pub(crate) const fn root(self) -> MechanismTargetCaseSetRoot {
        self.root
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

/// Content identity of one typed target-closure receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismTargetSealId([u8; 32]);

impl MechanismTargetSealId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed upstream evidence from which the exact mechanism target was
/// derived. A frontier root or operational cursor cannot inhabit this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismTargetSealUpstream {
    SelectedQuestion {
        content_root: QuestionContentRoot,
    },
    /// Exact selected population certified by the support/proof DAG without
    /// claiming an extensional question-content root.
    CertifiedSelectedSupport {
        population_root: CertifiedSelectedPopulationRoot,
        exact_cardinality: u128,
    },
    Choice {
        choice_id: ChoiceId,
        content_root: ChoiceContentRoot,
    },
}

/// Semantic proof that one request target equals an exact closed upstream set.
///
/// Construction is private to the two checked sealing paths below. The seal
/// commits both the upstream content root and the independently canonicalized
/// set of target CaseIds, so replay can reject a root attached to missing or
/// extra streamed target members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismTargetSeal {
    version: u32,
    id: MechanismTargetSealId,
    scope: MechanismRequestScope,
    upstream: MechanismTargetSealUpstream,
    target_case_set_root: MechanismTargetCaseSetRoot,
    target_case_count: u128,
}

impl MechanismTargetSeal {
    fn issue(
        scope: MechanismRequestScope,
        upstream: MechanismTargetSealUpstream,
        target: MechanismTargetCaseSetCommitment,
    ) -> Self {
        let version = MECHANISM_TARGET_SEAL_VERSION;
        let target_case_set_root = target.root;
        let target_case_count = target.count;
        let id = derive_mechanism_target_seal_id(
            version,
            scope,
            upstream,
            target_case_set_root,
            target_case_count,
        );
        Self {
            version,
            id,
            scope,
            upstream,
            target_case_set_root,
            target_case_count,
        }
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn id(&self) -> MechanismTargetSealId {
        self.id
    }

    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn upstream(&self) -> MechanismTargetSealUpstream {
        self.upstream
    }

    pub(crate) const fn target_case_set_root(&self) -> MechanismTargetCaseSetRoot {
        self.target_case_set_root
    }

    pub(crate) const fn target_case_count(&self) -> u128 {
        self.target_case_count
    }

    pub(crate) fn validate_identity(&self) -> Result<(), MechanismIncidenceError> {
        if self.version != MECHANISM_TARGET_SEAL_VERSION {
            return Err(MechanismIncidenceError::UnsupportedTargetSealVersion {
                actual: self.version,
                expected: MECHANISM_TARGET_SEAL_VERSION,
            });
        }
        validate_target_upstream_scope(self.scope, self.upstream)?;
        if let MechanismTargetSealUpstream::CertifiedSelectedSupport {
            exact_cardinality, ..
        } = self.upstream
        {
            if exact_cardinality != self.target_case_count {
                return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
            }
        }
        let derived = derive_mechanism_target_seal_id(
            self.version,
            self.scope,
            self.upstream,
            self.target_case_set_root,
            self.target_case_count,
        );
        if derived != self.id {
            return Err(MechanismIncidenceError::TargetSealIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// Terminal mechanism evidence for one request target case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismCaseTerminal {
    Incidence {
        transition_id: TransitionId,
        signature_id: MechanismSignatureId,
    },
    Unavailable {
        reason_id: MechanismUnavailableReasonId,
    },
}

/// One canonical case-keyed row in a mechanism-incidence snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismCaseTerminalRecord {
    case_id: RelationalCaseId,
    terminal: MechanismCaseTerminal,
}

/// Branch-sensitive prefix commitment for the operational terminal discovery
/// lane. It lets a downstream stream cursor prove that a same-scope catalog is
/// an extension of the exact prefix it already consumed. This is not the
/// canonical incidence root and does not enter answer identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismTerminalDiscoveryRevision([u8; 32]);

impl MechanismTerminalDiscoveryRevision {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Branch-sensitive prefix commitment for target-case discovery. It is an
/// operational cursor guard and never contributes to the canonical target set
/// root or answer identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismTargetDiscoveryRevision([u8; 32]);

impl MechanismTargetDiscoveryRevision {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl MechanismCaseTerminalRecord {
    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn terminal(self) -> MechanismCaseTerminal {
        self.terminal
    }
}

/// Closure-aware request-relative count.
///
/// `Unknown` is used when no complete signature has been confirmed and the
/// remaining frontier could still contain one. It is intentionally distinct
/// from an exact or presented zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismCountEvidence {
    Unknown { confirmed_lower_bound: u128 },
    LowerBound(u128),
    Exact(u128),
}

impl MechanismCountEvidence {
    pub(crate) const fn confirmed_lower_bound(self) -> u128 {
        match self {
            Self::Unknown {
                confirmed_lower_bound,
            }
            | Self::LowerBound(confirmed_lower_bound)
            | Self::Exact(confirmed_lower_bound) => confirmed_lower_bound,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Request-relative population counts at one target/incidence frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismIncidenceCounts {
    target_cases: MechanismCountEvidence,
    terminal_cases: MechanismCountEvidence,
    incidence_cases: MechanismCountEvidence,
    unavailable_cases: MechanismCountEvidence,
    distinct_signatures: MechanismCountEvidence,
}

impl MechanismIncidenceCounts {
    pub(crate) const fn target_cases(self) -> MechanismCountEvidence {
        self.target_cases
    }

    pub(crate) const fn terminal_cases(self) -> MechanismCountEvidence {
        self.terminal_cases
    }

    pub(crate) const fn incidence_cases(self) -> MechanismCountEvidence {
        self.incidence_cases
    }

    pub(crate) const fn unavailable_cases(self) -> MechanismCountEvidence {
        self.unavailable_cases
    }

    pub(crate) const fn distinct_signatures(self) -> MechanismCountEvidence {
        self.distinct_signatures
    }
}

/// Authenticated, arrival-order-independent content of one mechanism request
/// frontier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismIncidenceRoot([u8; 32]);

impl MechanismIncidenceRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Immutable canonical projection of one open or closed request frontier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismIncidenceSnapshot {
    scope: MechanismRequestScope,
    target_seal: Option<MechanismTargetSeal>,
    frontier_complete: bool,
    root: MechanismIncidenceRoot,
    counts: MechanismIncidenceCounts,
    target_cases: Box<[RelationalCaseId]>,
    signature_definitions: Box<[MechanismSignatureDefinition]>,
    unavailable_reason_definitions: Box<[MechanismUnavailableReasonDefinition]>,
    terminals: Box<[MechanismCaseTerminalRecord]>,
}

impl MechanismIncidenceSnapshot {
    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.scope.request_id
    }

    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn target_is_sealed(&self) -> bool {
        self.target_seal.is_some()
    }

    pub(crate) const fn target_seal(&self) -> Option<&MechanismTargetSeal> {
        self.target_seal.as_ref()
    }

    pub(crate) const fn frontier_is_complete(&self) -> bool {
        self.frontier_complete
    }

    pub(crate) const fn root(&self) -> MechanismIncidenceRoot {
        self.root
    }

    pub(crate) const fn counts(&self) -> MechanismIncidenceCounts {
        self.counts
    }

    pub(crate) fn target_cases(&self) -> &[RelationalCaseId] {
        &self.target_cases
    }

    pub(crate) fn signature_definitions(&self) -> &[MechanismSignatureDefinition] {
        &self.signature_definitions
    }

    pub(crate) fn signature_definition(
        &self,
        signature_id: MechanismSignatureId,
    ) -> Option<&MechanismSignatureDefinition> {
        self.signature_definitions
            .binary_search_by_key(&signature_id, |definition| definition.id)
            .ok()
            .map(|index| &self.signature_definitions[index])
    }

    pub(crate) fn unavailable_reason_definitions(&self) -> &[MechanismUnavailableReasonDefinition] {
        &self.unavailable_reason_definitions
    }

    pub(crate) fn unavailable_reason_definition(
        &self,
        reason_id: MechanismUnavailableReasonId,
    ) -> Option<&MechanismUnavailableReasonDefinition> {
        self.unavailable_reason_definitions
            .binary_search_by_key(&reason_id, |definition| definition.id)
            .ok()
            .map(|index| &self.unavailable_reason_definitions[index])
    }

    pub(crate) fn terminals(&self) -> &[MechanismCaseTerminalRecord] {
        &self.terminals
    }

    pub(crate) fn terminal(&self, case_id: RelationalCaseId) -> Option<MechanismCaseTerminal> {
        self.terminals
            .binary_search_by_key(&case_id, |record| record.case_id)
            .ok()
            .map(|index| self.terminals[index].terminal)
    }

    /// Recheck canonical ordering, typed closure, derived counts, and the
    /// authenticated root without trusting serialized summary fields.
    pub(crate) fn validate(&self) -> Result<(), MechanismIncidenceError> {
        validate_mechanism_incidence_snapshot(self)
    }
}

/// Outcome of atomically interning one definition and assigning one incidence
/// terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismIncidenceInsert {
    signature_inserted: bool,
    terminal_inserted: bool,
}

/// One immutable fact in the operational mechanism-publication stream.
///
/// These events are resolved from a compact lane/ordinal merge log rebuilt by
/// journal replay. Their order is useful to an append-only publisher, but it
/// is deliberately absent from mechanism snapshots and semantic roots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismPublicationDiscoveryEvent {
    Signature {
        signature_id: MechanismSignatureId,
    },
    UnavailableReason {
        reason_id: MechanismUnavailableReasonId,
    },
    Terminal(MechanismCaseTerminalRecord),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MechanismPublicationDiscoveryLane {
    Signature,
    UnavailableReason,
    Terminal,
}

/// Compact coordinate into one of the three typed discovery lanes. Keeping
/// the merge log ordinal-only avoids duplicating wide content IDs or terminal
/// records for every public event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanismPublicationDiscoveryEntry {
    lane: MechanismPublicationDiscoveryLane,
    lane_ordinal: usize,
}

impl MechanismPublicationDiscoveryEntry {
    const fn signature(lane_ordinal: usize) -> Self {
        Self {
            lane: MechanismPublicationDiscoveryLane::Signature,
            lane_ordinal,
        }
    }

    const fn unavailable_reason(lane_ordinal: usize) -> Self {
        Self {
            lane: MechanismPublicationDiscoveryLane::UnavailableReason,
            lane_ordinal,
        }
    }

    const fn terminal(lane_ordinal: usize) -> Self {
        Self {
            lane: MechanismPublicationDiscoveryLane::Terminal,
            lane_ordinal,
        }
    }
}

/// Replay-derived publication order retained after the mutable analysis
/// catalog is consumed. This is operational addressing state only: the three
/// canonical maps and their closure roots remain the semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismPublicationDiscovery {
    signatures: Box<[MechanismSignatureId]>,
    unavailable_reasons: Box<[MechanismUnavailableReasonId]>,
    terminals: Box<[MechanismCaseTerminalRecord]>,
    events: Box<[MechanismPublicationDiscoveryEntry]>,
}

impl MechanismPublicationDiscovery {
    pub(crate) const fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub(crate) fn signature_at(&self, ordinal: usize) -> Option<MechanismSignatureId> {
        self.signatures.get(ordinal).copied()
    }

    pub(crate) fn signature_suffix(&self, from_ordinal: usize) -> &[MechanismSignatureId] {
        &self.signatures[from_ordinal..]
    }

    pub(crate) const fn unavailable_reason_count(&self) -> usize {
        self.unavailable_reasons.len()
    }

    pub(crate) fn unavailable_reason_at(
        &self,
        ordinal: usize,
    ) -> Option<MechanismUnavailableReasonId> {
        self.unavailable_reasons.get(ordinal).copied()
    }

    pub(crate) fn unavailable_reason_suffix(
        &self,
        from_ordinal: usize,
    ) -> &[MechanismUnavailableReasonId] {
        &self.unavailable_reasons[from_ordinal..]
    }

    pub(crate) const fn terminal_count(&self) -> usize {
        self.terminals.len()
    }

    pub(crate) fn terminal_at(&self, ordinal: usize) -> Option<MechanismCaseTerminalRecord> {
        self.terminals.get(ordinal).copied()
    }

    pub(crate) fn terminal_suffix(&self, from_ordinal: usize) -> &[MechanismCaseTerminalRecord] {
        &self.terminals[from_ordinal..]
    }

    pub(crate) const fn event_count(&self) -> usize {
        self.events.len()
    }

    pub(crate) fn event_at(&self, ordinal: usize) -> Option<MechanismPublicationDiscoveryEvent> {
        resolve_publication_discovery_event(
            &self.signatures,
            &self.unavailable_reasons,
            &self.terminals,
            *self.events.get(ordinal)?,
        )
    }

    pub(crate) fn event_suffix(
        &self,
        from_ordinal: usize,
    ) -> MechanismPublicationDiscoveryEventSuffix<'_> {
        MechanismPublicationDiscoveryRef::Closed(self).event_suffix(from_ordinal)
    }
}

/// Borrow either the live replay-built discovery indexes or their moved
/// post-analysis-closure form through one publication-facing API.
#[derive(Clone, Copy)]
pub(crate) enum MechanismPublicationDiscoveryRef<'a> {
    Open(&'a MechanismIncidenceCatalogBuilder),
    Closed(&'a MechanismPublicationDiscovery),
}

impl<'a> MechanismPublicationDiscoveryRef<'a> {
    pub(crate) fn signature_count(self) -> usize {
        match self {
            Self::Open(builder) => builder.signature_discovery_count(),
            Self::Closed(discovery) => discovery.signature_count(),
        }
    }

    pub(crate) fn signature_at(self, ordinal: usize) -> Option<MechanismSignatureId> {
        match self {
            Self::Open(builder) => builder.signature_discovery_at(ordinal),
            Self::Closed(discovery) => discovery.signature_at(ordinal),
        }
    }

    pub(crate) fn event_count(self) -> usize {
        match self {
            Self::Open(builder) => builder.publication_event_count(),
            Self::Closed(discovery) => discovery.event_count(),
        }
    }

    pub(crate) fn event_at(self, ordinal: usize) -> Option<MechanismPublicationDiscoveryEvent> {
        match self {
            Self::Open(builder) => builder.publication_event_at(ordinal),
            Self::Closed(discovery) => discovery.event_at(ordinal),
        }
    }

    pub(crate) fn event_suffix(
        self,
        from_ordinal: usize,
    ) -> MechanismPublicationDiscoveryEventSuffix<'a> {
        assert!(from_ordinal <= self.event_count());
        MechanismPublicationDiscoveryEventSuffix {
            discovery: self,
            next_ordinal: from_ordinal,
        }
    }
}

pub(crate) struct MechanismPublicationDiscoveryEventSuffix<'a> {
    discovery: MechanismPublicationDiscoveryRef<'a>,
    next_ordinal: usize,
}

impl Iterator for MechanismPublicationDiscoveryEventSuffix<'_> {
    type Item = MechanismPublicationDiscoveryEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.discovery.event_at(self.next_ordinal)?;
        self.next_ordinal += 1;
        Some(event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self
            .discovery
            .event_count()
            .saturating_sub(self.next_ordinal);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for MechanismPublicationDiscoveryEventSuffix<'_> {}

impl MechanismIncidenceInsert {
    pub(crate) const fn signature_inserted(self) -> bool {
        self.signature_inserted
    }

    pub(crate) const fn terminal_inserted(self) -> bool {
        self.terminal_inserted
    }
}

/// Incremental, request-scoped target and incidence catalog.
#[derive(Clone, Debug)]
pub(crate) struct MechanismIncidenceCatalogBuilder {
    scope: MechanismRequestScope,
    target_seal: Option<MechanismTargetSeal>,
    target_cases: BTreeSet<RelationalCaseId>,
    target_discovery_order: Vec<RelationalCaseId>,
    target_discovery_revisions: Vec<MechanismTargetDiscoveryRevision>,
    signature_definitions: BTreeMap<MechanismSignatureId, MechanismSignatureDefinition>,
    unavailable_reason_definitions:
        BTreeMap<MechanismUnavailableReasonId, MechanismUnavailableReasonDefinition>,
    terminals: BTreeMap<RelationalCaseId, MechanismCaseTerminal>,
    /// First-intern order for immutable signature definitions. This is an
    /// operational publication lane and never enters a semantic root.
    signature_discovery_order: Vec<MechanismSignatureId>,
    /// Canonical BTree key order frozen only after exact incidence closure.
    /// This operational index is validated against the root-bearing maps and
    /// gives structural quotient replay O(1) ordinal addressing.
    closed_signature_order: OnceLock<Box<[MechanismSignatureId]>>,
    /// First-intern order for immutable permanent-reason definitions. This is
    /// likewise operational and replay-derived.
    unavailable_reason_discovery_order: Vec<MechanismUnavailableReasonId>,
    /// Operational terminal arrival order rebuilt by journal replay. The
    /// canonical map above remains the sole input to semantic roots and exact
    /// snapshots; this index only gives downstream streaming schedulers a
    /// monotone discovery ordinal when CaseIds themselves are hashes.
    terminal_discovery_order: Vec<MechanismCaseTerminalRecord>,
    /// Hash-chain revisions for every terminal prefix, including the empty
    /// prefix at ordinal zero. Length is always terminal count plus one.
    terminal_discovery_revisions: Vec<MechanismTerminalDiscoveryRevision>,
    /// Actual successful mutation order across the three discovery lanes.
    /// Entries contain only compact lane ordinals; resolving them yields a
    /// definition/reason before any dependent terminal.
    publication_discovery_order: Vec<MechanismPublicationDiscoveryEntry>,
    transition_cases: BTreeMap<TransitionId, RelationalCaseId>,
}

impl MechanismIncidenceCatalogBuilder {
    pub(crate) fn new(scope: MechanismRequestScope) -> Self {
        let initial_terminal_revision = initial_terminal_discovery_revision(scope);
        let initial_target_revision = initial_target_discovery_revision(scope);
        Self {
            scope,
            target_seal: None,
            target_cases: BTreeSet::new(),
            target_discovery_order: Vec::new(),
            target_discovery_revisions: vec![initial_target_revision],
            signature_definitions: BTreeMap::new(),
            unavailable_reason_definitions: BTreeMap::new(),
            terminals: BTreeMap::new(),
            signature_discovery_order: Vec::new(),
            closed_signature_order: OnceLock::new(),
            unavailable_reason_discovery_order: Vec::new(),
            terminal_discovery_order: Vec::new(),
            terminal_discovery_revisions: vec![initial_terminal_revision],
            publication_discovery_order: Vec::new(),
            transition_cases: BTreeMap::new(),
        }
    }

    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.scope.request_id
    }

    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    /// Add one case emitted by the request's already resolved selected or
    /// view-chosen target. Repeating an existing case is idempotent, including
    /// after the target has sealed; a genuinely new case after sealing fails.
    pub(crate) fn insert_target_case(
        &mut self,
        case_id: RelationalCaseId,
    ) -> Result<bool, MechanismIncidenceError> {
        if self.target_cases.contains(&case_id) {
            return Ok(false);
        }
        if self.target_seal.is_some() {
            return Err(MechanismIncidenceError::TargetAlreadySealed);
        }
        self.target_cases.insert(case_id);
        self.append_target_discovery(case_id);
        Ok(true)
    }

    /// Close a `for selected` target from the immutable selected relation.
    /// The streamed target prefix must already equal that exact set.
    pub(crate) fn seal_selected_target(
        &mut self,
        question: &QuestionCatalog,
    ) -> Result<bool, MechanismIncidenceError> {
        if self.scope.target != MechanismTargetId::Selected {
            return Err(MechanismIncidenceError::TargetScopeMismatch);
        }
        if question.question_id() != self.scope.question_id {
            return Err(MechanismIncidenceError::QuestionScopeMismatch {
                expected: self.scope.question_id,
                actual: question.question_id(),
            });
        }
        self.install_target_seal(
            MechanismTargetSealUpstream::SelectedQuestion {
                content_root: question.content_root(),
            },
            question.selected_case_ids(),
        )
    }

    /// Close a selected target from a compact receipt minted while the exact
    /// selected-question catalog was available. Replay rederives the local
    /// target commitment and refuses a missing or extra case before minting
    /// the request-scoped seal.
    pub(crate) fn seal_selected_target_commitment(
        &mut self,
        content_root: QuestionContentRoot,
        exact_target: MechanismTargetCaseSetCommitment,
    ) -> Result<bool, MechanismIncidenceError> {
        if self.scope.target != MechanismTargetId::Selected {
            return Err(MechanismIncidenceError::TargetScopeMismatch);
        }
        let actual = MechanismTargetCaseSetCommitment::from_canonical_cases(&self.target_cases);
        if actual != exact_target {
            return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
        }
        self.install_prechecked_target_seal(
            MechanismTargetSealUpstream::SelectedQuestion { content_root },
            exact_target,
        )
    }

    /// Close a selected target from an exact support/proof population receipt.
    /// The certified cardinality and the independently accumulated real
    /// CaseId set are both checked, so a positive proof population needs no
    /// fabricated extensional question root or representative case.
    pub(crate) fn seal_certified_selected_target_commitment(
        &mut self,
        population_root: CertifiedSelectedPopulationRoot,
        exact_cardinality: u128,
        exact_target: MechanismTargetCaseSetCommitment,
    ) -> Result<bool, MechanismIncidenceError> {
        if self.scope.target != MechanismTargetId::Selected {
            return Err(MechanismIncidenceError::TargetScopeMismatch);
        }
        let actual = MechanismTargetCaseSetCommitment::from_canonical_cases(&self.target_cases);
        if exact_target.count() != exact_cardinality || actual != exact_target {
            return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
        }
        self.install_prechecked_target_seal(
            MechanismTargetSealUpstream::CertifiedSelectedSupport {
                population_root,
                exact_cardinality,
            },
            exact_target,
        )
    }

    /// Close a semantic-choice target from its independently authenticated
    /// content root and exact canonical member set. No presentation ViewId or
    /// result root participates in this seal.
    pub(crate) fn seal_choice_target_commitment(
        &mut self,
        choice_id: ChoiceId,
        content_root: ChoiceContentRoot,
        exact_cardinality: u128,
    ) -> Result<bool, MechanismIncidenceError> {
        let MechanismTargetId::Choice(expected_choice_id) = self.scope.target else {
            return Err(MechanismIncidenceError::TargetScopeMismatch);
        };
        if choice_id != expected_choice_id {
            return Err(MechanismIncidenceError::TargetScopeMismatch);
        }
        let target = MechanismTargetCaseSetCommitment::from_canonical_cases(&self.target_cases);
        if target.count() != exact_cardinality {
            return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
        }
        self.install_prechecked_target_seal(
            MechanismTargetSealUpstream::Choice {
                choice_id,
                content_root,
            },
            target,
        )
    }

    pub(crate) const fn target_is_sealed(&self) -> bool {
        self.target_seal.is_some()
    }

    pub(crate) const fn target_seal(&self) -> Option<&MechanismTargetSeal> {
        self.target_seal.as_ref()
    }

    pub(crate) fn target_cases(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.target_cases.iter().copied()
    }

    /// O(log N) membership check for readiness-driven target admission.
    /// CaseIds are content identities, so discovery order is not guaranteed
    /// to be monotone in their canonical byte order; a scheduler must repair
    /// gaps by membership rather than treating its greatest ID as coverage.
    pub(crate) fn contains_target_case(&self, case_id: RelationalCaseId) -> bool {
        self.target_cases.contains(&case_id)
    }

    /// Number of durable target members. This is frontier state, not closure
    /// evidence; exactness still requires the private target seal.
    pub(crate) fn target_case_count(&self) -> usize {
        self.target_cases.len()
    }

    pub(crate) const fn target_discovery_count(&self) -> usize {
        self.target_discovery_order.len()
    }

    pub(crate) fn target_discovery_at(&self, ordinal: usize) -> Option<RelationalCaseId> {
        self.target_discovery_order.get(ordinal).copied()
    }

    /// Borrow target members in their durable discovery order starting at an
    /// invocation-local cursor.  This order is operational only; the sealed
    /// target set and incidence root remain canonical by CaseId.
    pub(crate) fn target_discovery_suffix(&self, from_ordinal: usize) -> &[RelationalCaseId] {
        &self.target_discovery_order[from_ordinal..]
    }

    pub(crate) fn target_discovery_prefix_revision(
        &self,
        consumed_target_count: usize,
    ) -> Option<MechanismTargetDiscoveryRevision> {
        self.target_discovery_revisions
            .get(consumed_target_count)
            .copied()
    }

    pub(crate) fn terminal(&self, case_id: RelationalCaseId) -> Option<MechanismCaseTerminal> {
        self.terminals.get(&case_id).copied()
    }

    /// Borrow terminal records in durable journal discovery order starting at
    /// an invocation-local ordinal. This is readiness state only; callers
    /// must use the canonical incidence root/seal for answer identity and
    /// exact closure.
    pub(crate) fn terminal_discovery_suffix(
        &self,
        from_ordinal: usize,
    ) -> &[MechanismCaseTerminalRecord] {
        &self.terminal_discovery_order[from_ordinal..]
    }

    pub(crate) const fn terminal_discovery_count(&self) -> usize {
        self.terminal_discovery_order.len()
    }

    pub(crate) fn terminal_discovery_at(
        &self,
        ordinal: usize,
    ) -> Option<MechanismCaseTerminalRecord> {
        self.terminal_discovery_order.get(ordinal).copied()
    }

    pub(crate) fn terminal_discovery_prefix_revision(
        &self,
        consumed_terminal_count: usize,
    ) -> Option<MechanismTerminalDiscoveryRevision> {
        self.terminal_discovery_revisions
            .get(consumed_terminal_count)
            .copied()
    }

    pub(crate) const fn signature_discovery_count(&self) -> usize {
        self.signature_discovery_order.len()
    }

    pub(crate) fn signature_discovery_at(&self, ordinal: usize) -> Option<MechanismSignatureId> {
        self.signature_discovery_order.get(ordinal).copied()
    }

    pub(crate) fn signature_discovery_suffix(
        &self,
        from_ordinal: usize,
    ) -> &[MechanismSignatureId] {
        &self.signature_discovery_order[from_ordinal..]
    }

    pub(crate) const fn unavailable_reason_discovery_count(&self) -> usize {
        self.unavailable_reason_discovery_order.len()
    }

    pub(crate) fn unavailable_reason_discovery_at(
        &self,
        ordinal: usize,
    ) -> Option<MechanismUnavailableReasonId> {
        self.unavailable_reason_discovery_order
            .get(ordinal)
            .copied()
    }

    pub(crate) fn unavailable_reason_discovery_suffix(
        &self,
        from_ordinal: usize,
    ) -> &[MechanismUnavailableReasonId] {
        &self.unavailable_reason_discovery_order[from_ordinal..]
    }

    pub(crate) const fn publication_event_count(&self) -> usize {
        self.publication_discovery_order.len()
    }

    pub(crate) fn publication_event_at(
        &self,
        ordinal: usize,
    ) -> Option<MechanismPublicationDiscoveryEvent> {
        resolve_publication_discovery_event(
            &self.signature_discovery_order,
            &self.unavailable_reason_discovery_order,
            &self.terminal_discovery_order,
            *self.publication_discovery_order.get(ordinal)?,
        )
    }

    pub(crate) fn publication_event_suffix(
        &self,
        from_ordinal: usize,
    ) -> MechanismPublicationDiscoveryEventSuffix<'_> {
        MechanismPublicationDiscoveryRef::Open(self).event_suffix(from_ordinal)
    }

    pub(crate) const fn publication_discovery(&self) -> MechanismPublicationDiscoveryRef<'_> {
        MechanismPublicationDiscoveryRef::Open(self)
    }

    pub(crate) fn terminal_case_count(&self) -> usize {
        self.terminals.len()
    }

    /// Borrow the semantic terminal relation in canonical CaseId order
    /// without materializing an immutable snapshot. Exact downstream seals
    /// use this only after [`Self::validate_complete`] succeeds.
    pub(crate) fn canonical_terminal_records(
        &self,
    ) -> impl ExactSizeIterator<Item = MechanismCaseTerminalRecord> + '_ {
        self.terminals
            .iter()
            .map(|(case_id, terminal)| MechanismCaseTerminalRecord {
                case_id: *case_id,
                terminal: *terminal,
            })
    }

    /// Number of currently durable successful incidence terminals. This is a
    /// frontier cardinality, not closure evidence; exact downstream closure
    /// still validates the canonical incidence-derived row set.
    pub(crate) fn incidence_case_count(&self) -> usize {
        self.transition_cases.len()
    }

    pub(crate) fn signature_definition(
        &self,
        signature_id: MechanismSignatureId,
    ) -> Option<&MechanismSignatureDefinition> {
        self.signature_definitions.get(&signature_id)
    }

    /// Borrow the canonical raw-signature key order for downstream quotient
    /// closure. This avoids cloning definitions or materializing a second set.
    pub(crate) fn signature_ids(&self) -> impl Iterator<Item = MechanismSignatureId> + '_ {
        self.signature_definitions.keys().copied()
    }

    pub(crate) fn signature_definition_count(&self) -> usize {
        self.signature_definitions.len()
    }

    /// O(1) canonical signature addressing is available only after the exact
    /// incidence authority has frozen and validated this order.
    pub(crate) fn closed_signature_count(&self) -> Result<usize, MechanismIncidenceError> {
        Ok(self
            .closed_signature_order
            .get()
            .ok_or(MechanismIncidenceError::CanonicalSignatureOrderNotFrozen)?
            .len())
    }

    pub(crate) fn closed_signature_id_at(
        &self,
        ordinal: usize,
    ) -> Result<Option<MechanismSignatureId>, MechanismIncidenceError> {
        Ok(self
            .closed_signature_order
            .get()
            .ok_or(MechanismIncidenceError::CanonicalSignatureOrderNotFrozen)?
            .get(ordinal)
            .copied())
    }

    pub(crate) fn unavailable_reason_definition(
        &self,
        reason_id: MechanismUnavailableReasonId,
    ) -> Option<&MechanismUnavailableReasonDefinition> {
        self.unavailable_reason_definitions.get(&reason_id)
    }

    /// Intern a normalized signature independently of concrete case
    /// materialization. Certified uniform-mechanism cells use this path; their
    /// incidence support remains in the support-evidence DAG rather than being
    /// expanded into synthetic case terminals.
    pub(crate) fn intern_signature(
        &mut self,
        definition: &MechanismSignatureDefinition,
    ) -> Result<bool, MechanismIncidenceError> {
        definition.validate_for_request(self.scope.request_id)?;
        let signature_id = definition.id;
        let inserted = self.preflight_signature(definition)?;
        if inserted {
            self.reserve_publication_discovery(1, 0, 0, 1)?;
            let discovery_ordinal = self.signature_discovery_order.len();
            self.signature_definitions
                .insert(signature_id, definition.clone());
            self.signature_discovery_order.push(signature_id);
            self.publication_discovery_order
                .push(MechanismPublicationDiscoveryEntry::signature(
                    discovery_ordinal,
                ));
        }
        Ok(inserted)
    }

    /// Atomically intern a complete signature definition and assign it to one
    /// target case. Equal repeats are idempotent. A second case may use the
    /// same signature without collapsing either case or transition row.
    pub(crate) fn record_incidence(
        &mut self,
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        definition: &MechanismSignatureDefinition,
    ) -> Result<MechanismIncidenceInsert, MechanismIncidenceError> {
        self.require_target(case_id)?;
        definition.validate_for_request(self.scope.request_id)?;
        let signature_id = definition.id;
        let terminal = MechanismCaseTerminal::Incidence {
            transition_id,
            signature_id,
        };
        self.validate_terminal_repeat(case_id, terminal)?;

        if let Some(other_case_id) = self.transition_cases.get(&transition_id).copied() {
            if other_case_id != case_id {
                return Err(MechanismIncidenceError::TransitionAssignedToMultipleCases {
                    transition_id,
                    first_case_id: other_case_id,
                    second_case_id: case_id,
                });
            }
        }

        let signature_inserted = self.preflight_signature(definition)?;
        let terminal_inserted = !self.terminals.contains_key(&case_id);
        let signature_discoveries = if signature_inserted { 1 } else { 0 };
        let terminal_discoveries = if terminal_inserted { 1 } else { 0 };
        let publication_events = signature_discoveries + terminal_discoveries;
        self.reserve_publication_discovery(
            signature_discoveries,
            0,
            terminal_discoveries,
            publication_events,
        )?;

        if signature_inserted {
            let discovery_ordinal = self.signature_discovery_order.len();
            self.signature_definitions
                .insert(signature_id, definition.clone());
            self.signature_discovery_order.push(signature_id);
            self.publication_discovery_order
                .push(MechanismPublicationDiscoveryEntry::signature(
                    discovery_ordinal,
                ));
        }
        if terminal_inserted {
            let record = MechanismCaseTerminalRecord { case_id, terminal };
            let discovery_ordinal = self.terminal_discovery_order.len();
            self.terminals.insert(case_id, terminal);
            self.transition_cases.insert(transition_id, case_id);
            self.append_terminal_discovery(record);
            self.publication_discovery_order
                .push(MechanismPublicationDiscoveryEntry::terminal(
                    discovery_ordinal,
                ));
        }
        Ok(MechanismIncidenceInsert {
            signature_inserted,
            terminal_inserted,
        })
    }

    /// Permanently close one target case without assigning a synthetic
    /// signature. An operational pause or retry is not permanent evidence and
    /// must leave the case's work node open instead of calling this method.
    pub(crate) fn record_unavailable(
        &mut self,
        case_id: RelationalCaseId,
        definition: &MechanismUnavailableReasonDefinition,
    ) -> Result<bool, MechanismIncidenceError> {
        self.require_target(case_id)?;
        definition.validate_identity()?;
        let reason_id = definition.id;
        let terminal = MechanismCaseTerminal::Unavailable { reason_id };
        self.validate_terminal_repeat(case_id, terminal)?;
        let reason_inserted = self.preflight_unavailable_reason(definition)?;
        if self.terminals.contains_key(&case_id) {
            return Ok(false);
        }
        let reason_discoveries = if reason_inserted { 1 } else { 0 };
        self.reserve_publication_discovery(0, reason_discoveries, 1, reason_discoveries + 1)?;
        if reason_inserted {
            let discovery_ordinal = self.unavailable_reason_discovery_order.len();
            self.unavailable_reason_definitions
                .insert(reason_id, definition.clone());
            self.unavailable_reason_discovery_order.push(reason_id);
            self.publication_discovery_order.push(
                MechanismPublicationDiscoveryEntry::unavailable_reason(discovery_ordinal),
            );
        }
        let record = MechanismCaseTerminalRecord { case_id, terminal };
        let discovery_ordinal = self.terminal_discovery_order.len();
        self.terminals.insert(case_id, terminal);
        self.append_terminal_discovery(record);
        self.publication_discovery_order
            .push(MechanismPublicationDiscoveryEntry::terminal(
                discovery_ordinal,
            ));
        Ok(true)
    }

    pub(crate) fn counts(&self) -> MechanismIncidenceCounts {
        let observed_signatures = self
            .terminals
            .values()
            .filter_map(|terminal| match terminal {
                MechanismCaseTerminal::Incidence { signature_id, .. } => Some(*signature_id),
                MechanismCaseTerminal::Unavailable { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        mechanism_counts(
            self.target_seal.is_some(),
            self.target_cases.len(),
            self.terminals.len(),
            self.transition_cases.len(),
            observed_signatures,
        )
    }

    pub(crate) fn frontier_is_complete(&self) -> bool {
        self.target_seal.is_some() && self.terminals.len() == self.target_cases.len()
    }

    /// Hash the canonical frontier directly from ordered indexes without
    /// allocating/cloning an immutable snapshot.
    pub(crate) fn root(&self) -> MechanismIncidenceRoot {
        mechanism_incidence_builder_root(self)
    }

    pub(crate) fn snapshot(&self) -> MechanismIncidenceSnapshot {
        let target_cases = self.target_cases.iter().copied().collect::<Vec<_>>();
        let signature_definitions = self
            .signature_definitions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let unavailable_reason_definitions = self
            .unavailable_reason_definitions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let terminals = self
            .terminals
            .iter()
            .map(|(case_id, terminal)| MechanismCaseTerminalRecord {
                case_id: *case_id,
                terminal: *terminal,
            })
            .collect::<Vec<_>>();
        let root = self.root();
        MechanismIncidenceSnapshot {
            scope: self.scope,
            target_seal: self.target_seal.clone(),
            frontier_complete: self.frontier_is_complete(),
            root,
            counts: self.counts(),
            target_cases: target_cases.into_boxed_slice(),
            signature_definitions: signature_definitions.into_boxed_slice(),
            unavailable_reason_definitions: unavailable_reason_definitions.into_boxed_slice(),
            terminals: terminals.into_boxed_slice(),
        }
    }

    /// Consume the mutable incidence indexes into their canonical snapshot
    /// projection. Discovery order and transition lookup are operational
    /// accelerators and are deliberately discarded by this snapshot-only
    /// path.
    pub(crate) fn into_snapshot(self) -> MechanismIncidenceSnapshot {
        self.into_snapshot_with_publication_discovery().0
    }

    /// Consume the catalog while moving its replay-derived publication order
    /// beside, rather than into, the canonical snapshot. The analysis journal
    /// uses this only when it consumes the final open catalog; neither the
    /// discovery lanes nor their merge log enter snapshot equality or roots.
    pub(crate) fn into_snapshot_with_publication_discovery(
        self,
    ) -> (MechanismIncidenceSnapshot, MechanismPublicationDiscovery) {
        let root = self.root();
        let frontier_complete = self.frontier_is_complete();
        let counts = self.counts();
        let Self {
            scope,
            target_seal,
            target_cases,
            target_discovery_order,
            target_discovery_revisions,
            signature_definitions,
            unavailable_reason_definitions,
            terminals,
            signature_discovery_order,
            closed_signature_order,
            unavailable_reason_discovery_order,
            terminal_discovery_order,
            terminal_discovery_revisions,
            publication_discovery_order,
            transition_cases,
        } = self;
        drop(transition_cases);
        drop(target_discovery_order);
        drop(target_discovery_revisions);
        drop(terminal_discovery_revisions);
        drop(closed_signature_order);
        (
            MechanismIncidenceSnapshot {
                scope,
                target_seal,
                frontier_complete,
                root,
                counts,
                target_cases: target_cases
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                signature_definitions: signature_definitions
                    .into_values()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                unavailable_reason_definitions: unavailable_reason_definitions
                    .into_values()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                terminals: terminals
                    .into_iter()
                    .map(|(case_id, terminal)| MechanismCaseTerminalRecord { case_id, terminal })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            MechanismPublicationDiscovery {
                signatures: signature_discovery_order.into_boxed_slice(),
                unavailable_reasons: unavailable_reason_discovery_order.into_boxed_slice(),
                terminals: terminal_discovery_order.into_boxed_slice(),
                events: publication_discovery_order.into_boxed_slice(),
            },
        )
    }

    pub(crate) fn finish(self) -> Result<ClosedMechanismIncidence, MechanismIncidenceError> {
        self.validate_complete()?;
        Ok(ClosedMechanismIncidence {
            snapshot: self.into_snapshot(),
        })
    }

    /// Materialize the immutable closure artifact without first cloning the
    /// mutable builder and its secondary transition/signature indexes.
    pub(crate) fn materialize_closed(
        &self,
    ) -> Result<ClosedMechanismIncidence, MechanismIncidenceError> {
        self.validate_complete()?;
        Ok(ClosedMechanismIncidence {
            snapshot: self.snapshot(),
        })
    }

    /// Validate exact target/terminal closure without cloning the accumulated
    /// maps. The immutable snapshot is materialized only after this preflight
    /// succeeds.
    pub(crate) fn validate_complete(&self) -> Result<(), MechanismIncidenceError> {
        let seal = self
            .target_seal
            .as_ref()
            .ok_or(MechanismIncidenceError::TargetFrontierOpen)?;
        let missing = self
            .target_cases
            .iter()
            .filter(|case_id| !self.terminals.contains_key(case_id))
            .count();
        let unexpected = self
            .terminals
            .keys()
            .filter(|case_id| !self.target_cases.contains(case_id))
            .count();
        if missing != 0 || self.terminals.len() != self.target_cases.len() {
            return Err(MechanismIncidenceError::TerminalFrontierIncomplete {
                missing,
                unexpected,
            });
        }

        seal.validate_identity()?;
        if seal.scope() != self.scope {
            return Err(MechanismIncidenceError::TargetSealScopeMismatch);
        }
        let target_case_count = self.target_cases.len() as u128;
        let target_case_set_root =
            mechanism_target_case_set_root(self.target_cases.iter().copied(), target_case_count);
        if seal.target_case_count() != target_case_count
            || seal.target_case_set_root() != target_case_set_root
        {
            return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
        }

        for (signature_id, definition) in &self.signature_definitions {
            definition.validate_for_request(self.scope.request_id())?;
            if definition.id() != *signature_id {
                return Err(MechanismIncidenceError::SnapshotContentMismatch);
            }
        }
        for (reason_id, definition) in &self.unavailable_reason_definitions {
            definition.validate_identity()?;
            if definition.id() != *reason_id {
                return Err(MechanismIncidenceError::SnapshotContentMismatch);
            }
        }

        let mut incidence_terminal_count = 0usize;
        for (case_id, terminal) in &self.terminals {
            match terminal {
                MechanismCaseTerminal::Incidence {
                    transition_id,
                    signature_id,
                } => {
                    if !self.signature_definitions.contains_key(signature_id) {
                        return Err(MechanismIncidenceError::UnknownSnapshotSignature {
                            signature_id: *signature_id,
                        });
                    }
                    if self.transition_cases.get(transition_id) != Some(case_id) {
                        return Err(MechanismIncidenceError::SnapshotContentMismatch);
                    }
                    incidence_terminal_count = incidence_terminal_count
                        .checked_add(1)
                        .ok_or(MechanismIncidenceError::SnapshotContentMismatch)?;
                }
                MechanismCaseTerminal::Unavailable { reason_id } => {
                    if !self.unavailable_reason_definitions.contains_key(reason_id) {
                        return Err(MechanismIncidenceError::UnknownSnapshotUnavailableReason {
                            reason_id: *reason_id,
                        });
                    }
                }
            }
        }
        if incidence_terminal_count != self.transition_cases.len() {
            return Err(MechanismIncidenceError::SnapshotContentMismatch);
        }
        if let Some(frozen) = self.closed_signature_order.get() {
            if frozen.len() != self.signature_definitions.len()
                || frozen
                    .iter()
                    .copied()
                    .ne(self.signature_definitions.keys().copied())
            {
                return Err(MechanismIncidenceError::CanonicalSignatureOrderMismatch);
            }
        }
        Ok(())
    }

    /// Borrow an exact closure authority without cloning the canonical maps or
    /// their definition payloads. The cached root and counts are derived only
    /// after the target seal, target set, terminal references, and transition
    /// index have all passed the same strong closure validation used by owned
    /// snapshots.
    pub(crate) fn closed_ref(
        &self,
    ) -> Result<ClosedMechanismIncidenceRef<'_>, MechanismIncidenceError> {
        self.validate_complete()?;
        self.freeze_validated_canonical_signature_order()?;
        Ok(ClosedMechanismIncidenceRef {
            incidence: self,
            root: self.root(),
        })
    }

    fn freeze_validated_canonical_signature_order(
        &self,
    ) -> Result<&[MechanismSignatureId], MechanismIncidenceError> {
        if self.closed_signature_order.get().is_none() {
            let mut canonical = Vec::new();
            canonical
                .try_reserve_exact(self.signature_definitions.len())
                .map_err(|_| {
                    MechanismIncidenceError::DiscoveryAllocationFailed(
                        "closed canonical signature order",
                    )
                })?;
            canonical.extend(self.signature_definitions.keys().copied());
            let _ = self
                .closed_signature_order
                .set(canonical.into_boxed_slice());
        }
        let frozen = self
            .closed_signature_order
            .get()
            .expect("the canonical signature order was just initialized");
        if frozen.len() != self.signature_definitions.len()
            || frozen
                .iter()
                .copied()
                .ne(self.signature_definitions.keys().copied())
        {
            return Err(MechanismIncidenceError::CanonicalSignatureOrderMismatch);
        }
        Ok(frozen)
    }

    /// Restore only after rebuilding every map through the same checked
    /// insertion paths and reproducing the complete canonical snapshot.
    pub(crate) fn from_snapshot(
        snapshot: MechanismIncidenceSnapshot,
        expected_scope: MechanismRequestScope,
    ) -> Result<Self, MechanismIncidenceError> {
        if snapshot.scope != expected_scope {
            return Err(MechanismIncidenceError::RequestScopeMismatch);
        }
        rebuild_mechanism_incidence_snapshot(&snapshot)
    }

    fn install_target_seal(
        &mut self,
        upstream: MechanismTargetSealUpstream,
        exact_target: impl IntoIterator<Item = RelationalCaseId>,
    ) -> Result<bool, MechanismIncidenceError> {
        validate_target_upstream_scope(self.scope, upstream)?;
        let (missing, unexpected) = compare_canonical_target_cases(
            exact_target.into_iter(),
            self.target_cases.iter().copied(),
        );
        if missing != 0 || unexpected != 0 {
            return Err(MechanismIncidenceError::TargetCaseSetMismatch {
                missing,
                unexpected,
            });
        }
        let target = MechanismTargetCaseSetCommitment::from_canonical_cases(&self.target_cases);
        self.install_prechecked_target_seal(upstream, target)
    }

    fn install_prechecked_target_seal(
        &mut self,
        upstream: MechanismTargetSealUpstream,
        target: MechanismTargetCaseSetCommitment,
    ) -> Result<bool, MechanismIncidenceError> {
        let seal = MechanismTargetSeal::issue(self.scope, upstream, target);
        match self.target_seal.as_ref() {
            Some(existing) if existing == &seal => Ok(false),
            Some(_) => Err(MechanismIncidenceError::TargetSealConflict),
            None => {
                self.target_seal = Some(seal);
                Ok(true)
            }
        }
    }

    fn require_target(&self, case_id: RelationalCaseId) -> Result<(), MechanismIncidenceError> {
        if self.target_cases.contains(&case_id) {
            Ok(())
        } else {
            Err(MechanismIncidenceError::UnknownTargetCase { case_id })
        }
    }

    /// Reserve every operational append before any corresponding semantic map
    /// mutation. Once this preflight succeeds, all discovery pushes below are
    /// allocation-free and cannot strand a definition without its merge-log
    /// entry or a terminal without its prior definition/reason event.
    fn reserve_publication_discovery(
        &mut self,
        signatures: usize,
        unavailable_reasons: usize,
        terminals: usize,
        events: usize,
    ) -> Result<(), MechanismIncidenceError> {
        self.signature_discovery_order
            .try_reserve(signatures)
            .map_err(|_| {
                MechanismIncidenceError::DiscoveryAllocationFailed("signature discovery lane")
            })?;
        self.unavailable_reason_discovery_order
            .try_reserve(unavailable_reasons)
            .map_err(|_| {
                MechanismIncidenceError::DiscoveryAllocationFailed(
                    "unavailable-reason discovery lane",
                )
            })?;
        self.terminal_discovery_order
            .try_reserve(terminals)
            .map_err(|_| {
                MechanismIncidenceError::DiscoveryAllocationFailed("terminal discovery lane")
            })?;
        self.terminal_discovery_revisions
            .try_reserve(terminals)
            .map_err(|_| {
                MechanismIncidenceError::DiscoveryAllocationFailed(
                    "terminal discovery revision lane",
                )
            })?;
        self.publication_discovery_order
            .try_reserve(events)
            .map_err(|_| {
                MechanismIncidenceError::DiscoveryAllocationFailed(
                    "mechanism publication merge log",
                )
            })?;
        Ok(())
    }

    fn append_terminal_discovery(&mut self, record: MechanismCaseTerminalRecord) {
        let previous = self
            .terminal_discovery_revisions
            .last()
            .copied()
            .expect("terminal discovery revision lane always retains its empty prefix");
        let next = advance_terminal_discovery_revision(previous, record);
        self.terminal_discovery_order.push(record);
        self.terminal_discovery_revisions.push(next);
    }

    fn append_target_discovery(&mut self, case_id: RelationalCaseId) {
        let previous = self
            .target_discovery_revisions
            .last()
            .copied()
            .expect("target discovery revision lane always retains its empty prefix");
        let next = advance_target_discovery_revision(previous, case_id);
        self.target_discovery_order.push(case_id);
        self.target_discovery_revisions.push(next);
    }

    fn preflight_signature(
        &mut self,
        definition: &MechanismSignatureDefinition,
    ) -> Result<bool, MechanismIncidenceError> {
        let signature_id = definition.id;
        match self.signature_definitions.get(&signature_id) {
            Some(existing) if existing == definition => Ok(false),
            Some(_) => Err(MechanismIncidenceError::SignatureDefinitionCollision { signature_id }),
            None => {
                // This cache may have been minted while planning a close event
                // that was not appended. Any legitimate pre-close extension
                // invalidates it; the journal's stored closure remains the
                // actual write barrier and will reject post-close payload.
                self.closed_signature_order.take();
                Ok(true)
            }
        }
    }

    fn preflight_unavailable_reason(
        &self,
        definition: &MechanismUnavailableReasonDefinition,
    ) -> Result<bool, MechanismIncidenceError> {
        let reason_id = definition.id;
        match self.unavailable_reason_definitions.get(&reason_id) {
            Some(existing) if existing == definition => Ok(false),
            Some(_) => {
                Err(MechanismIncidenceError::UnavailableReasonDefinitionCollision { reason_id })
            }
            None => Ok(true),
        }
    }

    fn validate_terminal_repeat(
        &self,
        case_id: RelationalCaseId,
        terminal: MechanismCaseTerminal,
    ) -> Result<(), MechanismIncidenceError> {
        match self.terminals.get(&case_id) {
            Some(existing) if *existing == terminal => Ok(()),
            Some(_) => Err(MechanismIncidenceError::TerminalConflict { case_id }),
            None => Ok(()),
        }
    }
}

fn resolve_publication_discovery_event(
    signatures: &[MechanismSignatureId],
    unavailable_reasons: &[MechanismUnavailableReasonId],
    terminals: &[MechanismCaseTerminalRecord],
    entry: MechanismPublicationDiscoveryEntry,
) -> Option<MechanismPublicationDiscoveryEvent> {
    match entry.lane {
        MechanismPublicationDiscoveryLane::Signature => signatures
            .get(entry.lane_ordinal)
            .copied()
            .map(|signature_id| MechanismPublicationDiscoveryEvent::Signature { signature_id }),
        MechanismPublicationDiscoveryLane::UnavailableReason => unavailable_reasons
            .get(entry.lane_ordinal)
            .copied()
            .map(|reason_id| MechanismPublicationDiscoveryEvent::UnavailableReason { reason_id }),
        MechanismPublicationDiscoveryLane::Terminal => terminals
            .get(entry.lane_ordinal)
            .copied()
            .map(MechanismPublicationDiscoveryEvent::Terminal),
    }
}

/// Borrowed exact closure authority over a live incidence catalog.
///
/// Unlike [`ClosedMechanismIncidence`], this view owns no target, definition,
/// reason, or terminal collections. Construction performs strong closure
/// validation and then caches the authenticated root and counts. Its immutable
/// borrow prevents the underlying exact sets from changing while a structural
/// or support closure streams their canonical order.
#[derive(Clone, Copy)]
pub(crate) struct ClosedMechanismIncidenceRef<'a> {
    incidence: &'a MechanismIncidenceCatalogBuilder,
    root: MechanismIncidenceRoot,
}

impl<'a> ClosedMechanismIncidenceRef<'a> {
    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.incidence.scope
    }

    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.incidence.scope.request_id
    }

    pub(crate) const fn root(&self) -> MechanismIncidenceRoot {
        self.root
    }

    pub(crate) fn target_seal(&self) -> &'a MechanismTargetSeal {
        self.incidence
            .target_seal
            .as_ref()
            .expect("a borrowed closed incidence always has a target seal")
    }

    pub(crate) fn target_case_count(&self) -> usize {
        self.incidence.target_cases.len()
    }

    pub(crate) fn terminal_case_count(&self) -> usize {
        self.incidence.terminals.len()
    }

    pub(crate) fn incidence_case_count(&self) -> usize {
        self.incidence.transition_cases.len()
    }

    pub(crate) fn signature_definition_count(&self) -> usize {
        self.incidence
            .closed_signature_order
            .get()
            .expect("a borrowed closed incidence has frozen signature order")
            .len()
    }

    pub(crate) fn signature_ids(&self) -> impl ExactSizeIterator<Item = MechanismSignatureId> + '_ {
        self.incidence
            .closed_signature_order
            .get()
            .expect("a borrowed closed incidence has frozen signature order")
            .iter()
            .copied()
    }

    pub(crate) fn signature_id_at(&self, ordinal: usize) -> Option<MechanismSignatureId> {
        self.incidence
            .closed_signature_order
            .get()
            .expect("a borrowed closed incidence has frozen signature order")
            .get(ordinal)
            .copied()
    }

    pub(crate) fn canonical_terminal_records(
        &self,
    ) -> impl ExactSizeIterator<Item = MechanismCaseTerminalRecord> + '_ {
        self.incidence.canonical_terminal_records()
    }

    pub(super) const fn builder(&self) -> &'a MechanismIncidenceCatalogBuilder {
        self.incidence
    }
}

/// Exact target and terminal frontier after finite exhaustion. Distinct
/// signature counts may still be a lower bound or unknown when one or more
/// cases are permanently unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedMechanismIncidence {
    snapshot: MechanismIncidenceSnapshot,
}

impl ClosedMechanismIncidence {
    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.snapshot.scope.request_id
    }

    pub(crate) const fn target_seal(&self) -> &MechanismTargetSeal {
        self.snapshot
            .target_seal
            .as_ref()
            .expect("a closed mechanism incidence always has a target seal")
    }

    pub(crate) const fn root(&self) -> MechanismIncidenceRoot {
        self.snapshot.root
    }

    pub(crate) const fn counts(&self) -> MechanismIncidenceCounts {
        self.snapshot.counts
    }

    pub(crate) fn snapshot(&self) -> &MechanismIncidenceSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismIncidenceError {
    RequestScopeMismatch,
    QuestionScopeMismatch {
        expected: QuestionId,
        actual: QuestionId,
    },
    TargetScopeMismatch,
    TargetCaseSetMismatch {
        missing: usize,
        unexpected: usize,
    },
    TargetSealConflict,
    UnsupportedTargetSealVersion {
        actual: u32,
        expected: u32,
    },
    TargetSealScopeMismatch,
    TargetSealIdMismatch {
        claimed: MechanismTargetSealId,
        derived: MechanismTargetSealId,
    },
    TargetSealCaseSetMismatch,
    NonCanonicalSnapshot(&'static str),
    SnapshotContentMismatch,
    UnknownSnapshotSignature {
        signature_id: MechanismSignatureId,
    },
    UnknownSnapshotUnavailableReason {
        reason_id: MechanismUnavailableReasonId,
    },
    SignatureRequestMismatch,
    SignatureDefinitionDigestMismatch {
        signature_id: MechanismSignatureId,
    },
    SignatureIdMismatch {
        claimed: MechanismSignatureId,
        derived: MechanismSignatureId,
    },
    SignatureDefinitionCollision {
        signature_id: MechanismSignatureId,
    },
    CanonicalSignatureOrderNotFrozen,
    CanonicalSignatureOrderMismatch,
    UnavailableReasonIdMismatch {
        claimed: MechanismUnavailableReasonId,
        derived: MechanismUnavailableReasonId,
    },
    UnavailableReasonDefinitionCollision {
        reason_id: MechanismUnavailableReasonId,
    },
    TargetAlreadySealed,
    UnknownTargetCase {
        case_id: RelationalCaseId,
    },
    TerminalConflict {
        case_id: RelationalCaseId,
    },
    TransitionAssignedToMultipleCases {
        transition_id: TransitionId,
        first_case_id: RelationalCaseId,
        second_case_id: RelationalCaseId,
    },
    DiscoveryAllocationFailed(&'static str),
    TargetFrontierOpen,
    TerminalFrontierIncomplete {
        missing: usize,
        unexpected: usize,
    },
}

impl fmt::Display for MechanismIncidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestScopeMismatch => formatter
                .write_str("mechanism snapshot belongs to another checked request scope"),
            Self::QuestionScopeMismatch { .. } => formatter.write_str(
                "mechanism selected target belongs to a different closed FIND question",
            ),
            Self::TargetScopeMismatch => formatter
                .write_str("mechanism target closure kind does not match the request target"),
            Self::TargetCaseSetMismatch { .. } => formatter.write_str(
                "mechanism target prefix has missing or extra cases relative to closed upstream content",
            ),
            Self::TargetSealConflict => formatter
                .write_str("mechanism target already has different closure evidence"),
            Self::UnsupportedTargetSealVersion { actual, expected } => write!(
                formatter,
                "unsupported mechanism target seal version {actual}; expected {expected}"
            ),
            Self::TargetSealScopeMismatch => formatter
                .write_str("mechanism target seal does not match its checked request scope"),
            Self::TargetSealIdMismatch { .. } => formatter
                .write_str("mechanism target seal ID does not match its semantic content"),
            Self::TargetSealCaseSetMismatch => formatter.write_str(
                "mechanism target seal case root or count does not match the snapshot target set",
            ),
            Self::NonCanonicalSnapshot(subject) => {
                write!(formatter, "mechanism snapshot {subject} are not canonical")
            }
            Self::SnapshotContentMismatch => formatter.write_str(
                "mechanism snapshot summary or root does not match reconstructed content",
            ),
            Self::UnknownSnapshotSignature { .. } => formatter.write_str(
                "mechanism snapshot incidence references an absent signature definition",
            ),
            Self::UnknownSnapshotUnavailableReason { .. } => formatter.write_str(
                "mechanism snapshot terminal references an absent unavailable-reason definition",
            ),
            Self::SignatureRequestMismatch => {
                formatter.write_str("mechanism signature definition belongs to another request")
            }
            Self::SignatureDefinitionDigestMismatch { .. } => formatter.write_str(
                "mechanism signature definition bytes disagree with their canonical digest",
            ),
            Self::SignatureIdMismatch { .. } => {
                formatter.write_str("mechanism signature ID disagrees with its request and content")
            }
            Self::SignatureDefinitionCollision { .. } => formatter.write_str(
                "mechanism signature content-ID collision has unequal canonical definitions",
            ),
            Self::CanonicalSignatureOrderNotFrozen => formatter.write_str(
                "canonical mechanism signature order is unavailable before incidence closure",
            ),
            Self::CanonicalSignatureOrderMismatch => formatter.write_str(
                "frozen canonical mechanism signature order disagrees with incidence closure",
            ),
            Self::UnavailableReasonIdMismatch { .. } => formatter.write_str(
                "mechanism unavailable-reason ID disagrees with its canonical payload",
            ),
            Self::UnavailableReasonDefinitionCollision { .. } => formatter.write_str(
                "mechanism unavailable-reason content-ID collision has unequal canonical payloads",
            ),
            Self::TargetAlreadySealed => {
                formatter.write_str("mechanism request target cannot grow after its frontier seals")
            }
            Self::UnknownTargetCase { .. } => formatter
                .write_str("mechanism terminal names a case outside the known request target"),
            Self::TerminalConflict { .. } => formatter
                .write_str("mechanism case terminal contradicts previously accepted evidence"),
            Self::TransitionAssignedToMultipleCases { .. } => formatter.write_str(
                "one request-relative transition is assigned to more than one relational case",
            ),
            Self::DiscoveryAllocationFailed(subject) => {
                write!(formatter, "cannot reserve {subject}")
            }
            Self::TargetFrontierOpen => formatter
                .write_str("mechanism incidence cannot finish while its target frontier is open"),
            Self::TerminalFrontierIncomplete { .. } => formatter.write_str(
                "mechanism incidence cannot finish before every target case has one terminal",
            ),
        }
    }
}

impl Error for MechanismIncidenceError {}

fn validate_target_upstream_scope(
    scope: MechanismRequestScope,
    upstream: MechanismTargetSealUpstream,
) -> Result<(), MechanismIncidenceError> {
    match (scope.target, upstream) {
        (
            MechanismTargetId::Selected,
            MechanismTargetSealUpstream::SelectedQuestion { .. }
            | MechanismTargetSealUpstream::CertifiedSelectedSupport { .. },
        ) => Ok(()),
        (
            MechanismTargetId::Choice(expected),
            MechanismTargetSealUpstream::Choice { choice_id, .. },
        ) if expected == choice_id => Ok(()),
        _ => Err(MechanismIncidenceError::TargetSealScopeMismatch),
    }
}

/// Compare two already canonical CaseId streams without allocating a second
/// copy of a potentially large selected population.
fn compare_canonical_target_cases(
    expected: impl Iterator<Item = RelationalCaseId>,
    actual: impl Iterator<Item = RelationalCaseId>,
) -> (usize, usize) {
    let mut expected = expected.peekable();
    let mut actual = actual.peekable();
    let mut missing = 0usize;
    let mut unexpected = 0usize;
    loop {
        match (expected.peek(), actual.peek()) {
            (Some(left), Some(right)) => match left.cmp(right) {
                std::cmp::Ordering::Less => {
                    missing = missing.saturating_add(1);
                    expected.next();
                }
                std::cmp::Ordering::Equal => {
                    expected.next();
                    actual.next();
                }
                std::cmp::Ordering::Greater => {
                    unexpected = unexpected.saturating_add(1);
                    actual.next();
                }
            },
            (Some(_), None) => {
                missing = missing.saturating_add(expected.count());
                break;
            }
            (None, Some(_)) => {
                unexpected = unexpected.saturating_add(actual.count());
                break;
            }
            (None, None) => break,
        }
    }
    (missing, unexpected)
}

fn initial_terminal_discovery_revision(
    scope: MechanismRequestScope,
) -> MechanismTerminalDiscoveryRevision {
    let mut hasher = CanonicalHasher::new(TERMINAL_DISCOVERY_REVISION_HASH_V1);
    hasher.tag(0x01);
    hasher.digest(scope.request_id().bytes());
    MechanismTerminalDiscoveryRevision(hasher.finish())
}

fn initial_target_discovery_revision(
    scope: MechanismRequestScope,
) -> MechanismTargetDiscoveryRevision {
    let mut hasher = CanonicalHasher::new(TARGET_DISCOVERY_REVISION_HASH_V1);
    hasher.tag(0x01);
    hasher.digest(scope.request_id().bytes());
    MechanismTargetDiscoveryRevision(hasher.finish())
}

fn advance_target_discovery_revision(
    previous: MechanismTargetDiscoveryRevision,
    case_id: RelationalCaseId,
) -> MechanismTargetDiscoveryRevision {
    let mut hasher = CanonicalHasher::new(TARGET_DISCOVERY_REVISION_HASH_V1);
    hasher.tag(0x01);
    hasher.digest(previous.bytes());
    hasher.tag(0x02);
    hasher.digest(case_id.bytes());
    MechanismTargetDiscoveryRevision(hasher.finish())
}

fn advance_terminal_discovery_revision(
    previous: MechanismTerminalDiscoveryRevision,
    record: MechanismCaseTerminalRecord,
) -> MechanismTerminalDiscoveryRevision {
    let mut hasher = CanonicalHasher::new(TERMINAL_DISCOVERY_REVISION_HASH_V1);
    hasher.tag(0x01);
    hasher.digest(previous.bytes());
    hasher.tag(0x02);
    hasher.digest(record.case_id().bytes());
    match record.terminal() {
        MechanismCaseTerminal::Incidence {
            transition_id,
            signature_id,
        } => {
            hasher.tag(0x03);
            hasher.digest(transition_id.bytes());
            hasher.digest(signature_id.request_id().bytes());
            hasher.digest(signature_id.bytes());
        }
        MechanismCaseTerminal::Unavailable { reason_id } => {
            hasher.tag(0x04);
            hasher.digest(reason_id.bytes());
        }
    }
    MechanismTerminalDiscoveryRevision(hasher.finish())
}

fn mechanism_target_case_set_root(
    cases: impl IntoIterator<Item = RelationalCaseId>,
    count: u128,
) -> MechanismTargetCaseSetRoot {
    let mut hasher = CanonicalHasher::new(MECHANISM_TARGET_CASE_SET_ROOT_HASH_V1);
    hasher.u128(count);
    for case_id in cases {
        hasher.digest(case_id.bytes());
    }
    MechanismTargetCaseSetRoot(hasher.finish())
}

fn derive_mechanism_target_seal_id(
    version: u32,
    scope: MechanismRequestScope,
    upstream: MechanismTargetSealUpstream,
    target_case_set_root: MechanismTargetCaseSetRoot,
    target_case_count: u128,
) -> MechanismTargetSealId {
    let mut hasher = CanonicalHasher::new(MECHANISM_TARGET_SEAL_ID_HASH_V3);
    hasher.u32(version);
    hash_request_scope(&mut hasher, scope);
    hash_target_seal_upstream(&mut hasher, upstream);
    hasher.digest(target_case_set_root.bytes());
    hasher.u128(target_case_count);
    MechanismTargetSealId(hasher.finish())
}

fn hash_request_scope(hasher: &mut CanonicalHasher, scope: MechanismRequestScope) {
    hasher.digest(scope.request_id.bytes());
    hasher.digest(scope.question_id.bytes());
    match scope.target {
        MechanismTargetId::Selected => hasher.tag(0x01),
        MechanismTargetId::Choice(choice_id) => {
            hasher.tag(0x02);
            hasher.digest(choice_id.bytes());
        }
    }
}

fn hash_target_seal_upstream(hasher: &mut CanonicalHasher, upstream: MechanismTargetSealUpstream) {
    match upstream {
        MechanismTargetSealUpstream::SelectedQuestion { content_root } => {
            hasher.tag(0x01);
            hasher.digest(content_root.bytes());
        }
        MechanismTargetSealUpstream::CertifiedSelectedSupport {
            population_root,
            exact_cardinality,
        } => {
            hasher.tag(0x03);
            hasher.digest(population_root.bytes());
            hasher.u128(exact_cardinality);
        }
        MechanismTargetSealUpstream::Choice {
            choice_id,
            content_root,
        } => {
            hasher.tag(0x02);
            hasher.digest(choice_id.bytes());
            hasher.digest(content_root.bytes());
        }
    }
}

fn validate_mechanism_incidence_snapshot(
    snapshot: &MechanismIncidenceSnapshot,
) -> Result<(), MechanismIncidenceError> {
    rebuild_mechanism_incidence_snapshot(snapshot).map(|_| ())
}

fn rebuild_mechanism_incidence_snapshot(
    snapshot: &MechanismIncidenceSnapshot,
) -> Result<MechanismIncidenceCatalogBuilder, MechanismIncidenceError> {
    if snapshot
        .target_cases
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(MechanismIncidenceError::NonCanonicalSnapshot(
            "target cases",
        ));
    }
    if snapshot
        .signature_definitions
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(MechanismIncidenceError::NonCanonicalSnapshot(
            "signature definitions",
        ));
    }
    if snapshot
        .unavailable_reason_definitions
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(MechanismIncidenceError::NonCanonicalSnapshot(
            "unavailable reason definitions",
        ));
    }
    if snapshot
        .terminals
        .windows(2)
        .any(|pair| pair[0].case_id >= pair[1].case_id)
    {
        return Err(MechanismIncidenceError::NonCanonicalSnapshot(
            "case terminals",
        ));
    }

    if let Some(seal) = snapshot.target_seal.as_ref() {
        seal.validate_identity()?;
        if seal.scope != snapshot.scope {
            return Err(MechanismIncidenceError::TargetSealScopeMismatch);
        }
        let count = snapshot.target_cases.len() as u128;
        let root = mechanism_target_case_set_root(snapshot.target_cases.iter().copied(), count);
        if seal.target_case_count != count || seal.target_case_set_root != root {
            return Err(MechanismIncidenceError::TargetSealCaseSetMismatch);
        }
    }

    let mut builder = MechanismIncidenceCatalogBuilder::new(snapshot.scope);
    for case_id in snapshot.target_cases.iter().copied() {
        if !builder.insert_target_case(case_id)? {
            return Err(MechanismIncidenceError::NonCanonicalSnapshot(
                "target cases",
            ));
        }
    }
    for definition in snapshot.signature_definitions.iter() {
        if !builder.intern_signature(definition)? {
            return Err(MechanismIncidenceError::NonCanonicalSnapshot(
                "signature definitions",
            ));
        }
    }
    for record in snapshot.terminals.iter().copied() {
        match record.terminal {
            MechanismCaseTerminal::Incidence {
                transition_id,
                signature_id,
            } => {
                let definition = builder
                    .signature_definition(signature_id)
                    .cloned()
                    .ok_or(MechanismIncidenceError::UnknownSnapshotSignature { signature_id })?;
                builder.record_incidence(record.case_id, transition_id, &definition)?;
            }
            MechanismCaseTerminal::Unavailable { reason_id } => {
                let definition = snapshot.unavailable_reason_definition(reason_id).ok_or(
                    MechanismIncidenceError::UnknownSnapshotUnavailableReason { reason_id },
                )?;
                builder.record_unavailable(record.case_id, definition)?;
            }
        }
    }
    builder.target_seal = snapshot.target_seal.clone();
    if builder.snapshot() != *snapshot {
        return Err(MechanismIncidenceError::SnapshotContentMismatch);
    }
    Ok(builder)
}

fn mechanism_counts(
    target_sealed: bool,
    target_count: usize,
    terminal_count: usize,
    incidence_count: usize,
    distinct_signature_count: usize,
) -> MechanismIncidenceCounts {
    let target_count = target_count as u128;
    let terminal_count = terminal_count as u128;
    let incidence_count = incidence_count as u128;
    let distinct_signature_count = distinct_signature_count as u128;
    let unavailable_count = terminal_count - incidence_count;
    let frontier_complete = target_sealed && terminal_count == target_count;
    let population = |value| {
        if frontier_complete {
            MechanismCountEvidence::Exact(value)
        } else {
            MechanismCountEvidence::LowerBound(value)
        }
    };
    let distinct_signatures = if frontier_complete && unavailable_count == 0 {
        MechanismCountEvidence::Exact(distinct_signature_count)
    } else if distinct_signature_count == 0 {
        MechanismCountEvidence::Unknown {
            confirmed_lower_bound: 0,
        }
    } else {
        MechanismCountEvidence::LowerBound(distinct_signature_count)
    };
    MechanismIncidenceCounts {
        target_cases: if target_sealed {
            MechanismCountEvidence::Exact(target_count)
        } else {
            MechanismCountEvidence::LowerBound(target_count)
        },
        terminal_cases: population(terminal_count),
        incidence_cases: population(incidence_count),
        unavailable_cases: population(unavailable_count),
        distinct_signatures,
    }
}

fn mechanism_incidence_root(
    scope: MechanismRequestScope,
    target_seal: Option<&MechanismTargetSeal>,
    target_cases: &[RelationalCaseId],
    signature_definitions: &[MechanismSignatureDefinition],
    unavailable_reason_definitions: &[MechanismUnavailableReasonDefinition],
    terminals: &[MechanismCaseTerminalRecord],
) -> MechanismIncidenceRoot {
    let mut hasher = CanonicalHasher::new(MECHANISM_INCIDENCE_ROOT_HASH_V3);
    hash_request_scope(&mut hasher, scope);
    match target_seal {
        None => hasher.tag(0x00),
        Some(seal) => {
            hasher.tag(0x01);
            hasher.u32(seal.version);
            hasher.digest(seal.id.bytes());
            hash_target_seal_upstream(&mut hasher, seal.upstream);
            hasher.digest(seal.target_case_set_root.bytes());
            hasher.u128(seal.target_case_count);
        }
    }

    hasher.u128(target_cases.len() as u128);
    for case_id in target_cases {
        hasher.digest(case_id.bytes());
    }

    hasher.u128(signature_definitions.len() as u128);
    for definition in signature_definitions {
        hasher.digest(definition.id.bytes());
        hasher.digest(definition.canonical_differential_digest);
        hasher.bytes(&definition.canonical_definition);
    }

    hasher.u128(unavailable_reason_definitions.len() as u128);
    for definition in unavailable_reason_definitions {
        hasher.digest(definition.id.bytes());
        hasher.bytes(&definition.canonical_reason);
    }

    hasher.u128(terminals.len() as u128);
    for record in terminals {
        hasher.digest(record.case_id.bytes());
        match record.terminal {
            MechanismCaseTerminal::Incidence {
                transition_id,
                signature_id,
            } => {
                hasher.tag(0x01);
                hasher.digest(transition_id.bytes());
                hasher.digest(signature_id.bytes());
            }
            MechanismCaseTerminal::Unavailable { reason_id } => {
                hasher.tag(0x02);
                hasher.digest(reason_id.bytes());
            }
        }
    }
    MechanismIncidenceRoot(hasher.finish())
}

fn mechanism_incidence_builder_root(
    builder: &MechanismIncidenceCatalogBuilder,
) -> MechanismIncidenceRoot {
    let mut hasher = CanonicalHasher::new(MECHANISM_INCIDENCE_ROOT_HASH_V3);
    hash_request_scope(&mut hasher, builder.scope);
    match builder.target_seal.as_ref() {
        None => hasher.tag(0x00),
        Some(seal) => {
            hasher.tag(0x01);
            hasher.u32(seal.version);
            hasher.digest(seal.id.bytes());
            hash_target_seal_upstream(&mut hasher, seal.upstream);
            hasher.digest(seal.target_case_set_root.bytes());
            hasher.u128(seal.target_case_count);
        }
    }

    hasher.u128(builder.target_cases.len() as u128);
    for case_id in &builder.target_cases {
        hasher.digest(case_id.bytes());
    }

    hasher.u128(builder.signature_definitions.len() as u128);
    for definition in builder.signature_definitions.values() {
        hasher.digest(definition.id.bytes());
        hasher.digest(definition.canonical_differential_digest);
        hasher.bytes(&definition.canonical_definition);
    }

    hasher.u128(builder.unavailable_reason_definitions.len() as u128);
    for definition in builder.unavailable_reason_definitions.values() {
        hasher.digest(definition.id.bytes());
        hasher.bytes(&definition.canonical_reason);
    }

    hasher.u128(builder.terminals.len() as u128);
    for (case_id, terminal) in &builder.terminals {
        hasher.digest(case_id.bytes());
        match terminal {
            MechanismCaseTerminal::Incidence {
                transition_id,
                signature_id,
            } => {
                hasher.tag(0x01);
                hasher.digest(transition_id.bytes());
                hasher.digest(signature_id.bytes());
            }
            MechanismCaseTerminal::Unavailable { reason_id } => {
                hasher.tag(0x02);
                hasher.digest(reason_id.bytes());
            }
        }
    }
    MechanismIncidenceRoot(hasher.finish())
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

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relation::{
        AdmissionCatalogBuilder, AdmissionDecision, AdmissionId, FindPolarity,
        QuestionCatalogBuilder, RelationCatalogBuilder, RelationId, RelationLineageId,
        RelationProvenance, RelationSupportId, SelectionDecision, SourceKey, SourceRow,
        SuccessorKey, SuccessorRow,
    };
    use crate::explore::ExploreValue;

    fn request(name: &str) -> (RelationId, AdmissionId, QuestionId, MechanismRequestScope) {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(format!("relation-{name}").as_bytes());
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"resources-never-fall",
            FindPolarity::Violations,
        );
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"assess-policy",
            b"dynamic-control-v1",
        );
        (
            relation_id,
            admission_id,
            question_id,
            MechanismRequestScope::new(request_id, question_id, MechanismTargetId::Selected),
        )
    }

    fn provenance(name: &str) -> RelationProvenance {
        RelationProvenance::new(
            [RelationLineageId::from_canonical_preimage(
                format!("lineage-{name}").as_bytes(),
            )],
            [RelationSupportId::from_canonical_preimage(
                format!("support-{name}").as_bytes(),
            )],
        )
    }

    #[derive(Clone)]
    struct CaseFixture {
        source: SourceRow,
        successor: SuccessorRow,
        case_id: RelationalCaseId,
    }

    fn case_fixture(relation_id: RelationId, name: &str, before: i64, after: i64) -> CaseFixture {
        let source = SourceRow::new(
            ExploreValue::String("salary-promotion".to_string()),
            ExploreValue::Tuple(vec![
                ExploreValue::String(name.to_string()),
                ExploreValue::Int(before),
            ]),
            provenance(&format!("source-{name}")),
        );
        let source_key = SourceKey::derive(relation_id, &source);
        let successor = SuccessorRow::new(
            ExploreValue::Tuple(vec![
                ExploreValue::String(name.to_string()),
                ExploreValue::Int(after),
            ]),
            provenance(&format!("successor-{name}")),
        );
        let successor_key = SuccessorKey::derive(relation_id, source_key, &successor);
        let case_id = RelationalCaseId::derive(relation_id, source_key, successor_key);
        CaseFixture {
            source,
            successor,
            case_id,
        }
    }

    fn case(relation_id: RelationId, name: &str, before: i64, after: i64) -> RelationalCaseId {
        case_fixture(relation_id, name, before, after).case_id
    }

    fn closed_question(
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        cases: &[CaseFixture],
    ) -> QuestionCatalog {
        let mut relation = RelationCatalogBuilder::new(relation_id);
        for fixture in cases {
            let source_key = relation.insert_source(fixture.source.clone()).unwrap();
            let (_, case_id) = relation
                .insert_successor(source_key, fixture.successor.clone())
                .unwrap();
            assert_eq!(case_id, fixture.case_id);
            relation.seal_successor_enumeration(source_key).unwrap();
        }
        relation.seal_source_enumeration();
        let relation_snapshot = relation.snapshot();
        let mut admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        let mut question = QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        for fixture in cases {
            admission
                .classify(
                    &relation_snapshot,
                    fixture.case_id,
                    AdmissionDecision::Admitted,
                )
                .unwrap();
            question
                .classify(
                    &relation_snapshot,
                    &admission,
                    fixture.case_id,
                    SelectionDecision::Selected,
                )
                .unwrap();
        }
        let relation = relation.finish().unwrap();
        let admission = admission.finish(&relation).unwrap();
        question.finish(&relation, &admission).unwrap()
    }

    fn transition(name: &str) -> TransitionId {
        TransitionId::from_bytes(Sha256::digest(format!("transition-{name}")).into())
    }

    #[test]
    fn distinct_cases_can_share_one_signature_without_collapsing_support() {
        let (relation_id, admission_id, question_id, scope) = request("shared-signature");
        let request_id = scope.request_id();
        let carl = case_fixture(relation_id, "Carl", 199_999, 200_000);
        let john = case_fixture(relation_id, "John", 9_999, 10_000);
        let question = closed_question(
            relation_id,
            admission_id,
            question_id,
            &[carl.clone(), john.clone()],
        );
        let definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"same-complete-differential-control-graph".as_slice(),
        );

        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        assert!(builder.insert_target_case(carl.case_id).unwrap());
        assert!(builder.insert_target_case(john.case_id).unwrap());
        let carl_insert = builder
            .record_incidence(carl.case_id, transition("Carl"), &definition)
            .unwrap();
        let john_insert = builder
            .record_incidence(john.case_id, transition("John"), &definition)
            .unwrap();
        assert!(carl_insert.signature_inserted());
        assert!(!john_insert.signature_inserted());
        assert!(carl_insert.terminal_inserted());
        assert!(john_insert.terminal_inserted());
        assert_eq!(
            builder.counts().target_cases(),
            MechanismCountEvidence::LowerBound(2)
        );
        assert_eq!(
            builder.counts().distinct_signatures(),
            MechanismCountEvidence::LowerBound(1)
        );

        assert!(builder.seal_selected_target(&question).unwrap());
        let closed = builder.finish().unwrap();
        assert_eq!(
            closed.counts().incidence_cases(),
            MechanismCountEvidence::Exact(2)
        );
        assert_eq!(
            closed.counts().distinct_signatures(),
            MechanismCountEvidence::Exact(1)
        );
        assert_eq!(closed.snapshot().terminals().len(), 2);
        assert_eq!(closed.snapshot().signature_definitions().len(), 1);
    }

    #[test]
    fn finish_requires_a_sealed_target_and_one_terminal_per_case() {
        let (relation_id, admission_id, question_id, scope) = request("finish");
        let fixture = case_fixture(relation_id, "Ada", 49_999, 50_000);
        let case_id = fixture.case_id;
        let question = closed_question(relation_id, admission_id, question_id, &[fixture]);
        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        builder.insert_target_case(case_id).unwrap();

        assert_eq!(
            builder.clone().finish().unwrap_err(),
            MechanismIncidenceError::TargetFrontierOpen
        );
        builder.seal_selected_target(&question).unwrap();
        assert_eq!(
            builder.clone().finish().unwrap_err(),
            MechanismIncidenceError::TerminalFrontierIncomplete {
                missing: 1,
                unexpected: 0,
            }
        );

        builder
            .record_unavailable(
                case_id,
                &MechanismUnavailableReasonDefinition::from_canonical_reason(
                    b"dynamic tracing unsupported".as_slice(),
                ),
            )
            .unwrap();
        let closed = builder.finish().unwrap();
        assert_eq!(
            closed.counts().unavailable_cases(),
            MechanismCountEvidence::Exact(1)
        );
        assert_eq!(
            closed.counts().distinct_signatures(),
            MechanismCountEvidence::Unknown {
                confirmed_lower_bound: 0,
            }
        );
    }

    #[test]
    fn canonical_root_does_not_depend_on_discovery_or_replay_order() {
        let (relation_id, admission_id, question_id, scope) = request("order");
        let request_id = scope.request_id();
        let first_fixture = case_fixture(relation_id, "First", 10, 11);
        let second_fixture = case_fixture(relation_id, "Second", 20, 21);
        let first = first_fixture.case_id;
        let second = second_fixture.case_id;
        let question = closed_question(
            relation_id,
            admission_id,
            question_id,
            &[first_fixture, second_fixture],
        );
        let first_definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"first-signature".as_slice(),
        );
        let second_definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"second-signature".as_slice(),
        );

        let mut left = MechanismIncidenceCatalogBuilder::new(scope);
        left.insert_target_case(first).unwrap();
        left.insert_target_case(second).unwrap();
        left.record_incidence(first, transition("first"), &first_definition)
            .unwrap();
        left.record_incidence(second, transition("second"), &second_definition)
            .unwrap();
        left.seal_selected_target(&question).unwrap();

        let mut right = MechanismIncidenceCatalogBuilder::new(scope);
        right.insert_target_case(second).unwrap();
        right.insert_target_case(first).unwrap();
        right
            .record_incidence(second, transition("second"), &second_definition)
            .unwrap();
        right
            .record_incidence(first, transition("first"), &first_definition)
            .unwrap();
        right.seal_selected_target(&question).unwrap();

        assert_eq!(left.snapshot(), right.snapshot());
        assert_eq!(left.snapshot().root(), right.snapshot().root());
    }

    #[test]
    fn target_and_terminal_evidence_is_monotone_and_request_scoped() {
        let (relation_id, admission_id, question_id, scope) = request("monotone");
        let request_id = scope.request_id();
        let fixture = case_fixture(relation_id, "Case", 100, 101);
        let case_id = fixture.case_id;
        let question = closed_question(relation_id, admission_id, question_id, &[fixture]);
        let another_case = case(relation_id, "Another", 200, 201);
        let (_, _, _, another_scope) = request("another-request");
        let another_request_id = another_scope.request_id();
        let wrong_definition = MechanismSignatureDefinition::from_canonical_definition(
            another_request_id,
            b"wrong-request".as_slice(),
        );
        let definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"right-request".as_slice(),
        );

        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        assert_eq!(
            builder
                .record_incidence(case_id, transition("unknown"), &definition)
                .unwrap_err(),
            MechanismIncidenceError::UnknownTargetCase { case_id }
        );
        builder.insert_target_case(case_id).unwrap();
        assert_eq!(
            builder
                .record_incidence(case_id, transition("wrong"), &wrong_definition)
                .unwrap_err(),
            MechanismIncidenceError::SignatureRequestMismatch
        );
        builder
            .record_incidence(case_id, transition("right"), &definition)
            .unwrap();
        assert_eq!(
            builder
                .record_unavailable(
                    case_id,
                    &MechanismUnavailableReasonDefinition::from_canonical_reason(
                        b"conflict".as_slice(),
                    ),
                )
                .unwrap_err(),
            MechanismIncidenceError::TerminalConflict { case_id }
        );
        builder.seal_selected_target(&question).unwrap();
        assert_eq!(
            builder.insert_target_case(another_case).unwrap_err(),
            MechanismIncidenceError::TargetAlreadySealed
        );
    }

    #[test]
    fn equal_transition_identity_cannot_be_assigned_to_two_cases() {
        let (relation_id, _, _, scope) = request("transition-injective");
        let request_id = scope.request_id();
        let first = case(relation_id, "First", 1, 2);
        let second = case(relation_id, "Second", 3, 4);
        let shared_transition = transition("shared");
        let definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"shared-signature".as_slice(),
        );
        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        builder.insert_target_case(first).unwrap();
        builder.insert_target_case(second).unwrap();
        builder
            .record_incidence(first, shared_transition, &definition)
            .unwrap();
        assert_eq!(
            builder
                .record_incidence(second, shared_transition, &definition)
                .unwrap_err(),
            MechanismIncidenceError::TransitionAssignedToMultipleCases {
                transition_id: shared_transition,
                first_case_id: first,
                second_case_id: second,
            }
        );
    }

    #[test]
    fn selected_target_seal_rejects_incomplete_prefix_and_restore_rechecks_case_root() {
        let (relation_id, admission_id, question_id, scope) = request("selected-seal");
        let first = case_fixture(relation_id, "First", 10, 11);
        let second = case_fixture(relation_id, "Second", 20, 21);
        let question = closed_question(
            relation_id,
            admission_id,
            question_id,
            &[first.clone(), second.clone()],
        );
        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        builder.insert_target_case(first.case_id).unwrap();
        assert_eq!(
            builder.seal_selected_target(&question).unwrap_err(),
            MechanismIncidenceError::TargetCaseSetMismatch {
                missing: 1,
                unexpected: 0,
            }
        );

        builder.insert_target_case(second.case_id).unwrap();
        assert!(builder.seal_selected_target(&question).unwrap());
        assert!(!builder.seal_selected_target(&question).unwrap());
        let snapshot = builder.snapshot();
        snapshot.validate().unwrap();
        let restored =
            MechanismIncidenceCatalogBuilder::from_snapshot(snapshot.clone(), scope).unwrap();
        assert_eq!(restored.snapshot(), snapshot);

        let mut tampered = snapshot;
        tampered.target_cases = vec![first.case_id].into_boxed_slice();
        assert_eq!(
            tampered.validate().unwrap_err(),
            MechanismIncidenceError::TargetSealCaseSetMismatch
        );
    }

    #[test]
    fn choice_seal_binds_independent_content_root_and_exact_member_count() {
        let (relation_id, _, question_id, _) = request("chosen-seal");
        let lower = case(relation_id, "Lower", 10, 11);
        let choice_id = ChoiceId::from_canonical_choice_preimage(question_id, b"chosen");
        let content_root = ChoiceContentRoot::from_journal_codec_bytes([0x55; 32]);
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Choice(choice_id),
            b"assess-policy",
            b"dynamic-control-v1",
        );
        let scope = MechanismRequestScope::new(
            request_id,
            question_id,
            MechanismTargetId::Choice(choice_id),
        );
        let mut builder = MechanismIncidenceCatalogBuilder::new(scope);
        builder.insert_target_case(lower).unwrap();
        assert!(builder
            .seal_choice_target_commitment(choice_id, content_root, 1)
            .unwrap());
        let seal = builder.target_seal().unwrap();
        assert_eq!(seal.target_case_count(), 1);
        assert_eq!(seal.scope(), scope);
        assert_eq!(
            seal.upstream(),
            MechanismTargetSealUpstream::Choice {
                choice_id,
                content_root,
            }
        );
        seal.validate_identity().unwrap();

        let mismatched_choice_id =
            ChoiceId::from_canonical_choice_preimage(question_id, b"mismatched");
        let mismatched_request = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Choice(mismatched_choice_id),
            b"assess-policy",
            b"dynamic-control-v1",
        );
        let mut mismatched_builder =
            MechanismIncidenceCatalogBuilder::new(MechanismRequestScope::new(
                mismatched_request,
                question_id,
                MechanismTargetId::Choice(mismatched_choice_id),
            ));
        mismatched_builder.insert_target_case(lower).unwrap();
        assert_eq!(
            mismatched_builder
                .seal_choice_target_commitment(mismatched_choice_id, content_root, 2)
                .unwrap_err(),
            MechanismIncidenceError::TargetSealCaseSetMismatch
        );
    }

    #[test]
    fn signature_identity_is_content_and_request_scoped() {
        let (_, _, _, first_scope) = request("signature-first");
        let (_, _, _, second_scope) = request("signature-second");
        let first_request = first_scope.request_id();
        let second_request = second_scope.request_id();
        let first = MechanismSignatureDefinition::from_canonical_definition(
            first_request,
            b"signature-a".as_slice(),
        );
        let changed = MechanismSignatureDefinition::from_canonical_definition(
            first_request,
            b"signature-b".as_slice(),
        );
        let other_request = MechanismSignatureDefinition::from_canonical_definition(
            second_request,
            b"signature-a".as_slice(),
        );

        assert_ne!(first.id(), changed.id());
        assert_ne!(first.id(), other_request.id());
        assert_eq!(first.id().request_id(), first_request);
    }
}
