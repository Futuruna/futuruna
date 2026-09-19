//! Optional checked classification of source-coordinate boxes. This shares
//! the totality interpreter's collection, callback and rule semantics; it does
//! not infer a result from a sampled endpoint or a mechanism signature.

use super::*;
use crate::explore::relational_classification_capsule::FrozenClassificationQuestionSet;
use crate::explore::relational_classified_sweep::{
    RelationalClassifiedCaseOutcome, RelationalOrderedClassificationSubject,
    RelationalQuestionDecisionMask,
};
use crate::explore::relational_proof_strategy::RelationalProofStrategyInventory;
use crate::explore::relational_support_planner::RelationalSupportPlan;
use crate::explore::ExploreFindIr;
use crate::explore::SelectionDecision;
use crate::{CheckedAnalysisProgram, OwnedCheckedExploreQuery, TypeCheckArtifacts};
use crate::{CheckedExploreQueryView, CheckedResolutionRecorder};

type Coordinates = Vec<Option<(i64, i64)>>;

pub(crate) const MAX_SCOPE_BINDINGS: usize = 64;
pub(crate) const MAX_CACHED_COVER_SCOPES: usize = 64;
const SHARED_SCOPE_WIDTH: i128 = 32_768;
type ScopeCache = std::collections::VecDeque<(Coordinates, Option<Arc<CheckedSourceBoxProof>>)>;

/// An immutable scope/proof pair minted only from this producer's cache.
/// A decoded scope or digest cannot construct this authority.
#[derive(Clone, Debug)]
pub(crate) struct CachedCoverScope {
    coordinates: Coordinates,
    proof: Arc<CheckedSourceBoxProof>,
}

impl CachedCoverScope {
    pub(crate) fn coordinates(&self) -> &[Option<(i64, i64)>] {
        &self.coordinates
    }

    pub(crate) fn proof(&self) -> &CheckedSourceBoxProof {
        &self.proof
    }
}

/// Exact coordinate containment, including the finite/singleton binding shape.
pub(crate) fn coordinates_contained(
    inner: &[Option<(i64, i64)>],
    outer: &[Option<(i64, i64)>],
) -> bool {
    inner.len() == outer.len()
        && inner
            .iter()
            .zip(outer)
            .all(|(inner, outer)| match (inner, outer) {
                (None, None) => true,
                (Some((a, b)), Some((low, high))) => low <= a && a <= b && b <= high,
                _ => false,
            })
}

struct BoxInputs {
    program: CheckedAnalysisProgram,
    resolutions: CheckedResolutionArtifacts,
    checked: Arc<OwnedCheckedExploreQuery>,
    domains: Coordinates,
}

impl std::fmt::Debug for BoxInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxInputs")
            .field("domains", &self.domains)
            .finish_non_exhaustive()
    }
}

/// Bounded, ephemeral optimization cache. Neither a cache hit nor its key is
/// durable evidence: the ordinary sweep host still authenticates every case
/// and records its classification. Cold runs derive these facts anew.
#[derive(Clone, Debug)]
pub(crate) struct CheckedBoxClassifier {
    inputs: Arc<BoxInputs>,
    cache: std::collections::VecDeque<(Coordinates, Option<RelationalClassifiedCaseOutcome>)>,
    /// Contains only producer-created proofs from this checked snapshot, never
    /// decoded artifacts. Clones share fresh derivations within one process.
    scope_cache: Arc<std::sync::Mutex<ScopeCache>>,
}

impl CheckedBoxClassifier {
    pub(crate) fn prefers_shared_cover(&self) -> bool {
        self.inputs
            .domains
            .iter()
            .flatten()
            .any(|(low, high)| i128::from(*high) - i128::from(*low) >= SHARED_SCOPE_WIDTH)
    }

    /// Operational widening only. The chosen domain is recorded explicitly in
    /// each scoped leaf, so replay never depends on this heuristic.
    pub(crate) fn shared_cover_scope(
        &self,
        coordinates: &[Option<(i64, i64)>],
        refinement: u32,
    ) -> Option<Coordinates> {
        if refinement > 6
            || coordinates.len() > MAX_SCOPE_BINDINGS
            || !coordinates_contained(coordinates, &self.inputs.domains)
        {
            return None;
        }
        let width = SHARED_SCOPE_WIDTH >> refinement;
        let mut scope = coordinates.to_vec();
        for (range, domain) in scope.iter_mut().zip(&self.inputs.domains) {
            let (Some((low, high)), Some((start, end))) = (*range, *domain) else {
                continue;
            };
            if i128::from(end) - i128::from(start) < SHARED_SCOPE_WIDTH || high == end {
                continue;
            }
            let low_tile = (i128::from(low) - i128::from(start)) / width;
            let high_tile = (i128::from(high) - i128::from(start)) / width;
            if low_tile != high_tile {
                continue;
            }
            let start = i128::from(start) + low_tile * width;
            let end = (start + width - 1).min(i128::from(end) - 1);
            *range = Some((i64::try_from(start).ok()?, i64::try_from(end).ok()?));
        }
        (scope != coordinates).then_some(scope)
    }

    /// Scheduling budget only; presence never establishes an outcome.
    pub(crate) fn cover_scope_is_cached(&self, scope: &[Option<(i64, i64)>]) -> bool {
        self.scope_cache
            .lock()
            .is_ok_and(|cache| cache.iter().any(|(key, _)| key == scope))
    }

