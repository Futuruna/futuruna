//! One-quantum support-frontier and concrete fallback scheduling for
//! relational Explore.
//!
//! This module is deliberately smaller than a run loop. It inspects one
//! authenticated journal prefix, emits at most one support-root seed or
//! executes at most one base-relation quantum, and returns an ordered batch of
//! idempotent frames for a durable outer loop to append. CPU/RAM limits,
//! deadlines, worker assignment, retry policy, and post-FIND analysis
//! scheduling remain outside this semantic boundary.
//!
//! Cursor advancement follows every consequence of a yielded-member chunk.
//! For a nonterminal chunk it is the final frame; for a terminal chunk it is a
//! crash barrier followed only by retryable exhaustion/seal/completion frames.
//! Consequently, a process crash before the cursor reselects the same first
//! unadvanced member and idempotently rediscovers its evidence/readiness. Once
//! the cursor is durable, every member consequence needed to continue past the
//! bounded chunk is durable and resume can close the exhausted fiber directly.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

use crate::CheckedExploreQueryView;

use super::relation::{
    AdmissionDecision, AdmissionId, QuestionId, RelationId, RelationalCaseId, SelectionDecision,
    SourceKey,
};
use super::relational_case_executor::{
    RelationalCaseExecutor, RelationalCaseExecutorError, RelationalSingletonTransition,
    RelationalSuccessorAdvance, SuccessorFiberExhaustionReceiptId,
};
use super::relational_classified_sweep::{
    RelationalClassifiedCaseOutcome, RelationalOrderedClassificationSubject,
};
use super::relational_classified_sweep_step_driver::{
    RelationalClassifiedSweepStepDriver, RelationalClassifiedSweepStepDriverError,
    RelationalClassifiedSweepStepOutcome, RelationalClassifiedSweepStepQuantum,
};
use super::relational_executor::{
    RelationalExpressionRuntime, RelationalSourceAdvance, RelationalSourceContinuation,
    RelationalSourceEnumerator, RelationalSourceExecutorError, SourceBindingExhaustionReceiptId,
};
use super::relational_frontier::{
    CanonicalSourcePrefix, RelationalWorkFrontier, WorkCompletionRef, WorkCursor,
    WorkFrontierError, WorkNodeId, WorkNodeSnapshot, WorkNodeSpec,
    WORK_FRONTIER_MAX_COMPACTION_NODES,
};
use super::relational_ir::ExploreSuccessorKindIr;
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalSchedulerView,
};
use super::relational_native_classifier::RelationalNativeClassifierV2;
use super::relational_selected_run_step_driver::{
    RelationalSelectedRunStepDriver, RelationalSelectedRunStepDriverError,
    RelationalSelectedRunStepOutcome, RelationalSelectedRunStepQuantum,
};
use super::relational_support_planner::{RelationalSupportPlan, RelationalSupportPlanRoot};
use super::relational_support_step_driver::{
    RelationalSupportStepDriver, RelationalSupportStepDriverError, RelationalSupportStepOutcome,
    RelationalSupportStepQuantum,
};
use super::support_evidence::SupportEvidenceRoot;
use super::support_journal::SupportJournalEvent;

const DEFAULT_COMPLETED_WORK_COMPACTION_TRIGGER: NonZeroU32 =
    NonZeroU32::new(8_192).expect("the default work compaction trigger is nonzero");
const DEFAULT_MAX_COMPACTION_NODES: NonZeroU32 =
    NonZeroU32::new(4_096).expect("the default work compaction batch is nonzero");

/// What one invocation accomplished. This is presentation metadata only; it
/// is not hashed into the journal or any answer identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStepQuantum {
    RegisterSupportPlan {
        plan_root: RelationalSupportPlanRoot,
    },
    Support(RelationalSupportStepQuantum),
    ClassifiedSweep(RelationalClassifiedSweepStepQuantum),
    SelectedRunMaterialization(RelationalSelectedRunStepQuantum),
    SealClassifiedSupportObligationFrontier {
        chunk_count: usize,
        coordinate_count: u128,
    },
    SealClassifiedSupportCatalog {
        chunk_count: usize,
        coordinate_count: u128,
    },
    CompactWorkFrontier {
        removed_nodes: u32,
    },
    SeedSourceRoot,
    SourceMembers {
        node_id: WorkNodeId,
        binding_index: u32,
        first_member_ordinal: u128,
        member_count: NonZeroU16,
        fused_singleton_member_count: u16,
    },
    SourceMembersAndBindingExhaustion {
        node_id: WorkNodeId,
        binding_index: u32,
        first_member_ordinal: u128,
        member_count: NonZeroU16,
        fused_singleton_member_count: u16,
        receipt_id: SourceBindingExhaustionReceiptId,
    },
    SourceBindingExhaustion {
        node_id: WorkNodeId,
        receipt_id: SourceBindingExhaustionReceiptId,
    },
    SourceRelationExhaustion,
    SuccessorMembers {
        node_id: WorkNodeId,
        source_key: SourceKey,
        first_case_id: RelationalCaseId,
        first_member_ordinal: u128,
        member_count: NonZeroU16,
    },
    SuccessorMembersAndFiberExhaustion {
        node_id: WorkNodeId,
        source_key: SourceKey,
        first_case_id: RelationalCaseId,
        first_member_ordinal: u128,
        member_count: NonZeroU16,
        receipt_id: SuccessorFiberExhaustionReceiptId,
    },
    SuccessorFiberExhaustion {
        node_id: WorkNodeId,
        source_key: SourceKey,
        receipt_id: SuccessorFiberExhaustionReceiptId,
    },
    Admission {
        node_id: WorkNodeId,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    },
    Find {
        node_id: WorkNodeId,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    },
}

/// A batch is valid only against the journal head from which it was planned.
/// A durable adapter should compare both fields before appending the frames in
/// order. It may persist a proper prefix: the consequence-before-cursor crash
/// barrier makes the next planning call recover without a second transaction
/// log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalStepQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

/// A non-progress result deliberately stops short of claiming a result view,
/// mechanism target, or extensional relation root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalConcreteQuiescence {
    /// Exact support already closes the requested logical population. The
    /// analysis layer may turn these typed roots into its own upstream receipt;
    /// this concrete scheduler neither guesses that receipt nor materializes
    /// CaseIds merely to obtain one.
    SupportEvidenceClosed {
        support_plan_root: RelationalSupportPlanRoot,
        support_evidence_root: SupportEvidenceRoot,
    },
    /// Source/successor enumeration and admission/FIND classification are
    /// exact. This is still only a base-catalog statement, not an extensional
    /// journal close and not completion of the post-FIND analysis DAG.
    ConcreteBaseClassified {
        cases: u128,
        admitted: u128,
        selected: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStepOutcome {
    Emitted(RelationalStepBatch),
    Quiescent(RelationalConcreteQuiescence),
}

/// Support-root and concrete-fallback planner bound to the immutable checked
/// query artifact.
///
/// Construction accepts no independently paired IR/identity tuple. The
/// producer-owned checked view supplies all semantic IDs, and the support plan
/// must match that same tuple before the driver can be used.
pub(crate) struct RelationalStepDriver<'query> {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan: &'query RelationalSupportPlan,
    support: RelationalSupportStepDriver,
    classified_sweep: Option<RelationalClassifiedSweepStepDriver<'query>>,
    selected_runs: Option<RelationalSelectedRunStepDriver<'query>>,
    source: RelationalSourceEnumerator<'query>,
    cases: RelationalCaseExecutor<'query>,
    /// Whether source-member execution can fuse TO, admission, and FIND into
    /// the same expensive quantum. This operational shape bit selects adaptive
    /// source chunking; classified slices are independently eligible for the
    /// same caller bound. It never enters an event or semantic identity.
    fuses_singleton_source_members: bool,
    /// Query-bound operational accelerator for concrete singleton transitions
    /// that are not eligible for the support-cell classified sweep.
    native_classifier: Option<RelationalNativeClassifierV2>,
    /// Purely operational batch bound. It is absent from every event, work
    /// identity, evidence root, and journal contract.
    max_members_per_quantum: NonZeroU16,
    completed_work_compaction_trigger: NonZeroU32,
    max_compaction_nodes: NonZeroU32,
}

