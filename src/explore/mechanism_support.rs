//! Target-conditioned support fibers over structural mechanisms.
//!
//! Complete raw signatures remain the disjoint case-partition authority. This
//! layer joins their structural assignments lazily and projects exact
//! `(Context, Before)` starter keys without materializing a case-by-node table.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;
use std::ops::Bound::{Excluded, Unbounded};

use sha2::{Digest, Sha256};

use super::authenticated_treap::{AuthenticatedTreapMap, AuthenticatedTreapValue};
use super::mechanism_incidence::{
    ClosedMechanismIncidenceRef, MechanismCaseTerminal, MechanismCaseTerminalRecord,
    MechanismIncidenceCatalogBuilder, MechanismIncidenceRoot, MechanismRequestScope,
    MechanismSignatureId, MechanismTargetDiscoveryRevision, MechanismTargetSeal,
    MechanismTargetSealId, MechanismTerminalDiscoveryRevision,
};
use super::relation::{
    MechanismRequestId, MechanismTargetId, QuestionId, RelationalCaseId, RelationalCaseRef,
    SourceKey, SuccessorKey,
};
use super::structural_mechanism::{
    StructuralCatalogRevision, StructuralEdgeId, StructuralMechanismCatalogBuilder,
    StructuralMechanismError, StructuralMechanismId, StructuralNodeId,
    StructuralQuotientClosureRoot, StructuralSignatureAssignment,
};

pub(crate) const MECHANISM_SUPPORT_VERSION: u32 = 2;
pub(crate) const MECHANISM_SUPPORT_VIEW_VERSION: u32 = 4;
pub(crate) const MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION: u32 = 2;
pub(crate) const MECHANISM_SUPPORT_FIBER_EXPR_VERSION: u32 = 1;
pub(crate) const MECHANISM_STARTER_PROJECTION_PLAN_VERSION: u32 = 2;
/// Automatic all-subject publication may inspect at most this many immutable
/// signature-fiber summaries for one row. The cap is part of the summary
/// schema, not a runtime tuning knob: crossing it yields honest wider bounds
/// and a deferred projection plan instead of a hidden full union.
pub(crate) const AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT: usize = 256;
// One broad shared-node projection can approach the full target size. Keep the
// default cache to one such derived accelerator; authority remains factorized
// in the signature fibers and callers may explicitly choose another limit.
const DEFAULT_HOT_SUBJECT_PROJECTION_LIMIT: usize = 1;

const SUPPORT_VIEW_ROOT_V4: &[u8] = b"futuruna.explore.mechanism-support-view-root.v4";
const FACTORIZED_SUBJECT_SIGNATURE_PREFIX_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-subject-signature-prefix-root.v2";
const FACTORIZED_SUBJECT_SUMMARY_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-subject-summary-root.v2";
const SUPPORT_FIBER_EXPR_ROOT_V1: &[u8] = b"futuruna.explore.mechanism-support-fiber-expr-root.v1";
const FIBER_EXPR_FACTORIZED_SUBJECT_UNION: u8 = 0x01;
const FIBER_EXPR_MATERIALIZED_PROJECTION: u8 = 0x02;
const FIBER_EXPR_POSSIBLE_SUPPORT_ENVELOPE: u8 = 0x03;
const FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE: u8 = 0x01;
const FIBER_EXPR_SOURCE_CONTEXT_BEFORE: u8 = 0x01;
const FIBER_EXPR_SUCCESSOR_AFTER: u8 = 0x01;
const STARTER_PROJECTION_PLAN_ID_V2: &[u8] =
    b"futuruna.explore.mechanism-subject-starter-projection-plan-id.v2";
const SUPPORT_FRONTIER_ROOT_V1: &[u8] = b"futuruna.explore.mechanism-support-frontier-root.v1";
const SUPPORT_CLOSURE_ROOT_V1: &[u8] = b"futuruna.explore.mechanism-support-closure-root.v1";
const SHARED_RESIDUAL_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-support-factorized-residual-root.v2";
const FIBER_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-fiber-case-index.v1";
const PENDING_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-pending-case-index.v1";
const UNAVAILABLE_CASE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unavailable-case-index.v1";
const SIGNATURE_FIBER_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-fiber-index.v1";
const TERMINAL_FACT_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-terminal-fact-index.v1";
const TARGET_STARTER_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-target-starter-index.v1";
const SUBJECT_SIGNATURE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-signature-index.v1";
const SUBJECT_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-subject-case-index.v1";
const SUBJECT_STARTER_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-starter-index.v1";
const SUBJECT_SUCCESSOR_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-successor-index.v1";
const UNASSIGNED_SIGNATURE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unassigned-signature-index.v1";
const COORDINATE_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-coordinate-value.v1";
const UNAVAILABLE_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-unavailable-value.v1";
const TERMINAL_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-terminal-value.v1";
const SIGNATURE_FIBER_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-fiber-value.v1";
const STARTER_FIBER_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-starter-fiber-value.v1";
const TARGET_STARTER_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-target-starter-value.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MechanismSupportFacet {
    Activation,
    DifferentialParticipation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MechanismSupportSubject {
    Mechanism(StructuralMechanismId),
    Node {
        facet: MechanismSupportFacet,
        node_id: StructuralNodeId,
    },
    Edge {
        facet: MechanismSupportFacet,
        edge_id: StructuralEdgeId,
    },
}

impl MechanismSupportSubject {
    pub(crate) const fn facet(self) -> Option<MechanismSupportFacet> {
        match self {
            Self::Mechanism(_) => None,
            Self::Node { facet, .. } | Self::Edge { facet, .. } => Some(facet),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportKey {
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
}

impl MechanismSupportKey {
    pub(crate) const fn new(
        scope: MechanismRequestScope,
        subject: MechanismSupportSubject,
    ) -> Self {
        Self {
            request_id: scope.request_id(),
            target: scope.target(),
            subject,
        }
    }

    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn target(self) -> MechanismTargetId {
        self.target
    }

    pub(crate) const fn facet(self) -> Option<MechanismSupportFacet> {
        self.subject.facet()
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.subject
    }
}

/// Independently typed case or distinct-starter cardinality knowledge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismSupportCount {
    Unknown {
        confirmed_lower_bound: u128,
    },
    Interval {
        lower_bound: u128,
        upper_bound: u128,
    },
    Exact(u128),
}

impl MechanismSupportCount {
    pub(crate) const fn lower_bound(self) -> u128 {
        match self {
            Self::Unknown {
                confirmed_lower_bound,
            } => confirmed_lower_bound,
            Self::Interval { lower_bound, .. } => lower_bound,
            Self::Exact(value) => value,
        }
    }

    pub(crate) const fn upper_bound(self) -> Option<u128> {
        match self {
            Self::Unknown { .. } => None,
            Self::Interval { upper_bound, .. } | Self::Exact(upper_bound) => Some(upper_bound),
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Authenticated provenance for the upper side of a distinct-starter bound.
///
/// An open target has no finite upper projection. Once sealed, an exact count
/// may be backed either by the exact inner starter/successor correlation or
/// only by saturation of the target's starter-key set. The latter does not
/// claim that residual cases cannot add successors to an already-known key.
/// Otherwise the target-wide starter projection is only a conservative upper
/// envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismStarterUpperProvenance {
    OpenOpaque,
    ExactCorrelatedInner { inner_starter_root: [u8; 32] },
    ExactStarterSetFromTargetSaturation { target_starter_root: [u8; 32] },
    ConservativeTargetProjectionUpper { target_starter_root: [u8; 32] },
}

impl MechanismStarterUpperProvenance {
    pub(crate) const fn projection_root(self) -> Option<[u8; 32]> {
        match self {
            Self::OpenOpaque => None,
            Self::ExactCorrelatedInner { inner_starter_root } => Some(inner_starter_root),
            Self::ExactStarterSetFromTargetSaturation {
                target_starter_root,
            }
            | Self::ConservativeTargetProjectionUpper {
                target_starter_root,
            } => Some(target_starter_root),
        }
    }

    pub(crate) const fn is_conservative_target_projection_upper(self) -> bool {
        matches!(self, Self::ConservativeTargetProjectionUpper { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportViewRoot([u8; 32]);

impl MechanismSupportViewRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated identity of a correlated origin-preimage fiber expression.
///
/// The expression's key space is `SourceKey = (Context, Before)` and each
/// source maps to a set of `SuccessorKey = After` members. This is an identity
/// for a factorized or materialized expression, not a public content root and
/// not authorization to serialize its typed values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportFiberExprRoot([u8; 32]);

impl MechanismSupportFiberExprRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated compact support statement for one structural subject. This
/// root commits the closed factorized authority and the bounded signature
/// prefix inspected by automatic publication; it is not a correlated starter
/// projection root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismFactorizedSubjectSummaryRoot([u8; 32]);

impl MechanismFactorizedSubjectSummaryRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authorization-neutral identity of the desired exact `(Context, Before) ->
/// After` union for one structural subject. A future public materialization
/// must derive its job identity from this plan plus explicit publication
/// authorization; this value alone never authorizes cells.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismStarterProjectionPlanId([u8; 32]);

impl MechanismStarterProjectionPlanId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closure-checked authority for paging one structural subject's exact
/// origin-preimage relation. The subject can be a whole mechanism or one
/// activation/differential node or edge facet. This token is
/// authorization-neutral: it proves which key relation may be materialized,
/// but it does not permit typed Context/Before/After values to cross a
/// publication boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismClosedSubjectStarterProjectionAuthority {
    key: MechanismSupportKey,
    question_id: QuestionId,
    projection_plan_id: MechanismStarterProjectionPlanId,
    correlated_fiber_expr_root: MechanismSupportFiberExprRoot,
    structural_root: StructuralQuotientClosureRoot,
    support_root: MechanismSupportClosureRoot,
    exact_case_count: u128,
}

impl MechanismClosedSubjectStarterProjectionAuthority {
    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.key
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.key.subject()
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.projection_plan_id
    }

    pub(crate) const fn correlated_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.correlated_fiber_expr_root
    }

    pub(crate) const fn structural_root(self) -> StructuralQuotientClosureRoot {
        self.structural_root
    }

    pub(crate) const fn support_root(self) -> MechanismSupportClosureRoot {
        self.support_root
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.exact_case_count
    }
}

/// Temporary whole-mechanism adapter for the current automatic publication
/// lane. The semantic authority above is subject-generic; this wrapper only
/// prevents the existing mechanism enumerator from fabricating a subject key
/// while publication is migrated independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismClosedStarterProjectionAuthority {
    inner: MechanismClosedSubjectStarterProjectionAuthority,
    mechanism_id: StructuralMechanismId,
}

impl MechanismClosedStarterProjectionAuthority {
    pub(crate) const fn subject_authority(
        self,
    ) -> MechanismClosedSubjectStarterProjectionAuthority {
        self.inner
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.inner.key()
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.inner.question_id()
    }

    pub(crate) const fn mechanism_id(self) -> StructuralMechanismId {
        self.mechanism_id
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.inner.projection_plan_id()
    }

    pub(crate) const fn correlated_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.inner.correlated_fiber_expr_root()
    }

    pub(crate) const fn structural_root(self) -> StructuralQuotientClosureRoot {
        self.inner.structural_root()
    }

    pub(crate) const fn support_root(self) -> MechanismSupportClosureRoot {
        self.inner.support_root()
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.inner.exact_case_count()
    }
}

impl From<MechanismClosedStarterProjectionAuthority>
    for MechanismClosedSubjectStarterProjectionAuthority
{
    fn from(authority: MechanismClosedStarterProjectionAuthority) -> Self {
        authority.inner
    }
}

/// Canonical key cursor for a mechanism starter fiber. A cursor names the
/// last emitted member; a resumed page starts strictly after it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportStarterCursor {
    source_key: SourceKey,
    successor_key: SuccessorKey,
}

impl MechanismSupportStarterCursor {
    pub(crate) const fn new(source_key: SourceKey, successor_key: SuccessorKey) -> Self {
        Self {
            source_key,
            successor_key,
        }
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }
}

/// One exact key-only member of a closed structural mechanism's starter
/// projection. Typed values remain in the relation catalog and are joined by
/// the separately authorized projection layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportStarterMember {
    raw_signature_id: MechanismSignatureId,
    case_id: RelationalCaseId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
}

impl MechanismSupportStarterMember {
    pub(crate) const fn raw_signature_id(self) -> MechanismSignatureId {
        self.raw_signature_id
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn cursor(self) -> MechanismSupportStarterCursor {
        MechanismSupportStarterCursor::new(self.source_key, self.successor_key)
    }
}

/// One bounded canonical suffix of a closed structural subject's starter
/// relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportSubjectStarterPage {
    authority: MechanismClosedSubjectStarterProjectionAuthority,
    start_after: Option<MechanismSupportStarterCursor>,
    members: Box<[MechanismSupportStarterMember]>,
    exhausted: bool,
}

impl MechanismSupportSubjectStarterPage {
    pub(crate) const fn authority(&self) -> MechanismClosedSubjectStarterProjectionAuthority {
        self.authority
    }

    pub(crate) const fn start_after(&self) -> Option<MechanismSupportStarterCursor> {
        self.start_after
    }

    pub(crate) fn members(&self) -> &[MechanismSupportStarterMember] {
        &self.members
    }

    pub(crate) const fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub(crate) fn end_cursor(&self) -> Option<MechanismSupportStarterCursor> {
        self.members
            .last()
            .map(|member| member.cursor())
            .or(self.start_after)
    }
}

/// Transitional name retained for callers which still publish whole
/// mechanisms. Its value and authority are already subject-generic.
pub(crate) type MechanismSupportStarterPage = MechanismSupportSubjectStarterPage;

/// Why the scalar distinct-starter bounds are exact or conservative. None of
/// these variants claims that correlated starter/successor cells were
/// materialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismFactorizedStarterBoundBasis {
    ExactEmpty,
    ExactFactorizedBoundCollapse,
    ExactTargetStarterSaturation { target_starter_root: [u8; 32] },
    ConservativeTargetProjectionUpper { target_starter_root: [u8; 32] },
}

/// Constant-space automatic-publication summary. Authenticated inner/outer
/// fiber-expression identities retain the correlation contract without
/// serializing values. Exact correlated cells remain a separate, explicitly
/// authorized projection job derived from the plan even when the expression
/// bounds or a scalar bound have collapsed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismFactorizedSubjectSummary {
    key: MechanismSupportKey,
    root: MechanismFactorizedSubjectSummaryRoot,
    projection_plan_id: MechanismStarterProjectionPlanId,
    inner_fiber_expr_root: MechanismSupportFiberExprRoot,
    outer_fiber_expr_root: MechanismSupportFiberExprRoot,
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    signature_scan_complete: bool,
    signature_prefix_root: [u8; 32],
    shared_residual_root: MechanismSupportResidualRoot,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_bound_basis: MechanismFactorizedStarterBoundBasis,
}

impl MechanismFactorizedSubjectSummary {
    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.key
    }

    pub(crate) const fn root(self) -> MechanismFactorizedSubjectSummaryRoot {
        self.root
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.projection_plan_id
    }

    pub(crate) const fn inner_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.inner_fiber_expr_root
    }

    pub(crate) const fn outer_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.outer_fiber_expr_root
    }

    pub(crate) fn fiber_expr_bounds_are_equal(self) -> bool {
        self.inner_fiber_expr_root == self.outer_fiber_expr_root
    }

    pub(crate) const fn contributing_signature_count(self) -> u128 {
        self.contributing_signature_count
    }

    pub(crate) const fn inspected_signature_count(self) -> u128 {
        self.inspected_signature_count
    }

    pub(crate) const fn signature_scan_complete(self) -> bool {
        self.signature_scan_complete
    }

    pub(crate) const fn signature_prefix_root(self) -> [u8; 32] {
        self.signature_prefix_root
    }

    pub(crate) const fn shared_residual_root(self) -> MechanismSupportResidualRoot {
        self.shared_residual_root
    }

    pub(crate) const fn case_count(self) -> MechanismSupportCount {
        self.case_count
    }

    pub(crate) const fn starter_count(self) -> MechanismSupportCount {
        self.starter_count
    }

    pub(crate) const fn starter_bound_basis(self) -> MechanismFactorizedStarterBoundBasis {
        self.starter_bound_basis
    }
}

