//! One semantic scheduling turn for the complete relational Explore DAG.
//!
//! The subordinate drivers intentionally remain small and independently
//! testable. This coordinator supplies the missing ordering contract between
//! them: register the checked analysis plan, let ready result and mechanism
//! work catch up with each authenticated base prefix, advance the base once,
//! bind its exact terminal population when closed, and only then seal every
//! dependent layer and emit the terminal analysis event.
//!
//! It is still not an I/O or resource loop. Every emitted batch is bound to
//! the journal head from which it was planned; the durable owner installs the
//! batch, and a resource governor admits at most one call at a work boundary.

use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

use crate::CheckedExploreQueryView;

use super::mechanism_support::{
    MechanismSupportCheckpointCursor, MechanismSupportClosureRoot, MechanismSupportFrontierRoot,
};
use super::relation::{MechanismRequestId, RelationalCaseId, ViewId};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerRegistration, RelationalAnalysisPlan, RelationalAnalysisPlanError,
};
use super::relational_classification_evaluator::RelationalClassificationEvaluatorBackend;
use super::relational_executor::RelationalExpressionRuntime;
use super::relational_incidence_result_step_driver::{
    RelationalIncidenceResultStepDriver, RelationalIncidenceResultStepDriverError,
    RelationalIncidenceResultStepOutcome, RelationalIncidenceResultStepQuantum,
    RelationalIncidenceResultStepQuiescence,
};
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalMechanismSupportStepEvents,
};
use super::relational_mechanism_executor::{
    RelationalMechanismEndpoint, RelationalMechanismReplayPause, RelationalMechanismReplayRunError,
    RelationalMechanismReplayRuntime,
};
use super::relational_mechanism_step_driver::{
    RelationalMechanismStepDriver, RelationalMechanismStepDriverError,
    RelationalMechanismStepOutcome, RelationalMechanismStepQuantum,
    RelationalMechanismStepQuiescence, RelationalMechanismStepRunError,
};
use super::relational_native_classifier::RelationalNativeClassifierV2;
use super::relational_result_executor::RelationalResultExpressionRuntime;
use super::relational_result_step_driver::{
    RelationalResultStepDriver, RelationalResultStepDriverError, RelationalResultStepOutcome,
    RelationalResultStepQuantum, RelationalResultStepQuiescence,
};
use super::relational_step_driver::{
    RelationalConcreteQuiescence, RelationalStepDriver, RelationalStepDriverError,
    RelationalStepOutcome, RelationalStepQuantum,
};
use super::relational_support_planner::RelationalSupportPlan;

/// Purely operational bounds. They change journal batch shape, never query or
/// answer identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalStreamDriverLimits {
    base_members_per_quantum: NonZeroU16,
    result_rows_per_quantum: NonZeroU16,
    mechanism_target_cases_per_quantum: NonZeroU16,
    mechanism_artifact_chunk_bytes: NonZeroU32,
}

impl RelationalStreamDriverLimits {
    pub(crate) const fn new(
        base_members_per_quantum: NonZeroU16,
        result_rows_per_quantum: NonZeroU16,
        mechanism_target_cases_per_quantum: NonZeroU16,
    ) -> Self {
        Self {
            base_members_per_quantum,
            result_rows_per_quantum,
            mechanism_target_cases_per_quantum,
            mechanism_artifact_chunk_bytes: NonZeroU32::new(
                super::relational_analysis_journal::RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES
                    as u32,
            )
            .expect("the default mechanism artifact chunk is nonzero"),
        }
    }

    pub(crate) const fn with_mechanism_artifact_chunk_bytes(
        mut self,
        mechanism_artifact_chunk_bytes: NonZeroU32,
    ) -> Self {
        self.mechanism_artifact_chunk_bytes = mechanism_artifact_chunk_bytes;
        self
    }

    pub(crate) const fn base_members_per_quantum(self) -> NonZeroU16 {
        self.base_members_per_quantum
    }

    pub(crate) const fn result_rows_per_quantum(self) -> NonZeroU16 {
        self.result_rows_per_quantum
    }

    pub(crate) const fn mechanism_target_cases_per_quantum(self) -> NonZeroU16 {
        self.mechanism_target_cases_per_quantum
    }

    pub(crate) const fn mechanism_artifact_chunk_bytes(self) -> NonZeroU32 {
        self.mechanism_artifact_chunk_bytes
    }
}

