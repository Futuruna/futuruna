//! Outer sampled watchdog boundary for durable Explore CLI slices.
//!
//! The exact coordinator still performs its own fail-closed admission at work
//! boundaries. This supervisor is deliberately a different layer: it owns a
//! continuously runnable guardian plus a separate worker process group,
//! samples the worker group and host while an atomic unit is running, and can
//! pause or contain work without sharing the run-state writer fence. A
//! containment kill is recovered by ordinary journal replay on the next
//! invocation.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use std::collections::BTreeSet;
#[cfg(target_os = "macos")]
use std::fs::File;
#[cfg(target_os = "macos")]
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
#[cfg(target_os = "macos")]
use std::os::raw::{c_int, c_void};
#[cfg(target_os = "macos")]
use std::os::unix::process::{CommandExt, ExitStatusExt};
#[cfg(target_os = "macos")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::thread;

pub(crate) const EXPLORE_STREAM_CHILD_MARKER: &str = "FUTURUNA_INTERNAL_EXPLORE_STREAM_CHILD_V3";
const EXPLORE_STREAM_GUARDIAN_MARKER: &str = "FUTURUNA_INTERNAL_EXPLORE_STREAM_GUARDIAN_V1";
static IS_VALIDATED_EXPLORE_STREAM_CHILD: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static VALIDATED_EXPLORE_STREAM_CONTAINMENT: OnceLock<ValidatedExactStreamContainment> =
    OnceLock::new();

const GROUP_SAMPLE_CADENCE: Duration = Duration::from_millis(100);
const HOST_SAMPLE_CADENCE: Duration = Duration::from_millis(500);
/// Host CPU decisions use a longer cumulative interval than the liveness/RSS
/// loop. Darwin reports separately floored scheduler ticks per CPU/state; one
/// second keeps a conservative rounding envelope meaningfully below the 10
/// percentage-point operational reserve.
const HOST_CPU_DECISION_WINDOW: Duration = Duration::from_secs(1);
/// The child subtracts preparation from the user-visible epoch budget before
/// starting semantic work. This outer-only grace leaves bounded time for the
/// final journal flush, result publication, and report serialization; it does
/// not authorize another semantic slice.
const OUTER_DEADLINE_GRACE: Duration = Duration::from_secs(30);
const ONE_GIB: u64 = 1024 * 1024 * 1024;
#[cfg(target_os = "macos")]
const CHILD_HEARTBEAT_TIMEOUT_MILLIS: c_int = 5_000;
#[cfg(target_os = "macos")]
const GUARDIAN_READY_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(target_os = "macos")]
const WORKER_START_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(target_os = "macos")]
const WORKER_EXIT_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
const GUARDIAN_READY_RECEIPT_LEN: usize = 16;
#[cfg(target_os = "macos")]
const GUARDIAN_READY_MAGIC: [u8; 8] = *b"RUNAWRK1";
#[cfg(target_os = "macos")]
const F_GETFD: c_int = 1;
#[cfg(target_os = "macos")]
const F_SETFD: c_int = 2;
#[cfg(target_os = "macos")]
const FD_CLOEXEC: c_int = 1;
#[cfg(target_os = "macos")]
const F_GETFL: c_int = 3;
#[cfg(target_os = "macos")]
const F_SETFL: c_int = 4;
#[cfg(target_os = "macos")]
const O_NONBLOCK: c_int = 0x0004;
#[cfg(target_os = "macos")]
const F_SETNOSIGPIPE: c_int = 73;
#[cfg(target_os = "macos")]
const SIGKILL: c_int = 9;
#[cfg(target_os = "macos")]
const SIGSTOP: c_int = 17;
#[cfg(target_os = "macos")]
const SIGCONT: c_int = 19;
#[cfg(target_os = "macos")]
const EINTR: i32 = 4;
#[cfg(target_os = "macos")]
const ESRCH: i32 = 3;
#[cfg(target_os = "macos")]
const POLLIN: i16 = 0x0001;
#[cfg(target_os = "macos")]
const POLL_ERROR_MASK: i16 = 0x0008 | 0x0010 | 0x0020;
#[cfg(target_os = "macos")]
const KERN_SUCCESS: c_int = 0;
#[cfg(target_os = "macos")]
const PROCESSOR_CPU_LOAD_INFO: c_int = 2;
#[cfg(target_os = "macos")]
const PROCESSOR_CPU_LOAD_INFO_COUNT: u32 = 4;
#[cfg(target_os = "macos")]
const MAX_PROCESSOR_COUNT: u32 = 1024;
#[cfg(target_os = "macos")]
const CPU_STATE_USER: usize = 0;
#[cfg(target_os = "macos")]
const CPU_STATE_SYSTEM: usize = 1;
#[cfg(target_os = "macos")]
const CPU_STATE_IDLE: usize = 2;
#[cfg(target_os = "macos")]
const CPU_STATE_NICE: usize = 3;
#[cfg(target_os = "macos")]
const HEARTBEAT_BYTE: u8 = 0xa5;
#[cfg(target_os = "macos")]
const WORKER_READY_ACK_BYTE: u8 = 0x5a;
#[cfg(target_os = "macos")]
const WORKER_START_BYTE: u8 = 0xc3;

#[cfg(target_os = "macos")]
#[repr(C)]
struct PollDescriptor {
    file_descriptor: c_int,
    requested_events: i16,
    returned_events: i16,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn close(file_descriptor: c_int) -> c_int;
    fn fcntl(file_descriptor: c_int, command: c_int, ...) -> c_int;
    fn getpgrp() -> c_int;
    fn getpid() -> c_int;
    fn getppid() -> c_int;
    fn kill(process_or_group: c_int, signal: c_int) -> c_int;
    fn mach_host_self() -> u32;
    fn host_processor_info(
        host: u32,
        flavor: c_int,
        processor_count: *mut u32,
        info: *mut *mut c_int,
        info_count: *mut u32,
    ) -> c_int;
    fn pipe(file_descriptors: *mut c_int) -> c_int;
    fn poll(descriptors: *mut PollDescriptor, count: u32, timeout_millis: c_int) -> c_int;
    fn read(file_descriptor: c_int, buffer: *mut c_void, count: usize) -> isize;
    static mach_task_self_: u32;
    fn vm_deallocate(target_task: u32, address: usize, size: usize) -> c_int;
    fn write(file_descriptor: c_int, buffer: *const c_void, count: usize) -> isize;
}

/// CPU may reach the authorized 80% ceiling. Memory has a separate explicit
/// operator envelope: at most 6 GiB and never more than 80% of physical RAM,
/// with a 512 MiB-or-5% runway between the synchronous Rust-heap ceiling / RSS
/// trip and that envelope. Current free queues govern live admission and
/// pausing; they do not permanently shrink the lifetime ceiling, because XNU
/// may reclaim caches and compress memory while the stream runs. Stacks,
/// direct FFI allocations, mappings and subprocesses remain protected by the
/// runway and sampled guards, not a kernel RSS quota.
const ABSOLUTE_MEMORY_CEILING_PERCENT: u64 = 80;
const OPERATOR_MEMORY_CEILING: u64 = 6 * ONE_GIB;
const HOST_AVAILABLE_MEMORY_FLOOR: u64 = ONE_GIB;
const ADMISSION_MEMORY_QUANTUM: u64 = 256 * 1024 * 1024;
const CPU_TRIP_PERCENT: u64 = 80;
const UNTRACKED_MEMORY_RESERVE_PERCENT: u64 = 5;
const MIN_UNTRACKED_MEMORY_RESERVE: u64 = 512 * 1024 * 1024;

/// XNU reports 0=normal, 1=warning, 2=urgent (also a warning state),
/// 3=critical and 4=jetsam. Warning states remain admissible while the hard
/// headroom and throttling checks below pass. Critical pressure is a
/// fail-closed boundary. Swap-out remains useful telemetry, but a machine may
/// page for unrelated work while this process group is small; the hard group
/// RSS, host headroom and pressure guards remain the containment authority.
const MEMORY_PRESSURE_CRITICAL: u64 = 3;

#[derive(Debug)]
pub(crate) enum ExactStreamSupervisionOutcome {
    Exited {
        status: ExitStatus,
        operational: ExactStreamOperationalReport,
    },
    Contained(ExactStreamContainmentReport),
}

/// Supervisor-only observations. These never enter an Explore journal,
/// evidence root, result manifest, or semantic report.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExactStreamOperationalReport {
    pub(crate) host_cpu_pause_count: u64,
    pub(crate) host_cpu_paused_duration: Duration,
    pub(crate) maximum_observed_host_cpu_hundredths_percent: u64,
    pub(crate) final_host_cpu_debt_tick_percent: u128,
    pub(crate) final_host_cpu_quantization_credit_tick_percent: u128,
}

#[derive(Debug, Clone)]
pub(crate) struct ExactStreamContainmentReport {
    pub(crate) reason: ExactStreamContainmentReason,
    pub(crate) observed_group_rss_bytes: Option<u64>,
    pub(crate) group_rss_limit_bytes: u64,
    pub(crate) observed_available_memory_bytes: Option<u64>,
    pub(crate) available_memory_floor_bytes: u64,
    pub(crate) operational: ExactStreamOperationalReport,
}

/// Process-local receipt for the outer envelope that was computed by the
/// parent and revalidated before the child installed its irrevocable Rust-heap
/// cap. The relational engine may use this receipt to replace its generic cold
/// 2 GiB estimate with the smaller *proved* heap ceiling; the remaining fields
/// stay as independently monitored outer guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedExactStreamContainment {
    pub(crate) rust_heap_limit_bytes: u64,
    pub(crate) untracked_memory_reserve_bytes: u64,
    pub(crate) group_rss_limit_bytes: u64,
    pub(crate) available_memory_floor_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) enum ExactStreamContainmentReason {
    OuterDeadline,
    GroupMemory,
    HostMemory,
    HostPressureCritical,
    HostPagesThrottled,
    TelemetryLost,
    ResidualProcesses,
}

impl fmt::Display for ExactStreamContainmentReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OuterDeadline => "outer wall deadline",
            Self::GroupMemory => "exploration process-group memory guard",
            Self::HostMemory => "host available-memory guard",
            Self::HostPressureCritical => "critical host memory pressure",
            Self::HostPagesThrottled => "host memory throttling",
            Self::TelemetryLost => "required containment telemetry became unavailable",
            Self::ResidualProcesses => "residual exploration process-group members",
        })
    }
}

#[derive(Debug)]
pub(crate) enum ExactStreamSupervisorError {
    #[cfg(not(target_os = "macos"))]
    UnsupportedPlatform,
    Telemetry(&'static str),
    HeapLimit(crate::runa_explore_heap::ExactStreamHeapLimitInstallError),
    UnsafeInitialHost {
        available_memory_bytes: u64,
        required_floor_bytes: u64,
        pressure: u64,
        cpu_hundredths_percent: u64,
        swapout_growth: bool,
    },
    Spawn(io::Error),
    Monitor(io::Error),
    ContainmentFailed {
        reason: ExactStreamContainmentReason,
        error: io::Error,
    },
}

impl fmt::Display for ExactStreamSupervisorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(not(target_os = "macos"))]
            Self::UnsupportedPlatform => formatter.write_str(
                "durable Explore process-group watchdog is not integrated on this platform",
            ),
            Self::Telemetry(detail) => write!(
                formatter,
                "required durable Explore containment telemetry is unavailable: {detail}"
            ),
            Self::HeapLimit(error) => write!(
                formatter,
                "cannot install the validated durable-child Rust heap limit: {error}"
            ),
            Self::UnsafeInitialHost {
                available_memory_bytes,
                required_floor_bytes,
                pressure,
                cpu_hundredths_percent,
                swapout_growth,
            } => write!(
                formatter,
                "host is not safe for a contained Explore child (available memory {available_memory_bytes} bytes, required floor {required_floor_bytes} bytes, pressure level {pressure}, sampled CPU {}.{:02}%, swap-out growth {swapout_growth})",
                cpu_hundredths_percent / 100,
                cpu_hundredths_percent % 100,
            ),
            Self::Spawn(error) => write!(formatter, "cannot spawn contained Explore child: {error}"),
            Self::Monitor(error) => write!(formatter, "cannot monitor contained Explore child: {error}"),
            Self::ContainmentFailed { reason, error } => write!(
                formatter,
                "failed to verify Explore process-group containment after {reason}: {error}"
            ),
        }
    }
}

