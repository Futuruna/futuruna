//! Crash-conservative host telemetry preparation for the resource governor.
//!
//! The platform-neutral reducer in this module accepts either one complete
//! host observation or no host observation at all. It never joins partial
//! facts from different sampling attempts. Evaluator and compiler usage is
//! supplied by the coordinator as one owned-process snapshot and is copied
//! verbatim into the governor sample.
//!
//! Host collection is deliberately a separate boundary. The macOS provider
//! runs one deadline-bounded, boot-bracketed command transaction and publishes
//! facts only when every command, parser, and coherence check succeeds.
//! Unknown telemetry is therefore an admission-zero result, never optimistic
//! capacity.

use std::cmp;
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use std::io::Read;
#[cfg(target_os = "macos")]
use std::process::{Child, Command, Stdio};

use super::resource_governor::{
    CompileEpoch, CompilerObservation, CpuHeadroom, EvaluatorObservation, HostCapacity,
    MemoryPressure, ResourceSample, StabilityEpoch, StabilityObservation, SwapOutCounter,
    TelemetryCursor, TelemetryEpoch,
};

/// Frozen API contract consumed from `resource_governor.rs`.
pub(crate) const RESOURCE_GOVERNOR_CONTRACT_SHA256: &str =
    "007b57b7e4a41ff40d40132df22b57062225cfc75d77cf7148685491a3f93bf5";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SampleUnavailable {
    ProviderUnavailable,
    MissingHostFact(&'static str),
    InvalidScalar(&'static str),
    ZeroEpoch(&'static str),
    EpochOverflow(&'static str),
    CursorOverflow,
    NonMonotonicClock,
    NonMonotonicSourceGeneration,
    NonMonotonicLeaseGeneration,
    NonMonotonicSwapCounter,
    IncoherentCompileTransition,
    IncoherentHostFacts,
    IncoherentOwnedSnapshot,
    SnapshotRace,
    InvalidWatchdog,
}

impl SampleUnavailable {
    /// Stable, non-sensitive category used when a fail-closed stream pause
    /// needs to explain why the complete provider transaction was discarded.
    pub(crate) fn diagnostic_code(self) -> &'static str {
        match self {
            Self::ProviderUnavailable => "telemetry_provider_unavailable",
            Self::MissingHostFact(field) => match field {
                "vm_stat_header" => "telemetry_missing_vm_stat_header",
                "pages_active" => "telemetry_missing_pages_active",
                "pages_free" => "telemetry_missing_pages_free",
                "pages_inactive" => "telemetry_missing_pages_inactive",
                "pages_speculative" => "telemetry_missing_pages_speculative",
                "pages_throttled" => "telemetry_missing_pages_throttled",
                "swapouts" => "telemetry_missing_swapouts",
                "top_cpu" => "telemetry_missing_top_cpu",
                _ => "telemetry_missing_host_fact",
            },
            Self::InvalidScalar(field) => match field {
                "boot_time" | "boot_seconds" | "boot_microseconds" => "telemetry_invalid_boot_time",
                "host_scalars" => "telemetry_invalid_host_scalars",
                "live_scalars" => "telemetry_invalid_live_scalars",
                "logical_cpu_count" => "telemetry_invalid_logical_cpu_count",
                "active_cpu_count" => "telemetry_invalid_active_cpu_count",
                "total_memory_bytes" => "telemetry_invalid_total_memory",
                "memory_pressure" => "telemetry_invalid_memory_pressure",
                "vm_stat" | "vm_stat_header" | "vm_stat_page_size" | "vm_stat_record" => {
                    "telemetry_invalid_vm_stat"
                }
                "top" => "telemetry_invalid_top_report",
                "top_cpu" => "telemetry_invalid_top_cpu_line",
                "cpu_percentage" => "telemetry_invalid_top_cpu_percentage",
                _ => "telemetry_invalid_scalar",
            },
            Self::IncoherentHostFacts => "telemetry_incoherent_host_facts",
            Self::SnapshotRace => "telemetry_snapshot_race",
            _ => "telemetry_incoherent_sample",
        }
    }
}

/// Complete host facts from one collection attempt. A provider must return an
/// error instead of constructing this value when any field is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawHostFacts {
    pub(crate) logical_cpu_count: u16,
    pub(crate) total_memory_bytes: u64,
    pub(crate) live_capacity_millicores: u32,
    pub(crate) idle_millicores: u32,
    /// Conservative reclaimable memory, never swap occupancy.
    pub(crate) available_memory_bytes: u64,
    /// XNU non-compressed reclaimable memory used only when an independently
    /// validated outer process-group supervisor owns the hard containment
    /// boundary. This includes active pages in addition to the conservative
    /// standalone queues above.
    pub(crate) outer_contained_available_memory_bytes: u64,
    pub(crate) pressure: MemoryPressure,
    pub(crate) oom_risk: bool,
    /// Boot-bound cumulative swap-out bytes. A boot/reset must use a newer
    /// generation; a counter may not decrease within one generation.
    pub(crate) swap_out: SwapOutCounter,
}

/// One atomically captured observation of every coordinator-owned evaluator
/// and compiler process. The caller, not the host provider, owns process
/// attribution and generation correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnedProcessSnapshot {
    pub(crate) evaluator: EvaluatorObservation,
    pub(crate) compiler: CompilerObservation,
    pub(crate) compile_epoch: Option<CompileEpoch>,
}

/// Input for one reducer transition. `host = None` means the complete host
/// transaction failed; the reducer emits unknown host telemetry while keeping
/// the exact owned-process snapshot so existing work cannot disappear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawHostSample {
    /// Durable provider generation. A restarted provider must use a larger
    /// value; this advances the output telemetry epoch and resets stability.
    pub(crate) source_generation: NonZeroU64,
    pub(crate) observed_at_millis: u64,
    pub(crate) host: Option<RawHostFacts>,
    pub(crate) owned: OwnedProcessSnapshot,
}

