//! Bounded checked source-factor covers of one canonical ranked page.
//!
//! Splits are exact tuple partitions, not claims of uniform parent admission.
//! Only rejected or admitted/not-selected leaves can close a cover. Failed
//! proof search gives no coverage and leaves the original page residual.

use super::*;
use crate::explore::support_cell::{RankedProductBox, SupportExpr};

pub(crate) const MAX_COVER_NODES: usize = 31;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoverNode {
    Split {
        axis: u32,
        pivot: u128,
    },
    Leaf {
        outcome: RelationalCertifiedRegionConclusion,
        derivation_root: [u8; 32],
        coordinate_count: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoverLeaf {
    pub(crate) region: RankedProductBox,
    pub(crate) outcome: RelationalCertifiedRegionConclusion,
    pub(crate) derivation_root: [u8; 32],
}

/// Untrusted durable recipe. Only CheckedRegionCover's fresh replay grants
/// semantic authority; identity checks alone do not establish a partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoverArtifact {
    nodes: Box<[CoverNode]>,
    rejected_count: u128,
}

impl CoverArtifact {
    pub(crate) fn restore(
        nodes: Box<[CoverNode]>,
        rejected_count: u128,
    ) -> Result<Self, RelationalRegionProofError> {
        if nodes.is_empty() || nodes.len() > MAX_COVER_NODES {
            return Err(RelationalRegionProofError::InvalidArtifactShape);
        }
        Ok(Self {
            nodes,
            rejected_count,
        })
    }

    pub(crate) fn nodes(&self) -> &[CoverNode] {
        &self.nodes
    }
    pub(crate) const fn rejected_count(&self) -> u128 {
        self.rejected_count
    }
    pub(crate) fn digest(&self) -> [u8; 32] {
        let mut hash =
            CanonicalProofHasher::new(b"futuruna.explore.checked-region-cover.recipe.v1");
        hash.u128(self.rejected_count);
        hash.u128(self.nodes.len() as u128);
        for node in &self.nodes {
            match node {
                CoverNode::Split { axis, pivot } => {
                    hash.u8(1);
                    hash.u32(*axis);
                    hash.u128(*pivot);
                }
                CoverNode::Leaf {
                    outcome,
                    derivation_root,
                    coordinate_count,
                } => {
                    hash.u8(2);
                    hash.u8(outcome.canonical_tag());
                    hash.digest(*derivation_root);
                    hash.u128(*coordinate_count);
                }
            }
        }
        hash.finish()
    }
}

/// Fresh producer authority; cannot be constructed by the journal decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedRegionCover {
    nodes: Box<[CoverNode]>,
    leaves: Box<[CoverLeaf]>,
}

impl CheckedRegionCover {
    pub(crate) fn artifact(&self) -> Result<CoverArtifact, RelationalRegionProofError> {
        let rejected_count = self
            .leaves
            .iter()
            .filter(|leaf| leaf.outcome == RelationalCertifiedRegionConclusion::Rejected)
            .try_fold(0u128, |count, leaf| {
                count.checked_add(leaf.region.coordinate_count())
            })
            .ok_or(RelationalRegionProofError::InvalidArtifactShape)?;
        CoverArtifact::restore(self.nodes.clone(), rejected_count)
    }
    pub(super) fn nodes(&self) -> &[CoverNode] {
        &self.nodes
    }
    pub(crate) fn leaves(&self) -> &[CoverLeaf] {
        &self.leaves
    }

    pub(super) fn prove(
        classifier: &CheckedBoxClassifier,
        checked: &CheckedExploreQueryView<'_>,
        plan: &RelationalSupportPlan,
        expression: &SupportExpr,
    ) -> Option<Self> {
        let inventory = RelationalProofStrategyInventory::from_checked(checked, plan).ok()?;
        let root = RankedProductBox::from_expr(expression).ok()?;
        if root.factors().len() != inventory.axes().len()
            || root.factors().len() != inventory.finite_binding_indices().len()
        {
            return None;
        }
        let mut nodes = vec![];
        let mut leaves = vec![];
        prove_node(
            classifier,
            checked,
            &inventory,
            &root,
            &root,
            &mut nodes,
            &mut leaves,
        )?;
        Some(Self {
            nodes: nodes.into_boxed_slice(),
            leaves: leaves.into_boxed_slice(),
        })
    }

