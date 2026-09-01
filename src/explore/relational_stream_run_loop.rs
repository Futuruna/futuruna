//! Durable, resource-admitted execution of a relational Explore stream.
//!
//! [`RelationalStreamDriver`] remains the semantic scheduler. This module owns
//! only invocation concerns: an optional monotonic deadline, one-worker host
//! admission under the existing CPU/RAM reserve policy, head-bound journal
//! installation, and a flush before returning any resumable cursor.
//!
//! A resource permit names the exact journal sequence and head inspected by
//! the driver. Evaluation and installation of the resulting bounded batch are
//! one operational work unit. Runtime failure or semantic quiescence never
//! becomes mechanism evidence, result closure, or another semantic event.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;
use std::time::{Duration, Instant};

use super::relational_durable_journal::{
    RelationalDurableCheckpoint, RelationalDurableJournal, RelationalDurableJournalError,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::relational_journal::RelationalJournalHead;
use super::relational_mechanism_executor::RelationalMechanismReplayRuntime;
use super::relational_result_executor::RelationalResultExpressionRuntime;
use super::relational_step_driver::RelationalStepQuantum;
use super::relational_stream_driver::{
    RelationalStreamDriver, RelationalStreamQuantum, RelationalStreamQuiescence,
    RelationalStreamRunError, RelationalStreamStepOutcome,
};
pub(super) use super::stream_resource::ExactStreamOuterContainmentReceipt;
use super::stream_resource::{
    ExactStreamOneWorkerEnvelope, ExactStreamPermitError, ExactStreamResourceAction,
    ExactStreamResourcePauseReason, ExactStreamWorkInFlight, ExactStreamWorkSubject,
};

const RESOURCE_WAIT_FALLBACK: Duration = Duration::from_millis(10);
const BASE_QUANTUM_TARGET: Duration = Duration::from_secs(5);
const BASE_QUANTUM_PREDICTED_CEILING: Duration = Duration::from_secs(10);
const BASE_QUANTUM_DEADLINE_RESERVE: Duration = Duration::from_millis(250);
const BASE_QUANTUM_MAX_MEMBERS: u16 = 256;
const EXPENSIVE_CHECKPOINT_CADENCE: Duration = Duration::from_secs(15);

/// Invocation-local tuning for expensive checked base work. It deliberately
/// survives warm slices in an epoch but is absent from the checked query,
/// durable journal, result projections, and every semantic identity.
#[derive(Debug)]
pub(super) struct RelationalBaseQuantumController {
    next_members: NonZeroU16,
    ewma_nanos_per_member: Option<u128>,
    last_nanos_per_member: Option<u128>,
    last_expensive_checkpoint: Option<Instant>,
}

impl Default for RelationalBaseQuantumController {
    fn default() -> Self {
        Self {
            next_members: NonZeroU16::MIN,
            ewma_nanos_per_member: None,
            last_nanos_per_member: None,
            last_expensive_checkpoint: None,
        }
    }
}

impl RelationalBaseQuantumController {
    fn should_pause_before_quantum(&self, remaining: Duration) -> bool {
        self.conservative_nanos_per_member()
            .is_some_and(|estimate| {
                let required = estimate.saturating_add(BASE_QUANTUM_DEADLINE_RESERVE.as_nanos());
                required > remaining.as_nanos()
            })
    }

    fn member_limit(&self, remaining: Option<Duration>) -> NonZeroU16 {
        let mut limit = self.next_members.get();
        let Some(nanos_per_member) = self.conservative_nanos_per_member() else {
            return self.next_members;
        };

        limit = limit.min(members_within(
            BASE_QUANTUM_PREDICTED_CEILING,
            nanos_per_member,
        ));
        if let Some(remaining) = remaining {
            limit = limit.min(members_within(
                remaining.saturating_sub(BASE_QUANTUM_DEADLINE_RESERVE),
                nanos_per_member,
            ));
        }
        NonZeroU16::new(limit.max(1)).expect("the adaptive base quantum is nonzero")
    }

    /// Observe only an installed checked-member base batch. The supplied
    /// elapsed time includes deterministic evaluation and the ordinary journal
    /// append, but not the one-time forced durability flush.
    fn observe_appended_quantum(
        &mut self,
        quantum: RelationalStreamQuantum,
        elapsed: Duration,
    ) -> bool {
        let Some(member_count) = expensive_base_member_count(quantum) else {
            return false;
        };
        let sample = elapsed
            .as_nanos()
            .max(1)
            .div_ceil(u128::from(member_count.get()));
        self.last_nanos_per_member = Some(sample);
        self.ewma_nanos_per_member = Some(match self.ewma_nanos_per_member {
            Some(previous) => previous
                .saturating_mul(3)
                .saturating_add(sample)
                .div_ceil(4),
            None => sample,
        });

        let target_estimate = self
            .ewma_nanos_per_member
            .expect("the observation installed a smoothed timing estimate");
        let ceiling_estimate = self
            .conservative_nanos_per_member()
            .expect("the observation installed a timing estimate");
        let desired = members_within(BASE_QUANTUM_TARGET, target_estimate).min(members_within(
            BASE_QUANTUM_PREDICTED_CEILING,
            ceiling_estimate,
        ));
        let previous = self.next_members.get();
        let next = if desired < previous {
            desired
        } else {
            desired.min(previous.saturating_mul(2))
        }
        .clamp(1, BASE_QUANTUM_MAX_MEMBERS);
        self.next_members = NonZeroU16::new(next).expect("the adaptive base quantum is nonzero");

        true
    }

    fn expensive_checkpoint_due(&self, now: Instant) -> bool {
        self.last_expensive_checkpoint
            .is_none_or(|last| now.saturating_duration_since(last) >= EXPENSIVE_CHECKPOINT_CADENCE)
    }

    fn mark_expensive_checkpoint(&mut self, now: Instant) {
        self.last_expensive_checkpoint = Some(now);
    }

    fn conservative_nanos_per_member(&self) -> Option<u128> {
        match (self.ewma_nanos_per_member, self.last_nanos_per_member) {
            (Some(ewma), Some(last)) => Some(ewma.max(last)),
            (Some(ewma), None) => Some(ewma),
            (None, Some(last)) => Some(last),
            (None, None) => None,
        }
    }
}

fn members_within(duration: Duration, nanos_per_member: u128) -> u16 {
    let count = duration.as_nanos() / nanos_per_member.max(1);
    count.clamp(1, u128::from(BASE_QUANTUM_MAX_MEMBERS)) as u16
}

fn expensive_base_member_count(quantum: RelationalStreamQuantum) -> Option<NonZeroU16> {
    match quantum {
        RelationalStreamQuantum::Base(
            RelationalStepQuantum::SourceMembers {
                fused_singleton_member_count,
                ..
            }
            | RelationalStepQuantum::SourceMembersAndBindingExhaustion {
                fused_singleton_member_count,
                ..
            },
        ) => NonZeroU16::new(fused_singleton_member_count),
        RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(quantum)) => {
            quantum.evaluated_member_count()
        }
        _ => None,
    }
}

