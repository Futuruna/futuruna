//! Conservative source-event candidates for one checked boundary query.
//!
//! This module is deliberately downstream of name resolution and type
//! checking, but upstream of any boundary scheduler.  The extraction core does
//! **not** walk the parser AST.  Futuruna's current [`ExploreQueryIr`] retains
//! source `Expr` nodes without a resolved callable identity or a type on every
//! child node; trying to reconstruct a call graph from that IR would duplicate
//! (and eventually disagree with) compiler resolution.  The sibling checked
//! adapter therefore walks only the frozen Phase-A declaration snapshot, and
//! consumes Phase-B identities to lower the reachable slice into
//! [`ResolvedBoundaryFragment`].
//!
//! The adapter contract is intentionally strict:
//!
//! - start from the exact resolved question, validity constraints and
//!   boundary-sensitive derived values for one outer-profile assignment;
//! - retain the exact overload, module/import origin and `RuleScope` owner for
//!   every reachable call;
//! - prove every normalized arithmetic node has `Int` semantics and that its
//!   [`AffineForm`] agrees with runtime `i64` evaluation throughout the
//!   declared axis (including absence of overflow);
//! - preserve source-order dispatch, tie ownership and exact finite-table
//!   selection semantics;
//! - attach a hash-bound exact [`ResolvedAxisSupport`] certificate when
//!   downstream guards/clamps make an occurrence observationally live on only
//!   part of the axis; ordinary dynamic guard/clamp decisions remain their own
//!   source events, while a proof may end liveness where output is invariant;
//! - identify a source site by stable declaration identity plus structural AST
//!   child path; a [`Span`] is annotation only and never identity; and
//! - turn every unresolved, recursive, effectful, non-affine or otherwise
//!   unsupported reachable node into an explicit residual.  It may mark the
//!   fragment complete only after every reachable event site is either
//!   normalized or represented by such a residual.  If an unsupported parent
//!   has supported event-producing descendants, emit those descendants as
//!   sibling roots as well as the parent residual so partial extraction stays
//!   useful.
//!
//! Candidate extraction is only a scheduling optimization.  A candidate
//! closes one case only after ordinary evaluation, and this result type has no
//! constructor or field capable of claiming complement closure.  Even a
//! complete extraction cannot prove that cases between source events do not
//! match; interval/congruence certificates, SMT, or singleton fallback must do
//! that separately.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;

use super::{ExploreExactDomain, ExploreQueryIr};
use crate::Span;

#[path = "resolved_event_adapter.rs"]
mod resolved_event_adapter;

pub(super) use resolved_event_adapter::{
    adapt_checked_boundary_fragment, AdaptedBoundaryFragment, PreparedResolvedEventAdapter,
    ResolvedEventAdapterError, ResolvedEventAdapterLimits, ResolvedEventAdapterRequest,
    CHECKED_RESOLUTION_CONTRACT_SHA256, SOURCE_PROOF_ADAPTER_LIMITS_V1,
};

/// Stable semantic location of one expression in the resolved program.
///
/// `declaration_id` must already include canonical module/scope/overload
/// identity. `ast_path` is the child-index path within that declaration after
/// the compiler's identity-preserving normalization, not a byte offset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SourceSiteId {
    pub(super) declaration_id: Box<str>,
    pub(super) ast_path: Box<[u32]>,
}

/// Stable source identity plus a non-identifying source annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceSite {
    pub(super) id: SourceSiteId,
    pub(super) span: Span,
}

/// An affine integer expression `coefficient * axis + intercept`.
///
/// Construction is trusted only after the checked-callable adapter has proved
/// that this `i128` normalization is exactly equal to Futuruna's runtime `Int`
/// value over the whole bounded axis.  In particular, an adapter must not use
/// this representation to paper over a possible `i64` overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct AffineForm {
    pub(super) coefficient: i128,
    pub(super) intercept: i128,
}

impl AffineForm {
    pub(super) const fn new(coefficient: i128, intercept: i128) -> Self {
        Self {
            coefficient,
            intercept,
        }
    }

    fn evaluate(self, axis: i128) -> Result<i128, ArithmeticIssue> {
        self.coefficient
            .checked_mul(axis)
            .and_then(|value| value.checked_add(self.intercept))
            .ok_or(ArithmeticIssue::Overflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum BoundaryRelation {
    Less,
    LessOrEqual,
    Equal,
    NotEqual,
    GreaterOrEqual,
    Greater,
}

impl BoundaryRelation {
    fn cuts(self) -> &'static [i128] {
        match self {
            Self::Less | Self::GreaterOrEqual => &[0],
            Self::LessOrEqual | Self::Greater => &[1],
            Self::Equal | Self::NotEqual => &[0, 1],
        }
    }

    fn evaluate(self, difference: i128) -> bool {
        match self {
            Self::Less => difference < 0,
            Self::LessOrEqual => difference <= 0,
            Self::Equal => difference == 0,
            Self::NotEqual => difference != 0,
            Self::GreaterOrEqual => difference >= 0,
            Self::Greater => difference > 0,
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Less => "lt",
            Self::LessOrEqual => "le",
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::GreaterOrEqual => "ge",
            Self::Greater => "gt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum TieArm {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum ExactRoundingMode {
    Floor,
    Ceil,
    NearestTiesAwayFromZero,
}

/// A Boolean expression whose every integer atom has already been resolved
/// and normalized by the checked-callable adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundaryPredicate {
    Constant(bool),
    Comparison {
        difference: AffineForm,
        relation: BoundaryRelation,
        source: SourceSite,
    },
    Not(Box<BoundaryPredicate>),
    All(Box<[BoundaryPredicate]>),
    Any(Box<[BoundaryPredicate]>),
    FiniteDispatch {
        arms: Box<[FinitePredicateDispatchArm]>,
        otherwise: Option<Box<BoundaryPredicate>>,
        source: SourceSite,
    },
    Unsupported(UnsupportedResidual),
}

/// One exact constant-division term in the checked classification formula.
/// `nonnegative_numerator` denotes `max(numerator, 0)` before truncating
/// division. The adapter has already proved the numerator and final linear
/// combination stay within Futuruna `Int` over the declared boundary axis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ResolvedQuantizedTerm {
    pub(super) source: SourceSiteId,
    pub(super) numerator: AffineForm,
    pub(super) positive_divisor: i64,
    pub(super) nonnegative_numerator: bool,
    pub(super) coefficient: i128,
}

/// Exact one-axis quasi-affine integer normal form retained for proof closure.
/// Terms are canonical source-identity order and have nonzero coefficients.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ResolvedQuasiAffineForm {
    pub(super) affine: AffineForm,
    pub(super) quantized_terms: Box<[ResolvedQuantizedTerm]>,
}

/// The checked query's exact Boolean classification formula for one outer
/// profile. Unlike source-event labels, this formula can feed a proof backend.
/// Unsupported leaves are explicit and can never close an interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ResolvedClassificationFormula {
    Constant(bool),
    Comparison {
        difference: ResolvedQuasiAffineForm,
        relation: BoundaryRelation,
        source: SourceSite,
    },
    Not(Box<ResolvedClassificationFormula>),
    All(Box<[ResolvedClassificationFormula]>),
    Any(Box<[ResolvedClassificationFormula]>),
    Unsupported(UnsupportedResidual),
}

/// One source-order arm in a finite dispatch whose result is Boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FinitePredicateDispatchArm {
    pub(super) predicate: BoundaryPredicate,
    pub(super) value: Box<BoundaryPredicate>,
}

/// One statically enumerable exact-key table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FiniteTableEntry {
    pub(super) key: i128,
    pub(super) value: Box<BoundaryIntExpr>,
}

/// One source-order arm in a finite ordered dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FiniteDispatchArm {
    pub(super) predicate: BoundaryPredicate,
    pub(super) value: Box<BoundaryIntExpr>,
}

/// Narrow exact integer fragment accepted by the first extractor.
///
/// Operands of the event-producing nodes are affine on purpose.  Nested
/// semilinear expressions belong in the later certificate normalizer; the
/// checked adapter must emit an unsupported residual rather than pretending a
/// nested non-affine expression is affine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundaryIntExpr {
    Affine(AffineForm),
    TruncDiv {
        numerator: AffineForm,
        divisor: i64,
        source: SourceSite,
    },
    TruncRem {
        numerator: AffineForm,
        divisor: i64,
        source: SourceSite,
    },
    ExactRound {
        numerator: AffineForm,
        positive_denominator: i64,
        mode: ExactRoundingMode,
        source: SourceSite,
    },
    Min {
        left_minus_right: AffineForm,
        tie_arm: TieArm,
        source: SourceSite,
    },
    Max {
        left_minus_right: AffineForm,
        tie_arm: TieArm,
        source: SourceSite,
    },
    Clamp {
        /// Standard inclusive clamp: values equal to either bound belong to
        /// the middle/value arm, so cuts are `lower` and `upper + 1`.
        value: AffineForm,
        lower: i128,
        upper: i128,
        source: SourceSite,
    },
    FiniteTable {
        selector: AffineForm,
        entries: Box<[FiniteTableEntry]>,
        default: Option<Box<BoundaryIntExpr>>,
        source: SourceSite,
    },
    FiniteDispatch {
        arms: Box<[FiniteDispatchArm]>,
        otherwise: Option<Box<BoundaryIntExpr>>,
        source: SourceSite,
    },
    Sequence(Box<[BoundaryIntExpr]>),
    Unsupported(UnsupportedResidual),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum BoundaryFragmentRootRole {
    Question,
    Validity,
    BoundarySensitiveFact,
    RequestedValue,
}

/// One reachable root plus activation guards supplied by the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedBoundaryRoot {
    pub(super) role: BoundaryFragmentRootRole,
    pub(super) guards: Box<[SourceGuard]>,
    /// Exact endpoint values on which this occurrence can influence the
    /// requested root.  This is liveness, not merely evaluation reachability:
    /// a checked adapter may use it to avoid scheduling quantizer cuts after a
    /// downstream clamp has made the occurrence observationally dead.
    ///
    /// An event candidate is retained only when both boundary endpoints are
    /// in this support. Ordinary control-flow decisions that feed the proof
    /// must still appear as predicate/dispatch events. The abstract liveness
    /// boundary itself need not be an event when its certificate proves the
    /// requested value invariant outside the support.
    pub(super) active_support: ResolvedAxisSupport,
    pub(super) node: ResolvedBoundaryNode,
}

/// One canonical half-open interval of boundary-axis endpoint values.
///
/// `i128` permits the half-open successor of `i64::MAX`; actual axis values
/// remain Futuruna `Int` (`i64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct BoundaryAxisInterval {
    pub(super) start_inclusive: i128,
    pub(super) end_exclusive: i128,
}

/// Exact liveness support supplied by the checked, profile-specialized
/// adapter.  Intervals must be nonempty, sorted, disjoint, and coalesced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ResolvedAxisSupport {
    Everywhere,
    ExactIntervals {
        intervals: Box<[BoundaryAxisInterval]>,
        certificate: ResolvedLivenessCertificate,
    },
}