impl std::error::Error for ExactStreamSupervisorError {}

pub(crate) fn is_exact_stream_child() -> bool {
    IS_VALIDATED_EXPLORE_STREAM_CHILD.load(Ordering::Acquire)
}

pub(crate) fn validated_exact_stream_containment() -> Option<ValidatedExactStreamContainment> {
    #[cfg(target_os = "macos")]
    {
        return VALIDATED_EXPLORE_STREAM_CONTAINMENT.get().copied();
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Enter the guardian or validate the worker side of the containment chain.
///
/// This runs before the CLI creates its large-stack main thread. The outer
/// supervisor launches a tiny guardian, and that guardian launches the
/// evaluator as the leader of a *different* process group. The guardian stays
/// runnable while the evaluator group is stopped for host-CPU debt, so there
/// is no stop/wake gap in which a dead parent can strand stopped work. Markers
/// are process-shape receipts, not authentication against a hostile local
/// launcher, and are removed before descendants can inherit them.
pub(crate) fn activate_exact_stream_child_liveness() -> Result<(), ExactStreamSupervisorError> {
    let guardian_marker = std::env::var_os(EXPLORE_STREAM_GUARDIAN_MARKER);
    let worker_marker = std::env::var_os(EXPLORE_STREAM_CHILD_MARKER);
    if guardian_marker.is_some() && worker_marker.is_some() {
        return Err(ExactStreamSupervisorError::Telemetry(
            "conflicting guardian and worker markers",
        ));
    }

    if guardian_marker.is_none() && worker_marker.is_none() {
        crate::runa_explore_heap::disable_exact_stream_heap_accounting_for_ordinary_process();
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (guardian_marker, worker_marker);
        return Err(ExactStreamSupervisorError::UnsupportedPlatform);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(marker) = guardian_marker {
            crate::runa_explore_heap::disable_exact_stream_heap_accounting_for_ordinary_process();
            let marker = marker
                .into_string()
                .map_err(|_| ExactStreamSupervisorError::Telemetry("guardian liveness marker"))?;
            return run_exact_stream_guardian(&marker);
        }

        let marker = worker_marker
            .expect("the mutually exclusive worker marker is present")
            .into_string()
            .map_err(|_| ExactStreamSupervisorError::Telemetry("worker containment marker"))?;
        let launch = parse_worker_launch_envelope(&marker)?;
        let claimed_guardian = launch.guardian;
        let claimed_start_fd = launch.start_fd;

        // SAFETY: these process-identity calls take no pointers and have no
        // preconditions. The worker must be the leader of a fresh group and a
        // direct child of the separately runnable guardian.
        let (actual_parent, actual_pid, actual_group) = unsafe { (getppid(), getpid(), getpgrp()) };
        if actual_parent <= 0
            || actual_parent as u32 != claimed_guardian
            || actual_pid <= 0
            || actual_group != actual_pid
        {
            return Err(ExactStreamSupervisorError::Telemetry(
                "worker containment process identity",
            ));
        }
        crate::runa_explore_heap::install_validated_exact_stream_child_heap_limit(
            launch.rust_heap_limit_bytes,
        )
        .map_err(ExactStreamSupervisorError::HeapLimit)?;
        let containment = ValidatedExactStreamContainment {
            rust_heap_limit_bytes: launch.rust_heap_limit_bytes,
            untracked_memory_reserve_bytes: launch.untracked_memory_reserve_bytes,
            group_rss_limit_bytes: launch.group_rss_limit_bytes,
            available_memory_floor_bytes: launch.available_memory_floor_bytes,
        };
        VALIDATED_EXPLORE_STREAM_CONTAINMENT
            .set(containment)
            .map_err(|_| ExactStreamSupervisorError::Telemetry("worker containment receipt"))?;
        std::env::remove_var(EXPLORE_STREAM_CHILD_MARKER);
        wait_for_guardian_worker_start(claimed_start_fd)
            .map_err(ExactStreamSupervisorError::Monitor)?;
        IS_VALIDATED_EXPLORE_STREAM_CHILD.store(true, Ordering::Release);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct ExactStreamGuardianLaunchEnvelope {
    parent: u32,
    liveness_fd: RawFd,
    ready_fd: RawFd,
    rust_heap_limit_bytes: u64,
    untracked_memory_reserve_bytes: u64,
    group_rss_limit_bytes: u64,
    available_memory_floor_bytes: u64,
}

#[cfg(target_os = "macos")]
struct ExactStreamWorkerLaunchEnvelope {
    guardian: u32,
    start_fd: RawFd,
    rust_heap_limit_bytes: u64,
    untracked_memory_reserve_bytes: u64,
    group_rss_limit_bytes: u64,
    available_memory_floor_bytes: u64,
}

#[cfg(target_os = "macos")]
fn parse_nonzero_marker_field(
    value: &str,
    detail: &'static str,
) -> Result<u64, ExactStreamSupervisorError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value != 0)
        .ok_or(ExactStreamSupervisorError::Telemetry(detail))
}

#[cfg(target_os = "macos")]
fn parse_guardian_launch_envelope(
    marker: &str,
) -> Result<ExactStreamGuardianLaunchEnvelope, ExactStreamSupervisorError> {
    const DETAIL: &str = "guardian liveness marker";
    let fields = marker.split(':').collect::<Vec<_>>();
    if fields.len() != 7 {
        return Err(ExactStreamSupervisorError::Telemetry(DETAIL));
    }
    Ok(ExactStreamGuardianLaunchEnvelope {
        parent: u32::try_from(parse_nonzero_marker_field(fields[0], DETAIL)?)
            .map_err(|_| ExactStreamSupervisorError::Telemetry(DETAIL))?,
        liveness_fd: RawFd::try_from(parse_nonzero_marker_field(fields[1], DETAIL)?)
            .map_err(|_| ExactStreamSupervisorError::Telemetry(DETAIL))?,
        ready_fd: RawFd::try_from(parse_nonzero_marker_field(fields[2], DETAIL)?)
            .map_err(|_| ExactStreamSupervisorError::Telemetry(DETAIL))?,
        rust_heap_limit_bytes: parse_nonzero_marker_field(fields[3], DETAIL)?,
        untracked_memory_reserve_bytes: parse_nonzero_marker_field(fields[4], DETAIL)?,
        group_rss_limit_bytes: parse_nonzero_marker_field(fields[5], DETAIL)?,
        available_memory_floor_bytes: parse_nonzero_marker_field(fields[6], DETAIL)?,
    })
}

#[cfg(target_os = "macos")]
fn parse_worker_launch_envelope(
    marker: &str,
) -> Result<ExactStreamWorkerLaunchEnvelope, ExactStreamSupervisorError> {
    const DETAIL: &str = "worker containment marker";
    let fields = marker.split(':').collect::<Vec<_>>();
    if fields.len() != 6 {
        return Err(ExactStreamSupervisorError::Telemetry(DETAIL));
    }
    Ok(ExactStreamWorkerLaunchEnvelope {
        guardian: u32::try_from(parse_nonzero_marker_field(fields[0], DETAIL)?)
            .map_err(|_| ExactStreamSupervisorError::Telemetry(DETAIL))?,
        start_fd: RawFd::try_from(parse_nonzero_marker_field(fields[1], DETAIL)?)
            .map_err(|_| ExactStreamSupervisorError::Telemetry(DETAIL))?,
        rust_heap_limit_bytes: parse_nonzero_marker_field(fields[2], DETAIL)?,
        untracked_memory_reserve_bytes: parse_nonzero_marker_field(fields[3], DETAIL)?,
        group_rss_limit_bytes: parse_nonzero_marker_field(fields[4], DETAIL)?,
        available_memory_floor_bytes: parse_nonzero_marker_field(fields[5], DETAIL)?,
    })
}

#[cfg(target_os = "macos")]
fn run_exact_stream_guardian(marker: &str) -> Result<(), ExactStreamSupervisorError> {
    let launch = parse_guardian_launch_envelope(marker)?;
    // SAFETY: these identity calls take no pointers. The guardian must be the
    // fresh process-group leader directly spawned by the outer supervisor.
    let (actual_parent, actual_pid, actual_group) = unsafe { (getppid(), getpid(), getpgrp()) };
    if actual_parent <= 0
        || actual_parent as u32 != launch.parent
        || actual_pid <= 0
        || actual_group != actual_pid
    {
        return Err(ExactStreamSupervisorError::Telemetry(
            "guardian liveness process identity",
        ));
    }
    set_close_on_exec(launch.liveness_fd, true).map_err(ExactStreamSupervisorError::Monitor)?;
    set_close_on_exec(launch.ready_fd, true).map_err(ExactStreamSupervisorError::Monitor)?;
    std::env::remove_var(EXPLORE_STREAM_GUARDIAN_MARKER);

    let guardian = actual_pid as u32;
    let (worker_start_reader, worker_start_writer) =
        create_guardian_handoff_pipe().map_err(ExactStreamSupervisorError::Spawn)?;
    let inherited_start_reader = worker_start_reader.as_raw_fd();
    let worker_marker = format!(
        "{guardian}:{inherited_start_reader}:{}:{}:{}:{}",
        launch.rust_heap_limit_bytes,
        launch.untracked_memory_reserve_bytes,
        launch.group_rss_limit_bytes,
        launch.available_memory_floor_bytes,
    );
    let executable = std::env::current_exe().map_err(ExactStreamSupervisorError::Spawn)?;
    let mut command = Command::new(executable);
    command
        .args(std::env::args_os().skip(1))
        .env_remove(EXPLORE_STREAM_GUARDIAN_MARKER)
        .env(EXPLORE_STREAM_CHILD_MARKER, worker_marker)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0);
    // SAFETY: only the async-signal-safe fcntl descriptor operation runs
    // between fork and exec. The worker start gate is the only inherited end.
    unsafe {
        command.pre_exec(move || set_close_on_exec(inherited_start_reader, false));
    }
    let mut worker = command.spawn().map_err(ExactStreamSupervisorError::Spawn)?;
    let worker_group = worker.id();
    drop(worker_start_reader);

    // SAFETY: the ready descriptor is uniquely owned by this guardian after
    // exec. File takes that ownership and closes it immediately after the one
    // bounded atomic receipt.
    let ready_writer = unsafe { File::from_raw_fd(launch.ready_fd) };
    if let Err(_error) = write_guardian_ready_receipt(&ready_writer, worker_group) {
        guardian_fail_closed(&mut worker, worker_group);
    }
    drop(ready_writer);

    if let Some(status) = guardian_wait_for_start_ack(&mut worker, worker_group, launch.liveness_fd)
    {
        return exit_guardian_with_worker_status(status, actual_pid);
    }
    if let Err(_error) = write_control_byte(&worker_start_writer, WORKER_START_BYTE) {
        guardian_fail_closed(&mut worker, worker_group);
    }
    drop(worker_start_writer);

    let status = guardian_wait_for_worker(&mut worker, worker_group, launch.liveness_fd);
    exit_guardian_with_worker_status(status, actual_pid)
}

#[cfg(target_os = "macos")]
fn exit_guardian_with_worker_status(
    status: ExitStatus,
    guardian_pid: c_int,
) -> Result<(), ExactStreamSupervisorError> {
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    if let Some(signal) = status.signal() {
        // SAFETY: a positive pid addresses only the guardian. Reproducing the
        // worker's terminating signal lets the outer supervisor report the
        // ordinary Unix exit shape without waking or signalling group peers.
        let _ = unsafe { kill(guardian_pid, signal) };
    }
    std::process::abort()
}

#[cfg(target_os = "macos")]
fn guardian_wait_for_start_ack(
    worker: &mut Child,
    worker_group: u32,
    liveness_fd: RawFd,
) -> Option<ExitStatus> {
    let mut heartbeat_deadline = guardian_heartbeat_deadline();
    loop {
        match guardian_poll_liveness(liveness_fd, &mut heartbeat_deadline, true) {
            Ok(true) => return None,
            Ok(false) => {}
            Err(_) => guardian_fail_closed(worker, worker_group),
        }
        match worker.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => guardian_fail_closed(worker, worker_group),
        }
    }
}

