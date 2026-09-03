//! Fresh endpoint replay for relational Explore mechanism requests.
//!
//! This module is the narrow execution seam between one checked
//! [`MechanismObservationIr`], a resolved relational request scope, and one
//! concrete [`RelationalCaseRef`]. It deliberately does not reuse the legacy
//! Cartesian request or case identities. A runtime must replay the same
//! checked endpoint template twice in independent trace sessions: once with
//! Before and once with After.
//!
//! The resulting [`MechanismSignatureDefinition`] contains the complete
//! canonical two-edge-coloured control DAG. Case and transition identity live
//! in a separate replay receipt. Keeping those layers separate is essential:
//! putting a CaseId in the signature would turn every concrete case into a
//! different "mechanism" and destroy incidence sharing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::mechanism::{MechanismObservationIr, MechanismSiteId};
use super::mechanism_incidence::{
    MechanismRequestScope, MechanismSignatureDefinition, MechanismSignatureId,
    MechanismUnavailableReasonId,
};
use super::relation::{
    MechanismTargetId, QuestionId, RelationProvenance, RelationalCaseId, RelationalCaseRef,
    SourceKey, SourceRow, SuccessorKey, SuccessorRow, ViewId,
};
use super::relational_endpoint_totality::RelationalEndpointTotalityCertificateId;
use super::structural_mechanism::{
    derive_structural_signature_quotient_v1, StructuralActivationInputV1,
    StructuralDerivationBudget, StructuralMechanismError, StructuralOccurrenceInputV1,
    StructuralPairedDagInputV1, StructuralSignatureQuotientArtifact,
};
use super::transition::{
    canonical_explore_value_digest, ContextSchemaId, StateId, StateSchemaId, TransitionId,
    TransitionInstance, TransitionInstanceCanonicalV1, TransitionSchemaIdentities,
    TransitionTypeId,
};
use super::ExploreValue;
use crate::{
    AnalysisProgramId, CheckedCallableId, CheckedRuleCandidateResolution, ExprSiteId,
    RuleDispatchKey, Ty,
};

/// Semantic ABI of the relational fresh-replay and normalization boundary.
/// Operational retry, scheduling, and resource policy are deliberately absent.
pub(crate) const RELATIONAL_MECHANISM_REPLAY_ABI_VERSION: u32 = 3;

const OBSERVATION_ID_V3: &[u8] = b"futuruna.explore.relational-mechanism-observation-id.v3";
const ENDPOINT_TRACE_ROOT_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-endpoint-trace-root.v3";
const SIGNATURE_DEFINITION_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-signature-definition.v3";
const REPLAY_RECEIPT_ID_V3: &[u8] = b"futuruna.explore.relational-mechanism-replay-receipt-id.v3";
const UNAVAILABLE_EVIDENCE_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-unavailable-evidence.v3";
const REPLAY_DURABLE_PAYLOAD_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-replay-durable-payload.v3";
const REPLAY_COMPACT_INCIDENCE_DURABLE_PAYLOAD_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-compact-incidence-durable-payload.v3";
const MECHANISM_TYPE_V3: &[u8] = b"futuruna.explore.relational-mechanism-type.v3";
const LEGACY_MECHANISM_SITE_HASH_V2: &[u8] = b"futuruna.explore.mechanism-site.v2";

const MAX_TRACE_NODES: usize = 65_536;
const MAX_TRACE_EDGES: usize = 262_144;
const MAX_ACTIVATION_DEPTH: usize = 256;
const MAX_TRACE_ACTIVATION_NODES: usize = 1_048_576;
const MAX_CHECKED_SITE_PATH_ITEMS: usize = 1_048_576;
const MAX_DURABLE_VALUE_DEPTH: usize = 128;
const MAX_DURABLE_VALUE_NODES: usize = 1_000_000;
const MAX_DURABLE_BLOB_BYTES: usize = 512 << 20;
const MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES: usize = MAX_TRACE_ACTIVATION_NODES * 2;
const MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES: usize = MAX_TRACE_NODES * 2;
const MAX_STRUCTURAL_QUOTIENT_ENDPOINT_EDGES: usize = MAX_TRACE_EDGES * 2;
const MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ACTIVATION_NODES: usize = 200_000;
const MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ENDPOINT_NODES: usize = 120_000;
const MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ENDPOINT_EDGES: usize = 200_000;
const MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_REFINEMENT_ROUNDS: usize = 64;

/// Content identity of the checked endpoint template and its typed contract.
///
/// This identity is kept beside the opaque request ID. The request prevents
/// cross-request conflation; this value makes the observation/template binding
/// independently inspectable in replay evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismReplayObservationId([u8; 32]);

impl RelationalMechanismReplayObservationId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn derive_checked(
        observation: &MechanismObservationIr,
    ) -> Result<Self, RelationalMechanismReplayError> {
        derive_observation_id(observation)
    }
}

/// Endpoint role of one isolated evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismEndpoint {
    Before,
    After,
}

impl RelationalMechanismEndpoint {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Before => 0x01,
            Self::After => 0x02,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Stable checked program-site kind retained in a relational trace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismSiteKind {
    Expression,
    Callable,
    RuleFamily,
    RuleCandidate,
}

impl RelationalMechanismSiteKind {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Expression => 0x01,
            Self::Callable => 0x02,
            Self::RuleFamily => 0x03,
            Self::RuleCandidate => 0x04,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Expression => "expression",
            Self::Callable => "callable",
            Self::RuleFamily => "rule_family",
            Self::RuleCandidate => "rule_candidate",
        }
    }
}

/// Stable, checked program-scoped semantic site.
///
/// Constructors consume checked-source identities and reuse the maintained
/// site hashing contract. There is intentionally no constructor from a naked
/// digest at this fresh-replay boundary.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismSiteId {
    analysis_program: AnalysisProgramId,
    kind: RelationalMechanismSiteKind,
    digest: [u8; 32],
}

impl RelationalMechanismSiteId {
    pub(crate) fn from_checked_expression(
        site: &ExprSiteId,
    ) -> Result<Self, RelationalMechanismReplayError> {
        let legacy_site = MechanismSiteId::from_expression_site(site)
            .map_err(|_| RelationalMechanismReplayError::InvalidCheckedObservation)?;
        Ok(Self {
            analysis_program: site.analysis_program.clone(),
            kind: RelationalMechanismSiteKind::Expression,
            digest: legacy_site.digest_bytes(),
        })
    }

    pub(crate) fn from_checked_callable(
        analysis_program: &AnalysisProgramId,
        callable: &CheckedCallableId,
    ) -> Result<Self, RelationalMechanismReplayError> {
        let legacy_site = MechanismSiteId::from_callable(analysis_program, callable)
            .map_err(|_| RelationalMechanismReplayError::InvalidCheckedObservation)?;
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: RelationalMechanismSiteKind::Callable,
            digest: legacy_site.digest_bytes(),
        })
    }

    pub(crate) fn from_checked_rule_family(
        analysis_program: &AnalysisProgramId,
        family: &RuleDispatchKey,
    ) -> Result<Self, RelationalMechanismReplayError> {
        let legacy_site = MechanismSiteId::from_rule_family(analysis_program, family)
            .map_err(|_| RelationalMechanismReplayError::InvalidCheckedObservation)?;
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: RelationalMechanismSiteKind::RuleFamily,
            digest: legacy_site.digest_bytes(),
        })
    }

    pub(crate) fn from_checked_rule_candidate(
        analysis_program: &AnalysisProgramId,
        candidate: &CheckedRuleCandidateResolution,
    ) -> Result<Self, RelationalMechanismReplayError> {
        let legacy_site = MechanismSiteId::from_rule_candidate(analysis_program, candidate)
            .map_err(|_| RelationalMechanismReplayError::InvalidCheckedObservation)?;
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: RelationalMechanismSiteKind::RuleCandidate,
            digest: legacy_site.digest_bytes(),
        })
    }

    pub(crate) const fn kind(&self) -> RelationalMechanismSiteKind {
        self.kind
    }

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }

    fn validate_for(
        &self,
        analysis_program: &AnalysisProgramId,
        expected_kind: Option<RelationalMechanismSiteKind>,
    ) -> Result<(), RelationalMechanismReplayError> {
        validate_analysis_program(&self.analysis_program)?;
        if &self.analysis_program != analysis_program {
            return Err(RelationalMechanismReplayError::ForeignTraceSite);
        }
        if expected_kind.is_some_and(|kind| kind != self.kind) {
            return Err(RelationalMechanismReplayError::TraceSiteKindMismatch);
        }
        Ok(())
    }
}

/// Stable callee kind in one dynamic activation path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismCalleeId {
    Function(RelationalMechanismSiteId),
    RuleFamily(RelationalMechanismSiteId),
}

impl RelationalMechanismCalleeId {
    pub(crate) fn function(
        site: RelationalMechanismSiteId,
    ) -> Result<Self, RelationalMechanismReplayError> {
        if site.kind != RelationalMechanismSiteKind::Callable {
            return Err(RelationalMechanismReplayError::TraceSiteKindMismatch);
        }
        Ok(Self::Function(site))
    }

    pub(crate) fn rule_family(
        site: RelationalMechanismSiteId,
    ) -> Result<Self, RelationalMechanismReplayError> {
        if site.kind != RelationalMechanismSiteKind::RuleFamily {
            return Err(RelationalMechanismReplayError::TraceSiteKindMismatch);
        }
        Ok(Self::RuleFamily(site))
    }

    pub(crate) fn site(&self) -> &RelationalMechanismSiteId {
        match self {
            Self::Function(site) | Self::RuleFamily(site) => site,
        }
    }

    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Function(_) => "function",
            Self::RuleFamily(_) => "rule_family",
        }
    }

    fn canonical_tag(&self) -> u8 {
        match self {
            Self::Function(_) => 0x01,
            Self::RuleFamily(_) => 0x02,
        }
    }

    fn validate_for(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), RelationalMechanismReplayError> {
        let expected = match self {
            Self::Function(_) => RelationalMechanismSiteKind::Callable,
            Self::RuleFamily(_) => RelationalMechanismSiteKind::RuleFamily,
        };
        self.site().validate_for(analysis_program, Some(expected))
    }
}

/// One outcome-free activation frame. Invocation ordinals are local to the
/// same enclosing path and checked call site.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismActivationStep {
    call_site: RelationalMechanismSiteId,
    callee: RelationalMechanismCalleeId,
    invocation_ordinal: u32,
}

impl RelationalMechanismActivationStep {
    pub(crate) fn new(
        call_site: RelationalMechanismSiteId,
        callee: RelationalMechanismCalleeId,
        invocation_ordinal: u32,
    ) -> Result<Self, RelationalMechanismReplayError> {
        if call_site.kind != RelationalMechanismSiteKind::Expression {
            return Err(RelationalMechanismReplayError::TraceSiteKindMismatch);
        }
        if call_site.analysis_program != callee.site().analysis_program {
            return Err(RelationalMechanismReplayError::ForeignTraceSite);
        }
        Ok(Self {
            call_site,
            callee,
            invocation_ordinal,
        })
    }

    pub(crate) const fn call_site(&self) -> &RelationalMechanismSiteId {
        &self.call_site
    }

    pub(crate) const fn callee(&self) -> &RelationalMechanismCalleeId {
        &self.callee
    }

    pub(crate) const fn invocation_ordinal(&self) -> u32 {
        self.invocation_ordinal
    }

    fn validate_for(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), RelationalMechanismReplayError> {
        self.call_site.validate_for(
            analysis_program,
            Some(RelationalMechanismSiteKind::Expression),
        )?;
        self.callee.validate_for(analysis_program)
    }
}

/// Producer-local activation-path identity. The proposal table owns the path
/// nodes; occurrences retain only this bounded index until normalization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismActivationPathId(u32);

impl RelationalMechanismActivationPathId {
    pub(crate) fn from_index(index: usize) -> Result<Self, RelationalMechanismReplayError> {
        let value =
            u32::try_from(index).map_err(|_| RelationalMechanismReplayError::TraceCapacity {
                resource: "activation path identifiers",
                actual: index,
                limit: u32::MAX as usize,
            })?;
        Ok(Self(value))
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One node in an untrusted parent-linked activation-path proposal. Parent
/// indices are required to precede children; the executor then reorders the
/// trie into content-canonical prefix-first lexicographic order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismActivationPathNode {
    parent: Option<RelationalMechanismActivationPathId>,
    step: RelationalMechanismActivationStep,
}

impl RelationalMechanismActivationPathNode {
    pub(crate) const fn new(
        parent: Option<RelationalMechanismActivationPathId>,
        step: RelationalMechanismActivationStep,
    ) -> Self {
        Self { parent, step }
    }

    pub(crate) const fn parent(&self) -> Option<RelationalMechanismActivationPathId> {
        self.parent
    }

    pub(crate) const fn step(&self) -> &RelationalMechanismActivationStep {
        &self.step
    }
}

/// Producer-local occurrence identity used by root and dependency references.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismOccurrenceId(u32);

impl RelationalMechanismOccurrenceId {
    pub(crate) fn from_index(index: usize) -> Result<Self, RelationalMechanismReplayError> {
        let value =
            u32::try_from(index).map_err(|_| RelationalMechanismReplayError::TraceCapacity {
                resource: "occurrence identifiers",
                actual: index,
                limit: u32::MAX as usize,
            })?;
        Ok(Self(value))
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Kind of a dynamically observed control event.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismEventKind {
    RuleAttempt,
    RuleSelection,
    IfDecision,
    MatchDecision,
    ShortCircuitAnd,
    ShortCircuitOr,
}

impl RelationalMechanismEventKind {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::RuleAttempt => 0x01,
            Self::RuleSelection => 0x02,
            Self::IfDecision => 0x03,
            Self::MatchDecision => 0x04,
            Self::ShortCircuitAnd => 0x05,
            Self::ShortCircuitOr => 0x06,
        }
    }

    const fn required_site_kind(self) -> RelationalMechanismSiteKind {
        match self {
            Self::RuleAttempt => RelationalMechanismSiteKind::RuleCandidate,
            Self::RuleSelection => RelationalMechanismSiteKind::RuleFamily,
            Self::IfDecision
            | Self::MatchDecision
            | Self::ShortCircuitAnd
            | Self::ShortCircuitOr => RelationalMechanismSiteKind::Expression,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::RuleAttempt => "rule_attempt",
            Self::RuleSelection => "rule_selection",
            Self::IfDecision => "if_decision",
            Self::MatchDecision => "match_decision",
            Self::ShortCircuitAnd => "short_circuit_and",
            Self::ShortCircuitOr => "short_circuit_or",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalRuleAttemptOutcome {
    HeadMismatch,
    GuardFalse,
    BodyFalse,
    Applicable,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalRuleSelectionOutcome {
    NoApplicableRule,
    Selected(RelationalMechanismSiteId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalIfDecisionOutcome {
    Then,
    Else,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalShortCircuitOutcome {
    SkippedRight { result: bool },
    EvaluatedRight { result: bool },
}

/// Complete typed outcome of one retained dynamic event.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismEventOutcome {
    RuleAttempt(RelationalRuleAttemptOutcome),
    RuleSelection(RelationalRuleSelectionOutcome),
    IfDecision(RelationalIfDecisionOutcome),
    MatchDecision { arm_index: u32 },
    ShortCircuit(RelationalShortCircuitOutcome),
}

impl RelationalMechanismEventOutcome {
    fn validate_for(
        &self,
        kind: RelationalMechanismEventKind,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), RelationalMechanismReplayError> {
        let compatible = matches!(
            (kind, self),
            (
                RelationalMechanismEventKind::RuleAttempt,
                Self::RuleAttempt(_)
            ) | (
                RelationalMechanismEventKind::RuleSelection,
                Self::RuleSelection(_)
            ) | (
                RelationalMechanismEventKind::IfDecision,
                Self::IfDecision(_)
            ) | (
                RelationalMechanismEventKind::MatchDecision,
                Self::MatchDecision { .. }
            ) | (
                RelationalMechanismEventKind::ShortCircuitAnd
                    | RelationalMechanismEventKind::ShortCircuitOr,
                Self::ShortCircuit(_)
            )
        );
        if !compatible {
            return Err(RelationalMechanismReplayError::TraceOutcomeKindMismatch);
        }
        if let Self::RuleSelection(RelationalRuleSelectionOutcome::Selected(site)) = self {
            site.validate_for(
                analysis_program,
                Some(RelationalMechanismSiteKind::RuleCandidate),
            )?;
        }
        Ok(())
    }
}

/// Outcome-free pairing key for one endpoint-local dynamic occurrence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismOccurrenceSlot {
    root_index: u32,
    activation_path: Arc<[RelationalMechanismActivationStep]>,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    visit_ordinal: u32,
}

impl RelationalMechanismOccurrenceSlot {
    pub(crate) fn new(
        root_index: u32,
        activation_path: impl Into<Arc<[RelationalMechanismActivationStep]>>,
        site: RelationalMechanismSiteId,
        kind: RelationalMechanismEventKind,
        visit_ordinal: u32,
    ) -> Result<Self, RelationalMechanismReplayError> {
        let slot = Self {
            root_index,
            activation_path: activation_path.into(),
            site,
            kind,
            visit_ordinal,
        };
        slot.validate_local()?;
        Ok(slot)
    }

    fn validate_local(&self) -> Result<(), RelationalMechanismReplayError> {
        if self.root_index != 0 {
            return Err(RelationalMechanismReplayError::UnsupportedTraceRoot);
        }
        if self.activation_path.len() > MAX_ACTIVATION_DEPTH {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "activation depth",
                actual: self.activation_path.len(),
                limit: MAX_ACTIVATION_DEPTH,
            });
        }
        if self.site.kind != self.kind.required_site_kind() {
            return Err(RelationalMechanismReplayError::TraceSiteKindMismatch);
        }
        for step in self.activation_path.iter() {
            if step.call_site.analysis_program != self.site.analysis_program
                || step.callee.site().analysis_program != self.site.analysis_program
            {
                return Err(RelationalMechanismReplayError::ForeignTraceSite);
            }
        }
        Ok(())
    }

    pub(crate) const fn root_index(&self) -> u32 {
        self.root_index
    }

    pub(crate) fn activation_path(&self) -> &[RelationalMechanismActivationStep] {
        &self.activation_path
    }

    pub(crate) const fn site(&self) -> &RelationalMechanismSiteId {
        &self.site
    }

    pub(crate) const fn kind(&self) -> RelationalMechanismEventKind {
        self.kind
    }

    pub(crate) const fn visit_ordinal(&self) -> u32 {
        self.visit_ordinal
    }

    fn validate_for(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), RelationalMechanismReplayError> {
        self.validate_local()?;
        self.site
            .validate_for(analysis_program, Some(self.kind.required_site_kind()))?;
        for step in self.activation_path.iter() {
            step.call_site.validate_for(
                analysis_program,
                Some(RelationalMechanismSiteKind::Expression),
            )?;
            step.callee.validate_for(analysis_program)?;
        }
        Ok(())
    }
}

/// Untrusted endpoint-local occurrence proposal returned by the replay seam.
/// Paths and graph edges remain compact producer-local references here; the
/// executor validates and content-normalizes both tables before minting any
/// evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismOccurrenceProposal {
    root_index: u32,
    activation_path: RelationalMechanismActivationPathId,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    visit_ordinal: u32,
    outcome: RelationalMechanismEventOutcome,
    dependencies: Box<[RelationalMechanismOccurrenceId]>,
}

impl RelationalMechanismOccurrenceProposal {
    pub(crate) fn new(
        root_index: u32,
        activation_path: RelationalMechanismActivationPathId,
        site: RelationalMechanismSiteId,
        kind: RelationalMechanismEventKind,
        visit_ordinal: u32,
        outcome: RelationalMechanismEventOutcome,
        dependencies: impl Into<Box<[RelationalMechanismOccurrenceId]>>,
    ) -> Self {
        Self {
            root_index,
            activation_path,
            site,
            kind,
            visit_ordinal,
            outcome,
            dependencies: dependencies.into(),
        }
    }
}

/// Untrusted complete-trace proposal from one fresh endpoint evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismEndpointTraceProposal {
    activation_paths: Box<[RelationalMechanismActivationPathNode]>,
    roots: Box<[RelationalMechanismOccurrenceId]>,
    occurrences: Box<[RelationalMechanismOccurrenceProposal]>,
}

impl RelationalMechanismEndpointTraceProposal {
    pub(crate) fn new(
        activation_paths: impl Into<Box<[RelationalMechanismActivationPathNode]>>,
        roots: impl Into<Box<[RelationalMechanismOccurrenceId]>>,
        occurrences: impl Into<Box<[RelationalMechanismOccurrenceProposal]>>,
    ) -> Self {
        Self {
            activation_paths: activation_paths.into(),
            roots: roots.into(),
            occurrences: occurrences.into(),
        }
    }

    pub(crate) fn empty(root_activation: RelationalMechanismActivationStep) -> Self {
        Self::new(
            vec![RelationalMechanismActivationPathNode::new(
                None,
                root_activation,
            )]
            .into_boxed_slice(),
            Vec::<RelationalMechanismOccurrenceId>::new().into_boxed_slice(),
            Vec::<RelationalMechanismOccurrenceProposal>::new().into_boxed_slice(),
        )
    }
}

/// Canonical, prefix-first activation-trie node. `parent` is a canonical
/// ordinal and therefore always precedes this node. `depth` is validated
/// metadata used only for bounded expansion into the public projection.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalActivationPathNode {
    parent: Option<usize>,
    step: RelationalMechanismActivationStep,
    depth: usize,
}

/// Compact content key for one normalized occurrence. Canonical activation
/// ordinals are prefix-first lexicographic ranks, so derived ordering matches
/// ordering by the full logical path without retaining that path per event.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CompactOccurrenceSlot {
    root_index: u32,
    activation_path: usize,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    visit_ordinal: u32,
}

impl CompactOccurrenceSlot {
    fn validate_for(
        &self,
        activation_path_count: usize,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), RelationalMechanismReplayError> {
        if self.root_index != 0 {
            return Err(RelationalMechanismReplayError::UnsupportedTraceRoot);
        }
        if self.activation_path >= activation_path_count {
            return Err(RelationalMechanismReplayError::MissingActivationPath);
        }
        self.site
            .validate_for(analysis_program, Some(self.kind.required_site_kind()))
    }
}

/// A transient stop. This never mints an unavailable terminal and the caller
/// must leave the case replay work open for a later fresh retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismReplayPause {
    ResourceGovernor,
    TimeLimit,
    Cancellation,
    RetryableRuntime,
}

/// Closed permanent instrumentation failures under this replay ABI. Semantic
/// observer failure after endpoint-totality certification is an integrity
/// error; cancellation, resource pause, timeout, panic, or an integrity
/// disagreement must not use this enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismPermanentUnavailable {
    /// The observer is semantically total, but this replay ABI cannot expose
    /// one of the required trace events.
    ObservationInstrumentationUnsupported,
    /// Complete deterministic replay exceeded a fixed capacity of this replay
    /// ABI. Retrying the same endpoint under the same ABI cannot make progress.
    ReplayAbiCapacityExceeded,
}

impl RelationalMechanismPermanentUnavailable {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::ObservationInstrumentationUnsupported => 0x01,
            Self::ReplayAbiCapacityExceeded => 0x04,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ObservationInstrumentationUnsupported => {
                "observation_instrumentation_unsupported"
            }
            Self::ReplayAbiCapacityExceeded => "replay_abi_capacity_exceeded",
        }
    }
}

/// Result of one isolated runtime call. `Complete` is a trusted assertion that
/// the proposal covers the whole endpoint evaluation; this module then checks
/// its graph structure, site scope, reachability, outcomes, and canonicity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismEndpointReplayProgress {
    Complete(RelationalMechanismEndpointTraceProposal),
    Paused(RelationalMechanismReplayPause),
    PermanentlyUnavailable(RelationalMechanismPermanentUnavailable),
}

/// Borrowed replay command for exactly one endpoint. The executor carries the
/// plan-authorized totality certificate as a runtime handshake; that
/// authorization does not enter request, signature, or mechanism identity.
/// A new command requires a fresh trace result for each endpoint call.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalMechanismEndpointReplayRequest<'a> {
    scope: MechanismRequestScope,
    endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
    observation_id: RelationalMechanismReplayObservationId,
    observation: &'a MechanismObservationIr,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    endpoint: RelationalMechanismEndpoint,
    state: &'a ExploreValue,
    context: &'a ExploreValue,
}

impl<'a> RelationalMechanismEndpointReplayRequest<'a> {
    pub(crate) const fn scope(self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn endpoint_totality_certificate_id(
        self,
    ) -> RelationalEndpointTotalityCertificateId {
        self.endpoint_totality_certificate_id
    }

    pub(crate) const fn observation_id(self) -> RelationalMechanismReplayObservationId {
        self.observation_id
    }

    pub(crate) const fn observation(self) -> &'a MechanismObservationIr {
        self.observation
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn transition_id(self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn endpoint(self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn state(self) -> &'a ExploreValue {
        self.state
    }

    pub(crate) const fn context(self) -> &'a ExploreValue {
        self.context
    }

    pub(crate) const fn state_type(self) -> &'a Ty {
        &self.observation.state_type
    }

