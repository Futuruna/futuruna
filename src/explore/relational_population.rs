//! Certified selected-case populations derived from exact support evidence.
//!
//! A closed support catalog can prove the exact FIND population without
//! materializing every `RelationalCaseId`. This module turns that proof DAG
//! into a small, typed upstream receipt for result and mechanism layers. It
//! never manufactures a `QuestionContentRoot`: concrete and symbolic
//! closure remain distinct authorities.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
use super::relational_support_planner::{
    RelationalRootObligationPlan, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::support_cell::SupportCellId;
use super::support_evidence::{
    SupportEvidenceClassificationView, SupportEvidenceRecord, SupportEvidenceRoot,
    SupportEvidenceSnapshot, ValidatedSupportEvidenceClosure,
};

const CERTIFIED_SELECTED_POPULATION_ROOT_V1: &[u8] =
    b"futuruna.explore.certified-selected-population-root.v1";
const CERTIFIED_SELECTED_FRAGMENT_COVERAGE_ROOT_V1: &[u8] =
    b"futuruna.explore.certified-selected-fragment-coverage-root.v1";

pub(crate) const CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION: u32 = 1;

/// Arrival-order-independent identity of one exact FIND population proved by
/// a support partition/evidence DAG.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CertifiedSelectedPopulationRoot([u8; 32]);

impl CertifiedSelectedPopulationRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent identity of the canonical disjoint fragments
/// through which a certified selected population can be consumed.
///
/// This is proof-support coverage, not an extensional question-content or
/// concrete CaseId-set root. It remains scoped to the population and support
/// evidence that certified the fragment decomposition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CertifiedSelectedFragmentCoverageRoot([u8; 32]);

impl CertifiedSelectedFragmentCoverageRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Compact commitment to the complete canonical fragment support of one
/// certified selected population.
///
/// The fragment root commits the sorted `(SupportCellId, exact_case_count)`
/// pairs retained by the population snapshot. Counts are repeated here so a
/// downstream consumer can compare its locally accumulated fragment stream
/// without treating the number of fragments as the number of cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedSelectedFragmentCoverageCommitment {
    root: CertifiedSelectedFragmentCoverageRoot,
    population_root: CertifiedSelectedPopulationRoot,
    support_evidence_root: SupportEvidenceRoot,
    exact_case_count: u128,
    fragment_count: u128,
}

impl CertifiedSelectedFragmentCoverageCommitment {
    pub(crate) const fn root(self) -> CertifiedSelectedFragmentCoverageRoot {
        self.root
    }

    pub(crate) const fn population_root(self) -> CertifiedSelectedPopulationRoot {
        self.population_root
    }

    pub(crate) const fn support_evidence_root(self) -> SupportEvidenceRoot {
        self.support_evidence_root
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.exact_case_count
    }

    pub(crate) const fn fragment_count(self) -> u128 {
        self.fragment_count
    }
}

/// One disjoint selected region whose exact extensional case cardinality is
/// certified. A fragment is a set, never a representative singleton.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedSelectedFragment {
    cell_id: SupportCellId,
    exact_case_count: u128,
}

impl CertifiedSelectedFragment {
    pub(crate) const fn cell_id(self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.exact_case_count
    }
}

/// Canonical durable payload. Restoration re-derives the population from the
/// named plan and support snapshot rather than trusting serialized counts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedSelectedPopulationSnapshot {
    pub(crate) version: u32,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan_root: RelationalSupportPlanRoot,
    support_evidence_root: SupportEvidenceRoot,
    root: CertifiedSelectedPopulationRoot,
    fragment_coverage: CertifiedSelectedFragmentCoverageCommitment,
    exact_case_count: u128,
    fragments: Box<[CertifiedSelectedFragment]>,
}

impl CertifiedSelectedPopulationSnapshot {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn support_evidence_root(&self) -> SupportEvidenceRoot {
        self.support_evidence_root
    }

    pub(crate) const fn root(&self) -> CertifiedSelectedPopulationRoot {
        self.root
    }

    pub(crate) const fn fragment_coverage(&self) -> CertifiedSelectedFragmentCoverageCommitment {
        self.fragment_coverage
    }

    pub(crate) const fn exact_case_count(&self) -> u128 {
        self.exact_case_count
    }