#[cfg(target_os = "macos")]
fn guardian_wait_for_worker(
    worker: &mut Child,
    worker_group: u32,
    liveness_fd: RawFd,
) -> ExitStatus {
    let mut heartbeat_deadline = guardian_heartbeat_deadline();
    loop {
        if guardian_poll_liveness(liveness_fd, &mut heartbeat_deadline, false).is_err() {
            guardian_fail_closed(worker, worker_group);
        }
        match worker.try_wait() {
            Ok(Some(status)) => match process_group_exists(worker_group) {
                Ok(false) => return status,
                Ok(true) | Err(_) => guardian_fail_closed(worker, worker_group),
            },
            Ok(None) => {}
            Err(_) => guardian_fail_closed(worker, worker_group),
        }
    }
}

#[cfg(target_os = "macos")]
fn guardian_heartbeat_deadline() -> Instant {
    let heartbeat_timeout = Duration::from_millis(
        u64::try_from(CHILD_HEARTBEAT_TIMEOUT_MILLIS)
            .expect("the guardian heartbeat timeout is positive"),
    );
    Instant::now()
        .checked_add(heartbeat_timeout)
        .expect("the bounded guardian heartbeat deadline fits the monotonic clock")
}

#[cfg(target_os = "macos")]
fn guardian_poll_liveness(
    liveness_fd: RawFd,
    heartbeat_deadline: &mut Instant,
    allow_ready_ack: bool,
) -> io::Result<bool> {
    let now = Instant::now();
    if now >= *heartbeat_deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "outer supervisor heartbeat expired",
        ));
    }
    let remaining = heartbeat_deadline
        .saturating_duration_since(now)
        .min(GROUP_SAMPLE_CADENCE);
    let timeout_millis = remaining
        .as_millis()
        .saturating_add(1)
        .min(c_int::MAX as u128) as c_int;
    let mut descriptor = PollDescriptor {
        file_descriptor: liveness_fd,
        requested_events: POLLIN,
        returned_events: 0,
    };
    // SAFETY: `descriptor` is a valid one-element pollfd-compatible array.
    let ready = unsafe { poll(&mut descriptor, 1, timeout_millis) };
    if ready < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(EINTR) {
            return Ok(false);
        }
        return Err(error);
    }
    if descriptor.returned_events & POLL_ERROR_MASK != 0 {
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "outer supervisor liveness channel closed",
        ));
    }
    if ready == 0 {
        return Ok(false);
    }
    if descriptor.returned_events & POLLIN == 0 {
        return Err(invalid_telemetry("unexpected guardian liveness event"));
    }
    let mut controls = [0_u8; 256];
    // SAFETY: `controls` is writable for its complete length and the validated
    // descriptor remains owned by this guardian.
    let received = loop {
        let received = unsafe {
            read(
                liveness_fd,
                controls.as_mut_ptr().cast::<c_void>(),
                controls.len(),
            )
        };
        if received < 0 && io::Error::last_os_error().raw_os_error() == Some(EINTR) {
            continue;
        }
        break received;
    };
    if received <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "outer supervisor liveness channel ended",
        ));
    }
    let controls = &controls[..received as usize];
    let ready_ack_count = controls
        .iter()
        .filter(|byte| **byte == WORKER_READY_ACK_BYTE)
        .count();
    if ready_ack_count > usize::from(allow_ready_ack)
        || controls.iter().any(|byte| {
            *byte != HEARTBEAT_BYTE && !(allow_ready_ack && *byte == WORKER_READY_ACK_BYTE)
        })
    {
        return Err(invalid_telemetry("invalid guardian liveness control byte"));
    }
    *heartbeat_deadline = guardian_heartbeat_deadline();
    Ok(ready_ack_count == 1)
}

#[cfg(target_os = "macos")]
fn guardian_fail_closed(worker: &mut Child, worker_group: u32) -> ! {
    loop {
        if terminate_worker_group_and_reap(worker, worker_group).is_ok() {
            std::process::exit(1);
        }
        // Do not abandon custody merely because verification took longer than
        // one bounded attempt. The outer parent independently knows the PGID
        // after readiness and can perform its own final containment.
        thread::sleep(GROUP_SAMPLE_CADENCE);
    }
}

#[cfg(target_os = "macos")]
fn wait_for_guardian_worker_start(start_fd: RawFd) -> io::Result<()> {
    set_close_on_exec(start_fd, true)?;
    // SAFETY: the descriptor was inherited exclusively by this worker and is
    // transferred once into File for bounded polling and automatic closure.
    let reader = unsafe { File::from_raw_fd(start_fd) };
    let mut descriptor = PollDescriptor {
        file_descriptor: reader.as_raw_fd(),
        requested_events: POLLIN,
        returned_events: 0,
    };
    let deadline = Instant::now()
        .checked_add(WORKER_START_TIMEOUT)
        .ok_or_else(|| invalid_telemetry("worker start deadline overflow"))?;
    // SAFETY: `descriptor` is one valid poll descriptor. EINTR retries retain
    // the original monotonic deadline rather than extending the gate.
    let ready = loop {
        let now = Instant::now();
        if now >= deadline {
            break 0;
        }
        let timeout_millis = deadline
            .saturating_duration_since(now)
            .as_millis()
            .saturating_add(1)
            .min(c_int::MAX as u128) as c_int;
        let ready = unsafe { poll(&mut descriptor, 1, timeout_millis) };
        if ready < 0 && io::Error::last_os_error().raw_os_error() == Some(EINTR) {
            continue;
        }
        break ready;
    };
    if ready <= 0 || descriptor.returned_events & POLLIN == 0 {
        return Err(if ready < 0 {
            io::Error::last_os_error()
        } else {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "guardian did not release worker start gate",
            )
        });
    }
    let mut byte = 0_u8;
    // SAFETY: the descriptor is readable and `byte` has space for one byte.
    let received = loop {
        // SAFETY: the descriptor is readable and `byte` has one writable byte.
        let received = unsafe {
            read(
                reader.as_raw_fd(),
                (&mut byte as *mut u8).cast::<c_void>(),
                1,
            )
        };
        if received < 0 && io::Error::last_os_error().raw_os_error() == Some(EINTR) {
            continue;
        }
        break received;
    };
    if received == 1 && byte == WORKER_START_BYTE {
        Ok(())
    } else {
        Err(invalid_telemetry("invalid guardian worker start gate"))
    }
}

#[cfg(target_os = "macos")]
fn create_liveness_pipe() -> io::Result<(File, File)> {
    let mut descriptors = [-1 as c_int; 2];
    // SAFETY: `descriptors` has space for both descriptors required by pipe.
    if unsafe { pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if let Err(error) = set_close_on_exec(descriptors[0], true)
        .and_then(|()| set_close_on_exec(descriptors[1], true))
        .and_then(|()| set_no_sigpipe(descriptors[1]))
        .and_then(|()| set_nonblocking(descriptors[1]))
    {
        // SAFETY: both values were returned by a successful pipe call and are
        // still owned by this function.
        unsafe {
            close(descriptors[0]);
            close(descriptors[1]);
        }
        return Err(error);
    }
    // SAFETY: ownership of each fresh descriptor is transferred exactly once.
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    // SAFETY: ownership of each fresh descriptor is transferred exactly once.
    let writer = unsafe { File::from_raw_fd(descriptors[1]) };
    Ok((reader, writer))
}

#[cfg(target_os = "macos")]
fn create_guardian_handoff_pipe() -> io::Result<(File, File)> {
    let mut descriptors = [-1 as c_int; 2];
    // SAFETY: `descriptors` has space for both descriptors required by pipe.
    if unsafe { pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if let Err(error) = set_close_on_exec(descriptors[0], true)
        .and_then(|()| set_close_on_exec(descriptors[1], true))
        .and_then(|()| set_no_sigpipe(descriptors[1]))
    {
        // SAFETY: both values came from the successful pipe call and remain
        // owned here on the setup failure path.
        unsafe {
            close(descriptors[0]);
            close(descriptors[1]);
        }
        return Err(error);
    }
    // SAFETY: ownership of each fresh descriptor is transferred exactly once.
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    // SAFETY: ownership of each fresh descriptor is transferred exactly once.
    let writer = unsafe { File::from_raw_fd(descriptors[1]) };
    Ok((reader, writer))
}

#[cfg(target_os = "macos")]
fn write_guardian_ready_receipt(writer: &File, worker_group: u32) -> io::Result<()> {
    let mut receipt = [0_u8; GUARDIAN_READY_RECEIPT_LEN];
    receipt[..GUARDIAN_READY_MAGIC.len()].copy_from_slice(&GUARDIAN_READY_MAGIC);
    receipt[8..12].copy_from_slice(&worker_group.to_be_bytes());
    receipt[12..16].copy_from_slice(&worker_group.to_be_bytes());
    write_atomic_pipe_record(writer, &receipt)
}

#[cfg(target_os = "macos")]
fn write_atomic_pipe_record(writer: &File, record: &[u8]) -> io::Result<()> {
    loop {
        // SAFETY: `record` is readable for its complete bounded length and the
        // pipe writer is owned by the caller. A record this small is below
        // PIPE_BUF, so a successful blocking write is atomic.
        let written = unsafe {
            write(
                writer.as_raw_fd(),
                record.as_ptr().cast::<c_void>(),
                record.len(),
            )
        };
        if written == record.len() as isize {
            return Ok(());
        }
        if written < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(EINTR) {
                continue;
            }
            return Err(error);
        }
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "bounded guardian pipe record was not written atomically",
        ));
    }
}

#[cfg(target_os = "macos")]
fn write_control_byte(writer: &File, byte: u8) -> io::Result<()> {
    write_atomic_pipe_record(writer, std::slice::from_ref(&byte))
}

#[cfg(target_os = "macos")]
fn terminate_worker_group_and_reap(worker: &mut Child, worker_group: u32) -> io::Result<()> {
    let _ = signal_process_group(worker_group, SIGSTOP);
    let mut last_error = signal_process_group(worker_group, SIGKILL).err();
    let mut worker_reaped = false;
    for _ in 0..100 {
        if !worker_reaped {
            match worker.try_wait() {
                Ok(Some(_)) => worker_reaped = true,
                Ok(None) => {
                    if let Err(error) = worker.kill() {
                        if error.raw_os_error() != Some(ESRCH) {
                            last_error = Some(error);
                        }
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }
        match process_group_exists(worker_group) {
            Ok(false) if worker_reaped => return Ok(()),
            Ok(false) => {}
            Ok(true) => {
                if let Err(error) = signal_process_group(worker_group, SIGKILL) {
                    last_error = Some(error);
                }
            }
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "worker process group {worker_group} or its leader remained after bounded containment"
            ),
        )
    }))
}