impl<'query> RelationalStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
    ) -> Result<Self, RelationalStepDriverError> {
        Self::from_checked_with_max_members_per_quantum(checked, support_plan, NonZeroU16::MIN)
    }

    pub(crate) fn from_checked_with_max_members_per_quantum(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        max_members_per_quantum: NonZeroU16,
    ) -> Result<Self, RelationalStepDriverError> {
        Self::from_checked_with_max_members_per_quantum_and_native_classifier(
            checked,
            support_plan,
            max_members_per_quantum,
            None,
        )
    }

    pub(crate) fn from_checked_with_max_members_per_quantum_and_native_classifier(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        max_members_per_quantum: NonZeroU16,
        native_classifier: Option<RelationalNativeClassifierV2>,
    ) -> Result<Self, RelationalStepDriverError> {
        Self::from_checked_with_operational_limits(
            checked,
            support_plan,
            max_members_per_quantum,
            DEFAULT_COMPLETED_WORK_COMPACTION_TRIGGER,
            DEFAULT_MAX_COMPACTION_NODES,
            native_classifier,
        )
    }

    pub(crate) fn from_checked_with_operational_limits(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        max_members_per_quantum: NonZeroU16,
        completed_work_compaction_trigger: NonZeroU32,
        max_compaction_nodes: NonZeroU32,
        native_classifier: Option<RelationalNativeClassifierV2>,
    ) -> Result<Self, RelationalStepDriverError> {
        if max_compaction_nodes.get() > WORK_FRONTIER_MAX_COMPACTION_NODES {
            return Err(RelationalStepDriverError::InvalidCompactionLimit {
                actual: max_compaction_nodes.get(),
                maximum: WORK_FRONTIER_MAX_COMPACTION_NODES,
            });
        }
        checked
            .closed_query
            .validate()
            .map_err(RelationalStepDriverError::InvalidQuery)?;
        if !support_plan.validate_root()
            || support_plan.relation_id() != checked.relation_id()
            || support_plan.admission_id() != checked.admission_id()
            || support_plan.question_id() != checked.question_id()
        {
            return Err(RelationalStepDriverError::SupportPlanScopeMismatch);
        }

        let support = RelationalSupportStepDriver::from_plan(support_plan)?;
        let classified_sweep = if support.has_case_chunk_partition() {
            Some(
                RelationalClassifiedSweepStepDriver::from_checked_with_native_classifier(
                    checked,
                    support_plan,
                    native_classifier.clone(),
                )?,
            )
        } else {
            None
        };
        let selected_runs = if classified_sweep.is_some() {
            Some(RelationalSelectedRunStepDriver::from_checked(
                checked,
                support_plan,
            )?)
        } else {
            None
        };
        Ok(Self {
            relation_id: checked.relation_id(),
            admission_id: checked.admission_id(),
            question_id: checked.question_id(),
            support_plan,
            support,
            classified_sweep,
            selected_runs,
            source: RelationalSourceEnumerator::new(
                checked.relation_id(),
                &checked.closed_query.source,
            )?,
            cases: RelationalCaseExecutor::new(checked.relation_id(), checked.closed_query)?,
            fuses_singleton_source_members: matches!(
                &checked.closed_query.successor.kind,
                ExploreSuccessorKindIr::Singleton { .. }
            ),
            native_classifier,
            max_members_per_quantum,
            completed_work_compaction_trigger,
            max_compaction_nodes,
        })
    }

    pub(crate) const fn max_members_per_quantum(&self) -> NonZeroU16 {
        self.max_members_per_quantum
    }

    /// Emit at most one support-root or concrete-base quantum against the
    /// current durable journal prefix. Returned events are unapplied.
    pub(crate) fn step<R: RelationalExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<RelationalStepOutcome, RelationalStepDriverError> {
        self.step_with_max_members_per_quantum(journal, runtime, self.max_members_per_quantum)
    }

    /// Execute one concrete base quantum while allowing the invocation owner
    /// to tighten statically singleton fused source work or one classified
    /// chunk slice. Other query shapes retain the fixed construction-time seam.
    pub(crate) fn step_with_max_members_per_quantum<R: RelationalExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
        max_members_per_quantum: NonZeroU16,
    ) -> Result<RelationalStepOutcome, RelationalStepDriverError> {
        let view = journal.scheduler_view()?;
        self.validate_scope(view)?;

        match view.support_plan_root() {
            None => {
                return Ok(self.batch(
                    view,
                    RelationalStepQuantum::RegisterSupportPlan {
                        plan_root: self.support_plan.root(),
                    },
                    vec![RelationalJournalEvent::support_plan_registered(
                        self.support_plan.clone(),
                    )],
                ));
            }
            Some(root) if root != self.support_plan.root() => {
                return Err(RelationalStepDriverError::SupportPlanRootMismatch {
                    expected: self.support_plan.root(),
                    actual: root,
                });
            }
            Some(_) => {}
        }

        // Drain concrete witnesses from the already accepted classified
        // prefix before support closure can advance. One missing selected run
        // is materialized per invocation, in canonical chunk/run order, so an
        // interesting case becomes observable before the full sweep ends.
        if let Some(selected_runs) = &self.selected_runs {
            match selected_runs.step(journal, runtime)? {
                RelationalSelectedRunStepOutcome::Emitted(batch) => {
                    debug_assert_eq!(batch.expected_sequence(), view.sequence());
                    debug_assert_eq!(batch.expected_head(), view.head());
                    return Ok(self.batch(
                        view,
                        RelationalStepQuantum::SelectedRunMaterialization(batch.quantum()),
                        Vec::from(batch.into_events()),
                    ));
                }
                RelationalSelectedRunStepOutcome::CaughtUp => {}
            }
        }

        match self.support.step(view)? {
            RelationalSupportStepOutcome::Emitted(batch) => {
                debug_assert_eq!(batch.expected_sequence(), view.sequence());
                debug_assert_eq!(batch.expected_head(), view.head());
                let quantum = batch.quantum();
                return Ok(self.batch(
                    view,
                    RelationalStepQuantum::Support(quantum),
                    Vec::from(batch.into_events()),
                ));
            }
            RelationalSupportStepOutcome::AwaitingSupportPlanRegistration => {
                return Err(RelationalStepDriverError::SupportPlanRegistrationMissing);
            }
            RelationalSupportStepOutcome::CaughtUp => {}
        }

        // Symbolic proof closure wins before concrete work seeding. In
        // particular, a statically exact-empty case population reaches this
        // branch immediately after support-plan registration and never mints
        // a synthetic CaseId or an extensional relation claim.
        if view.support_catalog_is_sealed() {
            return Ok(RelationalStepOutcome::Quiescent(
                RelationalConcreteQuiescence::SupportEvidenceClosed {
                    support_plan_root: self.support_plan.root(),
                    support_evidence_root: view.support_evidence_root()?,
                },
            ));
        }

        // A recognized exact one-axis root is classified directly into
        // bounded support runs. This is the resumable fast path: it never
        // seeds the extensional source work tree. Checked work is checkpointed
        // in caller-bounded slices; support advances only when those slices
        // deterministically close one canonical chunk transcript.
        if let Some(classified_sweep) = &self.classified_sweep {
            match classified_sweep.step(journal, max_members_per_quantum, runtime)? {
                RelationalClassifiedSweepStepOutcome::Emitted(batch) => {
                    debug_assert_eq!(batch.expected_sequence(), view.sequence());
                    debug_assert_eq!(batch.expected_head(), view.head());
                    return Ok(self.batch(
                        view,
                        RelationalStepQuantum::ClassifiedSweep(batch.quantum()),
                        Vec::from(batch.into_events()),
                    ));
                }
                RelationalClassifiedSweepStepOutcome::ExhaustedAwaitingClosure {
                    partition_artifact_id: _,
                    chunk_count,
                    coordinate_count,
                } => {
                    return self.close_classified_support(view, chunk_count, coordinate_count);
                }
            }
        }

        if !view.source_traversal_is_started() {
            if let Some(batch) = self.seed_root_if_absent(view)? {
                return Ok(RelationalStepOutcome::Emitted(batch));
            }
        }

        // The aggregate source receipt is minted only after every source
        // binding fiber is durably exhausted. It is its own quantum because
        // the just-produced final receipt is not visible to an unapplied batch.
        if view.source_traversal_is_started()
            && !view.source_enumeration_is_closed()
            && !has_open_source_work(view)
        {
            return Ok(self.batch(
                view,
                RelationalStepQuantum::SourceRelationExhaustion,
                vec![journal.source_enumeration_seal_event()?],
            ));
        }

        if view.completed_work_node_count()
            >= usize::try_from(self.completed_work_compaction_trigger.get())
                .expect("u32 compaction trigger fits usize on supported targets")
        {
            if let Some(batch) = self.compaction_batch(journal, view)? {
                return Ok(RelationalStepOutcome::Emitted(batch));
            }
        }

        if let Some(node) = runnable_base_node(view) {
            let outcome = match &node.spec {
                WorkNodeSpec::ExpandSourceBinding { .. } => self.step_source(
                    journal,
                    view,
                    &node,
                    runtime,
                    if self.fuses_singleton_source_members {
                        max_members_per_quantum
                    } else {
                        self.max_members_per_quantum
                    },
                )?,
                WorkNodeSpec::ExpandSuccessors { .. } => {
                    self.step_successor(view, &node, runtime)?
                }
                WorkNodeSpec::EvaluateAdmission { .. } => {
                    self.step_admission(view, &node, runtime)?
                }
                WorkNodeSpec::EvaluateFind { .. } => self.step_find(view, &node, runtime)?,
                _ => unreachable!("runnable_base_node returns only base concrete work"),
            };
            return Ok(RelationalStepOutcome::Emitted(outcome));
        }

        if view.concrete_base_is_classified() {
            // Peel every remaining completed DAG layer before presenting base
            // quiescence. The final checkpoint therefore scales with live
            // resumable work, not with the number of cases already classified.
            if let Some(batch) = self.compaction_batch(journal, view)? {
                return Ok(RelationalStepOutcome::Emitted(batch));
            }
            return Ok(RelationalStepOutcome::Quiescent(
                RelationalConcreteQuiescence::ConcreteBaseClassified {
                    cases: view.case_count() as u128,
                    admitted: view.admitted_count() as u128,
                    selected: view.selected_count() as u128,
                },
            ));
        }

        Err(RelationalStepDriverError::BaseFrontierStalled)
    }

    fn close_classified_support(
        &self,
        view: RelationalSchedulerView<'_>,
        chunk_count: usize,
        coordinate_count: u128,
    ) -> Result<RelationalStepOutcome, RelationalStepDriverError> {
        let support = view.support_validated_closure()?;
        let open_leaves = support.open_leaf_count();
        let open_obligations = support.open_obligation_count();
        if open_leaves != 0 || open_obligations != 0 || !support.support_frontier_is_complete() {
            return Err(
                RelationalStepDriverError::ClassifiedSupportClosureNotReady {
                    open_leaves,
                    open_obligations,
                },
            );
        }

        // Keep the two semantic seals as separate crash-safe quanta. A pause
        // after the obligation frontier seal resumes by emitting only the
        // catalog seal; neither checkpoint claims downstream result closure.
        if !support.obligation_frontier_is_sealed() {
            return Ok(self.batch(
                view,
                RelationalStepQuantum::SealClassifiedSupportObligationFrontier {
                    chunk_count,
                    coordinate_count,
                },
                vec![RelationalJournalEvent::support(
                    SupportJournalEvent::ObligationFrontierSealed,
                )],
            ));
        }
        // Readiness is intentionally weaker than final completeness here: the
        // catalog seal is the next durable event, so requiring that seal before
        // emitting it would make this state machine impossible to advance.
        if !support.catalog_seal_is_ready() {
            return Err(
                RelationalStepDriverError::ClassifiedSupportClosureNotReady {
                    open_leaves,
                    open_obligations,
                },
            );
        }
        if !support.catalog_is_sealed() {
            return Ok(self.batch(
                view,
                RelationalStepQuantum::SealClassifiedSupportCatalog {
                    chunk_count,
                    coordinate_count,
                },
                vec![RelationalJournalEvent::support(
                    SupportJournalEvent::CatalogSealed,
                )],
            ));
        }
        if !support.obligation_frontier_is_complete() {
            return Err(
                RelationalStepDriverError::ClassifiedSupportClosureNotReady {
                    open_leaves,
                    open_obligations,
                },
            );
        }

        Ok(RelationalStepOutcome::Quiescent(
            RelationalConcreteQuiescence::SupportEvidenceClosed {
                support_plan_root: self.support_plan.root(),
                support_evidence_root: support.root(),
            },
        ))
    }

    fn compaction_batch(
        &self,
        journal: &RelationalJournal,
        view: RelationalSchedulerView<'_>,
    ) -> Result<Option<RelationalStepBatch>, RelationalStepDriverError> {
        let Some(event) = journal.work_frontier_compaction_event(self.max_compaction_nodes)? else {
            return Ok(None);
        };
        let removed_nodes = event
            .compacted_work_node_count()
            .expect("the journal compaction factory returns a compaction checkpoint");
        Ok(Some(self.make_batch(
            view,
            RelationalStepQuantum::CompactWorkFrontier { removed_nodes },
            vec![event],
        )))
    }

    fn validate_scope(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != self.relation_id
            || contract.admission_id() != self.admission_id
            || contract.question_id() != self.question_id
        {
            return Err(RelationalStepDriverError::JournalScopeMismatch);
        }
        Ok(())
    }

    fn seed_root_if_absent(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<Option<RelationalStepBatch>, RelationalStepDriverError> {
        let root_cursor = self.source.root_cursor()?;
        let work_spec = self.source.work_spec(&root_cursor)?;
        let readiness_spec = WorkNodeSpec::SourcePrefixReady {
            relation_id: self.relation_id,
            binding_index: 0,
            prefix: CanonicalSourcePrefix::empty(),
        };
        let readiness_id = RelationalWorkFrontier::derive_node_id(&readiness_spec, [])?;
        let work_id = RelationalWorkFrontier::derive_node_id(&work_spec, [readiness_id])?;
        let readiness_exists = view.work_node(readiness_id).is_some();
        let work_exists = view.work_node(work_id).is_some();
        if work_exists && !readiness_exists {
            return Err(RelationalStepDriverError::BaseFrontierStalled);
        }
        if readiness_exists && work_exists {
            return Ok(None);
        }

        let mut events = Vec::with_capacity(2);
        if !readiness_exists {
            events.push(RelationalJournalEvent::work_readiness_materialized(
                readiness_spec,
            )?);
        }
        if !work_exists {
            events.push(RelationalJournalEvent::work_node_inserted(
                work_spec,
                [readiness_id],
            )?);
        }
        Ok(Some(self.make_batch(
            view,
            RelationalStepQuantum::SeedSourceRoot,
            events,
        )))
    }

    fn step_source<R: RelationalExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        view: RelationalSchedulerView<'_>,
        node: &WorkNodeSnapshot,
        runtime: &mut R,
        max_members_per_quantum: NonZeroU16,
    ) -> Result<RelationalStepBatch, RelationalStepDriverError> {
        let WorkCursor::NextMemberOrdinal(next_member_ordinal) = node.progress.cursor() else {
            return Err(RelationalStepDriverError::CursorShapeMismatch(node.id));
        };
        let (mut cursor, fiber) =
            self.source
                .resume_cursor_with_fiber(&node.spec, next_member_ordinal, runtime)?;
        let binding_index = cursor.binding_index();
        let first_member_ordinal = cursor.next_member_ordinal();
        let mut member_count = 0u16;
        let mut fused_singleton_member_count = 0u16;
        let mut events = Vec::new();
        let mut pending_work_ids = BTreeSet::new();
        let mut fused_source_keys = BTreeSet::new();

        for _ in 0..max_members_per_quantum.get() {
            if member_count != 0 && cursor.next_member_ordinal() == fiber.cardinality() {
                break;
            }
            let advance = self.source.advance_in_fiber(&cursor, &fiber)?;
            let traversal_event = journal.source_traversal_event(advance.clone())?;
            match advance {
                RelationalSourceAdvance::Yielded {
                    resume,
                    continuation,
                    ..
                } => {
                    events.push(traversal_event);
                    match continuation {
                        RelationalSourceContinuation::Expand(child) => {
                            let child_spec = self.source.work_spec(&child)?;
                            let readiness_spec = WorkNodeSpec::SourcePrefixReady {
                                relation_id: self.relation_id,
                                binding_index: child.binding_index(),
                                prefix: child.canonical_prefix().clone(),
                            };
                            append_ready_and_work_if_absent(
                                view,
                                &mut pending_work_ids,
                                &mut events,
                                readiness_spec,
                                child_spec,
                            )?;
                        }
                        RelationalSourceContinuation::Source(source) => {
                            let source_key = source.source_key();
                            let mut fused = fused_source_keys.contains(&source_key);
                            if !fused
                                && !legacy_successor_path_is_open(
                                    view,
                                    &pending_work_ids,
                                    self.relation_id,
                                    source_key,
                                )?
                            {
                                if let Some(transition) =
                                    self.cases.statically_singleton_transition(
                                        source_key,
                                        source.row(),
                                        runtime,
                                    )?
                                {
                                    if !legacy_case_path_is_open(
                                        view,
                                        &pending_work_ids,
                                        self.admission_id,
                                        self.question_id,
                                        transition.case_id(),
                                    )? {
                                        self.append_fused_singleton_transition(
                                            view,
                                            &mut events,
                                            &source,
                                            transition,
                                            runtime,
                                        )?;
                                        fused_source_keys.insert(source_key);
                                        fused = true;
                                    }
                                }
                            }

                            if fused {
                                fused_singleton_member_count = fused_singleton_member_count
                                    .checked_add(1)
                                    .ok_or(RelationalStepDriverError::ChunkMemberCountOverflow)?;
                            } else {
                                let readiness_spec = WorkNodeSpec::SourceRowReady {
                                    relation_id: self.relation_id,
                                    source_key,
                                };
                                let successor = self.cases.root_cursor(source_key, source.row())?;
                                let successor_spec = self.cases.work_spec(&successor)?;
                                append_ready_and_work_if_absent(
                                    view,
                                    &mut pending_work_ids,
                                    &mut events,
                                    readiness_spec,
                                    successor_spec,
                                )?;
                            }
                        }
                    }
                    cursor = resume;
                    member_count = member_count
                        .checked_add(1)
                        .ok_or(RelationalStepDriverError::ChunkMemberCountOverflow)?;
                }
                RelationalSourceAdvance::Exhausted {
                    cursor,
                    cardinality,
                    receipt,
                } => {
                    if member_count != 0 {
                        return Err(RelationalStepDriverError::ExhaustionAfterChunkMembers(
                            node.id,
                        ));
                    }
                    let receipt_id = receipt.id();
                    let completion = WorkCompletionRef::SourceBindingExhausted {
                        relation_id: self.relation_id,
                        binding_index: cursor.binding_index(),
                        prefix: cursor.canonical_prefix().clone(),
                        terminal_ordinal: cardinality,
                        receipt_id,
                    };
                    return Ok(self.make_batch(
                        view,
                        RelationalStepQuantum::SourceBindingExhaustion {
                            node_id: node.id,
                            receipt_id,
                        },
                        vec![
                            traversal_event,
                            RelationalJournalEvent::work_node_completed(node.id, completion),
                        ],
                    ));
                }
            }
        }

        let member_count = NonZeroU16::new(member_count)
            .ok_or(RelationalStepDriverError::ChunkMadeNoProgress(node.id))?;
        if cursor.next_member_ordinal() == fiber.cardinality() {
            let advance = self.source.advance_in_fiber(&cursor, &fiber)?;
            let traversal_event = journal.source_traversal_event(advance.clone())?;
            let RelationalSourceAdvance::Exhausted {
                cursor: terminal_cursor,
                cardinality,
                receipt,
            } = advance
            else {
                return Err(RelationalStepDriverError::ExpectedFiberExhaustion(node.id));
            };
            let receipt_id = receipt.id();
            // This cursor is the crash barrier for the terminal member chunk.
            // If replay stops after it, the next step resumes directly at the
            // retryable exhaustion transition instead of yielding the already
            // durable members again.
            events.push(RelationalJournalEvent::work_cursor_advanced(
                node.id,
                cardinality,
            ));
            events.push(traversal_event);
            events.push(RelationalJournalEvent::work_node_completed(
                node.id,
                WorkCompletionRef::SourceBindingExhausted {
                    relation_id: self.relation_id,
                    binding_index: terminal_cursor.binding_index(),
                    prefix: terminal_cursor.canonical_prefix().clone(),
                    terminal_ordinal: cardinality,
                    receipt_id,
                },
            ));
            return Ok(self.make_batch(
                view,
                RelationalStepQuantum::SourceMembersAndBindingExhaustion {
                    node_id: node.id,
                    binding_index,
                    first_member_ordinal,
                    member_count,
                    fused_singleton_member_count,
                    receipt_id,
                },
                events,
            ));
        }
        // Last by design: every consequence for every member in the bounded
        // nonterminal chunk is durable before the one progress frame can skip
        // over them. A terminal chunk uses the same cursor barrier, followed
        // by its independently retryable exhaustion and completion receipts.
        events.push(RelationalJournalEvent::work_cursor_advanced(
            node.id,
            cursor.next_member_ordinal(),
        ));
        Ok(self.make_batch(
            view,
            RelationalStepQuantum::SourceMembers {
                node_id: node.id,
                binding_index,
                first_member_ordinal,
                member_count,
                fused_singleton_member_count,
            },
            events,
        ))
    }

    fn append_fused_singleton_transition<R: RelationalExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        events: &mut Vec<RelationalJournalEvent>,
        source: &super::relational_executor::RelationalCompletedSource,
        transition: RelationalSingletonTransition,
        runtime: &mut R,
    ) -> Result<(), RelationalStepDriverError> {
        let (case, receipt) = transition.into_parts();
        match view.case(case.case_id()) {
            Some(durable) => self
                .cases
                .validate_durable_case(source.row(), &case, durable)?,
            None => events.push(case.discovered_event()),
        }

        // Native V2 is used only for a completely unclassified concrete case.
        // If a crash prefix already contains either decision, the ordinary
        // checked leaf path below resumes it instead of risking a conflict
        // between durable and accelerator-produced outcomes.
        if view.admission_decision(case.case_id()).is_none()
            && view.question_decision(case.case_id()).is_none()
        {
            if let Some(native) = &self.native_classifier {
                let subject = RelationalOrderedClassificationSubject::new(source, &case);
                let subjects = [subject];
                let (outcomes, _) = native.classify_or_fallback(&subjects, || {
                    let classification = self.cases.classify(source.row(), &case, runtime)?;
                    let outcome = match (classification.admission(), classification.selection()) {
                        (AdmissionDecision::Rejected, None) => {
                            RelationalClassifiedCaseOutcome::Rejected
                        }
                        (AdmissionDecision::Admitted, Some(SelectionDecision::NotSelected)) => {
                            RelationalClassifiedCaseOutcome::AdmittedNotSelected
                        }
                        (AdmissionDecision::Admitted, Some(SelectionDecision::Selected)) => {
                            RelationalClassifiedCaseOutcome::AdmittedSelected
                        }
                        _ => {
                            return Err(RelationalStepDriverError::InvalidNativeClassification);
                        }
                    };
                    Ok(vec![outcome].into_boxed_slice())
                })?;
                let [outcome] = outcomes.as_ref() else {
                    return Err(RelationalStepDriverError::InvalidNativeClassification);
                };
                events.push(RelationalJournalEvent::admission_classified(
                    case.case_id(),
                    outcome.admission(),
                ));
                if let Some(selection) = outcome.selection() {
                    events.push(RelationalJournalEvent::question_classified(
                        case.case_id(),
                        selection,
                    ));
                }
                events.push(RelationalJournalEvent::successor_fiber_exhaustion_accepted(
                    receipt.clone(),
                ));
                events.push(RelationalJournalEvent::successor_enumeration_sealed(
                    &receipt,
                ));
                return Ok(());
            }
        }

        let admission = match view.admission_decision(case.case_id()) {
            Some(decision) => decision,
            None => {
                let classification = self
                    .cases
                    .evaluate_admission(source.row(), &case, runtime)?;
                let decision = classification.decision();
                events.push(classification.event());
                decision
            }
        };
        match admission {
            AdmissionDecision::Admitted => {
                if view.question_decision(case.case_id()).is_none() {
                    let classification = self
                        .cases
                        .evaluate_find_for_admission_decision(
                            source.row(),
                            &case,
                            admission,
                            runtime,
                        )?
                        .ok_or(RelationalStepDriverError::FindClassificationMissing(
                            case.case_id(),
                        ))?;
                    events.push(classification.event());
                }
            }
            AdmissionDecision::Rejected => {
                if view.question_decision(case.case_id()).is_some() {
                    return Err(RelationalStepDriverError::QuestionForRejectedCase(
                        case.case_id(),
                    ));
                }
            }
        }

        events.push(RelationalJournalEvent::successor_fiber_exhaustion_accepted(
            receipt.clone(),
        ));
        events.push(RelationalJournalEvent::successor_enumeration_sealed(
            &receipt,
        ));
        Ok(())
    }

    fn step_successor<R: RelationalExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        node: &WorkNodeSnapshot,
        runtime: &mut R,
    ) -> Result<RelationalStepBatch, RelationalStepDriverError> {
        let WorkNodeSpec::ExpandSuccessors { source_key, .. } = &node.spec else {
            return Err(RelationalStepDriverError::UnexpectedWorkKind);
        };
        let source = view
            .source_row(*source_key)
            .ok_or(RelationalStepDriverError::UnknownSource(*source_key))?;
        let WorkCursor::NextMemberOrdinal(next_member_ordinal) = node.progress.cursor() else {
            return Err(RelationalStepDriverError::CursorShapeMismatch(node.id));
        };
        let (mut cursor, fiber) = self.cases.resume_cursor_with_fiber(
            &node.spec,
            next_member_ordinal,
            source,
            runtime,
        )?;
        let first_member_ordinal = cursor.next_successor_ordinal();
        let mut first_case_id = None;
        let mut member_count = 0u16;
        let mut events = Vec::new();
        let mut pending_work_ids = BTreeSet::new();

        for _ in 0..self.max_members_per_quantum.get() {
            if member_count != 0 && cursor.next_successor_ordinal() == fiber.cardinality() {
                break;
            }
            match self.cases.advance_in_fiber(&cursor, &fiber)? {
                RelationalSuccessorAdvance::Yielded { case, resume, .. } => {
                    first_case_id.get_or_insert(case.case_id());
                    events.push(case.discovered_event());
                    let readiness_spec = WorkNodeSpec::CaseReady {
                        case_id: case.case_id(),
                    };
                    let admission_spec = WorkNodeSpec::EvaluateAdmission {
                        admission_id: self.admission_id,
                        case_id: case.case_id(),
                    };
                    append_ready_and_work_if_absent(
                        view,
                        &mut pending_work_ids,
                        &mut events,
                        readiness_spec,
                        admission_spec,
                    )?;
                    cursor = resume;
                    member_count = member_count
                        .checked_add(1)
                        .ok_or(RelationalStepDriverError::ChunkMemberCountOverflow)?;
                }
                RelationalSuccessorAdvance::Exhausted {
                    cursor: _,
                    cardinality,
                    receipt,
                } => {
                    if member_count != 0 {
                        return Err(RelationalStepDriverError::ExhaustionAfterChunkMembers(
                            node.id,
                        ));
                    }
                    let receipt_id = receipt.id();
                    let completion = WorkCompletionRef::SuccessorsSealed {
                        relation_id: self.relation_id,
                        source_key: *source_key,
                        terminal_ordinal: cardinality,
                        receipt_id,
                    };
                    return Ok(self.make_batch(
                        view,
                        RelationalStepQuantum::SuccessorFiberExhaustion {
                            node_id: node.id,
                            source_key: *source_key,
                            receipt_id,
                        },
                        vec![
                            RelationalJournalEvent::successor_fiber_exhaustion_accepted(
                                receipt.clone(),
                            ),
                            RelationalJournalEvent::successor_enumeration_sealed(&receipt),
                            RelationalJournalEvent::work_node_completed(node.id, completion),
                        ],
                    ));
                }
            }
        }

        let member_count = NonZeroU16::new(member_count)
            .ok_or(RelationalStepDriverError::ChunkMadeNoProgress(node.id))?;
        let first_case_id =
            first_case_id.ok_or(RelationalStepDriverError::ChunkMadeNoProgress(node.id))?;
        if cursor.next_successor_ordinal() == fiber.cardinality() {
            let advance = self.cases.advance_in_fiber(&cursor, &fiber)?;
            let RelationalSuccessorAdvance::Exhausted {
                cursor: _,
                cardinality,
                receipt,
            } = advance
            else {
                return Err(RelationalStepDriverError::ExpectedFiberExhaustion(node.id));
            };
            let receipt_id = receipt.id();
            // As for source traversal, persist the terminal ordinal before
            // sealing the fiber. Every crash prefix then resumes either at
            // exhaustion or after an idempotently replayable seal, never at an
            // already-discovered case.
            events.push(RelationalJournalEvent::work_cursor_advanced(
                node.id,
                cardinality,
            ));
            events.push(RelationalJournalEvent::successor_fiber_exhaustion_accepted(
                receipt.clone(),
            ));
            events.push(RelationalJournalEvent::successor_enumeration_sealed(
                &receipt,
            ));
            events.push(RelationalJournalEvent::work_node_completed(
                node.id,
                WorkCompletionRef::SuccessorsSealed {
                    relation_id: self.relation_id,
                    source_key: *source_key,
                    terminal_ordinal: cardinality,
                    receipt_id,
                },
            ));
            return Ok(self.make_batch(
                view,
                RelationalStepQuantum::SuccessorMembersAndFiberExhaustion {
                    node_id: node.id,
                    source_key: *source_key,
                    first_case_id,
                    first_member_ordinal,
                    member_count,
                    receipt_id,
                },
                events,
            ));
        }
        // Last by design: every discovered CaseId and its runnable admission
        // leaf precede the single durable successor cursor. A terminal chunk
        // uses the same cursor barrier, followed by its independently
        // retryable exhaustion, seal, and completion receipts.
        events.push(RelationalJournalEvent::work_cursor_advanced(
            node.id,
            cursor.next_successor_ordinal(),
        ));
        Ok(self.make_batch(
            view,
            RelationalStepQuantum::SuccessorMembers {
                node_id: node.id,
                source_key: *source_key,
                first_case_id,
                first_member_ordinal,
                member_count,
            },
            events,
        ))
    }

    fn step_admission<R: RelationalExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        node: &WorkNodeSnapshot,
        runtime: &mut R,
    ) -> Result<RelationalStepBatch, RelationalStepDriverError> {
        let WorkNodeSpec::EvaluateAdmission {
            admission_id,
            case_id,
        } = &node.spec
        else {
            return Err(RelationalStepDriverError::UnexpectedWorkKind);
        };
        if *admission_id != self.admission_id {
            return Err(RelationalStepDriverError::JournalScopeMismatch);
        }
        let case = view
            .case(*case_id)
            .ok_or(RelationalStepDriverError::UnknownCase(*case_id))?;
        let durable = view.admission_decision(*case_id);
        let (decision, evidence_event) = match durable {
            Some(decision) => (decision, None),
            None => {
                let classification = self.cases.evaluate_catalog_admission(case, runtime)?;
                (classification.decision(), Some(classification.event()))
            }
        };

        let mut events = Vec::with_capacity(3);
        if let Some(event) = evidence_event {
            events.push(event);
        }
        if decision == AdmissionDecision::Admitted {
            let readiness_id = case_readiness_id(*case_id)?;
            let find_spec = WorkNodeSpec::EvaluateFind {
                question_id: self.question_id,
                case_id: *case_id,
            };
            let find_id =
                RelationalWorkFrontier::derive_node_id(&find_spec, [readiness_id, node.id])?;
            if view.work_node(find_id).is_none() {
                events.push(RelationalJournalEvent::work_node_inserted(
                    find_spec,
                    [readiness_id, node.id],
                )?);
            }
        }
        // The dependent FIND node may be declared while this node is open;
        // only its execution waits for the completion below.
        events.push(RelationalJournalEvent::work_node_completed(
            node.id,
            WorkCompletionRef::AdmissionDecided {
                admission_id: self.admission_id,
                case_id: *case_id,
                decision,
            },
        ));
        Ok(self.make_batch(
            view,
            RelationalStepQuantum::Admission {
                node_id: node.id,
                case_id: *case_id,
                decision,
            },
            events,
        ))
    }

    fn step_find<R: RelationalExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        node: &WorkNodeSnapshot,
        runtime: &mut R,
    ) -> Result<RelationalStepBatch, RelationalStepDriverError> {
        let WorkNodeSpec::EvaluateFind {
            question_id,
            case_id,
        } = &node.spec
        else {
            return Err(RelationalStepDriverError::UnexpectedWorkKind);
        };
        if *question_id != self.question_id {
            return Err(RelationalStepDriverError::JournalScopeMismatch);
        }
        let admission = view.admission_decision(*case_id);
        if admission != Some(AdmissionDecision::Admitted) {
            return Err(RelationalStepDriverError::FindWithoutDurableAdmission(
                *case_id,
            ));
        }
        let case = view
            .case(*case_id)
            .ok_or(RelationalStepDriverError::UnknownCase(*case_id))?;
        let durable = view.question_decision(*case_id);
        let (decision, evidence_event) = match durable {
            Some(decision) => (decision, None),
            None => {
                let classification = self
                    .cases
                    .evaluate_catalog_find(case, AdmissionDecision::Admitted, runtime)?
                    .ok_or(RelationalStepDriverError::FindClassificationMissing(
                        *case_id,
                    ))?;
                (classification.decision(), Some(classification.event()))
            }
        };

        let mut events = Vec::with_capacity(2);
        if let Some(event) = evidence_event {
            events.push(event);
        }
        events.push(RelationalJournalEvent::work_node_completed(
            node.id,
            WorkCompletionRef::FindDecided {
                question_id: self.question_id,
                case_id: *case_id,
                decision,
            },
        ));
        Ok(self.make_batch(
            view,
            RelationalStepQuantum::Find {
                node_id: node.id,
                case_id: *case_id,
                decision,
            },
            events,
        ))
    }

    fn batch(
        &self,
        view: RelationalSchedulerView<'_>,
        quantum: RelationalStepQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalStepOutcome {
        RelationalStepOutcome::Emitted(self.make_batch(view, quantum, events))
    }

    fn make_batch(
        &self,
        view: RelationalSchedulerView<'_>,
        quantum: RelationalStepQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalStepBatch {
        debug_assert!(!events.is_empty());
        RelationalStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum,
            events: events.into_boxed_slice(),
        }
    }
}