    pub(crate) fn fragments(&self) -> &[CertifiedSelectedFragment] {
        &self.fragments
    }
}

/// Immutable exact selected population. It intentionally contains no concrete
/// CaseIds; downstream consumers must either understand certified fragments or
/// bind them to an independently verified exact concrete case cover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedCertifiedSelectedPopulation {
    snapshot: CertifiedSelectedPopulationSnapshot,
}

#[derive(Clone, Copy)]
struct CertifiedSelectedSupportAuthority {
    root: SupportEvidenceRoot,
    catalog_is_sealed: bool,
    support_frontier_is_complete: bool,
    obligation_frontier_is_complete: bool,
}

impl ClosedCertifiedSelectedPopulation {
    pub(crate) fn derive(
        plan: &RelationalSupportPlan,
        support: &SupportEvidenceSnapshot,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedSelectedPopulationError> {
        Self::derive_from_closed_support(
            plan,
            CertifiedSelectedSupportAuthority {
                root: support.root(),
                catalog_is_sealed: support.catalog_is_sealed(),
                support_frontier_is_complete: support.support_frontier_is_complete(),
                obligation_frontier_is_complete: support.obligation_frontier_is_complete(),
            },
            support.classification_view(),
            support.active_leaf_ids(),
            question_id,
        )
    }

    /// Derive from a globally validated immutable builder prefix without first
    /// cloning its semantic graph into a [`SupportEvidenceSnapshot`].
    pub(crate) fn derive_from_validated_support(
        plan: &RelationalSupportPlan,
        support: &ValidatedSupportEvidenceClosure<'_>,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedSelectedPopulationError> {
        Self::derive_from_closed_support(
            plan,
            CertifiedSelectedSupportAuthority {
                root: support.root(),
                catalog_is_sealed: support.catalog_is_sealed(),
                support_frontier_is_complete: support.support_frontier_is_complete(),
                obligation_frontier_is_complete: support.obligation_frontier_is_complete(),
            },
            support.classification_view(),
            support.active_leaf_ids(),
            question_id,
        )
    }

    fn derive_from_closed_support(
        plan: &RelationalSupportPlan,
        authority: CertifiedSelectedSupportAuthority,
        support: SupportEvidenceClassificationView<'_>,
        active_leaf_ids: impl IntoIterator<Item = SupportCellId>,
        question_id: QuestionId,
    ) -> Result<Self, CertifiedSelectedPopulationError> {
        if !plan.validate_root() {
            return Err(CertifiedSelectedPopulationError::InvalidSupportPlanRoot);
        }
        if !authority.catalog_is_sealed
            || !authority.support_frontier_is_complete
            || !authority.obligation_frontier_is_complete
        {
            return Err(CertifiedSelectedPopulationError::SupportEvidenceOpen);
        }

        let [registered_question_id] = plan.question_ids() else {
            return Err(CertifiedSelectedPopulationError::UnsupportedQuestionSet);
        };
        if *registered_question_id != question_id {
            return Err(CertifiedSelectedPopulationError::UnknownQuestion { question_id });
        }
        let relation_id = plan.relation_id();
        let admission_id = plan.admission_id();
        let support_plan_root = plan.root();
        let support_evidence_root = authority.root;

        let fragments = match plan.root_obligations() {
            RelationalRootObligationPlan::ResolvedExactEmpty {
                admission_id: planned_admission_id,
            } => {
                if *planned_admission_id != admission_id
                    || support.root_cell_ids().next().is_some()
                    || plan.root_cell_id().is_some()
                {
                    return Err(CertifiedSelectedPopulationError::SupportPlanScopeMismatch);
                }
                Vec::new()
            }
            RelationalRootObligationPlan::CellBacked { root_cell_id, .. } => {
                let roots = support.root_cell_ids().collect::<Vec<_>>();
                if roots.as_slice() != &[*root_cell_id]
                    || plan.root_cell_id() != Some(*root_cell_id)
                {
                    return Err(CertifiedSelectedPopulationError::SupportPlanScopeMismatch);
                }
                derive_selected_fragments(
                    *root_cell_id,
                    admission_id,
                    question_id,
                    &support,
                    active_leaf_ids,
                )?
            }
        };

        let exact_case_count = fragments.iter().try_fold(0_u128, |total, fragment| {
            total
                .checked_add(fragment.exact_case_count)
                .ok_or(CertifiedSelectedPopulationError::CardinalityOverflow)
        })?;
        let fragments = fragments.into_boxed_slice();
        let root = derive_population_root(
            relation_id,
            admission_id,
            question_id,
            support_plan_root,
            support_evidence_root,
            exact_case_count,
            &fragments,
        );
        let fragment_coverage =
            derive_fragment_coverage(root, support_evidence_root, exact_case_count, &fragments);
        Ok(Self {
            snapshot: CertifiedSelectedPopulationSnapshot {
                version: CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION,
                relation_id,
                admission_id,
                question_id,
                support_plan_root,
                support_evidence_root,
                root,
                fragment_coverage,
                exact_case_count,
                fragments,
            },
        })
    }

    /// Restore only by re-deriving the mathematical population from its proof
    /// authorities and comparing the complete canonical payload.
    pub(crate) fn from_snapshot(
        snapshot: CertifiedSelectedPopulationSnapshot,
        plan: &RelationalSupportPlan,
        support: &SupportEvidenceSnapshot,
    ) -> Result<Self, CertifiedSelectedPopulationError> {
        if snapshot.version != CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION {
            return Err(
                CertifiedSelectedPopulationError::UnsupportedSnapshotVersion {
                    actual: snapshot.version,
                    expected: CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION,
                },
            );
        }
        let derived = Self::derive(plan, support, snapshot.question_id)?;
        if derived.snapshot != snapshot {
            return Err(CertifiedSelectedPopulationError::SnapshotMismatch);
        }
        Ok(derived)
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.snapshot.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.snapshot.admission_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.snapshot.question_id
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.snapshot.support_plan_root
    }

    pub(crate) const fn support_evidence_root(&self) -> SupportEvidenceRoot {
        self.snapshot.support_evidence_root
    }

    pub(crate) const fn root(&self) -> CertifiedSelectedPopulationRoot {
        self.snapshot.root
    }

    pub(crate) const fn fragment_coverage(&self) -> CertifiedSelectedFragmentCoverageCommitment {
        self.snapshot.fragment_coverage
    }

    pub(crate) const fn exact_cardinality(&self) -> u128 {
        self.snapshot.exact_case_count
    }

    pub(crate) const fn is_exact_empty(&self) -> bool {
        self.snapshot.exact_case_count == 0
    }

    pub(crate) fn fragments(&self) -> &[CertifiedSelectedFragment] {
        &self.snapshot.fragments
    }

    pub(crate) const fn snapshot(&self) -> &CertifiedSelectedPopulationSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LayerFact<T> {
    conclusion: T,
}

#[derive(Default)]
struct DirectFacts {
    admission: BTreeMap<SupportCellId, LayerFact<AdmissionDecision>>,
    selection: BTreeMap<SupportCellId, LayerFact<SelectionDecision>>,
    cardinality: BTreeMap<SupportCellId, u128>,
}

fn derive_selected_fragments(
    root_cell_id: SupportCellId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support: &SupportEvidenceClassificationView<'_>,
    active_leaf_ids: impl IntoIterator<Item = SupportCellId>,
) -> Result<Vec<CertifiedSelectedFragment>, CertifiedSelectedPopulationError> {
    let mut all_children_by_parent = BTreeMap::<SupportCellId, Box<[SupportCellId]>>::new();
    for partition in support.partitions() {
        let parent_id = partition.parent_id();
        if all_children_by_parent
            .insert(parent_id, partition.child_ids().to_vec().into_boxed_slice())
            .is_some()
        {
            return Err(CertifiedSelectedPopulationError::AmbiguousPartitionTree {
                cell_id: parent_id,
            });
        }
    }

    // Source-image and other auxiliary proof cells may coexist in the same
    // catalog without belonging to the sole case-root partition. Selected
    // population closure is intentionally scoped to that reachable subtree.
    let mut reachable = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    collect_reachable_cells(
        root_cell_id,
        &all_children_by_parent,
        &mut visiting,
        &mut reachable,
    )?;
    let children_by_parent = all_children_by_parent
        .into_iter()
        .filter(|(parent_id, _)| reachable.contains(parent_id))
        .collect::<BTreeMap<_, _>>();
    let mut parent_by_child = BTreeMap::<SupportCellId, SupportCellId>::new();
    for (parent_id, child_ids) in &children_by_parent {
        for child_id in child_ids.iter().copied() {
            if !reachable.contains(&child_id)
                || parent_by_child.insert(child_id, *parent_id).is_some()
            {
                return Err(CertifiedSelectedPopulationError::AmbiguousPartitionTree {
                    cell_id: child_id,
                });
            }
        }
    }
    if parent_by_child.contains_key(&root_cell_id) {
        return Err(CertifiedSelectedPopulationError::AmbiguousPartitionTree {
            cell_id: root_cell_id,
        });
    }

    let facts = collect_direct_facts(admission_id, question_id, &reachable, support)?;
    let active_leaves = active_leaf_ids
        .into_iter()
        .filter(|cell_id| reachable.contains(cell_id))
        .collect::<BTreeSet<_>>();
    let sealed_leaves = support
        .sealed_leaf_ids()
        .filter(|cell_id| reachable.contains(cell_id))
        .collect::<BTreeSet<_>>();
    if active_leaves != sealed_leaves {
        return Err(CertifiedSelectedPopulationError::SupportEvidenceOpen);
    }

    let mut leaf_decisions = BTreeMap::<SupportCellId, SelectionDecision>::new();
    validate_classification_tree(
        root_cell_id,
        None,
        None,
        &children_by_parent,
        &facts,
        &active_leaves,
        &mut leaf_decisions,
    )?;
    if leaf_decisions.len() != active_leaves.len() {
        return Err(CertifiedSelectedPopulationError::PartitionCoverageMismatch);
    }

    let mut fragments = Vec::new();
    collect_selected_fragments(
        root_cell_id,
        &children_by_parent,
        &leaf_decisions,
        &facts,
        support,
        &mut fragments,
    )?;
    fragments.sort_by_key(|fragment| fragment.cell_id);
    if fragments
        .windows(2)
        .any(|pair| pair[0].cell_id >= pair[1].cell_id)
    {
        return Err(CertifiedSelectedPopulationError::OverlappingSelectedFragments);
    }
    Ok(fragments)
}

fn collect_reachable_cells(
    cell_id: SupportCellId,
    children_by_parent: &BTreeMap<SupportCellId, Box<[SupportCellId]>>,
    visiting: &mut BTreeSet<SupportCellId>,
    reachable: &mut BTreeSet<SupportCellId>,
) -> Result<(), CertifiedSelectedPopulationError> {
    if reachable.contains(&cell_id) {
        return Ok(());
    }
    if !visiting.insert(cell_id) {
        return Err(CertifiedSelectedPopulationError::AmbiguousPartitionTree { cell_id });
    }
    if let Some(children) = children_by_parent.get(&cell_id) {
        for child_id in children.iter().copied() {
            collect_reachable_cells(child_id, children_by_parent, visiting, reachable)?;
        }
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
) -> Result<DirectFacts, CertifiedSelectedPopulationError> {
    let mut facts = DirectFacts::default();
    for record in support.evidence() {
        if !reachable.contains(&record.cell_id()) {
            continue;
        }
        match record {
            SupportEvidenceRecord::Cardinality(evidence) => {
                insert_cardinality(
                    &mut facts.cardinality,
                    evidence.obligation().cell_id(),
                    evidence.exact_cardinality(),
                )?;
            }
            SupportEvidenceRecord::Injectivity(evidence) => {
                let cell_id = evidence.obligation().cell_id();
                let cell = support
                    .cell(cell_id)
                    .ok_or(CertifiedSelectedPopulationError::UnknownEvidenceCell { cell_id })?;
                let cardinality =
                    cell.cardinality_with_injectivity(evidence)
                        .map_err(|_| {
                            CertifiedSelectedPopulationError::InvalidCardinalityEvidence { cell_id }
                        })?
                        .exact();
                if let Some(cardinality) = cardinality {
                    insert_cardinality(&mut facts.cardinality, cell_id, cardinality)?;
                }
            }
            SupportEvidenceRecord::Admission(evidence)
                if evidence.obligation().claim().admission_id() == admission_id =>
            {
                insert_layer_fact(
                    &mut facts.admission,
                    evidence.obligation().cell_id(),
                    *evidence.conclusion(),
                    CertifiedSelectedPopulationError::ContradictoryAdmissionCoverage {
                        cell_id: evidence.obligation().cell_id(),
                    },
                )?;
            }
            SupportEvidenceRecord::Selection(evidence)
                if evidence.obligation().claim().question_id() == question_id =>
            {
                insert_layer_fact(
                    &mut facts.selection,
                    evidence.obligation().cell_id(),
                    *evidence.conclusion(),
                    CertifiedSelectedPopulationError::ContradictorySelectionCoverage {
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

fn insert_layer_fact<T: Copy + Eq>(
    facts: &mut BTreeMap<SupportCellId, LayerFact<T>>,
    cell_id: SupportCellId,
    conclusion: T,
    contradiction: CertifiedSelectedPopulationError,
) -> Result<(), CertifiedSelectedPopulationError> {
    match facts.get_mut(&cell_id) {
        Some(existing) if existing.conclusion != conclusion => Err(contradiction),
        Some(_) => Ok(()),
        None => {
            facts.insert(cell_id, LayerFact { conclusion });
            Ok(())
        }
    }
}

fn insert_cardinality(
    cardinalities: &mut BTreeMap<SupportCellId, u128>,
    cell_id: SupportCellId,
    count: u128,
) -> Result<(), CertifiedSelectedPopulationError> {
    match cardinalities.insert(cell_id, count) {
        Some(existing) if existing != count => {
            Err(CertifiedSelectedPopulationError::ContradictoryCardinality {
                cell_id,
                first: existing,
                second: count,
            })
        }
        _ => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_classification_tree(
    cell_id: SupportCellId,
    inherited_admission: Option<AdmissionDecision>,
    inherited_selection: Option<SelectionDecision>,
    children_by_parent: &BTreeMap<SupportCellId, Box<[SupportCellId]>>,
    facts: &DirectFacts,
    active_leaves: &BTreeSet<SupportCellId>,
    leaf_decisions: &mut BTreeMap<SupportCellId, SelectionDecision>,
) -> Result<(), CertifiedSelectedPopulationError> {
    let admission = merge_layer(
        inherited_admission,
        facts.admission.get(&cell_id).map(|fact| fact.conclusion),
        CertifiedSelectedPopulationError::ContradictoryAdmissionCoverage { cell_id },
    )?;
    let selection = merge_layer(
        inherited_selection,
        facts.selection.get(&cell_id).map(|fact| fact.conclusion),
        CertifiedSelectedPopulationError::ContradictorySelectionCoverage { cell_id },
    )?;

    let Some(children) = children_by_parent.get(&cell_id) else {
        if !active_leaves.contains(&cell_id) {
            return Err(CertifiedSelectedPopulationError::PartitionCoverageMismatch);
        }
        let admission = admission
            .ok_or(CertifiedSelectedPopulationError::MissingAdmissionClassification { cell_id })?;
        let decision = match admission {
            AdmissionDecision::Rejected => {
                if selection.is_some() {
                    return Err(
                        CertifiedSelectedPopulationError::SelectionWithoutAdmission { cell_id },
                    );
                }
                SelectionDecision::NotSelected
            }
            AdmissionDecision::Admitted => selection.ok_or(
                CertifiedSelectedPopulationError::MissingSelectionClassification { cell_id },
            )?,
        };
        leaf_decisions.insert(cell_id, decision);
        return Ok(());
    };

    for child_id in children.iter().copied() {
        validate_classification_tree(
            child_id,
            admission,
            selection,
            children_by_parent,
            facts,
            active_leaves,
            leaf_decisions,
        )?;
    }
    Ok(())
}

fn merge_layer<T: Copy + Eq>(
    inherited: Option<T>,
    direct: Option<T>,
    contradiction: CertifiedSelectedPopulationError,
) -> Result<Option<T>, CertifiedSelectedPopulationError> {
    match (inherited, direct) {
        (Some(left), Some(right)) if left != right => Err(contradiction),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn collect_selected_fragments(
    cell_id: SupportCellId,
    children_by_parent: &BTreeMap<SupportCellId, Box<[SupportCellId]>>,
    leaf_decisions: &BTreeMap<SupportCellId, SelectionDecision>,
    facts: &DirectFacts,
    support: &SupportEvidenceClassificationView<'_>,
    output: &mut Vec<CertifiedSelectedFragment>,
) -> Result<SubtreeSelection, CertifiedSelectedPopulationError> {
    let children = children_by_parent.get(&cell_id);
    let selection = match children {
        None => match leaf_decisions.get(&cell_id) {
            Some(SelectionDecision::Selected) => SubtreeSelection::All,
            Some(SelectionDecision::NotSelected) => SubtreeSelection::None,
            None => return Err(CertifiedSelectedPopulationError::PartitionCoverageMismatch),
        },
        Some(children) => {
            let mut all = true;
            let mut any = false;
            let output_start = output.len();
            for child_id in children.iter().copied() {
                match collect_selected_fragments(
                    child_id,
                    children_by_parent,
                    leaf_decisions,
                    facts,
                    support,
                    output,
                )? {
                    SubtreeSelection::None => all = false,
                    SubtreeSelection::Some => {
                        all = false;
                        any = true;
                    }
                    SubtreeSelection::All => any = true,
                }
            }
            if all {
                if let Some(exact_case_count) = exact_cell_cardinality(cell_id, facts, support)? {
                    output.truncate(output_start);
                    output.push(CertifiedSelectedFragment {
                        cell_id,
                        exact_case_count,
                    });
                    return Ok(SubtreeSelection::All);
                }
                SubtreeSelection::All
            } else if any {
                SubtreeSelection::Some
            } else {
                SubtreeSelection::None
            }
        }
    };

    match selection {
        SubtreeSelection::None => Ok(SubtreeSelection::None),
        SubtreeSelection::Some => Ok(SubtreeSelection::Some),
        SubtreeSelection::All => {
            if children.is_none() {
                let exact_case_count = exact_cell_cardinality(cell_id, facts, support)?
                    .ok_or(CertifiedSelectedPopulationError::OpenSelectedCardinality { cell_id })?;
                output.push(CertifiedSelectedFragment {
                    cell_id,
                    exact_case_count,
                });
            }
            Ok(SubtreeSelection::All)
        }
    }
}

#[derive(Clone, Copy)]
enum SubtreeSelection {
    None,
    Some,
    All,
}

fn exact_cell_cardinality(
    cell_id: SupportCellId,
    facts: &DirectFacts,
    support: &SupportEvidenceClassificationView<'_>,
) -> Result<Option<u128>, CertifiedSelectedPopulationError> {
    if let Some(count) = facts.cardinality.get(&cell_id).copied() {
        return Ok(Some(count));
    }
    let cell = support
        .cell(cell_id)
        .ok_or(CertifiedSelectedPopulationError::UnknownEvidenceCell { cell_id })?;
    if let Some(count) = cell.cardinality().exact() {
        return Ok(Some(count));
    }
    if let Some(count) = support
        .partition_for_parent(cell_id)
        .and_then(|partition| partition.cardinality().exact())
    {
        return Ok(Some(count));
    }
    Ok(None)
}

fn derive_population_root(
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan_root: RelationalSupportPlanRoot,
    support_evidence_root: SupportEvidenceRoot,
    exact_case_count: u128,
    fragments: &[CertifiedSelectedFragment],
) -> CertifiedSelectedPopulationRoot {
    let mut hasher = CanonicalHasher::new(CERTIFIED_SELECTED_POPULATION_ROOT_V1);
    hasher.u32(CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION);
    hasher.digest(relation_id.bytes());
    hasher.digest(admission_id.bytes());
    hasher.digest(question_id.bytes());
    hasher.digest(support_plan_root.bytes());
    hasher.digest(support_evidence_root.bytes());
    hasher.u128(exact_case_count);
    hasher.len(fragments.len());
    for fragment in fragments {
        hasher.digest(fragment.cell_id.bytes());
        hasher.u128(fragment.exact_case_count);
    }
    CertifiedSelectedPopulationRoot(hasher.finish())
}

fn derive_fragment_coverage(
    population_root: CertifiedSelectedPopulationRoot,
    support_evidence_root: SupportEvidenceRoot,
    exact_case_count: u128,
    fragments: &[CertifiedSelectedFragment],
) -> CertifiedSelectedFragmentCoverageCommitment {
    let fragment_count = fragments.len() as u128;
    let mut hasher = CanonicalHasher::new(CERTIFIED_SELECTED_FRAGMENT_COVERAGE_ROOT_V1);
    hasher.digest(population_root.bytes());
    hasher.digest(support_evidence_root.bytes());
    hasher.u128(exact_case_count);
    hasher.u128(fragment_count);
    for fragment in fragments {
        hasher.digest(fragment.cell_id.bytes());
        hasher.u128(fragment.exact_case_count);
    }
    CertifiedSelectedFragmentCoverageCommitment {
        root: CertifiedSelectedFragmentCoverageRoot(hasher.finish()),
        population_root,
        support_evidence_root,
        exact_case_count,
        fragment_count,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CertifiedSelectedPopulationError {
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
    ContradictoryAdmissionCoverage {
        cell_id: SupportCellId,
    },
    ContradictorySelectionCoverage {
        cell_id: SupportCellId,
    },
    MissingAdmissionClassification {
        cell_id: SupportCellId,
    },
    MissingSelectionClassification {
        cell_id: SupportCellId,
    },
    SelectionWithoutAdmission {
        cell_id: SupportCellId,
    },
    OpenSelectedCardinality {
        cell_id: SupportCellId,
    },
    OverlappingSelectedFragments,
    CardinalityOverflow,
    UnsupportedSnapshotVersion {
        actual: u32,
        expected: u32,
    },
    SnapshotMismatch,
}

impl fmt::Display for CertifiedSelectedPopulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSupportPlanRoot => {
                formatter.write_str("certified population received an invalid support-plan root")
            }
            Self::UnsupportedQuestionSet => formatter.write_str(
                "certified selected-population acceleration requires exactly one semantic question",
            ),
            Self::UnknownQuestion { question_id } => write!(
                formatter,
                "certified population requested unregistered question {question_id:?}"
            ),
            Self::SupportEvidenceOpen => formatter.write_str(
                "certified population requires sealed support, partition, and proof frontiers",
            ),
            Self::SupportPlanScopeMismatch => formatter.write_str(
                "certified population support roots disagree with the registered support plan",
            ),
            Self::AmbiguousPartitionTree { .. } => formatter.write_str(
                "certified selected population requires one unambiguous rooted partition tree",
            ),
            Self::PartitionCoverageMismatch => formatter.write_str(
                "certified selected population does not cover exactly the active support leaves",
            ),
            Self::UnknownEvidenceCell { .. } => {
                formatter.write_str("certified population evidence names an unknown support cell")
            }
            Self::InvalidCardinalityEvidence { .. } => formatter
                .write_str("certified population contains invalid exact-cardinality evidence"),
            Self::ContradictoryCardinality { .. } => formatter.write_str(
                "certified population contains contradictory exact cardinalities for one cell",
            ),
            Self::ContradictoryAdmissionCoverage { .. } => formatter.write_str(
                "certified population contains contradictory admission coverage on one path",
            ),
            Self::ContradictorySelectionCoverage { .. } => formatter
                .write_str("certified population contains contradictory FIND coverage on one path"),
            Self::MissingAdmissionClassification { .. } => formatter.write_str(
                "certified population has an active case region without admission classification",
            ),
            Self::MissingSelectionClassification { .. } => formatter.write_str(
                "certified population has an admitted case region without FIND classification",
            ),
            Self::SelectionWithoutAdmission { .. } => formatter.write_str(
                "certified population has FIND evidence on a region not proved admitted",
            ),
            Self::OpenSelectedCardinality { .. } => formatter.write_str(
                "a selected support region has no exact extensional case cardinality proof",
            ),
            Self::OverlappingSelectedFragments => formatter.write_str(
                "certified selected population fragments are not a canonical disjoint set",
            ),
            Self::CardinalityOverflow => {
                formatter.write_str("certified selected population cardinality exceeds u128")
            }
            Self::UnsupportedSnapshotVersion { actual, expected } => write!(
                formatter,
                "unsupported certified selected-population snapshot version {actual}; expected {expected}"
            ),
            Self::SnapshotMismatch => formatter.write_str(
                "certified selected-population snapshot does not match its plan and evidence roots",
            ),
        }
    }
}

impl Error for CertifiedSelectedPopulationError {}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        Self(hasher)
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn len(&mut self, value: usize) {
        self.0.update((value as u128).to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