/// Authenticated resumable prefix of the request-level support join. Unlike a
/// final support closure, this deliberately binds operational branch revisions
/// and cursors so a checkpoint cannot resume on a divergent discovery fork.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportFrontierRoot([u8; 32]);

impl MechanismSupportFrontierRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact replay cursors for the three independently growing support lanes.
/// The outer checkpoint event authenticates this tuple together with the
/// frontier root; no lane may be inferred from another lane's progress.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportCheckpointCursor {
    target_discovery: u128,
    terminal_discovery: u128,
    structural_assignment: u128,
}

impl MechanismSupportCheckpointCursor {
    pub(crate) const fn new(
        target_discovery: u128,
        terminal_discovery: u128,
        structural_assignment: u128,
    ) -> Self {
        Self {
            target_discovery,
            terminal_discovery,
            structural_assignment,
        }
    }

    pub(crate) const fn target_discovery(self) -> u128 {
        self.target_discovery
    }

    pub(crate) const fn terminal_discovery(self) -> u128 {
        self.terminal_discovery
    }

    pub(crate) const fn structural_assignment(self) -> u128 {
        self.structural_assignment
    }
}

/// Final request-level identity of the factorized case/starter support join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportClosureRoot([u8; 32]);

impl MechanismSupportClosureRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Compact closure receipt. Structural subject views remain lazy derivations
/// over the authenticated signature fibers; closure never emits node x case
/// rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportClosureReceipt {
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    target_seal_id: MechanismTargetSealId,
    incidence_root: super::mechanism_incidence::MechanismIncidenceRoot,
    structural_root: StructuralQuotientClosureRoot,
    target_case_count: u128,
    successful_case_count: u128,
    unavailable_case_count: u128,
    signature_fiber_count: u128,
    target_starter_count: u128,
    residual_root: MechanismSupportResidualRoot,
    root: MechanismSupportClosureRoot,
}

impl MechanismSupportClosureReceipt {
    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn target(self) -> MechanismTargetId {
        self.target
    }

    pub(crate) const fn target_seal_id(self) -> MechanismTargetSealId {
        self.target_seal_id
    }

    pub(crate) const fn incidence_root(self) -> super::mechanism_incidence::MechanismIncidenceRoot {
        self.incidence_root
    }

    pub(crate) const fn structural_root(self) -> StructuralQuotientClosureRoot {
        self.structural_root
    }

    pub(crate) const fn target_case_count(self) -> u128 {
        self.target_case_count
    }

    pub(crate) const fn successful_case_count(self) -> u128 {
        self.successful_case_count
    }

    pub(crate) const fn unavailable_case_count(self) -> u128 {
        self.unavailable_case_count
    }

    pub(crate) const fn signature_fiber_count(self) -> u128 {
        self.signature_fiber_count
    }

    pub(crate) const fn target_starter_count(self) -> u128 {
        self.target_starter_count
    }

    pub(crate) const fn residual_root(self) -> MechanismSupportResidualRoot {
        self.residual_root
    }

    pub(crate) const fn root(self) -> MechanismSupportClosureRoot {
        self.root
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportResidualRoot([u8; 32]);

impl MechanismSupportResidualRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug)]
struct SignatureCaseFiber {
    cases: BTreeMap<RelationalCaseId, (SourceKey, SuccessorKey)>,
    starters: BTreeMap<SourceKey, BTreeSet<SuccessorKey>>,
    authenticated_cases: AuthenticatedTreapMap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetCaseCoordinate {
    source: SourceKey,
    successor: SuccessorKey,
    terminal: Option<MechanismCaseTerminal>,
}

impl SignatureCaseFiber {
    fn new() -> Self {
        Self {
            cases: BTreeMap::new(),
            starters: BTreeMap::new(),
            authenticated_cases: AuthenticatedTreapMap::new(FIBER_CASE_INDEX_V1),
        }
    }
}

/// Exact inner starter/successor fiber plus the one shared unresolved frontier
/// which may still add support for the subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportView {
    key: MechanismSupportKey,
    root: MechanismSupportViewRoot,
    inner_fiber_expr_root: MechanismSupportFiberExprRoot,
    outer_fiber_expr_root: MechanismSupportFiberExprRoot,
    inner_signature_root: [u8; 32],
    inner_case_root: [u8; 32],
    inner_starter_root: [u8; 32],
    shared_residual_root: MechanismSupportResidualRoot,
    target_frontier_open: bool,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_upper_provenance: MechanismStarterUpperProvenance,
}

impl MechanismSupportView {
    pub(crate) const fn key(&self) -> MechanismSupportKey {
        self.key
    }

    pub(crate) const fn root(&self) -> MechanismSupportViewRoot {
        self.root
    }

    pub(crate) const fn inner_fiber_expr_root(&self) -> MechanismSupportFiberExprRoot {
        self.inner_fiber_expr_root
    }

    pub(crate) const fn outer_fiber_expr_root(&self) -> MechanismSupportFiberExprRoot {
        self.outer_fiber_expr_root
    }

    pub(crate) fn fiber_expr_bounds_are_equal(&self) -> bool {
        self.inner_fiber_expr_root == self.outer_fiber_expr_root
    }

    pub(crate) const fn case_count(&self) -> MechanismSupportCount {
        self.case_count
    }

    pub(crate) const fn starter_count(&self) -> MechanismSupportCount {
        self.starter_count
    }

    pub(crate) const fn starter_upper_provenance(&self) -> MechanismStarterUpperProvenance {
        self.starter_upper_provenance
    }

    pub(crate) const fn target_frontier_is_open(&self) -> bool {
        self.target_frontier_open
    }

    pub(crate) const fn shared_residual_root(&self) -> MechanismSupportResidualRoot {
        self.shared_residual_root
    }

    pub(crate) const fn inner_signature_root(&self) -> [u8; 32] {
        self.inner_signature_root
    }

    pub(crate) const fn inner_case_root(&self) -> [u8; 32] {
        self.inner_case_root
    }

    /// Commits the correlated `SourceKey -> SuccessorKey set` projection.
    /// Scalar/range summaries are navigation metadata and never replace it.
    pub(crate) const fn inner_starter_root(&self) -> [u8; 32] {
        self.inner_starter_root
    }
}

#[derive(Clone, Debug)]
struct SubjectProjectionCache {
    signature_index: AuthenticatedTreapMap,
    case_index: AuthenticatedTreapMap,
    starter_index: AuthenticatedTreapMap,
    successor_fibers: BTreeMap<SourceKey, AuthenticatedTreapMap>,
}

impl SubjectProjectionCache {
    fn new() -> Self {
        Self {
            signature_index: AuthenticatedTreapMap::new(SUBJECT_SIGNATURE_INDEX_V1),
            case_index: AuthenticatedTreapMap::new(SUBJECT_CASE_INDEX_V1),
            starter_index: AuthenticatedTreapMap::new(SUBJECT_STARTER_INDEX_V1),
            successor_fibers: BTreeMap::new(),
        }
    }

    fn case_count(&self) -> u128 {
        self.case_index.total_weight()
    }