/// Operational slice budget. It is absent from the query, journal contract,
/// evidence roots, and answer identity. `None` runs until semantic completion
/// or another honest pause boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RelationalStreamSliceBudget {
    max_runtime: Option<Duration>,
}

impl RelationalStreamSliceBudget {
    pub(super) fn new(
        max_runtime: Option<Duration>,
    ) -> Result<Self, RelationalStreamSliceBudgetError> {
        if max_runtime.is_some_and(|runtime| runtime.is_zero()) {
            return Err(RelationalStreamSliceBudgetError::ZeroRuntimeLimit);
        }
        Ok(Self { max_runtime })
    }

    pub(super) const fn max_runtime(self) -> Option<Duration> {
        self.max_runtime
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelationalStreamSliceBudgetError {
    ZeroRuntimeLimit,
}

impl fmt::Display for RelationalStreamSliceBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRuntimeLimit => {
                formatter.write_str("relational stream max_runtime must be positive")
            }
        }
    }
}

impl Error for RelationalStreamSliceBudgetError {}

/// A checkpoint plus work counters scoped only to this invocation. The
/// checkpoint has passed [`RelationalDurableJournal::flush_for_pause`]; the
/// counters are observability metadata and make no coverage claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelationalStreamSliceProgress {
    checkpoint: RelationalDurableCheckpoint,
    semantic_batches_appended: u64,
    semantic_events_appended: u64,
}