fn append_ready_and_work_if_absent(
    view: RelationalSchedulerView<'_>,
    pending_work_ids: &mut BTreeSet<WorkNodeId>,
    events: &mut Vec<RelationalJournalEvent>,
    readiness_spec: WorkNodeSpec,
    work_spec: WorkNodeSpec,
) -> Result<(), RelationalStepDriverError> {
    let readiness_id = RelationalWorkFrontier::derive_node_id(&readiness_spec, [])?;
    let work_id = RelationalWorkFrontier::derive_node_id(&work_spec, [readiness_id])?;
    let readiness_exists =
        pending_work_ids.contains(&readiness_id) || view.work_node(readiness_id).is_some();
    let work_exists = pending_work_ids.contains(&work_id) || view.work_node(work_id).is_some();
    if work_exists && !readiness_exists {
        return Err(RelationalStepDriverError::BaseFrontierStalled);
    }
    if !readiness_exists {
        events.push(RelationalJournalEvent::work_readiness_materialized(
            readiness_spec,
        )?);
        pending_work_ids.insert(readiness_id);
    }
    if !work_exists {
        events.push(RelationalJournalEvent::work_node_inserted(
            work_spec,
            [readiness_id],
        )?);
        pending_work_ids.insert(work_id);
    }
    Ok(())
}

