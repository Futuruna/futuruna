//! Deterministic resource admission for bounded Explore execution.
//!
//! This module is deliberately operational, not semantic.  It never chooses
//! cases or shards and it never reads the host.  A coordinator supplies atomic,
//! generation-bound telemetry samples and performs the requested lease changes.
//! The governor only computes a safe target and records enough evidence to
//! decide whether a later transition is fresh.  The default automatic policy
//! never budgets more than 80% of installed CPU or physical memory, and live
//! pressure, headroom, and swap evidence can reduce that budget immediately.

use std::cmp;
use std::error::Error;
use std::fmt;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const MILLICORES_PER_CORE: u64 = 1_000;
const MINIMUM_STABLE_WINDOW_MILLIS: u64 = 30_000;
const OUTER_CONTAINED_MINIMUM_STABLE_WINDOW_MILLIS: u64 = 5_000;
const MAX_RAMP_EVIDENCE_IDENTITIES: u16 = 64;
// Cold evaluator and compiler sizing remain more conservative than the host
// reserve: their unknown peak is charged at least one quarter of physical RAM.
const CONSERVATIVE_PHASE_CHARGE_DIVISOR: u64 = 4;

macro_rules! generation_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(pub(crate) u64);

        impl $name {
            fn validate(self, field: &'static str) -> Result<(), ResourceGovernorError> {
                if self.0 == 0 {
                    Err(ResourceGovernorError::ZeroGeneration(field))
                } else {
                    Ok(())
                }
            }

            fn next(self, field: &'static str) -> Result<Self, ResourceGovernorError> {
                self.0
                    .checked_add(1)
                    .map(Self)
                    .ok_or(ResourceGovernorError::GenerationOverflow(field))
            }
        }
    };
}

generation_type!(TelemetryEpoch);
generation_type!(StabilityEpoch);
generation_type!(LeaseGeneration);
generation_type!(CompileEpoch);
generation_type!(SwapOutGeneration);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TelemetryCursor {
    pub(crate) epoch: TelemetryEpoch,
    pub(crate) sequence: u64,
    pub(crate) observed_at_millis: u64,
}

impl TelemetryCursor {
    fn validate(self) -> Result<(), ResourceGovernorError> {
        self.epoch.validate("telemetry_epoch")?;
        if self.sequence == 0 {
            return Err(ResourceGovernorError::ZeroSequence);
        }
        Ok(())
    }