#[cfg(target_os = "macos")]
fn await_guardian_ready_receipt(
    guardian: &mut Child,
    liveness_writer: &File,
    ready_reader: &File,
) -> io::Result<u32> {
    let deadline = Instant::now()
        .checked_add(GUARDIAN_READY_TIMEOUT)
        .ok_or_else(|| invalid_telemetry("guardian ready deadline overflow"))?;
    let mut receipt = [0_u8; GUARDIAN_READY_RECEIPT_LEN];
    let mut received_len = 0_usize;
    loop {
        write_control_byte(liveness_writer, HEARTBEAT_BYTE)?;
        let now = Instant::now();
        if now >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "guardian did not identify its worker process group",
            ));
        }
        let timeout = deadline
            .saturating_duration_since(now)
            .min(GROUP_SAMPLE_CADENCE);
        let timeout_millis = timeout
            .as_millis()
            .saturating_add(1)
            .min(c_int::MAX as u128) as c_int;
        let mut descriptor = PollDescriptor {
            file_descriptor: ready_reader.as_raw_fd(),
            requested_events: POLLIN,
            returned_events: 0,
        };
        // SAFETY: `descriptor` is one valid poll descriptor.
        let ready = unsafe { poll(&mut descriptor, 1, timeout_millis) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(EINTR) {
                continue;
            }
            return Err(error);
        }
        if ready != 0 && descriptor.returned_events & POLLIN != 0 {
            // SAFETY: the remaining suffix is writable and the descriptor is
            // the parent-owned read end of the bounded ready pipe.
            let count = unsafe {
                read(
                    ready_reader.as_raw_fd(),
                    receipt[received_len..].as_mut_ptr().cast::<c_void>(),
                    receipt.len() - received_len,
                )
            };
            if count <= 0 {
                return Err(if count < 0 {
                    io::Error::last_os_error()
                } else {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "empty guardian ready receipt")
                });
            }
            received_len = received_len
                .checked_add(count as usize)
                .ok_or_else(|| invalid_telemetry("guardian ready receipt overflow"))?;
            if received_len == receipt.len() {
                if receipt[..8] != GUARDIAN_READY_MAGIC {
                    return Err(invalid_telemetry("invalid guardian ready receipt magic"));
                }
                let worker_pid = u32::from_be_bytes(
                    receipt[8..12]
                        .try_into()
                        .expect("the worker PID receipt field has four bytes"),
                );
                let worker_group = u32::from_be_bytes(
                    receipt[12..16]
                        .try_into()
                        .expect("the worker PGID receipt field has four bytes"),
                );
                if worker_pid == 0 || worker_pid != worker_group {
                    return Err(invalid_telemetry(
                        "guardian worker is not its process-group leader",
                    ));
                }
                return Ok(worker_group);
            }
        }
        if descriptor.returned_events & POLL_ERROR_MASK != 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "guardian ready channel ended before a complete receipt",
            ));
        }
        if let Some(status) = guardian.try_wait()? {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("guardian exited before worker readiness ({status})"),
            ));
        }
    }
}

#[cfg(target_os = "macos")]
fn terminate_guardian_before_ready(guardian: &mut Child) -> io::Result<()> {
    // Before the parent acknowledges a valid worker receipt, the worker is
    // blocked on its guardian-owned start gate. Give the guardian longer than
    // its heartbeat timeout to observe EOF, kill/reap that blocked group, and
    // exit under its own custody protocol.
    for _ in 0..600 {
        if guardian.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    guardian.kill()?;
    for _ in 0..100 {
        if guardian.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "guardian remained after bounded pre-readiness termination",
    ))
}