impl RawHostSample {
    /// Collapse every provider error to the single conservative host-unknown
    /// representation while retaining the exact owned-process observation.
    pub(crate) fn from_provider_result(
        source_generation: NonZeroU64,
        observed_at_millis: u64,
        host: Result<RawHostFacts, SampleUnavailable>,
        owned: OwnedProcessSnapshot,
    ) -> Self {
        Self {
            source_generation,
            observed_at_millis,
            host: host.ok(),
            owned,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReducerEpochSeed {
    pub(crate) telemetry: NonZeroU64,
    pub(crate) stability: NonZeroU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StabilityPressurePolicy {
    NormalOnly,
    NormalOrOuterContainedWarning,
}

impl StabilityPressurePolicy {
    const fn admits(self, pressure: MemoryPressure) -> bool {
        match pressure {
            MemoryPressure::Normal => true,
            MemoryPressure::Warning => {
                matches!(self, Self::NormalOrOuterContainedWarning)
            }
            MemoryPressure::Critical | MemoryPressure::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReducedResourceSample {
    pub(crate) capacity: HostCapacity,
    pub(crate) sample: ResourceSample,
    /// Explicit coordinator guard. This is true for unavailable host facts;
    /// callers must not use a separate override to admit automatic work.
    pub(crate) force_zero_admission: bool,
}

#[derive(Debug, Clone, Copy)]
struct StableWindow {
    since_millis: u64,
    minimum_available_memory_bytes: u64,
    minimum_idle_cpu_millicores: u32,
    minimum_memory_before_evaluator_charge_bytes: u64,
    minimum_cpu_before_evaluator_charge_millicores: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct StabilityWindowReducer {
    pressure_policy: StabilityPressurePolicy,
    telemetry_epoch: TelemetryEpoch,
    stability_epoch: StabilityEpoch,
    sequence: u64,
    last_observed_at_millis: Option<u64>,
    last_source_generation: Option<NonZeroU64>,
    last_swap: Option<SwapOutCounter>,
    last_lease_generation: Option<super::resource_governor::LeaseGeneration>,
    last_compile_epoch: Option<CompileEpoch>,
    stable: Option<StableWindow>,
}

impl StabilityWindowReducer {
    pub(crate) fn new(seed: ReducerEpochSeed, pressure_policy: StabilityPressurePolicy) -> Self {
        Self {
            pressure_policy,
            telemetry_epoch: TelemetryEpoch(seed.telemetry.get()),
            stability_epoch: StabilityEpoch(seed.stability.get()),
            sequence: 0,
            last_observed_at_millis: None,
            last_source_generation: None,
            last_swap: None,
            last_lease_generation: None,
            last_compile_epoch: None,
            stable: None,
        }
    }

    /// Transactional reduction. Errors leave reducer state unchanged and mean
    /// automatic admission is zero; callers must not replay an older sample.
    pub(crate) fn reduce(
        &mut self,
        raw: RawHostSample,
    ) -> Result<ReducedResourceSample, SampleUnavailable> {
        let mut next = self.clone();
        let reduced = next.reduce_inner(raw)?;
        *self = next;
        Ok(reduced)
    }

    fn reduce_inner(
        &mut self,
        raw: RawHostSample,
    ) -> Result<ReducedResourceSample, SampleUnavailable> {
        self.validate_owned(raw.owned)?;
        if self
            .last_observed_at_millis
            .is_some_and(|previous| raw.observed_at_millis <= previous)
        {
            return Err(SampleUnavailable::NonMonotonicClock);
        }

        let source_reset = match self.last_source_generation {
            None => false,
            Some(previous) if raw.source_generation < previous => {
                return Err(SampleUnavailable::NonMonotonicSourceGeneration);
            }
            Some(previous) => raw.source_generation > previous,
        };
        let mut telemetry_reset = source_reset;
        if telemetry_reset {
            self.telemetry_epoch =
                TelemetryEpoch(next_epoch(self.telemetry_epoch.0, "telemetry_epoch")?);
            self.sequence = 0;
        }
        self.sequence = match self.sequence.checked_add(1) {
            Some(sequence) => sequence,
            None => {
                self.telemetry_epoch =
                    TelemetryEpoch(next_epoch(self.telemetry_epoch.0, "telemetry_epoch")?);
                telemetry_reset = true;
                1
            }
        };

        let cursor = TelemetryCursor {
            epoch: self.telemetry_epoch,
            sequence: self.sequence,
            observed_at_millis: raw.observed_at_millis,
        };
        let owned = raw.owned;
        let ownership_boundary = self.ownership_boundary(owned)?;
        let host = match raw.host {
            Some(host) => Some(self.validate_host(host, owned)?),
            None => None,
        };

        let swap_state = self.swap_state(host.map(|value| value.swap_out), telemetry_reset)?;
        let current_window = host.and_then(|host| {
            let evaluator_rss = owned.evaluator.aggregate_rss_bytes?;
            let evaluator_cpu = owned.evaluator.aggregate_cpu_millicores?;
            let memory_before = host.available_memory_bytes.checked_add(evaluator_rss)?;
            let cpu_before = host.idle_millicores.checked_add(evaluator_cpu)?;
            Some((host, memory_before, cpu_before))
        });
        let stable_now = current_window.is_some_and(|(host, _, _)| {
            self.pressure_policy.admits(host.pressure)
                && !host.oom_risk
                && !telemetry_reset
                && matches!(swap_state, SwapState::Unchanged)
        });

        if stable_now {
            let (host, memory_before, cpu_before) =
                current_window.ok_or(SampleUnavailable::SnapshotRace)?;
            if ownership_boundary {
                // A lease boundary invalidates the *duration* and minima of
                // the preceding worker population, not the complete host facts
                // observed at this boundary. Seed the new epoch at age zero so
                // the governor can retain the new target while waiting for its
                // fresh stability duration instead of confusing the boundary
                // with missing telemetry and immediately revoking it.
                self.advance_stability_epoch()?;
                self.stable = None;
            }
            match &mut self.stable {
                Some(window) => {
                    window.minimum_available_memory_bytes = cmp::min(
                        window.minimum_available_memory_bytes,
                        host.available_memory_bytes,
                    );
                    window.minimum_idle_cpu_millicores =
                        cmp::min(window.minimum_idle_cpu_millicores, host.idle_millicores);
                    window.minimum_memory_before_evaluator_charge_bytes = cmp::min(
                        window.minimum_memory_before_evaluator_charge_bytes,
                        memory_before,
                    );
                    window.minimum_cpu_before_evaluator_charge_millicores = cmp::min(
                        window.minimum_cpu_before_evaluator_charge_millicores,
                        cpu_before,
                    );
                }
                None => {
                    self.advance_stability_epoch()?;
                    self.stable = Some(StableWindow {
                        since_millis: raw.observed_at_millis,
                        minimum_available_memory_bytes: host.available_memory_bytes,
                        minimum_idle_cpu_millicores: host.idle_millicores,
                        minimum_memory_before_evaluator_charge_bytes: memory_before,
                        minimum_cpu_before_evaluator_charge_millicores: cpu_before,
                    });
                }
            }
        } else {
            // Gaps, unknown/raised pressure, source reset, first/reset/growing
            // swap evidence, and incoherent arithmetic never inherit a window.
            self.advance_stability_epoch()?;
            self.stable = None;
        }

        let stability = match self.stable {
            Some(window) => StabilityObservation {
                epoch: self.stability_epoch,
                stable_since_millis: window.since_millis,
                minimum_available_memory_bytes: Some(window.minimum_available_memory_bytes),
                minimum_idle_cpu_millicores: Some(window.minimum_idle_cpu_millicores),
                minimum_memory_before_evaluator_charge_bytes: Some(
                    window.minimum_memory_before_evaluator_charge_bytes,
                ),
                minimum_cpu_before_evaluator_charge_millicores: Some(
                    window.minimum_cpu_before_evaluator_charge_millicores,
                ),
            },
            None => StabilityObservation {
                epoch: self.stability_epoch,
                stable_since_millis: raw.observed_at_millis,
                minimum_available_memory_bytes: None,
                minimum_idle_cpu_millicores: None,
                minimum_memory_before_evaluator_charge_bytes: None,
                minimum_cpu_before_evaluator_charge_millicores: None,
            },
        };

        self.last_observed_at_millis = Some(raw.observed_at_millis);
        self.last_source_generation = Some(raw.source_generation);
        self.last_lease_generation = Some(owned.evaluator.lease_generation);
        self.last_compile_epoch = owned.compile_epoch;

        let (capacity, pressure, oom_risk, available_memory_bytes, cpu, swap_out) = match host {
            Some(host) => (
                HostCapacity {
                    logical_cpu_count: Some(host.logical_cpu_count),
                    total_memory_bytes: Some(host.total_memory_bytes),
                },
                host.pressure,
                host.oom_risk,
                Some(host.available_memory_bytes),
                Some(CpuHeadroom {
                    live_capacity_millicores: host.live_capacity_millicores,
                    idle_millicores: host.idle_millicores,
                }),
                Some(host.swap_out),
            ),
            None => (
                HostCapacity {
                    logical_cpu_count: None,
                    total_memory_bytes: None,
                },
                MemoryPressure::Unknown,
                true,
                None,
                None,
                None,
            ),
        };
        Ok(ReducedResourceSample {
            capacity,
            force_zero_admission: host.is_none(),
            sample: ResourceSample {
                cursor,
                stability,
                compile_epoch: owned.compile_epoch,
                pressure,
                oom_risk,
                available_memory_bytes,
                cpu,
                swap_out,
                evaluator: owned.evaluator,
                compiler: owned.compiler,
            },
        })
    }

    fn validate_owned(&self, owned: OwnedProcessSnapshot) -> Result<(), SampleUnavailable> {
        if owned.evaluator.lease_generation.0 == 0
            || owned.evaluator.draining_workers > owned.evaluator.resident_workers
            || owned.evaluator.aggregate_rss_bytes.is_none()
            || owned.evaluator.aggregate_cpu_millicores.is_none()
            || owned.compiler.rss_bytes.is_none()
            || owned.compiler.cpu_millicores.is_none()
            || owned.compile_epoch.is_some_and(|epoch| epoch.0 == 0)
        {
            return Err(SampleUnavailable::IncoherentOwnedSnapshot);
        }
        if owned.evaluator.resident_workers == 0
            && (owned.evaluator.aggregate_rss_bytes != Some(0)
                || owned.evaluator.aggregate_cpu_millicores != Some(0))
        {
            return Err(SampleUnavailable::IncoherentOwnedSnapshot);
        }
        Ok(())
    }

    /// A compile entry deliberately inherits the already-complete host
    /// stability window: the governor checks that window before starting the
    /// compiler and requires it while monitoring the compile. Every other
    /// lease boundary, and especially compile exit, begins fresh stability
    /// evidence so a governor-blocked epoch cannot remain current forever.
    fn ownership_boundary(&self, owned: OwnedProcessSnapshot) -> Result<bool, SampleUnavailable> {
        let Some(previous_lease) = self.last_lease_generation else {
            return Ok(false);
        };
        if owned.evaluator.lease_generation < previous_lease {
            return Err(SampleUnavailable::NonMonotonicLeaseGeneration);
        }
        let lease_changed = owned.evaluator.lease_generation > previous_lease;
        match (self.last_compile_epoch, owned.compile_epoch) {
            (None, Some(_)) if !lease_changed => {
                Err(SampleUnavailable::IncoherentCompileTransition)
            }
            (None, Some(_)) => Ok(false),
            (Some(previous), Some(current)) if previous != current || lease_changed => {
                Err(SampleUnavailable::IncoherentCompileTransition)
            }
            (Some(_), None) if !lease_changed => {
                Err(SampleUnavailable::IncoherentCompileTransition)
            }
            (Some(_), None) => Ok(true),
            (None, None) => Ok(lease_changed),
            (Some(_), Some(_)) => Ok(false),
        }
    }

    fn validate_host(
        &self,
        host: RawHostFacts,
        owned: OwnedProcessSnapshot,
    ) -> Result<RawHostFacts, SampleUnavailable> {
        let installed_cpu = u32::from(host.logical_cpu_count)
            .checked_mul(1_000)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        if host.logical_cpu_count == 0
            || host.total_memory_bytes == 0
            || host.live_capacity_millicores == 0
            || host.live_capacity_millicores > installed_cpu
            || host.idle_millicores > host.live_capacity_millicores
            || host.available_memory_bytes > host.total_memory_bytes
            || host.outer_contained_available_memory_bytes < host.available_memory_bytes
            || host.outer_contained_available_memory_bytes > host.total_memory_bytes
            || host.pressure == MemoryPressure::Unknown
            || host.swap_out.generation.0 == 0
        {
            return Err(SampleUnavailable::IncoherentHostFacts);
        }
        let reconstructed_memory = host
            .available_memory_bytes
            .checked_add(
                owned
                    .evaluator
                    .aggregate_rss_bytes
                    .ok_or(SampleUnavailable::IncoherentOwnedSnapshot)?,
            )
            .ok_or(SampleUnavailable::SnapshotRace)?;
        let reconstructed_cpu = host
            .idle_millicores
            .checked_add(
                owned
                    .evaluator
                    .aggregate_cpu_millicores
                    .ok_or(SampleUnavailable::IncoherentOwnedSnapshot)?,
            )
            .ok_or(SampleUnavailable::SnapshotRace)?;
        if reconstructed_memory > host.total_memory_bytes
            || reconstructed_cpu > host.live_capacity_millicores
        {
            return Err(SampleUnavailable::SnapshotRace);
        }
        Ok(host)
    }

    fn swap_state(
        &mut self,
        current: Option<SwapOutCounter>,
        source_reset: bool,
    ) -> Result<SwapState, SampleUnavailable> {
        let Some(current) = current else {
            return Ok(SwapState::Unknown);
        };
        if current.generation.0 == 0 {
            return Err(SampleUnavailable::ZeroEpoch("swap_out_generation"));
        }
        let state = match self.last_swap {
            None => SwapState::Baseline,
            Some(previous) if current.generation < previous.generation => {
                return Err(SampleUnavailable::NonMonotonicSwapCounter);
            }
            Some(previous)
                if current.generation == previous.generation
                    && current.cumulative_bytes < previous.cumulative_bytes =>
            {
                return Err(SampleUnavailable::NonMonotonicSwapCounter);
            }
            Some(_) if source_reset => SwapState::Reset,
            Some(previous) if current.generation > previous.generation => SwapState::Reset,
            Some(previous) if current.cumulative_bytes > previous.cumulative_bytes => {
                SwapState::Growth
            }
            Some(_) => SwapState::Unchanged,
        };
        self.last_swap = Some(current);
        Ok(state)
    }

    fn advance_stability_epoch(&mut self) -> Result<(), SampleUnavailable> {
        self.stability_epoch =
            StabilityEpoch(next_epoch(self.stability_epoch.0, "stability_epoch")?);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapState {
    Unknown,
    Baseline,
    Reset,
    Growth,
    Unchanged,
}

fn next_epoch(value: u64, field: &'static str) -> Result<u64, SampleUnavailable> {
    if value == 0 {
        return Err(SampleUnavailable::ZeroEpoch(field));
    }
    value
        .checked_add(1)
        .ok_or(SampleUnavailable::EpochOverflow(field))
}

/// Cooperative scheduling information only. This type never starts, signals,
/// or kills a process; the coordinator owns execution and missed-deadline
/// policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SamplerWatchdog {
    cadence: Duration,
    deadline: Duration,
}

impl SamplerWatchdog {
    pub(crate) fn new(cadence: Duration, deadline: Duration) -> Result<Self, SampleUnavailable> {
        if cadence.is_zero() || deadline.is_zero() || deadline > cadence {
            return Err(SampleUnavailable::InvalidWatchdog);
        }
        Ok(Self { cadence, deadline })
    }

    pub(crate) fn next_due(self, last_started: Instant) -> Result<Instant, SampleUnavailable> {
        last_started
            .checked_add(self.cadence)
            .ok_or(SampleUnavailable::InvalidWatchdog)
    }

    pub(crate) fn deadline(self, started: Instant) -> Result<Instant, SampleUnavailable> {
        started
            .checked_add(self.deadline)
            .ok_or(SampleUnavailable::InvalidWatchdog)
    }

    pub(crate) fn is_due(
        self,
        last_started: Instant,
        now: Instant,
    ) -> Result<bool, SampleUnavailable> {
        self.next_due(last_started).map(|due| now >= due)
    }
}

pub(crate) trait HostFactProvider {
    fn collect(&mut self, deadline: Instant) -> Result<RawHostFacts, SampleUnavailable>;
}

/// macOS command provider. Every collection is one all-or-unavailable
/// transaction under the caller's deadline. The transaction brackets its
/// observations with the boot identity and samples pressure and active CPU on
/// both sides of the variable-cost commands. This prevents a reboot or an
/// optimistic edge transition from being joined into one published sample.
#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
pub(crate) struct MacOsCommandProvider;

#[cfg(target_os = "macos")]
impl MacOsCommandProvider {
    pub(crate) const SYSCTL: &'static str = "/usr/sbin/sysctl";
    pub(crate) const VM_STAT: &'static str = "/usr/bin/vm_stat";
    pub(crate) const TOP: &'static str = "/usr/bin/top";
    pub(crate) const MAX_STDOUT_BYTES: usize = 64 * 1024;

    fn collect_transaction(
        &mut self,
        deadline: Instant,
    ) -> Result<RawHostFacts, SampleUnavailable> {
        let mut transaction = MacOsCommandTransaction::new(deadline)?;

        let boot_before =
            parse_macos_boot_generation(&transaction.run(Self::SYSCTL, &["-n", "kern.boottime"])?)?;
        let before = parse_macos_host_scalars(&transaction.run(
            Self::SYSCTL,
            &[
                "-n",
                "hw.logicalcpu_max",
                "hw.activecpu",
                "hw.memsize",
                "kern.memorystatus_vm_pressure_level",
            ],
        )?)?;
        let vm = parse_macos_vm_stat(&transaction.run(Self::VM_STAT, &[])?)?;
        let cpu = parse_macos_top_cpu(&transaction.run(Self::TOP, &["-l", "1", "-n", "0"])?)?;
        let after = parse_macos_live_scalars(&transaction.run(
            Self::SYSCTL,
            &["-n", "hw.activecpu", "kern.memorystatus_vm_pressure_level"],
        )?)?;
        let boot_after =
            parse_macos_boot_generation(&transaction.run(Self::SYSCTL, &["-n", "kern.boottime"])?)?;

        if boot_before != boot_after {
            return Err(SampleUnavailable::ProviderUnavailable);
        }

        let logical_cpu_count = u16::try_from(before.logical_cpu_count)
            .ok()
            .filter(|count| *count != 0)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        let active_cpu_count = cmp::min(before.active_cpu_count, after.active_cpu_count);
        if active_cpu_count == 0 || active_cpu_count > u64::from(logical_cpu_count) {
            return Err(SampleUnavailable::IncoherentHostFacts);
        }
        let live_capacity_millicores = active_cpu_count
            .checked_mul(1_000)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        let idle_millicores = cpu.idle_millicores(live_capacity_millicores)?;

        // `vm_stat` reports free pages with speculative pages removed. Add
        // those disjoint queues back together with inactive pages, but exclude
        // active, purgeable, and compressor-backed pages from standalone
        // admission. Active pages are exposed separately for a validated outer
        // containment mode whose process-group, host-floor, and heap guards
        // make XNU's broader non-compressed reclaimable measure safe to use.
        let available_pages = vm
            .free_pages
            .checked_add(vm.inactive_pages)
            .and_then(|value| value.checked_add(vm.speculative_pages))
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        let available_memory_bytes = available_pages
            .checked_mul(vm.page_size_bytes)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        let outer_contained_available_pages = available_pages
            .checked_add(vm.active_pages)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        let outer_contained_available_memory_bytes = outer_contained_available_pages
            .checked_mul(vm.page_size_bytes)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;
        if before.total_memory_bytes == 0
            || available_memory_bytes > before.total_memory_bytes
            || outer_contained_available_memory_bytes > before.total_memory_bytes
        {
            return Err(SampleUnavailable::IncoherentHostFacts);
        }

        let pressure = worst_pressure(before.pressure, after.pressure);
        let cumulative_swap_out_bytes = vm
            .swapout_pages
            .checked_mul(vm.page_size_bytes)
            .ok_or(SampleUnavailable::IncoherentHostFacts)?;

        Ok(RawHostFacts {
            logical_cpu_count,
            total_memory_bytes: before.total_memory_bytes,
            live_capacity_millicores,
            idle_millicores,
            available_memory_bytes,
            outer_contained_available_memory_bytes,
            pressure,
            // macOS does not expose a stable scalar probability of an OOM
            // kill. Critical memorystatus pressure and any currently
            // throttled pages are the fail-closed, actionable signals here.
            oom_risk: pressure == MemoryPressure::Critical || vm.throttled_pages != 0,
            swap_out: SwapOutCounter {
                generation: super::resource_governor::SwapOutGeneration(boot_before),
                cumulative_bytes: cumulative_swap_out_bytes,
            },
        })
    }
}

#[cfg(target_os = "macos")]
impl HostFactProvider for MacOsCommandProvider {
    fn collect(&mut self, deadline: Instant) -> Result<RawHostFacts, SampleUnavailable> {
        // Preserve only the typed failure category for diagnostics. Callers
        // still receive either one complete transaction or no host facts, and
        // may never join successful fragments from separate attempts.
        self.collect_transaction(deadline)
    }
}

#[cfg(target_os = "macos")]
struct MacOsCommandTransaction {
    deadline: Instant,
    remaining_stdout_bytes: usize,
}

#[cfg(target_os = "macos")]
impl MacOsCommandTransaction {
    fn new(deadline: Instant) -> Result<Self, SampleUnavailable> {
        if Instant::now() >= deadline {
            return Err(SampleUnavailable::ProviderUnavailable);
        }
        Ok(Self {
            deadline,
            remaining_stdout_bytes: MacOsCommandProvider::MAX_STDOUT_BYTES,
        })
    }

    fn run(
        &mut self,
        program: &'static str,
        arguments: &[&str],
    ) -> Result<Vec<u8>, SampleUnavailable> {
        if self.remaining_stdout_bytes == 0 || Instant::now() >= self.deadline {
            return Err(SampleUnavailable::ProviderUnavailable);
        }
        let output = run_macos_command(
            program,
            arguments,
            self.deadline,
            self.remaining_stdout_bytes,
        )?;
        self.remaining_stdout_bytes = self
            .remaining_stdout_bytes
            .checked_sub(output.len())
            .ok_or(SampleUnavailable::ProviderUnavailable)?;
        Ok(output)
    }
}

#[cfg(target_os = "macos")]
fn run_macos_command(
    program: &'static str,
    arguments: &[&str],
    deadline: Instant,
    stdout_limit: usize,
) -> Result<Vec<u8>, SampleUnavailable> {
    if (program != MacOsCommandProvider::SYSCTL
        && program != MacOsCommandProvider::VM_STAT
        && program != MacOsCommandProvider::TOP)
        || Instant::now() >= deadline
        || stdout_limit == 0
    {
        return Err(SampleUnavailable::ProviderUnavailable);
    }

    let mut child = Command::new(program)
        .args(arguments)
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| SampleUnavailable::ProviderUnavailable)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            terminate_and_wait(&mut child);
            return Err(SampleUnavailable::ProviderUnavailable);
        }
    };
    let reader = match std::thread::Builder::new()
        .name("futuruna-host-sample".to_owned())
        .spawn(move || read_bounded_stdout(stdout, stdout_limit))
    {
        Ok(reader) => reader,
        Err(_) => {
            terminate_and_wait(&mut child);
            return Err(SampleUnavailable::ProviderUnavailable);
        }
    };

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    terminate_and_wait(&mut child);
                    let _ = reader.join();
                    return Err(SampleUnavailable::ProviderUnavailable);
                }
                std::thread::sleep(cmp::min(
                    Duration::from_millis(5),
                    deadline.saturating_duration_since(now),
                ));
            }
            Err(_) => {
                terminate_and_wait(&mut child);
                let _ = reader.join();
                return Err(SampleUnavailable::ProviderUnavailable);
            }
        }
    };
    let output = reader
        .join()
        .map_err(|_| SampleUnavailable::ProviderUnavailable)??;
    if !status.success() || output.overflowed || Instant::now() > deadline {
        return Err(SampleUnavailable::ProviderUnavailable);
    }
    Ok(output.bytes)
}

#[cfg(target_os = "macos")]
fn terminate_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "macos")]
struct BoundedStdout {
    bytes: Vec<u8>,
    overflowed: bool,
}