    pub(crate) const fn context_type(self) -> &'a Ty {
        &self.observation.context_type
    }

    pub(crate) const fn observation_type(self) -> &'a Ty {
        &self.observation.observation_type
    }
}

/// Trusted fresh-replay adapter. Implementations must evaluate the supplied
/// checked template as `template(state, context)` from empty evaluator and
/// instrumentation state, and must reject a certificate ID they cannot match
/// to the request-scoped checked authorization. They may reuse an immutable
/// complete proposal from an earlier such evaluation only when certificate,
/// checked observation identity, and canonical state/context values are
/// identical; case, transition and endpoint role are rebound and validated by
/// this module. They must return `Complete` only after the originating endpoint
/// evaluation and all instrumentation have finished.
pub(crate) trait RelationalMechanismReplayRuntime {
    type Error;

    fn replay_fresh_endpoint(
        &mut self,
        request: RelationalMechanismEndpointReplayRequest<'_>,
    ) -> Result<RelationalMechanismEndpointReplayProgress, Self::Error>;
}

/// Authenticated, normalized endpoint trace root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismEndpointTraceRoot([u8; 32]);

impl RelationalMechanismEndpointTraceRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedEndpointOccurrence {
    outcome: RelationalMechanismEventOutcome,
    dependencies: BTreeSet<CompactOccurrenceSlot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalEndpointGraph {
    activation_paths: Arc<[CanonicalActivationPathNode]>,
    roots: BTreeSet<CompactOccurrenceSlot>,
    occurrences: BTreeMap<CompactOccurrenceSlot, ValidatedEndpointOccurrence>,
}

/// Typed and structurally validated evidence from one isolated endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismEndpointTraceEvidence {
    endpoint: RelationalMechanismEndpoint,
    observation_id: RelationalMechanismReplayObservationId,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
    root: RelationalMechanismEndpointTraceRoot,
    graph: CanonicalEndpointGraph,
}

impl RelationalMechanismEndpointTraceEvidence {
    pub(crate) const fn endpoint(&self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismReplayObservationId {
        self.observation_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn root(&self) -> RelationalMechanismEndpointTraceRoot {
        self.root
    }

    pub(crate) fn occurrence_count(&self) -> usize {
        self.graph.occurrences.len()
    }

    fn validate_identity(&self) -> Result<(), RelationalMechanismReplayError> {
        validate_canonical_endpoint_graph(&self.graph, None)?;

        let derived = derive_endpoint_trace_root(
            self.endpoint,
            self.observation_id,
            self.case_id,
            self.transition_id,
            self.state_value_digest,
            self.context_value_digest,
            &self.graph,
        )?;
        if derived != self.root {
            return Err(RelationalMechanismReplayError::EndpointTraceRootMismatch);
        }
        Ok(())
    }
}

/// Compact random-access index for the value-free public projection of one
/// canonical mechanism signature. Construction parses and validates the whole
/// definition once. Later publication reads one bounded node, outcome, root,
/// or edge directly from retained byte offsets; it never reparses the graph
/// for every emitted NDJSON record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismSignatureDagIndex {
    signature_id: MechanismSignatureId,
    definition_digest: [u8; 32],
    definition_bytes: usize,
    before: RelationalMechanismEndpointDagIndex,
    after: RelationalMechanismEndpointDagIndex,
    structured_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismEndpointDagSummary {
    node_count: usize,
    root_count: usize,
    edge_count: usize,
}

impl RelationalMechanismEndpointDagSummary {
    pub(crate) const fn node_count(self) -> usize {
        self.node_count
    }

    pub(crate) const fn root_count(self) -> usize {
        self.root_count
    }

    pub(crate) const fn edge_count(self) -> usize {
        self.edge_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismSignatureDagRecord {
    Node {
        endpoint: RelationalMechanismEndpoint,
        node_ordinal: usize,
        slot: RelationalMechanismOccurrenceSlot,
    },
    Outcome {
        endpoint: RelationalMechanismEndpoint,
        node_ordinal: usize,
        event_kind: RelationalMechanismEventKind,
        outcome: RelationalMechanismEventOutcome,
    },
    Root {
        endpoint: RelationalMechanismEndpoint,
        root_ordinal: usize,
        node_ordinal: usize,
    },
    DependencyEdge {
        endpoint: RelationalMechanismEndpoint,
        edge_ordinal: usize,
        dependent_node_ordinal: usize,
        dependency_node_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanismDefinitionByteSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanismDependencyRowIndex {
    ordinals_offset: usize,
    dependency_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalMechanismEndpointDagIndex {
    activation_paths: Arc<[CanonicalActivationPathNode]>,
    node_spans: Box<[MechanismDefinitionByteSpan]>,
    root_ordinals: Box<[usize]>,
    dependency_rows: Box<[MechanismDependencyRowIndex]>,
    edge_record_ends: Box<[u128]>,
    record_count: u128,
}

impl RelationalMechanismSignatureDagIndex {
    pub(crate) fn from_definition(
        definition: &MechanismSignatureDefinition,
        expected_scope: MechanismRequestScope,
    ) -> Result<Self, RelationalMechanismReplayError> {
        index_signature_definition(definition, expected_scope)
    }

    pub(crate) fn before_summary(&self) -> RelationalMechanismEndpointDagSummary {
        self.before.summary()
    }

    pub(crate) fn after_summary(&self) -> RelationalMechanismEndpointDagSummary {
        self.after.summary()
    }

    pub(crate) const fn structured_record_count(&self) -> u128 {
        self.structured_record_count
    }

    pub(crate) fn record_at(
        &self,
        definition: &MechanismSignatureDefinition,
        mut ordinal: u128,
    ) -> Result<Option<RelationalMechanismSignatureDagRecord>, RelationalMechanismReplayError> {
        self.validate_binding(definition)?;
        if ordinal < self.before.record_count {
            return self.before.record_at(
                definition.canonical_definition(),
                RelationalMechanismEndpoint::Before,
                ordinal,
            );
        }
        ordinal -= self.before.record_count;
        if ordinal < self.after.record_count {
            return self.after.record_at(
                definition.canonical_definition(),
                RelationalMechanismEndpoint::After,
                ordinal,
            );
        }
        Ok(None)
    }

    fn validate_binding(
        &self,
        definition: &MechanismSignatureDefinition,
    ) -> Result<(), RelationalMechanismReplayError> {
        if definition.id() != self.signature_id
            || definition.canonical_differential_digest() != self.definition_digest
            || definition.canonical_definition().len() != self.definition_bytes
        {
            return Err(RelationalMechanismReplayError::SignatureDefinitionIdentityMismatch);
        }
        Ok(())
    }
}

impl RelationalMechanismEndpointDagIndex {
    fn summary(&self) -> RelationalMechanismEndpointDagSummary {
        RelationalMechanismEndpointDagSummary {
            node_count: self.node_spans.len(),
            root_count: self.root_ordinals.len(),
            edge_count: self.edge_record_ends.last().copied().unwrap_or(0) as usize,
        }
    }

    fn record_at(
        &self,
        definition: &[u8],
        endpoint: RelationalMechanismEndpoint,
        mut ordinal: u128,
    ) -> Result<Option<RelationalMechanismSignatureDagRecord>, RelationalMechanismReplayError> {
        let node_count = self.node_spans.len() as u128;
        if ordinal < node_count {
            let node_ordinal = usize::try_from(ordinal).map_err(|_| {
                RelationalMechanismReplayError::InvalidDurablePayload("node ordinal")
            })?;
            let (slot, _) = self.decode_node(definition, node_ordinal)?;
            return Ok(Some(RelationalMechanismSignatureDagRecord::Node {
                endpoint,
                node_ordinal,
                slot,
            }));
        }
        ordinal -= node_count;
        if ordinal < node_count {
            let node_ordinal = usize::try_from(ordinal).map_err(|_| {
                RelationalMechanismReplayError::InvalidDurablePayload("outcome ordinal")
            })?;
            let (slot, outcome) = self.decode_node(definition, node_ordinal)?;
            return Ok(Some(RelationalMechanismSignatureDagRecord::Outcome {
                endpoint,
                node_ordinal,
                event_kind: slot.kind,
                outcome,
            }));
        }
        ordinal -= node_count;

        let root_count = self.root_ordinals.len() as u128;
        if ordinal < root_count {
            let root_ordinal = usize::try_from(ordinal).map_err(|_| {
                RelationalMechanismReplayError::InvalidDurablePayload("root ordinal")
            })?;
            return Ok(Some(RelationalMechanismSignatureDagRecord::Root {
                endpoint,
                root_ordinal,
                node_ordinal: self.root_ordinals[root_ordinal],
            }));
        }
        ordinal -= root_count;

        let edge_count = self.edge_record_ends.last().copied().unwrap_or(0);
        if ordinal >= edge_count {
            return Ok(None);
        }
        let dependent_node_ordinal = self.edge_record_ends.partition_point(|end| *end <= ordinal);
        let prior_end = dependent_node_ordinal
            .checked_sub(1)
            .map_or(0, |prior| self.edge_record_ends[prior]);
        let dependency_in_row = usize::try_from(ordinal - prior_end).map_err(|_| {
            RelationalMechanismReplayError::InvalidDurablePayload("dependency ordinal")
        })?;
        let row = self.dependency_rows[dependent_node_ordinal];
        if dependency_in_row >= row.dependency_count {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "dependency row index",
            ));
        }
        let byte_offset = dependency_in_row
            .checked_mul(16)
            .and_then(|offset| row.ordinals_offset.checked_add(offset))
            .ok_or(RelationalMechanismReplayError::InvalidDurablePayload(
                "dependency byte offset",
            ))?;
        let mut reader = PayloadReader::new(definition.get(byte_offset..).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("dependency byte offset"),
        )?);
        let dependency_node_ordinal =
            reader.ordinal(self.node_spans.len(), "trace dependency ordinal")?;
        Ok(Some(
            RelationalMechanismSignatureDagRecord::DependencyEdge {
                endpoint,
                edge_ordinal: usize::try_from(ordinal).map_err(|_| {
                    RelationalMechanismReplayError::InvalidDurablePayload("edge ordinal")
                })?,
                dependent_node_ordinal,
                dependency_node_ordinal,
            },
        ))
    }

    fn decode_node(
        &self,
        definition: &[u8],
        node_ordinal: usize,
    ) -> Result<
        (
            RelationalMechanismOccurrenceSlot,
            RelationalMechanismEventOutcome,
        ),
        RelationalMechanismReplayError,
    > {
        let span = self.node_spans.get(node_ordinal).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("node ordinal"),
        )?;
        let bytes = definition.get(span.start..span.end).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("node byte span"),
        )?;
        let mut reader = PayloadReader::new(bytes);
        let compact = decode_compact_occurrence_slot(&mut reader, self.activation_paths.len())?;
        let outcome =
            decode_event_outcome(&mut reader, compact.kind, &compact.site.analysis_program)?;
        reader.finish()?;
        let slot = materialize_occurrence_slot(&self.activation_paths, &compact)?;
        Ok((slot, outcome))
    }
}

/// Content identity of a successful case-to-signature replay receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismReplayReceiptId([u8; 32]);

impl RelationalMechanismReplayReceiptId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Collision-checkable binding of one reusable mechanism definition to one
/// concrete relational case and semantic transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismReplayReceipt {
    id: RelationalMechanismReplayReceiptId,
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismReplayObservationId,
    relation_id: super::relation::RelationId,
    source_key: super::relation::SourceKey,
    successor_key: super::relation::SuccessorKey,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    state_schema_id: super::transition::StateSchemaId,
    context_schema_id: super::transition::ContextSchemaId,
    transition_type_id: super::transition::TransitionTypeId,
    state_type_digest: [u8; 32],
    context_type_digest: [u8; 32],
    observation_type_digest: [u8; 32],
    before_trace_root: RelationalMechanismEndpointTraceRoot,
    after_trace_root: RelationalMechanismEndpointTraceRoot,
    signature_id: super::mechanism_incidence::MechanismSignatureId,
    signature_definition_digest: [u8; 32],
}

impl RelationalMechanismReplayReceipt {
    pub(crate) const fn id(&self) -> RelationalMechanismReplayReceiptId {
        self.id
    }

    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismReplayObservationId {
        self.observation_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn signature_id(&self) -> super::mechanism_incidence::MechanismSignatureId {
        self.signature_id
    }
}

/// Successful fresh replay ready for collision-checked transition interning
/// and mechanism-incidence recording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismReplayEvidence {
    transition: TransitionInstance,
    definition: MechanismSignatureDefinition,
    receipt: RelationalMechanismReplayReceipt,
    before_trace: RelationalMechanismEndpointTraceEvidence,
    after_trace: RelationalMechanismEndpointTraceEvidence,
}

impl RelationalMechanismReplayEvidence {
    pub(crate) fn transition(&self) -> &TransitionInstance {
        &self.transition
    }

    pub(crate) fn definition(&self) -> &MechanismSignatureDefinition {
        &self.definition
    }

    pub(crate) const fn receipt(&self) -> &RelationalMechanismReplayReceipt {
        &self.receipt
    }

    pub(crate) const fn before_trace(&self) -> &RelationalMechanismEndpointTraceEvidence {
        &self.before_trace
    }

    pub(crate) const fn after_trace(&self) -> &RelationalMechanismEndpointTraceEvidence {
        &self.after_trace
    }

    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.receipt.scope
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismReplayObservationId {
        self.receipt.observation_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.receipt.case_id
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.receipt.transition_id
    }

    pub(crate) const fn signature_id(&self) -> MechanismSignatureId {
        self.receipt.signature_id
    }

    /// Revalidate a producer-minted replay bundle without evaluating policy
    /// code again. This checks every independently reproducible identity and
    /// all cross-links between the transition, case coordinate, endpoint
    /// traces, normalized definition, and private replay receipt.
    pub(crate) fn validate_identity(&self) -> Result<(), RelationalMechanismReplayError> {
        let rehydrated = TransitionInstance::from_canonical_v1(self.transition.canonical_v1())
            .map_err(|_| RelationalMechanismReplayError::InvalidTransitionIdentity)?;
        if rehydrated != self.transition {
            return Err(RelationalMechanismReplayError::InvalidTransitionIdentity);
        }

        validate_signature_definition(&self.definition, self.receipt.scope.request_id())?;
        self.before_trace.validate_identity()?;
        self.after_trace.validate_identity()?;
        if self.before_trace.endpoint != RelationalMechanismEndpoint::Before
            || self.after_trace.endpoint != RelationalMechanismEndpoint::After
        {
            return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch);
        }
        ensure_unambiguous_pairing(&self.before_trace, &self.after_trace)?;

        let receipt = &self.receipt;
        if receipt.transition_id != self.transition.id()
            || receipt.state_schema_id != self.transition.state_schema_id()
            || receipt.context_schema_id != self.transition.context_schema_id()
            || receipt.transition_type_id != self.transition.transition_type_id()
        {
            return Err(RelationalMechanismReplayError::ReplayReceiptTransitionMismatch);
        }

        let empty_provenance = || RelationProvenance::new([], []);
        let source = SourceRow::new(
            self.transition.context().clone(),
            self.transition.before().clone(),
            empty_provenance(),
        );
        let source_key = SourceKey::derive(receipt.relation_id, &source);
        let successor = SuccessorRow::new(self.transition.after().clone(), empty_provenance());
        let successor_key = SuccessorKey::derive(receipt.relation_id, source_key, &successor);
        let case_id = RelationalCaseId::derive(receipt.relation_id, source_key, successor_key);
        if receipt.source_key != source_key
            || receipt.successor_key != successor_key
            || receipt.case_id != case_id
        {
            return Err(RelationalMechanismReplayError::ReplayReceiptCaseMismatch);
        }

        for trace in [&self.before_trace, &self.after_trace] {
            if trace.observation_id != receipt.observation_id
                || trace.case_id != receipt.case_id
                || trace.transition_id != receipt.transition_id
            {
                return Err(RelationalMechanismReplayError::EndpointTraceIdentityMismatch);
            }
        }
        if self.before_trace.state_value_digest
            != canonical_explore_value_digest(self.transition.before())
            || self.after_trace.state_value_digest
                != canonical_explore_value_digest(self.transition.after())
            || self.before_trace.context_value_digest
                != canonical_explore_value_digest(self.transition.context())
            || self.after_trace.context_value_digest
                != canonical_explore_value_digest(self.transition.context())
        {
            return Err(RelationalMechanismReplayError::EndpointTraceValueMismatch);
        }
        if receipt.before_trace_root != self.before_trace.root
            || receipt.after_trace_root != self.after_trace.root
        {
            return Err(RelationalMechanismReplayError::EndpointTraceIdentityMismatch);
        }
        if receipt.signature_id != self.definition.id()
            || receipt.signature_definition_digest
                != self.definition.canonical_differential_digest()
        {
            return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch);
        }

        let derived = derive_replay_receipt_id(receipt, &self.before_trace, &self.after_trace);
        if derived != receipt.id {
            return Err(RelationalMechanismReplayError::ReplayReceiptIdMismatch);
        }
        Ok(())
    }

    /// Canonical, self-contained durable payload used by the bounded analysis
    /// artifact stream. The payload remains private to this module so journal
    /// restoration cannot manufacture unchecked receipt or trace fields.
    pub(crate) fn canonical_durable_payload(
        &self,
    ) -> Result<Box<[u8]>, RelationalMechanismReplayError> {
        self.validate_identity()?;
        let mut encoder = Encoder::bounded(REPLAY_DURABLE_PAYLOAD_V3, MAX_DURABLE_BLOB_BYTES);
        encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
        encode_transition_canonical(&mut encoder, &self.transition.canonical_v1())?;
        encoder.bytes(self.definition.canonical_definition());
        encode_replay_receipt(&mut encoder, &self.receipt);
        let payload = encoder.try_finish()?;
        Ok(payload.into_boxed_slice())
    }

    /// Canonical per-case payload used after the signature definition has
    /// crossed its own durable artifact boundary. The replay receipt binds the
    /// referenced signature ID and definition digest, so repeating the full
    /// endpoint DAG here would add storage without adding evidence.
    pub(crate) fn canonical_compact_incidence_durable_payload(
        &self,
    ) -> Result<Box<[u8]>, RelationalMechanismReplayError> {
        self.validate_identity()?;
        let mut encoder = Encoder::bounded(
            REPLAY_COMPACT_INCIDENCE_DURABLE_PAYLOAD_V3,
            MAX_DURABLE_BLOB_BYTES,
        );
        encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
        encode_transition_canonical(&mut encoder, &self.transition.canonical_v1())?;
        encode_replay_receipt(&mut encoder, &self.receipt);
        let payload = encoder.try_finish()?;
        Ok(payload.into_boxed_slice())
    }

    /// Restore a producer-minted replay bundle from its canonical durable
    /// payload, rederiving the transition, both endpoint DAG roots, signature,
    /// case coordinate, and private receipt before returning the value.
    pub(crate) fn restore_from_durable_payload(
        payload: &[u8],
    ) -> Result<Self, RelationalMechanismReplayError> {
        Self::restore_incidence_from_durable_payload(payload, None)
    }

    /// Restore either the original self-contained replay payload or the
    /// compact incidence payload. Compact restoration is fail-closed unless
    /// the exact request-scoped signature definition is already interned.
    pub(crate) fn restore_incidence_from_durable_payload(
        payload: &[u8],
        interned_definition: Option<&MechanismSignatureDefinition>,
    ) -> Result<Self, RelationalMechanismReplayError> {
        if payload.len() > MAX_DURABLE_BLOB_BYTES {
            return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
                actual: payload.len(),
                limit: MAX_DURABLE_BLOB_BYTES,
            });
        }
        let mut reader = PayloadReader::new(payload);
        let domain = reader.bytes()?;
        if domain != REPLAY_DURABLE_PAYLOAD_V3
            && domain != REPLAY_COMPACT_INCIDENCE_DURABLE_PAYLOAD_V3
        {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "domain separator",
            ));
        }
        reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
        let transition = decode_transition_canonical(&mut reader)?;
        let embedded_definition = if domain == REPLAY_DURABLE_PAYLOAD_V3 {
            Some(reader.owned_bytes()?)
        } else {
            None
        };
        let receipt = decode_replay_receipt(&mut reader)?;
        reader.finish()?;

        let definition = match embedded_definition {
            Some(canonical_definition) => MechanismSignatureDefinition::from_canonical_definition(
                receipt.scope.request_id(),
                canonical_definition,
            ),
            None => interned_definition
                .cloned()
                .ok_or(RelationalMechanismReplayError::InternedSignatureDefinitionRequired)?,
        };
        let (before_graph, after_graph) = decode_signature_endpoint_graphs(
            definition.canonical_definition(),
            &receipt,
            &transition,
        )?;
        let before_trace = RelationalMechanismEndpointTraceEvidence {
            endpoint: RelationalMechanismEndpoint::Before,
            observation_id: receipt.observation_id,
            case_id: receipt.case_id,
            transition_id: receipt.transition_id,
            state_value_digest: canonical_explore_value_digest(transition.before()),
            context_value_digest: canonical_explore_value_digest(transition.context()),
            root: receipt.before_trace_root,
            graph: before_graph,
        };
        let after_trace = RelationalMechanismEndpointTraceEvidence {
            endpoint: RelationalMechanismEndpoint::After,
            observation_id: receipt.observation_id,
            case_id: receipt.case_id,
            transition_id: receipt.transition_id,
            state_value_digest: canonical_explore_value_digest(transition.after()),
            context_value_digest: canonical_explore_value_digest(transition.context()),
            root: receipt.after_trace_root,
            graph: after_graph,
        };
        let evidence = Self {
            transition,
            definition,
            receipt,
            before_trace,
            after_trace,
        };
        evidence.validate_identity()?;
        Ok(evidence)
    }
}

/// Collision-checkable permanent-unavailability evidence. Its reason bytes
/// bind the run ABI, request, observation, endpoint, case, and transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismUnavailableEvidence {
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismReplayObservationId,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    endpoint: RelationalMechanismEndpoint,
    kind: RelationalMechanismPermanentUnavailable,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
    reason_id: MechanismUnavailableReasonId,
    canonical_reason: Box<[u8]>,
}

impl RelationalMechanismUnavailableEvidence {
    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismReplayObservationId {
        self.observation_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn endpoint(&self) -> RelationalMechanismEndpoint {
        self.endpoint
    }

    pub(crate) const fn kind(&self) -> RelationalMechanismPermanentUnavailable {
        self.kind
    }

    pub(crate) const fn reason_id(&self) -> MechanismUnavailableReasonId {
        self.reason_id
    }

    pub(crate) fn canonical_reason(&self) -> &[u8] {
        &self.canonical_reason
    }

    /// Rebuild the exact canonical reason payload and its content identity.
    /// Operational pauses cannot pass this check because they have no
    /// permanent-unavailability kind or producer-minted payload.
    pub(crate) fn validate_identity(&self) -> Result<(), RelationalMechanismReplayError> {
        let canonical_reason = encode_unavailable_reason(
            self.scope,
            self.observation_id,
            self.case_id,
            self.transition_id,
            self.endpoint,
            self.kind,
            self.state_value_digest,
            self.context_value_digest,
        );
        if canonical_reason.as_slice() != self.canonical_reason.as_ref() {
            return Err(RelationalMechanismReplayError::UnavailableReasonPayloadMismatch);
        }
        let derived =
            MechanismUnavailableReasonId::from_canonical_reason_preimage(&canonical_reason);
        if derived != self.reason_id {
            return Err(RelationalMechanismReplayError::UnavailableReasonIdMismatch);
        }
        Ok(())
    }

    /// Restore only from the canonical permanent-reason payload. Operational
    /// pauses have no such payload and therefore cannot cross this boundary.
    pub(crate) fn restore_from_canonical_reason(
        canonical_reason: &[u8],
    ) -> Result<Self, RelationalMechanismReplayError> {
        let mut reader = PayloadReader::new(canonical_reason);
        reader.expect_bytes(UNAVAILABLE_EVIDENCE_V3)?;
        reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
        let scope = decode_scope(&mut reader)?;
        let observation_id = RelationalMechanismReplayObservationId(reader.digest()?);
        let case_id = RelationalCaseId::from_journal_codec_bytes(reader.digest()?);
        let transition_id = TransitionId::from_bytes(reader.digest()?);
        let endpoint = decode_endpoint(reader.tag()?)?;
        let kind = decode_permanent_unavailable(reader.tag()?)?;
        let state_value_digest = reader.digest()?;
        let context_value_digest = reader.digest()?;
        reader.finish()?;
        let canonical_reason = canonical_reason.to_vec().into_boxed_slice();
        let reason_id =
            MechanismUnavailableReasonId::from_canonical_reason_preimage(&canonical_reason);
        let evidence = Self {
            scope,
            observation_id,
            case_id,
            transition_id,
            endpoint,
            kind,
            state_value_digest,
            context_value_digest,
            reason_id,
            canonical_reason,
        };
        evidence.validate_identity()?;
        Ok(evidence)
    }
}

/// One step of resumable case replay. `Paused` contains no semantic terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismReplayOutcome {
    Observed(RelationalMechanismReplayEvidence),
    Paused {
        case_id: RelationalCaseId,
        endpoint: RelationalMechanismEndpoint,
        reason: RelationalMechanismReplayPause,
    },
    PermanentlyUnavailable(RelationalMechanismUnavailableEvidence),
}

