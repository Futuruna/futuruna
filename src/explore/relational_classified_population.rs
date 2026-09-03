//! Aggregate classification counts derived from the case-root support DAG.
//!
//! These counts are logical population measures. They do not depend on how
//! many concrete `CaseId`s were retained and they never infer cases from the
//! number of support fragments. Open prefixes expose exact totals for the
//! classified support already sealed plus an exact candidate denominator;
//! only a completely classified root upgrades those totals to final counts.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, SelectionDecision};
use super::relational_support_planner::{RelationalRootObligationPlan, RelationalSupportPlan};
use super::support_cell::SupportCellId;
use super::support_evidence::{
    SupportEvidenceCatalogBuilder, SupportEvidenceClassificationView, SupportEvidenceRecord,
    SupportEvidenceSnapshot,
};

/// Exact mutually exclusive counts over one fully certified case population.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedRelationalClassificationCounts {
    candidates: u128,
    rejected: u128,
    admitted_not_selected: u128,
    admitted_selected: u128,
}

/// Exact accounting for the classified portion of one authenticated prefix.
///
/// `candidates` is the exact case-root cardinality. The other fields count
/// only sealed classification leaves, so while `complete` is false they are
/// monotone lower bounds on their final populations. No unclassified leaf is
/// assigned an outcome merely because an ancestor has some other proof fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassificationProgressCounts {
    candidates: u128,
    classified: u128,
    rejected: u128,
    admitted_not_selected: u128,
    admitted_selected: u128,
    complete: bool,
}