#[cfg(target_os = "macos")]
fn read_bounded_stdout(
    mut stdout: std::process::ChildStdout,
    limit: usize,
) -> Result<BoundedStdout, SampleUnavailable> {
    let mut bytes = Vec::with_capacity(cmp::min(limit, 4 * 1024));
    let mut overflowed = false;
    let mut buffer = [0_u8; 4 * 1024];
    loop {
        let count = stdout
            .read(&mut buffer)
            .map_err(|_| SampleUnavailable::ProviderUnavailable)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = cmp::min(remaining, count);
        bytes.extend_from_slice(&buffer[..retained]);
        if retained != count {
            overflowed = true;
        }
    }
    Ok(BoundedStdout { bytes, overflowed })
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacOsHostScalars {
    logical_cpu_count: u64,
    active_cpu_count: u64,
    total_memory_bytes: u64,
    pressure: MemoryPressure,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacOsLiveScalars {
    active_cpu_count: u64,
    pressure: MemoryPressure,
}

#[cfg(target_os = "macos")]
fn parse_macos_host_scalars(bytes: &[u8]) -> Result<MacOsHostScalars, SampleUnavailable> {
    let lines = parse_exact_ascii_lines(bytes, 4, "host_scalars")?;
    Ok(MacOsHostScalars {
        logical_cpu_count: parse_ascii_u64(lines[0], "logical_cpu_count")?,
        active_cpu_count: parse_ascii_u64(lines[1], "active_cpu_count")?,
        total_memory_bytes: parse_ascii_u64(lines[2], "total_memory_bytes")?,
        pressure: parse_macos_pressure(lines[3])?,
    })
}

#[cfg(target_os = "macos")]
fn parse_macos_live_scalars(bytes: &[u8]) -> Result<MacOsLiveScalars, SampleUnavailable> {
    let lines = parse_exact_ascii_lines(bytes, 2, "live_scalars")?;
    Ok(MacOsLiveScalars {
        active_cpu_count: parse_ascii_u64(lines[0], "active_cpu_count")?,
        pressure: parse_macos_pressure(lines[1])?,
    })
}

#[cfg(target_os = "macos")]
fn parse_macos_boot_generation(bytes: &[u8]) -> Result<u64, SampleUnavailable> {
    let line = parse_exact_ascii_lines(bytes, 1, "boot_time")?[0];
    let body = line
        .strip_prefix(b"{ sec = ")
        .ok_or(SampleUnavailable::InvalidScalar("boot_time"))?;
    let (seconds, body) = split_once_bytes(body, b", usec = ")
        .ok_or(SampleUnavailable::InvalidScalar("boot_time"))?;
    let (microseconds, suffix) =
        split_once_bytes(body, b" }").ok_or(SampleUnavailable::InvalidScalar("boot_time"))?;
    if !suffix.is_empty()
        && (suffix.first() != Some(&b' ')
            || suffix.len() == 1
            || !suffix[1..]
                .iter()
                .all(|byte| byte.is_ascii_graphic() || *byte == b' '))
    {
        return Err(SampleUnavailable::InvalidScalar("boot_time"));
    }

    let seconds = parse_ascii_u64(seconds, "boot_seconds")?;
    let microseconds = parse_ascii_u64(microseconds, "boot_microseconds")?;
    if seconds == 0 || microseconds >= 1_000_000 {
        return Err(SampleUnavailable::InvalidScalar("boot_time"));
    }
    seconds
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(microseconds))
        .filter(|value| *value != 0)
        .ok_or(SampleUnavailable::InvalidScalar("boot_time"))
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacOsVmStat {
    page_size_bytes: u64,
    active_pages: u64,
    free_pages: u64,
    inactive_pages: u64,
    speculative_pages: u64,
    throttled_pages: u64,
    swapout_pages: u64,
}

#[cfg(target_os = "macos")]
fn parse_macos_vm_stat(bytes: &[u8]) -> Result<MacOsVmStat, SampleUnavailable> {
    let lines = parse_ascii_lines(bytes, "vm_stat")?;
    let (header, records) = lines
        .split_first()
        .ok_or(SampleUnavailable::MissingHostFact("vm_stat_header"))?;
    let page_size = header
        .strip_prefix(b"Mach Virtual Memory Statistics: (page size of ")
        .and_then(|value| value.strip_suffix(b" bytes)"))
        .ok_or(SampleUnavailable::InvalidScalar("vm_stat_page_size"))?;
    let page_size_bytes = parse_ascii_u64(page_size, "vm_stat_page_size")?;
    if !(4_096..=1024 * 1024).contains(&page_size_bytes) || !page_size_bytes.is_power_of_two() {
        return Err(SampleUnavailable::InvalidScalar("vm_stat_page_size"));
    }

    let mut active_pages = None;
    let mut free_pages = None;
    let mut inactive_pages = None;
    let mut speculative_pages = None;
    let mut throttled_pages = None;
    let mut swapout_pages = None;
    for line in records {
        let (label, value) = parse_macos_vm_stat_record(line)?;
        match label {
            b"Pages active" => set_once(&mut active_pages, value, "pages_active")?,
            b"Pages free" => set_once(&mut free_pages, value, "pages_free")?,
            b"Pages inactive" => set_once(&mut inactive_pages, value, "pages_inactive")?,
            b"Pages speculative" => set_once(&mut speculative_pages, value, "pages_speculative")?,
            b"Pages throttled" => set_once(&mut throttled_pages, value, "pages_throttled")?,
            b"Swapouts" => set_once(&mut swapout_pages, value, "swapouts")?,
            _ => {}
        }
    }

    Ok(MacOsVmStat {
        page_size_bytes,
        active_pages: active_pages.ok_or(SampleUnavailable::MissingHostFact("pages_active"))?,
        free_pages: free_pages.ok_or(SampleUnavailable::MissingHostFact("pages_free"))?,
        inactive_pages: inactive_pages
            .ok_or(SampleUnavailable::MissingHostFact("pages_inactive"))?,
        speculative_pages: speculative_pages
            .ok_or(SampleUnavailable::MissingHostFact("pages_speculative"))?,
        throttled_pages: throttled_pages
            .ok_or(SampleUnavailable::MissingHostFact("pages_throttled"))?,
        swapout_pages: swapout_pages.ok_or(SampleUnavailable::MissingHostFact("swapouts"))?,
    })
}

#[cfg(target_os = "macos")]
fn parse_macos_vm_stat_record(line: &[u8]) -> Result<(&[u8], u64), SampleUnavailable> {
    let separator = line
        .iter()
        .position(|byte| *byte == b':')
        .ok_or(SampleUnavailable::InvalidScalar("vm_stat_record"))?;
    let label = &line[..separator];
    let value = &line[separator + 1..];
    if label.is_empty()
        || label.first() == Some(&b' ')
        || label.last() == Some(&b' ')
        || value.first() != Some(&b' ')
    {
        return Err(SampleUnavailable::InvalidScalar("vm_stat_record"));
    }
    let value = value
        .iter()
        .position(|byte| *byte != b' ')
        .map(|start| &value[start..])
        .ok_or(SampleUnavailable::InvalidScalar("vm_stat_record"))?;
    let value = value
        .strip_suffix(b".")
        .ok_or(SampleUnavailable::InvalidScalar("vm_stat_record"))?;
    Ok((label, parse_ascii_u64(value, "vm_stat_record")?))
}

#[cfg(target_os = "macos")]
fn set_once(
    slot: &mut Option<u64>,
    value: u64,
    field: &'static str,
) -> Result<(), SampleUnavailable> {
    if slot.replace(value).is_some() {
        return Err(SampleUnavailable::InvalidScalar(field));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct ScaledPercent {
    millionths: u64,
    quantum_millionths: u64,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacOsTopCpu {
    idle: ScaledPercent,
}

#[cfg(target_os = "macos")]
impl MacOsTopCpu {
    fn idle_millicores(self, live_capacity_millicores: u32) -> Result<u32, SampleUnavailable> {
        const ONE_HUNDRED_PERCENT: u64 = 100 * 1_000_000;
        // `top` prints a rounded decimal. Subtract one display quantum before
        // converting it to capacity so the provider never rounds idle CPU up.
        let conservative_idle = self
            .idle
            .millionths
            .saturating_sub(self.idle.quantum_millionths);
        u64::from(live_capacity_millicores)
            .checked_mul(conservative_idle)
            .map(|value| value / ONE_HUNDRED_PERCENT)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SampleUnavailable::IncoherentHostFacts)
    }
}

#[cfg(target_os = "macos")]
fn parse_macos_top_cpu(bytes: &[u8]) -> Result<MacOsTopCpu, SampleUnavailable> {
    // `top -l 1 -n 0` terminates its report with one or two blank display lines
    // across supported macOS releases. Remove at most those two command-specific
    // LFs; `parse_ascii_lines` still rejects an empty line anywhere in the body
    // and consumes the ordinary final record terminator after this step.
    let bytes = bytes.strip_suffix(b"\n\n").unwrap_or(bytes);
    let lines = parse_ascii_lines(bytes, "top")?;
    let mut parsed = None;
    for line in lines {
        if line.starts_with(b"CPU usage:") {
            if parsed.is_some() {
                return Err(SampleUnavailable::InvalidScalar("top_cpu"));
            }
            parsed = Some(parse_macos_top_cpu_line(line)?);
        }
    }
    parsed.ok_or(SampleUnavailable::MissingHostFact("top_cpu"))
}

#[cfg(target_os = "macos")]
fn parse_macos_top_cpu_line(line: &[u8]) -> Result<MacOsTopCpu, SampleUnavailable> {
    // macOS `top` emits one display-padding space after the final percentage
    // on current releases. Accept exactly that presentation byte, while still
    // rejecting arbitrary leading/trailing whitespace or malformed fields.
    let line = line.strip_suffix(b" ").unwrap_or(line);
    let line = line
        .strip_prefix(b"CPU usage: ")
        .ok_or(SampleUnavailable::InvalidScalar("top_cpu"))?;
    let (user, line) =
        split_once_bytes(line, b"% user, ").ok_or(SampleUnavailable::InvalidScalar("top_cpu"))?;
    let (system, idle) =
        split_once_bytes(line, b"% sys, ").ok_or(SampleUnavailable::InvalidScalar("top_cpu"))?;
    let idle = idle
        .strip_suffix(b"% idle")
        .ok_or(SampleUnavailable::InvalidScalar("top_cpu"))?;
    let _user = parse_scaled_percent(user)?;
    let _system = parse_scaled_percent(system)?;
    let idle = parse_scaled_percent(idle)?;
    // `top` occasionally publishes individually valid percentages from a
    // boundary-straddling sample that do not sum to exactly 100%. Idle is the
    // only value consumed here and is rounded down again in `idle_millicores`;
    // rejecting the whole transaction on the cross-field sum would turn a
    // conservative usable value into an availability failure.
    Ok(MacOsTopCpu { idle })
}

#[cfg(target_os = "macos")]
fn parse_scaled_percent(bytes: &[u8]) -> Result<ScaledPercent, SampleUnavailable> {
    const SCALE: u64 = 1_000_000;

    if bytes.is_empty() {
        return Err(SampleUnavailable::InvalidScalar("cpu_percentage"));
    }
    let (whole, fraction) = match bytes.iter().position(|byte| *byte == b'.') {
        Some(point) => {
            if bytes[point + 1..].is_empty()
                || bytes[point + 1..].len() > 6
                || bytes[point + 1..].contains(&b'.')
            {
                return Err(SampleUnavailable::InvalidScalar("cpu_percentage"));
            }
            (&bytes[..point], Some(&bytes[point + 1..]))
        }
        None => (bytes, None),
    };
    let whole = parse_ascii_u64(whole, "cpu_percentage")?;
    let (fraction, quantum_millionths) = match fraction {
        Some(fraction) => {
            let value = parse_ascii_u64(fraction, "cpu_percentage")?;
            let factor = 10_u64.pow(
                u32::try_from(6_usize.saturating_sub(fraction.len()))
                    .map_err(|_| SampleUnavailable::InvalidScalar("cpu_percentage"))?,
            );
            (
                value
                    .checked_mul(factor)
                    .ok_or(SampleUnavailable::InvalidScalar("cpu_percentage"))?,
                factor,
            )
        }
        None => (0, SCALE),
    };
    let millionths = whole
        .checked_mul(SCALE)
        .and_then(|value| value.checked_add(fraction))
        .filter(|value| *value <= 100 * SCALE)
        .ok_or(SampleUnavailable::InvalidScalar("cpu_percentage"))?;
    Ok(ScaledPercent {
        millionths,
        quantum_millionths,
    })
}

#[cfg(target_os = "macos")]
fn worst_pressure(left: MemoryPressure, right: MemoryPressure) -> MemoryPressure {
    fn rank(pressure: MemoryPressure) -> u8 {
        match pressure {
            MemoryPressure::Normal => 0,
            MemoryPressure::Warning => 1,
            MemoryPressure::Critical => 2,
            MemoryPressure::Unknown => 3,
        }
    }
    if rank(left) >= rank(right) {
        left
    } else {
        right
    }
}

#[cfg(target_os = "macos")]
fn parse_exact_ascii_lines<'a>(
    bytes: &'a [u8],
    expected: usize,
    field: &'static str,
) -> Result<Vec<&'a [u8]>, SampleUnavailable> {
    let lines = parse_ascii_lines(bytes, field)?;
    if lines.len() != expected {
        return Err(SampleUnavailable::InvalidScalar(field));
    }
    Ok(lines)
}

#[cfg(target_os = "macos")]
fn parse_ascii_lines<'a>(
    bytes: &'a [u8],
    field: &'static str,
) -> Result<Vec<&'a [u8]>, SampleUnavailable> {
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if body.is_empty()
        || !body
            .iter()
            .all(|byte| byte.is_ascii_graphic() || *byte == b' ' || *byte == b'\n')
    {
        return Err(SampleUnavailable::InvalidScalar(field));
    }
    let lines = body.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if lines.iter().any(|line| line.is_empty()) {
        return Err(SampleUnavailable::InvalidScalar(field));
    }
    Ok(lines)
}