/// Error wrapper preserving runtime errors without turning them into durable
/// unavailability claims.
#[derive(Debug)]
pub(crate) enum RelationalMechanismReplayRunError<E> {
    InvalidEvidence(RelationalMechanismReplayError),
    Runtime {
        endpoint: RelationalMechanismEndpoint,
        source: E,
    },
}

impl<E: fmt::Display> fmt::Display for RelationalMechanismReplayRunError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEvidence(error) => fmt::Display::fmt(error, formatter),
            Self::Runtime { endpoint, source } => {
                write!(formatter, "{endpoint:?} mechanism replay failed: {source}")
            }
        }
    }
}

/// Closed structural failures. None should be converted to an unavailable
/// incidence terminal without an explicit higher-level evidence policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismReplayError {
    InvalidCheckedObservation,
    UnsupportedNormalizationVersion {
        actual: u32,
        expected: u32,
    },
    ObservationDependenciesNotClosed,
    OpenObservationType,
    ForeignTraceSite,
    TraceSiteKindMismatch,
    TraceOutcomeKindMismatch,
    UnsupportedTraceRoot,
    TraceRootActivationMismatch,
    MissingActivationPath,
    DuplicateActivationPath,
    UnreachableActivationPath,
    DuplicateTraceRoot,
    DuplicateOccurrence,
    DuplicateDependency,
    MissingTraceRoot,
    MissingDependency,
    CyclicTrace,
    UnreachableOccurrence,
    NonContiguousInvocationOrdinals,
    NonContiguousVisitOrdinals,
    AmbiguousEndpointPairing,
    InvalidTransitionIdentity,
    EndpointTraceRoleMismatch,
    EndpointTraceIdentityMismatch,
    EndpointTraceValueMismatch,
    EndpointTraceRootMismatch,
    SignatureDefinitionIdentityMismatch,
    ReplayReceiptTransitionMismatch,
    ReplayReceiptTypeContractMismatch,
    ReplayReceiptCaseMismatch,
    ReplayReceiptSignatureMismatch,
    ReplayReceiptIdMismatch,
    InternedSignatureDefinitionRequired,
    UnavailableReasonPayloadMismatch,
    UnavailableReasonIdMismatch,
    InvalidDurablePayload(&'static str),
    DurablePayloadCapacity {
        actual: usize,
        limit: usize,
    },
    TraceCapacity {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
}

/// Failure to derive the separately versioned structural quotient from an
/// already interned exact replay signature. Raw replay evidence remains valid
/// when the quotient branch fails closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStructuralMechanismError {
    Replay(RelationalMechanismReplayError),
    Quotient(StructuralMechanismError),
}

impl fmt::Display for RelationalStructuralMechanismError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replay(error) => fmt::Display::fmt(error, formatter),
            Self::Quotient(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for RelationalStructuralMechanismError {}

impl From<RelationalMechanismReplayError> for RelationalStructuralMechanismError {
    fn from(error: RelationalMechanismReplayError) -> Self {
        Self::Replay(error)
    }
}

impl From<StructuralMechanismError> for RelationalStructuralMechanismError {
    fn from(error: StructuralMechanismError) -> Self {
        Self::Quotient(error)
    }
}

impl fmt::Display for RelationalMechanismReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCheckedObservation => {
                formatter.write_str("mechanism replay received an invalid checked observation")
            }
            Self::UnsupportedNormalizationVersion { actual, expected } => write!(
                formatter,
                "unsupported mechanism normalization version {actual}; expected {expected}"
            ),
            Self::ObservationDependenciesNotClosed => formatter.write_str(
                "mechanism observation dependency roots are not the closed v1 template root",
            ),
            Self::OpenObservationType => formatter
                .write_str("mechanism replay observation contains an unresolved checked type"),
            Self::ForeignTraceSite => formatter
                .write_str("mechanism trace site belongs to another checked analysis program"),
            Self::TraceSiteKindMismatch => formatter
                .write_str("mechanism trace event kind disagrees with its checked site kind"),
            Self::TraceOutcomeKindMismatch => formatter
                .write_str("mechanism trace outcome disagrees with its dynamic event kind"),
            Self::UnsupportedTraceRoot => formatter
                .write_str("mechanism trace uses an unsupported observation-root index"),
            Self::TraceRootActivationMismatch => formatter.write_str(
                "mechanism trace root activation disagrees with the checked endpoint template",
            ),
            Self::MissingActivationPath => formatter
                .write_str("mechanism trace references an absent or incomplete activation path"),
            Self::DuplicateActivationPath => formatter
                .write_str("mechanism activation trie repeats one parent-linked path node"),
            Self::UnreachableActivationPath => formatter
                .write_str("mechanism activation trie contains a path unreachable from its root"),
            Self::DuplicateTraceRoot => {
                formatter.write_str("mechanism endpoint trace repeats a root occurrence")
            }
            Self::DuplicateOccurrence => formatter
                .write_str("mechanism endpoint trace contains an ambiguous duplicate occurrence"),
            Self::DuplicateDependency => formatter
                .write_str("mechanism endpoint trace repeats one dependency edge"),
            Self::MissingTraceRoot => formatter
                .write_str("mechanism endpoint roots disagree with occurrence presence"),
            Self::MissingDependency => formatter
                .write_str("mechanism endpoint trace references a missing occurrence"),
            Self::CyclicTrace => {
                formatter.write_str("mechanism endpoint dependency graph contains a cycle")
            }
            Self::UnreachableOccurrence => formatter
                .write_str("mechanism endpoint trace retains an occurrence unreachable from roots"),
            Self::NonContiguousInvocationOrdinals => formatter.write_str(
                "mechanism activation invocation ordinals are not zero-based and contiguous",
            ),
            Self::NonContiguousVisitOrdinals => formatter.write_str(
                "mechanism occurrence visit ordinals are not zero-based and contiguous",
            ),
            Self::AmbiguousEndpointPairing => formatter.write_str(
                "before/after mechanism traces cannot be paired without guessing occurrence correspondence",
            ),
            Self::InvalidTransitionIdentity => formatter.write_str(
                "mechanism replay transition does not reproduce its canonical semantic identity",
            ),
            Self::EndpointTraceRoleMismatch => formatter.write_str(
                "mechanism replay endpoint traces do not occupy the required Before/After roles",
            ),
            Self::EndpointTraceIdentityMismatch => formatter.write_str(
                "mechanism endpoint trace identity disagrees with its replay receipt",
            ),
            Self::EndpointTraceValueMismatch => formatter.write_str(
                "mechanism endpoint trace value digests disagree with the retained transition",
            ),
            Self::EndpointTraceRootMismatch => formatter.write_str(
                "mechanism endpoint trace root does not match its canonical graph payload",
            ),
            Self::SignatureDefinitionIdentityMismatch => formatter.write_str(
                "mechanism signature definition does not reproduce its request-scoped identity",
            ),
            Self::ReplayReceiptTransitionMismatch => formatter.write_str(
                "mechanism replay receipt disagrees with its canonical transition",
            ),
            Self::ReplayReceiptTypeContractMismatch => formatter.write_str(
                "mechanism replay receipt type digests disagree with its signature observation contract",
            ),
            Self::ReplayReceiptCaseMismatch => formatter.write_str(
                "mechanism replay receipt case coordinate disagrees with its transition values",
            ),
            Self::ReplayReceiptSignatureMismatch => formatter.write_str(
                "mechanism replay receipt disagrees with its canonical signature definition",
            ),
            Self::ReplayReceiptIdMismatch => formatter.write_str(
                "mechanism replay receipt ID does not match its complete replay payload",
            ),
            Self::InternedSignatureDefinitionRequired => formatter.write_str(
                "compact mechanism incidence references a signature definition that is not interned",
            ),
            Self::UnavailableReasonPayloadMismatch => formatter.write_str(
                "mechanism unavailable reason bytes disagree with their typed evidence payload",
            ),
            Self::UnavailableReasonIdMismatch => formatter.write_str(
                "mechanism unavailable reason ID disagrees with its canonical reason bytes",
            ),
            Self::InvalidDurablePayload(subject) => write!(
                formatter,
                "mechanism durable payload contains invalid or non-canonical {subject}"
            ),
            Self::DurablePayloadCapacity { actual, limit } => write!(
                formatter,
                "mechanism durable payload needs {actual} bytes; replay ABI limit is {limit}"
            ),
            Self::TraceCapacity {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "mechanism trace needs {actual} {resource}; replay ABI limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for RelationalMechanismReplayError {}

/// Replay one concrete relational case through the same checked endpoint
/// template at Before and After.
///
/// The supplied certificate ID is authorization metadata only. The runtime
/// must match it to the checked request before evaluating either endpoint;
/// signatures remain identities of observed control behavior, not proofs.
///
/// A paused second endpoint intentionally discards the completed first trace;
/// retry starts both endpoint traces fresh. This costs bounded duplicate work
/// but prevents a partial operational checkpoint from masquerading as a
/// durable differential signature.
pub(crate) fn replay_relational_mechanism_case<R: RelationalMechanismReplayRuntime>(
    runtime: &mut R,
    scope: MechanismRequestScope,
    endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
    observation: &MechanismObservationIr,
    schemas: &TransitionSchemaIdentities,
    case: RelationalCaseRef<'_>,
) -> Result<RelationalMechanismReplayOutcome, RelationalMechanismReplayRunError<R::Error>> {
    let observation_id = RelationalMechanismReplayObservationId::derive_checked(observation)
        .map_err(RelationalMechanismReplayRunError::InvalidEvidence)?;
    let transition = schemas.instantiate(
        case.context().clone(),
        case.before().clone(),
        case.after().clone(),
    );
    let transition_id = transition.id();

    let before_request = RelationalMechanismEndpointReplayRequest {
        scope,
        endpoint_totality_certificate_id,
        observation_id,
        observation,
        case_id: case.case_id(),
        transition_id,
        endpoint: RelationalMechanismEndpoint::Before,
        state: case.before(),
        context: case.context(),
    };
    let before = match runtime.replay_fresh_endpoint(before_request) {
        Ok(progress) => progress,
        Err(source) => {
            return Err(RelationalMechanismReplayRunError::Runtime {
                endpoint: RelationalMechanismEndpoint::Before,
                source,
            });
        }
    };
    let before = match before {
        RelationalMechanismEndpointReplayProgress::Complete(proposal) => {
            validate_endpoint_trace(before_request, proposal)
                .map_err(RelationalMechanismReplayRunError::InvalidEvidence)?
        }
        RelationalMechanismEndpointReplayProgress::Paused(reason) => {
            return Ok(RelationalMechanismReplayOutcome::Paused {
                case_id: case.case_id(),
                endpoint: RelationalMechanismEndpoint::Before,
                reason,
            });
        }
        RelationalMechanismEndpointReplayProgress::PermanentlyUnavailable(kind) => {
            return Ok(RelationalMechanismReplayOutcome::PermanentlyUnavailable(
                build_unavailable_evidence(before_request, kind),
            ));
        }
    };

    let after_request = RelationalMechanismEndpointReplayRequest {
        scope,
        endpoint_totality_certificate_id,
        observation_id,
        observation,
        case_id: case.case_id(),
        transition_id,
        endpoint: RelationalMechanismEndpoint::After,
        state: case.after(),
        context: case.context(),
    };
    let after = match runtime.replay_fresh_endpoint(after_request) {
        Ok(progress) => progress,
        Err(source) => {
            return Err(RelationalMechanismReplayRunError::Runtime {
                endpoint: RelationalMechanismEndpoint::After,
                source,
            });
        }
    };
    let after = match after {
        RelationalMechanismEndpointReplayProgress::Complete(proposal) => {
            validate_endpoint_trace(after_request, proposal)
                .map_err(RelationalMechanismReplayRunError::InvalidEvidence)?
        }
        RelationalMechanismEndpointReplayProgress::Paused(reason) => {
            return Ok(RelationalMechanismReplayOutcome::Paused {
                case_id: case.case_id(),
                endpoint: RelationalMechanismEndpoint::After,
                reason,
            });
        }
        RelationalMechanismEndpointReplayProgress::PermanentlyUnavailable(kind) => {
            return Ok(RelationalMechanismReplayOutcome::PermanentlyUnavailable(
                build_unavailable_evidence(after_request, kind),
            ));
        }
    };

    ensure_unambiguous_pairing(&before, &after)
        .map_err(RelationalMechanismReplayRunError::InvalidEvidence)?;
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        emit_counted_structural_quotient_diagnostic(&before.graph, &after.graph);
    }
    let canonical_definition =
        encode_signature_definition(scope, observation_id, observation, schemas, &before, &after)
            .map_err(RelationalMechanismReplayRunError::InvalidEvidence)?;
    let definition = MechanismSignatureDefinition::from_canonical_definition(
        scope.request_id(),
        canonical_definition,
    );
    let receipt = build_replay_receipt(
        scope,
        observation_id,
        observation,
        schemas,
        case,
        &transition,
        &definition,
        &before,
        &after,
    );
    Ok(RelationalMechanismReplayOutcome::Observed(
        RelationalMechanismReplayEvidence {
            transition,
            definition,
            receipt,
            before_trace: before,
            after_trace: after,
        },
    ))
}

fn derive_observation_id(
    observation: &MechanismObservationIr,
) -> Result<RelationalMechanismReplayObservationId, RelationalMechanismReplayError> {
    if observation.normalization_version != 1 {
        return Err(
            RelationalMechanismReplayError::UnsupportedNormalizationVersion {
                actual: observation.normalization_version,
                expected: 1,
            },
        );
    }
    validate_analysis_program(&observation.template_site.analysis_program)?;
    if observation.dependency_roots.len() != 1
        || observation.dependency_roots[0] != observation.template_root
    {
        return Err(RelationalMechanismReplayError::ObservationDependenciesNotClosed);
    }
    for (resource, actual) in [
        (
            "callable structural path",
            observation.endpoint_template.structural_path.len(),
        ),
        (
            "expression AST path",
            observation.template_site.ast_path.len(),
        ),
    ] {
        if actual > MAX_CHECKED_SITE_PATH_ITEMS {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource,
                actual,
                limit: MAX_CHECKED_SITE_PATH_ITEMS,
            });
        }
    }
    let mut type_nodes = 0usize;
    for ty in [
        &observation.state_type,
        &observation.context_type,
        &observation.observation_type,
    ] {
        validate_observation_type_capacity(ty, 0, &mut type_nodes)?;
    }

    let template_site =
        RelationalMechanismSiteId::from_checked_expression(&observation.template_site)?;
    let endpoint_callable = RelationalMechanismSiteId::from_checked_callable(
        &observation.template_site.analysis_program,
        &observation.endpoint_template,
    )?;
    let mut encoder = Encoder::new(OBSERVATION_ID_V3);
    encode_observation_contract(
        &mut encoder,
        observation,
        &template_site,
        &endpoint_callable,
    );
    Ok(RelationalMechanismReplayObservationId(
        Sha256::digest(encoder.finish()).into(),
    ))
}

fn validate_endpoint_trace(
    request: RelationalMechanismEndpointReplayRequest<'_>,
    proposal: RelationalMechanismEndpointTraceProposal,
) -> Result<RelationalMechanismEndpointTraceEvidence, RelationalMechanismReplayError> {
    if proposal.activation_paths.len() > MAX_TRACE_ACTIVATION_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: proposal.activation_paths.len(),
            limit: MAX_TRACE_ACTIVATION_NODES,
        });
    }
    if proposal.occurrences.len() > MAX_TRACE_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "nodes",
            actual: proposal.occurrences.len(),
            limit: MAX_TRACE_NODES,
        });
    }
    if proposal.roots.len() > MAX_TRACE_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "roots",
            actual: proposal.roots.len(),
            limit: MAX_TRACE_NODES,
        });
    }
    let analysis_program = &request.observation.template_site.analysis_program;
    let expected_root_activation = RelationalMechanismActivationStep::new(
        RelationalMechanismSiteId::from_checked_expression(&request.observation.template_site)?,
        RelationalMechanismCalleeId::function(RelationalMechanismSiteId::from_checked_callable(
            analysis_program,
            &request.observation.endpoint_template,
        )?)?,
        0,
    )?;
    let (activation_paths, proposed_to_canonical_path) = normalize_activation_paths(
        proposal.activation_paths,
        analysis_program,
        &expected_root_activation,
    )?;
    let proposed_occurrences = proposal.occurrences.into_vec();

    let mut compact_slots = Vec::new();
    compact_slots
        .try_reserve_exact(proposed_occurrences.len())
        .map_err(|_| RelationalMechanismReplayError::TraceCapacity {
            resource: "nodes",
            actual: proposed_occurrences.len(),
            limit: MAX_TRACE_NODES,
        })?;
    let mut unique_slots = BTreeSet::new();
    for occurrence in &proposed_occurrences {
        if occurrence.root_index != 0 {
            return Err(RelationalMechanismReplayError::UnsupportedTraceRoot);
        }
        let proposed_path = occurrence.activation_path.index();
        let Some(&activation_path) = proposed_to_canonical_path.get(proposed_path) else {
            return Err(RelationalMechanismReplayError::MissingActivationPath);
        };
        occurrence
            .site
            .validate_for(analysis_program, Some(occurrence.kind.required_site_kind()))?;
        occurrence
            .outcome
            .validate_for(occurrence.kind, analysis_program)?;
        let slot = CompactOccurrenceSlot {
            root_index: occurrence.root_index,
            activation_path,
            site: occurrence.site.clone(),
            kind: occurrence.kind,
            visit_ordinal: occurrence.visit_ordinal,
        };
        if !unique_slots.insert(slot.clone()) {
            return Err(RelationalMechanismReplayError::DuplicateOccurrence);
        }
        compact_slots.push(slot);
    }

    let mut roots = BTreeSet::new();
    for root in proposal.roots {
        let Some(slot) = compact_slots.get(root.index()) else {
            return Err(RelationalMechanismReplayError::MissingTraceRoot);
        };
        if !roots.insert(slot.clone()) {
            return Err(RelationalMechanismReplayError::DuplicateTraceRoot);
        }
    }

    let mut occurrences = BTreeMap::new();
    let mut edge_count = 0usize;
    for (occurrence_index, proposal) in proposed_occurrences.into_iter().enumerate() {
        let mut dependencies = BTreeSet::new();
        for dependency in proposal.dependencies {
            edge_count =
                edge_count
                    .checked_add(1)
                    .ok_or(RelationalMechanismReplayError::TraceCapacity {
                        resource: "edges",
                        actual: usize::MAX,
                        limit: MAX_TRACE_EDGES,
                    })?;
            if edge_count > MAX_TRACE_EDGES {
                return Err(RelationalMechanismReplayError::TraceCapacity {
                    resource: "edges",
                    actual: edge_count,
                    limit: MAX_TRACE_EDGES,
                });
            }
            let Some(slot) = compact_slots.get(dependency.index()) else {
                return Err(RelationalMechanismReplayError::MissingDependency);
            };
            if !dependencies.insert(slot.clone()) {
                return Err(RelationalMechanismReplayError::DuplicateDependency);
            }
        }
        occurrences.insert(
            compact_slots[occurrence_index].clone(),
            ValidatedEndpointOccurrence {
                outcome: proposal.outcome,
                dependencies,
            },
        );
    }

    let graph = CanonicalEndpointGraph {
        activation_paths,
        roots,
        occurrences,
    };
    validate_canonical_endpoint_graph(&graph, Some(analysis_program))?;

    let state_value_digest = canonical_explore_value_digest(request.state);
    let context_value_digest = canonical_explore_value_digest(request.context);
    let root = derive_endpoint_trace_root(
        request.endpoint,
        request.observation_id,
        request.case_id,
        request.transition_id,
        state_value_digest,
        context_value_digest,
        &graph,
    )?;

    Ok(RelationalMechanismEndpointTraceEvidence {
        endpoint: request.endpoint,
        observation_id: request.observation_id,
        case_id: request.case_id,
        transition_id: request.transition_id,
        state_value_digest,
        context_value_digest,
        root,
        graph,
    })
}

fn normalize_activation_paths(
    proposed: Box<[RelationalMechanismActivationPathNode]>,
    analysis_program: &AnalysisProgramId,
    expected_root_activation: &RelationalMechanismActivationStep,
) -> Result<(Arc<[CanonicalActivationPathNode]>, Vec<usize>), RelationalMechanismReplayError> {
    if proposed.len() > MAX_TRACE_ACTIVATION_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: proposed.len(),
            limit: MAX_TRACE_ACTIVATION_NODES,
        });
    }
    if proposed.is_empty() {
        return Err(RelationalMechanismReplayError::MissingActivationPath);
    }

    let mut depths = Vec::new();
    depths.try_reserve_exact(proposed.len()).map_err(|_| {
        RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: proposed.len(),
            limit: MAX_TRACE_ACTIVATION_NODES,
        }
    })?;
    let mut children =
        BTreeMap::<Option<usize>, BTreeMap<RelationalMechanismActivationStep, usize>>::new();
    for (ordinal, node) in proposed.iter().enumerate() {
        node.step.validate_for(analysis_program)?;
        let parent = node.parent.map(RelationalMechanismActivationPathId::index);
        let depth = match parent {
            Some(parent) if parent < ordinal => depths[parent] + 1,
            Some(_) => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "activation parent order",
                ));
            }
            None => 1,
        };
        if depth > MAX_ACTIVATION_DEPTH {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "activation depth",
                actual: depth,
                limit: MAX_ACTIVATION_DEPTH,
            });
        }
        depths.push(depth);
        if children
            .entry(parent)
            .or_default()
            .insert(node.step.clone(), ordinal)
            .is_some()
        {
            return Err(RelationalMechanismReplayError::DuplicateActivationPath);
        }
    }

    let roots = children.get(&None).map(BTreeMap::len).unwrap_or(0);
    if roots != 1 {
        return Err(RelationalMechanismReplayError::MissingActivationPath);
    }
    let root = *children
        .get(&None)
        .and_then(|roots| roots.values().next())
        .expect("checked exactly one activation root");
    if &proposed[root].step != expected_root_activation {
        return Err(RelationalMechanismReplayError::TraceRootActivationMismatch);
    }
    let mut proposed_to_canonical = vec![usize::MAX; proposed.len()];
    let mut canonical = Vec::new();
    canonical.try_reserve_exact(proposed.len()).map_err(|_| {
        RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: proposed.len(),
            limit: MAX_TRACE_ACTIVATION_NODES,
        }
    })?;
    let mut stack = vec![root];
    while let Some(proposed_ordinal) = stack.pop() {
        let proposed_node = &proposed[proposed_ordinal];
        let parent = proposed_node
            .parent
            .map(RelationalMechanismActivationPathId::index)
            .map(|parent| proposed_to_canonical[parent]);
        if parent == Some(usize::MAX) {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "activation prefix order",
            ));
        }
        let canonical_ordinal = canonical.len();
        proposed_to_canonical[proposed_ordinal] = canonical_ordinal;
        canonical.push(CanonicalActivationPathNode {
            parent,
            step: proposed_node.step.clone(),
            depth: depths[proposed_ordinal],
        });
        if let Some(descendants) = children.get(&Some(proposed_ordinal)) {
            stack.extend(descendants.values().rev().copied());
        }
    }
    if canonical.len() != proposed.len() {
        return Err(RelationalMechanismReplayError::UnreachableActivationPath);
    }
    validate_complete_activation_invocation_ordinals(&canonical)?;
    Ok((canonical.into(), proposed_to_canonical))
}

#[allow(clippy::too_many_arguments)]
fn derive_endpoint_trace_root(
    endpoint: RelationalMechanismEndpoint,
    observation_id: RelationalMechanismReplayObservationId,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
    graph: &CanonicalEndpointGraph,
) -> Result<RelationalMechanismEndpointTraceRoot, RelationalMechanismReplayError> {
    let mut canonical = Encoder::bounded(ENDPOINT_TRACE_ROOT_V3, MAX_DURABLE_BLOB_BYTES);
    canonical.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
    canonical.tag(endpoint.canonical_tag());
    canonical.digest(observation_id.bytes());
    canonical.digest(case_id.bytes());
    canonical.digest(transition_id.bytes());
    canonical.digest(state_value_digest);
    canonical.digest(context_value_digest);
    encode_endpoint_graph(&mut canonical, graph);
    Ok(RelationalMechanismEndpointTraceRoot(
        Sha256::digest(canonical.try_finish()?).into(),
    ))
}