/// A partially persisted legacy expansion remains the sole owner of its
/// source until it completes. Completed readiness/checkpoint leaves do not
/// block fusion: they carry no cursor and can be compacted independently.
fn legacy_successor_path_is_open(
    view: RelationalSchedulerView<'_>,
    pending_work_ids: &BTreeSet<WorkNodeId>,
    relation_id: RelationId,
    source_key: SourceKey,
) -> Result<bool, RelationalStepDriverError> {
    let readiness_spec = WorkNodeSpec::SourceRowReady {
        relation_id,
        source_key,
    };
    let readiness_id = RelationalWorkFrontier::derive_node_id(&readiness_spec, [])?;
    let successor_spec = WorkNodeSpec::ExpandSuccessors {
        relation_id,
        source_key,
    };
    let successor_id = RelationalWorkFrontier::derive_node_id(&successor_spec, [readiness_id])?;
    Ok(
        work_is_pending_or_open(view, pending_work_ids, readiness_id)
            || work_is_pending_or_open(view, pending_work_ids, successor_id),
    )
}

/// Detect the remaining canonical legacy leaves after a singleton case has
/// been derived. This makes an upgrade safe across every durable prefix: an
/// existing open admission or FIND leaf continues through the old DAG, while
/// fully completed/compacted work can be replaced by idempotent evidence
/// replay in the fused source quantum.
fn legacy_case_path_is_open(
    view: RelationalSchedulerView<'_>,
    pending_work_ids: &BTreeSet<WorkNodeId>,
    admission_id: AdmissionId,
    question_id: QuestionId,
    case_id: RelationalCaseId,
) -> Result<bool, RelationalStepDriverError> {
    let readiness_id = case_readiness_id(case_id)?;
    let admission_spec = WorkNodeSpec::EvaluateAdmission {
        admission_id,
        case_id,
    };
    let admission_work_id =
        RelationalWorkFrontier::derive_node_id(&admission_spec, [readiness_id])?;
    let find_spec = WorkNodeSpec::EvaluateFind {
        question_id,
        case_id,
    };
    let find_work_id =
        RelationalWorkFrontier::derive_node_id(&find_spec, [readiness_id, admission_work_id])?;
    Ok(
        work_is_pending_or_open(view, pending_work_ids, readiness_id)
            || work_is_pending_or_open(view, pending_work_ids, admission_work_id)
            || work_is_pending_or_open(view, pending_work_ids, find_work_id),
    )
}