    fn starter_count(&self) -> u128 {
        self.starter_index.total_weight()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FactorizedResidualSummary {
    root: MechanismSupportResidualRoot,
    case_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignatureFiberSummary {
    root: [u8; 32],
    case_count: u128,
    starter_count: u128,
}

/// Request/target-local support authority. Target coordinates are accepted
/// once, then imported from checked raw-incidence terminals. Pending replay
/// and replay unavailability remain in one shared possible-support residual;
/// neither is evidence that a structural subject was absent.
#[derive(Clone, Debug)]
pub(crate) struct MechanismSupportCatalogBuilder {
    scope: MechanismRequestScope,
    closure: Option<MechanismSupportClosureReceipt>,
    target_seal: Option<MechanismTargetSeal>,
    target: BTreeMap<RelationalCaseId, TargetCaseCoordinate>,
    target_starter_refcounts: BTreeMap<SourceKey, u128>,
    target_starter_index: AuthenticatedTreapMap,
    target_discovery_cursor: usize,
    target_discovery_revision: Option<MechanismTargetDiscoveryRevision>,
    pending_cases: AuthenticatedTreapMap,
    unavailable_cases: AuthenticatedTreapMap,
    terminal_fact_index: AuthenticatedTreapMap,
    signature_fibers: BTreeMap<MechanismSignatureId, SignatureCaseFiber>,
    signature_fiber_index: AuthenticatedTreapMap,
    unassigned_signature_index: AuthenticatedTreapMap,
    terminal_discovery_cursor: usize,
    terminal_discovery_revision: Option<MechanismTerminalDiscoveryRevision>,
    structural_assignment_cursor: usize,
    structural_assignment_revision: Option<StructuralCatalogRevision>,
    subject_projection_cache: BTreeMap<MechanismSupportSubject, SubjectProjectionCache>,
    subject_projection_lru: VecDeque<MechanismSupportSubject>,
    subject_projection_cache_limit: usize,
}

impl MechanismSupportCatalogBuilder {
    pub(crate) fn new(scope: MechanismRequestScope) -> Self {
        Self::with_projection_cache_limit(scope, DEFAULT_HOT_SUBJECT_PROJECTION_LIMIT)
    }

    pub(crate) fn with_projection_cache_limit(
        scope: MechanismRequestScope,
        subject_projection_cache_limit: usize,
    ) -> Self {
        Self {
            scope,
            closure: None,
            target_seal: None,
            target: BTreeMap::new(),
            target_starter_refcounts: BTreeMap::new(),
            target_starter_index: AuthenticatedTreapMap::new(TARGET_STARTER_INDEX_V1),
            target_discovery_cursor: 0,
            target_discovery_revision: None,
            pending_cases: AuthenticatedTreapMap::new(PENDING_CASE_INDEX_V1),
            unavailable_cases: AuthenticatedTreapMap::new(UNAVAILABLE_CASE_INDEX_V1),
            terminal_fact_index: AuthenticatedTreapMap::new(TERMINAL_FACT_INDEX_V1),
            signature_fibers: BTreeMap::new(),
            signature_fiber_index: AuthenticatedTreapMap::new(SIGNATURE_FIBER_INDEX_V1),
            unassigned_signature_index: AuthenticatedTreapMap::new(UNASSIGNED_SIGNATURE_INDEX_V1),
            terminal_discovery_cursor: 0,
            terminal_discovery_revision: None,
            structural_assignment_cursor: 0,
            structural_assignment_revision: None,
            subject_projection_cache: BTreeMap::new(),
            subject_projection_lru: VecDeque::new(),
            subject_projection_cache_limit: subject_projection_cache_limit.max(1),
        }
    }

    pub(crate) const fn closure(&self) -> Option<MechanismSupportClosureReceipt> {
        self.closure
    }

    /// Return the checked request/target scope needed to address a lazy
    /// subject view without reconstructing either coordinate from its hashes.
    pub(crate) const fn scope(&self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn target_discovery_cursor(&self) -> usize {
        self.target_discovery_cursor
    }

    pub(crate) const fn checkpoint_cursor(&self) -> MechanismSupportCheckpointCursor {
        MechanismSupportCheckpointCursor::new(
            self.target_discovery_cursor as u128,
            self.terminal_discovery_cursor as u128,
            self.structural_assignment_cursor as u128,
        )
    }

    pub(crate) fn accept_target_case(
        &mut self,
        incidence: &MechanismIncidenceCatalogBuilder,
        case: RelationalCaseRef<'_>,
    ) -> Result<bool, MechanismSupportError> {
        self.validate_incidence_scope(incidence)?;
        self.validate_target_prefix(incidence)?;
        self.validate_terminal_prefix(incidence)?;
        let case_id = case.case_id();
        let source = case.source_key();
        let successor = case.successor_key();
        if RelationalCaseId::derive(case.relation_id(), source, successor) != case_id {
            return Err(MechanismSupportError::TargetCaseIdentityMismatch);
        }
        if !incidence.contains_target_case(case_id) {
            return Err(MechanismSupportError::UnknownIncidenceTargetCase);
        }
        match self.target.entry(case_id) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get().source == source && entry.get().successor == successor {
                    Ok(false)
                } else {
                    Err(MechanismSupportError::TargetCaseIdentityMismatch)
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                if self.closure.is_some() {
                    return Err(MechanismSupportError::CatalogClosed);
                }
                if self.target_seal.is_some() {
                    return Err(MechanismSupportError::TargetCaseSetSealed);
                }
                let expected = incidence
                    .target_discovery_at(self.target_discovery_cursor)
                    .ok_or(MechanismSupportError::UnknownIncidenceTargetCase)?;
                if expected != case_id {
                    return Err(MechanismSupportError::TargetDiscoveryOrderMismatch);
                }
                let prior_refcount = self
                    .target_starter_refcounts
                    .get(&source)
                    .copied()
                    .unwrap_or(0);
                let next_refcount = prior_refcount
                    .checked_add(1)
                    .ok_or(MechanismSupportError::CountOverflow)?;
                let next_target_cursor = self
                    .target_discovery_cursor
                    .checked_add(1)
                    .ok_or(MechanismSupportError::CountOverflow)?;
                let mut next_pending = self.pending_cases.clone();
                next_pending
                    .insert(
                        case_key(case_id),
                        AuthenticatedTreapValue::new(coordinate_value_digest(source, successor), 1),
                    )
                    .map_err(|_| MechanismSupportError::AuthenticatedIndex("pending cases"))?;
                let mut next_target_starters = self.target_starter_index.clone();
                set_authenticated_value(
                    &mut next_target_starters,
                    source.bytes().to_vec().into_boxed_slice(),
                    target_starter_value(source, next_refcount),
                    "target starters",
                )?;
                self.pending_cases = next_pending;
                self.target_starter_index = next_target_starters;
                entry.insert(TargetCaseCoordinate {
                    source,
                    successor,
                    terminal: None,
                });
                self.target_starter_refcounts.insert(source, next_refcount);
                self.target_discovery_cursor = next_target_cursor;
                self.target_discovery_revision =
                    incidence.target_discovery_prefix_revision(self.target_discovery_cursor);
                Ok(true)
            }
        }
    }

    /// Attach closure to the same stable support stream once the raw target
    /// has sealed. Until then, views honestly carry an opaque open-target
    /// obligation and `Unknown(lower)` cardinalities.
    pub(crate) fn attach_target_seal(
        &mut self,
        incidence: &MechanismIncidenceCatalogBuilder,
    ) -> Result<bool, MechanismSupportError> {
        self.validate_incidence_scope(incidence)?;
        self.validate_target_prefix(incidence)?;
        self.validate_terminal_prefix(incidence)?;
        let seal = incidence
            .target_seal()
            .ok_or(MechanismSupportError::TargetSealUnavailable)?;
        seal.validate_identity()
            .map_err(|_| MechanismSupportError::InvalidTargetSeal)?;
        if seal.scope() != self.scope {
            return Err(MechanismSupportError::RequestMismatch);
        }
        if self.target_discovery_cursor != incidence.target_discovery_count() {
            return Err(MechanismSupportError::TargetCaseSetIncomplete);
        }
        match self.target_seal.as_ref() {
            Some(existing) if existing == seal => return Ok(false),
            Some(_) => return Err(MechanismSupportError::TargetSealConflict),
            None => {}
        }
        // Every coordinate was accepted in the exact authenticated incidence
        // discovery order, and `validate_target_prefix` has just checked that
        // complete prefix revision. Rebuilding the canonical target-set root
        // here would add an O(N) burst to the first otherwise-bounded final
        // checkpoint without adding authority.
        if self.target.len() as u128 != seal.target_case_count() {
            return Err(MechanismSupportError::TargetCaseSetIncomplete);
        }
        self.target_seal = Some(seal.clone());
        Ok(true)
    }

    /// Consume only the new terminal discovery suffix already accepted by the
    /// live raw-incidence catalog. Full cumulative snapshots remain a restore
    /// or final-audit format rather than the per-event streaming path.
    pub(crate) fn sync_incidence_terminals_through(
        &mut self,
        incidence: &MechanismIncidenceCatalogBuilder,
        structural: &StructuralMechanismCatalogBuilder,
        terminal_discovery_cursor: u128,
    ) -> Result<usize, MechanismSupportError> {
        self.validate_incidence_scope(incidence)?;
        self.validate_target_prefix(incidence)?;
        self.validate_terminal_prefix(incidence)?;
        if structural.request_id() != self.scope.request_id() {
            return Err(MechanismSupportError::RequestMismatch);
        }
        let current = self.terminal_discovery_cursor as u128;
        let available = incidence.terminal_discovery_count() as u128;
        if terminal_discovery_cursor < current || terminal_discovery_cursor > available {
            return Err(MechanismSupportError::TerminalDiscoveryCursorRegression);
        }
        let requested = usize::try_from(terminal_discovery_cursor)
            .expect("a cursor bounded by an in-memory terminal count fits usize");
        let suffix = &incidence.terminal_discovery_suffix(self.terminal_discovery_cursor)
            [..requested - self.terminal_discovery_cursor];
        if self.closure.is_some() && !suffix.is_empty() {
            return Err(MechanismSupportError::CatalogClosed);
        }
        for record in suffix.iter().copied() {
            if !self.target.contains_key(&record.case_id()) {
                return Err(MechanismSupportError::UnknownTargetCase);
            }
            self.preflight_terminal(record)?;
        }
        let mut changed = 0usize;
        for record in suffix.iter().copied() {
            if self.accept_checked_terminal(record, structural)? {
                changed = changed
                    .checked_add(1)
                    .ok_or(MechanismSupportError::CountOverflow)?;
            }
            self.terminal_discovery_cursor = self
                .terminal_discovery_cursor
                .checked_add(1)
                .ok_or(MechanismSupportError::CountOverflow)?;
            self.terminal_discovery_revision =
                incidence.terminal_discovery_prefix_revision(self.terminal_discovery_cursor);
        }
        Ok(changed)
    }

    /// Furthest terminal cursor reachable within one bounded lane delta and
    /// the already imported target-coordinate prefix. Stops at the first
    /// terminal whose target coordinate is not yet available.
    pub(crate) fn bounded_terminal_discovery_cursor(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
        maximum_delta: usize,
    ) -> Result<u128, MechanismSupportError> {
        self.validate_incidence_scope(incidence)?;
        self.validate_target_prefix(incidence)?;
        self.validate_terminal_prefix(incidence)?;
        let consumable = incidence
            .terminal_discovery_suffix(self.terminal_discovery_cursor)
            .iter()
            .take(maximum_delta)
            .take_while(|record| self.target.contains_key(&record.case_id()))
            .count();
        Ok(self
            .terminal_discovery_cursor
            .checked_add(consumable)
            .ok_or(MechanismSupportError::CountOverflow)? as u128)
    }

    /// Consume only the checked structural-assignment suffix. Removing one
    /// signature from the unassigned manifest is independent of the number of
    /// concrete cases in that signature's fiber.
    pub(crate) fn sync_structural_assignments(
        &mut self,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<usize, MechanismSupportError> {
        self.sync_structural_assignments_through(
            structural,
            structural.assignment_discovery_count() as u128,
        )
    }

    pub(crate) fn sync_structural_assignments_through(
        &mut self,
        structural: &StructuralMechanismCatalogBuilder,
        structural_assignment_cursor: u128,
    ) -> Result<usize, MechanismSupportError> {
        if structural.request_id() != self.scope.request_id() {
            return Err(MechanismSupportError::RequestMismatch);
        }
        let current = self.structural_assignment_cursor as u128;
        let available = structural.assignment_discovery_count() as u128;
        if structural_assignment_cursor < current || structural_assignment_cursor > available {
            return Err(MechanismSupportError::StructuralAssignmentCursorRegression);
        }
        self.validate_structural_assignment_prefix(structural)?;
        let requested = usize::try_from(structural_assignment_cursor)
            .expect("a cursor bounded by an in-memory assignment count fits usize");
        let suffix = &structural.assignment_discovery_suffix(self.structural_assignment_cursor)
            [..requested - self.structural_assignment_cursor];
        if self.closure.is_some() && !suffix.is_empty() {
            return Err(MechanismSupportError::CatalogClosed);
        }
        for signature_id in suffix.iter().copied() {
            let assignment = structural
                .assignment(signature_id)
                .ok_or(MechanismSupportError::UnknownStructuralAssignment)?;
            if assignment.signature_id() != signature_id
                || signature_id.request_id() != self.scope.request_id()
            {
                return Err(MechanismSupportError::RequestMismatch);
            }
        }
        for signature_id in suffix.iter().copied() {
            let assignment = structural
                .assignment(signature_id)
                .expect("checked structural assignment remains present");
            if authenticated_contains(
                &self.unassigned_signature_index,
                &signature_key(signature_id),
                "unassigned signatures",
            )? {
                self.unassigned_signature_index
                    .remove(&signature_key(signature_id))
                    .map_err(|_| {
                        MechanismSupportError::AuthenticatedIndex("unassigned signatures")
                    })?;
            }
            self.extend_cached_subjects_for_assignment(signature_id, assignment);
            self.structural_assignment_cursor = self
                .structural_assignment_cursor
                .checked_add(1)
                .ok_or(MechanismSupportError::CountOverflow)?;
            self.structural_assignment_revision =
                structural.assignment_discovery_prefix_revision(self.structural_assignment_cursor);
        }
        let consumed = suffix.len();
        Ok(consumed)
    }

    fn validate_incidence_scope(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
    ) -> Result<(), MechanismSupportError> {
        if incidence.scope() != self.scope {
            return Err(MechanismSupportError::RequestMismatch);
        }
        Ok(())
    }

    fn validate_target_prefix(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
    ) -> Result<(), MechanismSupportError> {
        if self.target_discovery_cursor > incidence.target_discovery_count() {
            return Err(MechanismSupportError::TargetDiscoveryCursorRegression);
        }
        let actual = incidence
            .target_discovery_prefix_revision(self.target_discovery_cursor)
            .ok_or(MechanismSupportError::TargetDiscoveryCursorRegression)?;
        if self
            .target_discovery_revision
            .is_some_and(|expected| actual != expected)
        {
            return Err(MechanismSupportError::TargetDiscoveryPrefixConflict);
        }
        Ok(())
    }

    fn validate_terminal_prefix(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
    ) -> Result<(), MechanismSupportError> {
        let actual = incidence
            .terminal_discovery_prefix_revision(self.terminal_discovery_cursor)
            .ok_or(MechanismSupportError::TerminalDiscoveryCursorRegression)?;
        if let Some(expected) = self.terminal_discovery_revision {
            if actual != expected {
                return Err(MechanismSupportError::TerminalDiscoveryPrefixConflict);
            }
        }
        Ok(())
    }

    fn validate_structural_assignment_prefix(
        &self,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<(), MechanismSupportError> {
        let actual = structural
            .assignment_discovery_prefix_revision(self.structural_assignment_cursor)
            .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;
        if self
            .structural_assignment_revision
            .is_some_and(|expected| actual != expected)
        {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        Ok(())
    }

    fn preflight_terminal(
        &self,
        record: MechanismCaseTerminalRecord,
    ) -> Result<(), MechanismSupportError> {
        let coordinate = self
            .target
            .get(&record.case_id())
            .ok_or(MechanismSupportError::UnknownTargetCase)?;
        let terminal = record.terminal();
        if let MechanismCaseTerminal::Incidence { signature_id, .. } = terminal {
            if signature_id.request_id() != self.scope.request_id() {
                return Err(MechanismSupportError::RequestMismatch);
            }
        }
        match coordinate.terminal {
            Some(existing) if existing != terminal => Err(MechanismSupportError::TerminalConflict),
            Some(_) => Ok(()),
            None if !authenticated_contains(
                &self.pending_cases,
                &case_key(record.case_id()),
                "pending cases",
            )? =>
            {
                Err(MechanismSupportError::ResidualPartitionConflict)
            }
            None => Ok(()),
        }
    }

    fn accept_checked_terminal(
        &mut self,
        record: MechanismCaseTerminalRecord,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<bool, MechanismSupportError> {
        let coordinate = *self
            .target
            .get(&record.case_id())
            .ok_or(MechanismSupportError::UnknownTargetCase)?;
        let terminal = record.terminal();
        match coordinate.terminal {
            Some(existing) if existing == terminal => return Ok(false),
            Some(_) => return Err(MechanismSupportError::TerminalConflict),
            None => {}
        }

        let mut next_pending = self.pending_cases.clone();
        next_pending
            .remove(&case_key(record.case_id()))
            .map_err(|_| MechanismSupportError::AuthenticatedIndex("pending cases"))?;
        let mut next_terminal_facts = self.terminal_fact_index.clone();
        next_terminal_facts
            .insert(
                case_key(record.case_id()),
                AuthenticatedTreapValue::new(
                    terminal_value_digest(coordinate.source, coordinate.successor, terminal),
                    1,
                ),
            )
            .map_err(|_| MechanismSupportError::AuthenticatedIndex("terminal facts"))?;

        match terminal {
            MechanismCaseTerminal::Incidence { signature_id, .. } => {
                let existing = self.signature_fibers.get(&signature_id);
                if existing.is_some_and(|fiber| fiber.cases.contains_key(&record.case_id())) {
                    return Err(MechanismSupportError::SignatureConflict);
                }
                let mut next_cases = existing.map_or_else(
                    || AuthenticatedTreapMap::new(FIBER_CASE_INDEX_V1),
                    |fiber| fiber.authenticated_cases.clone(),
                );
                next_cases
                    .insert(
                        case_key(record.case_id()),
                        AuthenticatedTreapValue::new(
                            coordinate_value_digest(coordinate.source, coordinate.successor),
                            1,
                        ),
                    )
                    .map_err(|_| {
                        MechanismSupportError::AuthenticatedIndex("signature coordinates")
                    })?;
                let prior_starters = existing.map_or(0usize, |fiber| fiber.starters.len());
                let source_is_new =
                    existing.is_none_or(|fiber| !fiber.starters.contains_key(&coordinate.source));
                let starter_count = prior_starters
                    .checked_add(usize::from(source_is_new))
                    .ok_or(MechanismSupportError::CountOverflow)?
                    as u128;
                let summary = SignatureFiberSummary {
                    root: next_cases.root_hash(),
                    case_count: next_cases.total_weight(),
                    starter_count,
                };
                let summary_value = signature_fiber_value(signature_id, summary);

                let mut next_fiber_index = self.signature_fiber_index.clone();
                set_authenticated_value(
                    &mut next_fiber_index,
                    signature_key(signature_id),
                    summary_value,
                    "signature fibers",
                )?;
                let mut next_unassigned = self.unassigned_signature_index.clone();
                if structural.assignment(signature_id).is_none() {
                    set_authenticated_value(
                        &mut next_unassigned,
                        signature_key(signature_id),
                        summary_value,
                        "unassigned signatures",
                    )?;
                } else if authenticated_contains(
                    &next_unassigned,
                    &signature_key(signature_id),
                    "unassigned signatures",
                )? {
                    next_unassigned
                        .remove(&signature_key(signature_id))
                        .map_err(|_| {
                            MechanismSupportError::AuthenticatedIndex("unassigned signatures")
                        })?;
                }

                self.pending_cases = next_pending;
                self.terminal_fact_index = next_terminal_facts;
                self.signature_fiber_index = next_fiber_index;
                self.unassigned_signature_index = next_unassigned;
                let fiber = self
                    .signature_fibers
                    .entry(signature_id)
                    .or_insert_with(SignatureCaseFiber::new);
                fiber.authenticated_cases = next_cases;
                fiber
                    .cases
                    .insert(record.case_id(), (coordinate.source, coordinate.successor));
                fiber
                    .starters
                    .entry(coordinate.source)
                    .or_default()
                    .insert(coordinate.successor);
                self.extend_cached_subjects_for_case(
                    structural,
                    signature_id,
                    summary,
                    record.case_id(),
                    coordinate.source,
                    coordinate.successor,
                );
            }
            MechanismCaseTerminal::Unavailable { reason_id } => {
                let mut next_unavailable = self.unavailable_cases.clone();
                next_unavailable
                    .insert(
                        case_key(record.case_id()),
                        AuthenticatedTreapValue::new(
                            unavailable_value_digest(
                                coordinate.source,
                                coordinate.successor,
                                reason_id.bytes(),
                            ),
                            1,
                        ),
                    )
                    .map_err(|_| MechanismSupportError::AuthenticatedIndex("unavailable cases"))?;
                self.pending_cases = next_pending;
                self.unavailable_cases = next_unavailable;
                self.terminal_fact_index = next_terminal_facts;
            }
        }
        self.target
            .get_mut(&record.case_id())
            .expect("checked target coordinate remains present")
            .terminal = Some(terminal);
        Ok(true)
    }

    /// Projection caches are derived accelerators, never authority. Updating a
    /// hot subject therefore either succeeds atomically or invalidates that
    /// one cache so its next observation is rebuilt from authoritative fibers.
    fn extend_cached_subjects_for_assignment(
        &mut self,
        signature_id: MechanismSignatureId,
        assignment: &StructuralSignatureAssignment,
    ) {
        let Some(fiber) = self.signature_fibers.get(&signature_id) else {
            return;
        };
        let mut invalid = Vec::new();
        // Structural assignments may contain many nodes and edges. The
        // projection cache is deliberately bounded, so scan that cache and
        // membership-test its subjects instead of allocating every subject
        // in the assignment during a checkpoint quantum.
        for (subject, projection) in &mut self.subject_projection_cache {
            if assignment_supports_subject(assignment, *subject)
                && insert_signature_fiber(projection, signature_id, fiber).is_err()
            {
                invalid.push(*subject);
            }
        }
        for subject in invalid {
            self.invalidate_subject_projection(subject);
        }
    }

    fn extend_cached_subjects_for_case(
        &mut self,
        structural: &StructuralMechanismCatalogBuilder,
        signature_id: MechanismSignatureId,
        summary: SignatureFiberSummary,
        case_id: RelationalCaseId,
        source: SourceKey,
        successor: SuccessorKey,
    ) {
        let Some(assignment) = structural.assignment(signature_id) else {
            return;
        };
        let mut invalid = Vec::new();
        // The cache is deliberately tiny while a structural assignment may
        // contain many nodes and edges. Scan the bounded cache and membership-
        // test its subjects instead of allocating the full assignment subject
        // list for every case in the signature fiber.
        for (subject, projection) in &mut self.subject_projection_cache {
            if assignment_supports_subject(assignment, *subject)
                && insert_subject_case(
                    projection,
                    signature_id,
                    summary,
                    case_id,
                    source,
                    successor,
                )
                .is_err()
            {
                invalid.push(*subject);
            }
        }
        for subject in invalid {
            self.invalidate_subject_projection(subject);
        }
    }

    fn install_subject_projection(
        &mut self,
        subject: MechanismSupportSubject,
        projection: SubjectProjectionCache,
    ) {
        while self.subject_projection_cache.len() >= self.subject_projection_cache_limit {
            let Some(evicted) = self.subject_projection_lru.pop_front() else {
                break;
            };
            self.subject_projection_cache.remove(&evicted);
        }
        self.subject_projection_cache.insert(subject, projection);
        self.touch_subject_projection(subject);
    }

    fn touch_subject_projection(&mut self, subject: MechanismSupportSubject) {
        if let Some(index) = self
            .subject_projection_lru
            .iter()
            .position(|cached| *cached == subject)
        {
            self.subject_projection_lru.remove(index);
        }
        self.subject_projection_lru.push_back(subject);
    }

    fn invalidate_subject_projection(&mut self, subject: MechanismSupportSubject) {
        self.subject_projection_cache.remove(&subject);
        self.subject_projection_lru
            .retain(|cached| *cached != subject);
    }

    pub(crate) fn target_is_complete(&self) -> bool {
        self.target_seal.is_some()
    }

    /// Catch the factorized support join up to one journal prefix and mint a
    /// sparse pause/checkpoint root. Target coordinates are intentionally
    /// supplied at their own events because only the outer relation catalog
    /// can resolve a CaseId to checked SourceKey/SuccessorKey values.
    pub(crate) fn checkpoint_frontier(
        &mut self,
        incidence: &MechanismIncidenceCatalogBuilder,
        closed_incidence_root: MechanismIncidenceRoot,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismSupportFrontierRoot, MechanismSupportError> {
        self.validate_incidence_scope(incidence)?;
        self.validate_target_prefix(incidence)?;
        self.validate_terminal_prefix(incidence)?;
        if structural.request_id() != self.scope.request_id() {
            return Err(MechanismSupportError::RequestMismatch);
        }
        self.validate_structural_assignment_prefix(structural)?;
        if self.target_discovery_cursor == incidence.target_discovery_count()
            && incidence.target_seal().is_some()
        {
            self.attach_target_seal(incidence)?;
        }
        let target_revision = incidence
            .target_discovery_prefix_revision(self.target_discovery_cursor)
            .ok_or(MechanismSupportError::TargetDiscoveryCursorRegression)?;
        let terminal_revision = incidence
            .terminal_discovery_prefix_revision(self.terminal_discovery_cursor)
            .ok_or(MechanismSupportError::TerminalDiscoveryCursorRegression)?;
        let structural_revision = structural
            .assignment_discovery_prefix_revision(self.structural_assignment_cursor)
            .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;
        let residual = self.factorized_residual()?;
        let mut encoder = SupportEncoder::new(SUPPORT_FRONTIER_ROOT_V1);
        encoder.u32(MECHANISM_SUPPORT_VERSION);
        encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut encoder, self.scope.target());
        encode_optional_target_seal(&mut encoder, self.target_seal.as_ref());
        // Raw incidence is already durably closed before support lifecycle
        // starts. Its stored closure root is immutable authority; recomputing
        // that canonical O(N) root in every sparse checkpoint would defeat
        // the bounded replay protocol.
        encoder.digest(closed_incidence_root.bytes());
        encoder.digest(structural.revision().bytes());
        encoder.digest(structural.assignment_root());
        encoder.u128(structural.assignment_count() as u128);
        encoder.u128(self.target_discovery_cursor as u128);
        encoder.digest(target_revision.bytes());
        encoder.u128(self.terminal_discovery_cursor as u128);
        encoder.digest(terminal_revision.bytes());
        encoder.u128(self.structural_assignment_cursor as u128);
        encoder.digest(structural_revision.bytes());
        encode_authenticated_index(&mut encoder, &self.pending_cases);
        encode_authenticated_index(&mut encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut encoder, &self.unassigned_signature_index);
        encoder.digest(self.target_starter_index.root_hash());
        encoder.u128(self.target_starter_index.total_weight());
        encoder.digest(residual.root.bytes());
        encoder.u128(residual.case_count);
        Ok(MechanismSupportFrontierRoot(encoder.finish()))
    }

    /// Derive the exact request-level closure after both upstream authorities
    /// have closed, without crossing the semantic close barrier. Unavailable
    /// cases are a legitimate stable residual: they keep affected-subject
    /// counts interval-valued but do not prevent durable support closure.
    pub(super) fn derive_closure(
        &mut self,
        closed_incidence: ClosedMechanismIncidenceRef<'_>,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismSupportClosureReceipt, MechanismSupportError> {
        let incidence = closed_incidence.builder();
        if !incidence.frontier_is_complete()
            || closed_incidence.request_id() != self.scope.request_id()
            || closed_incidence.root() != incidence.root()
            || incidence.target_seal() != Some(closed_incidence.target_seal())
        {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "immutable raw incidence closure",
            ));
        }
        if self.target_discovery_cursor != incidence.target_discovery_count()
            || self.terminal_discovery_cursor != incidence.terminal_discovery_count()
            || self.structural_assignment_cursor != structural.assignment_discovery_count()
        {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "support discovery coverage",
            ));
        }
        self.attach_target_seal(incidence)?;
        let structural_closure = structural
            .validate_closure_against_expected_signatures(
                closed_incidence.signature_definition_count() as u128,
                closed_incidence.signature_ids(),
            )
            .map_err(MechanismSupportError::StructuralClosure)?;
        let target_seal = self
            .target_seal
            .as_ref()
            .expect("checked complete incidence supplied its target seal");
        let target_cases = self.target.len() as u128;
        let terminal_cases = self.terminal_fact_index.total_weight();
        let successful_cases = self.signature_fiber_index.total_weight();
        let unavailable_cases = self.unavailable_cases.total_weight();
        let signature_fibers = self.signature_fiber_index.entry_count();
        let target_starters = self.target_starter_index.total_weight();
        let residual = self.factorized_residual()?;
        if self.pending_cases.entry_count() != 0
            || self.pending_cases.total_weight() != 0
            || self.unassigned_signature_index.entry_count() != 0
            || self.unassigned_signature_index.total_weight() != 0
            || terminal_cases != target_cases
            || successful_cases
                .checked_add(unavailable_cases)
                .ok_or(MechanismSupportError::CountOverflow)?
                != target_cases
            || signature_fibers != self.signature_fibers.len() as u128
            || target_starters != self.target_starter_refcounts.len() as u128
            || residual.case_count != unavailable_cases
            || self
                .target
                .values()
                .any(|coordinate| coordinate.terminal.is_none())
        {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "factorized support conservation",
            ));
        }

        let incidence_root = closed_incidence.root();
        let structural_root = structural_closure.root();
        let mut encoder = SupportEncoder::new(SUPPORT_CLOSURE_ROOT_V1);
        encoder.u32(MECHANISM_SUPPORT_VERSION);
        encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut encoder, self.scope.target());
        encoder.digest(target_seal.id().bytes());
        encoder.digest(incidence_root.bytes());
        encoder.digest(structural_root.bytes());
        encoder.u128(target_cases);
        encode_authenticated_index(&mut encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut encoder, &self.target_starter_index);
        encode_authenticated_index(&mut encoder, &self.pending_cases);
        encode_authenticated_index(&mut encoder, &self.unassigned_signature_index);
        encoder.digest(residual.root.bytes());
        encoder.u128(residual.case_count);
        let receipt = MechanismSupportClosureReceipt {
            request_id: self.scope.request_id(),
            target: self.scope.target(),
            target_seal_id: target_seal.id(),
            incidence_root,
            structural_root,
            target_case_count: target_cases,
            successful_case_count: successful_cases,
            unavailable_case_count: unavailable_cases,
            signature_fiber_count: signature_fibers,
            target_starter_count: target_starters,
            residual_root: residual.root,
            root: MechanismSupportClosureRoot(encoder.finish()),
        };
        match self.closure {
            Some(existing) if existing == receipt => Ok(existing),
            Some(_) => Err(MechanismSupportError::ClosureConflict),
            None => Ok(receipt),
        }
    }