fn validate_endpoint_dag(
    roots: &BTreeSet<CompactOccurrenceSlot>,
    occurrences: &BTreeMap<CompactOccurrenceSlot, ValidatedEndpointOccurrence>,
) -> Result<(), RelationalMechanismReplayError> {
    let mut remaining = occurrences
        .iter()
        .map(|(slot, occurrence)| (slot.clone(), occurrence.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<CompactOccurrenceSlot, Vec<CompactOccurrenceSlot>>::new();
    for (slot, occurrence) in occurrences {
        for dependency in &occurrence.dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(slot.clone());
        }
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(slot, count)| (*count == 0).then_some(slot.clone()))
        .collect::<BTreeSet<_>>();
    let mut ordered = 0usize;
    while let Some(slot) = ready.pop_first() {
        ordered += 1;
        for dependent in dependents.get(&slot).into_iter().flatten() {
            let count = remaining
                .get_mut(dependent)
                .expect("validated dependent must be present");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    if ordered != occurrences.len() {
        return Err(RelationalMechanismReplayError::CyclicTrace);
    }

    let mut reachable = BTreeSet::new();
    let mut stack = roots.iter().cloned().collect::<Vec<_>>();
    while let Some(slot) = stack.pop() {
        if !reachable.insert(slot.clone()) {
            continue;
        }
        stack.extend(occurrences[&slot].dependencies.iter().cloned());
    }
    if reachable.len() != occurrences.len() {
        return Err(RelationalMechanismReplayError::UnreachableOccurrence);
    }
    Ok(())
}

fn validate_canonical_endpoint_graph(
    graph: &CanonicalEndpointGraph,
    expected_analysis_program: Option<&AnalysisProgramId>,
) -> Result<(), RelationalMechanismReplayError> {
    if graph.activation_paths.len() > MAX_TRACE_ACTIVATION_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: graph.activation_paths.len(),
            limit: MAX_TRACE_ACTIVATION_NODES,
        });
    }
    if graph.occurrences.len() > MAX_TRACE_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "nodes",
            actual: graph.occurrences.len(),
            limit: MAX_TRACE_NODES,
        });
    }
    if graph.roots.len() > MAX_TRACE_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "roots",
            actual: graph.roots.len(),
            limit: MAX_TRACE_NODES,
        });
    }
    if graph.occurrences.is_empty() != graph.roots.is_empty() {
        return Err(RelationalMechanismReplayError::MissingTraceRoot);
    }
    if graph.activation_paths.is_empty() {
        return Err(RelationalMechanismReplayError::MissingActivationPath);
    }

    let inferred_analysis_program = graph
        .activation_paths
        .first()
        .map(|node| &node.step.call_site.analysis_program)
        .or_else(|| {
            graph
                .occurrences
                .keys()
                .next()
                .map(|slot| &slot.site.analysis_program)
        });
    let analysis_program = match (expected_analysis_program, inferred_analysis_program) {
        (Some(expected), Some(inferred)) if expected != inferred => {
            return Err(RelationalMechanismReplayError::ForeignTraceSite);
        }
        (Some(expected), _) => expected,
        (None, Some(inferred)) => inferred,
        (None, None) => return Err(RelationalMechanismReplayError::MissingActivationPath),
    };
    validate_canonical_activation_trie(&graph.activation_paths, analysis_program)?;
    validate_complete_activation_invocation_ordinals(&graph.activation_paths)?;

    let mut edge_count = 0usize;
    for (slot, occurrence) in &graph.occurrences {
        slot.validate_for(graph.activation_paths.len(), analysis_program)?;
        occurrence
            .outcome
            .validate_for(slot.kind, analysis_program)?;
        edge_count = edge_count
            .checked_add(occurrence.dependencies.len())
            .ok_or(RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: usize::MAX,
                limit: MAX_TRACE_EDGES,
            })?;
        if edge_count > MAX_TRACE_EDGES {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: edge_count,
                limit: MAX_TRACE_EDGES,
            });
        }
    }
    if graph
        .roots
        .iter()
        .any(|root| !graph.occurrences.contains_key(root))
    {
        return Err(RelationalMechanismReplayError::MissingTraceRoot);
    }
    if graph.occurrences.values().any(|occurrence| {
        occurrence
            .dependencies
            .iter()
            .any(|dependency| !graph.occurrences.contains_key(dependency))
    }) {
        return Err(RelationalMechanismReplayError::MissingDependency);
    }
    validate_endpoint_dag(&graph.roots, &graph.occurrences)?;
    PairingShape::from_graph(graph)?.validate_contiguous_visit_ordinals()
}

fn validate_canonical_activation_trie(
    paths: &[CanonicalActivationPathNode],
    analysis_program: &AnalysisProgramId,
) -> Result<(), RelationalMechanismReplayError> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut children =
        BTreeMap::<Option<usize>, BTreeMap<RelationalMechanismActivationStep, usize>>::new();
    for (ordinal, node) in paths.iter().enumerate() {
        node.step.validate_for(analysis_program)?;
        let expected_depth = match node.parent {
            Some(parent) if parent < ordinal => paths[parent].depth + 1,
            Some(_) => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "activation parent order",
                ));
            }
            None => 1,
        };
        if node.depth != expected_depth || node.depth > MAX_ACTIVATION_DEPTH {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "activation depth",
            ));
        }
        if children
            .entry(node.parent)
            .or_default()
            .insert(node.step.clone(), ordinal)
            .is_some()
        {
            return Err(RelationalMechanismReplayError::DuplicateActivationPath);
        }
    }
    if children.get(&None).map(BTreeMap::len) != Some(1) {
        return Err(RelationalMechanismReplayError::MissingActivationPath);
    }
    let root = *children
        .get(&None)
        .and_then(|roots| roots.values().next())
        .expect("checked one canonical activation root");
    let mut expected = 0usize;
    let mut stack = vec![root];
    while let Some(ordinal) = stack.pop() {
        if ordinal != expected {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "activation prefix order",
            ));
        }
        expected += 1;
        if let Some(descendants) = children.get(&Some(ordinal)) {
            stack.extend(descendants.values().rev().copied());
        }
    }
    if expected != paths.len() {
        return Err(RelationalMechanismReplayError::UnreachableActivationPath);
    }
    Ok(())
}

fn validate_canonical_root_activation(
    graph: &CanonicalEndpointGraph,
    expected_root_activation: &RelationalMechanismActivationStep,
) -> Result<(), RelationalMechanismReplayError> {
    let root = graph
        .activation_paths
        .first()
        .ok_or(RelationalMechanismReplayError::MissingActivationPath)?;
    if &root.step != expected_root_activation {
        return Err(RelationalMechanismReplayError::TraceRootActivationMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PairingActivationBase {
    call_site: RelationalMechanismSiteId,
    callee: RelationalMechanismCalleeId,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PairingOccurrenceBase {
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
}

/// The raw and canonical activation arenas are complete, including activations
/// which contain no retained mechanism event. Invocation completeness is part
/// of durable endpoint evidence: eventless siblings anchor the actual execution
/// position of every later invocation and therefore may not be sliced here.
fn validate_complete_activation_invocation_ordinals(
    paths: &[CanonicalActivationPathNode],
) -> Result<(), RelationalMechanismReplayError> {
    let mut invocations = BTreeMap::<(Option<usize>, PairingActivationBase), BTreeSet<u32>>::new();
    for path in paths {
        invocations
            .entry((
                path.parent,
                PairingActivationBase {
                    call_site: path.step.call_site.clone(),
                    callee: path.step.callee.clone(),
                },
            ))
            .or_default()
            .insert(path.step.invocation_ordinal);
    }
    if invocations
        .values()
        .any(|ordinals| !ordinals_are_zero_based(ordinals.iter().copied()))
    {
        return Err(RelationalMechanismReplayError::NonContiguousInvocationOrdinals);
    }
    Ok(())
}

#[derive(Default)]
struct PairingShapeNode {
    activations: BTreeMap<PairingActivationBase, BTreeMap<u32, usize>>,
    occurrences: BTreeMap<PairingOccurrenceBase, BTreeSet<u32>>,
}

#[derive(Default)]
struct PairingShape {
    roots: BTreeMap<u32, usize>,
    nodes: Vec<PairingShapeNode>,
}

impl PairingShape {
    fn from_graph(graph: &CanonicalEndpointGraph) -> Result<Self, RelationalMechanismReplayError> {
        let mut shape = Self::default();
        if graph.activation_paths.is_empty() {
            return Ok(shape);
        }
        shape.nodes.push(PairingShapeNode::default());
        shape.roots.insert(0, 0);
        let mut path_nodes = Vec::new();
        path_nodes
            .try_reserve_exact(graph.activation_paths.len())
            .map_err(|_| RelationalMechanismReplayError::TraceCapacity {
                resource: "activation nodes",
                actual: graph.activation_paths.len(),
                limit: MAX_TRACE_ACTIVATION_NODES,
            })?;
        for path in graph.activation_paths.iter() {
            let parent_node = path.parent.map_or(0, |parent| path_nodes[parent]);
            let child_node = shape.nodes.len();
            shape.nodes.push(PairingShapeNode::default());
            if shape.nodes[parent_node]
                .activations
                .entry(PairingActivationBase {
                    call_site: path.step.call_site.clone(),
                    callee: path.step.callee.clone(),
                })
                .or_default()
                .insert(path.step.invocation_ordinal, child_node)
                .is_some()
            {
                return Err(RelationalMechanismReplayError::DuplicateActivationPath);
            }
            path_nodes.push(child_node);
        }
        for slot in graph.occurrences.keys() {
            shape.nodes[path_nodes[slot.activation_path]]
                .occurrences
                .entry(PairingOccurrenceBase {
                    site: slot.site.clone(),
                    kind: slot.kind,
                })
                .or_default()
                .insert(slot.visit_ordinal);
        }
        Ok(shape)
    }

    fn validate_contiguous_visit_ordinals(&self) -> Result<(), RelationalMechanismReplayError> {
        self.nodes
            .iter()
            .try_for_each(|node| node.validate_contiguous_visit_ordinals())
    }

    fn ensure_unambiguous_with(&self, other: &Self) -> Result<(), RelationalMechanismReplayError> {
        for (root, left_node) in &self.roots {
            if let Some(right_node) = other.roots.get(root) {
                self.ensure_nodes_unambiguous_with(*left_node, other, *right_node)?;
            }
        }
        Ok(())
    }

    fn ensure_nodes_unambiguous_with(
        &self,
        left_index: usize,
        other: &Self,
        right_index: usize,
    ) -> Result<(), RelationalMechanismReplayError> {
        let left = &self.nodes[left_index];
        let right = &other.nodes[right_index];
        for (activation, left_invocations) in &left.activations {
            let Some(right_invocations) = right.activations.get(activation) else {
                continue;
            };
            if !left_invocations.keys().eq(right_invocations.keys()) {
                return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
            }
            for (ordinal, left_child) in left_invocations {
                self.ensure_nodes_unambiguous_with(
                    *left_child,
                    other,
                    *right_invocations
                        .get(ordinal)
                        .expect("equal invocation ordinals must exist"),
                )?;
            }
        }
        for (occurrence, left_visits) in &left.occurrences {
            if let Some(right_visits) = right.occurrences.get(occurrence) {
                if left_visits != right_visits {
                    return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
                }
            }
        }
        Ok(())
    }
}

impl PairingShapeNode {
    fn validate_contiguous_visit_ordinals(&self) -> Result<(), RelationalMechanismReplayError> {
        for visits in self.occurrences.values() {
            if !ordinals_are_zero_based(visits.iter().copied()) {
                return Err(RelationalMechanismReplayError::NonContiguousVisitOrdinals);
            }
        }
        Ok(())
    }
}

fn ensure_unambiguous_pairing(
    before: &RelationalMechanismEndpointTraceEvidence,
    after: &RelationalMechanismEndpointTraceEvidence,
) -> Result<(), RelationalMechanismReplayError> {
    PairingShape::from_graph(&before.graph)?
        .ensure_unambiguous_with(&PairingShape::from_graph(&after.graph)?)
}

/// Derive the authoritative V1 structural quotient from one complete raw V3
/// signature. This path reparses and validates the raw definition, establishes
/// exact Before/After correspondence using full activation and visit ordinals,
/// and only then discards dynamic multiplicity from structural identity.
pub(crate) fn derive_relational_structural_mechanism_v1(
    definition: &MechanismSignatureDefinition,
    expected_scope: MechanismRequestScope,
    mut budget: StructuralDerivationBudget,
) -> Result<StructuralSignatureQuotientArtifact, RelationalStructuralMechanismError> {
    // Reject an oversized source before decoding allocates the two restored
    // endpoint graphs. The independent logical-work lane below accounts for
    // every activation, occurrence, and edge before quotient preparation.
    budget.admit_source(definition.canonical_definition().len())?;
    let (before, after) =
        decode_validated_signature_graphs_for_scope(definition, expected_scope, &mut budget)?;
    let input = prepare_structural_paired_input(definition.id(), &before, &after)?;
    Ok(derive_structural_signature_quotient_v1(input, budget)?)
}

fn prepare_structural_paired_input(
    signature_id: MechanismSignatureId,
    before: &CanonicalEndpointGraph,
    after: &CanonicalEndpointGraph,
) -> Result<StructuralPairedDagInputV1, RelationalMechanismReplayError> {
    PairingShape::from_graph(before)?.ensure_unambiguous_with(&PairingShape::from_graph(after)?)?;
    let total_activation_paths = before
        .activation_paths
        .len()
        .checked_add(after.activation_paths.len())
        .ok_or(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural activation nodes",
            actual: usize::MAX,
            limit: MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES,
        })?;
    if total_activation_paths > MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural activation nodes",
            actual: total_activation_paths,
            limit: MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES,
        });
    }
    let total_endpoint_nodes = before
        .occurrences
        .len()
        .checked_add(after.occurrences.len())
        .ok_or(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural endpoint nodes",
            actual: usize::MAX,
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES,
        })?;
    if total_endpoint_nodes > MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural endpoint nodes",
            actual: total_endpoint_nodes,
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES,
        });
    }
    let total_endpoint_edges = diagnostic_endpoint_edge_count(before)
        .map_err(structural_preparation_error)?
        .checked_add(diagnostic_endpoint_edge_count(after).map_err(structural_preparation_error)?)
        .ok_or(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural endpoint edges",
            actual: usize::MAX,
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_EDGES,
        })?;
    if total_endpoint_edges > MAX_STRUCTURAL_QUOTIENT_ENDPOINT_EDGES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "structural endpoint edges",
            actual: total_endpoint_edges,
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_EDGES,
        });
    }

    let mut exact_paths = HashMap::new();
    exact_paths
        .try_reserve(total_activation_paths)
        .map_err(|_| RelationalMechanismReplayError::TraceCapacity {
            resource: "structural exact activation paths",
            actual: total_activation_paths,
            limit: MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES,
        })?;
    let mut static_paths = HashMap::new();
    static_paths
        .try_reserve(total_activation_paths)
        .map_err(|_| RelationalMechanismReplayError::TraceCapacity {
            resource: "structural static activation paths",
            actual: total_activation_paths,
            limit: MAX_STRUCTURAL_QUOTIENT_ACTIVATION_NODES,
        })?;
    let before_path_ids = diagnostic_endpoint_path_ids(before, &mut exact_paths, &mut static_paths)
        .map_err(structural_preparation_error)?;
    let after_path_ids = diagnostic_endpoint_path_ids(after, &mut exact_paths, &mut static_paths)
        .map_err(structural_preparation_error)?;
    let before_prepared = diagnostic_prepare_endpoint(
        RelationalMechanismEndpoint::Before,
        before,
        &before_path_ids,
    )
    .map_err(structural_preparation_error)?;
    let after_prepared =
        diagnostic_prepare_endpoint(RelationalMechanismEndpoint::After, after, &after_path_ids)
            .map_err(structural_preparation_error)?;
    let StructuralQuotientPreparedEndpoint {
        mut rows,
        edges: before_local_edges,
        totals: before_totals,
    } = before_prepared;
    let StructuralQuotientPreparedEndpoint {
        rows: after_rows,
        edges: after_local_edges,
        totals: after_totals,
    } = after_prepared;
    rows.try_reserve(after_rows.len()).map_err(|_| {
        RelationalMechanismReplayError::TraceCapacity {
            resource: "structural pairing rows",
            actual: total_endpoint_nodes,
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES,
        }
    })?;
    rows.extend(after_rows);
    rows.sort_unstable_by(|left, right| {
        left.exact_key
            .cmp(&right.exact_key)
            .then_with(|| left.endpoint.cmp(&right.endpoint))
    });

    let mut before_local_to_raw = vec![usize::MAX; before_totals.nodes];
    let mut after_local_to_raw = vec![usize::MAX; after_totals.nodes];
    let mut occurrences = Vec::new();
    occurrences.try_reserve_exact(rows.len()).map_err(|_| {
        RelationalMechanismReplayError::TraceCapacity {
            resource: "structural paired occurrences",
            actual: rows.len(),
            limit: MAX_STRUCTURAL_QUOTIENT_ENDPOINT_NODES,
        }
    })?;
    let mut row_index = 0usize;
    while row_index < rows.len() {
        let group_start = row_index;
        let raw_ordinal = occurrences.len();
        let mut before_owner_activation = None;
        let mut after_owner_activation = None;
        let mut before_root = false;
        let mut after_root = false;
        let mut before_outcome = None;
        let mut after_outcome = None;
        while row_index < rows.len() && rows[row_index].exact_key == rows[group_start].exact_key {
            let row = &rows[row_index];
            let local_to_raw = match row.endpoint {
                RelationalMechanismEndpoint::Before => {
                    if before_outcome.replace(row.outcome.clone()).is_some() {
                        return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
                    }
                    if before_owner_activation
                        .replace(row.activation_path)
                        .is_some()
                    {
                        return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
                    }
                    before_root = row.is_root;
                    &mut before_local_to_raw
                }
                RelationalMechanismEndpoint::After => {
                    if after_outcome.replace(row.outcome.clone()).is_some() {
                        return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
                    }
                    if after_owner_activation
                        .replace(row.activation_path)
                        .is_some()
                    {
                        return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
                    }
                    after_root = row.is_root;
                    &mut after_local_to_raw
                }
            };
            let destination = local_to_raw.get_mut(row.local_ordinal).ok_or(
                RelationalMechanismReplayError::InvalidDurablePayload(
                    "structural endpoint pairing ordinal",
                ),
            )?;
            if *destination != usize::MAX {
                return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
            }
            *destination = raw_ordinal;
            row_index += 1;
        }
        let exact_key = &rows[group_start].exact_key;
        occurrences.push(StructuralOccurrenceInputV1 {
            before_owner_activation,
            after_owner_activation,
            site: exact_key.site.clone(),
            kind: exact_key.kind,
            before_outcome,
            after_outcome,
            before_root,
            after_root,
        });
    }
    if before_local_to_raw.iter().any(|raw| *raw == usize::MAX)
        || after_local_to_raw.iter().any(|raw| *raw == usize::MAX)
    {
        return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
    }
    let before_edges = map_structural_raw_edges(&before_local_to_raw, &before_local_edges)?;
    let after_edges = map_structural_raw_edges(&after_local_to_raw, &after_local_edges)?;
    let activation_input = |graph: &CanonicalEndpointGraph| {
        graph
            .activation_paths
            .iter()
            .map(|path| StructuralActivationInputV1 {
                parent: path.parent,
                step: path.step.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    };
    Ok(StructuralPairedDagInputV1 {
        signature_id,
        before_activations: activation_input(before),
        after_activations: activation_input(after),
        occurrences: occurrences.into_boxed_slice(),
        before_edges: before_edges.into_boxed_slice(),
        after_edges: after_edges.into_boxed_slice(),
    })
}

fn map_structural_raw_edges(
    local_to_raw: &[usize],
    local_edges: &[(usize, usize)],
) -> Result<Vec<(usize, usize)>, RelationalMechanismReplayError> {
    local_edges
        .iter()
        .map(|(dependent, dependency)| {
            let dependent = *local_to_raw.get(*dependent).ok_or(
                RelationalMechanismReplayError::InvalidDurablePayload(
                    "structural dependent ordinal",
                ),
            )?;
            let dependency = *local_to_raw.get(*dependency).ok_or(
                RelationalMechanismReplayError::InvalidDurablePayload(
                    "structural dependency ordinal",
                ),
            )?;
            if dependent == usize::MAX || dependency == usize::MAX {
                return Err(RelationalMechanismReplayError::AmbiguousEndpointPairing);
            }
            Ok((dependent, dependency))
        })
        .collect()
}

fn structural_preparation_error(subject: &'static str) -> RelationalMechanismReplayError {
    RelationalMechanismReplayError::InvalidDurablePayload(subject)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StructuralQuotientExactActivationPathKey {
    parent: Option<usize>,
    step: RelationalMechanismActivationStep,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StructuralQuotientStaticActivationPathKey {
    parent: Option<usize>,
    call_site: RelationalMechanismSiteId,
    callee: RelationalMechanismCalleeId,
}

struct StructuralQuotientEndpointPathIds {
    exact: Vec<usize>,
    static_checked: Vec<usize>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StructuralQuotientExactOccurrenceKey {
    root_index: u32,
    exact_activation_path: usize,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    visit_ordinal: u32,
}

struct StructuralQuotientEndpointJoinRow {
    exact_key: StructuralQuotientExactOccurrenceKey,
    static_checked_activation_path: usize,
    activation_path: usize,
    endpoint: RelationalMechanismEndpoint,
    local_ordinal: usize,
    outcome: RelationalMechanismEventOutcome,
    is_root: bool,
}

#[derive(Clone, Copy)]
struct StructuralQuotientEndpointTotals {
    nodes: usize,
    roots: usize,
    edges: usize,
}

struct StructuralQuotientPreparedEndpoint {
    rows: Vec<StructuralQuotientEndpointJoinRow>,
    edges: Vec<(usize, usize)>,
    totals: StructuralQuotientEndpointTotals,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StructuralQuotientOccurrenceSeed {
    root_index: u32,
    static_checked_activation_path: usize,
    site: RelationalMechanismSiteId,
    kind: RelationalMechanismEventKind,
    before_root: bool,
    after_root: bool,
    before_outcome: Option<RelationalMechanismEventOutcome>,
    after_outcome: Option<RelationalMechanismEventOutcome>,
}

struct StructuralQuotientRawOccurrence {
    seed: StructuralQuotientOccurrenceSeed,
}

struct StructuralQuotientAdjacency {
    offsets: Vec<usize>,
    dependencies: Vec<usize>,
}

impl StructuralQuotientAdjacency {
    fn dependencies_of(&self, node: usize) -> Result<&[usize], &'static str> {
        let start = *self.offsets.get(node).ok_or("adjacency node is absent")?;
        let end = *self
            .offsets
            .get(node + 1)
            .ok_or("adjacency node end is absent")?;
        self.dependencies
            .get(start..end)
            .ok_or("adjacency span is invalid")
    }
}

struct StructuralQuotientDependencyClassRows {
    spans: Vec<(usize, usize)>,
    classes: Vec<usize>,
}

impl StructuralQuotientDependencyClassRows {
    fn row(&self, node: usize) -> Result<&[usize], &'static str> {
        let (start, end) = *self
            .spans
            .get(node)
            .ok_or("dependency-class row is absent")?;
        self.classes
            .get(start..end)
            .ok_or("dependency-class span is invalid")
    }
}

struct CountedStructuralQuotientClass {
    seed: StructuralQuotientOccurrenceSeed,
    members: usize,
    before_occurrences: usize,
    after_occurrences: usize,
    before_roots: usize,
    after_roots: usize,
}

struct CountedStructuralQuotientEdge {
    dependent_class: usize,
    dependency_class: usize,
    multiplicity: usize,
}

struct CountedStructuralQuotientDiagnostic {
    exact_activation_paths: usize,
    static_checked_activation_paths: usize,
    refinement_rounds: usize,
    raw_union_nodes: usize,
    before_totals: StructuralQuotientEndpointTotals,
    after_totals: StructuralQuotientEndpointTotals,
    classes: Vec<CountedStructuralQuotientClass>,
    before_edges: Vec<CountedStructuralQuotientEdge>,
    after_edges: Vec<CountedStructuralQuotientEdge>,
}

fn emit_counted_structural_quotient_diagnostic(
    before: &CanonicalEndpointGraph,
    after: &CanonicalEndpointGraph,
) {
    let quotient = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        derive_counted_structural_quotient_diagnostic(before, after)
    })) {
        Ok(Ok(quotient)) => quotient,
        Ok(Err(reason)) => {
            eprintln!("[explore trace] mechanism structural quotient diagnostic skipped: {reason}");
            return;
        }
        Err(_) => {
            eprintln!(
                "[explore trace] mechanism structural quotient diagnostic skipped: internal panic"
            );
            return;
        }
    };
    let before_class_count = quotient
        .classes
        .iter()
        .filter(|class| class.before_occurrences != 0)
        .count();
    let after_class_count = quotient
        .classes
        .iter()
        .filter(|class| class.after_occurrences != 0)
        .count();
    let paired_class_count = quotient
        .classes
        .iter()
        .filter(|class| class.before_occurrences != 0 && class.after_occurrences != 0)
        .count();
    let before_root_class_count = quotient
        .classes
        .iter()
        .filter(|class| class.before_roots != 0)
        .count();
    let after_root_class_count = quotient
        .classes
        .iter()
        .filter(|class| class.after_roots != 0)
        .count();
    let max_before_class_multiplicity = quotient
        .classes
        .iter()
        .map(|class| class.before_occurrences)
        .max()
        .unwrap_or(0);
    let max_after_class_multiplicity = quotient
        .classes
        .iter()
        .map(|class| class.after_occurrences)
        .max()
        .unwrap_or(0);
    eprintln!(
        "[explore trace] mechanism structural quotient diagnostic: raw=union:{},before(nodes/roots/edges):{}/{}/{},after(nodes/roots/edges):{}/{}/{}; quotient=classes:{},paths(exact/static):{}/{},rounds:{}; occurrence-classes(before/after/both,max-before/max-after):{}/{}/{},{}/{}; root-classes/uses(before,after):{}/{},{}/{}; edge-classes/uses(before,after):{}/{},{}/{}",
        quotient.raw_union_nodes,
        quotient.before_totals.nodes,
        quotient.before_totals.roots,
        quotient.before_totals.edges,
        quotient.after_totals.nodes,
        quotient.after_totals.roots,
        quotient.after_totals.edges,
        quotient.classes.len(),
        quotient.exact_activation_paths,
        quotient.static_checked_activation_paths,
        quotient.refinement_rounds,
        before_class_count,
        after_class_count,
        paired_class_count,
        max_before_class_multiplicity,
        max_after_class_multiplicity,
        before_root_class_count,
        quotient.before_totals.roots,
        after_root_class_count,
        quotient.after_totals.roots,
        quotient.before_edges.len(),
        quotient.before_totals.edges,
        quotient.after_edges.len(),
        quotient.after_totals.edges,
    );
}

