//! Backend-neutral dynamic mechanism evidence for bounded Explore.
//!
//! This is deliberately an evidence core, not runtime instrumentation.  It
//! fixes the identities, canonical differential trace shape, scoped counts
//! and incidence invariants that a later fresh-replay adapter must satisfy.
//! A mechanism is never inferred from a source-event scheduling hint or an
//! equal result value.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{
    case_graph::{
        CaseDecisionDag, CaseTerminal, CheckedCardinality, DecisionRef, DecisionRoot,
        OrderedDecisionDag,
    },
    report::ExploreCaseId,
};
use crate::{AnalysisProgramId, ExprSiteId};

const MECHANISM_REQUEST_HASH_V1: &[u8] = b"futuruna.explore.mechanism-request.v1";
const MECHANISM_CASE_TARGET_HASH_V1: &[u8] = b"futuruna.explore.case-target.v1";
const MECHANISM_TARGET_MEMBERSHIP_HASH_V1: &[u8] = b"futuruna.explore.target-membership.v1";
const MECHANISM_SITE_HASH_V1: &[u8] = b"futuruna.explore.mechanism-site.v1";
const MECHANISM_OCCURRENCE_HASH_V1: &[u8] = b"futuruna.explore.mechanism-occurrence.v1";
const MECHANISM_SIGNATURE_HASH_V1: &[u8] = b"futuruna.explore.mechanism-signature.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismValidationError(String);

impl fmt::Display for MechanismValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for MechanismValidationError {}

fn invalid(message: impl Into<String>) -> MechanismValidationError {
    MechanismValidationError(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StableDigest([u8; 32]);

struct StableHasher(Sha256);

impl StableHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.segment(domain);
        hasher
    }

    fn segment(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }

    fn u32(&mut self, value: u32) {
        self.segment(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.segment(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.segment(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.segment(&value.to_le_bytes());
    }

    fn digest(self) -> StableDigest {
        let digest = self.0.finalize();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        StableDigest(bytes)
    }
}

fn validate_analysis_program_id(
    analysis_program: &AnalysisProgramId,
) -> Result<(), MechanismValidationError> {
    let value = analysis_program.as_str();
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "mechanism analysis-program identity must be a lowercase SHA-256 digest",
        ));
    }
    Ok(())
}

/// Hash identity of the checked query-and-domain contract.
///
/// The checked Explore adapter supplies its canonical bytes. Operational
/// budgets and sampling order must not be included in those bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismQueryId(StableDigest);

impl MechanismQueryId {
    pub(crate) fn from_checked_query_bytes(canonical: &[u8]) -> Self {
        let mut hasher = StableHasher::new(b"futuruna.explore.checked-query.v1");
        hasher.segment(canonical);
        Self(hasher.digest())
    }
}

/// Stable semantic expression site, scoped to one checked analysis program.
/// Spans and filesystem paths never participate in this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismSiteId {
    analysis_program: AnalysisProgramId,
    digest: StableDigest,
}

impl MechanismSiteId {
    pub(crate) fn from_expression_site(
        site: &ExprSiteId,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(&site.analysis_program)?;
        let mut hasher = StableHasher::new(MECHANISM_SITE_HASH_V1);
        hasher.segment(site.analysis_program.as_str().as_bytes());
        hasher.segment(site.declaration.semantic_key().as_bytes());
        hasher.u128(site.normalized_declaration_ordinal as u128);
        hasher.u128(site.ast_path.len() as u128);
        for child in site.ast_path.iter().copied() {
            hasher.u32(child);
        }
        Ok(Self {
            analysis_program: site.analysis_program.clone(),
            digest: hasher.digest(),
        })
    }

    fn validate_scope(
        &self,
        analysis_program: &AnalysisProgramId,
        what: &str,
    ) -> Result<(), MechanismValidationError> {
        validate_analysis_program_id(&self.analysis_program)?;
        if &self.analysis_program != analysis_program {
            return Err(invalid(format!(
                "{what} belongs to a different analysis program"
            )));
        }
        Ok(())
    }
}

/// A semantic observation root is distinct from a dynamic occurrence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismSemanticRootId(MechanismSiteId);

impl MechanismSemanticRootId {
    pub(crate) fn from_site(site: MechanismSiteId) -> Self {
        Self(site)
    }