#[cfg(target_os = "macos")]
fn set_no_sigpipe(file_descriptor: RawFd) -> io::Result<()> {
    // SAFETY: Darwin F_SETNOSIGPIPE accepts an integer boolean and prevents a
    // liveness-channel EPIPE from terminating the supervising parent.
    if unsafe { fcntl(file_descriptor, F_SETNOSIGPIPE, 1 as c_int) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_nonblocking(file_descriptor: RawFd) -> io::Result<()> {
    // SAFETY: fcntl F_GETFL/F_SETFL operate on the owned pipe descriptor and
    // accept the integer status flags returned by the kernel.
    let flags = unsafe { fcntl(file_descriptor, F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { fcntl(file_descriptor, F_SETFL, flags | O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_close_on_exec(file_descriptor: RawFd, enabled: bool) -> io::Result<()> {
    // SAFETY: fcntl F_GETFD reads descriptor flags and has no pointer argument.
    let flags = unsafe { fcntl(file_descriptor, F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let next = if enabled {
        flags | FD_CLOEXEC
    } else {
        flags & !FD_CLOEXEC
    };
    // SAFETY: F_SETFD accepts the integer flag set returned above.
    if unsafe { fcntl(file_descriptor, F_SETFD, next) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn supervise_current_executable(
    _arguments: &[OsString],
    _max_runtime: Option<Duration>,
) -> Result<ExactStreamSupervisionOutcome, ExactStreamSupervisorError> {
    Err(ExactStreamSupervisorError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
pub(crate) fn supervise_current_executable(
    arguments: &[OsString],
    max_runtime: Option<Duration>,
) -> Result<ExactStreamSupervisionOutcome, ExactStreamSupervisorError> {
    let preflight_host = collect_host_snapshot()?;
    let preflight_cpu = collect_host_cpu_ticks().map_err(ExactStreamSupervisorError::Monitor)?;
    // Admission is based on a measured interval, never a lifetime-average CPU
    // percentage. No exploration child exists during this bounded window.
    thread::sleep(HOST_CPU_DECISION_WINDOW);
    let initial = collect_host_snapshot()?;
    let initial_cpu = collect_host_cpu_ticks().map_err(ExactStreamSupervisorError::Monitor)?;
    if preflight_host.total_memory_bytes != initial.total_memory_bytes
        || preflight_host.active_cpu_count != initial.active_cpu_count
        || preflight_host.page_size_bytes != initial.page_size_bytes
    {
        return Err(ExactStreamSupervisorError::Telemetry(
            "host capacity changed during initial admission",
        ));
    }
    let initial_cpu_hundredths = host_cpu_upper_hundredths(&preflight_cpu, &initial_cpu)
        .map_err(ExactStreamSupervisorError::Monitor)?
        .ok_or(ExactStreamSupervisorError::Telemetry(
            "initial host CPU interval was too short",
        ))?;
    // Host safety and the lifetime operator ceiling are deliberately
    // separate. Keep one GiB live for the host, and require one accounted work
    // quantum beyond it before launch. The same live floor is sampled
    // throughout execution; unlike the old launch-headroom formula it does
    // not freeze a transient free-page count into the child's lifetime cap.
    let available_floor = HOST_AVAILABLE_MEMORY_FLOOR;
    let admission_floor = available_floor
        .checked_add(ADMISSION_MEMORY_QUANTUM)
        .ok_or(ExactStreamSupervisorError::Telemetry(
            "initial memory admission floor overflow",
        ))?;
    let preflight_swap_growth = initial.swapout_pages > preflight_host.swapout_pages;
    if preflight_host.pressure >= MEMORY_PRESSURE_CRITICAL
        || initial.pressure >= MEMORY_PRESSURE_CRITICAL
        || preflight_host.throttled_pages != 0
        || initial.throttled_pages != 0
        || initial.available_memory_bytes <= admission_floor
        || initial_cpu_hundredths > CPU_TRIP_PERCENT * 100
    {
        return Err(ExactStreamSupervisorError::UnsafeInitialHost {
            available_memory_bytes: initial.available_memory_bytes,
            required_floor_bytes: admission_floor,
            pressure: initial.pressure,
            cpu_hundredths_percent: initial_cpu_hundredths,
            swapout_growth: preflight_swap_growth,
        });
    }

    let untracked_memory_reserve =
        percent_ceil(initial.total_memory_bytes, UNTRACKED_MEMORY_RESERVE_PERCENT)
            .max(MIN_UNTRACKED_MEMORY_RESERVE);
    let physical_memory_ceiling = initial
        .total_memory_bytes
        .checked_sub(available_floor)
        .ok_or(ExactStreamSupervisorError::Telemetry(
            "physical memory is smaller than the host floor",
        ))?;
    let operator_memory_ceiling = OPERATOR_MEMORY_CEILING
        .min(percent_floor(
            initial.total_memory_bytes,
            ABSOLUTE_MEMORY_CEILING_PERCENT,
        ))
        .min(physical_memory_ceiling);
    // Rust requests are synchronously refused at the soft trip. The sampled
    // whole-group RSS guard uses the same boundary, leaving the independently
    // computed reserve as termination/allocator-retention runway below the
    // operator ceiling.
    let group_rss_limit = operator_memory_ceiling.checked_sub(untracked_memory_reserve);
    if group_rss_limit.is_none_or(|limit| limit < ADMISSION_MEMORY_QUANTUM) {
        return Err(ExactStreamSupervisorError::UnsafeInitialHost {
            available_memory_bytes: initial.available_memory_bytes,
            required_floor_bytes: admission_floor,
            pressure: initial.pressure,
            cpu_hundredths_percent: initial_cpu_hundredths,
            swapout_growth: preflight_swap_growth,
        });
    }
    let group_rss_limit = group_rss_limit.expect("positive RSS runway was checked");
    let rust_heap_limit = group_rss_limit;

    let started = Instant::now();
    let outer_deadline = max_runtime.and_then(|runtime| {
        runtime
            .checked_add(OUTER_DEADLINE_GRACE)
            .and_then(|bounded| started.checked_add(bounded))
    });
    if max_runtime.is_some() && outer_deadline.is_none() {
        return Err(ExactStreamSupervisorError::Telemetry(
            "outer wall deadline overflow",
        ));
    }

    let executable = std::env::current_exe().map_err(ExactStreamSupervisorError::Spawn)?;
    let (guardian_liveness_reader, parent_liveness_writer) =
        create_liveness_pipe().map_err(ExactStreamSupervisorError::Spawn)?;
    let (parent_ready_reader, guardian_ready_writer) =
        create_guardian_handoff_pipe().map_err(ExactStreamSupervisorError::Spawn)?;
    let inherited_liveness_reader = guardian_liveness_reader.as_raw_fd();
    let inherited_ready_writer = guardian_ready_writer.as_raw_fd();
    let marker = format!(
        "{}:{inherited_liveness_reader}:{inherited_ready_writer}:{rust_heap_limit}:{untracked_memory_reserve}:{group_rss_limit}:{available_floor}",
        std::process::id()
    );
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env_remove(EXPLORE_STREAM_CHILD_MARKER)
        .env(EXPLORE_STREAM_GUARDIAN_MARKER, marker)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0);
    // SAFETY: the closure performs only async-signal-safe descriptor fcntl
    // calls. These are the only two pipe ends allowed to survive guardian exec.
    unsafe {
        command.pre_exec(move || {
            set_close_on_exec(inherited_liveness_reader, false)
                .and_then(|()| set_close_on_exec(inherited_ready_writer, false))
        });
    }
    let mut guardian = command.spawn().map_err(ExactStreamSupervisorError::Spawn)?;
    drop(guardian_liveness_reader);
    drop(guardian_ready_writer);
    let worker_process_group = match await_guardian_ready_receipt(
        &mut guardian,
        &parent_liveness_writer,
        &parent_ready_reader,
    ) {
        Ok(worker_process_group) => worker_process_group,
        Err(error) => {
            drop(parent_liveness_writer);
            return match terminate_guardian_before_ready(&mut guardian) {
                Ok(()) => Err(ExactStreamSupervisorError::Monitor(error)),
                Err(containment_error) => {
                    Err(ExactStreamSupervisorError::ContainmentFailed {
                        reason: ExactStreamContainmentReason::TelemetryLost,
                        error: io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "worker readiness failed ({error}); pre-readiness guardian containment also failed ({containment_error})"
                            ),
                        ),
                    })
                }
            };
        }
    };
    drop(parent_ready_reader);
    // This instant is after a successful spawn, so using it as the zero-CPU
    // baseline can only shorten the first accounting interval. That makes the
    // first rate estimate conservative.
    let child_spawned_at = Instant::now();
    let mut child = ChildGroupGuard::new(
        guardian,
        worker_process_group,
        parent_liveness_writer,
        initial_cpu_hundredths,
    );
    if let Err(error) = child.release_worker() {
        return Err(contain_after_monitor_error(&mut child, error));
    }
    if let Err(error) = child.heartbeat() {
        if let Ok(Some(status)) = child.try_wait() {
            return finish_exited_child(
                &mut child,
                status,
                None,
                group_rss_limit,
                Some(initial.available_memory_bytes),
                available_floor,
            );
        }
        return Err(contain_after_monitor_error(&mut child, error));
    }
    let process_group = child.process_group();
    let mut last_group = None;
    let mut last_cpu_group = None;
    let mut last_host = initial;
    let mut last_host_cpu = initial_cpu;
    let mut next_host_sample = started;
    let mut next_host_cpu_sample = started
        .checked_add(HOST_CPU_DECISION_WINDOW)
        .unwrap_or(started);

    loop {
        let child_status = match child.wait_for_worker_exit_settlement() {
            Ok(status) => status,
            Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
        };
        if let Some(status) = child_status {
            return finish_exited_child(
                &mut child,
                status,
                last_group,
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
        }

        let now = Instant::now();
        if outer_deadline.is_some_and(|deadline| now >= deadline) {
            return contain(
                &mut child,
                ExactStreamContainmentReason::OuterDeadline,
                last_group,
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
        }

        let group = match collect_group_snapshot(process_group) {
            Ok(Some(group)) => group,
            Ok(None) => {
                // The worker-group leader can disappear just before the
                // separately runnable guardian observes and relays its exit.
                // Keep the liveness contract active for one bounded settling
                // window instead of turning that ordinary handoff race into
                // telemetry loss. A guardian that does not finish still
                // reaches the same fail-closed containment path below.
                let child_status = match child.wait_for_worker_exit_settlement() {
                    Ok(status) => status,
                    Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
                };
                if let Some(status) = child_status {
                    return finish_exited_child(
                        &mut child,
                        status,
                        last_group,
                        group_rss_limit,
                        Some(last_host.available_memory_bytes),
                        available_floor,
                    );
                }
                return contain(
                    &mut child,
                    ExactStreamContainmentReason::TelemetryLost,
                    last_group,
                    group_rss_limit,
                    Some(last_host.available_memory_bytes),
                    available_floor,
                );
            }
            Err(_) => {
                return contain(
                    &mut child,
                    ExactStreamContainmentReason::TelemetryLost,
                    last_group,
                    group_rss_limit,
                    Some(last_host.available_memory_bytes),
                    available_floor,
                );
            }
        };

        // The child may have exited while `ps` was running. Never turn that
        // ordinary exit into a liveness-pipe error or reason about a stale
        // process identity.
        let child_status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
        };
        if let Some(status) = child_status {
            return finish_exited_child(
                &mut child,
                status,
                Some(group),
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
        }

        // Darwin can keep the exited worker-group leader visible to `ps` as a
        // zero-RSS zombie for the short interval before the independently
        // runnable guardian reaps it and relays its status. CPU fields on that
        // row are no longer a trustworthy monotone sample (and may appear to
        // regress to zero), so give the guardian the same bounded settlement
        // window used when the group has disappeared entirely. A genuinely
        // live zero-RSS row still fails closed after the window.
        if group.rss_bytes == 0 {
            let child_status = match child.wait_for_worker_exit_settlement() {
                Ok(status) => status,
                Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
            };
            if let Some(status) = child_status {
                return finish_exited_child(
                    &mut child,
                    status,
                    Some(group),
                    group_rss_limit,
                    Some(last_host.available_memory_bytes),
                    available_floor,
                );
            }
            return contain(
                &mut child,
                ExactStreamContainmentReason::TelemetryLost,
                Some(group),
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
        }

        if group.rss_bytes > group_rss_limit {
            return contain(
                &mut child,
                ExactStreamContainmentReason::GroupMemory,
                Some(group),
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
        }
        let cpu_exceeded = match sampled_cpu_exceeds_limit(
            last_cpu_group,
            group,
            child_spawned_at,
            initial.active_cpu_count,
            CPU_TRIP_PERCENT,
        ) {
            Ok(Some(exceeded)) => {
                last_cpu_group = Some(group);
                exceeded
            }
            Ok(None) => false,
            Err(_) => {
                return contain(
                    &mut child,
                    ExactStreamContainmentReason::TelemetryLost,
                    Some(group),
                    group_rss_limit,
                    Some(last_host.available_memory_bytes),
                    available_floor,
                );
            }
        };
        last_group = Some(group);
        if cpu_exceeded {
            if let Err(error) = child.pause_for_group_cpu(now) {
                return Err(contain_after_monitor_error(&mut child, error));
            }
        }

        if now >= next_host_cpu_sample {
            let host_cpu = match collect_host_cpu_ticks() {
                Ok(sample) => sample,
                Err(_) => {
                    return contain(
                        &mut child,
                        ExactStreamContainmentReason::TelemetryLost,
                        Some(group),
                        group_rss_limit,
                        Some(last_host.available_memory_bytes),
                        available_floor,
                    );
                }
            };
            match host_cpu_interval(&last_host_cpu, &host_cpu) {
                Ok(Some(interval)) => {
                    last_host_cpu = host_cpu;
                    next_host_cpu_sample = now.checked_add(HOST_CPU_DECISION_WINDOW).unwrap_or(now);
                    if let Err(error) = child.apply_host_cpu_interval(interval, now) {
                        return Err(contain_after_monitor_error(&mut child, error));
                    }
                }
                Ok(None) => {}
                Err(_) => {
                    return contain(
                        &mut child,
                        ExactStreamContainmentReason::TelemetryLost,
                        Some(group),
                        group_rss_limit,
                        Some(last_host.available_memory_bytes),
                        available_floor,
                    );
                }
            }
        }

        if now >= next_host_sample {
            let host = match collect_host_snapshot() {
                Ok(host) => host,
                Err(_) => {
                    return contain(
                        &mut child,
                        ExactStreamContainmentReason::TelemetryLost,
                        Some(group),
                        group_rss_limit,
                        None,
                        available_floor,
                    );
                }
            };
            let reason = if host.pressure >= MEMORY_PRESSURE_CRITICAL {
                Some(ExactStreamContainmentReason::HostPressureCritical)
            } else if host.throttled_pages != 0 {
                Some(ExactStreamContainmentReason::HostPagesThrottled)
            } else if host.available_memory_bytes <= available_floor {
                Some(ExactStreamContainmentReason::HostMemory)
            } else if host.total_memory_bytes != initial.total_memory_bytes
                || host.active_cpu_count != initial.active_cpu_count
                || host.page_size_bytes != initial.page_size_bytes
            {
                Some(ExactStreamContainmentReason::TelemetryLost)
            } else {
                None
            };
            last_host = host;
            if let Some(reason) = reason {
                return contain(
                    &mut child,
                    reason,
                    Some(group),
                    group_rss_limit,
                    Some(host.available_memory_bytes),
                    available_floor,
                );
            }
            next_host_sample = now.checked_add(HOST_SAMPLE_CADENCE).unwrap_or(now);
        }

        if let Err(error) = child.heartbeat() {
            if let Ok(Some(status)) = child.try_wait() {
                return finish_exited_child(
                    &mut child,
                    status,
                    Some(group),
                    group_rss_limit,
                    Some(last_host.available_memory_bytes),
                    available_floor,
                );
            }
            return Err(contain_after_monitor_error(&mut child, error));
        }
        std::thread::sleep(GROUP_SAMPLE_CADENCE);
    }
}

#[cfg(target_os = "macos")]
fn finish_exited_child(
    child: &mut ChildGroupGuard,
    status: ExitStatus,
    last_group: Option<GroupSnapshot>,
    group_rss_limit_bytes: u64,
    observed_available_memory_bytes: Option<u64>,
    available_memory_floor_bytes: u64,
) -> Result<ExactStreamSupervisionOutcome, ExactStreamSupervisorError> {
    let operational = child.operational_report(Instant::now());
    let residual = match child.finish_exited_group() {
        Ok(residual) => residual,
        Err(error) => return Err(contain_after_monitor_error(child, error)),
    };
    if residual {
        return Ok(ExactStreamSupervisionOutcome::Contained(
            ExactStreamContainmentReport {
                reason: ExactStreamContainmentReason::ResidualProcesses,
                observed_group_rss_bytes: last_group.map(|snapshot| snapshot.rss_bytes),
                group_rss_limit_bytes,
                observed_available_memory_bytes,
                available_memory_floor_bytes,
                operational,
            },
        ));
    }
    Ok(ExactStreamSupervisionOutcome::Exited {
        status,
        operational,
    })
}

#[cfg(target_os = "macos")]
fn contain_after_monitor_error(
    child: &mut ChildGroupGuard,
    monitor_error: io::Error,
) -> ExactStreamSupervisorError {
    match child.contain_verified() {
        Ok(()) => ExactStreamSupervisorError::Monitor(monitor_error),
        Err(containment_error) => ExactStreamSupervisorError::ContainmentFailed {
            reason: ExactStreamContainmentReason::TelemetryLost,
            error: io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "monitor failed ({monitor_error}); containment verification also failed ({containment_error})"
                ),
            ),
        },
    }
}

#[cfg(target_os = "macos")]
fn contain(
    child: &mut ChildGroupGuard,
    reason: ExactStreamContainmentReason,
    group: Option<GroupSnapshot>,
    group_rss_limit_bytes: u64,
    observed_available_memory_bytes: Option<u64>,
    available_memory_floor_bytes: u64,
) -> Result<ExactStreamSupervisionOutcome, ExactStreamSupervisorError> {
    // A sample can fail in the interval after the worker has completed and
    // the guardian has relayed its status. Once that status is waitable it is
    // stronger evidence than a terminal telemetry race: finalize the known
    // exit and still verify that no residual worker-group members remain.
    // A guardian that is not yet waitable remains live and therefore takes
    // the ordinary fail-closed containment path below.
    if matches!(&reason, ExactStreamContainmentReason::TelemetryLost) {
        let child_status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => return Err(contain_after_monitor_error(child, error)),
        };
        if let Some(status) = child_status {
            return finish_exited_child(
                child,
                status,
                group,
                group_rss_limit_bytes,
                observed_available_memory_bytes,
                available_memory_floor_bytes,
            );
        }
    }
    let operational = child.operational_report(Instant::now());
    child
        .contain_verified()
        .map_err(|error| ExactStreamSupervisorError::ContainmentFailed {
            reason: reason.clone(),
            error,
        })?;
    Ok(ExactStreamSupervisionOutcome::Contained(
        ExactStreamContainmentReport {
            reason,
            observed_group_rss_bytes: group.map(|snapshot| snapshot.rss_bytes),
            group_rss_limit_bytes,
            observed_available_memory_bytes,
            available_memory_floor_bytes,
            operational,
        },
    ))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostCpuBudgetAction {
    None,
    Pause,
    Resume,
}

/// Conservative tokenless budget/debt accounting for the total host. A safe
/// interval may repay debt but never creates credit for a later burst. This
/// gives the worker at most 80% of each cumulative controlled epoch (subject
/// to one sampled decision interval), while unrelated host load is charged to
/// the same budget.
#[cfg(target_os = "macos")]
struct HostCpuBudgetDebt {
    debt_tick_percent: u128,
    /// A paused decision window may repay more debt than necessary because the
    /// one-second telemetry interval is indivisible. Preserve only that
    /// crossing overshoot, capped at one 80%-window. Ordinary running
    /// headroom never adds credit.
    repayment_quantization_credit_tick_percent: u128,
    paused: bool,
    pause_count: u64,
    paused_duration: Duration,
    pause_started: Option<Instant>,
    maximum_observed_hundredths_percent: u64,
}

#[cfg(target_os = "macos")]
impl HostCpuBudgetDebt {
    fn new(initial_hundredths_percent: u64) -> Self {
        Self {
            debt_tick_percent: 0,
            repayment_quantization_credit_tick_percent: 0,
            paused: false,
            pause_count: 0,
            paused_duration: Duration::ZERO,
            pause_started: None,
            maximum_observed_hundredths_percent: initial_hundredths_percent,
        }
    }

    fn observe(&mut self, interval: HostCpuInterval) -> io::Result<HostCpuBudgetAction> {
        self.maximum_observed_hundredths_percent = self
            .maximum_observed_hundredths_percent
            .max(interval.upper_hundredths_percent);
        let charged = u128::from(interval.upper_busy_ticks)
            .checked_mul(100)
            .ok_or_else(|| invalid_telemetry("host CPU debt charge overflow"))?;
        let budget = u128::from(interval.lower_total_ticks)
            .checked_mul(u128::from(CPU_TRIP_PERCENT))
            .ok_or_else(|| invalid_telemetry("host CPU debt budget overflow"))?;
        if charged > budget {
            let excess = charged - budget;
            let credit_spent = self.repayment_quantization_credit_tick_percent.min(excess);
            self.repayment_quantization_credit_tick_percent -= credit_spent;
            self.debt_tick_percent = self
                .debt_tick_percent
                .checked_add(excess - credit_spent)
                .ok_or_else(|| invalid_telemetry("host CPU debt overflow"))?;
        } else if self.debt_tick_percent != 0 {
            let repayment = budget - charged;
            if repayment < self.debt_tick_percent {
                self.debt_tick_percent -= repayment;
            } else {
                let crossing_overshoot = repayment - self.debt_tick_percent;
                self.debt_tick_percent = 0;
                if self.paused {
                    // This credit exists solely because a stopped worker could
                    // only be reconsidered at a sampled boundary. It is never
                    // minted from ordinary below-budget running intervals.
                    self.repayment_quantization_credit_tick_percent =
                        crossing_overshoot.min(budget);
                }
            }
        }

        Ok(match (self.paused, self.debt_tick_percent == 0) {
            (false, false) => HostCpuBudgetAction::Pause,
            (true, true) => HostCpuBudgetAction::Resume,
            _ => HostCpuBudgetAction::None,
        })
    }

    fn force_pause(&mut self) -> HostCpuBudgetAction {
        // One debt unit is enough to retain the pause until the next
        // authoritative Mach host interval either repays or increases it.
        self.repayment_quantization_credit_tick_percent = 0;
        self.debt_tick_percent = self.debt_tick_percent.max(1);
        if self.paused {
            HostCpuBudgetAction::None
        } else {
            HostCpuBudgetAction::Pause
        }
    }

    fn mark_paused(&mut self, now: Instant) -> io::Result<()> {
        if self.paused || self.pause_started.is_some() {
            return Err(invalid_telemetry("duplicate host CPU pause"));
        }
        self.pause_count = self
            .pause_count
            .checked_add(1)
            .ok_or_else(|| invalid_telemetry("host CPU pause count overflow"))?;
        self.paused = true;
        self.pause_started = Some(now);
        Ok(())
    }

    fn mark_resumed(&mut self, now: Instant) -> io::Result<()> {
        if !self.paused {
            return Err(invalid_telemetry("host CPU resume without pause"));
        }
        let started = self
            .pause_started
            .take()
            .ok_or_else(|| invalid_telemetry("host CPU pause clock missing"))?;
        self.paused_duration = self
            .paused_duration
            .checked_add(now.saturating_duration_since(started))
            .ok_or_else(|| invalid_telemetry("host CPU paused duration overflow"))?;
        self.paused = false;
        Ok(())
    }

    fn operational_report(&self, now: Instant) -> ExactStreamOperationalReport {
        let current_pause = self
            .pause_started
            .map(|started| now.saturating_duration_since(started))
            .unwrap_or(Duration::ZERO);
        ExactStreamOperationalReport {
            host_cpu_pause_count: self.pause_count,
            host_cpu_paused_duration: self
                .paused_duration
                .checked_add(current_pause)
                .unwrap_or(Duration::MAX),
            maximum_observed_host_cpu_hundredths_percent: self.maximum_observed_hundredths_percent,
            final_host_cpu_debt_tick_percent: self.debt_tick_percent,
            final_host_cpu_quantization_credit_tick_percent: self
                .repayment_quantization_credit_tick_percent,
        }
    }
}

#[cfg(target_os = "macos")]
struct ChildGroupGuard {
    guardian: Child,
    guardian_pid: u32,
    worker_process_group: u32,
    liveness_writer: Option<File>,
    guardian_reaped: bool,
    armed: bool,
    host_cpu_budget: HostCpuBudgetDebt,
}

#[cfg(target_os = "macos")]
impl ChildGroupGuard {
    fn new(
        guardian: Child,
        worker_process_group: u32,
        liveness_writer: File,
        initial_host_cpu_hundredths_percent: u64,
    ) -> Self {
        let guardian_pid = guardian.id();
        Self {
            guardian,
            guardian_pid,
            worker_process_group,
            liveness_writer: Some(liveness_writer),
            guardian_reaped: false,
            armed: true,
            host_cpu_budget: HostCpuBudgetDebt::new(initial_host_cpu_hundredths_percent),
        }
    }

    fn process_group(&self) -> u32 {
        self.worker_process_group
    }

    fn heartbeat(&self) -> io::Result<()> {
        let writer = self.liveness_writer.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "child liveness writer is closed")
        })?;
        write_control_byte(writer, HEARTBEAT_BYTE)
    }

    fn release_worker(&self) -> io::Result<()> {
        let writer = self.liveness_writer.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "guardian liveness writer is closed",
            )
        })?;
        write_control_byte(writer, WORKER_READY_ACK_BYTE)
    }

    fn apply_host_cpu_interval(
        &mut self,
        interval: HostCpuInterval,
        now: Instant,
    ) -> io::Result<()> {
        let action = self.host_cpu_budget.observe(interval)?;
        self.apply_cpu_budget_action(action, now)
    }

    fn pause_for_group_cpu(&mut self, now: Instant) -> io::Result<()> {
        let action = self.host_cpu_budget.force_pause();
        self.apply_cpu_budget_action(action, now)
    }

    fn apply_cpu_budget_action(
        &mut self,
        action: HostCpuBudgetAction,
        now: Instant,
    ) -> io::Result<()> {
        match action {
            HostCpuBudgetAction::None => Ok(()),
            HostCpuBudgetAction::Pause => {
                require_signal(
                    signal_process_group(self.worker_process_group, SIGSTOP)?,
                    "worker process group disappeared while pausing for host CPU",
                )?;
                // The guardian is outside this group and remains continuously
                // runnable; there is no second wake syscall or orphan window.
                self.host_cpu_budget.mark_paused(now)
            }
            HostCpuBudgetAction::Resume => {
                require_signal(
                    signal_process_group(self.worker_process_group, SIGCONT)?,
                    "worker process group disappeared while resuming host CPU work",
                )?;
                self.host_cpu_budget.mark_resumed(now)
            }
        }
    }

    fn operational_report(&self, now: Instant) -> ExactStreamOperationalReport {
        self.host_cpu_budget.operational_report(now)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.guardian.try_wait()?;
        if status.is_some() {
            self.guardian_reaped = true;
        }
        Ok(status)
    }

    /// Allow the guardian to relay a worker exit after the sampled process
    /// group has already disappeared. The outer heartbeat remains live during
    /// this bounded handoff, so a stuck or disconnected guardian still fails
    /// closed rather than becoming an unmonitored child.
    fn wait_for_worker_exit_settlement(&mut self) -> io::Result<Option<ExitStatus>> {
        let deadline = Instant::now()
            .checked_add(WORKER_EXIT_SETTLE_TIMEOUT)
            .ok_or_else(|| invalid_telemetry("worker exit settlement deadline overflow"))?;
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            if let Err(error) = self.heartbeat() {
                // A guardian may close the pipe immediately before becoming
                // waitable. Reap once more before treating that close as a
                // broken liveness contract.
                if let Some(status) = self.try_wait()? {
                    return Ok(Some(status));
                }
                return Err(error);
            }
            thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(GROUP_SAMPLE_CADENCE),
            );
        }
    }

    /// Finish a normal guardian exit. Returns true when residual worker-group
    /// members existed and had to be contained before status could be returned.
    fn finish_exited_group(&mut self) -> io::Result<bool> {
        self.liveness_writer.take();
        let residual = process_group_exists(self.worker_process_group)?;
        if residual {
            self.terminate_and_verify()?;
        } else {
            self.armed = false;
        }
        Ok(residual)
    }

    fn contain_verified(&mut self) -> io::Result<()> {
        self.liveness_writer.take();
        self.terminate_and_verify()
    }

    fn terminate_and_verify(&mut self) -> io::Result<()> {
        let _ = signal_process_group(self.worker_process_group, SIGSTOP);
        let mut last_error = signal_process_group(self.worker_process_group, SIGKILL).err();

        // Closing the heartbeat asks the still-runnable guardian to perform
        // the same kill/verify/reap transaction. Retain it until both custody
        // targets are gone; never kill the guardian in the stop/wake gap that
        // this topology exists to remove.
        for _ in 0..600 {
            if !self.guardian_reaped {
                match self.guardian.try_wait() {
                    Ok(Some(_)) => self.guardian_reaped = true,
                    Ok(None) => {}
                    Err(error) => last_error = Some(error),
                }
            }
            match process_group_exists(self.worker_process_group) {
                Ok(false) if self.guardian_reaped => {
                    self.armed = false;
                    return Ok(());
                }
                Ok(false) => {}
                Ok(true) => {
                    if let Err(error) = signal_process_group(self.worker_process_group, SIGKILL) {
                        last_error = Some(error);
                    }
                }
                Err(error) => last_error = Some(error),
            }
            thread::sleep(Duration::from_millis(10));
        }

        // The worker group has been sent SIGKILL throughout the bounded grace.
        // If a faulty guardian still refuses to exit, terminate that guardian
        // directly, then continue verifying both independent identities.
        if !self.guardian_reaped {
            if let Err(error) = self.guardian.kill() {
                if error.raw_os_error() != Some(ESRCH) {
                    last_error = Some(error);
                }
            }
        }
        for _ in 0..100 {
            if !self.guardian_reaped {
                match self.guardian.try_wait() {
                    Ok(Some(_)) => self.guardian_reaped = true,
                    Ok(None) => {}
                    Err(error) => last_error = Some(error),
                }
            }
            let group_exists = match process_group_exists(self.worker_process_group) {
                Ok(exists) => exists,
                Err(error) => {
                    last_error = Some(error);
                    true
                }
            };
            if group_exists {
                if let Err(error) = signal_process_group(self.worker_process_group, SIGKILL) {
                    last_error = Some(error);
                }
            } else if self.guardian_reaped {
                self.armed = false;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }

        Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "worker process group {} or guardian {} remained after bounded containment",
                    self.worker_process_group, self.guardian_pid
                ),
            )
        }))
    }
}

