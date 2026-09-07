//! Bounded checked source-factor covers of one canonical ranked page.
//!
//! Splits are exact tuple partitions, not claims of uniform parent admission.
//! Only rejected or admitted/not-selected leaves can close a cover. Failed
//! proof search gives no coverage and leaves the original page residual.

use super::*;
use crate::explore::relational_endpoint_totality_proof::abstract_classification::{
    coordinates_contained, CachedCoverScope, CheckedSourceBoxProof, MAX_CACHED_COVER_SCOPES,
    MAX_SCOPE_BINDINGS,
};
use crate::explore::support_cell::{RankedProductBox, SupportExpr};

// At most 32 leaves in a full binary cover. Share this bound with decoding:
// larger checked covers must not be produced only to fail cold journal replay.
pub(crate) const MAX_COVER_NODES: usize = 63;

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
    /// V7: a freshly checked superset domain plus exact page-leaf geometry.
    ScopedLeaf {
        outcome: RelationalCertifiedRegionConclusion,
        derivation_root: [u8; 32],
        coordinate_count: u128,
        scope: Box<[Option<(i64, i64)>]>,
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
        for node in &nodes {
            if let CoverNode::ScopedLeaf { scope, .. } = node {
                if scope.is_empty()
                    || scope.len() > MAX_SCOPE_BINDINGS
                    || scope.iter().flatten().any(|(low, high)| low > high)
                {
                    return Err(RelationalRegionProofError::InvalidArtifactShape);
                }
            }
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
    pub(crate) fn version(&self) -> u32 {
        if self
            .nodes
            .iter()
            .any(|node| matches!(node, CoverNode::ScopedLeaf { .. }))
        {
            RELATIONAL_SCOPED_COVER_REGION_PROOF_VERSION
        } else {
            RELATIONAL_CHECKED_COVER_REGION_PROOF_VERSION
        }
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
                CoverNode::ScopedLeaf {
                    outcome,
                    derivation_root,
                    coordinate_count,
                    scope,
                } => {
                    hash.u8(3);
                    hash.u8(outcome.canonical_tag());
                    hash.digest(*derivation_root);
                    hash.u128(*coordinate_count);
                    hash.u128(scope.len() as u128);
                    for coordinate in scope {
                        match coordinate {
                            None => hash.u8(0),
                            Some((low, high)) => {
                                hash.u8(1);
                                hash.i64(*low);
                                hash.i64(*high);
                            }
                        }
                    }
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
        if let Some(cover) = Self::from_cached_scopes(classifier, checked, &inventory, &root) {
            if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                eprintln!(
                    "Explore checked cached cover: nodes={}; leaves={}; cases={}",
                    cover.nodes.len(),
                    cover.leaves.len(),
                    root.coordinate_count()
                );
            }
            return Some(cover);
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

    fn from_cached_scopes(
        classifier: &CheckedBoxClassifier,
        checked: &CheckedExploreQueryView<'_>,
        inventory: &RelationalProofStrategyInventory,
        root: &RankedProductBox,
    ) -> Option<Self> {
        let scopes = classifier.cached_closed_cover_scopes(checked)?;
        if scopes.is_empty() || scopes.len() > MAX_CACHED_COVER_SCOPES {
            return None;
        }
        let mut nodes = vec![];
        let mut leaves = vec![];
        cached_cover_node(checked, inventory, root, &scopes, &mut nodes, &mut leaves)?;
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
        schema_version: recipe.version(),
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

fn region_coordinates(
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    region: &RankedProductBox,
) -> Option<Vec<Option<(i64, i64)>>> {
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
    Some(coordinates)
}

struct RegionProbe {
    proof: Arc<CheckedSourceBoxProof>,
    scope: Option<Box<[Option<(i64, i64)>]>>,
}

impl RegionProbe {
    fn split_hints(&self) -> &[(usize, i64)] {
        self.proof.split_hints()
    }
}

fn probe_region(
    classifier: &CheckedBoxClassifier,
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    region: &RankedProductBox,
) -> Option<RegionProbe> {
    let coordinates = region_coordinates(checked, inventory, region)?;
    // Try cached facts at all seven dyadic widths, but spend at most one new
    // shared-scope derivation before the exact local box. Several intermediate
    // widths can repeat the same solver timeout even when the local obligation
    // is cheap. This only changes scheduling; recorded V7 scopes still replay
    // independently of the current nomination policy.
    let mut fresh_scopes = 0;
    for refinement in 0..=6 {
        let Some(scope) = classifier.shared_cover_scope(&coordinates, refinement) else {
            continue;
        };
        if !classifier.cover_scope_is_cached(&scope) {
            if fresh_scopes == 1 {
                continue;
            }
            fresh_scopes += 1;
        }
        if let Some(proof) = classifier.prove_cached_cover_scope(checked, &scope) {
            let can_split = coordinates
                .iter()
                .flatten()
                .any(|(low, high)| i128::from(*high) - i128::from(*low) == 1)
                || proof.split_hints().iter().any(|&(axis, cut)| {
                    coordinates
                        .iter()
                        .flatten()
                        .nth(axis)
                        .is_some_and(|&(low, high)| low < cut && cut <= high)
                });
            if proof_leaf(&proof, region).is_some() || can_split {
                if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                    eprintln!("Explore checked shared scope: coordinates={coordinates:?}; scope={scope:?}; closed={}", proof_leaf(&proof, region).is_some());
                }
                return Some(RegionProbe {
                    proof,
                    scope: Some(scope.into_boxed_slice()),
                });
            }
        }
    }
    let proof = classifier.prove_cover_coordinates(checked, &coordinates);
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        eprintln!(
            "Explore checked cover probe: coordinates={coordinates:?}; outcome={:?}; cuts={:?}",
            proof
                .as_ref()
                .and_then(|proof| proof_leaf(proof, region))
                .map(|leaf| leaf.outcome),
            proof.as_ref().map(|proof| proof.split_hints())
        );
    }
    proof.map(|proof| RegionProbe {
        proof: Arc::new(proof),
        scope: None,
    })
}

fn proof_leaf(proof: &CheckedSourceBoxProof, region: &RankedProductBox) -> Option<CoverLeaf> {
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

/// One bounded geometry-only pass. Scope boundaries are split hints, never
/// evidence about the parent. Every accepted leaf must fit a complete fresh
/// cached theorem; any gap abandons this pass without spending a solver call.
fn cached_cover_node(
    checked: &CheckedExploreQueryView<'_>,
    inventory: &RelationalProofStrategyInventory,
    region: &RankedProductBox,
    scopes: &[CachedCoverScope],
    nodes: &mut Vec<CoverNode>,
    leaves: &mut Vec<CoverLeaf>,
) -> Option<()> {
    if nodes.len() >= MAX_COVER_NODES {
        return None;
    }
    let coordinates = region_coordinates(checked, inventory, region)?;
    for scope in scopes {
        if coordinates_contained(&coordinates, scope.coordinates()) {
            let leaf = proof_leaf(scope.proof(), region)?;
            nodes.push(CoverNode::ScopedLeaf {
                outcome: leaf.outcome,
                derivation_root: leaf.derivation_root,
                coordinate_count: region.coordinate_count(),
                scope: scope.coordinates().to_vec().into_boxed_slice(),
            });
            leaves.push(leaf);
            return Some(());
        }
    }

    let mut best = None;
    // At most 63 nodes * 64 scopes * MAX_AXES * 2 boundaries. No backtracking
    // or new proof attempts. Exact ranked intersections reject empty splits.
    for scope in scopes {
        let bounds = scope.coordinates();
        if coordinates.len() != bounds.len()
            || !coordinates.iter().zip(bounds).all(|(a, b)| match (a, b) {
                (None, None) => true,
                (Some((a, b)), Some((low, high))) => a <= high && low <= b,
                _ => false,
            })
        {
            continue;
        }
        for (ordinal, binding) in inventory.finite_binding_indices().iter().enumerate() {
            let (low, high) = bounds.get(usize::try_from(*binding).ok()?)?.as_ref()?;
            let source = inventory
                .axes()
                .iter()
                .find(|axis| axis.binding_index() == *binding)?;
            for value in [i128::from(*low), i128::from(*high) + 1] {
                let Some(pivot) = value
                    .checked_sub(i128::from(source.value_start()))
                    .and_then(|offset| u128::try_from(offset).ok())
                else {
                    continue;
                };
                let axis = u32::try_from(ordinal).ok()?;
                let Some((left, right)) = split(region, axis, pivot) else {
                    continue;
                };
                let key = (
                    left.coordinate_count().min(right.coordinate_count()),
                    axis,
                    std::cmp::Reverse(pivot),
                );
                if best.as_ref().is_none_or(|(previous, _, _)| key > *previous) {
                    best = Some((key, left, right));
                }
            }
        }
    }
    let ((_, axis, std::cmp::Reverse(pivot)), left, right) = best?;
    nodes.push(CoverNode::Split { axis, pivot });
    cached_cover_node(checked, inventory, &left, scopes, nodes, leaves)?;
    cached_cover_node(checked, inventory, &right, scopes, nodes, leaves)
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
    let proof = probe_region(classifier, checked, inventory, region);
    if let Some(leaf) = proof
        .as_ref()
        .and_then(|probe| proof_leaf(&probe.proof, region))
    {
        nodes.push(match proof.as_ref()?.scope.as_ref() {
            Some(scope) => CoverNode::ScopedLeaf {
                outcome: leaf.outcome,
                derivation_root: leaf.derivation_root,
                coordinate_count: leaf.region.coordinate_count(),
                scope: scope.clone(),
            },
            None => CoverNode::Leaf {
                outcome: leaf.outcome,
                derivation_root: leaf.derivation_root,
                coordinate_count: leaf.region.coordinate_count(),
            },
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
    let default_pivot = if high.checked_add(1)? == root.factors().get(axis)?.1 {
        high
    } else {
        low.checked_add((high - low) / 2)?.checked_add(1)?
    };
    // Preserve tiny categorical isolation. Prefer checked guard cuts that
    // isolate a declared upper endpoint, then balanced interior guard cuts.
    // Hints never bypass split validation or either child's semantic proof.
    let hinted = (high - low > 1)
        .then(|| {
            proof
                .as_ref()?
                .split_hints()
                .iter()
                .filter_map(|&(ordinal, value)| {
                    let binding = inventory.finite_binding_indices().get(ordinal)?;
                    let source = inventory
                        .axes()
                        .iter()
                        .find(|axis| axis.binding_index() == *binding)?;
                    let pivot =
                        u128::try_from(i128::from(value) - i128::from(source.value_start()))
                            .ok()?;
                    let axis = u32::try_from(ordinal).ok()?;
                    let (left, right) = split(region, axis, pivot)?;
                    let (_, high) = *enclosure.get(ordinal)?;
                    let endpoint = pivot == high
                        && high.checked_add(1) == Some(root.factors().get(ordinal)?.1);
                    Some((
                        endpoint,
                        left.coordinate_count().min(right.coordinate_count()),
                        axis,
                        pivot,
                    ))
                })
                .max_by_key(|&(endpoint, balance, axis, pivot)| {
                    (endpoint, balance, axis, std::cmp::Reverse(pivot))
                })
        })
        .flatten();
    let (axis, pivot) = hinted
        .map(|(_, _, axis, pivot)| (axis, pivot))
        .unwrap_or((u32::try_from(axis).ok()?, default_pivot));
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
            // Preserve V6's exact local derivation, even if today's producer
            // would choose a wider scope for this leaf.
            let coordinates = region_coordinates(checked, inventory, region)?;
            let proof = classifier.prove_cover_coordinates(checked, &coordinates)?;
            let leaf = proof_leaf(&proof, region)?;
            if leaf.outcome != *outcome
                || leaf.derivation_root != *derivation_root
                || leaf.region.coordinate_count() != *coordinate_count
            {
                return None;
            }
            leaves.push(leaf);
            Some(())
        }
        CoverNode::ScopedLeaf {
            outcome,
            derivation_root,
            coordinate_count,
            scope,
        } => {
            let coordinates = region_coordinates(checked, inventory, region)?;
            if !coordinates_contained(&coordinates, scope) {
                return None;
            }
            let proof = classifier.prove_cached_cover_scope(checked, scope)?;
            let leaf = proof_leaf(&proof, region)?;
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
    use crate::explore::support_cell::SupportExprKind;
    use crate::{Lexer, Parser, TypeChecker};

    #[test]
    fn scoped_cover_cached_partition_projects_fringe_and_refuses_gaps() {
        let source = r#"
# Starter(income: Int, distance: Int)
> net(s: Starter) -> Int { s.income * 3 + s.distance * 2 }
? explore cache_geometry {
    from {
        vary income in range(-10, 100001)
        let fixed = 7
        vary distance in range(0, 3)
        vary direction in range(0, 2)
        let before = Starter(income, distance)
        let context = direction
    }
    transition after = Starter(before.income + 1 - context, before.distance + context)
    where after after.income <= 100000 && after.distance <= 2
    find cliffs = violations of net(after) >= net(before)
}
"#;
        let prepare = |source: &str| {
            let mut lexer = Lexer::new(source);
            let parsed = Parser::new(lexer.tokenize(), source)
                .parse_program()
                .unwrap();
            let artifacts = TypeChecker::check_with_explore_artifacts(&parsed, None, source);
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
            (artifacts, owned)
        };
        let (artifacts, owned) = prepare(source);
        let checked = owned.view();
        let plan = RelationalSupportPlanner::from_checked(&checked)
            .unwrap()
            .plan()
            .unwrap();
        let inventory = RelationalProofStrategyInventory::from_checked(&checked, &plan).unwrap();
        let classifier = CheckedBoxClassifier::new(artifacts, owned.clone(), &plan).unwrap();
        let SupportExprKind::Product(factors) = plan.cases().cell().unwrap().expression().kind()
        else {
            panic!("expected independent product")
        };
        let page =
            |start, end| SupportExpr::product_rank_interval(factors.to_vec(), start, end).unwrap();
        let first = RankedProductBox::from_expr(&page(5, 601)).unwrap();
        let cached = |classifier: &CheckedBoxClassifier, root: &RankedProductBox| {
            CheckedRegionCover::from_cached_scopes(classifier, &checked, &inventory, root)
        };
        assert!(cached(&classifier, &first).is_none());
        assert!(classifier
            .cached_closed_cover_scopes(&checked)
            .unwrap()
            .is_empty());
        // Interposed singleton binding and a negative source origin exercise
        // binding-to-factor and value-to-ordinal conversion independently.
        let coordinates = |income, distance, direction| {
            vec![
                Some(income),
                None,
                Some(distance),
                Some(direction),
                None,
                None,
            ]
        };
        for income in [(-10, 39), (40, 99)] {
            for (distance, direction) in [((0, 1), (0, 1)), ((2, 2), (0, 0)), ((2, 2), (1, 1))] {
                let proof = classifier
                    .prove_cached_cover_scope(&checked, &coordinates(income, distance, direction))
                    .unwrap();
                assert!(proof.all_rejected() || proof.all_admitted_not_selected());
            }
        }
        // A mixed parent is cached too, but it must not enter the closed set.
        classifier
            .prove_cached_cover_scope(&checked, &coordinates((-10, 99), (2, 2), (0, 1)))
            .unwrap();
        assert_eq!(
            classifier
                .cached_closed_cover_scopes(&checked)
                .unwrap()
                .len(),
            6
        );
        for (start, end) in [(5, 601), (8, 604)] {
            let expression = page(start, end);
            let root = RankedProductBox::from_expr(&expression).unwrap();
            let cover =
                cached(&classifier, &root).expect("all cached rectangles cover this fringe");
            assert!(cover.nodes.len() <= MAX_COVER_NODES);
            assert!(cover
                .nodes
                .iter()
                .all(|node| !matches!(node, CoverNode::Leaf { .. })));
            assert_eq!(
                cover
                    .leaves
                    .iter()
                    .map(|leaf| leaf.region.coordinate_count())
                    .sum::<u128>(),
                end - start
            );
            let rejected = (start..end).filter(|rank| rank % 6 == 5).count() as u128;
            assert_eq!(cover.artifact().unwrap().rejected_count(), rejected);
            // Only these small rank fringes are enumerated, not the large root.
            for rank in start..end {
                let hits = cover
                    .leaves
                    .iter()
                    .filter(|leaf| {
                        root.restricted_rank_count(&leaf.region, rank, rank + 1)
                            .unwrap()
                            == 1
                    })
                    .collect::<Vec<_>>();
                assert_eq!(hits.len(), 1);
                assert_eq!(
                    hits[0].outcome == RelationalCertifiedRegionConclusion::Rejected,
                    rank % 6 == 5
                );
            }
            assert_eq!(
                CheckedRegionCover::prove(&classifier, &checked, &plan, &expression).unwrap(),
                cover
            );
            assert_eq!(
                CheckedRegionCover::reverify(
                    &classifier,
                    &checked,
                    &plan,
                    &expression,
                    cover.nodes()
                )
                .unwrap(),
                cover
            );
        }
        let gap_expression = page(660, 700); // incomes 100..106, beyond every cached rectangle
        let gap = RankedProductBox::from_expr(&gap_expression).unwrap();
        assert!(cached(&classifier, &gap).is_none());
        assert_eq!(
            classifier
                .cached_closed_cover_scopes(&checked)
                .unwrap()
                .len(),
            6
        );
        assert!(
            CheckedRegionCover::prove(&classifier, &checked, &plan, &gap_expression).is_some(),
            "cache miss retains ordinary proof path"
        );

        let (artifacts, _) = prepare(source);
        let limited = CheckedBoxClassifier::new(artifacts, owned.clone(), &plan).unwrap();
        for income in 0..33 {
            limited
                .prove_cached_cover_scope(&checked, &coordinates((income, income), (0, 1), (0, 0)))
                .unwrap();
        }
        let over_budget = RankedProductBox::from_expr(plan.cases().cell().unwrap().expression())
            .unwrap()
            .restrict_factor(0, 10, 43)
            .unwrap()
            .unwrap()
            .restrict_factor(1, 0, 2)
            .unwrap()
            .unwrap()
            .restrict_factor(2, 0, 1)
            .unwrap()
            .unwrap();
        assert_eq!(over_budget.coordinate_count(), 66);
        assert_eq!(
            limited.cached_closed_cover_scopes(&checked).unwrap().len(),
            33
        );
        assert!(
            cached(&limited, &over_budget).is_none(),
            "33 distinct leaves exceed the shared 63-node cap"
        );
        assert_eq!(
            limited.cached_closed_cover_scopes(&checked).unwrap().len(),
            33
        );
        assert!(CheckedRegionCover::prove(
            &limited,
            &checked,
            &plan,
            &over_budget.expression().unwrap()
        )
        .is_some());

        let selected_source = source.replace(
            "violations of net(after) >= net(before)",
            "matches of before.income == 100",
        );
        let (artifacts, other) = prepare(&selected_source);
        let other_checked = other.view();
        assert!(
            classifier
                .cached_closed_cover_scopes(&other_checked)
                .is_none(),
            "foreign checked snapshot cannot reuse proofs"
        );
        let other_plan = RelationalSupportPlanner::from_checked(&other_checked)
            .unwrap()
            .plan()
            .unwrap();
        let other_inventory =
            RelationalProofStrategyInventory::from_checked(&other_checked, &other_plan).unwrap();
        let selected = CheckedBoxClassifier::new(artifacts, other.clone(), &other_plan).unwrap();
        for income in [99, 100, 101] {
            selected
                .prove_cached_cover_scope(
                    &other_checked,
                    &coordinates((income, income), (0, 1), (0, 0)),
                )
                .unwrap();
        }
        assert_eq!(
            selected
                .cached_closed_cover_scopes(&other_checked)
                .unwrap()
                .len(),
            2,
            "selected scope is not a no-cliff proof"
        );
        let hole = RankedProductBox::from_expr(other_plan.cases().cell().unwrap().expression())
            .unwrap()
            .restrict_factor(0, 109, 112)
            .unwrap()
            .unwrap()
            .restrict_factor(1, 0, 2)
            .unwrap()
            .unwrap()
            .restrict_factor(2, 0, 1)
            .unwrap()
            .unwrap();
        assert!(CheckedRegionCover::from_cached_scopes(
            &selected,
            &other_checked,
            &other_inventory,
            &hole
        )
        .is_none());
        assert!(
            CheckedRegionCover::prove(
                &selected,
                &other_checked,
                &other_plan,
                &hole.expression().unwrap()
            )
            .is_none(),
            "real selected members remain residual"
        );
    }

    #[test]
    #[ignore = "explicit installed-solver cover authority edge"]
    fn checked_integer_cover_replays_unsat_and_rejects_forged_roots() {
        for predicate in [
            "violations of after * 7 / 19 >= before * 7 / 19",
            "matches of after * 7 / 19 < before * 7 / 19",
        ] {
            let source = format!(
                r#"
? explore rounding {{
    from {{
        vary before in range(-100, 101)
        vary context in range(0, 2)
    }}
    transition after = before + 1
    where after after <= 100
    find losses = {predicate}
}}
"#
            );
            let mut lexer = Lexer::new(&source);
            let parsed = Parser::new(lexer.tokenize(), &source)
                .parse_program()
                .unwrap();
            let artifacts = TypeChecker::check_with_explore_artifacts(&parsed, None, &source);
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
            let coordinates = [Some((-100, 99)), Some((0, 1))];
            let old = classifier
                .prove_coordinates(&checked, &coordinates)
                .unwrap();
            assert!(
                !old.all_admitted_not_selected(),
                "fixture must need stronger rounding dependencies"
            );
            assert!(classifier
                .prove_cover_coordinates(&checked, &coordinates)
                .unwrap()
                .all_admitted_not_selected());
            let expression = plan.cases().cell().unwrap().expression();
            let cover =
                CheckedRegionCover::prove(&classifier, &checked, &plan, expression).unwrap();
            assert_eq!(cover.artifact().unwrap().rejected_count(), 2);
            assert_eq!(
                cover
                    .leaves()
                    .iter()
                    .filter(|leaf| leaf.outcome
                        == RelationalCertifiedRegionConclusion::AdmittedNotSelected)
                    .map(|leaf| leaf.region.coordinate_count())
                    .sum::<u128>(),
                400,
            );
            assert_eq!(
                CheckedRegionCover::reverify(
                    &classifier,
                    &checked,
                    &plan,
                    expression,
                    cover.nodes()
                )
                .unwrap(),
                cover
            );
            let mut forged = cover.nodes().to_vec();
            let leaf_index = forged
                .iter()
                .position(|node| {
                    matches!(
                        node,
                        CoverNode::Leaf {
                            outcome: RelationalCertifiedRegionConclusion::AdmittedNotSelected,
                            ..
                        }
                    )
                })
                .unwrap();
            let CoverNode::Leaf {
                derivation_root, ..
            } = &mut forged[leaf_index]
            else {
                panic!("expected proved leaf")
            };
            *derivation_root = old.derivation_root();
            assert!(CheckedRegionCover::reverify(
                &classifier,
                &checked,
                &plan,
                expression,
                &forged
            )
            .is_none());
        }
    }

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
