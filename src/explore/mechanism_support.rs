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

pub(crate) const MECHANISM_SUPPORT_VERSION: u32 = 4;
pub(crate) const MECHANISM_SUPPORT_VIEW_VERSION: u32 = 6;
pub(crate) const MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION: u32 = 3;
pub(crate) const MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION: u32 = 2;
pub(crate) const MECHANISM_SUPPORT_SLICE_ID_VERSION: u32 = 1;
pub(crate) const MECHANISM_SUPPORT_FIBER_EXPR_VERSION: u32 = 1;
pub(crate) const MECHANISM_STARTER_PROJECTION_EXPR_VERSION: u32 = 1;
pub(crate) const MECHANISM_STARTER_PROJECTION_PLAN_VERSION: u32 = 3;
/// Automatic all-subject publication may inspect at most this many immutable
/// signature-fiber summaries for one row. The cap is part of the summary
/// schema, not a runtime tuning knob: crossing it yields honest wider bounds
/// and a deferred projection plan instead of a hidden full union.
pub(crate) const AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT: usize = 256;
/// One explicit-demand backfill event may inspect at most this many imported
/// structural assignments. The bound is protocol behavior rather than a
/// runtime tuning knob so replay never turns one late registration into an
/// unbounded pause.
pub(crate) const EXPLICIT_OBSERVATION_BACKFILL_MAX_ASSIGNMENTS: usize = 256;
// One broad shared-node projection can approach the full target size. Keep the
// default cache to one such derived accelerator; authority remains factorized
// in the signature fibers and callers may explicitly choose another limit.
const DEFAULT_HOT_SUBJECT_PROJECTION_LIMIT: usize = 1;

const SUPPORT_VIEW_ROOT_V6: &[u8] = b"futuruna.explore.mechanism-support-view-root.v6";
const SUPPORT_SLICE_ID_V1: &[u8] = b"futuruna.explore.mechanism-support-slice-id.v1";
const FACTORIZED_SUPPORT_OBSERVATION_SIGNATURE_PREFIX_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-observation-signature-prefix-root.v2";
const FACTORIZED_SUPPORT_OBSERVATION_STARTER_PREFIX_ROOT_V1: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-observation-starter-prefix-root.v1";
const FACTORIZED_SUPPORT_OBSERVATION_SUMMARY_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-observation-summary-root.v2";
const FACTORIZED_SUPPORT_OBSERVATION_INNER_FIBER_EXPR_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-observation-inner-fiber-expr-root.v2";
const FACTORIZED_SUPPORT_OBSERVATION_OUTER_FIBER_EXPR_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-observation-outer-fiber-expr-root.v2";
const FACTORIZED_SUBJECT_SIGNATURE_PREFIX_ROOT_V3: &[u8] =
    b"futuruna.explore.mechanism-factorized-subject-signature-prefix-root.v3";
const FACTORIZED_SUPPORT_SLICE_SIGNATURE_PREFIX_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-slice-signature-prefix-root.v2";
const FACTORIZED_SUBJECT_SUMMARY_ROOT_V3: &[u8] =
    b"futuruna.explore.mechanism-factorized-subject-summary-root.v3";
const FACTORIZED_SUPPORT_SLICE_SUMMARY_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-factorized-support-slice-summary-root.v2";
const SUPPORT_FIBER_EXPR_ROOT_V1: &[u8] = b"futuruna.explore.mechanism-support-fiber-expr-root.v1";
const SUPPORT_SLICE_FIBER_EXPR_ROOT_V1: &[u8] =
    b"futuruna.explore.mechanism-support-slice-fiber-expr-root.v1";
const FIBER_EXPR_FACTORIZED_SUBJECT_UNION: u8 = 0x01;
const FIBER_EXPR_MATERIALIZED_PROJECTION: u8 = 0x02;
const FIBER_EXPR_POSSIBLE_SUPPORT_ENVELOPE: u8 = 0x03;
const FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE: u8 = 0x01;
const FIBER_EXPR_SOURCE_CONTEXT_BEFORE: u8 = 0x01;
const FIBER_EXPR_SUCCESSOR_AFTER: u8 = 0x01;
const STARTER_PROJECTION_EXPR_FACTORIZED_SUBJECT_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-projection-expression.factorized-subject.v1";
const STARTER_PROJECTION_EXPR_OBSERVATION_PREFIX_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-projection-expression.observation-prefix.v1";
const STARTER_PROJECTION_EXPR_MATERIALIZED_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-projection-expression.materialized.v1";
const STARTER_PROJECTION_EXPR_TARGET_ENVELOPE_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-projection-expression.target-envelope.v1";
const STARTER_PROJECTION_EXPR_OPAQUE_UPPER_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-projection-expression.opaque-upper.v1";
const STARTER_PROJECTION_PLAN_ID_V3: &[u8] =
    b"futuruna.explore.mechanism-subject-starter-projection-plan-id.v3";
const SUPPORT_SLICE_STARTER_PROJECTION_PLAN_ID_V2: &[u8] =
    b"futuruna.explore.mechanism-support-slice-starter-projection-plan-id.v2";
const SUPPORT_FRONTIER_ROOT_V4: &[u8] = b"futuruna.explore.mechanism-support-frontier-root.v4";
const SUPPORT_FRONTIER_IMPORTED_PREFIX_ROOT_V4: &[u8] =
    b"futuruna.explore.mechanism-support-frontier-imported-prefix-root.v4";
const SUPPORT_CLOSURE_ROOT_V2: &[u8] = b"futuruna.explore.mechanism-support-closure-root.v2";
const SHARED_RESIDUAL_ROOT_V2: &[u8] =
    b"futuruna.explore.mechanism-support-factorized-residual-root.v2";
const FIBER_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-fiber-case-index.v1";
const PENDING_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-pending-case-index.v1";
const UNAVAILABLE_CASE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unavailable-case-index.v1";
const SIGNATURE_FIBER_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-fiber-index.v1";
const SIGNATURE_STARTER_SET_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-starter-set-index.v1";
const TERMINAL_FACT_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-terminal-fact-index.v1";
const TARGET_STARTER_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-target-starter-index.v1";
const TARGET_STARTER_SET_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-target-starter-set-index.v1";
const SUBJECT_SIGNATURE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-signature-index.v1";
const SUBJECT_CASE_INDEX_V1: &[u8] = b"futuruna.explore.mechanism-support-subject-case-index.v1";
const SUBJECT_CORRELATED_STARTER_INDEX_V2: &[u8] =
    b"futuruna.explore.mechanism-support-subject-correlated-starter-index.v2";
const SUBJECT_STARTER_SET_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-starter-set-index.v1";
const SUBJECT_SUCCESSOR_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-subject-successor-index.v1";
const UNASSIGNED_SIGNATURE_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unassigned-signature-index.v1";
const AUTOMATIC_OBSERVATION_REGISTRY_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-automatic-observation-registry-index.v1";
const DIRTY_AUTOMATIC_OBSERVATION_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-dirty-automatic-observation-index.v1";
const EXPLICIT_OBSERVATION_REGISTRY_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-explicit-observation-registry-index.v1";
const PENDING_EXPLICIT_OBSERVATION_BACKFILL_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-pending-explicit-observation-backfill-index.v1";
const DIRTY_EXPLICIT_OBSERVATION_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-dirty-explicit-observation-index.v1";
const UNSEALED_EXPLICIT_OBSERVATION_INDEX_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unsealed-explicit-observation-index.v1";
const COORDINATE_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-coordinate-value.v1";
const UNAVAILABLE_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-unavailable-value.v1";
const TERMINAL_VALUE_V1: &[u8] = b"futuruna.explore.mechanism-support-terminal-value.v1";
const SIGNATURE_FIBER_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-fiber-value.v1";
const SIGNATURE_STARTER_SET_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-signature-starter-set-value.v1";
const STARTER_SET_MEMBER_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-starter-set-member-value.v1";
const CORRELATED_STARTER_FIBER_VALUE_V2: &[u8] =
    b"futuruna.explore.mechanism-support-correlated-starter-fiber-value.v2";
const TARGET_STARTER_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-target-starter-value.v1";
const AUTOMATIC_OBSERVATION_REGISTRY_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-automatic-observation-registry-value.v1";
const DIRTY_AUTOMATIC_OBSERVATION_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-dirty-automatic-observation-value.v1";
const EXPLICIT_OBSERVATION_REGISTRY_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-explicit-observation-registry-value.v1";
const PENDING_EXPLICIT_OBSERVATION_BACKFILL_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-pending-explicit-observation-backfill-value.v1";
const DIRTY_EXPLICIT_OBSERVATION_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-dirty-explicit-observation-value.v1";
const UNSEALED_EXPLICIT_OBSERVATION_VALUE_V1: &[u8] =
    b"futuruna.explore.mechanism-support-unsealed-explicit-observation-value.v1";

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

    /// Reconstruct a serialized slice coordinate before its owning analysis
    /// state is available. Derivation validates these parts against the live
    /// request scope before trusting the key.
    pub(crate) const fn from_journal_codec_parts(
        request_id: MechanismRequestId,
        target: MechanismTargetId,
        subject: MechanismSupportSubject,
    ) -> Self {
        Self {
            request_id,
            target,
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

/// Stable identity of one complete support-slice coordinate. Unlike a subject
/// identity, this commits both the total/conditioned selector and (for a
/// conditioned slice) the enclosing mechanism route.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportSliceId([u8; 32]);

impl MechanismSupportSliceId {
    pub(crate) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One structural subject's total support or its support along one enclosing
/// mechanism route. The underlying [`MechanismSupportKey`] remains unchanged:
/// route conditioning is a derived intersection of the subject and mechanism
/// signature indexes, never a second structural node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportSlice {
    key: MechanismSupportKey,
    within_mechanism: Option<StructuralMechanismId>,
}

impl MechanismSupportSlice {
    pub(crate) const fn total(key: MechanismSupportKey) -> Self {
        Self {
            key,
            within_mechanism: None,
        }
    }

    pub(crate) const fn within_mechanism(
        key: MechanismSupportKey,
        mechanism_id: StructuralMechanismId,
    ) -> Self {
        Self {
            key,
            within_mechanism: Some(mechanism_id),
        }
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.key
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.key.subject()
    }

    pub(crate) const fn enclosing_mechanism(self) -> Option<StructuralMechanismId> {
        self.within_mechanism
    }

    pub(crate) fn id(self) -> MechanismSupportSliceId {
        derive_support_slice_id(self)
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

    const fn proves_exact_starter_set(self) -> bool {
        matches!(
            self,
            Self::ExactCorrelatedInner { .. } | Self::ExactStarterSetFromTargetSaturation { .. }
        )
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

/// Authenticated identity of a distinct `SourceKey<(Context, Before)>` set
/// expression. This is deliberately a different type and hash domain from
/// [`MechanismSupportFiberExprRoot`]: changing successor multiplicity may
/// change `S` without changing its distinct-starter projection `P`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismStarterProjectionExprRoot([u8; 32]);

impl MechanismStarterProjectionExprRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Whether the distinct starter set `P` is closed. This is intentionally
/// independent of both its scalar count and closure of the successor fibers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MechanismStarterSetStatus {
    Open,
    ExactStarterSet,
}

impl MechanismStarterSetStatus {
    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::ExactStarterSet)
    }
}

/// Whether the complete correlated `SourceKey -> Set<SuccessorKey>` relation
/// `S` is closed, including every dependent successor fiber.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MechanismCorrelatedSupportStatus {
    Open,
    ExactCorrelatedSupport,
}

impl MechanismCorrelatedSupportStatus {
    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::ExactCorrelatedSupport)
    }
}

/// Independently authenticated set bounds for correlated case support `S`
/// and its distinct-source projection `P = distinct_sources(S)`.
///
/// Root equality is necessary but never sufficient for an exactness claim;
/// callers must inspect the explicit status. Construction is checked so an
/// exact correlated claim cannot exist without both equal `S` bounds and an
/// exact starter set, while an independently proved exact starter set may
/// close before the successor fibers do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportExpressionBounds {
    case_inner_root: MechanismSupportFiberExprRoot,
    case_outer_root: MechanismSupportFiberExprRoot,
    starter_inner_root: MechanismStarterProjectionExprRoot,
    starter_outer_root: MechanismStarterProjectionExprRoot,
    starter_set_status: MechanismStarterSetStatus,
    correlated_support_status: MechanismCorrelatedSupportStatus,
}

impl MechanismSupportExpressionBounds {
    fn checked(
        case_inner_root: MechanismSupportFiberExprRoot,
        case_outer_root: MechanismSupportFiberExprRoot,
        starter_inner_root: MechanismStarterProjectionExprRoot,
        starter_outer_root: MechanismStarterProjectionExprRoot,
        starter_set_status: MechanismStarterSetStatus,
        correlated_support_status: MechanismCorrelatedSupportStatus,
    ) -> Result<Self, MechanismSupportError> {
        if (starter_set_status.is_exact() && starter_inner_root != starter_outer_root)
            || (correlated_support_status.is_exact()
                && (case_inner_root != case_outer_root || !starter_set_status.is_exact()))
        {
            return Err(MechanismSupportError::SupportExpressionBoundsConflict);
        }
        Ok(Self {
            case_inner_root,
            case_outer_root,
            starter_inner_root,
            starter_outer_root,
            starter_set_status,
            correlated_support_status,
        })
    }

    pub(crate) const fn case_inner_root(self) -> MechanismSupportFiberExprRoot {
        self.case_inner_root
    }

    pub(crate) const fn case_outer_root(self) -> MechanismSupportFiberExprRoot {
        self.case_outer_root
    }

    pub(crate) const fn starter_inner_root(self) -> MechanismStarterProjectionExprRoot {
        self.starter_inner_root
    }

    pub(crate) const fn starter_outer_root(self) -> MechanismStarterProjectionExprRoot {
        self.starter_outer_root
    }

    pub(crate) const fn starter_set_status(self) -> MechanismStarterSetStatus {
        self.starter_set_status
    }

    pub(crate) const fn correlated_support_status(self) -> MechanismCorrelatedSupportStatus {
        self.correlated_support_status
    }

    pub(crate) fn case_bounds_are_equal(self) -> bool {
        self.case_inner_root == self.case_outer_root
    }

    pub(crate) fn starter_bounds_are_equal(self) -> bool {
        self.starter_inner_root == self.starter_outer_root
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
    slice: MechanismSupportSlice,
    question_id: QuestionId,
    projection_plan_id: MechanismStarterProjectionPlanId,
    support_expression_bounds: MechanismSupportExpressionBounds,
    structural_root: StructuralQuotientClosureRoot,
    support_root: MechanismSupportClosureRoot,
    exact_case_count: u128,
}

impl MechanismClosedSubjectStarterProjectionAuthority {
    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.slice.key()
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.slice.subject()
    }

    pub(crate) const fn enclosing_mechanism(self) -> Option<StructuralMechanismId> {
        self.slice.enclosing_mechanism()
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.projection_plan_id
    }

    pub(crate) const fn support_expression_bounds(self) -> MechanismSupportExpressionBounds {
        self.support_expression_bounds
    }

    /// Transitional case-support accessor. New consumers should retain the
    /// complete S/P bundle through [`Self::support_expression_bounds`].
    pub(crate) const fn correlated_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_inner_root()
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

    pub(crate) const fn support_expression_bounds(self) -> MechanismSupportExpressionBounds {
        self.inner.support_expression_bounds()
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
    /// The target may still admit new `(Context, Before)` starters, so no
    /// finite outer starter projection is yet justified.
    OpenOpaque,
    ExactEmpty,
    ExactFactorizedBoundCollapse,
    ExactTargetStarterSaturation {
        target_starter_root: [u8; 32],
    },
    ConservativeTargetProjectionUpper {
        target_starter_root: [u8; 32],
    },
}

impl MechanismFactorizedStarterBoundBasis {
    const fn proves_exact_starter_set(self) -> bool {
        matches!(
            self,
            Self::ExactEmpty
                | Self::ExactFactorizedBoundCollapse
                | Self::ExactTargetStarterSaturation { .. }
        )
    }
}

/// Constant-space automatic-publication summary. Authenticated inner/outer
/// fiber-expression identities retain the correlation contract without
/// serializing values. Exact correlated cells remain a separate, explicitly
/// authorized projection job derived from the plan even when the expression
/// bounds or a scalar bound have collapsed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismFactorizedSubjectSummary {
    slice: MechanismSupportSlice,
    root: MechanismFactorizedSubjectSummaryRoot,
    projection_plan_id: MechanismStarterProjectionPlanId,
    support_expression_bounds: MechanismSupportExpressionBounds,
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
    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.slice.key()
    }

    pub(crate) const fn root(self) -> MechanismFactorizedSubjectSummaryRoot {
        self.root
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.projection_plan_id
    }

    pub(crate) const fn support_expression_bounds(self) -> MechanismSupportExpressionBounds {
        self.support_expression_bounds
    }

    pub(crate) const fn inner_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_inner_root()
    }

    pub(crate) const fn outer_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_outer_root()
    }

    pub(crate) fn fiber_expr_bounds_are_equal(self) -> bool {
        self.support_expression_bounds.case_bounds_are_equal()
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

/// Immutable identity of one prefix-relative support observation. The root is
/// distinct from both the evolving support-frontier root and the final support
/// closure root: it commits one slice's bounded interpretation of exactly one
/// durable imported prefix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismFactorizedSupportObservationSummaryRoot([u8; 32]);

impl MechanismFactorizedSupportObservationSummaryRoot {
    pub(crate) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Constant-space observation of one total or mechanism-conditioned support
/// slice at a durable support frontier. The inner expression contains only
/// confirmed imported signature fibers inspected under the automatic scan
/// cap. The outer expression retains the shared residual, any unscanned
/// imported supporting signatures, and the opaque undiscovered target while
/// it remains open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismFactorizedSupportObservationSummary {
    slice: MechanismSupportSlice,
    slice_id: MechanismSupportSliceId,
    root: MechanismFactorizedSupportObservationSummaryRoot,
    frontier_root: MechanismSupportFrontierRoot,
    imported_prefix_root: [u8; 32],
    structural_root: Option<StructuralQuotientClosureRoot>,
    support_root: Option<MechanismSupportClosureRoot>,
    projection_plan_id: Option<MechanismStarterProjectionPlanId>,
    target_frontier_open: bool,
    support_expression_bounds: MechanismSupportExpressionBounds,
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    signature_scan_complete: bool,
    signature_prefix_root: [u8; 32],
    residual: MechanismSupportResidualSummary,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_bound_basis: MechanismFactorizedStarterBoundBasis,
}

impl MechanismFactorizedSupportObservationSummary {
    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn slice_id(self) -> MechanismSupportSliceId {
        self.slice_id
    }

    pub(crate) const fn root(self) -> MechanismFactorizedSupportObservationSummaryRoot {
        self.root
    }

    pub(crate) const fn frontier_root(self) -> MechanismSupportFrontierRoot {
        self.frontier_root
    }

    pub(crate) const fn imported_prefix_root(self) -> [u8; 32] {
        self.imported_prefix_root
    }

    pub(crate) const fn structural_root(self) -> Option<StructuralQuotientClosureRoot> {
        self.structural_root
    }

    pub(crate) const fn support_root(self) -> Option<MechanismSupportClosureRoot> {
        self.support_root
    }

    pub(crate) const fn projection_plan_id(self) -> Option<MechanismStarterProjectionPlanId> {
        self.projection_plan_id
    }

    pub(crate) const fn target_frontier_is_open(self) -> bool {
        self.target_frontier_open
    }

    pub(crate) const fn support_expression_bounds(self) -> MechanismSupportExpressionBounds {
        self.support_expression_bounds
    }

    pub(crate) const fn inner_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_inner_root()
    }

    pub(crate) const fn outer_fiber_expr_root(self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_outer_root()
    }

    pub(crate) fn fiber_expr_bounds_are_equal(self) -> bool {
        self.support_expression_bounds.case_bounds_are_equal()
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

    pub(crate) const fn residual_summary(self) -> MechanismSupportResidualSummary {
        self.residual
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

/// Reproducible decomposition of one V4 frontier. The imported-prefix root is
/// invariant when only an optional upstream seal arrives, which lets the
/// journal distinguish legitimate monotone enrichment at the same cursor from
/// arbitrary rewriting of already-checkpointed derived support state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportFrontierSummary {
    root: MechanismSupportFrontierRoot,
    imported_prefix_root: [u8; 32],
    cursor: MechanismSupportCheckpointCursor,
    target_discovery_revision: MechanismTargetDiscoveryRevision,
    terminal_discovery_revision: MechanismTerminalDiscoveryRevision,
    structural_assignment_revision: StructuralCatalogRevision,
    target_seal_id: Option<MechanismTargetSealId>,
    incidence_closure_root: Option<MechanismIncidenceRoot>,
    structural_closure_root: Option<StructuralQuotientClosureRoot>,
}

impl MechanismSupportFrontierSummary {
    pub(crate) const fn root(self) -> MechanismSupportFrontierRoot {
        self.root
    }

    pub(crate) const fn imported_prefix_root(self) -> [u8; 32] {
        self.imported_prefix_root
    }

    pub(crate) const fn cursor(self) -> MechanismSupportCheckpointCursor {
        self.cursor
    }

    pub(crate) const fn target_discovery_revision(self) -> MechanismTargetDiscoveryRevision {
        self.target_discovery_revision
    }

    pub(crate) const fn terminal_discovery_revision(self) -> MechanismTerminalDiscoveryRevision {
        self.terminal_discovery_revision
    }

    pub(crate) const fn structural_assignment_revision(self) -> StructuralCatalogRevision {
        self.structural_assignment_revision
    }

    pub(crate) const fn target_seal_id(self) -> Option<MechanismTargetSealId> {
        self.target_seal_id
    }

    pub(crate) const fn incidence_closure_root(self) -> Option<MechanismIncidenceRoot> {
        self.incidence_closure_root
    }

    pub(crate) const fn structural_closure_root(self) -> Option<StructuralQuotientClosureRoot> {
        self.structural_closure_root
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
    authenticated_starters: AuthenticatedTreapMap,
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
            authenticated_starters: AuthenticatedTreapMap::new(SUBJECT_STARTER_SET_INDEX_V1),
        }
    }
}

/// Exact inner starter/successor fiber plus the one shared unresolved frontier
/// which may still add support for the subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportView {
    key: MechanismSupportKey,
    root: MechanismSupportViewRoot,
    support_expression_bounds: MechanismSupportExpressionBounds,
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

    pub(crate) const fn support_expression_bounds(&self) -> MechanismSupportExpressionBounds {
        self.support_expression_bounds
    }

    pub(crate) const fn inner_fiber_expr_root(&self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_inner_root()
    }

    pub(crate) const fn outer_fiber_expr_root(&self) -> MechanismSupportFiberExprRoot {
        self.support_expression_bounds.case_outer_root()
    }

    pub(crate) fn fiber_expr_bounds_are_equal(&self) -> bool {
        self.support_expression_bounds.case_bounds_are_equal()
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
    structural_assignment_cursor: usize,
    structural_prefix_revision: StructuralCatalogRevision,
    signature_index: AuthenticatedTreapMap,
    case_index: AuthenticatedTreapMap,
    correlated_starter_index: AuthenticatedTreapMap,
    starter_set_index: AuthenticatedTreapMap,
    successor_fibers: BTreeMap<SourceKey, AuthenticatedTreapMap>,
}

impl SubjectProjectionCache {
    fn new(
        structural_assignment_cursor: usize,
        structural_prefix_revision: StructuralCatalogRevision,
    ) -> Self {
        Self {
            structural_assignment_cursor,
            structural_prefix_revision,
            signature_index: AuthenticatedTreapMap::new(SUBJECT_SIGNATURE_INDEX_V1),
            case_index: AuthenticatedTreapMap::new(SUBJECT_CASE_INDEX_V1),
            correlated_starter_index: AuthenticatedTreapMap::new(
                SUBJECT_CORRELATED_STARTER_INDEX_V2,
            ),
            starter_set_index: AuthenticatedTreapMap::new(SUBJECT_STARTER_SET_INDEX_V1),
            successor_fibers: BTreeMap::new(),
        }
    }

    fn case_count(&self) -> u128 {
        self.case_index.total_weight()
    }

    fn starter_count(&self) -> u128 {
        self.starter_set_index.total_weight()
    }

    fn is_for_structural_prefix(
        &self,
        structural_assignment_cursor: usize,
        structural_prefix_revision: StructuralCatalogRevision,
    ) -> bool {
        self.structural_assignment_cursor == structural_assignment_cursor
            && self.structural_prefix_revision == structural_prefix_revision
    }

    fn advance_structural_prefix(
        &mut self,
        structural_assignment_cursor: usize,
        structural_prefix_revision: StructuralCatalogRevision,
    ) {
        self.structural_assignment_cursor = structural_assignment_cursor;
        self.structural_prefix_revision = structural_prefix_revision;
    }
}

/// V6 view roots distinguish resumable operational prefixes from final
/// semantic authority. Open roots bind the exact imported discovery prefix;
/// closed roots return to canonical, discovery-order-independent structural
/// commitments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MechanismSupportViewStructuralAuthority {
    OpenPrefix {
        assignment_cursor: usize,
        prefix_revision: StructuralCatalogRevision,
    },
    Closed {
        structural_root: StructuralQuotientClosureRoot,
        assignment_root: [u8; 32],
        assignment_count: usize,
    },
}

/// One independently authenticated component of the shared possible-support
/// residual. `member_count` names cases for the pending/unavailable lanes and
/// raw signatures for the unassigned lane; `case_count` always names the
/// concrete cases represented by that component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportResidualComponentRoot([u8; 32]);

impl MechanismSupportResidualComponentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportResidualComponentSummary {
    root: MechanismSupportResidualComponentRoot,
    member_count: u128,
    case_count: u128,
}

impl MechanismSupportResidualComponentSummary {
    pub(crate) const fn root(self) -> MechanismSupportResidualComponentRoot {
        self.root
    }