fn derive_counted_structural_quotient_diagnostic(
    before: &CanonicalEndpointGraph,
    after: &CanonicalEndpointGraph,
) -> Result<CountedStructuralQuotientDiagnostic, &'static str> {
    let total_activation_paths = before
        .activation_paths
        .len()
        .checked_add(after.activation_paths.len())
        .ok_or("activation-path count overflow")?;
    if total_activation_paths > MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ACTIVATION_NODES {
        return Err("activation-path capacity exceeded");
    }
    let total_endpoint_nodes = before
        .occurrences
        .len()
        .checked_add(after.occurrences.len())
        .ok_or("endpoint-node count overflow")?;
    if total_endpoint_nodes > MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ENDPOINT_NODES {
        return Err("endpoint-node capacity exceeded");
    }
    let total_endpoint_edges = diagnostic_endpoint_edge_count(before)?
        .checked_add(diagnostic_endpoint_edge_count(after)?)
        .ok_or("endpoint-edge count overflow")?;
    if total_endpoint_edges > MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_ENDPOINT_EDGES {
        return Err("endpoint-edge capacity exceeded");
    }

    let mut exact_paths = HashMap::new();
    exact_paths
        .try_reserve(total_activation_paths)
        .map_err(|_| "exact activation-path allocation failed")?;
    let mut static_checked_paths = HashMap::new();
    static_checked_paths
        .try_reserve(total_activation_paths)
        .map_err(|_| "static activation-path allocation failed")?;
    let before_path_ids =
        diagnostic_endpoint_path_ids(before, &mut exact_paths, &mut static_checked_paths)?;
    let after_path_ids =
        diagnostic_endpoint_path_ids(after, &mut exact_paths, &mut static_checked_paths)?;
    let exact_activation_path_count = exact_paths.len();
    let static_checked_activation_path_count = static_checked_paths.len();
    drop(exact_paths);
    drop(static_checked_paths);

    let before_prepared = diagnostic_prepare_endpoint(
        RelationalMechanismEndpoint::Before,
        before,
        &before_path_ids,
    )?;
    let after_prepared =
        diagnostic_prepare_endpoint(RelationalMechanismEndpoint::After, after, &after_path_ids)?;
    let StructuralQuotientPreparedEndpoint {
        mut rows,
        edges: before_edges,
        totals: before_totals,
    } = before_prepared;
    let StructuralQuotientPreparedEndpoint {
        rows: after_rows,
        edges: after_edges,
        totals: after_totals,
    } = after_prepared;
    rows.try_reserve(after_rows.len())
        .map_err(|_| "paired occurrence-row allocation failed")?;
    rows.extend(after_rows);
    rows.sort_unstable_by(|left, right| {
        left.exact_key
            .cmp(&right.exact_key)
            .then_with(|| left.endpoint.cmp(&right.endpoint))
    });

    let mut before_local_to_raw = Vec::new();
    before_local_to_raw
        .try_reserve_exact(before_totals.nodes)
        .map_err(|_| "before pairing-map allocation failed")?;
    before_local_to_raw.resize(before_totals.nodes, usize::MAX);
    let mut after_local_to_raw = Vec::new();
    after_local_to_raw
        .try_reserve_exact(after_totals.nodes)
        .map_err(|_| "after pairing-map allocation failed")?;
    after_local_to_raw.resize(after_totals.nodes, usize::MAX);
    let mut raw_occurrences = Vec::new();
    raw_occurrences
        .try_reserve_exact(rows.len())
        .map_err(|_| "raw paired-occurrence allocation failed")?;

    let mut row_index = 0usize;
    while row_index < rows.len() {
        let group_start = row_index;
        let raw_ordinal = raw_occurrences.len();
        let mut static_checked_activation_path = None;
        let mut before_root = false;
        let mut after_root = false;
        let mut before_outcome = None;
        let mut after_outcome = None;
        while row_index < rows.len() && rows[row_index].exact_key == rows[group_start].exact_key {
            let row = &rows[row_index];
            match static_checked_activation_path {
                Some(path) if path != row.static_checked_activation_path => {
                    return Err("paired occurrence has conflicting static activation paths");
                }
                Some(_) => {}
                None => static_checked_activation_path = Some(row.static_checked_activation_path),
            }
            let local_to_raw = match row.endpoint {
                RelationalMechanismEndpoint::Before => {
                    if before_outcome.replace(row.outcome.clone()).is_some() {
                        return Err("duplicate before occurrence in exact pairing group");
                    }
                    before_root = row.is_root;
                    &mut before_local_to_raw
                }
                RelationalMechanismEndpoint::After => {
                    if after_outcome.replace(row.outcome.clone()).is_some() {
                        return Err("duplicate after occurrence in exact pairing group");
                    }
                    after_root = row.is_root;
                    &mut after_local_to_raw
                }
            };
            let destination = local_to_raw
                .get_mut(row.local_ordinal)
                .ok_or("endpoint pairing-map ordinal is absent")?;
            if *destination != usize::MAX {
                return Err("endpoint pairing-map ordinal is duplicated");
            }
            *destination = raw_ordinal;
            row_index += 1;
        }
        let exact_key = &rows[group_start].exact_key;
        raw_occurrences.push(StructuralQuotientRawOccurrence {
            seed: StructuralQuotientOccurrenceSeed {
                root_index: exact_key.root_index,
                static_checked_activation_path: static_checked_activation_path
                    .ok_or("paired occurrence has no static activation path")?,
                site: exact_key.site.clone(),
                kind: exact_key.kind,
                before_root,
                after_root,
                before_outcome,
                after_outcome,
            },
        });
    }
    drop(rows);
    if before_local_to_raw
        .iter()
        .any(|ordinal| *ordinal == usize::MAX)
        || after_local_to_raw
            .iter()
            .any(|ordinal| *ordinal == usize::MAX)
    {
        return Err("endpoint pairing map is incomplete");
    }

    let before_adjacency =
        diagnostic_build_adjacency(raw_occurrences.len(), &before_local_to_raw, before_edges)?;
    let after_adjacency =
        diagnostic_build_adjacency(raw_occurrences.len(), &after_local_to_raw, after_edges)?;
    let (class_of, class_count, refinement_rounds) = diagnostic_refine_structural_classes(
        &raw_occurrences,
        &before_adjacency,
        &after_adjacency,
    )?;
    let quotient = diagnostic_build_counted_structural_quotient(
        exact_activation_path_count,
        static_checked_activation_path_count,
        refinement_rounds,
        before_totals,
        after_totals,
        &raw_occurrences,
        &before_adjacency,
        &after_adjacency,
        &class_of,
        class_count,
    )?;
    quotient.validate_conservation()?;
    Ok(quotient)
}

fn diagnostic_endpoint_edge_count(graph: &CanonicalEndpointGraph) -> Result<usize, &'static str> {
    graph
        .occurrences
        .values()
        .try_fold(0usize, |total, occurrence| {
            total
                .checked_add(occurrence.dependencies.len())
                .ok_or("endpoint-edge count overflow")
        })
}

fn diagnostic_endpoint_path_ids(
    graph: &CanonicalEndpointGraph,
    exact_paths: &mut HashMap<StructuralQuotientExactActivationPathKey, usize>,
    static_checked_paths: &mut HashMap<StructuralQuotientStaticActivationPathKey, usize>,
) -> Result<StructuralQuotientEndpointPathIds, &'static str> {
    let mut exact = Vec::new();
    exact
        .try_reserve_exact(graph.activation_paths.len())
        .map_err(|_| "endpoint exact-path ID allocation failed")?;
    let mut static_checked = Vec::new();
    static_checked
        .try_reserve_exact(graph.activation_paths.len())
        .map_err(|_| "endpoint static-path ID allocation failed")?;
    for path in graph.activation_paths.iter() {
        let exact_parent = path
            .parent
            .map(|parent| {
                exact
                    .get(parent)
                    .copied()
                    .ok_or("exact activation-path parent is absent")
            })
            .transpose()?;
        let static_parent = path
            .parent
            .map(|parent| {
                static_checked
                    .get(parent)
                    .copied()
                    .ok_or("static activation-path parent is absent")
            })
            .transpose()?;
        let exact_key = StructuralQuotientExactActivationPathKey {
            parent: exact_parent,
            step: path.step.clone(),
        };
        let exact_id = match exact_paths.get(&exact_key).copied() {
            Some(id) => id,
            None => {
                let id = exact_paths.len();
                exact_paths.insert(exact_key, id);
                id
            }
        };
        let static_key = StructuralQuotientStaticActivationPathKey {
            parent: static_parent,
            call_site: path.step.call_site.clone(),
            callee: path.step.callee.clone(),
        };
        let static_id = match static_checked_paths.get(&static_key).copied() {
            Some(id) => id,
            None => {
                let id = static_checked_paths.len();
                static_checked_paths.insert(static_key, id);
                id
            }
        };
        exact.push(exact_id);
        static_checked.push(static_id);
    }
    Ok(StructuralQuotientEndpointPathIds {
        exact,
        static_checked,
    })
}

fn diagnostic_prepare_endpoint(
    endpoint: RelationalMechanismEndpoint,
    graph: &CanonicalEndpointGraph,
    path_ids: &StructuralQuotientEndpointPathIds,
) -> Result<StructuralQuotientPreparedEndpoint, &'static str> {
    if path_ids.exact.len() != graph.activation_paths.len()
        || path_ids.static_checked.len() != graph.activation_paths.len()
    {
        return Err("endpoint activation-path ID table is incomplete");
    }
    let edge_count = diagnostic_endpoint_edge_count(graph)?;
    let mut ordered_slots = Vec::new();
    ordered_slots
        .try_reserve_exact(graph.occurrences.len())
        .map_err(|_| "ordered endpoint-slot allocation failed")?;
    ordered_slots.extend(graph.occurrences.keys());
    let mut rows = Vec::new();
    rows.try_reserve_exact(graph.occurrences.len())
        .map_err(|_| "endpoint join-row allocation failed")?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(edge_count)
        .map_err(|_| "endpoint local-edge allocation failed")?;
    for (local_ordinal, (slot, occurrence)) in graph.occurrences.iter().enumerate() {
        let exact_activation_path = *path_ids
            .exact
            .get(slot.activation_path)
            .ok_or("endpoint exact activation path is absent")?;
        let static_checked_activation_path = *path_ids
            .static_checked
            .get(slot.activation_path)
            .ok_or("endpoint static activation path is absent")?;
        rows.push(StructuralQuotientEndpointJoinRow {
            exact_key: StructuralQuotientExactOccurrenceKey {
                root_index: slot.root_index,
                exact_activation_path,
                site: slot.site.clone(),
                kind: slot.kind,
                visit_ordinal: slot.visit_ordinal,
            },
            static_checked_activation_path,
            activation_path: slot.activation_path,
            endpoint,
            local_ordinal,
            outcome: occurrence.outcome.clone(),
            is_root: graph.roots.contains(slot),
        });
        for dependency in &occurrence.dependencies {
            let dependency_ordinal = ordered_slots
                .binary_search_by(|candidate| (*candidate).cmp(dependency))
                .map_err(|_| "endpoint dependency has no local ordinal")?;
            edges.push((local_ordinal, dependency_ordinal));
        }
    }
    if edges.len() != edge_count {
        return Err("endpoint edge count changed during preparation");
    }
    Ok(StructuralQuotientPreparedEndpoint {
        rows,
        edges,
        totals: StructuralQuotientEndpointTotals {
            nodes: graph.occurrences.len(),
            roots: graph.roots.len(),
            edges: edge_count,
        },
    })
}

fn diagnostic_build_adjacency(
    raw_node_count: usize,
    local_to_raw: &[usize],
    local_edges: Vec<(usize, usize)>,
) -> Result<StructuralQuotientAdjacency, &'static str> {
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(raw_node_count.saturating_add(1))
        .map_err(|_| "quotient adjacency-offset allocation failed")?;
    offsets.resize(raw_node_count.saturating_add(1), 0usize);
    for (dependent, _) in &local_edges {
        let dependent = *local_to_raw
            .get(*dependent)
            .ok_or("local dependent ordinal is absent")?;
        if dependent == usize::MAX {
            return Err("local dependent ordinal is unpaired");
        }
        let count = offsets
            .get_mut(dependent + 1)
            .ok_or("raw dependent ordinal is absent")?;
        *count = count.checked_add(1).ok_or("adjacency degree overflow")?;
    }
    for index in 1..offsets.len() {
        offsets[index] = offsets[index]
            .checked_add(offsets[index - 1])
            .ok_or("adjacency offset overflow")?;
    }
    if offsets.last().copied().unwrap_or(0) != local_edges.len() {
        return Err("adjacency offset total disagrees with local edges");
    }
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(local_edges.len())
        .map_err(|_| "quotient adjacency allocation failed")?;
    dependencies.resize(local_edges.len(), usize::MAX);
    let mut cursors = Vec::new();
    cursors
        .try_reserve_exact(raw_node_count)
        .map_err(|_| "quotient adjacency-cursor allocation failed")?;
    cursors.extend_from_slice(&offsets[..raw_node_count]);
    for (dependent, dependency) in local_edges {
        let dependent = *local_to_raw
            .get(dependent)
            .ok_or("local dependent ordinal is absent")?;
        let dependency = *local_to_raw
            .get(dependency)
            .ok_or("local dependency ordinal is absent")?;
        if dependent == usize::MAX || dependency == usize::MAX {
            return Err("local edge ordinal is unpaired");
        }
        let destination = *cursors
            .get(dependent)
            .ok_or("raw adjacency cursor is absent")?;
        *dependencies
            .get_mut(destination)
            .ok_or("raw adjacency destination is absent")? = dependency;
        let cursor = cursors
            .get_mut(dependent)
            .ok_or("raw adjacency cursor is absent")?;
        *cursor = cursor
            .checked_add(1)
            .ok_or("raw adjacency cursor overflow")?;
    }
    if dependencies
        .iter()
        .any(|dependency| *dependency == usize::MAX)
    {
        return Err("quotient adjacency is incomplete");
    }
    Ok(StructuralQuotientAdjacency {
        offsets,
        dependencies,
    })
}

fn diagnostic_initial_structural_classes(
    raw_occurrences: &[StructuralQuotientRawOccurrence],
) -> Result<(Vec<usize>, usize), &'static str> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(raw_occurrences.len())
        .map_err(|_| "initial quotient order allocation failed")?;
    order.extend(0..raw_occurrences.len());
    order.sort_unstable_by(|left, right| {
        raw_occurrences[*left]
            .seed
            .cmp(&raw_occurrences[*right].seed)
    });
    let mut class_of = Vec::new();
    class_of
        .try_reserve_exact(raw_occurrences.len())
        .map_err(|_| "initial quotient class allocation failed")?;
    class_of.resize(raw_occurrences.len(), usize::MAX);
    let mut class_count = 0usize;
    let mut previous: Option<usize> = None;
    for node in order {
        if previous
            .is_some_and(|previous| raw_occurrences[previous].seed != raw_occurrences[node].seed)
        {
            class_count = class_count
                .checked_add(1)
                .ok_or("initial quotient class count overflow")?;
        }
        class_of[node] = class_count;
        previous = Some(node);
    }
    if !raw_occurrences.is_empty() {
        class_count = class_count
            .checked_add(1)
            .ok_or("initial quotient class count overflow")?;
    }
    Ok((class_of, class_count))
}

fn diagnostic_dependency_class_rows(
    adjacency: &StructuralQuotientAdjacency,
    class_of: &[usize],
) -> Result<StructuralQuotientDependencyClassRows, &'static str> {
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(class_of.len())
        .map_err(|_| "dependency-class span allocation failed")?;
    let mut classes = Vec::new();
    classes
        .try_reserve_exact(adjacency.dependencies.len())
        .map_err(|_| "dependency-class value allocation failed")?;
    for node in 0..class_of.len() {
        let start = classes.len();
        for dependency in adjacency.dependencies_of(node)? {
            classes.push(
                *class_of
                    .get(*dependency)
                    .ok_or("dependency class is absent")?,
            );
        }
        classes[start..].sort_unstable();
        spans.push((start, classes.len()));
    }
    Ok(StructuralQuotientDependencyClassRows { spans, classes })
}

fn diagnostic_refinement_cmp(
    left: usize,
    right: usize,
    class_of: &[usize],
    before: &StructuralQuotientDependencyClassRows,
    after: &StructuralQuotientDependencyClassRows,
) -> Result<std::cmp::Ordering, &'static str> {
    Ok(class_of
        .get(left)
        .ok_or("left refinement class is absent")?
        .cmp(
            class_of
                .get(right)
                .ok_or("right refinement class is absent")?,
        )
        .then_with(|| {
            before
                .row(left)
                .expect("validated left before dependency-class row")
                .cmp(
                    before
                        .row(right)
                        .expect("validated right before dependency-class row"),
                )
        })
        .then_with(|| {
            after
                .row(left)
                .expect("validated left after dependency-class row")
                .cmp(
                    after
                        .row(right)
                        .expect("validated right after dependency-class row"),
                )
        }))
}

fn diagnostic_refine_structural_classes(
    raw_occurrences: &[StructuralQuotientRawOccurrence],
    before_adjacency: &StructuralQuotientAdjacency,
    after_adjacency: &StructuralQuotientAdjacency,
) -> Result<(Vec<usize>, usize, usize), &'static str> {
    let (mut class_of, mut class_count) = diagnostic_initial_structural_classes(raw_occurrences)?;
    if class_count == raw_occurrences.len() {
        return Ok((class_of, class_count, 0));
    }
    let mut order = Vec::new();
    order
        .try_reserve_exact(raw_occurrences.len())
        .map_err(|_| "refinement order allocation failed")?;
    let mut refinement_rounds = 0usize;
    loop {
        if refinement_rounds >= MAX_STRUCTURAL_QUOTIENT_DIAGNOSTIC_REFINEMENT_ROUNDS {
            return Err("fixed-point refinement limit reached");
        }
        let before_rows = diagnostic_dependency_class_rows(before_adjacency, &class_of)?;
        let after_rows = diagnostic_dependency_class_rows(after_adjacency, &class_of)?;
        order.clear();
        order.extend(0..raw_occurrences.len());
        order.sort_unstable_by(|left, right| {
            diagnostic_refinement_cmp(*left, *right, &class_of, &before_rows, &after_rows)
                .expect("validated quotient refinement rows")
        });
        let mut next_class_of = Vec::new();
        next_class_of
            .try_reserve_exact(raw_occurrences.len())
            .map_err(|_| "refined quotient class allocation failed")?;
        next_class_of.resize(raw_occurrences.len(), usize::MAX);
        let mut next_class_count = 0usize;
        let mut previous: Option<usize> = None;
        for node in order.iter().copied() {
            if let Some(previous) = previous {
                if diagnostic_refinement_cmp(previous, node, &class_of, &before_rows, &after_rows)?
                    != std::cmp::Ordering::Equal
                {
                    next_class_count = next_class_count
                        .checked_add(1)
                        .ok_or("refined quotient class count overflow")?;
                }
            }
            next_class_of[node] = next_class_count;
            previous = Some(node);
        }
        if !raw_occurrences.is_empty() {
            next_class_count = next_class_count
                .checked_add(1)
                .ok_or("refined quotient class count overflow")?;
        }
        refinement_rounds += 1;
        if next_class_of == class_of {
            return Ok((class_of, class_count, refinement_rounds));
        }
        class_of = next_class_of;
        class_count = next_class_count;
        if class_count == raw_occurrences.len() {
            return Ok((class_of, class_count, refinement_rounds));
        }
    }
}

fn diagnostic_count_class_edges(
    adjacency: &StructuralQuotientAdjacency,
    class_of: &[usize],
) -> Result<Vec<CountedStructuralQuotientEdge>, &'static str> {
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(adjacency.dependencies.len())
        .map_err(|_| "quotient edge-pair allocation failed")?;
    for dependent in 0..class_of.len() {
        let dependent_class = *class_of
            .get(dependent)
            .ok_or("dependent quotient class is absent")?;
        for dependency in adjacency.dependencies_of(dependent)? {
            let dependency_class = *class_of
                .get(*dependency)
                .ok_or("dependency quotient class is absent")?;
            pairs.push((dependent_class, dependency_class));
        }
    }
    pairs.sort_unstable();
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(pairs.len())
        .map_err(|_| "counted quotient-edge allocation failed")?;
    let mut index = 0usize;
    while index < pairs.len() {
        let pair = pairs[index];
        let start = index;
        while index < pairs.len() && pairs[index] == pair {
            index += 1;
        }
        edges.push(CountedStructuralQuotientEdge {
            dependent_class: pair.0,
            dependency_class: pair.1,
            multiplicity: index - start,
        });
    }
    Ok(edges)
}

#[allow(clippy::too_many_arguments)]
fn diagnostic_build_counted_structural_quotient(
    exact_activation_paths: usize,
    static_checked_activation_paths: usize,
    refinement_rounds: usize,
    before_totals: StructuralQuotientEndpointTotals,
    after_totals: StructuralQuotientEndpointTotals,
    raw_occurrences: &[StructuralQuotientRawOccurrence],
    before_adjacency: &StructuralQuotientAdjacency,
    after_adjacency: &StructuralQuotientAdjacency,
    class_of: &[usize],
    class_count: usize,
) -> Result<CountedStructuralQuotientDiagnostic, &'static str> {
    if class_of.len() != raw_occurrences.len() {
        return Err("quotient class assignment is incomplete");
    }
    let mut representatives = Vec::new();
    representatives
        .try_reserve_exact(class_count)
        .map_err(|_| "quotient representative allocation failed")?;
    representatives.resize(class_count, usize::MAX);
    for (node, class) in class_of.iter().copied().enumerate() {
        let representative = representatives
            .get_mut(class)
            .ok_or("quotient representative class is absent")?;
        if *representative == usize::MAX {
            *representative = node;
        }
    }
    let mut classes = Vec::new();
    classes
        .try_reserve_exact(class_count)
        .map_err(|_| "counted quotient-class allocation failed")?;
    for representative in representatives {
        let representative = raw_occurrences
            .get(representative)
            .ok_or("quotient representative node is absent")?;
        classes.push(CountedStructuralQuotientClass {
            seed: representative.seed.clone(),
            members: 0,
            before_occurrences: 0,
            after_occurrences: 0,
            before_roots: 0,
            after_roots: 0,
        });
    }
    for (node, class) in class_of.iter().copied().enumerate() {
        let raw = raw_occurrences
            .get(node)
            .ok_or("raw quotient occurrence is absent")?;
        let counted = classes
            .get_mut(class)
            .ok_or("counted quotient class is absent")?;
        counted.members = counted
            .members
            .checked_add(1)
            .ok_or("quotient member multiplicity overflow")?;
        if raw.seed.before_outcome.is_some() {
            counted.before_occurrences = counted
                .before_occurrences
                .checked_add(1)
                .ok_or("before occurrence multiplicity overflow")?;
        }
        if raw.seed.after_outcome.is_some() {
            counted.after_occurrences = counted
                .after_occurrences
                .checked_add(1)
                .ok_or("after occurrence multiplicity overflow")?;
        }
        if raw.seed.before_root {
            counted.before_roots = counted
                .before_roots
                .checked_add(1)
                .ok_or("before root multiplicity overflow")?;
        }
        if raw.seed.after_root {
            counted.after_roots = counted
                .after_roots
                .checked_add(1)
                .ok_or("after root multiplicity overflow")?;
        }
    }
    let before_edges = diagnostic_count_class_edges(before_adjacency, class_of)?;
    let after_edges = diagnostic_count_class_edges(after_adjacency, class_of)?;
    Ok(CountedStructuralQuotientDiagnostic {
        exact_activation_paths,
        static_checked_activation_paths,
        refinement_rounds,
        raw_union_nodes: raw_occurrences.len(),
        before_totals,
        after_totals,
        classes,
        before_edges,
        after_edges,
    })
}

fn diagnostic_checked_sum(values: impl IntoIterator<Item = usize>) -> Result<usize, &'static str> {
    values.into_iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or("quotient conservation sum overflow")
    })
}