    /// Follow the recorded partition recipe, not the current search heuristic.
    /// Every split is reconstructed and every leaf is freshly classified.
    pub(super) fn reverify(
        classifier: &CheckedBoxClassifier,
        checked: &CheckedExploreQueryView<'_>,
        plan: &RelationalSupportPlan,
        expression: &SupportExpr,
        nodes: &[CoverNode],
    ) -> Option<Self> {
        if nodes.is_empty() || nodes.len() > MAX_COVER_NODES {
            return None;
        }
        let inventory = RelationalProofStrategyInventory::from_checked(checked, plan).ok()?;
        let root = RankedProductBox::from_expr(expression).ok()?;
        if root.factors().len() != inventory.axes().len()
            || root.factors().len() != inventory.finite_binding_indices().len()
        {
            return None;
        }
        let mut next = 0;
        let mut leaves = vec![];
        reverify_node(
            classifier,
            checked,
            &inventory,
            &root,
            nodes,
            &mut next,
            &mut leaves,
        )?;
        if next != nodes.len() {
            return None;
        }
        Some(Self {
            nodes: nodes.to_vec().into_boxed_slice(),
            leaves: leaves.into_boxed_slice(),
        })
    }
}

pub(super) fn prove_artifact(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
    target: &RelationalRegionProofTarget<'_>,
    replay_authority_id: [u8; 32],
    classifier: &CheckedBoxClassifier,
    recorded: Option<&CoverArtifact>,
) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
    let residual = || fallback(RelationalRegionProofResidual::SelectionTruthVariesOverAxis);
    if !target.product_rank {
        return Ok(residual());
    }
    let cover = match recorded {
        Some(recipe) => CheckedRegionCover::reverify(
            classifier,
            checked,
            plan,
            target.cell.expression(),
            recipe.nodes(),
        ),
        None => CheckedRegionCover::prove(classifier, checked, plan, target.cell.expression()),
    };
    let Some(cover) = cover else {
        return Ok(residual());
    };
    let recipe = cover.artifact()?;
    let inventory = RelationalProofStrategyInventory::from_checked(checked, plan)?;
    let first_axis = inventory
        .axes()
        .iter()
        .find(|axis| Some(&axis.binding_index()) == inventory.finite_binding_indices().first())
        .ok_or(RelationalRegionProofError::InvalidArtifactShape)?;
    let (Some(assignment), Some(source), Some(successor), Some(root_cell_id)) = (
        plan.source_assignments().cell(),
        plan.source_rows().cell(),
        plan.successor_coordinates().cell(),
        plan.root_cell_id(),
    ) else {
        return Ok(residual());
    };
    let mut artifact = RelationalRegionProofArtifact {
        schema_version: RELATIONAL_CHECKED_COVER_REGION_PROOF_VERSION,
        product_rank: true,
        certificate_id: [0; 32],
        replay_authority_id,
        classification_capsule_id: capsule.id(),
        basis: RelationalRegionProofBasis::CheckedSourceCover {
            checked_program: decode_lowercase_sha256(checked.program_hash())
                .ok_or(RelationalRegionProofError::InvalidCheckedProgramDigest)?,
            derivation_root: recipe.digest(),
        },
        relation_id: checked.relation_id(),
        admission_id: checked.admission_id(),
        question_id: checked.question_ids()[0],
        plan_root: plan.root(),
        root_cell_id,
        subject: target.subject,
        conclusion: None,
        starter_region_id: RelationalStarterRegionId([0; 32]),
        source_assignment_cell_id: assignment.id(),
        source_row_cell_id: source.id(),
        successor_coordinate_cell_id: successor.id(),
        axis_stage_id: first_axis.stage_id(),
        axis_dimension_id: first_axis.dimension_id(),
        axis_cell_id: first_axis.cell().id(),
        value_start: i64::try_from(target.coordinate_start)
            .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?,
        value_end_exclusive: i64::try_from(target.coordinate_end_exclusive)
            .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?,
        coordinate_start: target.coordinate_start,
        coordinate_end_exclusive: target.coordinate_end_exclusive,
        case_cardinality: target.coordinate_end_exclusive - target.coordinate_start,
        selected_formula_digest: recipe.digest(),
        cover: Some(Box::new(recipe)),
    };
    artifact.starter_region_id = derive_starter_region_id(&artifact);
    artifact.certificate_id = derive_certificate_id(&artifact);
    artifact.validate_identity()?;
    let proof = VerifiedRelationalRegionProof {
        artifact,
        evidence: VerifiedRegionEvidence::Cover(cover),
    };
    let events = proof.cover_events(target.cell)?;
    Ok(RelationalRegionProofOutcome::ExactEmpty(
        RelationalRegionSupportClosure { proof, events },
    ))
}