    pub(crate) const fn member_count(self) -> u128 {
        self.member_count
    }

    pub(crate) const fn case_count(self) -> u128 {
        self.case_count
    }
}

/// Typed decomposition of the shared residual. Keeping the three components
/// visible prevents a consumer from treating replay unavailability as an
/// unassigned successful signature, or either one as merely pending work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportResidualSummary {
    root: MechanismSupportResidualRoot,
    case_count: u128,
    pending_cases: MechanismSupportResidualComponentSummary,
    unavailable_cases: MechanismSupportResidualComponentSummary,
    unassigned_signatures: MechanismSupportResidualComponentSummary,
}

impl MechanismSupportResidualSummary {
    pub(crate) const fn root(self) -> MechanismSupportResidualRoot {
        self.root
    }

    pub(crate) const fn case_count(self) -> u128 {
        self.case_count
    }

    pub(crate) const fn pending_cases(self) -> MechanismSupportResidualComponentSummary {
        self.pending_cases
    }

    pub(crate) const fn unavailable_cases(self) -> MechanismSupportResidualComponentSummary {
        self.unavailable_cases
    }

    pub(crate) const fn unassigned_signatures(self) -> MechanismSupportResidualComponentSummary {
        self.unassigned_signatures
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignatureFiberSummary {
    root: [u8; 32],
    starter_set_root: [u8; 32],
    case_count: u128,
    starter_count: u128,
}

/// Authenticated root of the append-only automatic whole-mechanism registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismAutomaticObservationRegistryRoot([u8; 32]);

impl MechanismAutomaticObservationRegistryRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Semantic imported-prefix summary for all automatically observed mechanism
/// slices. `indexed_assignment_count` is the exact global structural prefix
/// consumed by the per-mechanism accumulators; every assignment contributes
/// to exactly one registry value's authenticated weight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismAutomaticObservationRegistrySummary {
    root: MechanismAutomaticObservationRegistryRoot,
    slice_count: u128,
    indexed_assignment_count: u128,
}

impl MechanismAutomaticObservationRegistrySummary {
    pub(crate) const fn root(self) -> MechanismAutomaticObservationRegistryRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }

    pub(crate) const fn indexed_assignment_count(self) -> u128 {
        self.indexed_assignment_count
    }
}

/// Authenticated operational registry of explicitly requested node/edge
/// slices. Unlike the automatic whole-mechanism registry, this root is not
/// semantic support-frontier evidence: adding a reader after closure must not
/// rename the already proved support result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismExplicitObservationRegistryRoot([u8; 32]);

impl MechanismExplicitObservationRegistryRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismExplicitObservationRegistrySummary {
    root: MechanismExplicitObservationRegistryRoot,
    slice_count: u128,
    ready_slice_count: u128,
}