impl RelationalClassificationProgressCounts {
    pub(crate) fn derive(
        plan: &RelationalSupportPlan,
        support: &SupportEvidenceSnapshot,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedRelationalClassificationCountsError> {
        Self::derive_from_view(plan, support.classification_view(), question_id)
    }

    pub(crate) fn derive_from_builder(
        plan: &RelationalSupportPlan,
        support: &SupportEvidenceCatalogBuilder,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedRelationalClassificationCountsError> {
        Self::derive_from_view(plan, support.classification_view(), question_id)
    }

    fn derive_from_view(
        plan: &RelationalSupportPlan,
        support: SupportEvidenceClassificationView<'_>,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedRelationalClassificationCountsError> {
        if !plan.validate_root() {
            return Err(CertifiedRelationalClassificationCountsError::InvalidSupportPlanRoot);
        }
        let [registered_question_id] = plan.question_ids() else {
            return Err(CertifiedRelationalClassificationCountsError::UnsupportedQuestionSet);
        };
        if *registered_question_id != question_id {
            return Err(
                CertifiedRelationalClassificationCountsError::UnknownQuestion { question_id },
            );
        }

        match plan.root_obligations() {
            RelationalRootObligationPlan::ResolvedExactEmpty { admission_id }
                if *admission_id == plan.admission_id()
                    && plan.root_cell_id().is_none()
                    && support.root_cell_ids().next().is_none() =>
            {
                Ok(Self {
                    candidates: 0,
                    classified: 0,
                    rejected: 0,
                    admitted_not_selected: 0,
                    admitted_selected: 0,
                    complete: true,
                })
            }
            RelationalRootObligationPlan::ResolvedExactEmpty { .. } => {
                Err(CertifiedRelationalClassificationCountsError::SupportPlanScopeMismatch)
            }
            RelationalRootObligationPlan::CellBacked { root_cell_id, .. } => {
                let mut support_roots = support.root_cell_ids();
                let support_root_matches =
                    support_roots.next() == Some(*root_cell_id) && support_roots.next().is_none();
                if plan.root_cell_id() != Some(*root_cell_id) || !support_root_matches {
                    return Err(
                        CertifiedRelationalClassificationCountsError::SupportPlanScopeMismatch,
                    );
                }
                derive_positive_progress(*root_cell_id, plan.admission_id(), question_id, support)
            }
        }
    }

    pub(crate) const fn candidates(self) -> u128 {
        self.candidates
    }

    pub(crate) const fn classified(self) -> u128 {
        self.classified
    }

    pub(crate) const fn rejected(self) -> u128 {
        self.rejected
    }

    pub(crate) const fn admitted(self) -> u128 {
        self.admitted_not_selected + self.admitted_selected
    }

    pub(crate) const fn admitted_not_selected(self) -> u128 {
        self.admitted_not_selected
    }

    pub(crate) const fn admitted_selected(self) -> u128 {
        self.admitted_selected
    }

    pub(crate) const fn is_complete(self) -> bool {
        self.complete
    }
}

impl CertifiedRelationalClassificationCounts {
    pub(crate) fn derive(
        plan: &RelationalSupportPlan,
        support: &SupportEvidenceSnapshot,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedRelationalClassificationCountsError> {
        if !support.catalog_is_sealed()
            || !support.support_frontier_is_complete()
            || !support.obligation_frontier_is_complete()
        {
            return Err(CertifiedRelationalClassificationCountsError::SupportEvidenceOpen);
        }

        let progress = RelationalClassificationProgressCounts::derive(plan, support, question_id)?;
        if !progress.is_complete() || progress.classified() != progress.candidates() {
            return Err(CertifiedRelationalClassificationCountsError::SupportEvidenceOpen);
        }
        Ok(Self {
            candidates: progress.candidates(),
            rejected: progress.rejected(),
            admitted_not_selected: progress.admitted_not_selected(),
            admitted_selected: progress.admitted_selected(),
        })
    }

    pub(crate) const fn candidates(self) -> u128 {
        self.candidates
    }

    pub(crate) const fn rejected(self) -> u128 {
        self.rejected
    }

    pub(crate) const fn admitted(self) -> u128 {
        self.admitted_not_selected + self.admitted_selected
    }

    pub(crate) const fn admitted_not_selected(self) -> u128 {
        self.admitted_not_selected
    }

    pub(crate) const fn admitted_selected(self) -> u128 {
        self.admitted_selected
    }
}

#[derive(Clone, Copy)]
struct LayerFact<T> {
    conclusion: T,
}

#[derive(Default)]
struct DirectFacts {
    admission: BTreeMap<SupportCellId, LayerFact<AdmissionDecision>>,
    selection: BTreeMap<SupportCellId, LayerFact<SelectionDecision>>,
    cardinality: BTreeMap<SupportCellId, u128>,
}

fn derive_positive_progress(
    root_cell_id: SupportCellId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support: SupportEvidenceClassificationView<'_>,
) -> Result<RelationalClassificationProgressCounts, CertifiedRelationalClassificationCountsError> {
    // Evidence catalogs may contain auxiliary/source proof cells. Exact case
    // counts are a reduction over the case-root subtree only; an unrelated
    // open mapped image must not poison this result.
    let mut reachable = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    let mut children_by_parent = BTreeMap::<SupportCellId, Box<[SupportCellId]>>::new();
    let mut parent_by_child = BTreeMap::<SupportCellId, SupportCellId>::new();
    collect_case_root_tree(
        root_cell_id,
        &support,
        &mut visiting,
        &mut reachable,
        &mut children_by_parent,
        &mut parent_by_child,
    )?;
    if parent_by_child.contains_key(&root_cell_id) {
        return Err(
            CertifiedRelationalClassificationCountsError::AmbiguousPartitionTree {
                cell_id: root_cell_id,
            },
        );
    }

    let facts = collect_direct_facts(admission_id, question_id, &reachable, &support)?;
    let active_leaves = reachable
        .iter()
        .filter(|cell_id| !children_by_parent.contains_key(*cell_id))
        .copied()
        .collect::<BTreeSet<_>>();
    let sealed_leaves = support
        .sealed_leaf_ids()
        .filter(|cell_id| reachable.contains(cell_id))
        .collect::<BTreeSet<_>>();
    if !sealed_leaves.is_subset(&active_leaves) {
        return Err(CertifiedRelationalClassificationCountsError::PartitionCoverageMismatch);
    }

    let root_cardinality = exact_cell_cardinality(root_cell_id, &facts, &support)?;
    let mut counts = RelationalClassificationProgressCounts {
        candidates: root_cardinality,
        classified: 0,
        rejected: 0,
        admitted_not_selected: 0,
        admitted_selected: 0,
        complete: active_leaves == sealed_leaves,
    };
    let mut visited_active_leaves = BTreeSet::new();
    collect_progress_counts(
        root_cell_id,
        None,
        None,
        &children_by_parent,
        &active_leaves,
        &sealed_leaves,
        &facts,
        &support,
        &mut visited_active_leaves,
        &mut counts,
    )?;
    if visited_active_leaves != active_leaves {
        return Err(CertifiedRelationalClassificationCountsError::PartitionCoverageMismatch);
    }

    let outcome_total = counts
        .rejected
        .checked_add(counts.admitted_not_selected)
        .and_then(|value| value.checked_add(counts.admitted_selected));
    if outcome_total != Some(counts.classified)
        || counts.classified > root_cardinality
        || (counts.complete && counts.classified != root_cardinality)
    {
        return Err(
            CertifiedRelationalClassificationCountsError::CardinalityConservationMismatch {
                root: root_cardinality,
                classified: counts.classified,
            },
        );
    }
    Ok(counts)
}

fn collect_case_root_tree(
    cell_id: SupportCellId,
    support: &SupportEvidenceClassificationView<'_>,
    visiting: &mut BTreeSet<SupportCellId>,
    reachable: &mut BTreeSet<SupportCellId>,
    children_by_parent: &mut BTreeMap<SupportCellId, Box<[SupportCellId]>>,
    parent_by_child: &mut BTreeMap<SupportCellId, SupportCellId>,
) -> Result<(), CertifiedRelationalClassificationCountsError> {
    if reachable.contains(&cell_id) {
        return Ok(());
    }
    if !visiting.insert(cell_id) {
        return Err(
            CertifiedRelationalClassificationCountsError::AmbiguousPartitionTree { cell_id },
        );
    }
    if let Some(partition) = support.partition_for_parent(cell_id) {
        let children = partition.child_ids().to_vec().into_boxed_slice();
        for child_id in children.iter().copied() {
            if parent_by_child.insert(child_id, cell_id).is_some() {
                return Err(
                    CertifiedRelationalClassificationCountsError::AmbiguousPartitionTree {
                        cell_id: child_id,
                    },
                );
            }
            collect_case_root_tree(
                child_id,
                support,
                visiting,
                reachable,
                children_by_parent,
                parent_by_child,
            )?;
        }
        children_by_parent.insert(cell_id, children);
    }
    visiting.remove(&cell_id);
    reachable.insert(cell_id);
    Ok(())
}

fn collect_direct_facts(
    admission_id: AdmissionId,
    question_id: QuestionId,
    reachable: &BTreeSet<SupportCellId>,
    support: &SupportEvidenceClassificationView<'_>,
) -> Result<DirectFacts, CertifiedRelationalClassificationCountsError> {
    let mut facts = DirectFacts::default();
    for record in support.evidence() {
        if !reachable.contains(&record.cell_id()) {
            continue;
        }
        match record {
            SupportEvidenceRecord::Cardinality(evidence) => insert_cardinality(
                &mut facts.cardinality,
                evidence.obligation().cell_id(),
                evidence.exact_cardinality(),
            )?,
            SupportEvidenceRecord::Injectivity(evidence) => {
                let cell_id = evidence.obligation().cell_id();
                let cell = support.cell(cell_id).ok_or(
                    CertifiedRelationalClassificationCountsError::UnknownEvidenceCell { cell_id },
                )?;
                let cardinality = cell
                    .cardinality_with_injectivity(evidence)
                    .map_err(|_| {
                        CertifiedRelationalClassificationCountsError::InvalidCardinalityEvidence {
                            cell_id,
                        }
                    })?
                    .exact();
                if let Some(cardinality) = cardinality {
                    insert_cardinality(&mut facts.cardinality, cell_id, cardinality)?;
                }
            }
            SupportEvidenceRecord::Admission(evidence)
                if evidence.obligation().claim().admission_id() == admission_id =>
            {
                insert_fact(
                    &mut facts.admission,
                    evidence.obligation().cell_id(),
                    *evidence.conclusion(),
                    CertifiedRelationalClassificationCountsError::ContradictoryAdmission {
                        cell_id: evidence.obligation().cell_id(),
                    },
                )?;
            }
            SupportEvidenceRecord::Selection(evidence)
                if evidence.obligation().claim().question_id() == question_id =>
            {
                insert_fact(
                    &mut facts.selection,
                    evidence.obligation().cell_id(),
                    *evidence.conclusion(),
                    CertifiedRelationalClassificationCountsError::ContradictorySelection {
                        cell_id: evidence.obligation().cell_id(),
                    },
                )?;
            }
            SupportEvidenceRecord::Admission(_)
            | SupportEvidenceRecord::Selection(_)
            | SupportEvidenceRecord::UniformValue(_)
            | SupportEvidenceRecord::UniformMechanism(_) => {}
        }
    }
    Ok(facts)
}

#[allow(clippy::too_many_arguments)]
fn collect_progress_counts(
    cell_id: SupportCellId,
    inherited_admission: Option<AdmissionDecision>,
    inherited_selection: Option<SelectionDecision>,
    children_by_parent: &BTreeMap<SupportCellId, Box<[SupportCellId]>>,
    active_leaves: &BTreeSet<SupportCellId>,
    sealed_leaves: &BTreeSet<SupportCellId>,
    facts: &DirectFacts,
    support: &SupportEvidenceClassificationView<'_>,
    visited_leaves: &mut BTreeSet<SupportCellId>,
    counts: &mut RelationalClassificationProgressCounts,
) -> Result<(), CertifiedRelationalClassificationCountsError> {
    let admission = merge_fact(
        inherited_admission,
        facts.admission.get(&cell_id).map(|fact| fact.conclusion),
        CertifiedRelationalClassificationCountsError::ContradictoryAdmission { cell_id },
    )?;
    let selection = merge_fact(
        inherited_selection,
        facts.selection.get(&cell_id).map(|fact| fact.conclusion),
        CertifiedRelationalClassificationCountsError::ContradictorySelection { cell_id },
    )?;

    if let Some(children) = children_by_parent.get(&cell_id) {
        for child_id in children.iter().copied() {
            collect_progress_counts(
                child_id,
                admission,
                selection,
                children_by_parent,
                active_leaves,
                sealed_leaves,
                facts,
                support,
                visited_leaves,
                counts,
            )?;
        }
        return Ok(());
    }

    if !active_leaves.contains(&cell_id) || !visited_leaves.insert(cell_id) {
        return Err(CertifiedRelationalClassificationCountsError::PartitionCoverageMismatch);
    }
    if !sealed_leaves.contains(&cell_id) {
        return Ok(());
    }
    let cardinality = exact_cell_cardinality(cell_id, facts, support)?;
    counts.classified = counts
        .classified
        .checked_add(cardinality)
        .ok_or(CertifiedRelationalClassificationCountsError::CardinalityOverflow)?;
    match admission
        .ok_or(CertifiedRelationalClassificationCountsError::MissingAdmission { cell_id })?
    {
        AdmissionDecision::Rejected => {
            if selection.is_some() {
                return Err(
                    CertifiedRelationalClassificationCountsError::SelectionWithoutAdmission {
                        cell_id,
                    },
                );
            }
            counts.rejected = counts
                .rejected
                .checked_add(cardinality)
                .ok_or(CertifiedRelationalClassificationCountsError::CardinalityOverflow)?;
        }
        AdmissionDecision::Admitted => match selection
            .ok_or(CertifiedRelationalClassificationCountsError::MissingSelection { cell_id })?
        {
            SelectionDecision::NotSelected => {
                counts.admitted_not_selected = counts
                    .admitted_not_selected
                    .checked_add(cardinality)
                    .ok_or(CertifiedRelationalClassificationCountsError::CardinalityOverflow)?;
            }
            SelectionDecision::Selected => {
                counts.admitted_selected = counts
                    .admitted_selected
                    .checked_add(cardinality)
                    .ok_or(CertifiedRelationalClassificationCountsError::CardinalityOverflow)?;
            }
        },
    }
    Ok(())
}

fn exact_cell_cardinality(
    cell_id: SupportCellId,
    facts: &DirectFacts,
    support: &SupportEvidenceClassificationView<'_>,
) -> Result<u128, CertifiedRelationalClassificationCountsError> {
    if let Some(count) = facts.cardinality.get(&cell_id).copied() {
        return Ok(count);
    }
    let cell = support
        .cell(cell_id)
        .ok_or(CertifiedRelationalClassificationCountsError::UnknownEvidenceCell { cell_id })?;
    if let Some(count) = cell.cardinality().exact() {
        return Ok(count);
    }
    if let Some(count) = support
        .partition_for_parent(cell_id)
        .and_then(|partition| partition.cardinality().exact())
    {
        return Ok(count);
    }
    Err(CertifiedRelationalClassificationCountsError::OpenLeafCardinality { cell_id })
}

fn insert_cardinality(
    facts: &mut BTreeMap<SupportCellId, u128>,
    cell_id: SupportCellId,
    value: u128,
) -> Result<(), CertifiedRelationalClassificationCountsError> {
    match facts.insert(cell_id, value) {
        Some(existing) if existing != value => Err(
            CertifiedRelationalClassificationCountsError::ContradictoryCardinality {
                cell_id,
                first: existing,
                second: value,
            },
        ),
        _ => Ok(()),
    }
}

fn insert_fact<T: Copy + Eq>(
    facts: &mut BTreeMap<SupportCellId, LayerFact<T>>,
    cell_id: SupportCellId,
    conclusion: T,
    contradiction: CertifiedRelationalClassificationCountsError,
) -> Result<(), CertifiedRelationalClassificationCountsError> {
    match facts.get(&cell_id) {
        Some(existing) if existing.conclusion != conclusion => Err(contradiction),
        Some(_) => Ok(()),
        None => {
            facts.insert(cell_id, LayerFact { conclusion });
            Ok(())
        }
    }
}

fn merge_fact<T: Copy + Eq>(
    inherited: Option<T>,
    direct: Option<T>,
    contradiction: CertifiedRelationalClassificationCountsError,
) -> Result<Option<T>, CertifiedRelationalClassificationCountsError> {
    match (inherited, direct) {
        (Some(left), Some(right)) if left != right => Err(contradiction),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CertifiedRelationalClassificationCountsError {
    InvalidSupportPlanRoot,
    UnsupportedQuestionSet,
    UnknownQuestion {
        question_id: QuestionId,
    },
    SupportEvidenceOpen,
    SupportPlanScopeMismatch,
    AmbiguousPartitionTree {
        cell_id: SupportCellId,
    },
    PartitionCoverageMismatch,
    UnknownEvidenceCell {
        cell_id: SupportCellId,
    },
    InvalidCardinalityEvidence {
        cell_id: SupportCellId,
    },
    ContradictoryCardinality {
        cell_id: SupportCellId,
        first: u128,
        second: u128,
    },
    ContradictoryAdmission {
        cell_id: SupportCellId,
    },
    ContradictorySelection {
        cell_id: SupportCellId,
    },
    MissingAdmission {
        cell_id: SupportCellId,
    },
    MissingSelection {
        cell_id: SupportCellId,
    },
    SelectionWithoutAdmission {
        cell_id: SupportCellId,
    },
    OpenLeafCardinality {
        cell_id: SupportCellId,
    },
    CardinalityConservationMismatch {
        root: u128,
        classified: u128,
    },
    CardinalityOverflow,
}

impl fmt::Display for CertifiedRelationalClassificationCountsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSupportPlanRoot => {
                formatter.write_str("classification counts received an invalid support plan")
            }
            Self::UnsupportedQuestionSet => formatter.write_str(
                "certified classification-count acceleration requires exactly one semantic question",
            ),
            Self::UnknownQuestion { question_id } => write!(
                formatter,
                "classification counts requested unregistered question {question_id:?}"
            ),
            Self::SupportEvidenceOpen => formatter.write_str(
                "classification counts require closed support, partition, and obligation frontiers",
            ),
            Self::SupportPlanScopeMismatch => formatter
                .write_str("classification count roots do not match the installed support plan"),
            Self::AmbiguousPartitionTree { .. } => {
                formatter.write_str("classification support contains an ambiguous partition tree")
            }
            Self::PartitionCoverageMismatch => {
                formatter.write_str("classification leaves do not exactly cover the support root")
            }
            Self::UnknownEvidenceCell { .. } => {
                formatter.write_str("classification evidence names an unknown support cell")
            }
            Self::InvalidCardinalityEvidence { .. } => {
                formatter.write_str("classification cardinality evidence is invalid")
            }
            Self::ContradictoryCardinality { .. } => {
                formatter.write_str("classification support has contradictory cardinalities")
            }
            Self::ContradictoryAdmission { .. } => {
                formatter.write_str("classification support has contradictory admission facts")
            }
            Self::ContradictorySelection { .. } => {
                formatter.write_str("classification support has contradictory FIND facts")
            }
            Self::MissingAdmission { .. } => {
                formatter.write_str("a classified support leaf has no admission decision")
            }
            Self::MissingSelection { .. } => {
                formatter.write_str("an admitted support leaf has no FIND decision")
            }
            Self::SelectionWithoutAdmission { .. } => {
                formatter.write_str("a rejected support leaf cannot carry a FIND decision")
            }
            Self::OpenLeafCardinality { .. } => {
                formatter.write_str("a classified support leaf has no exact case cardinality")
            }
            Self::CardinalityConservationMismatch { root, classified } => write!(
                formatter,
                "classified population {classified} does not conserve root cardinality {root}"
            ),
            Self::CardinalityOverflow => {
                formatter.write_str("classification population cardinality exceeds u128")
            }
        }
    }
}

impl Error for CertifiedRelationalClassificationCountsError {}