#[cfg(target_os = "macos")]
fn split_once_bytes<'a>(bytes: &'a [u8], delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    if delimiter.is_empty() {
        return None;
    }
    let start = bytes
        .windows(delimiter.len())
        .position(|window| window == delimiter)?;
    let after = start.checked_add(delimiter.len())?;
    if bytes[after..]
        .windows(delimiter.len())
        .any(|window| window == delimiter)
    {
        return None;
    }
    Some((&bytes[..start], &bytes[after..]))
}

/// Strict parser for one ASCII unsigned decimal line produced under the fixed
/// C locale. It accepts one optional trailing LF and nothing else.
#[cfg(any(target_os = "macos", test))]
fn parse_ascii_u64(bytes: &[u8], field: &'static str) -> Result<u64, SampleUnavailable> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(SampleUnavailable::InvalidScalar(field));
    }
    bytes.iter().try_fold(0_u64, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(SampleUnavailable::InvalidScalar(field))
    })
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_pressure(bytes: &[u8]) -> Result<MemoryPressure, SampleUnavailable> {
    match parse_ascii_u64(bytes, "memory_pressure")? {
        // These are the kernel's memorystatus pressure levels, not the
        // similarly named libdispatch bit masks. Urgent is documented as a
        // warning synonym; Jetsam is treated as critical because foreground
        // process termination is approaching.
        0 => Ok(MemoryPressure::Normal),
        1 | 2 => Ok(MemoryPressure::Warning),
        3 | 4 => Ok(MemoryPressure::Critical),
        _ => Err(SampleUnavailable::InvalidScalar("memory_pressure")),
    }
}