impl RelationalStreamSliceProgress {
    pub(super) const fn checkpoint(&self) -> &RelationalDurableCheckpoint {
        &self.checkpoint
    }

    pub(super) const fn semantic_batches_appended(&self) -> u64 {
        self.semantic_batches_appended
    }

    pub(super) const fn semantic_events_appended(&self) -> u64 {
        self.semantic_events_appended
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelationalStreamSlicePauseReason {
    RuntimeLimit,
    ResourceAdmission { code: &'static str },
    Semantic(RelationalStreamQuiescence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RelationalStreamSliceOutcome {
    Paused {
        progress: RelationalStreamSliceProgress,
        reason: RelationalStreamSlicePauseReason,
    },
    Complete {
        progress: RelationalStreamSliceProgress,
    },
}

impl RelationalStreamSliceOutcome {
    pub(super) const fn progress(&self) -> &RelationalStreamSliceProgress {
        match self {
            Self::Paused { progress, .. } | Self::Complete { progress } => progress,
        }
    }

    pub(super) const fn pause_reason(&self) -> Option<RelationalStreamSlicePauseReason> {
        match self {
            Self::Paused { reason, .. } => Some(*reason),
            Self::Complete { .. } => None,
        }
    }
}

/// Advance one relational stream slice with a freshly initialized conservative
/// one-worker envelope. The envelope preserves at least the configured 20%
/// host CPU/RAM reserve; this loop never launches a second worker. Optional
/// outer containment is invocation-local scheduling authority and never enters
/// the semantic journal or result identity.
pub(super) fn run_relational_stream_slice<R, M>(
    durable: &mut RelationalDurableJournal,
    driver: &RelationalStreamDriver<'_>,
    expression_runtime: &mut R,
    mechanism_runtime: &mut M,
    base_quantum_controller: &mut RelationalBaseQuantumController,
    budget: RelationalStreamSliceBudget,
    outer_containment: Option<ExactStreamOuterContainmentReceipt>,
) -> Result<RelationalStreamSliceOutcome, RelationalStreamSliceError<M::Error>>
where
    R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    M: RelationalMechanismReplayRuntime,
{
    let mut resources = ExactStreamOneWorkerEnvelope::new_with_outer_containment(outer_containment)
        .map_err(
            |reason| RelationalStreamSliceError::ResourceInitialization {
                code: reason.code(),
            },
        )?;
    run_relational_stream_slice_with_resources(
        durable,
        driver,
        expression_runtime,
        mechanism_runtime,
        &mut resources,
        base_quantum_controller,
        budget,
    )
}

/// Injection seam for a caller that already owns the one-worker envelope.
/// Resource state is intentionally not journaled: it authorizes work, while
/// the semantic journal alone defines resumable progress. The supplied
/// envelope must be idle at a work boundary; the ordinary entry point above
/// creates that state by construction.
pub(super) fn run_relational_stream_slice_with_resources<R, M>(
    durable: &mut RelationalDurableJournal,
    driver: &RelationalStreamDriver<'_>,
    expression_runtime: &mut R,
    mechanism_runtime: &mut M,
    resources: &mut ExactStreamOneWorkerEnvelope,
    base_quantum_controller: &mut RelationalBaseQuantumController,
    budget: RelationalStreamSliceBudget,
) -> Result<RelationalStreamSliceOutcome, RelationalStreamSliceError<M::Error>>
where
    R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    M: RelationalMechanismReplayRuntime,
{
    let started = Instant::now();
    let deadline = match budget.max_runtime() {
        Some(runtime) => Some(
            started
                .checked_add(runtime)
                .ok_or(RelationalStreamSliceError::RuntimeDeadlineOverflow)?,
        ),
        None => None,
    };
    let mut semantic_batches_appended = 0_u64;
    let mut semantic_events_appended = 0_u64;

    loop {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            // This is an observable slice boundary, not the end of the warm
            // epoch. No permit is outstanding here. Retaining the governor's
            // checked host window lets a following slice reuse still-current
            // authority or take the next due sample instead of restarting the
            // entire stability protocol.
            return pause(
                durable,
                semantic_batches_appended,
                semantic_events_appended,
                RelationalStreamSlicePauseReason::RuntimeLimit,
            );
        }

        let identity = {
            let journal = durable.journal()?;
            RelationalQuantumIdentity {
                expected_sequence: journal.next_sequence(),
                expected_head: journal.head(),
            }
        };
        let in_flight = match admit_relational_quantum(resources, identity, deadline) {
            Ok(RelationalQuantumAdmission::Granted(in_flight)) => in_flight,
            Ok(RelationalQuantumAdmission::RuntimeLimit) => {
                return pause(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    RelationalStreamSlicePauseReason::RuntimeLimit,
                );
            }
            Ok(RelationalQuantumAdmission::ResourcePause(reason)) => {
                return pause(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    RelationalStreamSlicePauseReason::ResourceAdmission {
                        code: reason.code(),
                    },
                );
            }
            Err(error) => {
                return resource_protocol_error(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    error,
                    None,
                );
            }
        };

        // Admission may itself wait until the deadline. Do not begin an
        // atomic semantic quantum merely because the permit was granted at
        // the boundary; predicted sizing below handles the remaining positive
        // budget, while a one-member quantum remains the minimum atomic unit.
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            if let Err(error) = finish_relational_quantum(resources, in_flight) {
                return resource_protocol_error(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    error,
                    None,
                );
            }
            return pause(
                durable,
                semantic_batches_appended,
                semantic_events_appended,
                RelationalStreamSlicePauseReason::RuntimeLimit,
            );
        }

        let remaining = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        if remaining
            .is_some_and(|remaining| base_quantum_controller.should_pause_before_quantum(remaining))
        {
            if let Err(error) = finish_relational_quantum(resources, in_flight) {
                return resource_protocol_error(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    error,
                    None,
                );
            }
            return pause(
                durable,
                semantic_batches_appended,
                semantic_events_appended,
                RelationalStreamSlicePauseReason::RuntimeLimit,
            );
        }

        let base_member_limit = base_quantum_controller.member_limit(remaining);
        let quantum_started = Instant::now();
        let step = match durable.journal_mut_for_event_planning() {
            Ok(journal) => driver.step_with_base_member_limit(
                journal,
                expression_runtime,
                mechanism_runtime,
                base_member_limit,
            ),
            Err(error) => {
                let _ = finish_relational_quantum(resources, in_flight);
                return Err(error.into());
            }
        };
        match step {
            Ok(RelationalStreamStepOutcome::Emitted(batch)) => {
                if batch.expected_sequence() != identity.expected_sequence
                    || batch.expected_head() != identity.expected_head
                {
                    let finish_error = finish_relational_quantum(resources, in_flight).err();
                    return resource_protocol_error(
                        durable,
                        semantic_batches_appended,
                        semantic_events_appended,
                        RelationalStreamResourceProtocolError::BatchAuthorityMismatch,
                        finish_error,
                    );
                }

                let quantum = batch.quantum();
                let append = durable.append_events(
                    batch.expected_sequence(),
                    batch.expected_head(),
                    batch.into_events(),
                );
                let compute_and_append_elapsed = quantum_started.elapsed();
                let finish = finish_relational_quantum(resources, in_flight);
                let append = append?;
                if let Err(error) = finish {
                    return resource_protocol_error(
                        durable,
                        semantic_batches_appended,
                        semantic_events_appended,
                        error,
                        None,
                    );
                }

                semantic_batches_appended = semantic_batches_appended.checked_add(1).ok_or(
                    RelationalStreamSliceError::CounterOverflow("semantic batch count"),
                )?;
                semantic_events_appended = semantic_events_appended
                    .checked_add(append.semantic_event_count().get())
                    .ok_or(RelationalStreamSliceError::CounterOverflow(
                        "semantic event count",
                    ))?;

                if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                    eprintln!(
                        "Explore stream: quantum={quantum:?}; events={}; elapsed={}ms; next_sequence={}",
                        append.semantic_event_count(),
                        compute_and_append_elapsed.as_millis(),
                        durable.journal()?.next_sequence(),
                    );
                }

                // Expensive base proposals target roughly five seconds and
                // are capped at ten. Flush the first accepted proposal and
                // then at a bounded cadence, rather than fsyncing every small
                // ramp-up proposal. This is operational only: it changes
                // neither the semantic events nor their identities.
                let expensive = base_quantum_controller
                    .observe_appended_quantum(quantum, compute_and_append_elapsed);
                let checkpoint_now = Instant::now();
                if expensive && base_quantum_controller.expensive_checkpoint_due(checkpoint_now) {
                    durable.flush_for_pause()?;
                    base_quantum_controller.mark_expensive_checkpoint(Instant::now());
                }

                if durable
                    .journal()?
                    .analysis_state()
                    .is_some_and(|analysis| analysis.is_closed())
                {
                    return complete(durable, semantic_batches_appended, semantic_events_appended);
                }
            }
            Ok(RelationalStreamStepOutcome::Quiescent(reason)) => {
                if let Err(error) = finish_relational_quantum(resources, in_flight) {
                    return resource_protocol_error(
                        durable,
                        semantic_batches_appended,
                        semantic_events_appended,
                        error,
                        None,
                    );
                }
                return pause(
                    durable,
                    semantic_batches_appended,
                    semantic_events_appended,
                    RelationalStreamSlicePauseReason::Semantic(reason),
                );
            }
            Ok(RelationalStreamStepOutcome::Complete) => {
                if let Err(error) = finish_relational_quantum(resources, in_flight) {
                    return resource_protocol_error(
                        durable,
                        semantic_batches_appended,
                        semantic_events_appended,
                        error,
                        None,
                    );
                }
                return complete(durable, semantic_batches_appended, semantic_events_appended);
            }
            Err(error) => {
                let finish_error = finish_relational_quantum(resources, in_flight).err();
                let progress =
                    flush_progress(durable, semantic_batches_appended, semantic_events_appended)?;
                return Err(RelationalStreamSliceError::SemanticRun {
                    source: error,
                    finish_error,
                    progress,
                });
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalQuantumIdentity {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
}

impl RelationalQuantumIdentity {
    const fn subject(self) -> ExactStreamWorkSubject {
        ExactStreamWorkSubject::RelationalJournalQuantum {
            expected_sequence: self.expected_sequence,
            expected_head: self.expected_head.bytes(),
        }
    }
}

enum RelationalQuantumAdmission {
    Granted(ExactStreamWorkInFlight),
    RuntimeLimit,
    ResourcePause(ExactStreamResourcePauseReason),
}

fn admit_relational_quantum(
    resources: &mut ExactStreamOneWorkerEnvelope,
    identity: RelationalQuantumIdentity,
    deadline: Option<Instant>,
) -> Result<RelationalQuantumAdmission, RelationalStreamResourceProtocolError> {
    let subject = identity.subject();
    loop {
        let now = Instant::now();
        if deadline.is_some_and(|deadline| now >= deadline) {
            return Ok(RelationalQuantumAdmission::RuntimeLimit);
        }

        let owned = resources.conservative_in_process_owned_snapshot();
        let poll = resources.poll(owned, None, Some(subject));
        match poll.action {
            ExactStreamResourceAction::Dispatch(permit) => {
                if permit.subject() != subject {
                    return Err(RelationalStreamResourceProtocolError::WrongDispatchSubject);
                }
                let in_flight = resources
                    .begin_work(permit)
                    .map_err(|error| permit_error("begin", error))?;
                if in_flight.subject() != subject {
                    let finish_error = resources.finish_or_abandon_work(in_flight).err();
                    return Err(finish_error.map_or(
                        RelationalStreamResourceProtocolError::WrongInFlightSubject,
                        |error| permit_error("finish", error),
                    ));
                }
                return Ok(RelationalQuantumAdmission::Granted(in_flight));
            }
            ExactStreamResourceAction::Pause(reason) => {
                return Ok(RelationalQuantumAdmission::ResourcePause(reason));
            }
            ExactStreamResourceAction::Wait(
                reason @ (ExactStreamResourcePauseReason::WaitingForWorkSubject
                | ExactStreamResourcePauseReason::InvalidWorkSubject
                | ExactStreamResourcePauseReason::PermitOutstanding
                | ExactStreamResourcePauseReason::WorkInFlight),
            ) => {
                return Err(RelationalStreamResourceProtocolError::UnexpectedWait {
                    code: reason.code(),
                });
            }
            ExactStreamResourceAction::Wait(_) => {
                let now = Instant::now();
                let fallback = now.checked_add(RESOURCE_WAIT_FALLBACK).unwrap_or(now);
                let mut wake = poll.next_host_sample_due.unwrap_or(fallback);
                if let Some(deadline) = deadline {
                    wake = wake.min(deadline);
                }
                if wake > now {
                    std::thread::sleep(wake.saturating_duration_since(now));
                } else {
                    std::thread::yield_now();
                }
            }
        }
    }
}

fn finish_relational_quantum(
    resources: &mut ExactStreamOneWorkerEnvelope,
    in_flight: ExactStreamWorkInFlight,
) -> Result<(), RelationalStreamResourceProtocolError> {
    resources
        .finish_or_abandon_work(in_flight)
        .map_err(|error| permit_error("finish", error))
}

fn permit_error(
    phase: &'static str,
    error: ExactStreamPermitError,
) -> RelationalStreamResourceProtocolError {
    let detail = match error {
        ExactStreamPermitError::Revoked => "permit revoked",
        ExactStreamPermitError::Expired => "permit expired",
        ExactStreamPermitError::WrongPermit => "wrong permit",
        ExactStreamPermitError::WorkAlreadyInFlight => "work already in flight",
        ExactStreamPermitError::WrongInFlightWork => "wrong in-flight work",
    };
    RelationalStreamResourceProtocolError::Permit { phase, detail }
}

fn pause<E>(
    durable: &mut RelationalDurableJournal,
    semantic_batches_appended: u64,
    semantic_events_appended: u64,
    reason: RelationalStreamSlicePauseReason,
) -> Result<RelationalStreamSliceOutcome, RelationalStreamSliceError<E>> {
    Ok(RelationalStreamSliceOutcome::Paused {
        progress: flush_progress(durable, semantic_batches_appended, semantic_events_appended)?,
        reason,
    })
}

fn complete<E>(
    durable: &mut RelationalDurableJournal,
    semantic_batches_appended: u64,
    semantic_events_appended: u64,
) -> Result<RelationalStreamSliceOutcome, RelationalStreamSliceError<E>> {
    Ok(RelationalStreamSliceOutcome::Complete {
        progress: flush_progress(durable, semantic_batches_appended, semantic_events_appended)?,
    })
}

fn flush_progress<E>(
    durable: &mut RelationalDurableJournal,
    semantic_batches_appended: u64,
    semantic_events_appended: u64,
) -> Result<RelationalStreamSliceProgress, RelationalStreamSliceError<E>> {
    Ok(RelationalStreamSliceProgress {
        checkpoint: durable.flush_for_pause()?,
        semantic_batches_appended,
        semantic_events_appended,
    })
}

fn resource_protocol_error<E>(
    durable: &mut RelationalDurableJournal,
    semantic_batches_appended: u64,
    semantic_events_appended: u64,
    source: RelationalStreamResourceProtocolError,
    finish_error: Option<RelationalStreamResourceProtocolError>,
) -> Result<RelationalStreamSliceOutcome, RelationalStreamSliceError<E>> {
    let progress = flush_progress(durable, semantic_batches_appended, semantic_events_appended)?;
    Err(RelationalStreamSliceError::ResourceProtocol {
        source,
        finish_error,
        progress,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelationalStreamResourceProtocolError {
    WrongDispatchSubject,
    WrongInFlightSubject,
    BatchAuthorityMismatch,
    UnexpectedWait {
        code: &'static str,
    },
    Permit {
        phase: &'static str,
        detail: &'static str,
    },
}

impl fmt::Display for RelationalStreamResourceProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDispatchSubject => {
                formatter.write_str("resource governor dispatched another work subject")
            }
            Self::WrongInFlightSubject => {
                formatter.write_str("consumed resource work names another subject")
            }
            Self::BatchAuthorityMismatch => formatter
                .write_str("relational semantic batch is not bound to its admitted journal prefix"),
            Self::UnexpectedWait { code } => {
                write!(
                    formatter,
                    "resource envelope is not at an admissible boundary: {code}"
                )
            }
            Self::Permit { phase, detail } => {
                write!(formatter, "resource permit {phase} failed: {detail}")
            }
        }
    }
}

impl Error for RelationalStreamResourceProtocolError {}

#[derive(Debug)]
pub(super) enum RelationalStreamSliceError<E> {
    RuntimeDeadlineOverflow,
    CounterOverflow(&'static str),
    ResourceInitialization {
        code: &'static str,
    },
    Durable(RelationalDurableJournalError),
    ResourceProtocol {
        source: RelationalStreamResourceProtocolError,
        finish_error: Option<RelationalStreamResourceProtocolError>,
        progress: RelationalStreamSliceProgress,
    },
    SemanticRun {
        source: RelationalStreamRunError<E>,
        finish_error: Option<RelationalStreamResourceProtocolError>,
        progress: RelationalStreamSliceProgress,
    },
}

impl<E> RelationalStreamSliceError<E> {
    /// Present only when this invocation could flush the exact semantic tail
    /// after the failure. A durable-storage failure deliberately exposes no
    /// newer cursor; recovery must reopen the last immutable prefix.
    pub(super) const fn progress(&self) -> Option<&RelationalStreamSliceProgress> {
        match self {
            Self::ResourceProtocol { progress, .. } | Self::SemanticRun { progress, .. } => {
                Some(progress)
            }
            Self::RuntimeDeadlineOverflow
            | Self::CounterOverflow(_)
            | Self::ResourceInitialization { .. }
            | Self::Durable(_) => None,
        }
    }
}

impl<E> From<RelationalDurableJournalError> for RelationalStreamSliceError<E> {
    fn from(error: RelationalDurableJournalError) -> Self {
        Self::Durable(error)
    }
}

impl<E: fmt::Display> fmt::Display for RelationalStreamSliceError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeDeadlineOverflow => {
                formatter.write_str("relational stream deadline exceeds the monotonic clock")
            }
            Self::CounterOverflow(counter) => {
                write!(formatter, "relational stream {counter} exceeds u64::MAX")
            }
            Self::ResourceInitialization { code } => {
                write!(
                    formatter,
                    "cannot initialize relational resource governor: {code}"
                )
            }
            Self::Durable(error) => fmt::Display::fmt(error, formatter),
            Self::ResourceProtocol {
                source,
                finish_error,
                ..
            } => {
                write!(formatter, "relational resource protocol failed: {source}")?;
                if let Some(finish_error) = finish_error {
                    write!(
                        formatter,
                        "; closing the work unit also failed: {finish_error}"
                    )?;
                }
                Ok(())
            }
            Self::SemanticRun {
                source,
                finish_error,
                ..
            } => {
                write!(formatter, "relational semantic quantum failed: {source}")?;
                if let Some(finish_error) = finish_error {
                    write!(
                        formatter,
                        "; closing the work unit also failed: {finish_error}"
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl<E: Error + 'static> Error for RelationalStreamSliceError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Durable(error) => Some(error),
            Self::ResourceProtocol { source, .. } => Some(source),
            Self::SemanticRun { source, .. } => Some(source),
            Self::RuntimeDeadlineOverflow
            | Self::CounterOverflow(_)
            | Self::ResourceInitialization { .. } => None,
        }
    }
}
