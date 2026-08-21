//! One-worker resource orchestration for a durable exact Explore stream.
//!
//! This module does not launch a worker and it does not duplicate resource
//! arithmetic. It turns one complete host-provider transaction plus one
//! complete coordinator-owned process snapshot into the existing sampler
//! reducer and resource governor. The only execution authority it emits is a
//! short-lived, generation-bound capability for one explicit work subject.
//!
//! The worker ceilings are fixed at one. Consequently the governor's private
//! durable-shard evidence seam is not needed here: that evidence exists only
//! to authorize scale-up. Durable case/result installation remains a separate
//! coordinator responsibility.
//!
//! A v1 in-process coordinator may deliberately report the logical evaluator
//! as resident with aggregate RSS/CPU `Some(0)` only when the host sample
//! already includes the whole process's consumption. Zero then means “credit
//! none of this process back into headroom”, which is conservative. Such a run
//! must never invent a separable calibration peak: it remains in the cold,
//! `max(2 GiB, ceil(total RAM / 4))` one-worker mode and yields between bounded
//! work permits. This is admission, not hard containment; an unbounded probe
//! must fail closed until it is resumably sliced or isolated in a child.

use std::num::{NonZeroU16, NonZeroU64};
use std::time::{Duration, Instant};

use super::resource_governor::{
    CalibrationPeakEvidence, CompilerObservation, DecisionReason, EvaluatorObservation,
    GovernorDecision, GovernorPhase, HostCapacity, LeaseAuthority, LeaseGeneration, MemoryPressure,
    ResourceGovernor, ResourceGovernorError, ResourceGovernorEvent, ResourcePolicy, ResourceSample,
    StabilityEpoch, SwapAssessment, TelemetryCursor,
};
#[cfg(target_os = "macos")]
use super::resource_sampler::{HostFactProvider, MacOsCommandProvider};
use super::resource_sampler::{
    OwnedProcessSnapshot, RawHostFacts, RawHostSample, ReducedResourceSample, ReducerEpochSeed,
    SampleUnavailable, SamplerWatchdog, StabilityWindowReducer,
};

const SAMPLE_DEADLINE: Duration = Duration::from_secs(3);
const SAMPLE_CADENCE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactStreamResourcePauseReason {
    InvalidConfiguration,
    UnsupportedPlatform,
    TelemetryUnavailable,
    IncoherentTelemetry,
    HostCapacityChanged,
    WaitingForSwapBaseline,
    WaitingForStableWindow,
    WaitingForWorkerReconciliation,
    WaitingForCalibrationPeak,
    WaitingForWorkSubject,
    InvalidWorkSubject,
    Draining,
    ResourceBackoff,
    GovernorFailed,
    RuntimeLimit,
    PermitOutstanding,
    WorkInFlight,
}