/// Checked proof identity for one root-wide observational-liveness support.
///
/// The proposition is pair-relative: for the declared positive `step`, every
/// eligible endpoint pair omitted by `ResolvedAxisSupport::contains_pair`
/// makes every `covered_event_site` observationally irrelevant to the
/// requested classification/value roots. Ordinary control-flow events that
/// establish the proposition remain separately extractable. This certificate
/// schedules no classification and closes no case region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedLivenessCertificate {
    pub(super) certificate_id: Box<str>,
    pub(super) analysis_program_hash: Box<str>,
    pub(super) query_hash: Box<str>,
    pub(super) outer_ordinals: Box<[u128]>,
    pub(super) axis_name: Box<str>,
    pub(super) step: i64,
    /// Canonical sorted unique set. A heterogeneous root must be split unless
    /// one proof covers every event-producing descendant exactly.
    pub(super) covered_event_sites: Box<[SourceSiteId]>,
}

/// Auditable copy retained by the extraction even when the support emits no
/// candidate. Hash/profile/query context lives on the enclosing extraction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SourceLivenessCertificateEvidence {
    pub(super) certificate_id: Box<str>,
    pub(super) intervals: Box<[BoundaryAxisInterval]>,
    pub(super) covered_event_sites: Box<[SourceSiteId]>,
}