#[cfg(target_os = "macos")]
impl Drop for ChildGroupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.liveness_writer.take();
        let _ = self.terminate_and_verify();
    }
}

#[cfg(target_os = "macos")]
fn signal_process_group(process_group: u32, signal: c_int) -> io::Result<bool> {
    let process_group = c_int::try_from(process_group)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "process group exceeds c_int"))?;
    loop {
        // SAFETY: a negative nonzero pid addresses one process group; callers
        // provide only valid macOS signal numbers (or zero for existence).
        if unsafe { kill(-process_group, signal) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(EINTR) => continue,
            Some(ESRCH) => return Ok(false),
            _ => return Err(error),
        }
    }
}

#[cfg(target_os = "macos")]
fn require_signal(delivered: bool, detail: &'static str) -> io::Result<()> {
    if delivered {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, detail))
    }
}

#[cfg(target_os = "macos")]
fn process_group_exists(process_group: u32) -> io::Result<bool> {
    signal_process_group(process_group, 0)
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct GroupSnapshot {
    rss_bytes: u64,
    leader_cpu_centiseconds: u64,
    process_id: u32,
    process_started: ProcessStartIdentity,
    sample_started: Instant,
    sample_finished: Instant,
}

#[cfg(target_os = "macos")]
fn collect_group_snapshot(process_group: u32) -> io::Result<Option<GroupSnapshot>> {
    // The stream child currently launches only bounded host-telemetry helpers;
    // they remain in this process group so containment can never orphan one.
    // RSS covers every member visible at the sample. The fast CPU sample uses
    // only the stable group leader because a sum over currently live helpers
    // is not monotone when one exits. The separate Mach host counters are the
    // authoritative CPU cap and charge every helper, including work between
    // two `ps` samples. Arbitrary evaluator subprocess workers remain
    // forbidden until event-backed tree accounting is integrated.
    let sample_started = Instant::now();
    let group_selector = process_group.to_string();
    let output = Command::new("/bin/ps")
        .args([
            "-g",
            group_selector.as_str(),
            "-o",
            "pid=,pgid=,rss=,lstart=,time=",
        ])
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .output()?;
    let sample_finished = Instant::now();
    if output.stdout.len() > 8 * 1024 * 1024
        || output.stderr.len() > 1024 * 1024
        || !output.stderr.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "process-group sample failed",
        ));
    }
    // Darwin ps exits 1 with empty output when the selected process group no
    // longer exists. Preserve that as the ordinary disappearance signal used
    // by the guardian-settlement path; every diagnosed command failure still
    // fails closed.
    if !output.status.success() && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "process-group sample failed",
        ));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 ps output"))?;
    let mut seen_processes = BTreeSet::new();
    let mut group_process_count = 0_u64;
    let mut group_rss_kib = 0_u64;
    let mut group_leader = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row = parse_ps_process_row(line)?;
        if !seen_processes.insert(row.process_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate process identity in ps output",
            ));
        }
        if row.process_group != process_group {
            continue;
        }
        group_process_count = group_process_count
            .checked_add(1)
            .ok_or_else(|| invalid_telemetry("process-group member count overflow"))?;
        group_rss_kib = group_rss_kib
            .checked_add(row.rss_kib)
            .ok_or_else(|| invalid_telemetry("process-group RSS overflow"))?;
        if row.process_id == process_group && group_leader.replace(row).is_some() {
            return Err(invalid_telemetry("duplicate process-group leader"));
        }
    }
    if group_process_count == 0 {
        return Ok(None);
    }
    let process =
        group_leader.ok_or_else(|| invalid_telemetry("live process group has no leader"))?;
    let rss_bytes = group_rss_kib
        .checked_mul(1024)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "RSS byte overflow"))?;
    Ok(Some(GroupSnapshot {
        rss_bytes,
        leader_cpu_centiseconds: process.cumulative_cpu_centiseconds,
        process_id: process.process_id,
        process_started: process.process_started,
        sample_started,
        sample_finished,
    }))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct PsProcessRow {
    process_id: u32,
    process_group: u32,
    rss_kib: u64,
    process_started: ProcessStartIdentity,
    cumulative_cpu_centiseconds: u64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessStartIdentity {
    weekday: u8,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    year: u16,
}

#[cfg(target_os = "macos")]
fn parse_ps_process_row(line: &str) -> io::Result<PsProcessRow> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 9 {
        return Err(invalid_telemetry("unexpected ps fields"));
    }
    let process_id = parse_nonzero_u32(fields[0], "invalid ps PID")?;
    let process_group = parse_nonzero_u32(fields[1], "invalid ps process group")?;
    let rss_kib = fields[2]
        .parse::<u64>()
        .map_err(|_| invalid_telemetry("invalid ps RSS"))?;
    let process_started = parse_process_start(&fields[3..8])?;
    let cumulative_cpu_centiseconds = parse_cumulative_cpu_centiseconds(fields[8])?;
    Ok(PsProcessRow {
        process_id,
        process_group,
        rss_kib,
        process_started,
        cumulative_cpu_centiseconds,
    })
}