    /// Trust a closure receipt only after the caller has compared its root to
    /// the authenticated semantic claim. `derive_closure` may leave valid
    /// prefix caches advanced when derivation or comparison fails, but it
    /// deliberately never crosses this semantic close barrier itself.
    pub(super) fn commit_derived_closure(
        &mut self,
        receipt: MechanismSupportClosureReceipt,
    ) -> Result<MechanismSupportClosureReceipt, MechanismSupportError> {
        if receipt.request_id() != self.scope.request_id()
            || receipt.target() != self.scope.target()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        match self.closure {
            Some(existing) if existing == receipt => Ok(existing),
            Some(_) => Err(MechanismSupportError::ClosureConflict),
            None => {
                self.closure = Some(receipt);
                Ok(receipt)
            }
        }
    }

    pub(crate) fn close(
        &mut self,
        closed_incidence: ClosedMechanismIncidenceRef<'_>,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismSupportClosureReceipt, MechanismSupportError> {
        let receipt = self.derive_closure(closed_incidence, structural)?;
        self.commit_derived_closure(receipt)
    }

    /// Derive the bounded, factorized row used by automatic all-subject
    /// publication. This inspects at most
    /// [`AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT`] immutable fiber summaries
    /// and never constructs the subject's case or starter union.
    pub(crate) fn derive_closed_factorized_subject_summary(
        &self,
        key: MechanismSupportKey,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismFactorizedSubjectSummary, MechanismSupportError> {
        if key.request_id != self.scope.request_id()
            || key.target != self.scope.target()
            || structural.request_id() != self.scope.request_id()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        let support_closure = self
            .closure
            .ok_or(MechanismSupportError::ClosurePrerequisite(
                "mechanism support closure",
            ))?;
        let structural_closure =
            structural
                .closure()
                .ok_or(MechanismSupportError::ClosurePrerequisite(
                    "structural quotient closure",
                ))?;
        if support_closure.request_id() != key.request_id
            || support_closure.target() != key.target
            || support_closure.structural_root() != structural_closure.root()
            || self.target_seal.as_ref().map(MechanismTargetSeal::id)
                != Some(support_closure.target_seal_id())
            || self.target.len() as u128 != support_closure.target_case_count()
            || self.signature_fiber_index.total_weight() != support_closure.successful_case_count()
            || self.unavailable_cases.total_weight() != support_closure.unavailable_case_count()
            || self.signature_fiber_index.entry_count() != support_closure.signature_fiber_count()
            || self.signature_fibers.len() as u128 != support_closure.signature_fiber_count()
            || self.target_starter_index.total_weight() != support_closure.target_starter_count()
            || self.target_starter_refcounts.len() as u128 != support_closure.target_starter_count()
            || self.terminal_fact_index.total_weight() != support_closure.target_case_count()
            || support_closure
                .successful_case_count()
                .checked_add(support_closure.unavailable_case_count())
                != Some(support_closure.target_case_count())
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        let residual = self.factorized_residual()?;
        let target_frontier_open = !self.target_is_complete();
        if residual.root != support_closure.residual_root()
            || residual.case_count != support_closure.unavailable_case_count()
            || target_frontier_open
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        // Recompute the O(1) root envelope so a row cannot combine a stored
        // closure receipt with a same-cardinality but divergent authenticated
        // index state.
        let mut closure_encoder = SupportEncoder::new(SUPPORT_CLOSURE_ROOT_V1);
        closure_encoder.u32(MECHANISM_SUPPORT_VERSION);
        closure_encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut closure_encoder, self.scope.target());
        closure_encoder.digest(support_closure.target_seal_id().bytes());
        closure_encoder.digest(support_closure.incidence_root().bytes());
        closure_encoder.digest(structural_closure.root().bytes());
        closure_encoder.u128(support_closure.target_case_count());
        encode_authenticated_index(&mut closure_encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut closure_encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut closure_encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut closure_encoder, &self.target_starter_index);
        encode_authenticated_index(&mut closure_encoder, &self.pending_cases);
        encode_authenticated_index(&mut closure_encoder, &self.unassigned_signature_index);
        closure_encoder.digest(residual.root.bytes());
        closure_encoder.u128(residual.case_count);
        if closure_encoder.finish() != support_closure.root().bytes() {
            return Err(MechanismSupportError::ClosureConflict);
        }
        validate_structural_subject(structural, key.subject)?;

        let signatures = supporting_signatures(structural, key.subject);
        let contributing_signature_count = signatures.map_or(0, |set| set.len() as u128);
        let mut prefix_encoder = SupportEncoder::new(FACTORIZED_SUBJECT_SIGNATURE_PREFIX_ROOT_V2);
        prefix_encoder.u32(MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION);
        encode_support_key(&mut prefix_encoder, key);
        prefix_encoder.u128(contributing_signature_count);
        prefix_encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);

        let mut inspected_signature_count = 0u128;
        let mut case_lower_bound = 0u128;
        let mut starter_lower_bound = 0u128;
        if let Some(signatures) = signatures {
            for signature_id in signatures
                .iter()
                .copied()
                .take(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT)
            {
                let fiber = self
                    .signature_fibers
                    .get(&signature_id)
                    .ok_or(MechanismSupportError::ClosureConflict)?;
                let summary = signature_fiber_summary(fiber);
                let signature_bytes = signature_id.bytes();
                let authenticated_summary = self
                    .signature_fiber_index
                    .get(&signature_bytes)
                    .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature fibers"))?;
                if authenticated_summary != Some(signature_fiber_value(signature_id, summary)) {
                    return Err(MechanismSupportError::ClosureConflict);
                }
                inspected_signature_count = inspected_signature_count
                    .checked_add(1)
                    .ok_or(MechanismSupportError::CountOverflow)?;
                case_lower_bound = case_lower_bound
                    .checked_add(summary.case_count)
                    .ok_or(MechanismSupportError::CountOverflow)?;
                starter_lower_bound = starter_lower_bound.max(summary.starter_count);
                prefix_encoder.digest(signature_bytes);
                prefix_encoder.digest(summary.root);
                prefix_encoder.u128(summary.case_count);
                prefix_encoder.u128(summary.starter_count);
            }
        }
        prefix_encoder.u128(inspected_signature_count);
        let signature_prefix_root = prefix_encoder.finish();
        let signature_scan_complete = inspected_signature_count == contributing_signature_count;

        let case_upper_bound = if signature_scan_complete {
            case_lower_bound
                .checked_add(residual.case_count)
                .ok_or(MechanismSupportError::CountOverflow)?
        } else {
            support_closure.target_case_count()
        };
        if case_lower_bound > case_upper_bound
            || case_upper_bound > support_closure.target_case_count()
            || starter_lower_bound > support_closure.target_starter_count()
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        let case_count = if case_lower_bound == case_upper_bound {
            MechanismSupportCount::Exact(case_lower_bound)
        } else {
            MechanismSupportCount::Interval {
                lower_bound: case_lower_bound,
                upper_bound: case_upper_bound,
            }
        };

        let target_starter_count = support_closure.target_starter_count();
        let target_starter_root = self.target_starter_index.root_hash();
        let (starter_count, starter_bound_basis) = if starter_lower_bound == target_starter_count {
            (
                MechanismSupportCount::Exact(starter_lower_bound),
                if starter_lower_bound == 0 {
                    MechanismFactorizedStarterBoundBasis::ExactEmpty
                } else {
                    MechanismFactorizedStarterBoundBasis::ExactTargetStarterSaturation {
                        target_starter_root,
                    }
                },
            )
        } else if signature_scan_complete
            && residual.case_count == 0
            && contributing_signature_count <= 1
        {
            (
                MechanismSupportCount::Exact(starter_lower_bound),
                if starter_lower_bound == 0 {
                    MechanismFactorizedStarterBoundBasis::ExactEmpty
                } else {
                    MechanismFactorizedStarterBoundBasis::ExactFactorizedBoundCollapse
                },
            )
        } else {
            (
                MechanismSupportCount::Interval {
                    lower_bound: starter_lower_bound,
                    upper_bound: target_starter_count,
                },
                MechanismFactorizedStarterBoundBasis::ConservativeTargetProjectionUpper {
                    target_starter_root,
                },
            )
        };