impl Default for RelationalStreamDriverLimits {
    fn default() -> Self {
        // Small enough for frequent pause/resource boundaries, while avoiding
        // one journal segment frame per source member on ordinary scans.
        let chunk = NonZeroU16::new(256).expect("the default relational chunk is nonzero");
        Self::new(chunk, chunk, chunk)
    }
}

/// One complete semantic quantum selected by the coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStreamQuantum {
    RegisterAnalysisPlan,
    Base(RelationalStepQuantum),
    BindExtensionalSelectedQuestion,
    BindCertifiedSelectedQuestion,
    Result(RelationalResultStepQuantum),
    IncidenceResult(RelationalIncidenceResultStepQuantum),
    Mechanism(RelationalMechanismStepQuantum),
    CheckpointMechanismSupport {
        request_id: MechanismRequestId,
        accepted_target_cases: usize,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
    },
    CloseMechanismSupport {
        request_id: MechanismRequestId,
        checkpointed_frontier: bool,
        cursor: MechanismSupportCheckpointCursor,
        support_root: MechanismSupportClosureRoot,
    },
    CloseAnalysis,
}

/// Ordered, unapplied frames bound to one exact journal prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalStreamBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalStreamQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalStreamBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalStreamQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

/// An honest pause or an analysis shape whose executor has not yet joined the
/// coordinator. None of these states claims complement closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStreamQuiescence {
    AwaitingSourceResult {
        view_id: ViewId,
    },
    MechanismReplayPaused {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        endpoint: RelationalMechanismEndpoint,
        reason: RelationalMechanismReplayPause,
    },
    AwaitingChosenViewMechanisms {
        request_id: MechanismRequestId,
        view_id: ViewId,
    },
    AwaitingMechanismIncidenceResult {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    AwaitingMechanismSupport {
        request_id: MechanismRequestId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStreamStepOutcome {
    Emitted(RelationalStreamBatch),
    Quiescent(RelationalStreamQuiescence),
    Complete,
}

/// Complete checked-query-bound semantic scheduler. Durable storage,
/// telemetry, deadlines, retries, and worker ownership remain outside it.
pub(crate) struct RelationalStreamDriver<'query> {
    analysis_plan: RelationalAnalysisPlan,
    base: RelationalStepDriver<'query>,
    results: RelationalResultStepDriver<'query>,
    incidence_results: RelationalIncidenceResultStepDriver<'query>,
    mechanisms: RelationalMechanismStepDriver<'query>,
    support_requests: Box<[MechanismRequestId]>,
    /// Process-local round-robin cursor. Durable support prefixes remain the
    /// resume authority; this only prevents an idle open request from starving
    /// another request's ready support suffix.
    support_request_ordinal: RefCell<usize>,
}

impl<'query> RelationalStreamDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
    ) -> Result<Self, RelationalStreamDriverError> {
        Self::from_checked_with_limits(checked, support_plan, Default::default())
    }

    pub(crate) fn from_checked_with_limits(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        limits: RelationalStreamDriverLimits,
    ) -> Result<Self, RelationalStreamDriverError> {
        Self::from_checked_with_limits_and_native_classifier(checked, support_plan, limits, None)
    }

    pub(crate) fn from_checked_with_limits_and_native_classifier(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        limits: RelationalStreamDriverLimits,
        native_classifier: Option<RelationalNativeClassifierV2>,
    ) -> Result<Self, RelationalStreamDriverError> {
        Self::from_checked_with_limits_and_classification_backends(
            checked,
            support_plan,
            limits,
            native_classifier,
            None,
        )
    }

    pub(crate) fn from_checked_with_limits_and_classification_backends(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        limits: RelationalStreamDriverLimits,
        native_classifier: Option<RelationalNativeClassifierV2>,
        classification_evaluator: Option<&'query RefCell<RelationalClassificationEvaluatorBackend>>,
    ) -> Result<Self, RelationalStreamDriverError> {
        let analysis_plan = RelationalAnalysisPlan::from_checked(checked)?;
        let base = RelationalStepDriver::from_checked_with_max_members_per_quantum_and_classification_backends(
            checked,
            support_plan,
            limits.base_members_per_quantum,
            native_classifier,
            classification_evaluator,
        )?;
        let results = RelationalResultStepDriver::from_checked_with_max_rows_per_quantum(
            checked,
            limits.result_rows_per_quantum,
        )?;
        let incidence_results =
            RelationalIncidenceResultStepDriver::from_checked_with_max_rows_per_quantum(
                checked,
                limits.result_rows_per_quantum,
            )?;
        let mechanisms = RelationalMechanismStepDriver::from_checked_with_limits(
            checked,
            limits.mechanism_target_cases_per_quantum,
            limits.mechanism_artifact_chunk_bytes,
        )?;
        let support_requests = analysis_plan
            .layer_registrations()
            .iter()
            .filter_map(|registration| {
                let RelationalAnalysisLayerRegistration::Mechanisms(registration) = registration
                else {
                    return None;
                };
                Some(registration.request_id())
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            analysis_plan,
            base,
            results,
            incidence_results,
            mechanisms,
            support_requests,
            support_request_ordinal: RefCell::new(0),
        })
    }

    /// Execute at most one semantic quantum. A runtime failure is returned as
    /// a transient execution error; it is never converted to unavailable or
    /// closed evidence by this coordinator.
    pub(crate) fn step<R, M>(
        &self,
        journal: &mut RelationalJournal,
        expression_runtime: &mut R,
        mechanism_runtime: &mut M,
    ) -> Result<RelationalStreamStepOutcome, RelationalStreamRunError<M::Error>>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
        M: RelationalMechanismReplayRuntime,
    {
        self.step_with_base_member_limit(
            journal,
            expression_runtime,
            mechanism_runtime,
            self.base.max_members_per_quantum(),
        )
    }

    /// Execute at most one semantic quantum with an invocation-selected bound
    /// for checked singleton source work and classified-chunk slices. The
    /// bound is operational only; fixed driver limits still apply to every
    /// other scheduler layer.
    pub(crate) fn step_with_base_member_limit<R, M>(
        &self,
        journal: &mut RelationalJournal,
        expression_runtime: &mut R,
        mechanism_runtime: &mut M,
        base_member_limit: NonZeroU16,
    ) -> Result<RelationalStreamStepOutcome, RelationalStreamRunError<M::Error>>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
        M: RelationalMechanismReplayRuntime,
    {
        let view = journal.scheduler_view()?;
        match view.analysis_plan_root() {
            None => {
                return Ok(self.batch(
                    journal,
                    RelationalStreamQuantum::RegisterAnalysisPlan,
                    vec![RelationalJournalEvent::analysis_plan_registered(
                        self.analysis_plan.clone(),
                    )],
                ));
            }
            Some(root) if root != self.analysis_plan.root() => {
                return Err(RelationalStreamDriverError::AnalysisPlanRootMismatch.into());
            }
            Some(_) => {}
        }

        // A replayed terminal bit cannot bypass compiler/runtime rebinding of
        // compact source-summary proof artifacts. This call is bounded by the
        // compact group cap and memoized for the lifetime of the driver.
        self.results
            .rebind_certified_source_summaries(journal, expression_runtime)?;

        if journal
            .analysis_state()
            .is_some_and(|analysis| analysis.is_closed())
        {
            return Ok(RelationalStreamStepOutcome::Complete);
        }

        // An installed segment may end between bounded mechanism chunks. No
        // other answer-defining layer may interleave that artifact, so fresh
        // deterministic replay must reproduce and append its exact suffix
        // before normal readiness/fairness scheduling resumes.
        if journal
            .analysis_state()
            .is_some_and(|analysis| analysis.has_pending_mechanism_artifact())
        {
            match self.mechanisms.step(journal, mechanism_runtime) {
                Ok(RelationalMechanismStepOutcome::Emitted(batch)) => {
                    return Ok(RelationalStreamStepOutcome::Emitted(
                        RelationalStreamBatch {
                            expected_sequence: batch.expected_sequence(),
                            expected_head: batch.expected_head(),
                            quantum: RelationalStreamQuantum::Mechanism(batch.quantum()),
                            events: batch.into_events(),
                        },
                    ));
                }
                Ok(RelationalMechanismStepOutcome::Quiescent(
                    RelationalMechanismStepQuiescence::ReplayPaused {
                        request_id,
                        case_id,
                        endpoint,
                        reason,
                    },
                )) => {
                    return Ok(RelationalStreamStepOutcome::Quiescent(
                        RelationalStreamQuiescence::MechanismReplayPaused {
                            request_id,
                            case_id,
                            endpoint,
                            reason,
                        },
                    ));
                }
                Ok(RelationalMechanismStepOutcome::Quiescent(_)) => {
                    return Err(RelationalStreamDriverError::PendingArtifactNotResumed.into());
                }
                Err(RelationalMechanismStepRunError::Driver(error)) => {
                    return Err(error.into());
                }
                Err(RelationalMechanismStepRunError::Replay(error)) => {
                    return Err(RelationalStreamRunError::MechanismReplay(error));
                }
            }
        }

        // Result specs and row-local evidence are readiness-driven. A case
        // classified as selected is immutable evidence even while the FIND
        // frontier remains open, so consume that work before asking the base
        // enumerator for another quantum. Only the result input seal and
        // projection publication wait for exact selected-population closure.
        let result_quiescence = match self.results.step(journal, expression_runtime)? {
            RelationalResultStepOutcome::Emitted(batch) => {
                return Ok(RelationalStreamStepOutcome::Emitted(
                    RelationalStreamBatch {
                        expected_sequence: batch.expected_sequence(),
                        expected_head: batch.expected_head(),
                        quantum: RelationalStreamQuantum::Result(batch.quantum()),
                        events: batch.into_events(),
                    },
                ));
            }
            RelationalResultStepOutcome::Quiescent(quiescence) => quiescence,
        };
        if matches!(
            result_quiescence,
            RelationalResultStepQuiescence::AnalysisAlreadyClosed
        ) {
            return Ok(RelationalStreamStepOutcome::Complete);
        }

        // Incidence-backed results are equally readiness-driven. They consume
        // each newly durable successful mechanism terminal before another
        // mechanism or base quantum is admitted, then wait honestly for the
        // exact mechanism-incidence closure before sealing their input.
        let incidence_result_quiescence =
            match self.incidence_results.step(journal, expression_runtime)? {
                RelationalIncidenceResultStepOutcome::Emitted(batch) => {
                    return Ok(RelationalStreamStepOutcome::Emitted(
                        RelationalStreamBatch {
                            expected_sequence: batch.expected_sequence(),
                            expected_head: batch.expected_head(),
                            quantum: RelationalStreamQuantum::IncidenceResult(batch.quantum()),
                            events: batch.into_events(),
                        },
                    ));
                }
                RelationalIncidenceResultStepOutcome::Quiescent(quiescence) => quiescence,
            };
        if matches!(
            incidence_result_quiescence,
            RelationalIncidenceResultStepQuiescence::AnalysisAlreadyClosed
        ) {
            return Ok(RelationalStreamStepOutcome::Complete);
        }

        // Import at most one request-local support quantum before admitting
        // more mechanism or base work. Each of its three independent lanes is
        // capped by the smaller runtime/protocol bound inside the journal.
        // A caught-up open request is ordinary Idle and falls through, letting
        // upstream work continue to grow.
        if let Some(request_id) = self.next_open_support_request(journal)? {
            match journal.support_lifecycle_step_events(
                request_id,
                self.mechanisms.max_target_cases_per_quantum(),
            )? {
                RelationalMechanismSupportStepEvents::Checkpoint {
                    accepted_target_cases,
                    cursor,
                    frontier_root,
                    events,
                } => {
                    return Ok(self.batch(
                        journal,
                        RelationalStreamQuantum::CheckpointMechanismSupport {
                            request_id,
                            accepted_target_cases,
                            cursor,
                            frontier_root,
                        },
                        events.into_vec(),
                    ));
                }
                RelationalMechanismSupportStepEvents::Closed {
                    checkpointed_frontier,
                    cursor,
                    support_root,
                    events,
                } => {
                    return Ok(self.batch(
                        journal,
                        RelationalStreamQuantum::CloseMechanismSupport {
                            request_id,
                            checkpointed_frontier,
                            cursor,
                            support_root,
                        },
                        events.into_vec(),
                    ));
                }
                RelationalMechanismSupportStepEvents::Idle => {}
            }
        }

        // Give currently ready mechanism work the same catch-up chance. The
        // selected-target driver admits and replays immutable selected cases
        // from the open FIND prefix, while its exact target seal still waits
        // for question closure. Paused/deferred work is not ready and
        // therefore does not prevent the base frontier from advancing.
        let mechanism_quiescence = match self.mechanisms.step(journal, mechanism_runtime) {
            Ok(RelationalMechanismStepOutcome::Emitted(batch)) => {
                return Ok(RelationalStreamStepOutcome::Emitted(
                    RelationalStreamBatch {
                        expected_sequence: batch.expected_sequence(),
                        expected_head: batch.expected_head(),
                        quantum: RelationalStreamQuantum::Mechanism(batch.quantum()),
                        events: batch.into_events(),
                    },
                ));
            }
            Ok(RelationalMechanismStepOutcome::Quiescent(quiescence)) => quiescence,
            Err(RelationalMechanismStepRunError::Driver(error)) => return Err(error.into()),
            Err(RelationalMechanismStepRunError::Replay(error)) => {
                return Err(RelationalStreamRunError::MechanismReplay(error));
            }
        };
        if matches!(
            mechanism_quiescence,
            RelationalMechanismStepQuiescence::AnalysisAlreadyClosed
        ) {
            return Ok(RelationalStreamStepOutcome::Complete);
        }

        // Reaching here means both downstream schedulers are caught up at the
        // current authenticated base prefix (or explicitly not ready). Admit
        // exactly one more base quantum, so neither direction can starve the
        // other as the relation grows.
        let base = self.base.step_with_max_members_per_quantum(
            journal,
            expression_runtime,
            base_member_limit,
        )?;
        let base = match base {
            RelationalStepOutcome::Emitted(batch) => {
                return Ok(RelationalStreamStepOutcome::Emitted(
                    RelationalStreamBatch {
                        expected_sequence: batch.expected_sequence(),
                        expected_head: batch.expected_head(),
                        quantum: RelationalStreamQuantum::Base(batch.quantum()),
                        events: batch.into_events(),
                    },
                ));
            }
            RelationalStepOutcome::Quiescent(quiescence) => quiescence,
        };

        let analysis = journal
            .analysis_state()
            .ok_or(RelationalStreamDriverError::AnalysisStateMissing)?;
        if analysis.selected_question().is_none() {
            let (quantum, event) = match base {
                RelationalConcreteQuiescence::ConcreteBaseClassified { .. } => (
                    RelationalStreamQuantum::BindExtensionalSelectedQuestion,
                    journal.selected_question_extensional_event()?,
                ),
                RelationalConcreteQuiescence::SupportEvidenceClosed { .. } => (
                    RelationalStreamQuantum::BindCertifiedSelectedQuestion,
                    journal.selected_question_certified_event()?,
                ),
            };
            return Ok(self.batch(journal, quantum, vec![event]));
        }

        if matches!(
            result_quiescence,
            RelationalResultStepQuiescence::AwaitingSelectedQuestion
        ) {
            return Err(RelationalStreamDriverError::SelectedQuestionBridgeMissing.into());
        }
        match mechanism_quiescence {
            RelationalMechanismStepQuiescence::ReplayPaused {
                request_id,
                case_id,
                endpoint,
                reason,
            } => {
                return Ok(RelationalStreamStepOutcome::Quiescent(
                    RelationalStreamQuiescence::MechanismReplayPaused {
                        request_id,
                        case_id,
                        endpoint,
                        reason,
                    },
                ));
            }
            RelationalMechanismStepQuiescence::DeferredChosenView {
                request_id,
                view_id,
            } => {
                return Ok(RelationalStreamStepOutcome::Quiescent(
                    RelationalStreamQuiescence::AwaitingChosenViewMechanisms {
                        request_id,
                        view_id,
                    },
                ));
            }
            RelationalMechanismStepQuiescence::AwaitingSelectedQuestion => {
                return Err(RelationalStreamDriverError::SelectedQuestionBridgeMissing.into());
            }
            RelationalMechanismStepQuiescence::AnalysisAlreadyClosed => {
                return Ok(RelationalStreamStepOutcome::Complete);
            }
            RelationalMechanismStepQuiescence::SelectedMechanismsComplete => {}
        }

        if let RelationalResultStepQuiescence::AwaitingSourceMaterialization { view_id } =
            result_quiescence
        {
            return Ok(RelationalStreamStepOutcome::Quiescent(
                RelationalStreamQuiescence::AwaitingSourceResult { view_id },
            ));
        }
        if !matches!(
            result_quiescence,
            RelationalResultStepQuiescence::SelectedResultsComplete
                | RelationalResultStepQuiescence::DeferredMechanismIncidence { .. }
        ) {
            return Err(RelationalStreamDriverError::ResultDriverQuiescenceMismatch.into());
        }

        match incidence_result_quiescence {
            RelationalIncidenceResultStepQuiescence::IncidenceResultsComplete => {}
            RelationalIncidenceResultStepQuiescence::AwaitingMechanismIncidence {
                view_id,
                request_id,
            } => {
                return Ok(RelationalStreamStepOutcome::Quiescent(
                    RelationalStreamQuiescence::AwaitingMechanismIncidenceResult {
                        view_id,
                        request_id,
                    },
                ));
            }
            RelationalIncidenceResultStepQuiescence::AnalysisAlreadyClosed => {
                return Ok(RelationalStreamStepOutcome::Complete);
            }
        }

        // Support was already offered its one interleaved quantum earlier in
        // this step. If any request remains open, yield honestly and let the
        // next invocation advance or close it; never run a second support
        // quantum merely because all upstream drivers are now quiescent.
        for registration in self.analysis_plan.layer_registrations() {
            let RelationalAnalysisLayerRegistration::Mechanisms(registration) = registration else {
                continue;
            };
            let request_id = registration.request_id();
            let analysis = journal
                .analysis_state()
                .ok_or(RelationalStreamDriverError::AnalysisStateMissing)?;
            if analysis.mechanism_support_closure(request_id).is_some() {
                continue;
            }
            return Ok(RelationalStreamStepOutcome::Quiescent(
                RelationalStreamQuiescence::AwaitingMechanismSupport { request_id },
            ));
        }

        Ok(self.batch(
            journal,
            RelationalStreamQuantum::CloseAnalysis,
            vec![journal.analysis_terminal_event()?],
        ))
    }

    fn next_open_support_request(
        &self,
        journal: &RelationalJournal,
    ) -> Result<Option<MechanismRequestId>, RelationalStreamDriverError> {
        let analysis = journal
            .analysis_state()
            .ok_or(RelationalStreamDriverError::AnalysisStateMissing)?;
        if self.support_requests.is_empty() {
            return Ok(None);
        }
        let mut ordinal = self.support_request_ordinal.borrow_mut();
        let start = *ordinal % self.support_requests.len();
        for offset in 0..self.support_requests.len() {
            let index = (start + offset) % self.support_requests.len();
            let request_id = self.support_requests[index];
            if analysis.mechanism_support_closure(request_id).is_none()
                && analysis
                    .support_checkpoint_has_ready_work(request_id)
                    .map_err(RelationalMechanismStepDriverError::from)?
            {
                *ordinal = (index + 1) % self.support_requests.len();
                return Ok(Some(request_id));
            }
        }
        for offset in 0..self.support_requests.len() {
            let index = (start + offset) % self.support_requests.len();
            let request_id = self.support_requests[index];
            if analysis.mechanism_support_closure(request_id).is_none() {
                *ordinal = (index + 1) % self.support_requests.len();
                return Ok(Some(request_id));
            }
        }
        Ok(None)
    }

    fn batch(
        &self,
        journal: &RelationalJournal,
        quantum: RelationalStreamQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalStreamStepOutcome {
        debug_assert!(!events.is_empty());
        RelationalStreamStepOutcome::Emitted(RelationalStreamBatch {
            expected_sequence: journal.next_sequence(),
            expected_head: journal.head(),
            quantum,
            events: events.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStreamDriverError {
    AnalysisPlan(RelationalAnalysisPlanError),
    Base(RelationalStepDriverError),
    Result(RelationalResultStepDriverError),
    IncidenceResult(RelationalIncidenceResultStepDriverError),
    Mechanism(RelationalMechanismStepDriverError),
    Journal(RelationalJournalError),
    AnalysisPlanRootMismatch,
    AnalysisStateMissing,
    SelectedQuestionBridgeMissing,
    ResultDriverQuiescenceMismatch,
    PendingArtifactNotResumed,
}

impl fmt::Display for RelationalStreamDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisPlan(error) => fmt::Display::fmt(error, formatter),
            Self::Base(error) => fmt::Display::fmt(error, formatter),
            Self::Result(error) => fmt::Display::fmt(error, formatter),
            Self::IncidenceResult(error) => fmt::Display::fmt(error, formatter),
            Self::Mechanism(error) => fmt::Display::fmt(error, formatter),
            Self::Journal(error) => fmt::Display::fmt(error, formatter),
            Self::AnalysisPlanRootMismatch => {
                formatter.write_str("relational stream driver and journal analysis plans differ")
            }
            Self::AnalysisStateMissing => {
                formatter.write_str("registered relational analysis plan has no journal state")
            }
            Self::SelectedQuestionBridgeMissing => formatter.write_str(
                "post-FIND driver is waiting after the base scheduler reported exact closure",
            ),
            Self::ResultDriverQuiescenceMismatch => formatter.write_str(
                "selected result driver stopped without completing or naming a deferred input",
            ),
            Self::PendingArtifactNotResumed => formatter.write_str(
                "open mechanism artifact was not resumed before normal stream scheduling",
            ),
        }
    }
}

impl Error for RelationalStreamDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisPlan(error) => Some(error),
            Self::Base(error) => Some(error),
            Self::Result(error) => Some(error),
            Self::IncidenceResult(error) => Some(error),
            Self::Mechanism(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::AnalysisPlanRootMismatch
            | Self::AnalysisStateMissing
            | Self::SelectedQuestionBridgeMissing
            | Self::ResultDriverQuiescenceMismatch
            | Self::PendingArtifactNotResumed => None,
        }
    }
}