    fn is_strictly_after(self, previous: Self) -> bool {
        self.epoch > previous.epoch
            || (self.epoch == previous.epoch
                && self.sequence > previous.sequence
                && self.observed_at_millis > previous.observed_at_millis)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostCapacity {
    pub(crate) logical_cpu_count: Option<u16>,
    pub(crate) total_memory_bytes: Option<u64>,
}

impl HostCapacity {
    fn validate(self) -> Result<(), ResourceGovernorError> {
        if self.logical_cpu_count == Some(0) {
            return Err(ResourceGovernorError::InvalidHostCapacity(
                "logical_cpu_count must be positive when known",
            ));
        }
        if self.total_memory_bytes == Some(0) {
            return Err(ResourceGovernorError::InvalidHostCapacity(
                "total_memory_bytes must be positive when known",
            ));
        }
        Ok(())
    }
}

/// Authority for interpreting a host-global swap-out delta.
///
/// The strict default treats growth as a work-stopping pressure signal. The
/// advisory mode is valid only when an independently validated outer boundary
/// continuously enforces the process-group RSS, Rust heap, host-memory floor,
/// pressure, and throttling guards. Growth remains observable in decision
/// metadata in both modes; only its authority to stop otherwise-safe work
/// changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwapGrowthAuthority {
    StrictStandalone,
    ValidatedOuterContainmentAdvisory,
}

impl SwapGrowthAuthority {
    pub(crate) const fn admits(self, assessment: SwapAssessment) -> bool {
        match assessment {
            SwapAssessment::Unchanged => true,
            SwapAssessment::Growth => {
                matches!(self, Self::ValidatedOuterContainmentAdvisory)
            }
            SwapAssessment::Unknown | SwapAssessment::Baseline | SwapAssessment::CounterReset => {
                false
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourcePolicy {
    /// Immutable operator cap.  `None` means no additional configured cap.
    pub(crate) configured_worker_ceiling: Option<u16>,
    /// Immutable `--jobs`-style cap.  `None` means no requested cap.
    pub(crate) requested_jobs_ceiling: Option<u16>,
    /// Installed/live CPU fraction kept for the host.  Values above five are
    /// rejected so automatic work can never claim more than 80%.
    pub(crate) cpu_reserve_divisor: u16,
    /// Physical-memory fraction kept for the host, subject to the absolute
    /// reserve floor.  Values above five are rejected.
    pub(crate) memory_reserve_divisor: u16,
    pub(crate) minimum_memory_reserve_bytes: u64,
    pub(crate) minimum_worker_charge_bytes: u64,
    pub(crate) worker_cpu_charge_millicores: u32,
    pub(crate) minimum_cold_calibration_memory_charge_bytes: u64,
    pub(crate) minimum_compile_memory_charge_bytes: u64,
    pub(crate) minimum_compile_cpu_charge_millicores: u32,
    /// A validated outer supervisor's bounded per-worker admission charge.
    /// Presence means evaluator admission may remain live at macOS Warning
    /// pressure; the outer process-group boundary independently caps the
    /// epoch, retains untracked memory, and stops pressure escalation. It never
    /// relaxes compiler charge or pressure admission.
    pub(crate) outer_contained_cold_worker_memory_charge_bytes: Option<u64>,
    /// Whether host-global swap growth may remain advisory. The advisory
    /// variant is rejected unless the independently contained worker charge is
    /// also present.
    pub(crate) swap_growth_authority: SwapGrowthAuthority,
    pub(crate) stable_window_millis: u64,
    pub(crate) committed_shards_before_scale_up: u16,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            configured_worker_ceiling: None,
            requested_jobs_ceiling: None,
            // The operator-authorized automatic ceiling is 80% of installed
            // capacity. Preserve the fraction in millicores so a six-core host
            // reserves exactly 1.2 cores rather than accidentally turning the
            // authorized 80% ceiling into a 66% ceiling.
            cpu_reserve_divisor: 5,
            memory_reserve_divisor: 5,
            // Keep an absolute cushion on small hosts without silently turning
            // an 8 GiB host's authorized 20% reserve back into a 25% reserve.
            minimum_memory_reserve_bytes: GIB,
            minimum_worker_charge_bytes: 512 * MIB,
            worker_cpu_charge_millicores: 1_000,
            minimum_cold_calibration_memory_charge_bytes: 2 * GIB,
            minimum_compile_memory_charge_bytes: 2 * GIB,
            minimum_compile_cpu_charge_millicores: 1_000,
            outer_contained_cold_worker_memory_charge_bytes: None,
            swap_growth_authority: SwapGrowthAuthority::StrictStandalone,
            stable_window_millis: MINIMUM_STABLE_WINDOW_MILLIS,
            committed_shards_before_scale_up: 2,
        }
    }
}

impl ResourcePolicy {
    fn validate(self) -> Result<(), ResourceGovernorError> {
        if self.configured_worker_ceiling == Some(0) {
            return Err(ResourceGovernorError::InvalidPolicy(
                "configured_worker_ceiling cannot be zero",
            ));
        }
        if self.requested_jobs_ceiling == Some(0) {
            return Err(ResourceGovernorError::InvalidPolicy(
                "requested_jobs_ceiling cannot be zero",
            ));
        }
        if self.cpu_reserve_divisor == 0
            || self.memory_reserve_divisor == 0
            || self.cpu_reserve_divisor > 5
            || self.memory_reserve_divisor > 5
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "reserve divisors must preserve at least one fifth",
            ));
        }
        if self.minimum_memory_reserve_bytes == 0
            || self.minimum_worker_charge_bytes == 0
            || self.worker_cpu_charge_millicores == 0
            || self.minimum_cold_calibration_memory_charge_bytes == 0
            || self.minimum_compile_memory_charge_bytes == 0
            || self.minimum_compile_cpu_charge_millicores == 0
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "resource charges and reserve floors must be positive",
            ));
        }
        if self.outer_contained_cold_worker_memory_charge_bytes == Some(0) {
            return Err(ResourceGovernorError::InvalidPolicy(
                "outer-contained cold worker charge must be positive",
            ));
        }
        if self.swap_growth_authority == SwapGrowthAuthority::ValidatedOuterContainmentAdvisory
            && self
                .outer_contained_cold_worker_memory_charge_bytes
                .is_none()
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "advisory swap growth requires validated outer containment",
            ));
        }
        if self.minimum_memory_reserve_bytes < GIB
            || self.minimum_cold_calibration_memory_charge_bytes < 2 * GIB
            || self.minimum_compile_memory_charge_bytes < 2 * GIB
            || self.minimum_worker_charge_bytes < 512 * MIB
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "memory floors cannot be lower than the conservative defaults",
            ));
        }
        if self.worker_cpu_charge_millicores < 1_000
            || self.minimum_compile_cpu_charge_millicores < 1_000
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "worker and compile CPU charges cannot be below one core",
            ));
        }
        let minimum_stable_window = if self
            .outer_contained_cold_worker_memory_charge_bytes
            .is_some()
        {
            OUTER_CONTAINED_MINIMUM_STABLE_WINDOW_MILLIS
        } else {
            MINIMUM_STABLE_WINDOW_MILLIS
        };
        if self.stable_window_millis < minimum_stable_window {
            return Err(ResourceGovernorError::InvalidPolicy(
                "stable_window_millis is shorter than its containment mode permits",
            ));
        }
        if self.committed_shards_before_scale_up < 2
            || self.committed_shards_before_scale_up > MAX_RAMP_EVIDENCE_IDENTITIES
        {
            return Err(ResourceGovernorError::InvalidPolicy(
                "committed shards before scale-up must be between two and 64",
            ));
        }
        Ok(())
    }

    pub(crate) const fn evaluator_pressure_is_admissible(self, pressure: MemoryPressure) -> bool {
        match pressure {
            MemoryPressure::Normal => true,
            MemoryPressure::Warning => self
                .outer_contained_cold_worker_memory_charge_bytes
                .is_some(),
            MemoryPressure::Critical | MemoryPressure::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoryPressure {
    Normal,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuHeadroom {
    /// Capacity currently available to this coordinator, after cgroup or VM
    /// limits but before this governor's reserve.
    pub(crate) live_capacity_millicores: u32,
    /// Idle capacity at the same instant as the owned-process observation.
    pub(crate) idle_millicores: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SwapOutCounter {
    pub(crate) generation: SwapOutGeneration,
    /// Monotonic bytes swapped out within `generation`; this is not swap
    /// occupancy and must never decrease within a generation.
    pub(crate) cumulative_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StabilityObservation {
    pub(crate) epoch: StabilityEpoch,
    pub(crate) stable_since_millis: u64,
    /// Raw available-memory low-water mark.  This proves the fixed host
    /// reserve survived even while old workers were still draining.
    pub(crate) minimum_available_memory_bytes: Option<u64>,
    /// Raw idle-CPU low-water mark for the same purpose.
    pub(crate) minimum_idle_cpu_millicores: Option<u32>,
    /// Minimum of `available_memory + aggregate_owned_evaluator_rss` observed
    /// throughout this external stability epoch.
    pub(crate) minimum_memory_before_evaluator_charge_bytes: Option<u64>,
    /// Minimum of `idle_cpu + aggregate_owned_evaluator_cpu` observed
    /// throughout this external stability epoch.
    pub(crate) minimum_cpu_before_evaluator_charge_millicores: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvaluatorObservation {
    pub(crate) lease_generation: LeaseGeneration,
    /// Processes that still consume resources, including draining processes.
    pub(crate) resident_workers: u16,
    pub(crate) draining_workers: u16,
    /// Granted leases whose process is not resident yet.
    pub(crate) reserved_workers: u16,
    /// Aggregate usage of every resident evaluator process in this observation.
    pub(crate) aggregate_rss_bytes: Option<u64>,
    pub(crate) aggregate_cpu_millicores: Option<u32>,
}

impl EvaluatorObservation {
    fn active_workers(self) -> Result<u16, ResourceGovernorError> {
        self.resident_workers
            .checked_sub(self.draining_workers)
            .ok_or(ResourceGovernorError::InvalidResidentObservation(
                "draining_workers exceeds resident_workers",
            ))
    }

    fn accounted_target(self) -> Result<u16, ResourceGovernorError> {
        self.active_workers()?
            .checked_add(self.reserved_workers)
            .ok_or(ResourceGovernorError::ArithmeticOverflow(
                "active plus reserved workers",
            ))
    }

    fn live_and_reserved_commitments(self) -> Result<u16, ResourceGovernorError> {
        self.resident_workers
            .checked_add(self.reserved_workers)
            .ok_or(ResourceGovernorError::ArithmeticOverflow(
                "resident plus reserved worker commitments",
            ))
    }

    fn is_fully_active(self, target: u16) -> bool {
        self.resident_workers == target && self.draining_workers == 0 && self.reserved_workers == 0
    }

    fn is_fully_stopped(self) -> bool {
        self.resident_workers == 0
            && self.draining_workers == 0
            && self.reserved_workers == 0
            && self.aggregate_rss_bytes == Some(0)
            && self.aggregate_cpu_millicores == Some(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompilerObservation {
    pub(crate) rss_bytes: Option<u64>,
    pub(crate) cpu_millicores: Option<u32>,
}

impl CompilerObservation {
    fn is_known_zero(self) -> bool {
        self.rss_bytes == Some(0) && self.cpu_millicores == Some(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceSample {
    pub(crate) cursor: TelemetryCursor,
    pub(crate) stability: StabilityObservation,
    /// `Some` only for samples atomically captured inside that compile epoch.
    pub(crate) compile_epoch: Option<CompileEpoch>,
    pub(crate) pressure: MemoryPressure,
    pub(crate) oom_risk: bool,
    pub(crate) available_memory_bytes: Option<u64>,
    pub(crate) cpu: Option<CpuHeadroom>,
    pub(crate) swap_out: Option<SwapOutCounter>,
    pub(crate) evaluator: EvaluatorObservation,
    pub(crate) compiler: CompilerObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvaluatorCalibration {
    pub(crate) lease_generation: LeaseGeneration,
    pub(crate) measured_at: TelemetryCursor,
    pub(crate) stability_epoch: StabilityEpoch,
    pub(crate) measured_peak_rss_bytes: u64,
    pub(crate) charged_worker_memory_bytes: u64,
}

impl EvaluatorCalibration {
    fn from_measured_peak(
        evidence: CalibrationPeakEvidence,
        floor_bytes: u64,
    ) -> Result<Self, ResourceGovernorError> {
        let measured_peak_rss_bytes = evidence.measured_peak_rss_bytes;
        if measured_peak_rss_bytes == 0 {
            return Err(ResourceGovernorError::InvalidCalibration(
                "measured peak RSS must be positive",
            ));
        }
        let one_and_a_half = measured_peak_rss_bytes
            .checked_mul(3)
            .and_then(|value| value.checked_add(1))
            .map(|value| value / 2)
            .ok_or(ResourceGovernorError::ArithmeticOverflow(
                "1.5x evaluator RSS charge",
            ))?;
        Ok(Self {
            lease_generation: evidence.lease_generation,
            measured_at: evidence.measured_at,
            stability_epoch: evidence.stability_epoch,
            measured_peak_rss_bytes,
            charged_worker_memory_bytes: cmp::max(one_and_a_half, floor_bytes),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CalibrationPeakEvidence {
    pub(crate) lease_generation: LeaseGeneration,
    pub(crate) measured_at: TelemetryCursor,
    pub(crate) stability_epoch: StabilityEpoch,
    pub(crate) measured_peak_rss_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompileCharge {
    pub(crate) memory_bytes: u64,
    pub(crate) cpu_millicores: u32,
}

mod journal_boundary {
    use super::{LeaseGeneration, ResourceGovernorError, StabilityEpoch, TelemetryCursor};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct ShardIdentity([u8; 32]);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct JournalGeneration(u64);

    impl JournalGeneration {
        fn validate(self) -> Result<(), ResourceGovernorError> {
            if self.0 == 0 {
                Err(ResourceGovernorError::ZeroGeneration("journal_generation"))
            } else {
                Ok(())
            }
        }
    }

    /// Sealed durable-journal readback.  Production minting is intentionally
    /// unavailable until the journal adapter can supply one exact, immutable
    /// receipt containing every field below.  Crate peers can name and consume
    /// this type through the governor event, but cannot construct or relabel it.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct ShardCommitEvidence {
        lease_generation: LeaseGeneration,
        stability_epoch: StabilityEpoch,
        committed_at: TelemetryCursor,
        shard_identity: ShardIdentity,
        journal_generation: JournalGeneration,
        durable_commit_sequence: u64,
    }

    impl ShardCommitEvidence {
        pub(super) fn lease_generation(&self) -> LeaseGeneration {
            self.lease_generation
        }

        pub(super) fn stability_epoch(&self) -> StabilityEpoch {
            self.stability_epoch
        }

        pub(super) fn committed_at(&self) -> TelemetryCursor {
            self.committed_at
        }

        pub(super) fn shard_identity(&self) -> ShardIdentity {
            self.shard_identity
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(super) struct DurableCommitCursor {
        journal_generation: JournalGeneration,
        sequence: u64,
    }

    impl DurableCommitCursor {
        pub(super) fn from_evidence(
            evidence: &ShardCommitEvidence,
        ) -> Result<Self, ResourceGovernorError> {
            evidence.journal_generation.validate()?;
            if evidence.durable_commit_sequence == 0 || evidence.shard_identity.0 == [0; 32] {
                return Err(ResourceGovernorError::ShardEvidenceMismatch);
            }
            Ok(Self {
                journal_generation: evidence.journal_generation,
                sequence: evidence.durable_commit_sequence,
            })
        }

        #[cfg(test)]
        pub(super) fn sequence(self) -> u64 {
            self.sequence
        }
    }

    #[cfg(test)]
    #[derive(Debug, Clone, Copy)]
    struct CanaryReceiptRecord {
        lease_generation: LeaseGeneration,
        stability_epoch: StabilityEpoch,
        committed_at: TelemetryCursor,
        shard_identity: ShardIdentity,
        journal_generation: JournalGeneration,
        durable_commit_sequence: u64,
    }

    #[cfg(test)]
    impl CanaryReceiptRecord {
        fn readback(self) -> ShardCommitEvidence {
            ShardCommitEvidence {
                lease_generation: self.lease_generation,
                stability_epoch: self.stability_epoch,
                committed_at: self.committed_at,
                shard_identity: self.shard_identity,
                journal_generation: self.journal_generation,
                durable_commit_sequence: self.durable_commit_sequence,
            }
        }
    }

    /// Test-only append/readback canary.  It chooses the journal cursor itself
    /// and can reissue only a byte-for-byte record it previously stored.
    #[cfg(test)]
    pub(super) struct CanaryJournal {
        generation: JournalGeneration,
        next_sequence: u64,
        receipts: Vec<CanaryReceiptRecord>,
    }

    #[cfg(test)]
    impl CanaryJournal {
        pub(super) fn new() -> Self {
            Self {
                generation: JournalGeneration(1),
                next_sequence: 1,
                receipts: Vec::new(),
            }
        }

        pub(super) fn append_and_readback(
            &mut self,
            lease_generation: LeaseGeneration,
            stability_epoch: StabilityEpoch,
            committed_at: TelemetryCursor,
            shard_identity: [u8; 32],
        ) -> ShardCommitEvidence {
            assert_ne!(shard_identity, [0; 32]);
            let record = CanaryReceiptRecord {
                lease_generation,
                stability_epoch,
                committed_at,
                shard_identity: ShardIdentity(shard_identity),
                journal_generation: self.generation,
                durable_commit_sequence: self.next_sequence,
            };
            self.next_sequence = self.next_sequence.checked_add(1).unwrap();
            self.receipts.push(record);
            record.readback()
        }

        pub(super) fn last_receipt_index(&self) -> usize {
            self.receipts.len().checked_sub(1).unwrap()
        }

        pub(super) fn readback(&self, receipt_index: usize) -> ShardCommitEvidence {
            self.receipts[receipt_index].readback()
        }
    }
}

pub(crate) use journal_boundary::ShardCommitEvidence;
use journal_boundary::{DurableCommitCursor, ShardIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovernorPhase {
    Idle,
    CalibratingOneWorker,
    Scanning,
    Draining,
    Compiling { epoch: CompileEpoch },
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaseAuthority {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwapAssessment {
    Unknown,
    Baseline,
    CounterReset,
    Unchanged,
    Growth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecisionReason {
    Initialized,
    WaitingForSwapBaseline,
    WaitingForStableWindow,
    WaitingForResidents,
    WaitingForPostCompileWindow,
    ColdCalibrationMemoryLimited,
    CapacityLimited,
    Holding,
    CalibrationStarted,
    CalibrationMeasured,
    ScanStarted,
    ShardEvidenceRecorded,
    ScaledUpOneWorker,
    ScanEnded,
    WarningBackoff,
    CriticalBackoff,
    UnknownPressureBackoff,
    OomRiskBackoff,
    SwapGrowthBackoff,
    SwapCounterResetBackoff,
    UnknownTelemetryBackoff,
    LeaseOversubscriptionBackoff,
    DrainingWorkersBackoff,
    ReserveBackoff,
    Draining,
    DrainCompleted,
    CompileStarted,
    CompileFinished,
    CompileAborted,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovernorFailure {
    EvaluatorResidentDuringCompile,
    CompilerExceededCharge,
    CompilerTelemetryUnknown,
    CompilerOverlappedEvaluatorPhase,
    UnsafeTelemetryDuringCompile,
    IncoherentTelemetry,
    EventRejectedWhileWorkActive,
    ArithmeticOrGenerationOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CapacityAssessment {
    pub(crate) telemetry_complete: bool,
    pub(crate) memory_reserve_bytes: u64,
    pub(crate) cpu_reserve_millicores: u32,
    pub(crate) worker_memory_charge_bytes: u64,
    pub(crate) worker_cpu_charge_millicores: u32,
    pub(crate) current_memory_before_worker_charge_bytes: Option<u64>,
    pub(crate) current_cpu_before_worker_charge_millicores: Option<u32>,
    pub(crate) memory_worker_ceiling: u16,
    pub(crate) cpu_worker_ceiling: u16,
    pub(crate) policy_worker_ceiling: u16,
    pub(crate) charged_worker_commitments: u16,
    pub(crate) safe_worker_ceiling: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecisionMetadata {
    pub(crate) cursor: Option<TelemetryCursor>,
    pub(crate) stability_epoch: Option<StabilityEpoch>,
    pub(crate) stable_duration_millis: u64,
    pub(crate) stable: bool,
    pub(crate) swap: SwapAssessment,
    pub(crate) pressure: MemoryPressure,
    pub(crate) lease_generation: LeaseGeneration,
    pub(crate) lease_authority: LeaseAuthority,
    pub(crate) lease_observation_cutoff: Option<TelemetryCursor>,
    pub(crate) observed_lease_generation: Option<LeaseGeneration>,
    pub(crate) resident_workers: Option<u16>,
    pub(crate) draining_workers: Option<u16>,
    pub(crate) reserved_workers: Option<u16>,
    pub(crate) committed_shards_in_ramp_window: u16,
    pub(crate) calibration: Option<EvaluatorCalibration>,
    pub(crate) capacity: Option<CapacityAssessment>,
    pub(crate) post_compile_cutoff: Option<TelemetryCursor>,
    pub(crate) failure: Option<GovernorFailure>,
    pub(crate) drain_trigger: Option<DecisionReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GovernorDecision {
    pub(crate) phase: GovernorPhase,
    /// Desired leases, distinct from resident and draining processes.
    pub(crate) target_worker_leases: u16,
    pub(crate) reason: DecisionReason,
    pub(crate) metadata: DecisionMetadata,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceGovernorEvent {
    Observe(ResourceSample),
    BeginOneWorkerCalibration(ResourceSample),
    FinishOneWorkerCalibration {
        sample: ResourceSample,
        evidence: CalibrationPeakEvidence,
    },
    BeginScan(ResourceSample),
    CommitScanShard {
        sample: ResourceSample,
        evidence: ShardCommitEvidence,
    },
    EndScan,
    BeginCompile {
        epoch: CompileEpoch,
        charge: CompileCharge,
        sample: ResourceSample,
    },
    EndCompile {
        epoch: CompileEpoch,
        sample: ResourceSample,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceGovernorError {
    InvalidPolicy(&'static str),
    InvalidHostCapacity(&'static str),
    InvalidSample(&'static str),
    InvalidResidentObservation(&'static str),
    InvalidCalibration(&'static str),
    InvalidCompileCharge(&'static str),
    WrongPhase {
        expected: &'static str,
        actual: GovernorPhase,
    },
    ZeroGeneration(&'static str),
    GenerationOverflow(&'static str),
    ZeroSequence,
    NonMonotonicTelemetry,
    NonMonotonicStability,
    NonMonotonicSwapCounter,
    StaleResidentObservation {
        expected: LeaseGeneration,
        observed: LeaseGeneration,
    },
    StaleCompileEpoch,
    NotStable,
    CapacityUnavailable,
    ResidentsNotStopped,
    TargetNotReconciled,
    PostCompileWindowNotEstablished,
    ShardEvidenceMismatch,
    ArithmeticOverflow(&'static str),
    TerminalFailure(GovernorFailure),
}

impl fmt::Display for ResourceGovernorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(message) => write!(formatter, "invalid resource policy: {message}"),
            Self::InvalidHostCapacity(message) => {
                write!(formatter, "invalid host capacity: {message}")
            }
            Self::InvalidSample(message) => write!(formatter, "invalid resource sample: {message}"),
            Self::InvalidResidentObservation(message) => {
                write!(formatter, "invalid resident observation: {message}")
            }
            Self::InvalidCalibration(message) => {
                write!(formatter, "invalid evaluator calibration: {message}")
            }
            Self::InvalidCompileCharge(message) => {
                write!(formatter, "invalid compile charge: {message}")
            }
            Self::WrongPhase { expected, actual } => {
                write!(formatter, "expected {expected}, found phase {actual:?}")
            }
            Self::ZeroGeneration(field) => write!(formatter, "{field} cannot be zero"),
            Self::GenerationOverflow(field) => write!(formatter, "{field} overflowed"),
            Self::ZeroSequence => write!(formatter, "telemetry sequence cannot be zero"),
            Self::NonMonotonicTelemetry => write!(formatter, "telemetry is not monotonic"),
            Self::NonMonotonicStability => {
                write!(formatter, "stability evidence is not monotonic")
            }
            Self::NonMonotonicSwapCounter => {
                write!(formatter, "swap-out counter is not monotonic")
            }
            Self::StaleResidentObservation { expected, observed } => write!(
                formatter,
                "resident observation is for lease generation {observed:?}, expected {expected:?}"
            ),
            Self::StaleCompileEpoch => write!(formatter, "compile epoch is stale or mismatched"),
            Self::NotStable => write!(formatter, "the required stable telemetry window is absent"),
            Self::CapacityUnavailable => write!(formatter, "safe resource capacity is unavailable"),
            Self::ResidentsNotStopped => write!(formatter, "evaluator residents are not stopped"),
            Self::TargetNotReconciled => {
                write!(
                    formatter,
                    "resident leases have not reconciled to the target"
                )
            }
            Self::PostCompileWindowNotEstablished => {
                write!(
                    formatter,
                    "a fresh post-compile stability window is required"
                )
            }
            Self::ShardEvidenceMismatch => {
                write!(formatter, "shard evidence does not match current telemetry")
            }
            Self::ArithmeticOverflow(field) => {
                write!(formatter, "overflow while computing {field}")
            }
            Self::TerminalFailure(failure) => {
                write!(formatter, "resource governor failed closed: {failure:?}")
            }
        }
    }
}

impl Error for ResourceGovernorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RampWindow {
    lease_generation: LeaseGeneration,
    stability_epoch: StabilityEpoch,
    started_at_millis: u64,
    committed_shards: u16,
    counted_shard_identities: Vec<ShardIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompileSession {
    epoch: CompileEpoch,
    charge: CompileCharge,
    began_at: TelemetryCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompileCutoff {
    cursor: TelemetryCursor,
    stability_epoch: StabilityEpoch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SwapTracker {
    telemetry_epoch: TelemetryEpoch,
    counter: SwapOutCounter,
    baseline_at_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SwapCandidate {
    assessment: SwapAssessment,
    tracker: Option<SwapTracker>,
    observation_gap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IngestedSample {
    sample: ResourceSample,
    capacity: CapacityAssessment,
    stable: bool,
    swap: SwapAssessment,
}

#[derive(Debug)]
pub(crate) struct ResourceGovernor {
    host: HostCapacity,
    policy: ResourcePolicy,
    phase: GovernorPhase,
    target_worker_leases: u16,
    lease_generation: LeaseGeneration,
    lease_authority: LeaseAuthority,
    lease_observation_cutoff: Option<TelemetryCursor>,
    calibration: Option<EvaluatorCalibration>,
    last_cursor: Option<TelemetryCursor>,
    last_stability: Option<StabilityObservation>,
    last_swap: Option<SwapTracker>,
    swap_observation_gap: bool,
    blocked_stability_through: Option<StabilityEpoch>,
    last_observation: Option<EvaluatorObservation>,
    last_capacity: Option<CapacityAssessment>,
    last_stable: bool,
    last_stable_duration_millis: u64,
    last_swap_assessment: SwapAssessment,
    last_pressure: MemoryPressure,
    ramp: Option<RampWindow>,
    last_durable_commit: Option<DurableCommitCursor>,
    compile_session: Option<CompileSession>,
    last_compile_epoch: Option<CompileEpoch>,
    post_compile_cutoff: Option<CompileCutoff>,
    failure: Option<GovernorFailure>,
    drain_trigger: Option<DecisionReason>,
    last_reason: DecisionReason,
}

impl ResourceGovernor {
    pub(crate) fn new(
        host: HostCapacity,
        policy: ResourcePolicy,
    ) -> Result<Self, ResourceGovernorError> {
        host.validate()?;
        policy.validate()?;
        Ok(Self {
            host,
            policy,
            phase: GovernorPhase::Idle,
            target_worker_leases: 0,
            lease_generation: LeaseGeneration(1),
            lease_authority: LeaseAuthority::Active,
            lease_observation_cutoff: None,
            calibration: None,
            last_cursor: None,
            last_stability: None,
            last_swap: None,
            swap_observation_gap: false,
            blocked_stability_through: None,
            last_observation: None,
            last_capacity: None,
            last_stable: false,
            last_stable_duration_millis: 0,
            last_swap_assessment: SwapAssessment::Unknown,
            last_pressure: MemoryPressure::Unknown,
            ramp: None,
            last_durable_commit: None,
            compile_session: None,
            last_compile_epoch: None,
            post_compile_cutoff: None,
            failure: None,
            drain_trigger: None,
            last_reason: DecisionReason::Initialized,
        })
    }

    pub(crate) fn phase(&self) -> GovernorPhase {
        self.phase
    }

    pub(crate) fn target_worker_leases(&self) -> u16 {
        self.target_worker_leases
    }

    pub(crate) fn lease_generation(&self) -> LeaseGeneration {
        self.lease_generation
    }

    pub(crate) fn calibration(&self) -> Option<EvaluatorCalibration> {
        self.calibration
    }

    pub(crate) fn transition(
        &mut self,
        event: ResourceGovernorEvent,
    ) -> Result<GovernorDecision, ResourceGovernorError> {
        if let Some(failure) = self.failure {
            return Err(ResourceGovernorError::TerminalFailure(failure));
        }

        let work_was_active = self.work_may_be_active();
        let result = match event {
            ResourceGovernorEvent::Observe(sample) => self.observe(sample),
            ResourceGovernorEvent::BeginOneWorkerCalibration(sample) => {
                self.begin_one_worker_calibration(sample)
            }
            ResourceGovernorEvent::FinishOneWorkerCalibration { sample, evidence } => {
                self.finish_one_worker_calibration(sample, evidence)
            }
            ResourceGovernorEvent::BeginScan(sample) => self.begin_scan(sample),
            ResourceGovernorEvent::CommitScanShard { sample, evidence } => {
                self.commit_scan_shard(sample, evidence)
            }
            ResourceGovernorEvent::EndScan => self.end_scan(),
            ResourceGovernorEvent::BeginCompile {
                epoch,
                charge,
                sample,
            } => self.begin_compile(epoch, charge, sample),
            ResourceGovernorEvent::EndCompile { epoch, sample } => self.end_compile(epoch, sample),
        };
        if let Err(error) = result {
            if self.failure.is_none() {
                if matches!(
                    &error,
                    ResourceGovernorError::ArithmeticOverflow(_)
                        | ResourceGovernorError::GenerationOverflow(_)
                ) {
                    self.fail_closed(GovernorFailure::ArithmeticOrGenerationOverflow);
                } else if work_was_active || self.work_may_be_active() {
                    // Returning an event error must never leave a previously
                    // granted lease, drain, or compile epoch authoritative.
                    // This deliberately treats malformed/replayed evidence and
                    // caller ordering mistakes as adversarial once work exists.
                    self.fail_closed(GovernorFailure::EventRejectedWhileWorkActive);
                }
            }
            return Err(error);
        }
        Ok(self.decision())
    }

    pub(crate) fn decision(&self) -> GovernorDecision {
        let observation = self.last_observation;
        GovernorDecision {
            phase: self.phase,
            target_worker_leases: self.target_worker_leases,
            reason: self.last_reason,
            metadata: DecisionMetadata {
                cursor: self.last_cursor,
                stability_epoch: self.last_stability.map(|value| value.epoch),
                stable_duration_millis: self.last_stable_duration_millis,
                stable: self.last_stable,
                swap: self.last_swap_assessment,
                pressure: self.last_pressure,
                lease_generation: self.lease_generation,
                lease_authority: self.lease_authority,
                lease_observation_cutoff: self.lease_observation_cutoff,
                observed_lease_generation: observation.map(|value| value.lease_generation),
                resident_workers: observation.map(|value| value.resident_workers),
                draining_workers: observation.map(|value| value.draining_workers),
                reserved_workers: observation.map(|value| value.reserved_workers),
                committed_shards_in_ramp_window: self
                    .ramp
                    .as_ref()
                    .map(|value| value.committed_shards)
                    .unwrap_or(0),
                calibration: self.calibration,
                capacity: self.last_capacity,
                post_compile_cutoff: self.post_compile_cutoff.map(|value| value.cursor),
                failure: self.failure,
                drain_trigger: self.drain_trigger,
            },
        }
    }

    fn work_may_be_active(&self) -> bool {
        self.target_worker_leases > 0
            || matches!(
                self.phase,
                GovernorPhase::CalibratingOneWorker
                    | GovernorPhase::Scanning
                    | GovernorPhase::Draining
                    | GovernorPhase::Compiling { .. }
            )
            || self
                .last_observation
                .is_some_and(|observation| !observation.is_fully_stopped())
    }

    fn observe(&mut self, sample: ResourceSample) -> Result<(), ResourceGovernorError> {
        let ingested = self.ingest(sample)?;
        if matches!(self.phase, GovernorPhase::Compiling { .. }) {
            self.monitor_compile(&ingested);
            return Ok(());
        }
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        // A matching-generation zero observation may complete a drain even
        // when this sample is only a swap baseline.  It grants no new work.
        let was_draining = self.phase == GovernorPhase::Draining;
        self.maybe_finish_draining(ingested.sample.evaluator);
        let drain_completed = was_draining && self.phase == GovernorPhase::Idle;
        if self.apply_safety(&ingested)? {
            if self.phase == GovernorPhase::Draining {
                self.last_reason = DecisionReason::Draining;
            }
            return Ok(());
        }
        if drain_completed {
            self.last_reason = DecisionReason::DrainCompleted;
            return Ok(());
        }
        if self.phase == GovernorPhase::Draining {
            self.last_reason = DecisionReason::Draining;
            return Ok(());
        }
        self.maybe_open_post_compile_gate(&ingested);
        self.maybe_start_ramp_window(&ingested)?;
        self.last_reason = if self.last_stable {
            DecisionReason::Holding
        } else {
            DecisionReason::WaitingForStableWindow
        };
        Ok(())
    }

    fn begin_one_worker_calibration(
        &mut self,
        sample: ResourceSample,
    ) -> Result<(), ResourceGovernorError> {
        self.require_phase(GovernorPhase::Idle, "idle")?;
        let ingested = self.ingest(sample)?;
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        if self.apply_safety(&ingested)? {
            return Ok(());
        }
        self.maybe_open_post_compile_gate(&ingested);
        if self.post_compile_cutoff.is_some() {
            self.last_reason = DecisionReason::WaitingForPostCompileWindow;
            return Err(ResourceGovernorError::PostCompileWindowNotEstablished);
        }
        self.require_stable(&ingested)?;
        self.require_fully_stopped(ingested.sample.evaluator)?;
        if ingested.capacity.safe_worker_ceiling < 1 {
            self.last_reason = DecisionReason::ColdCalibrationMemoryLimited;
            return Err(ResourceGovernorError::CapacityUnavailable);
        }
        self.change_target(1, ingested.sample.cursor.observed_at_millis)?;
        self.phase = GovernorPhase::CalibratingOneWorker;
        self.last_reason = DecisionReason::CalibrationStarted;
        Ok(())
    }

    fn finish_one_worker_calibration(
        &mut self,
        sample: ResourceSample,
        evidence: CalibrationPeakEvidence,
    ) -> Result<(), ResourceGovernorError> {
        self.require_phase(
            GovernorPhase::CalibratingOneWorker,
            "one-worker calibration",
        )?;
        let ingested = self.ingest(sample)?;
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        if self.apply_safety(&ingested)? {
            return Ok(());
        }
        self.require_stable(&ingested)?;
        if !ingested.sample.evaluator.is_fully_active(1) {
            self.last_reason = DecisionReason::WaitingForResidents;
            return Err(ResourceGovernorError::TargetNotReconciled);
        }
        if evidence.lease_generation != self.lease_generation
            || evidence.measured_at != ingested.sample.cursor
            || evidence.stability_epoch != ingested.sample.stability.epoch
        {
            return Err(ResourceGovernorError::InvalidCalibration(
                "peak evidence does not match the calibration worker sample",
            ));
        }
        let observed_rss = ingested
            .sample
            .evaluator
            .aggregate_rss_bytes
            .ok_or(ResourceGovernorError::CapacityUnavailable)?;
        if evidence.measured_peak_rss_bytes < observed_rss {
            return Err(ResourceGovernorError::InvalidCalibration(
                "measured peak is below current aggregate evaluator RSS",
            ));
        }
        let calibration = EvaluatorCalibration::from_measured_peak(
            evidence,
            self.policy.minimum_worker_charge_bytes,
        )?;
        self.change_target(0, ingested.sample.cursor.observed_at_millis)?;
        self.calibration = Some(calibration);
        self.phase = GovernorPhase::Draining;
        self.drain_trigger = Some(DecisionReason::CalibrationMeasured);
        self.last_reason = DecisionReason::CalibrationMeasured;
        Ok(())
    }

    fn begin_scan(&mut self, sample: ResourceSample) -> Result<(), ResourceGovernorError> {
        self.require_phase(GovernorPhase::Idle, "idle")?;
        if self.calibration.is_none()
            && self
                .policy
                .outer_contained_cold_worker_memory_charge_bytes
                .is_none()
        {
            return Err(ResourceGovernorError::InvalidCalibration(
                "a measured evaluator calibration is required before scanning",
            ));
        }
        let ingested = self.ingest(sample)?;
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        if self.apply_safety(&ingested)? {
            return Ok(());
        }
        self.maybe_open_post_compile_gate(&ingested);
        if self.post_compile_cutoff.is_some() {
            self.last_reason = DecisionReason::WaitingForPostCompileWindow;
            return Err(ResourceGovernorError::PostCompileWindowNotEstablished);
        }
        self.require_stable(&ingested)?;
        self.require_fully_stopped(ingested.sample.evaluator)?;
        if ingested.capacity.safe_worker_ceiling < 1 {
            self.last_reason = DecisionReason::CapacityLimited;
            return Err(ResourceGovernorError::CapacityUnavailable);
        }
        self.change_target(1, ingested.sample.cursor.observed_at_millis)?;
        self.phase = GovernorPhase::Scanning;
        self.last_reason = DecisionReason::ScanStarted;
        Ok(())
    }

    fn commit_scan_shard(
        &mut self,
        sample: ResourceSample,
        evidence: ShardCommitEvidence,
    ) -> Result<(), ResourceGovernorError> {
        self.require_phase(GovernorPhase::Scanning, "scanning")?;
        let ingested = self.ingest(sample)?;
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        if self.apply_safety(&ingested)? {
            return Ok(());
        }
        self.require_stable(&ingested)?;
        if !ingested
            .sample
            .evaluator
            .is_fully_active(self.target_worker_leases)
        {
            self.last_reason = DecisionReason::WaitingForResidents;
            return Err(ResourceGovernorError::TargetNotReconciled);
        }
        if evidence.lease_generation() != self.lease_generation
            || evidence.stability_epoch() != ingested.sample.stability.epoch
            || evidence.committed_at() != ingested.sample.cursor
        {
            return Err(ResourceGovernorError::ShardEvidenceMismatch);
        }
        let durable_cursor = DurableCommitCursor::from_evidence(&evidence)?;
        if self
            .last_durable_commit
            .is_some_and(|previous| durable_cursor <= previous)
        {
            return Err(ResourceGovernorError::ShardEvidenceMismatch);
        }
        self.maybe_start_ramp_window(&ingested)?;
        let ramp = self.ramp.as_ref().ok_or(ResourceGovernorError::NotStable)?;
        let count_this_shard = ramp.committed_shards < self.policy.committed_shards_before_scale_up;
        if count_this_shard
            && ramp
                .counted_shard_identities
                .contains(&evidence.shard_identity())
        {
            return Err(ResourceGovernorError::ShardEvidenceMismatch);
        }
        let committed_shards = if count_this_shard {
            Some(ramp.committed_shards.checked_add(1).ok_or(
                ResourceGovernorError::ArithmeticOverflow("committed shard evidence"),
            )?)
        } else {
            None
        };

        let (last_durable_commit, ramp) = match (&mut self.last_durable_commit, &mut self.ramp) {
            (last_durable_commit, Some(ramp)) => (last_durable_commit, ramp),
            (_, None) => return Err(ResourceGovernorError::NotStable),
        };
        *last_durable_commit = Some(durable_cursor);
        if let Some(committed_shards) = committed_shards {
            ramp.counted_shard_identities
                .push(evidence.shard_identity());
            ramp.committed_shards = committed_shards;
        }
        self.last_reason = DecisionReason::ShardEvidenceRecorded;
        self.maybe_scale_up_one(&ingested)?;
        Ok(())
    }

    fn end_scan(&mut self) -> Result<(), ResourceGovernorError> {
        self.require_phase(GovernorPhase::Scanning, "scanning")?;
        let observed_at = self
            .last_cursor
            .map(|cursor| cursor.observed_at_millis)
            .unwrap_or(0);
        self.change_target(0, observed_at)?;
        self.phase = GovernorPhase::Draining;
        self.drain_trigger = Some(DecisionReason::ScanEnded);
        self.last_reason = DecisionReason::ScanEnded;
        Ok(())
    }

    fn begin_compile(
        &mut self,
        epoch: CompileEpoch,
        charge: CompileCharge,
        sample: ResourceSample,
    ) -> Result<(), ResourceGovernorError> {
        self.require_phase(GovernorPhase::Idle, "idle")?;
        epoch.validate("compile_epoch")?;
        if self
            .last_compile_epoch
            .is_some_and(|previous| epoch <= previous)
        {
            return Err(ResourceGovernorError::StaleCompileEpoch);
        }
        self.validate_compile_charge(charge)?;
        let ingested = self.ingest(sample)?;
        if self.compiler_overlaps_evaluator_phase(ingested.sample) {
            self.fail_closed(GovernorFailure::CompilerOverlappedEvaluatorPhase);
            return Ok(());
        }
        if self.apply_safety(&ingested)? {
            return Ok(());
        }
        self.maybe_open_post_compile_gate(&ingested);
        if self.post_compile_cutoff.is_some() {
            self.last_reason = DecisionReason::WaitingForPostCompileWindow;
            return Err(ResourceGovernorError::PostCompileWindowNotEstablished);
        }
        self.require_stable(&ingested)?;
        self.require_fully_stopped(ingested.sample.evaluator)?;
        if !ingested.sample.compiler.is_known_zero() {
            return Err(ResourceGovernorError::ResidentsNotStopped);
        }
        if !self.compile_charge_fits(&ingested, charge)? {
            self.last_reason = DecisionReason::CapacityLimited;
            return Err(ResourceGovernorError::CapacityUnavailable);
        }

        self.advance_lease_generation()?;
        self.drain_trigger = None;
        self.phase = GovernorPhase::Compiling { epoch };
        self.compile_session = Some(CompileSession {
            epoch,
            charge,
            began_at: ingested.sample.cursor,
        });
        self.last_compile_epoch = Some(epoch);
        self.last_reason = DecisionReason::CompileStarted;
        Ok(())
    }

    fn end_compile(
        &mut self,
        epoch: CompileEpoch,
        sample: ResourceSample,
    ) -> Result<(), ResourceGovernorError> {
        let session = self
            .compile_session
            .ok_or(ResourceGovernorError::WrongPhase {
                expected: "compiling",
                actual: self.phase,
            })?;
        if session.epoch != epoch || self.phase != (GovernorPhase::Compiling { epoch }) {
            return Err(ResourceGovernorError::StaleCompileEpoch);
        }
        let ingested = self.ingest(sample)?;
        self.monitor_compile(&ingested);
        if let Some(failure) = self.failure {
            return Err(ResourceGovernorError::TerminalFailure(failure));
        }
        if !ingested.sample.compiler.is_known_zero() {
            self.fail_closed(GovernorFailure::CompilerExceededCharge);
            return Err(ResourceGovernorError::TerminalFailure(
                GovernorFailure::CompilerExceededCharge,
            ));
        }
        self.require_fully_stopped(ingested.sample.evaluator)?;

        self.advance_lease_generation()?;
        self.post_compile_cutoff = Some(CompileCutoff {
            cursor: ingested.sample.cursor,
            stability_epoch: ingested.sample.stability.epoch,
        });
        self.block_stability_epoch(ingested.sample.stability.epoch);
        self.calibration = None;
        self.compile_session = None;
        self.phase = GovernorPhase::Idle;
        self.last_reason = DecisionReason::CompileFinished;
        Ok(())
    }

    fn ingest(&mut self, sample: ResourceSample) -> Result<IngestedSample, ResourceGovernorError> {
        if let Err(error) = self.validate_sample(sample) {
            self.fail_for_ingest_error(&error);
            return Err(error);
        }

        let previous_cursor = self.last_cursor;
        let telemetry_reset =
            previous_cursor.is_some_and(|previous| sample.cursor.epoch > previous.epoch);
        let stability_advanced = self
            .last_stability
            .is_some_and(|previous| sample.stability.epoch > previous.epoch);
        // Compute a complete candidate without mutating the reducer.  Only
        // after validation, swap analysis, capacity arithmetic, and stability
        // arithmetic all succeed is the candidate committed below.
        let swap_candidate = match self.assess_swap_candidate(sample, telemetry_reset) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.fail_for_ingest_error(&error);
                return Err(error);
            }
        };
        let capacity = match self.assess_worker_capacity(sample) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.fail_for_ingest_error(&error);
                return Err(error);
            }
        };
        let stable_duration_millis = self.stable_duration(sample, swap_candidate.tracker);
        let stable = self.sample_is_stable(
            sample,
            capacity,
            swap_candidate.assessment,
            stable_duration_millis,
        );
        let swap_breaks_stability = !self
            .policy
            .swap_growth_authority
            .admits(swap_candidate.assessment);
        let reset_ramp = telemetry_reset
            || stability_advanced
            || swap_breaks_stability
            || (self.phase == GovernorPhase::Scanning
                && (!stable || !sample.evaluator.is_fully_active(self.target_worker_leases)));

        self.last_swap = swap_candidate.tracker;
        self.swap_observation_gap = swap_candidate.observation_gap;
        if reset_ramp {
            self.ramp = None;
        }
        self.last_cursor = Some(sample.cursor);
        self.last_stability = Some(sample.stability);
        self.last_observation = Some(sample.evaluator);
        self.last_capacity = Some(capacity);
        self.last_stable = stable;
        self.last_stable_duration_millis = stable_duration_millis;
        self.last_swap_assessment = swap_candidate.assessment;
        self.last_pressure = sample.pressure;

        Ok(IngestedSample {
            sample,
            capacity,
            stable,
            swap: swap_candidate.assessment,
        })
    }

    fn fail_for_ingest_error(&mut self, error: &ResourceGovernorError) {
        if matches!(
            error,
            ResourceGovernorError::ArithmeticOverflow(_)
                | ResourceGovernorError::GenerationOverflow(_)
        ) {
            self.fail_closed(GovernorFailure::ArithmeticOrGenerationOverflow);
        } else if matches!(self.phase, GovernorPhase::Compiling { .. }) {
            self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
        } else {
            // Incoherent automatic telemetry cannot leave an existing target
            // authoritative.  The coordinator gets an explicit revoked lease
            // authority even though the original validation error is returned.
            self.fail_closed(GovernorFailure::IncoherentTelemetry);
        }
    }

    fn validate_sample(&self, sample: ResourceSample) -> Result<(), ResourceGovernorError> {
        sample.cursor.validate()?;
        sample.stability.epoch.validate("stability_epoch")?;
        sample
            .evaluator
            .lease_generation
            .validate("lease_generation")?;
        if let Some(epoch) = sample.compile_epoch {
            epoch.validate("sample.compile_epoch")?;
        }
        if sample.stability.stable_since_millis > sample.cursor.observed_at_millis {
            return Err(ResourceGovernorError::InvalidSample(
                "stable_since_millis is after the observation",
            ));
        }
        if let Some(previous) = self.last_cursor {
            if !sample.cursor.is_strictly_after(previous)
                || sample.cursor.observed_at_millis <= previous.observed_at_millis
            {
                return Err(ResourceGovernorError::NonMonotonicTelemetry);
            }
        }
        self.validate_stability_progress(sample)?;

        if sample.evaluator.lease_generation != self.lease_generation {
            return Err(ResourceGovernorError::StaleResidentObservation {
                expected: self.lease_generation,
                observed: sample.evaluator.lease_generation,
            });
        }
        if self
            .lease_observation_cutoff
            .is_some_and(|cutoff| sample.cursor <= cutoff)
        {
            return Err(ResourceGovernorError::StaleResidentObservation {
                expected: self.lease_generation,
                observed: sample.evaluator.lease_generation,
            });
        }
        sample.evaluator.active_workers()?;
        sample.evaluator.accounted_target()?;
        sample.evaluator.live_and_reserved_commitments()?;
        if sample.evaluator.resident_workers == 0
            && (sample
                .evaluator
                .aggregate_rss_bytes
                .is_some_and(|value| value != 0)
                || sample
                    .evaluator
                    .aggregate_cpu_millicores
                    .is_some_and(|value| value != 0))
        {
            return Err(ResourceGovernorError::InvalidResidentObservation(
                "zero residents cannot have nonzero aggregate usage",
            ));
        }

        match self.phase {
            GovernorPhase::Compiling { epoch } if sample.compile_epoch != Some(epoch) => {
                return Err(ResourceGovernorError::StaleCompileEpoch);
            }
            GovernorPhase::Compiling { .. } => {}
            _ if sample.compile_epoch.is_some() => {
                return Err(ResourceGovernorError::StaleCompileEpoch);
            }
            _ => {}
        }

        if let Some(cpu) = sample.cpu {
            if cpu.live_capacity_millicores == 0
                || cpu.idle_millicores > cpu.live_capacity_millicores
            {
                return Err(ResourceGovernorError::InvalidSample(
                    "CPU headroom is outside live capacity",
                ));
            }
            if let Some(logical_cpu_count) = self.host.logical_cpu_count {
                let installed = u64::from(logical_cpu_count)
                    .checked_mul(MILLICORES_PER_CORE)
                    .ok_or(ResourceGovernorError::ArithmeticOverflow(
                        "installed CPU capacity",
                    ))?;
                if u64::from(cpu.live_capacity_millicores) > installed {
                    return Err(ResourceGovernorError::InvalidSample(
                        "live CPU capacity exceeds installed capacity",
                    ));
                }
            }
            let evaluator_cpu = sample.evaluator.aggregate_cpu_millicores.unwrap_or(0);
            let compiler_cpu = sample.compiler.cpu_millicores.unwrap_or(0);
            let observed_cpu = u64::from(cpu.idle_millicores)
                .checked_add(u64::from(evaluator_cpu))
                .and_then(|value| value.checked_add(u64::from(compiler_cpu)))
                .ok_or(ResourceGovernorError::ArithmeticOverflow(
                    "observed CPU capacity",
                ))?;
            if observed_cpu > u64::from(cpu.live_capacity_millicores) {
                return Err(ResourceGovernorError::InvalidSample(
                    "idle and owned CPU exceed live capacity",
                ));
            }
        }

        if let Some(total_memory) = self.host.total_memory_bytes {
            let observed_memory = sample
                .available_memory_bytes
                .unwrap_or(0)
                .checked_add(sample.evaluator.aggregate_rss_bytes.unwrap_or(0))
                .and_then(|value| value.checked_add(sample.compiler.rss_bytes.unwrap_or(0)))
                .ok_or(ResourceGovernorError::ArithmeticOverflow(
                    "observed memory capacity",
                ))?;
            if observed_memory > total_memory {
                return Err(ResourceGovernorError::InvalidSample(
                    "available and owned memory exceed installed memory",
                ));
            }
        }

        let current_memory = checked_sum_options(
            sample.available_memory_bytes,
            sample.evaluator.aggregate_rss_bytes,
            "available memory plus evaluator RSS",
        )?;
        if let (Some(minimum), Some(current)) = (
            sample.stability.minimum_available_memory_bytes,
            sample.available_memory_bytes,
        ) {
            if minimum > current {
                return Err(ResourceGovernorError::InvalidSample(
                    "stable-window available-memory minimum exceeds current availability",
                ));
            }
        }
        if let (Some(minimum), Some(current)) = (
            sample.stability.minimum_idle_cpu_millicores,
            sample.cpu.map(|value| value.idle_millicores),
        ) {
            if minimum > current {
                return Err(ResourceGovernorError::InvalidSample(
                    "stable-window idle-CPU minimum exceeds current idleness",
                ));
            }
        }
        if let (Some(minimum), Some(current)) = (
            sample
                .stability
                .minimum_memory_before_evaluator_charge_bytes,
            current_memory,
        ) {
            if minimum > current {
                return Err(ResourceGovernorError::InvalidSample(
                    "stable-window memory minimum exceeds current reconstructed memory",
                ));
            }
        }
        let current_cpu = checked_sum_options_u32(
            sample.cpu.map(|value| value.idle_millicores),
            sample.evaluator.aggregate_cpu_millicores,
            "idle CPU plus evaluator CPU",
        )?;
        if let (Some(minimum), Some(current)) = (
            sample
                .stability
                .minimum_cpu_before_evaluator_charge_millicores,
            current_cpu,
        ) {
            if minimum > current {
                return Err(ResourceGovernorError::InvalidSample(
                    "stable-window CPU minimum exceeds current reconstructed CPU",
                ));
            }
        }
        Ok(())
    }

    fn validate_stability_progress(
        &self,
        sample: ResourceSample,
    ) -> Result<(), ResourceGovernorError> {
        let Some(previous) = self.last_stability else {
            return Ok(());
        };
        if sample.stability.epoch < previous.epoch {
            return Err(ResourceGovernorError::NonMonotonicStability);
        }
        if sample.stability.epoch == previous.epoch {
            if sample.stability.stable_since_millis != previous.stable_since_millis
                || !option_minimum_progresses(
                    previous.minimum_available_memory_bytes,
                    sample.stability.minimum_available_memory_bytes,
                )
                || !option_minimum_progresses(
                    previous.minimum_idle_cpu_millicores,
                    sample.stability.minimum_idle_cpu_millicores,
                )
                || !option_minimum_progresses(
                    previous.minimum_memory_before_evaluator_charge_bytes,
                    sample
                        .stability
                        .minimum_memory_before_evaluator_charge_bytes,
                )
                || !option_minimum_progresses(
                    previous.minimum_cpu_before_evaluator_charge_millicores,
                    sample
                        .stability
                        .minimum_cpu_before_evaluator_charge_millicores,
                )
            {
                return Err(ResourceGovernorError::NonMonotonicStability);
            }
        } else if let Some(previous_cursor) = self.last_cursor {
            if sample.stability.stable_since_millis < previous_cursor.observed_at_millis {
                return Err(ResourceGovernorError::NonMonotonicStability);
            }
        }
        Ok(())
    }

    fn assess_swap_candidate(
        &self,
        sample: ResourceSample,
        telemetry_reset: bool,
    ) -> Result<SwapCandidate, ResourceGovernorError> {
        let Some(counter) = sample.swap_out else {
            return Ok(SwapCandidate {
                assessment: SwapAssessment::Unknown,
                tracker: self.last_swap,
                observation_gap: true,
            });
        };
        counter.generation.validate("swap_out_generation")?;

        let Some(previous) = self.last_swap else {
            return Ok(SwapCandidate {
                assessment: SwapAssessment::Baseline,
                tracker: Some(SwapTracker {
                    telemetry_epoch: sample.cursor.epoch,
                    counter,
                    baseline_at_millis: sample.cursor.observed_at_millis,
                }),
                observation_gap: false,
            });
        };
        if telemetry_reset || self.swap_observation_gap {
            return Ok(SwapCandidate {
                assessment: SwapAssessment::Baseline,
                tracker: Some(SwapTracker {
                    telemetry_epoch: sample.cursor.epoch,
                    counter,
                    baseline_at_millis: sample.cursor.observed_at_millis,
                }),
                observation_gap: false,
            });
        }
        if counter.generation < previous.counter.generation {
            return Err(ResourceGovernorError::NonMonotonicSwapCounter);
        }
        if counter.generation > previous.counter.generation {
            return Ok(SwapCandidate {
                assessment: SwapAssessment::CounterReset,
                tracker: Some(SwapTracker {
                    telemetry_epoch: sample.cursor.epoch,
                    counter,
                    baseline_at_millis: sample.cursor.observed_at_millis,
                }),
                observation_gap: false,
            });
        }
        if counter.cumulative_bytes < previous.counter.cumulative_bytes {
            return Err(ResourceGovernorError::NonMonotonicSwapCounter);
        }
        let assessment = if counter.cumulative_bytes == previous.counter.cumulative_bytes {
            SwapAssessment::Unchanged
        } else {
            SwapAssessment::Growth
        };
        Ok(SwapCandidate {
            assessment,
            tracker: Some(SwapTracker {
                telemetry_epoch: sample.cursor.epoch,
                counter,
                baseline_at_millis: if assessment == SwapAssessment::Growth
                    && !self.policy.swap_growth_authority.admits(assessment)
                {
                    sample.cursor.observed_at_millis
                } else {
                    previous.baseline_at_millis
                },
            }),
            observation_gap: false,
        })
    }

    fn assess_worker_capacity(
        &self,
        sample: ResourceSample,
    ) -> Result<CapacityAssessment, ResourceGovernorError> {
        let worker_memory_charge_bytes = self.worker_memory_charge()?;
        let worker_cpu_charge_millicores = self.policy.worker_cpu_charge_millicores;
        let memory_reserve_bytes = self.memory_reserve_bytes()?;

        let current_memory = checked_sum_options(
            sample.available_memory_bytes,
            sample.evaluator.aggregate_rss_bytes,
            "available memory plus aggregate owned evaluator RSS",
        )?;
        let window_memory = match (
            current_memory,
            sample
                .stability
                .minimum_memory_before_evaluator_charge_bytes,
        ) {
            (Some(current), Some(minimum)) => Some(cmp::min(current, minimum)),
            _ => None,
        };
        let memory_budget = window_memory
            .and_then(|value| value.checked_sub(memory_reserve_bytes))
            .map(|value| {
                if let Some(total) = self.host.total_memory_bytes {
                    cmp::min(value, total.saturating_sub(memory_reserve_bytes))
                } else {
                    value
                }
            });
        let memory_worker_ceiling = memory_budget
            .map(|value| saturating_u16(value / worker_memory_charge_bytes))
            .unwrap_or(0);

        let current_cpu = checked_sum_options_u32(
            sample.cpu.map(|value| value.idle_millicores),
            sample.evaluator.aggregate_cpu_millicores,
            "idle CPU plus aggregate owned evaluator CPU",
        )?;
        let (cpu_reserve_millicores, cpu_budget) = match (sample.cpu, current_cpu) {
            (Some(cpu), Some(current)) => {
                let reserve = self.cpu_reserve_millicores(cpu.live_capacity_millicores)?;
                let window = sample
                    .stability
                    .minimum_cpu_before_evaluator_charge_millicores
                    .map(|minimum| cmp::min(current, minimum));
                let live_budget = cpu.live_capacity_millicores.saturating_sub(reserve);
                (
                    reserve,
                    window.map(|value| cmp::min(value.saturating_sub(reserve), live_budget)),
                )
            }
            _ => (0, None),
        };
        let policy_worker_ceiling = self.policy_worker_ceiling();
        let cpu_worker_ceiling = if self
            .policy
            .outer_contained_cold_worker_memory_charge_bytes
            .is_some()
        {
            // The validated outer supervisor samples authoritative Mach host
            // counters continuously and pauses the entire worker group to
            // repay any 80%-budget debt. Do not pre-charge another whole core
            // at this slower semantic boundary; retain complete CPU telemetry
            // and the live/window reserve checks below.
            policy_worker_ceiling
        } else {
            cpu_budget
                .map(|value| {
                    saturating_u16(u64::from(value) / u64::from(worker_cpu_charge_millicores))
                })
                .unwrap_or(0)
        };
        let charged_worker_commitments = cmp::max(
            self.target_worker_leases,
            sample.evaluator.live_and_reserved_commitments()?,
        );
        let telemetry_complete = current_memory.is_some()
            && window_memory.is_some()
            && current_cpu.is_some()
            && cpu_budget.is_some()
            && sample.stability.minimum_available_memory_bytes.is_some()
            && sample.stability.minimum_idle_cpu_millicores.is_some()
            && sample.evaluator.aggregate_rss_bytes.is_some()
            && sample.evaluator.aggregate_cpu_millicores.is_some()
            && sample.swap_out.is_some()
            && sample.compiler.rss_bytes.is_some()
            && sample.compiler.cpu_millicores.is_some()
            && self.host.logical_cpu_count.is_some()
            && self.host.total_memory_bytes.is_some();
        let mut safe_worker_ceiling = cmp::min(
            cmp::min(memory_worker_ceiling, cpu_worker_ceiling),
            policy_worker_ceiling,
        );
        if self.calibration.is_none() {
            safe_worker_ceiling = cmp::min(safe_worker_ceiling, 1);
        }
        if !telemetry_complete {
            safe_worker_ceiling = 0;
        }

        Ok(CapacityAssessment {
            telemetry_complete,
            memory_reserve_bytes,
            cpu_reserve_millicores,
            worker_memory_charge_bytes,
            worker_cpu_charge_millicores,
            current_memory_before_worker_charge_bytes: current_memory,
            current_cpu_before_worker_charge_millicores: current_cpu,
            memory_worker_ceiling,
            cpu_worker_ceiling,
            policy_worker_ceiling,
            charged_worker_commitments,
            safe_worker_ceiling,
        })
    }

    fn sample_is_stable(
        &self,
        sample: ResourceSample,
        capacity: CapacityAssessment,
        swap: SwapAssessment,
        stable_duration_millis: u64,
    ) -> bool {
        self.policy
            .evaluator_pressure_is_admissible(sample.pressure)
            && !sample.oom_risk
            && capacity.telemetry_complete
            && self.current_and_window_reserve_intact(sample, capacity)
            && self.policy.swap_growth_authority.admits(swap)
            && stable_duration_millis >= self.policy.stable_window_millis
            && self
                .blocked_stability_through
                .is_none_or(|blocked| sample.stability.epoch > blocked)
            && capacity.safe_worker_ceiling >= capacity.charged_worker_commitments
            && match self.phase {
                GovernorPhase::Compiling { .. } => true,
                _ => sample.compiler.is_known_zero(),
            }
    }

    fn stable_duration(&self, sample: ResourceSample, candidate_swap: Option<SwapTracker>) -> u64 {
        let swap_baseline = candidate_swap
            .filter(|tracker| tracker.telemetry_epoch == sample.cursor.epoch)
            .map(|tracker| tracker.baseline_at_millis)
            .unwrap_or(sample.cursor.observed_at_millis);
        sample.cursor.observed_at_millis.saturating_sub(cmp::max(
            sample.stability.stable_since_millis,
            swap_baseline,
        ))
    }

    /// Returns true when a safety transition consumed the event.
    fn apply_safety(&mut self, ingested: &IngestedSample) -> Result<bool, ResourceGovernorError> {
        let sample = ingested.sample;
        if sample.evaluator.draining_workers > 0 {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::DrainingWorkersBackoff,
            )?;
            return Ok(true);
        }
        if sample.evaluator.accounted_target()? > self.target_worker_leases {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::LeaseOversubscriptionBackoff,
            )?;
            return Ok(true);
        }
        if sample.oom_risk {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::OomRiskBackoff,
            )?;
            return Ok(true);
        }
        match sample.pressure {
            MemoryPressure::Critical => {
                self.block_stability_epoch(sample.stability.epoch);
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::CriticalBackoff,
                )?;
                return Ok(true);
            }
            MemoryPressure::Unknown => {
                self.block_stability_epoch(sample.stability.epoch);
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::UnknownPressureBackoff,
                )?;
                return Ok(true);
            }
            MemoryPressure::Warning | MemoryPressure::Normal => {}
        }
        match ingested.swap {
            SwapAssessment::Growth
                if self
                    .policy
                    .swap_growth_authority
                    .admits(SwapAssessment::Growth) => {}
            SwapAssessment::Growth => {
                self.block_stability_epoch(sample.stability.epoch);
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::SwapGrowthBackoff,
                )?;
                return Ok(true);
            }
            SwapAssessment::CounterReset => {
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::SwapCounterResetBackoff,
                )?;
                return Ok(true);
            }
            SwapAssessment::Unknown => {
                self.block_stability_epoch(sample.stability.epoch);
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::UnknownTelemetryBackoff,
                )?;
                return Ok(true);
            }
            SwapAssessment::Baseline => {
                self.backoff_to(
                    0,
                    sample.cursor.observed_at_millis,
                    DecisionReason::WaitingForSwapBaseline,
                )?;
                return Ok(true);
            }
            SwapAssessment::Unchanged => {}
        }
        if !ingested.capacity.telemetry_complete {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::UnknownTelemetryBackoff,
            )?;
            return Ok(true);
        }
        if ingested.capacity.charged_worker_commitments > ingested.capacity.safe_worker_ceiling {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::ReserveBackoff,
            )?;
            return Ok(true);
        }
        if !self.current_and_window_reserve_intact(sample, ingested.capacity) {
            self.block_stability_epoch(sample.stability.epoch);
            self.backoff_to(
                0,
                sample.cursor.observed_at_millis,
                DecisionReason::ReserveBackoff,
            )?;
            return Ok(true);
        }
        if sample.pressure == MemoryPressure::Warning
            && self
                .policy
                .outer_contained_cold_worker_memory_charge_bytes
                .is_none()
        {
            self.block_stability_epoch(sample.stability.epoch);
            let one_lower = self.target_worker_leases.saturating_sub(1);
            let target = cmp::min(one_lower, ingested.capacity.safe_worker_ceiling);
            self.backoff_to(
                target,
                sample.cursor.observed_at_millis,
                DecisionReason::WarningBackoff,
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    fn backoff_to(
        &mut self,
        target: u16,
        observed_at_millis: u64,
        reason: DecisionReason,
    ) -> Result<(), ResourceGovernorError> {
        if target != self.target_worker_leases {
            self.change_target(target, observed_at_millis)?;
        } else {
            self.ramp = None;
        }
        if target == 0
            && !matches!(
                self.phase,
                GovernorPhase::Compiling { .. } | GovernorPhase::Failed
            )
            && (matches!(
                self.phase,
                GovernorPhase::Scanning | GovernorPhase::CalibratingOneWorker
            ) || self
                .last_observation
                .is_some_and(|observation| !observation.is_fully_stopped()))
        {
            if self.phase != GovernorPhase::Draining || self.drain_trigger.is_none() {
                self.drain_trigger = Some(reason);
            }
            self.phase = GovernorPhase::Draining;
        }
        self.last_reason = reason;
        Ok(())
    }

    fn monitor_compile(&mut self, ingested: &IngestedSample) {
        let Some(session) = self.compile_session else {
            self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
            return;
        };
        let sample = ingested.sample;
        if sample.cursor <= session.began_at {
            self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
            return;
        }
        let stability_window_complete = sample.stability.minimum_available_memory_bytes.is_some()
            && sample.stability.minimum_idle_cpu_millicores.is_some()
            && sample
                .stability
                .minimum_memory_before_evaluator_charge_bytes
                .is_some()
            && sample
                .stability
                .minimum_cpu_before_evaluator_charge_millicores
                .is_some();
        if !ingested.capacity.telemetry_complete || !stability_window_complete || !ingested.stable {
            self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
            return;
        }
        if !sample.evaluator.is_fully_stopped() || self.target_worker_leases != 0 {
            self.fail_closed(GovernorFailure::EvaluatorResidentDuringCompile);
            return;
        }
        let (Some(compiler_rss), Some(compiler_cpu)) =
            (sample.compiler.rss_bytes, sample.compiler.cpu_millicores)
        else {
            self.fail_closed(GovernorFailure::CompilerTelemetryUnknown);
            return;
        };
        if compiler_rss > session.charge.memory_bytes
            || compiler_cpu > session.charge.cpu_millicores
        {
            self.fail_closed(GovernorFailure::CompilerExceededCharge);
            return;
        }
        let memory_reserve = match self.memory_reserve_bytes() {
            Ok(value) => value,
            Err(_) => {
                self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
                return;
            }
        };
        let memory_safe = sample
            .available_memory_bytes
            .is_some_and(|available| available >= memory_reserve);
        let cpu_safe = sample.cpu.is_some_and(|cpu| {
            self.cpu_reserve_millicores(cpu.live_capacity_millicores)
                .is_ok_and(|reserve| cpu.idle_millicores >= reserve)
        });
        if sample.pressure != MemoryPressure::Normal
            || sample.oom_risk
            || !self.policy.swap_growth_authority.admits(ingested.swap)
            || !memory_safe
            || !cpu_safe
        {
            self.fail_closed(GovernorFailure::UnsafeTelemetryDuringCompile);
            return;
        }
        self.last_reason = DecisionReason::Holding;
    }

    fn compiler_overlaps_evaluator_phase(&self, sample: ResourceSample) -> bool {
        !matches!(self.phase, GovernorPhase::Compiling { .. })
            && (sample.compiler.rss_bytes.is_some_and(|value| value != 0)
                || sample
                    .compiler
                    .cpu_millicores
                    .is_some_and(|value| value != 0))
    }

    fn compile_charge_fits(
        &self,
        ingested: &IngestedSample,
        charge: CompileCharge,
    ) -> Result<bool, ResourceGovernorError> {
        if self.host.logical_cpu_count.is_none() || self.host.total_memory_bytes.is_none() {
            return Ok(false);
        }
        let memory_reserve = self.memory_reserve_bytes()?;
        let memory_needed = memory_reserve.checked_add(charge.memory_bytes).ok_or(
            ResourceGovernorError::ArithmeticOverflow("compile memory admission"),
        )?;
        let current_memory = ingested
            .sample
            .available_memory_bytes
            .zip(ingested.sample.evaluator.aggregate_rss_bytes)
            .and_then(|(available, owned)| available.checked_add(owned));
        let window_memory = ingested.sample.stability.minimum_available_memory_bytes;
        let memory_fits = current_memory
            .zip(window_memory)
            .is_some_and(|(current, minimum)| cmp::min(current, minimum) >= memory_needed);

        let Some(cpu) = ingested.sample.cpu else {
            return Ok(false);
        };
        let cpu_reserve = self.cpu_reserve_millicores(cpu.live_capacity_millicores)?;
        let cpu_needed = cpu_reserve.checked_add(charge.cpu_millicores).ok_or(
            ResourceGovernorError::ArithmeticOverflow("compile CPU admission"),
        )?;
        let current_cpu = cpu.idle_millicores.checked_add(
            ingested
                .sample
                .evaluator
                .aggregate_cpu_millicores
                .unwrap_or(0),
        );
        let window_cpu = ingested.sample.stability.minimum_idle_cpu_millicores;
        let cpu_fits = current_cpu
            .zip(window_cpu)
            .is_some_and(|(current, minimum)| cmp::min(current, minimum) >= cpu_needed);
        Ok(memory_fits && cpu_fits)
    }

    fn validate_compile_charge(&self, charge: CompileCharge) -> Result<(), ResourceGovernorError> {
        let conservative_memory = self.conservative_compile_memory_charge()?;
        if charge.memory_bytes < conservative_memory {
            return Err(ResourceGovernorError::InvalidCompileCharge(
                "memory charge is below the conservative compile charge",
            ));
        }
        if charge.cpu_millicores < self.policy.minimum_compile_cpu_charge_millicores {
            return Err(ResourceGovernorError::InvalidCompileCharge(
                "CPU charge is below the conservative compile charge",
            ));
        }
        Ok(())
    }

    fn current_and_window_reserve_intact(
        &self,
        sample: ResourceSample,
        capacity: CapacityAssessment,
    ) -> bool {
        sample
            .available_memory_bytes
            .zip(sample.stability.minimum_available_memory_bytes)
            .is_some_and(|(current, minimum)| {
                cmp::min(current, minimum) >= capacity.memory_reserve_bytes
            })
            && sample
                .cpu
                .map(|cpu| cpu.idle_millicores)
                .zip(sample.stability.minimum_idle_cpu_millicores)
                .is_some_and(|(current, minimum)| {
                    cmp::min(current, minimum) >= capacity.cpu_reserve_millicores
                })
    }

    fn maybe_start_ramp_window(
        &mut self,
        ingested: &IngestedSample,
    ) -> Result<(), ResourceGovernorError> {
        if self.phase != GovernorPhase::Scanning
            || !ingested.stable
            || !ingested
                .sample
                .evaluator
                .is_fully_active(self.target_worker_leases)
        {
            return Ok(());
        }
        if ingested.sample.evaluator.accounted_target()? != self.target_worker_leases {
            return Ok(());
        }
        let matches = self.ramp.as_ref().is_some_and(|window| {
            window.lease_generation == self.lease_generation
                && window.stability_epoch == ingested.sample.stability.epoch
        });
        if !matches {
            self.ramp = Some(RampWindow {
                lease_generation: self.lease_generation,
                stability_epoch: ingested.sample.stability.epoch,
                started_at_millis: ingested.sample.cursor.observed_at_millis,
                committed_shards: 0,
                counted_shard_identities: Vec::with_capacity(usize::from(
                    self.policy.committed_shards_before_scale_up,
                )),
            });
        }
        Ok(())
    }

    fn maybe_scale_up_one(
        &mut self,
        ingested: &IngestedSample,
    ) -> Result<(), ResourceGovernorError> {
        if self.phase != GovernorPhase::Scanning
            || !ingested
                .sample
                .evaluator
                .is_fully_active(self.target_worker_leases)
            || ingested.sample.evaluator.accounted_target()? != self.target_worker_leases
        {
            return Ok(());
        }
        let Some(ramp) = self.ramp.as_ref() else {
            return Ok(());
        };
        let local_stable_duration = ingested
            .sample
            .cursor
            .observed_at_millis
            .saturating_sub(ramp.started_at_millis);
        if ramp.committed_shards < self.policy.committed_shards_before_scale_up
            || local_stable_duration < self.policy.stable_window_millis
            || ingested.capacity.safe_worker_ceiling <= self.target_worker_leases
        {
            return Ok(());
        }
        let next = self.target_worker_leases.checked_add(1).ok_or(
            ResourceGovernorError::ArithmeticOverflow("worker target scale-up"),
        )?;
        self.change_target(next, ingested.sample.cursor.observed_at_millis)?;
        self.last_reason = DecisionReason::ScaledUpOneWorker;
        Ok(())
    }

    fn maybe_finish_draining(&mut self, observation: EvaluatorObservation) {
        if self.phase == GovernorPhase::Draining
            && observation.lease_generation == self.lease_generation
            && observation.is_fully_stopped()
        {
            self.phase = GovernorPhase::Idle;
            self.last_reason = DecisionReason::DrainCompleted;
        } else if self.phase == GovernorPhase::Draining {
            self.last_reason = DecisionReason::Draining;
        }
    }

    fn maybe_open_post_compile_gate(&mut self, ingested: &IngestedSample) {
        let Some(cutoff) = self.post_compile_cutoff else {
            return;
        };
        let sample = ingested.sample;
        if sample.compile_epoch.is_none()
            && sample.cursor > cutoff.cursor
            && sample.stability.epoch > cutoff.stability_epoch
            && sample.stability.stable_since_millis > cutoff.cursor.observed_at_millis
            && ingested.stable
        {
            self.post_compile_cutoff = None;
        }
    }

    fn require_stable(&mut self, ingested: &IngestedSample) -> Result<(), ResourceGovernorError> {
        if !ingested.stable {
            self.last_reason = DecisionReason::WaitingForStableWindow;
            return Err(ResourceGovernorError::NotStable);
        }
        Ok(())
    }

    fn require_fully_stopped(
        &mut self,
        observation: EvaluatorObservation,
    ) -> Result<(), ResourceGovernorError> {
        if !observation.is_fully_stopped() {
            self.last_reason = DecisionReason::WaitingForResidents;
            return Err(ResourceGovernorError::ResidentsNotStopped);
        }
        Ok(())
    }

    fn require_phase(
        &self,
        expected: GovernorPhase,
        expected_name: &'static str,
    ) -> Result<(), ResourceGovernorError> {
        if self.phase == expected {
            Ok(())
        } else {
            Err(ResourceGovernorError::WrongPhase {
                expected: expected_name,
                actual: self.phase,
            })
        }
    }

    fn change_target(
        &mut self,
        target: u16,
        _observed_at_millis: u64,
    ) -> Result<(), ResourceGovernorError> {
        if target == self.target_worker_leases {
            return Ok(());
        }
        self.advance_lease_generation()?;
        self.target_worker_leases = target;
        if target > 0 {
            self.drain_trigger = None;
        }
        Ok(())
    }

    fn advance_lease_generation(&mut self) -> Result<(), ResourceGovernorError> {
        self.lease_generation = self.lease_generation.next("lease_generation")?;
        self.lease_authority = LeaseAuthority::Active;
        self.lease_observation_cutoff = self.last_cursor;
        // Preserve the old, explicitly generation-bound observation for
        // diagnostics.  It cannot reconcile the new generation because every
        // transition that consumes residents uses the event's fresh sample.
        self.ramp = None;
        Ok(())
    }

    fn fail_closed(&mut self, failure: GovernorFailure) {
        let was_compiling = matches!(self.phase, GovernorPhase::Compiling { .. });
        self.target_worker_leases = 0;
        if let Ok(next) = self.lease_generation.next("lease_generation") {
            self.lease_generation = next;
        }
        // Revocation is independent of generation increment, so even a u64
        // generation overflow cannot leave stale authority to run workers.
        self.lease_authority = LeaseAuthority::Revoked;
        self.lease_observation_cutoff = self.last_cursor;
        self.ramp = None;
        self.compile_session = None;
        self.calibration = None;
        self.drain_trigger = None;
        self.phase = GovernorPhase::Failed;
        self.failure = Some(failure);
        self.last_reason = if was_compiling {
            DecisionReason::CompileAborted
        } else {
            DecisionReason::FailedClosed
        };
    }

    fn block_stability_epoch(&mut self, epoch: StabilityEpoch) {
        self.blocked_stability_through = Some(
            self.blocked_stability_through
                .map(|blocked| cmp::max(blocked, epoch))
                .unwrap_or(epoch),
        );
        self.ramp = None;
    }

    fn worker_memory_charge(&self) -> Result<u64, ResourceGovernorError> {
        if let Some(calibration) = self.calibration {
            Ok(calibration.charged_worker_memory_bytes)
        } else {
            self.conservative_cold_calibration_memory_charge()
        }
    }

    fn conservative_cold_calibration_memory_charge(&self) -> Result<u64, ResourceGovernorError> {
        if let Some(contained_charge) = self.policy.outer_contained_cold_worker_memory_charge_bytes
        {
            return Ok(contained_charge);
        }
        let host_fraction = self
            .host
            .total_memory_bytes
            .map(|total| ceil_div_u64(total, CONSERVATIVE_PHASE_CHARGE_DIVISOR))
            .transpose()?
            .unwrap_or(0);
        Ok(cmp::max(
            self.policy.minimum_cold_calibration_memory_charge_bytes,
            host_fraction,
        ))
    }

    fn conservative_compile_memory_charge(&self) -> Result<u64, ResourceGovernorError> {
        let host_fraction = self
            .host
            .total_memory_bytes
            .map(|total| ceil_div_u64(total, CONSERVATIVE_PHASE_CHARGE_DIVISOR))
            .transpose()?
            .unwrap_or(0);
        Ok(cmp::max(
            self.policy.minimum_compile_memory_charge_bytes,
            host_fraction,
        ))
    }

    fn memory_reserve_bytes(&self) -> Result<u64, ResourceGovernorError> {
        let host_fraction = self
            .host
            .total_memory_bytes
            .map(|total| ceil_div_u64(total, u64::from(self.policy.memory_reserve_divisor)))
            .transpose()?
            .unwrap_or(0);
        Ok(cmp::max(
            self.policy.minimum_memory_reserve_bytes,
            host_fraction,
        ))
    }

    fn cpu_reserve_millicores(
        &self,
        live_capacity_millicores: u32,
    ) -> Result<u32, ResourceGovernorError> {
        let live_reserve = ceil_div_u64(
            u64::from(live_capacity_millicores),
            u64::from(self.policy.cpu_reserve_divisor),
        )?;
        let installed_reserve = self
            .host
            .logical_cpu_count
            .map(|cores| {
                u64::from(cores)
                    .checked_mul(MILLICORES_PER_CORE)
                    .ok_or(ResourceGovernorError::ArithmeticOverflow(
                        "installed CPU capacity",
                    ))
                    .and_then(|capacity| {
                        ceil_div_u64(capacity, u64::from(self.policy.cpu_reserve_divisor))
                    })
            })
            .transpose()?
            .unwrap_or(0);
        let reserve = cmp::max(live_reserve, installed_reserve);
        u32::try_from(reserve)
            .map_err(|_| ResourceGovernorError::ArithmeticOverflow("CPU reserve conversion"))
    }

    fn policy_worker_ceiling(&self) -> u16 {
        [
            self.policy.configured_worker_ceiling,
            self.policy.requested_jobs_ceiling,
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(u16::MAX)
    }
}

fn option_minimum_progresses<T: Ord>(previous: Option<T>, current: Option<T>) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) => current <= previous,
        (None, None) => true,
        _ => false,
    }
}

fn checked_sum_options(
    left: Option<u64>,
    right: Option<u64>,
    field: &'static str,
) -> Result<Option<u64>, ResourceGovernorError> {
    match (left, right) {
        (Some(left), Some(right)) => left
            .checked_add(right)
            .map(Some)
            .ok_or(ResourceGovernorError::ArithmeticOverflow(field)),
        _ => Ok(None),
    }
}

fn checked_sum_options_u32(
    left: Option<u32>,
    right: Option<u32>,
    field: &'static str,
) -> Result<Option<u32>, ResourceGovernorError> {
    match (left, right) {
        (Some(left), Some(right)) => left
            .checked_add(right)
            .map(Some)
            .ok_or(ResourceGovernorError::ArithmeticOverflow(field)),
        _ => Ok(None),
    }
}

fn ceil_div_u64(left: u64, right: u64) -> Result<u64, ResourceGovernorError> {
    if right == 0 {
        return Err(ResourceGovernorError::ArithmeticOverflow(
            "division by zero",
        ));
    }
    let quotient = left / right;
    if left % right == 0 {
        Ok(quotient)
    } else {
        // A nonzero remainder implies `right >= 2`, so the quotient cannot be
        // `u64::MAX` and this increment is representable.
        Ok(quotient + 1)
    }
}

fn saturating_u16(value: u64) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::journal_boundary::CanaryJournal;
    use super::*;

    fn host() -> HostCapacity {
        HostCapacity {
            logical_cpu_count: Some(6),
            total_memory_bytes: Some(8 * GIB),
        }
    }

    fn governor() -> ResourceGovernor {
        ResourceGovernor::new(host(), ResourcePolicy::default()).unwrap()
    }

    fn sample(
        governor: &ResourceGovernor,
        sequence: u64,
        observed_at_millis: u64,
        stability_epoch: u64,
        stable_since_millis: u64,
        resident_workers: u16,
    ) -> ResourceSample {
        let evaluator_rss = u64::from(resident_workers) * 512 * MIB;
        let evaluator_cpu = u32::from(resident_workers) * 1_000;
        ResourceSample {
            cursor: TelemetryCursor {
                epoch: TelemetryEpoch(1),
                sequence,
                observed_at_millis,
            },
            stability: StabilityObservation {
                epoch: StabilityEpoch(stability_epoch),
                stable_since_millis,
                minimum_available_memory_bytes: Some(8 * GIB - evaluator_rss),
                minimum_idle_cpu_millicores: Some(6_000 - evaluator_cpu),
                minimum_memory_before_evaluator_charge_bytes: Some(8 * GIB),
                minimum_cpu_before_evaluator_charge_millicores: Some(6_000),
            },
            compile_epoch: None,
            pressure: MemoryPressure::Normal,
            oom_risk: false,
            available_memory_bytes: Some(8 * GIB - evaluator_rss),
            cpu: Some(CpuHeadroom {
                live_capacity_millicores: 6_000,
                idle_millicores: 6_000 - evaluator_cpu,
            }),
            swap_out: Some(SwapOutCounter {
                generation: SwapOutGeneration(1),
                cumulative_bytes: 0,
            }),
            evaluator: EvaluatorObservation {
                lease_generation: governor.lease_generation(),
                resident_workers,
                draining_workers: 0,
                reserved_workers: 0,
                aggregate_rss_bytes: Some(evaluator_rss),
                aggregate_cpu_millicores: Some(evaluator_cpu),
            },
            compiler: CompilerObservation {
                rss_bytes: Some(0),
                cpu_millicores: Some(0),
            },
        }
    }

    fn calibrated_governor() -> ResourceGovernor {
        let mut governor = governor();
        governor.calibration = Some(EvaluatorCalibration {
            lease_generation: LeaseGeneration(1),
            measured_at: TelemetryCursor {
                epoch: TelemetryEpoch(1),
                sequence: 1,
                observed_at_millis: 0,
            },
            stability_epoch: StabilityEpoch(1),
            measured_peak_rss_bytes: 256 * MIB,
            charged_worker_memory_bytes: 512 * MIB,
        });
        governor
    }

    fn outer_contained_governor() -> ResourceGovernor {
        let mut policy = ResourcePolicy::default();
        policy.configured_worker_ceiling = Some(1);
        policy.requested_jobs_ceiling = Some(1);
        policy.outer_contained_cold_worker_memory_charge_bytes = Some(256 * MIB);
        policy.swap_growth_authority = SwapGrowthAuthority::ValidatedOuterContainmentAdvisory;
        let mut governor = ResourceGovernor::new(host(), policy).unwrap();
        governor.calibration = calibrated_governor().calibration;
        governor
    }

    fn begin_scanning(governor: &mut ResourceGovernor) {
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        governor
            .transition(ResourceGovernorEvent::BeginScan(sample(
                governor, 2, 30_000, 1, 0, 0,
            )))
            .unwrap();
        assert_eq!(governor.phase(), GovernorPhase::Scanning);
        assert_eq!(governor.target_worker_leases(), 1);
    }

    fn append_shard_receipt(
        journal: &mut CanaryJournal,
        governor: &ResourceGovernor,
        sample: ResourceSample,
        shard_identity: [u8; 32],
    ) -> ShardCommitEvidence {
        journal.append_and_readback(
            governor.lease_generation(),
            sample.stability.epoch,
            sample.cursor,
            shard_identity,
        )
    }

    #[test]
    fn six_core_eight_gib_defaults_preserve_twenty_percent_before_workers() {
        let governor = calibrated_governor();
        let capacity = governor
            .assess_worker_capacity(sample(&governor, 1, 0, 1, 0, 0))
            .unwrap();
        assert_eq!(
            capacity.memory_reserve_bytes,
            ceil_div_u64(8 * GIB, 5).unwrap()
        );
        assert_eq!(capacity.cpu_reserve_millicores, 2_000);
        assert_eq!(capacity.memory_worker_ceiling, 12);
        assert_eq!(capacity.cpu_worker_ceiling, 4);
        assert_eq!(capacity.safe_worker_ceiling, 4);
    }

    #[test]
    fn advisory_swap_growth_requires_outer_containment_and_keeps_other_states_strict() {
        let mut policy = ResourcePolicy::default();
        policy.swap_growth_authority = SwapGrowthAuthority::ValidatedOuterContainmentAdvisory;

        assert!(matches!(
            ResourceGovernor::new(host(), policy),
            Err(ResourceGovernorError::InvalidPolicy(
                "advisory swap growth requires validated outer containment"
            ))
        ));
        assert!(policy
            .swap_growth_authority
            .admits(SwapAssessment::Unchanged));
        assert!(policy.swap_growth_authority.admits(SwapAssessment::Growth));
        for assessment in [
            SwapAssessment::Unknown,
            SwapAssessment::Baseline,
            SwapAssessment::CounterReset,
        ] {
            assert!(!policy.swap_growth_authority.admits(assessment));
        }
    }

    #[test]
    fn four_is_not_a_universal_worker_cap() {
        let mut governor = ResourceGovernor::new(
            HostCapacity {
                logical_cpu_count: Some(16),
                total_memory_bytes: Some(64 * GIB),
            },
            ResourcePolicy::default(),
        )
        .unwrap();
        governor.calibration = calibrated_governor().calibration;
        let mut roomy = sample(&governor, 1, 0, 1, 0, 0);
        roomy.available_memory_bytes = Some(64 * GIB);
        roomy.cpu = Some(CpuHeadroom {
            live_capacity_millicores: 16_000,
            idle_millicores: 16_000,
        });
        roomy.stability.minimum_available_memory_bytes = Some(64 * GIB);
        roomy.stability.minimum_idle_cpu_millicores = Some(16_000);
        roomy.stability.minimum_memory_before_evaluator_charge_bytes = Some(64 * GIB);
        roomy
            .stability
            .minimum_cpu_before_evaluator_charge_millicores = Some(16_000);
        let capacity = governor.assess_worker_capacity(roomy).unwrap();
        assert_eq!(capacity.cpu_reserve_millicores, 4_000);
        assert_eq!(capacity.safe_worker_ceiling, 12);
    }

    #[test]
    fn unknown_telemetry_and_first_swap_counter_admit_zero() {
        let mut state = governor();
        let mut unknown = sample(&state, 1, 0, 1, 0, 0);
        unknown.available_memory_bytes = None;
        unknown.swap_out = None;
        state
            .transition(ResourceGovernorEvent::Observe(unknown))
            .unwrap();
        assert_eq!(state.target_worker_leases(), 0);

        let mut fresh = governor();
        let decision = fresh
            .transition(ResourceGovernorEvent::Observe(sample(
                &fresh, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        assert_eq!(decision.target_worker_leases, 0);
        assert_eq!(decision.metadata.swap, SwapAssessment::Baseline);
    }

    #[test]
    fn zero_target_pre_admission_order_error_is_nonterminal() {
        let mut governor = governor();
        assert!(matches!(
            governor.transition(ResourceGovernorEvent::EndScan),
            Err(ResourceGovernorError::WrongPhase { .. })
        ));
        assert_eq!(governor.phase(), GovernorPhase::Idle);
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.lease_authority, LeaseAuthority::Active);
        assert_eq!(governor.failure, None);
    }

    #[test]
    fn cold_calibration_waits_when_reserve_plus_cold_charge_does_not_fit() {
        let mut governor = governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        let mut constrained = sample(&governor, 2, 30_000, 1, 0, 0);
        constrained.available_memory_bytes = Some(3 * GIB);
        constrained.stability.minimum_available_memory_bytes = Some(3 * GIB);
        constrained
            .stability
            .minimum_memory_before_evaluator_charge_bytes = Some(3 * GIB);
        let result = governor.transition(ResourceGovernorEvent::BeginOneWorkerCalibration(
            constrained,
        ));
        assert_eq!(result, Err(ResourceGovernorError::CapacityUnavailable));
        assert_eq!(governor.target_worker_leases(), 0);
    }

    #[test]
    fn measured_peak_is_bound_to_exact_calibration_generation_and_sample() {
        let mut governor = governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        governor
            .transition(ResourceGovernorEvent::BeginOneWorkerCalibration(sample(
                &governor, 2, 30_000, 1, 0, 0,
            )))
            .unwrap();
        let measured = sample(&governor, 3, 60_000, 1, 0, 1);
        let evidence = CalibrationPeakEvidence {
            lease_generation: governor.lease_generation(),
            measured_at: measured.cursor,
            stability_epoch: measured.stability.epoch,
            measured_peak_rss_bytes: 512 * MIB,
        };
        governor
            .transition(ResourceGovernorEvent::FinishOneWorkerCalibration {
                sample: measured,
                evidence,
            })
            .unwrap();
        let calibration = governor.calibration().unwrap();
        assert_eq!(calibration.lease_generation, evidence.lease_generation);
        assert_eq!(calibration.measured_at, evidence.measured_at);
        assert_eq!(calibration.charged_worker_memory_bytes, 768 * MIB);
    }

    #[test]
    fn rejected_calibration_evidence_revokes_the_live_worker_lease() {
        let mut governor = governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        governor
            .transition(ResourceGovernorEvent::BeginOneWorkerCalibration(sample(
                &governor, 2, 30_000, 1, 0, 0,
            )))
            .unwrap();
        let measured = sample(&governor, 3, 60_000, 1, 0, 1);
        let invalid = CalibrationPeakEvidence {
            lease_generation: governor.lease_generation(),
            measured_at: TelemetryCursor {
                sequence: 2,
                ..measured.cursor
            },
            stability_epoch: measured.stability.epoch,
            measured_peak_rss_bytes: 512 * MIB,
        };
        assert!(matches!(
            governor.transition(ResourceGovernorEvent::FinishOneWorkerCalibration {
                sample: measured,
                evidence: invalid,
            }),
            Err(ResourceGovernorError::InvalidCalibration(_))
        ));
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
        assert_eq!(
            governor.failure,
            Some(GovernorFailure::EventRejectedWhileWorkActive)
        );
    }

    #[test]
    fn warning_reduces_one_and_critical_reduces_to_zero() {
        let mut governor = calibrated_governor();
        begin_scanning(&mut governor);
        governor.change_target(2, 30_001).unwrap();
        let mut warning = sample(&governor, 3, 31_000, 1, 0, 2);
        warning.pressure = MemoryPressure::Warning;
        governor
            .transition(ResourceGovernorEvent::Observe(warning))
            .unwrap();
        assert_eq!(governor.target_worker_leases(), 1);

        let mut critical = sample(&governor, 4, 32_000, 2, 31_000, 1);
        critical.pressure = MemoryPressure::Critical;
        governor
            .transition(ResourceGovernorEvent::Observe(critical))
            .unwrap();
        assert_eq!(governor.target_worker_leases(), 0);
    }

    #[test]
    fn jobs_ceiling_caps_capacity_without_coercing_zero() {
        let mut policy = ResourcePolicy::default();
        policy.requested_jobs_ceiling = Some(2);
        let mut governor = ResourceGovernor::new(host(), policy).unwrap();
        governor.calibration = calibrated_governor().calibration;
        let capacity = governor
            .assess_worker_capacity(sample(&governor, 1, 0, 1, 0, 0))
            .unwrap();
        assert_eq!(capacity.safe_worker_ceiling, 2);

        policy.requested_jobs_ceiling = Some(0);
        assert!(matches!(
            ResourceGovernor::new(host(), policy),
            Err(ResourceGovernorError::InvalidPolicy(_))
        ));
    }

    #[test]
    fn sub_core_worker_or_compile_charge_is_rejected() {
        let mut worker_policy = ResourcePolicy::default();
        worker_policy.worker_cpu_charge_millicores = 999;
        assert!(matches!(
            ResourceGovernor::new(host(), worker_policy),
            Err(ResourceGovernorError::InvalidPolicy(_))
        ));

        let mut compile_policy = ResourcePolicy::default();
        compile_policy.minimum_compile_cpu_charge_millicores = 999;
        assert!(matches!(
            ResourceGovernor::new(host(), compile_policy),
            Err(ResourceGovernorError::InvalidPolicy(_))
        ));

        let mut evidence_policy = ResourcePolicy::default();
        evidence_policy.committed_shards_before_scale_up = MAX_RAMP_EVIDENCE_IDENTITIES + 1;
        assert!(matches!(
            ResourceGovernor::new(host(), evidence_policy),
            Err(ResourceGovernorError::InvalidPolicy(_))
        ));
    }

    #[test]
    fn unknown_installed_capacity_admits_no_automatic_work_or_compile() {
        let unknown_host = HostCapacity {
            logical_cpu_count: None,
            total_memory_bytes: None,
        };
        let mut calibration_state =
            ResourceGovernor::new(unknown_host, ResourcePolicy::default()).unwrap();
        calibration_state
            .transition(ResourceGovernorEvent::Observe(sample(
                &calibration_state,
                1,
                0,
                1,
                0,
                0,
            )))
            .unwrap();
        let decision = calibration_state
            .transition(ResourceGovernorEvent::BeginOneWorkerCalibration(sample(
                &calibration_state,
                2,
                30_000,
                1,
                0,
                0,
            )))
            .unwrap();
        assert_eq!(decision.target_worker_leases, 0);
        assert_eq!(decision.metadata.capacity.unwrap().safe_worker_ceiling, 0);

        let mut compile_state =
            ResourceGovernor::new(unknown_host, ResourcePolicy::default()).unwrap();
        compile_state
            .transition(ResourceGovernorEvent::Observe(sample(
                &compile_state,
                1,
                0,
                1,
                0,
                0,
            )))
            .unwrap();
        let compile_decision = compile_state
            .transition(ResourceGovernorEvent::BeginCompile {
                epoch: CompileEpoch(1),
                charge: CompileCharge {
                    memory_bytes: 2 * GIB,
                    cpu_millicores: 1_000,
                },
                sample: sample(&compile_state, 2, 30_000, 1, 0, 0),
            })
            .unwrap();
        assert_eq!(compile_decision.phase, GovernorPhase::Idle);
        assert_eq!(compile_decision.target_worker_leases, 0);
    }

    #[test]
    fn aggregate_owned_usage_is_reconstructed_before_every_target_is_charged() {
        let governor = calibrated_governor();
        let mut two_residents = sample(&governor, 1, 0, 1, 0, 2);
        two_residents.available_memory_bytes = Some(3 * GIB);
        two_residents.evaluator.aggregate_rss_bytes = Some(1 * GIB);
        two_residents.stability.minimum_available_memory_bytes = Some(3 * GIB);
        two_residents
            .stability
            .minimum_memory_before_evaluator_charge_bytes = Some(4 * GIB);
        let capacity = governor.assess_worker_capacity(two_residents).unwrap();
        assert_eq!(
            capacity.current_memory_before_worker_charge_bytes,
            Some(4 * GIB)
        );
        assert_eq!(capacity.memory_worker_ceiling, 4);
    }

    #[test]
    fn draining_residents_are_charged_and_cannot_be_replaced() {
        let mut policy = ResourcePolicy::default();
        policy.requested_jobs_ceiling = Some(1);
        let mut governor = ResourceGovernor::new(host(), policy).unwrap();
        governor.calibration = calibrated_governor().calibration;
        begin_scanning(&mut governor);

        let mut draining = sample(&governor, 3, 31_000, 1, 0, 2);
        draining.evaluator.draining_workers = 1;
        let decision = governor
            .transition(ResourceGovernorEvent::Observe(draining))
            .unwrap();
        assert_eq!(
            decision
                .metadata
                .capacity
                .unwrap()
                .charged_worker_commitments,
            2
        );
        assert_eq!(decision.target_worker_leases, 0);
        assert_eq!(decision.phase, GovernorPhase::Draining);
        assert_eq!(decision.reason, DecisionReason::Draining);
        assert_eq!(
            decision.metadata.drain_trigger,
            Some(DecisionReason::DrainingWorkersBackoff)
        );
    }

    #[test]
    fn reserved_workers_are_charged_as_live_commitments() {
        let governor = calibrated_governor();
        let mut reserved = sample(&governor, 1, 0, 1, 0, 0);
        reserved.evaluator.reserved_workers = 2;
        let capacity = governor.assess_worker_capacity(reserved).unwrap();
        assert_eq!(capacity.charged_worker_commitments, 2);
    }

    #[test]
    fn normal_observation_does_not_overwrite_draining_reason() {
        let mut governor = calibrated_governor();
        governor.phase = GovernorPhase::Draining;
        let mut draining = sample(&governor, 1, 0, 1, 0, 1);
        draining.evaluator.draining_workers = 1;
        let decision = governor
            .transition(ResourceGovernorEvent::Observe(draining))
            .unwrap();
        assert_eq!(decision.phase, GovernorPhase::Draining);
        assert_eq!(decision.reason, DecisionReason::Draining);
    }

    #[test]
    fn stale_zero_observation_cannot_finish_a_later_drain() {
        let mut governor = calibrated_governor();
        governor.phase = GovernorPhase::Scanning;
        governor.target_worker_leases = 1;
        governor.transition(ResourceGovernorEvent::EndScan).unwrap();
        let current_generation = governor.lease_generation();
        let mut stale = sample(&governor, 1, 0, 1, 0, 0);
        stale.evaluator.lease_generation = LeaseGeneration(current_generation.0 - 1);
        assert!(matches!(
            governor.transition(ResourceGovernorEvent::Observe(stale)),
            Err(ResourceGovernorError::StaleResidentObservation { .. })
        ));
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
    }

    #[test]
    fn rejected_candidate_does_not_partially_commit_swap_or_cursor_state() {
        let mut governor = governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        governor.phase = GovernorPhase::Scanning;
        governor.target_worker_leases = 1;
        let committed_swap = governor.last_swap;
        let committed_cursor = governor.last_cursor;

        let mut overflowing = sample(&governor, 2, 1, 1, 0, 1);
        overflowing.available_memory_bytes = Some(u64::MAX);
        overflowing.swap_out.as_mut().unwrap().cumulative_bytes = 1;
        assert!(matches!(
            governor.transition(ResourceGovernorEvent::Observe(overflowing)),
            Err(ResourceGovernorError::ArithmeticOverflow(_))
        ));
        assert_eq!(governor.last_swap, committed_swap);
        assert_eq!(governor.last_cursor, committed_cursor);
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
    }

    #[test]
    fn incoherent_swap_counter_revokes_a_live_scan_without_partial_commit() {
        let mut governor = calibrated_governor();
        let mut baseline = sample(&governor, 1, 0, 1, 0, 0);
        baseline.swap_out.as_mut().unwrap().cumulative_bytes = 10;
        governor
            .transition(ResourceGovernorEvent::Observe(baseline))
            .unwrap();
        let mut begin = sample(&governor, 2, 30_000, 1, 0, 0);
        begin.swap_out.as_mut().unwrap().cumulative_bytes = 10;
        governor
            .transition(ResourceGovernorEvent::BeginScan(begin))
            .unwrap();
        let committed_cursor = governor.last_cursor;
        let committed_swap = governor.last_swap;

        let mut decreasing = sample(&governor, 3, 31_000, 1, 0, 1);
        decreasing.swap_out.as_mut().unwrap().cumulative_bytes = 9;
        assert_eq!(
            governor.transition(ResourceGovernorEvent::Observe(decreasing)),
            Err(ResourceGovernorError::NonMonotonicSwapCounter)
        );
        assert_eq!(governor.last_cursor, committed_cursor);
        assert_eq!(governor.last_swap, committed_swap);
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
    }

    #[test]
    fn lease_generation_overflow_revokes_authority_and_stops_target() {
        let mut governor = calibrated_governor();
        governor.phase = GovernorPhase::Scanning;
        governor.target_worker_leases = 1;
        governor.lease_generation = LeaseGeneration(u64::MAX);
        assert!(matches!(
            governor.transition(ResourceGovernorEvent::EndScan),
            Err(ResourceGovernorError::GenerationOverflow(
                "lease_generation"
            ))
        ));
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
        assert_eq!(governor.lease_generation, LeaseGeneration(u64::MAX));
    }

    #[test]
    fn ramp_is_one_at_a_time_and_waits_for_new_generation_residents() {
        let mut governor = calibrated_governor();
        let mut journal = CanaryJournal::new();
        begin_scanning(&mut governor);
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 3, 31_000, 1, 0, 1,
            )))
            .unwrap();
        for (sequence, time) in [(4, 32_000), (5, 61_000)] {
            let shard_sample = sample(&governor, sequence, time, 1, 0, 1);
            let evidence =
                append_shard_receipt(&mut journal, &governor, shard_sample, [sequence as u8; 32]);
            governor
                .transition(ResourceGovernorEvent::CommitScanShard {
                    sample: shard_sample,
                    evidence,
                })
                .unwrap();
        }
        assert_eq!(governor.target_worker_leases(), 2);
        let generation_two = governor.lease_generation();

        let one_resident = sample(&governor, 6, 62_000, 1, 0, 1);
        governor
            .transition(ResourceGovernorEvent::Observe(one_resident))
            .unwrap();
        assert_eq!(governor.target_worker_leases(), 2);
        assert_eq!(governor.lease_generation(), generation_two);
        assert!(governor.ramp.is_none());

        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 7, 63_000, 1, 0, 2,
            )))
            .unwrap();
        for (sequence, time) in [(8, 64_000), (9, 93_000)] {
            let shard_sample = sample(&governor, sequence, time, 1, 0, 2);
            let evidence =
                append_shard_receipt(&mut journal, &governor, shard_sample, [sequence as u8; 32]);
            governor
                .transition(ResourceGovernorEvent::CommitScanShard {
                    sample: shard_sample,
                    evidence,
                })
                .unwrap();
        }
        assert_eq!(governor.target_worker_leases(), 3);
    }

    #[test]
    fn replayed_shard_identity_across_ramp_reset_revokes_live_scan_authority() {
        let mut governor = calibrated_governor();
        let mut journal = CanaryJournal::new();
        begin_scanning(&mut governor);
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 3, 31_000, 1, 0, 1,
            )))
            .unwrap();
        let first = sample(&governor, 4, 32_000, 1, 0, 1);
        let first_evidence = append_shard_receipt(&mut journal, &governor, first, [7; 32]);
        let first_receipt = journal.last_receipt_index();
        governor
            .transition(ResourceGovernorEvent::CommitScanShard {
                sample: first,
                evidence: first_evidence,
            })
            .unwrap();

        let replay = sample(&governor, 5, 62_000, 2, 32_000, 1);
        assert_eq!(
            governor.transition(ResourceGovernorEvent::CommitScanShard {
                sample: replay,
                evidence: journal.readback(first_receipt),
            }),
            Err(ResourceGovernorError::ShardEvidenceMismatch)
        );
        assert_eq!(governor.target_worker_leases(), 0);
        assert_eq!(governor.lease_authority, LeaseAuthority::Revoked);
        assert_eq!(
            governor.failure,
            Some(GovernorFailure::EventRejectedWhileWorkActive)
        );
    }

    #[test]
    fn repeated_one_commit_ramp_resets_keep_identity_memory_constant() {
        let mut governor = calibrated_governor();
        let mut journal = CanaryJournal::new();
        begin_scanning(&mut governor);
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 3, 31_000, 1, 0, 1,
            )))
            .unwrap();

        for step in 0_u64..16 {
            let stable_since = 31_000 + step * 30_000;
            let shard_sample = sample(
                &governor,
                4 + step,
                stable_since + 30_000,
                2 + step,
                stable_since,
                1,
            );
            let evidence = append_shard_receipt(
                &mut journal,
                &governor,
                shard_sample,
                [(step as u8) + 1; 32],
            );
            governor
                .transition(ResourceGovernorEvent::CommitScanShard {
                    sample: shard_sample,
                    evidence,
                })
                .unwrap();
            let ramp = governor.ramp.as_ref().unwrap();
            assert_eq!(ramp.committed_shards, 1);
            assert_eq!(ramp.counted_shard_identities.len(), 1);
            assert!(
                ramp.counted_shard_identities.len()
                    <= usize::from(governor.policy.committed_shards_before_scale_up)
            );
        }
        assert_eq!(
            governor
                .last_durable_commit
                .map(DurableCommitCursor::sequence),
            Some(16)
        );
    }

    #[test]
    fn swap_growth_and_counter_generation_reset_clear_ramp_and_stop() {
        let mut governor = calibrated_governor();
        begin_scanning(&mut governor);
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 3, 31_000, 1, 0, 1,
            )))
            .unwrap();
        assert!(governor.ramp.is_some());

        let mut reset = sample(&governor, 4, 32_000, 1, 0, 1);
        reset.swap_out.as_mut().unwrap().generation = SwapOutGeneration(2);
        governor
            .transition(ResourceGovernorEvent::Observe(reset))
            .unwrap();
        assert_eq!(governor.target_worker_leases(), 0);
        assert!(governor.ramp.is_none());
    }

    #[test]
    fn swap_growth_overrides_warning_and_stops_all_workers() {
        let mut governor = calibrated_governor();
        begin_scanning(&mut governor);
        governor.change_target(2, 30_001).unwrap();
        let mut growth = sample(&governor, 3, 31_000, 1, 0, 2);
        growth.pressure = MemoryPressure::Warning;
        growth.swap_out.as_mut().unwrap().cumulative_bytes = 1;
        let decision = governor
            .transition(ResourceGovernorEvent::Observe(growth))
            .unwrap();
        assert_eq!(decision.target_worker_leases, 0);
        assert_eq!(decision.reason, DecisionReason::Draining);
        assert_eq!(
            decision.metadata.drain_trigger,
            Some(DecisionReason::SwapGrowthBackoff)
        );
    }

    #[test]
    fn outer_contained_swap_growth_remains_observable_without_stopping_work() {
        let mut governor = outer_contained_governor();
        begin_scanning(&mut governor);
        let mut growth = sample(&governor, 3, 31_000, 1, 0, 1);
        growth.swap_out.as_mut().unwrap().cumulative_bytes = 1;

        let decision = governor
            .transition(ResourceGovernorEvent::Observe(growth))
            .unwrap();

        assert_eq!(decision.phase, GovernorPhase::Scanning);
        assert_eq!(decision.target_worker_leases, 1);
        assert_eq!(decision.reason, DecisionReason::Holding);
        assert_eq!(decision.metadata.swap, SwapAssessment::Growth);
        assert!(decision.metadata.stable);
        assert_eq!(decision.metadata.drain_trigger, None);
        assert!(governor.ramp.is_some());
    }

    #[test]
    fn advancing_external_stability_epoch_discards_old_shard_evidence() {
        let mut governor = calibrated_governor();
        let mut journal = CanaryJournal::new();
        begin_scanning(&mut governor);
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 3, 31_000, 1, 0, 1,
            )))
            .unwrap();
        let first = sample(&governor, 4, 32_000, 1, 0, 1);
        let first_evidence = append_shard_receipt(&mut journal, &governor, first, [1; 32]);
        governor
            .transition(ResourceGovernorEvent::CommitScanShard {
                evidence: first_evidence,
                sample: first,
            })
            .unwrap();
        assert_eq!(governor.ramp.as_ref().unwrap().committed_shards, 1);

        let advanced = sample(&governor, 5, 62_000, 2, 32_000, 1);
        let advanced_evidence = append_shard_receipt(&mut journal, &governor, advanced, [2; 32]);
        governor
            .transition(ResourceGovernorEvent::CommitScanShard {
                evidence: advanced_evidence,
                sample: advanced,
            })
            .unwrap();
        assert_eq!(governor.ramp.as_ref().unwrap().committed_shards, 1);
        assert_eq!(governor.target_worker_leases(), 1);
    }

    #[test]
    fn live_cpu_headroom_can_hold_cold_calibration_at_zero() {
        let mut governor = governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        let mut busy = sample(&governor, 2, 30_000, 1, 0, 0);
        busy.cpu.as_mut().unwrap().idle_millicores = 1_000;
        busy.stability.minimum_idle_cpu_millicores = Some(1_000);
        busy.stability
            .minimum_cpu_before_evaluator_charge_millicores = Some(1_000);
        let decision = governor
            .transition(ResourceGovernorEvent::BeginOneWorkerCalibration(busy))
            .unwrap();
        assert_eq!(decision.target_worker_leases, 0);
        assert_eq!(decision.reason, DecisionReason::ReserveBackoff);
    }

    #[test]
    fn compile_requires_fresh_stability_and_never_overlaps_residents() {
        let mut governor = calibrated_governor();
        let charge = CompileCharge {
            memory_bytes: 2 * GIB,
            cpu_millicores: 1_000,
        };
        let first = sample(&governor, 1, 0, 1, 0, 0);
        let first_decision = governor
            .transition(ResourceGovernorEvent::BeginCompile {
                epoch: CompileEpoch(1),
                charge,
                sample: first,
            })
            .unwrap();
        assert_eq!(first_decision.target_worker_leases, 0);
        assert_eq!(first_decision.phase, GovernorPhase::Idle);
        assert_eq!(first_decision.metadata.swap, SwapAssessment::Baseline);
        let stable = sample(&governor, 2, 30_000, 1, 0, 0);
        governor
            .transition(ResourceGovernorEvent::BeginCompile {
                epoch: CompileEpoch(1),
                charge,
                sample: stable,
            })
            .unwrap();

        let mut overlap = sample(&governor, 3, 31_000, 1, 0, 1);
        overlap.compile_epoch = Some(CompileEpoch(1));
        governor
            .transition(ResourceGovernorEvent::Observe(overlap))
            .unwrap();
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(
            governor.failure,
            Some(GovernorFailure::EvaluatorResidentDuringCompile)
        );
    }

    #[test]
    fn compile_monitor_rejects_incomplete_stability_window_telemetry() {
        let mut governor = calibrated_governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        governor
            .transition(ResourceGovernorEvent::BeginCompile {
                epoch: CompileEpoch(1),
                charge: CompileCharge {
                    memory_bytes: 2 * GIB,
                    cpu_millicores: 1_000,
                },
                sample: sample(&governor, 2, 30_000, 1, 0, 0),
            })
            .unwrap();

        let mut incomplete = sample(&governor, 3, 61_000, 2, 30_000, 0);
        incomplete.compile_epoch = Some(CompileEpoch(1));
        incomplete
            .stability
            .minimum_memory_before_evaluator_charge_bytes = None;
        governor
            .transition(ResourceGovernorEvent::Observe(incomplete))
            .unwrap();
        assert_eq!(governor.phase(), GovernorPhase::Failed);
        assert_eq!(
            governor.failure,
            Some(GovernorFailure::UnsafeTelemetryDuringCompile)
        );
    }

    #[test]
    fn compile_end_invalidates_calibration_and_requires_post_compile_epoch() {
        let mut governor = calibrated_governor();
        governor
            .transition(ResourceGovernorEvent::Observe(sample(
                &governor, 1, 0, 1, 0, 0,
            )))
            .unwrap();
        let charge = CompileCharge {
            memory_bytes: 2 * GIB,
            cpu_millicores: 1_000,
        };
        governor
            .transition(ResourceGovernorEvent::BeginCompile {
                epoch: CompileEpoch(1),
                charge,
                sample: sample(&governor, 2, 30_000, 1, 0, 0),
            })
            .unwrap();
        let mut end = sample(&governor, 3, 31_000, 1, 0, 0);
        end.compile_epoch = Some(CompileEpoch(1));
        governor
            .transition(ResourceGovernorEvent::EndCompile {
                epoch: CompileEpoch(1),
                sample: end,
            })
            .unwrap();
        assert!(governor.calibration().is_none());
        assert!(governor.post_compile_cutoff.is_some());

        let old_epoch = sample(&governor, 4, 62_000, 1, 0, 0);
        governor
            .transition(ResourceGovernorEvent::Observe(old_epoch))
            .unwrap();
        assert!(governor.post_compile_cutoff.is_some());

        let fresh_epoch = sample(&governor, 5, 93_000, 2, 62_000, 0);
        governor
            .transition(ResourceGovernorEvent::Observe(fresh_epoch))
            .unwrap();
        assert!(governor.post_compile_cutoff.is_none());
    }
}