impl CountedStructuralQuotientDiagnostic {
    fn validate_conservation(&self) -> Result<(), &'static str> {
        if diagnostic_checked_sum(self.classes.iter().map(|class| class.members))?
            != self.raw_union_nodes
        {
            return Err("quotient member conservation failed");
        }
        if diagnostic_checked_sum(self.classes.iter().map(|class| class.before_occurrences))?
            != self.before_totals.nodes
            || diagnostic_checked_sum(self.classes.iter().map(|class| class.after_occurrences))?
                != self.after_totals.nodes
        {
            return Err("quotient endpoint-node conservation failed");
        }
        if diagnostic_checked_sum(self.classes.iter().map(|class| class.before_roots))?
            != self.before_totals.roots
            || diagnostic_checked_sum(self.classes.iter().map(|class| class.after_roots))?
                != self.after_totals.roots
        {
            return Err("quotient root conservation failed");
        }
        if diagnostic_checked_sum(self.before_edges.iter().map(|edge| edge.multiplicity))?
            != self.before_totals.edges
            || diagnostic_checked_sum(self.after_edges.iter().map(|edge| edge.multiplicity))?
                != self.after_totals.edges
        {
            return Err("quotient edge conservation failed");
        }
        for class in &self.classes {
            let expected_before = if class.seed.before_outcome.is_some() {
                class.members
            } else {
                0
            };
            let expected_after = if class.seed.after_outcome.is_some() {
                class.members
            } else {
                0
            };
            let expected_before_roots = if class.seed.before_root {
                class.members
            } else {
                0
            };
            let expected_after_roots = if class.seed.after_root {
                class.members
            } else {
                0
            };
            if class.before_occurrences != expected_before
                || class.after_occurrences != expected_after
                || class.before_roots != expected_before_roots
                || class.after_roots != expected_after_roots
            {
                return Err("quotient class multiplicity disagrees with its structural seed");
            }
            if class.seed.static_checked_activation_path >= self.static_checked_activation_paths {
                return Err("quotient class static activation path is absent");
            }
        }
        for edge in &self.before_edges {
            let dependent = self
                .classes
                .get(edge.dependent_class)
                .ok_or("before quotient dependent class is absent")?;
            let dependency = self
                .classes
                .get(edge.dependency_class)
                .ok_or("before quotient dependency class is absent")?;
            if dependent.before_occurrences == 0 || dependency.before_occurrences == 0 {
                return Err("before quotient edge crosses an absent endpoint class");
            }
        }
        for edge in &self.after_edges {
            let dependent = self
                .classes
                .get(edge.dependent_class)
                .ok_or("after quotient dependent class is absent")?;
            let dependency = self
                .classes
                .get(edge.dependency_class)
                .ok_or("after quotient dependency class is absent")?;
            if dependent.after_occurrences == 0 || dependency.after_occurrences == 0 {
                return Err("after quotient edge crosses an absent endpoint class");
            }
        }
        Ok(())
    }
}

fn ordinals_are_zero_based(ordinals: impl IntoIterator<Item = u32>) -> bool {
    ordinals
        .into_iter()
        .enumerate()
        .all(|(expected, actual)| usize::try_from(actual) == Ok(expected))
}

fn encode_signature_definition(
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismReplayObservationId,
    observation: &MechanismObservationIr,
    schemas: &TransitionSchemaIdentities,
    before: &RelationalMechanismEndpointTraceEvidence,
    after: &RelationalMechanismEndpointTraceEvidence,
) -> Result<Box<[u8]>, RelationalMechanismReplayError> {
    let mut encoder = Encoder::bounded(SIGNATURE_DEFINITION_V3, MAX_DURABLE_BLOB_BYTES);
    encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
    encoder.u32(observation.normalization_version);
    encode_scope(&mut encoder, scope);
    encoder.digest(observation_id.bytes());
    let template_site =
        RelationalMechanismSiteId::from_checked_expression(&observation.template_site)?;
    let endpoint_callable = RelationalMechanismSiteId::from_checked_callable(
        &observation.template_site.analysis_program,
        &observation.endpoint_template,
    )?;
    encode_observation_contract(
        &mut encoder,
        observation,
        &template_site,
        &endpoint_callable,
    );
    encoder.digest(schemas.state_schema_id().bytes());
    encoder.digest(schemas.context_schema_id().bytes());
    encoder.digest(schemas.transition_type_id().bytes());

    // These are two genuine edge-coloured DAGs. We encode roots, every
    // occurrence slot, every event outcome, and each endpoint dependency set;
    // no discovery ordinal or presentation rule name substitutes for graph
    // content.
    encoder.tag(RelationalMechanismEndpoint::Before.canonical_tag());
    encode_endpoint_graph(&mut encoder, &before.graph);
    encoder.tag(RelationalMechanismEndpoint::After.canonical_tag());
    encode_endpoint_graph(&mut encoder, &after.graph);
    Ok(encoder.try_finish()?.into_boxed_slice())
}

#[allow(clippy::too_many_arguments)]
fn build_replay_receipt(
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismReplayObservationId,
    observation: &MechanismObservationIr,
    schemas: &TransitionSchemaIdentities,
    case: RelationalCaseRef<'_>,
    transition: &TransitionInstance,
    definition: &MechanismSignatureDefinition,
    before: &RelationalMechanismEndpointTraceEvidence,
    after: &RelationalMechanismEndpointTraceEvidence,
) -> RelationalMechanismReplayReceipt {
    let state_type_digest = type_digest(&observation.state_type);
    let context_type_digest = type_digest(&observation.context_type);
    let observation_type_digest = type_digest(&observation.observation_type);
    let mut receipt = RelationalMechanismReplayReceipt {
        id: RelationalMechanismReplayReceiptId([0; 32]),
        scope,
        observation_id,
        relation_id: case.relation_id(),
        source_key: case.source_key(),
        successor_key: case.successor_key(),
        case_id: case.case_id(),
        transition_id: transition.id(),
        state_schema_id: schemas.state_schema_id(),
        context_schema_id: schemas.context_schema_id(),
        transition_type_id: schemas.transition_type_id(),
        state_type_digest,
        context_type_digest,
        observation_type_digest,
        before_trace_root: before.root,
        after_trace_root: after.root,
        signature_id: definition.id(),
        signature_definition_digest: definition.canonical_differential_digest(),
    };
    receipt.id = derive_replay_receipt_id(&receipt, before, after);
    receipt
}

fn derive_replay_receipt_id(
    receipt: &RelationalMechanismReplayReceipt,
    before: &RelationalMechanismEndpointTraceEvidence,
    after: &RelationalMechanismEndpointTraceEvidence,
) -> RelationalMechanismReplayReceiptId {
    let mut encoder = Encoder::new(REPLAY_RECEIPT_ID_V3);
    encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
    encode_scope(&mut encoder, receipt.scope);
    encoder.digest(receipt.observation_id.bytes());
    encoder.digest(receipt.relation_id.bytes());
    encoder.digest(receipt.source_key.bytes());
    encoder.digest(receipt.successor_key.bytes());
    encoder.digest(receipt.case_id.bytes());
    encoder.digest(receipt.transition_id.bytes());
    encoder.digest(receipt.state_schema_id.bytes());
    encoder.digest(receipt.context_schema_id.bytes());
    encoder.digest(receipt.transition_type_id.bytes());
    encoder.digest(receipt.state_type_digest);
    encoder.digest(receipt.context_type_digest);
    encoder.digest(receipt.observation_type_digest);
    encoder.digest(receipt.before_trace_root.bytes());
    encoder.digest(receipt.after_trace_root.bytes());
    encoder.digest(before.state_value_digest);
    encoder.digest(after.state_value_digest);
    encoder.digest(before.context_value_digest);
    encoder.digest(after.context_value_digest);
    encoder.digest(receipt.signature_id.bytes());
    encoder.digest(receipt.signature_definition_digest);
    RelationalMechanismReplayReceiptId(Sha256::digest(encoder.finish()).into())
}

fn validate_signature_definition(
    definition: &MechanismSignatureDefinition,
    request_id: super::relation::MechanismRequestId,
) -> Result<(), RelationalMechanismReplayError> {
    if definition.canonical_definition().len() > MAX_DURABLE_BLOB_BYTES {
        return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: definition.canonical_definition().len(),
            limit: MAX_DURABLE_BLOB_BYTES,
        });
    }
    let digest: [u8; 32] = Sha256::digest(definition.canonical_definition()).into();
    if digest != definition.canonical_differential_digest()
        || definition.id().request_id() != request_id
        || MechanismSignatureId::from_canonical_differential_signature_digest(request_id, digest)
            != definition.id()
    {
        return Err(RelationalMechanismReplayError::SignatureDefinitionIdentityMismatch);
    }
    Ok(())
}

fn build_unavailable_evidence(
    request: RelationalMechanismEndpointReplayRequest<'_>,
    kind: RelationalMechanismPermanentUnavailable,
) -> RelationalMechanismUnavailableEvidence {
    let state_value_digest = canonical_explore_value_digest(request.state);
    let context_value_digest = canonical_explore_value_digest(request.context);
    let canonical_reason = encode_unavailable_reason(
        request.scope,
        request.observation_id,
        request.case_id,
        request.transition_id,
        request.endpoint,
        kind,
        state_value_digest,
        context_value_digest,
    )
    .into_boxed_slice();
    let reason_id = MechanismUnavailableReasonId::from_canonical_reason_preimage(&canonical_reason);
    RelationalMechanismUnavailableEvidence {
        scope: request.scope,
        observation_id: request.observation_id,
        case_id: request.case_id,
        transition_id: request.transition_id,
        endpoint: request.endpoint,
        kind,
        state_value_digest,
        context_value_digest,
        reason_id,
        canonical_reason,
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_unavailable_reason(
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismReplayObservationId,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    endpoint: RelationalMechanismEndpoint,
    kind: RelationalMechanismPermanentUnavailable,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
) -> Vec<u8> {
    let mut encoder = Encoder::new(UNAVAILABLE_EVIDENCE_V3);
    encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
    encode_scope(&mut encoder, scope);
    encoder.digest(observation_id.bytes());
    encoder.digest(case_id.bytes());
    encoder.digest(transition_id.bytes());
    encoder.tag(endpoint.canonical_tag());
    encoder.tag(kind.canonical_tag());
    encoder.digest(state_value_digest);
    encoder.digest(context_value_digest);
    encoder.finish()
}

fn encode_endpoint_graph(encoder: &mut Encoder, graph: &CanonicalEndpointGraph) {
    // Activation paths are a canonical prefix trie. Each parent-linked step is
    // written once; occurrence nodes retain only the canonical path ordinal.
    encoder.u128(graph.activation_paths.len() as u128);
    for path in graph.activation_paths.iter() {
        match path.parent {
            None => encoder.tag(0x00),
            Some(parent) => {
                encoder.tag(0x01);
                encoder.u128(parent as u128);
            }
        }
        encode_activation_step(encoder, &path.step);
    }

    let ordinals = graph
        .occurrences
        .keys()
        .enumerate()
        .map(|(ordinal, slot)| (slot, ordinal as u128))
        .collect::<BTreeMap<_, _>>();
    encoder.u128(graph.occurrences.len() as u128);
    for (slot, occurrence) in &graph.occurrences {
        encode_compact_occurrence_slot(encoder, slot);
        encode_event_outcome(encoder, &occurrence.outcome);
    }
    encoder.u128(graph.roots.len() as u128);
    for root in &graph.roots {
        encoder.u128(
            *ordinals
                .get(&root)
                .expect("validated endpoint root must have a canonical node ordinal"),
        );
    }
    encoder.u128(graph.occurrences.len() as u128);
    for occurrence in graph.occurrences.values() {
        encoder.u128(occurrence.dependencies.len() as u128);
        for dependency in &occurrence.dependencies {
            encoder.u128(
                *ordinals
                    .get(&dependency)
                    .expect("validated endpoint dependency must have a canonical node ordinal"),
            );
        }
    }
}

fn encode_activation_step(encoder: &mut Encoder, step: &RelationalMechanismActivationStep) {
    encode_site(encoder, &step.call_site);
    encoder.tag(step.callee.canonical_tag());
    encode_site(encoder, step.callee.site());
    encoder.u32(step.invocation_ordinal);
}

fn encode_compact_occurrence_slot(encoder: &mut Encoder, slot: &CompactOccurrenceSlot) {
    encoder.u32(slot.root_index);
    encoder.u128(slot.activation_path as u128);
    encode_site(encoder, &slot.site);
    encoder.tag(slot.kind.canonical_tag());
    encoder.u32(slot.visit_ordinal);
}

fn encode_event_outcome(encoder: &mut Encoder, outcome: &RelationalMechanismEventOutcome) {
    match outcome {
        RelationalMechanismEventOutcome::RuleAttempt(outcome) => {
            encoder.tag(0x01);
            encoder.tag(match outcome {
                RelationalRuleAttemptOutcome::HeadMismatch => 0x01,
                RelationalRuleAttemptOutcome::GuardFalse => 0x02,
                RelationalRuleAttemptOutcome::BodyFalse => 0x03,
                RelationalRuleAttemptOutcome::Applicable => 0x04,
            });
        }
        RelationalMechanismEventOutcome::RuleSelection(outcome) => {
            encoder.tag(0x02);
            match outcome {
                RelationalRuleSelectionOutcome::NoApplicableRule => encoder.tag(0x01),
                RelationalRuleSelectionOutcome::Selected(site) => {
                    encoder.tag(0x02);
                    encode_site(encoder, site);
                }
            }
        }
        RelationalMechanismEventOutcome::IfDecision(outcome) => {
            encoder.tag(0x03);
            encoder.tag(match outcome {
                RelationalIfDecisionOutcome::Then => 0x01,
                RelationalIfDecisionOutcome::Else => 0x02,
            });
        }
        RelationalMechanismEventOutcome::MatchDecision { arm_index } => {
            encoder.tag(0x04);
            encoder.u32(*arm_index);
        }
        RelationalMechanismEventOutcome::ShortCircuit(outcome) => {
            encoder.tag(0x05);
            match outcome {
                RelationalShortCircuitOutcome::SkippedRight { result } => {
                    encoder.tag(0x01);
                    encoder.bool(*result);
                }
                RelationalShortCircuitOutcome::EvaluatedRight { result } => {
                    encoder.tag(0x02);
                    encoder.bool(*result);
                }
            }
        }
    }
}

fn encode_scope(encoder: &mut Encoder, scope: MechanismRequestScope) {
    encoder.digest(scope.request_id().bytes());
    encoder.digest(scope.question_id().bytes());
    match scope.target() {
        MechanismTargetId::Selected => encoder.tag(0x01),
        MechanismTargetId::ChosenView(view_id) => {
            encoder.tag(0x02);
            encoder.digest(view_id.bytes());
        }
    }
}

fn encode_observation_contract(
    encoder: &mut Encoder,
    observation: &MechanismObservationIr,
    template_site: &RelationalMechanismSiteId,
    endpoint_callable: &RelationalMechanismSiteId,
) {
    encoder.u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION);
    encoder.u32(observation.normalization_version);
    encode_site(encoder, template_site);
    encode_site(encoder, endpoint_callable);
    encode_callable_id(encoder, &observation.endpoint_template);
    encode_expression_site(encoder, &observation.template_site);
    encode_ty(encoder, &observation.state_type);
    encode_ty(encoder, &observation.context_type);
    encode_ty(encoder, &observation.observation_type);
    // DynamicControl v1 admits exactly the checked template root. Encoding
    // the independently derived template site plus this cardinality therefore
    // commits the complete dependency-root set without peeking through the
    // legacy root wrapper's private representation.
    encoder.u128(observation.dependency_roots.len() as u128);
}

fn encode_site(encoder: &mut Encoder, site: &RelationalMechanismSiteId) {
    encoder.bytes(site.analysis_program.as_str().as_bytes());
    encoder.tag(site.kind.canonical_tag());
    encoder.digest(site.digest);
}

fn encode_callable_id(encoder: &mut Encoder, callable: &CheckedCallableId) {
    encoder.bytes(callable.declaration.declaration.semantic_key().as_bytes());
    encoder.u128(callable.declaration.declaration_occurrence_ordinal as u128);
    encoder.u128(callable.declaration.normalized_ordinal as u128);
    encoder.u128(callable.structural_path.len() as u128);
    for child in &callable.structural_path {
        encoder.u32(*child);
    }
}

fn encode_expression_site(encoder: &mut Encoder, site: &ExprSiteId) {
    encoder.bytes(site.analysis_program.as_str().as_bytes());
    encoder.bytes(site.declaration.semantic_key().as_bytes());
    encoder.u128(site.normalized_declaration_ordinal as u128);
    encoder.u128(site.ast_path.len() as u128);
    for child in &site.ast_path {
        encoder.u32(*child);
    }
}

fn type_digest(ty: &Ty) -> [u8; 32] {
    let mut encoder = Encoder::new(MECHANISM_TYPE_V3);
    encode_ty(&mut encoder, ty);
    Sha256::digest(encoder.finish()).into()
}

fn encode_ty(encoder: &mut Encoder, ty: &Ty) {
    match ty {
        Ty::Name(name) => {
            encoder.tag(0x01);
            encoder.bytes(name.as_bytes());
        }
        Ty::App(constructor, arguments) => {
            encoder.tag(0x02);
            encode_ty(encoder, constructor);
            encoder.u128(arguments.len() as u128);
            for argument in arguments {
                encode_ty(encoder, argument);
            }
        }
        Ty::Arrow(parameter, result) => {
            encoder.tag(0x03);
            encode_ty(encoder, parameter);
            encode_ty(encoder, result);
        }
        Ty::Ref(inner) => {
            encoder.tag(0x04);
            encode_ty(encoder, inner);
        }
        Ty::MutRef(inner) => {
            encoder.tag(0x05);
            encode_ty(encoder, inner);
        }
        Ty::Shared(inner) => {
            encoder.tag(0x06);
            encode_ty(encoder, inner);
        }
        Ty::Optional(inner) => {
            encoder.tag(0x07);
            encode_ty(encoder, inner);
        }
        Ty::Var(name) => {
            encoder.tag(0x08);
            encoder.bytes(name.as_bytes());
        }
        Ty::Unit => encoder.tag(0x09),
        Ty::Hole => encoder.tag(0x0a),
    }
}

fn validate_observation_type_capacity(
    ty: &Ty,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), RelationalMechanismReplayError> {
    if depth > MAX_DURABLE_VALUE_DEPTH {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "observation type depth",
            actual: depth,
            limit: MAX_DURABLE_VALUE_DEPTH,
        });
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or(RelationalMechanismReplayError::TraceCapacity {
            resource: "observation type nodes",
            actual: usize::MAX,
            limit: MAX_DURABLE_VALUE_NODES,
        })?;
    if *nodes > MAX_DURABLE_VALUE_NODES {
        return Err(RelationalMechanismReplayError::TraceCapacity {
            resource: "observation type nodes",
            actual: *nodes,
            limit: MAX_DURABLE_VALUE_NODES,
        });
    }
    match ty {
        Ty::Name(_) | Ty::Unit => Ok(()),
        Ty::App(constructor, arguments) => {
            validate_observation_type_capacity(constructor, depth + 1, nodes)?;
            for argument in arguments {
                validate_observation_type_capacity(argument, depth + 1, nodes)?;
            }
            Ok(())
        }
        Ty::Arrow(parameter, result) => {
            validate_observation_type_capacity(parameter, depth + 1, nodes)?;
            validate_observation_type_capacity(result, depth + 1, nodes)
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
            validate_observation_type_capacity(inner, depth + 1, nodes)
        }
        Ty::Var(_) | Ty::Hole => Err(RelationalMechanismReplayError::OpenObservationType),
    }
}

fn validate_analysis_program(
    analysis_program: &AnalysisProgramId,
) -> Result<(), RelationalMechanismReplayError> {
    let value = analysis_program.as_str();
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(RelationalMechanismReplayError::InvalidCheckedObservation);
    }
    Ok(())
}

fn encode_transition_canonical(
    encoder: &mut Encoder,
    transition: &TransitionInstanceCanonicalV1,
) -> Result<(), RelationalMechanismReplayError> {
    encoder.digest(transition.state_schema_id().bytes());
    encoder.digest(transition.context_schema_id().bytes());
    encoder.digest(transition.transition_type_id().bytes());
    encoder.digest(transition.before_state_id().bytes());
    encoder.digest(transition.after_state_id().bytes());
    encoder.digest(transition.transition_id().bytes());
    encoder.bytes(transition.state_schema_preimage());
    encoder.bytes(transition.context_schema_preimage());
    encoder.bytes(transition.transition_type_preimage());
    encode_durable_value(encoder, transition.context(), 0)?;
    encode_durable_value(encoder, transition.before(), 0)?;
    encode_durable_value(encoder, transition.after(), 0)
}

fn decode_transition_canonical(
    reader: &mut PayloadReader<'_>,
) -> Result<TransitionInstance, RelationalMechanismReplayError> {
    let state_schema_id = StateSchemaId::from_bytes(reader.digest()?);
    let context_schema_id = ContextSchemaId::from_bytes(reader.digest()?);
    let transition_type_id = TransitionTypeId::from_bytes(reader.digest()?);
    let before_state_id = StateId::from_bytes(reader.digest()?);
    let after_state_id = StateId::from_bytes(reader.digest()?);
    let transition_id = TransitionId::from_bytes(reader.digest()?);
    let state_schema_preimage = reader.owned_bytes()?;
    let context_schema_preimage = reader.owned_bytes()?;
    let transition_type_preimage = reader.owned_bytes()?;
    let context = decode_durable_value(reader, 0)?;
    let before = decode_durable_value(reader, 0)?;
    let after = decode_durable_value(reader, 0)?;
    TransitionInstance::from_canonical_v1(TransitionInstanceCanonicalV1::new(
        state_schema_id,
        context_schema_id,
        transition_type_id,
        before_state_id,
        after_state_id,
        transition_id,
        state_schema_preimage,
        context_schema_preimage,
        transition_type_preimage,
        context,
        before,
        after,
    ))
    .map_err(|_| RelationalMechanismReplayError::InvalidTransitionIdentity)
}

fn encode_replay_receipt(encoder: &mut Encoder, receipt: &RelationalMechanismReplayReceipt) {
    encoder.digest(receipt.id.bytes());
    encode_scope(encoder, receipt.scope);
    encoder.digest(receipt.observation_id.bytes());
    encoder.digest(receipt.relation_id.bytes());
    encoder.digest(receipt.source_key.bytes());
    encoder.digest(receipt.successor_key.bytes());
    encoder.digest(receipt.case_id.bytes());
    encoder.digest(receipt.transition_id.bytes());
    encoder.digest(receipt.state_schema_id.bytes());
    encoder.digest(receipt.context_schema_id.bytes());
    encoder.digest(receipt.transition_type_id.bytes());
    encoder.digest(receipt.state_type_digest);
    encoder.digest(receipt.context_type_digest);
    encoder.digest(receipt.observation_type_digest);
    encoder.digest(receipt.before_trace_root.bytes());
    encoder.digest(receipt.after_trace_root.bytes());
    encoder.digest(receipt.signature_id.bytes());
    encoder.digest(receipt.signature_definition_digest);
}

fn decode_replay_receipt(
    reader: &mut PayloadReader<'_>,
) -> Result<RelationalMechanismReplayReceipt, RelationalMechanismReplayError> {
    let id = RelationalMechanismReplayReceiptId(reader.digest()?);
    let scope = decode_scope(reader)?;
    Ok(RelationalMechanismReplayReceipt {
        id,
        scope,
        observation_id: RelationalMechanismReplayObservationId(reader.digest()?),
        relation_id: super::relation::RelationId::from_journal_codec_bytes(reader.digest()?),
        source_key: SourceKey::from_journal_codec_bytes(reader.digest()?),
        successor_key: SuccessorKey::from_journal_codec_bytes(reader.digest()?),
        case_id: RelationalCaseId::from_journal_codec_bytes(reader.digest()?),
        transition_id: TransitionId::from_bytes(reader.digest()?),
        state_schema_id: StateSchemaId::from_bytes(reader.digest()?),
        context_schema_id: ContextSchemaId::from_bytes(reader.digest()?),
        transition_type_id: TransitionTypeId::from_bytes(reader.digest()?),
        state_type_digest: reader.digest()?,
        context_type_digest: reader.digest()?,
        observation_type_digest: reader.digest()?,
        before_trace_root: RelationalMechanismEndpointTraceRoot(reader.digest()?),
        after_trace_root: RelationalMechanismEndpointTraceRoot(reader.digest()?),
        signature_id: MechanismSignatureId::from_journal_codec_parts(
            scope.request_id(),
            reader.digest()?,
        ),
        signature_definition_digest: reader.digest()?,
    })
}

fn decode_scope(
    reader: &mut PayloadReader<'_>,
) -> Result<MechanismRequestScope, RelationalMechanismReplayError> {
    let request_id =
        super::relation::MechanismRequestId::from_journal_codec_bytes(reader.digest()?);
    let question_id = QuestionId::from_journal_codec_bytes(reader.digest()?);
    let target = match reader.tag()? {
        0x01 => MechanismTargetId::Selected,
        0x02 => MechanismTargetId::ChosenView(ViewId::from_journal_codec_bytes(reader.digest()?)),
        _ => {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "mechanism target tag",
            ));
        }
    };
    Ok(MechanismRequestScope::new(request_id, question_id, target))
}