impl From<RelationalAnalysisPlanError> for RelationalStreamDriverError {
    fn from(error: RelationalAnalysisPlanError) -> Self {
        Self::AnalysisPlan(error)
    }
}

impl From<RelationalStepDriverError> for RelationalStreamDriverError {
    fn from(error: RelationalStepDriverError) -> Self {
        Self::Base(error)
    }
}

impl From<RelationalResultStepDriverError> for RelationalStreamDriverError {
    fn from(error: RelationalResultStepDriverError) -> Self {
        Self::Result(error)
    }
}

impl From<RelationalIncidenceResultStepDriverError> for RelationalStreamDriverError {
    fn from(error: RelationalIncidenceResultStepDriverError) -> Self {
        Self::IncidenceResult(error)
    }
}

impl From<RelationalMechanismStepDriverError> for RelationalStreamDriverError {
    fn from(error: RelationalMechanismStepDriverError) -> Self {
        Self::Mechanism(error)
    }
}

impl From<RelationalJournalError> for RelationalStreamDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

/// Runtime failures remain distinct so an outer loop can pause/retry without
/// writing semantic unavailability.
#[derive(Debug)]
pub(crate) enum RelationalStreamRunError<E> {
    Driver(RelationalStreamDriverError),
    MechanismReplay(RelationalMechanismReplayRunError<E>),
}