        let projection_plan_id = derive_starter_projection_plan_id(
            key,
            structural_closure.root(),
            support_closure.root(),
        );
        let inner_fiber_expr_root = derive_factorized_inner_fiber_expr_root(
            key,
            structural_closure.root(),
            self.signature_fiber_index.root_hash(),
        );
        let outer_fiber_expr_root = derive_outer_fiber_expr_root(
            key,
            inner_fiber_expr_root,
            residual.root,
            residual.case_count,
            target_frontier_open,
        );
        let root = derive_factorized_subject_summary_root(
            key,
            structural_closure.root(),
            support_closure.root(),
            self.signature_fiber_index.root_hash(),
            target_starter_root,
            contributing_signature_count,
            inspected_signature_count,
            signature_scan_complete,
            signature_prefix_root,
            residual.root,
            case_count,
            starter_count,
            starter_bound_basis,
            projection_plan_id,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
        );
        Ok(MechanismFactorizedSubjectSummary {
            key,
            root,
            projection_plan_id,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
            contributing_signature_count,
            inspected_signature_count,
            signature_scan_complete,
            signature_prefix_root,
            shared_residual_root: residual.root,
            case_count,
            starter_count,
            starter_bound_basis,
        })
    }

    /// Freeze the key-only authority needed to page one structural subject's
    /// exact starter/successor relation. A structurally known node or edge
    /// facet with no supporting signatures is an exact empty relation, not an
    /// unknown subject. This performs no value publication and does not mutate
    /// support or structural state.
    pub(crate) fn derive_closed_subject_starter_projection_authority(
        &self,
        key: MechanismSupportKey,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismClosedSubjectStarterProjectionAuthority, MechanismSupportError> {
        let summary = self.derive_closed_factorized_subject_summary(key, structural)?;
        let support_closure = self
            .closure
            .ok_or(MechanismSupportError::ClosurePrerequisite(
                "mechanism support closure",
            ))?;
        let structural_closure =
            structural
                .closure()
                .ok_or(MechanismSupportError::ClosurePrerequisite(
                    "structural quotient closure",
                ))?;
        if !summary.fiber_expr_bounds_are_equal()
            || support_closure.unavailable_case_count() != 0
            || summary.shared_residual_root() != support_closure.residual_root()
            || support_closure.structural_root() != structural_closure.root()
        {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "exact correlated structural-subject starter support",
            ));
        }

        // `derive_closed_factorized_subject_summary` already proved structural
        // existence. An absent differential signature index therefore means
        // exact empty support for this facet; it must not be conflated with an
        // unknown node or edge.
        let signatures = supporting_signatures(structural, key.subject);
        let mut exact_case_count = 0u128;
        if let Some(signatures) = signatures {
            for signature_id in signatures.iter().copied() {
                let assignment = structural
                    .assignment(signature_id)
                    .ok_or(MechanismSupportError::ClosureConflict)?;
                if !assignment_supports_subject(assignment, key.subject) {
                    return Err(MechanismSupportError::ClosureConflict);
                }
                let fiber = self
                    .signature_fibers
                    .get(&signature_id)
                    .ok_or(MechanismSupportError::ClosureConflict)?;
                let fiber_summary = signature_fiber_summary(fiber);
                let authenticated_summary = self
                    .signature_fiber_index
                    .get(&signature_id.bytes())
                    .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("signature fibers")
                })?;
                if authenticated_summary != Some(signature_fiber_value(signature_id, fiber_summary))
                    || fiber_summary.case_count != fiber.cases.len() as u128
                    || fiber_summary.case_count
                        != fiber
                            .starters
                            .values()
                            .map(|successors| successors.len() as u128)
                            .try_fold(0u128, |count, successors| count.checked_add(successors))
                            .ok_or(MechanismSupportError::CountOverflow)?
                {
                    return Err(MechanismSupportError::ClosureConflict);
                }
                exact_case_count = exact_case_count
                    .checked_add(fiber_summary.case_count)
                    .ok_or(MechanismSupportError::CountOverflow)?;
            }
        }
        if exact_case_count > support_closure.successful_case_count()
            || summary.case_count().lower_bound() > exact_case_count
            || summary
                .case_count()
                .upper_bound()
                .is_some_and(|upper_bound| exact_case_count > upper_bound)
        {
            return Err(MechanismSupportError::ClosureConflict);
        }

        Ok(MechanismClosedSubjectStarterProjectionAuthority {
            key,
            question_id: self.scope.question_id(),
            projection_plan_id: summary.projection_plan_id(),
            correlated_fiber_expr_root: summary.inner_fiber_expr_root(),
            structural_root: structural_closure.root(),
            support_root: support_closure.root(),
            exact_case_count,
        })
    }

    /// Transitional whole-mechanism adapter for the current automatic
    /// publication enumerator. New projection consumers should address the
    /// desired subject with a complete [`MechanismSupportKey`].
    pub(crate) fn derive_closed_mechanism_starter_projection_authority(
        &self,
        mechanism_id: StructuralMechanismId,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismClosedStarterProjectionAuthority, MechanismSupportError> {
        let key =
            MechanismSupportKey::new(self.scope, MechanismSupportSubject::Mechanism(mechanism_id));
        let inner = self.derive_closed_subject_starter_projection_authority(key, structural)?;
        Ok(MechanismClosedStarterProjectionAuthority {
            inner,
            mechanism_id,
        })
    }

    /// Resolve the outer projection dimension in the structural closure's
    /// canonical mechanism-ID order.
    pub(crate) fn closed_mechanism_starter_projection_authority_at(
        &self,
        mechanism_ordinal: usize,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<Option<MechanismClosedStarterProjectionAuthority>, MechanismSupportError> {
        let support_closure = self
            .closure
            .ok_or(MechanismSupportError::ClosurePrerequisite(
                "mechanism support closure",
            ))?;
        let structural_closure =
            structural
                .closure()
                .ok_or(MechanismSupportError::ClosurePrerequisite(
                    "structural quotient closure",
                ))?;
        if structural.request_id() != self.scope.request_id()
            || support_closure.request_id() != self.scope.request_id()
            || support_closure.target() != self.scope.target()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        if support_closure.structural_root() != structural_closure.root() {
            return Err(MechanismSupportError::ClosureConflict);
        }
        if support_closure.unavailable_case_count() != 0 {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "exact correlated mechanism starter support",
            ));
        }
        let Some(mechanism_id) = structural.canonical_mechanism_id_at(mechanism_ordinal) else {
            return Ok(None);
        };
        self.derive_closed_mechanism_starter_projection_authority(mechanism_id, structural)
            .map(Some)
    }

    /// Read one bounded canonical suffix of a closed structural subject's
    /// key-only starter relation. The k-way merge retains one candidate per
    /// raw signature and therefore never constructs the complete subject
    /// union. Exact-empty subjects return one exhausted empty page.
    pub(crate) fn closed_subject_starter_page(
        &self,
        authority: MechanismClosedSubjectStarterProjectionAuthority,
        structural: &StructuralMechanismCatalogBuilder,
        relation_id: super::relation::RelationId,
        start_after: Option<MechanismSupportStarterCursor>,
        maximum_members: NonZeroU16,
    ) -> Result<MechanismSupportSubjectStarterPage, MechanismSupportError> {
        let current =
            self.derive_closed_subject_starter_projection_authority(authority.key(), structural)?;
        if current != authority {
            return Err(MechanismSupportError::ClosureConflict);
        }
        let signatures = supporting_signatures(structural, authority.subject());
        let mut candidates = BTreeSet::new();
        if let Some(signatures) = signatures {
            for signature_id in signatures.iter().copied() {
                let fiber = self
                    .signature_fibers
                    .get(&signature_id)
                    .ok_or(MechanismSupportError::ClosureConflict)?;
                if let Some((source_key, successor_key)) =
                    first_signature_starter_after(fiber, start_after)
                {
                    candidates.insert((source_key, successor_key, signature_id));
                }
            }
        }

        let mut members = Vec::with_capacity(maximum_members.get() as usize);
        while members.len() < maximum_members.get() as usize {
            let Some((source_key, successor_key, signature_id)) = candidates.pop_first() else {
                break;
            };
            if candidates
                .first()
                .is_some_and(|(next_source, next_successor, _)| {
                    *next_source == source_key && *next_successor == successor_key
                })
            {
                return Err(MechanismSupportError::ClosureConflict);
            }
            let fiber = self
                .signature_fibers
                .get(&signature_id)
                .ok_or(MechanismSupportError::ClosureConflict)?;
            let case_id = RelationalCaseId::derive(relation_id, source_key, successor_key);
            if fiber.cases.get(&case_id) != Some(&(source_key, successor_key)) {
                return Err(MechanismSupportError::TargetCaseIdentityMismatch);
            }
            members.push(MechanismSupportStarterMember {
                raw_signature_id: signature_id,
                case_id,
                source_key,
                successor_key,
            });
            if let Some((next_source, next_successor)) = first_signature_starter_after(
                fiber,
                Some(MechanismSupportStarterCursor::new(
                    source_key,
                    successor_key,
                )),
            ) {
                candidates.insert((next_source, next_successor, signature_id));
            }
        }

        Ok(MechanismSupportSubjectStarterPage {
            authority,
            start_after,
            members: members.into_boxed_slice(),
            exhausted: candidates.is_empty(),
        })
    }

    /// Transitional whole-mechanism pager adapter. The returned page is the
    /// subject-generic value and retains the complete mechanism subject key.
    pub(crate) fn closed_mechanism_starter_page(
        &self,
        authority: MechanismClosedStarterProjectionAuthority,
        structural: &StructuralMechanismCatalogBuilder,
        relation_id: super::relation::RelationId,
        start_after: Option<MechanismSupportStarterCursor>,
        maximum_members: NonZeroU16,
    ) -> Result<MechanismSupportStarterPage, MechanismSupportError> {
        self.closed_subject_starter_page(
            authority.subject_authority(),
            structural,
            relation_id,
            start_after,
            maximum_members,
        )
    }

    /// Derive one explicitly requested full projection from already closed
    /// semantic authority. Its work and memory may scale with every case in the
    /// subject, so automatic all-subject publication must use
    /// `derive_closed_factorized_subject_summary` instead. This on-demand path
    /// bypasses the mutable hot-subject cache and leaves every cursor,
    /// authenticated index, cache entry, and LRU position unchanged.
    pub(crate) fn derive_closed_view(
        &self,
        key: MechanismSupportKey,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismSupportView, MechanismSupportError> {
        if key.request_id != self.scope.request_id()
            || key.target != self.scope.target()
            || structural.request_id() != self.scope.request_id()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        let support_closure = self
            .closure
            .ok_or(MechanismSupportError::ClosurePrerequisite(
                "mechanism support closure",
            ))?;
        let structural_closure =
            structural
                .closure()
                .ok_or(MechanismSupportError::ClosurePrerequisite(
                    "structural quotient closure",
                ))?;
        if support_closure.request_id() != key.request_id
            || support_closure.target() != key.target
            || support_closure.structural_root() != structural_closure.root()
            || self.target_seal.as_ref().map(MechanismTargetSeal::id)
                != Some(support_closure.target_seal_id())
            || self.target.len() as u128 != support_closure.target_case_count()
            || self.signature_fiber_index.total_weight() != support_closure.successful_case_count()
            || self.unavailable_cases.total_weight() != support_closure.unavailable_case_count()
            || self.signature_fiber_index.entry_count() != support_closure.signature_fiber_count()
            || self.signature_fibers.len() as u128 != support_closure.signature_fiber_count()
            || self.target_starter_index.total_weight() != support_closure.target_starter_count()
            || self.target_starter_refcounts.len() as u128 != support_closure.target_starter_count()
            || self.terminal_fact_index.total_weight() != support_closure.target_case_count()
            || support_closure
                .successful_case_count()
                .checked_add(support_closure.unavailable_case_count())
                != Some(support_closure.target_case_count())
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        let residual = self.factorized_residual()?;
        if residual.root != support_closure.residual_root()
            || residual.case_count != support_closure.unavailable_case_count()
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        validate_structural_subject(structural, key.subject)?;
        let projection = build_subject_projection(key.subject, structural, &self.signature_fibers)?;
        let view = self.derive_view_from_projection(key, structural, &projection)?;
        if view.target_frontier_is_open() || view.shared_residual_root() != residual.root {
            return Err(MechanismSupportError::ClosureConflict);
        }
        Ok(view)
    }

    pub(crate) fn derive_view(
        &mut self,
        key: MechanismSupportKey,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismSupportView, MechanismSupportError> {
        if key.request_id != self.scope.request_id()
            || key.target != self.scope.target()
            || structural.request_id() != self.scope.request_id()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        validate_structural_subject(structural, key.subject)?;
        self.sync_structural_assignments(structural)?;
        if !self.subject_projection_cache.contains_key(&key.subject) {
            let projection =
                build_subject_projection(key.subject, structural, &self.signature_fibers)?;
            self.install_subject_projection(key.subject, projection);
        } else {
            self.touch_subject_projection(key.subject);
        }
        let projection = self
            .subject_projection_cache
            .get(&key.subject)
            .expect("requested subject projection was installed");
        self.derive_view_from_projection(key, structural, projection)
    }

    fn derive_view_from_projection(
        &self,
        key: MechanismSupportKey,
        structural: &StructuralMechanismCatalogBuilder,
        projection: &SubjectProjectionCache,
    ) -> Result<MechanismSupportView, MechanismSupportError> {
        let inner_signature_root = projection.signature_index.root_hash();
        let inner_case_root = projection.case_index.root_hash();
        let inner_starter_root = projection.starter_index.root_hash();
        let inner_cases = projection.case_count();
        let inner_starters = projection.starter_count();
        let target_starter_root = self.target_starter_index.root_hash();
        let target_starters = self.target_starter_index.total_weight();
        if target_starters != self.target_starter_refcounts.len() as u128 {
            return Err(MechanismSupportError::ResidualPartitionConflict);
        }
        // Every projection coordinate is admitted through this exact target,
        // so its starter set is maintained as a subset of target starters.
        // Equal distinct cardinality therefore proves set equality without an
        // O(target starters) rescan on every observation.
        let all_target_starters_confirmed = inner_starters == target_starters;
        let shared_residual = self.factorized_residual()?;
        let residual_cases = shared_residual.case_count;
        let target_frontier_open = !self.target_is_complete();
        let inner_fiber_expr_root =
            derive_materialized_inner_fiber_expr_root(key, inner_starter_root);
        let outer_fiber_expr_root = derive_outer_fiber_expr_root(
            key,
            inner_fiber_expr_root,
            shared_residual.root,
            residual_cases,
            target_frontier_open,
        );
        let case_count = if target_frontier_open {
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: inner_cases,
            }
        } else if residual_cases == 0 {
            MechanismSupportCount::Exact(inner_cases)
        } else {
            MechanismSupportCount::Interval {
                lower_bound: inner_cases,
                upper_bound: inner_cases
                    .checked_add(residual_cases)
                    .ok_or(MechanismSupportError::CountOverflow)?,
            }
        };
        if inner_starters > target_starters {
            return Err(MechanismSupportError::ResidualPartitionConflict);
        }
        let (starter_count, starter_upper_provenance) = if target_frontier_open {
            (
                MechanismSupportCount::Unknown {
                    confirmed_lower_bound: inner_starters,
                },
                MechanismStarterUpperProvenance::OpenOpaque,
            )
        } else if residual_cases == 0 {
            (
                MechanismSupportCount::Exact(inner_starters),
                MechanismStarterUpperProvenance::ExactCorrelatedInner { inner_starter_root },
            )
        } else if all_target_starters_confirmed {
            (
                MechanismSupportCount::Exact(inner_starters),
                MechanismStarterUpperProvenance::ExactStarterSetFromTargetSaturation {
                    target_starter_root,
                },
            )
        } else {
            (
                MechanismSupportCount::Interval {
                    lower_bound: inner_starters,
                    // This is deliberately conservative until the resumable
                    // subject projection accumulator has closed. Every possible
                    // residual starter is nevertheless in the sealed target.
                    upper_bound: target_starters,
                },
                MechanismStarterUpperProvenance::ConservativeTargetProjectionUpper {
                    target_starter_root,
                },
            )
        };
        let root = derive_support_view_root(
            key,
            self.target_seal.as_ref().map(MechanismTargetSeal::id),
            structural.assignment_root(),
            self.terminal_fact_index.root_hash(),
            self.signature_fiber_index.root_hash(),
            target_starter_root,
            inner_signature_root,
            inner_case_root,
            inner_starter_root,
            shared_residual.root,
            target_frontier_open,
            case_count,
            starter_count,
            starter_upper_provenance,
        );
        Ok(MechanismSupportView {
            key,
            root,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
            inner_signature_root,
            inner_case_root,
            inner_starter_root,
            shared_residual_root: shared_residual.root,
            target_frontier_open,
            case_count,
            starter_count,
            starter_upper_provenance,
        })
    }

    fn factorized_residual(&self) -> Result<FactorizedResidualSummary, MechanismSupportError> {
        let pending_cases = self.pending_cases.total_weight();
        let unavailable_cases = self.unavailable_cases.total_weight();
        let unassigned_cases = self.unassigned_signature_index.total_weight();
        let successful_cases = self.signature_fiber_index.total_weight();
        let terminal_cases = self.terminal_fact_index.total_weight();
        let target_starters = self.target_starter_index.total_weight();
        let partitioned_target_cases = pending_cases
            .checked_add(unavailable_cases)
            .and_then(|count| count.checked_add(successful_cases))
            .ok_or(MechanismSupportError::CountOverflow)?;
        if partitioned_target_cases != self.target.len() as u128
            || unassigned_cases > successful_cases
            || terminal_cases
                != unavailable_cases
                    .checked_add(successful_cases)
                    .ok_or(MechanismSupportError::CountOverflow)?
            || target_starters != self.target_starter_refcounts.len() as u128
        {
            return Err(MechanismSupportError::ResidualPartitionConflict);
        }
        let case_count = pending_cases
            .checked_add(unavailable_cases)
            .and_then(|count| count.checked_add(unassigned_cases))
            .ok_or(MechanismSupportError::CountOverflow)?;
        let mut encoder = SupportEncoder::new(SHARED_RESIDUAL_ROOT_V2);
        encoder.u32(MECHANISM_SUPPORT_VERSION);
        encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut encoder, self.scope.target());
        encoder.digest(self.pending_cases.root_hash());
        encoder.u128(self.pending_cases.entry_count());
        encoder.u128(pending_cases);
        encoder.digest(self.unavailable_cases.root_hash());
        encoder.u128(self.unavailable_cases.entry_count());
        encoder.u128(unavailable_cases);
        encoder.digest(self.unassigned_signature_index.root_hash());
        encoder.u128(self.unassigned_signature_index.entry_count());
        encoder.u128(unassigned_cases);
        encoder.u128(case_count);
        Ok(FactorizedResidualSummary {
            root: MechanismSupportResidualRoot(encoder.finish()),
            case_count,
        })
    }
}