fn decode_endpoint(tag: u8) -> Result<RelationalMechanismEndpoint, RelationalMechanismReplayError> {
    match tag {
        0x01 => Ok(RelationalMechanismEndpoint::Before),
        0x02 => Ok(RelationalMechanismEndpoint::After),
        _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "endpoint tag",
        )),
    }
}

fn decode_permanent_unavailable(
    tag: u8,
) -> Result<RelationalMechanismPermanentUnavailable, RelationalMechanismReplayError> {
    match tag {
        0x01 => Ok(RelationalMechanismPermanentUnavailable::ObservationInstrumentationUnsupported),
        0x04 => Ok(RelationalMechanismPermanentUnavailable::ReplayAbiCapacityExceeded),
        _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "permanent-unavailability tag",
        )),
    }
}

type RestoredEndpointGraph = CanonicalEndpointGraph;

fn decode_validated_signature_graphs_for_scope(
    definition: &MechanismSignatureDefinition,
    expected_scope: MechanismRequestScope,
    budget: &mut StructuralDerivationBudget,
) -> Result<(RestoredEndpointGraph, RestoredEndpointGraph), RelationalStructuralMechanismError> {
    validate_signature_definition(definition, expected_scope.request_id())?;
    let mut reader = PayloadReader::new(definition.canonical_definition());
    reader.expect_bytes(SIGNATURE_DEFINITION_V3)?;
    reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
    reader.expect_u32(1)?;
    if decode_scope(&mut reader)? != expected_scope {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch.into());
    }
    let observation_id = reader.digest()?;
    let observation_contract_start = reader.position();
    let observation_contract = decode_observation_contract(&mut reader)?;
    let observation_contract_end = reader.position();
    let mut observation_identity = Encoder::new(OBSERVATION_ID_V3);
    observation_identity.bytes.extend_from_slice(
        definition
            .canonical_definition()
            .get(observation_contract_start..observation_contract_end)
            .ok_or(RelationalMechanismReplayError::InvalidDurablePayload(
                "observation contract byte span",
            ))?,
    );
    let derived_observation_id: [u8; 32] = Sha256::digest(observation_identity.finish()).into();
    if derived_observation_id != observation_id {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch.into());
    }
    let _state_schema_id = reader.digest()?;
    let _context_schema_id = reader.digest()?;
    let _transition_type_id = reader.digest()?;
    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::Before {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch.into());
    }
    let before = decode_endpoint_graph_with_budget(
        &mut reader,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
        Some(&mut *budget),
    )?;
    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::After {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch.into());
    }
    let after = decode_endpoint_graph_with_budget(
        &mut reader,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
        Some(&mut *budget),
    )?;
    reader.finish()?;
    PairingShape::from_graph(&before)?
        .ensure_unambiguous_with(&PairingShape::from_graph(&after)?)?;
    budget.finish_shape_admission()?;
    Ok((before, after))
}

fn index_signature_definition(
    definition: &MechanismSignatureDefinition,
    expected_scope: MechanismRequestScope,
) -> Result<RelationalMechanismSignatureDagIndex, RelationalMechanismReplayError> {
    validate_signature_definition(definition, expected_scope.request_id())?;
    let mut reader = PayloadReader::new(definition.canonical_definition());
    reader.expect_bytes(SIGNATURE_DEFINITION_V3)?;
    reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
    reader.expect_u32(1)?;
    if decode_scope(&mut reader)? != expected_scope {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch);
    }
    let observation_id = reader.digest()?;
    let observation_contract_start = reader.position();
    let observation_contract = decode_observation_contract(&mut reader)?;
    let observation_contract_end = reader.position();
    let mut observation_identity = Encoder::new(OBSERVATION_ID_V3);
    observation_identity.bytes.extend_from_slice(
        definition
            .canonical_definition()
            .get(observation_contract_start..observation_contract_end)
            .ok_or(RelationalMechanismReplayError::InvalidDurablePayload(
                "observation contract byte span",
            ))?,
    );
    let derived_observation_id: [u8; 32] = Sha256::digest(observation_identity.finish()).into();
    if derived_observation_id != observation_id {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch);
    }
    let _state_schema_id = reader.digest()?;
    let _context_schema_id = reader.digest()?;
    let _transition_type_id = reader.digest()?;

    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::Before {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch);
    }
    let (before_graph, before) =
        decode_indexed_endpoint_graph(&mut reader, &observation_contract.analysis_program)?;
    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::After {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch);
    }
    let (after_graph, after) =
        decode_indexed_endpoint_graph(&mut reader, &observation_contract.analysis_program)?;
    reader.finish()?;

    validate_restored_endpoint_graph(
        &before_graph,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
    )?;
    validate_restored_endpoint_graph(
        &after_graph,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
    )?;
    PairingShape::from_graph(&before_graph)?
        .ensure_unambiguous_with(&PairingShape::from_graph(&after_graph)?)?;

    let structured_record_count = before.record_count.checked_add(after.record_count).ok_or(
        RelationalMechanismReplayError::InvalidDurablePayload("structured mechanism record count"),
    )?;
    Ok(RelationalMechanismSignatureDagIndex {
        signature_id: definition.id(),
        definition_digest: definition.canonical_differential_digest(),
        definition_bytes: definition.canonical_definition().len(),
        before,
        after,
        structured_record_count,
    })
}

fn validate_restored_endpoint_graph(
    graph: &RestoredEndpointGraph,
    analysis_program: &AnalysisProgramId,
    expected_root_activation: &RelationalMechanismActivationStep,
) -> Result<(), RelationalMechanismReplayError> {
    validate_canonical_endpoint_graph(graph, Some(analysis_program))?;
    validate_canonical_root_activation(graph, expected_root_activation)
}

fn decode_indexed_endpoint_graph(
    reader: &mut PayloadReader<'_>,
    analysis_program: &AnalysisProgramId,
) -> Result<
    (RestoredEndpointGraph, RelationalMechanismEndpointDagIndex),
    RelationalMechanismReplayError,
> {
    let activation_paths = decode_canonical_activation_paths(reader, analysis_program)?;
    let node_count = reader.collection_len(MAX_TRACE_NODES, "trace nodes")?;
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    let mut outcomes = Vec::new();
    outcomes.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    let mut node_spans = Vec::new();
    node_spans.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    for _ in 0..node_count {
        let start = reader.position();
        let slot = decode_compact_occurrence_slot(reader, activation_paths.len())?;
        if ordered.last().is_some_and(|previous| previous >= &slot) {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "trace node order",
            ));
        }
        slot.validate_for(activation_paths.len(), analysis_program)?;
        let outcome = decode_event_outcome(reader, slot.kind, analysis_program)?;
        node_spans.push(MechanismDefinitionByteSpan {
            start,
            end: reader.position(),
        });
        ordered.push(slot);
        outcomes.push(outcome);
    }

    let root_count = reader.collection_len(node_count, "trace roots")?;
    let mut roots = BTreeSet::new();
    let mut root_ordinals = Vec::new();
    root_ordinals.try_reserve_exact(root_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: root_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    let mut previous_root = None;
    for _ in 0..root_count {
        let ordinal = reader.ordinal(node_count, "trace root ordinal")?;
        if previous_root.is_some_and(|previous| previous >= ordinal) {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "trace root order",
            ));
        }
        previous_root = Some(ordinal);
        root_ordinals.push(ordinal);
        roots.insert(ordered[ordinal].clone());
    }
    if reader.collection_len(node_count, "trace dependency rows")? != node_count {
        return Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "trace dependency row count",
        ));
    }

    let mut occurrences = BTreeMap::new();
    let mut dependency_rows = Vec::new();
    dependency_rows.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    let mut edge_record_ends = Vec::new();
    edge_record_ends
        .try_reserve_exact(node_count)
        .map_err(|_| RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        })?;
    let mut total_edges = 0usize;
    for (slot, outcome) in ordered.iter().cloned().zip(outcomes) {
        let dependency_count = reader.collection_len(MAX_TRACE_EDGES, "trace dependencies")?;
        total_edges = total_edges.checked_add(dependency_count).ok_or(
            RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: usize::MAX,
                limit: MAX_TRACE_EDGES,
            },
        )?;
        if total_edges > MAX_TRACE_EDGES {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: total_edges,
                limit: MAX_TRACE_EDGES,
            });
        }
        let ordinals_offset = reader.position();
        let mut dependencies = BTreeSet::new();
        let mut previous_dependency = None;
        for _ in 0..dependency_count {
            let ordinal = reader.ordinal(node_count, "trace dependency ordinal")?;
            if previous_dependency.is_some_and(|previous| previous >= ordinal) {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "trace dependency order",
                ));
            }
            previous_dependency = Some(ordinal);
            dependencies.insert(ordered[ordinal].clone());
        }
        dependency_rows.push(MechanismDependencyRowIndex {
            ordinals_offset,
            dependency_count,
        });
        edge_record_ends.push(total_edges as u128);
        occurrences.insert(
            slot,
            ValidatedEndpointOccurrence {
                outcome,
                dependencies,
            },
        );
    }
    let record_count = (node_count as u128)
        .checked_mul(2)
        .and_then(|count| count.checked_add(root_count as u128))
        .and_then(|count| count.checked_add(total_edges as u128))
        .ok_or(RelationalMechanismReplayError::InvalidDurablePayload(
            "endpoint projection record count",
        ))?;
    Ok((
        CanonicalEndpointGraph {
            activation_paths: Arc::clone(&activation_paths),
            roots,
            occurrences,
        },
        RelationalMechanismEndpointDagIndex {
            activation_paths,
            node_spans: node_spans.into_boxed_slice(),
            root_ordinals: root_ordinals.into_boxed_slice(),
            dependency_rows: dependency_rows.into_boxed_slice(),
            edge_record_ends: edge_record_ends.into_boxed_slice(),
            record_count,
        },
    ))
}

fn decode_signature_endpoint_graphs(
    canonical_definition: &[u8],
    receipt: &RelationalMechanismReplayReceipt,
    transition: &TransitionInstance,
) -> Result<(RestoredEndpointGraph, RestoredEndpointGraph), RelationalMechanismReplayError> {
    let mut reader = PayloadReader::new(canonical_definition);
    reader.expect_bytes(SIGNATURE_DEFINITION_V3)?;
    reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
    reader.expect_u32(1)?;
    if decode_scope(&mut reader)? != receipt.scope {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch);
    }
    if reader.digest()? != receipt.observation_id.bytes() {
        return Err(RelationalMechanismReplayError::ReplayReceiptSignatureMismatch);
    }
    let observation_contract = decode_observation_contract(&mut reader)?;
    if receipt.state_type_digest != observation_contract.state_type_digest
        || receipt.context_type_digest != observation_contract.context_type_digest
        || receipt.observation_type_digest != observation_contract.observation_type_digest
    {
        return Err(RelationalMechanismReplayError::ReplayReceiptTypeContractMismatch);
    }
    if reader.digest()? != transition.state_schema_id().bytes()
        || reader.digest()? != transition.context_schema_id().bytes()
        || reader.digest()? != transition.transition_type_id().bytes()
    {
        return Err(RelationalMechanismReplayError::ReplayReceiptTransitionMismatch);
    }
    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::Before {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch);
    }
    let before = decode_endpoint_graph(
        &mut reader,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
    )?;
    if decode_endpoint(reader.tag()?)? != RelationalMechanismEndpoint::After {
        return Err(RelationalMechanismReplayError::EndpointTraceRoleMismatch);
    }
    let after = decode_endpoint_graph(
        &mut reader,
        &observation_contract.analysis_program,
        &observation_contract.expected_root_activation,
    )?;
    reader.finish()?;
    Ok((before, after))
}

struct DecodedObservationContract {
    analysis_program: AnalysisProgramId,
    expected_root_activation: RelationalMechanismActivationStep,
    state_type_digest: [u8; 32],
    context_type_digest: [u8; 32],
    observation_type_digest: [u8; 32],
}

fn decode_observation_contract(
    reader: &mut PayloadReader<'_>,
) -> Result<DecodedObservationContract, RelationalMechanismReplayError> {
    reader.expect_u32(RELATIONAL_MECHANISM_REPLAY_ABI_VERSION)?;
    reader.expect_u32(1)?;
    let template_site = decode_site(reader)?;
    template_site.validate_for(
        &template_site.analysis_program,
        Some(RelationalMechanismSiteKind::Expression),
    )?;
    let endpoint_callable = decode_site(reader)?;
    endpoint_callable.validate_for(
        &template_site.analysis_program,
        Some(RelationalMechanismSiteKind::Callable),
    )?;
    let callable_semantic_key = reader.bytes()?;
    std::str::from_utf8(callable_semantic_key).map_err(|_| {
        RelationalMechanismReplayError::InvalidDurablePayload("callable semantic key UTF-8")
    })?;
    reader.skip_u128_index()?;
    let callable_normalized_ordinal = reader.usize_index("callable normalized ordinal")?;
    let callable_path =
        reader.collection_len(MAX_CHECKED_SITE_PATH_ITEMS, "callable structural path")?;
    let mut callable_site_digest = mechanism_site_digest_prefix(
        b"callable",
        &template_site.analysis_program,
        callable_semantic_key,
        callable_normalized_ordinal,
        callable_path,
    );
    for _ in 0..callable_path {
        mechanism_site_hash_u32(&mut callable_site_digest, reader.u32()?);
    }
    let derived_callable_digest: [u8; 32] = callable_site_digest.finalize().into();
    if derived_callable_digest != endpoint_callable.digest {
        return Err(RelationalMechanismReplayError::SignatureDefinitionIdentityMismatch);
    }

    let expression_analysis_program = AnalysisProgramId(reader.string()?.into_boxed_str());
    validate_analysis_program(&expression_analysis_program)?;
    if expression_analysis_program != template_site.analysis_program {
        return Err(RelationalMechanismReplayError::ForeignTraceSite);
    }
    let expression_semantic_key = reader.bytes()?;
    std::str::from_utf8(expression_semantic_key).map_err(|_| {
        RelationalMechanismReplayError::InvalidDurablePayload("expression semantic key UTF-8")
    })?;
    let expression_normalized_ordinal = reader.usize_index("expression normalized ordinal")?;
    let expression_path =
        reader.collection_len(MAX_CHECKED_SITE_PATH_ITEMS, "expression AST path")?;
    let mut expression_site_digest = mechanism_site_digest_prefix(
        b"expression",
        &expression_analysis_program,
        expression_semantic_key,
        expression_normalized_ordinal,
        expression_path,
    );
    for _ in 0..expression_path {
        mechanism_site_hash_u32(&mut expression_site_digest, reader.u32()?);
    }
    let derived_expression_digest: [u8; 32] = expression_site_digest.finalize().into();
    if derived_expression_digest != template_site.digest {
        return Err(RelationalMechanismReplayError::SignatureDefinitionIdentityMismatch);
    }
    let state_type_digest = decode_canonical_ty_digest(reader)?;
    let context_type_digest = decode_canonical_ty_digest(reader)?;
    let observation_type_digest = decode_canonical_ty_digest(reader)?;
    if reader.u128()? != 1 {
        return Err(RelationalMechanismReplayError::ObservationDependenciesNotClosed);
    }
    let analysis_program = template_site.analysis_program.clone();
    let expected_root_activation = RelationalMechanismActivationStep::new(
        template_site,
        RelationalMechanismCalleeId::function(endpoint_callable)?,
        0,
    )?;
    Ok(DecodedObservationContract {
        analysis_program,
        expected_root_activation,
        state_type_digest,
        context_type_digest,
        observation_type_digest,
    })
}

fn decode_canonical_ty_digest(
    reader: &mut PayloadReader<'_>,
) -> Result<[u8; 32], RelationalMechanismReplayError> {
    let start = reader.position();
    skip_ty(reader, 0)?;
    let encoded_ty = reader.bytes.get(start..reader.position()).ok_or(
        RelationalMechanismReplayError::InvalidDurablePayload("type byte span"),
    )?;
    let mut canonical = Encoder::new(MECHANISM_TYPE_V3);
    canonical.bytes.extend_from_slice(encoded_ty);
    Ok(Sha256::digest(canonical.finish()).into())
}

fn mechanism_site_digest_prefix(
    kind: &[u8],
    analysis_program: &AnalysisProgramId,
    semantic_key: &[u8],
    normalized_ordinal: usize,
    path_len: usize,
) -> Sha256 {
    let mut hasher = Sha256::new();
    mechanism_site_hash_segment(&mut hasher, LEGACY_MECHANISM_SITE_HASH_V2);
    mechanism_site_hash_segment(&mut hasher, kind);
    mechanism_site_hash_segment(&mut hasher, analysis_program.as_str().as_bytes());
    mechanism_site_hash_segment(&mut hasher, semantic_key);
    mechanism_site_hash_segment(&mut hasher, &(normalized_ordinal as u128).to_le_bytes());
    mechanism_site_hash_segment(&mut hasher, &(path_len as u128).to_le_bytes());
    hasher
}

fn mechanism_site_hash_u32(hasher: &mut Sha256, value: u32) {
    mechanism_site_hash_segment(hasher, &value.to_le_bytes());
}

fn mechanism_site_hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn skip_ty(
    reader: &mut PayloadReader<'_>,
    depth: usize,
) -> Result<(), RelationalMechanismReplayError> {
    if depth > MAX_DURABLE_VALUE_DEPTH {
        return Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "type depth",
        ));
    }
    reader.value_node(depth)?;
    match reader.tag()? {
        0x01 => {
            std::str::from_utf8(reader.bytes()?).map_err(|_| {
                RelationalMechanismReplayError::InvalidDurablePayload("type name UTF-8")
            })?;
            Ok(())
        }
        0x02 => {
            skip_ty(reader, depth + 1)?;
            let count = reader.collection_len(MAX_DURABLE_VALUE_NODES, "type arguments")?;
            for _ in 0..count {
                skip_ty(reader, depth + 1)?;
            }
            Ok(())
        }
        0x03 => {
            skip_ty(reader, depth + 1)?;
            skip_ty(reader, depth + 1)
        }
        0x04 | 0x05 | 0x06 | 0x07 => skip_ty(reader, depth + 1),
        0x09 => Ok(()),
        0x08 | 0x0a => Err(RelationalMechanismReplayError::OpenObservationType),
        _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "type tag",
        )),
    }
}

fn decode_endpoint_graph(
    reader: &mut PayloadReader<'_>,
    analysis_program: &AnalysisProgramId,
    expected_root_activation: &RelationalMechanismActivationStep,
) -> Result<RestoredEndpointGraph, RelationalMechanismReplayError> {
    match decode_endpoint_graph_with_budget(
        reader,
        analysis_program,
        expected_root_activation,
        None,
    ) {
        Ok(graph) => Ok(graph),
        Err(RelationalStructuralMechanismError::Replay(error)) => Err(error),
        Err(RelationalStructuralMechanismError::Quotient(_)) => {
            unreachable!("an unbudgeted replay decode cannot fail structural work admission")
        }
    }
}

fn decode_endpoint_graph_with_budget(
    reader: &mut PayloadReader<'_>,
    analysis_program: &AnalysisProgramId,
    expected_root_activation: &RelationalMechanismActivationStep,
    mut budget: Option<&mut StructuralDerivationBudget>,
) -> Result<RestoredEndpointGraph, RelationalStructuralMechanismError> {
    let activation_paths = decode_canonical_activation_paths_with_budget(
        reader,
        analysis_program,
        budget.as_deref_mut(),
    )?;
    let node_count = reader.collection_len(MAX_TRACE_NODES, "trace nodes")?;
    if let Some(budget) = budget.as_deref_mut() {
        budget.admit_occurrences(node_count)?;
    }
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    let mut outcomes = Vec::new();
    outcomes.try_reserve_exact(node_count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: node_count,
            limit: MAX_TRACE_NODES,
        }
    })?;
    for _ in 0..node_count {
        let slot = decode_compact_occurrence_slot(reader, activation_paths.len())?;
        if ordered.last().is_some_and(|previous| previous >= &slot) {
            return Err(
                RelationalMechanismReplayError::InvalidDurablePayload("trace node order").into(),
            );
        }
        slot.validate_for(activation_paths.len(), analysis_program)?;
        let outcome = decode_event_outcome(reader, slot.kind, analysis_program)?;
        ordered.push(slot);
        outcomes.push(outcome);
    }

    let root_count = reader.collection_len(node_count, "trace roots")?;
    let mut roots = BTreeSet::new();
    let mut previous_root = None;
    for _ in 0..root_count {
        let ordinal = reader.ordinal(node_count, "trace root ordinal")?;
        if previous_root.is_some_and(|previous| previous >= ordinal) {
            return Err(
                RelationalMechanismReplayError::InvalidDurablePayload("trace root order").into(),
            );
        }
        previous_root = Some(ordinal);
        roots.insert(ordered[ordinal].clone());
    }
    if reader.collection_len(node_count, "trace dependency rows")? != node_count {
        return Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "trace dependency row count",
        )
        .into());
    }
    let mut occurrences = BTreeMap::new();
    let mut total_edges = 0usize;
    for (slot, outcome) in ordered.iter().cloned().zip(outcomes) {
        let dependency_count = reader.collection_len(MAX_TRACE_EDGES, "trace dependencies")?;
        total_edges = total_edges.checked_add(dependency_count).ok_or(
            RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: usize::MAX,
                limit: MAX_TRACE_EDGES,
            },
        )?;
        if total_edges > MAX_TRACE_EDGES {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "edges",
                actual: total_edges,
                limit: MAX_TRACE_EDGES,
            }
            .into());
        }
        if let Some(budget) = budget.as_deref_mut() {
            budget.admit_edges(dependency_count)?;
        }
        let mut dependencies = BTreeSet::new();
        let mut previous_dependency = None;
        for _ in 0..dependency_count {
            let ordinal = reader.ordinal(node_count, "trace dependency ordinal")?;
            if previous_dependency.is_some_and(|previous| previous >= ordinal) {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "trace dependency order",
                )
                .into());
            }
            previous_dependency = Some(ordinal);
            dependencies.insert(ordered[ordinal].clone());
        }
        occurrences.insert(
            slot,
            ValidatedEndpointOccurrence {
                outcome,
                dependencies,
            },
        );
    }
    let graph = CanonicalEndpointGraph {
        activation_paths,
        roots,
        occurrences,
    };
    validate_canonical_endpoint_graph(&graph, Some(analysis_program))?;
    validate_canonical_root_activation(&graph, expected_root_activation)?;
    Ok(graph)
}

fn decode_canonical_activation_paths(
    reader: &mut PayloadReader<'_>,
    analysis_program: &AnalysisProgramId,
) -> Result<Arc<[CanonicalActivationPathNode]>, RelationalMechanismReplayError> {
    match decode_canonical_activation_paths_with_budget(reader, analysis_program, None) {
        Ok(paths) => Ok(paths),
        Err(RelationalStructuralMechanismError::Replay(error)) => Err(error),
        Err(RelationalStructuralMechanismError::Quotient(_)) => {
            unreachable!("an unbudgeted replay decode cannot fail structural work admission")
        }
    }
}

fn decode_canonical_activation_paths_with_budget(
    reader: &mut PayloadReader<'_>,
    analysis_program: &AnalysisProgramId,
    budget: Option<&mut StructuralDerivationBudget>,
) -> Result<Arc<[CanonicalActivationPathNode]>, RelationalStructuralMechanismError> {
    let count = reader.collection_len(MAX_TRACE_ACTIVATION_NODES, "activation nodes")?;
    if let Some(budget) = budget {
        budget.admit_activations(count)?;
    }
    let mut paths = Vec::<CanonicalActivationPathNode>::new();
    paths
        .try_reserve_exact(count)
        .map_err(|_| RelationalMechanismReplayError::TraceCapacity {
            resource: "activation nodes",
            actual: count,
            limit: MAX_TRACE_ACTIVATION_NODES,
        })?;
    for ordinal in 0..count {
        let parent = match reader.tag()? {
            0x00 => None,
            0x01 => Some(reader.ordinal(ordinal, "activation parent ordinal")?),
            _ => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "activation parent tag",
                )
                .into());
            }
        };
        let step = decode_activation_step(reader)?;
        step.validate_for(analysis_program)?;
        let depth = parent.map_or(1, |parent| paths[parent].depth + 1);
        if depth > MAX_ACTIVATION_DEPTH {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: "activation depth",
                actual: depth,
                limit: MAX_ACTIVATION_DEPTH,
            }
            .into());
        }
        paths.push(CanonicalActivationPathNode {
            parent,
            step,
            depth,
        });
    }
    let paths: Arc<[CanonicalActivationPathNode]> = paths.into();
    validate_canonical_activation_trie(&paths, analysis_program)?;
    Ok(paths)
}

fn decode_activation_step(
    reader: &mut PayloadReader<'_>,
) -> Result<RelationalMechanismActivationStep, RelationalMechanismReplayError> {
    let call_site = decode_site(reader)?;
    let callee_tag = reader.tag()?;
    let callee_site = decode_site(reader)?;
    let callee = match callee_tag {
        0x01 => RelationalMechanismCalleeId::function(callee_site)?,
        0x02 => RelationalMechanismCalleeId::rule_family(callee_site)?,
        _ => {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "callee tag",
            ));
        }
    };
    RelationalMechanismActivationStep::new(call_site, callee, reader.u32()?)
}

