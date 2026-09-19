//! Deterministic root scheduling for the relational support-proof frontier.
//!
//! This driver deliberately does not run an open-ended proof strategy. Once a
//! support plan is durably registered, it publishes the case-root readiness
//! token and one resolver node for each still-open root obligation. It may
//! first certify a recognized exact source image, then discharge
//! producer-verified case-image injectivity, optional exact case cardinality,
//! literal uniform-admission shapes, or the canonical bounded fallback
//! partition when no uniform admission proof applies. Event ordering and work
//! identity are canonical, so a durable owner may install any proper prefix
//! and resume by asking for another step.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::relation::AdmissionDecision;
use super::relational_bounded_chunk_partition::{
    plan_relational_bounded_case_chunks, RelationalCaseChunkPartitionArtifact,
    RelationalCaseChunkPartitionArtifactId, RelationalCaseChunkPartitionError,
    RelationalCaseChunkPlanningOutcome,
};
use super::relational_frontier::{
    RelationalWorkFrontier, WorkCompletionRef, WorkFrontierError, WorkNodeId, WorkNodeSpec,
};
use super::relational_journal::{
    RelationalJournalError, RelationalJournalEvent, RelationalJournalHead, RelationalSchedulerView,
};
use super::relational_source_image_exactness::{
    prove_relational_source_image_exactness, CertifiedSourcePopulationBinding,
    CertifiedSourcePopulationRoot, RelationalSourceImageExactnessProofArtifact,
    RelationalSourceImageExactnessProofError,
};
use super::relational_support_planner::{
    prove_relational_case_image_injectivity, RelationalCaseImageInjectivityProofArtifact,
    RelationalCaseImageInjectivityProofError, RelationalObligationActivation,
    RelationalRootObligationPlan, RelationalStagedObligationDescriptor, RelationalSupportPlan,
    RelationalSupportPlanRoot,
};
use super::relational_uniform_admission_proof::{
    prove_relational_uniform_admission, RelationalUniformAdmissionProofArtifact,
    RelationalUniformAdmissionProofError,
};
use super::support_cell::{
    AdmissionClassificationClaim, SupportCellError, SupportCellEvidenceId, SupportCellId,
    SupportCellObligation, SupportPartitionId, SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceError, SupportEvidenceKind, SupportObligationRecord,
    SupportObligationRefinement, SupportObligationRefinementId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct RootSupportFrontier {
    cell_id: SupportCellId,
    obligation_ids: Box<[SupportProofObligationId]>,
    injectivity_obligation_id: Option<SupportProofObligationId>,
    exact_cardinality_obligation_id: Option<SupportProofObligationId>,
    admission_obligation_id: Option<SupportProofObligationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaseImageProofProposal {
    artifact: RelationalCaseImageInjectivityProofArtifact,
    injectivity: CaseImageEvidenceProposal,
    exact_cardinality: Option<CaseImageEvidenceProposal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceImageExactnessProofProposal {
    artifact: RelationalSourceImageExactnessProofArtifact,
    binding: CertifiedSourcePopulationBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaseImageEvidenceProposal {
    obligation_id: SupportProofObligationId,
    evidence_id: SupportCellEvidenceId,
    kind: SupportEvidenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UniformAdmissionProofProposal {
    artifact: RelationalUniformAdmissionProofArtifact,
    obligation_id: SupportProofObligationId,
    evidence_id: SupportCellEvidenceId,
    decision: AdmissionDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaseChunkPartitionProposal {
    artifact: RelationalCaseChunkPartitionArtifact,
    partition_id: SupportPartitionId,
    admission_obligation_id: SupportProofObligationId,
    refinement_id: SupportObligationRefinementId,
    child_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportStepQuantum {
    AcceptSourceImageExactnessProof {
        source_cell_id: SupportCellId,
        injectivity_evidence_id: SupportCellEvidenceId,
        cardinality_evidence_id: SupportCellEvidenceId,
        population_root: CertifiedSourcePopulationRoot,
        exact_cardinality: u128,
    },
    SeedSupportFrontier {
        root_cell_id: SupportCellId,
        obligation_count: usize,
    },
    AcceptCaseImageProof {
        root_cell_id: SupportCellId,
        injectivity_obligation_id: SupportProofObligationId,
        injectivity_evidence_id: SupportCellEvidenceId,
        exact_cardinality: Option<(SupportProofObligationId, SupportCellEvidenceId)>,
    },
    AcceptUniformAdmission {
        root_cell_id: SupportCellId,
        obligation_id: SupportProofObligationId,
        evidence_id: SupportCellEvidenceId,
        decision: AdmissionDecision,
    },
    AcceptCaseChunkPartition {
        root_cell_id: SupportCellId,
        artifact_id: RelationalCaseChunkPartitionArtifactId,
        partition_id: SupportPartitionId,
        admission_obligation_id: SupportProofObligationId,
        refinement_id: SupportObligationRefinementId,
        child_count: usize,
    },
}

/// One head-bound, unapplied support-frontier batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSupportStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalSupportStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalSupportStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalSupportStepQuantum {
        self.quantum
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportStepOutcome {
    Emitted(RelationalSupportStepBatch),
    AwaitingSupportPlanRegistration,
    CaughtUp,
}

/// Support-root scheduler derived only from the immutable checked plan.
///
/// Root descriptors are normalized by obligation identity during
/// construction. Admission-conditioned FIND descriptors are not initial root
/// work; accepting a uniform admitted classification activates them in the
/// support journal for a later FIND producer.
pub(crate) struct RelationalSupportStepDriver {
    plan_root: RelationalSupportPlanRoot,
    root: Option<RootSupportFrontier>,
    source_image_exactness_proof: Option<SourceImageExactnessProofProposal>,
    case_image_proof: Option<CaseImageProofProposal>,
    uniform_admission_proof: Option<UniformAdmissionProofProposal>,
    case_chunk_partition: Option<CaseChunkPartitionProposal>,
}

impl RelationalSupportStepDriver {
    pub(crate) fn from_plan(
        plan: &RelationalSupportPlan,
    ) -> Result<Self, RelationalSupportStepDriverError> {
        if !plan.validate_root() {
            return Err(RelationalSupportStepDriverError::InvalidPlanRoot);
        }
        // Uniform admission remains a deliberately unary accelerator. The
        // source/case structural proofs and bounded partition below are shared
        // by the complete question set and must not invent a primary question.
        let singleton_question_id = match plan.question_ids() {
            [question_id] => Some(*question_id),
            _ => None,
        };

        let root = match plan.root_obligations() {
            RelationalRootObligationPlan::ResolvedExactEmpty { .. } => None,
            RelationalRootObligationPlan::CellBacked {
                root_cell_id,
                descriptors,
            } => {
                if !plan.cell_catalog().contains(*root_cell_id) {
                    return Err(RelationalSupportStepDriverError::RootCellMissing(
                        *root_cell_id,
                    ));
                }
                let mut obligation_ids = BTreeSet::new();
                let mut injectivity_obligation_id = None;
                let mut exact_cardinality_obligation_id = None;
                let mut admission_obligation_id = None;
                for descriptor in descriptors {
                    let RelationalStagedObligationDescriptor::Root {
                        activation,
                        obligation,
                    } = descriptor
                    else {
                        continue;
                    };
                    if *activation != RelationalObligationActivation::RootCasePopulation {
                        return Err(RelationalSupportStepDriverError::InvalidRootActivation(
                            obligation.id(),
                        ));
                    }
                    if obligation.cell_id() != *root_cell_id {
                        return Err(
                            RelationalSupportStepDriverError::RootObligationCellMismatch {
                                obligation_id: obligation.id(),
                                expected: *root_cell_id,
                                actual: obligation.cell_id(),
                            },
                        );
                    }
                    obligation_ids.insert(obligation.id());
                    if matches!(obligation, SupportObligationRecord::Injectivity(_)) {
                        match injectivity_obligation_id {
                            Some(existing) if existing != obligation.id() => {
                                return Err(
                                    RelationalSupportStepDriverError::MultipleRootInjectivityObligations,
                                );
                            }
                            Some(_) => {}
                            None => injectivity_obligation_id = Some(obligation.id()),
                        }
                    }
                    if matches!(obligation, SupportObligationRecord::Cardinality(_)) {
                        match exact_cardinality_obligation_id {
                            Some(existing) if existing != obligation.id() => {
                                return Err(
                                    RelationalSupportStepDriverError::MultipleRootCardinalityObligations,
                                );
                            }
                            Some(_) => {}
                            None => exact_cardinality_obligation_id = Some(obligation.id()),
                        }
                    }
                    if matches!(obligation, SupportObligationRecord::Admission(_)) {
                        match admission_obligation_id {
                            Some(existing) if existing != obligation.id() => {
                                return Err(
                                    RelationalSupportStepDriverError::MultipleRootAdmissionObligations,
                                );
                            }
                            Some(_) => {}
                            None => admission_obligation_id = Some(obligation.id()),
                        }
                    }
                }
                Some(RootSupportFrontier {
                    cell_id: *root_cell_id,
                    obligation_ids: obligation_ids
                        .into_iter()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    injectivity_obligation_id,
                    exact_cardinality_obligation_id,
                    admission_obligation_id,
                })
            }
        };

        let source_image_exactness_proof = match prove_relational_source_image_exactness(plan) {
            Ok(proof) => Some(SourceImageExactnessProofProposal {
                artifact: proof.proof().artifact().clone(),
                binding: proof.population_binding(),
            }),
            Err(RelationalSourceImageExactnessProofError::UnsupportedPlanShape(_)) => None,
            Err(error) => return Err(error.into()),
        };

        let verified_case_image_proof = if root.is_some() {
            match prove_relational_case_image_injectivity(plan) {
                Ok(proof) => Some(proof),
                Err(RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(_)) => None,
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        let case_image_proof = verified_case_image_proof
            .as_ref()
            .map(|proof| {
                let root = root.as_ref().ok_or(
                    RelationalSupportStepDriverError::QualifyingInjectivityProofObligationMismatch,
                )?;
                let injectivity = proof.injectivity();
                let injectivity_obligation_id = injectivity.obligation().id();
                if root.injectivity_obligation_id != Some(injectivity_obligation_id)
                    || injectivity.obligation().cell_id() != root.cell_id
                {
                    return Err(
                        RelationalSupportStepDriverError::QualifyingInjectivityProofObligationMismatch,
                    );
                }
                let exact_cardinality = proof
                    .exact_cardinality()
                    .map(|evidence| {
                        let obligation_id = evidence.obligation().id();
                        if root.exact_cardinality_obligation_id != Some(obligation_id)
                            || evidence.obligation().cell_id() != root.cell_id
                        {
                            return Err(
                                RelationalSupportStepDriverError::QualifyingCardinalityProofObligationMismatch,
                            );
                        }
                        Ok(CaseImageEvidenceProposal {
                            obligation_id,
                            evidence_id: evidence.id(),
                            kind: SupportEvidenceKind::Cardinality,
                        })
                    })
                    .transpose()?;
                Ok(CaseImageProofProposal {
                    artifact: proof.proof().artifact().clone(),
                    injectivity: CaseImageEvidenceProposal {
                        obligation_id: injectivity_obligation_id,
                        evidence_id: injectivity.id(),
                        kind: SupportEvidenceKind::Injectivity,
                    },
                    exact_cardinality,
                })
            })
            .transpose()?;

        let uniform_admission_proof_candidate = match (singleton_question_id, root.as_ref()) {
            (Some(_), Some(root)) => match prove_relational_uniform_admission(plan) {
                Ok(proof) => {
                    let evidence = proof.evidence();
                    let obligation_id = evidence.obligation().id();
                    if root.admission_obligation_id != Some(obligation_id)
                        || evidence.obligation().cell_id() != root.cell_id
                    {
                        let error = RelationalSupportStepDriverError::
                            QualifyingAdmissionProofObligationMismatch;
                        return Err(error);
                    }
                    Some(UniformAdmissionProofProposal {
                        artifact: proof.proof().artifact().clone(),
                        obligation_id,
                        evidence_id: evidence.id(),
                        decision: *evidence.conclusion(),
                    })
                }
                Err(RelationalUniformAdmissionProofError::UnsupportedPlanShape(_)) => None,
                Err(error) => return Err(error.into()),
            },
            _ => None,
        };

        // A proper canonical partition owns the ordered classified stream.
        // Refine the still-open root admission into its child obligations even
        // when admission is globally uniform: regional FIND proof and concrete
        // fallback must agree on those exact mapped children. The stronger
        // root shortcut remains available only when the root is already
        // bounded or cannot be partitioned.
        let case_chunk_partition = match verified_case_image_proof.as_ref() {
            Some(proof) => match plan_relational_bounded_case_chunks(plan, proof)? {
                RelationalCaseChunkPlanningOutcome::Partitioned(partition) => {
                    let root = root.as_ref().ok_or(
                        RelationalSupportStepDriverError::QualifyingChunkAdmissionObligationMissing,
                    )?;
                    let admission_obligation_id = root.admission_obligation_id.ok_or(
                        RelationalSupportStepDriverError::QualifyingChunkAdmissionObligationMissing,
                    )?;
                    let parent_admission = plan
                        .obligations()
                        .iter()
                        .find_map(|descriptor| match descriptor {
                            RelationalStagedObligationDescriptor::Root {
                                activation: RelationalObligationActivation::RootCasePopulation,
                                obligation: SupportObligationRecord::Admission(obligation),
                            } if obligation.id() == admission_obligation_id => {
                                Some(obligation.clone())
                            }
                            _ => None,
                        })
                        .ok_or(
                            RelationalSupportStepDriverError::QualifyingChunkAdmissionObligationMissing,
                        )?;
                    let parent_record =
                        SupportObligationRecord::Admission(parent_admission.clone());
                    let child_admissions = partition
                        .chunks()
                        .iter()
                        .map(|chunk| {
                            SupportCellObligation::new(
                                chunk.cell(),
                                AdmissionClassificationClaim::new(plan.admission_id()),
                            )
                            .map(SupportObligationRecord::Admission)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let refinement = SupportObligationRefinement::new(
                        &parent_record,
                        partition.certificate(),
                        child_admissions.iter(),
                    )?;
                    Some(CaseChunkPartitionProposal {
                        artifact: partition.artifact().clone(),
                        partition_id: partition.certificate().id(),
                        admission_obligation_id,
                        refinement_id: refinement.id(),
                        child_count: partition.chunks().len(),
                    })
                }
                RelationalCaseChunkPlanningOutcome::AlreadyBounded { .. }
                | RelationalCaseChunkPlanningOutcome::Unsupported(_) => None,
            },
            None => None,
        };
        let uniform_admission_proof = if case_chunk_partition.is_some() {
            None
        } else {
            uniform_admission_proof_candidate
        };

        Ok(Self {
            plan_root: plan.root(),
            root,
            source_image_exactness_proof,
            case_image_proof,
            uniform_admission_proof,
            case_chunk_partition,
        })
    }

    /// Whether this checked plan has a canonical bounded case partition that
    /// a classified-sweep scheduler may consume after support catches up.
    pub(crate) const fn has_case_chunk_partition(&self) -> bool {
        self.case_chunk_partition.is_some()
    }

    /// Plan exactly one root-frontier quantum against an authenticated journal
    /// prefix. A recognized source-image proof is accepted before any root or
    /// population work. Seeding then publishes readiness first and resolver
    /// nodes in canonical obligation-ID order; the recognized case producer
    /// proof publishes evidence before its work completion.
    pub(crate) fn step(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<RelationalSupportStepOutcome, RelationalSupportStepDriverError> {
        match view.support_plan_root() {
            None => return Ok(RelationalSupportStepOutcome::AwaitingSupportPlanRegistration),
            Some(actual) if actual != self.plan_root => {
                return Err(RelationalSupportStepDriverError::SupportPlanRootMismatch {
                    expected: self.plan_root,
                    actual,
                });
            }
            Some(_) => {}
        }

        if let Some(batch) = self.source_image_exactness_proof_batch(view)? {
            return Ok(RelationalSupportStepOutcome::Emitted(batch));
        }

        let Some(root) = &self.root else {
            return Ok(RelationalSupportStepOutcome::CaughtUp);
        };
        let mut open_obligations = Vec::new();
        for obligation_id in root.obligation_ids.iter().copied() {
            match view.support_root_obligation_is_open(obligation_id) {
                Some(true) => open_obligations.push(obligation_id),
                Some(false) => {}
                None => {
                    return Err(
                        RelationalSupportStepDriverError::RegisteredRootObligationMissing(
                            obligation_id,
                        ),
                    );
                }
            }
        }
        let readiness_spec = WorkNodeSpec::SupportCellReady {
            cell_id: root.cell_id,
        };
        let readiness_id = RelationalWorkFrontier::derive_node_id(&readiness_spec, [])?;
        let readiness_exists = view.work_node(readiness_id).is_some();
        let mut missing = Vec::new();
        for obligation_id in open_obligations.iter().copied() {
            let spec = WorkNodeSpec::ResolveSupportObligation {
                cell_id: root.cell_id,
                obligation_id,
            };
            let node_id = RelationalWorkFrontier::derive_node_id(&spec, [readiness_id])?;
            if view.work_node(node_id).is_some() {
                if !readiness_exists {
                    return Err(RelationalSupportStepDriverError::ResolverWithoutReadiness(
                        obligation_id,
                    ));
                }
            } else {
                missing.push((obligation_id, spec));
            }
        }
        if !missing.is_empty() {
            let readiness_event_count = if readiness_exists { 0 } else { 1 };
            let mut events = Vec::with_capacity(missing.len() + readiness_event_count);
            if !readiness_exists {
                events.push(RelationalJournalEvent::work_readiness_materialized(
                    readiness_spec,
                )?);
            }
            let obligation_count = missing.len();
            for (_, spec) in missing {
                events.push(RelationalJournalEvent::work_node_inserted(
                    spec,
                    [readiness_id],
                )?);
            }

            return Ok(RelationalSupportStepOutcome::Emitted(
                RelationalSupportStepBatch {
                    expected_sequence: view.sequence(),
                    expected_head: view.head(),
                    quantum: RelationalSupportStepQuantum::SeedSupportFrontier {
                        root_cell_id: root.cell_id,
                        obligation_count,
                    },
                    events: events.into_boxed_slice(),
                },
            ));
        }

        if let Some(batch) =
            self.case_image_proof_batch(view, root, readiness_id, readiness_exists)?
        {
            return Ok(RelationalSupportStepOutcome::Emitted(batch));
        }

        if let Some(batch) =
            self.uniform_admission_batch(view, root, readiness_id, readiness_exists)?
        {
            return Ok(RelationalSupportStepOutcome::Emitted(batch));
        }

        if let Some(batch) =
            self.case_chunk_partition_batch(view, root, readiness_id, readiness_exists)?
        {
            return Ok(RelationalSupportStepOutcome::Emitted(batch));
        }

        Ok(RelationalSupportStepOutcome::CaughtUp)
    }

    fn source_image_exactness_proof_batch(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<Option<RelationalSupportStepBatch>, RelationalSupportStepDriverError> {
        let Some(proposal) = &self.source_image_exactness_proof else {
            return Ok(None);
        };
        match view.certified_source_population()? {
            Some(durable) if durable == proposal.binding => return Ok(None),
            Some(durable) => {
                return Err(
                    RelationalSupportStepDriverError::DurableSourceImageBindingMismatch {
                        expected: proposal.binding.population_root(),
                        actual: durable.population_root(),
                    },
                );
            }
            None => {}
        }
        if view.source_traversal_is_started()
            // Installing the canonical partition already chooses the
            // classified branch.  Do not wait for chunk zero: a late source
            // proof would otherwise make accepted event order depend on
            // whether the first expensive classification had finished.
            || view
                .classified_sweep_progress()?
                .is_some()
            || view.support_catalog_is_sealed()
        {
            return Err(
                RelationalSupportStepDriverError::SourceImageProofMissingBeforePopulationWork,
            );
        }

        Ok(Some(RelationalSupportStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum: RelationalSupportStepQuantum::AcceptSourceImageExactnessProof {
                source_cell_id: proposal.binding.source_cell_id(),
                injectivity_evidence_id: proposal.binding.injectivity_evidence_id(),
                cardinality_evidence_id: proposal.binding.cardinality_evidence_id(),
                population_root: proposal.binding.population_root(),
                exact_cardinality: proposal.binding.exact_cardinality(),
            },
            events: vec![
                RelationalJournalEvent::relational_source_image_exactness_proof_accepted(
                    proposal.artifact.clone(),
                ),
            ]
            .into_boxed_slice(),
        }))
    }

    fn case_image_proof_batch(
        &self,
        view: RelationalSchedulerView<'_>,
        root: &RootSupportFrontier,
        readiness_id: WorkNodeId,
        readiness_exists: bool,
    ) -> Result<Option<RelationalSupportStepBatch>, RelationalSupportStepDriverError> {
        let Some(proposal) = &self.case_image_proof else {
            return Ok(None);
        };
        let mut evidence_proposals = Vec::with_capacity(2);
        evidence_proposals.push(proposal.injectivity);
        if let Some(exact_cardinality) = proposal.exact_cardinality {
            evidence_proposals.push(exact_cardinality);
        }
        evidence_proposals.sort_by_key(|evidence| evidence.obligation_id);

        let mut proof_event_needed = false;
        let mut pending_completions = Vec::with_capacity(evidence_proposals.len());
        for evidence_proposal in evidence_proposals {
            let spec = WorkNodeSpec::ResolveSupportObligation {
                cell_id: root.cell_id,
                obligation_id: evidence_proposal.obligation_id,
            };
            let node_id = RelationalWorkFrontier::derive_node_id(&spec, [readiness_id])?;
            let Some(node) = view.work_node(node_id) else {
                continue;
            };
            if !readiness_exists {
                return Err(RelationalSupportStepDriverError::ResolverWithoutReadiness(
                    evidence_proposal.obligation_id,
                ));
            }
            if node.progress.is_complete() {
                continue;
            }

            match view.support_evidence_record(evidence_proposal.evidence_id) {
                None if view.support_root_obligation_is_open(evidence_proposal.obligation_id)
                    == Some(true) =>
                {
                    proof_event_needed = true;
                }
                None => return Ok(None),
                Some(evidence)
                    if evidence.id() == evidence_proposal.evidence_id
                        && evidence.kind() == evidence_proposal.kind
                        && evidence.cell_id() == root.cell_id
                        && evidence.obligation_id() == evidence_proposal.obligation_id => {}
                Some(_) => {
                    return Err(
                        RelationalSupportStepDriverError::DurableProofEvidenceMismatch(
                            evidence_proposal.evidence_id,
                        ),
                    );
                }
            }
            pending_completions.push((evidence_proposal, node_id));
        }
        if pending_completions.is_empty() {
            return Ok(None);
        }

        let proof_event_count = if proof_event_needed { 1 } else { 0 };
        let mut events = Vec::with_capacity(pending_completions.len() + proof_event_count);
        if proof_event_needed {
            events.push(
                RelationalJournalEvent::relational_case_image_injectivity_proof_accepted(
                    proposal.artifact.clone(),
                ),
            );
        }
        for (evidence_proposal, node_id) in pending_completions {
            events.push(RelationalJournalEvent::work_node_completed(
                node_id,
                WorkCompletionRef::DirectSupportEvidence {
                    cell_id: root.cell_id,
                    obligation_id: evidence_proposal.obligation_id,
                    evidence_id: evidence_proposal.evidence_id,
                },
            ));
        }

        Ok(Some(RelationalSupportStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum: RelationalSupportStepQuantum::AcceptCaseImageProof {
                root_cell_id: root.cell_id,
                injectivity_obligation_id: proposal.injectivity.obligation_id,
                injectivity_evidence_id: proposal.injectivity.evidence_id,
                exact_cardinality: proposal
                    .exact_cardinality
                    .map(|evidence| (evidence.obligation_id, evidence.evidence_id)),
            },
            events: events.into_boxed_slice(),
        }))
    }

    fn uniform_admission_batch(
        &self,
        view: RelationalSchedulerView<'_>,
        root: &RootSupportFrontier,
        readiness_id: WorkNodeId,
        readiness_exists: bool,
    ) -> Result<Option<RelationalSupportStepBatch>, RelationalSupportStepDriverError> {
        let Some(proposal) = &self.uniform_admission_proof else {
            return Ok(None);
        };
        let spec = WorkNodeSpec::ResolveSupportObligation {
            cell_id: root.cell_id,
            obligation_id: proposal.obligation_id,
        };
        let node_id = RelationalWorkFrontier::derive_node_id(&spec, [readiness_id])?;
        let Some(node) = view.work_node(node_id) else {
            return Ok(None);
        };
        if !readiness_exists {
            return Err(RelationalSupportStepDriverError::ResolverWithoutReadiness(
                proposal.obligation_id,
            ));
        }
        if node.progress.is_complete() {
            return Ok(None);
        }

        let mut events = Vec::with_capacity(2);
        match view.support_evidence_record(proposal.evidence_id) {
            None if view.support_root_obligation_is_open(proposal.obligation_id) == Some(true) => {
                events.push(
                    RelationalJournalEvent::relational_uniform_admission_proof_accepted(
                        proposal.artifact.clone(),
                    ),
                );
            }
            None => return Ok(None),
            Some(evidence)
                if evidence.id() == proposal.evidence_id
                    && evidence.kind() == SupportEvidenceKind::Admission
                    && evidence.cell_id() == root.cell_id
                    && evidence.obligation_id() == proposal.obligation_id => {}
            Some(_) => {
                return Err(
                    RelationalSupportStepDriverError::DurableProofEvidenceMismatch(
                        proposal.evidence_id,
                    ),
                );
            }
        }
        events.push(RelationalJournalEvent::work_node_completed(
            node_id,
            WorkCompletionRef::DirectSupportEvidence {
                cell_id: root.cell_id,
                obligation_id: proposal.obligation_id,
                evidence_id: proposal.evidence_id,
            },
        ));

        Ok(Some(RelationalSupportStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum: RelationalSupportStepQuantum::AcceptUniformAdmission {
                root_cell_id: root.cell_id,
                obligation_id: proposal.obligation_id,
                evidence_id: proposal.evidence_id,
                decision: proposal.decision,
            },
            events: events.into_boxed_slice(),
        }))
    }

    fn case_chunk_partition_batch(
        &self,
        view: RelationalSchedulerView<'_>,
        root: &RootSupportFrontier,
        readiness_id: WorkNodeId,
        readiness_exists: bool,
    ) -> Result<Option<RelationalSupportStepBatch>, RelationalSupportStepDriverError> {
        let Some(proposal) = &self.case_chunk_partition else {
            return Ok(None);
        };
        if root.admission_obligation_id != Some(proposal.admission_obligation_id) {
            return Err(
                RelationalSupportStepDriverError::QualifyingChunkAdmissionObligationMissing,
            );
        }
        let spec = WorkNodeSpec::ResolveSupportObligation {
            cell_id: root.cell_id,
            obligation_id: proposal.admission_obligation_id,
        };
        let node_id = RelationalWorkFrontier::derive_node_id(&spec, [readiness_id])?;
        let durable_refinement = view
            .support_refinement_for_parent(proposal.admission_obligation_id)
            .map(|refinement| {
                if refinement.id() != proposal.refinement_id
                    || refinement.parent_obligation_id() != proposal.admission_obligation_id
                    || refinement.partition_id() != proposal.partition_id
                    || refinement.child_obligation_ids().len() != proposal.child_count
                {
                    return Err(
                        RelationalSupportStepDriverError::DurableChunkRefinementMismatch(
                            proposal.refinement_id,
                        ),
                    );
                }
                Ok(refinement.id())
            })
            .transpose()?;
        let Some(node) = view.work_node(node_id) else {
            // A matching durable refinement is the semantic result. Its
            // completed resolver and readiness node may already have been
            // compacted from the operational frontier, so their absence must
            // neither invalidate nor resurrect accepted work.
            return Ok(None);
        };
        if !readiness_exists {
            return Err(RelationalSupportStepDriverError::ResolverWithoutReadiness(
                proposal.admission_obligation_id,
            ));
        }
        if node.progress.is_complete() {
            if durable_refinement == Some(proposal.refinement_id) {
                return Ok(None);
            }
            return Err(
                RelationalSupportStepDriverError::ChunkResolverCompletionMismatch(
                    proposal.admission_obligation_id,
                ),
            );
        }

        let partition_event_needed = match durable_refinement {
            Some(_) => false,
            None if view.support_root_obligation_is_open(proposal.admission_obligation_id)
                == Some(true) =>
            {
                true
            }
            None => {
                return Err(
                    RelationalSupportStepDriverError::ChunkAdmissionStateMismatch(
                        proposal.admission_obligation_id,
                    ),
                );
            }
        };
        let mut events = Vec::with_capacity(if partition_event_needed { 2 } else { 1 });
        if partition_event_needed {
            events.push(
                RelationalJournalEvent::relational_case_chunk_partition_accepted(
                    proposal.artifact.clone(),
                ),
            );
        }
        events.push(RelationalJournalEvent::work_node_completed(
            node_id,
            WorkCompletionRef::SupportObligationRefined {
                cell_id: root.cell_id,
                obligation_id: proposal.admission_obligation_id,
                refinement_id: proposal.refinement_id,
            },
        ));

        Ok(Some(RelationalSupportStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum: RelationalSupportStepQuantum::AcceptCaseChunkPartition {
                root_cell_id: root.cell_id,
                artifact_id: proposal.artifact.id(),
                partition_id: proposal.partition_id,
                admission_obligation_id: proposal.admission_obligation_id,
                refinement_id: proposal.refinement_id,
                child_count: proposal.child_count,
            },
            events: events.into_boxed_slice(),
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportStepDriverError {
    InvalidPlanRoot,
    RootCellMissing(SupportCellId),
    InvalidRootActivation(SupportProofObligationId),
    RootObligationCellMismatch {
        obligation_id: SupportProofObligationId,
        expected: SupportCellId,
        actual: SupportCellId,
    },
    MultipleRootInjectivityObligations,
    MultipleRootCardinalityObligations,
    MultipleRootAdmissionObligations,
    QualifyingInjectivityProofObligationMismatch,
    QualifyingCardinalityProofObligationMismatch,
    QualifyingAdmissionProofObligationMismatch,
    QualifyingChunkAdmissionObligationMissing,
    SupportPlanRootMismatch {
        expected: RelationalSupportPlanRoot,
        actual: RelationalSupportPlanRoot,
    },
    RegisteredRootObligationMissing(SupportProofObligationId),
    ResolverWithoutReadiness(SupportProofObligationId),
    DurableSourceImageBindingMismatch {
        expected: CertifiedSourcePopulationRoot,
        actual: CertifiedSourcePopulationRoot,
    },
    SourceImageProofMissingBeforePopulationWork,
    DurableProofEvidenceMismatch(SupportCellEvidenceId),
    DurableChunkRefinementMismatch(SupportObligationRefinementId),
    ChunkResolverCompletionMismatch(SupportProofObligationId),
    ChunkAdmissionStateMismatch(SupportProofObligationId),
    SourceImageProof(RelationalSourceImageExactnessProofError),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    CaseChunkPartition(RelationalCaseChunkPartitionError),
    UniformAdmissionProof(RelationalUniformAdmissionProofError),
    SupportCell(SupportCellError),
    SupportEvidence(SupportEvidenceError),
    Journal(RelationalJournalError),
    Work(WorkFrontierError),
}

impl fmt::Display for RelationalSupportStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlanRoot => formatter.write_str("support plan root is not canonical"),
            Self::RootCellMissing(_) => {
                formatter.write_str("support plan case root is absent from its cell catalog")
            }
            Self::InvalidRootActivation(_) => {
                formatter.write_str("support plan root obligation has a deferred activation")
            }
            Self::RootObligationCellMismatch { .. } => {
                formatter.write_str("support plan root obligation names a different support cell")
            }
            Self::MultipleRootInjectivityObligations => {
                formatter.write_str("support plan contains multiple root injectivity obligations")
            }
            Self::MultipleRootCardinalityObligations => {
                formatter.write_str("support plan contains multiple root cardinality obligations")
            }
            Self::MultipleRootAdmissionObligations => {
                formatter.write_str("support plan contains multiple root admission obligations")
            }
            Self::QualifyingInjectivityProofObligationMismatch => formatter.write_str(
                "case-image injectivity proof does not name the planned root obligation",
            ),
            Self::QualifyingCardinalityProofObligationMismatch => formatter.write_str(
                "case-image exact-cardinality proof does not name the planned root obligation",
            ),
            Self::QualifyingAdmissionProofObligationMismatch => formatter
                .write_str("uniform-admission proof does not name the planned root obligation"),
            Self::QualifyingChunkAdmissionObligationMissing => formatter.write_str(
                "bounded case partition has no matching planned root admission obligation",
            ),
            Self::SupportPlanRootMismatch { .. } => {
                formatter.write_str("journal registered a different support plan root")
            }
            Self::RegisteredRootObligationMissing(_) => formatter
                .write_str("registered support plan is missing one declared root obligation"),
            Self::ResolverWithoutReadiness(_) => formatter
                .write_str("support resolver exists without its required case-root readiness"),
            Self::DurableSourceImageBindingMismatch { .. } => formatter.write_str(
                "durable certified source population does not match the checked support plan",
            ),
            Self::SourceImageProofMissingBeforePopulationWork => formatter.write_str(
                "source-image exactness proof is missing after population work began or support evidence sealed",
            ),
            Self::DurableProofEvidenceMismatch(_) => formatter
                .write_str("durable support-proof evidence does not match its resolver proposal"),
            Self::DurableChunkRefinementMismatch(_) => formatter.write_str(
                "durable support refinement does not match the bounded partition proposal",
            ),
            Self::ChunkResolverCompletionMismatch(_) => formatter.write_str(
                "bounded admission resolver completion is missing its durable refinement",
            ),
            Self::ChunkAdmissionStateMismatch(_) => formatter
                .write_str("root admission is neither open nor refined by the bounded partition"),
            Self::SourceImageProof(error) => {
                write!(formatter, "source-image exactness proof failed: {error}")
            }
            Self::CaseImageProof(error) => write!(formatter, "case-image proof failed: {error}"),
            Self::CaseChunkPartition(error) => {
                write!(formatter, "case-chunk partition failed: {error}")
            }
            Self::UniformAdmissionProof(error) => {
                write!(formatter, "uniform-admission proof failed: {error}")
            }
            Self::SupportCell(error) => write!(formatter, "invalid support cell: {error}"),
            Self::SupportEvidence(error) => write!(formatter, "invalid support evidence: {error}"),
            Self::Journal(error) => write!(formatter, "invalid relational journal state: {error}"),
            Self::Work(error) => write!(formatter, "support-frontier work is invalid: {error}"),
        }
    }
}

impl Error for RelationalSupportStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SourceImageProof(error) => Some(error),
            Self::CaseImageProof(error) => Some(error),
            Self::CaseChunkPartition(error) => Some(error),
            Self::UniformAdmissionProof(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            Self::SupportEvidence(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::Work(error) => Some(error),
            Self::InvalidPlanRoot
            | Self::RootCellMissing(_)
            | Self::InvalidRootActivation(_)
            | Self::RootObligationCellMismatch { .. }
            | Self::MultipleRootInjectivityObligations
            | Self::MultipleRootCardinalityObligations
            | Self::MultipleRootAdmissionObligations
            | Self::QualifyingInjectivityProofObligationMismatch
            | Self::QualifyingCardinalityProofObligationMismatch
            | Self::QualifyingAdmissionProofObligationMismatch
            | Self::QualifyingChunkAdmissionObligationMissing
            | Self::SupportPlanRootMismatch { .. }
            | Self::RegisteredRootObligationMissing(_)
            | Self::ResolverWithoutReadiness(_)
            | Self::DurableSourceImageBindingMismatch { .. }
            | Self::SourceImageProofMissingBeforePopulationWork
            | Self::DurableProofEvidenceMismatch(_)
            | Self::DurableChunkRefinementMismatch(_)
            | Self::ChunkResolverCompletionMismatch(_)
            | Self::ChunkAdmissionStateMismatch(_) => None,
        }
    }
}

impl From<WorkFrontierError> for RelationalSupportStepDriverError {
    fn from(error: WorkFrontierError) -> Self {
        Self::Work(error)
    }
}

impl From<RelationalCaseImageInjectivityProofError> for RelationalSupportStepDriverError {
    fn from(error: RelationalCaseImageInjectivityProofError) -> Self {
        Self::CaseImageProof(error)
    }
}

impl From<RelationalSourceImageExactnessProofError> for RelationalSupportStepDriverError {
    fn from(error: RelationalSourceImageExactnessProofError) -> Self {
        Self::SourceImageProof(error)
    }
}

impl From<RelationalCaseChunkPartitionError> for RelationalSupportStepDriverError {
    fn from(error: RelationalCaseChunkPartitionError) -> Self {
        Self::CaseChunkPartition(error)
    }
}

impl From<RelationalUniformAdmissionProofError> for RelationalSupportStepDriverError {
    fn from(error: RelationalUniformAdmissionProofError) -> Self {
        Self::UniformAdmissionProof(error)
    }
}

impl From<SupportCellError> for RelationalSupportStepDriverError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl From<SupportEvidenceError> for RelationalSupportStepDriverError {
    fn from(error: SupportEvidenceError) -> Self {
        Self::SupportEvidence(error)
    }
}

impl From<RelationalJournalError> for RelationalSupportStepDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}