fn assignment_supports_subject(
    assignment: &StructuralSignatureAssignment,
    subject: MechanismSupportSubject,
) -> bool {
    // Structural membership slices are canonical sorted sets, so a hot-cache
    // membership check is logarithmic in the assignment size.
    match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => {
            assignment.mechanism_id() == mechanism_id
        }
        MechanismSupportSubject::Node { facet, node_id } => match facet {
            MechanismSupportFacet::Activation => {
                assignment.node_membership().binary_search(&node_id).is_ok()
            }
            MechanismSupportFacet::DifferentialParticipation => assignment
                .differential_node_membership()
                .binary_search(&node_id)
                .is_ok(),
        },
        MechanismSupportSubject::Edge { facet, edge_id } => match facet {
            MechanismSupportFacet::Activation => {
                assignment.edge_membership().binary_search(&edge_id).is_ok()
            }
            MechanismSupportFacet::DifferentialParticipation => assignment
                .differential_edge_membership()
                .binary_search(&edge_id)
                .is_ok(),
        },
    }
}

fn supporting_signatures<'a>(
    structural: &'a StructuralMechanismCatalogBuilder,
    subject: MechanismSupportSubject,
) -> Option<&'a BTreeSet<MechanismSignatureId>> {
    match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => {
            structural.signatures_for_mechanism(mechanism_id)
        }
        MechanismSupportSubject::Node { facet, node_id } => structural.signatures_for_node(
            node_id,
            facet == MechanismSupportFacet::DifferentialParticipation,
        ),
        MechanismSupportSubject::Edge { facet, edge_id } => structural.signatures_for_edge(
            edge_id,
            facet == MechanismSupportFacet::DifferentialParticipation,
        ),
    }
}

fn first_signature_starter_after(
    fiber: &SignatureCaseFiber,
    start_after: Option<MechanismSupportStarterCursor>,
) -> Option<(SourceKey, SuccessorKey)> {
    let first_successor = |source_key: SourceKey, successors: &BTreeSet<SuccessorKey>| {
        successors
            .first()
            .copied()
            .map(|successor_key| (source_key, successor_key))
    };
    let Some(start_after) = start_after else {
        return fiber
            .starters
            .iter()
            .find_map(|(source_key, successors)| first_successor(*source_key, successors));
    };
    if let Some(successor_key) =
        fiber
            .starters
            .get(&start_after.source_key())
            .and_then(|successors| {
                successors
                    .range((Excluded(start_after.successor_key()), Unbounded))
                    .next()
                    .copied()
            })
    {
        return Some((start_after.source_key(), successor_key));
    }
    fiber
        .starters
        .range((Excluded(start_after.source_key()), Unbounded))
        .find_map(|(source_key, successors)| first_successor(*source_key, successors))
}

fn validate_structural_subject(
    structural: &StructuralMechanismCatalogBuilder,
    subject: MechanismSupportSubject,
) -> Result<(), MechanismSupportError> {
    let known = match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => {
            structural.contains_mechanism(mechanism_id)
        }
        MechanismSupportSubject::Node { node_id, .. } => structural.contains_node(node_id),
        MechanismSupportSubject::Edge { edge_id, .. } => structural.contains_edge(edge_id),
    };
    if known {
        Ok(())
    } else {
        Err(MechanismSupportError::UnknownStructuralSubject)
    }
}

fn build_subject_projection(
    subject: MechanismSupportSubject,
    structural: &StructuralMechanismCatalogBuilder,
    fibers: &BTreeMap<MechanismSignatureId, SignatureCaseFiber>,
) -> Result<SubjectProjectionCache, MechanismSupportError> {
    let mut projection = SubjectProjectionCache::new();
    if let Some(signatures) = supporting_signatures(structural, subject) {
        for signature_id in signatures.iter().copied() {
            if let Some(fiber) = fibers.get(&signature_id) {
                insert_signature_fiber(&mut projection, signature_id, fiber)?;
            }
        }
    }
    Ok(projection)
}

fn signature_fiber_summary(fiber: &SignatureCaseFiber) -> SignatureFiberSummary {
    SignatureFiberSummary {
        root: fiber.authenticated_cases.root_hash(),
        case_count: fiber.authenticated_cases.total_weight(),
        starter_count: fiber.starters.len() as u128,
    }
}

fn insert_signature_fiber(
    projection: &mut SubjectProjectionCache,
    signature_id: MechanismSignatureId,
    fiber: &SignatureCaseFiber,
) -> Result<(), MechanismSupportError> {
    let mut next_signatures = projection.signature_index.clone();
    set_authenticated_value(
        &mut next_signatures,
        signature_key(signature_id),
        signature_fiber_value(signature_id, signature_fiber_summary(fiber)),
        "subject signatures",
    )?;
    let mut next_cases = projection.case_index.clone();
    for (case_id, (source, successor)) in &fiber.cases {
        next_cases
            .insert(
                case_key(*case_id),
                AuthenticatedTreapValue::new(coordinate_value_digest(*source, *successor), 1),
            )
            .map_err(|_| MechanismSupportError::AuthenticatedIndex("subject cases"))?;
    }
    projection.signature_index = next_signatures;
    projection.case_index = next_cases;
    for (source, successors) in &fiber.starters {
        for successor in successors.iter().copied() {
            insert_projection_successor(projection, *source, successor)?;
        }
    }
    Ok(())
}

fn insert_subject_case(
    projection: &mut SubjectProjectionCache,
    signature_id: MechanismSignatureId,
    summary: SignatureFiberSummary,
    case_id: RelationalCaseId,
    source: SourceKey,
    successor: SuccessorKey,
) -> Result<(), MechanismSupportError> {
    let mut next_signatures = projection.signature_index.clone();
    set_authenticated_value(
        &mut next_signatures,
        signature_key(signature_id),
        signature_fiber_value(signature_id, summary),
        "subject signatures",
    )?;
    let mut next_cases = projection.case_index.clone();
    next_cases
        .insert(
            case_key(case_id),
            AuthenticatedTreapValue::new(coordinate_value_digest(source, successor), 1),
        )
        .map_err(|_| MechanismSupportError::AuthenticatedIndex("subject cases"))?;
    projection.signature_index = next_signatures;
    projection.case_index = next_cases;
    insert_projection_successor(projection, source, successor)?;
    Ok(())
}