    /// Snapshot only already-closed producer proofs. No derivation, solver
    /// attempt, cache insertion or decoded-artifact promotion occurs here.
    pub(crate) fn cached_closed_cover_scopes(
        &self,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Option<Vec<CachedCoverScope>> {
        if !self.matches_checked(checked) {
            return None;
        }
        let cache = self.scope_cache.lock().ok()?;
        if cache.len() > MAX_CACHED_COVER_SCOPES {
            return None;
        }
        Some(
            cache
                .iter()
                .rev()
                .filter_map(|(coordinates, proof)| {
                    let proof = proof.as_ref()?;
                    (coordinates.len() <= MAX_SCOPE_BINDINGS
                        && coordinates_contained(coordinates, &self.inputs.domains)
                        && (proof.all_rejected() || proof.all_admitted_not_selected()))
                    .then(|| CachedCoverScope {
                        coordinates: coordinates.clone(),
                        proof: proof.clone(),
                    })
                })
                .collect(),
        )
    }

    /// Fresh source proof once per retained scope and checked snapshot. A cold
    /// classifier has an empty cache and must run the producer (including any
    /// solver obligation) again. Scope bytes or digests cannot fill this cache.
    pub(crate) fn prove_cached_cover_scope(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        scope: &[Option<(i64, i64)>],
    ) -> Option<Arc<CheckedSourceBoxProof>> {
        if !self.matches_checked(checked)
            || scope.len() > MAX_SCOPE_BINDINGS
            || !coordinates_contained(scope, &self.inputs.domains)
        {
            return None;
        }
        {
            let mut cache = self.scope_cache.lock().ok()?;
            if let Some(index) = cache.iter().position(|(key, _)| key == scope) {
                let entry = cache.remove(index)?;
                let proof = entry.1.clone();
                cache.push_back(entry);
                return proof;
            }
        }
        let proof = self.prove_cover_coordinates(checked, scope).map(Arc::new);
        let mut cache = self.scope_cache.lock().ok()?;
        if cache.len() >= MAX_CACHED_COVER_SCOPES {
            cache.pop_front();
        }
        cache.push_back((scope.to_vec(), proof.clone()));
        proof
    }

    fn matches_checked(&self, checked: &CheckedExploreQueryView<'_>) -> bool {
        let retained = self.inputs.checked.view();
        checked.program_hash() == retained.program_hash()
            && checked.relation_id() == retained.relation_id()
            && checked.admission_id() == retained.admission_id()
            && checked.question_ids() == retained.question_ids()
    }

    /// This is a fresh proof over checked source coordinates, not a reuse of
    /// the operational tile cache. Regional replay must call it again.
    pub(crate) fn prove_coordinates(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        coordinates: &[Option<(i64, i64)>],
    ) -> Option<CheckedSourceBoxProof> {
        self.prove_coordinates_mode(checked, coordinates, false)
    }

    pub(crate) fn prove_cover_coordinates(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        coordinates: &[Option<(i64, i64)>],
    ) -> Option<CheckedSourceBoxProof> {
        self.prove_coordinates_mode(checked, coordinates, true)
    }

    fn prove_coordinates_mode(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        coordinates: &[Option<(i64, i64)>],
        branch_constants: bool,
    ) -> Option<CheckedSourceBoxProof> {
        if !self.matches_checked(checked) {
            return None;
        }
        let index = CheckedExploreSemanticIndex::build(&self.inputs.program);
        let proof = prove_box_with_branch_constants(
            &index,
            &self.inputs.resolutions,
            checked,
            coordinates,
            branch_constants,
        )
        .ok()?;
        // Preserve existing V6 leaf roots: stronger precision is attempted
        // only where the original recipe could not issue a closed outcome.
        if !branch_constants || proof.classification.outcome(checked).is_some() {
            return Some(proof);
        }
        let arithmetic = prove_box_with_precision(
            &index,
            &self.inputs.resolutions,
            checked,
            coordinates,
            branch_constants,
            true,
            IntegerPrecision::Arithmetic,
        )
        .ok()?;
        // A definite SAT in the old relaxation can never become UNSAT merely
        // by giving that same obligation more time. Only that stable negative
        // result permits a new recipe, preserving every previously closed root.
        // Timeout/unavailable/unsupported do not authorize a recipe switch.
        if arithmetic.classification.outcome(checked).is_some()
            || !arithmetic.arithmetic_relaxation_sat
        {
            return Some(arithmetic);
        }
        let clamps = prove_box_with_precision(
            &index,
            &self.inputs.resolutions,
            checked,
            coordinates,
            branch_constants,
            true,
            IntegerPrecision::Clamps,
        )
        .ok()?;
        // Bound-binder clamps are a separate stronger recipe. Never alter a
        // previously successful literal-clamp root, including after a timeout.
        if clamps.classification.outcome(checked).is_some() || !clamps.arithmetic_relaxation_sat {
            return Some(clamps);
        }
        prove_box_with_precision(
            &index,
            &self.inputs.resolutions,
            checked,
            coordinates,
            branch_constants,
            true,
            IntegerPrecision::BoundClamps,
        )
        .ok()
    }

    pub(crate) fn new(
        artifacts: TypeCheckArtifacts,
        checked: Arc<OwnedCheckedExploreQuery>,
        plan: &RelationalSupportPlan,
    ) -> Option<Self> {
        let view = checked.view();
        let inventory = RelationalProofStrategyInventory::from_checked(&view, plan).ok()?;
        if inventory.axes().len() != inventory.finite_binding_indices().len()
            || inventory.axes().len() > affine_interval::MAX_AXES
            || !matches!(
                view.closed_query.successor.kind,
                ExploreSuccessorKindIr::Singleton { .. }
            )
        {
            return None;
        }
        let domains = view
            .closed_query
            .source
            .bindings
            .iter()
            .map(|binding| match binding.kind {
                ExploreSourceBindingKindIr::Singleton { .. } => Some(None),
                ExploreSourceBindingKindIr::Finite { .. } => {
                    let axis = inventory.axes().iter().find(|axis| {
                        usize::try_from(axis.binding_index()).ok() == Some(binding.binding_index)
                    })?;
                    let end = i128::from(axis.value_start())
                        .checked_add(i128::try_from(axis.cardinality()).ok()?)?;
                    Some(Some((
                        axis.value_start(),
                        i64::try_from(end.checked_sub(1)?).ok()?,
                    )))
                }
            })
            .collect::<Option<Coordinates>>()?;
        Some(Self {
            inputs: Arc::new(BoxInputs {
                program: artifacts.analysis_program,
                resolutions: artifacts.checked_resolutions,
                checked,
                domains,
            }),
            cache: std::collections::VecDeque::new(),
            scope_cache: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
        })
    }

    fn coordinates(
        &self,
        subject: RelationalOrderedClassificationSubject<'_>,
    ) -> Option<Coordinates> {
        self.inputs
            .domains
            .iter()
            .enumerate()
            .map(|(binding, domain)| {
                let Some((start, end)) = domain else {
                    return Some(None);
                };
                let ExploreValue::Int(value) = subject.source_binding(binding)? else {
                    return None;
                };
                if value < start || value > end {
                    return None;
                }
                let width = i128::from(*end) - i128::from(*start) + 1;
                let (low, high) = if width <= 8 || value == end {
                    (*value, *value)
                } else if width <= 512 {
                    (*start, end.checked_sub(1)?)
                } else {
                    // Fixed operational tiles, not tax thresholds. The final
                    // singleton isolates an outward endpoint of unit transitions.
                    let offset = i128::from(*value) - i128::from(*start);
                    let low = i128::from(*start) + (offset / 4096) * 4096;
                    let high = (low + 4095).min(i128::from(*end) - 1);
                    (i64::try_from(low).ok()?, i64::try_from(high).ok()?)
                };
                Some(Some((low, high)))
            })
            .collect()
    }

    pub(crate) fn classify(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
    ) -> Vec<Option<RelationalClassifiedCaseOutcome>> {
        // Build the source index only for a cache miss, at most once per
        // batch. It borrows the frozen checked snapshot, never the filesystem.
        let mut index = None;
        let mut outcomes = Vec::with_capacity(subjects.len());
        for subject in subjects.iter().copied() {
            let Some(coordinates) = self.coordinates(subject) else {
                outcomes.push(None);
                continue;
            };
            if let Some((_, outcome)) = self.cache.iter().find(|(key, _)| key == &coordinates) {
                outcomes.push(outcome.clone());
                continue;
            }
            let index = index
                .get_or_insert_with(|| CheckedExploreSemanticIndex::build(&self.inputs.program));
            let checked = self.inputs.checked.view();
            let started = std::time::Instant::now();
            let proof = classify_box(index, &self.inputs.resolutions, &checked, &coordinates);
            let outcome = proof
                .as_ref()
                .ok()
                .and_then(|proof| proof.outcome(&checked));
            if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                eprintln!("Explore abstract classification: coordinates={coordinates:?}; proof={proof:?}; elapsed={:?}", started.elapsed());
            }
            if self.cache.len() == 16 {
                self.cache.pop_front();
            }
            self.cache.push_back((coordinates, outcome.clone()));
            outcomes.push(outcome);
        }
        outcomes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoxClassification {
    pub(crate) admissions: Box<[Option<bool>]>,
    /// Selection decisions, in authored FIND order (not predicate polarity).
    pub(crate) selections: Box<[Option<bool>]>,
}

/// Producer-owned abstract derivation. The root is an integrity commitment,
/// not replay authority; durable consumers re-run the checked derivation.
#[derive(Debug)]
pub(crate) struct CheckedSourceBoxProof {
    classification: BoxClassification,
    derivation_root: [u8; 32],
    split_hints: Box<[(usize, i64)]>,
    // Operational fallback eligibility, not durable proof authority.
    arithmetic_relaxation_sat: bool,
}

impl CheckedSourceBoxProof {
    pub(crate) fn all_rejected(&self) -> bool {
        self.classification.admissions.contains(&Some(false))
    }

    pub(crate) fn all_admitted_not_selected(&self) -> bool {
        self.classification
            .admissions
            .iter()
            .all(|value| *value == Some(true))
            && self.classification.selections.as_ref() == [Some(false)]
    }

    pub(crate) const fn derivation_root(&self) -> [u8; 32] {
        self.derivation_root
    }

    /// Search nominations only. These confer no classification authority.
    pub(crate) fn split_hints(&self) -> &[(usize, i64)] {
        &self.split_hints
    }
}

fn finish_box_proof(
    prover: &EndpointTotalityProver<'_, '_>,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
    classification: BoxClassification,
) -> CheckedSourceBoxProof {
    let mut hash = Sha256::new();
    hash_segment(
        &mut hash,
        b"futuruna.explore.checked-source-box.derivation.v1\0",
    );
    hash_segment(&mut hash, checked.program_hash().as_bytes());
    hash_segment(&mut hash, prover.index.program.id.as_str().as_bytes());
    hash_segment(&mut hash, &checked.relation_id().bytes());
    hash_segment(&mut hash, &checked.admission_id().bytes());
    for question in checked.question_ids() {
        hash_segment(&mut hash, &question.bytes());
    }
    hash_segment(&mut hash, &(coordinates.len() as u64).to_le_bytes());
    for coordinate in coordinates {
        match coordinate {
            None => hash_segment(&mut hash, &[0]),
            Some((low, high)) => {
                hash_segment(&mut hash, &[1]);
                hash_segment(&mut hash, &low.to_le_bytes());
                hash_segment(&mut hash, &high.to_le_bytes());
            }
        }
    }
    for decisions in [&classification.admissions, &classification.selections] {
        hash_segment(&mut hash, &(decisions.len() as u64).to_le_bytes());
        for decision in decisions.iter() {
            hash_segment(
                &mut hash,
                &[match decision {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                }],
            );
        }
    }
    hash_segment(&mut hash, &prover.proof_root().bytes());
    if !prover.arithmetic_proofs.is_empty() {
        hash_segment(
            &mut hash,
            b"futuruna.explore.checked-source-box.integer-unsat.v1\0",
        );
        hash_segment(
            &mut hash,
            &(prover.arithmetic_proofs.len() as u64).to_le_bytes(),
        );
        for proof in &prover.arithmetic_proofs {
            hash_segment(&mut hash, &proof.digest());
        }
    }
    CheckedSourceBoxProof {
        classification,
        derivation_root: hash.finalize().into(),
        split_hints: prover.split_hints.iter().copied().collect(),
        arithmetic_relaxation_sat: prover.arithmetic_relaxation_sat,
    }
}

impl BoxClassification {
    fn outcome(
        &self,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Option<RelationalClassifiedCaseOutcome> {
        if self.admissions.contains(&Some(false)) {
            return Some(RelationalClassifiedCaseOutcome::Rejected);
        }
        if self
            .admissions
            .iter()
            .any(|decision| *decision != Some(true))
        {
            return None;
        }
        let decisions = checked
            .find_question_ids()
            .iter()
            .copied()
            .zip(self.selections.iter())
            .map(|(id, selected)| {
                Some((
                    id,
                    if selected.as_ref().copied()? {
                        SelectionDecision::Selected
                    } else {
                        SelectionDecision::NotSelected
                    },
                ))
            })
            .collect::<Option<BTreeMap<_, _>>>()?;
        let questions =
            FrozenClassificationQuestionSet::freeze(checked.question_ids().iter().copied()).ok()?;
        RelationalQuestionDecisionMask::from_ordered_decisions(&questions, decisions)
            .ok()
            .map(RelationalClassifiedCaseOutcome::Admitted)
    }
}

/// Bounds are supplied in authored source-binding order. A finite binding
/// must have an integer enclosure; derived bindings must have no override.
/// The caller retains responsibility for binding these coordinates to its
/// exact support subject. Extra points in an enclosure can only lose precision.
pub(crate) fn classify_box(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
) -> Result<BoxClassification, RelationalEndpointTotalityIssue> {
    prove_box(index, resolutions, checked, coordinates).map(|proof| proof.classification)
}

fn prove_box(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
) -> Result<CheckedSourceBoxProof, RelationalEndpointTotalityIssue> {
    prove_box_with_branch_constants(index, resolutions, checked, coordinates, false)
}

fn prove_box_with_branch_constants(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
    branch_constants: bool,
) -> Result<CheckedSourceBoxProof, RelationalEndpointTotalityIssue> {
    prove_box_with_clamp_identities(
        index,
        resolutions,
        checked,
        coordinates,
        branch_constants,
        false,
    )
}

fn prove_box_with_clamp_identities(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
    branch_constants: bool,
    clamp_identities: bool,
) -> Result<CheckedSourceBoxProof, RelationalEndpointTotalityIssue> {
    prove_box_with_precision(
        index,
        resolutions,
        checked,
        coordinates,
        branch_constants,
        clamp_identities,
        IntegerPrecision::None,
    )
}

#[derive(Clone, Copy)]
enum IntegerPrecision {
    None,
    Arithmetic,
    Clamps,
    BoundClamps,
}

fn prove_box_with_precision(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    checked: &CheckedExploreQueryView<'_>,
    coordinates: &[Option<(i64, i64)>],
    branch_constants: bool,
    clamp_identities: bool,
    precision: IntegerPrecision,
) -> Result<CheckedSourceBoxProof, RelationalEndpointTotalityIssue> {
    let integer_solver = !matches!(precision, IntegerPrecision::None);
    let query = checked.closed_query;
    let sites = &checked.artifact.sites;
    let mut prover = EndpointTotalityProver::new(index, resolutions, checked.relation_id());
    prover.track_scalar_call_identities = true;
    prover.refine_known_parameter_constants = branch_constants;
    prover.refine_checked_clamp_identities = clamp_identities;
    prover.retain_checked_clamp_dependencies = matches!(
        precision,
        IntegerPrecision::Clamps | IntegerPrecision::BoundClamps
    );
    prover.retain_checked_bound_clamp_dependencies =
        matches!(precision, IntegerPrecision::BoundClamps);
    prover.collect_split_hints = integer_solver;
    if integer_solver {
        prover.arithmetic_trace = Some(arithmetic_trace::ArithmeticTrace::default());
        prover.arithmetic_axes = coordinates.iter().flatten().copied().collect();
    }
    #[cfg(test)]
    if std::env::var_os("FUTURUNA_EXPLORE_SMT_DUMP").is_some() {
        prover.arithmetic_trace = Some(arithmetic_trace::ArithmeticTrace::default());
        prover.arithmetic_axes = coordinates.iter().flatten().copied().collect();
    }
    prover.source_axis_count = query
        .source
        .bindings
        .iter()
        .filter(|binding| matches!(binding.kind, ExploreSourceBindingKindIr::Finite { .. }))
        .count();
    let invalid = || {
        prover.issue(
            &sites.successor,
            RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
            "classification box does not match the checked finite integer source product",
        )
    };
    if !resolutions.source_snapshot_coherent
        || resolutions.analysis_program != index.program.id
        || coordinates.len() != query.source.bindings.len()
        || coordinates.len() != sites.source_bindings.len()
        || sites.admissions.len() != query.admissions.len()
        || sites.find_predicates.len() != query.finds.len()
        || !matches!(
            query.successor.kind,
            ExploreSuccessorKindIr::Singleton { .. }
        )
    {
        return Err(invalid());
    }
    let mut env = AbstractEnv::new();
    let mut retained = prover.new_retained_budget(&sites.successor)?;
    let mut axis = 0;
    for ((binding, site), coordinate) in query
        .source
        .bindings
        .iter()
        .zip(sites.source_bindings.iter())
        .zip(coordinates)
    {
        let value = match (&binding.kind, coordinate) {
            (ExploreSourceBindingKindIr::Singleton { .. }, None) => {
                prover.eval_site(&site.expression, &env)?
            }
            (ExploreSourceBindingKindIr::Finite { domain }, Some((minimum, maximum))) => {
                let original = prover.eval_domain(domain, &site.expression, &env)?;
                let AbstractValue::Int(original) = original.value else {
                    return Err(prover.issue(
                        &site.expression,
                        RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                        "classification box requires finite integer coordinates",
                    ));
                };
                let mut interval = IntInterval::new(i128::from(*minimum), i128::from(*maximum))
                    .filter(|interval| {
                        interval.minimum >= original.minimum && interval.maximum <= original.maximum
                    })
                    .ok_or_else(|| {
                        prover.issue(
                            &site.expression,
                            RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                            "classification coordinate exceeds the checked source enclosure",
                        )
                    })?;
                if minimum != maximum {
                    interval.correlation = Some(Correlation::axis(axis).ok_or_else(|| {
                        prover.issue(
                            &site.expression,
                            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
                            "classification box exceeds the bounded correlation axis count",
                        )
                    })?);
                }
                axis += 1;
                prover.budget_owned_value(AbstractValue::Int(interval), &site.expression)?
            }
            _ => {
                return Err(prover.issue(
                    &site.expression,
                    RelationalEndpointTotalityIssueReason::ExactDomainUnavailable,
                    "classification coordinate is not aligned with its source binding",
                ))
            }
        };
        prover.require_bounded_value(&value.value, &site.expression)?;
        prover.retain_value(&mut retained, &value.value, &site.expression)?;
        env.insert(site.binder.clone(), value.value.clone());
    }
    let after = prover.eval_site(&sites.successor, &env)?;
    prover.require_bounded_value(&after.value, &sites.successor)?;
    prover.retain_value(&mut retained, &after.value, &sites.successor)?;
    let successor = &sites.successor;
    env.insert(
        CheckedBinderSiteId::Structural {
            analysis_program: successor.analysis_program.clone(),
            declaration: successor.declaration.clone(),
            normalized_declaration_ordinal: successor.normalized_declaration_ordinal,
            ast_path: successor.ast_path.clone(),
            binder_path: vec![CheckedResolutionRecorder::BINDER_EXPLORE_ROLE, 2].into_boxed_slice(),
        },
        after.value.clone(),
    );

    let mut admissions = Vec::new();
    #[cfg(test)]
    {
        prover.trace_unknown_equalities =
            std::env::var_os("FUTURUNA_EXPLORE_ADMISSION_TRACE").is_some();
    }
    for site in sites.admissions.iter() {
        let value = prover.eval_site(site, &env)?;
        let truth = value.value.truth();
        let decision = truth.and_then(TruthDomain::singleton);
        admissions.push(decision);
        #[cfg(test)]
        if prover.trace_unknown_equalities {
            eprintln!(
                "CANONICAL_BOX_ADMISSION ordinal={}; decision={decision:?}",
                admissions.len() - 1
            );
            for ((_, family, _), cached) in prover.rule_family_cache.borrow().iter() {
                if family.name.contains("gyldig") || family.name.contains("afstemt") {
                    if cached
                        .value
                        .truth()
                        .is_some_and(|truth| truth.singleton().is_none())
                    {
                        eprintln!("CANONICAL_BOX_UNKNOWN_VALIDITY family={family:?}");
                    }
                }
            }
        }
        if decision == Some(false) {
            // Subsequent predicates are not executed on rejected subjects.
            admissions.resize(sites.admissions.len(), None);
            return Ok(finish_box_proof(
                &prover,
                checked,
                coordinates,
                BoxClassification {
                    admissions: admissions.into_boxed_slice(),
                    selections: vec![None; query.finds.len()].into_boxed_slice(),
                },
            ));
        }
        // All later lanes run only when this admission holds. Narrowing is
        // sound even if this first lane remains unknown over the whole box.
        prover.refine_condition_into(site, true, &mut env);
    }
    let mut selections = Vec::new();
    #[cfg(test)]
    {
        prover.trace_unknown_equalities = false;
    }
    for (find, site) in query.finds.iter().zip(sites.find_predicates.iter()) {
        let decision = match (&find.find, site) {
            (ExploreFindIr::All { .. }, None) => Some(true),
            (ExploreFindIr::Matches { .. } | ExploreFindIr::Violations { .. }, Some(site)) => {
                // Solver authority is confined to this direct FIND predicate,
                // after the checked interpreter proved every admission true.
                // Prove only non-selection; a relaxed SAT model is not a case.
                if integer_solver && admissions.iter().all(|value| *value == Some(true)) {
                    prover.arithmetic_comparison = Some((
                        site.clone(),
                        matches!(find.find, ExploreFindIr::Violations { .. }),
                    ));
                }
                #[cfg(test)]
                if std::env::var_os("FUTURUNA_EXPLORE_BOXES").is_some() {
                    prover.trace_comparison_site = Some(site.clone());
                }
                let value = prover.eval_site(site, &env)?;
                prover.arithmetic_comparison = None;
                value
                    .value
                    .truth()
                    .and_then(TruthDomain::singleton)
                    .map(|truth| {
                        if matches!(find.find, ExploreFindIr::Violations { .. }) {
                            !truth
                        } else {
                            truth
                        }
                    })
            }
            _ => None,
        };
        selections.push(decision);
    }
    Ok(finish_box_proof(
        &prover,
        checked,
        coordinates,
        BoxClassification {
            admissions: admissions.into_boxed_slice(),
            selections: selections.into_boxed_slice(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser, TypeChecker};

    fn bound_clamp_fixture(clamp: &str, call: &str) -> String {
        format!(
            r#"
{clamp}
> tax(x: Int) -> Int {{ {call} * 100 }}
? explore bound_clamp_boundary {{
    from {{
        vary before in range(-164, 164)
        given context = ()
    }}
    transition after = before + 1
    where before true
    find rows = violations of after * 100 - tax(after) >= before * 100 - tax(before)
}}
"#
        )
    }

    #[test]
    #[ignore = "explicit installed-solver checked bound-clamp edge"]
    fn checked_bound_clamp_dependencies_preserve_order_and_replay() {
        for (operator, bound) in [("<", 3), ("<=", -3), (">", -3), (">=", 3)] {
            for head in ["| cap(a: Int, b: Int) ->", "> cap(a: Int, b: Int) -> Int"] {
                let source = bound_clamp_fixture(
                    &format!("{head} {{ if a {operator} b {{ a }} else {{ b }} }}"),
                    &format!("cap(x * 8 / 100, {bound})"),
                );
                let mut lexer = Lexer::new(&source);
                let statements = Parser::new(lexer.tokenize(), &source)
                    .parse_program()
                    .unwrap();
                let artifacts =
                    TypeChecker::check_with_explore_artifacts(&statements, None, &source);
                assert!(
                    artifacts.diagnostics.is_empty(),
                    "{:?}",
                    artifacts.diagnostics
                );
                let checked = artifacts.checked_exploration_query(0).unwrap();
                let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
                let prove = |precision| {
                    prove_box_with_precision(
                        &index,
                        &artifacts.checked_resolutions,
                        &checked,
                        &[Some((-164, 163)), None],
                        true,
                        true,
                        precision,
                    )
                    .unwrap()
                };
                let legacy = prove(IntegerPrecision::Clamps);
                assert!(!legacy.all_admitted_not_selected(), "{source}");
                assert!(legacy.arithmetic_relaxation_sat, "{source}");
                let refined = prove(IntegerPrecision::BoundClamps);
                assert!(refined.all_admitted_not_selected(), "{source}");
                assert_eq!(
                    refined.derivation_root(),
                    prove(IntegerPrecision::BoundClamps).derivation_root(),
                    "fresh checked bound-clamp replay: {source}"
                );
            }
        }
    }

    #[test]
    #[ignore = "explicit installed-solver checked bound-clamp edge"]
    fn checked_bound_clamp_preserves_legacy_roots_and_strict_operand_identity() {
        for (clamp, call, closed, total) in [
            // A successful literal-clamp recipe must remain byte-identical.
            (
                "| cap(a: Int, b: Int) -> if a < 3 { a } else { 3 }",
                "cap(x * 8 / 100, 3)",
                Some(true),
                true,
            ),
            // Changing either returned operand is not the checked min/max.
            (
                "| cap(a: Int, b: Int) -> if a < b { a } else { b + 1000 }",
                "cap(x * 8 / 100, 3)",
                Some(false),
                true,
            ),
            (
                "| cap(a: Int, b: Int) -> if a > b { a + 1000 } else { b }",
                "cap(x * 8 / 100, -3)",
                Some(false),
                true,
            ),
            (
                "| cap(a: Int, b: Int, c: Int) -> if a < b { a } else { c }",
                "cap(x * 8 / 100, 3, 1003)",
                Some(false),
                true,
            ),
            // Even equal constant enclosures do not equate distinct binders.
            (
                "| cap(a: Int, b: Int, c: Int) -> if a < b { a } else { c }",
                "cap(x * 8 / 100, 3, 3)",
                None,
                true,
            ),
            // A varying bound cannot become a constant after guard refinement.
            (
                "| cap(a: Int, b: Int) -> if a < b { a } else { b }",
                "cap(x * 8 / 100, x * 9 / 100)",
                None,
                true,
            ),
            // Never erase a reachable strict failure in a branch's block.
            (
                "| cap(a: Int, b: Int) -> if a < b { = bad = 1 / (a - a); a } else { b }",
                "cap(x * 8 / 100, 3)",
                None,
                false,
            ),
        ] {
            let source = bound_clamp_fixture(clamp, call);
            let mut lexer = Lexer::new(&source);
            let statements = Parser::new(lexer.tokenize(), &source)
                .parse_program()
                .unwrap();
            let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
            assert!(
                artifacts.diagnostics.is_empty(),
                "{:?}",
                artifacts.diagnostics
            );
            let checked = artifacts.checked_exploration_query(0).unwrap();
            let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
            let prove = |precision| {
                prove_box_with_precision(
                    &index,
                    &artifacts.checked_resolutions,
                    &checked,
                    &[Some((-164, 163)), None],
                    true,
                    true,
                    precision,
                )
            };
            let legacy = prove(IntegerPrecision::Clamps);
            let refined = prove(IntegerPrecision::BoundClamps);
            assert_eq!(legacy.is_ok(), total, "{source}");
            assert_eq!(refined.is_ok(), total, "{source}");
            if let (Ok(legacy), Ok(refined)) = (legacy, refined) {
                if let Some(closed) = closed {
                    assert_eq!(refined.all_admitted_not_selected(), closed, "{source}");
                }
                assert_eq!(
                    legacy.derivation_root(),
                    refined.derivation_root(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    #[ignore = "explicit installed-solver checked clamp edge"]
    fn checked_clamp_dependencies_cross_zero_preserve_polarity_and_legacy_roots() {
        let template = r#"
CLAMP
EXTRA
> tax(x: Int) -> Int { clamp(x * 8 / 100) * 100 }
? explore clamp_boundary {
    from {
        vary before in range(-164, 164)
        given context = ()
    }
    transition after = before + 1
    where before true
    find rows = FIND of after * 100 - tax(after) CMP before * 100 - tax(before)
}
"#;
        for (operator, constant, bound, value, extra, exact) in [
            (">", 0, 0, "x", "", true),
            (">=", 3, 3, "x", "", true),
            ("<", 0, 0, "x", "", true),
            ("<=", -3, -3, "x", "", true),
            (">", 0, 3, "x", "", false),
            (">", 0, 0, "x + 3", "", false),
            ("<", 0, 0, "0 - x", "", true),
            ("<=", 0, -3, "-3 - x", "", true),
            (">", 0, 3, "x - 3", "", true),
            (">=", 0, -3, "x - (-3)", "", true),
            ("<", 0, 0, "x - 0", "", false),
            (
                ">",
                0,
                0,
                "x",
                "| exception extra clamp(x: Int) -> 1000 under x == 10",
                false,
            ),
        ] {
            for (form, clamp) in [
                (
                    "rules",
                    "| clamp(x: Int) -> DEFAULT\n| exception clamp_side clamp(x: Int) -> VALUE under x OP BOUND",
                ),
                (
                    "if-rule",
                    "| clamp(x: Int) -> if x OP BOUND { VALUE } else { DEFAULT }",
                ),
                (
                    "if-function",
                    "> clamp(x: Int) -> Int { if x OP BOUND { VALUE } else { DEFAULT } }",
                ),
            ] {
                if !extra.is_empty() && form != "rules" {
                    continue;
                }
                if value.contains('-') && form == "rules" {
                    continue;
                }
                for (find, comparison) in [("matches", "<"), ("violations", ">=")] {
                    let source = template
                        .replace("CLAMP", clamp)
                        .replace("DEFAULT", &constant.to_string())
                        .replace("BOUND", &bound.to_string())
                        .replace("VALUE", value)
                        .replace(" OP ", &format!(" {operator} "))
                        .replace("EXTRA", extra)
                        .replace("FIND", find)
                        .replace("CMP", comparison);
                    let mut lexer = Lexer::new(&source);
                    let statements = Parser::new(lexer.tokenize(), &source)
                        .parse_program()
                        .unwrap();
                    let artifacts =
                        TypeChecker::check_with_explore_artifacts(&statements, None, &source);
                    assert!(
                        artifacts.diagnostics.is_empty(),
                        "{:?}",
                        artifacts.diagnostics
                    );
                    let checked = artifacts.checked_exploration_query(0).unwrap();
                    let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
                    let coordinates = [Some((-164, 163)), None];
                    let prove = |precision, coordinates: &[Option<(i64, i64)>]| {
                        prove_box_with_precision(
                            &index,
                            &artifacts.checked_resolutions,
                            &checked,
                            coordinates,
                            true,
                            true,
                            precision,
                        )
                        .unwrap()
                    };
                    let legacy = prove(IntegerPrecision::Arithmetic, &coordinates);
                    assert!(!legacy.all_admitted_not_selected(), "{source}");
                    assert!(legacy.arithmetic_relaxation_sat, "{source}");
                    let refined = prove(IntegerPrecision::Clamps, &coordinates);
                    assert_eq!(refined.all_admitted_not_selected(), exact, "{source}");
                    if exact {
                        assert_eq!(
                            refined.derivation_root(),
                            prove(IntegerPrecision::Clamps, &coordinates).derivation_root(),
                            "fresh producer replay: {source}"
                        );
                    }
                    // Known identity-side proofs must retain the same old root;
                    // retaining dependencies must not change already precise calls.
                    if operator == ">" && constant == 0 && exact && value == "x" {
                        let positive = [Some((if form == "rules" { 0 } else { 13 }, 163)), None];
                        let old = prove(IntegerPrecision::Arithmetic, &positive);
                        assert!(old.all_admitted_not_selected());
                        assert_eq!(
                            old.derivation_root(),
                            prove(IntegerPrecision::Clamps, &positive).derivation_root()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cover_checked_clamp_preserves_rounded_income_difference_only_for_exact_identity() {
        let template = r#"
| clamp(x: Int) -> DEFAULT
| exception clamp_side clamp(x: Int) -> x under x > BOUND
EXTRA
> tax(x: Int) -> Int { clamp(x * 8 / 100) * 100 }
? explore clamp_identity {
    from {
        vary before in range(START, 164)
        given context = ()
    }
    transition after = before + 1
    where before clamp(before) == before
    find rows = matches of after * 100 - tax(after) < before * 100 - tax(before)
}
"#;
        for (start, default, bound, extra, identity) in [
            (0, 0, 0, "", true),
            (-1, 0, 0, "", false),
            (0, 1, 0, "", false),
            (0, 0, 1, "", false),
            (
                0,
                0,
                0,
                "| exception extra clamp(x: Int) -> -1 under x == 10",
                false,
            ),
        ] {
            let source = template
                .replace("START", &start.to_string())
                .replace("DEFAULT", &default.to_string())
                .replace("BOUND", &bound.to_string())
                .replace("EXTRA", extra);
            let mut lexer = Lexer::new(&source);
            let statements = Parser::new(lexer.tokenize(), &source)
                .parse_program()
                .unwrap();
            let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
            assert!(
                artifacts.diagnostics.is_empty(),
                "{:?}",
                artifacts.diagnostics
            );
            let checked = artifacts.checked_exploration_query(0).unwrap();
            let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
            let coordinates = [Some((start, 163)), None];
            let proof = prove_box_with_clamp_identities(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &coordinates,
                true,
                true,
            )
            .unwrap();
            assert_eq!(
                proof.classification.admissions[0] == Some(true),
                identity,
                "start={start} default={default} bound={bound} extra={extra}"
            );
            if identity {
                assert_eq!(proof.classification.selections[0], Some(false));
                let legacy = prove_box_with_branch_constants(
                    &index,
                    &artifacts.checked_resolutions,
                    &checked,
                    &coordinates,
                    true,
                )
                .unwrap();
                assert_eq!(legacy.classification.admissions[0], None);
            }
        }
    }

    #[test]
    fn cover_branch_constants_prove_minimum_without_treating_ranges_as_constants() {
        let template = r#"
> minimum(a: Int, b: Int) -> Int { if a < b { a } else { b } }
? explore constant_parameter {
    from {
        vary before in range(START, 164)
        given context = ()
    }
    transition after = before + 1
    where before minimum(BOUND, before) == BOUND
    find rows = matches of false
}
"#;
        for (start, bound, expected) in [(0, 0, Some(true)), (-1, 0, None), (0, 1, None)] {
            let source = template
                .replace("START", &start.to_string())
                .replace("BOUND", &bound.to_string());
            let mut lexer = Lexer::new(&source);
            let statements = Parser::new(lexer.tokenize(), &source)
                .parse_program()
                .unwrap();
            let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
            assert!(
                artifacts.diagnostics.is_empty(),
                "{:?}",
                artifacts.diagnostics
            );
            let checked = artifacts.checked_exploration_query(0).unwrap();
            let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
            let coordinates = [Some((start, 163)), None];
            let legacy = prove_box(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &coordinates,
            )
            .unwrap();
            assert_eq!(legacy.classification.admissions[0], None);
            let cover = prove_box_with_branch_constants(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &coordinates,
                true,
            )
            .unwrap();
            assert_eq!(cover.classification.admissions[0], expected);
        }
    }

    #[test]
    fn symbolic_scalar_calls_preserve_aliases_not_equal_enclosures() {
        let template = r#"
# ScalarIdentityPair(left: Int, right: Int)
DEFINITION
> facts(x: Int) -> ScalarIdentityPair { ScalarIdentityPair(LEFT, RIGHT) }
> valid(x: Int) -> Bool {
    = pair = facts(x)
    pair.left == pair.right
}
? explore scalar_identity {
    from {
        vary before in range(-10, 11)
        given context = ()
    }
    transition after = before + 1
    where before valid(before)
    find rows = all
}
"#;
        let definitions = [
            "| positive(x: Int) -> 0\n| exception positive_case positive(x: Int) -> x under x > 0",
            "> positive(x: Int) -> Int { if x > 0 { x } else { 0 } }",
        ];
        for definition in definitions {
            for (left, right, expected) in [
                ("positive(x)", "positive(x)", Some(true)),
                ("positive(x)", "positive(-x)", None),
                ("positive(x) + 1", "positive(x) + 2", Some(false)),
            ] {
                let source = template
                    .replace("DEFINITION", definition)
                    .replace("LEFT", left)
                    .replace("RIGHT", right);
                let mut lexer = Lexer::new(&source);
                let statements = Parser::new(lexer.tokenize(), &source)
                    .parse_program()
                    .unwrap();
                let artifacts =
                    TypeChecker::check_with_explore_artifacts(&statements, None, &source);
                assert!(
                    artifacts.diagnostics.is_empty(),
                    "{:?}",
                    artifacts.diagnostics
                );
                let checked = artifacts.checked_exploration_query(0).unwrap();
                let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
                let proof = classify_box(
                    &index,
                    &artifacts.checked_resolutions,
                    &checked,
                    &[Some((-10, 10)), None],
                )
                .unwrap();
                assert_eq!(proof.admissions.as_ref(), [expected], "{source}");
            }
        }
        let source = template
            .replace(
                "DEFINITION",
                r#"
# Offset(offset: Int) {
    | value(x: Int) -> if x > 0 { x + offset } else { offset }
}
"#,
            )
            .replace("LEFT", "Offset(0).value(x)")
            .replace("RIGHT", "Offset(1).value(x)");
        let mut lexer = Lexer::new(&source);
        let statements = Parser::new(lexer.tokenize(), &source)
            .parse_program()
            .unwrap();
        let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts.checked_exploration_query(0).unwrap();
        let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        let proof = classify_box(
            &index,
            &artifacts.checked_resolutions,
            &checked,
            &[Some((-10, 10)), None],
        )
        .unwrap();
        assert_ne!(
            proof.admissions.as_ref(),
            [Some(true)],
            "scoped captures must distinguish calls"
        );
    }

    #[test]
    fn constant_quotients_drop_rounding_uncertainty_without_hiding_step_cliffs() {
        let source = r#"
> net(x: Int) -> Int {
    x * 100 - (x * 8 / 100) * 100 - (x / 1000) * 10000
}
? explore constant_quotient {
    from {
        vary before in range(-1000, 1001)
        given context = ()
    }
    transition after = before + 1
    find losses = violations of net(after) >= net(before)
}
"#;
        let mut lexer = Lexer::new(source);
        let parsed = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .unwrap();
        let statements = crate::prepend_prelude(crate::parse_prelude(), &parsed);
        let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts.checked_exploration_query(0).unwrap();
        let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        let net = |x: i64| x * 100 - (x * 8 / 100) * 100 - (x / 1000) * 10000;
        for (low, high, expected) in [
            (-998, -1, Some(false)),
            (0, 998, Some(false)),
            (999, 999, Some(true)),
            (998, 1000, None),
        ] {
            let proof = classify_box(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &[Some((low, high)), None],
            )
            .unwrap();
            assert_eq!(proof.selections.as_ref(), [expected], "{low}..{high}");
            if let Some(selected) = expected {
                assert!((low..=high).all(|x| (net(x + 1) < net(x)) == selected));
            } else {
                assert!((low..=high).any(|x| net(x + 1) < net(x)));
                assert!((low..=high).any(|x| net(x + 1) >= net(x)));
            }
        }
    }

    #[test]
    fn adjacent_rounding_box_preserves_integer_units() {
        let source = r#"
> net(x: Int) -> Int {
    x * 100 - (x * 8 / 100) * 100
}
? explore adjacent {
    from {
        vary before in range(0, 1001)
        given context = ()
    }
    transition after = before + 1
    where before before >= 0
    find losses = violations of net(after) >= net(before)
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .unwrap();
        let statements = crate::prepend_prelude(crate::parse_prelude(), &statements);
        let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts.checked_exploration_query(0).unwrap();
        let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        let result = classify_box(
            &index,
            &artifacts.checked_resolutions,
            &checked,
            &[Some((0, 1000)), None],
        )
        .unwrap();
        assert_eq!(result.admissions.as_ref(), [Some(true)]);
        assert_eq!(result.selections.as_ref(), [Some(false)]);
        assert!(classify_box(
            &index,
            &artifacts.checked_resolutions,
            &checked,
            &[Some((0, 1001)), None]
        )
        .is_err());
    }

    /// Explicit canonical-model experiment, excluded from routine test runs.
    #[test]
    #[ignore = "canonical 2026 proof/output experiment; run explicitly"]
    fn canonical_2026_box_output() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/danish-income-tax/personskat-income-distance-unit.explore.runa");
        let source = std::fs::read_to_string(&fixture).unwrap();
        let mut lexer = Lexer::new(&source);
        let parsed = Parser::new(lexer.tokenize(), &source)
            .parse_program()
            .unwrap();
        let statements = crate::prepend_prelude(crate::parse_prelude(), &parsed);
        let artifacts = TypeChecker::check_with_explore_artifacts(
            &statements,
            Some(fixture.parent().unwrap().to_string_lossy().into_owned()),
            &source,
        );
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts.checked_exploration_query(0).unwrap();
        let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        let default_coordinates = [
            [Some((1000, 1100)), Some((50, 50)), Some((0, 0)), None, None],
            [
                Some((349499, 349499)),
                Some((50, 50)),
                Some((0, 0)),
                None,
                None,
            ],
            [Some((1000, 1100)), Some((0, 199)), Some((1, 1)), None, None],
            [
                Some((342497, 342497)),
                Some((0, 199)),
                Some((0, 0)),
                None,
                None,
            ],
            [
                Some((342497, 342497)),
                Some((0, 199)),
                Some((1, 1)),
                None,
                None,
            ],
            [
                Some((342000, 342498)),
                Some((0, 199)),
                Some((0, 0)),
                None,
                None,
            ],
        ];
        // Explicit measurement control, not query syntax or proof authority.
        // Each supplied box still passes classify_box's checked-domain bounds.
        let coordinates = std::env::var("FUTURUNA_EXPLORE_BOXES")
            .ok()
            .map(|json| {
                serde_json::from_str::<Vec<[[i64; 2]; 3]>>(&json)
                    .expect("FUTURUNA_EXPLORE_BOXES must contain income/km/direction bound pairs")
                    .into_iter()
                    .map(|axes| {
                        [
                            Some((axes[0][0], axes[0][1])),
                            Some((axes[1][0], axes[1][1])),
                            Some((axes[2][0], axes[2][1])),
                            None,
                            None,
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| default_coordinates.to_vec());
        let measure = |coordinates: Coordinates| {
            let started = std::time::Instant::now();
            let result = prove_box_with_precision(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &coordinates,
                std::env::var_os("FUTURUNA_EXPLORE_COVER_PROOF").is_some(),
                std::env::var_os("FUTURUNA_EXPLORE_CLAMP_PROOF").is_some(),
                if std::env::var_os("FUTURUNA_EXPLORE_CLAMP_DEPENDENCIES").is_some() {
                    IntegerPrecision::Clamps
                } else {
                    IntegerPrecision::None
                },
            )
            .map(|proof| proof.classification);
            eprintln!(
                "CANONICAL_2026_BOX {coordinates:?}: {result:?} elapsed={:?}",
                started.elapsed()
            );
        };
        for coordinates in coordinates {
            measure(coordinates.to_vec());
        }
        // Optional measurement REPL: retain the already checked model between
        // boxes instead of paying its load cost for each follow-up question.
        // This ignored diagnostic does not issue or accept journal evidence.
        if std::env::var_os("FUTURUNA_EXPLORE_BOXES_STDIN").is_some() {
            use std::io::BufRead;
            eprintln!("CANONICAL_BOX_READY: enter JSON boxes or quit");
            for line in std::io::stdin().lock().lines() {
                let line = line.unwrap();
                if line.trim() == "quit" {
                    break;
                }
                let boxes: Vec<[[i64; 2]; 3]> = serde_json::from_str(&line).unwrap();
                for axes in boxes {
                    measure(vec![
                        Some((axes[0][0], axes[0][1])),
                        Some((axes[1][0], axes[1][1])),
                        Some((axes[2][0], axes[2][1])),
                        None,
                        None,
                    ]);
                }
                eprintln!("CANONICAL_BOX_READY: enter JSON boxes or quit");
            }
        }
    }
}
