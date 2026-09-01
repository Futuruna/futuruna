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
    run_stream::{ExactCaseSupport, ExploreCaseUniverse},
};
use crate::{
    AnalysisProgramId, CheckedCallTarget, CheckedCallableId, CheckedExploreQueryView,
    CheckedExpressionType, CheckedResolutionArtifacts, CheckedRuleCandidateResolution, ExprSiteId,
    RuleDispatchKey, RuleDispatchTier, Ty,
};

const MECHANISM_REQUEST_HASH_V3: &[u8] = b"futuruna.explore.mechanism-request.v3";
const MECHANISM_CASE_TARGET_HASH_V1: &[u8] = b"futuruna.explore.case-target.v1";
const MECHANISM_TARGET_MEMBERSHIP_HASH_V2: &[u8] = b"futuruna.explore.target-membership.v2";
const MECHANISM_SITE_HASH_V2: &[u8] = b"futuruna.explore.mechanism-site.v2";
const MECHANISM_OCCURRENCE_HASH_V2: &[u8] = b"futuruna.explore.mechanism-occurrence.v2";
const MECHANISM_SIGNATURE_HASH_V2: &[u8] = b"futuruna.explore.mechanism-signature.v2";
const CHECKED_MECHANISM_REQUEST_HASH_V1: &[u8] = b"futuruna.explore.checked-mechanism-request.v1";
const MECHANISM_CHECKED_QUERY_HASH_V2: &[u8] = b"futuruna.explore.mechanism-checked-query.v2";

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

/// Hash identity of the producer-minted relational question and analysis DAG.
///
/// Operational budgets and sampling order are deliberately absent. Production
/// construction consumes only a [`CheckedExploreQueryView`], whose accessor
/// has already revalidated the layered relation/admission/question identities
/// and analysis graph against the checked source declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismQueryId(StableDigest);

impl MechanismQueryId {
    pub(crate) fn from_checked_query(
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(&checked.artifact.identity.analysis_program)?;
        validate_lowercase_sha256(
            checked.analysis_graph_hash(),
            "checked Explore analysis-graph identity",
        )?;

        let mut hasher = StableHasher::new(MECHANISM_CHECKED_QUERY_HASH_V2);
        hasher.segment(
            checked
                .artifact
                .identity
                .analysis_program
                .as_str()
                .as_bytes(),
        );
        hasher.segment(&checked.relation_id().bytes());
        hasher.segment(&checked.admission_id().bytes());
        hasher.segment(&checked.question_id().bytes());
        hasher.segment(checked.analysis_graph_hash().as_bytes());
        Ok(Self(hasher.digest()))
    }

    /// Synthetic identity for isolated mechanism-core tests. Runtime callers
    /// must bind through [`Self::from_checked_query`].
    #[cfg(test)]
    pub(super) fn from_checked_query_bytes(canonical: &[u8]) -> Self {
        let mut hasher = StableHasher::new(b"futuruna.explore.synthetic-checked-query.test.v1");
        hasher.segment(canonical);
        Self(hasher.digest())
    }
}

fn validate_lowercase_sha256(value: &str, what: &str) -> Result<(), MechanismValidationError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(format!(
            "{what} must be a lowercase SHA-256 digest"
        )));
    }
    Ok(())
}

/// Kind of stable semantic program site retained by a mechanism trace.
///
/// Keeping the kind in the identity prevents an expression which happens to
/// share a structural prefix with a callable, dispatch family, or rule
/// candidate from being treated as the same observation site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MechanismSiteKind {
    Expression,
    Callable,
    RuleFamily,
    RuleCandidate,
}

impl MechanismSiteKind {
    fn token(self) -> &'static [u8] {
        match self {
            Self::Expression => b"expression",
            Self::Callable => b"callable",
            Self::RuleFamily => b"rule-family",
            Self::RuleCandidate => b"rule-candidate",
        }
    }
}

/// Stable semantic site, scoped to one checked analysis program. Spans,
/// filesystem paths, and runtime addresses never participate in this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismSiteId {
    analysis_program: AnalysisProgramId,
    kind: MechanismSiteKind,
    digest: StableDigest,
}

impl MechanismSiteId {
    /// Reconstruct a decoder proposal from canonical wire identity. This does
    /// not prove that the site occurs in the checked program; the sealing
    /// boundary must source-confirm it before evidence is accepted.
    pub(crate) fn from_untrusted_digest(
        analysis_program: AnalysisProgramId,
        kind: MechanismSiteKind,
        digest: [u8; 32],
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(&analysis_program)?;
        Ok(Self {
            analysis_program,
            kind,
            digest: StableDigest(digest),
        })
    }

    pub(crate) fn from_expression_site(
        site: &ExprSiteId,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(&site.analysis_program)?;
        let mut hasher = StableHasher::new(MECHANISM_SITE_HASH_V2);
        hasher.segment(MechanismSiteKind::Expression.token());
        hasher.segment(site.analysis_program.as_str().as_bytes());
        hasher.segment(site.declaration.semantic_key().as_bytes());
        hasher.u128(site.normalized_declaration_ordinal as u128);
        hasher.u128(site.ast_path.len() as u128);
        for child in site.ast_path.iter().copied() {
            hasher.u32(child);
        }
        Ok(Self {
            analysis_program: site.analysis_program.clone(),
            kind: MechanismSiteKind::Expression,
            digest: hasher.digest(),
        })
    }

    pub(crate) fn from_callable(
        analysis_program: &AnalysisProgramId,
        callable: &CheckedCallableId,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(analysis_program)?;
        let mut hasher = StableHasher::new(MECHANISM_SITE_HASH_V2);
        hasher.segment(MechanismSiteKind::Callable.token());
        hasher.segment(analysis_program.as_str().as_bytes());
        hasher.segment(callable.declaration.declaration.semantic_key().as_bytes());
        hasher.u128(callable.declaration.normalized_ordinal as u128);
        hasher.u128(callable.structural_path.len() as u128);
        for child in callable.structural_path.iter().copied() {
            hasher.u32(child);
        }
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: MechanismSiteKind::Callable,
            digest: hasher.digest(),
        })
    }

    pub(crate) fn from_rule_family(
        analysis_program: &AnalysisProgramId,
        family: &RuleDispatchKey,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(analysis_program)?;
        let mut hasher = StableHasher::new(MECHANISM_SITE_HASH_V2);
        hasher.segment(MechanismSiteKind::RuleFamily.token());
        hasher.segment(analysis_program.as_str().as_bytes());
        match family.scope.as_deref() {
            Some(scope) => {
                hasher.segment(b"scope-present");
                hasher.segment(scope.as_bytes());
            }
            None => hasher.segment(b"scope-absent"),
        }
        hasher.segment(family.name.as_bytes());
        hasher.u128(family.arity as u128);
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: MechanismSiteKind::RuleFamily,
            digest: hasher.digest(),
        })
    }

    pub(crate) fn from_rule_candidate(
        analysis_program: &AnalysisProgramId,
        candidate: &CheckedRuleCandidateResolution,
    ) -> Result<Self, MechanismValidationError> {
        validate_analysis_program_id(analysis_program)?;
        if std::iter::once(&candidate.head_site)
            .chain(candidate.condition_site.iter())
            .chain(candidate.value_site.iter())
            .any(|site| &site.analysis_program != analysis_program)
        {
            return Err(invalid(
                "rule candidate belongs to another analysis program",
            ));
        }
        let mut hasher = StableHasher::new(MECHANISM_SITE_HASH_V2);
        hasher.segment(MechanismSiteKind::RuleCandidate.token());
        hasher.segment(analysis_program.as_str().as_bytes());
        hasher.segment(candidate.declaration.declaration.semantic_key().as_bytes());
        hasher.u128(candidate.declaration.normalized_ordinal as u128);
        hasher.u128(candidate.statement_path.len() as u128);
        for child in candidate.statement_path.iter().copied() {
            hasher.u32(child);
        }
        hasher.segment(match candidate.tier {
            RuleDispatchTier::Exception => b"exception",
            RuleDispatchTier::ConditionalDefault => b"conditional-default",
            RuleDispatchTier::Clause => b"clause",
            RuleDispatchTier::UnconditionalDefault => b"unconditional-default",
        });
        hasher.u128(candidate.source_order as u128);
        Ok(Self {
            analysis_program: analysis_program.clone(),
            kind: MechanismSiteKind::RuleCandidate,
            digest: hasher.digest(),
        })
    }

    pub(crate) const fn kind(&self) -> MechanismSiteKind {
        self.kind
    }

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest.0
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
    pub(crate) fn from_checked_expression(
        resolutions: &CheckedResolutionArtifacts,
        site: &ExprSiteId,
    ) -> Result<Self, MechanismValidationError> {
        if site.analysis_program != resolutions.analysis_program
            || !resolutions.issues_for_reachable_sites([site]).is_empty()
        {
            return Err(invalid(
                "mechanism semantic root lacks coherent checked source resolution",
            ));
        }
        Self::from_expression_mechanism_site(MechanismSiteId::from_expression_site(site)?)
    }

    #[cfg(test)]
    pub(super) fn from_site(site: MechanismSiteId) -> Result<Self, MechanismValidationError> {
        Self::from_expression_mechanism_site(site)
    }

    fn from_expression_mechanism_site(
        site: MechanismSiteId,
    ) -> Result<Self, MechanismValidationError> {
        if site.kind != MechanismSiteKind::Expression {
            return Err(invalid(
                "mechanism semantic root must identify an expression semantic site",
            ));
        }
        Ok(Self(site))
    }

    fn validate_scope(
        &self,
        analysis_program: &AnalysisProgramId,
        what: &str,
    ) -> Result<(), MechanismValidationError> {
        self.0.validate_scope(analysis_program, what)?;
        if self.0.kind != MechanismSiteKind::Expression {
            return Err(invalid(format!(
                "{what} must identify an expression semantic site"
            )));
        }
        Ok(())
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

/// One checked endpoint observation template, independent of result fields.
///
/// The source-level callable is applied as `(state, context)` at this single
/// producer-minted expression site. Fresh replay evaluates the same template
/// once with Before and once with After; endpoint role is never encoded by two
/// unrelated expressions or positional output fields.
#[derive(Debug, Clone)]
pub(crate) struct MechanismObservationIr {
    pub(crate) endpoint_template: CheckedCallableId,
    pub(crate) template_site: ExprSiteId,
    pub(crate) template_root: MechanismSemanticRootId,
    pub(crate) state_type: Ty,
    pub(crate) context_type: Ty,
    pub(crate) observation_type: Ty,
    pub(crate) dependency_roots: Box<[MechanismSemanticRootId]>,
    pub(crate) normalization_version: u32,
}

impl PartialEq for MechanismObservationIr {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint_template == other.endpoint_template
            && self.template_site == other.template_site
            && self.template_root == other.template_root
            && mechanism_ty_structurally_equal(&self.state_type, &other.state_type)
            && mechanism_ty_structurally_equal(&self.context_type, &other.context_type)
            && mechanism_ty_structurally_equal(&self.observation_type, &other.observation_type)
            && self.dependency_roots == other.dependency_roots
            && self.normalization_version == other.normalization_version
    }
}