#[cfg(target_os = "macos")]
fn parse_nonzero_u32(value: &str, detail: &'static str) -> io::Result<u32> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| invalid_telemetry(detail))
}

#[cfg(target_os = "macos")]
fn parse_process_start(fields: &[&str]) -> io::Result<ProcessStartIdentity> {
    if fields.len() != 5 {
        return Err(invalid_telemetry("invalid ps start identity"));
    }
    let weekday = match fields[0] {
        "Sun" => 0,
        "Mon" => 1,
        "Tue" => 2,
        "Wed" => 3,
        "Thu" => 4,
        "Fri" => 5,
        "Sat" => 6,
        _ => return Err(invalid_telemetry("invalid ps start weekday")),
    };
    let month = match fields[1] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return Err(invalid_telemetry("invalid ps start month")),
    };
    let day = parse_bounded_decimal(fields[2], 1, 31, 1, 2, "invalid ps start day")?;
    let mut clock = fields[3].split(':');
    let hour = parse_bounded_decimal(
        clock.next().unwrap_or_default(),
        0,
        23,
        2,
        2,
        "invalid ps start hour",
    )?;
    let minute = parse_bounded_decimal(
        clock.next().unwrap_or_default(),
        0,
        59,
        2,
        2,
        "invalid ps start minute",
    )?;
    let second = parse_bounded_decimal(
        clock.next().unwrap_or_default(),
        0,
        59,
        2,
        2,
        "invalid ps start second",
    )?;
    if clock.next().is_some() {
        return Err(invalid_telemetry("invalid ps start clock"));
    }
    if fields[4].len() != 4 || !fields[4].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_telemetry("invalid ps start year"));
    }
    let year = fields[4]
        .parse::<u16>()
        .ok()
        .filter(|year| *year >= 1970)
        .ok_or_else(|| invalid_telemetry("invalid ps start year"))?;
    Ok(ProcessStartIdentity {
        weekday,
        month,
        day,
        hour,
        minute,
        second,
        year,
    })
}

#[cfg(target_os = "macos")]
fn parse_bounded_decimal(
    value: &str,
    minimum: u8,
    maximum: u8,
    minimum_digits: usize,
    maximum_digits: usize,
    detail: &'static str,
) -> io::Result<u8> {
    if !(minimum_digits..=maximum_digits).contains(&value.len())
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_telemetry(detail));
    }
    value
        .parse::<u8>()
        .ok()
        .filter(|parsed| (minimum..=maximum).contains(parsed))
        .ok_or_else(|| invalid_telemetry(detail))
}

#[cfg(target_os = "macos")]
fn parse_cumulative_cpu_centiseconds(value: &str) -> io::Result<u64> {
    let (minutes, seconds_and_fraction) = value
        .split_once(':')
        .ok_or_else(|| invalid_telemetry("invalid cumulative CPU time"))?;
    if minutes.is_empty()
        || !minutes.bytes().all(|byte| byte.is_ascii_digit())
        || seconds_and_fraction.len() != 5
        || seconds_and_fraction.as_bytes()[2] != b'.'
    {
        return Err(invalid_telemetry("invalid cumulative CPU time"));
    }
    let seconds = parse_bounded_decimal(
        &seconds_and_fraction[..2],
        0,
        59,
        2,
        2,
        "invalid cumulative CPU seconds",
    )?;
    let centiseconds = parse_bounded_decimal(
        &seconds_and_fraction[3..],
        0,
        99,
        2,
        2,
        "invalid cumulative CPU fraction",
    )?;
    minutes
        .parse::<u64>()
        .ok()
        .and_then(|minutes| minutes.checked_mul(60 * 100))
        .and_then(|total| total.checked_add(u64::from(seconds) * 100))
        .and_then(|total| total.checked_add(u64::from(centiseconds)))
        .ok_or_else(|| invalid_telemetry("cumulative CPU time overflow"))
}

#[cfg(target_os = "macos")]
fn sampled_cpu_exceeds_limit(
    previous: Option<GroupSnapshot>,
    current: GroupSnapshot,
    child_spawned_at: Instant,
    active_cpu_count: u16,
    percent: u64,
) -> io::Result<Option<bool>> {
    if active_cpu_count == 0 || percent == 0 || percent > 100 {
        return Err(invalid_telemetry("invalid CPU containment capacity"));
    }
    let (previous_cpu, previous_finished) = match previous {
        Some(previous) => {
            if previous.process_id != current.process_id
                || previous.process_started != current.process_started
            {
                return Err(invalid_telemetry("supervised process identity changed"));
            }
            (previous.leader_cpu_centiseconds, previous.sample_finished)
        }
        None => (0, child_spawned_at),
    };
    let elapsed = current
        .sample_started
        .checked_duration_since(previous_finished)
        .filter(|elapsed| !elapsed.is_zero())
        .ok_or_else(|| invalid_telemetry("invalid CPU sample interval"))?;
    if previous.is_none() && elapsed < GROUP_SAMPLE_CADENCE {
        return Ok(None);
    }
    let displayed_delta = current
        .leader_cpu_centiseconds
        .checked_sub(previous_cpu)
        .ok_or_else(|| invalid_telemetry("cumulative CPU time regressed"))?;
    // Darwin ps reports hundredths. Charge one additional tick so rounding
    // cannot make the sampled rate look safer than it was.
    let upper_delta = displayed_delta
        .checked_add(1)
        .ok_or_else(|| invalid_telemetry("CPU sample delta overflow"))?;
    let left = u128::from(upper_delta)
        .checked_mul(10_000_000)
        .and_then(|nanoseconds| nanoseconds.checked_mul(100))
        .ok_or_else(|| invalid_telemetry("CPU sample comparison overflow"))?;
    let right = elapsed
        .as_nanos()
        .checked_mul(u128::from(active_cpu_count))
        .and_then(|capacity| capacity.checked_mul(u128::from(percent)))
        .ok_or_else(|| invalid_telemetry("CPU capacity comparison overflow"))?;
    Ok(Some(left > right))
}