fn classify_leaf(
    classifier: &CheckedBoxClassifier,
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    region: &RankedProductBox,
) -> Option<CoverLeaf> {
    let enclosure = region.enclosure().ok()?;
    let mut coordinates = Vec::with_capacity(checked.closed_query.source.bindings.len());
    for binding in &checked.closed_query.source.bindings {
        coordinates.push(match binding.kind {
            ExploreSourceBindingKindIr::Singleton { .. } => None,
            ExploreSourceBindingKindIr::Finite { .. } => {
                let ordinal = inventory
                    .finite_binding_indices()
                    .iter()
                    .position(|index| {
                        usize::try_from(*index).ok() == Some(binding.binding_index)
                    })?;
                let axis = inventory.axes().iter().find(|axis| {
                    usize::try_from(axis.binding_index()).ok() == Some(binding.binding_index)
                })?;
                if axis.coordinate_start() != 0 {
                    return None;
                }
                let (low, high) = *enclosure.get(ordinal)?;
                if high >= axis.cardinality() {
                    return None;
                }
                let value = |coordinate| {
                    i128::from(axis.value_start())
                        .checked_add(i128::try_from(coordinate).ok()?)
                        .and_then(|v| i64::try_from(v).ok())
                };
                Some((value(low)?, value(high)?))
            }
        });
    }
    let proof = classifier.prove_cover_coordinates(checked, &coordinates)?;
    let outcome = if proof.all_rejected() {
        RelationalCertifiedRegionConclusion::Rejected
    } else if proof.all_admitted_not_selected() {
        RelationalCertifiedRegionConclusion::AdmittedNotSelected
    } else {
        return None;
    };
    Some(CoverLeaf {
        region: region.clone(),
        outcome,
        derivation_root: proof.derivation_root(),
    })
}

fn split(
    region: &RankedProductBox,
    axis: u32,
    pivot: u128,
) -> Option<(RankedProductBox, RankedProductBox)> {
    let axis = usize::try_from(axis).ok()?;
    let &(low, high) = region.factors().get(axis)?;
    if pivot <= low || pivot >= high {
        return None;
    }
    Some((
        region.restrict_factor(axis, low, pivot).ok()??,
        region.restrict_factor(axis, pivot, high).ok()??,
    ))
}

fn prove_node(
    classifier: &CheckedBoxClassifier,
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    root: &RankedProductBox,
    region: &RankedProductBox,
    nodes: &mut Vec<CoverNode>,
    leaves: &mut Vec<CoverLeaf>,
) -> Option<()> {
    if nodes.len() >= MAX_COVER_NODES {
        return None;
    }
    if let Some(leaf) = classify_leaf(classifier, checked, inventory, region) {
        nodes.push(CoverNode::Leaf {
            outcome: leaf.outcome,
            derivation_root: leaf.derivation_root,
            coordinate_count: leaf.region.coordinate_count(),
        });
        leaves.push(leaf);
        return Some(());
    }
    // Isolate declared upper endpoints before interior bisection. Prefer a
    // small endpoint-bearing axis (e.g. intervention) before a broad one;
    // otherwise use authored inner axes. This avoids walking every commute
    // while an unresolved income upper endpoint is still available.
    // This is scheduling only; no split or probe itself supplies evidence.
    let enclosure = region.enclosure().ok()?;
    let (axis, &(low, high)) = enclosure
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, (a, b))| a < b)
        .min_by_key(|(axis, (low, high))| {
            if high.checked_add(1) == Some(root.factors()[*axis].1) {
                (0u8, high - low)
            } else {
                (1u8, 0)
            }
        })?;
    let pivot = if high.checked_add(1)? == root.factors().get(axis)?.1 {
        high
    } else {
        low.checked_add((high - low) / 2)?.checked_add(1)?
    };
    let axis = u32::try_from(axis).ok()?;
    let (left, right) = split(region, axis, pivot)?;
    nodes.push(CoverNode::Split { axis, pivot });
    prove_node(classifier, checked, inventory, root, &left, nodes, leaves)?;
    prove_node(classifier, checked, inventory, root, &right, nodes, leaves)
}