impl<E> From<RelationalStreamDriverError> for RelationalStreamRunError<E> {
    fn from(error: RelationalStreamDriverError) -> Self {
        Self::Driver(error)
    }
}

impl<E> From<RelationalJournalError> for RelationalStreamRunError<E> {
    fn from(error: RelationalJournalError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E> From<RelationalStepDriverError> for RelationalStreamRunError<E> {
    fn from(error: RelationalStepDriverError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E> From<RelationalResultStepDriverError> for RelationalStreamRunError<E> {
    fn from(error: RelationalResultStepDriverError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E> From<RelationalIncidenceResultStepDriverError> for RelationalStreamRunError<E> {
    fn from(error: RelationalIncidenceResultStepDriverError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E> From<RelationalMechanismStepDriverError> for RelationalStreamRunError<E> {
    fn from(error: RelationalMechanismStepDriverError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E: fmt::Display> fmt::Display for RelationalStreamRunError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => fmt::Display::fmt(error, formatter),
            Self::MechanismReplay(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: Error + 'static> Error for RelationalStreamRunError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Driver(error) => Some(error),
            Self::MechanismReplay(RelationalMechanismReplayRunError::InvalidEvidence(error)) => {
                Some(error)
            }
            Self::MechanismReplay(RelationalMechanismReplayRunError::Runtime {
                source, ..
            }) => Some(source),
        }
    }
}