impl ExactStreamResourcePauseReason {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::TelemetryUnavailable => "telemetry_unavailable",
            Self::IncoherentTelemetry => "incoherent_telemetry",
            Self::HostCapacityChanged => "host_capacity_changed",
            Self::WaitingForSwapBaseline => "waiting_for_swap_baseline",
            Self::WaitingForStableWindow => "waiting_for_stable_window",
            Self::WaitingForWorkerReconciliation => "waiting_for_worker_reconciliation",
            Self::WaitingForCalibrationPeak => "waiting_for_calibration_peak",
            Self::WaitingForWorkSubject => "waiting_for_work_subject",
            Self::InvalidWorkSubject => "invalid_work_subject",
            Self::Draining => "draining",
            Self::ResourceBackoff => "resource_backoff",
            Self::GovernorFailed => "governor_failed",
            Self::RuntimeLimit => "runtime_limit",
            Self::PermitOutstanding => "permit_outstanding",
            Self::WorkInFlight => "work_in_flight",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactStreamResourceEpochSeed {
    pub(super) source_generation: NonZeroU64,
    pub(super) telemetry_epoch: NonZeroU64,
    pub(super) stability_epoch: NonZeroU64,
}

impl ExactStreamResourceEpochSeed {
    pub(super) fn initial() -> Result<Self, ExactStreamResourcePauseReason> {
        let one = NonZeroU64::new(1).ok_or(ExactStreamResourcePauseReason::InvalidConfiguration)?;
        Ok(Self {
            source_generation: one,
            telemetry_epoch: one,
            stability_epoch: one,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PermitIdentity {
    sequence: u64,
    subject: ExactStreamWorkSubject,
    purpose: ExactStreamWorkPurpose,
    lease_generation: LeaseGeneration,
    telemetry_cursor: TelemetryCursor,
    stability_epoch: StabilityEpoch,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactStreamWorkSubject {
    /// One bounded stream-open preparation phase: typecheck plus authenticated
    /// replay. It is admitted only under the conservative calibration policy.
    PreparationPhase,
    /// One bounded source-probe phase unit: initial analysis/manifest publish,
    /// manifest-backed coverage acceptance, or completion marker publication.
    ProbePhase,
    /// The explicitly requested atomic v1 terminal phase: fresh replay of the
    /// complete selected witness set, full answer publication, and sealing.
    /// Its retained replay bodies are hard-capped at 65,536 observations /
    /// 32 MiB; a process
    /// kill abandons the unit and durable recovery retries from its last
    /// committed replay/publication/seal boundary.
    FinalizationPhase,
    /// One canonical mixed-radix CaseId rank.
    CaseIdRank(u128),
    /// One deterministic candidate-first batch beginning at `first_rank` and
    /// evaluating no more than `case_cap` whole CaseIds.
    BoundedCaseIdBatch {
        first_rank: u128,
        case_cap: NonZeroU16,
    },
    /// One bounded batch drawn only from the durable source-probe candidate
    /// manifest; residual frontier ranks are not authorized by this subject.
    ProbeCandidateBatch {
        first_rank: u128,
        case_cap: NonZeroU16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactStreamWorkPurpose {
    /// Representative work while the conservative cold charge is active.
    Calibration,
    /// Normal residual work after a measured calibration was sealed.
    Scan,
}

/// A non-cloneable capability for beginning exactly one whole work subject.
///
/// The coordinator must move this value into
/// [`ExactStreamOneWorkerEnvelope::begin_work`]. It is bound to
/// the explicit subject selected by the coordinator, the current worker
/// lease generation, and the exact stable telemetry observation that admitted
/// it. No permit remains valid at or after `expires_at`.
#[derive(Debug)]
pub(super) struct ExactStreamWorkDispatchPermit {
    identity: PermitIdentity,
}

impl ExactStreamWorkDispatchPermit {
    pub(super) const fn subject(&self) -> ExactStreamWorkSubject {
        self.identity.subject
    }

    pub(super) const fn case_id_rank(&self) -> Option<u128> {
        match self.identity.subject {
            ExactStreamWorkSubject::CaseIdRank(rank) => Some(rank),
            ExactStreamWorkSubject::PreparationPhase
            | ExactStreamWorkSubject::ProbePhase
            | ExactStreamWorkSubject::FinalizationPhase
            | ExactStreamWorkSubject::BoundedCaseIdBatch { .. }
            | ExactStreamWorkSubject::ProbeCandidateBatch { .. } => None,
        }
    }

    pub(super) const fn first_case_id_rank(&self) -> Option<u128> {
        match self.identity.subject {
            ExactStreamWorkSubject::CaseIdRank(rank) => Some(rank),
            ExactStreamWorkSubject::BoundedCaseIdBatch { first_rank, .. }
            | ExactStreamWorkSubject::ProbeCandidateBatch { first_rank, .. } => Some(first_rank),
            ExactStreamWorkSubject::PreparationPhase
            | ExactStreamWorkSubject::ProbePhase
            | ExactStreamWorkSubject::FinalizationPhase => None,
        }
    }

    pub(super) const fn lease_generation(&self) -> LeaseGeneration {
        self.identity.lease_generation
    }

    pub(super) const fn purpose(&self) -> ExactStreamWorkPurpose {
        self.identity.purpose
    }

    pub(super) const fn telemetry_cursor(&self) -> TelemetryCursor {
        self.identity.telemetry_cursor
    }

    pub(super) const fn stability_epoch(&self) -> StabilityEpoch {
        self.identity.stability_epoch
    }

    pub(super) const fn expires_at(&self) -> Instant {
        self.identity.expires_at
    }
}

/// Linear token retained across one whole admitted work subject.
#[derive(Debug)]
pub(super) struct ExactStreamWorkInFlight {
    identity: PermitIdentity,
}

impl ExactStreamWorkInFlight {
    pub(super) const fn subject(&self) -> ExactStreamWorkSubject {
        self.identity.subject
    }

    pub(super) const fn case_id_rank(&self) -> Option<u128> {
        match self.identity.subject {
            ExactStreamWorkSubject::CaseIdRank(rank) => Some(rank),
            ExactStreamWorkSubject::PreparationPhase
            | ExactStreamWorkSubject::ProbePhase
            | ExactStreamWorkSubject::FinalizationPhase
            | ExactStreamWorkSubject::BoundedCaseIdBatch { .. }
            | ExactStreamWorkSubject::ProbeCandidateBatch { .. } => None,
        }
    }

    pub(super) const fn first_case_id_rank(&self) -> Option<u128> {
        match self.identity.subject {
            ExactStreamWorkSubject::CaseIdRank(rank) => Some(rank),
            ExactStreamWorkSubject::BoundedCaseIdBatch { first_rank, .. }
            | ExactStreamWorkSubject::ProbeCandidateBatch { first_rank, .. } => Some(first_rank),
            ExactStreamWorkSubject::PreparationPhase
            | ExactStreamWorkSubject::ProbePhase
            | ExactStreamWorkSubject::FinalizationPhase => None,
        }
    }

    pub(super) const fn lease_generation(&self) -> LeaseGeneration {
        self.identity.lease_generation
    }

    pub(super) const fn purpose(&self) -> ExactStreamWorkPurpose {
        self.identity.purpose
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactStreamPermitError {
    Revoked,
    Expired,
    WrongPermit,
    WorkAlreadyInFlight,
    WrongInFlightWork,
}

#[derive(Debug)]
pub(super) enum ExactStreamResourceAction {
    Dispatch(ExactStreamWorkDispatchPermit),
    /// No safety transition occurred; the caller already owns the referenced
    /// permit/work unit or has not supplied the next subject yet.
    Wait(ExactStreamResourcePauseReason),
    Pause(ExactStreamResourcePauseReason),
}

/// One coordinator directive. `target_worker_leases` and
/// `lease_generation` are the worker reconciliation contract; this adapter
/// never starts, signals, or kills the worker itself.
#[derive(Debug)]
pub(super) struct ExactStreamResourcePoll {
    pub(super) target_worker_leases: u16,
    pub(super) lease_generation: LeaseGeneration,
    /// Scheduling hint only. The outer max-runtime deadline always wins.
    pub(super) next_host_sample_due: Option<Instant>,
    pub(super) action: ExactStreamResourceAction,
    pub(super) governor_decision: Option<GovernorDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CalibrationPeak {
    lease_generation: LeaseGeneration,
    rss_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedAdmission {
    purpose: ExactStreamWorkPurpose,
    lease_generation: LeaseGeneration,
    telemetry_cursor: TelemetryCursor,
    stability_epoch: StabilityEpoch,
    expires_at: Instant,
}

/// Stateful resource boundary for one synchronous evaluator worker.
///
/// `poll` is called only between admitted work units. During calibration the coordinator
/// supplies a measured process high-water RSS; it is never inferred from a
/// current RSS sample. Once work starts, the whole subject is atomic. The
/// coordinator must finish or abandon its in-flight token before polling for
/// another work unit.
pub(super) struct ExactStreamOneWorkerEnvelope {
    policy: ResourcePolicy,
    source_generation: NonZeroU64,
    reducer: StabilityWindowReducer,
    watchdog: SamplerWatchdog,
    governor: Option<ResourceGovernor>,
    host_capacity: Option<HostCapacity>,
    clock_origin: Instant,
    last_observed_at_millis: Option<u64>,
    last_sample_started: Option<Instant>,
    calibration_peak: Option<CalibrationPeak>,
    cached_admission: Option<CachedAdmission>,
    outstanding_permit: Option<PermitIdentity>,
    in_flight: Option<PermitIdentity>,
    next_permit_sequence: u64,
    terminal_reason: Option<ExactStreamResourcePauseReason>,
    revoked_lease_generation: Option<LeaseGeneration>,
    #[cfg(target_os = "macos")]
    provider: MacOsCommandProvider,
}

impl ExactStreamOneWorkerEnvelope {
    pub(super) fn new() -> Result<Self, ExactStreamResourcePauseReason> {
        Self::with_epoch_seed(ExactStreamResourceEpochSeed::initial()?)
    }

    pub(super) fn with_epoch_seed(
        seed: ExactStreamResourceEpochSeed,
    ) -> Result<Self, ExactStreamResourcePauseReason> {
        let mut policy = ResourcePolicy::default();
        // The default reserve divisors independently retain at least 20% of
        // installed/live CPU and physical RAM. These caps cannot be raised by
        // this adapter, and the general governor supplies the conservative
        // max(2 GiB, ceil(total RAM / 4)) pre-calibration worker charge.
        policy.configured_worker_ceiling = Some(1);
        policy.requested_jobs_ceiling = Some(1);
        let watchdog = SamplerWatchdog::new(SAMPLE_CADENCE, SAMPLE_DEADLINE)
            .map_err(|_| ExactStreamResourcePauseReason::InvalidConfiguration)?;
        Ok(Self {
            policy,
            source_generation: seed.source_generation,
            reducer: StabilityWindowReducer::new(ReducerEpochSeed {
                telemetry: seed.telemetry_epoch,
                stability: seed.stability_epoch,
            }),
            watchdog,
            governor: None,
            host_capacity: None,
            clock_origin: Instant::now(),
            last_observed_at_millis: None,
            last_sample_started: None,
            calibration_peak: None,
            cached_admission: None,
            outstanding_permit: None,
            in_flight: None,
            next_permit_sequence: 0,
            terminal_reason: None,
            revoked_lease_generation: None,
            #[cfg(target_os = "macos")]
            provider: MacOsCommandProvider::default(),
        })
    }

    pub(super) fn target_worker_leases(&self) -> u16 {
        if self.terminal_reason.is_some() {
            0
        } else {
            self.governor
                .as_ref()
                .map(ResourceGovernor::target_worker_leases)
                .unwrap_or(0)
        }
    }

    pub(super) fn lease_generation(&self) -> LeaseGeneration {
        self.revoked_lease_generation.unwrap_or_else(|| {
            self.governor
                .as_ref()
                .map(ResourceGovernor::lease_generation)
                .unwrap_or(LeaseGeneration(1))
        })
    }

    /// Complete zero-credit attribution for an in-process v1 coordinator.
    ///
    /// Use only when the host provider's availability and idleness already
    /// include the whole process. Keep `measured_calibration_peak_rss_bytes`
    /// as `None`; this mode intentionally remains conservatively uncalibrated.
    pub(super) fn conservative_in_process_owned_snapshot(&self) -> OwnedProcessSnapshot {
        let resident_workers = self.target_worker_leases();
        OwnedProcessSnapshot {
            evaluator: EvaluatorObservation {
                lease_generation: self.lease_generation(),
                resident_workers,
                draining_workers: 0,
                reserved_workers: 0,
                aggregate_rss_bytes: Some(0),
                aggregate_cpu_millicores: Some(0),
            },
            compiler: CompilerObservation {
                rss_bytes: Some(0),
                cpu_millicores: Some(0),
            },
            compile_epoch: None,
        }
    }

    /// Revoke all local authority at an outer runtime boundary. Call this only
    /// between work units; an already-started subject remains atomic.
    pub(super) fn stop_at_work_boundary(&mut self) -> ExactStreamResourcePoll {
        if self.in_flight.is_some() {
            return self.wait(ExactStreamResourcePauseReason::WorkInFlight);
        }
        self.latch_terminal(ExactStreamResourcePauseReason::RuntimeLimit);
        self.pause(ExactStreamResourcePauseReason::RuntimeLimit)
    }

    /// Collect one due sample and advance the calibration/scan state machine,
    /// or reuse a still-fresh stable observation for the next work subject.
    ///
    /// `owned` must atomically account for every coordinator-owned evaluator
    /// and compiler process. `measured_calibration_peak_rss_bytes` is a true
    /// process high-water observation for completed representative calibration
    /// work in the current lease, not an estimate or an idle-worker RSS. The
    /// coordinator withholds it while requesting individual calibration-case
    /// permits, then supplies it once the representative calibration slice is
    /// complete. `next_work_subject` is required only when the caller wants a
    /// dispatch permit.
    pub(super) fn poll(
        &mut self,
        owned: OwnedProcessSnapshot,
        measured_calibration_peak_rss_bytes: Option<u64>,
        next_work_subject: Option<ExactStreamWorkSubject>,
    ) -> ExactStreamResourcePoll {
        let now = Instant::now();
        if let Some(reason) = self.terminal_reason {
            return self.pause(reason);
        }
        if self.in_flight.is_some() {
            return self.wait(ExactStreamResourcePauseReason::WorkInFlight);
        }
        // A caller may reach another work boundary before the host cadence is
        // due. Its fresh owned-process snapshot must still agree with the
        // cached authority. A resident/generation/compiler change forces an
        // immediate complete sample instead of granting from stale ownership.
        let ownership_changed = self
            .cached_admission
            .is_some_and(|cached| !owned_supports_cached_admission(owned, cached));
        if ownership_changed {
            self.revoke_dispatch_authority();
        }
        if let Some(outstanding) = self.outstanding_permit {
            if now < outstanding.expires_at {
                return self.wait(ExactStreamResourcePauseReason::PermitOutstanding);
            }
            self.revoke_dispatch_authority();
        }

        let calibration_confirmation = measured_calibration_peak_rss_bytes.is_some()
            && self
                .governor
                .as_ref()
                .is_some_and(|governor| governor.phase() == GovernorPhase::CalibratingOneWorker);
        let due = ownership_changed
            || calibration_confirmation
            || match self.last_sample_started {
                None => true,
                Some(last) => match self.watchdog.is_due(last, now) {
                    Ok(due) => due,
                    Err(_) => {
                        return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry)
                    }
                },
            };
        if !due {
            return self.dispatch_or_wait(now, next_work_subject);
        }

        // Every fresh sample invalidates authority derived from the preceding
        // cursor before collection starts. Provider failure, unknown facts,
        // pressure, swap growth/reset, or governor backoff cannot leave an old
        // permit live.
        self.revoke_dispatch_authority();
        self.last_sample_started = Some(now);
        let deadline = match self.watchdog.deadline(now) {
            Ok(deadline) => deadline,
            Err(_) => return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry),
        };
        let host_result = collect_complete_host_facts(self, deadline);
        let provider_unavailable = host_result.is_err();
        let completed = Instant::now();
        let elapsed_millis = match u64::try_from(
            completed
                .saturating_duration_since(self.clock_origin)
                .as_millis(),
        ) {
            Ok(value) => value,
            Err(_) => return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry),
        };
        let observed_at_millis = match self.last_observed_at_millis {
            Some(previous) => match previous.checked_add(1) {
                Some(next) => elapsed_millis.max(next),
                None => return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry),
            },
            None => elapsed_millis,
        };
        let raw = RawHostSample::from_provider_result(
            self.source_generation,
            observed_at_millis,
            host_result,
            owned,
        );
        let reduced = match self.reducer.reduce(raw) {
            Ok(reduced) => reduced,
            Err(_) => return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry),
        };
        self.last_observed_at_millis = Some(observed_at_millis);

        if let Some(expected) = self.host_capacity {
            if capacity_is_complete(reduced.capacity) && reduced.capacity != expected {
                return self.fail(ExactStreamResourcePauseReason::HostCapacityChanged);
            }
        } else if !reduced.force_zero_admission && capacity_is_complete(reduced.capacity) {
            let governor = match ResourceGovernor::new(reduced.capacity, self.policy) {
                Ok(governor) => governor,
                Err(_) => return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry),
            };
            self.host_capacity = Some(reduced.capacity);
            self.governor = Some(governor);
        }

        let (governor_phase, governor_lease_generation) = match self.governor.as_ref() {
            Some(governor) => (governor.phase(), governor.lease_generation()),
            None => {
                let reason = if platform_supported() {
                    ExactStreamResourcePauseReason::TelemetryUnavailable
                } else {
                    ExactStreamResourcePauseReason::UnsupportedPlatform
                };
                return self.pause(reason);
            }
        };
        self.update_calibration_peak(
            governor_phase,
            governor_lease_generation,
            reduced.sample,
            measured_calibration_peak_rss_bytes,
        );

        let calibration_peak = self.calibration_peak;
        let decision = match self.governor.as_mut() {
            Some(governor) => {
                match drive_governor(governor, self.policy, reduced.sample, calibration_peak) {
                    Ok(decision) => decision,
                    Err(_) => return self.fail(ExactStreamResourcePauseReason::GovernorFailed),
                }
            }
            None => return self.fail(ExactStreamResourcePauseReason::GovernorFailed),
        };
        if decision.phase != GovernorPhase::CalibratingOneWorker {
            self.calibration_peak = None;
        }

        let admission_purpose = if decision_allows_scan_case(decision) {
            Some(ExactStreamWorkPurpose::Scan)
        } else if self.calibration_peak.is_none() && decision_allows_calibration_case(decision) {
            Some(ExactStreamWorkPurpose::Calibration)
        } else {
            None
        };
        if let Some(purpose) = admission_purpose.filter(|_| !reduced.force_zero_admission) {
            let expires_at = match self.watchdog.next_due(now) {
                Ok(deadline) if completed < deadline => deadline,
                _ => return self.pause(ExactStreamResourcePauseReason::TelemetryUnavailable),
            };
            let (Some(cursor), Some(stability_epoch)) =
                (decision.metadata.cursor, decision.metadata.stability_epoch)
            else {
                return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry);
            };
            self.cached_admission = Some(CachedAdmission {
                purpose,
                lease_generation: decision.metadata.lease_generation,
                telemetry_cursor: cursor,
                stability_epoch,
                expires_at,
            });
            return self.dispatch_or_wait(completed, next_work_subject);
        }

        self.revoke_dispatch_authority();
        let reason = pause_reason_for_decision(
            decision,
            reduced,
            provider_unavailable,
            self.calibration_peak,
        );
        if is_coordination_wait(reason) {
            self.wait(reason)
        } else {
            self.pause(reason)
        }
    }

    /// Consume one permit immediately before starting its work subject.
    pub(super) fn begin_work(
        &mut self,
        permit: ExactStreamWorkDispatchPermit,
    ) -> Result<ExactStreamWorkInFlight, ExactStreamPermitError> {
        if self.terminal_reason.is_some() {
            self.revoke_dispatch_authority();
            return Err(ExactStreamPermitError::Revoked);
        }
        if self.in_flight.is_some() {
            return Err(ExactStreamPermitError::WorkAlreadyInFlight);
        }
        let now = Instant::now();
        let Some(outstanding) = self.outstanding_permit else {
            return Err(ExactStreamPermitError::Revoked);
        };
        if outstanding != permit.identity {
            self.latch_terminal(ExactStreamResourcePauseReason::IncoherentTelemetry);
            return Err(ExactStreamPermitError::WrongPermit);
        }
        if now >= permit.identity.expires_at {
            self.revoke_dispatch_authority();
            return Err(ExactStreamPermitError::Expired);
        }
        if !self.cached_admission.is_some_and(|cached| {
            cached.purpose == permit.identity.purpose
                && cached.lease_generation == permit.identity.lease_generation
                && cached.telemetry_cursor == permit.identity.telemetry_cursor
                && cached.stability_epoch == permit.identity.stability_epoch
                && cached.expires_at == permit.identity.expires_at
        }) || !self.governor.as_ref().is_some_and(|governor| {
            decision_allows_case_for_purpose(governor.decision(), permit.identity.purpose)
        }) {
            self.latch_terminal(ExactStreamResourcePauseReason::IncoherentTelemetry);
            return Err(ExactStreamPermitError::Revoked);
        }
        self.outstanding_permit = None;
        self.in_flight = Some(permit.identity);
        Ok(ExactStreamWorkInFlight {
            identity: permit.identity,
        })
    }

    /// Close either a successful or abandoned atomic work unit. Resource safety
    /// does not assert that a semantic result was durably installed.
    pub(super) fn finish_or_abandon_work(
        &mut self,
        work: ExactStreamWorkInFlight,
    ) -> Result<(), ExactStreamPermitError> {
        if self.in_flight != Some(work.identity) {
            self.latch_terminal(ExactStreamResourcePauseReason::IncoherentTelemetry);
            return Err(ExactStreamPermitError::WrongInFlightWork);
        }
        self.in_flight = None;
        if matches!(
            work.identity.subject,
            ExactStreamWorkSubject::PreparationPhase
                | ExactStreamWorkSubject::ProbePhase
                | ExactStreamWorkSubject::FinalizationPhase
        ) {
            // Never carry pre-preparation/probe host headroom across a
            // potentially heavy phase. The next poll samples immediately.
            self.revoke_dispatch_authority();
            self.last_sample_started = None;
        }
        Ok(())
    }

    fn update_calibration_peak(
        &mut self,
        phase: GovernorPhase,
        lease_generation: LeaseGeneration,
        sample: ResourceSample,
        measured_peak: Option<u64>,
    ) {
        if phase != GovernorPhase::CalibratingOneWorker
            || sample.evaluator.lease_generation != lease_generation
        {
            return;
        }
        let Some(measured_peak) = measured_peak.filter(|value| *value != 0) else {
            return;
        };
        self.calibration_peak = Some(match self.calibration_peak {
            Some(previous) if previous.lease_generation == lease_generation => CalibrationPeak {
                lease_generation,
                rss_bytes: previous.rss_bytes.max(measured_peak),
            },
            _ => CalibrationPeak {
                lease_generation,
                rss_bytes: measured_peak,
            },
        });
    }

    fn dispatch_or_wait(
        &mut self,
        now: Instant,
        next_work_subject: Option<ExactStreamWorkSubject>,
    ) -> ExactStreamResourcePoll {
        let Some(cached) = self.cached_admission else {
            return self.wait(ExactStreamResourcePauseReason::WaitingForStableWindow);
        };
        if now >= cached.expires_at {
            self.revoke_dispatch_authority();
            return self.pause(ExactStreamResourcePauseReason::TelemetryUnavailable);
        }
        let Some(subject) = next_work_subject else {
            return self.wait(ExactStreamResourcePauseReason::WaitingForWorkSubject);
        };
        if !work_subject_allowed(cached.purpose, subject) {
            return self.wait(ExactStreamResourcePauseReason::InvalidWorkSubject);
        }
        let Some(sequence) = self.next_permit_sequence.checked_add(1) else {
            return self.fail(ExactStreamResourcePauseReason::IncoherentTelemetry);
        };
        self.next_permit_sequence = sequence;
        let identity = PermitIdentity {
            sequence,
            subject,
            purpose: cached.purpose,
            lease_generation: cached.lease_generation,
            telemetry_cursor: cached.telemetry_cursor,
            stability_epoch: cached.stability_epoch,
            expires_at: cached.expires_at,
        };
        self.outstanding_permit = Some(identity);
        self.directive(ExactStreamResourceAction::Dispatch(
            ExactStreamWorkDispatchPermit { identity },
        ))
    }

    fn revoke_dispatch_authority(&mut self) {
        self.cached_admission = None;
        self.outstanding_permit = None;
    }

    fn fail(&mut self, reason: ExactStreamResourcePauseReason) -> ExactStreamResourcePoll {
        self.latch_terminal(reason);
        self.pause(reason)
    }

    fn latch_terminal(&mut self, reason: ExactStreamResourcePauseReason) {
        if self.terminal_reason.is_none() {
            let current = self.lease_generation();
            self.revoked_lease_generation = Some(
                current
                    .0
                    .checked_add(1)
                    .map(LeaseGeneration)
                    .unwrap_or(current),
            );
            self.terminal_reason = Some(reason);
        }
        self.revoke_dispatch_authority();
    }

    fn pause(&self, reason: ExactStreamResourcePauseReason) -> ExactStreamResourcePoll {
        self.directive(ExactStreamResourceAction::Pause(reason))
    }

    fn wait(&self, reason: ExactStreamResourcePauseReason) -> ExactStreamResourcePoll {
        self.directive(ExactStreamResourceAction::Wait(reason))
    }

    fn directive(&self, action: ExactStreamResourceAction) -> ExactStreamResourcePoll {
        let governor_decision = self
            .terminal_reason
            .is_none()
            .then(|| self.governor.as_ref().map(ResourceGovernor::decision))
            .flatten();
        ExactStreamResourcePoll {
            target_worker_leases: self.target_worker_leases(),
            lease_generation: self.lease_generation(),
            next_host_sample_due: self
                .last_sample_started
                .and_then(|started| self.watchdog.next_due(started).ok()),
            action,
            governor_decision,
        }
    }
}

fn drive_governor(
    governor: &mut ResourceGovernor,
    policy: ResourcePolicy,
    sample: ResourceSample,
    calibration_peak: Option<CalibrationPeak>,
) -> Result<GovernorDecision, ResourceGovernorError> {
    let phase = governor.phase();
    let previous = governor.decision();
    let stable_transition_ready = previous.metadata.stable
        && sample_has_complete_stability_window(sample, policy.stable_window_millis);
    let event = match phase {
        GovernorPhase::Idle if governor.calibration().is_none() && stable_transition_ready => {
            ResourceGovernorEvent::BeginOneWorkerCalibration(sample)
        }
        GovernorPhase::Idle if governor.calibration().is_some() && stable_transition_ready => {
            ResourceGovernorEvent::BeginScan(sample)
        }
        GovernorPhase::CalibratingOneWorker
            if stable_transition_ready
                && sample_has_one_fully_active_worker(sample)
                && calibration_peak.is_some_and(|peak| {
                    peak.lease_generation == governor.lease_generation()
                        && sample
                            .evaluator
                            .aggregate_rss_bytes
                            .is_some_and(|rss| peak.rss_bytes >= rss)
                }) =>
        {
            let peak = calibration_peak.ok_or(ResourceGovernorError::InvalidCalibration(
                "calibration peak disappeared before transition",
            ))?;
            ResourceGovernorEvent::FinishOneWorkerCalibration {
                sample,
                evidence: CalibrationPeakEvidence {
                    lease_generation: peak.lease_generation,
                    measured_at: sample.cursor,
                    stability_epoch: sample.stability.epoch,
                    measured_peak_rss_bytes: peak.rss_bytes,
                },
            }
        }
        _ => ResourceGovernorEvent::Observe(sample),
    };
    match governor.transition(event) {
        Ok(decision) => Ok(decision),
        Err(error)
            if phase == GovernorPhase::Idle
                && matches!(
                    &error,
                    ResourceGovernorError::NotStable
                        | ResourceGovernorError::CapacityUnavailable
                        | ResourceGovernorError::ResidentsNotStopped
                        | ResourceGovernorError::PostCompileWindowNotEstablished
                ) =>
        {
            Ok(governor.decision())
        }
        Err(error) => Err(error),
    }
}

fn sample_has_complete_stability_window(sample: ResourceSample, required_millis: u64) -> bool {
    sample.pressure == MemoryPressure::Normal
        && !sample.oom_risk
        && sample.swap_out.is_some()
        && sample.compiler.rss_bytes == Some(0)
        && sample.compiler.cpu_millicores == Some(0)
        && sample.compile_epoch.is_none()
        && sample.stability.minimum_available_memory_bytes.is_some()
        && sample.stability.minimum_idle_cpu_millicores.is_some()
        && sample
            .stability
            .minimum_memory_before_evaluator_charge_bytes
            .is_some()
        && sample
            .stability
            .minimum_cpu_before_evaluator_charge_millicores
            .is_some()
        && sample
            .cursor
            .observed_at_millis
            .saturating_sub(sample.stability.stable_since_millis)
            >= required_millis
}

fn sample_has_one_fully_active_worker(sample: ResourceSample) -> bool {
    sample.evaluator.resident_workers == 1
        && sample.evaluator.draining_workers == 0
        && sample.evaluator.reserved_workers == 0
        && sample.evaluator.aggregate_rss_bytes.is_some()
        && sample.evaluator.aggregate_cpu_millicores.is_some()
}

fn capacity_is_complete(capacity: HostCapacity) -> bool {
    capacity.logical_cpu_count.is_some() && capacity.total_memory_bytes.is_some()
}

fn work_subject_allowed(purpose: ExactStreamWorkPurpose, subject: ExactStreamWorkSubject) -> bool {
    match (purpose, subject) {
        (ExactStreamWorkPurpose::Calibration, ExactStreamWorkSubject::PreparationPhase)
        | (ExactStreamWorkPurpose::Calibration, ExactStreamWorkSubject::ProbePhase)
        | (ExactStreamWorkPurpose::Calibration, ExactStreamWorkSubject::FinalizationPhase)
        | (ExactStreamWorkPurpose::Calibration, ExactStreamWorkSubject::CaseIdRank(_))
        | (
            ExactStreamWorkPurpose::Calibration,
            ExactStreamWorkSubject::BoundedCaseIdBatch { .. },
        )
        | (
            ExactStreamWorkPurpose::Calibration,
            ExactStreamWorkSubject::ProbeCandidateBatch { .. },
        )
        | (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::CaseIdRank(_))
        | (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::BoundedCaseIdBatch { .. })
        | (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::ProbeCandidateBatch { .. })
        | (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::FinalizationPhase) => true,
        (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::PreparationPhase)
        | (ExactStreamWorkPurpose::Scan, ExactStreamWorkSubject::ProbePhase) => false,
    }
}

fn owned_supports_cached_admission(owned: OwnedProcessSnapshot, cached: CachedAdmission) -> bool {
    owned.evaluator.lease_generation == cached.lease_generation
        && owned.evaluator.resident_workers == 1
        && owned.evaluator.draining_workers == 0
        && owned.evaluator.reserved_workers == 0
        && owned.evaluator.aggregate_rss_bytes.is_some()
        && owned.evaluator.aggregate_cpu_millicores.is_some()
        && owned.compiler.rss_bytes == Some(0)
        && owned.compiler.cpu_millicores == Some(0)
        && owned.compile_epoch.is_none()
}

fn decision_allows_case_for_purpose(
    decision: GovernorDecision,
    purpose: ExactStreamWorkPurpose,
) -> bool {
    match purpose {
        ExactStreamWorkPurpose::Calibration => decision_allows_calibration_case(decision),
        ExactStreamWorkPurpose::Scan => decision_allows_scan_case(decision),
    }
}

fn decision_allows_calibration_case(decision: GovernorDecision) -> bool {
    decision.phase == GovernorPhase::CalibratingOneWorker
        && (decision_allows_cold_calibration_case(decision)
            || decision_allows_one_resident_worker(decision))
}

fn decision_allows_scan_case(decision: GovernorDecision) -> bool {
    decision.phase == GovernorPhase::Scanning && decision_allows_one_resident_worker(decision)
}

fn decision_allows_one_resident_worker(decision: GovernorDecision) -> bool {
    decision_has_safe_one_worker_capacity(decision)
        && decision.metadata.observed_lease_generation == Some(decision.metadata.lease_generation)
        && decision.metadata.resident_workers == Some(1)
        && decision.metadata.draining_workers == Some(0)
        && decision.metadata.reserved_workers == Some(0)
}

fn decision_allows_cold_calibration_case(decision: GovernorDecision) -> bool {
    decision.reason == DecisionReason::CalibrationStarted
        && decision_has_safe_one_worker_capacity(decision)
        && decision.metadata.lease_observation_cutoff == decision.metadata.cursor
}

fn decision_has_safe_one_worker_capacity(decision: GovernorDecision) -> bool {
    decision.target_worker_leases == 1
        && decision.metadata.failure.is_none()
        && decision.metadata.lease_authority == LeaseAuthority::Active
        && decision.metadata.stable
        && decision.metadata.swap == SwapAssessment::Unchanged
        && decision.metadata.pressure == MemoryPressure::Normal
        && decision.metadata.capacity.is_some_and(|capacity| {
            capacity.telemetry_complete
                && capacity.safe_worker_ceiling >= 1
                && capacity.charged_worker_commitments <= capacity.safe_worker_ceiling
        })
}

fn pause_reason_for_decision(
    decision: GovernorDecision,
    reduced: ReducedResourceSample,
    provider_unavailable: bool,
    calibration_peak: Option<CalibrationPeak>,
) -> ExactStreamResourcePauseReason {
    if !platform_supported() {
        return ExactStreamResourcePauseReason::UnsupportedPlatform;
    }
    if provider_unavailable || reduced.force_zero_admission {
        return ExactStreamResourcePauseReason::TelemetryUnavailable;
    }
    if decision.phase == GovernorPhase::Failed
        || decision.metadata.failure.is_some()
        || decision.metadata.lease_authority == LeaseAuthority::Revoked
    {
        return ExactStreamResourcePauseReason::GovernorFailed;
    }
    match decision.metadata.swap {
        SwapAssessment::Unknown | SwapAssessment::Baseline => {
            return ExactStreamResourcePauseReason::WaitingForSwapBaseline
        }
        SwapAssessment::CounterReset | SwapAssessment::Growth => {
            return ExactStreamResourcePauseReason::ResourceBackoff
        }
        SwapAssessment::Unchanged => {}
    }
    if decision.metadata.pressure != MemoryPressure::Normal
        || matches!(
            decision.reason,
            DecisionReason::WarningBackoff
                | DecisionReason::CriticalBackoff
                | DecisionReason::UnknownPressureBackoff
                | DecisionReason::OomRiskBackoff
                | DecisionReason::ReserveBackoff
                | DecisionReason::CapacityLimited
                | DecisionReason::ColdCalibrationMemoryLimited
        )
    {
        return ExactStreamResourcePauseReason::ResourceBackoff;
    }
    match decision.phase {
        GovernorPhase::CalibratingOneWorker
            if decision.metadata.observed_lease_generation
                != Some(decision.metadata.lease_generation)
                || decision.metadata.resident_workers != Some(1)
                || decision.metadata.draining_workers != Some(0)
                || decision.metadata.reserved_workers != Some(0) =>
        {
            ExactStreamResourcePauseReason::WaitingForWorkerReconciliation
        }
        GovernorPhase::CalibratingOneWorker if !decision.metadata.stable => {
            ExactStreamResourcePauseReason::WaitingForStableWindow
        }
        GovernorPhase::CalibratingOneWorker
            if calibration_peak
                .filter(|peak| peak.lease_generation == decision.metadata.lease_generation)
                .is_none() =>
        {
            ExactStreamResourcePauseReason::WaitingForCalibrationPeak
        }
        GovernorPhase::CalibratingOneWorker => {
            ExactStreamResourcePauseReason::WaitingForStableWindow
        }
        GovernorPhase::Draining => ExactStreamResourcePauseReason::Draining,
        GovernorPhase::Scanning
            if decision.metadata.resident_workers != Some(1)
                || decision.metadata.draining_workers != Some(0)
                || decision.metadata.reserved_workers != Some(0) =>
        {
            ExactStreamResourcePauseReason::WaitingForWorkerReconciliation
        }
        GovernorPhase::Failed | GovernorPhase::Compiling { .. } => {
            ExactStreamResourcePauseReason::GovernorFailed
        }
        _ => ExactStreamResourcePauseReason::WaitingForStableWindow,
    }
}

fn is_coordination_wait(reason: ExactStreamResourcePauseReason) -> bool {
    matches!(
        reason,
        ExactStreamResourcePauseReason::WaitingForSwapBaseline
            | ExactStreamResourcePauseReason::WaitingForStableWindow
            | ExactStreamResourcePauseReason::WaitingForWorkerReconciliation
            | ExactStreamResourcePauseReason::WaitingForCalibrationPeak
            | ExactStreamResourcePauseReason::WaitingForWorkSubject
            | ExactStreamResourcePauseReason::InvalidWorkSubject
            | ExactStreamResourcePauseReason::Draining
            | ExactStreamResourcePauseReason::PermitOutstanding
            | ExactStreamResourcePauseReason::WorkInFlight
    )
}

#[cfg(target_os = "macos")]
fn collect_complete_host_facts(
    envelope: &mut ExactStreamOneWorkerEnvelope,
    deadline: Instant,
) -> Result<RawHostFacts, SampleUnavailable> {
    envelope.provider.collect(deadline)
}

#[cfg(not(target_os = "macos"))]
fn collect_complete_host_facts(
    _envelope: &mut ExactStreamOneWorkerEnvelope,
    _deadline: Instant,
) -> Result<RawHostFacts, SampleUnavailable> {
    Err(SampleUnavailable::ProviderUnavailable)
}

#[cfg(target_os = "macos")]
const fn platform_supported() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
const fn platform_supported() -> bool {
    false
}
