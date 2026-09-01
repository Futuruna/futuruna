//! Structural mechanism quotients derived from complete relational replay.
//!
//! A raw [`MechanismSignatureId`] remains the exact, request-scoped replay
//! authority.  This module derives a separately versioned structural DAG from
//! an already validated Before/After occurrence union. Dynamic invocation and
//! visit multiplicity is retained in an exact execution profile instead of
//! becoming structural identity.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::authenticated_treap::{AuthenticatedTreapMap, AuthenticatedTreapValue};
use super::mechanism_incidence::MechanismSignatureId;
use super::relation::MechanismRequestId;
use super::relational_mechanism_executor::{
    RelationalIfDecisionOutcome, RelationalMechanismActivationStep, RelationalMechanismCalleeId,
    RelationalMechanismEndpoint, RelationalMechanismEventKind, RelationalMechanismEventOutcome,
    RelationalMechanismSiteId, RelationalMechanismSiteKind, RelationalRuleAttemptOutcome,
    RelationalRuleSelectionOutcome, RelationalShortCircuitOutcome,
};

pub(crate) const STRUCTURAL_MECHANISM_QUOTIENT_VERSION: u32 = 1;
pub(crate) const STRUCTURAL_QUOTIENT_CLOSURE_VERSION: u32 = 1;
pub(crate) const STRUCTURAL_DEFINITION_CATALOG_VERSION: u32 = 1;

/// Maximum authenticated raw signature accepted by the V1 structural
/// producer. Personskat's recorded one-case signature is about 30.1 MB, so
/// this admits it with more than twofold byte headroom without inheriting the
/// generic 512-MiB raw-artifact envelope.
pub(crate) const RELATIONAL_STRUCTURAL_SOURCE_MAX_BYTES: usize = 64 << 20;
/// Maximum deterministic logical work admitted for one structural derivation.
///
/// The recorded Personskat calibration consumes 612,550,656 units from
/// 33,198 activation occurrences, 105,718 endpoint occurrences and 108,922
/// edges. The 1-Gi-unit lane therefore leaves 461,191,168 units of headroom.
pub(crate) const RELATIONAL_STRUCTURAL_DERIVATION_MAX_WORK_UNITS: usize = 1 << 30;
/// Exact ceiling for the canonical structural assignment payload. This lane
/// is independent of source size and logical derivation work. The calibrated
/// Personskat shape has a conservative fixed-width encoding envelope of
/// 119,837,126 bytes even when Before/After occurrences are assumed disjoint.
pub(crate) const RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES: usize = 128 << 20;

// These fixed, dimensionless charges deliberately over-account the transient
// collections used by relational pairing and quotient derivation. They are a
// deterministic work-admission policy evaluated before the corresponding
// shape allocations, not a post-allocation memory sample. Variable-width raw
// material is bounded separately by the source lane above.
const STRUCTURAL_DERIVATION_BASE_WORK_UNITS: usize = 4 << 10;
const STRUCTURAL_DERIVATION_ACTIVATION_WORK_UNITS: usize = 2 << 10;
const STRUCTURAL_DERIVATION_OCCURRENCE_WORK_UNITS: usize = 4 << 10;
const STRUCTURAL_DERIVATION_EDGE_WORK_UNITS: usize = 1 << 10;

/// Explicit fail-closed budget shared by raw-signature decoding, structural
/// work admission, and final canonical artifact encoding.
///
/// Source bytes, logical work and encoded output are independent lanes. The
/// work lane is consumed before the corresponding quotient collections are
/// built. The payload lane is an exact byte ceiling enforced while encoding,
/// so a producer cannot transiently construct an oversized canonical vector
/// and reject it only afterward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StructuralDerivationBudget {
    source_limit_bytes: usize,
    work_limit_units: usize,
    payload_limit_bytes: usize,
    remaining_work_units: usize,
    source_admitted: bool,
    shape_admitted: bool,
    admitted_activations: usize,
    admitted_occurrences: usize,
    admitted_edges: usize,
}

impl StructuralDerivationBudget {
    pub(crate) const fn new(
        source_limit_bytes: usize,
        work_limit_units: usize,
        payload_limit_bytes: usize,
    ) -> Self {
        Self {
            source_limit_bytes,
            work_limit_units,
            payload_limit_bytes,
            remaining_work_units: work_limit_units,
            source_admitted: false,
            shape_admitted: false,
            admitted_activations: 0,
            admitted_occurrences: 0,
            admitted_edges: 0,
        }
    }

    pub(super) fn admit_source(
        &mut self,
        canonical_definition_bytes: usize,
    ) -> Result<(), StructuralMechanismError> {
        if self.source_admitted || self.shape_admitted {
            return Err(StructuralMechanismError::Conservation(
                "structural derivation budget source admission",
            ));
        }
        if canonical_definition_bytes > self.source_limit_bytes {
            return Err(StructuralMechanismError::SourcePayloadBudgetExceeded {
                actual: canonical_definition_bytes,
                limit: self.source_limit_bytes,
            });
        }
        self.charge(STRUCTURAL_DERIVATION_BASE_WORK_UNITS)?;
        self.source_admitted = true;
        Ok(())
    }

    pub(super) fn admit_activations(
        &mut self,
        activation_count: usize,
    ) -> Result<(), StructuralMechanismError> {
        self.require_open_shape_admission()?;
        let admitted = self
            .admitted_activations
            .checked_add(activation_count)
            .ok_or(StructuralMechanismError::DerivationWorkBudgetExceeded {
                minimum_required: usize::MAX,
                limit: self.work_limit_units,
            })?;
        self.charge_scaled(
            activation_count,
            STRUCTURAL_DERIVATION_ACTIVATION_WORK_UNITS,
        )?;
        self.admitted_activations = admitted;
        Ok(())
    }

    pub(super) fn admit_occurrences(
        &mut self,
        occurrence_count: usize,
    ) -> Result<(), StructuralMechanismError> {
        self.require_open_shape_admission()?;
        let admitted = self
            .admitted_occurrences
            .checked_add(occurrence_count)
            .ok_or(StructuralMechanismError::DerivationWorkBudgetExceeded {
                minimum_required: usize::MAX,
                limit: self.work_limit_units,
            })?;
        self.charge_scaled(
            occurrence_count,
            STRUCTURAL_DERIVATION_OCCURRENCE_WORK_UNITS,
        )?;
        self.admitted_occurrences = admitted;
        Ok(())
    }

    pub(super) fn admit_edges(
        &mut self,
        edge_count: usize,
    ) -> Result<(), StructuralMechanismError> {
        self.require_open_shape_admission()?;
        let admitted = self.admitted_edges.checked_add(edge_count).ok_or(
            StructuralMechanismError::DerivationWorkBudgetExceeded {
                minimum_required: usize::MAX,
                limit: self.work_limit_units,
            },
        )?;
        self.charge_scaled(edge_count, STRUCTURAL_DERIVATION_EDGE_WORK_UNITS)?;
        self.admitted_edges = admitted;
        Ok(())
    }

    pub(super) fn finish_shape_admission(&mut self) -> Result<(), StructuralMechanismError> {
        self.require_open_shape_admission()?;
        self.shape_admitted = true;
        Ok(())
    }

    fn require_open_shape_admission(self) -> Result<(), StructuralMechanismError> {
        if !self.source_admitted || self.shape_admitted {
            return Err(StructuralMechanismError::Conservation(
                "structural derivation budget shape admission",
            ));
        }
        Ok(())
    }

    fn require_shape_admitted(
        self,
        activation_count: usize,
        occurrence_count: usize,
        edge_count: usize,
    ) -> Result<(), StructuralMechanismError> {
        if !self.shape_admitted
            || self.admitted_activations != activation_count
            || self.admitted_occurrences != occurrence_count
            || self.admitted_edges != edge_count
        {
            return Err(StructuralMechanismError::Conservation(
                "structural derivation budget input binding",
            ));
        }
        Ok(())
    }

    const fn payload_limit(self) -> usize {
        self.payload_limit_bytes
    }

    fn charge_scaled(
        &mut self,
        count: usize,
        units_per_item: usize,
    ) -> Result<(), StructuralMechanismError> {
        let units = count.checked_mul(units_per_item).ok_or(
            StructuralMechanismError::DerivationWorkBudgetExceeded {
                minimum_required: usize::MAX,
                limit: self.work_limit_units,
            },
        )?;
        self.charge(units)
    }

    fn charge(&mut self, units: usize) -> Result<(), StructuralMechanismError> {
        let consumed = self.work_limit_units - self.remaining_work_units;
        let minimum_required = consumed.checked_add(units).unwrap_or(usize::MAX);
        self.remaining_work_units = self.remaining_work_units.checked_sub(units).ok_or(
            StructuralMechanismError::DerivationWorkBudgetExceeded {
                minimum_required,
                limit: self.work_limit_units,
            },
        )?;
        Ok(())
    }
}

/// The one policy constructor used by both the live producer and journal
/// rederivation. Keeping it here prevents a resume boundary from silently
/// changing any of the three independent admission lanes.
pub(crate) const fn relational_structural_derivation_budget() -> StructuralDerivationBudget {
    StructuralDerivationBudget::new(
        RELATIONAL_STRUCTURAL_SOURCE_MAX_BYTES,
        RELATIONAL_STRUCTURAL_DERIVATION_MAX_WORK_UNITS,
        RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES,
    )
}

const FRAME_ID_V1: &[u8] = b"futuruna.explore.structural-frame-id.v1";
const ACTIVATION_CONTEXT_ID_V1: &[u8] = b"futuruna.explore.structural-activation-context-id.v1";
const NODE_ID_V1: &[u8] = b"futuruna.explore.structural-node-id.v1";
const EDGE_ID_V1: &[u8] = b"futuruna.explore.structural-edge-id.v1";
const MECHANISM_ID_V1: &[u8] = b"futuruna.explore.structural-mechanism-id.v1";
const PROFILE_ID_V1: &[u8] = b"futuruna.explore.structural-execution-profile-id.v1";
const ARTIFACT_V1: &[u8] = b"futuruna.explore.structural-signature-quotient-artifact.v1";
const MEMBERSHIP_ROOT_V1: &[u8] = b"futuruna.explore.structural-membership-root.v1";
const CATALOG_REVISION_V1: &[u8] = b"futuruna.explore.structural-catalog-revision.v1";
const ASSIGNMENT_INDEX_V1: &[u8] = b"futuruna.explore.structural-assignment-index.v1";
const ASSIGNMENT_VALUE_V1: &[u8] = b"futuruna.explore.structural-assignment-value.v1";
const EXPECTED_SIGNATURE_SET_ROOT_V1: &[u8] =
    b"futuruna.explore.structural-expected-signature-set-root.v1";
const SIGNATURE_TO_QUOTIENT_ROOT_V1: &[u8] =
    b"futuruna.explore.structural-signature-to-quotient-root.v1";
const CATALOG_MEMBERSHIP_ROOT_V1: &[u8] = b"futuruna.explore.structural-catalog-membership-root.v1";
const QUOTIENT_CLOSURE_ROOT_V1: &[u8] = b"futuruna.explore.structural-quotient-closure-root.v1";
const DEFINITION_CATALOG_ROOT_V1: &[u8] = b"futuruna.explore.structural-definition-catalog-root.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralFrameId([u8; 32]);

impl StructuralFrameId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralActivationContextId([u8; 32]);

impl StructuralActivationContextId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralNodeId([u8; 32]);

impl StructuralNodeId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralEdgeId([u8; 32]);

impl StructuralEdgeId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralMechanismId([u8; 32]);

impl StructuralMechanismId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExecutionProfileId([u8; 32]);

impl ExecutionProfileId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralMembershipRoot([u8; 32]);

impl StructuralMembershipRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Branch-sensitive operational revision used to invalidate derived caches.
/// This hash chain is deliberately not the canonical structural closure root:
/// equal assignment sets discovered in different orders may have different
/// revisions while still closing to the same eventual semantic set root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralCatalogRevision([u8; 32]);

impl StructuralCatalogRevision {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of the exact raw-signature set against which the
/// structural quotient was closed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralExpectedSignatureSetRoot([u8; 32]);

impl StructuralExpectedSignatureSetRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical raw-signature -> (structural mechanism, execution profile)
/// assignment root. Per-signature occurrence membership is committed by the
/// separate catalog-membership root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralSignatureToQuotientRoot([u8; 32]);

impl StructuralSignatureToQuotientRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical raw-signature -> structural-membership commitment root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralCatalogMembershipRoot([u8; 32]);

impl StructuralCatalogMembershipRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of one exact, request-local structural quotient closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralQuotientClosureRoot([u8; 32]);

impl StructuralQuotientClosureRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of the closure-frozen, normalized structural definition
/// catalog. The root commits only definition identities in their fixed
/// section order; those identities already commit the collision-checked
/// definition preimages. Raw signatures, cases, values, and occurrence
/// membership are deliberately outside this projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralDefinitionCatalogRoot([u8; 32]);

impl StructuralDefinitionCatalogRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Fixed taxonomy for the normalized structural-definition catalog. Every
/// section is ordered by its typed content ID, not by discovery or execution
/// order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum StructuralDefinitionKind {
    Frame,
    ActivationContext,
    Node,
    Edge,
    Mechanism,
    ExecutionProfile,
}

impl StructuralDefinitionKind {
    pub(crate) const CANONICAL_ORDER: [Self; 6] = [
        Self::Frame,
        Self::ActivationContext,
        Self::Node,
        Self::Edge,
        Self::Mechanism,
        Self::ExecutionProfile,
    ];

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Frame => 0x01,
            Self::ActivationContext => 0x02,
            Self::Node => 0x03,
            Self::Edge => 0x04,
            Self::Mechanism => 0x05,
            Self::ExecutionProfile => 0x06,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::ActivationContext => "activation_context",
            Self::Node => "node",
            Self::Edge => "edge",
            Self::Mechanism => "mechanism",
            Self::ExecutionProfile => "execution_profile",
        }
    }
}

/// Exact cardinalities committed by structural closure. These are definition
/// and assignment counts only: no node x case expansion is materialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StructuralQuotientCounts {
    assignments: u128,
    frames: u128,
    activation_contexts: u128,
    nodes: u128,
    edges: u128,
    mechanisms: u128,
    execution_profiles: u128,
}

impl StructuralQuotientCounts {
    pub(super) const fn from_journal_codec_parts(
        assignments: u128,
        frames: u128,
        activation_contexts: u128,
        nodes: u128,
        edges: u128,
        mechanisms: u128,
        execution_profiles: u128,
    ) -> Self {
        Self {
            assignments,
            frames,
            activation_contexts,
            nodes,
            edges,
            mechanisms,
            execution_profiles,
        }
    }

    pub(crate) const fn assignments(self) -> u128 {
        self.assignments
    }

    pub(crate) const fn frames(self) -> u128 {
        self.frames
    }

    pub(crate) const fn activation_contexts(self) -> u128 {
        self.activation_contexts
    }

    pub(crate) const fn nodes(self) -> u128 {
        self.nodes
    }

    pub(crate) const fn edges(self) -> u128 {
        self.edges
    }

    pub(crate) const fn mechanisms(self) -> u128 {
        self.mechanisms
    }

    pub(crate) const fn execution_profiles(self) -> u128 {
        self.execution_profiles
    }
}

/// Typed receipt proving exact assignment coverage for one request-local
/// structural quotient catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StructuralQuotientClosureReceipt {
    closure_version: u32,
    quotient_version: u32,
    request_id: MechanismRequestId,
    expected_signature_count: u128,
    expected_signature_set_root: StructuralExpectedSignatureSetRoot,
    signature_to_quotient_root: StructuralSignatureToQuotientRoot,
    catalog_membership_root: StructuralCatalogMembershipRoot,
    counts: StructuralQuotientCounts,
    root: StructuralQuotientClosureRoot,
}

impl StructuralQuotientClosureReceipt {
    pub(crate) const fn closure_version(self) -> u32 {
        self.closure_version
    }

    pub(crate) const fn quotient_version(self) -> u32 {
        self.quotient_version
    }

    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn expected_signature_count(self) -> u128 {
        self.expected_signature_count
    }

    pub(crate) const fn expected_signature_set_root(self) -> StructuralExpectedSignatureSetRoot {
        self.expected_signature_set_root
    }

    pub(crate) const fn signature_to_quotient_root(self) -> StructuralSignatureToQuotientRoot {
        self.signature_to_quotient_root
    }

    pub(crate) const fn catalog_membership_root(self) -> StructuralCatalogMembershipRoot {
        self.catalog_membership_root
    }

    pub(crate) const fn counts(self) -> StructuralQuotientCounts {
        self.counts
    }

