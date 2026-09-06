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
}

impl CheckedBoxClassifier {
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
    let query = checked.closed_query;
    let sites = &checked.artifact.sites;
    let mut prover = EndpointTotalityProver::new(index, resolutions, checked.relation_id());
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
    for site in sites.admissions.iter() {
        let value = prover.eval_site(site, &env)?;
        let truth = value.value.truth();
        let decision = truth.and_then(TruthDomain::singleton);
        admissions.push(decision);
        if decision == Some(false) {
            // Subsequent predicates are not executed on rejected subjects.
            admissions.resize(sites.admissions.len(), None);
            return Ok(BoxClassification {
                admissions: admissions.into_boxed_slice(),
                selections: vec![None; query.finds.len()].into_boxed_slice(),
            });
        }
        // All later lanes run only when this admission holds. Narrowing is
        // sound even if this first lane remains unknown over the whole box.
        prover.refine_condition_into(site, true, &mut env);
    }
    let mut selections = Vec::new();
    for (find, site) in query.finds.iter().zip(sites.find_predicates.iter()) {
        let decision = match (&find.find, site) {
            (ExploreFindIr::All { .. }, None) => Some(true),
            (ExploreFindIr::Matches { .. } | ExploreFindIr::Violations { .. }, Some(site)) => {
                let value = prover.eval_site(site, &env)?;
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
    Ok(BoxClassification {
        admissions: admissions.into_boxed_slice(),
        selections: selections.into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser, TypeChecker};

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
        for coordinates in [
            [Some((1000, 1100)), Some((50, 50)), Some((0, 0)), None, None],
            [
                Some((349499, 349499)),
                Some((50, 50)),
                Some((0, 0)),
                None,
                None,
            ],
            [Some((1000, 1100)), Some((0, 199)), Some((1, 1)), None, None],
        ] {
            let started = std::time::Instant::now();
            let result = classify_box(
                &index,
                &artifacts.checked_resolutions,
                &checked,
                &coordinates,
            );
            eprintln!(
                "CANONICAL_2026_BOX {coordinates:?}: {result:?} elapsed={:?}",
                started.elapsed()
            );
        }
    }
}