fn mechanism_ty_structurally_equal(left: &Ty, right: &Ty) -> bool {
    match (left, right) {
        (Ty::Name(left), Ty::Name(right)) | (Ty::Var(left), Ty::Var(right)) => left == right,
        (Ty::App(left_base, left_arguments), Ty::App(right_base, right_arguments)) => {
            mechanism_ty_structurally_equal(left_base, right_base)
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| mechanism_ty_structurally_equal(left, right))
        }
        (Ty::Arrow(left_input, left_output), Ty::Arrow(right_input, right_output)) => {
            mechanism_ty_structurally_equal(left_input, right_input)
                && mechanism_ty_structurally_equal(left_output, right_output)
        }
        (Ty::Ref(left), Ty::Ref(right))
        | (Ty::MutRef(left), Ty::MutRef(right))
        | (Ty::Shared(left), Ty::Shared(right))
        | (Ty::Optional(left), Ty::Optional(right)) => mechanism_ty_structurally_equal(left, right),
        (Ty::Unit, Ty::Unit) | (Ty::Hole, Ty::Hole) => true,
        _ => false,
    }
}

impl Eq for MechanismObservationIr {}

impl MechanismObservationIr {
    pub(crate) fn derive_checked(
        resolutions: &CheckedResolutionArtifacts,
        template_site: ExprSiteId,
        state_type: Ty,
        context_type: Ty,
    ) -> Result<Self, MechanismValidationError> {
        if template_site.analysis_program != resolutions.analysis_program {
            return Err(invalid(
                "mechanism observation template belongs to another checked program",
            ));
        }
        if !resolutions
            .issues_for_reachable_sites([&template_site])
            .is_empty()
        {
            return Err(invalid(
                "mechanism observation template has unresolved checked-source issues",
            ));
        }
        let resolution = resolutions
            .expressions
            .get(&template_site)
            .ok_or_else(|| invalid("mechanism observation template has no checked resolution"))?;
        let CheckedCallTarget::Function { callable, arity: 2 } = resolution
            .call_target
            .as_ref()
            .ok_or_else(|| invalid("mechanism observation template is not a checked call"))?
        else {
            return Err(invalid(
                "mechanism observation template must call one ordinary two-argument function",
            ));
        };
        let CheckedExpressionType::Resolved(observation_type) = &resolution.resolved_type else {
            return Err(invalid(
                "mechanism observation template has no checked Observation type",
            ));
        };
        let template_root =
            MechanismSemanticRootId::from_checked_expression(resolutions, &template_site)?;
        let observation = Self {
            endpoint_template: callable.clone(),
            template_site,
            template_root: template_root.clone(),
            state_type,
            context_type,
            observation_type: observation_type.clone(),
            dependency_roots: vec![template_root].into_boxed_slice(),
            normalization_version: 1,
        };
        observation.validate(&resolutions.analysis_program)?;
        Ok(observation)
    }

    fn validate(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), MechanismValidationError> {
        if self.normalization_version != 1 {
            return Err(invalid(
                "mechanism observation normalization version is unsupported",
            ));
        }
        if self.template_site.analysis_program != *analysis_program {
            return Err(invalid(
                "mechanism observation template crosses its checked program boundary",
            ));
        }
        let expected_template_root = MechanismSemanticRootId::from_expression_mechanism_site(
            MechanismSiteId::from_expression_site(&self.template_site)?,
        )?;
        if self.template_root != expected_template_root {
            return Err(invalid(
                "mechanism observation template root does not identify its checked template site",
            ));
        }
        self.template_root
            .validate_scope(analysis_program, "mechanism observation template root")?;
        if self.dependency_roots.is_empty() {
            return Err(invalid(
                "mechanism observation template must retain at least one dependency root",
            ));
        }
        for root in self.dependency_roots.iter() {
            root.validate_scope(analysis_program, "mechanism observation dependency root")?;
        }
        if self
            .dependency_roots
            .windows(2)
            .any(|roots| roots[0] >= roots[1])
        {
            return Err(invalid(
                "mechanism observation dependency roots must be strictly canonical",
            ));
        }
        if self
            .dependency_roots
            .binary_search(&self.template_root)
            .is_err()
        {
            return Err(invalid(
                "mechanism observation dependencies omit the checked template root",
            ));
        }
        Ok(())
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.u32(self.normalization_version);
        hasher.segment(
            self.endpoint_template
                .declaration
                .declaration
                .semantic_key()
                .as_bytes(),
        );
        hasher.u128(self.endpoint_template.declaration.normalized_ordinal as u128);
        hasher.u128(self.endpoint_template.structural_path.len() as u128);
        for child in self.endpoint_template.structural_path.iter().copied() {
            hasher.u32(child);
        }
        hasher.segment(self.template_site.analysis_program.as_str().as_bytes());
        hasher.segment(self.template_site.declaration.semantic_key().as_bytes());
        hasher.u128(self.template_site.normalized_declaration_ordinal as u128);
        hasher.u128(self.template_site.ast_path.len() as u128);
        for child in self.template_site.ast_path.iter().copied() {
            hasher.u32(child);
        }
        hasher.segment(&self.template_root.0.digest.0);
        hash_checked_type(hasher, &self.state_type);
        hash_checked_type(hasher, &self.context_type);
        hash_checked_type(hasher, &self.observation_type);
        hasher.u128(self.dependency_roots.len() as u128);
        for root in self.dependency_roots.iter() {
            hasher.segment(&root.0.digest.0);
        }
    }
}