#[cfg(target_os = "macos")]
fn invalid_telemetry(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct HostCpuTicks {
    /// One stable processor-array row per logical CPU. Keeping the rows
    /// separate is required for correct wrapping subtraction.
    states: Box<[[u32; PROCESSOR_CPU_LOAD_INFO_COUNT as usize]]>,
    sampled_at: Instant,
}

#[cfg(target_os = "macos")]
fn collect_host_cpu_ticks() -> io::Result<HostCpuTicks> {
    static HOST_PORT: OnceLock<u32> = OnceLock::new();
    let host = *HOST_PORT.get_or_init(|| {
        // SAFETY: mach_host_self has no arguments or preconditions. Retaining
        // one send right for this short-lived supervisor avoids leaking a new
        // right on every CPU decision sample.
        unsafe { mach_host_self() }
    });
    if host == 0 {
        return Err(invalid_telemetry("Mach host port is unavailable"));
    }
    let mut processor_count = 0_u32;
    let mut raw_info = std::ptr::null_mut::<c_int>();
    let mut info_count = 0_u32;
    // SAFETY: the out parameters are valid writable scalars. On success Mach
    // returns an out-of-line array owned by this task; every path below
    // releases that mapping with vm_deallocate.
    let result = unsafe {
        host_processor_info(
            host,
            PROCESSOR_CPU_LOAD_INFO,
            &mut processor_count,
            &mut raw_info,
            &mut info_count,
        )
    };
    let mapped_bytes = usize::try_from(info_count)
        .ok()
        .and_then(|count| count.checked_mul(std::mem::size_of::<c_int>()));

    let decoded = (|| {
        if result != KERN_SUCCESS {
            return Err(invalid_telemetry(
                "Mach processor CPU counters are unavailable",
            ));
        }
        if processor_count == 0 || processor_count > MAX_PROCESSOR_COUNT {
            return Err(invalid_telemetry("invalid Mach processor count"));
        }
        let expected_count = processor_count
            .checked_mul(PROCESSOR_CPU_LOAD_INFO_COUNT)
            .ok_or_else(|| invalid_telemetry("Mach processor-info count overflow"))?;
        if info_count != expected_count
            || raw_info.is_null()
            || (raw_info as usize) % std::mem::align_of::<c_int>() != 0
        {
            return Err(invalid_telemetry("invalid Mach processor CPU array"));
        }
        let count = usize::try_from(info_count)
            .map_err(|_| invalid_telemetry("Mach processor-info count exceeds usize"))?;
        // SAFETY: a successful host_processor_info call returned a non-null,
        // correctly aligned array of exactly `info_count` natural_t/c_int
        // values, validated above. It remains mapped until after this copy.
        let raw = unsafe { std::slice::from_raw_parts(raw_info.cast_const(), count) };
        let mut states = Vec::with_capacity(processor_count as usize);
        for row in raw.chunks_exact(PROCESSOR_CPU_LOAD_INFO_COUNT as usize) {
            states.push([
                row[CPU_STATE_USER] as u32,
                row[CPU_STATE_SYSTEM] as u32,
                row[CPU_STATE_IDLE] as u32,
                row[CPU_STATE_NICE] as u32,
            ]);
        }
        if states.len() != processor_count as usize {
            return Err(invalid_telemetry("truncated Mach processor CPU array"));
        }
        Ok(HostCpuTicks {
            states: states.into_boxed_slice(),
            sampled_at: Instant::now(),
        })
    })();

    let release = match (raw_info.is_null(), mapped_bytes) {
        (true, _) => Ok(()),
        (false, Some(bytes)) if bytes != 0 => {
            // SAFETY: `raw_info` is the out-of-line mapping returned by Mach,
            // and the byte length is the checked natural_t count from that
            // same reply. mach_task_self_ names the current task map.
            let release = unsafe { vm_deallocate(mach_task_self_, raw_info as usize, bytes) };
            if release == KERN_SUCCESS {
                Ok(())
            } else {
                Err(invalid_telemetry("cannot release Mach processor CPU array"))
            }
        }
        (false, _) => Err(invalid_telemetry(
            "cannot size Mach processor CPU array for release",
        )),
    };
    release?;
    decoded
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct HostCpuInterval {
    upper_busy_ticks: u64,
    lower_total_ticks: u64,
    upper_hundredths_percent: u64,
}

#[cfg(target_os = "macos")]
fn host_cpu_interval(
    previous: &HostCpuTicks,
    current: &HostCpuTicks,
) -> io::Result<Option<HostCpuInterval>> {
    let elapsed = current
        .sampled_at
        .checked_duration_since(previous.sampled_at)
        .filter(|elapsed| !elapsed.is_zero())
        .ok_or_else(|| invalid_telemetry("invalid host CPU sample interval"))?;
    if elapsed < HOST_CPU_DECISION_WINDOW {
        return Ok(None);
    }
    if previous.states.len() != current.states.len() || current.states.is_empty() {
        return Err(invalid_telemetry("Mach processor topology changed"));
    }
    // Mach exposes separately floored u32 scheduler ticks. Subtract before
    // aggregating so one rollover per state remains correct. The decision
    // interval is bounded far below the time needed for two u32 rollovers.
    let mut busy = 0_u64;
    let mut total = 0_u64;
    for (prior, next) in previous.states.iter().zip(current.states.iter()) {
        let user = u64::from(next[CPU_STATE_USER].wrapping_sub(prior[CPU_STATE_USER]));
        let system = u64::from(next[CPU_STATE_SYSTEM].wrapping_sub(prior[CPU_STATE_SYSTEM]));
        let idle = u64::from(next[CPU_STATE_IDLE].wrapping_sub(prior[CPU_STATE_IDLE]));
        let nice = u64::from(next[CPU_STATE_NICE].wrapping_sub(prior[CPU_STATE_NICE]));
        let row_busy = user
            .checked_add(system)
            .and_then(|value| value.checked_add(nice))
            .ok_or_else(|| invalid_telemetry("host CPU busy-tick overflow"))?;
        busy = busy
            .checked_add(row_busy)
            .ok_or_else(|| invalid_telemetry("host CPU busy-tick overflow"))?;
        total = total
            .checked_add(row_busy)
            .and_then(|value| value.checked_add(idle))
            .ok_or_else(|| invalid_telemetry("host CPU total-tick overflow"))?;
    }
    // Each of USER, SYSTEM, NICE and IDLE is independently floored. Bound the
    // unknown fraction conservatively: every busy state may be almost one
    // tick higher, while every observed state may contribute almost one tick
    // less to the true denominator. The 1-second window keeps this envelope
    // useful; a zero lower denominator is telemetry loss.
    let processor_count = u64::try_from(current.states.len())
        .map_err(|_| invalid_telemetry("host processor count exceeds u64"))?;
    let busy_uncertainty = processor_count
        .checked_mul(3)
        .ok_or_else(|| invalid_telemetry("host CPU uncertainty overflow"))?;
    let total_uncertainty = processor_count
        .checked_mul(4)
        .ok_or_else(|| invalid_telemetry("host CPU uncertainty overflow"))?;
    let upper_busy = busy
        .checked_add(busy_uncertainty)
        .ok_or_else(|| invalid_telemetry("host CPU upper tick overflow"))?;
    let lower_total = total
        .checked_sub(total_uncertainty)
        .filter(|total| *total != 0)
        .ok_or_else(|| invalid_telemetry("host CPU tick interval is too short"))?;
    let numerator = u128::from(upper_busy)
        .checked_mul(10_000)
        .ok_or_else(|| invalid_telemetry("host CPU percentage overflow"))?;
    let denominator = u128::from(lower_total);
    let hundredths = numerator
        .checked_add(denominator - 1)
        .map(|rounded| rounded / denominator)
        .map(|rounded| rounded.min(10_000))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| invalid_telemetry("host CPU percentage overflow"))?;
    Ok(Some(HostCpuInterval {
        upper_busy_ticks: upper_busy,
        lower_total_ticks: lower_total,
        upper_hundredths_percent: hundredths,
    }))
}

#[cfg(target_os = "macos")]
fn host_cpu_upper_hundredths(
    previous: &HostCpuTicks,
    current: &HostCpuTicks,
) -> io::Result<Option<u64>> {
    Ok(host_cpu_interval(previous, current)?.map(|interval| interval.upper_hundredths_percent))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct HostSnapshot {
    active_cpu_count: u16,
    total_memory_bytes: u64,
    available_memory_bytes: u64,
    pressure: u64,
    page_size_bytes: u64,
    throttled_pages: u64,
    swapout_pages: u64,
}

#[cfg(target_os = "macos")]
fn collect_host_snapshot() -> Result<HostSnapshot, ExactStreamSupervisorError> {
    let scalars = command_output(
        "/usr/sbin/sysctl",
        &[
            "-n",
            "hw.activecpu",
            "hw.memsize",
            "kern.memorystatus_vm_pressure_level",
        ],
        1024,
    )?;
    let mut lines = scalars.lines();
    let active_cpu_count = lines
        .next()
        .and_then(|line| line.parse::<u16>().ok())
        .filter(|count| *count != 0)
        .ok_or(ExactStreamSupervisorError::Telemetry("active CPU count"))?;
    let total_memory_bytes = lines
        .next()
        .and_then(|line| line.parse::<u64>().ok())
        .filter(|bytes| *bytes != 0)
        .ok_or(ExactStreamSupervisorError::Telemetry("physical memory"))?;
    let pressure = lines
        .next()
        .and_then(|line| line.parse::<u64>().ok())
        .filter(|level| *level <= 4)
        .ok_or(ExactStreamSupervisorError::Telemetry("memory pressure"))?;
    if lines.next().is_some() {
        return Err(ExactStreamSupervisorError::Telemetry(
            "unexpected sysctl output",
        ));
    }

    let vm = command_output("/usr/bin/vm_stat", &[], 64 * 1024)?;
    let mut vm_lines = vm.lines();
    let header = vm_lines
        .next()
        .ok_or(ExactStreamSupervisorError::Telemetry("vm_stat header"))?;
    let page_size_bytes = header
        .strip_prefix("Mach Virtual Memory Statistics: (page size of ")
        .and_then(|value| value.strip_suffix(" bytes)"))
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| (4096..=1024 * 1024).contains(value) && value.is_power_of_two())
        .ok_or(ExactStreamSupervisorError::Telemetry("vm_stat page size"))?;
    let mut free_pages = None;
    let mut inactive_pages = None;
    let mut speculative_pages = None;
    let mut throttled_pages = None;
    let mut swapout_pages = None;
    for line in vm_lines {
        let Some((label, value)) = line.split_once(':') else {
            return Err(ExactStreamSupervisorError::Telemetry("vm_stat record"));
        };
        let value = value
            .trim()
            .strip_suffix('.')
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(ExactStreamSupervisorError::Telemetry("vm_stat value"))?;
        match label {
            "Pages free" => set_once(&mut free_pages, value)?,
            "Pages inactive" => set_once(&mut inactive_pages, value)?,
            "Pages speculative" => set_once(&mut speculative_pages, value)?,
            "Pages throttled" => set_once(&mut throttled_pages, value)?,
            "Swapouts" => set_once(&mut swapout_pages, value)?,
            _ => {}
        }
    }
    // Active pages are resident/in use and cannot safely be promised to a new
    // evaluator heap. Count only reclaimable/free non-compressed classes;
    // pressure and throttling remain independent hard gates. Swap-out is
    // retained as advisory telemetry; hard containment comes from the process
    // group RSS, available-memory floor and pressure/throttling signals.
    let available_pages = free_pages
        .and_then(|free| inactive_pages.and_then(|inactive| free.checked_add(inactive)))
        .and_then(|value| speculative_pages.and_then(|speculative| value.checked_add(speculative)))
        .ok_or(ExactStreamSupervisorError::Telemetry(
            "available-memory page accounting",
        ))?;
    let available_memory_bytes = available_pages
        .checked_mul(page_size_bytes)
        .filter(|available| *available <= total_memory_bytes)
        .ok_or(ExactStreamSupervisorError::Telemetry(
            "available-memory byte accounting",
        ))?;
    Ok(HostSnapshot {
        active_cpu_count,
        total_memory_bytes,
        available_memory_bytes,
        pressure,
        page_size_bytes,
        throttled_pages: throttled_pages
            .ok_or(ExactStreamSupervisorError::Telemetry("throttled pages"))?,
        swapout_pages: swapout_pages.ok_or(ExactStreamSupervisorError::Telemetry("swapouts"))?,
    })
}

#[cfg(target_os = "macos")]
fn command_output(
    program: &'static str,
    arguments: &[&str],
    byte_limit: usize,
) -> Result<String, ExactStreamSupervisorError> {
    let output = Command::new(program)
        .args(arguments)
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(ExactStreamSupervisorError::Monitor)?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > byte_limit {
        return Err(ExactStreamSupervisorError::Telemetry(
            "host command transaction",
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| ExactStreamSupervisorError::Telemetry("non-UTF-8 host command output"))
}

#[cfg(target_os = "macos")]
fn set_once(slot: &mut Option<u64>, value: u64) -> Result<(), ExactStreamSupervisorError> {
    if slot.replace(value).is_some() {
        return Err(ExactStreamSupervisorError::Telemetry(
            "duplicate vm_stat field",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn percent_floor(value: u64, percent: u64) -> u64 {
    (value / 100)
        .saturating_mul(percent)
        .saturating_add((value % 100).saturating_mul(percent) / 100)
}

#[cfg(target_os = "macos")]
fn percent_ceil(value: u64, percent: u64) -> u64 {
    let remainder_product = (value % 100).saturating_mul(percent);
    (value / 100)
        .saturating_mul(percent)
        .saturating_add(remainder_product / 100)
        .saturating_add(u64::from(remainder_product % 100 != 0))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn cpu_interval(upper_busy_ticks: u64, lower_total_ticks: u64) -> HostCpuInterval {
        HostCpuInterval {
            upper_busy_ticks,
            lower_total_ticks,
            upper_hundredths_percent: if lower_total_ticks == 0 {
                10_000
            } else {
                upper_busy_ticks
                    .saturating_mul(10_000)
                    .div_ceil(lower_total_ticks)
                    .min(10_000)
            },
        }
    }

    #[test]
    fn running_headroom_never_mints_cpu_credit() {
        let mut controller = HostCpuBudgetDebt::new(0);
        assert_eq!(
            controller.observe(cpu_interval(10, 100)).unwrap(),
            HostCpuBudgetAction::None
        );
        assert_eq!(controller.debt_tick_percent, 0);
        assert_eq!(controller.repayment_quantization_credit_tick_percent, 0);
    }

    #[test]
    fn paused_window_overshoot_is_bounded_credit_not_fifty_percent_throttling() {
        let mut controller = HostCpuBudgetDebt::new(0);
        let now = Instant::now();
        assert_eq!(
            controller.observe(cpu_interval(100, 100)).unwrap(),
            HostCpuBudgetAction::Pause
        );
        assert_eq!(controller.debt_tick_percent, 2_000);
        controller.mark_paused(now).unwrap();

        assert_eq!(
            controller.observe(cpu_interval(0, 100)).unwrap(),
            HostCpuBudgetAction::Resume
        );
        assert_eq!(controller.debt_tick_percent, 0);
        assert_eq!(controller.repayment_quantization_credit_tick_percent, 6_000);
        controller.mark_resumed(now).unwrap();

        for expected_credit in [4_000, 2_000, 0] {
            assert_eq!(
                controller.observe(cpu_interval(100, 100)).unwrap(),
                HostCpuBudgetAction::None
            );
            assert_eq!(
                controller.repayment_quantization_credit_tick_percent,
                expected_credit
            );
            assert_eq!(controller.debt_tick_percent, 0);
        }
        assert_eq!(
            controller.observe(cpu_interval(100, 100)).unwrap(),
            HostCpuBudgetAction::Pause
        );
        assert_eq!(controller.debt_tick_percent, 2_000);
    }
}