fn decode_compact_occurrence_slot(
    reader: &mut PayloadReader<'_>,
    activation_path_count: usize,
) -> Result<CompactOccurrenceSlot, RelationalMechanismReplayError> {
    let root_index = reader.u32()?;
    let activation_path = reader.ordinal(activation_path_count, "trace activation path ordinal")?;
    let site = decode_site(reader)?;
    let kind = decode_event_kind(reader.tag()?)?;
    Ok(CompactOccurrenceSlot {
        root_index,
        activation_path,
        site,
        kind,
        visit_ordinal: reader.u32()?,
    })
}

fn materialize_occurrence_slot(
    activation_paths: &[CanonicalActivationPathNode],
    compact: &CompactOccurrenceSlot,
) -> Result<RelationalMechanismOccurrenceSlot, RelationalMechanismReplayError> {
    let Some(path) = activation_paths.get(compact.activation_path) else {
        return Err(RelationalMechanismReplayError::MissingActivationPath);
    };
    let mut expanded = Vec::new();
    expanded.try_reserve_exact(path.depth).map_err(|_| {
        RelationalMechanismReplayError::TraceCapacity {
            resource: "activation depth",
            actual: path.depth,
            limit: MAX_ACTIVATION_DEPTH,
        }
    })?;
    let mut cursor = Some(compact.activation_path);
    while let Some(ordinal) = cursor {
        let node = &activation_paths[ordinal];
        expanded.push(node.step.clone());
        cursor = node.parent;
    }
    expanded.reverse();
    RelationalMechanismOccurrenceSlot::new(
        compact.root_index,
        Arc::<[RelationalMechanismActivationStep]>::from(expanded),
        compact.site.clone(),
        compact.kind,
        compact.visit_ordinal,
    )
}

fn decode_site(
    reader: &mut PayloadReader<'_>,
) -> Result<RelationalMechanismSiteId, RelationalMechanismReplayError> {
    let program = reader.string()?;
    let analysis_program = AnalysisProgramId(program.into_boxed_str());
    validate_analysis_program(&analysis_program)?;
    let kind = match reader.tag()? {
        0x01 => RelationalMechanismSiteKind::Expression,
        0x02 => RelationalMechanismSiteKind::Callable,
        0x03 => RelationalMechanismSiteKind::RuleFamily,
        0x04 => RelationalMechanismSiteKind::RuleCandidate,
        _ => {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "site kind",
            ));
        }
    };
    Ok(RelationalMechanismSiteId {
        analysis_program,
        kind,
        digest: reader.digest()?,
    })
}

fn decode_event_kind(
    tag: u8,
) -> Result<RelationalMechanismEventKind, RelationalMechanismReplayError> {
    match tag {
        0x01 => Ok(RelationalMechanismEventKind::RuleAttempt),
        0x02 => Ok(RelationalMechanismEventKind::RuleSelection),
        0x03 => Ok(RelationalMechanismEventKind::IfDecision),
        0x04 => Ok(RelationalMechanismEventKind::MatchDecision),
        0x05 => Ok(RelationalMechanismEventKind::ShortCircuitAnd),
        0x06 => Ok(RelationalMechanismEventKind::ShortCircuitOr),
        _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "event kind",
        )),
    }
}

fn decode_event_outcome(
    reader: &mut PayloadReader<'_>,
    kind: RelationalMechanismEventKind,
    analysis_program: &AnalysisProgramId,
) -> Result<RelationalMechanismEventOutcome, RelationalMechanismReplayError> {
    let outcome = match reader.tag()? {
        0x01 => RelationalMechanismEventOutcome::RuleAttempt(match reader.tag()? {
            0x01 => RelationalRuleAttemptOutcome::HeadMismatch,
            0x02 => RelationalRuleAttemptOutcome::GuardFalse,
            0x03 => RelationalRuleAttemptOutcome::BodyFalse,
            0x04 => RelationalRuleAttemptOutcome::Applicable,
            _ => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "rule-attempt outcome",
                ));
            }
        }),
        0x02 => RelationalMechanismEventOutcome::RuleSelection(match reader.tag()? {
            0x01 => RelationalRuleSelectionOutcome::NoApplicableRule,
            0x02 => RelationalRuleSelectionOutcome::Selected(decode_site(reader)?),
            _ => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "rule-selection outcome",
                ));
            }
        }),
        0x03 => RelationalMechanismEventOutcome::IfDecision(match reader.tag()? {
            0x01 => RelationalIfDecisionOutcome::Then,
            0x02 => RelationalIfDecisionOutcome::Else,
            _ => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "if outcome",
                ));
            }
        }),
        0x04 => RelationalMechanismEventOutcome::MatchDecision {
            arm_index: reader.u32()?,
        },
        0x05 => RelationalMechanismEventOutcome::ShortCircuit(match reader.tag()? {
            0x01 => RelationalShortCircuitOutcome::SkippedRight {
                result: reader.bool()?,
            },
            0x02 => RelationalShortCircuitOutcome::EvaluatedRight {
                result: reader.bool()?,
            },
            _ => {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "short-circuit outcome",
                ));
            }
        }),
        _ => {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "event outcome tag",
            ));
        }
    };
    outcome.validate_for(kind, analysis_program)?;
    Ok(outcome)
}

fn encode_durable_value(
    encoder: &mut Encoder,
    value: &ExploreValue,
    depth: usize,
) -> Result<(), RelationalMechanismReplayError> {
    encoder.value_node(depth)?;
    match value {
        ExploreValue::Int(value) => {
            encoder.tag(0x01);
            encoder.i64(*value);
        }
        ExploreValue::FloatBits(bits) => {
            encoder.tag(0x02);
            encoder.u64(*bits);
        }
        ExploreValue::String(value) => {
            encoder.tag(0x03);
            encoder.bytes(value.as_bytes());
        }
        ExploreValue::Character(value) => {
            encoder.tag(0x04);
            encoder.u32(u32::from(*value));
        }
        ExploreValue::Boolean(value) => {
            encoder.tag(0x05);
            encoder.bool(*value);
        }
        ExploreValue::Unit => encoder.tag(0x06),
        ExploreValue::List(values) => {
            encoder.tag(0x07);
            encode_durable_values(encoder, values, depth + 1)?;
        }
        ExploreValue::Set(values) => {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "canonical set order",
                ));
            }
            encoder.tag(0x08);
            encode_durable_values(encoder, values, depth + 1)?;
        }
        ExploreValue::Tuple(values) => {
            encoder.tag(0x09);
            encode_durable_values(encoder, values, depth + 1)?;
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            encoder.tag(0x0a);
            encoder.bytes(type_name.as_bytes());
            encoder.bytes(variant.as_bytes());
            encoder.bool(*positional);
            encoder.u128(fields.len() as u128);
            for (name, value) in fields.iter() {
                encoder.bytes(name.as_bytes());
                encode_durable_value(encoder, value, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn encode_durable_values(
    encoder: &mut Encoder,
    values: &[ExploreValue],
    depth: usize,
) -> Result<(), RelationalMechanismReplayError> {
    encoder.u128(values.len() as u128);
    for value in values {
        encode_durable_value(encoder, value, depth)?;
    }
    Ok(())
}

fn decode_durable_value(
    reader: &mut PayloadReader<'_>,
    depth: usize,
) -> Result<ExploreValue, RelationalMechanismReplayError> {
    reader.value_node(depth)?;
    match reader.tag()? {
        0x01 => Ok(ExploreValue::Int(reader.i64()?)),
        0x02 => Ok(ExploreValue::FloatBits(reader.u64()?)),
        0x03 => Ok(ExploreValue::String(reader.string()?)),
        0x04 => char::from_u32(reader.u32()?)
            .map(ExploreValue::Character)
            .ok_or(RelationalMechanismReplayError::InvalidDurablePayload(
                "character scalar",
            )),
        0x05 => Ok(ExploreValue::Boolean(reader.bool()?)),
        0x06 => Ok(ExploreValue::Unit),
        0x07 => Ok(ExploreValue::List(decode_durable_values(
            reader,
            depth + 1,
        )?)),
        0x08 => {
            let values = decode_durable_values(reader, depth + 1)?;
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                    "canonical set order",
                ));
            }
            Ok(ExploreValue::Set(values))
        }
        0x09 => Ok(ExploreValue::Tuple(decode_durable_values(
            reader,
            depth + 1,
        )?)),
        0x0a => {
            let type_name = reader.string()?;
            let variant = reader.string()?;
            let positional = reader.bool()?;
            let count = reader.collection_len(MAX_DURABLE_VALUE_NODES, "constructor fields")?;
            let mut fields = Vec::new();
            fields.try_reserve_exact(count).map_err(|_| {
                RelationalMechanismReplayError::DurablePayloadCapacity {
                    actual: count,
                    limit: MAX_DURABLE_VALUE_NODES,
                }
            })?;
            for _ in 0..count {
                fields.push((reader.string()?, decode_durable_value(reader, depth + 1)?));
            }
            Ok(ExploreValue::Constructor {
                type_name,
                variant,
                positional,
                fields: fields.into(),
            })
        }
        _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
            "ExploreValue tag",
        )),
    }
}

fn decode_durable_values(
    reader: &mut PayloadReader<'_>,
    depth: usize,
) -> Result<Vec<ExploreValue>, RelationalMechanismReplayError> {
    let count = reader.collection_len(MAX_DURABLE_VALUE_NODES, "ExploreValue collection")?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        RelationalMechanismReplayError::DurablePayloadCapacity {
            actual: count,
            limit: MAX_DURABLE_VALUE_NODES,
        }
    })?;
    for _ in 0..count {
        values.push(decode_durable_value(reader, depth)?);
    }
    Ok(values)
}

/// Small canonical byte encoder. Every variable-width segment is length
/// framed; graph maps and sets are already in strict BTree canonical order.
struct Encoder {
    bytes: Vec<u8>,
    durable_value_nodes: usize,
    byte_limit: Option<usize>,
    required_bytes: usize,
}

impl Encoder {
    fn new(domain: &[u8]) -> Self {
        Self::with_limit(domain, None)
    }

    fn bounded(domain: &[u8], byte_limit: usize) -> Self {
        Self::with_limit(domain, Some(byte_limit))
    }

    fn with_limit(domain: &[u8], byte_limit: Option<usize>) -> Self {
        let mut encoder = Self {
            bytes: Vec::new(),
            durable_value_nodes: 0,
            byte_limit,
            required_bytes: 0,
        };
        encoder.bytes(domain);
        encoder
    }

    fn append(&mut self, bytes: &[u8]) {
        self.required_bytes = self
            .required_bytes
            .checked_add(bytes.len())
            .unwrap_or(usize::MAX);
        if self
            .byte_limit
            .map_or(true, |limit| self.required_bytes <= limit)
        {
            self.bytes.extend_from_slice(bytes);
        }
    }

    fn tag(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.append(&value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.u128(bytes.len() as u128);
        self.append(bytes);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.append(&digest);
    }

    fn value_node(&mut self, depth: usize) -> Result<(), RelationalMechanismReplayError> {
        if depth > MAX_DURABLE_VALUE_DEPTH {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "ExploreValue depth",
            ));
        }
        self.durable_value_nodes = self.durable_value_nodes.checked_add(1).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("ExploreValue node count"),
        )?;
        if self.durable_value_nodes > MAX_DURABLE_VALUE_NODES {
            return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
                actual: self.durable_value_nodes,
                limit: MAX_DURABLE_VALUE_NODES,
            });
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn try_finish(self) -> Result<Vec<u8>, RelationalMechanismReplayError> {
        if let Some(limit) = self.byte_limit {
            if self.required_bytes > limit {
                return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
                    actual: self.required_bytes,
                    limit,
                });
            }
        }
        Ok(self.bytes)
    }
}

/// Capacity-checked inverse of [`Encoder`] for private durable restoration.
/// It deliberately has no public field constructors: decoded graph and
/// receipt values pass through the module's ordinary identity validators.
struct PayloadReader<'a> {
    bytes: &'a [u8],
    position: usize,
    value_nodes: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            value_nodes: 0,
        }
    }

    const fn position(&self) -> usize {
        self.position
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RelationalMechanismReplayError> {
        let end = self.position.checked_add(count).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("length overflow"),
        )?;
        let bytes = self.bytes.get(self.position..end).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("truncated bytes"),
        )?;
        self.position = end;
        Ok(bytes)
    }

    fn tag(&mut self) -> Result<u8, RelationalMechanismReplayError> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, RelationalMechanismReplayError> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "boolean",
            )),
        }
    }

    fn u32(&mut self) -> Result<u32, RelationalMechanismReplayError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| RelationalMechanismReplayError::InvalidDurablePayload("u32"),
        )?))
    }

    fn u64(&mut self) -> Result<u64, RelationalMechanismReplayError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| RelationalMechanismReplayError::InvalidDurablePayload("u64"),
        )?))
    }

    fn i64(&mut self) -> Result<i64, RelationalMechanismReplayError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| RelationalMechanismReplayError::InvalidDurablePayload("i64"),
        )?))
    }

    fn u128(&mut self) -> Result<u128, RelationalMechanismReplayError> {
        Ok(u128::from_le_bytes(self.take(16)?.try_into().map_err(
            |_| RelationalMechanismReplayError::InvalidDurablePayload("u128"),
        )?))
    }

    fn digest(&mut self) -> Result<[u8; 32], RelationalMechanismReplayError> {
        self.take(32)?
            .try_into()
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload("digest"))
    }

    fn bytes(&mut self) -> Result<&'a [u8], RelationalMechanismReplayError> {
        let count = usize::try_from(self.u128()?)
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload("byte length"))?;
        if count > MAX_DURABLE_BLOB_BYTES {
            return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
                actual: count,
                limit: MAX_DURABLE_BLOB_BYTES,
            });
        }
        self.take(count)
    }

    fn owned_bytes(&mut self) -> Result<Box<[u8]>, RelationalMechanismReplayError> {
        Ok(self.bytes()?.to_vec().into_boxed_slice())
    }

    fn string(&mut self) -> Result<String, RelationalMechanismReplayError> {
        String::from_utf8(self.bytes()?.to_vec())
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload("UTF-8 string"))
    }

    fn skip_bytes(&mut self) -> Result<(), RelationalMechanismReplayError> {
        let _ = self.bytes()?;
        Ok(())
    }

    fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), RelationalMechanismReplayError> {
        if self.bytes()? == expected {
            Ok(())
        } else {
            Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "domain separator",
            ))
        }
    }

    fn expect_u32(&mut self, expected: u32) -> Result<(), RelationalMechanismReplayError> {
        if self.u32()? == expected {
            Ok(())
        } else {
            Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "version",
            ))
        }
    }

    fn collection_len(
        &mut self,
        limit: usize,
        subject: &'static str,
    ) -> Result<usize, RelationalMechanismReplayError> {
        let count = usize::try_from(self.u128()?)
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload(subject))?;
        if count > limit {
            return Err(RelationalMechanismReplayError::TraceCapacity {
                resource: subject,
                actual: count,
                limit,
            });
        }
        Ok(count)
    }

    fn skip_u128_index(&mut self) -> Result<(), RelationalMechanismReplayError> {
        let _ = self.usize_index("index")?;
        Ok(())
    }

    fn usize_index(
        &mut self,
        subject: &'static str,
    ) -> Result<usize, RelationalMechanismReplayError> {
        usize::try_from(self.u128()?)
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload(subject))
    }

    fn ordinal(
        &mut self,
        upper_bound: usize,
        subject: &'static str,
    ) -> Result<usize, RelationalMechanismReplayError> {
        let ordinal = usize::try_from(self.u128()?)
            .map_err(|_| RelationalMechanismReplayError::InvalidDurablePayload(subject))?;
        if ordinal >= upper_bound {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                subject,
            ));
        }
        Ok(ordinal)
    }

    fn value_node(&mut self, depth: usize) -> Result<(), RelationalMechanismReplayError> {
        if depth > MAX_DURABLE_VALUE_DEPTH {
            return Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "ExploreValue depth",
            ));
        }
        self.value_nodes = self.value_nodes.checked_add(1).ok_or(
            RelationalMechanismReplayError::InvalidDurablePayload("ExploreValue node count"),
        )?;
        if self.value_nodes > MAX_DURABLE_VALUE_NODES {
            return Err(RelationalMechanismReplayError::DurablePayloadCapacity {
                actual: self.value_nodes,
                limit: MAX_DURABLE_VALUE_NODES,
            });
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), RelationalMechanismReplayError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(RelationalMechanismReplayError::InvalidDurablePayload(
                "trailing bytes",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT_GRAPH_TEST_DOMAIN: &[u8] =
        b"futuruna.test.relational-mechanism-endpoint-graph.v1";

    fn analysis_program() -> AnalysisProgramId {
        AnalysisProgramId("11".repeat(32).into_boxed_str())
    }

    fn site(
        analysis_program: &AnalysisProgramId,
        kind: RelationalMechanismSiteKind,
        tag: u8,
    ) -> RelationalMechanismSiteId {
        RelationalMechanismSiteId {
            analysis_program: analysis_program.clone(),
            kind,
            digest: [tag; 32],
        }
    }

    fn activation(
        analysis_program: &AnalysisProgramId,
        call_tag: u8,
        callee_tag: u8,
        invocation_ordinal: u32,
    ) -> RelationalMechanismActivationStep {
        RelationalMechanismActivationStep::new(
            site(
                analysis_program,
                RelationalMechanismSiteKind::Expression,
                call_tag,
            ),
            RelationalMechanismCalleeId::function(site(
                analysis_program,
                RelationalMechanismSiteKind::Callable,
                callee_tag,
            ))
            .expect("callable test site"),
            invocation_ordinal,
        )
        .expect("test activation")
    }

    fn activation_paths(
        analysis_program: &AnalysisProgramId,
        child_invocation_ordinals: &[u32],
    ) -> Vec<CanonicalActivationPathNode> {
        let mut paths = vec![CanonicalActivationPathNode {
            parent: None,
            step: activation(analysis_program, 1, 2, 0),
            depth: 1,
        }];
        paths.extend(
            child_invocation_ordinals
                .iter()
                .map(|ordinal| CanonicalActivationPathNode {
                    parent: Some(0),
                    step: activation(analysis_program, 3, 4, *ordinal),
                    depth: 2,
                }),
        );
        paths
    }

    fn occurrence_slot(
        analysis_program: &AnalysisProgramId,
        activation_path: usize,
        visit_ordinal: u32,
    ) -> CompactOccurrenceSlot {
        CompactOccurrenceSlot {
            root_index: 0,
            activation_path,
            site: site(analysis_program, RelationalMechanismSiteKind::Expression, 5),
            kind: RelationalMechanismEventKind::IfDecision,
            visit_ordinal,
        }
    }

    fn endpoint_graph(
        activation_paths: Vec<CanonicalActivationPathNode>,
        occurrences: Vec<(CompactOccurrenceSlot, RelationalIfDecisionOutcome)>,
    ) -> CanonicalEndpointGraph {
        let roots = occurrences
            .iter()
            .map(|(slot, _)| slot.clone())
            .collect::<BTreeSet<_>>();
        let occurrences = occurrences
            .into_iter()
            .map(|(slot, outcome)| {
                (
                    slot,
                    ValidatedEndpointOccurrence {
                        outcome: RelationalMechanismEventOutcome::IfDecision(outcome),
                        dependencies: BTreeSet::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        CanonicalEndpointGraph {
            activation_paths: activation_paths.into(),
            roots,
            occurrences,
        }
    }

    #[test]
    fn trace_empty_eventless_anchors_survive_durable_graph_round_trip() {
        let analysis_program = analysis_program();
        let graph = endpoint_graph(activation_paths(&analysis_program, &[0, 1]), Vec::new());
        validate_canonical_endpoint_graph(&graph, Some(&analysis_program))
            .expect("trace-empty graph with complete call anchors");

        let mut encoder = Encoder::new(ENDPOINT_GRAPH_TEST_DOMAIN);
        encode_endpoint_graph(&mut encoder, &graph);
        let encoded = encoder.finish();
        let mut reader = PayloadReader::new(&encoded);
        reader
            .expect_bytes(ENDPOINT_GRAPH_TEST_DOMAIN)
            .expect("test domain");
        let restored = decode_endpoint_graph(
            &mut reader,
            &analysis_program,
            &graph.activation_paths[0].step,
        )
        .expect("restore complete trace-empty graph");
        reader.finish().expect("consume complete test graph");

        assert_eq!(restored, graph);
        assert_eq!(restored.activation_paths.len(), 3);
        assert!(restored.occurrences.is_empty());
    }

    #[test]
    fn trailing_eventless_multiplicity_mismatch_fails_closed() {
        let analysis_program = analysis_program();
        let before = endpoint_graph(activation_paths(&analysis_program, &[0, 1]), Vec::new());
        let after = endpoint_graph(activation_paths(&analysis_program, &[0]), Vec::new());

        let error = PairingShape::from_graph(&before)
            .expect("before pairing shape")
            .ensure_unambiguous_with(
                &PairingShape::from_graph(&after).expect("after pairing shape"),
            )
            .expect_err("different multiplicities must not be guessed");
        assert_eq!(
            error,
            RelationalMechanismReplayError::AmbiguousEndpointPairing
        );
    }

    #[test]
    fn endpoint_only_eventless_call_remains_explicit_without_forcing_a_pair() {
        let analysis_program = analysis_program();
        let before = endpoint_graph(activation_paths(&analysis_program, &[0]), Vec::new());
        let after = endpoint_graph(activation_paths(&analysis_program, &[]), Vec::new());
        validate_canonical_endpoint_graph(&before, Some(&analysis_program))
            .expect("endpoint-only anchor is valid evidence");
        validate_canonical_endpoint_graph(&after, Some(&analysis_program))
            .expect("trace-empty root anchor is valid evidence");

        PairingShape::from_graph(&before)
            .expect("before pairing shape")
            .ensure_unambiguous_with(
                &PairingShape::from_graph(&after).expect("after pairing shape"),
            )
            .expect("an endpoint-only call base requires no invented counterpart");
        assert_eq!(before.activation_paths.len(), 2);
        assert_eq!(after.activation_paths.len(), 1);
    }

    #[test]
    fn reversed_outcomes_stay_bound_to_actual_invocation_ordinals() {
        let analysis_program = analysis_program();
        let paths = activation_paths(&analysis_program, &[0, 1]);
        let first = occurrence_slot(&analysis_program, 1, 0);
        let second = occurrence_slot(&analysis_program, 2, 0);
        let before = endpoint_graph(
            paths.clone(),
            vec![
                (first.clone(), RelationalIfDecisionOutcome::Then),
                (second.clone(), RelationalIfDecisionOutcome::Else),
            ],
        );
        let after = endpoint_graph(
            paths,
            vec![
                (first.clone(), RelationalIfDecisionOutcome::Else),
                (second.clone(), RelationalIfDecisionOutcome::Then),
            ],
        );
        validate_canonical_endpoint_graph(&before, Some(&analysis_program))
            .expect("valid before graph");
        validate_canonical_endpoint_graph(&after, Some(&analysis_program))
            .expect("valid after graph");
        PairingShape::from_graph(&before)
            .expect("before pairing shape")
            .ensure_unambiguous_with(
                &PairingShape::from_graph(&after).expect("after pairing shape"),
            )
            .expect("equal anchors pair without sorting by outcome");

        assert_ne!(
            before.occurrences[&first].outcome,
            after.occurrences[&first].outcome
        );
        assert_ne!(
            before.occurrences[&second].outcome,
            after.occurrences[&second].outcome
        );
        assert_eq!(
            before.activation_paths[first.activation_path]
                .step
                .invocation_ordinal,
            0
        );
        assert_eq!(
            before.activation_paths[second.activation_path]
                .step
                .invocation_ordinal,
            1
        );
    }

    #[test]
    fn first_invocation_cannot_start_at_one() {
        let analysis_program = analysis_program();
        let graph = endpoint_graph(activation_paths(&analysis_program, &[1]), Vec::new());
        let error = validate_canonical_endpoint_graph(&graph, Some(&analysis_program))
            .expect_err("missing invocation zero must fail");
        assert_eq!(
            error,
            RelationalMechanismReplayError::NonContiguousInvocationOrdinals
        );
    }

    #[test]
    fn visit_multiplicity_mismatch_fails_closed() {
        let analysis_program = analysis_program();
        let paths = activation_paths(&analysis_program, &[]);
        let visit_zero = occurrence_slot(&analysis_program, 0, 0);
        let visit_one = occurrence_slot(&analysis_program, 0, 1);
        let before = endpoint_graph(
            paths.clone(),
            vec![
                (visit_zero.clone(), RelationalIfDecisionOutcome::Then),
                (visit_one, RelationalIfDecisionOutcome::Else),
            ],
        );
        let after = endpoint_graph(paths, vec![(visit_zero, RelationalIfDecisionOutcome::Then)]);

        let error = PairingShape::from_graph(&before)
            .expect("before pairing shape")
            .ensure_unambiguous_with(
                &PairingShape::from_graph(&after).expect("after pairing shape"),
            )
            .expect_err("different visit multiplicities must not be guessed");
        assert_eq!(
            error,
            RelationalMechanismReplayError::AmbiguousEndpointPairing
        );
    }
}