fn hash_checked_type(hasher: &mut StableHasher, ty: &Ty) {
    match ty {
        Ty::Name(name) => {
            hasher.segment(b"name");
            hasher.segment(name.as_bytes());
        }
        Ty::App(constructor, arguments) => {
            hasher.segment(b"application");
            hash_checked_type(hasher, constructor);
            hasher.u128(arguments.len() as u128);
            for argument in arguments {
                hash_checked_type(hasher, argument);
            }
        }
        Ty::Arrow(parameter, result) => {
            hasher.segment(b"arrow");
            hash_checked_type(hasher, parameter);
            hash_checked_type(hasher, result);
        }
        Ty::Ref(inner) => {
            hasher.segment(b"reference");
            hash_checked_type(hasher, inner);
        }
        Ty::MutRef(inner) => {
            hasher.segment(b"mutable-reference");
            hash_checked_type(hasher, inner);
        }
        Ty::Shared(inner) => {
            hasher.segment(b"shared");
            hash_checked_type(hasher, inner);
        }
        Ty::Optional(inner) => {
            hasher.segment(b"optional");
            hash_checked_type(hasher, inner);
        }
        Ty::Var(name) => {
            hasher.segment(b"variable");
            hasher.segment(name.as_bytes());
        }
        Ty::Unit => hasher.segment(b"unit"),
        Ty::Hole => hasher.segment(b"hole"),
    }
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

impl MechanismRequestId {
    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest.0
    }

    pub(crate) fn analysis_program(&self) -> &AnalysisProgramId {
        &self.analysis_program
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismObservationRequest {
    pub(crate) id: MechanismRequestId,
    pub(crate) analysis_program: AnalysisProgramId,
    pub(crate) query: MechanismQueryId,
    pub(crate) target: MechanismObservationTarget,
    pub(crate) case_target: MechanismCaseTargetId,
    pub(crate) template: MechanismObservationIr,
    pub(crate) normalization: MechanismNormalization,
    pub(crate) axis_cardinalities: Box<[u128]>,
    pub(crate) sampling: MechanismSamplingPlan,
    pub(crate) bin_fields: Box<[MechanismBinField]>,
}

/// Case-level mechanism material the run is authorized to retain and expose.
/// This is deliberately separate from [`MechanismRequestId`]: disclosure may
/// change storage and publication without renaming the mathematical dynamic
/// signature `Sigma_(q,h)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MechanismIncidenceDisclosure {
    /// Publish aggregate signature and bin counts, but no case-to-signature
    /// incidence graph.
    SummaryOnly,
    /// Retain and publish the complete matching-case incidence relation when
    /// it closes.
    FullMatchingIncidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismDisclosureV1 {
    pub(crate) incidence: MechanismIncidenceDisclosure,
    pub(crate) retained_examples_per_signature: u32,
}

impl MechanismDisclosureV1 {
    pub(crate) const fn new(
        incidence: MechanismIncidenceDisclosure,
        retained_examples_per_signature: u32,
    ) -> Self {
        Self {
            incidence,
            retained_examples_per_signature,
        }
    }
}

/// Resume identity of the checked mechanism request, including its canonical
/// sampling and disclosure contracts. Operational trace order and time/work
/// budgets remain outside this digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CheckedMechanismRequestId {
    analysis_program: AnalysisProgramId,
    digest: StableDigest,
}

impl CheckedMechanismRequestId {
    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedMechanismObservationRequestV1 {
    pub(crate) id: CheckedMechanismRequestId,
    pub(crate) observation: MechanismObservationRequest,
    pub(crate) disclosure: MechanismDisclosureV1,
}

impl CheckedMechanismObservationRequestV1 {
    pub(crate) fn new(
        observation: MechanismObservationRequest,
        disclosure: MechanismDisclosureV1,
    ) -> Result<Self, MechanismValidationError> {
        observation.validate()?;
        let id = derive_checked_request_id(&observation, disclosure);
        Ok(Self {
            id,
            observation,
            disclosure,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        self.observation.validate()?;
        if self.id != derive_checked_request_id(&self.observation, self.disclosure) {
            return Err(invalid(
                "checked mechanism request identity disagrees with its observation and disclosure contract",
            ));
        }
        Ok(())
    }
}

fn derive_checked_request_id(
    observation: &MechanismObservationRequest,
    disclosure: MechanismDisclosureV1,
) -> CheckedMechanismRequestId {
    let mut hasher = StableHasher::new(CHECKED_MECHANISM_REQUEST_HASH_V1);
    hasher.segment(&observation.id.digest.0);
    for (name, cases) in [
        (
            b"result-representatives".as_slice(),
            &observation.sampling.result_representatives,
        ),
        (
            b"extrema-witnesses".as_slice(),
            &observation.sampling.extrema_witnesses,
        ),
        (
            b"required-case-ids".as_slice(),
            &observation.sampling.required_case_ids,
        ),
    ] {
        hasher.segment(name);
        hasher.u128(cases.len() as u128);
        for case_id in cases {
            hasher.u128(case_id.ordinals().len() as u128);
            for ordinal in case_id.ordinals().iter().copied() {
                hasher.u128(ordinal);
            }
        }
    }
    hasher.segment(match disclosure.incidence {
        MechanismIncidenceDisclosure::SummaryOnly => b"summary-only",
        MechanismIncidenceDisclosure::FullMatchingIncidence => b"full-matching-incidence",
    });
    hasher.u32(disclosure.retained_examples_per_signature);
    CheckedMechanismRequestId {
        analysis_program: observation.analysis_program.clone(),
        digest: hasher.digest(),
    }
}

impl MechanismObservationRequest {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        analysis_program: AnalysisProgramId,
        query: MechanismQueryId,
        target: MechanismObservationTarget,
        template: MechanismObservationIr,
        normalization: MechanismNormalization,
        axis_cardinalities: impl Into<Box<[u128]>>,
        sampling: MechanismSamplingPlan,
        bin_fields: impl Into<Box<[MechanismBinField]>>,
    ) -> Result<Self, MechanismValidationError> {
        let axis_cardinalities = axis_cardinalities.into();
        let bin_fields = bin_fields.into();
        validate_request_parts(
            &analysis_program,
            &template,
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
            &template,
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
            template,
            normalization,
            axis_cardinalities,
            sampling,
            bin_fields,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), MechanismValidationError> {
        validate_request_parts(
            &self.analysis_program,
            &self.template,
            &self.axis_cardinalities,
            &self.sampling,
            &self.bin_fields,
        )?;
        let expected = derive_request_id(
            &self.analysis_program,
            &self.query,
            self.target,
            &self.case_target,
            &self.template,
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
    template: &MechanismObservationIr,
    axis_cardinalities: &[u128],
    sampling: &MechanismSamplingPlan,
    bin_fields: &[MechanismBinField],
) -> Result<(), MechanismValidationError> {
    validate_analysis_program_id(analysis_program)?;
    template.validate(analysis_program)?;
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
    template: &MechanismObservationIr,
    normalization: MechanismNormalization,
    axis_cardinalities: &[u128],
    bin_fields: &[MechanismBinField],
) -> MechanismRequestId {
    let mut hasher = StableHasher::new(MECHANISM_REQUEST_HASH_V3);
    hasher.segment(analysis_program.as_str().as_bytes());
    hasher.segment(&(query.0).0);
    hasher.segment(match target {
        MechanismObservationTarget::MatchingConfigurations => b"matching-configurations",
    });
    hasher.segment(&case_target.digest.0);
    template.hash_into(&mut hasher);
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
            if site.kind != MechanismSiteKind::RuleCandidate {
                return Err(invalid(
                    "selected rule outcome must identify a checked rule-candidate site",
                ));
            }
        }
        Ok(())
    }
}

/// Stable checked callee used in an activation path. A rule family is kept
/// distinct from an ordinary function even when their display names match.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MechanismCallableSiteId {
    Function(MechanismSiteId),
    RuleFamily(MechanismSiteId),
}

impl MechanismCallableSiteId {
    fn from_checked_target(
        analysis_program: &AnalysisProgramId,
        target: &CheckedCallTarget,
    ) -> Result<Self, MechanismValidationError> {
        match target {
            CheckedCallTarget::Function { callable, .. } => {
                Self::function(MechanismSiteId::from_callable(analysis_program, callable)?)
            }
            CheckedCallTarget::RuleFamily(family) => {
                Self::rule_family(MechanismSiteId::from_rule_family(analysis_program, family)?)
            }
            CheckedCallTarget::Builtin { .. }
            | CheckedCallTarget::Constructor { .. }
            | CheckedCallTarget::BoundCallable { .. }
            | CheckedCallTarget::ScopedMember { .. } => Err(invalid(
                "mechanism endpoint call target is not traceable as a function or rule family",
            )),
        }
    }

    pub(crate) fn function(site: MechanismSiteId) -> Result<Self, MechanismValidationError> {
        if site.kind != MechanismSiteKind::Callable {
            return Err(invalid(
                "mechanism function activation requires a callable semantic site",
            ));
        }
        Ok(Self::Function(site))
    }

    pub(crate) fn rule_family(site: MechanismSiteId) -> Result<Self, MechanismValidationError> {
        if site.kind != MechanismSiteKind::RuleFamily {
            return Err(invalid(
                "mechanism rule activation requires a rule-family semantic site",
            ));
        }
        Ok(Self::RuleFamily(site))
    }

    fn site(&self) -> &MechanismSiteId {
        match self {
            Self::Function(site) | Self::RuleFamily(site) => site,
        }
    }

    fn token(&self) -> &'static [u8] {
        match self {
            Self::Function(_) => b"function",
            Self::RuleFamily(_) => b"rule-family",
        }
    }

    fn validate(
        &self,
        analysis_program: &AnalysisProgramId,
        what: &str,
    ) -> Result<(), MechanismValidationError> {
        let site = self.site();
        site.validate_scope(analysis_program, what)?;
        match (self, site.kind) {
            (Self::Function(_), MechanismSiteKind::Callable)
            | (Self::RuleFamily(_), MechanismSiteKind::RuleFamily) => Ok(()),
            _ => Err(invalid(format!(
                "{what} kind disagrees with its stable semantic site"
            ))),
        }
    }
}

/// One outcome-free frame in the enclosing dynamic activation path. The
/// invocation ordinal is local to the same call site and enclosing path, so
/// repeated calls remain distinct without using process order or addresses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismActivationStepV1 {
    pub(crate) call_site: MechanismSiteId,
    pub(crate) callee: MechanismCallableSiteId,
    pub(crate) invocation_ordinal: u32,
}

impl MechanismActivationStepV1 {
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        call_site: MechanismSiteId,
        callee: MechanismCallableSiteId,
        invocation_ordinal: u32,
    ) -> Result<Self, MechanismValidationError> {
        call_site.validate_scope(&request.analysis_program, "activation call site")?;
        if call_site.kind != MechanismSiteKind::Expression {
            return Err(invalid(
                "mechanism activation call site must be an expression site",
            ));
        }
        callee.validate(&request.analysis_program, "activation callee")?;
        Ok(Self {
            call_site,
            callee,
            invocation_ordinal,
        })
    }

    fn validate(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), MechanismValidationError> {
        self.call_site
            .validate_scope(analysis_program, "activation call site")?;
        if self.call_site.kind != MechanismSiteKind::Expression {
            return Err(invalid(
                "mechanism activation call site must be an expression site",
            ));
        }
        self.callee.validate(analysis_program, "activation callee")
    }
}

/// Outcome-free pairing key for one occurrence at the lower and upper
/// endpoints. Adding an earlier before-only event cannot rename this slot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MechanismOccurrenceSlotV1 {
    pub(crate) root_index: u32,
    pub(crate) activation_path: Box<[MechanismActivationStepV1]>,
    pub(crate) site: MechanismSiteId,
    pub(crate) kind: DynamicEventKind,
    pub(crate) visit_ordinal: u32,
}

impl MechanismOccurrenceSlotV1 {
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        root_index: u32,
        activation_path: impl Into<Box<[MechanismActivationStepV1]>>,
        site: MechanismSiteId,
        kind: DynamicEventKind,
        visit_ordinal: u32,
    ) -> Result<Self, MechanismValidationError> {
        let slot = Self {
            root_index,
            activation_path: activation_path.into(),
            site,
            kind,
            visit_ordinal,
        };
        slot.validate(request)?;
        Ok(slot)
    }

    fn validate(
        &self,
        request: &MechanismObservationRequest,
    ) -> Result<(), MechanismValidationError> {
        self.validate_for_program(&request.analysis_program)
    }

    fn validate_for_program(
        &self,
        analysis_program: &AnalysisProgramId,
    ) -> Result<(), MechanismValidationError> {
        // DynamicControlV1 currently binds one paired observation root. Keep
        // the index explicit so a future multi-root normalization can extend
        // the wire shape without conflating existing signatures.
        if self.root_index != 0 {
            return Err(invalid(
                "dynamic-control-v1 mechanism slot has an unknown root index",
            ));
        }
        self.site
            .validate_scope(analysis_program, "dynamic occurrence site")?;
        let site_kind_is_valid = matches!(
            (self.kind, self.site.kind),
            (
                DynamicEventKind::RuleAttempt,
                MechanismSiteKind::RuleCandidate
            ) | (
                DynamicEventKind::RuleSelection,
                MechanismSiteKind::RuleFamily
            ) | (
                DynamicEventKind::IfDecision
                    | DynamicEventKind::MatchDecision
                    | DynamicEventKind::ShortCircuitAnd
                    | DynamicEventKind::ShortCircuitOr,
                MechanismSiteKind::Expression
            )
        );
        if !site_kind_is_valid {
            return Err(invalid(
                "dynamic occurrence kind disagrees with its stable semantic-site kind",
            ));
        }
        for step in self.activation_path.iter() {
            step.validate(analysis_program)?;
        }
        Ok(())
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.u32(self.root_index);
        hasher.u128(self.activation_path.len() as u128);
        for step in self.activation_path.iter() {
            hasher.segment(&step.call_site.digest.0);
            hasher.segment(step.callee.token());
            hasher.segment(&step.callee.site().digest.0);
            hasher.u32(step.invocation_ordinal);
        }
        hasher.segment(&self.site.digest.0);
        hasher.segment(self.kind.token());
        hasher.u32(self.visit_ordinal);
    }
}

/// One endpoint-local observation before lower/upper pairing. Dependencies use
/// semantic slots rather than endpoint discovery indices.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EndpointOccurrenceV1 {
    pub(crate) slot: MechanismOccurrenceSlotV1,
    pub(crate) outcome: DynamicEventOutcome,
    pub(crate) dependencies: BTreeSet<MechanismOccurrenceSlotV1>,
}