    pub(crate) const fn root(self) -> StructuralQuotientClosureRoot {
        self.root
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralFrameDefinition {
    id: StructuralFrameId,
    call_site: RelationalMechanismSiteId,
    callee: RelationalMechanismCalleeId,
}

impl StructuralFrameDefinition {
    fn from_step(step: &RelationalMechanismActivationStep) -> Self {
        let mut encoder = CanonicalEncoder::new(FRAME_ID_V1);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encode_site(&mut encoder, step.call_site());
        encode_callee(&mut encoder, step.callee());
        let id = StructuralFrameId(encoder.digest());
        Self {
            id,
            call_site: step.call_site().clone(),
            callee: step.callee().clone(),
        }
    }

    pub(crate) const fn id(&self) -> StructuralFrameId {
        self.id
    }

    pub(crate) const fn call_site(&self) -> &RelationalMechanismSiteId {
        &self.call_site
    }

    pub(crate) const fn callee(&self) -> &RelationalMechanismCalleeId {
        &self.callee
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralActivationContextDefinition {
    id: StructuralActivationContextId,
    parent: Option<StructuralActivationContextId>,
    frame: StructuralFrameId,
}

impl StructuralActivationContextDefinition {
    fn new(parent: Option<StructuralActivationContextId>, frame: StructuralFrameId) -> Self {
        let mut encoder = CanonicalEncoder::new(ACTIVATION_CONTEXT_ID_V1);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        match parent {
            Some(parent) => {
                encoder.u8(0x01);
                encoder.digest_bytes(parent.bytes());
            }
            None => encoder.u8(0x00),
        }
        encoder.digest_bytes(frame.bytes());
        Self {
            id: StructuralActivationContextId(encoder.digest()),
            parent,
            frame,
        }
    }

    pub(crate) const fn id(self) -> StructuralActivationContextId {
        self.id
    }

    pub(crate) const fn parent(self) -> Option<StructuralActivationContextId> {
        self.parent
    }

    pub(crate) const fn frame(self) -> StructuralFrameId {
        self.frame
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralNodeDefinition {
    id: StructuralNodeId,
    owner_frame: StructuralFrameId,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    before_outcome: Option<RelationalMechanismEventOutcome>,
    after_outcome: Option<RelationalMechanismEventOutcome>,
    before_dependencies: Box<[StructuralNodeId]>,
    after_dependencies: Box<[StructuralNodeId]>,
}

impl StructuralNodeDefinition {
    pub(crate) const fn id(&self) -> StructuralNodeId {
        self.id
    }

    pub(crate) const fn owner_frame(&self) -> StructuralFrameId {
        self.owner_frame
    }

    pub(crate) const fn site(&self) -> &RelationalMechanismSiteId {
        &self.site
    }

    pub(crate) const fn kind(&self) -> RelationalMechanismEventKind {
        self.kind
    }

    pub(crate) const fn before_outcome(&self) -> Option<&RelationalMechanismEventOutcome> {
        self.before_outcome.as_ref()
    }

    pub(crate) const fn after_outcome(&self) -> Option<&RelationalMechanismEventOutcome> {
        self.after_outcome.as_ref()
    }

    pub(crate) fn before_dependencies(&self) -> &[StructuralNodeId] {
        &self.before_dependencies
    }

    pub(crate) fn after_dependencies(&self) -> &[StructuralNodeId] {
        &self.after_dependencies
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralEdgeDefinition {
    id: StructuralEdgeId,
    endpoint: RelationalMechanismEndpoint,
    dependent: StructuralNodeId,
    dependency: StructuralNodeId,
}

impl StructuralEdgeDefinition {
    pub(crate) const fn id(&self) -> StructuralEdgeId {
        self.id
    }

    pub(crate) const fn endpoint(&self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn dependent(&self) -> StructuralNodeId {
        self.dependent
    }

    pub(crate) const fn dependency(&self) -> StructuralNodeId {
        self.dependency
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralEndpointContext {
    endpoint: RelationalMechanismEndpoint,
    context: StructuralActivationContextId,
}

impl StructuralEndpointContext {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn context(self) -> StructuralActivationContextId {
        self.context
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralNodeOwnership {
    endpoint: RelationalMechanismEndpoint,
    node: StructuralNodeId,
    context: StructuralActivationContextId,
}

impl StructuralNodeOwnership {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn node(self) -> StructuralNodeId {
        self.node
    }

    pub(crate) const fn context(self) -> StructuralActivationContextId {
        self.context
    }
}

/// Multiplicity-free, collision-checkable structural topology.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralMechanismDefinition {
    id: StructuralMechanismId,
    frames: Box<[StructuralFrameDefinition]>,
    activation_contexts: Box<[StructuralActivationContextDefinition]>,
    nodes: Box<[StructuralNodeDefinition]>,
    edges: Box<[StructuralEdgeDefinition]>,
    context_inventory: Box<[StructuralEndpointContext]>,
    before_roots: Box<[StructuralNodeId]>,
    after_roots: Box<[StructuralNodeId]>,
    ownership: Box<[StructuralNodeOwnership]>,
}

impl StructuralMechanismDefinition {
    pub(crate) const fn id(&self) -> StructuralMechanismId {
        self.id
    }

    pub(crate) fn nodes(&self) -> &[StructuralNodeDefinition] {
        &self.nodes
    }

    pub(crate) fn frames(&self) -> &[StructuralFrameDefinition] {
        &self.frames
    }

    pub(crate) fn activation_contexts(&self) -> &[StructuralActivationContextDefinition] {
        &self.activation_contexts
    }

    pub(crate) fn edges(&self) -> &[StructuralEdgeDefinition] {
        &self.edges
    }

    pub(crate) fn context_inventory(&self) -> &[StructuralEndpointContext] {
        &self.context_inventory
    }

    pub(crate) fn before_roots(&self) -> &[StructuralNodeId] {
        &self.before_roots
    }

    pub(crate) fn after_roots(&self) -> &[StructuralNodeId] {
        &self.after_roots
    }

    pub(crate) fn ownership(&self) -> &[StructuralNodeOwnership] {
        &self.ownership
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralFrameCount {
    endpoint: RelationalMechanismEndpoint,
    frame: StructuralFrameId,
    count: u128,
}

impl StructuralFrameCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn frame(self) -> StructuralFrameId {
        self.frame
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralActivationCallCount {
    endpoint: RelationalMechanismEndpoint,
    parent: StructuralActivationContextId,
    child: StructuralActivationContextId,
    count: u128,
}

impl StructuralActivationCallCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn parent(self) -> StructuralActivationContextId {
        self.parent
    }

    pub(crate) const fn child(self) -> StructuralActivationContextId {
        self.child
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralContextCount {
    endpoint: RelationalMechanismEndpoint,
    context: StructuralActivationContextId,
    count: u128,
}

impl StructuralContextCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn context(self) -> StructuralActivationContextId {
        self.context
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralNodeCount {
    endpoint: RelationalMechanismEndpoint,
    node: StructuralNodeId,
    count: u128,
}

impl StructuralNodeCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn node(self) -> StructuralNodeId {
        self.node
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralEdgeCount {
    endpoint: RelationalMechanismEndpoint,
    edge: StructuralEdgeId,
    count: u128,
}

impl StructuralEdgeCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn edge(self) -> StructuralEdgeId {
        self.edge
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralOwnershipCount {
    endpoint: RelationalMechanismEndpoint,
    node: StructuralNodeId,
    context: StructuralActivationContextId,
    count: u128,
}

impl StructuralOwnershipCount {
    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn node(self) -> StructuralNodeId {
        self.node
    }

    pub(crate) const fn context(self) -> StructuralActivationContextId {
        self.context
    }

    pub(crate) const fn count(self) -> u128 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StructuralEndpointExecutionTotals {
    activation_nodes: u128,
    activation_roots: u128,
    activation_edges: u128,
    event_nodes: u128,
    event_roots: u128,
    event_edges: u128,
    ownership_occurrences: u128,
}

impl StructuralEndpointExecutionTotals {
    pub(crate) const fn activation_nodes(self) -> u128 {
        self.activation_nodes
    }

    pub(crate) const fn activation_roots(self) -> u128 {
        self.activation_roots
    }

    pub(crate) const fn activation_edges(self) -> u128 {
        self.activation_edges
    }

    pub(crate) const fn event_nodes(self) -> u128 {
        self.event_nodes
    }

    pub(crate) const fn event_roots(self) -> u128 {
        self.event_roots
    }

    pub(crate) const fn event_edges(self) -> u128 {
        self.event_edges
    }

    pub(crate) const fn ownership_occurrences(self) -> u128 {
        self.ownership_occurrences
    }
}

/// Exact execution multiplicity for one structural mechanism.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralExecutionProfile {
    id: ExecutionProfileId,
    mechanism_id: StructuralMechanismId,
    frames: Box<[StructuralFrameDefinition]>,
    activation_contexts: Box<[StructuralActivationContextDefinition]>,
    frame_counts: Box<[StructuralFrameCount]>,
    context_counts: Box<[StructuralContextCount]>,
    activation_root_counts: Box<[StructuralContextCount]>,
    activation_call_counts: Box<[StructuralActivationCallCount]>,
    node_counts: Box<[StructuralNodeCount]>,
    node_root_counts: Box<[StructuralNodeCount]>,
    edge_counts: Box<[StructuralEdgeCount]>,
    ownership_counts: Box<[StructuralOwnershipCount]>,
    before_totals: StructuralEndpointExecutionTotals,
    after_totals: StructuralEndpointExecutionTotals,
}

impl StructuralExecutionProfile {
    pub(crate) const fn id(&self) -> ExecutionProfileId {
        self.id
    }

    pub(crate) const fn mechanism_id(&self) -> StructuralMechanismId {
        self.mechanism_id
    }

    pub(crate) fn frames(&self) -> &[StructuralFrameDefinition] {
        &self.frames
    }

    pub(crate) fn activation_contexts(&self) -> &[StructuralActivationContextDefinition] {
        &self.activation_contexts
    }

    pub(crate) fn frame_counts(&self) -> &[StructuralFrameCount] {
        &self.frame_counts
    }

    pub(crate) fn context_counts(&self) -> &[StructuralContextCount] {
        &self.context_counts
    }

    pub(crate) fn activation_root_counts(&self) -> &[StructuralContextCount] {
        &self.activation_root_counts
    }

    pub(crate) fn activation_call_counts(&self) -> &[StructuralActivationCallCount] {
        &self.activation_call_counts
    }

    pub(crate) fn node_counts(&self) -> &[StructuralNodeCount] {
        &self.node_counts
    }

    pub(crate) fn node_root_counts(&self) -> &[StructuralNodeCount] {
        &self.node_root_counts
    }

    pub(crate) fn edge_counts(&self) -> &[StructuralEdgeCount] {
        &self.edge_counts
    }

    pub(crate) fn ownership_counts(&self) -> &[StructuralOwnershipCount] {
        &self.ownership_counts
    }

    pub(crate) const fn before_totals(&self) -> StructuralEndpointExecutionTotals {
        self.before_totals
    }

    pub(crate) const fn after_totals(&self) -> StructuralEndpointExecutionTotals {
        self.after_totals
    }
}

/// Borrowed entry in the closure-frozen normalized definition catalog. This
/// view never constructs a canonical preimage or exposes a raw signature,
/// occurrence membership, case, or value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralDefinitionRef<'catalog> {
    Frame(&'catalog StructuralFrameDefinition),
    ActivationContext(&'catalog StructuralActivationContextDefinition),
    Node(&'catalog StructuralNodeDefinition),
    Edge(&'catalog StructuralEdgeDefinition),
    Mechanism(&'catalog StructuralMechanismDefinition),
    ExecutionProfile(&'catalog StructuralExecutionProfile),
}

impl StructuralDefinitionRef<'_> {
    pub(crate) const fn kind(self) -> StructuralDefinitionKind {
        match self {
            Self::Frame(_) => StructuralDefinitionKind::Frame,
            Self::ActivationContext(_) => StructuralDefinitionKind::ActivationContext,
            Self::Node(_) => StructuralDefinitionKind::Node,
            Self::Edge(_) => StructuralDefinitionKind::Edge,
            Self::Mechanism(_) => StructuralDefinitionKind::Mechanism,
            Self::ExecutionProfile(_) => StructuralDefinitionKind::ExecutionProfile,
        }
    }

    pub(crate) const fn id_bytes(self) -> [u8; 32] {
        match self {
            Self::Frame(definition) => definition.id().bytes(),
            Self::ActivationContext(definition) => definition.id().bytes(),
            Self::Node(definition) => definition.id().bytes(),
            Self::Edge(definition) => definition.id().bytes(),
            Self::Mechanism(definition) => definition.id().bytes(),
            Self::ExecutionProfile(definition) => definition.id().bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralOccurrenceMembership {
    raw_union_ordinal: u32,
    node: StructuralNodeId,
    owner_frame: StructuralFrameId,
    owner_context: StructuralActivationContextId,
    before_present: bool,
    after_present: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralActivationMembership {
    endpoint: RelationalMechanismEndpoint,
    raw_activation_ordinal: u32,
    context: StructuralActivationContextId,
    frame: StructuralFrameId,
    parent_raw_activation_ordinal: Option<u32>,
    parent_context: Option<StructuralActivationContextId>,
    invocation_ordinal: u32,
}

/// One deterministic raw-signature assignment. The canonical payload is an
/// opaque durable artifact: journal restore rederives it from the already
/// interned V3 signature and compares bytes before accepting it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralSignatureQuotientArtifact {
    signature_id: MechanismSignatureId,
    mechanism: StructuralMechanismDefinition,
    profile: StructuralExecutionProfile,
    activation_membership: Box<[StructuralActivationMembership]>,
    membership: Box<[StructuralOccurrenceMembership]>,
    node_membership: Box<[StructuralNodeId]>,
    edge_membership: Box<[StructuralEdgeId]>,
    differential_node_membership: Box<[StructuralNodeId]>,
    differential_edge_membership: Box<[StructuralEdgeId]>,
    canonical_payload: Box<[u8]>,
}

impl StructuralSignatureQuotientArtifact {
    pub(crate) const fn signature_id(&self) -> MechanismSignatureId {
        self.signature_id
    }

    pub(crate) const fn mechanism(&self) -> &StructuralMechanismDefinition {
        &self.mechanism
    }

    pub(crate) const fn profile(&self) -> &StructuralExecutionProfile {
        &self.profile
    }

    pub(crate) fn node_membership(&self) -> &[StructuralNodeId] {
        &self.node_membership
    }

    pub(crate) fn edge_membership(&self) -> &[StructuralEdgeId] {
        &self.edge_membership
    }

    pub(crate) fn differential_node_membership(&self) -> &[StructuralNodeId] {
        &self.differential_node_membership
    }

    pub(crate) fn differential_edge_membership(&self) -> &[StructuralEdgeId] {
        &self.differential_edge_membership
    }

    pub(crate) fn canonical_payload(&self) -> &[u8] {
        &self.canonical_payload
    }

    fn membership_root(&self) -> StructuralMembershipRoot {
        let mut encoder = CanonicalEncoder::new(MEMBERSHIP_ROOT_V1);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encoder.digest_bytes(self.signature_id.request_id().bytes());
        encoder.digest_bytes(self.signature_id.bytes());
        encode_activation_membership(&mut encoder, &self.activation_membership);
        encode_occurrence_membership(&mut encoder, &self.membership);
        encode_ids(&mut encoder, &self.node_membership, |id| id.bytes());
        encode_ids(&mut encoder, &self.edge_membership, |id| id.bytes());
        encode_ids(&mut encoder, &self.differential_node_membership, |id| {
            id.bytes()
        });
        encode_ids(&mut encoder, &self.differential_edge_membership, |id| {
            id.bytes()
        });
        StructuralMembershipRoot(encoder.digest())
    }

    fn validate_internal_identity(&self) -> Result<(), StructuralMechanismError> {
        let canonical = encode_artifact(self, self.canonical_payload.len())?;
        if derive_mechanism_id(&self.mechanism) != self.mechanism.id
            || derive_profile_id(&self.profile) != self.profile.id
            || self.profile.mechanism_id != self.mechanism.id
            || canonical.as_slice() != self.canonical_payload.as_ref()
        {
            return Err(StructuralMechanismError::Conservation(
                "structural artifact identity",
            ));
        }
        validate_definition_references(&self.mechanism, &self.profile)?;
        let definition_nodes = set_box(self.mechanism.nodes.iter().map(|node| node.id));
        let definition_edges = set_box(self.mechanism.edges.iter().map(|edge| edge.id));
        if definition_nodes != self.node_membership || definition_edges != self.edge_membership {
            return Err(StructuralMechanismError::Conservation(
                "structural definition membership",
            ));
        }
        let (differential_nodes, differential_edges) =
            derive_differential_membership(&self.mechanism);
        if differential_nodes != self.differential_node_membership
            || differential_edges != self.differential_edge_membership
        {
            return Err(StructuralMechanismError::Conservation(
                "differential structural membership",
            ));
        }
        if self.membership.iter().enumerate().any(|(ordinal, row)| {
            usize::try_from(row.raw_union_ordinal) != Ok(ordinal)
                || self.node_membership.binary_search(&row.node).is_err()
        }) {
            return Err(StructuralMechanismError::Conservation(
                "occurrence membership partition",
            ));
        }
        validate_profile_conservation(&self.profile)?;
        let before_activations = self
            .activation_membership
            .iter()
            .filter(|row| row.endpoint == RelationalMechanismEndpoint::Before)
            .count();
        let after_activations = self
            .activation_membership
            .iter()
            .filter(|row| row.endpoint == RelationalMechanismEndpoint::After)
            .count();
        validate_activation_membership(
            &self.activation_membership,
            before_activations,
            after_activations,
        )?;
        let profile_frames: BTreeSet<_> = self.profile.frames.iter().map(|row| row.id).collect();
        let profile_contexts: BTreeSet<_> = self
            .profile
            .activation_contexts
            .iter()
            .map(|row| row.id)
            .collect();
        if self.activation_membership.iter().any(|row| {
            !profile_frames.contains(&row.frame) || !profile_contexts.contains(&row.context)
        }) {
            return Err(StructuralMechanismError::Conservation(
                "activation membership definition",
            ));
        }
        for row in &self.membership {
            let node = self
                .mechanism
                .nodes
                .binary_search_by_key(&row.node, |definition| definition.id)
                .ok()
                .map(|index| &self.mechanism.nodes[index])
                .ok_or(StructuralMechanismError::Conservation(
                    "occurrence membership node",
                ))?;
            if node.owner_frame != row.owner_frame || !profile_contexts.contains(&row.owner_context)
            {
                return Err(StructuralMechanismError::Conservation(
                    "occurrence owner membership",
                ));
            }
            for (endpoint, present) in [
                (RelationalMechanismEndpoint::Before, row.before_present),
                (RelationalMechanismEndpoint::After, row.after_present),
            ] {
                if present
                    != self
                        .mechanism
                        .ownership
                        .binary_search(&StructuralNodeOwnership {
                            endpoint,
                            node: row.node,
                            context: row.owner_context,
                        })
                        .is_ok()
                {
                    return Err(StructuralMechanismError::Conservation(
                        "endpoint ownership membership",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_definition_references(
    mechanism: &StructuralMechanismDefinition,
    profile: &StructuralExecutionProfile,
) -> Result<(), StructuralMechanismError> {
    let profile_frames: BTreeMap<_, _> = profile
        .frames
        .iter()
        .map(|definition| (definition.id, definition))
        .collect();
    let profile_contexts: BTreeMap<_, _> = profile
        .activation_contexts
        .iter()
        .map(|definition| (definition.id, definition))
        .collect();
    if profile_frames.len() != profile.frames.len()
        || profile_contexts.len() != profile.activation_contexts.len()
    {
        return Err(StructuralMechanismError::Conservation(
            "duplicate activation definition",
        ));
    }
    for definition in &profile.activation_contexts {
        if StructuralActivationContextDefinition::new(definition.parent, definition.frame)
            != *definition
            || !profile_frames.contains_key(&definition.frame)
            || definition
                .parent
                .is_some_and(|parent| !profile_contexts.contains_key(&parent))
        {
            return Err(StructuralMechanismError::Conservation(
                "activation context reference",
            ));
        }
    }
    for definition in &mechanism.frames {
        if profile_frames.get(&definition.id).copied() != Some(definition) {
            return Err(StructuralMechanismError::Conservation(
                "mechanism frame reference",
            ));
        }
    }
    for definition in &mechanism.activation_contexts {
        if profile_contexts.get(&definition.id).copied() != Some(definition) {
            return Err(StructuralMechanismError::Conservation(
                "mechanism activation context reference",
            ));
        }
    }
    let mechanism_contexts: BTreeSet<_> = mechanism
        .activation_contexts
        .iter()
        .map(|definition| definition.id)
        .collect();
    let nodes: BTreeSet<_> = mechanism.nodes.iter().map(|node| node.id).collect();
    let edges: BTreeSet<_> = mechanism.edges.iter().map(|edge| edge.id).collect();
    if nodes.len() != mechanism.nodes.len() || edges.len() != mechanism.edges.len() {
        return Err(StructuralMechanismError::Conservation(
            "duplicate structural definition",
        ));
    }
    for node in &mechanism.nodes {
        if derive_node_id(node) != node.id
            || !profile_frames.contains_key(&node.owner_frame)
            || node
                .before_dependencies
                .iter()
                .chain(node.after_dependencies.iter())
                .any(|dependency| !nodes.contains(dependency))
        {
            return Err(StructuralMechanismError::Conservation(
                "structural node reference",
            ));
        }
    }
    for edge in &mechanism.edges {
        let mut encoder = CanonicalEncoder::new(EDGE_ID_V1);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encode_endpoint(&mut encoder, edge.endpoint);
        encoder.digest_bytes(edge.dependent.bytes());
        encoder.digest_bytes(edge.dependency.bytes());
        if StructuralEdgeId(encoder.digest()) != edge.id
            || !nodes.contains(&edge.dependent)
            || !nodes.contains(&edge.dependency)
        {
            return Err(StructuralMechanismError::Conservation(
                "structural edge reference",
            ));
        }
    }
    if mechanism
        .before_roots
        .iter()
        .chain(mechanism.after_roots.iter())
        .any(|root| !nodes.contains(root))
        || mechanism
            .ownership
            .iter()
            .any(|row| !nodes.contains(&row.node) || !mechanism_contexts.contains(&row.context))
        || mechanism
            .context_inventory
            .iter()
            .any(|row| !mechanism_contexts.contains(&row.context))
    {
        return Err(StructuralMechanismError::Conservation(
            "structural topology reference",
        ));
    }
    for row in &profile.frame_counts {
        if !profile_frames.contains_key(&row.frame) {
            return Err(StructuralMechanismError::Conservation(
                "profile frame reference",
            ));
        }
    }
    for row in profile
        .context_counts
        .iter()
        .chain(profile.activation_root_counts.iter())
    {
        if !profile_contexts.contains_key(&row.context) {
            return Err(StructuralMechanismError::Conservation(
                "profile context reference",
            ));
        }
    }
    for row in &profile.activation_call_counts {
        if !profile_contexts.contains_key(&row.parent) || !profile_contexts.contains_key(&row.child)
        {
            return Err(StructuralMechanismError::Conservation(
                "profile activation-call reference",
            ));
        }
    }
    for row in profile
        .node_counts
        .iter()
        .chain(profile.node_root_counts.iter())
    {
        if !nodes.contains(&row.node) {
            return Err(StructuralMechanismError::Conservation(
                "profile node reference",
            ));
        }
    }
    for row in &profile.edge_counts {
        if !edges.contains(&row.edge) {
            return Err(StructuralMechanismError::Conservation(
                "profile edge reference",
            ));
        }
    }
    for row in &profile.ownership_counts {
        if !nodes.contains(&row.node) || !profile_contexts.contains_key(&row.context) {
            return Err(StructuralMechanismError::Conservation(
                "profile ownership reference",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralSignatureAssignment {
    signature_id: MechanismSignatureId,
    mechanism_id: StructuralMechanismId,
    profile_id: ExecutionProfileId,
    membership_root: StructuralMembershipRoot,
    node_membership: Box<[StructuralNodeId]>,
    edge_membership: Box<[StructuralEdgeId]>,
    differential_node_membership: Box<[StructuralNodeId]>,
    differential_edge_membership: Box<[StructuralEdgeId]>,
}

impl StructuralSignatureAssignment {
    pub(crate) const fn signature_id(&self) -> MechanismSignatureId {
        self.signature_id
    }

    pub(crate) const fn mechanism_id(&self) -> StructuralMechanismId {
        self.mechanism_id
    }

    pub(crate) const fn profile_id(&self) -> ExecutionProfileId {
        self.profile_id
    }

    pub(crate) const fn membership_root(&self) -> StructuralMembershipRoot {
        self.membership_root
    }

    pub(crate) fn node_membership(&self) -> &[StructuralNodeId] {
        &self.node_membership
    }

    pub(crate) fn edge_membership(&self) -> &[StructuralEdgeId] {
        &self.edge_membership
    }

    pub(crate) fn differential_node_membership(&self) -> &[StructuralNodeId] {
        &self.differential_node_membership
    }

    pub(crate) fn differential_edge_membership(&self) -> &[StructuralEdgeId] {
        &self.differential_edge_membership
    }
}

/// Collision-checked, request-local structural interner. Raw mechanism
/// incidence remains the case-partition authority; this catalog only assigns
/// each complete raw signature to one structural mechanism and profile.
#[derive(Clone, Debug)]
pub(crate) struct StructuralMechanismCatalogBuilder {
    request_id: MechanismRequestId,
    revision: StructuralCatalogRevision,
    closure: Option<StructuralQuotientClosureReceipt>,
    definition_catalog_root: Option<StructuralDefinitionCatalogRoot>,
    canonical_frame_order: Arc<[StructuralFrameId]>,
    canonical_context_order: Arc<[StructuralActivationContextId]>,
    canonical_mechanism_order: Arc<[StructuralMechanismId]>,
    canonical_node_order: Arc<[StructuralNodeId]>,
    canonical_edge_order: Arc<[StructuralEdgeId]>,
    canonical_profile_order: Arc<[ExecutionProfileId]>,
    assignment_discovery_order: Vec<MechanismSignatureId>,
    assignment_discovery_revisions: Vec<StructuralCatalogRevision>,
    assignment_index: AuthenticatedTreapMap,
    frames: BTreeMap<StructuralFrameId, StructuralFrameDefinition>,
    contexts: BTreeMap<StructuralActivationContextId, StructuralActivationContextDefinition>,
    nodes: BTreeMap<StructuralNodeId, StructuralNodeDefinition>,
    edges: BTreeMap<StructuralEdgeId, StructuralEdgeDefinition>,
    mechanisms: BTreeMap<StructuralMechanismId, StructuralMechanismDefinition>,
    profiles: BTreeMap<ExecutionProfileId, StructuralExecutionProfile>,
    assignments: BTreeMap<MechanismSignatureId, StructuralSignatureAssignment>,
    mechanism_signatures: BTreeMap<StructuralMechanismId, BTreeSet<MechanismSignatureId>>,
    node_mechanisms: BTreeMap<StructuralNodeId, BTreeSet<StructuralMechanismId>>,
    edge_mechanisms: BTreeMap<StructuralEdgeId, BTreeSet<StructuralMechanismId>>,
    node_signatures: BTreeMap<StructuralNodeId, BTreeSet<MechanismSignatureId>>,
    edge_signatures: BTreeMap<StructuralEdgeId, BTreeSet<MechanismSignatureId>>,
    differential_node_signatures: BTreeMap<StructuralNodeId, BTreeSet<MechanismSignatureId>>,
    differential_edge_signatures: BTreeMap<StructuralEdgeId, BTreeSet<MechanismSignatureId>>,
}

impl StructuralMechanismCatalogBuilder {
    pub(crate) fn new(request_id: MechanismRequestId) -> Self {
        let revision = initial_catalog_revision(request_id);
        Self {
            request_id,
            revision,
            closure: None,
            definition_catalog_root: None,
            canonical_frame_order: Arc::default(),
            canonical_context_order: Arc::default(),
            canonical_mechanism_order: Arc::default(),
            canonical_node_order: Arc::default(),
            canonical_edge_order: Arc::default(),
            canonical_profile_order: Arc::default(),
            assignment_discovery_order: Vec::new(),
            assignment_discovery_revisions: vec![revision],
            assignment_index: AuthenticatedTreapMap::new(ASSIGNMENT_INDEX_V1),
            frames: BTreeMap::new(),
            contexts: BTreeMap::new(),
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            mechanisms: BTreeMap::new(),
            profiles: BTreeMap::new(),
            assignments: BTreeMap::new(),
            mechanism_signatures: BTreeMap::new(),
            node_mechanisms: BTreeMap::new(),
            edge_mechanisms: BTreeMap::new(),
            node_signatures: BTreeMap::new(),
            edge_signatures: BTreeMap::new(),
            differential_node_signatures: BTreeMap::new(),
            differential_edge_signatures: BTreeMap::new(),
        }
    }

    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn revision(&self) -> StructuralCatalogRevision {
        self.revision
    }

    pub(crate) const fn closure(&self) -> Option<StructuralQuotientClosureReceipt> {
        self.closure
    }

    pub(crate) const fn definition_catalog_root(&self) -> Option<StructuralDefinitionCatalogRoot> {
        self.definition_catalog_root
    }

    pub(crate) fn assignment(
        &self,
        signature_id: MechanismSignatureId,
    ) -> Option<&StructuralSignatureAssignment> {
        self.assignments.get(&signature_id)
    }

    pub(crate) fn assignment_count(&self) -> usize {
        self.assignments.len()
    }

    pub(crate) fn assignment_root(&self) -> [u8; 32] {
        self.assignment_index.root_hash()
    }

    /// Return the accepted structural-assignment discovery suffix. This is an
    /// operational stream cursor, not the canonical assignment-set order used
    /// by the eventual structural closure.
    pub(crate) fn assignment_discovery_suffix(
        &self,
        from_ordinal: usize,
    ) -> &[MechanismSignatureId] {
        &self.assignment_discovery_order[from_ordinal..]
    }

    /// Resolve one accepted assignment in operational discovery order without
    /// exposing or copying the catalog's assignment map. This is a read-only
    /// publication cursor; canonical closure continues to use signature-ID
    /// set order instead.
    pub(crate) fn assignment_discovery_at(
        &self,
        ordinal: usize,
    ) -> Option<&StructuralSignatureAssignment> {
        let signature_id = *self.assignment_discovery_order.get(ordinal)?;
        self.assignments.get(&signature_id)
    }

    pub(crate) const fn assignment_discovery_count(&self) -> usize {
        self.assignment_discovery_order.len()
    }

    pub(crate) fn assignment_discovery_prefix_revision(
        &self,
        prefix_len: usize,
    ) -> Option<StructuralCatalogRevision> {
        self.assignment_discovery_revisions.get(prefix_len).copied()
    }

    pub(crate) fn signatures_for_mechanism(
        &self,
        mechanism_id: StructuralMechanismId,
    ) -> Option<&BTreeSet<MechanismSignatureId>> {
        self.mechanism_signatures.get(&mechanism_id)
    }

    pub(crate) fn contains_mechanism(&self, mechanism_id: StructuralMechanismId) -> bool {
        self.mechanisms.contains_key(&mechanism_id)
    }

    pub(crate) fn contains_node(&self, node_id: StructuralNodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    pub(crate) fn contains_edge(&self, edge_id: StructuralEdgeId) -> bool {
        self.edges.contains_key(&edge_id)
    }

    pub(crate) fn signatures_for_node(
        &self,
        node_id: StructuralNodeId,
        differential: bool,
    ) -> Option<&BTreeSet<MechanismSignatureId>> {
        if differential {
            self.differential_node_signatures.get(&node_id)
        } else {
            self.node_signatures.get(&node_id)
        }
    }

    pub(crate) fn signatures_for_edge(
        &self,
        edge_id: StructuralEdgeId,
        differential: bool,
    ) -> Option<&BTreeSet<MechanismSignatureId>> {
        if differential {
            self.differential_edge_signatures.get(&edge_id)
        } else {
            self.edge_signatures.get(&edge_id)
        }
    }

    pub(crate) fn structural_mechanism_count(&self) -> usize {
        self.mechanisms.len()
    }

    /// Iterate every interned node once in canonical structural identity
    /// order. Publication composes this with a facet role; the closure-frozen
    /// ordinal index duplicates IDs only, never facets or case support.
    pub(crate) fn canonical_node_ids(
        &self,
    ) -> impl DoubleEndedIterator<Item = StructuralNodeId> + ExactSizeIterator + '_ {
        self.nodes.keys().copied()
    }

    /// Iterate every interned edge once in canonical structural identity
    /// order. Like nodes, these identities remain structural while any
    /// request/target-conditioned support is derived separately.
    pub(crate) fn canonical_edge_ids(
        &self,
    ) -> impl DoubleEndedIterator<Item = StructuralEdgeId> + ExactSizeIterator + '_ {
        self.edges.keys().copied()
    }

    /// Resolve one unique mechanism from the closure-frozen canonical ID
    /// order in O(1). The definition-catalog identity vectors are deliberately
    /// installed only at structural closure; they never contain cases.
    pub(crate) fn canonical_mechanism_id_at(
        &self,
        ordinal: usize,
    ) -> Option<StructuralMechanismId> {
        self.canonical_mechanism_order.get(ordinal).copied()
    }

    pub(crate) fn canonical_node_id_at(&self, ordinal: usize) -> Option<StructuralNodeId> {
        self.canonical_node_order.get(ordinal).copied()
    }

    pub(crate) fn canonical_edge_id_at(&self, ordinal: usize) -> Option<StructuralEdgeId> {
        self.canonical_edge_order.get(ordinal).copied()
    }

    /// Return the closure-frozen ordinal index sizes in O(1). Construction and
    /// the explicit structural-closure validation paths compare every vector
    /// with its authoritative BTreeMap key order; post-close mutation cannot
    /// add a new identity.
    pub(crate) fn canonical_subject_ordinal_counts(&self) -> Option<(usize, usize, usize)> {
        self.closure?;
        Some((
            self.canonical_mechanism_order.len(),
            self.canonical_node_order.len(),
            self.canonical_edge_order.len(),
        ))
    }

    pub(crate) fn execution_profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Number of unique definitions in the fixed normalized catalog order.
    /// Assignments and per-signature membership are intentionally excluded.
    pub(crate) fn canonical_definition_count(&self) -> Option<u128> {
        let counts = self.closure?.counts();
        counts
            .frames()
            .checked_add(counts.activation_contexts())?
            .checked_add(counts.nodes())?
            .checked_add(counts.edges())?
            .checked_add(counts.mechanisms())?
            .checked_add(counts.execution_profiles())
    }

    /// Resolve one definition from the closure-frozen concatenation
    /// `frame, activation_context, node, edge, mechanism, execution_profile`.
    /// Only an ID vector and a BTreeMap lookup are consulted; no definition
    /// payload or canonical preimage is copied.
    pub(crate) fn canonical_definition_at(
        &self,
        mut ordinal: usize,
    ) -> Option<StructuralDefinitionRef<'_>> {
        self.closure?;

        if ordinal < self.canonical_frame_order.len() {
            let id = self.canonical_frame_order[ordinal];
            return self.frames.get(&id).map(StructuralDefinitionRef::Frame);
        }
        ordinal -= self.canonical_frame_order.len();

        if ordinal < self.canonical_context_order.len() {
            let id = self.canonical_context_order[ordinal];
            return self
                .contexts
                .get(&id)
                .map(StructuralDefinitionRef::ActivationContext);
        }
        ordinal -= self.canonical_context_order.len();

        if ordinal < self.canonical_node_order.len() {
            let id = self.canonical_node_order[ordinal];
            return self.nodes.get(&id).map(StructuralDefinitionRef::Node);
        }
        ordinal -= self.canonical_node_order.len();

        if ordinal < self.canonical_edge_order.len() {
            let id = self.canonical_edge_order[ordinal];
            return self.edges.get(&id).map(StructuralDefinitionRef::Edge);
        }
        ordinal -= self.canonical_edge_order.len();

        if ordinal < self.canonical_mechanism_order.len() {
            let id = self.canonical_mechanism_order[ordinal];
            return self
                .mechanisms
                .get(&id)
                .map(StructuralDefinitionRef::Mechanism);
        }
        ordinal -= self.canonical_mechanism_order.len();

        let id = *self.canonical_profile_order.get(ordinal)?;
        self.profiles
            .get(&id)
            .map(StructuralDefinitionRef::ExecutionProfile)
    }

    /// Seal the request-local structural quotient against the complete raw
    /// signature set. `expected_signatures` must be strictly increasing in
    /// canonical [`MechanismSignatureId`] order and must yield exactly
    /// `expected_signature_count` request-local IDs. The merge validates set
    /// equality without collecting another signature set or any node x case
    /// relation. The caller remains responsible for sourcing that iterator
    /// from the already closed raw-incidence authority; this catalog does not
    /// independently assert upstream incidence closure.
    pub(crate) fn close_against_expected_signatures<I>(
        &mut self,
        expected_signature_count: u128,
        expected_signatures: I,
    ) -> Result<StructuralQuotientClosureReceipt, StructuralMechanismError>
    where
        I: IntoIterator<Item = MechanismSignatureId>,
    {
        let expected_signature_set_root = self
            .validate_expected_signature_coverage(expected_signature_count, expected_signatures)?;
        let receipt =
            self.derive_closure_receipt(expected_signature_count, expected_signature_set_root);
        match self.closure {
            Some(existing) if existing == receipt && self.closed_definition_catalog_matches() => {
                Ok(existing)
            }
            Some(_) => Err(StructuralMechanismError::ClosureConflict),
            None => {
                self.canonical_frame_order = self.frames.keys().copied().collect::<Vec<_>>().into();
                self.canonical_context_order =
                    self.contexts.keys().copied().collect::<Vec<_>>().into();
                self.canonical_mechanism_order =
                    self.mechanisms.keys().copied().collect::<Vec<_>>().into();
                self.canonical_node_order = self.nodes.keys().copied().collect::<Vec<_>>().into();
                self.canonical_edge_order = self.edges.keys().copied().collect::<Vec<_>>().into();
                self.canonical_profile_order =
                    self.profiles.keys().copied().collect::<Vec<_>>().into();
                self.definition_catalog_root = Some(structural_definition_catalog_root(
                    receipt,
                    &self.canonical_frame_order,
                    &self.canonical_context_order,
                    &self.canonical_node_order,
                    &self.canonical_edge_order,
                    &self.canonical_mechanism_order,
                    &self.canonical_profile_order,
                ));
                self.closure = Some(receipt);
                Ok(receipt)
            }
        }
    }

    fn canonical_definition_ordinal_index_matches(&self) -> bool {
        self.canonical_frame_order
            .iter()
            .copied()
            .eq(self.frames.keys().copied())
            && self
                .canonical_context_order
                .iter()
                .copied()
                .eq(self.contexts.keys().copied())
            && self
                .canonical_node_order
                .iter()
                .copied()
                .eq(self.nodes.keys().copied())
            && self
                .canonical_edge_order
                .iter()
                .copied()
                .eq(self.edges.keys().copied())
            && self
                .canonical_mechanism_order
                .iter()
                .copied()
                .eq(self.mechanisms.keys().copied())
            && self
                .canonical_profile_order
                .iter()
                .copied()
                .eq(self.profiles.keys().copied())
    }

    fn closed_definition_catalog_matches(&self) -> bool {
        let Some(closure) = self.closure else {
            return false;
        };
        let counts = closure.counts();
        self.canonical_definition_ordinal_index_matches()
            && counts.frames() == self.canonical_frame_order.len() as u128
            && counts.activation_contexts() == self.canonical_context_order.len() as u128
            && counts.nodes() == self.canonical_node_order.len() as u128
            && counts.edges() == self.canonical_edge_order.len() as u128
            && counts.mechanisms() == self.canonical_mechanism_order.len() as u128
            && counts.execution_profiles() == self.canonical_profile_order.len() as u128
            && self.definition_catalog_root
                == Some(structural_definition_catalog_root(
                    closure,
                    &self.canonical_frame_order,
                    &self.canonical_context_order,
                    &self.canonical_node_order,
                    &self.canonical_edge_order,
                    &self.canonical_mechanism_order,
                    &self.canonical_profile_order,
                ))
    }

    /// Revalidate a stored structural closure against the caller's complete
    /// raw-incidence signature set without changing the catalog. This is the
    /// support-layer bridge: matching cardinality alone is insufficient; the
    /// supplied IDs must exactly cover the structural assignments and rederive
    /// the same expected-set commitment sealed in the stored receipt.
    pub(crate) fn validate_closure_against_expected_signatures<I>(
        &self,
        expected_signature_count: u128,
        expected_signatures: I,
    ) -> Result<StructuralQuotientClosureReceipt, StructuralMechanismError>
    where
        I: IntoIterator<Item = MechanismSignatureId>,
    {
        let stored = self
            .closure
            .ok_or(StructuralMechanismError::ClosureUnavailable)?;
        let expected_signature_set_root = self
            .validate_expected_signature_coverage(expected_signature_count, expected_signatures)?;
        if stored.expected_signature_count() != expected_signature_count
            || stored.expected_signature_set_root() != expected_signature_set_root
            || !self.closed_definition_catalog_matches()
        {
            return Err(StructuralMechanismError::ClosureConflict);
        }
        let derived =
            self.derive_closure_receipt(expected_signature_count, expected_signature_set_root);
        if derived != stored {
            return Err(StructuralMechanismError::ClosureConflict);
        }
        Ok(stored)
    }

    fn derive_closure_receipt(
        &self,
        expected_signature_count: u128,
        expected_signature_set_root: StructuralExpectedSignatureSetRoot,
    ) -> StructuralQuotientClosureReceipt {
        let counts = self.quotient_counts();
        let signature_to_quotient_root = self.signature_to_quotient_root();
        let catalog_membership_root = self.catalog_membership_root();
        let root = structural_quotient_closure_root(
            self.request_id,
            expected_signature_count,
            expected_signature_set_root,
            signature_to_quotient_root,
            catalog_membership_root,
            counts,
        );
        StructuralQuotientClosureReceipt {
            closure_version: STRUCTURAL_QUOTIENT_CLOSURE_VERSION,
            quotient_version: STRUCTURAL_MECHANISM_QUOTIENT_VERSION,
            request_id: self.request_id,
            expected_signature_count,
            expected_signature_set_root,
            signature_to_quotient_root,
            catalog_membership_root,
            counts,
            root,
        }
    }

    fn validate_expected_signature_coverage<I>(
        &self,
        expected_signature_count: u128,
        expected_signatures: I,
    ) -> Result<StructuralExpectedSignatureSetRoot, StructuralMechanismError>
    where
        I: IntoIterator<Item = MechanismSignatureId>,
    {
        let mut encoder = CanonicalEncoder::new(EXPECTED_SIGNATURE_SET_ROOT_V1);
        encoder.u32(STRUCTURAL_QUOTIENT_CLOSURE_VERSION);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encoder.digest_bytes(self.request_id.bytes());
        encoder.u128(expected_signature_count);

        let mut actual = self.assignments.keys();
        let mut previous: Option<MechanismSignatureId> = None;
        let mut observed = 0_u128;
        for expected in expected_signatures {
            if expected.request_id() != self.request_id {
                return Err(StructuralMechanismError::ExpectedSignatureRequestMismatch);
            }
            if previous.is_some_and(|prior| expected <= prior) {
                return Err(StructuralMechanismError::ExpectedSignatureOrder);
            }
            if actual.next().copied() != Some(expected) {
                return Err(StructuralMechanismError::AssignmentCoverageMismatch);
            }
            observed = observed
                .checked_add(1)
                .ok_or(StructuralMechanismError::Capacity(
                    "expected signature count",
                ))?;
            encoder.digest_bytes(expected.bytes());
            previous = Some(expected);
        }
        if actual.next().is_some() {
            return Err(StructuralMechanismError::AssignmentCoverageMismatch);
        }
        if observed != expected_signature_count {
            return Err(StructuralMechanismError::ExpectedSignatureCountMismatch {
                declared: expected_signature_count,
                observed,
            });
        }
        Ok(StructuralExpectedSignatureSetRoot(encoder.digest()))
    }

    fn quotient_counts(&self) -> StructuralQuotientCounts {
        StructuralQuotientCounts {
            assignments: self.assignments.len() as u128,
            frames: self.frames.len() as u128,
            activation_contexts: self.contexts.len() as u128,
            nodes: self.nodes.len() as u128,
            edges: self.edges.len() as u128,
            mechanisms: self.mechanisms.len() as u128,
            execution_profiles: self.profiles.len() as u128,
        }
    }

    fn signature_to_quotient_root(&self) -> StructuralSignatureToQuotientRoot {
        let mut encoder = CanonicalEncoder::new(SIGNATURE_TO_QUOTIENT_ROOT_V1);
        encoder.u32(STRUCTURAL_QUOTIENT_CLOSURE_VERSION);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encoder.digest_bytes(self.request_id.bytes());
        encoder.len(self.assignments.len());
        for (signature_id, assignment) in &self.assignments {
            encoder.digest_bytes(signature_id.bytes());
            encoder.digest_bytes(assignment.mechanism_id.bytes());
            encoder.digest_bytes(assignment.profile_id.bytes());
        }
        StructuralSignatureToQuotientRoot(encoder.digest())
    }

    fn catalog_membership_root(&self) -> StructuralCatalogMembershipRoot {
        let mut encoder = CanonicalEncoder::new(CATALOG_MEMBERSHIP_ROOT_V1);
        encoder.u32(STRUCTURAL_QUOTIENT_CLOSURE_VERSION);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encoder.digest_bytes(self.request_id.bytes());
        encoder.len(self.assignments.len());
        for (signature_id, assignment) in &self.assignments {
            encoder.digest_bytes(signature_id.bytes());
            encoder.digest_bytes(assignment.membership_root.bytes());
        }
        StructuralCatalogMembershipRoot(encoder.digest())
    }

    pub(crate) fn intern_artifact(
        &mut self,
        artifact: &StructuralSignatureQuotientArtifact,
    ) -> Result<bool, StructuralMechanismError> {
        artifact.validate_internal_identity()?;
        if artifact.signature_id.request_id() != self.request_id {
            return Err(StructuralMechanismError::SignatureRequestMismatch);
        }
        let assignment = StructuralSignatureAssignment {
            signature_id: artifact.signature_id,
            mechanism_id: artifact.mechanism.id,
            profile_id: artifact.profile.id,
            membership_root: artifact.membership_root(),
            node_membership: artifact.node_membership.clone(),
            edge_membership: artifact.edge_membership.clone(),
            differential_node_membership: artifact.differential_node_membership.clone(),
            differential_edge_membership: artifact.differential_edge_membership.clone(),
        };
        if self.closure.is_some()
            && !self
                .assignments
                .get(&assignment.signature_id)
                .is_some_and(|existing| existing == &assignment)
        {
            return Err(StructuralMechanismError::CatalogClosed);
        }
        // Even an idempotent compact assignment must collision-check every
        // retained definition preimage before it can reuse existing IDs.
        preflight_definitions(self, artifact)?;
        let assignment_value = structural_assignment_value(&assignment);
        match self.assignments.get(&assignment.signature_id) {
            Some(existing) if existing == &assignment => {
                let indexed = self
                    .assignment_index
                    .get(&assignment.signature_id.bytes())
                    .map_err(|_| StructuralMechanismError::AuthenticatedIndex)?;
                if indexed != Some(assignment_value) {
                    return Err(StructuralMechanismError::AuthenticatedIndex);
                }
                return Ok(false);
            }
            Some(_) => return Err(StructuralMechanismError::AssignmentConflict),
            None => {}
        }

        let mut next_assignment_index = self.assignment_index.clone();
        next_assignment_index
            .insert(
                assignment.signature_id.bytes().to_vec().into_boxed_slice(),
                assignment_value,
            )
            .map_err(|_| StructuralMechanismError::AuthenticatedIndex)?;

        install_definitions(self, artifact);
        self.assignments
            .insert(assignment.signature_id, assignment.clone());
        self.mechanism_signatures
            .entry(assignment.mechanism_id)
            .or_default()
            .insert(assignment.signature_id);
        for node in &assignment.node_membership {
            self.node_mechanisms
                .entry(*node)
                .or_default()
                .insert(assignment.mechanism_id);
            self.node_signatures
                .entry(*node)
                .or_default()
                .insert(assignment.signature_id);
        }
        for edge in &assignment.edge_membership {
            self.edge_mechanisms
                .entry(*edge)
                .or_default()
                .insert(assignment.mechanism_id);
            self.edge_signatures
                .entry(*edge)
                .or_default()
                .insert(assignment.signature_id);
        }
        for node in &assignment.differential_node_membership {
            self.differential_node_signatures
                .entry(*node)
                .or_default()
                .insert(assignment.signature_id);
        }
        for edge in &assignment.differential_edge_membership {
            self.differential_edge_signatures
                .entry(*edge)
                .or_default()
                .insert(assignment.signature_id);
        }
        self.revision = advance_catalog_revision(self.revision, &assignment);
        self.assignment_discovery_order
            .push(assignment.signature_id);
        self.assignment_discovery_revisions.push(self.revision);
        self.assignment_index = next_assignment_index;
        Ok(true)
    }
}

fn structural_quotient_closure_root(
    request_id: MechanismRequestId,
    expected_signature_count: u128,
    expected_signature_set_root: StructuralExpectedSignatureSetRoot,
    signature_to_quotient_root: StructuralSignatureToQuotientRoot,
    catalog_membership_root: StructuralCatalogMembershipRoot,
    counts: StructuralQuotientCounts,
) -> StructuralQuotientClosureRoot {
    let mut encoder = CanonicalEncoder::new(QUOTIENT_CLOSURE_ROOT_V1);
    encoder.u32(STRUCTURAL_QUOTIENT_CLOSURE_VERSION);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(request_id.bytes());
    encoder.u128(expected_signature_count);
    encoder.digest_bytes(expected_signature_set_root.bytes());
    encoder.digest_bytes(signature_to_quotient_root.bytes());
    encoder.digest_bytes(catalog_membership_root.bytes());
    encoder.u128(counts.assignments());
    encoder.u128(counts.frames());
    encoder.u128(counts.activation_contexts());
    encoder.u128(counts.nodes());
    encoder.u128(counts.edges());
    encoder.u128(counts.mechanisms());
    encoder.u128(counts.execution_profiles());
    StructuralQuotientClosureRoot(encoder.digest())
}

#[allow(clippy::too_many_arguments)]
fn structural_definition_catalog_root(
    closure: StructuralQuotientClosureReceipt,
    frames: &[StructuralFrameId],
    contexts: &[StructuralActivationContextId],
    nodes: &[StructuralNodeId],
    edges: &[StructuralEdgeId],
    mechanisms: &[StructuralMechanismId],
    profiles: &[ExecutionProfileId],
) -> StructuralDefinitionCatalogRoot {
    // This root intentionally hashes the closure-frozen content IDs directly
    // rather than rebuilding any expanded definition preimage. The typed IDs
    // are collision-checked when interned and already commit the definitions
    // borrowed through `canonical_definition_at`.
    let mut hasher = Sha256::new();
    structural_definition_hash_bytes(&mut hasher, DEFINITION_CATALOG_ROOT_V1);
    hasher.update(STRUCTURAL_DEFINITION_CATALOG_VERSION.to_be_bytes());
    hasher.update(STRUCTURAL_MECHANISM_QUOTIENT_VERSION.to_be_bytes());
    hasher.update(closure.request_id().bytes());
    hasher.update(closure.root().bytes());
    hasher.update(closure.catalog_membership_root().bytes());

    let counts = closure.counts();
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::Frame,
        counts.frames(),
        frames.iter().map(|id| id.bytes()),
    );
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::ActivationContext,
        counts.activation_contexts(),
        contexts.iter().map(|id| id.bytes()),
    );
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::Node,
        counts.nodes(),
        nodes.iter().map(|id| id.bytes()),
    );
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::Edge,
        counts.edges(),
        edges.iter().map(|id| id.bytes()),
    );
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::Mechanism,
        counts.mechanisms(),
        mechanisms.iter().map(|id| id.bytes()),
    );
    structural_definition_hash_section(
        &mut hasher,
        StructuralDefinitionKind::ExecutionProfile,
        counts.execution_profiles(),
        profiles.iter().map(|id| id.bytes()),
    );
    StructuralDefinitionCatalogRoot(hasher.finalize().into())
}

fn structural_definition_hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u128).to_be_bytes());
    hasher.update(bytes);
}

fn structural_definition_hash_section(
    hasher: &mut Sha256,
    kind: StructuralDefinitionKind,
    count: u128,
    ids: impl IntoIterator<Item = [u8; 32]>,
) {
    hasher.update([kind.canonical_tag()]);
    hasher.update(count.to_be_bytes());
    for id in ids {
        hasher.update(id);
    }
}

fn initial_catalog_revision(request_id: MechanismRequestId) -> StructuralCatalogRevision {
    let mut encoder = CanonicalEncoder::new(CATALOG_REVISION_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(request_id.bytes());
    StructuralCatalogRevision(encoder.digest())
}

fn advance_catalog_revision(
    previous: StructuralCatalogRevision,
    assignment: &StructuralSignatureAssignment,
) -> StructuralCatalogRevision {
    let mut encoder = CanonicalEncoder::new(CATALOG_REVISION_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(previous.bytes());
    encoder.digest_bytes(assignment.signature_id.bytes());
    encoder.digest_bytes(assignment.mechanism_id.bytes());
    encoder.digest_bytes(assignment.profile_id.bytes());
    encoder.digest_bytes(assignment.membership_root.bytes());
    StructuralCatalogRevision(encoder.digest())
}

fn structural_assignment_value(
    assignment: &StructuralSignatureAssignment,
) -> AuthenticatedTreapValue {
    let mut encoder = CanonicalEncoder::new(ASSIGNMENT_VALUE_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(assignment.signature_id.bytes());
    encoder.digest_bytes(assignment.mechanism_id.bytes());
    encoder.digest_bytes(assignment.profile_id.bytes());
    encoder.digest_bytes(assignment.membership_root.bytes());
    AuthenticatedTreapValue::new(encoder.digest(), 1)
}

fn preflight_definitions(
    catalog: &StructuralMechanismCatalogBuilder,
    artifact: &StructuralSignatureQuotientArtifact,
) -> Result<(), StructuralMechanismError> {
    for definition in artifact.profile.frames() {
        preflight_definition(&catalog.frames, definition.id, definition, "frame")?;
    }
    for definition in artifact.profile.activation_contexts() {
        preflight_definition(
            &catalog.contexts,
            definition.id,
            definition,
            "activation context",
        )?;
    }
    for definition in artifact.mechanism.nodes() {
        preflight_definition(&catalog.nodes, definition.id, definition, "node")?;
    }
    for definition in artifact.mechanism.edges() {
        preflight_definition(&catalog.edges, definition.id, definition, "edge")?;
    }
    preflight_definition(
        &catalog.mechanisms,
        artifact.mechanism.id,
        &artifact.mechanism,
        "mechanism",
    )?;
    preflight_definition(
        &catalog.profiles,
        artifact.profile.id,
        &artifact.profile,
        "execution profile",
    )
}

fn preflight_definition<K: Ord + Copy, V: Eq>(
    definitions: &BTreeMap<K, V>,
    id: K,
    definition: &V,
    subject: &'static str,
) -> Result<(), StructuralMechanismError> {
    if definitions
        .get(&id)
        .is_some_and(|existing| existing != definition)
    {
        return Err(StructuralMechanismError::IdentityCollision(subject));
    }
    Ok(())
}

fn install_definitions(
    catalog: &mut StructuralMechanismCatalogBuilder,
    artifact: &StructuralSignatureQuotientArtifact,
) {
    for definition in artifact.profile.frames() {
        catalog
            .frames
            .entry(definition.id)
            .or_insert_with(|| definition.clone());
    }
    for definition in artifact.profile.activation_contexts() {
        catalog.contexts.entry(definition.id).or_insert(*definition);
    }
    for definition in artifact.mechanism.nodes() {
        catalog
            .nodes
            .entry(definition.id)
            .or_insert_with(|| definition.clone());
    }
    for definition in artifact.mechanism.edges() {
        catalog
            .edges
            .entry(definition.id)
            .or_insert_with(|| definition.clone());
    }
    catalog
        .mechanisms
        .entry(artifact.mechanism.id)
        .or_insert_with(|| artifact.mechanism.clone());
    catalog
        .profiles
        .entry(artifact.profile.id)
        .or_insert_with(|| artifact.profile.clone());
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StructuralActivationInputV1 {
    pub(super) parent: Option<usize>,
    pub(super) step: RelationalMechanismActivationStep,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StructuralOccurrenceInputV1 {
    pub(super) before_owner_activation: Option<usize>,
    pub(super) after_owner_activation: Option<usize>,
    pub(super) site: RelationalMechanismSiteId,
    pub(super) kind: RelationalMechanismEventKind,
    pub(super) before_outcome: Option<RelationalMechanismEventOutcome>,
    pub(super) after_outcome: Option<RelationalMechanismEventOutcome>,
    pub(super) before_root: bool,
    pub(super) after_root: bool,
}

/// Private DTO produced only after the executor has validated and exactly
/// paired both replay-ABI-v3 endpoint graphs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StructuralPairedDagInputV1 {
    pub(super) signature_id: MechanismSignatureId,
    pub(super) before_activations: Box<[StructuralActivationInputV1]>,
    pub(super) after_activations: Box<[StructuralActivationInputV1]>,
    pub(super) occurrences: Box<[StructuralOccurrenceInputV1]>,
    pub(super) before_edges: Box<[(usize, usize)]>,
    pub(super) after_edges: Box<[(usize, usize)]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralMechanismError {
    SignatureRequestMismatch,
    ExpectedSignatureRequestMismatch,
    ExpectedSignatureOrder,
    ExpectedSignatureCountMismatch {
        declared: u128,
        observed: u128,
    },
    AssignmentCoverageMismatch,
    AssignmentConflict,
    CatalogClosed,
    ClosureUnavailable,
    ClosureConflict,
    EmptyActivationTrie(RelationalMechanismEndpoint),
    InvalidActivationParent(RelationalMechanismEndpoint),
    InvalidOccurrenceEdge(RelationalMechanismEndpoint),
    MissingEndpointOccurrence(RelationalMechanismEndpoint),
    CyclicPairedDependencyUnionUnsupported,
    SourcePayloadBudgetExceeded {
        actual: usize,
        limit: usize,
    },
    DerivationWorkBudgetExceeded {
        minimum_required: usize,
        limit: usize,
    },
    ArtifactPayloadBudgetExceeded {
        minimum_required: usize,
        limit: usize,
    },
    Capacity(&'static str),
    IdentityCollision(&'static str),
    Conservation(&'static str),
    AuthenticatedIndex,
}

impl fmt::Display for StructuralMechanismError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignatureRequestMismatch => {
                formatter.write_str("structural artifact belongs to another mechanism request")
            }
            Self::ExpectedSignatureRequestMismatch => {
                formatter.write_str("structural closure expected a signature from another request")
            }
            Self::ExpectedSignatureOrder => formatter.write_str(
                "structural closure expected signatures are not strictly canonical and unique",
            ),
            Self::ExpectedSignatureCountMismatch { declared, observed } => write!(
                formatter,
                "structural closure declared {declared} expected signatures but observed {observed}"
            ),
            Self::AssignmentCoverageMismatch => formatter.write_str(
                "structural closure assignments do not exactly cover the expected raw signatures",
            ),
            Self::AssignmentConflict => {
                formatter.write_str("raw signature already has a different structural assignment")
            }
            Self::CatalogClosed => formatter.write_str(
                "structural quotient catalog cannot accept a new artifact after closure",
            ),
            Self::ClosureUnavailable => {
                formatter.write_str("structural quotient catalog has not been closed")
            }
            Self::ClosureConflict => formatter
                .write_str("structural quotient catalog already has different closure evidence"),
            Self::EmptyActivationTrie(endpoint) => {
                write!(
                    formatter,
                    "{endpoint:?} structural input has no activation root"
                )
            }
            Self::InvalidActivationParent(endpoint) => write!(
                formatter,
                "{endpoint:?} structural input has a non-canonical activation parent"
            ),
            Self::InvalidOccurrenceEdge(endpoint) => write!(
                formatter,
                "{endpoint:?} structural input has an invalid occurrence edge"
            ),
            Self::MissingEndpointOccurrence(endpoint) => write!(
                formatter,
                "{endpoint:?} structural topology references an absent occurrence"
            ),
            Self::CyclicPairedDependencyUnionUnsupported => formatter.write_str(
                "the paired dependency union is cyclic and unsupported by structural quotient v1",
            ),
            Self::SourcePayloadBudgetExceeded { actual, limit } => write!(
                formatter,
                "structural quotient source has {actual} canonical bytes; limit is {limit}"
            ),
            Self::DerivationWorkBudgetExceeded {
                minimum_required,
                limit,
            } => write!(
                formatter,
                "structural quotient derivation needs at least {minimum_required} logical work units; limit is {limit}"
            ),
            Self::ArtifactPayloadBudgetExceeded {
                minimum_required,
                limit,
            } => write!(
                formatter,
                "structural quotient artifact needs at least {minimum_required} canonical bytes; limit is {limit}"
            ),
            Self::Capacity(subject) => write!(formatter, "structural quotient {subject} overflow"),
            Self::IdentityCollision(subject) => {
                write!(
                    formatter,
                    "structural quotient detected a {subject} ID collision"
                )
            }
            Self::Conservation(subject) => {
                write!(
                    formatter,
                    "structural quotient failed {subject} conservation"
                )
            }
            Self::AuthenticatedIndex => {
                formatter.write_str("structural quotient authenticated assignment index failed")
            }
        }
    }
}

impl Error for StructuralMechanismError {}

pub(super) fn derive_structural_signature_quotient_v1(
    input: StructuralPairedDagInputV1,
    budget: StructuralDerivationBudget,
) -> Result<StructuralSignatureQuotientArtifact, StructuralMechanismError> {
    let activation_count = input
        .before_activations
        .len()
        .checked_add(input.after_activations.len())
        .ok_or(StructuralMechanismError::Capacity(
            "budgeted activation count",
        ))?;
    let edge_count = input
        .before_edges
        .len()
        .checked_add(input.after_edges.len())
        .ok_or(StructuralMechanismError::Capacity("budgeted edge count"))?;
    let occurrence_count = input
        .occurrences
        .iter()
        .try_fold(0usize, |count, occurrence| {
            count
                .checked_add(occurrence.before_outcome.is_some() as usize)
                .and_then(|count| count.checked_add(occurrence.after_outcome.is_some() as usize))
                .ok_or(StructuralMechanismError::Capacity(
                    "budgeted occurrence count",
                ))
        })?;
    budget.require_shape_admitted(activation_count, occurrence_count, edge_count)?;
    let before_frames = prepare_activation_endpoint(
        RelationalMechanismEndpoint::Before,
        &input.before_activations,
    )?;
    let after_frames =
        prepare_activation_endpoint(RelationalMechanismEndpoint::After, &input.after_activations)?;

    validate_endpoint_edges(
        RelationalMechanismEndpoint::Before,
        &input.occurrences,
        &input.before_edges,
    )?;
    validate_endpoint_edges(
        RelationalMechanismEndpoint::After,
        &input.occurrences,
        &input.after_edges,
    )?;

    let dependency_order = dependency_first_order(
        input.occurrences.len(),
        &input.before_edges,
        &input.after_edges,
    )?;
    let before_adjacency = dependency_sets(input.occurrences.len(), &input.before_edges)?;
    let after_adjacency = dependency_sets(input.occurrences.len(), &input.after_edges)?;
    let mut raw_to_node = vec![None; input.occurrences.len()];
    let mut nodes = BTreeMap::new();
    let mut owner_frames = Vec::new();
    let mut owner_contexts = Vec::new();
    owner_frames
        .try_reserve_exact(input.occurrences.len())
        .map_err(|_| StructuralMechanismError::Capacity("owner-frame table"))?;
    owner_contexts
        .try_reserve_exact(input.occurrences.len())
        .map_err(|_| StructuralMechanismError::Capacity("owner-context table"))?;
    for occurrence in &input.occurrences {
        let before_owner = endpoint_occurrence_owner(
            RelationalMechanismEndpoint::Before,
            occurrence.before_outcome.is_some(),
            occurrence.before_owner_activation,
            &before_frames,
        )?;
        let after_owner = endpoint_occurrence_owner(
            RelationalMechanismEndpoint::After,
            occurrence.after_outcome.is_some(),
            occurrence.after_owner_activation,
            &after_frames,
        )?;
        let owner_context = match (before_owner, after_owner) {
            (Some(before), Some(after)) if before != after => {
                return Err(StructuralMechanismError::Conservation(
                    "paired owner activation context",
                ));
            }
            (Some(owner), _) | (_, Some(owner)) => owner,
            (None, None) => {
                return Err(StructuralMechanismError::Conservation(
                    "paired occurrence owner",
                ));
            }
        };
        let owner_frame = before_frames
            .context_definition(owner_context)
            .or_else(|| after_frames.context_definition(owner_context))
            .ok_or(StructuralMechanismError::Conservation(
                "owner context definition",
            ))?
            .frame;
        owner_contexts.push(owner_context);
        owner_frames.push(owner_frame);
    }
    for raw in dependency_order {
        let occurrence = input
            .occurrences
            .get(raw)
            .ok_or(StructuralMechanismError::Capacity("raw occurrence index"))?;
        if occurrence.before_outcome.is_none() && occurrence.after_outcome.is_none() {
            return Err(StructuralMechanismError::Conservation(
                "paired occurrence presence",
            ));
        }
        if occurrence.before_root && occurrence.before_outcome.is_none() {
            return Err(StructuralMechanismError::MissingEndpointOccurrence(
                RelationalMechanismEndpoint::Before,
            ));
        }
        if occurrence.after_root && occurrence.after_outcome.is_none() {
            return Err(StructuralMechanismError::MissingEndpointOccurrence(
                RelationalMechanismEndpoint::After,
            ));
        }
        let before_dependencies = mapped_dependencies(
            before_adjacency
                .get(raw)
                .ok_or(StructuralMechanismError::Capacity("before adjacency index"))?,
            &raw_to_node,
        )?;
        let after_dependencies = mapped_dependencies(
            after_adjacency
                .get(raw)
                .ok_or(StructuralMechanismError::Capacity("after adjacency index"))?,
            &raw_to_node,
        )?;
        let mut definition = StructuralNodeDefinition {
            id: StructuralNodeId([0; 32]),
            owner_frame: owner_frames[raw],
            site: occurrence.site.clone(),
            kind: occurrence.kind,
            before_outcome: occurrence.before_outcome.clone(),
            after_outcome: occurrence.after_outcome.clone(),
            before_dependencies,
            after_dependencies,
        };
        definition.id = derive_node_id(&definition);
        match nodes.get(&definition.id) {
            Some(existing) if existing != &definition => {
                return Err(StructuralMechanismError::IdentityCollision("node"));
            }
            Some(_) => {}
            None => {
                nodes.insert(definition.id, definition.clone());
            }
        }
        raw_to_node[raw] = Some(definition.id);
    }
    if raw_to_node.iter().any(Option::is_none) {
        return Err(StructuralMechanismError::Conservation(
            "raw occurrence membership",
        ));
    }
    let raw_to_node: Vec<_> = raw_to_node.into_iter().map(Option::unwrap).collect();

    let (edge_definitions, before_edge_ids, after_edge_ids) =
        derive_edges(&raw_to_node, &input.before_edges, &input.after_edges)?;
    let before_roots = derive_node_roots(
        &input.occurrences,
        &raw_to_node,
        RelationalMechanismEndpoint::Before,
    );
    let after_roots = derive_node_roots(
        &input.occurrences,
        &raw_to_node,
        RelationalMechanismEndpoint::After,
    );

    let (frames, activation_contexts, context_inventory) =
        relevant_activation_topology(&input.occurrences, &before_frames, &after_frames)?;
    let ownership = derive_structural_ownership(&input.occurrences, &raw_to_node, &owner_contexts);

    let mut mechanism = StructuralMechanismDefinition {
        id: StructuralMechanismId([0; 32]),
        frames,
        activation_contexts,
        nodes: nodes.into_values().collect::<Vec<_>>().into_boxed_slice(),
        edges: edge_definitions,
        context_inventory,
        before_roots,
        after_roots,
        ownership,
    };
    mechanism.id = derive_mechanism_id(&mechanism);

    let membership = input
        .occurrences
        .iter()
        .enumerate()
        .map(|(raw, occurrence)| {
            Ok(StructuralOccurrenceMembership {
                raw_union_ordinal: u32::try_from(raw)
                    .map_err(|_| StructuralMechanismError::Capacity("membership ordinal"))?,
                node: raw_to_node[raw],
                owner_frame: owner_frames[raw],
                owner_context: owner_contexts[raw],
                before_present: occurrence.before_outcome.is_some(),
                after_present: occurrence.after_outcome.is_some(),
            })
        })
        .collect::<Result<Vec<_>, StructuralMechanismError>>()?
        .into_boxed_slice();

    let mut profile = derive_profile(
        mechanism.id,
        &input,
        &before_frames,
        &after_frames,
        &raw_to_node,
        &owner_contexts,
        &before_edge_ids,
        &after_edge_ids,
    )?;
    profile.id = derive_profile_id(&profile);
    validate_profile_conservation(&profile)?;
    let activation_membership = before_frames
        .membership
        .iter()
        .chain(after_frames.membership.iter())
        .copied()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    validate_activation_membership(
        &activation_membership,
        input.before_activations.len(),
        input.after_activations.len(),
    )?;

    let node_membership = set_box(raw_to_node.iter().copied());
    let edge_membership = set_box(before_edge_ids.iter().chain(after_edge_ids.iter()).copied());
    let (differential_node_membership, differential_edge_membership) =
        derive_differential_membership(&mechanism);
    let mut artifact = StructuralSignatureQuotientArtifact {
        signature_id: input.signature_id,
        mechanism,
        profile,
        activation_membership,
        membership,
        node_membership,
        edge_membership,
        differential_node_membership,
        differential_edge_membership,
        canonical_payload: Box::new([]),
    };
    artifact.canonical_payload =
        encode_artifact(&artifact, budget.payload_limit())?.into_boxed_slice();
    Ok(artifact)
}

struct PreparedActivationEndpoint {
    frames: BTreeMap<StructuralFrameId, StructuralFrameDefinition>,
    contexts: BTreeMap<StructuralActivationContextId, StructuralActivationContextDefinition>,
    context_ids: Vec<StructuralActivationContextId>,
    membership: Vec<StructuralActivationMembership>,
    frame_counts: BTreeMap<StructuralFrameId, u128>,
    context_counts: BTreeMap<StructuralActivationContextId, u128>,
    root_counts: BTreeMap<StructuralActivationContextId, u128>,
    call_counts: BTreeMap<(StructuralActivationContextId, StructuralActivationContextId), u128>,
}

impl PreparedActivationEndpoint {
    fn context_definition(
        &self,
        id: StructuralActivationContextId,
    ) -> Option<&StructuralActivationContextDefinition> {
        self.contexts.get(&id)
    }
}

fn prepare_activation_endpoint(
    endpoint: RelationalMechanismEndpoint,
    activations: &[StructuralActivationInputV1],
) -> Result<PreparedActivationEndpoint, StructuralMechanismError> {
    if activations.is_empty() {
        return Err(StructuralMechanismError::EmptyActivationTrie(endpoint));
    }
    let mut frames = BTreeMap::new();
    let mut contexts = BTreeMap::new();
    let mut context_ids = Vec::new();
    let mut membership = Vec::new();
    let mut frame_counts = BTreeMap::new();
    let mut context_counts = BTreeMap::new();
    let mut root_counts = BTreeMap::new();
    let mut call_counts = BTreeMap::new();
    let mut invocation_sets = BTreeMap::<
        (
            Option<usize>,
            RelationalMechanismSiteId,
            RelationalMechanismCalleeId,
        ),
        Vec<u32>,
    >::new();
    let mut root_count = 0usize;
    for (ordinal, activation) in activations.iter().enumerate() {
        if activation.parent.is_some_and(|parent| parent >= ordinal) {
            return Err(StructuralMechanismError::InvalidActivationParent(endpoint));
        }
        let frame = StructuralFrameDefinition::from_step(&activation.step);
        intern_frame(&mut frames, frame.clone())?;
        let parent_context = activation
            .parent
            .map(|parent| {
                context_ids
                    .get(parent)
                    .copied()
                    .ok_or(StructuralMechanismError::InvalidActivationParent(endpoint))
            })
            .transpose()?;
        let context = StructuralActivationContextDefinition::new(parent_context, frame.id);
        match contexts.get(&context.id) {
            Some(existing) if existing != &context => {
                return Err(StructuralMechanismError::IdentityCollision(
                    "activation context",
                ));
            }
            Some(_) => {}
            None => {
                contexts.insert(context.id, context);
            }
        }
        context_ids.push(context.id);
        increment(&mut frame_counts, frame.id)?;
        increment(&mut context_counts, context.id)?;
        invocation_sets
            .entry((
                activation.parent,
                activation.step.call_site().clone(),
                activation.step.callee().clone(),
            ))
            .or_default()
            .push(activation.step.invocation_ordinal());
        let parent_raw_activation_ordinal = activation
            .parent
            .map(|parent| {
                u32::try_from(parent)
                    .map_err(|_| StructuralMechanismError::Capacity("activation parent ordinal"))
            })
            .transpose()?;
        membership.push(StructuralActivationMembership {
            endpoint,
            raw_activation_ordinal: u32::try_from(ordinal)
                .map_err(|_| StructuralMechanismError::Capacity("activation ordinal"))?,
            context: context.id,
            frame: frame.id,
            parent_raw_activation_ordinal,
            parent_context,
            invocation_ordinal: activation.step.invocation_ordinal(),
        });
        match parent_context {
            Some(parent) => {
                increment(&mut call_counts, (parent, context.id))?;
            }
            None => {
                root_count = root_count
                    .checked_add(1)
                    .ok_or(StructuralMechanismError::Capacity("activation roots"))?;
                increment(&mut root_counts, context.id)?;
            }
        }
    }
    if root_count != 1 {
        return Err(StructuralMechanismError::InvalidActivationParent(endpoint));
    }
    for ordinals in invocation_sets.values_mut() {
        ordinals.sort_unstable();
        let raw_count = ordinals.len();
        ordinals.dedup();
        if ordinals.len() != raw_count
            || !ordinals
                .iter()
                .copied()
                .enumerate()
                .all(|(expected, actual)| usize::try_from(actual) == Ok(expected))
        {
            return Err(StructuralMechanismError::Conservation(
                "activation invocation ordinals",
            ));
        }
    }
    Ok(PreparedActivationEndpoint {
        frames,
        contexts,
        context_ids,
        membership,
        frame_counts,
        context_counts,
        root_counts,
        call_counts,
    })
}

fn intern_frame(
    frames: &mut BTreeMap<StructuralFrameId, StructuralFrameDefinition>,
    definition: StructuralFrameDefinition,
) -> Result<(), StructuralMechanismError> {
    match frames.get(&definition.id) {
        Some(existing) if existing != &definition => {
            Err(StructuralMechanismError::IdentityCollision("frame"))
        }
        Some(_) => Ok(()),
        None => {
            frames.insert(definition.id, definition);
            Ok(())
        }
    }
}

fn endpoint_occurrence_owner(
    endpoint: RelationalMechanismEndpoint,
    present: bool,
    owner: Option<usize>,
    prepared: &PreparedActivationEndpoint,
) -> Result<Option<StructuralActivationContextId>, StructuralMechanismError> {
    match (present, owner) {
        (false, None) => Ok(None),
        (true, Some(owner)) => prepared.context_ids.get(owner).copied().map(Some).ok_or(
            StructuralMechanismError::MissingEndpointOccurrence(endpoint),
        ),
        _ => Err(StructuralMechanismError::MissingEndpointOccurrence(
            endpoint,
        )),
    }
}

fn validate_endpoint_edges(
    endpoint: RelationalMechanismEndpoint,
    occurrences: &[StructuralOccurrenceInputV1],
    edges: &[(usize, usize)],
) -> Result<(), StructuralMechanismError> {
    let mut seen = BTreeSet::new();
    for &(dependent_index, dependency_index) in edges {
        let dependent = occurrences
            .get(dependent_index)
            .ok_or(StructuralMechanismError::InvalidOccurrenceEdge(endpoint))?;
        let dependency = occurrences
            .get(dependency_index)
            .ok_or(StructuralMechanismError::InvalidOccurrenceEdge(endpoint))?;
        let present = |occurrence: &StructuralOccurrenceInputV1| match endpoint {
            RelationalMechanismEndpoint::Before => occurrence.before_outcome.is_some(),
            RelationalMechanismEndpoint::After => occurrence.after_outcome.is_some(),
        };
        if !present(dependent) || !present(dependency) {
            return Err(StructuralMechanismError::MissingEndpointOccurrence(
                endpoint,
            ));
        }
        if !seen.insert((dependent_index, dependency_index)) {
            return Err(StructuralMechanismError::Conservation(
                "duplicate occurrence edge",
            ));
        }
    }
    Ok(())
}

fn all_activation_definitions(
    before: &PreparedActivationEndpoint,
    after: &PreparedActivationEndpoint,
) -> Result<
    (
        Box<[StructuralFrameDefinition]>,
        Box<[StructuralActivationContextDefinition]>,
    ),
    StructuralMechanismError,
> {
    let mut frames = BTreeMap::new();
    let mut contexts = BTreeMap::new();
    for prepared in [before, after] {
        for frame in prepared.frames.values() {
            intern_frame(&mut frames, frame.clone())?;
        }
        for definition in prepared.contexts.values().copied() {
            match contexts.get(&definition.id) {
                Some(existing) if existing != &definition => {
                    return Err(StructuralMechanismError::IdentityCollision(
                        "activation context",
                    ));
                }
                Some(_) => {}
                None => {
                    contexts.insert(definition.id, definition);
                }
            }
        }
    }
    Ok((
        frames.into_values().collect::<Vec<_>>().into_boxed_slice(),
        contexts
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    ))
}

fn relevant_activation_topology(
    occurrences: &[StructuralOccurrenceInputV1],
    before: &PreparedActivationEndpoint,
    after: &PreparedActivationEndpoint,
) -> Result<
    (
        Box<[StructuralFrameDefinition]>,
        Box<[StructuralActivationContextDefinition]>,
        Box<[StructuralEndpointContext]>,
    ),
    StructuralMechanismError,
> {
    let mut frames = BTreeMap::new();
    let mut contexts = BTreeMap::new();
    let mut inventory = BTreeSet::new();
    for (endpoint, prepared) in [
        (RelationalMechanismEndpoint::Before, before),
        (RelationalMechanismEndpoint::After, after),
    ] {
        for occurrence in occurrences {
            let owner = match endpoint {
                RelationalMechanismEndpoint::Before => occurrence.before_owner_activation,
                RelationalMechanismEndpoint::After => occurrence.after_owner_activation,
            };
            let Some(mut ordinal) = owner else {
                continue;
            };
            loop {
                let context_id = *prepared.context_ids.get(ordinal).ok_or(
                    StructuralMechanismError::MissingEndpointOccurrence(endpoint),
                )?;
                let definition = *prepared.contexts.get(&context_id).ok_or(
                    StructuralMechanismError::Conservation("activation context definition"),
                )?;
                match contexts.get(&context_id) {
                    Some(existing) if existing != &definition => {
                        return Err(StructuralMechanismError::IdentityCollision(
                            "activation context",
                        ));
                    }
                    Some(_) => {}
                    None => {
                        contexts.insert(context_id, definition);
                    }
                }
                let frame = prepared.frames.get(&definition.frame).ok_or(
                    StructuralMechanismError::Conservation("activation frame definition"),
                )?;
                intern_frame(&mut frames, frame.clone())?;
                inventory.insert(StructuralEndpointContext {
                    endpoint,
                    context: context_id,
                });
                match prepared.membership.get(ordinal).and_then(|row| {
                    row.parent_raw_activation_ordinal
                        .and_then(|parent| usize::try_from(parent).ok())
                }) {
                    Some(parent) => ordinal = parent,
                    None => break,
                }
            }
        }
    }
    Ok((
        frames.into_values().collect::<Vec<_>>().into_boxed_slice(),
        contexts
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        inventory.into_iter().collect::<Vec<_>>().into_boxed_slice(),
    ))
}

fn validate_activation_membership(
    rows: &[StructuralActivationMembership],
    before_count: usize,
    after_count: usize,
) -> Result<(), StructuralMechanismError> {
    for (endpoint, expected_count) in [
        (RelationalMechanismEndpoint::Before, before_count),
        (RelationalMechanismEndpoint::After, after_count),
    ] {
        let endpoint_rows = rows
            .iter()
            .filter(|row| row.endpoint == endpoint)
            .collect::<Vec<_>>();
        if endpoint_rows.len() != expected_count
            || !endpoint_rows
                .iter()
                .enumerate()
                .all(|(expected, row)| usize::try_from(row.raw_activation_ordinal) == Ok(expected))
        {
            return Err(StructuralMechanismError::Conservation(
                "activation membership partition",
            ));
        }
        for row in endpoint_rows {
            match (row.parent_raw_activation_ordinal, row.parent_context) {
                (None, None) => {}
                (Some(parent), Some(parent_context)) => {
                    let parent = usize::try_from(parent).map_err(|_| {
                        StructuralMechanismError::Capacity("activation parent ordinal")
                    })?;
                    let raw_ordinal = usize::try_from(row.raw_activation_ordinal)
                        .map_err(|_| StructuralMechanismError::Capacity("activation ordinal"))?;
                    if parent >= raw_ordinal
                        || rows
                            .iter()
                            .find(|candidate| {
                                candidate.endpoint == endpoint
                                    && usize::try_from(candidate.raw_activation_ordinal)
                                        == Ok(parent)
                            })
                            .map(|candidate| candidate.context)
                            != Some(parent_context)
                    {
                        return Err(StructuralMechanismError::Conservation(
                            "activation membership parent",
                        ));
                    }
                }
                _ => {
                    return Err(StructuralMechanismError::Conservation(
                        "activation membership parent",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn dependency_first_order(
    node_count: usize,
    before_edges: &[(usize, usize)],
    after_edges: &[(usize, usize)],
) -> Result<Vec<usize>, StructuralMechanismError> {
    let mut union = BTreeSet::new();
    for &(dependent, dependency) in before_edges.iter().chain(after_edges.iter()) {
        if dependent >= node_count || dependency >= node_count {
            return Err(StructuralMechanismError::Capacity("dependency index"));
        }
        union.insert((dependent, dependency));
    }
    let mut remaining = vec![0usize; node_count];
    let mut reverse = vec![Vec::new(); node_count];
    for (dependent, dependency) in union {
        remaining[dependent] = remaining[dependent]
            .checked_add(1)
            .ok_or(StructuralMechanismError::Capacity("dependency degree"))?;
        reverse[dependency].push(dependent);
    }
    let mut ready: VecDeque<_> = remaining
        .iter()
        .enumerate()
        .filter_map(|(node, count)| (*count == 0).then_some(node))
        .collect();
    let mut order = Vec::new();
    order
        .try_reserve_exact(node_count)
        .map_err(|_| StructuralMechanismError::Capacity("topological order"))?;
    while let Some(node) = ready.pop_front() {
        order.push(node);
        for dependent in &reverse[node] {
            remaining[*dependent] -= 1;
            if remaining[*dependent] == 0 {
                ready.push_back(*dependent);
            }
        }
    }
    if order.len() != node_count {
        return Err(StructuralMechanismError::CyclicPairedDependencyUnionUnsupported);
    }
    Ok(order)
}

fn dependency_sets(
    node_count: usize,
    edges: &[(usize, usize)],
) -> Result<Vec<Vec<usize>>, StructuralMechanismError> {
    let mut rows = vec![Vec::new(); node_count];
    for &(dependent, dependency) in edges {
        if dependent >= node_count || dependency >= node_count {
            return Err(StructuralMechanismError::Capacity("dependency index"));
        }
        rows[dependent].push(dependency);
    }
    for row in &mut rows {
        row.sort_unstable();
        row.dedup();
    }
    Ok(rows)
}

fn mapped_dependencies(
    dependencies: &[usize],
    raw_to_node: &[Option<StructuralNodeId>],
) -> Result<Box<[StructuralNodeId]>, StructuralMechanismError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(dependencies.len())
        .map_err(|_| StructuralMechanismError::Capacity("mapped dependencies"))?;
    for dependency in dependencies {
        result.push(raw_to_node.get(*dependency).copied().flatten().ok_or(
            StructuralMechanismError::Conservation("dependency-first order"),
        )?);
    }
    result.sort_unstable();
    result.dedup();
    Ok(result.into_boxed_slice())
}

fn derive_node_id(definition: &StructuralNodeDefinition) -> StructuralNodeId {
    let mut encoder = CanonicalEncoder::new(NODE_ID_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(definition.owner_frame.bytes());
    encode_site(&mut encoder, &definition.site);
    encode_event_kind(&mut encoder, definition.kind);
    encode_optional_outcome(&mut encoder, definition.before_outcome.as_ref());
    encode_optional_outcome(&mut encoder, definition.after_outcome.as_ref());
    encode_ids(&mut encoder, &definition.before_dependencies, |id| {
        id.bytes()
    });
    encode_ids(&mut encoder, &definition.after_dependencies, |id| {
        id.bytes()
    });
    StructuralNodeId(encoder.digest())
}

fn derive_edges(
    raw_to_node: &[StructuralNodeId],
    before: &[(usize, usize)],
    after: &[(usize, usize)],
) -> Result<
    (
        Box<[StructuralEdgeDefinition]>,
        Vec<StructuralEdgeId>,
        Vec<StructuralEdgeId>,
    ),
    StructuralMechanismError,
> {
    let mut definitions = BTreeMap::new();
    let before_ids = derive_endpoint_edges(
        RelationalMechanismEndpoint::Before,
        raw_to_node,
        before,
        &mut definitions,
    )?;
    let after_ids = derive_endpoint_edges(
        RelationalMechanismEndpoint::After,
        raw_to_node,
        after,
        &mut definitions,
    )?;
    Ok((
        definitions
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        before_ids,
        after_ids,
    ))
}

fn derive_endpoint_edges(
    endpoint: RelationalMechanismEndpoint,
    raw_to_node: &[StructuralNodeId],
    raw_edges: &[(usize, usize)],
    definitions: &mut BTreeMap<StructuralEdgeId, StructuralEdgeDefinition>,
) -> Result<Vec<StructuralEdgeId>, StructuralMechanismError> {
    let mut ids = Vec::new();
    ids.try_reserve_exact(raw_edges.len())
        .map_err(|_| StructuralMechanismError::Capacity("edge membership"))?;
    for &(dependent, dependency) in raw_edges {
        let dependent = *raw_to_node
            .get(dependent)
            .ok_or(StructuralMechanismError::InvalidOccurrenceEdge(endpoint))?;
        let dependency = *raw_to_node
            .get(dependency)
            .ok_or(StructuralMechanismError::InvalidOccurrenceEdge(endpoint))?;
        let mut encoder = CanonicalEncoder::new(EDGE_ID_V1);
        encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
        encode_endpoint(&mut encoder, endpoint);
        encoder.digest_bytes(dependent.bytes());
        encoder.digest_bytes(dependency.bytes());
        let definition = StructuralEdgeDefinition {
            id: StructuralEdgeId(encoder.digest()),
            endpoint,
            dependent,
            dependency,
        };
        match definitions.get(&definition.id) {
            Some(existing) if existing != &definition => {
                return Err(StructuralMechanismError::IdentityCollision("edge"));
            }
            Some(_) => {}
            None => {
                definitions.insert(definition.id, definition.clone());
            }
        }
        ids.push(definition.id);
    }
    Ok(ids)
}

fn derive_node_roots(
    occurrences: &[StructuralOccurrenceInputV1],
    raw_to_node: &[StructuralNodeId],
    endpoint: RelationalMechanismEndpoint,
) -> Box<[StructuralNodeId]> {
    set_box(
        occurrences
            .iter()
            .zip(raw_to_node)
            .filter_map(|(occurrence, node)| {
                let root = match endpoint {
                    RelationalMechanismEndpoint::Before => occurrence.before_root,
                    RelationalMechanismEndpoint::After => occurrence.after_root,
                };
                root.then_some(*node)
            }),
    )
}

fn derive_structural_ownership(
    occurrences: &[StructuralOccurrenceInputV1],
    raw_to_node: &[StructuralNodeId],
    owners: &[StructuralActivationContextId],
) -> Box<[StructuralNodeOwnership]> {
    let mut rows = BTreeSet::new();
    for ((occurrence, node), context) in occurrences.iter().zip(raw_to_node).zip(owners) {
        if occurrence.before_outcome.is_some() {
            rows.insert(StructuralNodeOwnership {
                endpoint: RelationalMechanismEndpoint::Before,
                node: *node,
                context: *context,
            });
        }
        if occurrence.after_outcome.is_some() {
            rows.insert(StructuralNodeOwnership {
                endpoint: RelationalMechanismEndpoint::After,
                node: *node,
                context: *context,
            });
        }
    }
    rows.into_iter().collect::<Vec<_>>().into_boxed_slice()
}

fn derive_differential_membership(
    mechanism: &StructuralMechanismDefinition,
) -> (Box<[StructuralNodeId]>, Box<[StructuralEdgeId]>) {
    let differential_nodes: BTreeSet<_> = mechanism
        .nodes
        .iter()
        .filter(|node| node.before_outcome != node.after_outcome)
        .map(|node| node.id)
        .collect();
    let before_pairs: BTreeSet<_> = mechanism
        .edges
        .iter()
        .filter(|edge| edge.endpoint == RelationalMechanismEndpoint::Before)
        .map(|edge| (edge.dependent, edge.dependency))
        .collect();
    let after_pairs: BTreeSet<_> = mechanism
        .edges
        .iter()
        .filter(|edge| edge.endpoint == RelationalMechanismEndpoint::After)
        .map(|edge| (edge.dependent, edge.dependency))
        .collect();
    let differential_edges = mechanism
        .edges
        .iter()
        .filter(|edge| {
            differential_nodes.contains(&edge.dependent)
                || differential_nodes.contains(&edge.dependency)
                || match edge.endpoint {
                    RelationalMechanismEndpoint::Before => {
                        !after_pairs.contains(&(edge.dependent, edge.dependency))
                    }
                    RelationalMechanismEndpoint::After => {
                        !before_pairs.contains(&(edge.dependent, edge.dependency))
                    }
                }
        })
        .map(|edge| edge.id)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    (
        differential_nodes
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        differential_edges,
    )
}

fn derive_mechanism_id(definition: &StructuralMechanismDefinition) -> StructuralMechanismId {
    let mut encoder = CanonicalEncoder::new(MECHANISM_ID_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encode_mechanism_definition(&mut encoder, definition, false);
    StructuralMechanismId(encoder.digest())
}

#[allow(clippy::too_many_arguments)]
fn derive_profile(
    mechanism_id: StructuralMechanismId,
    input: &StructuralPairedDagInputV1,
    before_frames: &PreparedActivationEndpoint,
    after_frames: &PreparedActivationEndpoint,
    raw_to_node: &[StructuralNodeId],
    owner_contexts: &[StructuralActivationContextId],
    before_edge_ids: &[StructuralEdgeId],
    after_edge_ids: &[StructuralEdgeId],
) -> Result<StructuralExecutionProfile, StructuralMechanismError> {
    let (frames, activation_contexts) = all_activation_definitions(before_frames, after_frames)?;
    let frame_counts = frame_count_rows(before_frames, after_frames, |frames| &frames.frame_counts);
    let context_counts =
        context_count_rows(before_frames, after_frames, |frames| &frames.context_counts);
    let activation_root_counts =
        context_count_rows(before_frames, after_frames, |frames| &frames.root_counts);
    let activation_call_counts = call_count_rows(before_frames, after_frames);
    let mut node_counts = BTreeMap::new();
    let mut node_root_counts = BTreeMap::new();
    let mut ownership_counts = BTreeMap::new();
    for ((occurrence, node), context) in input
        .occurrences
        .iter()
        .zip(raw_to_node)
        .zip(owner_contexts)
    {
        if occurrence.before_outcome.is_some() {
            increment(
                &mut node_counts,
                (RelationalMechanismEndpoint::Before, *node),
            )?;
            increment(
                &mut ownership_counts,
                (RelationalMechanismEndpoint::Before, *node, *context),
            )?;
        }
        if occurrence.after_outcome.is_some() {
            increment(
                &mut node_counts,
                (RelationalMechanismEndpoint::After, *node),
            )?;
            increment(
                &mut ownership_counts,
                (RelationalMechanismEndpoint::After, *node, *context),
            )?;
        }
        if occurrence.before_root {
            increment(
                &mut node_root_counts,
                (RelationalMechanismEndpoint::Before, *node),
            )?;
        }
        if occurrence.after_root {
            increment(
                &mut node_root_counts,
                (RelationalMechanismEndpoint::After, *node),
            )?;
        }
    }
    let node_counts = node_counts
        .into_iter()
        .map(|((endpoint, node), count)| StructuralNodeCount {
            endpoint,
            node,
            count,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let node_root_counts = node_root_counts
        .into_iter()
        .map(|((endpoint, node), count)| StructuralNodeCount {
            endpoint,
            node,
            count,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ownership_counts = ownership_counts
        .into_iter()
        .map(
            |((endpoint, node, context), count)| StructuralOwnershipCount {
                endpoint,
                node,
                context,
                count,
            },
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let edge_counts = edge_count_rows(before_edge_ids, after_edge_ids)?;
    let before_totals = endpoint_totals(
        RelationalMechanismEndpoint::Before,
        &input.before_activations,
        &input.occurrences,
        &input.before_edges,
    )?;
    let after_totals = endpoint_totals(
        RelationalMechanismEndpoint::After,
        &input.after_activations,
        &input.occurrences,
        &input.after_edges,
    )?;
    Ok(StructuralExecutionProfile {
        id: ExecutionProfileId([0; 32]),
        mechanism_id,
        frames,
        activation_contexts,
        frame_counts,
        context_counts,
        activation_root_counts,
        activation_call_counts,
        node_counts,
        node_root_counts,
        edge_counts,
        ownership_counts,
        before_totals,
        after_totals,
    })
}

fn frame_count_rows(
    before: &PreparedActivationEndpoint,
    after: &PreparedActivationEndpoint,
    select: impl Fn(&PreparedActivationEndpoint) -> &BTreeMap<StructuralFrameId, u128>,
) -> Box<[StructuralFrameCount]> {
    [
        (RelationalMechanismEndpoint::Before, select(before)),
        (RelationalMechanismEndpoint::After, select(after)),
    ]
    .into_iter()
    .flat_map(|(endpoint, rows)| {
        rows.iter().map(move |(frame, count)| StructuralFrameCount {
            endpoint,
            frame: *frame,
            count: *count,
        })
    })
    .collect::<Vec<_>>()
    .into_boxed_slice()
}

fn context_count_rows(
    before: &PreparedActivationEndpoint,
    after: &PreparedActivationEndpoint,
    select: impl Fn(&PreparedActivationEndpoint) -> &BTreeMap<StructuralActivationContextId, u128>,
) -> Box<[StructuralContextCount]> {
    [
        (RelationalMechanismEndpoint::Before, select(before)),
        (RelationalMechanismEndpoint::After, select(after)),
    ]
    .into_iter()
    .flat_map(|(endpoint, rows)| {
        rows.iter()
            .map(move |(context, count)| StructuralContextCount {
                endpoint,
                context: *context,
                count: *count,
            })
    })
    .collect::<Vec<_>>()
    .into_boxed_slice()
}

fn call_count_rows(
    before: &PreparedActivationEndpoint,
    after: &PreparedActivationEndpoint,
) -> Box<[StructuralActivationCallCount]> {
    [
        (RelationalMechanismEndpoint::Before, &before.call_counts),
        (RelationalMechanismEndpoint::After, &after.call_counts),
    ]
    .into_iter()
    .flat_map(|(endpoint, rows)| {
        rows.iter().map(
            move |((parent, child), count)| StructuralActivationCallCount {
                endpoint,
                parent: *parent,
                child: *child,
                count: *count,
            },
        )
    })
    .collect::<Vec<_>>()
    .into_boxed_slice()
}

fn edge_count_rows(
    before: &[StructuralEdgeId],
    after: &[StructuralEdgeId],
) -> Result<Box<[StructuralEdgeCount]>, StructuralMechanismError> {
    let mut rows = BTreeMap::new();
    for (endpoint, ids) in [
        (RelationalMechanismEndpoint::Before, before),
        (RelationalMechanismEndpoint::After, after),
    ] {
        for id in ids {
            increment(&mut rows, (endpoint, *id))?;
        }
    }
    Ok(rows
        .into_iter()
        .map(|((endpoint, edge), count)| StructuralEdgeCount {
            endpoint,
            edge,
            count,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn endpoint_totals(
    endpoint: RelationalMechanismEndpoint,
    activations: &[StructuralActivationInputV1],
    occurrences: &[StructuralOccurrenceInputV1],
    edges: &[(usize, usize)],
) -> Result<StructuralEndpointExecutionTotals, StructuralMechanismError> {
    let to_u128 = |value: usize| {
        u128::try_from(value).map_err(|_| StructuralMechanismError::Capacity("endpoint total"))
    };
    let event_nodes = occurrences
        .iter()
        .filter(|occurrence| match endpoint {
            RelationalMechanismEndpoint::Before => occurrence.before_outcome.is_some(),
            RelationalMechanismEndpoint::After => occurrence.after_outcome.is_some(),
        })
        .count();
    let event_roots = occurrences
        .iter()
        .filter(|occurrence| match endpoint {
            RelationalMechanismEndpoint::Before => occurrence.before_root,
            RelationalMechanismEndpoint::After => occurrence.after_root,
        })
        .count();
    Ok(StructuralEndpointExecutionTotals {
        activation_nodes: to_u128(activations.len())?,
        activation_roots: to_u128(
            activations
                .iter()
                .filter(|activation| activation.parent.is_none())
                .count(),
        )?,
        activation_edges: to_u128(
            activations
                .iter()
                .filter(|activation| activation.parent.is_some())
                .count(),
        )?,
        event_nodes: to_u128(event_nodes)?,
        event_roots: to_u128(event_roots)?,
        event_edges: to_u128(edges.len())?,
        ownership_occurrences: to_u128(event_nodes)?,
    })
}

fn derive_profile_id(profile: &StructuralExecutionProfile) -> ExecutionProfileId {
    let mut encoder = CanonicalEncoder::new(PROFILE_ID_V1);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encode_profile(&mut encoder, profile, false);
    ExecutionProfileId(encoder.digest())
}

fn validate_profile_conservation(
    profile: &StructuralExecutionProfile,
) -> Result<(), StructuralMechanismError> {
    for endpoint in [
        RelationalMechanismEndpoint::Before,
        RelationalMechanismEndpoint::After,
    ] {
        let totals = match endpoint {
            RelationalMechanismEndpoint::Before => profile.before_totals,
            RelationalMechanismEndpoint::After => profile.after_totals,
        };
        let sum = |values: Vec<u128>| {
            values.into_iter().try_fold(0u128, |total, value| {
                total
                    .checked_add(value)
                    .ok_or(StructuralMechanismError::Capacity("profile sum"))
            })
        };
        if sum(profile
            .frame_counts
            .iter()
            .filter(|row| row.endpoint == endpoint)
            .map(|row| row.count)
            .collect())?
            != totals.activation_nodes
            || sum(profile
                .context_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.activation_nodes
            || sum(profile
                .activation_root_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.activation_roots
            || sum(profile
                .activation_call_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.activation_edges
            || sum(profile
                .node_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.event_nodes
            || sum(profile
                .node_root_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.event_roots
            || sum(profile
                .edge_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.event_edges
            || sum(profile
                .ownership_counts
                .iter()
                .filter(|row| row.endpoint == endpoint)
                .map(|row| row.count)
                .collect())?
                != totals.ownership_occurrences
        {
            return Err(StructuralMechanismError::Conservation("execution profile"));
        }
    }
    Ok(())
}

fn encode_artifact(
    artifact: &StructuralSignatureQuotientArtifact,
    max_bytes: usize,
) -> Result<Vec<u8>, StructuralMechanismError> {
    let mut encoder = CanonicalEncoder::new_bounded(ARTIFACT_V1, max_bytes);
    encoder.u32(STRUCTURAL_MECHANISM_QUOTIENT_VERSION);
    encoder.digest_bytes(artifact.signature_id.request_id().bytes());
    encoder.digest_bytes(artifact.signature_id.bytes());
    encode_mechanism_definition(&mut encoder, &artifact.mechanism, true);
    encode_profile(&mut encoder, &artifact.profile, true);
    encode_activation_membership(&mut encoder, &artifact.activation_membership);
    encode_occurrence_membership(&mut encoder, &artifact.membership);
    encode_ids(&mut encoder, &artifact.node_membership, |id| id.bytes());
    encode_ids(&mut encoder, &artifact.edge_membership, |id| id.bytes());
    encode_ids(&mut encoder, &artifact.differential_node_membership, |id| {
        id.bytes()
    });
    encode_ids(&mut encoder, &artifact.differential_edge_membership, |id| {
        id.bytes()
    });
    encoder.finish_bounded()
}

fn encode_activation_membership(
    encoder: &mut CanonicalEncoder,
    rows: &[StructuralActivationMembership],
) {
    encoder.len(rows.len());
    for row in rows {
        encode_endpoint(encoder, row.endpoint);
        encoder.u32(row.raw_activation_ordinal);
        encoder.digest_bytes(row.context.bytes());
        encoder.digest_bytes(row.frame.bytes());
        match (row.parent_raw_activation_ordinal, row.parent_context) {
            (Some(parent), Some(parent_context)) => {
                encoder.u8(0x01);
                encoder.u32(parent);
                encoder.digest_bytes(parent_context.bytes());
            }
            (None, None) => encoder.u8(0x00),
            _ => encoder.u8(0xff),
        }
        encoder.u32(row.invocation_ordinal);
    }
}

fn encode_occurrence_membership(
    encoder: &mut CanonicalEncoder,
    rows: &[StructuralOccurrenceMembership],
) {
    encoder.len(rows.len());
    for row in rows {
        encoder.u32(row.raw_union_ordinal);
        encoder.digest_bytes(row.node.bytes());
        encoder.digest_bytes(row.owner_frame.bytes());
        encoder.digest_bytes(row.owner_context.bytes());
        encoder.bool(row.before_present);
        encoder.bool(row.after_present);
    }
}

fn encode_mechanism_definition(
    encoder: &mut CanonicalEncoder,
    definition: &StructuralMechanismDefinition,
    include_id: bool,
) {
    if include_id {
        encoder.digest_bytes(definition.id.bytes());
    }
    encoder.len(definition.frames.len());
    for frame in &definition.frames {
        encoder.digest_bytes(frame.id.bytes());
        encode_site(encoder, &frame.call_site);
        encode_callee(encoder, &frame.callee);
    }
    encoder.len(definition.activation_contexts.len());
    for context in &definition.activation_contexts {
        encoder.digest_bytes(context.id.bytes());
        match context.parent {
            Some(parent) => {
                encoder.u8(0x01);
                encoder.digest_bytes(parent.bytes());
            }
            None => encoder.u8(0x00),
        }
        encoder.digest_bytes(context.frame.bytes());
    }
    encoder.len(definition.nodes.len());
    for node in &definition.nodes {
        encoder.digest_bytes(node.id.bytes());
        encoder.digest_bytes(node.owner_frame.bytes());
        encode_site(encoder, &node.site);
        encode_event_kind(encoder, node.kind);
        encode_optional_outcome(encoder, node.before_outcome.as_ref());
        encode_optional_outcome(encoder, node.after_outcome.as_ref());
        encode_ids(encoder, &node.before_dependencies, |id| id.bytes());
        encode_ids(encoder, &node.after_dependencies, |id| id.bytes());
    }
    encoder.len(definition.edges.len());
    for edge in &definition.edges {
        encoder.digest_bytes(edge.id.bytes());
        encode_endpoint(encoder, edge.endpoint);
        encoder.digest_bytes(edge.dependent.bytes());
        encoder.digest_bytes(edge.dependency.bytes());
    }
    encoder.len(definition.context_inventory.len());
    for row in &definition.context_inventory {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.context.bytes());
    }
    encode_ids(encoder, &definition.before_roots, |id| id.bytes());
    encode_ids(encoder, &definition.after_roots, |id| id.bytes());
    encoder.len(definition.ownership.len());
    for row in &definition.ownership {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.node.bytes());
        encoder.digest_bytes(row.context.bytes());
    }
}

fn encode_profile(
    encoder: &mut CanonicalEncoder,
    profile: &StructuralExecutionProfile,
    include_id: bool,
) {
    if include_id {
        encoder.digest_bytes(profile.id.bytes());
    }
    encoder.digest_bytes(profile.mechanism_id.bytes());
    encoder.len(profile.frames.len());
    for frame in &profile.frames {
        encoder.digest_bytes(frame.id.bytes());
        encode_site(encoder, &frame.call_site);
        encode_callee(encoder, &frame.callee);
    }
    encoder.len(profile.activation_contexts.len());
    for context in &profile.activation_contexts {
        encoder.digest_bytes(context.id.bytes());
        match context.parent {
            Some(parent) => {
                encoder.u8(0x01);
                encoder.digest_bytes(parent.bytes());
            }
            None => encoder.u8(0x00),
        }
        encoder.digest_bytes(context.frame.bytes());
    }
    encode_frame_counts(encoder, &profile.frame_counts);
    encode_context_counts(encoder, &profile.context_counts);
    encode_context_counts(encoder, &profile.activation_root_counts);
    encoder.len(profile.activation_call_counts.len());
    for row in &profile.activation_call_counts {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.parent.bytes());
        encoder.digest_bytes(row.child.bytes());
        encoder.u128(row.count);
    }
    encode_node_counts(encoder, &profile.node_counts);
    encode_node_counts(encoder, &profile.node_root_counts);
    encoder.len(profile.edge_counts.len());
    for row in &profile.edge_counts {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.edge.bytes());
        encoder.u128(row.count);
    }
    encoder.len(profile.ownership_counts.len());
    for row in &profile.ownership_counts {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.node.bytes());
        encoder.digest_bytes(row.context.bytes());
        encoder.u128(row.count);
    }
    encode_totals(encoder, profile.before_totals);
    encode_totals(encoder, profile.after_totals);
}

fn encode_frame_counts(encoder: &mut CanonicalEncoder, rows: &[StructuralFrameCount]) {
    encoder.len(rows.len());
    for row in rows {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.frame.bytes());
        encoder.u128(row.count);
    }
}

fn encode_context_counts(encoder: &mut CanonicalEncoder, rows: &[StructuralContextCount]) {
    encoder.len(rows.len());
    for row in rows {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.context.bytes());
        encoder.u128(row.count);
    }
}

fn encode_node_counts(encoder: &mut CanonicalEncoder, rows: &[StructuralNodeCount]) {
    encoder.len(rows.len());
    for row in rows {
        encode_endpoint(encoder, row.endpoint);
        encoder.digest_bytes(row.node.bytes());
        encoder.u128(row.count);
    }
}

fn encode_totals(encoder: &mut CanonicalEncoder, totals: StructuralEndpointExecutionTotals) {
    encoder.u128(totals.activation_nodes);
    encoder.u128(totals.activation_roots);
    encoder.u128(totals.activation_edges);
    encoder.u128(totals.event_nodes);
    encoder.u128(totals.event_roots);
    encoder.u128(totals.event_edges);
    encoder.u128(totals.ownership_occurrences);
}

fn encode_ids<T>(encoder: &mut CanonicalEncoder, values: &[T], bytes: impl Fn(&T) -> [u8; 32]) {
    encoder.len(values.len());
    for value in values {
        encoder.digest_bytes(bytes(value));
    }
}

fn encode_endpoint(encoder: &mut CanonicalEncoder, endpoint: RelationalMechanismEndpoint) {
    encoder.u8(match endpoint {
        RelationalMechanismEndpoint::Before => 0x01,
        RelationalMechanismEndpoint::After => 0x02,
    });
}

fn encode_site(encoder: &mut CanonicalEncoder, site: &RelationalMechanismSiteId) {
    encoder.u8(match site.kind() {
        RelationalMechanismSiteKind::Expression => 0x01,
        RelationalMechanismSiteKind::Callable => 0x02,
        RelationalMechanismSiteKind::RuleFamily => 0x03,
        RelationalMechanismSiteKind::RuleCandidate => 0x04,
    });
    encoder.digest_bytes(site.digest_bytes());
}

fn encode_callee(encoder: &mut CanonicalEncoder, callee: &RelationalMechanismCalleeId) {
    match callee {
        RelationalMechanismCalleeId::Function(site) => {
            encoder.u8(0x01);
            encode_site(encoder, site);
        }
        RelationalMechanismCalleeId::RuleFamily(site) => {
            encoder.u8(0x02);
            encode_site(encoder, site);
        }
    }
}

fn encode_event_kind(encoder: &mut CanonicalEncoder, kind: RelationalMechanismEventKind) {
    encoder.u8(match kind {
        RelationalMechanismEventKind::RuleAttempt => 0x01,
        RelationalMechanismEventKind::RuleSelection => 0x02,
        RelationalMechanismEventKind::IfDecision => 0x03,
        RelationalMechanismEventKind::MatchDecision => 0x04,
        RelationalMechanismEventKind::ShortCircuitAnd => 0x05,
        RelationalMechanismEventKind::ShortCircuitOr => 0x06,
    });
}

fn encode_optional_outcome(
    encoder: &mut CanonicalEncoder,
    outcome: Option<&RelationalMechanismEventOutcome>,
) {
    match outcome {
        None => encoder.u8(0x00),
        Some(outcome) => {
            encoder.u8(0x01);
            encode_outcome(encoder, outcome);
        }
    }
}

fn encode_outcome(encoder: &mut CanonicalEncoder, outcome: &RelationalMechanismEventOutcome) {
    match outcome {
        RelationalMechanismEventOutcome::RuleAttempt(outcome) => {
            encoder.u8(0x01);
            encoder.u8(match outcome {
                RelationalRuleAttemptOutcome::HeadMismatch => 0x01,
                RelationalRuleAttemptOutcome::GuardFalse => 0x02,
                RelationalRuleAttemptOutcome::BodyFalse => 0x03,
                RelationalRuleAttemptOutcome::Applicable => 0x04,
            });
        }
        RelationalMechanismEventOutcome::RuleSelection(outcome) => {
            encoder.u8(0x02);
            match outcome {
                RelationalRuleSelectionOutcome::NoApplicableRule => encoder.u8(0x00),
                RelationalRuleSelectionOutcome::Selected(site) => {
                    encoder.u8(0x01);
                    encode_site(encoder, site);
                }
            }
        }
        RelationalMechanismEventOutcome::IfDecision(outcome) => {
            encoder.u8(0x03);
            encoder.u8(match outcome {
                RelationalIfDecisionOutcome::Then => 0x01,
                RelationalIfDecisionOutcome::Else => 0x02,
            });
        }
        RelationalMechanismEventOutcome::MatchDecision { arm_index } => {
            encoder.u8(0x04);
            encoder.u32(*arm_index);
        }
        RelationalMechanismEventOutcome::ShortCircuit(outcome) => {
            encoder.u8(0x05);
            match outcome {
                RelationalShortCircuitOutcome::SkippedRight { result } => {
                    encoder.u8(0x01);
                    encoder.bool(*result);
                }
                RelationalShortCircuitOutcome::EvaluatedRight { result } => {
                    encoder.u8(0x02);
                    encoder.bool(*result);
                }
            }
        }
    }
}

fn set_box<T: Ord>(values: impl IntoIterator<Item = T>) -> Box<[T]> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn increment<K: Ord>(rows: &mut BTreeMap<K, u128>, key: K) -> Result<(), StructuralMechanismError> {
    let count = rows.entry(key).or_default();
    *count = count
        .checked_add(1)
        .ok_or(StructuralMechanismError::Capacity("multiplicity"))?;
    Ok(())
}

struct CanonicalEncoder {
    bytes: Vec<u8>,
    max_bytes: Option<usize>,
    minimum_required: Option<usize>,
    allocation_failed: bool,
}

impl CanonicalEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut encoder = Self {
            bytes: Vec::new(),
            max_bytes: None,
            minimum_required: None,
            allocation_failed: false,
        };
        encoder.bytes(domain);
        encoder
    }

    fn new_bounded(domain: &[u8], max_bytes: usize) -> Self {
        let mut encoder = Self {
            bytes: Vec::new(),
            max_bytes: Some(max_bytes),
            minimum_required: None,
            allocation_failed: false,
        };
        encoder.bytes(domain);
        encoder
    }

    fn u8(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.append(&value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.append(&value.to_be_bytes());
    }

    fn len(&mut self, value: usize) {
        self.u128(value as u128);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.len(value.len());
        self.append(value);
    }

    fn digest_bytes(&mut self, value: [u8; 32]) {
        self.append(&value);
    }

    fn digest(self) -> [u8; 32] {
        debug_assert!(self.max_bytes.is_none());
        debug_assert!(self.minimum_required.is_none());
        debug_assert!(!self.allocation_failed);
        Sha256::digest(self.bytes).into()
    }

    fn finish(self) -> Vec<u8> {
        debug_assert!(self.max_bytes.is_none());
        debug_assert!(self.minimum_required.is_none());
        debug_assert!(!self.allocation_failed);
        self.bytes
    }

    fn finish_bounded(self) -> Result<Vec<u8>, StructuralMechanismError> {
        let limit = self
            .max_bytes
            .expect("only a bounded canonical encoder uses bounded finish");
        if let Some(minimum_required) = self.minimum_required {
            return Err(StructuralMechanismError::ArtifactPayloadBudgetExceeded {
                minimum_required,
                limit,
            });
        }
        if self.allocation_failed {
            return Err(StructuralMechanismError::Capacity(
                "artifact payload allocation",
            ));
        }
        Ok(self.bytes)
    }

    fn append(&mut self, value: &[u8]) {
        if self.minimum_required.is_some() || self.allocation_failed {
            return;
        }
        let Some(limit) = self.max_bytes else {
            self.bytes.extend_from_slice(value);
            return;
        };
        let minimum_required = self
            .bytes
            .len()
            .checked_add(value.len())
            .unwrap_or(usize::MAX);
        if minimum_required > limit {
            self.minimum_required = Some(minimum_required);
            return;
        }
        if minimum_required > self.bytes.capacity() {
            let target_capacity = minimum_required
                .checked_next_power_of_two()
                .unwrap_or(limit)
                .min(limit);
            let additional = target_capacity.saturating_sub(self.bytes.len());
            if self.bytes.try_reserve_exact(additional).is_err() {
                self.allocation_failed = true;
                return;
            }
        }
        self.bytes.extend_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::super::mechanism_incidence::MechanismSignatureDefinition;
    use super::super::relation::{
        AdmissionId, FindPolarity, MechanismTargetId, QuestionId, RelationId,
    };
    use super::*;
    use crate::{
        AnalysisProgramId, CheckedCallableId, CheckedDeclarationOccurrenceId, DeclarationId,
        DeclarationKind, ExprSiteId, ModuleId,
    };

    const PERSONSKAT_CALIBRATED_SOURCE_UPPER_BYTES: usize = 31 << 20;
    const PERSONSKAT_CALIBRATED_ACTIVATIONS: usize = 16_703 + 16_495;
    const PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES: usize = 52_787 + 52_931;
    const PERSONSKAT_CALIBRATED_ENDPOINT_EDGES: usize = 54_378 + 54_544;
    const PERSONSKAT_CALIBRATED_WORK_UNITS: usize = 612_550_656;
    // Conservative fixed-width encoding envelope: assume no Before/After
    // occurrence pairing, every occurrence is a root and differential, and
    // every possible profile/count row is distinct. The final 64 KiB covers
    // all collection lengths, version fields, IDs, domains and totals omitted
    // from the per-row arithmetic.
    const PERSONSKAT_CALIBRATED_ARTIFACT_UPPER_BYTES: usize = PERSONSKAT_CALIBRATED_ACTIVATIONS
        * (99 + 97 + 33)
        + PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES * (202 + 32 + 65)
        + PERSONSKAT_CALIBRATED_ENDPOINT_EDGES * (32 + 97)
        + PERSONSKAT_CALIBRATED_ACTIVATIONS * (99 + 97 + 49 * 3 + 81)
        + PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES * (49 * 2 + 81)
        + PERSONSKAT_CALIBRATED_ENDPOINT_EDGES * 49
        + PERSONSKAT_CALIBRATED_ACTIVATIONS * 110
        + PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES * (102 + 32 + 32)
        + PERSONSKAT_CALIBRATED_ENDPOINT_EDGES * (32 + 32)
        + (64 << 10);

    fn quotient_test_declaration(name: &str, ordinal: usize) -> DeclarationId {
        DeclarationId {
            module: ModuleId {
                content_hash: "22".repeat(32).into_boxed_str(),
                internal_path: Box::default(),
            },
            kind: DeclarationKind::Function,
            owner: None,
            name: name.to_string().into_boxed_str(),
            arity: Some(1),
            ordinal,
        }
    }

    fn quotient_test_expression_site(
        program: &AnalysisProgramId,
        name: &str,
        ordinal: usize,
    ) -> RelationalMechanismSiteId {
        RelationalMechanismSiteId::from_checked_expression(&ExprSiteId {
            analysis_program: program.clone(),
            declaration: quotient_test_declaration(name, ordinal),
            normalized_declaration_ordinal: ordinal,
            ast_path: vec![ordinal as u32].into_boxed_slice(),
        })
        .expect("checked quotient-test expression site")
    }

    fn quotient_test_request_id() -> MechanismRequestId {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"structural-quotient-test");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"all-cases-admitted");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"all-cases-selected",
            FindPolarity::All,
        );
        MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"checked-quotient-observation",
            b"paired-dag-v1",
        )
    }

    fn quotient_test_artifact(
        request_id: MechanismRequestId,
        owner_invocation_ordinal: u32,
        occurrence_count: usize,
    ) -> StructuralSignatureQuotientArtifact {
        let owner_activation = usize::try_from(owner_invocation_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(1))
            .filter(|ordinal| *ordinal <= 2)
            .expect("quotient-test owner invocation 0 or 1");
        let signature = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            format!(
                "paired-dag-v1;activation=quotient_call;owner-invocation={owner_invocation_ordinal};\
                 event=quotient_event;outcome=then;occurrences={occurrence_count}"
            )
            .into_bytes(),
        );
        let program = AnalysisProgramId("11".repeat(32).into_boxed_str());
        let callable = CheckedCallableId {
            declaration: CheckedDeclarationOccurrenceId {
                declaration: quotient_test_declaration("quotient_callee", 1),
                declaration_occurrence_ordinal: 0,
                normalized_ordinal: 0,
            },
            structural_path: Box::default(),
        };
        let callee_site = RelationalMechanismSiteId::from_checked_callable(&program, &callable)
            .expect("checked quotient-test callee site");
        let callee = RelationalMechanismCalleeId::function(callee_site)
            .expect("checked quotient-test function callee");
        let root_activation = StructuralActivationInputV1 {
            parent: None,
            step: RelationalMechanismActivationStep::new(
                quotient_test_expression_site(&program, "quotient_root_call", 2),
                callee.clone(),
                0,
            )
            .expect("checked quotient-test root activation"),
        };
        let child_activation = |invocation_ordinal| StructuralActivationInputV1 {
            parent: Some(0),
            step: RelationalMechanismActivationStep::new(
                quotient_test_expression_site(&program, "quotient_child_call", 3),
                callee.clone(),
                invocation_ordinal,
            )
            .expect("checked quotient-test child activation"),
        };
        let activations = vec![root_activation, child_activation(0), child_activation(1)];
        let outcome = Some(RelationalMechanismEventOutcome::IfDecision(
            RelationalIfDecisionOutcome::Then,
        ));
        let occurrence = StructuralOccurrenceInputV1 {
            before_owner_activation: Some(owner_activation),
            after_owner_activation: Some(owner_activation),
            site: quotient_test_expression_site(&program, "quotient_event", 4),
            kind: RelationalMechanismEventKind::IfDecision,
            before_outcome: outcome.clone(),
            after_outcome: outcome,
            before_root: true,
            after_root: true,
        };
        let occurrences = vec![occurrence; occurrence_count].into_boxed_slice();

        let mut budget = relational_structural_derivation_budget();
        budget.admit_source(0).expect("quotient-test source budget");
        budget
            .admit_activations(6)
            .expect("quotient-test activation budget");
        budget
            .admit_occurrences(
                occurrence_count
                    .checked_mul(2)
                    .expect("quotient-test occurrence count"),
            )
            .expect("quotient-test occurrence budget");
        budget.admit_edges(0).expect("quotient-test edge budget");
        budget
            .finish_shape_admission()
            .expect("quotient-test shape budget");

        derive_structural_signature_quotient_v1(
            StructuralPairedDagInputV1 {
                signature_id: signature.id(),
                before_activations: activations.clone().into_boxed_slice(),
                after_activations: activations.into_boxed_slice(),
                occurrences,
                before_edges: Box::default(),
                after_edges: Box::default(),
            },
            budget,
        )
        .expect("complete paired DAG quotient")
    }

    fn quotient_negative_test_signature_id(
        request_id: MechanismRequestId,
        label: &str,
    ) -> MechanismSignatureId {
        MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            format!("paired-dag-v1;negative-contract={label}").into_bytes(),
        )
        .id()
    }

    fn quotient_negative_test_callee(program: &AnalysisProgramId) -> RelationalMechanismCalleeId {
        let callable = CheckedCallableId {
            declaration: CheckedDeclarationOccurrenceId {
                declaration: quotient_test_declaration("negative_quotient_callee", 10),
                declaration_occurrence_ordinal: 0,
                normalized_ordinal: 0,
            },
            structural_path: Box::default(),
        };
        let site = RelationalMechanismSiteId::from_checked_callable(program, &callable)
            .expect("checked negative quotient callee site");
        RelationalMechanismCalleeId::function(site)
            .expect("checked negative quotient function callee")
    }

    fn quotient_negative_test_activation(
        program: &AnalysisProgramId,
        name: &str,
        ordinal: usize,
    ) -> StructuralActivationInputV1 {
        StructuralActivationInputV1 {
            parent: None,
            step: RelationalMechanismActivationStep::new(
                quotient_test_expression_site(program, name, ordinal),
                quotient_negative_test_callee(program),
                0,
            )
            .expect("checked negative quotient root activation"),
        }
    }

    fn quotient_negative_test_occurrence(
        program: &AnalysisProgramId,
        name: &str,
        ordinal: usize,
    ) -> StructuralOccurrenceInputV1 {
        let outcome = Some(RelationalMechanismEventOutcome::IfDecision(
            RelationalIfDecisionOutcome::Then,
        ));
        StructuralOccurrenceInputV1 {
            before_owner_activation: Some(0),
            after_owner_activation: Some(0),
            site: quotient_test_expression_site(program, name, ordinal),
            kind: RelationalMechanismEventKind::IfDecision,
            before_outcome: outcome.clone(),
            after_outcome: outcome,
            before_root: true,
            after_root: true,
        }
    }

    fn quotient_negative_test_budget(
        input: &StructuralPairedDagInputV1,
    ) -> StructuralDerivationBudget {
        let activation_count = input.before_activations.len() + input.after_activations.len();
        let occurrence_count = input
            .occurrences
            .iter()
            .map(|occurrence| {
                usize::from(occurrence.before_outcome.is_some())
                    + usize::from(occurrence.after_outcome.is_some())
            })
            .sum();
        let edge_count = input.before_edges.len() + input.after_edges.len();
        let mut budget = relational_structural_derivation_budget();
        budget
            .admit_source(0)
            .expect("negative quotient source budget");
        budget
            .admit_activations(activation_count)
            .expect("negative quotient activation budget");
        budget
            .admit_occurrences(occurrence_count)
            .expect("negative quotient occurrence budget");
        budget
            .admit_edges(edge_count)
            .expect("negative quotient edge budget");
        budget
            .finish_shape_admission()
            .expect("negative quotient shape budget");
        budget
    }

    fn derive_and_intern_negative_test_input(
        catalog: &mut StructuralMechanismCatalogBuilder,
        input: StructuralPairedDagInputV1,
    ) -> Result<bool, StructuralMechanismError> {
        let budget = quotient_negative_test_budget(&input);
        let artifact = derive_structural_signature_quotient_v1(input, budget)?;
        catalog.intern_artifact(&artifact)
    }

    fn assert_no_structural_assignment(catalog: &StructuralMechanismCatalogBuilder) {
        assert_eq!(catalog.assignment_count(), 0);
        assert_eq!(catalog.assignment_discovery_count(), 0);
        assert_eq!(catalog.structural_mechanism_count(), 0);
        assert_eq!(catalog.execution_profile_count(), 0);
        assert!(catalog.assignments.is_empty());
        assert!(catalog.mechanisms.is_empty());
        assert!(catalog.profiles.is_empty());
    }

    #[test]
    fn catalog_quotients_raw_signatures_by_structure_and_retains_execution_profiles() {
        let request_id = quotient_test_request_id();
        let first = quotient_test_artifact(request_id, 0, 1);
        let renumbered = quotient_test_artifact(request_id, 1, 1);
        let repeated = quotient_test_artifact(request_id, 0, 2);

        let signature_ids = [
            first.signature_id(),
            renumbered.signature_id(),
            repeated.signature_id(),
        ];
        assert_ne!(signature_ids[0], signature_ids[1]);
        assert_ne!(signature_ids[0], signature_ids[2]);
        assert_ne!(signature_ids[1], signature_ids[2]);

        let mechanism_id: StructuralMechanismId = first.mechanism().id();
        assert_eq!(renumbered.mechanism().id(), mechanism_id);
        assert_eq!(repeated.mechanism().id(), mechanism_id);

        let single_visit_profile_id: ExecutionProfileId = first.profile().id();
        let repeated_visit_profile_id: ExecutionProfileId = repeated.profile().id();
        assert_eq!(renumbered.profile().id(), single_visit_profile_id);
        assert_ne!(repeated_visit_profile_id, single_visit_profile_id);
        assert_eq!(first.profile().mechanism_id(), mechanism_id);
        assert_eq!(renumbered.profile().mechanism_id(), mechanism_id);
        assert_eq!(repeated.profile().mechanism_id(), mechanism_id);

        let mut catalog = StructuralMechanismCatalogBuilder::new(request_id);
        assert!(catalog.intern_artifact(&first).unwrap());
        assert!(catalog.intern_artifact(&renumbered).unwrap());
        assert!(catalog.intern_artifact(&repeated).unwrap());

        let mut expected_signatures = signature_ids.to_vec();
        expected_signatures.sort_unstable();
        let closure = catalog
            .close_against_expected_signatures(
                expected_signatures.len() as u128,
                expected_signatures.iter().copied(),
            )
            .expect("exact structural quotient closure");

        assert_eq!(catalog.assignment_count(), 3);
        assert_eq!(catalog.structural_mechanism_count(), 1);
        assert_eq!(catalog.execution_profile_count(), 2);
        assert_eq!(catalog.canonical_subject_ordinal_counts(), Some((1, 1, 0)));
        assert_eq!(catalog.canonical_mechanism_id_at(0), Some(mechanism_id));
        assert_eq!(closure.expected_signature_count(), 3);
        assert_eq!(closure.counts().assignments(), 3);
        assert_eq!(closure.counts().frames(), 2);
        assert_eq!(closure.counts().activation_contexts(), 2);
        assert_eq!(closure.counts().nodes(), 1);
        assert_eq!(closure.counts().edges(), 0);
        assert_eq!(closure.counts().mechanisms(), 1);
        assert_eq!(closure.counts().execution_profiles(), 2);

        for signature_id in signature_ids[..2].iter().copied() {
            let assignment = catalog
                .assignment(signature_id)
                .expect("single-visit signature assignment");
            assert_eq!(assignment.signature_id(), signature_id);
            assert_eq!(assignment.mechanism_id(), mechanism_id);
            assert_eq!(assignment.profile_id(), single_visit_profile_id);
        }
        let repeated_assignment = catalog
            .assignment(signature_ids[2])
            .expect("repeated-visit signature assignment");
        assert_eq!(repeated_assignment.signature_id(), signature_ids[2]);
        assert_eq!(repeated_assignment.mechanism_id(), mechanism_id);
        assert_eq!(repeated_assignment.profile_id(), repeated_visit_profile_id);
        let expected_signature_set = expected_signatures.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(
            catalog.signatures_for_mechanism(mechanism_id),
            Some(&expected_signature_set)
        );
    }

    #[test]
    fn cyclic_structural_input_fails_closed_without_assignment() {
        let request_id = quotient_test_request_id();
        let program = AnalysisProgramId("33".repeat(32).into_boxed_str());
        let activation = quotient_negative_test_activation(&program, "cycle_root", 11);
        let input = StructuralPairedDagInputV1 {
            signature_id: quotient_negative_test_signature_id(request_id, "cycle"),
            before_activations: vec![activation.clone()].into_boxed_slice(),
            after_activations: vec![activation].into_boxed_slice(),
            occurrences: vec![
                quotient_negative_test_occurrence(&program, "cycle_first", 12),
                quotient_negative_test_occurrence(&program, "cycle_second", 13),
            ]
            .into_boxed_slice(),
            before_edges: vec![(0, 1), (1, 0)].into_boxed_slice(),
            after_edges: Box::default(),
        };
        let mut catalog = StructuralMechanismCatalogBuilder::new(request_id);

        assert_eq!(
            derive_and_intern_negative_test_input(&mut catalog, input),
            Err(StructuralMechanismError::CyclicPairedDependencyUnionUnsupported)
        );
        assert_no_structural_assignment(&catalog);
        assert!(catalog.frames.is_empty());
        assert!(catalog.contexts.is_empty());
        assert!(catalog.nodes.is_empty());
        assert!(catalog.edges.is_empty());
    }

    #[test]
    fn content_id_collision_fails_closed_without_assignment() {
        let request_id = quotient_test_request_id();
        let artifact = quotient_test_artifact(request_id, 0, 1);
        let expected_frame = artifact
            .profile()
            .frames()
            .first()
            .expect("quotient artifact frame")
            .clone();
        let collision_program = AnalysisProgramId("44".repeat(32).into_boxed_str());
        let colliding_frame = StructuralFrameDefinition {
            id: expected_frame.id(),
            call_site: quotient_test_expression_site(
                &collision_program,
                "colliding_frame_preimage",
                14,
            ),
            callee: expected_frame.callee().clone(),
        };
        assert_ne!(colliding_frame, expected_frame);

        let mut catalog = StructuralMechanismCatalogBuilder::new(request_id);
        catalog.frames.insert(expected_frame.id(), colliding_frame);
        let initial_revision = catalog.revision();
        let initial_assignment_root = catalog.assignment_root();

        assert_eq!(
            catalog.intern_artifact(&artifact),
            Err(StructuralMechanismError::IdentityCollision("frame"))
        );
        assert_eq!(catalog.revision(), initial_revision);
        assert_eq!(catalog.assignment_root(), initial_assignment_root);
        assert_no_structural_assignment(&catalog);
        assert_eq!(catalog.frames.len(), 1);
        assert!(catalog.contexts.is_empty());
        assert!(catalog.nodes.is_empty());
        assert!(catalog.edges.is_empty());
    }

    #[test]
    fn incomplete_and_unpaired_structural_input_fails_closed_without_assignment() {
        let request_id = quotient_test_request_id();
        let program = AnalysisProgramId("55".repeat(32).into_boxed_str());
        let activation = quotient_negative_test_activation(&program, "paired_root", 15);
        let mut incomplete_occurrence =
            quotient_negative_test_occurrence(&program, "incomplete_occurrence", 16);
        incomplete_occurrence.after_owner_activation = None;
        let incomplete = StructuralPairedDagInputV1 {
            signature_id: quotient_negative_test_signature_id(request_id, "incomplete"),
            before_activations: vec![activation.clone()].into_boxed_slice(),
            after_activations: vec![activation].into_boxed_slice(),
            occurrences: vec![incomplete_occurrence].into_boxed_slice(),
            before_edges: Box::default(),
            after_edges: Box::default(),
        };
        let mut catalog = StructuralMechanismCatalogBuilder::new(request_id);

        assert_eq!(
            derive_and_intern_negative_test_input(&mut catalog, incomplete),
            Err(StructuralMechanismError::MissingEndpointOccurrence(
                RelationalMechanismEndpoint::After
            ))
        );
        assert_no_structural_assignment(&catalog);

        let unpaired = StructuralPairedDagInputV1 {
            signature_id: quotient_negative_test_signature_id(request_id, "unpaired"),
            before_activations: vec![quotient_negative_test_activation(
                &program,
                "before_unpaired_root",
                17,
            )]
            .into_boxed_slice(),
            after_activations: vec![quotient_negative_test_activation(
                &program,
                "after_unpaired_root",
                18,
            )]
            .into_boxed_slice(),
            occurrences: vec![quotient_negative_test_occurrence(
                &program,
                "unpaired_occurrence",
                19,
            )]
            .into_boxed_slice(),
            before_edges: Box::default(),
            after_edges: Box::default(),
        };

        assert_eq!(
            derive_and_intern_negative_test_input(&mut catalog, unpaired),
            Err(StructuralMechanismError::Conservation(
                "paired owner activation context"
            ))
        );
        assert_no_structural_assignment(&catalog);
        assert!(catalog.frames.is_empty());
        assert!(catalog.contexts.is_empty());
        assert!(catalog.nodes.is_empty());
        assert!(catalog.edges.is_empty());
    }

    #[test]
    fn relational_budget_admits_the_recorded_personskat_shape_without_allocating_it() {
        let mut budget = relational_structural_derivation_budget();
        budget
            .admit_source(PERSONSKAT_CALIBRATED_SOURCE_UPPER_BYTES)
            .unwrap();
        budget
            .admit_activations(PERSONSKAT_CALIBRATED_ACTIVATIONS)
            .unwrap();
        budget
            .admit_occurrences(PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES)
            .unwrap();
        budget
            .admit_edges(PERSONSKAT_CALIBRATED_ENDPOINT_EDGES)
            .unwrap();
        budget.finish_shape_admission().unwrap();

        assert_eq!(
            STRUCTURAL_DERIVATION_BASE_WORK_UNITS
                + PERSONSKAT_CALIBRATED_ACTIVATIONS * STRUCTURAL_DERIVATION_ACTIVATION_WORK_UNITS
                + PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES
                    * STRUCTURAL_DERIVATION_OCCURRENCE_WORK_UNITS
                + PERSONSKAT_CALIBRATED_ENDPOINT_EDGES * STRUCTURAL_DERIVATION_EDGE_WORK_UNITS,
            PERSONSKAT_CALIBRATED_WORK_UNITS
        );
        assert_eq!(
            budget.remaining_work_units,
            RELATIONAL_STRUCTURAL_DERIVATION_MAX_WORK_UNITS - PERSONSKAT_CALIBRATED_WORK_UNITS
        );
        assert_eq!(
            budget.payload_limit(),
            RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES
        );
        assert_eq!(PERSONSKAT_CALIBRATED_ARTIFACT_UPPER_BYTES, 119_837_126);
        assert!(
            PERSONSKAT_CALIBRATED_ARTIFACT_UPPER_BYTES <= RELATIONAL_STRUCTURAL_ARTIFACT_MAX_BYTES
        );
        assert!(budget
            .require_shape_admitted(
                PERSONSKAT_CALIBRATED_ACTIVATIONS,
                PERSONSKAT_CALIBRATED_ENDPOINT_OCCURRENCES,
                PERSONSKAT_CALIBRATED_ENDPOINT_EDGES,
            )
            .is_ok());
    }

    #[test]
    fn relational_budget_fails_closed_in_source_and_work_lanes() {
        let mut source = relational_structural_derivation_budget();
        assert_eq!(
            source
                .admit_source(RELATIONAL_STRUCTURAL_SOURCE_MAX_BYTES + 1)
                .unwrap_err(),
            StructuralMechanismError::SourcePayloadBudgetExceeded {
                actual: RELATIONAL_STRUCTURAL_SOURCE_MAX_BYTES + 1,
                limit: RELATIONAL_STRUCTURAL_SOURCE_MAX_BYTES,
            }
        );

        let mut work = relational_structural_derivation_budget();
        work.admit_source(0).unwrap();
        let first_rejected_activation_count = (RELATIONAL_STRUCTURAL_DERIVATION_MAX_WORK_UNITS
            - STRUCTURAL_DERIVATION_BASE_WORK_UNITS)
            / STRUCTURAL_DERIVATION_ACTIVATION_WORK_UNITS
            + 1;
        assert!(matches!(
            work.admit_activations(first_rejected_activation_count),
            Err(StructuralMechanismError::DerivationWorkBudgetExceeded {
                limit: RELATIONAL_STRUCTURAL_DERIVATION_MAX_WORK_UNITS,
                ..
            })
        ));
    }
}