fn insert_projection_successor(
    projection: &mut SubjectProjectionCache,
    source: SourceKey,
    successor: SuccessorKey,
) -> Result<(), MechanismSupportError> {
    let mut next_successors = projection
        .successor_fibers
        .get(&source)
        .cloned()
        .unwrap_or_else(|| AuthenticatedTreapMap::new(SUBJECT_SUCCESSOR_INDEX_V1));
    if !authenticated_contains(
        &next_successors,
        &successor.bytes(),
        "subject successor fiber",
    )? {
        next_successors
            .insert(
                successor.bytes().to_vec().into_boxed_slice(),
                AuthenticatedTreapValue::new(coordinate_value_digest(source, successor), 1),
            )
            .map_err(|_| MechanismSupportError::AuthenticatedIndex("subject successor fiber"))?;
    }
    let successor_count = next_successors.total_weight();
    let mut encoder = SupportEncoder::new(STARTER_FIBER_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encoder.digest(source.bytes());
    encoder.digest(next_successors.root_hash());
    encoder.u128(successor_count);
    let starter_value = AuthenticatedTreapValue::new(encoder.finish(), 1);
    let mut next_starters = projection.starter_index.clone();
    set_authenticated_value(
        &mut next_starters,
        source.bytes().to_vec().into_boxed_slice(),
        starter_value,
        "subject starters",
    )?;
    projection.successor_fibers.insert(source, next_successors);
    projection.starter_index = next_starters;
    Ok(())
}

fn set_authenticated_value(
    index: &mut AuthenticatedTreapMap,
    key: Box<[u8]>,
    value: AuthenticatedTreapValue,
    label: &'static str,
) -> Result<(), MechanismSupportError> {
    if authenticated_contains(index, &key, label)? {
        index.update(&key, value)
    } else {
        index.insert(key, value)
    }
    .map(|_| ())
    .map_err(|_| MechanismSupportError::AuthenticatedIndex(label))
}

fn authenticated_contains(
    index: &AuthenticatedTreapMap,
    key: &[u8],
    label: &'static str,
) -> Result<bool, MechanismSupportError> {
    index
        .get(key)
        .map(|value| value.is_some())
        .map_err(|_| MechanismSupportError::AuthenticatedIndex(label))
}

fn signature_fiber_value(
    signature_id: MechanismSignatureId,
    summary: SignatureFiberSummary,
) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(SIGNATURE_FIBER_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encoder.digest(signature_id.bytes());
    encoder.digest(summary.root);
    encoder.u128(summary.case_count);
    encoder.u128(summary.starter_count);
    AuthenticatedTreapValue::new(encoder.finish(), summary.case_count)
}

fn coordinate_value_digest(source: SourceKey, successor: SuccessorKey) -> [u8; 32] {
    let mut encoder = SupportEncoder::new(COORDINATE_VALUE_V1);
    encoder.digest(source.bytes());
    encoder.digest(successor.bytes());
    encoder.finish()
}

fn target_starter_value(source: SourceKey, refcount: u128) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(TARGET_STARTER_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encoder.digest(source.bytes());
    encoder.u128(refcount);
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn unavailable_value_digest(
    source: SourceKey,
    successor: SuccessorKey,
    reason: [u8; 32],
) -> [u8; 32] {
    let mut encoder = SupportEncoder::new(UNAVAILABLE_VALUE_V1);
    encoder.digest(source.bytes());
    encoder.digest(successor.bytes());
    encoder.digest(reason);
    encoder.finish()
}

fn terminal_value_digest(
    source: SourceKey,
    successor: SuccessorKey,
    terminal: MechanismCaseTerminal,
) -> [u8; 32] {
    let mut encoder = SupportEncoder::new(TERMINAL_VALUE_V1);
    encoder.digest(source.bytes());
    encoder.digest(successor.bytes());
    match terminal {
        MechanismCaseTerminal::Incidence {
            transition_id,
            signature_id,
        } => {
            encoder.u8(0x01);
            encoder.digest(transition_id.bytes());
            encoder.digest(signature_id.request_id().bytes());
            encoder.digest(signature_id.bytes());
        }
        MechanismCaseTerminal::Unavailable { reason_id } => {
            encoder.u8(0x02);
            encoder.digest(reason_id.bytes());
        }
    }
    encoder.finish()
}

fn case_key(case_id: RelationalCaseId) -> Box<[u8]> {
    case_id.bytes().to_vec().into_boxed_slice()
}

fn signature_key(signature_id: MechanismSignatureId) -> Box<[u8]> {
    signature_id.bytes().to_vec().into_boxed_slice()
}

fn derive_starter_projection_plan_id(
    key: MechanismSupportKey,
    structural_root: StructuralQuotientClosureRoot,
    support_root: MechanismSupportClosureRoot,
) -> MechanismStarterProjectionPlanId {
    let mut encoder = SupportEncoder::new(STARTER_PROJECTION_PLAN_ID_V2);
    encoder.u32(MECHANISM_STARTER_PROJECTION_PLAN_VERSION);
    encode_support_key(&mut encoder, key);
    encoder.digest(structural_root.bytes());
    encoder.digest(support_root.bytes());
    MechanismStarterProjectionPlanId(encoder.finish())
}

fn derive_factorized_inner_fiber_expr_root(
    key: MechanismSupportKey,
    structural_root: StructuralQuotientClosureRoot,
    signature_fiber_root: [u8; 32],
) -> MechanismSupportFiberExprRoot {
    let mut encoder = support_fiber_expr_encoder(key, FIBER_EXPR_FACTORIZED_SUBJECT_UNION);
    encoder.digest(structural_root.bytes());
    encoder.digest(signature_fiber_root);
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_materialized_inner_fiber_expr_root(
    key: MechanismSupportKey,
    correlated_starter_root: [u8; 32],
) -> MechanismSupportFiberExprRoot {
    let mut encoder = support_fiber_expr_encoder(key, FIBER_EXPR_MATERIALIZED_PROJECTION);
    encoder.digest(correlated_starter_root);
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_outer_fiber_expr_root(
    key: MechanismSupportKey,
    inner: MechanismSupportFiberExprRoot,
    shared_residual_root: MechanismSupportResidualRoot,
    shared_residual_case_count: u128,
    opaque_undiscovered_target: bool,
) -> MechanismSupportFiberExprRoot {
    if shared_residual_case_count == 0 && !opaque_undiscovered_target {
        return inner;
    }
    let mut encoder = support_fiber_expr_encoder(key, FIBER_EXPR_POSSIBLE_SUPPORT_ENVELOPE);
    encoder.digest(inner.bytes());
    encoder.digest(shared_residual_root.bytes());
    encoder.u128(shared_residual_case_count);
    encoder.u8(u8::from(opaque_undiscovered_target));
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn support_fiber_expr_encoder(key: MechanismSupportKey, expression_kind: u8) -> SupportEncoder {
    let mut encoder = SupportEncoder::new(SUPPORT_FIBER_EXPR_ROOT_V1);
    encoder.u32(MECHANISM_SUPPORT_FIBER_EXPR_VERSION);
    // Coordinate contract: origin SourceKey `(Context, Before)` mapped to a
    // set of SuccessorKey `After` members in the request's typed relation.
    encoder.u8(FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE);
    encoder.u8(FIBER_EXPR_SOURCE_CONTEXT_BEFORE);
    encoder.u8(FIBER_EXPR_SUCCESSOR_AFTER);
    encode_support_key(&mut encoder, key);
    encoder.u8(expression_kind);
    encoder
}

#[allow(clippy::too_many_arguments)]
fn derive_factorized_subject_summary_root(
    key: MechanismSupportKey,
    structural_root: StructuralQuotientClosureRoot,
    support_root: MechanismSupportClosureRoot,
    signature_fiber_root: [u8; 32],
    target_starter_root: [u8; 32],
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    signature_scan_complete: bool,
    signature_prefix_root: [u8; 32],
    shared_residual_root: MechanismSupportResidualRoot,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_bound_basis: MechanismFactorizedStarterBoundBasis,
    projection_plan_id: MechanismStarterProjectionPlanId,
    inner_fiber_expr_root: MechanismSupportFiberExprRoot,
    outer_fiber_expr_root: MechanismSupportFiberExprRoot,
) -> MechanismFactorizedSubjectSummaryRoot {
    let mut encoder = SupportEncoder::new(FACTORIZED_SUBJECT_SUMMARY_ROOT_V2);
    encoder.u32(MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION);
    encode_support_key(&mut encoder, key);
    encoder.digest(structural_root.bytes());
    encoder.digest(support_root.bytes());
    encoder.digest(signature_fiber_root);
    encoder.digest(target_starter_root);
    encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);
    encoder.u128(contributing_signature_count);
    encoder.u128(inspected_signature_count);
    encoder.u8(u8::from(signature_scan_complete));
    encoder.digest(signature_prefix_root);
    encoder.digest(shared_residual_root.bytes());
    encode_count(&mut encoder, case_count);
    encode_count(&mut encoder, starter_count);
    encode_factorized_starter_bound_basis(&mut encoder, starter_bound_basis);
    encoder.digest(projection_plan_id.bytes());
    encoder.digest(inner_fiber_expr_root.bytes());
    encoder.digest(outer_fiber_expr_root.bytes());
    MechanismFactorizedSubjectSummaryRoot(encoder.finish())
}

fn encode_support_key(encoder: &mut SupportEncoder, key: MechanismSupportKey) {
    encoder.digest(key.request_id.bytes());
    encode_target(encoder, key.target);
    match key.subject {
        MechanismSupportSubject::Mechanism(id) => {
            encoder.u8(0x01);
            encoder.digest(id.bytes());
        }
        MechanismSupportSubject::Node { facet, node_id } => {
            encoder.u8(0x02);
            encode_facet(encoder, facet);
            encoder.digest(node_id.bytes());
        }
        MechanismSupportSubject::Edge { facet, edge_id } => {
            encoder.u8(0x03);
            encode_facet(encoder, facet);
            encoder.digest(edge_id.bytes());
        }
    }
}

fn derive_support_view_root(
    key: MechanismSupportKey,
    target_seal_id: Option<MechanismTargetSealId>,
    structural_assignment_root: [u8; 32],
    terminal_fact_root: [u8; 32],
    signature_fiber_root: [u8; 32],
    target_starter_root: [u8; 32],
    inner_signature_root: [u8; 32],
    inner_case_root: [u8; 32],
    inner_starter_root: [u8; 32],
    shared_residual_root: MechanismSupportResidualRoot,
    target_frontier_open: bool,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_upper_provenance: MechanismStarterUpperProvenance,
) -> MechanismSupportViewRoot {
    let mut encoder = SupportEncoder::new(SUPPORT_VIEW_ROOT_V4);
    encoder.u32(MECHANISM_SUPPORT_VIEW_VERSION);
    encoder.digest(key.request_id.bytes());
    encode_target(&mut encoder, key.target);
    match target_seal_id {
        Some(id) => {
            encoder.u8(0x01);
            encoder.digest(id.bytes());
        }
        None => encoder.u8(0x00),
    }
    match key.subject {
        MechanismSupportSubject::Mechanism(id) => {
            encoder.u8(0x01);
            encoder.digest(id.bytes());
        }
        MechanismSupportSubject::Node { facet, node_id } => {
            encoder.u8(0x02);
            encode_facet(&mut encoder, facet);
            encoder.digest(node_id.bytes());
        }
        MechanismSupportSubject::Edge { facet, edge_id } => {
            encoder.u8(0x03);
            encode_facet(&mut encoder, facet);
            encoder.digest(edge_id.bytes());
        }
    }
    encoder.u8(u8::from(target_frontier_open));
    encoder.digest(structural_assignment_root);
    encoder.digest(terminal_fact_root);
    encoder.digest(signature_fiber_root);
    encoder.digest(target_starter_root);
    encoder.digest(inner_signature_root);
    encoder.digest(inner_case_root);
    encoder.digest(inner_starter_root);
    encoder.digest(shared_residual_root.bytes());
    encode_count(&mut encoder, case_count);
    encode_count(&mut encoder, starter_count);
    encode_starter_upper_provenance(&mut encoder, starter_upper_provenance);
    MechanismSupportViewRoot(encoder.finish())
}

fn encode_optional_target_seal(
    encoder: &mut SupportEncoder,
    target_seal: Option<&MechanismTargetSeal>,
) {
    match target_seal {
        Some(seal) => {
            encoder.u8(0x01);
            encoder.digest(seal.id().bytes());
        }
        None => encoder.u8(0x00),
    }
}

fn encode_authenticated_index(encoder: &mut SupportEncoder, index: &AuthenticatedTreapMap) {
    encoder.digest(index.root_hash());
    encoder.u128(index.entry_count());
    encoder.u128(index.total_weight());
}

fn encode_target(encoder: &mut SupportEncoder, target: MechanismTargetId) {
    match target {
        MechanismTargetId::Selected => encoder.u8(0x01),
        MechanismTargetId::ChosenView(view_id) => {
            encoder.u8(0x02);
            encoder.digest(view_id.bytes());
        }
    }
}

fn encode_facet(encoder: &mut SupportEncoder, facet: MechanismSupportFacet) {
    encoder.u8(match facet {
        MechanismSupportFacet::Activation => 0x01,
        MechanismSupportFacet::DifferentialParticipation => 0x02,
    });
}

fn encode_count(encoder: &mut SupportEncoder, count: MechanismSupportCount) {
    match count {
        MechanismSupportCount::Unknown {
            confirmed_lower_bound,
        } => {
            encoder.u8(0x01);
            encoder.u128(confirmed_lower_bound);
        }
        MechanismSupportCount::Interval {
            lower_bound,
            upper_bound,
        } => {
            encoder.u8(0x02);
            encoder.u128(lower_bound);
            encoder.u128(upper_bound);
        }
        MechanismSupportCount::Exact(value) => {
            encoder.u8(0x03);
            encoder.u128(value);
        }
    }
}

fn encode_starter_upper_provenance(
    encoder: &mut SupportEncoder,
    provenance: MechanismStarterUpperProvenance,
) {
    match provenance {
        MechanismStarterUpperProvenance::OpenOpaque => encoder.u8(0x01),
        MechanismStarterUpperProvenance::ExactCorrelatedInner { inner_starter_root } => {
            encoder.u8(0x02);
            encoder.digest(inner_starter_root);
        }
        MechanismStarterUpperProvenance::ExactStarterSetFromTargetSaturation {
            target_starter_root,
        } => {
            encoder.u8(0x03);
            encoder.digest(target_starter_root);
        }
        MechanismStarterUpperProvenance::ConservativeTargetProjectionUpper {
            target_starter_root,
        } => {
            encoder.u8(0x04);
            encoder.digest(target_starter_root);
        }
    }
}

fn encode_factorized_starter_bound_basis(
    encoder: &mut SupportEncoder,
    basis: MechanismFactorizedStarterBoundBasis,
) {
    match basis {
        MechanismFactorizedStarterBoundBasis::ExactEmpty => encoder.u8(0x01),
        MechanismFactorizedStarterBoundBasis::ExactFactorizedBoundCollapse => encoder.u8(0x02),
        MechanismFactorizedStarterBoundBasis::ExactTargetStarterSaturation {
            target_starter_root,
        } => {
            encoder.u8(0x03);
            encoder.digest(target_starter_root);
        }
        MechanismFactorizedStarterBoundBasis::ConservativeTargetProjectionUpper {
            target_starter_root,
        } => {
            encoder.u8(0x04);
            encoder.digest(target_starter_root);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismSupportError {
    InvalidTargetSeal,
    RequestMismatch,
    TargetCaseIdentityMismatch,
    UnknownIncidenceTargetCase,
    UnknownTargetCase,
    TargetCaseSetSealed,
    TargetCaseSetIncomplete,
    TargetSealUnavailable,
    TargetSealConflict,
    TargetDiscoveryCursorRegression,
    TargetDiscoveryPrefixConflict,
    TargetDiscoveryOrderMismatch,
    TerminalDiscoveryCursorRegression,
    TerminalDiscoveryPrefixConflict,
    StructuralAssignmentCursorRegression,
    StructuralAssignmentPrefixConflict,
    UnknownStructuralAssignment,
    UnknownStructuralSubject,
    TerminalConflict,
    SignatureConflict,
    ResidualPartitionConflict,
    CatalogClosed,
    ClosurePrerequisite(&'static str),
    ClosureConflict,
    StructuralClosure(StructuralMechanismError),
    AuthenticatedIndex(&'static str),
    CountOverflow,
}

impl fmt::Display for MechanismSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTargetSeal => "mechanism support received an invalid target seal",
            Self::RequestMismatch => "mechanism support scope does not match its request/target",
            Self::TargetCaseIdentityMismatch => {
                "mechanism support target coordinate does not match its CaseId"
            }
            Self::UnknownIncidenceTargetCase => {
                "mechanism support coordinate is not in the checked raw target prefix"
            }
            Self::UnknownTargetCase => "mechanism support assignment references a non-target case",
            Self::TargetCaseSetSealed => {
                "mechanism support cannot add a new coordinate after target closure"
            }
            Self::TargetCaseSetIncomplete => {
                "mechanism support target coordinates do not match the sealed CaseId set"
            }
            Self::TargetSealUnavailable => "mechanism support target is still open",
            Self::TargetSealConflict => "mechanism support target seal conflicts with its closure",
            Self::TargetDiscoveryCursorRegression => {
                "mechanism support target cursor exceeds its raw catalog"
            }
            Self::TargetDiscoveryPrefixConflict => {
                "mechanism support target prefix belongs to a divergent catalog branch"
            }
            Self::TargetDiscoveryOrderMismatch => {
                "mechanism support target cases must follow checked discovery order"
            }
            Self::TerminalDiscoveryCursorRegression => {
                "mechanism support terminal discovery cursor exceeds its raw catalog"
            }
            Self::TerminalDiscoveryPrefixConflict => {
                "mechanism support raw terminal prefix belongs to a divergent catalog branch"
            }
            Self::StructuralAssignmentCursorRegression => {
                "mechanism support structural cursor exceeds its quotient catalog"
            }
            Self::StructuralAssignmentPrefixConflict => {
                "mechanism support structural prefix belongs to a divergent catalog branch"
            }
            Self::UnknownStructuralAssignment => {
                "mechanism support structural discovery names no checked assignment"
            }
            Self::UnknownStructuralSubject => {
                "mechanism support cannot prove absence for an unknown structural subject"
            }
            Self::TerminalConflict => "one target CaseId has conflicting mechanism terminals",
            Self::SignatureConflict => "one target coordinate has conflicting raw signatures",
            Self::ResidualPartitionConflict => {
                "mechanism support residual factors violate their disjoint partition"
            }
            Self::CatalogClosed => {
                "mechanism support cannot accept new evidence after request closure"
            }
            Self::ClosurePrerequisite(label) => {
                return write!(
                    formatter,
                    "mechanism support cannot close before {label} is exact"
                );
            }
            Self::ClosureConflict => {
                "mechanism support already closed with different derived evidence"
            }
            Self::StructuralClosure(error) => {
                return write!(
                    formatter,
                    "mechanism support structural closure failed: {error}"
                );
            }
            Self::AuthenticatedIndex(label) => {
                return write!(
                    formatter,
                    "mechanism support authenticated {label} index failed"
                );
            }
            Self::CountOverflow => "mechanism support cardinality overflowed",
        })
    }
}

impl Error for MechanismSupportError {}

struct SupportEncoder {
    hasher: Sha256,
}

impl SupportEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut encoder = Self {
            hasher: Sha256::new(),
        };
        encoder.u128(domain.len() as u128);
        encoder.hasher.update(domain);
        encoder
    }

    fn u8(&mut self, value: u8) {
        self.hasher.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.hasher.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.hasher.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.hasher.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}

#[cfg(test)]
pub(super) struct ClosedSubjectStarterFixture {
    pub(super) relation_id: super::relation::RelationId,
    pub(super) open_support: MechanismSupportCatalogBuilder,
    pub(super) support: MechanismSupportCatalogBuilder,
    pub(super) structural: StructuralMechanismCatalogBuilder,
    pub(super) mechanism_id: StructuralMechanismId,
    pub(super) node_ids: Box<[StructuralNodeId]>,
    pub(super) edge_ids: Box<[StructuralEdgeId]>,
}

/// Small closed two-case quotient used by support/projection unit tests. Its
/// two nodes and one edge occur on both endpoints with equal outcomes, so
/// activation support is exact two while differential support is exact empty.
#[cfg(test)]
pub(super) fn closed_subject_starter_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(false, false, false)
}

/// Two complete raw signatures which quotient to the same structural subject
/// support and whose cases are distinct successors of one shared origin.
#[cfg(test)]
fn multi_signature_shared_starter_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(false, true, true)
}

/// Variant of the shared subject fixture which can leave the second case
/// permanently unavailable and can make both cases share one origin starter.
/// `open_support` is captured before the exact target seal is attached, while
/// `support` is the subsequently closed view over the same semantic stream.
#[cfg(test)]
fn subject_starter_fixture(
    second_case_unavailable: bool,
    shared_starter: bool,
    split_raw_signatures: bool,
) -> ClosedSubjectStarterFixture {
    use super::mechanism_incidence::{
        MechanismIncidenceCatalogBuilder, MechanismSignatureDefinition,
        MechanismTargetCaseSetCommitment, MechanismUnavailableReasonDefinition,
    };
    use super::relation::{
        AdmissionId, FindPolarity, QuestionContentRoot, RelationId, RelationLineageId,
        RelationProvenance, RelationSupportId, SourceRow, SuccessorRow,
    };
    use super::relational_mechanism_executor::{
        RelationalIfDecisionOutcome, RelationalMechanismActivationStep,
        RelationalMechanismCalleeId, RelationalMechanismEventKind, RelationalMechanismEventOutcome,
        RelationalMechanismSiteId,
    };
    use super::structural_mechanism::{
        derive_structural_signature_quotient_v1, relational_structural_derivation_budget,
        StructuralActivationInputV1, StructuralOccurrenceInputV1, StructuralPairedDagInputV1,
    };
    use super::transition::TransitionId;
    use super::ExploreValue;
    use crate::{
        AnalysisProgramId, CheckedCallableId, CheckedDeclarationOccurrenceId, DeclarationId,
        DeclarationKind, ExprSiteId, ModuleId,
    };

    fn declaration(name: &str, ordinal: usize) -> DeclarationId {
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

    fn expression_site(
        program: &AnalysisProgramId,
        name: &str,
        ordinal: usize,
    ) -> RelationalMechanismSiteId {
        RelationalMechanismSiteId::from_checked_expression(&ExprSiteId {
            analysis_program: program.clone(),
            declaration: declaration(name, ordinal),
            normalized_declaration_ordinal: ordinal,
            ast_path: vec![ordinal as u32].into_boxed_slice(),
        })
        .expect("checked expression fixture")
    }

    fn provenance(label: &[u8]) -> RelationProvenance {
        RelationProvenance::new(
            [RelationLineageId::from_canonical_preimage(label)],
            [RelationSupportId::from_canonical_preimage(label)],
        )
    }

    assert!(!split_raw_signatures || !second_case_unavailable);

    let relation_id = RelationId::from_canonical_semantic_preimage(b"subject-starter-fixture");
    let admission_id =
        AdmissionId::from_canonical_admission_preimage(relation_id, b"all-fixture-cases");
    let question_id = super::relation::QuestionId::from_canonical_find_preimage(
        admission_id,
        b"find-all-fixture-cases",
        FindPolarity::All,
    );
    let target = MechanismTargetId::Selected;
    let request_id = MechanismRequestId::from_canonical_request_preimages(
        question_id,
        target,
        b"fixture-observation",
        b"fixture-normalization",
    );
    let scope = MechanismRequestScope::new(request_id, question_id, target);

    let mut relation = super::relation::RelationCatalogBuilder::new(relation_id);
    let mut cases = Vec::new();
    if shared_starter {
        let source_key = relation
            .insert_source(SourceRow::new(
                ExploreValue::Int(0),
                ExploreValue::Int(100),
                provenance(b"fixture-shared-source"),
            ))
            .expect("fixture shared source");
        for (ordinal, after) in [101_i64, 102_i64].into_iter().enumerate() {
            let label = format!("fixture-shared-successor-{ordinal}");
            let (_, case_id) = relation
                .insert_successor(
                    source_key,
                    SuccessorRow::new(ExploreValue::Int(after), provenance(label.as_bytes())),
                )
                .expect("fixture shared-source successor");
            cases.push(case_id);
        }
    } else {
        for (ordinal, before) in [100_i64, 200_i64].into_iter().enumerate() {
            let label = format!("fixture-case-{ordinal}");
            let source_key = relation
                .insert_source(SourceRow::new(
                    ExploreValue::Int(ordinal as i64),
                    ExploreValue::Int(before),
                    provenance(label.as_bytes()),
                ))
                .expect("fixture source");
            let (_, case_id) = relation
                .insert_successor(
                    source_key,
                    SuccessorRow::new(ExploreValue::Int(before + 1), provenance(label.as_bytes())),
                )
                .expect("fixture successor");
            cases.push(case_id);
        }
    }

    let mut signatures = vec![MechanismSignatureDefinition::from_canonical_definition(
        request_id,
        b"subject-starter-structural-fixture".as_slice(),
    )];
    if split_raw_signatures {
        signatures.push(MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"subject-starter-structural-fixture-second".as_slice(),
        ));
    }
    let mut incidence = MechanismIncidenceCatalogBuilder::new(scope);
    for case_id in cases.iter().copied() {
        incidence
            .insert_target_case(case_id)
            .expect("fixture target case");
    }
    for (ordinal, case_id) in cases.iter().copied().enumerate() {
        if second_case_unavailable && ordinal == 1 {
            incidence
                .record_unavailable(
                    case_id,
                    &MechanismUnavailableReasonDefinition::from_canonical_reason(
                        b"fixture replay unavailable".as_slice(),
                    ),
                )
                .expect("fixture unavailable terminal");
        } else {
            let signature = &signatures[if split_raw_signatures { ordinal } else { 0 }];
            incidence
                .record_incidence(
                    case_id,
                    TransitionId::from_bytes(
                        Sha256::digest(format!("transition-{ordinal}")).into(),
                    ),
                    signature,
                )
                .expect("fixture incidence");
        }
    }

    let program = AnalysisProgramId("11".repeat(32).into_boxed_str());
    let callable_declaration = declaration("fixture_callee", 10);
    let callable = CheckedCallableId {
        declaration: CheckedDeclarationOccurrenceId {
            declaration: callable_declaration,
            declaration_occurrence_ordinal: 0,
            normalized_ordinal: 0,
        },
        structural_path: Box::default(),
    };
    let callee_site =
        RelationalMechanismSiteId::from_checked_callable(&program, &callable).expect("callee site");
    let activation = StructuralActivationInputV1 {
        parent: None,
        step: RelationalMechanismActivationStep::new(
            expression_site(&program, "fixture_call", 11),
            RelationalMechanismCalleeId::function(callee_site).expect("function callee"),
            0,
        )
        .expect("activation step"),
    };
    let same_outcome = Some(RelationalMechanismEventOutcome::IfDecision(
        RelationalIfDecisionOutcome::Then,
    ));
    let occurrences = [
        StructuralOccurrenceInputV1 {
            before_owner_activation: Some(0),
            after_owner_activation: Some(0),
            site: expression_site(&program, "fixture_root", 12),
            kind: RelationalMechanismEventKind::IfDecision,
            before_outcome: same_outcome.clone(),
            after_outcome: same_outcome.clone(),
            before_root: true,
            after_root: true,
        },
        StructuralOccurrenceInputV1 {
            before_owner_activation: Some(0),
            after_owner_activation: Some(0),
            site: expression_site(&program, "fixture_dependency", 13),
            kind: RelationalMechanismEventKind::IfDecision,
            before_outcome: same_outcome.clone(),
            after_outcome: same_outcome,
            before_root: false,
            after_root: false,
        },
    ];
    let artifacts = signatures
        .iter()
        .map(|signature| {
            let mut budget = relational_structural_derivation_budget();
            budget.admit_source(0).expect("fixture source budget");
            budget
                .admit_activations(2)
                .expect("fixture activation budget");
            budget
                .admit_occurrences(4)
                .expect("fixture occurrence budget");
            budget.admit_edges(2).expect("fixture edge budget");
            budget
                .finish_shape_admission()
                .expect("fixture shape budget");
            derive_structural_signature_quotient_v1(
                StructuralPairedDagInputV1 {
                    signature_id: signature.id(),
                    before_activations: vec![activation.clone()].into_boxed_slice(),
                    after_activations: vec![activation.clone()].into_boxed_slice(),
                    occurrences: occurrences.clone().into(),
                    before_edges: vec![(0, 1)].into_boxed_slice(),
                    after_edges: vec![(0, 1)].into_boxed_slice(),
                },
                budget,
            )
            .expect("fixture structural quotient")
        })
        .collect::<Vec<_>>();
    let first_artifact = artifacts.first().expect("fixture structural artifact");
    assert!(first_artifact.differential_node_membership().is_empty());
    assert!(first_artifact.differential_edge_membership().is_empty());
    let mechanism_id = first_artifact.mechanism().id();
    let node_ids = first_artifact.node_membership().to_vec().into_boxed_slice();
    let edge_ids = first_artifact.edge_membership().to_vec().into_boxed_slice();
    let mut structural = StructuralMechanismCatalogBuilder::new(request_id);
    for artifact in &artifacts {
        assert_eq!(artifact.mechanism().id(), mechanism_id);
        assert_eq!(artifact.node_membership(), node_ids.as_ref());
        assert_eq!(artifact.edge_membership(), edge_ids.as_ref());
        assert!(artifact.differential_node_membership().is_empty());
        assert!(artifact.differential_edge_membership().is_empty());
        structural
            .intern_artifact(artifact)
            .expect("fixture structural interning");
    }
    let mut signature_ids = signatures
        .iter()
        .map(MechanismSignatureDefinition::id)
        .collect::<Vec<_>>();
    signature_ids.sort_unstable();
    structural
        .close_against_expected_signatures(signature_ids.len() as u128, signature_ids)
        .expect("fixture structural closure");

    let mut support = MechanismSupportCatalogBuilder::new(scope);
    for case_id in cases.iter().copied() {
        support
            .accept_target_case(
                &incidence,
                relation.case(case_id).expect("fixture retained case"),
            )
            .expect("fixture support target");
    }
    support
        .sync_structural_assignments(&structural)
        .expect("fixture support structural assignments");
    support
        .sync_incidence_terminals_through(
            &incidence,
            &structural,
            incidence.terminal_discovery_count() as u128,
        )
        .expect("fixture support terminals");
    let open_support = support.clone();
    incidence
        .seal_selected_target_commitment(
            QuestionContentRoot::from_journal_codec_bytes([0x51; 32]),
            MechanismTargetCaseSetCommitment::from_cases(cases.iter().copied()),
        )
        .expect("fixture target seal");
    support
        .attach_target_seal(&incidence)
        .expect("fixture support target seal");
    support
        .close(
            incidence.closed_ref().expect("fixture incidence closure"),
            &structural,
        )
        .expect("fixture support closure");

    ClosedSubjectStarterFixture {
        relation_id,
        open_support,
        support,
        structural,
        mechanism_id,
        node_ids,
        edge_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mechanism_support_key(fixture: &ClosedSubjectStarterFixture) -> MechanismSupportKey {
        MechanismSupportKey::new(
            fixture.support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        )
    }

    #[test]
    fn open_target_keeps_confirmed_case_and_starter_support_unknown() {
        let mut fixture = closed_subject_starter_fixture();
        let key = MechanismSupportKey::new(
            fixture.open_support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        );

        let view = fixture
            .open_support
            .derive_view(key, &fixture.structural)
            .expect("open mechanism support view");

        assert!(!fixture.open_support.target_is_complete());
        assert!(view.target_frontier_is_open());
        assert_eq!(
            view.case_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: 2,
            }
        );
        assert_eq!(
            view.starter_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: 2,
            }
        );
        assert_eq!(
            view.starter_upper_provenance(),
            MechanismStarterUpperProvenance::OpenOpaque
        );
        assert!(!view.fiber_expr_bounds_are_equal());
    }

    #[test]
    fn sealed_unavailable_residual_keeps_subject_case_and_starter_counts_interval_valued() {
        let fixture = subject_starter_fixture(true, false, false);
        let summary = fixture
            .support
            .derive_closed_factorized_subject_summary(
                mechanism_support_key(&fixture),
                &fixture.structural,
            )
            .expect("closed residual support summary");

        assert_eq!(
            summary.case_count(),
            MechanismSupportCount::Interval {
                lower_bound: 1,
                upper_bound: 2,
            }
        );
        assert_eq!(
            summary.starter_count(),
            MechanismSupportCount::Interval {
                lower_bound: 1,
                upper_bound: 2,
            }
        );
        assert!(matches!(
            summary.starter_bound_basis(),
            MechanismFactorizedStarterBoundBasis::ConservativeTargetProjectionUpper { .. }
        ));
        assert!(!summary.fiber_expr_bounds_are_equal());
    }

    #[test]
    fn target_starter_saturation_closes_only_the_distinct_starter_count() {
        let fixture = subject_starter_fixture(true, true, false);
        let key = mechanism_support_key(&fixture);
        let summary = fixture
            .support
            .derive_closed_factorized_subject_summary(key, &fixture.structural)
            .expect("starter-saturated support summary");

        assert_eq!(
            summary.case_count(),
            MechanismSupportCount::Interval {
                lower_bound: 1,
                upper_bound: 2,
            }
        );
        assert_eq!(summary.starter_count(), MechanismSupportCount::Exact(1));
        assert!(matches!(
            summary.starter_bound_basis(),
            MechanismFactorizedStarterBoundBasis::ExactTargetStarterSaturation { .. }
        ));
        assert!(!summary.fiber_expr_bounds_are_equal());
        assert_eq!(
            fixture
                .support
                .derive_closed_subject_starter_projection_authority(key, &fixture.structural),
            Err(MechanismSupportError::ClosurePrerequisite(
                "exact correlated structural-subject starter support",
            ))
        );
    }

    #[test]
    fn closed_subject_pager_covers_mechanism_node_and_edge_without_eager_union() {
        let fixture = closed_subject_starter_fixture();
        let scope = fixture.support.scope();
        let subjects = [
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::Activation,
                edge_id: fixture.edge_ids[0],
            },
        ];

        let mut plan_ids = BTreeSet::new();
        for subject in subjects {
            let key = MechanismSupportKey::new(scope, subject);
            let authority = fixture
                .support
                .derive_closed_subject_starter_projection_authority(key, &fixture.structural)
                .expect("closed subject authority");
            assert_eq!(authority.subject(), subject);
            assert_eq!(authority.exact_case_count(), 2);
            assert!(plan_ids.insert(authority.projection_plan_id()));

            let first = fixture
                .support
                .closed_subject_starter_page(
                    authority,
                    &fixture.structural,
                    fixture.relation_id,
                    None,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("first bounded subject page");
            assert_eq!(first.members().len(), 1);
            assert!(!first.exhausted());
            let first_cursor = first.end_cursor().expect("first member cursor");
            let second = fixture
                .support
                .closed_subject_starter_page(
                    authority,
                    &fixture.structural,
                    fixture.relation_id,
                    Some(first_cursor),
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("second bounded subject page");
            assert_eq!(second.start_after(), Some(first_cursor));
            assert_eq!(second.members().len(), 1);
            assert!(second.exhausted());
            assert!(second
                .end_cursor()
                .is_some_and(|cursor| cursor > first_cursor));
        }
    }

    #[test]
    fn shared_origin_across_raw_signatures_remains_distinct_non_additive_subject_support() {
        let fixture = multi_signature_shared_starter_fixture();
        let scope = fixture.support.scope();
        let support_closure = fixture.support.closure().expect("fixture support closure");
        let structural_closure = fixture
            .structural
            .closure()
            .expect("fixture structural closure");
        assert_eq!(support_closure.target_case_count(), 2);
        assert_eq!(support_closure.target_starter_count(), 1);
        assert_eq!(support_closure.signature_fiber_count(), 2);
        assert_eq!(structural_closure.expected_signature_count(), 2);
        assert_eq!(structural_closure.counts().assignments(), 2);
        assert_eq!(structural_closure.counts().mechanisms(), 1);

        let subjects = [
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::Activation,
                edge_id: fixture.edge_ids[0],
            },
        ];
        let mut view_roots = BTreeSet::new();
        let mut inner_case_roots = BTreeSet::new();
        let mut inner_starter_roots = BTreeSet::new();
        let mut plan_ids = BTreeSet::new();
        let mut all_raw_signatures = BTreeSet::new();
        let mut overlapping_case_count = 0;
        let mut overlapping_starter_count = 0;

        for subject in subjects {
            let key = MechanismSupportKey::new(scope, subject);
            let view = fixture
                .support
                .derive_closed_view(key, &fixture.structural)
                .expect("closed overlapping subject view");
            assert_eq!(view.key(), key);
            assert_eq!(view.case_count(), MechanismSupportCount::Exact(2));
            assert_eq!(view.starter_count(), MechanismSupportCount::Exact(1));
            assert!(view.fiber_expr_bounds_are_equal());
            assert!(view_roots.insert(view.root()));
            inner_case_roots.insert(view.inner_case_root());
            inner_starter_roots.insert(view.inner_starter_root());

            let authority = fixture
                .support
                .derive_closed_subject_starter_projection_authority(key, &fixture.structural)
                .expect("closed overlapping subject authority");
            assert_eq!(authority.key(), key);
            assert_eq!(authority.subject(), subject);
            assert_eq!(authority.exact_case_count(), 2);
            assert_eq!(authority.support_root(), support_closure.root());
            assert_eq!(authority.structural_root(), structural_closure.root());
            assert!(plan_ids.insert(authority.projection_plan_id()));

            let page = fixture
                .support
                .closed_subject_starter_page(
                    authority,
                    &fixture.structural,
                    fixture.relation_id,
                    None,
                    NonZeroU16::new(8).unwrap(),
                )
                .expect("complete overlapping subject page");
            assert_eq!(page.authority(), authority);
            assert_eq!(page.start_after(), None);
            assert_eq!(page.members().len(), 2);
            assert!(page.exhausted());

            let source_keys = page
                .members()
                .iter()
                .map(|member| member.source_key())
                .collect::<BTreeSet<_>>();
            let successor_keys = page
                .members()
                .iter()
                .map(|member| member.successor_key())
                .collect::<BTreeSet<_>>();
            let case_ids = page
                .members()
                .iter()
                .map(|member| member.case_id())
                .collect::<BTreeSet<_>>();
            let raw_signatures = page
                .members()
                .iter()
                .map(|member| member.raw_signature_id())
                .collect::<BTreeSet<_>>();
            assert_eq!(source_keys.len(), 1);
            assert_eq!(successor_keys.len(), 2);
            assert_eq!(case_ids.len(), 2);
            assert_eq!(raw_signatures.len(), 2);
            all_raw_signatures.extend(raw_signatures);

            overlapping_case_count += authority.exact_case_count();
            overlapping_starter_count += 1;
        }

        assert_eq!(view_roots.len(), 3);
        assert_eq!(plan_ids.len(), 3);
        assert_eq!(inner_case_roots.len(), 1);
        assert_eq!(inner_starter_roots.len(), 1);
        assert_eq!(all_raw_signatures.len(), 2);
        assert_eq!(overlapping_case_count, 6);
        assert_eq!(overlapping_starter_count, 3);
        assert_ne!(overlapping_case_count, support_closure.target_case_count());
        assert_ne!(
            overlapping_starter_count,
            support_closure.target_starter_count()
        );
    }

    #[test]
    fn known_differential_node_and_edge_without_signatures_are_exact_empty() {
        let fixture = closed_subject_starter_fixture();
        let scope = fixture.support.scope();
        let subjects = [
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::DifferentialParticipation,
                node_id: fixture.node_ids[0],
            },
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::DifferentialParticipation,
                edge_id: fixture.edge_ids[0],
            },
        ];

        for subject in subjects {
            let key = MechanismSupportKey::new(scope, subject);
            let summary = fixture
                .support
                .derive_closed_factorized_subject_summary(key, &fixture.structural)
                .expect("known empty subject summary");
            assert_eq!(summary.case_count(), MechanismSupportCount::Exact(0));
            assert_eq!(summary.starter_count(), MechanismSupportCount::Exact(0));
            assert_eq!(
                summary.starter_bound_basis(),
                MechanismFactorizedStarterBoundBasis::ExactEmpty
            );
            let authority = fixture
                .support
                .derive_closed_subject_starter_projection_authority(key, &fixture.structural)
                .expect("known empty subject authority");
            assert_eq!(authority.exact_case_count(), 0);
            let page = fixture
                .support
                .closed_subject_starter_page(
                    authority,
                    &fixture.structural,
                    fixture.relation_id,
                    None,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("known empty subject page");
            assert!(page.members().is_empty());
            assert!(page.exhausted());
            assert_eq!(page.end_cursor(), None);
        }
    }
}