impl EndpointOccurrenceV1 {
    pub(crate) fn new(
        slot: MechanismOccurrenceSlotV1,
        outcome: DynamicEventOutcome,
        dependencies: BTreeSet<MechanismOccurrenceSlotV1>,
    ) -> Self {
        Self {
            slot,
            outcome,
            dependencies,
        }
    }
}

/// Complete bounded dynamic slice for one endpoint. Duplicate occurrence slots
/// are rejected even when their payloads compare equal: accepting them would
/// silently guess how repeated visits should pair across endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicEndpointTraceV1 {
    pub(crate) roots: BTreeSet<MechanismOccurrenceSlotV1>,
    pub(crate) occurrences: BTreeMap<MechanismOccurrenceSlotV1, EndpointOccurrenceV1>,
}

impl DynamicEndpointTraceV1 {
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        roots: BTreeSet<MechanismOccurrenceSlotV1>,
        occurrences: impl IntoIterator<Item = EndpointOccurrenceV1>,
    ) -> Result<Self, MechanismValidationError> {
        let mut by_slot = BTreeMap::new();
        for occurrence in occurrences {
            let slot = occurrence.slot.clone();
            if by_slot.insert(slot, occurrence).is_some() {
                return Err(invalid(
                    "dynamic endpoint trace contains an ambiguous duplicate occurrence slot",
                ));
            }
        }
        let trace = Self {
            roots,
            occurrences: by_slot,
        };
        trace.validate(request)?;
        Ok(trace)
    }

    fn validate(
        &self,
        request: &MechanismObservationRequest,
    ) -> Result<(), MechanismValidationError> {
        if self.occurrences.is_empty() != self.roots.is_empty() {
            return Err(invalid(
                "only an empty endpoint trace may have no roots or no occurrences",
            ));
        }
        for root in &self.roots {
            if !self.occurrences.contains_key(root) {
                return Err(invalid(
                    "dynamic endpoint trace references a missing root slot",
                ));
            }
        }
        let mut remaining_dependencies = BTreeMap::new();
        let mut dependents =
            BTreeMap::<MechanismOccurrenceSlotV1, Vec<MechanismOccurrenceSlotV1>>::new();
        for (slot, occurrence) in &self.occurrences {
            if slot != &occurrence.slot {
                return Err(invalid(
                    "dynamic endpoint occurrence map key disagrees with its semantic slot",
                ));
            }
            slot.validate(request)?;
            occurrence
                .outcome
                .validate(slot.kind, &request.analysis_program)?;
            remaining_dependencies.insert(slot.clone(), occurrence.dependencies.len());
            for dependency in &occurrence.dependencies {
                if !self.occurrences.contains_key(dependency) {
                    return Err(invalid(
                        "dynamic endpoint occurrence references a missing dependency slot",
                    ));
                }
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .push(slot.clone());
            }
        }

        let mut ready = remaining_dependencies
            .iter()
            .filter_map(|(slot, count)| (*count == 0).then_some(slot.clone()))
            .collect::<BTreeSet<_>>();
        let mut ordered = 0_usize;
        while let Some(slot) = ready.pop_first() {
            ordered += 1;
            for dependent in dependents.get(&slot).into_iter().flatten() {
                let count = remaining_dependencies
                    .get_mut(dependent)
                    .expect("validated endpoint dependent must be present");
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
        if ordered != self.occurrences.len() {
            return Err(invalid(
                "dynamic endpoint occurrence dependencies contain a cycle",
            ));
        }

        let mut reachable = BTreeSet::new();
        let mut stack = self.roots.iter().cloned().collect::<Vec<_>>();
        while let Some(slot) = stack.pop() {
            if !reachable.insert(slot.clone()) {
                continue;
            }
            stack.extend(self.occurrences[&slot].dependencies.iter().cloned());
        }
        if reachable.len() != self.occurrences.len() {
            return Err(invalid(
                "dynamic endpoint trace retains an occurrence unreachable from its roots",
            ));
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
    fn derive(request: &MechanismRequestId, slot: &MechanismOccurrenceSlotV1) -> Self {
        let mut hasher = StableHasher::new(MECHANISM_OCCURRENCE_HASH_V2);
        hasher.segment(&request.digest.0);
        slot.hash_into(&mut hasher);
        Self {
            request: request.clone(),
            digest: hasher.digest(),
        }
    }

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest.0
    }
}

/// Compact storage for zero, one or two endpoint-qualified occurrence nodes
/// joined by stable-slot correspondence. The logical vertices are
/// `(before, id)` and `(after, id)`, not one uncoloured vertex: this preserves
/// two genuine endpoint DAGs even when their dependency orders reverse.
/// Exactly one or both endpoint observations are present, so before-only and
/// after-only reachability are preserved rather than erased as "no change".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PairedOccurrenceNode {
    pub(crate) id: MechanismOccurrenceId,
    pub(crate) slot: MechanismOccurrenceSlotV1,
    pub(crate) before: Option<DynamicEventOutcome>,
    pub(crate) after: Option<DynamicEventOutcome>,
    pub(crate) before_dependencies: BTreeSet<MechanismOccurrenceId>,
    pub(crate) after_dependencies: BTreeSet<MechanismOccurrenceId>,
}

impl PairedOccurrenceNode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        request: &MechanismObservationRequest,
        slot: MechanismOccurrenceSlotV1,
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
        slot.validate(request)?;
        if let Some(outcome) = &before {
            outcome.validate(slot.kind, &request.analysis_program)?;
        }
        if let Some(outcome) = &after {
            outcome.validate(slot.kind, &request.analysis_program)?;
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
            id: MechanismOccurrenceId::derive(&request.id, &slot),
            slot,
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
        self.slot.validate_for_program(&request.analysis_program)?;
        let expected = MechanismOccurrenceId::derive(request, &self.slot);
        if self.id != expected {
            return Err(invalid(
                "dynamic occurrence identity disagrees with its canonical semantic slot",
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
            outcome.validate(self.slot.kind, &request.analysis_program)?;
        }
        if let Some(outcome) = &self.after {
            outcome.validate(self.slot.kind, &request.analysis_program)?;
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

/// Outcome-free shape used only to prove that local occurrence ordinals are
/// safe to pair across the two endpoint traces. An ordinal is meaningful only
/// within the same enclosing activation, semantic site and event kind. If one
/// endpoint has an extra visit or invocation in a group which exists at both
/// endpoints, ordinal pairing could silently shift and therefore fails closed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PairingActivationBaseV1 {
    call_site: MechanismSiteId,
    callee: MechanismCallableSiteId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PairingOccurrenceBaseV1 {
    site: MechanismSiteId,
    kind: DynamicEventKind,
}

#[derive(Debug, Default)]
struct PairingShapeNodeV1 {
    activations: BTreeMap<PairingActivationBaseV1, BTreeMap<u32, PairingShapeNodeV1>>,
    occurrences: BTreeMap<PairingOccurrenceBaseV1, BTreeSet<u32>>,
}

#[derive(Debug, Default)]
struct EndpointPairingShapeV1 {
    roots: BTreeMap<u32, PairingShapeNodeV1>,
}

impl EndpointPairingShapeV1 {
    fn from_trace(trace: &DynamicEndpointTraceV1) -> Result<Self, MechanismValidationError> {
        Self::from_slots(trace.occurrences.keys())
    }

    fn from_slots<'slot>(
        slots: impl IntoIterator<Item = &'slot MechanismOccurrenceSlotV1>,
    ) -> Result<Self, MechanismValidationError> {
        let mut shape = Self::default();
        for slot in slots {
            let mut node = shape.roots.entry(slot.root_index).or_default();
            for step in slot.activation_path.iter() {
                let activation = PairingActivationBaseV1 {
                    call_site: step.call_site.clone(),
                    callee: step.callee.clone(),
                };
                node = node
                    .activations
                    .entry(activation)
                    .or_default()
                    .entry(step.invocation_ordinal)
                    .or_default();
            }
            node.occurrences
                .entry(PairingOccurrenceBaseV1 {
                    site: slot.site.clone(),
                    kind: slot.kind,
                })
                .or_default()
                .insert(slot.visit_ordinal);
        }
        for node in shape.roots.values() {
            node.validate_contiguous_ordinals()?;
        }
        Ok(shape)
    }

    fn ensure_unambiguous_with(&self, other: &Self) -> Result<(), MechanismValidationError> {
        for (root, before) in &self.roots {
            if let Some(after) = other.roots.get(root) {
                before.ensure_unambiguous_with(after)?;
            }
        }
        Ok(())
    }
}

impl PairingShapeNodeV1 {
    fn validate_contiguous_ordinals(&self) -> Result<(), MechanismValidationError> {
        for invocations in self.activations.values() {
            if !ordinals_are_zero_based(invocations.keys().copied()) {
                return Err(invalid(
                    "dynamic endpoint trace has a non-contiguous activation invocation ordinal",
                ));
            }
            for child in invocations.values() {
                child.validate_contiguous_ordinals()?;
            }
        }
        for visits in self.occurrences.values() {
            if !ordinals_are_zero_based(visits.iter().copied()) {
                return Err(invalid(
                    "dynamic endpoint trace has a non-contiguous occurrence visit ordinal",
                ));
            }
        }
        Ok(())
    }

    fn ensure_unambiguous_with(&self, other: &Self) -> Result<(), MechanismValidationError> {
        for (activation, before_invocations) in &self.activations {
            let Some(after_invocations) = other.activations.get(activation) else {
                continue;
            };
            if !before_invocations.keys().eq(after_invocations.keys()) {
                return Err(invalid(
                    "differential endpoint pairing is ambiguous because a shared activation has different invocation multiplicity",
                ));
            }
            for (ordinal, before_child) in before_invocations {
                before_child.ensure_unambiguous_with(
                    after_invocations
                        .get(ordinal)
                        .expect("equal invocation ordinals must be present"),
                )?;
            }
        }
        for (occurrence, before_visits) in &self.occurrences {
            let Some(after_visits) = other.occurrences.get(occurrence) else {
                continue;
            };
            if before_visits != after_visits {
                return Err(invalid(
                    "differential endpoint pairing is ambiguous because a shared semantic event has different visit multiplicity",
                ));
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

/// Canonical differential signature containing two edge-coloured endpoint
/// DAGs over a shared occurrence set. Each endpoint projection is acyclic;
/// their uncoloured edge union need not be. Empty endpoint roots and nodes are
/// a valid empty signature; otherwise every retained occurrence must be
/// reachable from a root at the endpoint where that occurrence exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DynamicMechanismSignature {
    pub(crate) request: MechanismRequestId,
    pub(crate) before_roots: BTreeSet<MechanismOccurrenceId>,
    pub(crate) after_roots: BTreeSet<MechanismOccurrenceId>,
    pub(crate) nodes: BTreeMap<MechanismOccurrenceId, PairedOccurrenceNode>,
}

impl DynamicMechanismSignature {
    /// Pair two independently validated endpoint slices by their outcome-free
    /// semantic slots. This union is deterministic and does not depend on the
    /// order in which either endpoint observer emitted events.
    pub(crate) fn from_endpoint_traces(
        request: &MechanismObservationRequest,
        before: DynamicEndpointTraceV1,
        after: DynamicEndpointTraceV1,
    ) -> Result<Self, MechanismValidationError> {
        before.validate(request)?;
        after.validate(request)?;
        EndpointPairingShapeV1::from_trace(&before)?
            .ensure_unambiguous_with(&EndpointPairingShapeV1::from_trace(&after)?)?;

        let slots = before
            .occurrences
            .keys()
            .chain(after.occurrences.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let ids = slots
            .iter()
            .map(|slot| {
                (
                    slot.clone(),
                    MechanismOccurrenceId::derive(&request.id, slot),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut nodes = Vec::with_capacity(slots.len());
        for slot in slots {
            let before_occurrence = before.occurrences.get(&slot);
            let after_occurrence = after.occurrences.get(&slot);
            let before_dependencies = before_occurrence
                .into_iter()
                .flat_map(|occurrence| occurrence.dependencies.iter())
                .map(|dependency| {
                    ids.get(dependency)
                        .cloned()
                        .expect("validated before dependency must have a paired ID")
                })
                .collect();
            let after_dependencies = after_occurrence
                .into_iter()
                .flat_map(|occurrence| occurrence.dependencies.iter())
                .map(|dependency| {
                    ids.get(dependency)
                        .cloned()
                        .expect("validated after dependency must have a paired ID")
                })
                .collect();
            nodes.push(PairedOccurrenceNode::new(
                request,
                slot,
                before_occurrence.map(|occurrence| occurrence.outcome.clone()),
                after_occurrence.map(|occurrence| occurrence.outcome.clone()),
                before_dependencies,
                after_dependencies,
            )?);
        }
        let before_roots = before
            .roots
            .iter()
            .map(|slot| {
                ids.get(slot)
                    .cloned()
                    .expect("validated before root must have a paired ID")
            })
            .collect();
        let after_roots = after
            .roots
            .iter()
            .map(|slot| {
                ids.get(slot)
                    .cloned()
                    .expect("validated after root must have a paired ID")
            })
            .collect();
        Self::new(request, before_roots, after_roots, nodes)
    }

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

        for (id, node) in &self.nodes {
            if id != &node.id {
                return Err(invalid(
                    "dynamic mechanism node map key disagrees with the node occurrence ID",
                ));
            }
            node.validate(&self.request)?;
        }

        let before_shape = EndpointPairingShapeV1::from_slots(
            self.nodes
                .values()
                .filter(|node| node.is_present_at(true))
                .map(|node| &node.slot),
        )?;
        let after_shape = EndpointPairingShapeV1::from_slots(
            self.nodes
                .values()
                .filter(|node| node.is_present_at(false))
                .map(|node| &node.slot),
        )?;
        before_shape.ensure_unambiguous_with(&after_shape)?;

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
            }
        }

        // Endpoint order is validated independently. The before and after
        // DAGs may legitimately reverse two occurrences, so requiring one
        // global topological ordinal would reject a real differential
        // mechanism or make occurrence identity insertion-sensitive.
        let mut remaining_dependencies = self
            .nodes
            .iter()
            .filter(|(_, node)| node.is_present_at(before))
            .map(|(id, node)| (id.clone(), node.dependencies_at(before).len()))
            .collect::<BTreeMap<_, _>>();
        let mut dependents = BTreeMap::<MechanismOccurrenceId, Vec<MechanismOccurrenceId>>::new();
        for (id, node) in self
            .nodes
            .iter()
            .filter(|(_, node)| node.is_present_at(before))
        {
            for dependency in node.dependencies_at(before) {
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
        let mut ready = remaining_dependencies
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
            .collect::<BTreeSet<_>>();
        let mut ordered = 0_usize;
        while let Some(id) = ready.pop_first() {
            ordered += 1;
            for dependent in dependents.get(&id).into_iter().flatten() {
                let count = remaining_dependencies
                    .get_mut(dependent)
                    .expect("validated endpoint dependent must be present");
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
        if ordered != endpoint_node_count {
            return Err(invalid(format!(
                "dynamic mechanism {endpoint} dependencies contain a cycle"
            )));
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
        let mut hasher = StableHasher::new(MECHANISM_SIGNATURE_HASH_V2);
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
        nodes.sort_by(|left, right| {
            left.slot
                .cmp(&right.slot)
                .then_with(|| left.id.cmp(&right.id))
        });
        hasher.u128(nodes.len() as u128);
        for node in nodes {
            hasher.segment(&node.id.digest.0);
            node.slot.hash_into(&mut hasher);
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

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest.0
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
    /// Exact target membership is known, but this case has not yet crossed a
    /// durable fresh-replay boundary. Pausing or hitting an operational cap
    /// leaves it pending; it does not mint a synthetic failure observation.
    Pending,
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

    /// Seal one coordinator-certified complete matching support without
    /// enumerating its ranks. The support identity must belong to the exact
    /// request universe; its canonical intervals are lowered directly into a
    /// binary target-membership DAG.
    pub(crate) fn from_exact_case_support(
        request: &MechanismObservationRequest,
        support: &ExactCaseSupport,
    ) -> Result<Self, MechanismValidationError> {
        request.validate()?;
        let universe = ExploreCaseUniverse::new(request.axis_cardinalities.clone())
            .map_err(|error| invalid(format!("invalid mechanism case universe: {error}")))?;
        ExactCaseSupport::empty(&universe)
            .merge_disjoint(support)
            .map_err(|error| {
                invalid(format!(
                    "exact matching support belongs to another case universe: {error}"
                ))
            })?;

        let base = MechanismTargetMembershipDag::from_sparse_classifications(
            request.axis_cardinalities.to_vec(),
            Vec::<(Vec<u128>, MechanismTargetMembership)>::new(),
            MechanismTargetMembership::OutsideTarget,
        )
        .map_err(|error| invalid(format!("cannot build empty target membership: {error}")))?;
        let membership = base
            .with_rank_interval_overrides(support.iter_intervals().map(|interval| {
                (
                    interval.start(),
                    interval.end_exclusive(),
                    MechanismTargetMembership::InsideTarget,
                )
            }))
            .map_err(|error| {
                invalid(format!(
                    "cannot lower exact matching support into target membership: {error}"
                ))
            })?;
        let id = derive_target_membership_id(&request.case_target, &membership);
        let exact = Self { id, membership };
        exact.validate_for_request(request)?;
        Ok(exact)
    }

    pub(crate) fn validate_for_request(
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

    pub(crate) fn inside_count(&self) -> Result<u128, MechanismValidationError> {
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
    let mut hasher = StableHasher::new(MECHANISM_TARGET_MEMBERSHIP_HASH_V2);
    hasher.segment(&case_target.digest.0);
    hasher.u128(membership.axis_cardinalities().len() as u128);
    for cardinality in membership.axis_cardinalities().iter().copied() {
        hasher.u128(cardinality);
    }
    match membership.root() {
        DecisionRoot::EmptySpace => hasher.segment(b"empty-space"),
        DecisionRoot::Target(target) => {
            hasher.segment(b"target");
            let logical_root =
                logical_membership_ref_digest(membership, target, &mut BTreeMap::new());
            hasher.segment(&logical_root.0);
        }
    }
    MechanismTargetMembershipId {
        case_target: case_target.clone(),
        digest: hasher.digest(),
    }
}

fn logical_membership_ref_digest(
    membership: &MechanismTargetMembershipDag,
    target: DecisionRef,
    memo: &mut BTreeMap<DecisionRef, StableDigest>,
) -> StableDigest {
    if let Some(digest) = memo.get(&target) {
        return *digest;
    }
    let digest = match target {
        DecisionRef::Terminal(id) => {
            let terminal = membership
                .terminal(id)
                .expect("validated target membership terminal must exist");
            let mut hasher =
                StableHasher::new(b"futuruna.explore.target-membership.logical-terminal.v1");
            hasher.segment(match terminal {
                MechanismTargetMembership::OutsideTarget => b"outside-target",
                MechanismTargetMembership::InsideTarget => b"inside-target",
            });
            hasher.digest()
        }
        DecisionRef::Node(id) => {
            let node = membership
                .node(id)
                .expect("validated target membership node must exist");
            let mut hasher =
                StableHasher::new(b"futuruna.explore.target-membership.logical-node.v1");
            hasher.u128(node.dimension_index() as u128);
            hasher.u128(node.arcs().len() as u128);
            for arc in node.arcs() {
                hasher.u128(arc.ordinals().intervals().len() as u128);
                for interval in arc.ordinals().intervals() {
                    hasher.u128(interval.start().get());
                    hasher.u128(interval.end_exclusive().get());
                }
                let child = logical_membership_ref_digest(membership, arc.child(), memo);
                hasher.segment(&child.0);
            }
            hasher.digest()
        }
    };
    memo.insert(target, digest);
    digest
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MechanismBinUnavailableSupport {
    pub(crate) signature: MechanismSignatureId,
    pub(crate) reason: MechanismBinUnavailableReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MechanismBinFieldEvidence {
    Unavailable(MechanismBinUnavailableReason),
    Observed {
        /// Cases classified into one declared bin for this field, partitioned
        /// by their already-known complete mechanism signature.
        observed_supports: BTreeMap<MechanismSignatureId, u128>,
        /// Successfully replayed values outside every declared interval. They
        /// make field replay total without fabricating an overflow bin or
        /// contributing to any requested mechanism-bin count.
        outside_declared_bins_supports: BTreeMap<MechanismSignatureId, u128>,
        /// Cases whose mechanism signature is known but whose requested field
        /// value could not be classified. These supports explain why bin
        /// counts remain lower bounds instead of silently disappearing.
        unavailable_supports: BTreeMap<MechanismBinUnavailableSupport, u128>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    /// Disclosure-bearing resume identity which authorized every retained
    /// case-level incidence edge and example in this materialized view.
    pub(crate) checked_request: CheckedMechanismObservationRequestV1,
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
        checked_request: CheckedMechanismObservationRequestV1,
        population: MechanismPopulationEvidence,
        exact_target_membership: Option<ExactMatchingTargetMembership>,
        canonical_signatures: BTreeMap<MechanismSignatureId, DynamicMechanismSignature>,
        observed_supports: BTreeMap<MechanismSignatureId, u128>,
        sampled_traces: BTreeMap<ExploreCaseId, MechanismSignatureId>,
        bin_fields: BTreeMap<Box<str>, MechanismBinFieldEvidence>,
        signature_bin_supports: BTreeMap<MechanismSignatureBinIncidence, u128>,
    ) -> Result<Self, MechanismValidationError> {
        checked_request.validate()?;
        let request = checked_request.observation.clone();
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
            checked_request,
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
        self.checked_request.validate()?;
        if self.checked_request.observation != self.request {
            return Err(invalid(
                "mechanism materialized view disagrees with its checked disclosure request",
            ));
        }
        self.request.validate()?;
        self.population.validate_shape()?;
        if self.population.incidence.is_some()
            && self.checked_request.disclosure.incidence
                != MechanismIncidenceDisclosure::FullMatchingIncidence
        {
            return Err(invalid(
                "mechanism incidence was materialized without full-incidence authorization",
            ));
        }
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
            if let Some(incidence) = self.population.incidence.as_ref() {
                let terminal = incidence
                    .terminal_for_path(case_id.ordinals())
                    .map_err(|error| {
                        invalid(format!(
                            "cannot resolve retained mechanism trace incidence: {error}"
                        ))
                    })?
                    .ok_or_else(|| invalid("retained mechanism trace has empty incidence space"))?;
                if terminal != &MechanismIncidenceTerminal::Signature(signature.clone()) {
                    return Err(invalid(
                        "retained mechanism trace disagrees with exact case-to-signature incidence",
                    ));
                }
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
            if retained
                > u128::from(
                    self.checked_request
                        .disclosure
                        .retained_examples_per_signature,
                )
            {
                return Err(invalid(
                    "retained mechanism examples exceed the checked disclosure cap",
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
                Some(MechanismBinFieldEvidence::Observed {
                    observed_supports,
                    ..
                })
                    if observed_supports.contains_key(&incidence.signature)
            ) {
                return Err(invalid(format!(
                    "mechanism signature/bin support for `{}` lacks observed field evidence",
                    incidence.field_name
                )));
            }
            if field.bins.binary_search(&incidence.bin).is_err() {
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
                MechanismBinFieldEvidence::Observed {
                    observed_supports,
                    outside_declared_bins_supports,
                    unavailable_supports,
                } => {
                    for (kind, supports) in [
                        ("declared-bin", observed_supports),
                        ("outside-declared-bins", outside_declared_bins_supports),
                    ] {
                        for (signature, support) in supports {
                            let Some(signature_evidence) = self.signatures.get(signature) else {
                                return Err(invalid(format!(
                                    "mechanism bin field `{name}` observes an unknown signature"
                                )));
                            };
                            if *support == 0 || *support > signature_evidence.observed_support {
                                return Err(invalid(format!(
                                    "mechanism bin field `{name}` has invalid {kind} support for a signature"
                                )));
                            }
                        }
                    }
                    for (unavailable, support) in unavailable_supports {
                        let Some(signature_evidence) = self.signatures.get(&unavailable.signature)
                        else {
                            return Err(invalid(format!(
                                "mechanism bin field `{name}` has unavailable support for an unknown signature"
                            )));
                        };
                        if *support == 0 || *support > signature_evidence.observed_support {
                            return Err(invalid(format!(
                                "mechanism bin field `{name}` has invalid unavailable support for a signature"
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
                    for (signature, evidence) in &self.signatures {
                        let binned = observed_supports.get(signature).copied().unwrap_or(0);
                        let outside = outside_declared_bins_supports
                            .get(signature)
                            .copied()
                            .unwrap_or(0);
                        let unavailable = unavailable_supports
                            .iter()
                            .filter(|(key, _)| &key.signature == signature)
                            .try_fold(0_u128, |total, (_, support)| {
                                total.checked_add(*support).ok_or_else(|| {
                                    invalid("mechanism bin unavailable support exceeds u128::MAX")
                                })
                            })?;
                        let classified = binned
                            .checked_add(outside)
                            .and_then(|value| value.checked_add(unavailable))
                            .ok_or_else(|| {
                                invalid("mechanism bin field classified support exceeds u128::MAX")
                            })?;
                        if classified > evidence.observed_support {
                            return Err(invalid(format!(
                                "mechanism bin field `{name}` classifies more cases than the signature supports"
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn bin_field_is_total(
        &self,
        observed_supports: &BTreeMap<MechanismSignatureId, u128>,
        outside_declared_bins_supports: &BTreeMap<MechanismSignatureId, u128>,
    ) -> bool {
        self.signatures.iter().all(|(signature, evidence)| {
            observed_supports
                .get(signature)
                .copied()
                .unwrap_or(0)
                .checked_add(
                    outside_declared_bins_supports
                        .get(signature)
                        .copied()
                        .unwrap_or(0),
                )
                == Some(evidence.observed_support)
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
        if field.bins.binary_search(&bin).is_err() {
            return None;
        }
        let MechanismBinFieldEvidence::Observed {
            observed_supports,
            outside_declared_bins_supports,
            unavailable_supports,
        } = self.bin_fields.get(field_name)?
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
                && unavailable_supports.is_empty()
                && self.bin_field_is_total(observed_supports, outside_declared_bins_supports)
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
    use super::super::case_graph::{
        DecisionPartition, DecisionPartitionArc, DecisionPartitionTarget,
    };
    use super::*;
    use crate::{
        CheckedDeclarationOccurrenceId, DeclarationId, DeclarationKind, Lexer, ModuleId, Parser,
        TypeChecker,
    };

    fn analysis_program() -> AnalysisProgramId {
        AnalysisProgramId("11".repeat(32).into_boxed_str())
    }

    fn declaration(name: &str) -> DeclarationId {
        DeclarationId {
            module: ModuleId {
                content_hash: "22".repeat(32).into_boxed_str(),
                internal_path: Box::default(),
            },
            kind: DeclarationKind::Function,
            owner: None,
            name: name.to_string().into_boxed_str(),
            arity: Some(1),
            ordinal: 0,
        }
    }

    fn expression_site(program: &AnalysisProgramId, name: &str, path: u32) -> ExprSiteId {
        ExprSiteId {
            analysis_program: program.clone(),
            declaration: declaration(name),
            normalized_declaration_ordinal: 0,
            ast_path: vec![path].into_boxed_slice(),
        }
    }

    fn site(program: &AnalysisProgramId, name: &str, path: u32) -> MechanismSiteId {
        MechanismSiteId::from_expression_site(&expression_site(program, name, path)).expect("site")
    }

    fn rule_family_site(program: &AnalysisProgramId, name: &str) -> MechanismSiteId {
        MechanismSiteId::from_rule_family(
            program,
            &RuleDispatchKey {
                scope: None,
                name: name.to_string(),
                arity: 1,
            },
        )
        .expect("rule family site")
    }

    fn rule_candidate_site(program: &AnalysisProgramId, name: &str, path: u32) -> MechanismSiteId {
        let declaration = declaration(name);
        let candidate = CheckedRuleCandidateResolution {
            tier: RuleDispatchTier::Clause,
            source_order: path as usize,
            declaration: CheckedDeclarationOccurrenceId {
                declaration: declaration.clone(),
                declaration_occurrence_ordinal: 0,
                normalized_ordinal: 0,
            },
            statement_path: vec![path].into_boxed_slice(),
            head_site: ExprSiteId {
                analysis_program: program.clone(),
                declaration,
                normalized_declaration_ordinal: 0,
                ast_path: vec![path, 0].into_boxed_slice(),
            },
            condition_site: None,
            value_site: None,
        };
        MechanismSiteId::from_rule_candidate(program, &candidate).expect("rule candidate site")
    }

    fn function_callee(program: &AnalysisProgramId) -> MechanismCallableSiteId {
        let callable = CheckedCallableId {
            declaration: CheckedDeclarationOccurrenceId {
                declaration: declaration("policy-callable"),
                declaration_occurrence_ordinal: 0,
                normalized_ordinal: 0,
            },
            structural_path: Box::default(),
        };
        let callable_site = MechanismSiteId::from_callable(program, &callable).expect("callable");
        MechanismCallableSiteId::function(callable_site).expect("function callee")
    }

    fn observation_template(program: &AnalysisProgramId) -> MechanismObservationIr {
        let template_site = expression_site(program, "policy-callable", 30);
        let template_root = MechanismSemanticRootId::from_site(
            MechanismSiteId::from_expression_site(&template_site).expect("template site"),
        )
        .expect("template root");
        MechanismObservationIr {
            endpoint_template: CheckedCallableId {
                declaration: CheckedDeclarationOccurrenceId {
                    declaration: declaration("policy-callable"),
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
            observation_template(&program),
            MechanismNormalization::DynamicControlV1,
            axis_cardinalities,
            sampling,
            bin_fields,
        )
        .expect("request")
    }

    fn request_with_template(
        program: &AnalysisProgramId,
        template: MechanismObservationIr,
    ) -> Result<MechanismObservationRequest, MechanismValidationError> {
        MechanismObservationRequest::new(
            program.clone(),
            MechanismQueryId::from_checked_query_bytes(b"query-and-domain"),
            MechanismObservationTarget::MatchingConfigurations,
            template,
            MechanismNormalization::DynamicControlV1,
            [1],
            MechanismSamplingPlan::empty(),
            Box::default(),
        )
    }

    #[test]
    fn observation_template_identity_binds_site_root_and_dependency_roots() {
        let program = analysis_program();
        let original_template = observation_template(&program);
        let original = request_with_template(&program, original_template.clone())
            .expect("canonical observation template");

        let mut unbound = original_template.clone();
        unbound.template_site.ast_path = vec![31].into_boxed_slice();
        let error = request_with_template(&program, unbound)
            .expect_err("a stale semantic root must not authenticate another template site");
        assert!(error.to_string().contains("does not identify"), "{error}");

        let mut rebound = original_template.clone();
        rebound.template_site.ast_path = vec![31].into_boxed_slice();
        rebound.template_root = MechanismSemanticRootId::from_site(
            MechanismSiteId::from_expression_site(&rebound.template_site)
                .expect("rebound template site"),
        )
        .expect("rebound template root");
        rebound.dependency_roots = vec![rebound.template_root.clone()].into_boxed_slice();
        let rebound = request_with_template(&program, rebound)
            .expect("a coherently rebound template remains valid");
        assert_ne!(original.id, rebound.id);

        let mut missing_dependency = original_template;
        let other_site = expression_site(&program, "other-root", 32);
        missing_dependency.dependency_roots = vec![MechanismSemanticRootId::from_site(
            MechanismSiteId::from_expression_site(&other_site).expect("other site"),
        )
        .expect("other root")]
        .into_boxed_slice();
        let error = request_with_template(&program, missing_dependency)
            .expect_err("dependencies must contain the template root");
        assert!(error.to_string().contains("omit"), "{error}");
    }

    #[test]
    fn observation_template_accepts_checked_callable_from_another_module() {
        let program = analysis_program();
        let mut template = observation_template(&program);
        template
            .endpoint_template
            .declaration
            .declaration
            .module
            .content_hash = "33".repeat(32).into_boxed_str();

        request_with_template(&program, template)
            .expect("module origin is callable identity, not a program-boundary rejection");
    }

    #[test]
    fn checked_query_identity_uses_revalidated_relational_identity_ladder() {
        let source = r#"
# Profile = Worker | Student
| eligible(profile: Profile, income: Int) -> income >= 0
? explore scan {
    over eligible(profile, income)
    find matches
    bounds {
        profile in values(Profile)
        income in range(0, 3)
    }
    output { key [profile, income] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse checked Explore fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("revalidated checked Explore query");

        let identity = MechanismQueryId::from_checked_query(&checked)
            .expect("checked query and domain identities");
        let mut expected = StableHasher::new(MECHANISM_CHECKED_QUERY_HASH_V2);
        expected.segment(
            checked
                .artifact
                .identity
                .analysis_program
                .as_str()
                .as_bytes(),
        );
        expected.segment(&checked.relation_id().bytes());
        expected.segment(&checked.admission_id().bytes());
        expected.segment(&checked.question_id().bytes());
        expected.segment(checked.analysis_graph_hash().as_bytes());

        assert_eq!(identity, MechanismQueryId(expected.digest()));
    }

    fn fully_disclosed(
        request: MechanismObservationRequest,
    ) -> CheckedMechanismObservationRequestV1 {
        CheckedMechanismObservationRequestV1::new(
            request,
            MechanismDisclosureV1::new(
                MechanismIncidenceDisclosure::FullMatchingIncidence,
                u32::MAX,
            ),
        )
        .expect("checked full-incidence request")
    }

    fn occurrence_slot(
        request: &MechanismObservationRequest,
        site: MechanismSiteId,
        kind: DynamicEventKind,
        visit_ordinal: u32,
    ) -> MechanismOccurrenceSlotV1 {
        MechanismOccurrenceSlotV1::new(
            request,
            0,
            Vec::<MechanismActivationStepV1>::new(),
            site,
            kind,
            visit_ordinal,
        )
        .expect("occurrence slot")
    }

    fn selection_signature(
        request: &MechanismObservationRequest,
        selected: MechanismSiteId,
    ) -> DynamicMechanismSignature {
        let dispatch = rule_family_site(&request.analysis_program, "dispatch");
        let outcome = DynamicEventOutcome::RuleSelection(RuleSelectionOutcome::Selected(selected));
        let node = PairedOccurrenceNode::new(
            request,
            occurrence_slot(request, dispatch, DynamicEventKind::RuleSelection, 0),
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
        let signature = selection_signature(
            request,
            rule_candidate_site(&request.analysis_program, "selected-rule", 2),
        );
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
    fn disclosure_changes_resume_identity_without_renaming_signatures() {
        let observation = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let summary = CheckedMechanismObservationRequestV1::new(
            observation.clone(),
            MechanismDisclosureV1::new(MechanismIncidenceDisclosure::SummaryOnly, 2),
        )
        .expect("summary request");
        let full = CheckedMechanismObservationRequestV1::new(
            observation.clone(),
            MechanismDisclosureV1::new(MechanismIncidenceDisclosure::FullMatchingIncidence, 2),
        )
        .expect("full request");

        assert_ne!(summary.id, full.id);
        assert_eq!(summary.observation.id, full.observation.id);
        let summary_signature = selection_signature(
            &summary.observation,
            rule_candidate_site(&summary.observation.analysis_program, "selected", 9),
        );
        let full_signature = selection_signature(
            &full.observation,
            rule_candidate_site(&full.observation.analysis_program, "selected", 9),
        );
        assert_eq!(
            MechanismSignatureId::derive(&summary_signature),
            MechanismSignatureId::derive(&full_signature)
        );
    }

    #[test]
    fn sampling_changes_checked_identity_without_renaming_observation_or_signature() {
        let selected = ExploreCaseId::new(vec![0_u128]);
        let sampling_plans = [
            MechanismSamplingPlan::empty(),
            MechanismSamplingPlan {
                result_representatives: BTreeSet::from([selected.clone()]),
                extrema_witnesses: BTreeSet::new(),
                required_case_ids: BTreeSet::new(),
            },
            MechanismSamplingPlan {
                result_representatives: BTreeSet::new(),
                extrema_witnesses: BTreeSet::from([selected.clone()]),
                required_case_ids: BTreeSet::new(),
            },
            MechanismSamplingPlan {
                result_representatives: BTreeSet::new(),
                extrema_witnesses: BTreeSet::new(),
                required_case_ids: BTreeSet::from([selected]),
            },
        ];
        let mut observation_ids = BTreeSet::new();
        let mut checked_ids = BTreeSet::new();
        let mut signature_ids = BTreeSet::new();
        for sampling in sampling_plans {
            let observation = request(vec![1], sampling, Vec::new());
            let signature = selection_signature(
                &observation,
                rule_candidate_site(&observation.analysis_program, "sampled-selected", 10),
            );
            observation_ids.insert(observation.id.clone());
            signature_ids.insert(MechanismSignatureId::derive(&signature));
            checked_ids.insert(fully_disclosed(observation).id);
        }

        assert_eq!(observation_ids.len(), 1);
        assert_eq!(signature_ids.len(), 1);
        assert_eq!(checked_ids.len(), 4);
    }

    #[test]
    fn materialized_mechanism_view_enforces_disclosure_and_example_cap() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let (signature, signatures) = one_signature(&request);
        let target = all_matching_target(&request);
        let incidence = MechanismIncidenceDag::from_sparse_classifications(
            vec![1],
            Vec::<(Vec<u128>, MechanismIncidenceTerminal)>::new(),
            MechanismIncidenceTerminal::Signature(signature.clone()),
        )
        .expect("incidence");
        let summary = CheckedMechanismObservationRequestV1::new(
            request.clone(),
            MechanismDisclosureV1::new(MechanismIncidenceDisclosure::SummaryOnly, 1),
        )
        .expect("summary request");
        assert!(MechanismObservedEvidence::new(
            summary,
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::MatchingClosed,
                MechanismCount::Exact(1),
                1,
                0,
                Some(incidence),
            )
            .expect("population"),
            Some(target),
            signatures.clone(),
            BTreeMap::from([(signature.clone(), 1)]),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .is_err());

        let no_examples = CheckedMechanismObservationRequestV1::new(
            request,
            MechanismDisclosureV1::new(MechanismIncidenceDisclosure::FullMatchingIncidence, 0),
        )
        .expect("zero-example request");
        assert!(MechanismObservedEvidence::new(
            no_examples,
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
            BTreeMap::from([(ExploreCaseId::new(vec![0]), signature)]),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .is_err());
    }

    #[test]
    fn stable_slot_pairing_survives_a_before_only_insertion() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let shared = occurrence_slot(
            &request,
            site(&request.analysis_program, "shared", 10),
            DynamicEventKind::IfDecision,
            0,
        );
        let earlier = occurrence_slot(
            &request,
            site(&request.analysis_program, "before-only", 11),
            DynamicEventKind::IfDecision,
            0,
        );
        let shared_occurrence = EndpointOccurrenceV1::new(
            shared.clone(),
            DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
            BTreeSet::new(),
        );
        let baseline_before = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([shared.clone()]),
            [shared_occurrence.clone()],
        )
        .expect("baseline before");
        let after = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([shared.clone()]),
            [shared_occurrence.clone()],
        )
        .expect("after");
        let baseline = DynamicMechanismSignature::from_endpoint_traces(
            &request,
            baseline_before,
            after.clone(),
        )
        .expect("baseline signature");

        let expanded_before = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([shared.clone()]),
            [
                EndpointOccurrenceV1::new(
                    earlier.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else),
                    BTreeSet::new(),
                ),
                EndpointOccurrenceV1::new(
                    shared.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                    BTreeSet::from([earlier]),
                ),
            ],
        )
        .expect("expanded before");
        let expanded =
            DynamicMechanismSignature::from_endpoint_traces(&request, expanded_before, after)
                .expect("expanded signature");

        let shared_id = MechanismOccurrenceId::derive(&request.id, &shared);
        assert!(baseline.nodes.contains_key(&shared_id));
        assert!(expanded.nodes.contains_key(&shared_id));
        assert_ne!(
            MechanismSignatureId::derive(&baseline),
            MechanismSignatureId::derive(&expanded)
        );
    }

    #[test]
    fn endpoint_order_may_reverse_without_creating_a_false_union_cycle() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let first = occurrence_slot(
            &request,
            site(&request.analysis_program, "first", 12),
            DynamicEventKind::IfDecision,
            0,
        );
        let second = occurrence_slot(
            &request,
            site(&request.analysis_program, "second", 13),
            DynamicEventKind::IfDecision,
            0,
        );
        let before = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([second.clone()]),
            [
                EndpointOccurrenceV1::new(
                    first.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                    BTreeSet::new(),
                ),
                EndpointOccurrenceV1::new(
                    second.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                    BTreeSet::from([first.clone()]),
                ),
            ],
        )
        .expect("before");
        let after = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([first.clone()]),
            [
                EndpointOccurrenceV1::new(
                    second.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else),
                    BTreeSet::new(),
                ),
                EndpointOccurrenceV1::new(
                    first,
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else),
                    BTreeSet::from([second]),
                ),
            ],
        )
        .expect("after");

        DynamicMechanismSignature::from_endpoint_traces(&request, before, after)
            .expect("independent endpoint DAG order must remain representable");
    }

    #[test]
    fn repeated_visits_are_distinct_and_duplicate_slots_fail_closed() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let semantic_site = site(&request.analysis_program, "repeated", 14);
        let first = occurrence_slot(
            &request,
            semantic_site.clone(),
            DynamicEventKind::MatchDecision,
            0,
        );
        let second = occurrence_slot(&request, semantic_site, DynamicEventKind::MatchDecision, 1);
        assert_ne!(
            MechanismOccurrenceId::derive(&request.id, &first),
            MechanismOccurrenceId::derive(&request.id, &second)
        );

        let occurrence = EndpointOccurrenceV1::new(
            first.clone(),
            DynamicEventOutcome::MatchDecision { arm_index: 0 },
            BTreeSet::new(),
        );
        assert!(DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([first]),
            [occurrence.clone(), occurrence],
        )
        .is_err());
    }

    #[test]
    fn endpoint_pairing_rejects_shifted_visits_at_one_semantic_site() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let semantic_site = site(&request.analysis_program, "shifted-visit", 15);
        let first = occurrence_slot(
            &request,
            semantic_site.clone(),
            DynamicEventKind::IfDecision,
            0,
        );
        let second = occurrence_slot(&request, semantic_site, DynamicEventKind::IfDecision, 1);
        let before = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([second.clone()]),
            [
                EndpointOccurrenceV1::new(
                    first.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else),
                    BTreeSet::new(),
                ),
                EndpointOccurrenceV1::new(
                    second,
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                    BTreeSet::from([first.clone()]),
                ),
            ],
        )
        .expect("before trace");
        let after = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([first.clone()]),
            [EndpointOccurrenceV1::new(
                first,
                DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                BTreeSet::new(),
            )],
        )
        .expect("after trace");

        let error = DynamicMechanismSignature::from_endpoint_traces(&request, before, after)
            .expect_err("shifted local visits must not be guessed");
        assert!(error.to_string().contains("different visit multiplicity"));
    }

    #[test]
    fn direct_signature_construction_cannot_bypass_visit_pairing_validation() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let semantic_site = site(&request.analysis_program, "decoded-shifted-visit", 22);
        let first_slot = occurrence_slot(
            &request,
            semantic_site.clone(),
            DynamicEventKind::IfDecision,
            0,
        );
        let second_slot = occurrence_slot(&request, semantic_site, DynamicEventKind::IfDecision, 1);
        let first = PairedOccurrenceNode::new(
            &request,
            first_slot,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else)),
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("first paired occurrence");
        let second = PairedOccurrenceNode::new(
            &request,
            second_slot,
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            None,
            BTreeSet::from([first.id.clone()]),
            BTreeSet::new(),
        )
        .expect("second paired occurrence");

        let error = DynamicMechanismSignature::new(
            &request,
            BTreeSet::from([second.id.clone()]),
            BTreeSet::from([first.id.clone()]),
            [first, second],
        )
        .expect_err("decoded signatures must obey the pairing-shape rule");
        assert!(error.to_string().contains("different visit multiplicity"));
    }

    #[test]
    fn occurrence_pairing_rejects_shifted_invocations_at_one_call_site() {
        let request = request(vec![1], MechanismSamplingPlan::empty(), Vec::new());
        let call_site = site(&request.analysis_program, "repeated-call", 16);
        let event_site = site(&request.analysis_program, "inside-call", 17);
        let common_callee = function_callee(&request.analysis_program);
        let activation = |ordinal| {
            MechanismActivationStepV1::new(
                &request,
                call_site.clone(),
                common_callee.clone(),
                ordinal,
            )
            .expect("activation")
        };
        let slot = |ordinal| {
            MechanismOccurrenceSlotV1::new(
                &request,
                0,
                vec![activation(ordinal)],
                event_site.clone(),
                DynamicEventKind::IfDecision,
                0,
            )
            .expect("occurrence slot")
        };
        let first = slot(0);
        let second = slot(1);
        let before = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([second.clone()]),
            [
                EndpointOccurrenceV1::new(
                    first.clone(),
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Else),
                    BTreeSet::new(),
                ),
                EndpointOccurrenceV1::new(
                    second,
                    DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                    BTreeSet::from([first.clone()]),
                ),
            ],
        )
        .expect("before trace");
        let after = DynamicEndpointTraceV1::new(
            &request,
            BTreeSet::from([first.clone()]),
            [EndpointOccurrenceV1::new(
                first,
                DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then),
                BTreeSet::new(),
            )],
        )
        .expect("after trace");

        let error = DynamicMechanismSignature::from_endpoint_traces(&request, before, after)
            .expect_err("shifted local invocations must not be guessed");
        assert!(error
            .to_string()
            .contains("different invocation multiplicity"));
    }

    #[test]
    fn semantic_roots_and_callable_variants_validate_their_site_kinds() {
        let program = analysis_program();
        assert!(MechanismSemanticRootId::from_site(rule_family_site(
            &program,
            "not-an-expression-root"
        ))
        .is_err());

        let mislabeled =
            MechanismCallableSiteId::Function(rule_family_site(&program, "not-a-function"));
        assert!(mislabeled
            .validate(&program, "mislabeled callable")
            .is_err());
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
            fully_disclosed(request),
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
            MechanismSemanticRootId::from_site(site(&program, "loss", 3))
                .expect("loss semantic root"),
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
            fully_disclosed(request),
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
                    outside_declared_bins_supports: BTreeMap::new(),
                    unavailable_supports: BTreeMap::new(),
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

        let mut with_outside_value = evidence.clone();
        with_outside_value.bin_fields.insert(
            "loss_ore".into(),
            MechanismBinFieldEvidence::Observed {
                observed_supports: BTreeMap::from([(signature.clone(), 1)]),
                outside_declared_bins_supports: BTreeMap::from([(signature.clone(), 1)]),
                unavailable_supports: BTreeMap::new(),
            },
        );
        with_outside_value
            .signature_bin_supports
            .remove(&MechanismSignatureBinIncidence {
                signature: signature.clone(),
                field_name: "loss_ore".into(),
                bin: bins[1],
            });
        with_outside_value
            .validate()
            .expect("outside-bin values still close replay totality");
        assert_eq!(
            with_outside_value.mechanisms_in_bin("loss_ore", bins[0]),
            Some(MechanismCount::Exact(1))
        );
        assert_eq!(
            with_outside_value.mechanisms_in_bin("loss_ore", bins[1]),
            Some(MechanismCount::Exact(0))
        );

        let mut incomplete = evidence.clone();
        incomplete.bin_fields.insert(
            "loss_ore".into(),
            MechanismBinFieldEvidence::Observed {
                observed_supports: BTreeMap::from([(signature.clone(), 1)]),
                outside_declared_bins_supports: BTreeMap::new(),
                unavailable_supports: BTreeMap::from([(
                    MechanismBinUnavailableSupport {
                        signature: signature.clone(),
                        reason: MechanismBinUnavailableReason::ValueReplayUnavailable,
                    },
                    1,
                )]),
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
        let MechanismBinFieldEvidence::Observed {
            unavailable_supports,
            ..
        } = &incomplete.bin_fields["loss_ore"]
        else {
            panic!("partial field must retain explicit unavailability")
        };
        assert_eq!(unavailable_supports.values().copied().sum::<u128>(), 1);

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
            rule_candidate_site(&request.analysis_program, "tied-terminal-a", 4),
        );
        let second = selection_signature(
            &request,
            rule_candidate_site(&request.analysis_program, "tied-terminal-b", 4),
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
            occurrence_slot(
                &request,
                shared_site.clone(),
                DynamicEventKind::IfDecision,
                0,
            ),
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            None,
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("before-only node");
        let after_only = PairedOccurrenceNode::new(
            &request,
            occurrence_slot(&request, shared_site, DynamicEventKind::IfDecision, 0),
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
            occurrence_slot(
                &request,
                site(&request.analysis_program, "before-dependency", 6),
                DynamicEventKind::IfDecision,
                0,
            ),
            Some(DynamicEventOutcome::IfDecision(IfDecisionOutcome::Then)),
            None,
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("before node");
        let after = PairedOccurrenceNode::new(
            &request,
            occurrence_slot(
                &request,
                site(&request.analysis_program, "after-dependent", 7),
                DynamicEventKind::IfDecision,
                0,
            ),
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
                occurrence_slot(
                    &request,
                    rule_candidate_site(&request.analysis_program, "rule-attempt", 7),
                    DynamicEventKind::RuleAttempt,
                    0,
                ),
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
            fully_disclosed(request),
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
    fn retained_examples_must_match_exact_incidence() {
        let request = request(vec![2], MechanismSamplingPlan::empty(), Vec::new());
        let (signature, signatures) = one_signature(&request);
        let target = all_matching_target(&request);
        let incidence = MechanismIncidenceDag::from_sparse_classifications(
            vec![2],
            [(
                vec![0],
                MechanismIncidenceTerminal::KnownTargetUntraced(KnownTargetUntracedReason::Pending),
            )],
            MechanismIncidenceTerminal::Signature(signature.clone()),
        )
        .expect("partial incidence");

        assert!(MechanismObservedEvidence::new(
            fully_disclosed(request),
            MechanismPopulationEvidence::new(
                MechanismEvidenceStatus::IncidenceOpen,
                MechanismCount::Exact(2),
                1,
                1,
                Some(incidence),
            )
            .expect("population"),
            Some(target),
            signatures,
            BTreeMap::from([(signature.clone(), 1)]),
            BTreeMap::from([(ExploreCaseId::new(vec![0]), signature)]),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .is_err());
    }

    #[test]
    fn target_membership_identity_ignores_report_local_interning_order() {
        let request = request(vec![2], MechanismSamplingPlan::empty(), Vec::new());
        let graph = |reverse: bool| {
            let mut arcs = vec![
                DecisionPartitionArc::new(
                    [(0, 1)],
                    DecisionPartitionTarget::terminal(MechanismTargetMembership::OutsideTarget),
                )
                .expect("outside arc"),
                DecisionPartitionArc::new(
                    [(1, 2)],
                    DecisionPartitionTarget::terminal(MechanismTargetMembership::InsideTarget),
                )
                .expect("inside arc"),
            ];
            if reverse {
                arcs.reverse();
            }
            let target = DecisionPartitionTarget::decision(0, arcs).expect("decision");
            MechanismTargetMembershipDag::from_decision_partition(
                vec![2],
                DecisionPartition::target(target),
            )
            .expect("membership graph")
        };
        let first = graph(false);
        let second = graph(true);
        assert_ne!(first, second, "fixture must vary physical IDs");
        assert_eq!(
            derive_target_membership_id(&request.case_target, &first),
            derive_target_membership_id(&request.case_target, &second),
        );
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
            fully_disclosed(request),
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