fn work_is_pending_or_open(
    view: RelationalSchedulerView<'_>,
    pending_work_ids: &BTreeSet<WorkNodeId>,
    node_id: WorkNodeId,
) -> bool {
    pending_work_ids.contains(&node_id)
        || view
            .work_node(node_id)
            .is_some_and(|node| !node.progress.is_complete())
}

fn case_readiness_id(case_id: RelationalCaseId) -> Result<WorkNodeId, WorkFrontierError> {
    RelationalWorkFrontier::derive_node_id(&WorkNodeSpec::CaseReady { case_id }, [])
}

/// Prioritize already-emerging cases, then TO work, then the deepest FROM
/// prefix. This keeps the concrete fallback close to depth-first streaming and
/// bounds incidental frontier fan-out without making priority semantic.
fn runnable_base_node(view: RelationalSchedulerView<'_>) -> Option<WorkNodeSnapshot> {
    view.runnable_work_nodes()
        .filter(|node| is_base_work(&node.spec))
        .min_by(|left, right| compare_work_priority(left, right))
}

fn compare_work_priority(left: &WorkNodeSnapshot, right: &WorkNodeSnapshot) -> Ordering {
    let left_priority = work_priority(&left.spec);
    let right_priority = work_priority(&right.spec);
    left_priority
        .0
        .cmp(&right_priority.0)
        // Higher binding indexes are deeper dependent prefixes.
        .then_with(|| right_priority.1.cmp(&left_priority.1))
        .then_with(|| left.id.cmp(&right.id))
}