#[cfg(test)]
mod source_canaries {
    use super::super::resource_governor::{LeaseGeneration, SwapOutGeneration};
    use super::*;

    fn owned() -> OwnedProcessSnapshot {
        OwnedProcessSnapshot {
            evaluator: EvaluatorObservation {
                lease_generation: LeaseGeneration(1),
                resident_workers: 0,
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

    fn host(swap_bytes: u64) -> RawHostFacts {
        RawHostFacts {
            logical_cpu_count: 8,
            total_memory_bytes: 16 * 1024 * 1024 * 1024,
            live_capacity_millicores: 8_000,
            idle_millicores: 6_000,
            available_memory_bytes: 12 * 1024 * 1024 * 1024,
            outer_contained_available_memory_bytes: 12 * 1024 * 1024 * 1024,
            pressure: MemoryPressure::Normal,
            oom_risk: false,
            swap_out: SwapOutCounter {
                generation: SwapOutGeneration(1),
                cumulative_bytes: swap_bytes,
            },
        }
    }

    fn reducer() -> StabilityWindowReducer {
        StabilityWindowReducer::new(
            ReducerEpochSeed {
                telemetry: NonZeroU64::new(1).unwrap(),
                stability: NonZeroU64::new(1).unwrap(),
            },
            StabilityPressurePolicy::NormalOnly,
        )
    }

    fn raw(at: u64, host: Option<RawHostFacts>) -> RawHostSample {
        RawHostSample {
            source_generation: NonZeroU64::new(1).unwrap(),
            observed_at_millis: at,
            host,
            owned: owned(),
        }
    }

    #[test]
    fn first_baseline_and_unknown_gap_never_fabricate_a_stable_window() {
        let mut reducer = reducer();
        let baseline = reducer.reduce(raw(1_000, Some(host(0)))).unwrap();
        assert_eq!(
            baseline.sample.stability.minimum_available_memory_bytes,
            None
        );
        let stable = reducer.reduce(raw(2_000, Some(host(0)))).unwrap();
        assert_eq!(stable.sample.stability.stable_since_millis, 2_000);
        let unknown = reducer.reduce(raw(3_000, None)).unwrap();
        assert!(unknown.force_zero_admission);
        assert_eq!(unknown.sample.pressure, MemoryPressure::Unknown);
        assert_eq!(
            unknown.sample.stability.minimum_available_memory_bytes,
            None
        );
        let restarted = reducer.reduce(raw(4_000, Some(host(0)))).unwrap();
        assert_eq!(restarted.sample.stability.stable_since_millis, 4_000);
    }

    #[test]
    fn swap_growth_resets_low_water_evidence() {
        let mut reducer = reducer();
        reducer.reduce(raw(1_000, Some(host(0)))).unwrap();
        let stable = reducer.reduce(raw(2_000, Some(host(0)))).unwrap();
        assert!(stable
            .sample
            .stability
            .minimum_available_memory_bytes
            .is_some());
        let growth = reducer.reduce(raw(3_000, Some(host(4_096)))).unwrap();
        assert_eq!(growth.sample.stability.minimum_available_memory_bytes, None);
    }

    #[test]
    fn macos_scalar_parsers_reject_whitespace_and_unknown_pressure() {
        assert_eq!(parse_ascii_u64(b"42\n", "value"), Ok(42));
        assert!(parse_ascii_u64(b" 42\n", "value").is_err());
        assert_eq!(parse_macos_pressure(b"0\n"), Ok(MemoryPressure::Normal));
        assert_eq!(parse_macos_pressure(b"2\n"), Ok(MemoryPressure::Warning));
        assert_eq!(parse_macos_pressure(b"4\n"), Ok(MemoryPressure::Critical));
        assert!(parse_macos_pressure(b"5\n").is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_top_parser_accepts_the_command_trailing_display_line() {
        let sample = b"Processes: 4 total \nCPU usage: 12.50% user, 7.50% sys, 80.00% idle \n\n\n";

        let parsed = parse_macos_top_cpu(sample).expect("parse current top report ending");

        assert_eq!(parsed.idle.millionths, 80_000_000);
        assert_eq!(parsed.idle.quantum_millionths, 10_000);
        let boundary_sample = b"CPU usage: 13.7% user, 38.83% sys, 48.8% idle\n";
        assert_eq!(
            parse_macos_top_cpu(boundary_sample)
                .expect("accept individually valid boundary-straddling percentages")
                .idle
                .millionths,
            48_800_000
        );
        assert!(parse_macos_top_cpu(
            b"Processes: 4 total\n\nCPU usage: 12.50% user, 7.50% sys, 80.00% idle\n\n"
        )
        .is_err());
    }
}