impl MechanismExplicitObservationRegistrySummary {
    pub(crate) const fn root(self) -> MechanismExplicitObservationRegistryRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }

    pub(crate) const fn ready_slice_count(self) -> u128 {
        self.ready_slice_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismPendingExplicitObservationBackfillRoot([u8; 32]);

impl MechanismPendingExplicitObservationBackfillRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismPendingExplicitObservationBackfillSummary {
    root: MechanismPendingExplicitObservationBackfillRoot,
    slice_count: u128,
}

impl MechanismPendingExplicitObservationBackfillSummary {
    pub(crate) const fn root(self) -> MechanismPendingExplicitObservationBackfillRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismDirtyExplicitObservationRoot([u8; 32]);

impl MechanismDirtyExplicitObservationRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismDirtyExplicitObservationSummary {
    root: MechanismDirtyExplicitObservationRoot,
    slice_count: u128,
}

impl MechanismDirtyExplicitObservationSummary {
    pub(crate) const fn root(self) -> MechanismDirtyExplicitObservationRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismExplicitObservationRegistrationDisposition {
    Registered,
    AlreadyRegistered,
    AutomaticWholeMechanism,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismExplicitObservationRegistrationPhase {
    Open,
    Sealed {
        support_root: MechanismSupportClosureRoot,
    },
}

impl MechanismExplicitObservationRegistrationPhase {
    pub(crate) const fn support_root(self) -> Option<MechanismSupportClosureRoot> {
        match self {
            Self::Open => None,
            Self::Sealed { support_root } => Some(support_root),
        }
    }

    pub(crate) const fn is_sealed(self) -> bool {
        matches!(self, Self::Sealed { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismUnsealedExplicitObservationRoot([u8; 32]);

impl MechanismUnsealedExplicitObservationRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismUnsealedExplicitObservationSummary {
    root: MechanismUnsealedExplicitObservationRoot,
    slice_count: u128,
}

impl MechanismUnsealedExplicitObservationSummary {
    pub(crate) const fn root(self) -> MechanismUnsealedExplicitObservationRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismExplicitObservationSchedulerSummary {
    registry: MechanismExplicitObservationRegistrySummary,
    pending_backfill: MechanismPendingExplicitObservationBackfillSummary,
    dirty: MechanismDirtyExplicitObservationSummary,
    unsealed: MechanismUnsealedExplicitObservationSummary,
}

impl MechanismExplicitObservationSchedulerSummary {
    /// Rebuilds the durable operational receipt decoded by the outer journal.
    /// The journal validates these counts against its event transition before
    /// accepting the receipt; the support builder remains the authority which
    /// can materialize and mutate the authenticated indexes themselves.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn restore_from_journal_codec(
        registry_root: [u8; 32],
        registered_slice_count: u128,
        ready_slice_count: u128,
        pending_backfill_root: [u8; 32],
        pending_backfill_slice_count: u128,
        dirty_root: [u8; 32],
        dirty_slice_count: u128,
        unsealed_root: [u8; 32],
        unsealed_slice_count: u128,
    ) -> Self {
        Self {
            registry: MechanismExplicitObservationRegistrySummary {
                root: MechanismExplicitObservationRegistryRoot(registry_root),
                slice_count: registered_slice_count,
                ready_slice_count,
            },
            pending_backfill: MechanismPendingExplicitObservationBackfillSummary {
                root: MechanismPendingExplicitObservationBackfillRoot(pending_backfill_root),
                slice_count: pending_backfill_slice_count,
            },
            dirty: MechanismDirtyExplicitObservationSummary {
                root: MechanismDirtyExplicitObservationRoot(dirty_root),
                slice_count: dirty_slice_count,
            },
            unsealed: MechanismUnsealedExplicitObservationSummary {
                root: MechanismUnsealedExplicitObservationRoot(unsealed_root),
                slice_count: unsealed_slice_count,
            },
        }
    }

    pub(crate) const fn registry(self) -> MechanismExplicitObservationRegistrySummary {
        self.registry
    }

    pub(crate) const fn pending_backfill(
        self,
    ) -> MechanismPendingExplicitObservationBackfillSummary {
        self.pending_backfill
    }

    pub(crate) const fn dirty(self) -> MechanismDirtyExplicitObservationSummary {
        self.dirty
    }

    pub(crate) const fn unsealed(self) -> MechanismUnsealedExplicitObservationSummary {
        self.unsealed
    }

    /// Whether every registered explicit observation has finished any
    /// backfill, consumed its last dirty prefix and received its terminal
    /// sealed observation. The registry itself intentionally remains durable
    /// after settlement.
    pub(crate) const fn is_fully_settled(self) -> bool {
        self.pending_backfill.slice_count() == 0
            && self.dirty.slice_count() == 0
            && self.unsealed.slice_count() == 0
    }
}

/// Authenticated root of the operational set of mechanism slices whose latest
/// semantic evidence has not yet been observed by the outer journal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismDirtyAutomaticObservationRoot([u8; 32]);

impl MechanismDirtyAutomaticObservationRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismDirtyAutomaticObservationSummary {
    root: MechanismDirtyAutomaticObservationRoot,
    slice_count: u128,
}

impl MechanismDirtyAutomaticObservationSummary {
    pub(crate) const fn root(self) -> MechanismDirtyAutomaticObservationRoot {
        self.root
    }

    pub(crate) const fn slice_count(self) -> u128 {
        self.slice_count
    }
}

/// Copyable scheduler checkpoint. The registry half is semantic and also
/// participates in the support frontier; the dirty half is intentionally only
/// outer-journal scheduling state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismAutomaticObservationSchedulerSummary {
    registry: MechanismAutomaticObservationRegistrySummary,
    dirty: MechanismDirtyAutomaticObservationSummary,
}

impl MechanismAutomaticObservationSchedulerSummary {
    pub(crate) const fn registry(self) -> MechanismAutomaticObservationRegistrySummary {
        self.registry
    }

    pub(crate) const fn dirty(self) -> MechanismDirtyAutomaticObservationSummary {
        self.dirty
    }
}

/// Opaque two-phase acknowledgement. Preparing clones and removes from the
/// authenticated dirty treap; committing only installs that already-validated
/// root and performs the matching infallible `BTreeSet` removal.
#[derive(Debug)]
pub(crate) struct MechanismAutomaticObservationAck {
    slice: MechanismSupportSlice,
    prior_dirty_root: [u8; 32],
    prior_dirty_count: u128,
    next_dirty_index: AuthenticatedTreapMap,
}

impl MechanismAutomaticObservationAck {
    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn prior_dirty_summary(&self) -> MechanismDirtyAutomaticObservationSummary {
        MechanismDirtyAutomaticObservationSummary {
            root: MechanismDirtyAutomaticObservationRoot(self.prior_dirty_root),
            slice_count: self.prior_dirty_count,
        }
    }

    pub(crate) fn next_dirty_summary(&self) -> MechanismDirtyAutomaticObservationSummary {
        MechanismDirtyAutomaticObservationSummary {
            root: MechanismDirtyAutomaticObservationRoot(self.next_dirty_index.root_hash()),
            slice_count: self.next_dirty_index.entry_count(),
        }
    }
}

/// Imported-prefix index for one automatically scheduled whole-mechanism
/// slice. Registry updates touch only the mechanism named by the incoming
/// structural assignment, so checkpoint observation never rescans either the
/// growing assignment catalog or unrelated mechanisms.
#[derive(Clone, Debug)]
struct AutomaticSupportObservationIndex {
    slice: MechanismSupportSlice,
    contributing_signature_count: u128,
    inspected_signatures: Vec<MechanismSignatureId>,
}

impl AutomaticSupportObservationIndex {
    fn new(scope: MechanismRequestScope, mechanism_id: StructuralMechanismId) -> Self {
        Self {
            slice: MechanismSupportSlice::total(MechanismSupportKey::new(
                scope,
                MechanismSupportSubject::Mechanism(mechanism_id),
            )),
            contributing_signature_count: 0,
            inspected_signatures: Vec::new(),
        }
    }

    fn observe_assignment(
        &mut self,
        signature_id: MechanismSignatureId,
        assignment: &StructuralSignatureAssignment,
    ) -> Result<(), MechanismSupportError> {
        if !assignment_supports_slice(assignment, self.slice) {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        self.contributing_signature_count = self
            .contributing_signature_count
            .checked_add(1)
            .ok_or(MechanismSupportError::CountOverflow)?;
        if self.inspected_signatures.len() < AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT {
            self.inspected_signatures.push(signature_id);
        }
        Ok(())
    }
}

/// Imported-assignment index for one explicitly requested node/edge slice.
/// The fixed registration target prevents live discovery from making a late
/// backfill recede forever. Structural import is held at that target until the
/// bounded backfill completes; later assignments use the watcher indexes.
#[derive(Clone, Debug)]
struct ExplicitSupportObservationIndex {
    slice: MechanismSupportSlice,
    registration_phase: MechanismExplicitObservationRegistrationPhase,
    registration_structural_cursor: usize,
    registration_structural_revision: StructuralCatalogRevision,
    backfill_cursor: usize,
    contributing_signature_count: u128,
    inspected_signatures: Vec<MechanismSignatureId>,
}

impl ExplicitSupportObservationIndex {
    fn new(
        slice: MechanismSupportSlice,
        registration_phase: MechanismExplicitObservationRegistrationPhase,
        registration_structural_cursor: usize,
        registration_structural_revision: StructuralCatalogRevision,
    ) -> Self {
        Self {
            slice,
            registration_phase,
            registration_structural_cursor,
            registration_structural_revision,
            backfill_cursor: 0,
            contributing_signature_count: 0,
            inspected_signatures: Vec::new(),
        }
    }

    fn is_ready(&self) -> bool {
        self.backfill_cursor == self.registration_structural_cursor
    }

    fn observe_assignment(
        &mut self,
        signature_id: MechanismSignatureId,
        assignment: &StructuralSignatureAssignment,
    ) -> Result<(), MechanismSupportError> {
        if !assignment_supports_slice(assignment, self.slice) {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        self.contributing_signature_count = self
            .contributing_signature_count
            .checked_add(1)
            .ok_or(MechanismSupportError::CountOverflow)?;
        if self.inspected_signatures.len() < AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT {
            self.inspected_signatures.push(signature_id);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ExplicitObservationWatcherKey {
    subject: MechanismSupportSubject,
    enclosing_mechanism: Option<StructuralMechanismId>,
}

impl ExplicitObservationWatcherKey {
    const fn from_slice(slice: MechanismSupportSlice) -> Self {
        Self {
            subject: slice.subject(),
            enclosing_mechanism: slice.enclosing_mechanism(),
        }
    }
}

/// Pure, fully preflighted explicit-demand registration. `commit` is a no-op
/// for an existing explicit slice and for a whole-mechanism demand which is
/// already served by the automatic registry.
#[derive(Debug)]
pub(crate) struct MechanismExplicitObservationRegistration {
    slice: MechanismSupportSlice,
    disposition: MechanismExplicitObservationRegistrationDisposition,
    registration_phase: MechanismExplicitObservationRegistrationPhase,
    registration_structural_cursor: u128,
    registration_structural_revision: StructuralCatalogRevision,
    prior_registry: MechanismExplicitObservationRegistrySummary,
    prior_pending: MechanismPendingExplicitObservationBackfillSummary,
    prior_dirty: MechanismDirtyExplicitObservationSummary,
    prior_unsealed: MechanismUnsealedExplicitObservationSummary,
    next_registry_index: Option<AuthenticatedTreapMap>,
    next_pending_index: Option<AuthenticatedTreapMap>,
    next_dirty_index: Option<AuthenticatedTreapMap>,
    next_unsealed_index: Option<AuthenticatedTreapMap>,
    entry: Option<ExplicitSupportObservationIndex>,
    watcher_key: Option<ExplicitObservationWatcherKey>,
}

impl MechanismExplicitObservationRegistration {
    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn disposition(&self) -> MechanismExplicitObservationRegistrationDisposition {
        self.disposition
    }

    pub(crate) const fn registration_phase(&self) -> MechanismExplicitObservationRegistrationPhase {
        self.registration_phase
    }

    pub(crate) const fn registration_structural_cursor(&self) -> u128 {
        self.registration_structural_cursor
    }

    pub(crate) const fn registration_structural_revision(&self) -> StructuralCatalogRevision {
        self.registration_structural_revision
    }

    pub(crate) const fn prior_scheduler_summary(
        &self,
    ) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.prior_registry,
            pending_backfill: self.prior_pending,
            dirty: self.prior_dirty,
            unsealed: self.prior_unsealed,
        }
    }

    pub(crate) fn next_registry_summary(&self) -> MechanismExplicitObservationRegistrySummary {
        match &self.next_registry_index {
            Some(index) => explicit_observation_registry_summary(
                index,
                self.next_pending_summary().slice_count(),
            ),
            None => self.prior_registry,
        }
    }

    pub(crate) fn next_pending_summary(
        &self,
    ) -> MechanismPendingExplicitObservationBackfillSummary {
        match &self.next_pending_index {
            Some(index) => pending_explicit_observation_backfill_summary(index),
            None => self.prior_pending,
        }
    }

    pub(crate) fn next_dirty_summary(&self) -> MechanismDirtyExplicitObservationSummary {
        match &self.next_dirty_index {
            Some(index) => dirty_explicit_observation_summary(index),
            None => self.prior_dirty,
        }
    }

    pub(crate) fn next_unsealed_summary(&self) -> MechanismUnsealedExplicitObservationSummary {
        match &self.next_unsealed_index {
            Some(index) => unsealed_explicit_observation_summary(index),
            None => self.prior_unsealed,
        }
    }

    pub(crate) fn next_scheduler_summary(&self) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.next_registry_summary(),
            pending_backfill: self.next_pending_summary(),
            dirty: self.next_dirty_summary(),
            unsealed: self.next_unsealed_summary(),
        }
    }
}

/// One deterministic bounded backfill page for the canonical minimum pending
/// explicit slice. Preparing performs every recoverable check and builds the
/// authenticated successors; committing only installs the prepared state.
#[derive(Debug)]
pub(crate) struct MechanismExplicitObservationBackfill {
    slice: MechanismSupportSlice,
    registration_phase: MechanismExplicitObservationRegistrationPhase,
    registration_structural_cursor: u128,
    registration_structural_revision: StructuralCatalogRevision,
    from_structural_cursor: u128,
    through_structural_cursor: u128,
    prior_registry: MechanismExplicitObservationRegistrySummary,
    prior_pending: MechanismPendingExplicitObservationBackfillSummary,
    prior_dirty: MechanismDirtyExplicitObservationSummary,
    prior_unsealed: MechanismUnsealedExplicitObservationSummary,
    next_registry_index: AuthenticatedTreapMap,
    next_pending_index: AuthenticatedTreapMap,
    next_dirty_index: AuthenticatedTreapMap,
    next_entry: ExplicitSupportObservationIndex,
    matched_signatures: Box<[MechanismSignatureId]>,
}

impl MechanismExplicitObservationBackfill {
    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn registration_structural_cursor(&self) -> u128 {
        self.registration_structural_cursor
    }

    pub(crate) const fn registration_phase(&self) -> MechanismExplicitObservationRegistrationPhase {
        self.registration_phase
    }

    pub(crate) const fn registration_structural_revision(&self) -> StructuralCatalogRevision {
        self.registration_structural_revision
    }

    pub(crate) const fn from_structural_cursor(&self) -> u128 {
        self.from_structural_cursor
    }

    pub(crate) const fn through_structural_cursor(&self) -> u128 {
        self.through_structural_cursor
    }

    pub(crate) const fn prior_scheduler_summary(
        &self,
    ) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.prior_registry,
            pending_backfill: self.prior_pending,
            dirty: self.prior_dirty,
            unsealed: self.prior_unsealed,
        }
    }

    pub(crate) fn completed(&self) -> bool {
        self.next_entry.is_ready()
    }

    pub(crate) fn next_registry_summary(&self) -> MechanismExplicitObservationRegistrySummary {
        explicit_observation_registry_summary(
            &self.next_registry_index,
            self.next_pending_index.entry_count(),
        )
    }

    pub(crate) fn next_pending_summary(
        &self,
    ) -> MechanismPendingExplicitObservationBackfillSummary {
        pending_explicit_observation_backfill_summary(&self.next_pending_index)
    }

    pub(crate) fn next_dirty_summary(&self) -> MechanismDirtyExplicitObservationSummary {
        dirty_explicit_observation_summary(&self.next_dirty_index)
    }

    pub(crate) const fn next_unsealed_summary(
        &self,
    ) -> MechanismUnsealedExplicitObservationSummary {
        self.prior_unsealed
    }

    pub(crate) fn next_scheduler_summary(&self) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.next_registry_summary(),
            pending_backfill: self.next_pending_summary(),
            dirty: self.next_dirty_summary(),
            unsealed: self.next_unsealed_summary(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MechanismExplicitObservationAck {
    slice: MechanismSupportSlice,
    prior_scheduler: MechanismExplicitObservationSchedulerSummary,
    next_dirty_index: AuthenticatedTreapMap,
}

impl MechanismExplicitObservationAck {
    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn prior_dirty_summary(&self) -> MechanismDirtyExplicitObservationSummary {
        self.prior_scheduler.dirty()
    }

    pub(crate) const fn prior_scheduler_summary(
        &self,
    ) -> MechanismExplicitObservationSchedulerSummary {
        self.prior_scheduler
    }

    pub(crate) fn next_dirty_summary(&self) -> MechanismDirtyExplicitObservationSummary {
        dirty_explicit_observation_summary(&self.next_dirty_index)
    }

    pub(crate) fn next_scheduler_summary(&self) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.prior_scheduler.registry(),
            pending_backfill: self.prior_scheduler.pending_backfill(),
            dirty: self.next_dirty_summary(),
            unsealed: self.prior_scheduler.unsealed(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MechanismExplicitObservationSealAck {
    slice: MechanismSupportSlice,
    prior_scheduler: MechanismExplicitObservationSchedulerSummary,
    retired_dirty: bool,
    next_dirty_index: AuthenticatedTreapMap,
    next_unsealed_index: AuthenticatedTreapMap,
}

impl MechanismExplicitObservationSealAck {
    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn prior_unsealed_summary(
        &self,
    ) -> MechanismUnsealedExplicitObservationSummary {
        self.prior_scheduler.unsealed()
    }

    pub(crate) const fn prior_scheduler_summary(
        &self,
    ) -> MechanismExplicitObservationSchedulerSummary {
        self.prior_scheduler
    }

    pub(crate) const fn retired_dirty_observation(&self) -> bool {
        self.retired_dirty
    }

    pub(crate) fn next_dirty_summary(&self) -> MechanismDirtyExplicitObservationSummary {
        dirty_explicit_observation_summary(&self.next_dirty_index)
    }

    pub(crate) fn next_unsealed_summary(&self) -> MechanismUnsealedExplicitObservationSummary {
        unsealed_explicit_observation_summary(&self.next_unsealed_index)
    }

    pub(crate) fn next_scheduler_summary(&self) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.prior_scheduler.registry(),
            pending_backfill: self.prior_scheduler.pending_backfill(),
            dirty: self.next_dirty_summary(),
            unsealed: self.next_unsealed_summary(),
        }
    }
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
    target_starter_set_index: AuthenticatedTreapMap,
    target_discovery_cursor: usize,
    target_discovery_revision: Option<MechanismTargetDiscoveryRevision>,
    pending_cases: AuthenticatedTreapMap,
    unavailable_cases: AuthenticatedTreapMap,
    terminal_fact_index: AuthenticatedTreapMap,
    signature_fibers: BTreeMap<MechanismSignatureId, SignatureCaseFiber>,
    signature_fiber_index: AuthenticatedTreapMap,
    signature_starter_set_index: AuthenticatedTreapMap,
    unassigned_signature_index: AuthenticatedTreapMap,
    terminal_discovery_cursor: usize,
    terminal_discovery_revision: Option<MechanismTerminalDiscoveryRevision>,
    structural_assignment_cursor: usize,
    structural_assignment_revision: Option<StructuralCatalogRevision>,
    imported_structural_assignments: BTreeSet<MechanismSignatureId>,
    automatic_observation_registry:
        BTreeMap<StructuralMechanismId, AutomaticSupportObservationIndex>,
    automatic_observation_registry_index: AuthenticatedTreapMap,
    automatic_observation_indexed_assignment_count: u128,
    dirty_automatic_observations: BTreeSet<MechanismSupportSlice>,
    dirty_automatic_observation_index: AuthenticatedTreapMap,
    explicit_observation_registry: BTreeMap<MechanismSupportSlice, ExplicitSupportObservationIndex>,
    explicit_observation_registry_index: AuthenticatedTreapMap,
    pending_explicit_observation_backfills: BTreeSet<MechanismSupportSlice>,
    pending_explicit_observation_backfill_index: AuthenticatedTreapMap,
    dirty_explicit_observations: BTreeSet<MechanismSupportSlice>,
    dirty_explicit_observation_index: AuthenticatedTreapMap,
    unsealed_explicit_observations: BTreeSet<MechanismSupportSlice>,
    unsealed_explicit_observation_index: AuthenticatedTreapMap,
    explicit_observation_watchers: BTreeMap<ExplicitObservationWatcherKey, MechanismSupportSlice>,
    signature_explicit_observation_watchers:
        BTreeMap<MechanismSignatureId, BTreeSet<MechanismSupportSlice>>,
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
            target_starter_set_index: AuthenticatedTreapMap::new(TARGET_STARTER_SET_INDEX_V1),
            target_discovery_cursor: 0,
            target_discovery_revision: None,
            pending_cases: AuthenticatedTreapMap::new(PENDING_CASE_INDEX_V1),
            unavailable_cases: AuthenticatedTreapMap::new(UNAVAILABLE_CASE_INDEX_V1),
            terminal_fact_index: AuthenticatedTreapMap::new(TERMINAL_FACT_INDEX_V1),
            signature_fibers: BTreeMap::new(),
            signature_fiber_index: AuthenticatedTreapMap::new(SIGNATURE_FIBER_INDEX_V1),
            signature_starter_set_index: AuthenticatedTreapMap::new(SIGNATURE_STARTER_SET_INDEX_V1),
            unassigned_signature_index: AuthenticatedTreapMap::new(UNASSIGNED_SIGNATURE_INDEX_V1),
            terminal_discovery_cursor: 0,
            terminal_discovery_revision: None,
            structural_assignment_cursor: 0,
            structural_assignment_revision: None,
            imported_structural_assignments: BTreeSet::new(),
            automatic_observation_registry: BTreeMap::new(),
            automatic_observation_registry_index: AuthenticatedTreapMap::new(
                AUTOMATIC_OBSERVATION_REGISTRY_INDEX_V1,
            ),
            automatic_observation_indexed_assignment_count: 0,
            dirty_automatic_observations: BTreeSet::new(),
            dirty_automatic_observation_index: AuthenticatedTreapMap::new(
                DIRTY_AUTOMATIC_OBSERVATION_INDEX_V1,
            ),
            explicit_observation_registry: BTreeMap::new(),
            explicit_observation_registry_index: AuthenticatedTreapMap::new(
                EXPLICIT_OBSERVATION_REGISTRY_INDEX_V1,
            ),
            pending_explicit_observation_backfills: BTreeSet::new(),
            pending_explicit_observation_backfill_index: AuthenticatedTreapMap::new(
                PENDING_EXPLICIT_OBSERVATION_BACKFILL_INDEX_V1,
            ),
            dirty_explicit_observations: BTreeSet::new(),
            dirty_explicit_observation_index: AuthenticatedTreapMap::new(
                DIRTY_EXPLICIT_OBSERVATION_INDEX_V1,
            ),
            unsealed_explicit_observations: BTreeSet::new(),
            unsealed_explicit_observation_index: AuthenticatedTreapMap::new(
                UNSEALED_EXPLICIT_OBSERVATION_INDEX_V1,
            ),
            explicit_observation_watchers: BTreeMap::new(),
            signature_explicit_observation_watchers: BTreeMap::new(),
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

    pub(crate) fn automatic_observation_slice_count(&self) -> u128 {
        self.automatic_observation_registry.len() as u128
    }

    pub(crate) fn automatic_observation_registry_summary(
        &self,
    ) -> MechanismAutomaticObservationRegistrySummary {
        let slice_count = self.automatic_observation_registry_index.entry_count();
        debug_assert_eq!(
            slice_count,
            self.automatic_observation_registry.len() as u128,
            "automatic observation registry representations remain aligned"
        );
        debug_assert_eq!(
            self.automatic_observation_registry_index.total_weight(),
            self.automatic_observation_indexed_assignment_count,
            "automatic observation registry weight is the indexed structural prefix"
        );
        MechanismAutomaticObservationRegistrySummary {
            root: MechanismAutomaticObservationRegistryRoot(
                self.automatic_observation_registry_index.root_hash(),
            ),
            slice_count,
            indexed_assignment_count: self.automatic_observation_indexed_assignment_count,
        }
    }

    pub(crate) fn dirty_automatic_observation_summary(
        &self,
    ) -> MechanismDirtyAutomaticObservationSummary {
        let slice_count = self.dirty_automatic_observation_index.entry_count();
        debug_assert_eq!(
            slice_count,
            self.dirty_automatic_observations.len() as u128,
            "dirty automatic observation representations remain aligned"
        );
        debug_assert_eq!(
            self.dirty_automatic_observation_index.total_weight(),
            slice_count,
            "every dirty automatic observation has unit weight"
        );
        MechanismDirtyAutomaticObservationSummary {
            root: MechanismDirtyAutomaticObservationRoot(
                self.dirty_automatic_observation_index.root_hash(),
            ),
            slice_count,
        }
    }

    pub(crate) fn automatic_observation_scheduler_summary(
        &self,
    ) -> MechanismAutomaticObservationSchedulerSummary {
        MechanismAutomaticObservationSchedulerSummary {
            registry: self.automatic_observation_registry_summary(),
            dirty: self.dirty_automatic_observation_summary(),
        }
    }

    pub(crate) fn explicit_observation_registry_summary(
        &self,
    ) -> MechanismExplicitObservationRegistrySummary {
        debug_assert_eq!(
            self.explicit_observation_registry.len() as u128,
            self.explicit_observation_registry_index.entry_count()
        );
        explicit_observation_registry_summary(
            &self.explicit_observation_registry_index,
            self.pending_explicit_observation_backfill_index
                .entry_count(),
        )
    }

    pub(crate) fn pending_explicit_observation_backfill_summary(
        &self,
    ) -> MechanismPendingExplicitObservationBackfillSummary {
        debug_assert_eq!(
            self.pending_explicit_observation_backfills.len() as u128,
            self.pending_explicit_observation_backfill_index
                .entry_count()
        );
        pending_explicit_observation_backfill_summary(
            &self.pending_explicit_observation_backfill_index,
        )
    }

    pub(crate) fn dirty_explicit_observation_summary(
        &self,
    ) -> MechanismDirtyExplicitObservationSummary {
        debug_assert_eq!(
            self.dirty_explicit_observations.len() as u128,
            self.dirty_explicit_observation_index.entry_count()
        );
        dirty_explicit_observation_summary(&self.dirty_explicit_observation_index)
    }

    pub(crate) fn unsealed_explicit_observation_summary(
        &self,
    ) -> MechanismUnsealedExplicitObservationSummary {
        debug_assert_eq!(
            self.unsealed_explicit_observations.len() as u128,
            self.unsealed_explicit_observation_index.entry_count()
        );
        unsealed_explicit_observation_summary(&self.unsealed_explicit_observation_index)
    }

    pub(crate) fn explicit_observation_scheduler_summary(
        &self,
    ) -> MechanismExplicitObservationSchedulerSummary {
        MechanismExplicitObservationSchedulerSummary {
            registry: self.explicit_observation_registry_summary(),
            pending_backfill: self.pending_explicit_observation_backfill_summary(),
            dirty: self.dirty_explicit_observation_summary(),
            unsealed: self.unsealed_explicit_observation_summary(),
        }
    }

    pub(crate) fn explicit_observation_slice_count(&self) -> u128 {
        self.explicit_observation_registry.len() as u128
    }

    pub(crate) fn ready_explicit_observation_slice_count(&self) -> u128 {
        self.explicit_observation_registry
            .len()
            .saturating_sub(self.pending_explicit_observation_backfills.len()) as u128
    }

    pub(crate) fn explicit_observation_contains(&self, slice: MechanismSupportSlice) -> bool {
        self.explicit_observation_registry.contains_key(&slice)
    }

    pub(crate) fn ready_explicit_observation_contains(&self, slice: MechanismSupportSlice) -> bool {
        self.explicit_observation_registry
            .get(&slice)
            .is_some_and(ExplicitSupportObservationIndex::is_ready)
    }

    pub(crate) fn explicit_observation_registration_phase(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<MechanismExplicitObservationRegistrationPhase, MechanismSupportError> {
        Ok(self
            .explicit_observation_index_for_slice(slice, false)?
            .registration_phase)
    }

    pub(crate) fn next_pending_explicit_observation_slice(&self) -> Option<MechanismSupportSlice> {
        self.pending_explicit_observation_backfills.first().copied()
    }

    pub(crate) fn next_dirty_explicit_observation_slice(&self) -> Option<MechanismSupportSlice> {
        self.dirty_explicit_observations.first().copied()
    }

    pub(crate) fn next_unsealed_explicit_observation_slice(&self) -> Option<MechanismSupportSlice> {
        self.unsealed_explicit_observations.first().copied()
    }

    pub(crate) fn next_explicit_observation_slice_after(
        &self,
        after: Option<MechanismSupportSlice>,
    ) -> Result<Option<MechanismSupportSlice>, MechanismSupportError> {
        if !self.pending_explicit_observation_backfills.is_empty() {
            return Err(MechanismSupportError::ExplicitObservationBackfillPending);
        }
        let next = match after {
            None => self.explicit_observation_registry.first_key_value(),
            Some(slice) => {
                self.explicit_observation_index_for_slice(slice, true)?;
                self.explicit_observation_registry
                    .range((Excluded(slice), Unbounded))
                    .next()
            }
        };
        Ok(next.map(|(slice, _)| *slice))
    }

    pub(crate) fn prepare_explicit_observation_demand_registration(
        &self,
        slice: MechanismSupportSlice,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismExplicitObservationRegistration, MechanismSupportError> {
        if structural.request_id() != self.scope.request_id()
            || slice.key().request_id() != self.scope.request_id()
            || slice.key().target() != self.scope.target()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        self.validate_structural_assignment_prefix(structural)?;
        self.validate_explicit_observation_indexes()?;
        let current_revision = structural
            .assignment_discovery_prefix_revision(self.structural_assignment_cursor)
            .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;
        let phase = self.closure.map_or(
            MechanismExplicitObservationRegistrationPhase::Open,
            |closure| MechanismExplicitObservationRegistrationPhase::Sealed {
                support_root: closure.root(),
            },
        );
        let prior_registry = self.explicit_observation_registry_summary();
        let prior_pending = self.pending_explicit_observation_backfill_summary();
        let prior_dirty = self.dirty_explicit_observation_summary();
        let prior_unsealed = self.unsealed_explicit_observation_summary();

        if matches!(slice.subject(), MechanismSupportSubject::Mechanism(_)) {
            validate_explicit_observation_slice(structural, slice)?;
            return Ok(MechanismExplicitObservationRegistration {
                slice,
                disposition:
                    MechanismExplicitObservationRegistrationDisposition::AutomaticWholeMechanism,
                registration_phase: phase,
                registration_structural_cursor: self.structural_assignment_cursor as u128,
                registration_structural_revision: current_revision,
                prior_registry,
                prior_pending,
                prior_dirty,
                prior_unsealed,
                next_registry_index: None,
                next_pending_index: None,
                next_dirty_index: None,
                next_unsealed_index: None,
                entry: None,
                watcher_key: None,
            });
        }
        validate_explicit_observation_slice(structural, slice)?;

        if let Some(existing) = self.explicit_observation_registry.get(&slice) {
            self.validate_explicit_observation_entry(existing)?;
            return Ok(MechanismExplicitObservationRegistration {
                slice,
                disposition: MechanismExplicitObservationRegistrationDisposition::AlreadyRegistered,
                registration_phase: existing.registration_phase,
                registration_structural_cursor: existing.registration_structural_cursor as u128,
                registration_structural_revision: existing.registration_structural_revision,
                prior_registry,
                prior_pending,
                prior_dirty,
                prior_unsealed,
                next_registry_index: None,
                next_pending_index: None,
                next_dirty_index: None,
                next_unsealed_index: None,
                entry: None,
                watcher_key: None,
            });
        }

        let watcher_key = ExplicitObservationWatcherKey::from_slice(slice);
        if self
            .explicit_observation_watchers
            .contains_key(&watcher_key)
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        let entry = ExplicitSupportObservationIndex::new(
            slice,
            phase,
            self.structural_assignment_cursor,
            current_revision,
        );
        let mut next_registry_index = self.explicit_observation_registry_index.clone();
        next_registry_index
            .insert(
                explicit_observation_key(slice),
                explicit_observation_registry_value(&entry),
            )
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("explicit observation registry")
            })?;
        let mut next_pending_index = self.pending_explicit_observation_backfill_index.clone();
        let mut next_dirty_index = self.dirty_explicit_observation_index.clone();
        if entry.is_ready() {
            next_dirty_index
                .insert(
                    explicit_observation_key(slice),
                    dirty_explicit_observation_value(slice),
                )
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
                })?;
        } else {
            next_pending_index
                .insert(
                    explicit_observation_key(slice),
                    pending_explicit_observation_backfill_value(slice),
                )
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex(
                        "pending explicit observation backfills",
                    )
                })?;
        }
        let mut next_unsealed_index = self.unsealed_explicit_observation_index.clone();
        next_unsealed_index
            .insert(
                explicit_observation_key(slice),
                unsealed_explicit_observation_value(slice),
            )
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("unsealed explicit observations")
            })?;
        let pending_delta = if entry.is_ready() { 0 } else { 1 };
        let dirty_delta = if entry.is_ready() { 1 } else { 0 };
        if next_registry_index.entry_count() != prior_registry.slice_count() + 1
            || next_pending_index.entry_count() != prior_pending.slice_count() + pending_delta
            || next_dirty_index.entry_count() != prior_dirty.slice_count() + dirty_delta
            || next_unsealed_index.entry_count() != prior_unsealed.slice_count() + 1
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        Ok(MechanismExplicitObservationRegistration {
            slice,
            disposition: MechanismExplicitObservationRegistrationDisposition::Registered,
            registration_phase: phase,
            registration_structural_cursor: self.structural_assignment_cursor as u128,
            registration_structural_revision: current_revision,
            prior_registry,
            prior_pending,
            prior_dirty,
            prior_unsealed,
            next_registry_index: Some(next_registry_index),
            next_pending_index: Some(next_pending_index),
            next_dirty_index: Some(next_dirty_index),
            next_unsealed_index: Some(next_unsealed_index),
            entry: Some(entry),
            watcher_key: Some(watcher_key),
        })
    }

    pub(crate) fn commit_explicit_observation_demand_registration(
        &mut self,
        mut prepared: MechanismExplicitObservationRegistration,
    ) {
        assert_eq!(
            self.explicit_observation_registry_summary(),
            prepared.prior_registry
        );
        assert_eq!(
            self.pending_explicit_observation_backfill_summary(),
            prepared.prior_pending
        );
        assert_eq!(
            self.dirty_explicit_observation_summary(),
            prepared.prior_dirty
        );
        assert_eq!(
            self.unsealed_explicit_observation_summary(),
            prepared.prior_unsealed
        );
        if prepared.disposition != MechanismExplicitObservationRegistrationDisposition::Registered {
            return;
        }
        let entry = prepared
            .entry
            .take()
            .expect("new explicit registration retains its prepared entry");
        let watcher_key = prepared
            .watcher_key
            .expect("new explicit registration retains its watcher key");
        self.explicit_observation_registry_index = prepared
            .next_registry_index
            .take()
            .expect("new explicit registration retains its registry successor");
        self.pending_explicit_observation_backfill_index = prepared
            .next_pending_index
            .take()
            .expect("new explicit registration retains its pending successor");
        self.dirty_explicit_observation_index = prepared
            .next_dirty_index
            .take()
            .expect("new explicit registration retains its dirty successor");
        self.unsealed_explicit_observation_index = prepared
            .next_unsealed_index
            .take()
            .expect("new explicit registration retains its unsealed successor");
        assert!(self
            .explicit_observation_registry
            .insert(prepared.slice, entry.clone())
            .is_none());
        assert!(self
            .explicit_observation_watchers
            .insert(watcher_key, prepared.slice)
            .is_none());
        if entry.is_ready() {
            assert!(self.dirty_explicit_observations.insert(prepared.slice));
        } else {
            assert!(self
                .pending_explicit_observation_backfills
                .insert(prepared.slice));
        }
        assert!(self.unsealed_explicit_observations.insert(prepared.slice));
        debug_assert!(self.validate_explicit_observation_indexes().is_ok());
    }

    pub(crate) fn register_explicit_observation_demand(
        &mut self,
        slice: MechanismSupportSlice,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<bool, MechanismSupportError> {
        let prepared = self.prepare_explicit_observation_demand_registration(slice, structural)?;
        let changed = prepared.disposition()
            == MechanismExplicitObservationRegistrationDisposition::Registered;
        self.commit_explicit_observation_demand_registration(prepared);
        Ok(changed)
    }

    pub(crate) fn prepare_next_explicit_observation_backfill(
        &self,
        structural: &StructuralMechanismCatalogBuilder,
        maximum_assignments: NonZeroU16,
    ) -> Result<Option<MechanismExplicitObservationBackfill>, MechanismSupportError> {
        if usize::from(maximum_assignments.get()) > EXPLICIT_OBSERVATION_BACKFILL_MAX_ASSIGNMENTS {
            return Err(MechanismSupportError::ExplicitObservationBackfillPageTooLarge);
        }
        if structural.request_id() != self.scope.request_id() {
            return Err(MechanismSupportError::RequestMismatch);
        }
        self.validate_structural_assignment_prefix(structural)?;
        self.validate_explicit_observation_indexes()?;
        let Some(slice) = self.next_pending_explicit_observation_slice() else {
            return Ok(None);
        };
        let current = self.explicit_observation_index_for_slice(slice, false)?;
        if current.is_ready()
            || current.registration_structural_cursor > self.structural_assignment_cursor
            || structural
                .assignment_discovery_prefix_revision(current.registration_structural_cursor)
                != Some(current.registration_structural_revision)
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        let mut next_entry = current.clone();
        let from = current.backfill_cursor;
        let through = from
            .checked_add(usize::from(maximum_assignments.get()))
            .ok_or(MechanismSupportError::CountOverflow)?
            .min(current.registration_structural_cursor);
        if through <= from {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        let mut matched_signatures = Vec::with_capacity(through - from);
        for ordinal in from..through {
            let assignment = structural
                .assignment_discovery_at(ordinal)
                .ok_or(MechanismSupportError::UnknownStructuralAssignment)?;
            let signature_id = assignment.signature_id();
            if !self.imported_structural_assignments.contains(&signature_id)
                || signature_id.request_id() != self.scope.request_id()
            {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }
            if assignment_supports_slice(assignment, slice) {
                if self
                    .signature_explicit_observation_watchers
                    .get(&signature_id)
                    .is_some_and(|watchers| watchers.contains(&slice))
                {
                    return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
                }
                next_entry.observe_assignment(signature_id, assignment)?;
                matched_signatures.push(signature_id);
            }
        }
        next_entry.backfill_cursor = through;
        let prior_registry = self.explicit_observation_registry_summary();
        let prior_pending = self.pending_explicit_observation_backfill_summary();
        let prior_dirty = self.dirty_explicit_observation_summary();
        let prior_unsealed = self.unsealed_explicit_observation_summary();
        let mut next_registry_index = self.explicit_observation_registry_index.clone();
        set_authenticated_value(
            &mut next_registry_index,
            explicit_observation_key(slice),
            explicit_observation_registry_value(&next_entry),
            "explicit observation registry",
        )?;
        let mut next_pending_index = self.pending_explicit_observation_backfill_index.clone();
        let mut next_dirty_index = self.dirty_explicit_observation_index.clone();
        if next_entry.is_ready() {
            next_pending_index
                .remove(&explicit_observation_key(slice))
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex(
                        "pending explicit observation backfills",
                    )
                })?;
            set_authenticated_value(
                &mut next_dirty_index,
                explicit_observation_key(slice),
                dirty_explicit_observation_value(slice),
                "dirty explicit observations",
            )?;
        }
        Ok(Some(MechanismExplicitObservationBackfill {
            slice,
            registration_phase: current.registration_phase,
            registration_structural_cursor: current.registration_structural_cursor as u128,
            registration_structural_revision: current.registration_structural_revision,
            from_structural_cursor: from as u128,
            through_structural_cursor: through as u128,
            prior_registry,
            prior_pending,
            prior_dirty,
            prior_unsealed,
            next_registry_index,
            next_pending_index,
            next_dirty_index,
            next_entry,
            matched_signatures: matched_signatures.into_boxed_slice(),
        }))
    }

    pub(crate) fn commit_explicit_observation_backfill(
        &mut self,
        prepared: MechanismExplicitObservationBackfill,
    ) {
        assert_eq!(
            self.explicit_observation_registry_summary(),
            prepared.prior_registry
        );
        assert_eq!(
            self.pending_explicit_observation_backfill_summary(),
            prepared.prior_pending
        );
        assert_eq!(
            self.dirty_explicit_observation_summary(),
            prepared.prior_dirty
        );
        assert_eq!(
            self.unsealed_explicit_observation_summary(),
            prepared.prior_unsealed
        );
        let completed = prepared.next_entry.is_ready();
        self.explicit_observation_registry_index = prepared.next_registry_index;
        self.pending_explicit_observation_backfill_index = prepared.next_pending_index;
        self.dirty_explicit_observation_index = prepared.next_dirty_index;
        let previous = self
            .explicit_observation_registry
            .insert(prepared.slice, prepared.next_entry);
        assert!(previous.is_some());
        for signature_id in prepared.matched_signatures.iter().copied() {
            assert!(self
                .signature_explicit_observation_watchers
                .entry(signature_id)
                .or_default()
                .insert(prepared.slice));
        }
        if completed {
            assert!(self
                .pending_explicit_observation_backfills
                .remove(&prepared.slice));
            assert!(self.dirty_explicit_observations.insert(prepared.slice));
        }
        debug_assert!(self.validate_explicit_observation_indexes().is_ok());
    }

    pub(crate) fn prepare_explicit_observation_ack(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<MechanismExplicitObservationAck, MechanismSupportError> {
        if self.closure.is_some() {
            return Err(MechanismSupportError::FrontierConflict);
        }
        self.explicit_observation_index_for_slice(slice, true)?;
        if !self.dirty_explicit_observations.contains(&slice)
            || self
                .dirty_explicit_observation_index
                .get(&explicit_observation_key(slice))
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
                })?
                != Some(dirty_explicit_observation_value(slice))
        {
            return Err(MechanismSupportError::FrontierConflict);
        }
        let prior_scheduler = self.explicit_observation_scheduler_summary();
        let mut next_dirty_index = self.dirty_explicit_observation_index.clone();
        next_dirty_index
            .remove(&explicit_observation_key(slice))
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
            })?;
        Ok(MechanismExplicitObservationAck {
            slice,
            prior_scheduler,
            next_dirty_index,
        })
    }

    pub(crate) fn commit_explicit_observation_ack(
        &mut self,
        prepared: MechanismExplicitObservationAck,
    ) {
        assert_eq!(
            self.explicit_observation_scheduler_summary(),
            prepared.prior_scheduler
        );
        assert!(self.dirty_explicit_observations.remove(&prepared.slice));
        self.dirty_explicit_observation_index = prepared.next_dirty_index;
        debug_assert!(self.validate_explicit_observation_indexes().is_ok());
    }

    pub(crate) fn prepare_explicit_observation_seal_ack(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<MechanismExplicitObservationSealAck, MechanismSupportError> {
        if self.closure.is_none() {
            return Err(MechanismSupportError::ClosurePrerequisite(
                "mechanism support closure",
            ));
        }
        self.explicit_observation_index_for_slice(slice, true)?;
        if self.next_unsealed_explicit_observation_slice() != Some(slice)
            || self
                .unsealed_explicit_observation_index
                .get(&explicit_observation_key(slice))
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("unsealed explicit observations")
                })?
                != Some(unsealed_explicit_observation_value(slice))
        {
            return Err(MechanismSupportError::FrontierConflict);
        }
        let prior_scheduler = self.explicit_observation_scheduler_summary();
        let retired_dirty = self.dirty_explicit_observations.contains(&slice);
        let mut next_dirty_index = self.dirty_explicit_observation_index.clone();
        if retired_dirty {
            next_dirty_index
                .remove(&explicit_observation_key(slice))
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
                })?;
        }
        let mut next_unsealed_index = self.unsealed_explicit_observation_index.clone();
        next_unsealed_index
            .remove(&explicit_observation_key(slice))
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("unsealed explicit observations")
            })?;
        Ok(MechanismExplicitObservationSealAck {
            slice,
            prior_scheduler,
            retired_dirty,
            next_dirty_index,
            next_unsealed_index,
        })
    }

    pub(crate) fn commit_explicit_observation_seal_ack(
        &mut self,
        prepared: MechanismExplicitObservationSealAck,
    ) {
        assert_eq!(
            self.explicit_observation_scheduler_summary(),
            prepared.prior_scheduler
        );
        assert!(self.unsealed_explicit_observations.remove(&prepared.slice));
        if prepared.retired_dirty {
            assert!(self.dirty_explicit_observations.remove(&prepared.slice));
        } else {
            assert!(!self.dirty_explicit_observations.contains(&prepared.slice));
        }
        self.dirty_explicit_observation_index = prepared.next_dirty_index;
        self.unsealed_explicit_observation_index = prepared.next_unsealed_index;
        debug_assert!(self.validate_explicit_observation_indexes().is_ok());
    }

    fn validate_explicit_observation_indexes(&self) -> Result<(), MechanismSupportError> {
        let registry_count = self.explicit_observation_registry.len() as u128;
        let pending_count = self.pending_explicit_observation_backfills.len() as u128;
        let dirty_count = self.dirty_explicit_observations.len() as u128;
        let unsealed_count = self.unsealed_explicit_observations.len() as u128;
        if self.explicit_observation_registry_index.entry_count() != registry_count
            || self.explicit_observation_registry_index.total_weight() != registry_count
            || self
                .pending_explicit_observation_backfill_index
                .entry_count()
                != pending_count
            || self
                .pending_explicit_observation_backfill_index
                .total_weight()
                != pending_count
            || self.dirty_explicit_observation_index.entry_count() != dirty_count
            || self.dirty_explicit_observation_index.total_weight() != dirty_count
            || self.unsealed_explicit_observation_index.entry_count() != unsealed_count
            || self.unsealed_explicit_observation_index.total_weight() != unsealed_count
            || pending_count > registry_count
            || dirty_count > registry_count - pending_count
            || unsealed_count > registry_count
            || self.explicit_observation_watchers.len() as u128 != registry_count
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        Ok(())
    }

    fn validate_explicit_observation_entry(
        &self,
        entry: &ExplicitSupportObservationIndex,
    ) -> Result<(), MechanismSupportError> {
        let slice = entry.slice;
        let key = explicit_observation_key(slice);
        let authenticated = self
            .explicit_observation_registry_index
            .get(&key)
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("explicit observation registry")
            })?;
        let pending = self.pending_explicit_observation_backfills.contains(&slice);
        let authenticated_pending = self
            .pending_explicit_observation_backfill_index
            .get(&key)
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("pending explicit observation backfills")
            })?;
        if authenticated != Some(explicit_observation_registry_value(entry))
            || pending == entry.is_ready()
            || authenticated_pending
                != pending.then(|| pending_explicit_observation_backfill_value(slice))
            || self
                .explicit_observation_watchers
                .get(&ExplicitObservationWatcherKey::from_slice(slice))
                != Some(&slice)
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        let dirty = self.dirty_explicit_observations.contains(&slice);
        let authenticated_dirty =
            self.dirty_explicit_observation_index
                .get(&key)
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
                })?;
        if (dirty && !entry.is_ready())
            || authenticated_dirty != dirty.then(|| dirty_explicit_observation_value(slice))
        {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        let unsealed = self.unsealed_explicit_observations.contains(&slice);
        let authenticated_unsealed =
            self.unsealed_explicit_observation_index
                .get(&key)
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("unsealed explicit observations")
                })?;
        if authenticated_unsealed != unsealed.then(|| unsealed_explicit_observation_value(slice)) {
            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
        }
        Ok(())
    }

    fn explicit_observation_index_for_slice(
        &self,
        slice: MechanismSupportSlice,
        require_ready: bool,
    ) -> Result<&ExplicitSupportObservationIndex, MechanismSupportError> {
        let entry = self
            .explicit_observation_registry
            .get(&slice)
            .ok_or(MechanismSupportError::UnknownStructuralSubject)?;
        self.validate_explicit_observation_entry(entry)?;
        if require_ready && !entry.is_ready() {
            return Err(MechanismSupportError::ExplicitObservationBackfillPending);
        }
        Ok(entry)
    }

    fn explicit_observation_slices_for_assignment(
        &self,
        assignment: &StructuralSignatureAssignment,
    ) -> Result<BTreeSet<MechanismSupportSlice>, MechanismSupportError> {
        if self.explicit_observation_watchers.is_empty() {
            return Ok(BTreeSet::new());
        }
        if !self.pending_explicit_observation_backfills.is_empty() {
            return Err(MechanismSupportError::ExplicitObservationBackfillPending);
        }
        let mechanism_id = assignment.mechanism_id();
        let mut slices = BTreeSet::new();
        let mut collect = |subject| -> Result<(), MechanismSupportError> {
            for enclosing_mechanism in [None, Some(mechanism_id)] {
                let key = ExplicitObservationWatcherKey {
                    subject,
                    enclosing_mechanism,
                };
                if let Some(slice) = self.explicit_observation_watchers.get(&key).copied() {
                    self.explicit_observation_index_for_slice(slice, true)?;
                    slices.insert(slice);
                }
            }
            Ok(())
        };
        for node_id in assignment.node_membership().iter().copied() {
            collect(MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id,
            })?;
        }
        for node_id in assignment.differential_node_membership().iter().copied() {
            collect(MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::DifferentialParticipation,
                node_id,
            })?;
        }
        for edge_id in assignment.edge_membership().iter().copied() {
            collect(MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::Activation,
                edge_id,
            })?;
        }
        for edge_id in assignment.differential_edge_membership().iter().copied() {
            collect(MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::DifferentialParticipation,
                edge_id,
            })?;
        }
        Ok(slices)
    }

    fn prepare_mark_explicit_observations_dirty(
        &self,
        slices: &BTreeSet<MechanismSupportSlice>,
    ) -> Result<Option<AuthenticatedTreapMap>, MechanismSupportError> {
        if slices.is_empty() {
            return Ok(None);
        }
        let mut next = self.dirty_explicit_observation_index.clone();
        for slice in slices.iter().copied() {
            self.explicit_observation_index_for_slice(slice, true)?;
            let key = explicit_observation_key(slice);
            let authenticated = next.get(&key).map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
            })?;
            if self.dirty_explicit_observations.contains(&slice) {
                if authenticated != Some(dirty_explicit_observation_value(slice)) {
                    return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
                }
            } else if authenticated.is_some() {
                return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
            } else {
                next.insert(key, dirty_explicit_observation_value(slice))
                    .map_err(|_| {
                        MechanismSupportError::AuthenticatedIndex("dirty explicit observations")
                    })?;
            }
        }
        Ok(Some(next))
    }

    fn commit_mark_explicit_observations_dirty(
        &mut self,
        slices: &BTreeSet<MechanismSupportSlice>,
        next_dirty_index: Option<AuthenticatedTreapMap>,
    ) {
        let Some(next_dirty_index) = next_dirty_index else {
            debug_assert!(slices.is_empty());
            return;
        };
        for slice in slices.iter().copied() {
            self.dirty_explicit_observations.insert(slice);
        }
        self.dirty_explicit_observation_index = next_dirty_index;
        debug_assert!(self.validate_explicit_observation_indexes().is_ok());
    }

    pub(crate) fn automatic_observation_contains(&self, slice: MechanismSupportSlice) -> bool {
        automatic_mechanism_id_for_slice(self.scope, slice).is_some_and(|mechanism_id| {
            self.automatic_observation_registry
                .get(&mechanism_id)
                .is_some_and(|index| index.slice == slice)
        })
    }

    fn automatic_observation_index_for_slice(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<&AutomaticSupportObservationIndex, MechanismSupportError> {
        let mechanism_id = automatic_mechanism_id_for_slice(self.scope, slice)
            .ok_or(MechanismSupportError::UnknownStructuralSubject)?;
        let index = self
            .automatic_observation_registry
            .get(&mechanism_id)
            .filter(|index| index.slice == slice)
            .ok_or(MechanismSupportError::UnknownStructuralSubject)?;
        let authenticated = self
            .automatic_observation_registry_index
            .get(&automatic_observation_registry_key(mechanism_id))
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("automatic observation registry")
            })?;
        if authenticated != Some(automatic_observation_registry_value(index)) {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        Ok(index)
    }

    fn observation_index_for_slice(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<(u128, &[MechanismSignatureId]), MechanismSupportError> {
        if automatic_mechanism_id_for_slice(self.scope, slice).is_some() {
            let index = self.automatic_observation_index_for_slice(slice)?;
            return Ok((
                index.contributing_signature_count,
                &index.inspected_signatures,
            ));
        }
        let index = self.explicit_observation_index_for_slice(slice, true)?;
        Ok((
            index.contributing_signature_count,
            &index.inspected_signatures,
        ))
    }

    pub(crate) fn next_dirty_automatic_observation_slice(&self) -> Option<MechanismSupportSlice> {
        self.dirty_automatic_observations.first().copied()
    }

    /// Return the next registry slice in canonical mechanism-id order without
    /// flattening or cloning the registry. The supplied cursor must itself be
    /// a registered automatic whole-mechanism slice.
    pub(crate) fn next_automatic_observation_slice_after(
        &self,
        after: Option<MechanismSupportSlice>,
    ) -> Result<Option<MechanismSupportSlice>, MechanismSupportError> {
        let next = match after {
            None => self.automatic_observation_registry.first_key_value(),
            Some(slice) => {
                let mechanism_id = automatic_mechanism_id_for_slice(self.scope, slice)
                    .filter(|mechanism_id| {
                        self.automatic_observation_registry
                            .get(mechanism_id)
                            .is_some_and(|index| index.slice == slice)
                    })
                    .ok_or(MechanismSupportError::UnknownStructuralSubject)?;
                self.automatic_observation_registry
                    .range((Excluded(mechanism_id), Unbounded))
                    .next()
            }
        };
        Ok(next.map(|(_, index)| index.slice))
    }

    pub(crate) fn prepare_automatic_observation_ack(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<MechanismAutomaticObservationAck, MechanismSupportError> {
        if !self.automatic_observation_contains(slice) {
            return Err(MechanismSupportError::UnknownStructuralSubject);
        }
        if !self.dirty_automatic_observations.contains(&slice) {
            return Err(MechanismSupportError::FrontierConflict);
        }
        let key = dirty_automatic_observation_key(slice);
        if self
            .dirty_automatic_observation_index
            .get(&key)
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("dirty automatic observations")
            })?
            != Some(dirty_automatic_observation_value(slice))
        {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        let prior_dirty_count = self.dirty_automatic_observation_index.entry_count();
        let mut next_dirty_index = self.dirty_automatic_observation_index.clone();
        next_dirty_index.remove(&key).map_err(|_| {
            MechanismSupportError::AuthenticatedIndex("dirty automatic observations")
        })?;
        Ok(MechanismAutomaticObservationAck {
            slice,
            prior_dirty_root: self.dirty_automatic_observation_index.root_hash(),
            prior_dirty_count,
            next_dirty_index,
        })
    }

    /// Commit a previously prepared acknowledgement. Callers must not mutate
    /// support scheduling state between prepare and commit; the opaque token
    /// binds the exact prior root/count and makes that contract explicit.
    pub(crate) fn commit_automatic_observation_ack(
        &mut self,
        prepared: MechanismAutomaticObservationAck,
    ) {
        assert_eq!(
            self.dirty_automatic_observation_index.root_hash(),
            prepared.prior_dirty_root,
            "automatic observation acknowledgement must commit against its prepared root"
        );
        assert_eq!(
            self.dirty_automatic_observation_index.entry_count(),
            prepared.prior_dirty_count,
            "automatic observation acknowledgement must commit against its prepared count"
        );
        assert!(
            self.dirty_automatic_observations.remove(&prepared.slice),
            "prepared automatic observation must still be dirty"
        );
        self.dirty_automatic_observation_index = prepared.next_dirty_index;
        debug_assert_eq!(
            self.dirty_automatic_observation_index.entry_count(),
            self.dirty_automatic_observations.len() as u128
        );
    }

    fn prepare_mark_automatic_observation_dirty(
        &self,
        slice: MechanismSupportSlice,
    ) -> Result<Option<AuthenticatedTreapMap>, MechanismSupportError> {
        let key = dirty_automatic_observation_key(slice);
        let authenticated = self
            .dirty_automatic_observation_index
            .get(&key)
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("dirty automatic observations")
            })?;
        if self.dirty_automatic_observations.contains(&slice) {
            if authenticated != Some(dirty_automatic_observation_value(slice)) {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }
            return Ok(None);
        }
        if authenticated.is_some() {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        let mut next_dirty_index = self.dirty_automatic_observation_index.clone();
        next_dirty_index
            .insert(key, dirty_automatic_observation_value(slice))
            .map_err(|_| {
                MechanismSupportError::AuthenticatedIndex("dirty automatic observations")
            })?;
        Ok(Some(next_dirty_index))
    }

    fn commit_mark_automatic_observation_dirty(
        &mut self,
        slice: MechanismSupportSlice,
        next_dirty_index: Option<AuthenticatedTreapMap>,
    ) {
        if let Some(next_dirty_index) = next_dirty_index {
            let inserted = self.dirty_automatic_observations.insert(slice);
            assert!(inserted, "prepared automatic observation dirtiness is new");
            self.dirty_automatic_observation_index = next_dirty_index;
        } else {
            assert!(
                self.dirty_automatic_observations.contains(&slice),
                "coalesced automatic observation dirtiness remains present"
            );
        }
        debug_assert_eq!(
            self.dirty_automatic_observation_index.entry_count(),
            self.dirty_automatic_observations.len() as u128
        );
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
                let mut next_target_starter_set = self.target_starter_set_index.clone();
                if prior_refcount == 0 {
                    next_target_starter_set
                        .insert(
                            source.bytes().to_vec().into_boxed_slice(),
                            starter_set_member_value(source),
                        )
                        .map_err(|_| {
                            MechanismSupportError::AuthenticatedIndex("target starter set")
                        })?;
                }
                self.pending_cases = next_pending;
                self.target_starter_index = next_target_starters;
                self.target_starter_set_index = next_target_starter_set;
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
    #[cfg(test)]
    fn sync_structural_assignments(
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
        if !suffix.is_empty() && !self.pending_explicit_observation_backfills.is_empty() {
            return Err(MechanismSupportError::ExplicitObservationBackfillPending);
        }
        let current_prefix_revision = structural
            .assignment_discovery_prefix_revision(self.structural_assignment_cursor)
            .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;
        self.invalidate_projection_caches_outside_prefix(
            self.structural_assignment_cursor,
            current_prefix_revision,
        );
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
            if self.imported_structural_assignments.contains(&signature_id) {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }

            // Prepare every fallible derived-index transition before mutating
            // the authoritative imported prefix. A rejected import must leave
            // observation replay exactly where the cursor says it is. Only the
            // incoming assignment's mechanism accumulator is cloned/touched.
            let mechanism_id = assignment.mechanism_id();
            let existing_automatic_observation =
                self.automatic_observation_registry.get(&mechanism_id);
            let authenticated_existing = self
                .automatic_observation_registry_index
                .get(&automatic_observation_registry_key(mechanism_id))
                .map_err(|_| {
                    MechanismSupportError::AuthenticatedIndex("automatic observation registry")
                })?;
            match (existing_automatic_observation, authenticated_existing) {
                (Some(index), Some(value))
                    if value == automatic_observation_registry_value(index) => {}
                (None, None) => {}
                _ => return Err(MechanismSupportError::StructuralAssignmentPrefixConflict),
            }
            let mut next_automatic_observation = existing_automatic_observation
                .cloned()
                .unwrap_or_else(|| AutomaticSupportObservationIndex::new(self.scope, mechanism_id));
            next_automatic_observation.observe_assignment(signature_id, assignment)?;
            let automatic_slice = next_automatic_observation.slice;
            let mut next_automatic_observation_registry_index =
                self.automatic_observation_registry_index.clone();
            set_authenticated_value(
                &mut next_automatic_observation_registry_index,
                automatic_observation_registry_key(mechanism_id),
                automatic_observation_registry_value(&next_automatic_observation),
                "automatic observation registry",
            )?;
            let next_automatic_observation_indexed_assignment_count = self
                .automatic_observation_indexed_assignment_count
                .checked_add(1)
                .ok_or(MechanismSupportError::CountOverflow)?;
            let expected_registry_count =
                self.automatic_observation_registry
                    .len()
                    .checked_add(usize::from(existing_automatic_observation.is_none()))
                    .ok_or(MechanismSupportError::CountOverflow)? as u128;
            if next_automatic_observation_registry_index.entry_count() != expected_registry_count
                || next_automatic_observation_registry_index.total_weight()
                    != next_automatic_observation_indexed_assignment_count
            {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }
            let next_dirty_automatic_observation_index =
                self.prepare_mark_automatic_observation_dirty(automatic_slice)?;
            let explicit_slices = self.explicit_observation_slices_for_assignment(assignment)?;
            if self
                .signature_explicit_observation_watchers
                .contains_key(&signature_id)
            {
                return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
            }
            let mut next_explicit_observation_registry_index =
                self.explicit_observation_registry_index.clone();
            let mut next_explicit_observations = Vec::with_capacity(explicit_slices.len());
            for slice in explicit_slices.iter().copied() {
                let mut next = self
                    .explicit_observation_index_for_slice(slice, true)?
                    .clone();
                next.observe_assignment(signature_id, assignment)?;
                set_authenticated_value(
                    &mut next_explicit_observation_registry_index,
                    explicit_observation_key(slice),
                    explicit_observation_registry_value(&next),
                    "explicit observation registry",
                )?;
                next_explicit_observations.push((slice, next));
            }
            let next_dirty_explicit_observation_index =
                self.prepare_mark_explicit_observations_dirty(&explicit_slices)?;
            let mut next_unassigned_signature_index = self.unassigned_signature_index.clone();
            if authenticated_contains(
                &next_unassigned_signature_index,
                &signature_key(signature_id),
                "unassigned signatures",
            )? {
                next_unassigned_signature_index
                    .remove(&signature_key(signature_id))
                    .map_err(|_| {
                        MechanismSupportError::AuthenticatedIndex("unassigned signatures")
                    })?;
            }
            let next_structural_assignment_cursor = self
                .structural_assignment_cursor
                .checked_add(1)
                .ok_or(MechanismSupportError::CountOverflow)?;
            let prefix_revision = structural
                .assignment_discovery_prefix_revision(next_structural_assignment_cursor)
                .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;

            self.automatic_observation_registry_index = next_automatic_observation_registry_index;
            self.automatic_observation_indexed_assignment_count =
                next_automatic_observation_indexed_assignment_count;
            self.automatic_observation_registry
                .insert(mechanism_id, next_automatic_observation);
            self.explicit_observation_registry_index = next_explicit_observation_registry_index;
            for (slice, observation) in next_explicit_observations {
                let previous = self
                    .explicit_observation_registry
                    .insert(slice, observation);
                assert!(previous.is_some());
            }
            if !explicit_slices.is_empty() {
                let previous = self
                    .signature_explicit_observation_watchers
                    .insert(signature_id, explicit_slices.clone());
                assert!(previous.is_none());
            }
            self.unassigned_signature_index = next_unassigned_signature_index;
            self.extend_cached_subjects_for_assignment(signature_id, assignment);
            let inserted = self.imported_structural_assignments.insert(signature_id);
            debug_assert!(inserted, "checked structural prefix assignment is new");
            self.structural_assignment_cursor = next_structural_assignment_cursor;
            self.structural_assignment_revision = Some(prefix_revision);
            for projection in self.subject_projection_cache.values_mut() {
                projection
                    .advance_structural_prefix(self.structural_assignment_cursor, prefix_revision);
            }
            self.commit_mark_automatic_observation_dirty(
                automatic_slice,
                next_dirty_automatic_observation_index,
            );
            self.commit_mark_explicit_observations_dirty(
                &explicit_slices,
                next_dirty_explicit_observation_index,
            );
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
        self.validate_explicit_observation_indexes()?;
        if self.imported_structural_assignments.len() != self.structural_assignment_cursor
            || self.automatic_observation_indexed_assignment_count
                != self.structural_assignment_cursor as u128
            || self.automatic_observation_registry.len() as u128
                != self.automatic_observation_registry_index.entry_count()
            || self.automatic_observation_registry_index.total_weight()
                != self.automatic_observation_indexed_assignment_count
            || self.dirty_automatic_observations.len() as u128
                != self.dirty_automatic_observation_index.entry_count()
            || self.dirty_automatic_observation_index.total_weight()
                != self.dirty_automatic_observation_index.entry_count()
        {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
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

        let mut prepared_dirty_automatic_observation = None;
        let mut prepared_dirty_explicit_observations = None;
        match terminal {
            MechanismCaseTerminal::Incidence { signature_id, .. } => {
                let imported_assignment = self
                    .imported_structural_assignments
                    .contains(&signature_id)
                    .then(|| structural.assignment(signature_id))
                    .flatten();
                if self.imported_structural_assignments.contains(&signature_id)
                    && imported_assignment.is_none()
                {
                    return Err(MechanismSupportError::UnknownStructuralAssignment);
                }
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
                let mut next_starters = existing.map_or_else(
                    || AuthenticatedTreapMap::new(SUBJECT_STARTER_SET_INDEX_V1),
                    |fiber| fiber.authenticated_starters.clone(),
                );
                if source_is_new {
                    next_starters
                        .insert(
                            coordinate.source.bytes().to_vec().into_boxed_slice(),
                            starter_set_member_value(coordinate.source),
                        )
                        .map_err(|_| {
                            MechanismSupportError::AuthenticatedIndex("signature starter set")
                        })?;
                }
                let starter_count = prior_starters
                    .checked_add(usize::from(source_is_new))
                    .ok_or(MechanismSupportError::CountOverflow)?
                    as u128;
                let summary = SignatureFiberSummary {
                    root: next_cases.root_hash(),
                    starter_set_root: next_starters.root_hash(),
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
                let mut next_signature_starter_sets = self.signature_starter_set_index.clone();
                set_authenticated_value(
                    &mut next_signature_starter_sets,
                    signature_key(signature_id),
                    signature_starter_set_value(signature_id, summary),
                    "signature starter sets",
                )?;
                let mut next_unassigned = self.unassigned_signature_index.clone();
                if imported_assignment.is_none() {
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
                if let Some(assignment) = imported_assignment {
                    let slice = MechanismSupportSlice::total(MechanismSupportKey::new(
                        self.scope,
                        MechanismSupportSubject::Mechanism(assignment.mechanism_id()),
                    ));
                    if !self.automatic_observation_contains(slice) {
                        return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
                    }
                    prepared_dirty_automatic_observation =
                        Some((slice, self.prepare_mark_automatic_observation_dirty(slice)?));
                    let explicit_slices = self
                        .signature_explicit_observation_watchers
                        .get(&signature_id)
                        .map_or_else(BTreeSet::new, |watchers| {
                            watchers
                                .iter()
                                .copied()
                                .filter(|slice| self.ready_explicit_observation_contains(*slice))
                                .collect()
                        });
                    for explicit_slice in explicit_slices.iter().copied() {
                        let index =
                            self.explicit_observation_index_for_slice(explicit_slice, true)?;
                        if !assignment_supports_slice(assignment, index.slice) {
                            return Err(MechanismSupportError::ExplicitObservationRegistryConflict);
                        }
                    }
                    let next_dirty =
                        self.prepare_mark_explicit_observations_dirty(&explicit_slices)?;
                    prepared_dirty_explicit_observations = Some((explicit_slices, next_dirty));
                }

                self.pending_cases = next_pending;
                self.terminal_fact_index = next_terminal_facts;
                self.signature_fiber_index = next_fiber_index;
                self.signature_starter_set_index = next_signature_starter_sets;
                self.unassigned_signature_index = next_unassigned;
                let fiber = self
                    .signature_fibers
                    .entry(signature_id)
                    .or_insert_with(SignatureCaseFiber::new);
                fiber.authenticated_cases = next_cases;
                fiber.authenticated_starters = next_starters;
                fiber
                    .cases
                    .insert(record.case_id(), (coordinate.source, coordinate.successor));
                fiber
                    .starters
                    .entry(coordinate.source)
                    .or_default()
                    .insert(coordinate.successor);
                if let Some(assignment) = imported_assignment {
                    self.extend_cached_subjects_for_case(
                        assignment,
                        signature_id,
                        summary,
                        record.case_id(),
                        coordinate.source,
                        coordinate.successor,
                    );
                }
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
        if let Some((slice, next_dirty_index)) = prepared_dirty_automatic_observation {
            self.commit_mark_automatic_observation_dirty(slice, next_dirty_index);
        }
        if let Some((slices, next_dirty_index)) = prepared_dirty_explicit_observations {
            self.commit_mark_explicit_observations_dirty(&slices, next_dirty_index);
        }
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
        assignment: &StructuralSignatureAssignment,
        signature_id: MechanismSignatureId,
        summary: SignatureFiberSummary,
        case_id: RelationalCaseId,
        source: SourceKey,
        successor: SuccessorKey,
    ) {
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

    fn invalidate_projection_caches_outside_prefix(
        &mut self,
        structural_assignment_cursor: usize,
        structural_prefix_revision: StructuralCatalogRevision,
    ) {
        self.subject_projection_cache.retain(|_, projection| {
            projection
                .is_for_structural_prefix(structural_assignment_cursor, structural_prefix_revision)
        });
        let retained = &self.subject_projection_cache;
        self.subject_projection_lru
            .retain(|subject| retained.contains_key(subject));
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
        closed_incidence_root: Option<MechanismIncidenceRoot>,
        structural: &StructuralMechanismCatalogBuilder,
        structural_closure_root: Option<StructuralQuotientClosureRoot>,
    ) -> Result<MechanismSupportFrontierSummary, MechanismSupportError> {
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
        let imported_prefix_root = self.derive_imported_prefix_root(
            target_revision,
            terminal_revision,
            structural_revision,
            residual,
        );

        let target_seal_id = self.target_seal.as_ref().map(MechanismTargetSeal::id);
        let root = derive_support_frontier_root(
            imported_prefix_root,
            target_seal_id,
            closed_incidence_root,
            structural_closure_root,
        );
        Ok(MechanismSupportFrontierSummary {
            root,
            imported_prefix_root,
            cursor: self.checkpoint_cursor(),
            target_discovery_revision: target_revision,
            terminal_discovery_revision: terminal_revision,
            structural_assignment_revision: structural_revision,
            target_seal_id,
            incidence_closure_root: closed_incidence_root,
            structural_closure_root,
        })
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
        let target_starter_set_count = self.target_starter_set_index.total_weight();
        let residual = self.factorized_residual()?;
        self.validate_starter_set_authentication()?;
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
            || self.signature_starter_set_index.entry_count() != signature_fibers
            || target_starters != self.target_starter_refcounts.len() as u128
            || target_starter_set_count != target_starters
            || self.target_starter_set_index.entry_count() != target_starters
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
        let mut encoder = SupportEncoder::new(SUPPORT_CLOSURE_ROOT_V2);
        encoder.u32(MECHANISM_SUPPORT_VERSION);
        encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut encoder, self.scope.target());
        encoder.digest(target_seal.id().bytes());
        encoder.digest(incidence_root.bytes());
        encoder.digest(structural_root.bytes());
        encoder.u128(target_cases);
        encode_authenticated_index(&mut encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut encoder, &self.signature_starter_set_index);
        encode_authenticated_index(&mut encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut encoder, &self.target_starter_index);
        encode_authenticated_index(&mut encoder, &self.target_starter_set_index);
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

    /// Derive one immutable, prefix-relative support observation without
    /// mutating projection caches or materializing a case/starter union.
    ///
    /// Only assignments already imported by this support builder are eligible
    /// to contribute to the inner expression. A live structural catalog may
    /// contain a longer discovery suffix; that suffix is deliberately neither
    /// inspected nor treated as confirmed support. Successful fibers whose
    /// assignment has not yet been imported remain in the shared unassigned
    /// residual maintained by the support stream.
    pub(crate) fn derive_factorized_support_observation(
        &self,
        slice: MechanismSupportSlice,
        frontier: MechanismSupportFrontierSummary,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismFactorizedSupportObservationSummary, MechanismSupportError> {
        let key = slice.key();
        if key.request_id != self.scope.request_id()
            || key.target != self.scope.target()
            || structural.request_id() != self.scope.request_id()
        {
            return Err(MechanismSupportError::RequestMismatch);
        }
        let (structural_assignment_cursor, structural_assignment_revision) =
            self.imported_structural_prefix_authority(structural)?;
        let residual = self.factorized_residual()?;
        self.validate_observation_frontier(
            frontier,
            structural,
            structural_assignment_cursor,
            structural_assignment_revision,
            residual,
        )?;
        let (contributing_signature_count, inspected_signatures) =
            self.observation_index_for_slice(slice)?;

        let mut signature_prefix_encoder =
            SupportEncoder::new(FACTORIZED_SUPPORT_OBSERVATION_SIGNATURE_PREFIX_ROOT_V2);
        signature_prefix_encoder.u32(MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION);
        encode_total_or_conditioned_support_slice(&mut signature_prefix_encoder, slice);
        signature_prefix_encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);
        let mut starter_prefix_encoder =
            SupportEncoder::new(FACTORIZED_SUPPORT_OBSERVATION_STARTER_PREFIX_ROOT_V1);
        starter_prefix_encoder.u32(MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION);
        encode_total_or_conditioned_support_slice(&mut starter_prefix_encoder, slice);
        starter_prefix_encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);

        let mut inspected_signature_count = 0u128;
        let mut case_lower_bound = 0u128;
        let mut starter_lower_bound = 0u128;
        for signature_id in inspected_signatures.iter().copied() {
            if !self.imported_structural_assignments.contains(&signature_id) {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }
            let assignment = structural
                .assignment(signature_id)
                .ok_or(MechanismSupportError::UnknownStructuralAssignment)?;
            if assignment.signature_id() != signature_id
                || signature_id.request_id() != self.scope.request_id()
            {
                return Err(MechanismSupportError::RequestMismatch);
            }
            if !assignment_supports_slice(assignment, slice) {
                return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
            }
            inspected_signature_count = inspected_signature_count
                .checked_add(1)
                .ok_or(MechanismSupportError::CountOverflow)?;
            signature_prefix_encoder.digest(signature_id.bytes());
            starter_prefix_encoder.digest(signature_id.bytes());
            let Some(fiber) = self.signature_fibers.get(&signature_id) else {
                // The structural assignment is imported but no successful
                // terminal for it is confirmed yet. Its possible cases are
                // already represented by the pending residual.
                signature_prefix_encoder.u8(0x00);
                starter_prefix_encoder.u8(0x00);
                continue;
            };
            let summary = signature_fiber_summary(fiber);
            let authenticated_summary = self
                .signature_fiber_index
                .get(&signature_id.bytes())
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature fibers"))?;
            let authenticated_starter_summary = self
                .signature_starter_set_index
                .get(&signature_id.bytes())
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature starter sets"))?;
            if authenticated_summary != Some(signature_fiber_value(signature_id, summary))
                || authenticated_starter_summary
                    != Some(signature_starter_set_value(signature_id, summary))
                || summary.starter_count != fiber.starters.len() as u128
                || fiber.authenticated_starters.entry_count() != summary.starter_count
            {
                return Err(MechanismSupportError::ResidualPartitionConflict);
            }
            signature_prefix_encoder.u8(0x01);
            signature_prefix_encoder.digest(summary.root);
            signature_prefix_encoder.u128(summary.case_count);
            signature_prefix_encoder.u128(summary.starter_count);
            starter_prefix_encoder.u8(0x01);
            starter_prefix_encoder.digest(summary.starter_set_root);
            starter_prefix_encoder.u128(summary.starter_count);
            case_lower_bound = case_lower_bound
                .checked_add(summary.case_count)
                .ok_or(MechanismSupportError::CountOverflow)?;
            starter_lower_bound = starter_lower_bound.max(summary.starter_count);
        }
        let scan_limit = u128::try_from(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT)
            .expect("the fixed signature scan limit fits u128");
        if self.automatic_observation_indexed_assignment_count
            != structural_assignment_cursor as u128
            || contributing_signature_count > structural_assignment_cursor as u128
            || inspected_signature_count
                != u128::try_from(inspected_signatures.len())
                    .expect("the bounded observation signature count fits u128")
            || inspected_signature_count != contributing_signature_count.min(scan_limit)
        {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        let signature_scan_complete = inspected_signature_count == contributing_signature_count;
        signature_prefix_encoder.u128(contributing_signature_count);
        signature_prefix_encoder.u128(inspected_signature_count);
        signature_prefix_encoder.u8(u8::from(signature_scan_complete));
        let signature_prefix_root = signature_prefix_encoder.finish();
        starter_prefix_encoder.u128(contributing_signature_count);
        starter_prefix_encoder.u128(inspected_signature_count);
        starter_prefix_encoder.u8(u8::from(signature_scan_complete));
        let starter_prefix_root = starter_prefix_encoder.finish();

        let target_case_count = self.target.len() as u128;
        let target_starter_count = self.target_starter_index.total_weight();
        if target_starter_count != self.target_starter_refcounts.len() as u128
            || case_lower_bound > target_case_count
            || starter_lower_bound > target_starter_count
        {
            return Err(MechanismSupportError::ResidualPartitionConflict);
        }
        let target_frontier_open = !self.target_is_complete();
        let case_count = if target_frontier_open {
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: case_lower_bound,
            }
        } else {
            let upper_bound = if signature_scan_complete {
                case_lower_bound
                    .checked_add(residual.case_count())
                    .ok_or(MechanismSupportError::CountOverflow)?
            } else {
                target_case_count
            };
            if upper_bound < case_lower_bound || upper_bound > target_case_count {
                return Err(MechanismSupportError::ResidualPartitionConflict);
            }
            if upper_bound == case_lower_bound {
                MechanismSupportCount::Exact(case_lower_bound)
            } else {
                MechanismSupportCount::Interval {
                    lower_bound: case_lower_bound,
                    upper_bound,
                }
            }
        };
        let target_starter_root = self.target_starter_set_index.root_hash();
        let (starter_count, starter_bound_basis) = if target_frontier_open {
            (
                MechanismSupportCount::Unknown {
                    confirmed_lower_bound: starter_lower_bound,
                },
                MechanismFactorizedStarterBoundBasis::OpenOpaque,
            )
        } else if starter_lower_bound == target_starter_count {
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
            && residual.case_count() == 0
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

        let structural_root = frontier.structural_closure_root();
        let support_root = self.closure.map(MechanismSupportClosureReceipt::root);
        let inner_fiber_expr_root = derive_factorized_observation_inner_fiber_expr_root(
            slice,
            frontier.imported_prefix_root(),
            signature_prefix_root,
            contributing_signature_count,
            inspected_signature_count,
            case_lower_bound,
            starter_lower_bound,
        );
        let outer_fiber_expr_root = derive_factorized_observation_outer_fiber_expr_root(
            slice,
            inner_fiber_expr_root,
            frontier.root(),
            residual,
            contributing_signature_count,
            inspected_signature_count,
            signature_scan_complete,
            target_frontier_open,
            support_root.is_some(),
        );
        let starter_inner_root =
            derive_factorized_observation_starter_projection_expr_root(slice, starter_prefix_root);
        let correlated_support_is_exact = signature_scan_complete
            && residual.case_count() == 0
            && !target_frontier_open
            && support_root.is_some();
        let support_expression_bounds = derive_support_expression_bounds(
            slice,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
            starter_inner_root,
            target_starter_root,
            target_frontier_open,
            starter_bound_basis.proves_exact_starter_set(),
            correlated_support_is_exact,
        )?;
        let projection_plan_id = match (structural_root, support_root) {
            (Some(structural_root), Some(support_root)) => Some(derive_starter_projection_plan_id(
                slice,
                structural_root,
                support_root,
                support_expression_bounds,
            )),
            _ => None,
        };
        let slice_id = slice.id();
        let root = derive_factorized_support_observation_summary_root(
            slice,
            slice_id,
            frontier,
            structural_root,
            support_root,
            projection_plan_id,
            target_frontier_open,
            support_expression_bounds,
            contributing_signature_count,
            inspected_signature_count,
            signature_scan_complete,
            signature_prefix_root,
            residual,
            case_count,
            starter_count,
            starter_bound_basis,
        );
        Ok(MechanismFactorizedSupportObservationSummary {
            slice,
            slice_id,
            root,
            frontier_root: frontier.root(),
            imported_prefix_root: frontier.imported_prefix_root(),
            structural_root,
            support_root,
            projection_plan_id,
            target_frontier_open,
            support_expression_bounds,
            contributing_signature_count,
            inspected_signature_count,
            signature_scan_complete,
            signature_prefix_root,
            residual,
            case_count,
            starter_count,
            starter_bound_basis,
        })
    }

    fn derive_imported_prefix_root(
        &self,
        target_revision: MechanismTargetDiscoveryRevision,
        terminal_revision: MechanismTerminalDiscoveryRevision,
        structural_revision: StructuralCatalogRevision,
        residual: MechanismSupportResidualSummary,
    ) -> [u8; 32] {
        let mut encoder = SupportEncoder::new(SUPPORT_FRONTIER_IMPORTED_PREFIX_ROOT_V4);
        encoder.u32(MECHANISM_SUPPORT_VERSION);
        encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut encoder, self.scope.target());
        // Only the imported structural prefix is checkpoint authority. The
        // live catalog may already contain later assignments, but hashing its
        // current revision/root here would make the same durable support
        // cursor depend on unimported upstream work.
        encoder.u128(self.target_discovery_cursor as u128);
        encoder.digest(target_revision.bytes());
        encoder.u128(self.terminal_discovery_cursor as u128);
        encoder.digest(terminal_revision.bytes());
        encoder.u128(self.structural_assignment_cursor as u128);
        encoder.digest(structural_revision.bytes());
        // The automatic registry is semantic imported-prefix evidence: one
        // assignment contributes to exactly one whole-mechanism accumulator.
        // The separate dirty scheduler is intentionally not encoded here.
        encode_authenticated_index(&mut encoder, &self.automatic_observation_registry_index);
        encoder.u128(self.automatic_observation_indexed_assignment_count);
        encode_authenticated_index(&mut encoder, &self.pending_cases);
        encode_authenticated_index(&mut encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut encoder, &self.signature_starter_set_index);
        encode_authenticated_index(&mut encoder, &self.unassigned_signature_index);
        encoder.digest(self.target_starter_index.root_hash());
        encoder.u128(self.target_starter_index.total_weight());
        encode_authenticated_index(&mut encoder, &self.target_starter_set_index);
        encoder.digest(residual.root().bytes());
        encoder.u128(residual.case_count());
        encoder.finish()
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_observation_frontier(
        &self,
        frontier: MechanismSupportFrontierSummary,
        structural: &StructuralMechanismCatalogBuilder,
        structural_assignment_cursor: usize,
        structural_assignment_revision: StructuralCatalogRevision,
        residual: MechanismSupportResidualSummary,
    ) -> Result<(), MechanismSupportError> {
        if frontier.cursor() != self.checkpoint_cursor()
            || frontier.structural_assignment_revision() != structural_assignment_revision
            || frontier.target_seal_id() != self.target_seal.as_ref().map(MechanismTargetSeal::id)
            || self
                .target_discovery_revision
                .is_some_and(|revision| revision != frontier.target_discovery_revision())
            || self
                .terminal_discovery_revision
                .is_some_and(|revision| revision != frontier.terminal_discovery_revision())
            || structural_assignment_cursor != self.imported_structural_assignments.len()
        {
            return Err(MechanismSupportError::FrontierConflict);
        }
        let imported_prefix_root = self.derive_imported_prefix_root(
            frontier.target_discovery_revision(),
            frontier.terminal_discovery_revision(),
            frontier.structural_assignment_revision(),
            residual,
        );
        if imported_prefix_root != frontier.imported_prefix_root()
            || derive_support_frontier_root(
                imported_prefix_root,
                frontier.target_seal_id(),
                frontier.incidence_closure_root(),
                frontier.structural_closure_root(),
            ) != frontier.root()
        {
            return Err(MechanismSupportError::FrontierConflict);
        }
        if let Some(structural_root) = frontier.structural_closure_root() {
            if structural.closure().map(|closure| closure.root()) != Some(structural_root) {
                return Err(MechanismSupportError::FrontierConflict);
            }
        }
        if let Some(support_closure) = self.closure {
            if frontier.target_seal_id() != Some(support_closure.target_seal_id())
                || frontier.incidence_closure_root() != Some(support_closure.incidence_root())
                || frontier.structural_closure_root() != Some(support_closure.structural_root())
                || support_closure.request_id() != self.scope.request_id()
                || support_closure.target() != self.scope.target()
                || structural_assignment_cursor != structural.assignment_count()
                || residual.root() != support_closure.residual_root()
                || residual.case_count() != support_closure.unavailable_case_count()
            {
                return Err(MechanismSupportError::ClosureConflict);
            }
        }
        Ok(())
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
        self.derive_closed_factorized_support_slice_summary(
            MechanismSupportSlice::total(key),
            structural,
        )
    }

    /// Derive the same bounded factorized summary for either total subject
    /// support or the subject's support within one enclosing mechanism. Route
    /// conditioning intersects the two immutable signature indexes before any
    /// case fiber is inspected.
    pub(crate) fn derive_closed_factorized_support_slice_summary(
        &self,
        slice: MechanismSupportSlice,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismFactorizedSubjectSummary, MechanismSupportError> {
        let key = slice.key();
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
            || self.signature_starter_set_index.entry_count()
                != support_closure.signature_fiber_count()
            || self.target_starter_index.total_weight() != support_closure.target_starter_count()
            || self.target_starter_refcounts.len() as u128 != support_closure.target_starter_count()
            || self.target_starter_set_index.total_weight()
                != support_closure.target_starter_count()
            || self.target_starter_set_index.entry_count() != support_closure.target_starter_count()
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
        let mut closure_encoder = SupportEncoder::new(SUPPORT_CLOSURE_ROOT_V2);
        closure_encoder.u32(MECHANISM_SUPPORT_VERSION);
        closure_encoder.digest(self.scope.request_id().bytes());
        encode_target(&mut closure_encoder, self.scope.target());
        closure_encoder.digest(support_closure.target_seal_id().bytes());
        closure_encoder.digest(support_closure.incidence_root().bytes());
        closure_encoder.digest(structural_closure.root().bytes());
        closure_encoder.u128(support_closure.target_case_count());
        encode_authenticated_index(&mut closure_encoder, &self.terminal_fact_index);
        encode_authenticated_index(&mut closure_encoder, &self.signature_fiber_index);
        encode_authenticated_index(&mut closure_encoder, &self.signature_starter_set_index);
        encode_authenticated_index(&mut closure_encoder, &self.unavailable_cases);
        encode_authenticated_index(&mut closure_encoder, &self.target_starter_index);
        encode_authenticated_index(&mut closure_encoder, &self.target_starter_set_index);
        encode_authenticated_index(&mut closure_encoder, &self.pending_cases);
        encode_authenticated_index(&mut closure_encoder, &self.unassigned_signature_index);
        closure_encoder.digest(residual.root.bytes());
        closure_encoder.u128(residual.case_count);
        if closure_encoder.finish() != support_closure.root().bytes() {
            return Err(MechanismSupportError::ClosureConflict);
        }
        validate_structural_subject(structural, slice.subject())?;

        let signatures = supporting_signatures_for_slice(structural, slice)?;
        let contributing_signature_count = signatures.len() as u128;
        let mut prefix_encoder = factorized_support_slice_signature_prefix_encoder(slice);
        prefix_encoder.u128(contributing_signature_count);
        prefix_encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);

        let mut inspected_signature_count = 0u128;
        let mut case_lower_bound = 0u128;
        let mut starter_lower_bound = 0u128;
        for signature_id in signatures
            .iter()
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
            let authenticated_starter_summary = self
                .signature_starter_set_index
                .get(&signature_bytes)
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature starter sets"))?;
            if authenticated_summary != Some(signature_fiber_value(signature_id, summary))
                || authenticated_starter_summary
                    != Some(signature_starter_set_value(signature_id, summary))
                || summary.starter_count != fiber.starters.len() as u128
                || fiber.authenticated_starters.entry_count() != summary.starter_count
            {
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
        let target_starter_root = self.target_starter_set_index.root_hash();
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

        let inner_fiber_expr_root = derive_factorized_inner_fiber_expr_root(
            slice,
            structural_closure.root(),
            self.signature_fiber_index.root_hash(),
        );
        let outer_fiber_expr_root = derive_outer_fiber_expr_root(
            slice,
            inner_fiber_expr_root,
            residual.root,
            residual.case_count,
            target_frontier_open,
        );
        let starter_inner_root = derive_factorized_subject_starter_projection_expr_root(
            slice,
            structural_closure.root(),
            self.signature_starter_set_index.root_hash(),
        );
        let correlated_support_is_exact = residual.case_count == 0 && !target_frontier_open;
        let support_expression_bounds = derive_support_expression_bounds(
            slice,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
            starter_inner_root,
            target_starter_root,
            target_frontier_open,
            starter_bound_basis.proves_exact_starter_set(),
            correlated_support_is_exact,
        )?;
        let projection_plan_id = derive_starter_projection_plan_id(
            slice,
            structural_closure.root(),
            support_closure.root(),
            support_expression_bounds,
        );
        let root = derive_factorized_subject_summary_root(
            slice,
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
            support_expression_bounds,
        );
        Ok(MechanismFactorizedSubjectSummary {
            slice,
            root,
            projection_plan_id,
            support_expression_bounds,
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
        self.derive_closed_support_slice_starter_projection_authority(
            MechanismSupportSlice::total(key),
            structural,
        )
    }

    /// Freeze exact key-only paging authority for a total or route-conditioned
    /// support slice. Both forms use the same correlated signature fibers; the
    /// slice only changes which raw signatures participate.
    pub(crate) fn derive_closed_support_slice_starter_projection_authority(
        &self,
        slice: MechanismSupportSlice,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<MechanismClosedSubjectStarterProjectionAuthority, MechanismSupportError> {
        let summary = self.derive_closed_factorized_support_slice_summary(slice, structural)?;
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
        if !summary
            .support_expression_bounds()
            .correlated_support_status()
            .is_exact()
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
        let signatures = supporting_signatures_for_slice(structural, slice)?;
        let mut exact_case_count = 0u128;
        for signature_id in signatures.iter() {
            let assignment = structural
                .assignment(signature_id)
                .ok_or(MechanismSupportError::ClosureConflict)?;
            if !assignment_supports_slice(assignment, slice) {
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
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature fibers"))?;
            let authenticated_starter_summary = self
                .signature_starter_set_index
                .get(&signature_id.bytes())
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature starter sets"))?;
            if authenticated_summary != Some(signature_fiber_value(signature_id, fiber_summary))
                || authenticated_starter_summary
                    != Some(signature_starter_set_value(signature_id, fiber_summary))
                || fiber_summary.case_count != fiber.cases.len() as u128
                || fiber_summary.starter_count != fiber.starters.len() as u128
                || fiber.authenticated_starters.entry_count() != fiber_summary.starter_count
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
            slice,
            question_id: self.scope.question_id(),
            projection_plan_id: summary.projection_plan_id(),
            support_expression_bounds: summary.support_expression_bounds(),
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
        let current = self.derive_closed_support_slice_starter_projection_authority(
            authority.slice(),
            structural,
        )?;
        if current != authority {
            return Err(MechanismSupportError::ClosureConflict);
        }
        let signatures = supporting_signatures_for_slice(structural, authority.slice())?;
        let mut candidates = BTreeSet::new();
        for signature_id in signatures.iter() {
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
            || self.signature_starter_set_index.entry_count()
                != support_closure.signature_fiber_count()
            || self.target_starter_index.total_weight() != support_closure.target_starter_count()
            || self.target_starter_refcounts.len() as u128 != support_closure.target_starter_count()
            || self.target_starter_set_index.total_weight()
                != support_closure.target_starter_count()
            || self.target_starter_set_index.entry_count() != support_closure.target_starter_count()
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
        let (structural_assignment_cursor, structural_prefix_revision) =
            self.imported_structural_prefix_authority(structural)?;
        self.validate_imported_structural_subject(structural, key.subject)?;
        let projection = build_imported_subject_projection(
            key.subject,
            structural,
            &self.imported_structural_assignments,
            &self.signature_fibers,
            structural_assignment_cursor,
            structural_prefix_revision,
        )?;
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
        let (structural_assignment_cursor, structural_prefix_revision) =
            self.imported_structural_prefix_authority(structural)?;
        self.invalidate_projection_caches_outside_prefix(
            structural_assignment_cursor,
            structural_prefix_revision,
        );
        self.validate_imported_structural_subject(structural, key.subject)?;
        if !self.subject_projection_cache.contains_key(&key.subject) {
            let projection = build_imported_subject_projection(
                key.subject,
                structural,
                &self.imported_structural_assignments,
                &self.signature_fibers,
                structural_assignment_cursor,
                structural_prefix_revision,
            )?;
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
        let (structural_assignment_cursor, structural_prefix_revision) =
            self.imported_structural_prefix_authority(structural)?;
        if !projection
            .is_for_structural_prefix(structural_assignment_cursor, structural_prefix_revision)
        {
            return Err(MechanismSupportError::StructuralAssignmentPrefixConflict);
        }
        let structural_authority = self.support_view_structural_authority(
            structural,
            structural_assignment_cursor,
            structural_prefix_revision,
        )?;
        let inner_signature_root = projection.signature_index.root_hash();
        let inner_case_root = projection.case_index.root_hash();
        let inner_starter_root = projection.correlated_starter_index.root_hash();
        let inner_starter_set_root = projection.starter_set_index.root_hash();
        let inner_cases = projection.case_count();
        let inner_starters = projection.starter_count();
        let target_starter_root = self.target_starter_set_index.root_hash();
        let target_starters = self.target_starter_index.total_weight();
        if target_starters != self.target_starter_refcounts.len() as u128
            || self.target_starter_set_index.entry_count() != target_starters
            || self.target_starter_set_index.total_weight() != target_starters
            || projection.correlated_starter_index.entry_count() != inner_starters
            || projection.correlated_starter_index.total_weight() != inner_starters
            || projection.successor_fibers.len() as u128 != inner_starters
        {
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
        let total_slice = MechanismSupportSlice::total(key);
        let inner_fiber_expr_root =
            derive_materialized_inner_fiber_expr_root(total_slice, inner_starter_root);
        let outer_fiber_expr_root = derive_outer_fiber_expr_root(
            total_slice,
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
        let starter_inner_expr_root =
            derive_materialized_starter_projection_expr_root(total_slice, inner_starter_set_root);
        let correlated_support_is_exact = !target_frontier_open && residual_cases == 0;
        let support_expression_bounds = derive_support_expression_bounds(
            total_slice,
            inner_fiber_expr_root,
            outer_fiber_expr_root,
            starter_inner_expr_root,
            target_starter_root,
            target_frontier_open,
            starter_upper_provenance.proves_exact_starter_set(),
            correlated_support_is_exact,
        )?;
        let root = derive_support_view_root(
            key,
            self.target_seal.as_ref().map(MechanismTargetSeal::id),
            structural_authority,
            self.terminal_fact_index.root_hash(),
            self.signature_fiber_index.root_hash(),
            target_starter_root,
            inner_signature_root,
            inner_case_root,
            inner_starter_root,
            shared_residual.root,
            target_frontier_open,
            support_expression_bounds,
            case_count,
            starter_count,
            starter_upper_provenance,
        );
        Ok(MechanismSupportView {
            key,
            root,
            support_expression_bounds,
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

    fn imported_structural_prefix_authority(
        &self,
        structural: &StructuralMechanismCatalogBuilder,
    ) -> Result<(usize, StructuralCatalogRevision), MechanismSupportError> {
        if structural.request_id() != self.scope.request_id() {
            return Err(MechanismSupportError::RequestMismatch);
        }
        self.validate_structural_assignment_prefix(structural)?;
        let revision = structural
            .assignment_discovery_prefix_revision(self.structural_assignment_cursor)
            .ok_or(MechanismSupportError::StructuralAssignmentCursorRegression)?;
        Ok((self.structural_assignment_cursor, revision))
    }

    fn support_view_structural_authority(
        &self,
        structural: &StructuralMechanismCatalogBuilder,
        assignment_cursor: usize,
        prefix_revision: StructuralCatalogRevision,
    ) -> Result<MechanismSupportViewStructuralAuthority, MechanismSupportError> {
        let Some(support_closure) = self.closure else {
            return Ok(MechanismSupportViewStructuralAuthority::OpenPrefix {
                assignment_cursor,
                prefix_revision,
            });
        };
        let structural_closure =
            structural
                .closure()
                .ok_or(MechanismSupportError::ClosurePrerequisite(
                    "structural quotient closure",
                ))?;
        if support_closure.structural_root() != structural_closure.root()
            || assignment_cursor != structural.assignment_count()
        {
            return Err(MechanismSupportError::ClosureConflict);
        }
        Ok(MechanismSupportViewStructuralAuthority::Closed {
            structural_root: structural_closure.root(),
            assignment_root: structural.assignment_root(),
            assignment_count: structural.assignment_count(),
        })
    }

    fn validate_imported_structural_subject(
        &self,
        structural: &StructuralMechanismCatalogBuilder,
        subject: MechanismSupportSubject,
    ) -> Result<(), MechanismSupportError> {
        validate_structural_subject(structural, subject)?;
        for signature_id in &self.imported_structural_assignments {
            let assignment = structural
                .assignment(*signature_id)
                .ok_or(MechanismSupportError::UnknownStructuralAssignment)?;
            if assignment_introduces_subject(assignment, subject) {
                return Ok(());
            }
        }
        Err(MechanismSupportError::UnknownStructuralSubject)
    }

    pub(crate) fn residual_summary(
        &self,
    ) -> Result<MechanismSupportResidualSummary, MechanismSupportError> {
        self.factorized_residual()
    }

    fn factorized_residual(
        &self,
    ) -> Result<MechanismSupportResidualSummary, MechanismSupportError> {
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
            || self.target_starter_set_index.entry_count() != target_starters
            || self.target_starter_set_index.total_weight() != target_starters
            || self.signature_starter_set_index.entry_count()
                != self.signature_fiber_index.entry_count()
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
        Ok(MechanismSupportResidualSummary {
            root: MechanismSupportResidualRoot(encoder.finish()),
            case_count,
            pending_cases: MechanismSupportResidualComponentSummary {
                root: MechanismSupportResidualComponentRoot(self.pending_cases.root_hash()),
                member_count: self.pending_cases.entry_count(),
                case_count: pending_cases,
            },
            unavailable_cases: MechanismSupportResidualComponentSummary {
                root: MechanismSupportResidualComponentRoot(self.unavailable_cases.root_hash()),
                member_count: self.unavailable_cases.entry_count(),
                case_count: unavailable_cases,
            },
            unassigned_signatures: MechanismSupportResidualComponentSummary {
                root: MechanismSupportResidualComponentRoot(
                    self.unassigned_signature_index.root_hash(),
                ),
                member_count: self.unassigned_signature_index.entry_count(),
                case_count: unassigned_cases,
            },
        })
    }

    fn validate_starter_set_authentication(&self) -> Result<(), MechanismSupportError> {
        for (source, refcount) in &self.target_starter_refcounts {
            let key = source.bytes();
            let refcount_value = self
                .target_starter_index
                .get(&key)
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("target starters"))?;
            let set_value = self
                .target_starter_set_index
                .get(&key)
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("target starter set"))?;
            if *refcount == 0
                || refcount_value != Some(target_starter_value(*source, *refcount))
                || set_value != Some(starter_set_member_value(*source))
            {
                return Err(MechanismSupportError::ResidualPartitionConflict);
            }
        }

        for (signature_id, fiber) in &self.signature_fibers {
            let summary = signature_fiber_summary(fiber);
            if summary.case_count != fiber.cases.len() as u128
                || summary.starter_count != fiber.starters.len() as u128
                || fiber.authenticated_cases.entry_count() != summary.case_count
                || fiber.authenticated_starters.entry_count() != summary.starter_count
            {
                return Err(MechanismSupportError::ResidualPartitionConflict);
            }
            let mut correlated_case_count = 0u128;
            for source in fiber.starters.keys().copied() {
                let value = fiber
                    .authenticated_starters
                    .get(&source.bytes())
                    .map_err(|_| {
                        MechanismSupportError::AuthenticatedIndex("signature starter set")
                    })?;
                if value != Some(starter_set_member_value(source)) {
                    return Err(MechanismSupportError::ResidualPartitionConflict);
                }
                correlated_case_count = correlated_case_count
                    .checked_add(fiber.starters[&source].len() as u128)
                    .ok_or(MechanismSupportError::CountOverflow)?;
            }
            let key = signature_id.bytes();
            let fiber_value = self
                .signature_fiber_index
                .get(&key)
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature fibers"))?;
            let starter_value = self
                .signature_starter_set_index
                .get(&key)
                .map_err(|_| MechanismSupportError::AuthenticatedIndex("signature starter sets"))?;
            if correlated_case_count != summary.case_count
                || fiber_value != Some(signature_fiber_value(*signature_id, summary))
                || starter_value != Some(signature_starter_set_value(*signature_id, summary))
            {
                return Err(MechanismSupportError::ResidualPartitionConflict);
            }
        }
        Ok(())
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

fn assignment_introduces_subject(
    assignment: &StructuralSignatureAssignment,
    subject: MechanismSupportSubject,
) -> bool {
    match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => {
            assignment.mechanism_id() == mechanism_id
        }
        // Differential participation is a support facet of an activation
        // subject. Its addressability comes from imported activation
        // membership even when this signature contributes no differential
        // support for the node or edge.
        MechanismSupportSubject::Node { node_id, .. } => {
            assignment.node_membership().binary_search(&node_id).is_ok()
        }
        MechanismSupportSubject::Edge { edge_id, .. } => {
            assignment.edge_membership().binary_search(&edge_id).is_ok()
        }
    }
}

fn assignment_supports_slice(
    assignment: &StructuralSignatureAssignment,
    slice: MechanismSupportSlice,
) -> bool {
    assignment_supports_subject(assignment, slice.subject())
        && slice
            .enclosing_mechanism()
            .is_none_or(|mechanism_id| assignment.mechanism_id() == mechanism_id)
}

fn automatic_mechanism_id_for_slice(
    scope: MechanismRequestScope,
    slice: MechanismSupportSlice,
) -> Option<StructuralMechanismId> {
    if slice.key().request_id() != scope.request_id()
        || slice.key().target() != scope.target()
        || slice.enclosing_mechanism().is_some()
    {
        return None;
    }
    match slice.subject() {
        MechanismSupportSubject::Mechanism(mechanism_id) => Some(mechanism_id),
        MechanismSupportSubject::Node { .. } | MechanismSupportSubject::Edge { .. } => None,
    }
}

enum SupportingSignatureSlice<'a> {
    Empty,
    Total(&'a BTreeSet<MechanismSignatureId>),
    Intersection {
        subject: &'a BTreeSet<MechanismSignatureId>,
        mechanism: &'a BTreeSet<MechanismSignatureId>,
    },
}

impl SupportingSignatureSlice<'_> {
    fn len(&self) -> usize {
        self.iter().count()
    }

    fn iter(&self) -> SupportingSignatureSliceIter<'_> {
        match self {
            Self::Empty => SupportingSignatureSliceIter::Empty,
            Self::Total(signatures) => {
                SupportingSignatureSliceIter::Total(signatures.iter().copied())
            }
            Self::Intersection { subject, mechanism } => {
                SupportingSignatureSliceIter::Intersection(subject.intersection(mechanism).copied())
            }
        }
    }
}

enum SupportingSignatureSliceIter<'a> {
    Empty,
    Total(std::iter::Copied<std::collections::btree_set::Iter<'a, MechanismSignatureId>>),
    Intersection(
        std::iter::Copied<std::collections::btree_set::Intersection<'a, MechanismSignatureId>>,
    ),
}

impl Iterator for SupportingSignatureSliceIter<'_> {
    type Item = MechanismSignatureId;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Total(iter) => iter.next(),
            Self::Intersection(iter) => iter.next(),
        }
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

fn supporting_signatures_for_slice<'a>(
    structural: &'a StructuralMechanismCatalogBuilder,
    slice: MechanismSupportSlice,
) -> Result<SupportingSignatureSlice<'a>, MechanismSupportError> {
    let mechanism_signatures = match slice.enclosing_mechanism() {
        Some(mechanism_id) => Some(
            structural
                .signatures_for_mechanism(mechanism_id)
                .ok_or(MechanismSupportError::UnknownStructuralSubject)?,
        ),
        None => None,
    };
    let Some(subject_signatures) = supporting_signatures(structural, slice.subject()) else {
        return Ok(SupportingSignatureSlice::Empty);
    };
    Ok(match mechanism_signatures {
        Some(mechanism) => SupportingSignatureSlice::Intersection {
            subject: subject_signatures,
            mechanism,
        },
        None => SupportingSignatureSlice::Total(subject_signatures),
    })
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

fn validate_explicit_observation_slice(
    structural: &StructuralMechanismCatalogBuilder,
    slice: MechanismSupportSlice,
) -> Result<(), MechanismSupportError> {
    validate_structural_subject(structural, slice.subject())?;
    let Some(enclosing_mechanism) = slice.enclosing_mechanism() else {
        return Ok(());
    };
    let activation_signatures = match slice.subject() {
        MechanismSupportSubject::Mechanism(_) => {
            return Err(MechanismSupportError::InvalidExplicitObservationRoute);
        }
        MechanismSupportSubject::Node { node_id, .. } => {
            structural.signatures_for_node(node_id, false)
        }
        MechanismSupportSubject::Edge { edge_id, .. } => {
            structural.signatures_for_edge(edge_id, false)
        }
    }
    .ok_or(MechanismSupportError::UnknownStructuralSubject)?;
    let mechanism_signatures = structural
        .signatures_for_mechanism(enclosing_mechanism)
        .ok_or(MechanismSupportError::InvalidExplicitObservationRoute)?;
    if activation_signatures
        .intersection(mechanism_signatures)
        .next()
        .is_none()
    {
        return Err(MechanismSupportError::InvalidExplicitObservationRoute);
    }
    Ok(())
}

fn build_imported_subject_projection(
    subject: MechanismSupportSubject,
    structural: &StructuralMechanismCatalogBuilder,
    imported_assignments: &BTreeSet<MechanismSignatureId>,
    fibers: &BTreeMap<MechanismSignatureId, SignatureCaseFiber>,
    structural_assignment_cursor: usize,
    structural_prefix_revision: StructuralCatalogRevision,
) -> Result<SubjectProjectionCache, MechanismSupportError> {
    let mut projection =
        SubjectProjectionCache::new(structural_assignment_cursor, structural_prefix_revision);
    for signature_id in imported_assignments.iter().copied() {
        let assignment = structural
            .assignment(signature_id)
            .ok_or(MechanismSupportError::UnknownStructuralAssignment)?;
        if assignment_supports_subject(assignment, subject) {
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
        starter_set_root: fiber.authenticated_starters.root_hash(),
        case_count: fiber.authenticated_cases.total_weight(),
        starter_count: fiber.authenticated_starters.total_weight(),
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
    let mut correlated_encoder = SupportEncoder::new(CORRELATED_STARTER_FIBER_VALUE_V2);
    correlated_encoder.u32(MECHANISM_SUPPORT_VERSION);
    correlated_encoder.digest(source.bytes());
    correlated_encoder.digest(next_successors.root_hash());
    correlated_encoder.u128(successor_count);
    let correlated_value = AuthenticatedTreapValue::new(correlated_encoder.finish(), 1);
    let mut next_correlated_starters = projection.correlated_starter_index.clone();
    set_authenticated_value(
        &mut next_correlated_starters,
        source.bytes().to_vec().into_boxed_slice(),
        correlated_value,
        "subject correlated starters",
    )?;
    let mut next_starter_set = projection.starter_set_index.clone();
    if !authenticated_contains(&next_starter_set, &source.bytes(), "subject starter set")? {
        next_starter_set
            .insert(
                source.bytes().to_vec().into_boxed_slice(),
                starter_set_member_value(source),
            )
            .map_err(|_| MechanismSupportError::AuthenticatedIndex("subject starter set"))?;
    }
    projection.successor_fibers.insert(source, next_successors);
    projection.correlated_starter_index = next_correlated_starters;
    projection.starter_set_index = next_starter_set;
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
    encoder.digest(summary.starter_set_root);
    encoder.u128(summary.case_count);
    encoder.u128(summary.starter_count);
    AuthenticatedTreapValue::new(encoder.finish(), summary.case_count)
}

fn signature_starter_set_value(
    signature_id: MechanismSignatureId,
    summary: SignatureFiberSummary,
) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(SIGNATURE_STARTER_SET_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encoder.digest(signature_id.bytes());
    encoder.digest(summary.starter_set_root);
    encoder.u128(summary.starter_count);
    AuthenticatedTreapValue::new(encoder.finish(), summary.starter_count)
}

fn starter_set_member_value(source: SourceKey) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(STARTER_SET_MEMBER_VALUE_V1);
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encoder.digest(source.bytes());
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn automatic_observation_registry_value(
    index: &AutomaticSupportObservationIndex,
) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(AUTOMATIC_OBSERVATION_REGISTRY_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, index.slice);
    encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);
    encoder.u128(index.contributing_signature_count);
    encoder.u128(index.inspected_signatures.len() as u128);
    for signature_id in index.inspected_signatures.iter().copied() {
        encoder.digest(signature_id.bytes());
    }
    AuthenticatedTreapValue::new(encoder.finish(), index.contributing_signature_count)
}

fn explicit_observation_registry_summary(
    index: &AuthenticatedTreapMap,
    pending_slice_count: u128,
) -> MechanismExplicitObservationRegistrySummary {
    let slice_count = index.entry_count();
    debug_assert_eq!(index.total_weight(), slice_count);
    let ready_slice_count = slice_count
        .checked_sub(pending_slice_count)
        .expect("pending explicit observations are a registry subset");
    MechanismExplicitObservationRegistrySummary {
        root: MechanismExplicitObservationRegistryRoot(index.root_hash()),
        slice_count,
        ready_slice_count,
    }
}

fn pending_explicit_observation_backfill_summary(
    index: &AuthenticatedTreapMap,
) -> MechanismPendingExplicitObservationBackfillSummary {
    debug_assert_eq!(index.total_weight(), index.entry_count());
    MechanismPendingExplicitObservationBackfillSummary {
        root: MechanismPendingExplicitObservationBackfillRoot(index.root_hash()),
        slice_count: index.entry_count(),
    }
}

fn dirty_explicit_observation_summary(
    index: &AuthenticatedTreapMap,
) -> MechanismDirtyExplicitObservationSummary {
    debug_assert_eq!(index.total_weight(), index.entry_count());
    MechanismDirtyExplicitObservationSummary {
        root: MechanismDirtyExplicitObservationRoot(index.root_hash()),
        slice_count: index.entry_count(),
    }
}

fn unsealed_explicit_observation_summary(
    index: &AuthenticatedTreapMap,
) -> MechanismUnsealedExplicitObservationSummary {
    debug_assert_eq!(index.total_weight(), index.entry_count());
    MechanismUnsealedExplicitObservationSummary {
        root: MechanismUnsealedExplicitObservationRoot(index.root_hash()),
        slice_count: index.entry_count(),
    }
}

fn explicit_observation_registry_value(
    index: &ExplicitSupportObservationIndex,
) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(EXPLICIT_OBSERVATION_REGISTRY_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, index.slice);
    match index.registration_phase {
        MechanismExplicitObservationRegistrationPhase::Open => encoder.u8(0x01),
        MechanismExplicitObservationRegistrationPhase::Sealed { support_root } => {
            encoder.u8(0x02);
            encoder.digest(support_root.bytes());
        }
    }
    encoder.u128(index.registration_structural_cursor as u128);
    encoder.digest(index.registration_structural_revision.bytes());
    encoder.u128(index.backfill_cursor as u128);
    encoder.u128(EXPLICIT_OBSERVATION_BACKFILL_MAX_ASSIGNMENTS as u128);
    encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);
    encoder.u128(index.contributing_signature_count);
    encoder.u128(index.inspected_signatures.len() as u128);
    for signature_id in index.inspected_signatures.iter().copied() {
        encoder.digest(signature_id.bytes());
    }
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn pending_explicit_observation_backfill_value(
    slice: MechanismSupportSlice,
) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(PENDING_EXPLICIT_OBSERVATION_BACKFILL_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn dirty_explicit_observation_value(slice: MechanismSupportSlice) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(DIRTY_EXPLICIT_OBSERVATION_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn unsealed_explicit_observation_value(slice: MechanismSupportSlice) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(UNSEALED_EXPLICIT_OBSERVATION_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    AuthenticatedTreapValue::new(encoder.finish(), 1)
}

fn dirty_automatic_observation_value(slice: MechanismSupportSlice) -> AuthenticatedTreapValue {
    let mut encoder = SupportEncoder::new(DIRTY_AUTOMATIC_OBSERVATION_VALUE_V1);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    AuthenticatedTreapValue::new(encoder.finish(), 1)
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

fn automatic_observation_registry_key(mechanism_id: StructuralMechanismId) -> Box<[u8]> {
    mechanism_id.bytes().to_vec().into_boxed_slice()
}

fn explicit_observation_key(slice: MechanismSupportSlice) -> Box<[u8]> {
    slice.id().bytes().to_vec().into_boxed_slice()
}

fn dirty_automatic_observation_key(slice: MechanismSupportSlice) -> Box<[u8]> {
    slice.id().bytes().to_vec().into_boxed_slice()
}

fn derive_support_slice_id(slice: MechanismSupportSlice) -> MechanismSupportSliceId {
    let mut encoder = SupportEncoder::new(SUPPORT_SLICE_ID_V1);
    encoder.u32(MECHANISM_SUPPORT_SLICE_ID_VERSION);
    encode_support_key(&mut encoder, slice.key());
    match slice.enclosing_mechanism() {
        None => encoder.u8(0x00),
        Some(mechanism_id) => {
            encoder.u8(0x01);
            encoder.digest(mechanism_id.bytes());
        }
    }
    MechanismSupportSliceId(encoder.finish())
}

fn derive_support_frontier_root(
    imported_prefix_root: [u8; 32],
    target_seal_id: Option<MechanismTargetSealId>,
    incidence_closure_root: Option<MechanismIncidenceRoot>,
    structural_closure_root: Option<StructuralQuotientClosureRoot>,
) -> MechanismSupportFrontierRoot {
    let mut encoder = SupportEncoder::new(SUPPORT_FRONTIER_ROOT_V4);
    encoder.u32(MECHANISM_SUPPORT_VERSION);
    encoder.digest(imported_prefix_root);
    encode_optional_digest(
        &mut encoder,
        target_seal_id.map(MechanismTargetSealId::bytes),
    );
    encode_optional_digest(
        &mut encoder,
        incidence_closure_root.map(MechanismIncidenceRoot::bytes),
    );
    encode_optional_digest(
        &mut encoder,
        structural_closure_root.map(StructuralQuotientClosureRoot::bytes),
    );
    MechanismSupportFrontierRoot(encoder.finish())
}

fn derive_starter_projection_plan_id(
    slice: MechanismSupportSlice,
    structural_root: StructuralQuotientClosureRoot,
    support_root: MechanismSupportClosureRoot,
    support_expression_bounds: MechanismSupportExpressionBounds,
) -> MechanismStarterProjectionPlanId {
    let mut encoder = SupportEncoder::new(if slice.enclosing_mechanism().is_some() {
        SUPPORT_SLICE_STARTER_PROJECTION_PLAN_ID_V2
    } else {
        STARTER_PROJECTION_PLAN_ID_V3
    });
    encoder.u32(MECHANISM_STARTER_PROJECTION_PLAN_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(structural_root.bytes());
    encoder.digest(support_root.bytes());
    encode_support_expression_bounds(&mut encoder, support_expression_bounds);
    MechanismStarterProjectionPlanId(encoder.finish())
}

fn derive_factorized_inner_fiber_expr_root(
    slice: MechanismSupportSlice,
    structural_root: StructuralQuotientClosureRoot,
    signature_fiber_root: [u8; 32],
) -> MechanismSupportFiberExprRoot {
    let mut encoder = support_fiber_expr_encoder(slice, FIBER_EXPR_FACTORIZED_SUBJECT_UNION);
    encoder.digest(structural_root.bytes());
    encoder.digest(signature_fiber_root);
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_materialized_inner_fiber_expr_root(
    slice: MechanismSupportSlice,
    correlated_starter_root: [u8; 32],
) -> MechanismSupportFiberExprRoot {
    let mut encoder = support_fiber_expr_encoder(slice, FIBER_EXPR_MATERIALIZED_PROJECTION);
    encoder.digest(correlated_starter_root);
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_outer_fiber_expr_root(
    slice: MechanismSupportSlice,
    inner: MechanismSupportFiberExprRoot,
    shared_residual_root: MechanismSupportResidualRoot,
    shared_residual_case_count: u128,
    opaque_undiscovered_target: bool,
) -> MechanismSupportFiberExprRoot {
    if shared_residual_case_count == 0 && !opaque_undiscovered_target {
        return inner;
    }
    let mut encoder = support_fiber_expr_encoder(slice, FIBER_EXPR_POSSIBLE_SUPPORT_ENVELOPE);
    encoder.digest(inner.bytes());
    encoder.digest(shared_residual_root.bytes());
    encoder.u128(shared_residual_case_count);
    encoder.u8(u8::from(opaque_undiscovered_target));
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_factorized_subject_starter_projection_expr_root(
    slice: MechanismSupportSlice,
    structural_root: StructuralQuotientClosureRoot,
    signature_starter_set_root: [u8; 32],
) -> MechanismStarterProjectionExprRoot {
    let mut encoder = SupportEncoder::new(STARTER_PROJECTION_EXPR_FACTORIZED_SUBJECT_V1);
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(structural_root.bytes());
    encoder.digest(signature_starter_set_root);
    MechanismStarterProjectionExprRoot(encoder.finish())
}

fn derive_factorized_observation_starter_projection_expr_root(
    slice: MechanismSupportSlice,
    starter_prefix_root: [u8; 32],
) -> MechanismStarterProjectionExprRoot {
    let mut encoder = SupportEncoder::new(STARTER_PROJECTION_EXPR_OBSERVATION_PREFIX_V1);
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(starter_prefix_root);
    MechanismStarterProjectionExprRoot(encoder.finish())
}

fn derive_materialized_starter_projection_expr_root(
    slice: MechanismSupportSlice,
    starter_set_root: [u8; 32],
) -> MechanismStarterProjectionExprRoot {
    let mut encoder = SupportEncoder::new(STARTER_PROJECTION_EXPR_MATERIALIZED_V1);
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(starter_set_root);
    MechanismStarterProjectionExprRoot(encoder.finish())
}

fn derive_starter_projection_outer_expr_root(
    slice: MechanismSupportSlice,
    starter_inner_root: MechanismStarterProjectionExprRoot,
    case_outer_root: MechanismSupportFiberExprRoot,
    target_starter_set_root: [u8; 32],
    target_frontier_open: bool,
    starter_set_status: MechanismStarterSetStatus,
) -> MechanismStarterProjectionExprRoot {
    if starter_set_status.is_exact() {
        return starter_inner_root;
    }
    let mut encoder = SupportEncoder::new(if target_frontier_open {
        STARTER_PROJECTION_EXPR_OPAQUE_UPPER_V1
    } else {
        STARTER_PROJECTION_EXPR_TARGET_ENVELOPE_V1
    });
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    if target_frontier_open {
        // The outer S expression commits the opaque undiscovered-target
        // obligation. Its projection is an abstract upper expression rather
        // than an enumerated finite starter set.
        encoder.digest(starter_inner_root.bytes());
        encoder.digest(case_outer_root.bytes());
    } else {
        // Every possible source belongs to the sealed target's canonical
        // distinct SourceKey set. This is a conservative envelope when the
        // subject-specific union has not itself closed.
        encoder.digest(target_starter_set_root);
    }
    MechanismStarterProjectionExprRoot(encoder.finish())
}

#[allow(clippy::too_many_arguments)]
fn derive_support_expression_bounds(
    slice: MechanismSupportSlice,
    case_inner_root: MechanismSupportFiberExprRoot,
    case_outer_root: MechanismSupportFiberExprRoot,
    starter_inner_root: MechanismStarterProjectionExprRoot,
    target_starter_set_root: [u8; 32],
    target_frontier_open: bool,
    starter_set_is_exact: bool,
    correlated_support_is_exact: bool,
) -> Result<MechanismSupportExpressionBounds, MechanismSupportError> {
    let correlated_support_status = if correlated_support_is_exact {
        MechanismCorrelatedSupportStatus::ExactCorrelatedSupport
    } else {
        MechanismCorrelatedSupportStatus::Open
    };
    let starter_set_status = if starter_set_is_exact || correlated_support_is_exact {
        MechanismStarterSetStatus::ExactStarterSet
    } else {
        MechanismStarterSetStatus::Open
    };
    let starter_outer_root = derive_starter_projection_outer_expr_root(
        slice,
        starter_inner_root,
        case_outer_root,
        target_starter_set_root,
        target_frontier_open,
        starter_set_status,
    );
    MechanismSupportExpressionBounds::checked(
        case_inner_root,
        case_outer_root,
        starter_inner_root,
        starter_outer_root,
        starter_set_status,
        correlated_support_status,
    )
}

fn support_fiber_expr_encoder(slice: MechanismSupportSlice, expression_kind: u8) -> SupportEncoder {
    let mut encoder = SupportEncoder::new(if slice.enclosing_mechanism().is_some() {
        SUPPORT_SLICE_FIBER_EXPR_ROOT_V1
    } else {
        SUPPORT_FIBER_EXPR_ROOT_V1
    });
    encoder.u32(MECHANISM_SUPPORT_FIBER_EXPR_VERSION);
    // Coordinate contract: origin SourceKey `(Context, Before)` mapped to a
    // set of SuccessorKey `After` members in the request's typed relation.
    encoder.u8(FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE);
    encoder.u8(FIBER_EXPR_SOURCE_CONTEXT_BEFORE);
    encoder.u8(FIBER_EXPR_SUCCESSOR_AFTER);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.u8(expression_kind);
    encoder
}

#[allow(clippy::too_many_arguments)]
fn derive_factorized_observation_inner_fiber_expr_root(
    slice: MechanismSupportSlice,
    imported_prefix_root: [u8; 32],
    signature_prefix_root: [u8; 32],
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    confirmed_case_count: u128,
    confirmed_starter_lower_bound: u128,
) -> MechanismSupportFiberExprRoot {
    let mut encoder = SupportEncoder::new(FACTORIZED_SUPPORT_OBSERVATION_INNER_FIBER_EXPR_ROOT_V2);
    encoder.u32(MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION);
    encoder.u8(FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE);
    encoder.u8(FIBER_EXPR_SOURCE_CONTEXT_BEFORE);
    encoder.u8(FIBER_EXPR_SUCCESSOR_AFTER);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(imported_prefix_root);
    encoder.digest(signature_prefix_root);
    encoder.u128(contributing_signature_count);
    encoder.u128(inspected_signature_count);
    encoder.u128(confirmed_case_count);
    encoder.u128(confirmed_starter_lower_bound);
    MechanismSupportFiberExprRoot(encoder.finish())
}

fn derive_factorized_observation_outer_fiber_expr_root(
    slice: MechanismSupportSlice,
    inner: MechanismSupportFiberExprRoot,
    frontier_root: MechanismSupportFrontierRoot,
    residual: MechanismSupportResidualSummary,
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    signature_scan_complete: bool,
    target_frontier_open: bool,
    support_is_closed: bool,
) -> MechanismSupportFiberExprRoot {
    if signature_scan_complete
        && residual.case_count() == 0
        && !target_frontier_open
        && support_is_closed
    {
        return inner;
    }
    let mut encoder = SupportEncoder::new(FACTORIZED_SUPPORT_OBSERVATION_OUTER_FIBER_EXPR_ROOT_V2);
    encoder.u32(MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION);
    encoder.u8(FIBER_EXPR_ORIGIN_PREIMAGE_COORDINATE);
    encoder.u8(FIBER_EXPR_SOURCE_CONTEXT_BEFORE);
    encoder.u8(FIBER_EXPR_SUCCESSOR_AFTER);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(inner.bytes());
    encoder.digest(frontier_root.bytes());
    encode_residual_summary(&mut encoder, residual);
    encoder.u128(contributing_signature_count);
    encoder.u128(inspected_signature_count);
    encoder.u8(u8::from(signature_scan_complete));
    encoder.u8(u8::from(target_frontier_open));
    encoder.u8(u8::from(support_is_closed));
    MechanismSupportFiberExprRoot(encoder.finish())
}

#[allow(clippy::too_many_arguments)]
fn derive_factorized_support_observation_summary_root(
    slice: MechanismSupportSlice,
    slice_id: MechanismSupportSliceId,
    frontier: MechanismSupportFrontierSummary,
    structural_root: Option<StructuralQuotientClosureRoot>,
    support_root: Option<MechanismSupportClosureRoot>,
    projection_plan_id: Option<MechanismStarterProjectionPlanId>,
    target_frontier_open: bool,
    support_expression_bounds: MechanismSupportExpressionBounds,
    contributing_signature_count: u128,
    inspected_signature_count: u128,
    signature_scan_complete: bool,
    signature_prefix_root: [u8; 32],
    residual: MechanismSupportResidualSummary,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_bound_basis: MechanismFactorizedStarterBoundBasis,
) -> MechanismFactorizedSupportObservationSummaryRoot {
    let mut encoder = SupportEncoder::new(FACTORIZED_SUPPORT_OBSERVATION_SUMMARY_ROOT_V2);
    encoder.u32(MECHANISM_FACTORIZED_SUPPORT_OBSERVATION_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder.digest(slice_id.bytes());
    encoder.digest(frontier.root().bytes());
    encoder.digest(frontier.imported_prefix_root());
    encode_optional_digest(
        &mut encoder,
        structural_root.map(StructuralQuotientClosureRoot::bytes),
    );
    encode_optional_digest(
        &mut encoder,
        support_root.map(MechanismSupportClosureRoot::bytes),
    );
    encode_optional_digest(
        &mut encoder,
        projection_plan_id.map(MechanismStarterProjectionPlanId::bytes),
    );
    encoder.u8(u8::from(target_frontier_open));
    encode_support_expression_bounds(&mut encoder, support_expression_bounds);
    encoder.u128(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128);
    encoder.u128(contributing_signature_count);
    encoder.u128(inspected_signature_count);
    encoder.u8(u8::from(signature_scan_complete));
    encoder.digest(signature_prefix_root);
    encode_residual_summary(&mut encoder, residual);
    encode_count(&mut encoder, case_count);
    encode_count(&mut encoder, starter_count);
    encode_factorized_starter_bound_basis(&mut encoder, starter_bound_basis);
    MechanismFactorizedSupportObservationSummaryRoot(encoder.finish())
}

#[allow(clippy::too_many_arguments)]
fn derive_factorized_subject_summary_root(
    slice: MechanismSupportSlice,
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
    support_expression_bounds: MechanismSupportExpressionBounds,
) -> MechanismFactorizedSubjectSummaryRoot {
    let mut encoder = SupportEncoder::new(if slice.enclosing_mechanism().is_some() {
        FACTORIZED_SUPPORT_SLICE_SUMMARY_ROOT_V2
    } else {
        FACTORIZED_SUBJECT_SUMMARY_ROOT_V3
    });
    encoder.u32(MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
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
    encode_support_expression_bounds(&mut encoder, support_expression_bounds);
    MechanismFactorizedSubjectSummaryRoot(encoder.finish())
}

fn factorized_support_slice_signature_prefix_encoder(
    slice: MechanismSupportSlice,
) -> SupportEncoder {
    let mut encoder = SupportEncoder::new(if slice.enclosing_mechanism().is_some() {
        FACTORIZED_SUPPORT_SLICE_SIGNATURE_PREFIX_ROOT_V2
    } else {
        FACTORIZED_SUBJECT_SIGNATURE_PREFIX_ROOT_V3
    });
    encoder.u32(MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION);
    encode_total_or_conditioned_support_slice(&mut encoder, slice);
    encoder
}

fn encode_total_or_conditioned_support_slice(
    encoder: &mut SupportEncoder,
    slice: MechanismSupportSlice,
) {
    encode_support_key(encoder, slice.key());
    if let Some(mechanism_id) = slice.enclosing_mechanism() {
        // The conditioned form has a distinct domain separator. The tag
        // leaves room for additional selector forms without reinterpreting
        // this mechanism-route identity.
        encoder.u8(0x01);
        encoder.digest(mechanism_id.bytes());
    }
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
    structural_authority: MechanismSupportViewStructuralAuthority,
    terminal_fact_root: [u8; 32],
    signature_fiber_root: [u8; 32],
    target_starter_root: [u8; 32],
    inner_signature_root: [u8; 32],
    inner_case_root: [u8; 32],
    inner_starter_root: [u8; 32],
    shared_residual_root: MechanismSupportResidualRoot,
    target_frontier_open: bool,
    support_expression_bounds: MechanismSupportExpressionBounds,
    case_count: MechanismSupportCount,
    starter_count: MechanismSupportCount,
    starter_upper_provenance: MechanismStarterUpperProvenance,
) -> MechanismSupportViewRoot {
    let mut encoder = SupportEncoder::new(SUPPORT_VIEW_ROOT_V6);
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
    match structural_authority {
        MechanismSupportViewStructuralAuthority::OpenPrefix {
            assignment_cursor,
            prefix_revision,
        } => {
            encoder.u8(0x01);
            encoder.u128(assignment_cursor as u128);
            encoder.digest(prefix_revision.bytes());
        }
        MechanismSupportViewStructuralAuthority::Closed {
            structural_root,
            assignment_root,
            assignment_count,
        } => {
            encoder.u8(0x02);
            encoder.digest(structural_root.bytes());
            encoder.digest(assignment_root);
            encoder.u128(assignment_count as u128);
        }
    }
    encoder.digest(terminal_fact_root);
    encoder.digest(signature_fiber_root);
    encoder.digest(target_starter_root);
    encoder.digest(inner_signature_root);
    encoder.digest(inner_case_root);
    encoder.digest(inner_starter_root);
    encoder.digest(shared_residual_root.bytes());
    encode_support_expression_bounds(&mut encoder, support_expression_bounds);
    encode_count(&mut encoder, case_count);
    encode_count(&mut encoder, starter_count);
    encode_starter_upper_provenance(&mut encoder, starter_upper_provenance);
    MechanismSupportViewRoot(encoder.finish())
}

fn encode_support_expression_bounds(
    encoder: &mut SupportEncoder,
    bounds: MechanismSupportExpressionBounds,
) {
    encoder.u32(MECHANISM_SUPPORT_FIBER_EXPR_VERSION);
    encoder.digest(bounds.case_inner_root().bytes());
    encoder.digest(bounds.case_outer_root().bytes());
    encoder.u32(MECHANISM_STARTER_PROJECTION_EXPR_VERSION);
    encoder.digest(bounds.starter_inner_root().bytes());
    encoder.digest(bounds.starter_outer_root().bytes());
    encoder.u8(match bounds.starter_set_status() {
        MechanismStarterSetStatus::Open => 0x00,
        MechanismStarterSetStatus::ExactStarterSet => 0x01,
    });
    encoder.u8(match bounds.correlated_support_status() {
        MechanismCorrelatedSupportStatus::Open => 0x00,
        MechanismCorrelatedSupportStatus::ExactCorrelatedSupport => 0x01,
    });
}

fn encode_optional_digest(encoder: &mut SupportEncoder, digest: Option<[u8; 32]>) {
    match digest {
        Some(digest) => {
            encoder.u8(0x01);
            encoder.digest(digest);
        }
        None => encoder.u8(0x00),
    }
}

fn encode_authenticated_index(encoder: &mut SupportEncoder, index: &AuthenticatedTreapMap) {
    encoder.digest(index.root_hash());
    encoder.u128(index.entry_count());
    encoder.u128(index.total_weight());
}

fn encode_residual_summary(
    encoder: &mut SupportEncoder,
    residual: MechanismSupportResidualSummary,
) {
    encoder.digest(residual.root().bytes());
    encoder.u128(residual.case_count());
    for component in [
        residual.pending_cases(),
        residual.unavailable_cases(),
        residual.unassigned_signatures(),
    ] {
        encoder.digest(component.root().bytes());
        encoder.u128(component.member_count());
        encoder.u128(component.case_count());
    }
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
        MechanismFactorizedStarterBoundBasis::OpenOpaque => encoder.u8(0x00),
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
    ExplicitObservationBackfillPending,
    ExplicitObservationBackfillPageTooLarge,
    ExplicitObservationRegistryConflict,
    InvalidExplicitObservationRoute,
    FrontierConflict,
    UnknownStructuralAssignment,
    UnknownStructuralSubject,
    TerminalConflict,
    SignatureConflict,
    ResidualPartitionConflict,
    SupportExpressionBoundsConflict,
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
            Self::ExplicitObservationBackfillPending => {
                "mechanism support explicit observation backfill is still pending"
            }
            Self::ExplicitObservationBackfillPageTooLarge => {
                "mechanism support explicit observation backfill page exceeds its protocol bound"
            }
            Self::ExplicitObservationRegistryConflict => {
                "mechanism support explicit observation registry is internally inconsistent"
            }
            Self::InvalidExplicitObservationRoute => {
                "mechanism support explicit observation route is not structurally addressable"
            }
            Self::FrontierConflict => {
                "mechanism support observation does not match the supplied durable frontier"
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
            Self::SupportExpressionBoundsConflict => {
                "mechanism support expression bounds contain an invalid exactness claim"
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
    pre_structural_support: MechanismSupportCatalogBuilder,
    pub(super) open_support: MechanismSupportCatalogBuilder,
    pub(super) support: MechanismSupportCatalogBuilder,
    open_incidence: super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
    incidence: super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
    pub(super) structural: StructuralMechanismCatalogBuilder,
    pub(super) mechanism_id: StructuralMechanismId,
    pub(super) mechanism_ids: Box<[StructuralMechanismId]>,
    pub(super) node_ids: Box<[StructuralNodeId]>,
    pub(super) edge_ids: Box<[StructuralEdgeId]>,
}

/// Small closed two-case quotient used by support/projection unit tests. Its
/// two nodes and one edge occur on both endpoints with equal outcomes, so
/// activation support is exact two while differential support is exact empty.
#[cfg(test)]
pub(super) fn closed_subject_starter_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(false, false, 1, false)
}

/// Two complete raw signatures which quotient to the same structural subject
/// support and whose cases are distinct successors of one shared origin.
#[cfg(test)]
fn multi_signature_shared_starter_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(false, true, 2, false)
}

/// Two raw signatures on different mechanism routes. The second route extends
/// the same base DAG, so both mechanisms retain stable membership in at least
/// one common node while their structural mechanism identities differ.
#[cfg(test)]
fn multi_mechanism_shared_node_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(false, true, 2, true)
}

/// One more raw signature than an automatic observation may inspect. Every
/// signature has one successful case with a distinct starter and all of them
/// quotient to the same structural mechanism.
#[cfg(test)]
fn capped_automatic_observation_fixture() -> ClosedSubjectStarterFixture {
    subject_starter_fixture(
        false,
        false,
        AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT + 1,
        false,
    )
}

/// Variant of the shared subject fixture which can leave the second case
/// permanently unavailable and can make both cases share one origin starter.
/// `open_support` is captured before the exact target seal is attached, while
/// `support` is the subsequently closed view over the same semantic stream.
#[cfg(test)]
fn subject_starter_fixture(
    second_case_unavailable: bool,
    shared_starter: bool,
    raw_signature_count: usize,
    split_structural_mechanisms: bool,
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

    assert!(raw_signature_count > 0);
    assert!(!second_case_unavailable || raw_signature_count == 1);
    assert!(!split_structural_mechanisms || raw_signature_count == 2);

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
    let case_count = raw_signature_count.max(2);
    if shared_starter {
        let source_key = relation
            .insert_source(SourceRow::new(
                ExploreValue::Int(0),
                ExploreValue::Int(100),
                provenance(b"fixture-shared-source"),
            ))
            .expect("fixture shared source");
        for ordinal in 0..case_count {
            let after = 101_i64
                .checked_add(i64::try_from(ordinal).expect("fixture ordinal fits i64"))
                .expect("fixture successor value fits i64");
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
        for ordinal in 0..case_count {
            let before = i64::try_from(ordinal + 1)
                .expect("fixture ordinal fits i64")
                .checked_mul(100)
                .expect("fixture before value fits i64");
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
    for ordinal in 1..raw_signature_count {
        let canonical_definition = if ordinal == 1 {
            b"subject-starter-structural-fixture-second".to_vec()
        } else {
            format!("subject-starter-structural-fixture-{ordinal}").into_bytes()
        };
        signatures.push(MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            canonical_definition,
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
            let signature = &signatures[if raw_signature_count > 1 { ordinal } else { 0 }];
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
        .enumerate()
        .map(|(ordinal, signature)| {
            let extended_route = split_structural_mechanisms && ordinal == 1;
            let mut artifact_occurrences = occurrences.to_vec();
            let artifact_edges = vec![(0, 1)];
            if extended_route {
                let then_outcome = Some(RelationalMechanismEventOutcome::IfDecision(
                    RelationalIfDecisionOutcome::Then,
                ));
                artifact_occurrences.push(StructuralOccurrenceInputV1 {
                    before_owner_activation: Some(0),
                    after_owner_activation: Some(0),
                    site: expression_site(&program, "fixture_route_extension", 14),
                    kind: RelationalMechanismEventKind::IfDecision,
                    before_outcome: then_outcome.clone(),
                    after_outcome: then_outcome,
                    before_root: true,
                    after_root: true,
                });
            }
            let mut budget = relational_structural_derivation_budget();
            budget.admit_source(0).expect("fixture source budget");
            budget
                .admit_activations(2)
                .expect("fixture activation budget");
            budget
                .admit_occurrences(if extended_route { 6 } else { 4 })
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
                    occurrences: artifact_occurrences.into_boxed_slice(),
                    before_edges: artifact_edges.clone().into_boxed_slice(),
                    after_edges: artifact_edges.into_boxed_slice(),
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
    let mechanism_ids = artifacts
        .iter()
        .map(|artifact| artifact.mechanism().id())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    assert_eq!(
        mechanism_ids.len(),
        if split_structural_mechanisms { 2 } else { 1 }
    );
    let node_ids = first_artifact
        .node_membership()
        .iter()
        .copied()
        .filter(|node_id| {
            artifacts
                .iter()
                .all(|artifact| artifact.node_membership().binary_search(node_id).is_ok())
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let edge_ids = first_artifact
        .edge_membership()
        .iter()
        .copied()
        .filter(|edge_id| {
            artifacts
                .iter()
                .all(|artifact| artifact.edge_membership().binary_search(edge_id).is_ok())
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    assert!(!node_ids.is_empty());
    assert!(!edge_ids.is_empty());
    let mut structural = StructuralMechanismCatalogBuilder::new(request_id);
    for artifact in &artifacts {
        if !split_structural_mechanisms {
            assert_eq!(artifact.mechanism().id(), mechanism_id);
            assert_eq!(artifact.node_membership(), node_ids.as_ref());
            assert_eq!(artifact.edge_membership(), edge_ids.as_ref());
        }
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
    let pre_structural_support = support.clone();
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
    let open_incidence = incidence.clone();
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
        pre_structural_support,
        open_support,
        support,
        open_incidence,
        incidence,
        structural,
        mechanism_id,
        mechanism_ids,
        node_ids,
        edge_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_observation_scheduler_settlement_covers_every_work_lane() {
        let summary = |pending_backfill, dirty, unsealed| {
            MechanismExplicitObservationSchedulerSummary::restore_from_journal_codec(
                [0x11; 32],
                1,
                1_u128.saturating_sub(pending_backfill),
                [0x22; 32],
                pending_backfill,
                [0x33; 32],
                dirty,
                [0x44; 32],
                unsealed,
            )
        };

        assert!(summary(0, 0, 0).is_fully_settled());
        assert!(!summary(1, 0, 1).is_fully_settled());
        assert!(!summary(0, 1, 1).is_fully_settled());
        assert!(!summary(0, 0, 1).is_fully_settled());
    }

    fn mechanism_support_key(fixture: &ClosedSubjectStarterFixture) -> MechanismSupportKey {
        MechanismSupportKey::new(
            fixture.support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        )
    }

    #[test]
    fn automatic_observation_registry_tracks_each_mechanism_and_coalesces_dirtiness() {
        let fixture = multi_mechanism_shared_node_fixture();
        let mut support = fixture.pre_structural_support;
        assert_eq!(support.automatic_observation_slice_count(), 0);
        assert_eq!(
            support
                .next_automatic_observation_slice_after(None)
                .expect("empty automatic registry"),
            None
        );

        support
            .sync_structural_assignments_through(&fixture.structural, 1)
            .expect("first mechanism assignment");
        let first_prefix = support.automatic_observation_scheduler_summary();
        assert_eq!(first_prefix.registry().slice_count(), 1);
        assert_eq!(first_prefix.registry().indexed_assignment_count(), 1);
        assert_eq!(first_prefix.dirty().slice_count(), 1);

        support
            .sync_structural_assignments_through(&fixture.structural, 2)
            .expect("second mechanism assignment");
        let complete_prefix = support.automatic_observation_scheduler_summary();
        assert_eq!(complete_prefix.registry().slice_count(), 2);
        assert_eq!(complete_prefix.registry().indexed_assignment_count(), 2);
        assert_eq!(complete_prefix.dirty().slice_count(), 2);

        let first = support
            .next_automatic_observation_slice_after(None)
            .expect("automatic registry head")
            .expect("first automatic mechanism");
        let second = support
            .next_automatic_observation_slice_after(Some(first))
            .expect("automatic registry successor")
            .expect("second automatic mechanism");
        assert_ne!(first, second);
        assert_eq!(
            support
                .next_automatic_observation_slice_after(Some(second))
                .expect("automatic registry tail"),
            None
        );
        assert!(support.automatic_observation_contains(first));
        assert!(support.automatic_observation_contains(second));

        // Successful terminals alter each touched mechanism, but both slices
        // are already dirty, so repeated dirtiness coalesces to the same set.
        support
            .sync_incidence_terminals_through(
                &fixture.open_incidence,
                &fixture.structural,
                fixture.open_incidence.terminal_discovery_count() as u128,
            )
            .expect("successful terminal prefix");
        assert_eq!(
            support.dirty_automatic_observation_summary().slice_count(),
            2
        );
        let frontier_before_ack = support
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("frontier before operational acknowledgement");
        let scheduler_before_ack = support.automatic_observation_scheduler_summary();

        let prepared = support
            .prepare_automatic_observation_ack(first)
            .expect("prepare first mechanism observation acknowledgement");
        assert_eq!(prepared.prior_dirty_summary().slice_count(), 2);
        assert_eq!(prepared.next_dirty_summary().slice_count(), 1);
        support.commit_automatic_observation_ack(prepared);
        assert_eq!(
            support.next_dirty_automatic_observation_slice(),
            Some(second)
        );
        let frontier_after_ack = support
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("frontier after operational acknowledgement");
        let scheduler_after_ack = support.automatic_observation_scheduler_summary();
        assert_eq!(frontier_after_ack, frontier_before_ack);
        assert_eq!(
            scheduler_after_ack.registry(),
            scheduler_before_ack.registry()
        );
        assert_ne!(scheduler_after_ack.dirty(), scheduler_before_ack.dirty());
    }

    #[test]
    fn explicit_node_and_edge_demands_backfill_canonically_and_only_incident_slices_redirty() {
        let fixture = subject_starter_fixture(false, false, 3, false);
        let mut support = fixture.pre_structural_support;
        support
            .sync_structural_assignments_through(&fixture.structural, 2)
            .expect("two-assignment structural prefix");

        let scope = support.scope();
        let node_slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            scope,
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
        ));
        let nonincident_edge_slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            scope,
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::DifferentialParticipation,
                edge_id: fixture.edge_ids[0],
            },
        ));

        for slice in [node_slice, nonincident_edge_slice] {
            let registration = support
                .prepare_explicit_observation_demand_registration(slice, &fixture.structural)
                .expect("late explicit observation registration");
            assert_eq!(
                registration.disposition(),
                MechanismExplicitObservationRegistrationDisposition::Registered
            );
            assert_eq!(
                registration.registration_phase(),
                MechanismExplicitObservationRegistrationPhase::Open
            );
            assert_eq!(registration.registration_structural_cursor(), 2);
            support.commit_explicit_observation_demand_registration(registration);
        }
        assert_eq!(support.explicit_observation_slice_count(), 2);
        assert_eq!(support.ready_explicit_observation_slice_count(), 0);
        assert_eq!(
            support
                .pending_explicit_observation_backfill_summary()
                .slice_count(),
            2
        );
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            0
        );

        // Whole-mechanism demands alias the automatic registry, and repeated
        // explicit declarations are idempotent even while backfill is pending.
        let whole_mechanism_slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            scope,
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        ));
        let scheduler_before_alias = support.explicit_observation_scheduler_summary();
        let alias = support
            .prepare_explicit_observation_demand_registration(
                whole_mechanism_slice,
                &fixture.structural,
            )
            .expect("whole-mechanism observation alias");
        assert_eq!(
            alias.disposition(),
            MechanismExplicitObservationRegistrationDisposition::AutomaticWholeMechanism
        );
        support.commit_explicit_observation_demand_registration(alias);
        assert_eq!(
            support.explicit_observation_scheduler_summary(),
            scheduler_before_alias
        );
        let duplicate = support
            .prepare_explicit_observation_demand_registration(node_slice, &fixture.structural)
            .expect("duplicate explicit observation registration");
        assert_eq!(
            duplicate.disposition(),
            MechanismExplicitObservationRegistrationDisposition::AlreadyRegistered
        );
        support.commit_explicit_observation_demand_registration(duplicate);
        assert_eq!(
            support.explicit_observation_scheduler_summary(),
            scheduler_before_alias
        );

        let page_size = NonZeroU16::new(1).expect("nonzero page size");
        let first_slice = support
            .next_pending_explicit_observation_slice()
            .expect("canonical first pending slice");
        let first_page = support
            .prepare_next_explicit_observation_backfill(&fixture.structural, page_size)
            .expect("first bounded backfill page")
            .expect("pending backfill page");
        assert_eq!(first_page.slice(), first_slice);
        assert_eq!(first_page.from_structural_cursor(), 0);
        assert_eq!(first_page.through_structural_cursor(), 1);
        assert!(!first_page.completed());
        support.commit_explicit_observation_backfill(first_page);
        assert_eq!(
            support.next_pending_explicit_observation_slice(),
            Some(first_slice)
        );
        assert!(!support.ready_explicit_observation_contains(first_slice));
        assert!(matches!(
            support.observation_index_for_slice(first_slice),
            Err(MechanismSupportError::ExplicitObservationBackfillPending)
        ));

        let second_page = support
            .prepare_next_explicit_observation_backfill(&fixture.structural, page_size)
            .expect("second bounded backfill page")
            .expect("same canonical pending slice");
        assert_eq!(second_page.slice(), first_slice);
        assert_eq!(second_page.from_structural_cursor(), 1);
        assert_eq!(second_page.through_structural_cursor(), 2);
        assert!(second_page.completed());
        support.commit_explicit_observation_backfill(second_page);
        assert!(support.ready_explicit_observation_contains(first_slice));
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            1
        );

        let second_slice = support
            .next_pending_explicit_observation_slice()
            .expect("canonical second pending slice");
        assert_ne!(second_slice, first_slice);
        for (from, through, completed) in [(0, 1, false), (1, 2, true)] {
            let page = support
                .prepare_next_explicit_observation_backfill(&fixture.structural, page_size)
                .expect("bounded second-slice backfill")
                .expect("second-slice backfill page");
            assert_eq!(page.slice(), second_slice);
            assert_eq!(page.from_structural_cursor(), from);
            assert_eq!(page.through_structural_cursor(), through);
            assert_eq!(page.completed(), completed);
            support.commit_explicit_observation_backfill(page);
        }
        assert_eq!(support.ready_explicit_observation_slice_count(), 2);
        assert_eq!(
            support
                .pending_explicit_observation_backfill_summary()
                .slice_count(),
            0
        );
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            2
        );

        while let Some(slice) = support.next_dirty_explicit_observation_slice() {
            let acknowledgement = support
                .prepare_explicit_observation_ack(slice)
                .expect("open explicit observation acknowledgement");
            support.commit_explicit_observation_ack(acknowledgement);
        }
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            0
        );

        support
            .sync_structural_assignments_through(&fixture.structural, 3)
            .expect("one live structural assignment");
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            1
        );
        assert_eq!(
            support.next_dirty_explicit_observation_slice(),
            Some(node_slice)
        );
        assert_ne!(node_slice, nonincident_edge_slice);
    }

    #[test]
    fn post_closure_explicit_observation_seals_without_changing_automatic_authority() {
        let fixture = closed_subject_starter_fixture();
        let mut support = fixture.support;
        let structural_root = fixture
            .structural
            .closure()
            .expect("fixture structural closure")
            .root();
        let incidence_root = fixture
            .incidence
            .closed_ref()
            .expect("fixture incidence closure")
            .root();
        let support_root = support.closure().expect("fixture support closure").root();
        let automatic_before = support.automatic_observation_scheduler_summary();
        let frontier_before = support
            .checkpoint_frontier(
                &fixture.incidence,
                Some(incidence_root),
                &fixture.structural,
                Some(structural_root),
            )
            .expect("sealed support frontier before late reader");

        let slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            support.scope(),
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
        ));
        let registration = support
            .prepare_explicit_observation_demand_registration(slice, &fixture.structural)
            .expect("post-closure explicit observation registration");
        assert_eq!(
            registration.registration_phase(),
            MechanismExplicitObservationRegistrationPhase::Sealed { support_root }
        );
        support.commit_explicit_observation_demand_registration(registration);
        let backfill = support
            .prepare_next_explicit_observation_backfill(
                &fixture.structural,
                NonZeroU16::new(1).expect("nonzero page size"),
            )
            .expect("post-closure backfill")
            .expect("post-closure backfill page");
        assert!(backfill.completed());
        support.commit_explicit_observation_backfill(backfill);
        assert!(support.ready_explicit_observation_contains(slice));
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            1
        );
        assert_eq!(
            support
                .unsealed_explicit_observation_summary()
                .slice_count(),
            1
        );

        let sealed = support
            .derive_factorized_support_observation(slice, frontier_before, &fixture.structural)
            .expect("sealed late observation");
        assert_eq!(sealed.support_root(), Some(support_root));
        assert!(!sealed.target_frontier_is_open());
        let acknowledgement = support
            .prepare_explicit_observation_seal_ack(slice)
            .expect("direct sealed observation acknowledgement");
        assert!(acknowledgement.retired_dirty_observation());
        support.commit_explicit_observation_seal_ack(acknowledgement);
        assert_eq!(
            support.dirty_explicit_observation_summary().slice_count(),
            0
        );
        assert_eq!(
            support
                .unsealed_explicit_observation_summary()
                .slice_count(),
            0
        );

        assert_eq!(
            support.automatic_observation_scheduler_summary(),
            automatic_before
        );
        assert_eq!(
            support.closure().map(|closure| closure.root()),
            Some(support_root)
        );
        let frontier_after = support
            .checkpoint_frontier(
                &fixture.incidence,
                Some(incidence_root),
                &fixture.structural,
                Some(structural_root),
            )
            .expect("sealed support frontier after late reader");
        assert_eq!(frontier_after, frontier_before);
    }

    #[test]
    fn open_mechanism_support_stream_observes_only_the_imported_structural_prefix() {
        let fixture = closed_subject_starter_fixture();
        let mut support = fixture.pre_structural_support;
        let key = MechanismSupportKey::new(
            support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        );
        let structural_closure = fixture
            .structural
            .closure()
            .expect("fixture structural closure")
            .root();
        let incidence_closure = fixture
            .incidence
            .closed_ref()
            .expect("fixture incidence closure")
            .root();
        let checkpoint = |support: &mut MechanismSupportCatalogBuilder| {
            support
                .checkpoint_frontier(
                    &fixture.incidence,
                    Some(incidence_closure),
                    &fixture.structural,
                    Some(structural_closure),
                )
                .expect("support checkpoint")
        };
        let before_cursor = support.checkpoint_cursor();
        let before_revision = support.structural_assignment_revision;
        let before_frontier = checkpoint(&mut support);

        assert_eq!(
            support.derive_view(key, &fixture.structural),
            Err(MechanismSupportError::UnknownStructuralSubject)
        );
        let unchanged_frontier = checkpoint(&mut support);
        assert_eq!(support.checkpoint_cursor(), before_cursor);
        assert_eq!(support.structural_assignment_revision, before_revision);
        assert_eq!(unchanged_frontier, before_frontier);

        assert_eq!(
            support
                .sync_structural_assignments_through(&fixture.structural, 1)
                .expect("bounded structural-prefix import"),
            1
        );
        let imported_cursor = support.checkpoint_cursor();
        let prefix_revision = fixture
            .structural
            .assignment_discovery_prefix_revision(1)
            .expect("one-assignment prefix revision");
        let imported_frontier = checkpoint(&mut support);
        assert_eq!(imported_cursor.structural_assignment(), 1);
        assert_eq!(
            support.structural_assignment_revision,
            Some(prefix_revision)
        );
        assert_ne!(
            imported_frontier.imported_prefix_root(),
            before_frontier.imported_prefix_root()
        );
        assert_eq!(
            support
                .support_view_structural_authority(&fixture.structural, 1, prefix_revision)
                .expect("open V5 structural authority"),
            MechanismSupportViewStructuralAuthority::OpenPrefix {
                assignment_cursor: 1,
                prefix_revision,
            }
        );
        assert_eq!(
            support
                .derive_view(key, &fixture.structural)
                .expect("imported structural subject")
                .key(),
            key
        );
        assert_eq!(support.checkpoint_cursor(), imported_cursor);
        assert_eq!(
            support.structural_assignment_revision,
            Some(prefix_revision)
        );
    }

    #[test]
    fn support_observation_tracks_only_its_imported_prefix_then_seals() {
        let fixture = multi_signature_shared_starter_fixture();
        let mut support = fixture.pre_structural_support;
        let key = MechanismSupportKey::new(
            support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        );
        let slice = MechanismSupportSlice::total(key);
        let conditioned_slice = MechanismSupportSlice::within_mechanism(key, fixture.mechanism_id);
        assert_ne!(slice.id(), conditioned_slice.id());

        support
            .sync_structural_assignments_through(&fixture.structural, 1)
            .expect("first structural prefix");
        support
            .sync_incidence_terminals_through(
                &fixture.open_incidence,
                &fixture.structural,
                fixture.open_incidence.terminal_discovery_count() as u128,
            )
            .expect("open terminal prefix");
        let first_frontier = support
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("first support frontier");
        let first = support
            .derive_factorized_support_observation(slice, first_frontier, &fixture.structural)
            .expect("first factorized observation");

        // The structural catalog is already closed with two assignments, but
        // only the single imported assignment is confirmed by this support
        // prefix. The second successful fiber remains one shared residual.
        assert_eq!(fixture.structural.assignment_discovery_count(), 2);
        assert_eq!(first.slice_id(), slice.id());
        assert_eq!(first.contributing_signature_count(), 1);
        assert_eq!(first.inspected_signature_count(), 1);
        assert!(first.signature_scan_complete());
        assert_eq!(
            first.case_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: 1,
            }
        );
        assert_eq!(first.residual_summary().case_count(), 1);
        assert!(first.target_frontier_is_open());
        assert_eq!(
            first.starter_bound_basis(),
            MechanismFactorizedStarterBoundBasis::OpenOpaque
        );
        assert!(first.structural_root().is_none());
        assert!(first.support_root().is_none());
        assert!(first.projection_plan_id().is_none());

        support
            .sync_structural_assignments_through(&fixture.structural, 2)
            .expect("complete structural prefix");
        let second_frontier = support
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("second support frontier");
        let second = support
            .derive_factorized_support_observation(slice, second_frontier, &fixture.structural)
            .expect("second factorized observation");
        assert_eq!(second.slice_id(), first.slice_id());
        assert_ne!(second.root(), first.root());
        assert_eq!(second.contributing_signature_count(), 2);
        assert_eq!(second.inspected_signature_count(), 2);
        assert_eq!(
            second.case_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: 2,
            }
        );
        assert_eq!(second.residual_summary().case_count(), 0);
        assert!(!second.fiber_expr_bounds_are_equal());

        support
            .close(
                fixture
                    .incidence
                    .closed_ref()
                    .expect("fixture incidence closure"),
                &fixture.structural,
            )
            .expect("support closure");
        let structural_root = fixture
            .structural
            .closure()
            .expect("structural closure")
            .root();
        let incidence_root = fixture
            .incidence
            .closed_ref()
            .expect("incidence closure")
            .root();
        let sealed_frontier = support
            .checkpoint_frontier(
                &fixture.incidence,
                Some(incidence_root),
                &fixture.structural,
                Some(structural_root),
            )
            .expect("sealed support frontier");
        let sealed = support
            .derive_factorized_support_observation(slice, sealed_frontier, &fixture.structural)
            .expect("sealed factorized observation");
        assert_eq!(sealed.slice_id(), first.slice_id());
        assert_ne!(sealed.root(), second.root());
        assert!(!sealed.target_frontier_is_open());
        assert_eq!(sealed.case_count(), MechanismSupportCount::Exact(2));
        assert_eq!(sealed.starter_count(), MechanismSupportCount::Exact(1));
        assert_eq!(sealed.structural_root(), Some(structural_root));
        assert_eq!(
            sealed.support_root(),
            support.closure().map(|root| root.root())
        );
        assert!(sealed.projection_plan_id().is_some());
        assert_eq!(
            sealed.inner_fiber_expr_root(),
            second.inner_fiber_expr_root()
        );
        assert!(sealed.fiber_expr_bounds_are_equal());
        let sealed_bounds = sealed.support_expression_bounds();
        assert!(sealed_bounds.case_bounds_are_equal());
        assert!(sealed_bounds.starter_bounds_are_equal());
        assert_eq!(
            sealed_bounds.starter_set_status(),
            MechanismStarterSetStatus::ExactStarterSet
        );
        assert_eq!(
            sealed_bounds.correlated_support_status(),
            MechanismCorrelatedSupportStatus::ExactCorrelatedSupport
        );
    }

    #[test]
    fn support_observation_cap_is_conservative_and_replay_equivalent() {
        let fixture = capped_automatic_observation_fixture();
        let signature_count = u128::try_from(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT + 1)
            .expect("automatic observation fixture size fits u128");
        assert_eq!(
            fixture.structural.assignment_discovery_count() as u128,
            signature_count
        );

        let mut incremental = fixture.pre_structural_support.clone();
        for cursor in 1..=signature_count {
            assert_eq!(
                incremental
                    .sync_structural_assignments_through(&fixture.structural, cursor)
                    .expect("incremental structural-prefix import"),
                1
            );
        }
        let mut batched = fixture.pre_structural_support.clone();
        assert_eq!(
            batched
                .sync_structural_assignments_through(&fixture.structural, signature_count)
                .expect("batched structural-prefix import") as u128,
            signature_count
        );

        let terminal_count = fixture.open_incidence.terminal_discovery_count() as u128;
        incremental
            .sync_incidence_terminals_through(
                &fixture.open_incidence,
                &fixture.structural,
                terminal_count,
            )
            .expect("incremental-path terminal import");
        batched
            .sync_incidence_terminals_through(
                &fixture.open_incidence,
                &fixture.structural,
                terminal_count,
            )
            .expect("batched-path terminal import");

        let incremental_open_frontier = incremental
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("incremental open frontier");
        let batched_open_frontier = batched
            .checkpoint_frontier(&fixture.open_incidence, None, &fixture.structural, None)
            .expect("batched open frontier");
        assert_eq!(incremental_open_frontier, batched_open_frontier);

        let slice = incremental
            .next_automatic_observation_slice_after(None)
            .expect("automatic observation registry")
            .expect("automatic observation slice");
        assert_eq!(
            batched
                .next_automatic_observation_slice_after(None)
                .expect("batched automatic observation registry"),
            Some(slice)
        );
        let incremental_open = incremental
            .derive_factorized_support_observation(
                slice,
                incremental_open_frontier,
                &fixture.structural,
            )
            .expect("incremental open observation");
        let batched_open = batched
            .derive_factorized_support_observation(
                slice,
                batched_open_frontier,
                &fixture.structural,
            )
            .expect("batched open observation");
        assert_eq!(incremental_open, batched_open);
        assert_eq!(
            incremental_open.contributing_signature_count(),
            signature_count
        );
        assert_eq!(
            incremental_open.inspected_signature_count(),
            AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128
        );
        assert!(!incremental_open.signature_scan_complete());
        assert_eq!(
            incremental_open.case_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128,
            }
        );
        assert_eq!(
            incremental_open.starter_count(),
            MechanismSupportCount::Unknown {
                confirmed_lower_bound: 1,
            }
        );
        assert_eq!(incremental_open.residual_summary().case_count(), 0);
        assert!(!incremental_open.fiber_expr_bounds_are_equal());

        incremental
            .attach_target_seal(&fixture.incidence)
            .expect("incremental target seal");
        batched
            .attach_target_seal(&fixture.incidence)
            .expect("batched target seal");
        let incremental_closure = incremental
            .close(
                fixture
                    .incidence
                    .closed_ref()
                    .expect("fixture incidence closure"),
                &fixture.structural,
            )
            .expect("incremental support closure");
        let batched_closure = batched
            .close(
                fixture
                    .incidence
                    .closed_ref()
                    .expect("fixture incidence closure"),
                &fixture.structural,
            )
            .expect("batched support closure");
        assert_eq!(incremental_closure, batched_closure);

        let structural_root = fixture
            .structural
            .closure()
            .expect("fixture structural closure")
            .root();
        let incidence_root = fixture
            .incidence
            .closed_ref()
            .expect("fixture incidence closure")
            .root();
        let incremental_sealed_frontier = incremental
            .checkpoint_frontier(
                &fixture.incidence,
                Some(incidence_root),
                &fixture.structural,
                Some(structural_root),
            )
            .expect("incremental sealed frontier");
        let batched_sealed_frontier = batched
            .checkpoint_frontier(
                &fixture.incidence,
                Some(incidence_root),
                &fixture.structural,
                Some(structural_root),
            )
            .expect("batched sealed frontier");
        assert_eq!(incremental_sealed_frontier, batched_sealed_frontier);

        let incremental_sealed = incremental
            .derive_factorized_support_observation(
                slice,
                incremental_sealed_frontier,
                &fixture.structural,
            )
            .expect("incremental sealed observation");
        let batched_sealed = batched
            .derive_factorized_support_observation(
                slice,
                batched_sealed_frontier,
                &fixture.structural,
            )
            .expect("batched sealed observation");
        assert_eq!(incremental_sealed, batched_sealed);
        assert_eq!(
            incremental_sealed.contributing_signature_count(),
            signature_count
        );
        assert_eq!(
            incremental_sealed.inspected_signature_count(),
            AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128
        );
        assert!(!incremental_sealed.signature_scan_complete());
        assert_eq!(
            incremental_sealed.case_count(),
            MechanismSupportCount::Interval {
                lower_bound: AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT as u128,
                upper_bound: signature_count,
            }
        );
        assert_eq!(
            incremental_sealed.starter_count(),
            MechanismSupportCount::Interval {
                lower_bound: 1,
                upper_bound: signature_count,
            }
        );
        assert!(matches!(
            incremental_sealed.starter_bound_basis(),
            MechanismFactorizedStarterBoundBasis::ConservativeTargetProjectionUpper { .. }
        ));
        assert!(!incremental_sealed.target_frontier_is_open());
        assert!(!incremental_sealed.fiber_expr_bounds_are_equal());
        let observation_bounds = incremental_sealed.support_expression_bounds();
        assert!(!observation_bounds.case_bounds_are_equal());
        assert!(!observation_bounds.starter_bounds_are_equal());
        assert_eq!(
            observation_bounds.starter_set_status(),
            MechanismStarterSetStatus::Open
        );
        assert_eq!(
            observation_bounds.correlated_support_status(),
            MechanismCorrelatedSupportStatus::Open
        );

        // The closed factorized authority is not limited by the publication
        // scan cap. Its S and P expressions can therefore be exact while the
        // deliberately cheap scalar cardinalities remain interval-valued.
        let closed_summary = incremental
            .derive_closed_factorized_support_slice_summary(slice, &fixture.structural)
            .expect("closed capped factorized summary");
        assert!(matches!(
            closed_summary.case_count(),
            MechanismSupportCount::Interval { .. }
        ));
        assert!(matches!(
            closed_summary.starter_count(),
            MechanismSupportCount::Interval { .. }
        ));
        let closed_bounds = closed_summary.support_expression_bounds();
        assert!(closed_bounds.case_bounds_are_equal());
        assert!(closed_bounds.starter_bounds_are_equal());
        assert_eq!(
            closed_bounds.starter_set_status(),
            MechanismStarterSetStatus::ExactStarterSet
        );
        assert_eq!(
            closed_bounds.correlated_support_status(),
            MechanismCorrelatedSupportStatus::ExactCorrelatedSupport
        );
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
        let fixture = subject_starter_fixture(true, false, 1, false);
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
        let fixture = subject_starter_fixture(true, true, 1, false);
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
        let bounds = summary.support_expression_bounds();
        assert!(!bounds.case_bounds_are_equal());
        assert!(bounds.starter_bounds_are_equal());
        assert_eq!(
            bounds.starter_set_status(),
            MechanismStarterSetStatus::ExactStarterSet
        );
        assert_eq!(
            bounds.correlated_support_status(),
            MechanismCorrelatedSupportStatus::Open
        );
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
    fn starter_projection_identity_ignores_successor_multiplicity() {
        let fixture = multi_signature_shared_starter_fixture();
        let mut support = fixture.pre_structural_support;
        support
            .sync_structural_assignments(&fixture.structural)
            .expect("shared-starter structural assignments");

        support
            .sync_incidence_terminals_through(&fixture.open_incidence, &fixture.structural, 1)
            .expect("first shared-starter successor");
        let key = MechanismSupportKey::new(
            support.scope(),
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
        );
        let first = support
            .derive_view(key, &fixture.structural)
            .expect("one-successor shared-starter view");

        support
            .sync_incidence_terminals_through(
                &fixture.open_incidence,
                &fixture.structural,
                fixture.open_incidence.terminal_discovery_count() as u128,
            )
            .expect("complete shared-starter successors");
        let second = support
            .derive_view(key, &fixture.structural)
            .expect("two-successor shared-starter view");

        assert_ne!(
            first.support_expression_bounds().case_inner_root(),
            second.support_expression_bounds().case_inner_root()
        );
        assert_eq!(
            first.support_expression_bounds().starter_inner_root(),
            second.support_expression_bounds().starter_inner_root()
        );
        assert_eq!(first.starter_count().lower_bound(), 1);
        assert_eq!(second.starter_count().lower_bound(), 1);
    }

    #[test]
    fn support_expression_exactness_requires_matching_roots() {
        let case_inner = MechanismSupportFiberExprRoot([0x11; 32]);
        let case_outer = MechanismSupportFiberExprRoot([0x12; 32]);
        let starter_inner = MechanismStarterProjectionExprRoot([0x21; 32]);
        let starter_outer = MechanismStarterProjectionExprRoot([0x22; 32]);

        let equal_but_open = MechanismSupportExpressionBounds::checked(
            case_inner,
            case_inner,
            starter_inner,
            starter_inner,
            MechanismStarterSetStatus::Open,
            MechanismCorrelatedSupportStatus::Open,
        )
        .expect("equal expression roots remain valid open bounds");
        assert!(equal_but_open.case_bounds_are_equal());
        assert!(equal_but_open.starter_bounds_are_equal());
        assert!(!equal_but_open.starter_set_status().is_exact());
        assert!(!equal_but_open.correlated_support_status().is_exact());

        assert_eq!(
            MechanismSupportExpressionBounds::checked(
                case_inner,
                case_inner,
                starter_inner,
                starter_outer,
                MechanismStarterSetStatus::ExactStarterSet,
                MechanismCorrelatedSupportStatus::Open,
            ),
            Err(MechanismSupportError::SupportExpressionBoundsConflict)
        );
        assert_eq!(
            MechanismSupportExpressionBounds::checked(
                case_inner,
                case_outer,
                starter_inner,
                starter_inner,
                MechanismStarterSetStatus::ExactStarterSet,
                MechanismCorrelatedSupportStatus::ExactCorrelatedSupport,
            ),
            Err(MechanismSupportError::SupportExpressionBoundsConflict)
        );
        assert_eq!(
            MechanismSupportExpressionBounds::checked(
                case_inner,
                case_inner,
                starter_inner,
                starter_inner,
                MechanismStarterSetStatus::Open,
                MechanismCorrelatedSupportStatus::ExactCorrelatedSupport,
            ),
            Err(MechanismSupportError::SupportExpressionBoundsConflict)
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
            assert_eq!(
                authority
                    .support_expression_bounds()
                    .correlated_support_status(),
                MechanismCorrelatedSupportStatus::ExactCorrelatedSupport
            );
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
    fn shared_node_support_can_be_sliced_by_enclosing_mechanism_route() {
        let fixture = multi_mechanism_shared_node_fixture();
        let scope = fixture.support.scope();
        let structural_closure = fixture
            .structural
            .closure()
            .expect("fixture structural closure");
        assert_eq!(structural_closure.counts().mechanisms(), 2);
        assert_eq!(fixture.mechanism_ids.len(), 2);

        let subject = MechanismSupportSubject::Node {
            facet: MechanismSupportFacet::Activation,
            node_id: fixture.node_ids[0],
        };
        let key = MechanismSupportKey::new(scope, subject);
        let total_slice = MechanismSupportSlice::total(key);
        let first_route = MechanismSupportSlice::within_mechanism(key, fixture.mechanism_ids[0]);
        let second_route = MechanismSupportSlice::within_mechanism(key, fixture.mechanism_ids[1]);

        for slice in [total_slice, first_route, second_route] {
            assert_eq!(slice.key(), key);
            assert_eq!(slice.subject(), subject);
            assert_eq!(slice.key().request_id(), scope.request_id());
            assert_eq!(slice.key().target(), scope.target());
            assert_eq!(slice.key().facet(), Some(MechanismSupportFacet::Activation));
        }
        assert_eq!(total_slice.enclosing_mechanism(), None);
        assert_eq!(
            first_route.enclosing_mechanism(),
            Some(fixture.mechanism_ids[0])
        );
        assert_eq!(
            second_route.enclosing_mechanism(),
            Some(fixture.mechanism_ids[1])
        );

        let total_summary = fixture
            .support
            .derive_closed_factorized_support_slice_summary(total_slice, &fixture.structural)
            .expect("total shared-node summary");
        let first_summary = fixture
            .support
            .derive_closed_factorized_support_slice_summary(first_route, &fixture.structural)
            .expect("first route summary");
        let second_summary = fixture
            .support
            .derive_closed_factorized_support_slice_summary(second_route, &fixture.structural)
            .expect("second route summary");

        assert_eq!(total_summary.slice(), total_slice);
        assert_eq!(total_summary.contributing_signature_count(), 2);
        assert_eq!(total_summary.case_count(), MechanismSupportCount::Exact(2));
        assert_eq!(
            total_summary.starter_count(),
            MechanismSupportCount::Exact(1)
        );
        for (summary, slice) in [(first_summary, first_route), (second_summary, second_route)] {
            assert_eq!(summary.slice(), slice);
            assert_eq!(summary.key(), key);
            assert_eq!(summary.contributing_signature_count(), 1);
            assert_eq!(summary.case_count(), MechanismSupportCount::Exact(1));
            assert_eq!(summary.starter_count(), MechanismSupportCount::Exact(1));
        }
        assert_ne!(total_summary.root(), first_summary.root());
        assert_ne!(total_summary.root(), second_summary.root());
        assert_ne!(first_summary.root(), second_summary.root());
        assert_ne!(
            first_summary.projection_plan_id(),
            second_summary.projection_plan_id()
        );
        assert_ne!(
            first_summary.inner_fiber_expr_root(),
            second_summary.inner_fiber_expr_root()
        );

        let total_authority = fixture
            .support
            .derive_closed_support_slice_starter_projection_authority(
                total_slice,
                &fixture.structural,
            )
            .expect("total shared-node authority");
        let first_authority = fixture
            .support
            .derive_closed_support_slice_starter_projection_authority(
                first_route,
                &fixture.structural,
            )
            .expect("first route authority");
        let second_authority = fixture
            .support
            .derive_closed_support_slice_starter_projection_authority(
                second_route,
                &fixture.structural,
            )
            .expect("second route authority");
        assert_eq!(total_authority.slice(), total_slice);
        assert_eq!(total_authority.exact_case_count(), 2);
        assert_eq!(first_authority.slice(), first_route);
        assert_eq!(first_authority.exact_case_count(), 1);
        assert_eq!(second_authority.slice(), second_route);
        assert_eq!(second_authority.exact_case_count(), 1);
        assert_ne!(
            first_authority.projection_plan_id(),
            second_authority.projection_plan_id()
        );

        let alternate_request_key = MechanismSupportKey {
            request_id: MechanismRequestId::from_journal_codec_bytes([0x71; 32]),
            ..key
        };
        let alternate_target_key = MechanismSupportKey {
            target: MechanismTargetId::ChosenView(
                super::super::relation::ViewId::from_journal_codec_bytes([0x72; 32]),
            ),
            ..key
        };
        let alternate_facet_key = MechanismSupportKey {
            subject: MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::DifferentialParticipation,
                node_id: fixture.node_ids[0],
            },
            ..key
        };
        let support_root = fixture.support.closure().expect("support closure").root();
        for changed_slice in [
            MechanismSupportSlice::within_mechanism(
                alternate_request_key,
                fixture.mechanism_ids[0],
            ),
            MechanismSupportSlice::within_mechanism(alternate_target_key, fixture.mechanism_ids[0]),
            MechanismSupportSlice::within_mechanism(alternate_facet_key, fixture.mechanism_ids[0]),
        ] {
            assert_ne!(
                derive_starter_projection_plan_id(
                    changed_slice,
                    structural_closure.root(),
                    support_root,
                    first_authority.support_expression_bounds(),
                ),
                first_authority.projection_plan_id()
            );
        }

        let total_page = fixture
            .support
            .closed_subject_starter_page(
                total_authority,
                &fixture.structural,
                fixture.relation_id,
                None,
                NonZeroU16::new(8).unwrap(),
            )
            .expect("total shared-node page");
        let first_page = fixture
            .support
            .closed_subject_starter_page(
                first_authority,
                &fixture.structural,
                fixture.relation_id,
                None,
                NonZeroU16::new(8).unwrap(),
            )
            .expect("first route page");
        let second_page = fixture
            .support
            .closed_subject_starter_page(
                second_authority,
                &fixture.structural,
                fixture.relation_id,
                None,
                NonZeroU16::new(8).unwrap(),
            )
            .expect("second route page");

        assert_eq!(total_page.members().len(), 2);
        assert_eq!(first_page.members().len(), 1);
        assert_eq!(second_page.members().len(), 1);
        assert!(total_page.exhausted());
        assert!(first_page.exhausted());
        assert!(second_page.exhausted());
        let total_sources = total_page
            .members()
            .iter()
            .map(|member| member.source_key())
            .collect::<BTreeSet<_>>();
        let total_successors = total_page
            .members()
            .iter()
            .map(|member| member.successor_key())
            .collect::<BTreeSet<_>>();
        let total_signatures = total_page
            .members()
            .iter()
            .map(|member| member.raw_signature_id())
            .collect::<BTreeSet<_>>();
        let first_signatures = first_page
            .members()
            .iter()
            .map(|member| member.raw_signature_id())
            .collect::<BTreeSet<_>>();
        let second_signatures = second_page
            .members()
            .iter()
            .map(|member| member.raw_signature_id())
            .collect::<BTreeSet<_>>();
        assert_eq!(total_sources.len(), 1);
        assert_eq!(total_successors.len(), 2);
        assert_eq!(total_signatures.len(), 2);
        assert!(first_signatures.is_disjoint(&second_signatures));
        assert_eq!(
            first_signatures
                .union(&second_signatures)
                .copied()
                .collect::<BTreeSet<_>>(),
            total_signatures
        );
        assert_eq!(
            first_page.members()[0].source_key(),
            total_page.members()[0].source_key()
        );
        assert_eq!(
            second_page.members()[0].source_key(),
            total_page.members()[0].source_key()
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