fn work_priority(spec: &WorkNodeSpec) -> (u8, u32) {
    match spec {
        WorkNodeSpec::EvaluateAdmission { .. } => (0, 0),
        WorkNodeSpec::EvaluateFind { .. } => (1, 0),
        WorkNodeSpec::ExpandSuccessors { .. } => (2, 0),
        WorkNodeSpec::ExpandSourceBinding { binding_index, .. } => (3, *binding_index),
        _ => (u8::MAX, 0),
    }
}

fn is_base_work(spec: &WorkNodeSpec) -> bool {
    matches!(
        spec,
        WorkNodeSpec::ExpandSourceBinding { .. }
            | WorkNodeSpec::ExpandSuccessors { .. }
            | WorkNodeSpec::EvaluateAdmission { .. }
            | WorkNodeSpec::EvaluateFind { .. }
    )
}

fn has_open_source_work(view: RelationalSchedulerView<'_>) -> bool {
    view.open_work_nodes()
        .any(|node| matches!(&node.spec, WorkNodeSpec::ExpandSourceBinding { .. }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStepDriverError {
    InvalidQuery(String),
    SupportPlanScopeMismatch,
    SupportPlanRootMismatch {
        expected: RelationalSupportPlanRoot,
        actual: RelationalSupportPlanRoot,
    },
    SupportPlanRegistrationMissing,
    JournalScopeMismatch,
    BaseFrontierStalled,
    ClassifiedSupportClosureNotReady {
        open_leaves: usize,
        open_obligations: usize,
    },
    CursorShapeMismatch(WorkNodeId),
    ChunkMadeNoProgress(WorkNodeId),
    ChunkMemberCountOverflow,
    InvalidCompactionLimit {
        actual: u32,
        maximum: u32,
    },
    ExpectedFiberExhaustion(WorkNodeId),
    ExhaustionAfterChunkMembers(WorkNodeId),
    UnexpectedWorkKind,
    UnknownSource(SourceKey),
    UnknownCase(RelationalCaseId),
    FindWithoutDurableAdmission(RelationalCaseId),
    FindClassificationMissing(RelationalCaseId),
    QuestionForRejectedCase(RelationalCaseId),
    InvalidNativeClassification,
    Source(RelationalSourceExecutorError),
    Case(RelationalCaseExecutorError),
    Support(RelationalSupportStepDriverError),
    ClassifiedSweep(RelationalClassifiedSweepStepDriverError),
    SelectedRun(RelationalSelectedRunStepDriverError),
    Work(WorkFrontierError),
    Journal(RelationalJournalError),
}

impl fmt::Display for RelationalStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(formatter, "invalid relational query: {message}"),
            Self::SupportPlanScopeMismatch => {
                formatter.write_str("support plan does not belong to the checked query")
            }
            Self::SupportPlanRootMismatch { .. } => {
                formatter.write_str("journal registered a different support plan root")
            }
            Self::SupportPlanRegistrationMissing => {
                formatter.write_str("support-frontier scheduling preceded plan registration")
            }
            Self::JournalScopeMismatch => {
                formatter.write_str("relational journal does not belong to the checked query")
            }
            Self::BaseFrontierStalled => formatter.write_str(
                "concrete base frontier is incomplete but has no runnable semantic work",
            ),
            Self::ClassifiedSupportClosureNotReady {
                open_leaves,
                open_obligations,
            } => write!(
                formatter,
                "classified sweep exhausted with {open_leaves} open support leaves and {open_obligations} open proof obligations"
            ),
            Self::CursorShapeMismatch(_) => {
                formatter.write_str("concrete enumerator work has an atomic cursor")
            }
            Self::ChunkMadeNoProgress(_) => {
                formatter.write_str("bounded concrete quantum made no cursor progress")
            }
            Self::ChunkMemberCountOverflow => {
                formatter.write_str("bounded concrete quantum exceeded its member count type")
            }
            Self::InvalidCompactionLimit { actual, maximum } => write!(
                formatter,
                "work compaction limit {actual} exceeds the hard maximum {maximum}"
            ),
            Self::ExpectedFiberExhaustion(_) => formatter
                .write_str("concrete fiber did not seal after reaching its declared cardinality"),
            Self::ExhaustionAfterChunkMembers(_) => formatter.write_str(
                "concrete fiber reported exhaustion after yielding within the same chunk",
            ),
            Self::UnexpectedWorkKind => {
                formatter.write_str("selected work node is not concrete base work")
            }
            Self::UnknownSource(_) => {
                formatter.write_str("successor work names an absent durable source")
            }
            Self::UnknownCase(_) => {
                formatter.write_str("classification work names an absent durable case")
            }
            Self::FindWithoutDurableAdmission(_) => {
                formatter.write_str("FIND work lacks durable admitted evidence")
            }
            Self::FindClassificationMissing(_) => {
                formatter.write_str("admitted FIND work produced no classification evidence")
            }
            Self::QuestionForRejectedCase(_) => {
                formatter.write_str("rejected case already has durable FIND evidence")
            }
            Self::InvalidNativeClassification => {
                formatter.write_str("native classifier did not produce one canonical case outcome")
            }
            Self::Source(error) => write!(formatter, "source step failed: {error}"),
            Self::Case(error) => write!(formatter, "case step failed: {error}"),
            Self::Support(error) => write!(formatter, "support step failed: {error}"),
            Self::ClassifiedSweep(error) => {
                write!(formatter, "classified-sweep step failed: {error}")
            }
            Self::SelectedRun(error) => {
                write!(formatter, "selected-run step failed: {error}")
            }
            Self::Work(error) => write!(formatter, "work-frontier step failed: {error}"),
            Self::Journal(error) => write!(formatter, "journal step failed: {error}"),
        }
    }
}