    fn validate_scope(
        &self,
        analysis_program: &AnalysisProgramId,
        what: &str,
    ) -> Result<(), MechanismValidationError> {
        self.0.validate_scope(analysis_program, what)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MechanismObservationTarget {
    MatchingConfigurations,
}

/// Semantic identity of `S_req`, independent of whether its membership has
/// closed yet. Exact membership evidence carries a second content identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismCaseTargetId {
    query: MechanismQueryId,
    digest: StableDigest,
}

impl MechanismCaseTargetId {
    fn derive(
        query: &MechanismQueryId,
        target: MechanismObservationTarget,
        axis_cardinalities: &[u128],
    ) -> Self {
        let mut hasher = StableHasher::new(MECHANISM_CASE_TARGET_HASH_V1);
        hasher.segment(&(query.0).0);
        hasher.segment(match target {
            MechanismObservationTarget::MatchingConfigurations => b"matching-configurations",
        });
        hasher.u128(axis_cardinalities.len() as u128);
        for cardinality in axis_cardinalities.iter().copied() {
            hasher.u128(cardinality);
        }
        Self {
            query: query.clone(),
            digest: hasher.digest(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MechanismNormalization {
    DynamicControlV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismSamplingPlan {
    pub(crate) result_representatives: BTreeSet<ExploreCaseId>,
    pub(crate) extrema_witnesses: BTreeSet<ExploreCaseId>,
    pub(crate) required_case_ids: BTreeSet<ExploreCaseId>,
}

impl MechanismSamplingPlan {
    pub(crate) fn empty() -> Self {
        Self {
            result_representatives: BTreeSet::new(),
            extrema_witnesses: BTreeSet::new(),
            required_case_ids: BTreeSet::new(),
        }
    }

    fn validate(&self, axis_cardinalities: &[u128]) -> Result<(), MechanismValidationError> {
        for (kind, cases) in [
            ("result representative", &self.result_representatives),
            ("extrema witness", &self.extrema_witnesses),
            ("required", &self.required_case_ids),
        ] {
            for case_id in cases {
                validate_case_id(axis_cardinalities, case_id, kind)?;
            }
        }
        Ok(())
    }

    pub(crate) fn selected_case_ids(&self) -> BTreeSet<ExploreCaseId> {
        self.result_representatives
            .iter()
            .chain(&self.extrema_witnesses)
            .chain(&self.required_case_ids)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismNumericBin {
    pub(crate) lower_inclusive: i64,
    pub(crate) upper_exclusive: i64,
}

impl MechanismNumericBin {
    pub(crate) fn new(
        lower_inclusive: i64,
        upper_exclusive: i64,
    ) -> Result<Self, MechanismValidationError> {
        if lower_inclusive >= upper_exclusive {
            return Err(invalid(format!(
                "mechanism bin [{lower_inclusive}, {upper_exclusive}) is empty or reversed"
            )));
        }
        Ok(Self {
            lower_inclusive,
            upper_exclusive,
        })
    }
}

/// Optional named numeric observation whose bins receive mechanism incidence.
/// Empty `bin_fields` on the request is the ordinary non-histogram case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismBinField {
    pub(crate) name: Box<str>,
    pub(crate) semantic_root: MechanismSemanticRootId,
    pub(crate) bins: Box<[MechanismNumericBin]>,
}

impl MechanismBinField {
    pub(crate) fn new(
        name: impl Into<Box<str>>,
        semantic_root: MechanismSemanticRootId,
        bins: impl Into<Box<[MechanismNumericBin]>>,
    ) -> Result<Self, MechanismValidationError> {
        let field = Self {
            name: name.into(),
            semantic_root,
            bins: bins.into(),
        };
        field.validate_bins()?;
        Ok(field)
    }

    fn validate_bins(&self) -> Result<(), MechanismValidationError> {
        if self.name.is_empty() {
            return Err(invalid("mechanism bin field name must not be empty"));
        }
        if self.bins.is_empty() {
            return Err(invalid(format!(
                "mechanism bin field `{}` must declare at least one bin",
                self.name
            )));
        }
        for bin in self.bins.iter().copied() {
            MechanismNumericBin::new(bin.lower_inclusive, bin.upper_exclusive)?;
        }
        for pair in self.bins.windows(2) {
            if pair[0].lower_inclusive >= pair[1].lower_inclusive {
                return Err(invalid(format!(
                    "mechanism bins for `{}` are not in increasing order",
                    self.name
                )));
            }
            if pair[0].upper_exclusive > pair[1].lower_inclusive {
                return Err(invalid(format!(
                    "mechanism bins for `{}` overlap",
                    self.name
                )));
            }
        }
        Ok(())
    }
}

/// Hash-scoped identity of the semantic observation specification `h`.
/// Sampling cases are deliberately excluded: changing warm-up coverage must
/// not rename an otherwise identical mechanism signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismRequestId {
    analysis_program: AnalysisProgramId,
    digest: StableDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismObservationRequest {
    pub(crate) id: MechanismRequestId,
    pub(crate) analysis_program: AnalysisProgramId,
    pub(crate) query: MechanismQueryId,
    pub(crate) target: MechanismObservationTarget,
    pub(crate) case_target: MechanismCaseTargetId,
    pub(crate) before_root: MechanismSemanticRootId,
    pub(crate) after_root: MechanismSemanticRootId,
    pub(crate) normalization: MechanismNormalization,
    pub(crate) axis_cardinalities: Box<[u128]>,
    pub(crate) sampling: MechanismSamplingPlan,
    pub(crate) bin_fields: Box<[MechanismBinField]>,
}

impl MechanismObservationRequest {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        analysis_program: AnalysisProgramId,
        query: MechanismQueryId,
        target: MechanismObservationTarget,
        before_root: MechanismSemanticRootId,
        after_root: MechanismSemanticRootId,
        normalization: MechanismNormalization,
        axis_cardinalities: impl Into<Box<[u128]>>,
        sampling: MechanismSamplingPlan,
        bin_fields: impl Into<Box<[MechanismBinField]>>,
    ) -> Result<Self, MechanismValidationError> {
        let axis_cardinalities = axis_cardinalities.into();
        let bin_fields = bin_fields.into();
        validate_request_parts(
            &analysis_program,
            &before_root,
            &after_root,
            &axis_cardinalities,
            &sampling,
            &bin_fields,
        )?;
        let case_target = MechanismCaseTargetId::derive(&query, target, &axis_cardinalities);
        let id = derive_request_id(
            &analysis_program,
            &query,
            target,
            &case_target,
            &before_root,
            &after_root,
            normalization,
            &axis_cardinalities,
            &bin_fields,
        );
        Ok(Self {
            id,
            analysis_program,
            query,
            target,
            case_target,
            before_root,
            after_root,
            normalization,
            axis_cardinalities,
            sampling,
            bin_fields,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        validate_request_parts(
            &self.analysis_program,
            &self.before_root,
            &self.after_root,
            &self.axis_cardinalities,
            &self.sampling,
            &self.bin_fields,
        )?;
        let expected = derive_request_id(
            &self.analysis_program,
            &self.query,
            self.target,
            &self.case_target,
            &self.before_root,
            &self.after_root,
            self.normalization,
            &self.axis_cardinalities,
            &self.bin_fields,
        );
        if self.id != expected {
            return Err(invalid(
                "mechanism request identity disagrees with its semantic contract",
            ));
        }
        if self.case_target
            != MechanismCaseTargetId::derive(&self.query, self.target, &self.axis_cardinalities)
        {
            return Err(invalid(
                "mechanism case-target identity disagrees with its query and axes",
            ));
        }
        Ok(())
    }
}

fn validate_request_parts(
    analysis_program: &AnalysisProgramId,
    before_root: &MechanismSemanticRootId,
    after_root: &MechanismSemanticRootId,
    axis_cardinalities: &[u128],
    sampling: &MechanismSamplingPlan,
    bin_fields: &[MechanismBinField],
) -> Result<(), MechanismValidationError> {
    validate_analysis_program_id(analysis_program)?;
    before_root.validate_scope(analysis_program, "before semantic root")?;
    after_root.validate_scope(analysis_program, "after semantic root")?;
    sampling.validate(axis_cardinalities)?;

    let mut names = BTreeSet::new();
    for field in bin_fields {
        field.validate_bins()?;
        field
            .semantic_root
            .validate_scope(analysis_program, "mechanism bin semantic root")?;
        if !names.insert(field.name.as_ref()) {
            return Err(invalid(format!(
                "mechanism bin field `{}` occurs more than once",
                field.name
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_request_id(
    analysis_program: &AnalysisProgramId,
    query: &MechanismQueryId,
    target: MechanismObservationTarget,
    case_target: &MechanismCaseTargetId,
    before_root: &MechanismSemanticRootId,
    after_root: &MechanismSemanticRootId,
    normalization: MechanismNormalization,
    axis_cardinalities: &[u128],
    bin_fields: &[MechanismBinField],
) -> MechanismRequestId {
    let mut hasher = StableHasher::new(MECHANISM_REQUEST_HASH_V1);
    hasher.segment(analysis_program.as_str().as_bytes());
    hasher.segment(&(query.0).0);
    hasher.segment(match target {
        MechanismObservationTarget::MatchingConfigurations => b"matching-configurations",
    });
    hasher.segment(&case_target.digest.0);
    hasher.segment(&before_root.0.digest.0);
    hasher.segment(&after_root.0.digest.0);
    hasher.segment(match normalization {
        MechanismNormalization::DynamicControlV1 => b"dynamic-control-v1",
    });
    hasher.u128(axis_cardinalities.len() as u128);
    for cardinality in axis_cardinalities.iter().copied() {
        hasher.u128(cardinality);
    }
    hasher.u128(bin_fields.len() as u128);
    for field in bin_fields {
        hasher.segment(field.name.as_bytes());
        hasher.segment(&field.semantic_root.0.digest.0);
        hasher.u128(field.bins.len() as u128);
        for bin in field.bins.iter().copied() {
            hasher.i64(bin.lower_inclusive);
            hasher.i64(bin.upper_exclusive);
        }
    }
    MechanismRequestId {
        analysis_program: analysis_program.clone(),
        digest: hasher.digest(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DynamicEventKind {
    RuleAttempt,
    RuleSelection,
    IfDecision,
    MatchDecision,
    ShortCircuitAnd,
    ShortCircuitOr,
}

impl DynamicEventKind {
    fn token(self) -> &'static [u8] {
        match self {
            Self::RuleAttempt => b"rule-attempt",
            Self::RuleSelection => b"rule-selection",
            Self::IfDecision => b"if",
            Self::MatchDecision => b"match",
            Self::ShortCircuitAnd => b"short-circuit-and",
            Self::ShortCircuitOr => b"short-circuit-or",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RuleAttemptOutcome {
    HeadMismatch,
    GuardFalse,
    BodyFalse,
    Applicable,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RuleSelectionOutcome {
    NoApplicableRule,
    Selected(MechanismSiteId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum IfDecisionOutcome {
    Then,
    Else,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ShortCircuitOutcome {
    SkippedRight { result: bool },
    EvaluatedRight { result: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DynamicEventOutcome {
    RuleAttempt(RuleAttemptOutcome),
    RuleSelection(RuleSelectionOutcome),
    IfDecision(IfDecisionOutcome),
    MatchDecision { arm_index: u32 },
    ShortCircuit(ShortCircuitOutcome),
}

impl DynamicEventOutcome {
    fn validate(
        &self,
        kind: DynamicEventKind,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), MechanismValidationError> {
        let compatible = matches!(
            (kind, self),
            (DynamicEventKind::RuleAttempt, Self::RuleAttempt(_))
                | (DynamicEventKind::RuleSelection, Self::RuleSelection(_))
                | (DynamicEventKind::IfDecision, Self::IfDecision(_))
                | (DynamicEventKind::MatchDecision, Self::MatchDecision { .. })
                | (
                    DynamicEventKind::ShortCircuitAnd | DynamicEventKind::ShortCircuitOr,
                    Self::ShortCircuit(_)
                )
        );
        if !compatible {
            return Err(invalid(
                "dynamic mechanism event outcome has the wrong kind",
            ));
        }
        if let Self::RuleSelection(RuleSelectionOutcome::Selected(site)) = self {
            site.validate_scope(analysis_program, "selected rule site")?;
        }
        Ok(())
    }
}

/// Request-scoped identity of one normalized dynamic occurrence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismOccurrenceId {
    request: MechanismRequestId,
    digest: StableDigest,
}

impl MechanismOccurrenceId {
    fn derive(
        request: &MechanismRequestId,
        topological_ordinal: u64,
        site: &MechanismSiteId,
        kind: DynamicEventKind,
    ) -> Self {
        let mut hasher = StableHasher::new(MECHANISM_OCCURRENCE_HASH_V1);
        hasher.segment(&request.digest.0);
        hasher.u64(topological_ordinal);
        hasher.segment(&site.digest.0);
        hasher.segment(kind.token());
        Self {
            request: request.clone(),
            digest: hasher.digest(),
        }
    }
}

/// One normalized node in the paired before/after dynamic occurrence DAG.
/// Exactly one or both endpoint observations are present, so before-only and
/// after-only reachability are preserved rather than erased as "no change".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PairedOccurrenceNode {
    pub(crate) id: MechanismOccurrenceId,
    pub(crate) topological_ordinal: u64,
    pub(crate) site: MechanismSiteId,
    pub(crate) kind: DynamicEventKind,
    pub(crate) before: Option<DynamicEventOutcome>,
    pub(crate) after: Option<DynamicEventOutcome>,
    pub(crate) before_dependencies: BTreeSet<MechanismOccurrenceId>,
    pub(crate) after_dependencies: BTreeSet<MechanismOccurrenceId>,
}

impl PairedOccurrenceNode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        topological_ordinal: u64,
        site: MechanismSiteId,
        kind: DynamicEventKind,
        before: Option<DynamicEventOutcome>,
        after: Option<DynamicEventOutcome>,
        before_dependencies: BTreeSet<MechanismOccurrenceId>,
        after_dependencies: BTreeSet<MechanismOccurrenceId>,
    ) -> Result<Self, MechanismValidationError> {
        if before.is_none() && after.is_none() {
            return Err(invalid(
                "paired mechanism occurrence must exist before, after, or at both endpoints",
            ));
        }
        site.validate_scope(&request.analysis_program, "dynamic occurrence site")?;
        if let Some(outcome) = &before {
            outcome.validate(kind, &request.analysis_program)?;
        }
        if let Some(outcome) = &after {
            outcome.validate(kind, &request.analysis_program)?;
        }
        if before.is_none() && !before_dependencies.is_empty() {
            return Err(invalid(
                "before dependencies require a before-endpoint occurrence",
            ));
        }
        if after.is_none() && !after_dependencies.is_empty() {
            return Err(invalid(
                "after dependencies require an after-endpoint occurrence",
            ));
        }
        for dependency in before_dependencies.iter().chain(&after_dependencies) {
            if dependency.request != request.id {
                return Err(invalid(
                    "dynamic occurrence dependency belongs to another mechanism request",
                ));
            }
        }
        Ok(Self {
            id: MechanismOccurrenceId::derive(&request.id, topological_ordinal, &site, kind),
            topological_ordinal,
            site,
            kind,
            before,
            after,
            before_dependencies,
            after_dependencies,
        })
    }

    fn validate(&self, request: &MechanismRequestId) -> Result<(), MechanismValidationError> {
        if &self.id.request != request {
            return Err(invalid(
                "dynamic occurrence belongs to another mechanism request",
            ));
        }
        self.site
            .validate_scope(&request.analysis_program, "dynamic occurrence site")?;
        let expected =
            MechanismOccurrenceId::derive(request, self.topological_ordinal, &self.site, self.kind);
        if self.id != expected {
            return Err(invalid(
                "dynamic occurrence identity disagrees with its canonical site and ordinal",
            ));
        }
        if self.before.is_none() && self.after.is_none() {
            return Err(invalid(
                "paired mechanism occurrence has neither endpoint observation",
            ));
        }
        if self.before.is_none() && !self.before_dependencies.is_empty() {
            return Err(invalid(
                "before dependencies require a before-endpoint occurrence",
            ));
        }
        if self.after.is_none() && !self.after_dependencies.is_empty() {
            return Err(invalid(
                "after dependencies require an after-endpoint occurrence",
            ));
        }
        if let Some(outcome) = &self.before {
            outcome.validate(self.kind, &request.analysis_program)?;
        }
        if let Some(outcome) = &self.after {
            outcome.validate(self.kind, &request.analysis_program)?;
        }
        Ok(())
    }

    fn is_present_at(&self, before: bool) -> bool {
        if before {
            self.before.is_some()
        } else {
            self.after.is_some()
        }
    }

    fn dependencies_at(&self, before: bool) -> &BTreeSet<MechanismOccurrenceId> {
        if before {
            &self.before_dependencies
        } else {
            &self.after_dependencies
        }
    }
}

/// Canonical differential signature. Empty endpoint roots and nodes are a
/// valid empty signature; otherwise every retained occurrence must be
/// reachable from a root at the endpoint where that occurrence exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DynamicMechanismSignature {
    pub(crate) request: MechanismRequestId,
    pub(crate) before_roots: BTreeSet<MechanismOccurrenceId>,
    pub(crate) after_roots: BTreeSet<MechanismOccurrenceId>,
    pub(crate) nodes: BTreeMap<MechanismOccurrenceId, PairedOccurrenceNode>,
}

impl DynamicMechanismSignature {
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        before_roots: BTreeSet<MechanismOccurrenceId>,
        after_roots: BTreeSet<MechanismOccurrenceId>,
        nodes: impl IntoIterator<Item = PairedOccurrenceNode>,
    ) -> Result<Self, MechanismValidationError> {
        let mut by_id = BTreeMap::new();
        for node in nodes {
            let id = node.id.clone();
            if by_id.insert(id, node).is_some() {
                return Err(invalid(
                    "dynamic mechanism signature contains a duplicate occurrence ID",
                ));
            }
        }
        let signature = Self {
            request: request.id.clone(),
            before_roots,
            after_roots,
            nodes: by_id,
        };
        signature.validate()?;
        Ok(signature)
    }

    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        if self.nodes.is_empty() != (self.before_roots.is_empty() && self.after_roots.is_empty()) {
            return Err(invalid(
                "only the empty mechanism signature may have no endpoint roots or no nodes",
            ));
        }

        let mut ordinals = BTreeSet::new();
        for (id, node) in &self.nodes {
            if id != &node.id {
                return Err(invalid(
                    "dynamic mechanism node map key disagrees with the node occurrence ID",
                ));
            }
            node.validate(&self.request)?;
            if !ordinals.insert(node.topological_ordinal) {
                return Err(invalid(
                    "dynamic mechanism occurrences have duplicate topological ordinals",
                ));
            }
        }
        for (expected, actual) in (0_u64..).zip(ordinals.iter().copied()) {
            if expected != actual {
                return Err(invalid(
                    "dynamic mechanism topological ordinals must be contiguous from zero",
                ));
            }
        }

        self.validate_endpoint(true)?;
        self.validate_endpoint(false)?;
        Ok(())
    }

    fn validate_endpoint(&self, before: bool) -> Result<(), MechanismValidationError> {
        let (endpoint, roots) = if before {
            ("before", &self.before_roots)
        } else {
            ("after", &self.after_roots)
        };
        let endpoint_node_count = self
            .nodes
            .values()
            .filter(|node| node.is_present_at(before))
            .count();
        if (endpoint_node_count == 0) != roots.is_empty() {
            return Err(invalid(format!(
                "{endpoint} endpoint roots disagree with endpoint occurrence presence"
            )));
        }
        for root in roots {
            let node = self.nodes.get(root).ok_or_else(|| {
                invalid(format!(
                    "dynamic mechanism signature references a missing {endpoint} root"
                ))
            })?;
            if !node.is_present_at(before) {
                return Err(invalid(format!(
                    "dynamic mechanism {endpoint} root is absent at that endpoint"
                )));
            }
        }

        for node in self
            .nodes
            .values()
            .filter(|node| node.is_present_at(before))
        {
            for dependency in node.dependencies_at(before) {
                let dependency_node = self.nodes.get(dependency).ok_or_else(|| {
                    invalid(format!(
                        "dynamic mechanism {endpoint} occurrence references a missing dependency"
                    ))
                })?;
                if !dependency_node.is_present_at(before) {
                    return Err(invalid(format!(
                        "dynamic mechanism {endpoint} dependency is absent at that endpoint"
                    )));
                }
                if dependency_node.topological_ordinal >= node.topological_ordinal {
                    return Err(invalid(format!(
                        "dynamic mechanism {endpoint} dependency does not precede its dependent occurrence"
                    )));
                }
            }
        }

        let mut reachable = BTreeSet::new();
        let mut stack = roots.iter().cloned().collect::<Vec<_>>();
        while let Some(id) = stack.pop() {
            if !reachable.insert(id.clone()) {
                continue;
            }
            stack.extend(self.nodes[&id].dependencies_at(before).iter().cloned());
        }
        if reachable.len() != endpoint_node_count {
            return Err(invalid(format!(
                "dynamic mechanism signature retains a {endpoint} occurrence unreachable from its {endpoint} roots"
            )));
        }
        Ok(())
    }
}

/// Request-scoped content hash of one canonical complete signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismSignatureId {
    request: MechanismRequestId,
    digest: StableDigest,
}

impl MechanismSignatureId {
    fn derive(signature: &DynamicMechanismSignature) -> Self {
        let mut hasher = StableHasher::new(MECHANISM_SIGNATURE_HASH_V1);
        hasher.segment(&signature.request.digest.0);
        hasher.segment(b"before-roots");
        hasher.u128(signature.before_roots.len() as u128);
        for root in &signature.before_roots {
            hasher.segment(&root.digest.0);
        }
        hasher.segment(b"after-roots");
        hasher.u128(signature.after_roots.len() as u128);
        for root in &signature.after_roots {
            hasher.segment(&root.digest.0);
        }

        let mut nodes = signature.nodes.values().collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.topological_ordinal);
        hasher.u128(nodes.len() as u128);
        for node in nodes {
            hasher.u64(node.topological_ordinal);
            hasher.segment(&node.id.digest.0);
            hasher.segment(&node.site.digest.0);
            hasher.segment(node.kind.token());
            hash_optional_outcome(&mut hasher, node.before.as_ref());
            hash_optional_outcome(&mut hasher, node.after.as_ref());
            hasher.segment(b"before-dependencies");
            hasher.u128(node.before_dependencies.len() as u128);
            for dependency in &node.before_dependencies {
                hasher.segment(&dependency.digest.0);
            }
            hasher.segment(b"after-dependencies");
            hasher.u128(node.after_dependencies.len() as u128);
            for dependency in &node.after_dependencies {
                hasher.segment(&dependency.digest.0);
            }
        }
        Self {
            request: signature.request.clone(),
            digest: hasher.digest(),
        }
    }
}

fn hash_optional_outcome(hasher: &mut StableHasher, outcome: Option<&DynamicEventOutcome>) {
    let Some(outcome) = outcome else {
        hasher.segment(b"absent");
        return;
    };
    hasher.segment(b"present");
    match outcome {
        DynamicEventOutcome::RuleAttempt(RuleAttemptOutcome::HeadMismatch) => {
            hasher.segment(b"rule-attempt-head-mismatch")
        }
        DynamicEventOutcome::RuleAttempt(RuleAttemptOutcome::GuardFalse) => {
            hasher.segment(b"rule-attempt-guard-false")
        }
        DynamicEventOutcome::RuleAttempt(RuleAttemptOutcome::BodyFalse) => {
            hasher.segment(b"rule-attempt-body-false")
        }
        DynamicEventOutcome::RuleAttempt(RuleAttemptOutcome::Applicable) => {
            hasher.segment(b"rule-attempt-applicable")
        }
        DynamicEventOutcome::RuleSelection(RuleSelectionOutcome::NoApplicableRule) => {
            hasher.segment(b"rule-selection-none")
        }
        DynamicEventOutcome::RuleSelection(RuleSelectionOutcome::Selected(site)) => {
            hasher.segment(b"rule-selection-selected");
            hasher.segment(&site.digest.0);
        }
        DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then) => hasher.segment(b"if-then"),
        DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else) => hasher.segment(b"if-else"),
        DynamicEventOutcome::MatchDecision { arm_index } => {
            hasher.segment(b"match-arm");
            hasher.u32(*arm_index);
        }
        DynamicEventOutcome::ShortCircuit(ShortCircuitOutcome::SkippedRight { result }) => {
            hasher.segment(b"short-circuit-skipped-right");
            hasher.segment(&[*result as u8]);
        }
        DynamicEventOutcome::ShortCircuit(ShortCircuitOutcome::EvaluatedRight { result }) => {
            hasher.segment(b"short-circuit-evaluated-right");
            hasher.segment(&[*result as u8]);
        }
    }
}

/// Canonical, collision-checking signature interner. IDs are content hashes,
/// never discovery-order numbers.
#[derive(Debug)]
pub(crate) struct CanonicalSignatureInterner {
    request: MechanismRequestId,
    by_signature: BTreeMap<DynamicMechanismSignature, MechanismSignatureId>,
    by_id: BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
}

impl CanonicalSignatureInterner {
    pub(crate) fn new(request: &MechanismObservationRequest) -> Self {
        Self {
            request: request.id.clone(),
            by_signature: BTreeMap::new(),
            by_id: BTreeMap::new(),
        }
    }

    pub(crate) fn intern(
        &mut self,
        signature: DynamicMechanismSignature,
    ) -> Result<MechanismSignatureId, MechanismValidationError> {
        signature.validate()?;
        if signature.request != self.request {
            return Err(invalid(
                "canonical signature interner received another request's signature",
            ));
        }
        if let Some(id) = self.by_signature.get(&signature) {
            return Ok(id.clone());
        }
        let id = MechanismSignatureId::derive(&signature);
        if let Some(other) = self.by_id.get(&id) {
            if other != &signature {
                return Err(invalid(
                    "mechanism signature SHA-256 collision rejected by canonical interner",
                ));
            }
        }
        self.by_signature.insert(signature.clone(), id.clone());
        self.by_id.insert(id.clone(), signature);
        Ok(id)
    }

    pub(crate) fn len(&self) -> usize {
        self.by_id.len()
    }

    pub(crate) fn into_signatures(
        self,
    ) -> BTreeMap<MechanismSignatureId, DynamicMechanismSignature> {
        self.by_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MechanismCount {
    Exact(u128),
    LowerBound(u128),
}

impl MechanismCount {
    pub(crate) fn value(self) -> u128 {
        match self {
            Self::Exact(value) | Self::LowerBound(value) => value,
        }
    }

    pub(crate) fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MechanismEvidenceStatus {
    ScopeOpen,
    IncidenceOpen,
    MatchingClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum KnownTargetUntracedReason {
    TraceBudgetExhausted,
    ReplayUnavailable,
    ObservationUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MechanismIncidenceTerminal {
    OutsideTarget,
    Signature(MechanismSignatureId),
    KnownTargetUntraced(KnownTargetUntracedReason),
}

pub(crate) type MechanismIncidenceDag = OrderedDecisionDag<MechanismIncidenceTerminal>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MechanismTargetMembership {
    OutsideTarget,
    InsideTarget,
}

pub(crate) type MechanismTargetMembershipDag = OrderedDecisionDag<MechanismTargetMembership>;

/// Content identity of one exact canonical binary membership function for the
/// semantic target named by [`MechanismCaseTargetId`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismTargetMembershipId {
    case_target: MechanismCaseTargetId,
    digest: StableDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactMatchingTargetMembership {
    pub(crate) id: MechanismTargetMembershipId,
    pub(crate) membership: MechanismTargetMembershipDag,
}

impl ExactMatchingTargetMembership {
    /// Project only an already-closed authoritative case-classification DAG.
    /// Open eligibility or polarity is rejected rather than mapped outside the
    /// target.
    pub(crate) fn from_case_evidence(
        request: &MechanismObservationRequest,
        cases: &CaseDecisionDag,
    ) -> Result<Self, MechanismValidationError> {
        request.validate()?;
        cases
            .validate()
            .map_err(|error| invalid(format!("invalid authoritative case DAG: {error}")))?;
        if cases.axis_cardinalities() != request.axis_cardinalities.as_ref() {
            return Err(invalid(
                "authoritative matching target axes disagree with the mechanism request",
            ));
        }
        if cases.terminals().iter().any(|terminal| {
            matches!(
                terminal,
                CaseTerminal::EligibilityOpen(_) | CaseTerminal::AdmissibleOpen(_)
            )
        }) {
            return Err(invalid(
                "exact matching target membership cannot be projected from open case evidence",
            ));
        }

        let membership = cases
            .project_terminals(|terminal| match terminal {
                CaseTerminal::AdmissibleMatch => MechanismTargetMembership::InsideTarget,
                CaseTerminal::Excluded | CaseTerminal::AdmissibleNonmatch => {
                    MechanismTargetMembership::OutsideTarget
                }
                CaseTerminal::EligibilityOpen(_) | CaseTerminal::AdmissibleOpen(_) => {
                    // Rejected above before the infallible canonical projection.
                    MechanismTargetMembership::OutsideTarget
                }
            })
            .map_err(|error| {
                invalid(format!(
                    "cannot project authoritative matching membership: {error}"
                ))
            })?;
        let id = derive_target_membership_id(&request.case_target, &membership);
        let exact = Self { id, membership };
        exact.validate_for_request(request)?;
        Ok(exact)
    }

    fn validate_for_request(
        &self,
        request: &MechanismObservationRequest,
    ) -> Result<(), MechanismValidationError> {
        if self.id.case_target != request.case_target {
            return Err(invalid(
                "exact target membership belongs to another mechanism case target",
            ));
        }
        if self.membership.axis_cardinalities() != request.axis_cardinalities.as_ref() {
            return Err(invalid(
                "exact target membership axes disagree with the mechanism request",
            ));
        }
        self.membership
            .validate()
            .map_err(|error| invalid(format!("invalid exact target membership DAG: {error}")))?;
        let expected = derive_target_membership_id(&request.case_target, &self.membership);
        if self.id != expected {
            return Err(invalid(
                "exact target membership identity disagrees with its canonical binary DAG",
            ));
        }
        Ok(())
    }

    fn inside_count(&self) -> Result<u128, MechanismValidationError> {
        let counts = self
            .membership
            .terminal_counts()
            .map_err(|error| invalid(format!("cannot count exact target membership: {error}")))?;
        match counts.get(&MechanismTargetMembership::InsideTarget) {
            None => Ok(0),
            Some(CheckedCardinality::Exact(count)) => Ok(*count),
            Some(CheckedCardinality::ExceedsU128) => Err(invalid(
                "exact target membership exceeds the u128 evidence boundary",
            )),
        }
    }
}

fn derive_target_membership_id(
    case_target: &MechanismCaseTargetId,
    membership: &MechanismTargetMembershipDag,
) -> MechanismTargetMembershipId {
    let mut hasher = StableHasher::new(MECHANISM_TARGET_MEMBERSHIP_HASH_V1);
    hasher.segment(&case_target.digest.0);
    hasher.u128(membership.axis_cardinalities().len() as u128);
    for cardinality in membership.axis_cardinalities().iter().copied() {
        hasher.u128(cardinality);
    }
    match membership.root() {
        DecisionRoot::EmptySpace => hasher.segment(b"empty-space"),
        DecisionRoot::Target(target) => {
            hasher.segment(b"target");
            hash_decision_ref(&mut hasher, target);
        }
    }
    hasher.u128(membership.terminals().len() as u128);
    for terminal in membership.terminals() {
        hasher.segment(match terminal {
            MechanismTargetMembership::OutsideTarget => b"outside-target",
            MechanismTargetMembership::InsideTarget => b"inside-target",
        });
    }
    hasher.u128(membership.nodes().len() as u128);
    for node in membership.nodes() {
        hasher.u128(node.dimension_index() as u128);
        hasher.u128(node.arcs().len() as u128);
        for arc in node.arcs() {
            hasher.u128(arc.ordinals().intervals().len() as u128);
            for interval in arc.ordinals().intervals() {
                hasher.u128(interval.start().get());
                hasher.u128(interval.end_exclusive().get());
            }
            hash_decision_ref(&mut hasher, arc.child());
        }
    }
    MechanismTargetMembershipId {
        case_target: case_target.clone(),
        digest: hasher.digest(),
    }
}

fn hash_decision_ref(hasher: &mut StableHasher, target: DecisionRef) {
    match target {
        DecisionRef::Node(id) => {
            hasher.segment(b"node");
            hasher.u128(id.index() as u128);
        }
        DecisionRef::Terminal(id) => {
            hasher.segment(b"terminal");
            hasher.u128(id.index() as u128);
        }
    }
}

/// Conservation evidence for `S_req` and the actually traced subset `T`.
/// For `ScopeOpen`, the lower-bound value is the known portion of `S_req` and
/// is exactly `|T| + known_target_untraced`; additional target cases may exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismPopulationEvidence {
    pub(crate) status: MechanismEvidenceStatus,
    pub(crate) requested_target: MechanismCount,
    pub(crate) traced: u128,
    pub(crate) known_target_untraced: u128,
    pub(crate) incidence: Option<MechanismIncidenceDag>,
}

impl MechanismPopulationEvidence {
    pub(crate) fn new(
        status: MechanismEvidenceStatus,
        requested_target: MechanismCount,
        traced: u128,
        known_target_untraced: u128,
        incidence: Option<MechanismIncidenceDag>,
    ) -> Result<Self, MechanismValidationError> {
        let evidence = Self {
            status,
            requested_target,
            traced,
            known_target_untraced,
            incidence,
        };
        evidence.validate_shape()?;
        Ok(evidence)
    }

    fn validate_shape(&self) -> Result<(), MechanismValidationError> {
        let known_target = self
            .traced
            .checked_add(self.known_target_untraced)
            .ok_or_else(|| invalid("mechanism target accounting exceeds u128::MAX"))?;
        if self.requested_target.value() != known_target {
            return Err(invalid(format!(
                "mechanism target count {} does not equal traced {} plus known untraced {}",
                self.requested_target.value(),
                self.traced,
                self.known_target_untraced
            )));
        }
        match self.status {
            MechanismEvidenceStatus::ScopeOpen => {
                if self.requested_target.is_exact() || self.incidence.is_some() {
                    return Err(invalid(
                        "scope_open mechanism evidence requires a lower-bound target and no total incidence DAG",
                    ));
                }
            }
            MechanismEvidenceStatus::IncidenceOpen => {
                if !self.requested_target.is_exact()
                    || self.known_target_untraced == 0
                    || self.incidence.is_none()
                {
                    return Err(invalid(
                        "incidence_open mechanism evidence requires an exact target, a nonempty untraced remainder, and an incidence DAG",
                    ));
                }
            }
            MechanismEvidenceStatus::MatchingClosed => {
                if !self.requested_target.is_exact()
                    || self.known_target_untraced != 0
                    || self.incidence.is_none()
                    || self.requested_target.value() != self.traced
                {
                    return Err(invalid(
                        "matching_closed mechanism evidence requires an exact fully traced target and an incidence DAG",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismSignatureEvidence {
    pub(crate) signature: DynamicMechanismSignature,
    pub(crate) observed_support: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MechanismSignatureBinIncidence {
    pub(crate) signature: MechanismSignatureId,
    pub(crate) field_name: Box<str>,
    pub(crate) bin: MechanismNumericBin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismBinFieldEvidence {
    Unavailable(MechanismBinUnavailableReason),
    Observed {
        /// Cases classified into one declared bin for this field, partitioned
        /// by their already-known complete mechanism signature.
        observed_supports: BTreeMap<MechanismSignatureId, u128>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismBinUnavailableReason {
    ValueReplayUnavailable,
    ValueUnsupported,
}

/// Future proof seam. There is intentionally no constructor and no current
/// evidence path accepting this type: one replayed witness cannot certify a
/// homogeneous signature region.
#[allow(dead_code)]
pub(crate) struct HomogeneousSignatureRegionCertificate {
    _sealed_proof: (),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismObservedEvidence {
    pub(crate) request: MechanismObservationRequest,
    pub(crate) population: MechanismPopulationEvidence,
    pub(crate) exact_target_membership: Option<ExactMatchingTargetMembership>,
    pub(crate) signatures: BTreeMap<MechanismSignatureId, MechanismSignatureEvidence>,
    /// Retained replay witnesses only. This need not materialize all of `T`.
    pub(crate) sampled_traces: BTreeMap<ExploreCaseId, MechanismSignatureId>,
    pub(crate) bin_fields: BTreeMap<Box<str>, MechanismBinFieldEvidence>,
    /// Positive replayed case support for each signature/bin incidence. A
    /// signature may occur in several bins, so distinct-signature bin counts
    /// remain deliberately non-additive.
    pub(crate) signature_bin_supports: BTreeMap<MechanismSignatureBinIncidence, u128>,
}

impl MechanismObservedEvidence {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        request: MechanismObservationRequest,
        population: MechanismPopulationEvidence,
        exact_target_membership: Option<ExactMatchingTargetMembership>,
        canonical_signatures: BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
        observed_supports: BTreeMap<MechanismSignatureId, u128>,
        sampled_traces: BTreeMap<ExploreCaseId, MechanismSignatureId>,
        bin_fields: BTreeMap<Box<str>, MechanismBinFieldEvidence>,
        signature_bin_supports: BTreeMap<MechanismSignatureBinIncidence, u128>,
    ) -> Result<Self, MechanismValidationError> {
        let signatures = canonical_signatures
            .into_iter()
            .map(|(id, signature)| {
                let support = observed_supports.get(&id).copied().unwrap_or(0);
                (
                    id,
                    MechanismSignatureEvidence {
                        signature,
                        observed_support: support,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        if observed_supports
            .keys()
            .any(|id| !signatures.contains_key(id))
        {
            return Err(invalid(
                "mechanism support references a signature absent from the canonical interner",
            ));
        }
        let evidence = Self {
            request,
            population,
            exact_target_membership,
            signatures,
            sampled_traces,
            bin_fields,
            signature_bin_supports,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        self.request.validate()?;
        self.population.validate_shape()?;
        match self.population.status {
            MechanismEvidenceStatus::ScopeOpen => {
                if self.exact_target_membership.is_some() {
                    return Err(invalid(
                        "scope_open mechanism evidence must not claim exact target membership",
                    ));
                }
            }
            MechanismEvidenceStatus::IncidenceOpen | MechanismEvidenceStatus::MatchingClosed => {
                let target = self.exact_target_membership.as_ref().ok_or_else(|| {
                    invalid("exact mechanism scope requires authoritative target membership")
                })?;
                target.validate_for_request(&self.request)?;
                if target.inside_count()? != self.population.requested_target.value() {
                    return Err(invalid(
                        "authoritative target membership count disagrees with mechanism population accounting",
                    ));
                }
            }
        }

        let mut support_sum = 0_u128;
        for (id, evidence) in &self.signatures {
            evidence.signature.validate()?;
            if evidence.signature.request != self.request.id || id.request != self.request.id {
                return Err(invalid(
                    "mechanism signature evidence belongs to another observation request",
                ));
            }
            if id != &MechanismSignatureId::derive(&evidence.signature) {
                return Err(invalid(
                    "mechanism signature ID disagrees with its canonical occurrence DAG",
                ));
            }
            if evidence.observed_support == 0 {
                return Err(invalid(
                    "observed mechanism signatures must have positive support",
                ));
            }
            support_sum = support_sum
                .checked_add(evidence.observed_support)
                .ok_or_else(|| invalid("mechanism support total exceeds u128::MAX"))?;
        }
        if support_sum != self.population.traced {
            return Err(invalid(format!(
                "mechanism signature fibers cover {support_sum} traced cases, expected {}",
                self.population.traced
            )));
        }

        let mut retained_by_signature = BTreeMap::<MechanismSignatureId, u128>::new();
        for (case_id, signature) in &self.sampled_traces {
            validate_case_id(
                &self.request.axis_cardinalities,
                case_id,
                "retained mechanism trace",
            )?;
            if !self.signatures.contains_key(signature) {
                return Err(invalid(
                    "retained mechanism trace references an unknown signature",
                ));
            }
            let count = retained_by_signature.entry(signature.clone()).or_default();
            *count = count
                .checked_add(1)
                .ok_or_else(|| invalid("retained mechanism trace count exceeds u128::MAX"))?;
        }
        for (signature, retained) in retained_by_signature {
            if retained > self.signatures[&signature].observed_support {
                return Err(invalid(
                    "retained mechanism examples exceed the signature's observed support",
                ));
            }
        }

        match self.population.status {
            MechanismEvidenceStatus::ScopeOpen => {}
            MechanismEvidenceStatus::IncidenceOpen | MechanismEvidenceStatus::MatchingClosed => {
                self.validate_incidence()?
            }
        }
        self.validate_bins()?;
        Ok(())
    }

    fn validate_incidence(&self) -> Result<(), MechanismValidationError> {
        let incidence = self
            .population
            .incidence
            .as_ref()
            .ok_or_else(|| invalid("closed target scope lacks mechanism incidence"))?;
        if incidence.axis_cardinalities() != self.request.axis_cardinalities.as_ref() {
            return Err(invalid(
                "mechanism incidence axes disagree with the observation request",
            ));
        }
        incidence
            .validate()
            .map_err(|error| invalid(format!("invalid mechanism incidence DAG: {error}")))?;
        let target = self.exact_target_membership.as_ref().ok_or_else(|| {
            invalid("mechanism incidence lacks authoritative exact target membership")
        })?;
        let incidence_membership = incidence
            .project_terminals(|terminal| match terminal {
                MechanismIncidenceTerminal::OutsideTarget => {
                    MechanismTargetMembership::OutsideTarget
                }
                MechanismIncidenceTerminal::Signature(_)
                | MechanismIncidenceTerminal::KnownTargetUntraced(_) => {
                    MechanismTargetMembership::InsideTarget
                }
            })
            .map_err(|error| {
                invalid(format!(
                    "cannot project mechanism target incidence: {error}"
                ))
            })?;
        if incidence_membership != target.membership {
            return Err(invalid(
                "mechanism OutsideTarget/signature incidence disagrees with authoritative matching membership",
            ));
        }

        let terminal_counts = incidence
            .terminal_counts()
            .map_err(|error| invalid(format!("cannot count mechanism incidence: {error}")))?;
        let mut support_by_signature = BTreeMap::<MechanismSignatureId, u128>::new();
        let mut untraced = 0_u128;
        for (terminal, count) in terminal_counts {
            let CheckedCardinality::Exact(count) = count else {
                return Err(invalid(
                    "mechanism incidence cardinality exceeds the exact u128 evidence boundary",
                ));
            };
            match terminal {
                MechanismIncidenceTerminal::OutsideTarget => {}
                MechanismIncidenceTerminal::Signature(signature) => {
                    if !self.signatures.contains_key(&signature) {
                        return Err(invalid(
                            "mechanism incidence references an unknown signature",
                        ));
                    }
                    support_by_signature.insert(signature, count);
                }
                MechanismIncidenceTerminal::KnownTargetUntraced(_) => {
                    untraced = untraced
                        .checked_add(count)
                        .ok_or_else(|| invalid("mechanism untraced incidence exceeds u128::MAX"))?;
                }
            }
        }
        if untraced != self.population.known_target_untraced {
            return Err(invalid(format!(
                "mechanism incidence has {untraced} known target cases untraced, expected {}",
                self.population.known_target_untraced
            )));
        }

        let expected_supports = self
            .signatures
            .iter()
            .map(|(id, evidence)| (id.clone(), evidence.observed_support))
            .collect::<BTreeMap<_, _>>();
        if support_by_signature != expected_supports {
            return Err(invalid(
                "mechanism incidence signature fibers do not partition the traced population",
            ));
        }

        for (case_id, signature) in &self.sampled_traces {
            let terminal = incidence
                .terminal_for_path(case_id.ordinals())
                .map_err(|error| invalid(format!("cannot replay mechanism incidence: {error}")))?
                .ok_or_else(|| invalid("nonempty sampled trace has empty incidence space"))?;
            if terminal != &MechanismIncidenceTerminal::Signature(signature.clone()) {
                return Err(invalid(
                    "retained mechanism trace disagrees with the incidence DAG",
                ));
            }
        }

        for selected in self.request.sampling.selected_case_ids() {
            let terminal = incidence
                .terminal_for_path(selected.ordinals())
                .map_err(|error| {
                    invalid(format!("cannot inspect sampled mechanism case: {error}"))
                })?
                .ok_or_else(|| invalid("sampled mechanism case has empty incidence space"))?;
            match (self.population.status, terminal) {
                (_, MechanismIncidenceTerminal::OutsideTarget) => {
                    return Err(invalid(
                        "mechanism sampling plan contains a case outside its matching target",
                    ));
                }
                (
                    MechanismEvidenceStatus::MatchingClosed,
                    MechanismIncidenceTerminal::KnownTargetUntraced(_),
                ) => {
                    return Err(invalid(
                        "matching_closed mechanism evidence left a sampled case untraced",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_bins(&self) -> Result<(), MechanismValidationError> {
        let requested_fields = self
            .request
            .bin_fields
            .iter()
            .map(|field| (field.name.clone(), field))
            .collect::<BTreeMap<_, _>>();
        if self.bin_fields.len() != requested_fields.len()
            || self
                .bin_fields
                .keys()
                .any(|name| !requested_fields.contains_key(name.as_ref()))
        {
            return Err(invalid(
                "mechanism bin evidence must account for exactly the requested bin fields",
            ));
        }

        let mut binned_by_field_signature =
            BTreeMap::<(Box<str>, MechanismSignatureId), u128>::new();
        for (incidence, support) in &self.signature_bin_supports {
            if *support == 0 {
                return Err(invalid(
                    "mechanism signature/bin support must be strictly positive",
                ));
            }
            if !self.signatures.contains_key(&incidence.signature) {
                return Err(invalid(
                    "mechanism signature/bin support references an unknown signature",
                ));
            }
            let Some(field) = requested_fields.get(incidence.field_name.as_ref()) else {
                return Err(invalid(format!(
                    "mechanism signature/bin support references unknown field `{}`",
                    incidence.field_name
                )));
            };
            if !matches!(
                self.bin_fields.get(incidence.field_name.as_ref()),
                Some(MechanismBinFieldEvidence::Observed { observed_supports })
                    if observed_supports.contains_key(&incidence.signature)
            ) {
                return Err(invalid(format!(
                    "mechanism signature/bin support for `{}` lacks observed field evidence",
                    incidence.field_name
                )));
            }
            if !field.bins.contains(&incidence.bin) {
                return Err(invalid(format!(
                    "mechanism signature/bin support for `{}` references an undeclared bin",
                    incidence.field_name
                )));
            }
            let total = binned_by_field_signature
                .entry((incidence.field_name.clone(), incidence.signature.clone()))
                .or_default();
            *total = total.checked_add(*support).ok_or_else(|| {
                invalid("mechanism signature/bin support total exceeds u128::MAX")
            })?;
        }

        for (name, evidence) in &self.bin_fields {
            match evidence {
                MechanismBinFieldEvidence::Unavailable(_) => {
                    if binned_by_field_signature
                        .keys()
                        .any(|(field, _)| field == name)
                    {
                        return Err(invalid(format!(
                            "unavailable mechanism bin field `{name}` has positive support"
                        )));
                    }
                }
                MechanismBinFieldEvidence::Observed { observed_supports } => {
                    for (signature, support) in observed_supports {
                        let Some(signature_evidence) = self.signatures.get(signature) else {
                            return Err(invalid(format!(
                                "mechanism bin field `{name}` observes an unknown signature"
                            )));
                        };
                        if *support == 0 || *support > signature_evidence.observed_support {
                            return Err(invalid(format!(
                                "mechanism bin field `{name}` has invalid observed support for a signature"
                            )));
                        }
                    }
                    let actual = binned_by_field_signature
                        .iter()
                        .filter(|((field, _), _)| field == name)
                        .map(|((_, signature), support)| (signature.clone(), *support))
                        .collect::<BTreeMap<_, _>>();
                    if &actual != observed_supports {
                        return Err(invalid(format!(
                            "mechanism bins for `{name}` do not exactly partition the field's observed signature supports"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn bin_field_is_total(&self, observed_supports: &BTreeMap<MechanismSignatureId, u128>) -> bool {
        observed_supports.len() == self.signatures.len()
            && self.signatures.iter().all(|(signature, evidence)| {
                observed_supports.get(signature) == Some(&evidence.observed_support)
            })
    }

    pub(crate) fn distinct_signatures(&self) -> MechanismCount {
        let count = self.signatures.len() as u128;
        if self.population.status == MechanismEvidenceStatus::MatchingClosed {
            MechanismCount::Exact(count)
        } else {
            MechanismCount::LowerBound(count)
        }
    }

    pub(crate) fn signature_support(
        &self,
        signature: &MechanismSignatureId,
    ) -> Option<MechanismCount> {
        let observed = self.signatures.get(signature)?.observed_support;
        Some(
            if self.population.status == MechanismEvidenceStatus::MatchingClosed {
                MechanismCount::Exact(observed)
            } else {
                MechanismCount::LowerBound(observed)
            },
        )
    }

    pub(crate) fn mechanisms_in_bin(
        &self,
        field_name: &str,
        bin: MechanismNumericBin,
    ) -> Option<MechanismCount> {
        let field = self
            .request
            .bin_fields
            .iter()
            .find(|field| field.name.as_ref() == field_name)?;
        if !field.bins.contains(&bin) {
            return None;
        }
        let MechanismBinFieldEvidence::Observed { observed_supports } =
            self.bin_fields.get(field_name)?
        else {
            return None;
        };
        let count = self
            .signature_bin_supports
            .keys()
            .filter(|incidence| incidence.field_name.as_ref() == field_name && incidence.bin == bin)
            .count() as u128;
        Some(
            if self.population.status == MechanismEvidenceStatus::MatchingClosed
                && self.bin_field_is_total(observed_supports)
            {
                MechanismCount::Exact(count)
            } else {
                MechanismCount::LowerBound(count)
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismUnavailableReason {
    EndpointPairingUnavailable,
    DynamicTracingUnsupported,
    ReplayUnavailable,
}

/// `Unavailable` deliberately has no signature/count payload. Callers must not
/// render unavailable evidence as zero mechanisms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismEvidence {
    Unavailable {
        request: MechanismObservationRequest,
        reason: MechanismUnavailableReason,
    },
    Observed(MechanismObservedEvidence),
}

impl MechanismEvidence {
    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        match self {
            Self::Unavailable { request, .. } => request.validate(),
            Self::Observed(evidence) => evidence.validate(),
        }
    }

    pub(crate) fn distinct_signatures(&self) -> Option<MechanismCount> {
        match self {
            Self::Unavailable { .. } => None,
            Self::Observed(evidence) => Some(evidence.distinct_signatures()),
        }
    }
}

fn validate_case_id(
    axis_cardinalities: &[u128],
    case_id: &ExploreCaseId,
    what: &str,
) -> Result<(), MechanismValidationError> {
    if case_id.len() != axis_cardinalities.len() {
        return Err(invalid(format!(
            "{what} CaseId has {} ordinals for {} dimensions",
            case_id.len(),
            axis_cardinalities.len()
        )));
    }
    for (axis, (&ordinal, &cardinality)) in case_id
        .ordinals()
        .iter()
        .zip(axis_cardinalities)
        .enumerate()
    {
        if ordinal >= cardinality {
            return Err(invalid(format!(
                "{what} CaseId ordinal {ordinal} is outside axis {axis} with cardinality {cardinality}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeclarationId, DeclarationKind, ModuleId};

    fn analysis_program() -> AnalysisProgramId {
        AnalysisProgramId("11".repeat(32).into_boxed_str())
    }

    fn site(program: &AnalysisProgramId, name: &str, path: u32) -> MechanismSiteId {
        let expression = ExprSiteId {
            analysis_program: program.clone(),
            declaration: DeclarationId {
                module: ModuleId {
                    content_hash: "22".repeat(32).into_boxed_str(),
                    internal_path: Box::default(),
                },
                kind: DeclarationKind::Function,
                owner: None,
                name: name.to_string().into_boxed_str(),
                arity: Some(1),
                ordinal: 0,
            },
            normalized_declaration_ordinal: 0,
            ast_path: vec![path].into_boxed_slice(),
        };
        MechanismSiteId::from_expression_site(&expression).expect("site")
    }

    fn request(
        axis_cardinalities: Vec<u128>,
        sampling: MechanismSamplingPlan,
        bin_fields: Vec<MechanismBinField>,
    ) -> MechanismObservationRequest {
        let program = analysis_program();
        MechanismObservationRequest::new(
            program.clone(),
            MechanismQueryId::from_checked_query_bytes(b"query-and-domain"),
            MechanismObservationTarget::MatchingConfigurations,
            MechanismSemanticRootId::from_site(site(&program, "before", 0)),
            MechanismSemanticRootId::from_site(site(&program, "after", 0)),
            MechanismNormalization::DynamicControlV1,
            axis_cardinalities,
            sampling,
            bin_fields,
        )
        .expect("request")
    }

    fn selection_signature(
        request: &MechanismObservationRequest,
        selected: MechanismSiteId,
    ) -> DynamicMechanismSignature {
        let dispatch = site(&request.analysis_program, "dispatch", 1);
        let outcome = DynamicEventOutcome::RuleSelection(RuleSelectionOutcome::Selected(selected));
        let node = PairedOccurrenceNode::new(
            request,
            0,
            dispatch,
            DynamicEventKind::RuleSelection,
            Some(outcome.clone()),
            Some(outcome),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("occurrence");
        DynamicMechanismSignature::new(
            request,
            BTreeSet::from([node.id.clone()]),
            BTreeSet::from([node.id.clone()]),
            [node],
        )
        .expect("signature")
    }

    fn one_signature(
        request: &MechanismObservationRequest,
    ) -> (
        MechanismSignatureId,
        BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
    ) {
        let signature =
            selection_signature(request, site(&request.analysis_program, "selected-rule", 2));
        let mut interner = CanonicalSignatureInterner::new(request);
        let id = interner.intern(signature).expect("intern");
        (id, interner.into_signatures())
    }

    fn all_matching_target(request: &MechanismObservationRequest) -> ExactMatchingTargetMembership {
        let cases = CaseDecisionDag::from_sparse_classifications(
            request.axis_cardinalities.to_vec(),
            Vec::<(Vec<u128>, CaseTerminal)>::new(),
            CaseTerminal::AdmissibleMatch,
        )
        .expect("case target");
        ExactMatchingTargetMembership::from_case_evidence(request, &cases).expect("exact target")
    }

    #[test]
    fn representative_sampling_remains_a_scope_open_lower_bound() {
        let representative = ExploreCaseId::new(vec![0_u128]);
        let request = request(
            vec![2],
            MechanismSamplingPlan {
                result_representatives: BTreeSet::from([representative.clone()]),
                extrema_witnesses: BTreeSet::new(),
                required_case_ids: BTreeSet::new(),
            },
            Vec::new(),
        );
        let (signature, signatures) = one_signature(&request);
        let evidence = MechanismObservedEvidence::new(
            request,
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::ScopeOpen,
                MechanismCount::LowerBound(1),
                1,
                0,
                None,
            )
            .expect("population"),
            None,
            signatures,
            BTreeMap::from([(signature.clone(), 1)]),
            BTreeMap::from([(representative, signature.clone())]),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .expect("scope-open evidence");

        assert_eq!(
            evidence.distinct_signatures(),
            MechanismCount::LowerBound(1)
        );
        assert_eq!(
            evidence.signature_support(&signature),
            Some(MechanismCount::LowerBound(1))
        );
    }

    #[test]
    fn one_signature_may_have_nonadditive_incidence_in_two_loss_bins() {
        let program = analysis_program();
        let bins = vec![
            MechanismNumericBin::new(0, 5_000).expect("bin"),
            MechanismNumericBin::new(5_000, 10_000).expect("bin"),
        ];
        let field = MechanismBinField::new(
            "loss_ore",
            MechanismSemanticRootId::from_site(site(&program, "loss", 3)),
            bins.clone(),
        )
        .expect("field");
        let request = request(vec![2], MechanismSamplingPlan::empty(), vec![field]);
        let (signature, signatures) = one_signature(&request);
        let target = all_matching_target(&request);
        let incidence = MechanismIncidenceDag::from_sparse_classifications(
            vec![2],
            Vec::<(Vec<u128>, MechanismIncidenceTerminal)>::new(),
            MechanismIncidenceTerminal::Signature(signature.clone()),
        )
        .expect("incidence");
        let signature_bin_supports = BTreeMap::from([
            (
                MechanismSignatureBinIncidence {
                    signature: signature.clone(),
                    field_name: "loss_ore".into(),
                    bin: bins[0],
                },
                1,
            ),
            (
                MechanismSignatureBinIncidence {
                    signature: signature.clone(),
                    field_name: "loss_ore".into(),
                    bin: bins[1],
                },
                1,
            ),
        ]);
        let evidence = MechanismObservedEvidence::new(
            request,
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::MatchingClosed,
                MechanismCount::Exact(2),
                2,
                0,
                Some(incidence),
            )
            .expect("population"),
            Some(target),
            signatures,
            BTreeMap::from([(signature.clone(), 2)]),
            BTreeMap::new(),
            BTreeMap::from([(
                "loss_ore".into(),
                MechanismBinFieldEvidence::Observed {
                    observed_supports: BTreeMap::from([(signature.clone(), 2)]),
                },
            )]),
            signature_bin_supports,
        )
        .expect("closed evidence");

        assert_eq!(
            evidence.mechanisms_in_bin("loss_ore", bins[0]),
            Some(MechanismCount::Exact(1))
        );
        assert_eq!(
            evidence.mechanisms_in_bin("loss_ore", bins[1]),
            Some(MechanismCount::Exact(1))
        );
        assert_eq!(evidence.distinct_signatures(), MechanismCount::Exact(1));

        let mut incomplete = evidence.clone();
        incomplete.bin_fields.insert(
            "loss_ore".into(),
            MechanismBinFieldEvidence::Observed {
                observed_supports: BTreeMap::from([(signature.clone(), 1)]),
            },
        );
        incomplete
            .signature_bin_supports
            .remove(&MechanismSignatureBinIncidence {
                signature: signature.clone(),
                field_name: "loss_ore".into(),
                bin: bins[1],
            });
        incomplete.validate().expect("partial bin classification");
        assert_eq!(
            incomplete.mechanisms_in_bin("loss_ore", bins[0]),
            Some(MechanismCount::LowerBound(1))
        );

        let first = MechanismSignatureBinIncidence {
            signature,
            field_name: "loss_ore".into(),
            bin: bins[0],
        };
        incomplete.signature_bin_supports.insert(first, 0);
        assert!(incomplete.validate().is_err());
    }

    #[test]
    fn terminal_rule_tie_selection_changes_the_signature() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let first = selection_signature(
            &request,
            site(&request.analysis_program, "tied-terminal-a", 4),
        );
        let second = selection_signature(
            &request,
            site(&request.analysis_program, "tied-terminal-b", 4),
        );
        let mut interner = CanonicalSignatureInterner::new(&request);
        let first_id = interner.intern(first).expect("first");
        let second_id = interner.intern(second).expect("second");

        assert_ne!(first_id, second_id);
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn endpoint_dependencies_and_roots_are_validated_independently() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let shared_site = site(&request.analysis_program, "endpoint-swap", 5);
        let before_only = PairedOccurrenceNode::new(
            &request,
            0,
            shared_site.clone(),
            DynamicEventKind::IfDecision,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            None,
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("before-only node");
        let after_only = PairedOccurrenceNode::new(
            &request,
            0,
            shared_site,
            DynamicEventKind::IfDecision,
            None,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("after-only node");
        let before_signature = DynamicMechanismSignature::new(
            &request,
            BTreeSet::from([before_only.id.clone()]),
            BTreeSet::new(),
            [before_only],
        )
        .expect("before signature");
        let after_signature = DynamicMechanismSignature::new(
            &request,
            BTreeSet::new(),
            BTreeSet::from([after_only.id.clone()]),
            [after_only],
        )
        .expect("after signature");
        let mut interner = CanonicalSignatureInterner::new(&request);
        let before_id = interner.intern(before_signature).expect("before intern");
        let after_id = interner.intern(after_signature).expect("after intern");
        assert_ne!(before_id, after_id);

        let before = PairedOccurrenceNode::new(
            &request,
            0,
            site(&request.analysis_program, "before-dependency", 6),
            DynamicEventKind::IfDecision,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            None,
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("before node");
        let after = PairedOccurrenceNode::new(
            &request,
            1,
            site(&request.analysis_program, "after-dependent", 7),
            DynamicEventKind::IfDecision,
            None,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else)),
            BTreeSet::new(),
            BTreeSet::from([before.id.clone()]),
        )
        .expect("after node");

        assert!(DynamicMechanismSignature::new(
            &request,
            BTreeSet::from([before.id.clone()]),
            BTreeSet::from([after.id.clone()]),
            [before, after],
        )
        .is_err());
    }

    #[test]
    fn dynamic_control_v1_distinguishes_rule_attempt_failures() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let mut interner = CanonicalSignatureInterner::new(&request);
        let mut ids = BTreeSet::new();
        for attempt in [
            RuleAttemptOutcome::HeadMismatch,
            RuleAttemptOutcome::GuardFalse,
            RuleAttemptOutcome::BodyFalse,
            RuleAttemptOutcome::Applicable,
        ] {
            let outcome = DynamicEventOutcome::RuleAttempt(attempt);
            let node = PairedOccurrenceNode::new(
                &request,
                0,
                site(&request.analysis_program, "rule-attempt", 7),
                DynamicEventKind::RuleAttempt,
                Some(outcome.clone()),
                Some(outcome),
                BTreeSet::new(),
                BTreeSet::new(),
            )
            .expect("attempt node");
            let signature = DynamicMechanismSignature::new(
                &request,
                BTreeSet::from([node.id.clone()]),
                BTreeSet::from([node.id.clone()]),
                [node],
            )
            .expect("attempt signature");
            ids.insert(interner.intern(signature).expect("attempt intern"));
        }
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn equal_target_counts_do_not_hide_different_case_membership() {
        let request = request(vec![2], MechanismSamplingPlan::empty(), Vec::new());
        let (signature, signatures) = one_signature(&request);
        let cases = CaseDecisionDag::from_sparse_classifications(
            vec![2],
            [(vec![0_u128], CaseTerminal::AdmissibleMatch)],
            CaseTerminal::AdmissibleNonmatch,
        )
        .expect("authoritative cases");
        let target = ExactMatchingTargetMembership::from_case_evidence(&request, &cases)
            .expect("target membership");
        let wrong_incidence = MechanismIncidenceDag::from_sparse_classifications(
            vec![2],
            [(
                vec![1_u128],
                MechanismIncidenceTerminal::Signature(signature.clone()),
            )],
            MechanismIncidenceTerminal::OutsideTarget,
        )
        .expect("wrong incidence");

        assert!(MechanismObservedEvidence::new(
            request,
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::MatchingClosed,
                MechanismCount::Exact(1),
                1,
                0,
                Some(wrong_incidence),
            )
            .expect("population"),
            Some(target),
            signatures,
            BTreeMap::from([(signature, 1)]),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .is_err());
    }

    #[test]
    fn matching_closed_requires_exact_target_traced_conservation() {
        assert!(MechanismPopulationEvidence::new(
            MechanismEvidenceStatus::MatchingClosed,
            MechanismCount::Exact(3),
            2,
            0,
            None,
        )
        .is_err());

        let request = request(vec![2], MechanismSamplingPlan::empty(), Vec::new());
        let (signature, signatures) = one_signature(&request);
        let target = all_matching_target(&request);
        let incidence = MechanismIncidenceDag::from_sparse_classifications(
            vec![2],
            Vec::<(Vec<u128>, MechanismIncidenceTerminal)>::new(),
            MechanismIncidenceTerminal::Signature(signature.clone()),
        )
        .expect("incidence");
        let evidence = MechanismObservedEvidence::new(
            request,
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::MatchingClosed,
                MechanismCount::Exact(2),
                2,
                0,
                Some(incidence),
            )
            .expect("population"),
            Some(target),
            signatures,
            BTreeMap::from([(signature, 2)]),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .expect("closed evidence");

        assert_eq!(evidence.distinct_signatures(), MechanismCount::Exact(1));
    }
}