fn reverify_node(
    classifier: &CheckedBoxClassifier,
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    region: &RankedProductBox,
    nodes: &[CoverNode],
    next: &mut usize,
    leaves: &mut Vec<CoverLeaf>,
) -> Option<()> {
    let node = nodes.get(*next)?;
    *next += 1;
    match node {
        CoverNode::Split { axis, pivot } => {
            let (left, right) = split(region, *axis, *pivot)?;
            reverify_node(classifier, checked, inventory, &left, nodes, next, leaves)?;
            reverify_node(classifier, checked, inventory, &right, nodes, next, leaves)
        }
        CoverNode::Leaf {
            outcome,
            derivation_root,
            coordinate_count,
        } => {
            let leaf = classify_leaf(classifier, checked, inventory, region)?;
            if leaf.outcome != *outcome
                || leaf.derivation_root != *derivation_root
                || leaf.region.coordinate_count() != *coordinate_count
            {
                return None;
            }
            leaves.push(leaf);
            Some(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relational_support_planner::RelationalSupportPlanner;
    use crate::{Lexer, Parser, TypeChecker};

    #[test]
    fn ranked_box_cover_proves_mixed_admission_and_replays_only_valid_leaf_proofs() {
        let source = r#"
# State(income: Int, distance: Int)
> total(s: State) -> Int { sum_list([s.income * 3, s.distance * 2]) }
? explore mixed_page {
    from {
        vary income in range(0, 151)
        vary distance in range(0, 3)
        vary direction in range(0, 2)
        let before = State(income, distance)
        let context = direction
    }
    transition after = State(before.income + 1 - context, before.distance + context)
    where after after.income <= 150 && after.distance <= 2
    find losses = violations of total(after) >= total(before)
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
        let owned = Arc::new(
            artifacts
                .checked_exploration_query(0)
                .unwrap()
                .to_owned_checked_query(),
        );
        let checked = owned.view();
        let plan = RelationalSupportPlanner::from_checked(&checked)
            .unwrap()
            .plan()
            .unwrap();
        let classifier = CheckedBoxClassifier::new(artifacts, owned.clone(), &plan).unwrap();
        let expression = plan.cases().cell().unwrap().expression();
        let cover = CheckedRegionCover::prove(&classifier, &checked, &plan, expression).unwrap();
        let rejected: u128 = cover
            .leaves()
            .iter()
            .filter(|leaf| leaf.outcome == RelationalCertifiedRegionConclusion::Rejected)
            .map(|leaf| leaf.region.coordinate_count())
            .sum();
        let admitted: u128 = cover
            .leaves()
            .iter()
            .filter(|leaf| leaf.outcome == RelationalCertifiedRegionConclusion::AdmittedNotSelected)
            .map(|leaf| leaf.region.coordinate_count())
            .sum();
        assert_eq!((admitted, rejected), (752, 154));
        assert!(cover.nodes().len() <= MAX_COVER_NODES);
        assert_eq!(
            CheckedRegionCover::reverify(&classifier, &checked, &plan, expression, cover.nodes())
                .unwrap(),
            cover
        );
        let mut forgeries = vec![];
        let mut changed = cover.nodes().to_vec();
        let CoverNode::Split { pivot, .. } = &mut changed[0] else {
            panic!("mixed root needs a split")
        };
        *pivot = 0;
        forgeries.push(changed);
        let leaf_index = cover
            .nodes()
            .iter()
            .position(|node| matches!(node, CoverNode::Leaf { .. }))
            .unwrap();
        for change_outcome in [true, false] {
            let mut changed = cover.nodes().to_vec();
            let CoverNode::Leaf {
                outcome,
                derivation_root,
                ..
            } = &mut changed[leaf_index]
            else {
                unreachable!()
            };
            if change_outcome {
                *outcome = if *outcome == RelationalCertifiedRegionConclusion::Rejected {
                    RelationalCertifiedRegionConclusion::AdmittedNotSelected
                } else {
                    RelationalCertifiedRegionConclusion::Rejected
                };
            } else {
                derivation_root[0] ^= 0xff;
            }
            forgeries.push(changed);
        }
        let mut changed = cover.nodes().to_vec();
        changed.push(changed[leaf_index].clone());
        forgeries.push(changed);
        for nodes in forgeries {
            assert!(
                CheckedRegionCover::reverify(&classifier, &checked, &plan, expression, &nodes)
                    .is_none()
            );
        }
        // A known loss must remain residual, not be hidden in an admitted leaf.
        let source = source.replace("s.income * 3", "0 - s.income * 3");
        let mut lexer = Lexer::new(&source);
        let parsed = Parser::new(lexer.tokenize(), &source)
            .parse_program()
            .unwrap();
        let statements = crate::prepend_prelude(crate::parse_prelude(), &parsed);
        let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
        assert!(artifacts.diagnostics.is_empty());
        let owned = Arc::new(
            artifacts
                .checked_exploration_query(0)
                .unwrap()
                .to_owned_checked_query(),
        );
        let checked = owned.view();
        let plan = RelationalSupportPlanner::from_checked(&checked)
            .unwrap()
            .plan()
            .unwrap();
        let classifier = CheckedBoxClassifier::new(artifacts, owned.clone(), &plan).unwrap();
        assert!(CheckedRegionCover::prove(
            &classifier,
            &checked,
            &plan,
            plan.cases().cell().unwrap().expression()
        )
        .is_none());
    }
}