impl Error for RelationalStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Case(error) => Some(error),
            Self::Support(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::SelectedRun(error) => Some(error),
            Self::Work(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::InvalidQuery(_)
            | Self::SupportPlanScopeMismatch
            | Self::SupportPlanRootMismatch { .. }
            | Self::SupportPlanRegistrationMissing
            | Self::JournalScopeMismatch
            | Self::BaseFrontierStalled
            | Self::ClassifiedSupportClosureNotReady { .. }
            | Self::CursorShapeMismatch(_)
            | Self::ChunkMadeNoProgress(_)
            | Self::ChunkMemberCountOverflow
            | Self::InvalidCompactionLimit { .. }
            | Self::ExpectedFiberExhaustion(_)
            | Self::ExhaustionAfterChunkMembers(_)
            | Self::UnexpectedWorkKind
            | Self::UnknownSource(_)
            | Self::UnknownCase(_)
            | Self::FindWithoutDurableAdmission(_)
            | Self::FindClassificationMissing(_)
            | Self::QuestionForRejectedCase(_)
            | Self::InvalidNativeClassification => None,
        }
    }
}

impl From<RelationalSourceExecutorError> for RelationalStepDriverError {
    fn from(error: RelationalSourceExecutorError) -> Self {
        Self::Source(error)
    }
}

impl From<RelationalCaseExecutorError> for RelationalStepDriverError {
    fn from(error: RelationalCaseExecutorError) -> Self {
        Self::Case(error)
    }
}

impl From<WorkFrontierError> for RelationalStepDriverError {
    fn from(error: WorkFrontierError) -> Self {
        Self::Work(error)
    }
}

impl From<RelationalJournalError> for RelationalStepDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<RelationalSupportStepDriverError> for RelationalStepDriverError {
    fn from(error: RelationalSupportStepDriverError) -> Self {
        Self::Support(error)
    }
}

impl From<RelationalClassifiedSweepStepDriverError> for RelationalStepDriverError {
    fn from(error: RelationalClassifiedSweepStepDriverError) -> Self {
        Self::ClassifiedSweep(error)
    }
}

impl From<RelationalSelectedRunStepDriverError> for RelationalStepDriverError {
    fn from(error: RelationalSelectedRunStepDriverError) -> Self {
        Self::SelectedRun(error)
    }
}