impl ResolvedAxisSupport {
    fn validate(&self) -> Result<(), &'static str> {
        let Self::ExactIntervals {
            intervals,
            certificate,
        } = self
        else {
            return Ok(());
        };
        if certificate.certificate_id.trim().is_empty() {
            return Err("exact axis support requires a nonempty liveness certificate identity");
        }
        let mut prior_end = None;
        let axis_min = i128::from(i64::MIN);
        let axis_max_exclusive = i128::from(i64::MAX) + 1;
        for interval in intervals.iter() {
            if interval.start_inclusive >= interval.end_exclusive {
                return Err("axis-support intervals must be nonempty");
            }
            if interval.start_inclusive < axis_min || interval.end_exclusive > axis_max_exclusive {
                return Err("axis-support intervals must contain only Futuruna Int endpoints");
            }
            if let Some(end) = prior_end {
                if interval.start_inclusive <= end {
                    return Err("axis-support intervals must be sorted, disjoint, and coalesced");
                }
            }
            prior_end = Some(interval.end_exclusive);
        }
        Ok(())
    }

    fn contains(&self, value: i128) -> bool {
        match self {
            Self::Everywhere => true,
            Self::ExactIntervals { intervals, .. } => intervals
                .binary_search_by(|interval| {
                    if value < interval.start_inclusive {
                        std::cmp::Ordering::Greater
                    } else if value >= interval.end_exclusive {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .is_ok(),
        }
    }

    fn contains_pair(&self, lower: i128, upper: i128) -> bool {
        self.contains(lower) && self.contains(upper)
    }

    fn certificate_id(&self) -> Option<&str> {
        match self {
            Self::Everywhere => None,
            Self::ExactIntervals { certificate, .. } => Some(&certificate.certificate_id),
        }
    }

    fn certificate_evidence(&self) -> Option<SourceLivenessCertificateEvidence> {
        match self {
            Self::Everywhere => None,
            Self::ExactIntervals {
                intervals,
                certificate,
            } => Some(SourceLivenessCertificateEvidence {
                certificate_id: certificate.certificate_id.clone(),
                intervals: intervals.clone(),
                covered_event_sites: certificate.covered_event_sites.clone(),
            }),
        }
    }

    fn endpoint_hull(&self) -> Option<(i128, i128)> {
        match self {
            Self::Everywhere => None,
            Self::ExactIntervals { intervals, .. } => {
                let first = intervals.first()?;
                let last = intervals.last()?;
                Some((first.start_inclusive, last.end_exclusive.checked_sub(1)?))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ResolvedBoundaryNode {
    Int(BoundaryIntExpr),
    Predicate(BoundaryPredicate),
    Unsupported(UnsupportedResidual),
}

/// Whether the checked-callable adapter covered the whole reachable slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ResolvedFragmentCoverage {
    Complete,
    Incomplete {
        residuals: Box<[UnsupportedResidual]>,
    },
}

/// Profile-specialized, resolved input to source-event extraction.
///
/// Constants that depend on non-boundary dimensions are substituted before
/// constructing this value.  `outer_ordinals` in the extraction request binds
/// the result to that specialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedBoundaryFragment {
    pub(super) analysis_program_hash: Box<str>,
    pub(super) query_hash: Box<str>,
    pub(super) outer_ordinals: Box<[u128]>,
    pub(super) axis_name: Box<str>,
    pub(super) step: i64,
    /// Exact final question semantics, when the checked adapter can normalize
    /// them. Event roots remain separate scheduling/provenance evidence.
    pub(super) classification: ResolvedClassificationFormula,
    pub(super) roots: Box<[ResolvedBoundaryRoot]>,
    pub(super) coverage: ResolvedFragmentCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum SourceGuard {
    /// Reachability context, retained alongside activation guards so the same
    /// event site reached from classification and requested-value slices is
    /// not silently conflated.
    ReachableFrom {
        role: BoundaryFragmentRootRole,
    },
    PredicateOutcome {
        site: SourceSiteId,
        expected: bool,
    },
    /// In an ordered dispatch, every source-order arm before `arm_index`
    /// evaluated false. The current arm predicate is intentionally not part of
    /// this prefix guard because it is the event being tested.
    DispatchPrefix {
        dispatch: SourceSiteId,
        arm_index: u32,
    },
    DispatchArm {
        dispatch: SourceSiteId,
        arm_index: u32,
    },
    TableEntry {
        table: SourceSiteId,
        key: i128,
    },
    TableDefault {
        table: SourceSiteId,
    },
    /// Proof identity for an exact observational-liveness restriction. This
    /// explains candidate pruning but cannot classify the complement.
    ActiveSupport {
        certificate_id: Box<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum SourceEventKind {
    Comparison(BoundaryRelation),
    DispatchGuard {
        arm_index: u32,
        relation: BoundaryRelation,
    },
    TruncDivision {
        divisor: i64,
    },
    TruncRemainderWrap {
        divisor: i64,
    },
    ExactRounding {
        positive_denominator: i64,
        mode: ExactRoundingMode,
    },
    Minimum {
        tie_arm: TieArm,
    },
    Maximum {
        tie_arm: TieArm,
    },
    ClampLower,
    ClampUpper,
    FiniteTableKey {
        key: i128,
    },
}

impl SourceEventKind {
    fn token(&self) -> String {
        match self {
            Self::Comparison(relation) => format!("comparison.{}", relation.token()),
            Self::DispatchGuard {
                arm_index,
                relation,
            } => format!("dispatch-arm-{arm_index}.{}", relation.token()),
            Self::TruncDivision { divisor } => format!("trunc-div.{divisor}"),
            Self::TruncRemainderWrap { divisor } => format!("trunc-rem-wrap.{divisor}"),
            Self::ExactRounding {
                positive_denominator,
                mode,
            } => format!(
                "round.{}.{}",
                match mode {
                    ExactRoundingMode::Floor => "floor",
                    ExactRoundingMode::Ceil => "ceil",
                    ExactRoundingMode::NearestTiesAwayFromZero => "nearest-ties-away",
                },
                positive_denominator
            ),
            Self::Minimum { tie_arm } => format!(
                "min.tie-{}",
                match tie_arm {
                    TieArm::Left => "left",
                    TieArm::Right => "right",
                }
            ),
            Self::Maximum { tie_arm } => format!(
                "max.tie-{}",
                match tie_arm {
                    TieArm::Left => "left",
                    TieArm::Right => "right",
                }
            ),
            Self::ClampLower => "clamp.lower".to_string(),
            Self::ClampUpper => "clamp.upper".to_string(),
            Self::FiniteTableKey { key } => format!("finite-table.key-{key}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SourceEventId {
    pub(super) site: SourceSiteId,
    pub(super) kind: SourceEventKind,
}

/// Stable, human-readable label.  The program/query hashes live on the
/// enclosing extraction artifact and bind this structural identity to exact
/// checked code without making source positions part of identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SourceEventLabel {
    pub(super) id: SourceEventId,
    pub(super) stable_label: Box<str>,
}

impl SourceEventLabel {
    fn new(site: &SourceSite, kind: SourceEventKind) -> Self {
        let path = if site.id.ast_path.is_empty() {
            "root".to_string()
        } else {
            site.id
                .ast_path
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(".")
        };
        let stable_label =
            format!("{}@{}:{}", site.id.declaration_id, path, kind.token()).into_boxed_str();
        Self {
            id: SourceEventId {
                site: site.id.clone(),
                kind,
            },
            stable_label,
        }
    }
}

/// One source event attached to a boundary candidate. `cut` is the exact
/// affine-input cell boundary crossed by the lower/upper endpoint pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundarySourceEvent {
    pub(super) label: SourceEventLabel,
    pub(super) source_span: Span,
    pub(super) cut: i128,
    pub(super) guards: Box<[SourceGuard]>,
}

/// One canonical boundary-axis scheduling candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundarySourceCandidate {
    pub(super) boundary_ordinal: u128,
    pub(super) boundary_value: i64,
    pub(super) events: Box<[BoundarySourceEvent]>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum UnsupportedResidualKind {
    UnresolvedCallable,
    UnsupportedType,
    NonAffineArithmetic,
    VariableDivisor,
    InexactFloatRounding,
    Recursion,
    Effect,
    NonEnumerableDispatch,
    NonTotalDispatch,
    InvalidConstant,
    RuntimeOverflowNotExcluded,
    ArithmeticOverflow,
    CandidateMaterializationLimit { limit: usize },
    EventCutMaterializationLimit { limit: usize },
    AdapterIncomplete,
}

/// Explicit reachable work the source-event extractor could not normalize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnsupportedResidual {
    pub(super) source: Option<SourceSite>,
    pub(super) kind: UnsupportedResidualKind,
    pub(super) detail: Box<str>,
}

impl UnsupportedResidual {
    fn arithmetic(source: &SourceSite, detail: impl Into<Box<str>>) -> Self {
        Self {
            source: Some(source.clone()),
            kind: UnsupportedResidualKind::ArithmeticOverflow,
            detail: detail.into(),
        }
    }
}

/// Operational caps for materializing what is only a search-order hint.
/// Reaching either cap creates an explicit residual and makes extraction
/// incomplete; it never shrinks the semantic query universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceEventExtractionOptions {
    pub(super) max_candidate_ordinals: NonZeroUsize,
    pub(super) max_event_cuts: NonZeroUsize,
}

/// First-generation durable-probe extraction budget for one outer profile.
///
/// With at most 64 profiles, this admits no more than 4,096 distinct candidate
/// ordinals and 32,768 arithmetic event cuts before incomplete extraction
/// leaves the remaining support to ordinary exact fallback.  The limits are
/// part of the immutable probe-plan identity; they are not semantic bounds.
pub(super) const SOURCE_PROOF_EXTRACTION_OPTIONS_V1: SourceEventExtractionOptions =
    SourceEventExtractionOptions {
        max_candidate_ordinals: NonZeroUsize::new(64).unwrap(),
        max_event_cuts: NonZeroUsize::new(512).unwrap(),
    };

impl Default for SourceEventExtractionOptions {
    fn default() -> Self {
        SOURCE_PROOF_EXTRACTION_OPTIONS_V1
    }
}

pub(super) struct SourceEventExtractionRequest<'a> {
    pub(super) query: &'a ExploreQueryIr,
    pub(super) analysis_program_hash: &'a str,
    pub(super) query_hash: &'a str,
    /// Canonical source-order ordinals for all dimensions except the boundary
    /// axis. The fragment's profile-specialized constants must correspond to
    /// this exact tuple.
    pub(super) outer_ordinals: &'a [u128],
    pub(super) fragment: &'a ResolvedBoundaryFragment,
    pub(super) options: SourceEventExtractionOptions,
}

/// Hash-bound scheduling metadata.  Deliberately contains no classification,
/// certificate, closed interval, or proof field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceEventExtraction {
    pub(super) analysis_program_hash: String,
    pub(super) query_hash: String,
    pub(super) query_name: String,
    pub(super) axis_name: String,
    pub(super) step: i64,
    pub(super) outer_ordinals: Box<[u128]>,
    pub(super) candidates: Box<[BoundarySourceCandidate]>,
    /// Every exact liveness certificate consulted, including certificates
    /// whose support emitted no source candidate.
    pub(super) liveness_certificates: Box<[SourceLivenessCertificateEvidence]>,
    pub(super) extraction_complete: bool,
    pub(super) unsupported_residuals: Box<[UnsupportedResidual]>,
}

impl SourceEventExtraction {
    /// Source events can schedule evaluations; they can never close their
    /// complement, even when every reachable event was extracted.
    pub(super) const fn establishes_complement_closure(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SourceEventExtractionError {
    QueryHasNoBoundary,
    BoundaryAxisIndexOutOfBounds {
        index: usize,
        dimensions: usize,
    },
    BoundaryAxisNameMismatch {
        declared: String,
        dimension: String,
    },
    FragmentAxisNameMismatch {
        declared: String,
        fragment: String,
    },
    FragmentIdentityMismatch,
    BoundaryStepIsNotPositive(i64),
    UnsupportedBoundaryDomain,
    NonIntegerBoundaryMember {
        ordinal: usize,
    },
    DuplicateBoundaryMember {
        value: i64,
    },
    OuterOrdinalArityMismatch {
        expected: usize,
        actual: usize,
    },
    OuterOrdinalOutOfBounds {
        dimension: String,
        ordinal: u128,
        cardinality: u128,
    },
    DimensionCardinalityExceedsU128 {
        dimension: String,
    },
    InvalidAxisSupport {
        root_index: usize,
        detail: String,
    },
}

impl fmt::Display for SourceEventExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryHasNoBoundary => {
                formatter.write_str("source-event extraction requires a boundary query")
            }
            Self::BoundaryAxisIndexOutOfBounds { index, dimensions } => write!(
                formatter,
                "boundary axis index {index} is outside {dimensions} exploration dimensions"
            ),
            Self::BoundaryAxisNameMismatch {
                declared,
                dimension,
            } => write!(
                formatter,
                "boundary names `{declared}` but its indexed dimension is `{dimension}`"
            ),
            Self::FragmentAxisNameMismatch { declared, fragment } => write!(
                formatter,
                "resolved fragment names boundary axis `{fragment}` but the query declares `{declared}`"
            ),
            Self::FragmentIdentityMismatch => formatter.write_str(
                "resolved boundary fragment program/query/profile/step identity does not match the extraction request",
            ),
            Self::BoundaryStepIsNotPositive(step) => {
                write!(formatter, "boundary step {step} is not positive")
            }
            Self::UnsupportedBoundaryDomain => formatter.write_str(
                "source-event extraction supports an Int range or enumerated Int boundary axis",
            ),
            Self::NonIntegerBoundaryMember { ordinal } => write!(
                formatter,
                "enumerated boundary member at ordinal {ordinal} is not an Int"
            ),
            Self::DuplicateBoundaryMember { value } => write!(
                formatter,
                "enumerated boundary value {value} occurs more than once and has no unique canonical ordinal"
            ),
            Self::OuterOrdinalArityMismatch { expected, actual } => write!(
                formatter,
                "outer profile has {actual} ordinals, expected {expected}"
            ),
            Self::OuterOrdinalOutOfBounds {
                dimension,
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "outer ordinal {ordinal} is outside dimension `{dimension}` cardinality {cardinality}"
            ),
            Self::DimensionCardinalityExceedsU128 { dimension } => write!(
                formatter,
                "dimension `{dimension}` cardinality exceeds u128"
            ),
            Self::InvalidAxisSupport { root_index, detail } => write!(
                formatter,
                "resolved boundary root {root_index} has invalid axis support: {detail}"
            ),
        }
    }
}

impl std::error::Error for SourceEventExtractionError {}

/// Extract deterministic source-event scheduling candidates from one resolved,
/// profile-specialized fragment of a checked query.
pub(super) fn extract_source_event_candidates(
    request: SourceEventExtractionRequest<'_>,
) -> Result<SourceEventExtraction, SourceEventExtractionError> {
    let boundary = request
        .query
        .universe
        .boundary
        .as_ref()
        .ok_or(SourceEventExtractionError::QueryHasNoBoundary)?;
    if boundary.step <= 0 {
        return Err(SourceEventExtractionError::BoundaryStepIsNotPositive(
            boundary.step,
        ));
    }
    let dimension = request
        .query
        .universe
        .dimensions
        .get(boundary.axis_dimension_index)
        .ok_or(SourceEventExtractionError::BoundaryAxisIndexOutOfBounds {
            index: boundary.axis_dimension_index,
            dimensions: request.query.universe.dimensions.len(),
        })?;
    if dimension.name != boundary.axis {
        return Err(SourceEventExtractionError::BoundaryAxisNameMismatch {
            declared: boundary.axis.clone(),
            dimension: dimension.name.clone(),
        });
    }
    if request.fragment.axis_name.as_ref() != boundary.axis {
        return Err(SourceEventExtractionError::FragmentAxisNameMismatch {
            declared: boundary.axis.clone(),
            fragment: request.fragment.axis_name.to_string(),
        });
    }
    if request.fragment.analysis_program_hash.as_ref() != request.analysis_program_hash
        || request.fragment.query_hash.as_ref() != request.query_hash
        || request.fragment.outer_ordinals.as_ref() != request.outer_ordinals
        || request.fragment.step != boundary.step
    {
        return Err(SourceEventExtractionError::FragmentIdentityMismatch);
    }
    validate_outer_ordinals(
        request.query,
        boundary.axis_dimension_index,
        request.outer_ordinals,
    )?;
    for (root_index, root) in request.fragment.roots.iter().enumerate() {
        validate_root_axis_support(
            root,
            root_index,
            request.analysis_program_hash,
            request.query_hash,
            request.outer_ordinals,
            &boundary.axis,
            boundary.step,
        )?;
    }
    let axis = AxisDomain::from_exact(&dimension.domain)?;
    let extracted = extract_fragment(&axis, boundary.step, request.fragment, request.options);

    Ok(SourceEventExtraction {
        analysis_program_hash: request.analysis_program_hash.to_string(),
        query_hash: request.query_hash.to_string(),
        query_name: request
            .query
            .query
            .name
            .clone()
            .unwrap_or_else(|| "<anonymous>".to_string()),
        axis_name: boundary.axis.clone(),
        step: boundary.step,
        outer_ordinals: request.outer_ordinals.to_vec().into_boxed_slice(),
        candidates: extracted.candidates,
        liveness_certificates: extracted.liveness_certificates,
        extraction_complete: extracted.extraction_complete,
        unsupported_residuals: extracted.unsupported_residuals,
    })
}

fn validate_outer_ordinals(
    query: &ExploreQueryIr,
    boundary_dimension: usize,
    outer_ordinals: &[u128],
) -> Result<(), SourceEventExtractionError> {
    let expected = query.universe.dimensions.len().saturating_sub(1);
    if outer_ordinals.len() != expected {
        return Err(SourceEventExtractionError::OuterOrdinalArityMismatch {
            expected,
            actual: outer_ordinals.len(),
        });
    }
    let mut outer_index = 0;
    for (dimension_index, dimension) in query.universe.dimensions.iter().enumerate() {
        if dimension_index == boundary_dimension {
            continue;
        }
        let ordinal = outer_ordinals[outer_index];
        outer_index += 1;
        let cardinality = dimension.domain.cardinality().exact().ok_or_else(|| {
            SourceEventExtractionError::DimensionCardinalityExceedsU128 {
                dimension: dimension.name.clone(),
            }
        })?;
        if ordinal >= cardinality {
            return Err(SourceEventExtractionError::OuterOrdinalOutOfBounds {
                dimension: dimension.name.clone(),
                ordinal,
                cardinality,
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_root_axis_support(
    root: &ResolvedBoundaryRoot,
    root_index: usize,
    analysis_program_hash: &str,
    query_hash: &str,
    outer_ordinals: &[u128],
    axis_name: &str,
    step: i64,
) -> Result<(), SourceEventExtractionError> {
    root.active_support.validate().map_err(|detail| {
        SourceEventExtractionError::InvalidAxisSupport {
            root_index,
            detail: detail.to_string(),
        }
    })?;
    let ResolvedAxisSupport::ExactIntervals { certificate, .. } = &root.active_support else {
        return Ok(());
    };
    let mismatch =
        |detail: String| SourceEventExtractionError::InvalidAxisSupport { root_index, detail };
    if certificate.analysis_program_hash.trim().is_empty()
        || certificate.query_hash.trim().is_empty()
        || certificate.axis_name.trim().is_empty()
    {
        return Err(mismatch(
            "liveness certificate program/query/axis identities must be nonempty".to_string(),
        ));
    }
    if certificate.analysis_program_hash.as_ref() != analysis_program_hash {
        return Err(mismatch(
            "liveness certificate analysis-program hash does not match the extraction".to_string(),
        ));
    }
    if certificate.query_hash.as_ref() != query_hash {
        return Err(mismatch(
            "liveness certificate query hash does not match the extraction".to_string(),
        ));
    }
    if certificate.outer_ordinals.as_ref() != outer_ordinals {
        return Err(mismatch(
            "liveness certificate outer profile does not match the extraction".to_string(),
        ));
    }
    if certificate.axis_name.as_ref() != axis_name || certificate.step != step {
        return Err(mismatch(format!(
            "liveness certificate axis/step `{}`/{} does not match `{axis_name}`/{step}",
            certificate.axis_name, certificate.step
        )));
    }
    let actual_sites = resolved_root_event_sites(&root.node)
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    if actual_sites.is_empty() {
        return Err(mismatch(
            "exact liveness support is meaningless for a root with no event-producing site"
                .to_string(),
        ));
    }
    if certificate.covered_event_sites.as_ref() != actual_sites.as_ref() {
        return Err(mismatch(
            "liveness certificate sites must be the canonical exact set of every event-producing descendant in the root"
                .to_string(),
        ));
    }
    Ok(())
}

fn resolved_root_event_sites(node: &ResolvedBoundaryNode) -> BTreeSet<SourceSiteId> {
    let mut sites = BTreeSet::new();
    match node {
        ResolvedBoundaryNode::Int(expression) => collect_int_event_sites(expression, &mut sites),
        ResolvedBoundaryNode::Predicate(predicate) => {
            collect_predicate_event_sites(predicate, &mut sites)
        }
        ResolvedBoundaryNode::Unsupported(_) => {}
    }
    sites
}

fn collect_predicate_event_sites(
    predicate: &BoundaryPredicate,
    sites: &mut BTreeSet<SourceSiteId>,
) {
    match predicate {
        BoundaryPredicate::Constant(_) | BoundaryPredicate::Unsupported(_) => {}
        BoundaryPredicate::Comparison { source, .. } => {
            sites.insert(source.id.clone());
        }
        BoundaryPredicate::Not(inner) => collect_predicate_event_sites(inner, sites),
        BoundaryPredicate::All(parts) | BoundaryPredicate::Any(parts) => {
            for part in parts.iter() {
                collect_predicate_event_sites(part, sites);
            }
        }
        BoundaryPredicate::FiniteDispatch {
            arms,
            otherwise,
            source,
        } => {
            sites.insert(source.id.clone());
            for arm in arms.iter() {
                collect_predicate_event_sites(&arm.predicate, sites);
                collect_predicate_event_sites(&arm.value, sites);
            }
            if let Some(otherwise) = otherwise {
                collect_predicate_event_sites(otherwise, sites);
            }
        }
    }
}

fn collect_int_event_sites(expression: &BoundaryIntExpr, sites: &mut BTreeSet<SourceSiteId>) {
    match expression {
        BoundaryIntExpr::Affine(_) | BoundaryIntExpr::Unsupported(_) => {}
        BoundaryIntExpr::TruncDiv { source, .. }
        | BoundaryIntExpr::TruncRem { source, .. }
        | BoundaryIntExpr::ExactRound { source, .. }
        | BoundaryIntExpr::Min { source, .. }
        | BoundaryIntExpr::Max { source, .. }
        | BoundaryIntExpr::Clamp { source, .. } => {
            sites.insert(source.id.clone());
        }
        BoundaryIntExpr::FiniteTable {
            entries,
            default,
            source,
            ..
        } => {
            sites.insert(source.id.clone());
            for entry in entries.iter() {
                collect_int_event_sites(&entry.value, sites);
            }
            if let Some(default) = default {
                collect_int_event_sites(default, sites);
            }
        }
        BoundaryIntExpr::FiniteDispatch {
            arms,
            otherwise,
            source,
        } => {
            sites.insert(source.id.clone());
            for arm in arms.iter() {
                collect_predicate_event_sites(&arm.predicate, sites);
                collect_int_event_sites(&arm.value, sites);
            }
            if let Some(otherwise) = otherwise {
                collect_int_event_sites(otherwise, sites);
            }
        }
        BoundaryIntExpr::Sequence(expressions) => {
            for expression in expressions.iter() {
                collect_int_event_sites(expression, sites);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum AxisDomain {
    Dense {
        start: i64,
        end_exclusive: i64,
    },
    Enumerated {
        values: Box<[i64]>,
        membership: BTreeSet<i64>,
    },
}

impl AxisDomain {
    fn from_exact(domain: &ExploreExactDomain) -> Result<Self, SourceEventExtractionError> {
        match domain {
            ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                ..
            } => Ok(Self::Dense {
                start: *start,
                end_exclusive: *end_exclusive,
            }),
            ExploreExactDomain::Enumerated { values, .. } => {
                let mut ints = Vec::with_capacity(values.len());
                let mut membership = BTreeSet::new();
                for (ordinal, value) in values.iter().enumerate() {
                    let value = value
                        .int()
                        .ok_or(SourceEventExtractionError::NonIntegerBoundaryMember { ordinal })?;
                    if !membership.insert(value) {
                        return Err(SourceEventExtractionError::DuplicateBoundaryMember { value });
                    }
                    ints.push(value);
                }
                Ok(Self::Enumerated {
                    values: ints.into_boxed_slice(),
                    membership,
                })
            }
            ExploreExactDomain::FiniteType { .. } => {
                Err(SourceEventExtractionError::UnsupportedBoundaryDomain)
            }
        }
    }

    fn eligible_endpoint_hull(&self, step: i64) -> Option<(i128, i128)> {
        match self {
            Self::Dense {
                start,
                end_exclusive,
            } => {
                let start = i128::from(*start);
                let last = i128::from(*end_exclusive).checked_sub(1)?;
                let greatest_lower = last.checked_sub(i128::from(step))?;
                (start <= greatest_lower).then_some((start, last))
            }
            Self::Enumerated { values, membership } => {
                let mut minimum = None::<i64>;
                let mut maximum = None::<i64>;
                for lower in values.iter().copied() {
                    let Some(upper) = lower.checked_add(step) else {
                        continue;
                    };
                    if !membership.contains(&upper) {
                        continue;
                    }
                    for endpoint in [lower, upper] {
                        minimum = Some(minimum.map_or(endpoint, |current| current.min(endpoint)));
                        maximum = Some(maximum.map_or(endpoint, |current| current.max(endpoint)));
                    }
                }
                minimum
                    .zip(maximum)
                    .map(|(lower, upper)| (i128::from(lower), i128::from(upper)))
            }
        }
    }

    fn for_each_eligible_in_interval(
        &self,
        interval: InclusiveInterval,
        step: i64,
        mut visit: impl FnMut(u128, i64) -> Result<bool, ArithmeticIssue>,
    ) -> Result<(), ArithmeticIssue> {
        match self {
            Self::Dense {
                start,
                end_exclusive,
            } => {
                let eligible_last = i128::from(*end_exclusive)
                    .checked_sub(1)
                    .and_then(|value| value.checked_sub(i128::from(step)))
                    .ok_or(ArithmeticIssue::Overflow)?;
                let first = interval.lower.max(i128::from(*start));
                let last = interval.upper.min(eligible_last);
                if first > last {
                    return Ok(());
                }
                let mut value = first;
                loop {
                    let ordinal = u128::try_from(
                        value
                            .checked_sub(i128::from(*start))
                            .ok_or(ArithmeticIssue::Overflow)?,
                    )
                    .map_err(|_| ArithmeticIssue::Overflow)?;
                    let int_value = i64::try_from(value).map_err(|_| ArithmeticIssue::Overflow)?;
                    if !visit(ordinal, int_value)? || value == last {
                        break;
                    }
                    value = value.checked_add(1).ok_or(ArithmeticIssue::Overflow)?;
                }
                Ok(())
            }
            Self::Enumerated { values, membership } => {
                for (ordinal, value) in values.iter().copied().enumerate() {
                    let axis = i128::from(value);
                    if axis < interval.lower || axis > interval.upper {
                        continue;
                    }
                    let Some(upper) = value.checked_add(step) else {
                        continue;
                    };
                    if membership.contains(&upper)
                        && !visit(
                            u128::try_from(ordinal).map_err(|_| ArithmeticIssue::Overflow)?,
                            value,
                        )?
                    {
                        break;
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InclusiveInterval {
    lower: i128,
    upper: i128,
}

#[derive(Debug, Clone, Copy)]
enum ArithmeticIssue {
    Overflow,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResidualKey {
    site: Option<SourceSiteId>,
    kind: UnsupportedResidualKind,
    detail: Box<str>,
}

impl ResidualKey {
    fn from_residual(residual: &UnsupportedResidual) -> Self {
        Self {
            site: residual.source.as_ref().map(|source| source.id.clone()),
            kind: residual.kind.clone(),
            detail: residual.detail.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EventOccurrenceKey {
    label: SourceEventLabel,
    cut: i128,
    guards: Box<[SourceGuard]>,
}

#[derive(Default)]
struct CandidateBuilder {
    boundary_value: i64,
    events: BTreeMap<EventOccurrenceKey, BoundarySourceEvent>,
}

struct ExtractionSink {
    candidates: BTreeMap<u128, CandidateBuilder>,
    residuals: BTreeMap<ResidualKey, UnsupportedResidual>,
    liveness_certificates: BTreeMap<Box<str>, SourceLivenessCertificateEvidence>,
    options: SourceEventExtractionOptions,
    active_support: ResolvedAxisSupport,
    event_cuts: usize,
    halted: bool,
}

impl ExtractionSink {
    fn new(options: SourceEventExtractionOptions) -> Self {
        Self {
            candidates: BTreeMap::new(),
            residuals: BTreeMap::new(),
            liveness_certificates: BTreeMap::new(),
            options,
            active_support: ResolvedAxisSupport::Everywhere,
            event_cuts: 0,
            halted: false,
        }
    }

    fn set_active_support(&mut self, support: &ResolvedAxisSupport) {
        self.active_support = support.clone();
        let Some(evidence) = support.certificate_evidence() else {
            return;
        };
        let conflict = self
            .liveness_certificates
            .get(&evidence.certificate_id)
            .is_some_and(|existing| existing != &evidence);
        if conflict {
            self.push_residual(UnsupportedResidual {
                source: None,
                kind: UnsupportedResidualKind::AdapterIncomplete,
                detail: "one liveness certificate identity was reused for different supports or site sets"
                    .into(),
            });
        } else {
            self.liveness_certificates
                .entry(evidence.certificate_id.clone())
                .or_insert(evidence);
        }
    }

    fn push_residual(&mut self, residual: UnsupportedResidual) {
        self.residuals
            .entry(ResidualKey::from_residual(&residual))
            .or_insert(residual);
    }

    fn reserve_event_cut(&mut self, source: &SourceSite) -> bool {
        if self.halted {
            return false;
        }
        let Some(next) = self.event_cuts.checked_add(1) else {
            self.push_residual(UnsupportedResidual::arithmetic(
                source,
                "source-event cut counter overflowed",
            ));
            self.halted = true;
            return false;
        };
        if next > self.options.max_event_cuts.get() {
            let limit = self.options.max_event_cuts.get();
            self.push_residual(UnsupportedResidual {
                source: Some(source.clone()),
                kind: UnsupportedResidualKind::EventCutMaterializationLimit { limit },
                detail: format!(
                    "source-event extraction stopped after materializing {limit} arithmetic cuts"
                )
                .into_boxed_str(),
            });
            self.halted = true;
            return false;
        }
        self.event_cuts = next;
        true
    }

    fn insert_candidate(
        &mut self,
        ordinal: u128,
        boundary_value: i64,
        event: BoundarySourceEvent,
    ) -> bool {
        if self.halted {
            return false;
        }
        if !self.candidates.contains_key(&ordinal)
            && self.candidates.len() >= self.options.max_candidate_ordinals.get()
        {
            let limit = self.options.max_candidate_ordinals.get();
            self.push_residual(UnsupportedResidual {
                source: Some(SourceSite {
                    id: event.label.id.site.clone(),
                    span: event.source_span,
                }),
                kind: UnsupportedResidualKind::CandidateMaterializationLimit { limit },
                detail: format!(
                    "source-event extraction stopped after materializing {limit} distinct boundary ordinals"
                )
                .into_boxed_str(),
            });
            self.halted = true;
            return false;
        }
        let occurrence = EventOccurrenceKey {
            label: event.label.clone(),
            cut: event.cut,
            guards: event.guards.clone(),
        };
        let builder = self
            .candidates
            .entry(ordinal)
            .or_insert_with(|| CandidateBuilder {
                boundary_value,
                events: BTreeMap::new(),
            });
        debug_assert_eq!(builder.boundary_value, boundary_value);
        builder.events.entry(occurrence).or_insert(event);
        true
    }

    fn finish(self) -> FragmentExtraction {
        let candidates = self
            .candidates
            .into_iter()
            .map(|(boundary_ordinal, builder)| BoundarySourceCandidate {
                boundary_ordinal,
                boundary_value: builder.boundary_value,
                events: builder.events.into_values().collect::<Vec<_>>().into(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let unsupported_residuals = self
            .residuals
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let liveness_certificates = self
            .liveness_certificates
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        FragmentExtraction {
            extraction_complete: unsupported_residuals.is_empty(),
            candidates,
            liveness_certificates,
            unsupported_residuals,
        }
    }
}

struct FragmentExtraction {
    candidates: Box<[BoundarySourceCandidate]>,
    liveness_certificates: Box<[SourceLivenessCertificateEvidence]>,
    extraction_complete: bool,
    unsupported_residuals: Box<[UnsupportedResidual]>,
}

fn extract_fragment(
    axis: &AxisDomain,
    step: i64,
    fragment: &ResolvedBoundaryFragment,
    options: SourceEventExtractionOptions,
) -> FragmentExtraction {
    let mut sink = ExtractionSink::new(options);
    if let ResolvedFragmentCoverage::Incomplete { residuals } = &fragment.coverage {
        if residuals.is_empty() {
            sink.push_residual(UnsupportedResidual {
                source: None,
                kind: UnsupportedResidualKind::AdapterIncomplete,
                detail: "the resolved boundary adapter marked the fragment incomplete without identifying a residual"
                    .into(),
            });
        } else {
            for residual in residuals.iter().cloned() {
                sink.push_residual(residual);
            }
        }
    }

    for root in fragment.roots.iter() {
        if sink.halted {
            break;
        }
        sink.set_active_support(&root.active_support);
        let mut guards = with_guard(&root.guards, SourceGuard::ReachableFrom { role: root.role });
        if let Some(certificate_id) = root.active_support.certificate_id() {
            guards = with_guard(
                &guards,
                SourceGuard::ActiveSupport {
                    certificate_id: certificate_id.into(),
                },
            );
        }
        match &root.node {
            ResolvedBoundaryNode::Int(expression) => {
                extract_int_expression(expression, &guards, axis, step, &mut sink)
            }
            ResolvedBoundaryNode::Predicate(predicate) => extract_predicate(
                predicate,
                &guards,
                axis,
                step,
                PredicateEventContext::Standalone,
                &mut sink,
            ),
            ResolvedBoundaryNode::Unsupported(residual) => sink.push_residual(residual.clone()),
        }
    }
    sink.finish()
}

fn canonical_guards(guards: &[SourceGuard]) -> Box<[SourceGuard]> {
    guards
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn with_guard(guards: &[SourceGuard], guard: SourceGuard) -> Box<[SourceGuard]> {
    guards
        .iter()
        .cloned()
        .chain(std::iter::once(guard))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[derive(Debug, Clone, Copy)]
enum PredicateEventContext {
    Standalone,
    DispatchArm(u32),
}

fn extract_predicate(
    predicate: &BoundaryPredicate,
    guards: &[SourceGuard],
    axis: &AxisDomain,
    step: i64,
    context: PredicateEventContext,
    sink: &mut ExtractionSink,
) {
    if sink.halted {
        return;
    }
    match predicate {
        BoundaryPredicate::Constant(_) => {}
        BoundaryPredicate::Comparison {
            difference,
            relation,
            source,
        } => {
            let kind = match context {
                PredicateEventContext::Standalone => SourceEventKind::Comparison(*relation),
                PredicateEventContext::DispatchArm(arm_index) => SourceEventKind::DispatchGuard {
                    arm_index,
                    relation: *relation,
                },
            };
            for cut in relation.cuts() {
                let emitted = emit_cut_candidates(
                    axis,
                    step,
                    *difference,
                    *cut,
                    source,
                    kind.clone(),
                    guards,
                    |lower, upper| relation.evaluate(lower) != relation.evaluate(upper),
                    sink,
                );
                if let Err(issue) = emitted {
                    push_arithmetic_issue(sink, source, issue, "comparison event extraction");
                    break;
                }
            }
        }
        BoundaryPredicate::Not(inner) => {
            extract_predicate(inner, guards, axis, step, context, sink)
        }
        BoundaryPredicate::All(parts) | BoundaryPredicate::Any(parts) => {
            for part in parts.iter() {
                extract_predicate(part, guards, axis, step, context, sink);
                if sink.halted {
                    break;
                }
            }
        }
        BoundaryPredicate::FiniteDispatch {
            arms,
            otherwise,
            source,
        } => {
            for (arm_index, arm) in arms.iter().enumerate() {
                let Ok(arm_index) = u32::try_from(arm_index) else {
                    sink.push_residual(UnsupportedResidual {
                        source: Some(source.clone()),
                        kind: UnsupportedResidualKind::NonEnumerableDispatch,
                        detail: "finite Boolean dispatch arm index exceeds u32".into(),
                    });
                    break;
                };
                let prefix_guards = with_guard(
                    guards,
                    SourceGuard::DispatchPrefix {
                        dispatch: source.id.clone(),
                        arm_index,
                    },
                );
                extract_predicate(
                    &arm.predicate,
                    &prefix_guards,
                    axis,
                    step,
                    PredicateEventContext::DispatchArm(arm_index),
                    sink,
                );
                let arm_guards = with_guard(
                    guards,
                    SourceGuard::DispatchArm {
                        dispatch: source.id.clone(),
                        arm_index,
                    },
                );
                extract_predicate(
                    &arm.value,
                    &arm_guards,
                    axis,
                    step,
                    PredicateEventContext::Standalone,
                    sink,
                );
                if sink.halted {
                    break;
                }
            }
            if let Some(otherwise) = otherwise {
                let default_index = u32::try_from(arms.len()).unwrap_or(u32::MAX);
                let default_guards = with_guard(
                    guards,
                    SourceGuard::DispatchArm {
                        dispatch: source.id.clone(),
                        arm_index: default_index,
                    },
                );
                extract_predicate(
                    otherwise,
                    &default_guards,
                    axis,
                    step,
                    PredicateEventContext::Standalone,
                    sink,
                );
            } else {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::NonTotalDispatch,
                    detail: "finite Boolean dispatch has no statically total fallback arm".into(),
                });
            }
        }
        BoundaryPredicate::Unsupported(residual) => sink.push_residual(residual.clone()),
    }
}

fn extract_int_expression(
    expression: &BoundaryIntExpr,
    guards: &[SourceGuard],
    axis: &AxisDomain,
    step: i64,
    sink: &mut ExtractionSink,
) {
    if sink.halted {
        return;
    }
    match expression {
        BoundaryIntExpr::Affine(_) => {}
        BoundaryIntExpr::TruncDiv {
            numerator,
            divisor,
            source,
        } => {
            if *divisor == 0 {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::InvalidConstant,
                    detail: "integer division has constant divisor zero".into(),
                });
                return;
            }
            let kind = SourceEventKind::TruncDivision { divisor: *divisor };
            emit_quantizer_cuts(
                axis,
                step,
                *numerator,
                source,
                kind,
                guards,
                Quantizer::TruncDivision(*divisor),
                sink,
            );
        }
        BoundaryIntExpr::TruncRem {
            numerator,
            divisor,
            source,
        } => {
            if *divisor == 0 {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::InvalidConstant,
                    detail: "integer remainder has constant divisor zero".into(),
                });
                return;
            }
            let kind = SourceEventKind::TruncRemainderWrap { divisor: *divisor };
            emit_quantizer_cuts(
                axis,
                step,
                *numerator,
                source,
                kind,
                guards,
                Quantizer::TruncRemainderWrap(*divisor),
                sink,
            );
        }
        BoundaryIntExpr::ExactRound {
            numerator,
            positive_denominator,
            mode,
            source,
        } => {
            if *positive_denominator <= 0 {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::InvalidConstant,
                    detail: format!(
                        "exact rounding denominator {positive_denominator} is not positive"
                    )
                    .into_boxed_str(),
                });
                return;
            }
            let kind = SourceEventKind::ExactRounding {
                positive_denominator: *positive_denominator,
                mode: *mode,
            };
            emit_quantizer_cuts(
                axis,
                step,
                *numerator,
                source,
                kind,
                guards,
                Quantizer::Round {
                    denominator: *positive_denominator,
                    mode: *mode,
                },
                sink,
            );
        }
        BoundaryIntExpr::Min {
            left_minus_right,
            tie_arm,
            source,
        } => {
            let cut = if *tie_arm == TieArm::Left { 1 } else { 0 };
            let emitted = emit_cut_candidates(
                axis,
                step,
                *left_minus_right,
                cut,
                source,
                SourceEventKind::Minimum { tie_arm: *tie_arm },
                guards,
                |lower, upper| {
                    min_left_selected(lower, *tie_arm) != min_left_selected(upper, *tie_arm)
                },
                sink,
            );
            if let Err(issue) = emitted {
                push_arithmetic_issue(sink, source, issue, "minimum branch extraction");
            }
        }
        BoundaryIntExpr::Max {
            left_minus_right,
            tie_arm,
            source,
        } => {
            let cut = if *tie_arm == TieArm::Left { 0 } else { 1 };
            let emitted = emit_cut_candidates(
                axis,
                step,
                *left_minus_right,
                cut,
                source,
                SourceEventKind::Maximum { tie_arm: *tie_arm },
                guards,
                |lower, upper| {
                    max_left_selected(lower, *tie_arm) != max_left_selected(upper, *tie_arm)
                },
                sink,
            );
            if let Err(issue) = emitted {
                push_arithmetic_issue(sink, source, issue, "maximum branch extraction");
            }
        }
        BoundaryIntExpr::Clamp {
            value,
            lower,
            upper,
            source,
        } => {
            if lower > upper {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::InvalidConstant,
                    detail: format!("clamp lower bound {lower} exceeds upper bound {upper}")
                        .into_boxed_str(),
                });
                return;
            }
            let Some(upper_cut) = upper.checked_add(1) else {
                sink.push_residual(UnsupportedResidual::arithmetic(
                    source,
                    "clamp upper event `upper + 1` overflows",
                ));
                return;
            };
            let events = [
                (*lower, SourceEventKind::ClampLower),
                (upper_cut, SourceEventKind::ClampUpper),
            ];
            for (cut, kind) in events {
                let emitted = emit_cut_candidates(
                    axis,
                    step,
                    *value,
                    cut,
                    source,
                    kind,
                    guards,
                    |before, after| {
                        clamp_region(before, *lower, *upper) != clamp_region(after, *lower, *upper)
                    },
                    sink,
                );
                if let Err(issue) = emitted {
                    push_arithmetic_issue(sink, source, issue, "clamp branch extraction");
                    break;
                }
            }
        }
        BoundaryIntExpr::FiniteTable {
            selector,
            entries,
            default,
            source,
        } => extract_finite_table(
            *selector,
            entries,
            default.as_deref(),
            source,
            guards,
            axis,
            step,
            sink,
        ),
        BoundaryIntExpr::FiniteDispatch {
            arms,
            otherwise,
            source,
        } => {
            for (arm_index, arm) in arms.iter().enumerate() {
                let Ok(arm_index) = u32::try_from(arm_index) else {
                    sink.push_residual(UnsupportedResidual {
                        source: Some(source.clone()),
                        kind: UnsupportedResidualKind::NonEnumerableDispatch,
                        detail: "finite dispatch arm index exceeds u32".into(),
                    });
                    break;
                };
                let prefix_guards = with_guard(
                    guards,
                    SourceGuard::DispatchPrefix {
                        dispatch: source.id.clone(),
                        arm_index,
                    },
                );
                extract_predicate(
                    &arm.predicate,
                    &prefix_guards,
                    axis,
                    step,
                    PredicateEventContext::DispatchArm(arm_index),
                    sink,
                );
                let arm_guards = with_guard(
                    guards,
                    SourceGuard::DispatchArm {
                        dispatch: source.id.clone(),
                        arm_index,
                    },
                );
                extract_int_expression(&arm.value, &arm_guards, axis, step, sink);
                if sink.halted {
                    break;
                }
            }
            if let Some(otherwise) = otherwise {
                let default_index = u32::try_from(arms.len()).unwrap_or(u32::MAX);
                let default_guards = with_guard(
                    guards,
                    SourceGuard::DispatchArm {
                        dispatch: source.id.clone(),
                        arm_index: default_index,
                    },
                );
                extract_int_expression(otherwise, &default_guards, axis, step, sink);
            } else {
                sink.push_residual(UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::NonTotalDispatch,
                    detail: "finite dispatch has no statically total fallback arm".into(),
                });
            }
        }
        BoundaryIntExpr::Sequence(expressions) => {
            for expression in expressions.iter() {
                extract_int_expression(expression, guards, axis, step, sink);
                if sink.halted {
                    break;
                }
            }
        }
        BoundaryIntExpr::Unsupported(residual) => sink.push_residual(residual.clone()),
    }
}

fn min_left_selected(difference: i128, tie_arm: TieArm) -> bool {
    difference < 0 || (difference == 0 && tie_arm == TieArm::Left)
}

fn max_left_selected(difference: i128, tie_arm: TieArm) -> bool {
    difference > 0 || (difference == 0 && tie_arm == TieArm::Left)
}

fn clamp_region(value: i128, lower: i128, upper: i128) -> u8 {
    if value < lower {
        0
    } else if value > upper {
        2
    } else {
        1
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_finite_table(
    selector: AffineForm,
    entries: &[FiniteTableEntry],
    default: Option<&BoundaryIntExpr>,
    source: &SourceSite,
    guards: &[SourceGuard],
    axis: &AxisDomain,
    step: i64,
    sink: &mut ExtractionSink,
) {
    let mut keys = BTreeSet::new();
    for entry in entries {
        if !keys.insert(entry.key) {
            sink.push_residual(UnsupportedResidual {
                source: Some(source.clone()),
                kind: UnsupportedResidualKind::InvalidConstant,
                detail: format!("finite table repeats exact key {}", entry.key).into_boxed_str(),
            });
        }
    }
    let keys = keys.into_iter().collect::<Vec<_>>();
    for key in keys.iter().copied() {
        for cut in [Some(key), key.checked_add(1)] {
            let Some(cut) = cut else {
                sink.push_residual(UnsupportedResidual::arithmetic(
                    source,
                    "finite-table key plus one overflows",
                ));
                continue;
            };
            let emitted = emit_cut_candidates(
                axis,
                step,
                selector,
                cut,
                source,
                SourceEventKind::FiniteTableKey { key },
                guards,
                |lower, upper| table_selection(lower, &keys) != table_selection(upper, &keys),
                sink,
            );
            if let Err(issue) = emitted {
                push_arithmetic_issue(sink, source, issue, "finite-table selection extraction");
                break;
            }
        }
        if sink.halted {
            break;
        }
    }

    for entry in entries {
        let entry_guards = with_guard(
            guards,
            SourceGuard::TableEntry {
                table: source.id.clone(),
                key: entry.key,
            },
        );
        extract_int_expression(&entry.value, &entry_guards, axis, step, sink);
    }
    if let Some(default) = default {
        let default_guards = with_guard(
            guards,
            SourceGuard::TableDefault {
                table: source.id.clone(),
            },
        );
        extract_int_expression(default, &default_guards, axis, step, sink);
    } else {
        sink.push_residual(UnsupportedResidual {
            source: Some(source.clone()),
            kind: UnsupportedResidualKind::NonTotalDispatch,
            detail: "finite exact-key table has no statically total default".into(),
        });
    }
}

fn table_selection(value: i128, keys: &[i128]) -> Option<usize> {
    keys.binary_search(&value).ok()
}

#[derive(Debug, Clone, Copy)]
enum Quantizer {
    TruncDivision(i64),
    TruncRemainderWrap(i64),
    Round {
        denominator: i64,
        mode: ExactRoundingMode,
    },
}

impl Quantizer {
    fn cell(self, value: i128) -> Result<i128, ArithmeticIssue> {
        match self {
            Self::TruncDivision(divisor) | Self::TruncRemainderWrap(divisor) => value
                .checked_div(i128::from(divisor))
                .ok_or(ArithmeticIssue::Overflow),
            Self::Round { denominator, mode } => exact_round(value, i128::from(denominator), mode),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_quantizer_cuts(
    axis: &AxisDomain,
    step: i64,
    affine: AffineForm,
    source: &SourceSite,
    kind: SourceEventKind,
    guards: &[SourceGuard],
    quantizer: Quantizer,
    sink: &mut ExtractionSink,
) {
    let image = affine_image_hull(axis, step, affine, &sink.active_support);
    let (lower, upper) = match image {
        Ok(Some(image)) => image,
        Ok(None) => return,
        Err(issue) => {
            push_arithmetic_issue(sink, source, issue, "quantizer image extraction");
            return;
        }
    };
    if lower >= upper {
        return;
    }

    let emitted = match quantizer {
        Quantizer::TruncDivision(divisor) | Quantizer::TruncRemainderWrap(divisor) => {
            let period = i128::from(divisor).abs();
            emit_truncation_cuts(lower, upper, period, |cut| {
                emit_cut_candidates(
                    axis,
                    step,
                    affine,
                    cut,
                    source,
                    kind.clone(),
                    guards,
                    |before, after| quantizer.cell(before).ok() != quantizer.cell(after).ok(),
                    sink,
                )
            })
        }
        Quantizer::Round { denominator, mode } => {
            emit_rounding_cuts(lower, upper, i128::from(denominator), mode, |cut| {
                emit_cut_candidates(
                    axis,
                    step,
                    affine,
                    cut,
                    source,
                    kind.clone(),
                    guards,
                    |before, after| quantizer.cell(before).ok() != quantizer.cell(after).ok(),
                    sink,
                )
            })
        }
    };
    if let Err(issue) = emitted {
        push_arithmetic_issue(sink, source, issue, "quantizer cut extraction");
    }
}

fn affine_image_hull(
    axis: &AxisDomain,
    step: i64,
    affine: AffineForm,
    support: &ResolvedAxisSupport,
) -> Result<Option<(i128, i128)>, ArithmeticIssue> {
    let Some((mut axis_lower, mut axis_upper)) = axis.eligible_endpoint_hull(step) else {
        return Ok(None);
    };
    if let Some((support_lower, support_upper)) = support.endpoint_hull() {
        axis_lower = axis_lower.max(support_lower);
        axis_upper = axis_upper.min(support_upper);
        if axis_lower > axis_upper {
            return Ok(None);
        }
    } else if matches!(support, ResolvedAxisSupport::ExactIntervals { .. }) {
        return Ok(None);
    }
    let first = affine.evaluate(axis_lower)?;
    let last = affine.evaluate(axis_upper)?;
    Ok(Some((first.min(last), first.max(last))))
}

/// Emit Rust/Futuruna truncation cell cuts over `(lower, upper]`.
///
/// For positive magnitude `q`, truncation changes at `k*q` for `k >= 1`
/// and at `-k*q + 1` for `k >= 1`. Remainder wraps at the same cuts.
fn emit_truncation_cuts(
    lower: i128,
    upper: i128,
    period: i128,
    mut emit: impl FnMut(i128) -> Result<(), ArithmeticIssue>,
) -> Result<(), ArithmeticIssue> {
    if period <= 0 {
        return Err(ArithmeticIssue::Overflow);
    }
    let positive_min = floor_div(lower, period)?
        .checked_add(1)
        .ok_or(ArithmeticIssue::Overflow)?
        .max(1);
    let positive_max = floor_div(upper, period)?;
    emit_progression(positive_min, positive_max, period, 0, &mut emit)?;

    let negative_min = ceil_div(
        1_i128.checked_sub(upper).ok_or(ArithmeticIssue::Overflow)?,
        period,
    )?
    .max(1);
    let negative_max = floor_div(
        lower.checked_neg().ok_or(ArithmeticIssue::Overflow)?,
        period,
    )?;
    emit_decreasing_progression(negative_min, negative_max, period, 1, &mut emit)
}

/// Emit exact rational rounding cuts over `(lower, upper]`.
fn emit_rounding_cuts(
    lower: i128,
    upper: i128,
    denominator: i128,
    mode: ExactRoundingMode,
    mut emit: impl FnMut(i128) -> Result<(), ArithmeticIssue>,
) -> Result<(), ArithmeticIssue> {
    if denominator <= 0 {
        return Err(ArithmeticIssue::Overflow);
    }
    match mode {
        ExactRoundingMode::Floor => {
            let first = floor_div(lower, denominator)?
                .checked_add(1)
                .ok_or(ArithmeticIssue::Overflow)?;
            let last = floor_div(upper, denominator)?;
            emit_progression(first, last, denominator, 0, &mut emit)
        }
        ExactRoundingMode::Ceil => {
            let shifted_lower = lower.checked_sub(1).ok_or(ArithmeticIssue::Overflow)?;
            let shifted_upper = upper.checked_sub(1).ok_or(ArithmeticIssue::Overflow)?;
            let first = floor_div(shifted_lower, denominator)?
                .checked_add(1)
                .ok_or(ArithmeticIssue::Overflow)?;
            let last = floor_div(shifted_upper, denominator)?;
            emit_progression(first, last, denominator, 1, &mut emit)
        }
        ExactRoundingMode::NearestTiesAwayFromZero => {
            let half_up = denominator
                .checked_add(1)
                .ok_or(ArithmeticIssue::Overflow)?
                / 2;
            let positive_min = floor_div(
                lower
                    .checked_sub(half_up)
                    .ok_or(ArithmeticIssue::Overflow)?,
                denominator,
            )?
            .checked_add(1)
            .ok_or(ArithmeticIssue::Overflow)?
            .max(0);
            let positive_max = floor_div(
                upper
                    .checked_sub(half_up)
                    .ok_or(ArithmeticIssue::Overflow)?,
                denominator,
            )?;
            emit_progression(positive_min, positive_max, denominator, half_up, &mut emit)?;

            let negative_base = 1_i128
                .checked_sub(half_up)
                .ok_or(ArithmeticIssue::Overflow)?;
            let negative_min = ceil_div(
                negative_base
                    .checked_sub(upper)
                    .ok_or(ArithmeticIssue::Overflow)?,
                denominator,
            )?
            .max(0);
            let negative_max = floor_div(
                negative_base
                    .checked_sub(lower)
                    .and_then(|value| value.checked_sub(1))
                    .ok_or(ArithmeticIssue::Overflow)?,
                denominator,
            )?;
            emit_decreasing_progression(
                negative_min,
                negative_max,
                denominator,
                negative_base,
                &mut emit,
            )
        }
    }
}

fn emit_progression(
    first_index: i128,
    last_index: i128,
    period: i128,
    base: i128,
    emit: &mut impl FnMut(i128) -> Result<(), ArithmeticIssue>,
) -> Result<(), ArithmeticIssue> {
    if first_index > last_index {
        return Ok(());
    }
    let mut index = first_index;
    loop {
        let cut = index
            .checked_mul(period)
            .and_then(|value| value.checked_add(base))
            .ok_or(ArithmeticIssue::Overflow)?;
        emit(cut)?;
        if index == last_index {
            break;
        }
        index = index.checked_add(1).ok_or(ArithmeticIssue::Overflow)?;
    }
    Ok(())
}

fn emit_decreasing_progression(
    first_index: i128,
    last_index: i128,
    period: i128,
    base: i128,
    emit: &mut impl FnMut(i128) -> Result<(), ArithmeticIssue>,
) -> Result<(), ArithmeticIssue> {
    if first_index > last_index {
        return Ok(());
    }
    let mut index = first_index;
    loop {
        let cut = base
            .checked_sub(index.checked_mul(period).ok_or(ArithmeticIssue::Overflow)?)
            .ok_or(ArithmeticIssue::Overflow)?;
        emit(cut)?;
        if index == last_index {
            break;
        }
        index = index.checked_add(1).ok_or(ArithmeticIssue::Overflow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_cut_candidates(
    axis: &AxisDomain,
    step: i64,
    affine: AffineForm,
    cut: i128,
    source: &SourceSite,
    kind: SourceEventKind,
    guards: &[SourceGuard],
    endpoint_changes_cell: impl Fn(i128, i128) -> bool,
    sink: &mut ExtractionSink,
) -> Result<(), ArithmeticIssue> {
    if !sink.reserve_event_cut(source) {
        return Err(ArithmeticIssue::Stopped);
    }
    let Some(interval) = crossing_lower_interval(affine, i128::from(step), cut)? else {
        return Ok(());
    };
    let guards = canonical_guards(guards);
    let active_support = sink.active_support.clone();
    axis.for_each_eligible_in_interval(interval, step, |ordinal, boundary_value| {
        let lower_axis = i128::from(boundary_value);
        let upper_axis = lower_axis
            .checked_add(i128::from(step))
            .ok_or(ArithmeticIssue::Overflow)?;
        if !active_support.contains_pair(lower_axis, upper_axis) {
            return Ok(true);
        }
        let lower = affine.evaluate(lower_axis)?;
        let upper = affine.evaluate(upper_axis)?;
        if !endpoint_changes_cell(lower, upper) {
            return Ok(true);
        }
        Ok(sink.insert_candidate(
            ordinal,
            boundary_value,
            BoundarySourceEvent {
                label: SourceEventLabel::new(source, kind.clone()),
                source_span: source.span,
                cut,
                guards: guards.clone(),
            },
        ))
    })?;
    if sink.halted {
        Err(ArithmeticIssue::Stopped)
    } else {
        Ok(())
    }
}

/// Exact inverse image of one crossed cut.
///
/// The endpoint pair crosses `cut` iff
/// `min(f(x), f(x+d)) < cut <= max(f(x), f(x+d))`.  For positive slope this
/// becomes `cut-a*d <= a*x+b <= cut-1`; for negative slope it becomes
/// `cut <= a*x+b <= cut-a*d-1`.  The resulting affine interval is solved with
/// Euclidean floor/ceil rather than scanning the declared axis.
fn crossing_lower_interval(
    affine: AffineForm,
    step: i128,
    cut: i128,
) -> Result<Option<InclusiveInterval>, ArithmeticIssue> {
    if affine.coefficient == 0 {
        return Ok(None);
    }
    let delta = affine
        .coefficient
        .checked_mul(step)
        .ok_or(ArithmeticIssue::Overflow)?;
    let (value_lower, value_upper) = if delta > 0 {
        (
            cut.checked_sub(delta).ok_or(ArithmeticIssue::Overflow)?,
            cut.checked_sub(1).ok_or(ArithmeticIssue::Overflow)?,
        )
    } else {
        (
            cut,
            cut.checked_sub(delta)
                .and_then(|value| value.checked_sub(1))
                .ok_or(ArithmeticIssue::Overflow)?,
        )
    };
    solve_affine_between(affine, value_lower, value_upper).map(Some)
}

fn solve_affine_between(
    affine: AffineForm,
    value_lower: i128,
    value_upper: i128,
) -> Result<InclusiveInterval, ArithmeticIssue> {
    debug_assert!(affine.coefficient != 0);
    if affine.coefficient > 0 {
        let lower = ceil_div(
            value_lower
                .checked_sub(affine.intercept)
                .ok_or(ArithmeticIssue::Overflow)?,
            affine.coefficient,
        )?;
        let upper = floor_div(
            value_upper
                .checked_sub(affine.intercept)
                .ok_or(ArithmeticIssue::Overflow)?,
            affine.coefficient,
        )?;
        Ok(InclusiveInterval { lower, upper })
    } else {
        let coefficient = affine
            .coefficient
            .checked_neg()
            .ok_or(ArithmeticIssue::Overflow)?;
        let intercept = affine
            .intercept
            .checked_neg()
            .ok_or(ArithmeticIssue::Overflow)?;
        let transformed_lower = value_upper.checked_neg().ok_or(ArithmeticIssue::Overflow)?;
        let transformed_upper = value_lower.checked_neg().ok_or(ArithmeticIssue::Overflow)?;
        let lower = ceil_div(
            transformed_lower
                .checked_sub(intercept)
                .ok_or(ArithmeticIssue::Overflow)?,
            coefficient,
        )?;
        let upper = floor_div(
            transformed_upper
                .checked_sub(intercept)
                .ok_or(ArithmeticIssue::Overflow)?,
            coefficient,
        )?;
        Ok(InclusiveInterval { lower, upper })
    }
}

fn floor_div(numerator: i128, positive_denominator: i128) -> Result<i128, ArithmeticIssue> {
    if positive_denominator <= 0 {
        return Err(ArithmeticIssue::Overflow);
    }
    numerator
        .checked_div_euclid(positive_denominator)
        .ok_or(ArithmeticIssue::Overflow)
}

fn ceil_div(numerator: i128, positive_denominator: i128) -> Result<i128, ArithmeticIssue> {
    let floor = floor_div(numerator, positive_denominator)?;
    let remainder = numerator
        .checked_rem_euclid(positive_denominator)
        .ok_or(ArithmeticIssue::Overflow)?;
    if remainder == 0 {
        Ok(floor)
    } else {
        floor.checked_add(1).ok_or(ArithmeticIssue::Overflow)
    }
}

fn exact_round(
    numerator: i128,
    positive_denominator: i128,
    mode: ExactRoundingMode,
) -> Result<i128, ArithmeticIssue> {
    if positive_denominator <= 0 {
        return Err(ArithmeticIssue::Overflow);
    }
    match mode {
        ExactRoundingMode::Floor => floor_div(numerator, positive_denominator),
        ExactRoundingMode::Ceil => ceil_div(numerator, positive_denominator),
        ExactRoundingMode::NearestTiesAwayFromZero => {
            if numerator < 0 {
                return exact_round(
                    numerator.checked_neg().ok_or(ArithmeticIssue::Overflow)?,
                    positive_denominator,
                    mode,
                )?
                .checked_neg()
                .ok_or(ArithmeticIssue::Overflow);
            }
            let quotient = numerator
                .checked_div(positive_denominator)
                .ok_or(ArithmeticIssue::Overflow)?;
            let remainder = numerator
                .checked_rem(positive_denominator)
                .ok_or(ArithmeticIssue::Overflow)?;
            let doubled = remainder.checked_mul(2).ok_or(ArithmeticIssue::Overflow)?;
            if doubled >= positive_denominator {
                quotient.checked_add(1).ok_or(ArithmeticIssue::Overflow)
            } else {
                Ok(quotient)
            }
        }
    }
}

fn push_arithmetic_issue(
    sink: &mut ExtractionSink,
    source: &SourceSite,
    _issue: ArithmeticIssue,
    operation: &str,
) {
    if matches!(_issue, ArithmeticIssue::Stopped) {
        return;
    }
    sink.push_residual(UnsupportedResidual::arithmetic(
        source,
        format!("checked i128 overflow while {operation}").into_boxed_str(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(path: &[u32]) -> SourceSite {
        SourceSite {
            id: SourceSiteId {
                declaration_id: "test::rule/1".into(),
                ast_path: path.to_vec().into_boxed_slice(),
            },
            span: Span::dummy(),
        }
    }

    fn options() -> SourceEventExtractionOptions {
        SourceEventExtractionOptions {
            max_candidate_ordinals: NonZeroUsize::new(10_000).unwrap(),
            max_event_cuts: NonZeroUsize::new(10_000).unwrap(),
        }
    }

    fn fragment(node: ResolvedBoundaryNode) -> ResolvedBoundaryFragment {
        ResolvedBoundaryFragment {
            analysis_program_hash: "program".into(),
            query_hash: "query".into(),
            outer_ordinals: Box::new([]),
            axis_name: "income".into(),
            step: 1,
            classification: ResolvedClassificationFormula::Constant(false),
            roots: vec![ResolvedBoundaryRoot {
                role: BoundaryFragmentRootRole::Question,
                guards: Box::new([]),
                active_support: ResolvedAxisSupport::Everywhere,
                node,
            }]
            .into_boxed_slice(),
            coverage: ResolvedFragmentCoverage::Complete,
        }
    }

    fn dense(start: i64, end_exclusive: i64) -> AxisDomain {
        AxisDomain::Dense {
            start,
            end_exclusive,
        }
    }

    fn candidate_values(extraction: &FragmentExtraction) -> Vec<i64> {
        extraction
            .candidates
            .iter()
            .map(|candidate| candidate.boundary_value)
            .collect()
    }

    #[test]
    fn affine_comparison_cut_maps_to_exact_step_wide_lower_endpoints() {
        let fragment = fragment(ResolvedBoundaryNode::Predicate(
            BoundaryPredicate::Comparison {
                difference: AffineForm::new(1, -10),
                relation: BoundaryRelation::Less,
                source: site(&[0]),
            },
        ));
        let extraction = extract_fragment(&dense(0, 20), 3, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![7, 8, 9]);
        assert!(extraction.extraction_complete);
    }

    #[test]
    fn decreasing_affine_comparison_uses_the_same_exact_cut_equation() {
        let fragment = fragment(ResolvedBoundaryNode::Predicate(
            BoundaryPredicate::Comparison {
                difference: AffineForm::new(-2, 10),
                relation: BoundaryRelation::GreaterOrEqual,
                source: site(&[1]),
            },
        ));
        let extraction = extract_fragment(&dense(0, 10), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![5]);
    }

    #[test]
    fn equality_filters_pairs_that_only_jump_over_the_singleton() {
        let fragment = fragment(ResolvedBoundaryNode::Predicate(
            BoundaryPredicate::Comparison {
                difference: AffineForm::new(1, -10),
                relation: BoundaryRelation::Equal,
                source: site(&[2]),
            },
        ));
        let extraction = extract_fragment(&dense(0, 20), 3, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![7, 10]);
    }

    #[test]
    fn truncating_division_uses_positive_and_negative_runtime_cuts() {
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncDiv {
            numerator: AffineForm::new(1, 0),
            divisor: 10,
            source: site(&[3]),
        }));
        let extraction = extract_fragment(&dense(-25, 26), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![-20, -10, 9, 19]);
    }

    #[test]
    fn exact_liveness_support_prunes_dead_quantizer_cuts_without_closing_them() {
        let mut fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncDiv {
            numerator: AffineForm::new(1, -341_500),
            divisor: 1_000,
            source: site(&[3, 1]),
        }));
        fragment.roots[0].active_support = ResolvedAxisSupport::ExactIntervals {
            intervals: vec![BoundaryAxisInterval {
                start_inclusive: 341_500,
                end_exclusive: 391_501,
            }]
            .into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:ll9c-live-q-0-through-50".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[3, 1]).id].into_boxed_slice(),
            },
        };
        validate_root_axis_support(&fragment.roots[0], 0, "program", "query", &[], "income", 1)
            .unwrap();
        assert!(validate_root_axis_support(
            &fragment.roots[0],
            0,
            "stale-program",
            "query",
            &[],
            "income",
            1,
        )
        .is_err());

        let extraction = extract_fragment(&dense(0, 1_500_001), 1, &fragment, options());
        let values = candidate_values(&extraction);

        assert_eq!(values.len(), 50);
        assert_eq!(values.first(), Some(&342_499));
        assert_eq!(values.last(), Some(&391_499));
        assert!(extraction
            .candidates
            .iter()
            .all(
                |candidate| candidate.events.iter().all(|event| event.guards.iter().any(
                    |guard| matches!(
                        guard,
                        SourceGuard::ActiveSupport { certificate_id }
                            if certificate_id.as_ref() == "checked:test:ll9c-live-q-0-through-50"
                    )
                ))
            ));
        assert!(extraction.extraction_complete);
        assert_eq!(extraction.liveness_certificates.len(), 1);
        assert_eq!(
            extraction.liveness_certificates[0].certificate_id.as_ref(),
            "checked:test:ll9c-live-q-0-through-50"
        );
    }

    #[test]
    fn axis_liveness_support_requires_one_canonical_interval_union() {
        let adjacent = ResolvedAxisSupport::ExactIntervals {
            intervals: vec![
                BoundaryAxisInterval {
                    start_inclusive: 0,
                    end_exclusive: 10,
                },
                BoundaryAxisInterval {
                    start_inclusive: 10,
                    end_exclusive: 20,
                },
            ]
            .into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:adjacent".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[90]).id].into_boxed_slice(),
            },
        };
        let empty = ResolvedAxisSupport::ExactIntervals {
            intervals: vec![BoundaryAxisInterval {
                start_inclusive: 5,
                end_exclusive: 5,
            }]
            .into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:empty-cell".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[91]).id].into_boxed_slice(),
            },
        };
        let uncertified = ResolvedAxisSupport::ExactIntervals {
            intervals: Vec::<BoundaryAxisInterval>::new().into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[92]).id].into_boxed_slice(),
            },
        };

        assert!(adjacent.validate().is_err());
        assert!(empty.validate().is_err());
        assert!(uncertified.validate().is_err());
        assert!(ResolvedAxisSupport::ExactIntervals {
            intervals: Vec::<BoundaryAxisInterval>::new().into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:unreachable".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[93]).id].into_boxed_slice(),
            },
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn liveness_certificate_must_cover_every_event_descendant_in_its_root() {
        let mut fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::Sequence(
            vec![
                BoundaryIntExpr::TruncDiv {
                    numerator: AffineForm::new(1, 0),
                    divisor: 10,
                    source: site(&[94, 0]),
                },
                BoundaryIntExpr::TruncDiv {
                    numerator: AffineForm::new(1, 0),
                    divisor: 20,
                    source: site(&[94, 1]),
                },
            ]
            .into_boxed_slice(),
        )));
        fragment.roots[0].active_support = ResolvedAxisSupport::ExactIntervals {
            intervals: vec![BoundaryAxisInterval {
                start_inclusive: 0,
                end_exclusive: 100,
            }]
            .into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:incomplete-site-set".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[94, 0]).id].into_boxed_slice(),
            },
        };

        assert!(validate_root_axis_support(
            &fragment.roots[0],
            0,
            "program",
            "query",
            &[],
            "income",
            1,
        )
        .is_err());
    }

    #[test]
    fn empty_live_support_still_retains_its_certificate_ledger() {
        let mut fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncDiv {
            numerator: AffineForm::new(1, 0),
            divisor: 10,
            source: site(&[95]),
        }));
        fragment.roots[0].active_support = ResolvedAxisSupport::ExactIntervals {
            intervals: Vec::<BoundaryAxisInterval>::new().into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "checked:test:no-live-pair".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 1,
                covered_event_sites: vec![site(&[95]).id].into_boxed_slice(),
            },
        };

        let extraction = extract_fragment(&dense(0, 100), 1, &fragment, options());

        assert!(extraction.candidates.is_empty());
        assert_eq!(extraction.liveness_certificates.len(), 1);
        assert!(extraction.extraction_complete);
    }

    #[test]
    fn remainder_wraps_share_truncating_quotient_cuts() {
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncRem {
            numerator: AffineForm::new(1, 0),
            divisor: 10,
            source: site(&[4]),
        }));
        let extraction = extract_fragment(&dense(-25, 26), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![-20, -10, 9, 19]);
    }

    #[test]
    fn exact_floor_ceil_and_nearest_rounding_have_distinct_signed_cuts() {
        let roots = [
            (ExactRoundingMode::Floor, vec![-21, -11, -1, 9, 19]),
            (ExactRoundingMode::Ceil, vec![-20, -10, 0, 10, 20]),
            (
                ExactRoundingMode::NearestTiesAwayFromZero,
                vec![-25, -15, -5, 4, 14, 24],
            ),
        ];
        for (mode, expected) in roots {
            let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::ExactRound {
                numerator: AffineForm::new(1, 0),
                positive_denominator: 10,
                mode,
                source: site(&[5]),
            }));
            let extraction = extract_fragment(&dense(-25, 26), 1, &fragment, options());
            assert_eq!(candidate_values(&extraction), expected, "mode {mode:?}");
        }
    }

    #[test]
    fn min_max_tie_ownership_and_clamp_cells_are_explicit() {
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::Sequence(
            vec![
                BoundaryIntExpr::Min {
                    left_minus_right: AffineForm::new(1, -5),
                    tie_arm: TieArm::Left,
                    source: site(&[6, 0]),
                },
                BoundaryIntExpr::Max {
                    left_minus_right: AffineForm::new(1, -8),
                    tie_arm: TieArm::Right,
                    source: site(&[6, 1]),
                },
                BoundaryIntExpr::Clamp {
                    value: AffineForm::new(1, 0),
                    lower: 3,
                    upper: 10,
                    source: site(&[6, 2]),
                },
            ]
            .into_boxed_slice(),
        )));
        let extraction = extract_fragment(&dense(0, 15), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![2, 5, 8, 10]);
    }

    #[test]
    fn finite_exact_key_table_emits_entry_and_exit_candidates() {
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::FiniteTable {
            selector: AffineForm::new(1, 0),
            entries: vec![
                FiniteTableEntry {
                    key: 5,
                    value: Box::new(BoundaryIntExpr::Affine(AffineForm::new(0, 1))),
                },
                FiniteTableEntry {
                    key: 10,
                    value: Box::new(BoundaryIntExpr::Affine(AffineForm::new(0, 2))),
                },
            ]
            .into_boxed_slice(),
            default: Some(Box::new(BoundaryIntExpr::Affine(AffineForm::new(0, 0)))),
            source: site(&[7]),
        }));
        let extraction = extract_fragment(&dense(0, 15), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![4, 5, 9, 10]);
    }

    #[test]
    fn finite_boolean_dispatch_labels_source_order_guard_events() {
        let fragment = fragment(ResolvedBoundaryNode::Predicate(
            BoundaryPredicate::FiniteDispatch {
                arms: vec![FinitePredicateDispatchArm {
                    predicate: BoundaryPredicate::Comparison {
                        difference: AffineForm::new(1, -5),
                        relation: BoundaryRelation::Less,
                        source: site(&[7, 1]),
                    },
                    value: Box::new(BoundaryPredicate::Constant(true)),
                }]
                .into_boxed_slice(),
                otherwise: Some(Box::new(BoundaryPredicate::Constant(false))),
                source: site(&[7, 0]),
            },
        ));
        let extraction = extract_fragment(&dense(0, 10), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![4]);
        assert!(extraction.candidates[0].events[0]
            .label
            .stable_label
            .contains("dispatch-arm-0.lt"));
    }

    #[test]
    fn sparse_enumerated_axis_preserves_source_ordinals_and_endpoint_membership() {
        let axis = AxisDomain::Enumerated {
            values: vec![1, 4, 5, 9, 10, 11].into_boxed_slice(),
            membership: BTreeSet::from([1, 4, 5, 9, 10, 11]),
        };
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::FiniteTable {
            selector: AffineForm::new(1, 0),
            entries: vec![FiniteTableEntry {
                key: 5,
                value: Box::new(BoundaryIntExpr::Affine(AffineForm::new(0, 1))),
            }]
            .into_boxed_slice(),
            default: Some(Box::new(BoundaryIntExpr::Affine(AffineForm::new(0, 0)))),
            source: site(&[8]),
        }));
        let extraction = extract_fragment(&axis, 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![4]);
        assert_eq!(extraction.candidates[0].boundary_ordinal, 1);
    }

    #[test]
    fn supported_siblings_survive_an_explicit_unsupported_residual() {
        let unsupported = UnsupportedResidual {
            source: Some(site(&[9, 1])),
            kind: UnsupportedResidualKind::NonAffineArithmetic,
            detail: "variable-by-variable multiplication".into(),
        };
        let fragment = fragment(ResolvedBoundaryNode::Int(BoundaryIntExpr::Sequence(
            vec![
                BoundaryIntExpr::Min {
                    left_minus_right: AffineForm::new(1, -5),
                    tie_arm: TieArm::Left,
                    source: site(&[9, 0]),
                },
                BoundaryIntExpr::Unsupported(unsupported.clone()),
            ]
            .into_boxed_slice(),
        )));
        let extraction = extract_fragment(&dense(0, 10), 1, &fragment, options());

        assert_eq!(candidate_values(&extraction), vec![4]);
        assert!(!extraction.extraction_complete);
        assert_eq!(extraction.unsupported_residuals.as_ref(), &[unsupported]);
    }

    #[test]
    fn materialization_cap_retains_partial_candidates_and_an_explicit_residual() {
        let fragment = fragment(ResolvedBoundaryNode::Predicate(
            BoundaryPredicate::Comparison {
                difference: AffineForm::new(1, -10),
                relation: BoundaryRelation::Less,
                source: site(&[10]),
            },
        ));
        let limited = SourceEventExtractionOptions {
            max_candidate_ordinals: NonZeroUsize::new(1).unwrap(),
            max_event_cuts: NonZeroUsize::new(10).unwrap(),
        };
        let extraction = extract_fragment(&dense(0, 20), 3, &fragment, limited);

        assert_eq!(candidate_values(&extraction), vec![7]);
        assert!(!extraction.extraction_complete);
        assert!(matches!(
            &extraction.unsupported_residuals[0].kind,
            UnsupportedResidualKind::CandidateMaterializationLimit { limit } if *limit == 1
        ));
    }

    #[test]
    fn complete_extraction_still_has_no_complement_closure_capability() {
        let artifact = SourceEventExtraction {
            analysis_program_hash: "program".to_string(),
            query_hash: "query".to_string(),
            query_name: "example".to_string(),
            axis_name: "income".to_string(),
            step: 1,
            outer_ordinals: Box::new([]),
            candidates: Box::new([]),
            liveness_certificates: Box::new([]),
            extraction_complete: true,
            unsupported_residuals: Box::new([]),
        };

        assert!(!artifact.establishes_complement_closure());
    }
}
