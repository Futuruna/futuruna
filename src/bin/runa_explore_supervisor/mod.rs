//! Outer sampled watchdog boundary for durable Explore CLI slices.
//!
//! The exact coordinator still performs its own fail-closed admission at work
//! boundaries. This supervisor is deliberately a different layer: it owns a
//! fresh process group containing the whole slice, samples that group and the
//! host while an atomic preparation/probe/case unit is running, and can stop
//! the group without sharing the run-state writer fence. A containment kill is
//! recovered by ordinary journal replay on the next invocation.

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
use std::os::unix::process::CommandExt;
#[cfg(target_os = "macos")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::thread;

pub(crate) const EXPLORE_STREAM_CHILD_MARKER: &str = "FUTURUNA_INTERNAL_EXPLORE_STREAM_CHILD_V2";
static IS_VALIDATED_EXPLORE_STREAM_CHILD: AtomicBool = AtomicBool::new(false);

const GROUP_SAMPLE_CADENCE: Duration = Duration::from_millis(100);
const HOST_SAMPLE_CADENCE: Duration = Duration::from_millis(500);
/// Host CPU decisions use a longer cumulative interval than the liveness/RSS
/// loop. Darwin reports separately floored scheduler ticks per CPU/state; one
/// second keeps a conservative rounding envelope meaningfully below the 10
/// percentage-point operational reserve.
const HOST_CPU_DECISION_WINDOW: Duration = Duration::from_secs(1);
const OUTER_DEADLINE_GRACE: Duration = Duration::from_secs(2);
const ONE_GIB: u64 = 1024 * 1024 * 1024;
#[cfg(target_os = "macos")]
const CHILD_HEARTBEAT_TIMEOUT_MILLIS: c_int = 5_000;

#[cfg(target_os = "macos")]
const CHILD_LIVENESS_FD: RawFd = 198;
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
#[repr(C)]
struct PollDescriptor {
    file_descriptor: c_int,
    requested_events: i16,
    returned_events: i16,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn close(file_descriptor: c_int) -> c_int;
    fn dup2(source: c_int, destination: c_int) -> c_int;
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

/// The operational upper bound remains 80%. The sampled watchdog trips
/// earlier so one sampling interval and termination latency retain headroom.
/// The validated child additionally receives a hard requested-Rust-heap
/// limit; stacks, direct FFI allocations, mappings and subprocesses remain
/// protected only by the reserve and sampled guards, not a kernel RSS quota.
const ABSOLUTE_CEILING_PERCENT: u64 = 80;
const MEMORY_TRIP_PERCENT: u64 = 70;
const CPU_TRIP_PERCENT: u64 = 70;
const UNTRACKED_MEMORY_RESERVE_PERCENT: u64 = 10;

#[derive(Debug)]
pub(crate) enum ExactStreamSupervisionOutcome {
    Exited(ExitStatus),
    Contained(ExactStreamContainmentReport),
}

#[derive(Debug, Clone)]
pub(crate) struct ExactStreamContainmentReport {
    pub(crate) reason: ExactStreamContainmentReason,
    pub(crate) observed_group_rss_bytes: Option<u64>,
    pub(crate) group_rss_limit_bytes: u64,
    pub(crate) observed_available_memory_bytes: Option<u64>,
    pub(crate) available_memory_floor_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) enum ExactStreamContainmentReason {
    OuterDeadline,
    GroupMemory,
    HostMemory,
    HostPressure,
    SwapGrowth,
    GroupCpu,
    HostCpu,
    TelemetryLost,
    ResidualProcesses,
}

impl fmt::Display for ExactStreamContainmentReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OuterDeadline => "outer wall deadline",
            Self::GroupMemory => "exploration process-group memory guard",
            Self::HostMemory => "host available-memory guard",
            Self::HostPressure => "host memory pressure",
            Self::SwapGrowth => "growing host swap-out counter",
            Self::GroupCpu => "exploration leader CPU guard",
            Self::HostCpu => "total host CPU guard",
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

/// Validate and arm the child-side end of the parent liveness channel.
///
/// This runs before the CLI creates its large-stack main thread. The marker is
/// not itself authority: the claimed parent, fresh process-group shape and
/// inherited liveness descriptor must all agree. This validates the expected
/// supervisor shape; it is not cryptographic authentication against a hostile
/// local launcher. The pipe then carries bounded
/// monitor heartbeats: EOF or a missed heartbeat kills the child group. Once
/// armed, the marker is removed so descendants cannot accidentally bypass
/// their own supervisor.
pub(crate) fn activate_exact_stream_child_liveness() -> Result<(), ExactStreamSupervisorError> {
    let Some(marker) = std::env::var_os(EXPLORE_STREAM_CHILD_MARKER) else {
        crate::runa_explore_heap::disable_exact_stream_heap_accounting_for_ordinary_process();
        return Ok(());
    };

    #[cfg(not(target_os = "macos"))]
    {
        let _ = marker;
        return Err(ExactStreamSupervisorError::UnsupportedPlatform);
    }

    #[cfg(target_os = "macos")]
    {
        let marker = marker
            .into_string()
            .map_err(|_| ExactStreamSupervisorError::Telemetry("child liveness marker"))?;
        let (claimed_parent, remainder) =
            marker
                .split_once(':')
                .ok_or(ExactStreamSupervisorError::Telemetry(
                    "child liveness marker",
                ))?;
        let (claimed_fd, claimed_heap_limit) = remainder
            .split_once(':')
            .filter(|(_, limit)| !limit.contains(':'))
            .ok_or(ExactStreamSupervisorError::Telemetry(
                "child liveness marker",
            ))?;
        let claimed_parent = claimed_parent
            .parse::<u32>()
            .ok()
            .filter(|parent| *parent != 0)
            .ok_or(ExactStreamSupervisorError::Telemetry(
                "child liveness parent",
            ))?;
        let claimed_fd = claimed_fd
            .parse::<RawFd>()
            .ok()
            .filter(|fd| *fd == CHILD_LIVENESS_FD)
            .ok_or(ExactStreamSupervisorError::Telemetry(
                "child liveness descriptor",
            ))?;
        let claimed_heap_limit = claimed_heap_limit
            .parse::<u64>()
            .ok()
            .filter(|limit| *limit != 0)
            .ok_or(ExactStreamSupervisorError::Telemetry(
                "child Rust heap limit",
            ))?;

        // SAFETY: these process-identity calls take no pointers and have no
        // preconditions. The child is required to be the process-group leader
        // created by CommandExt::process_group(0).
        let (actual_parent, actual_pid, actual_group) = unsafe { (getppid(), getpid(), getpgrp()) };
        if actual_parent <= 0
            || actual_parent as u32 != claimed_parent
            || actual_pid <= 0
            || actual_group != actual_pid
        {
            return Err(ExactStreamSupervisorError::Telemetry(
                "child liveness process identity",
            ));
        }
        set_close_on_exec(claimed_fd, true).map_err(ExactStreamSupervisorError::Monitor)?;
        crate::runa_explore_heap::install_validated_exact_stream_child_heap_limit(
            claimed_heap_limit,
        )
        .map_err(ExactStreamSupervisorError::HeapLimit)?;
        std::env::remove_var(EXPLORE_STREAM_CHILD_MARKER);
        IS_VALIDATED_EXPLORE_STREAM_CHILD.store(true, Ordering::Release);

        thread::Builder::new()
            .name("runa-explore-parent-watchdog".to_string())
            .spawn(move || child_parent_watchdog(claimed_fd))
            .map_err(ExactStreamSupervisorError::Monitor)?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn child_parent_watchdog(liveness_fd: RawFd) -> ! {
    let mut byte = 0_u8;
    let heartbeat_timeout = Duration::from_millis(
        u64::try_from(CHILD_HEARTBEAT_TIMEOUT_MILLIS)
            .expect("the child heartbeat timeout is positive"),
    );
    let mut heartbeat_deadline = Instant::now()
        .checked_add(heartbeat_timeout)
        .expect("the bounded child heartbeat deadline fits the monotonic clock");
    loop {
        let now = Instant::now();
        if now >= heartbeat_deadline {
            break;
        }
        let remaining = heartbeat_deadline.saturating_duration_since(now);
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
            if io::Error::last_os_error().raw_os_error() == Some(EINTR) {
                continue;
            }
            break;
        }
        if ready == 0 || descriptor.returned_events & POLL_ERROR_MASK != 0 {
            break;
        }
        if descriptor.returned_events & POLLIN == 0 {
            break;
        }
        // SAFETY: `liveness_fd` was validated with fcntl before this
        // thread started and `byte` is writable for exactly one byte.
        let result = unsafe { read(liveness_fd, (&mut byte as *mut u8).cast::<c_void>(), 1) };
        if result == 1 && byte == HEARTBEAT_BYTE {
            heartbeat_deadline = Instant::now()
                .checked_add(heartbeat_timeout)
                .expect("the bounded child heartbeat deadline fits the monotonic clock");
            continue;
        }
        if result < 0 && io::Error::last_os_error().raw_os_error() == Some(EINTR) {
            continue;
        }
        break;
    }

    // SAFETY: signal zero as a pid selects this process group, and SIGKILL is
    // valid on macOS. The call either terminates the complete worker group or
    // returns an error, in which case abort still prevents unsupervised work.
    let _ = unsafe { kill(0, SIGKILL) };
    std::process::abort()
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
    let available_floor =
        percent_ceil(initial.total_memory_bytes, 100 - MEMORY_TRIP_PERCENT).max(ONE_GIB);
    let preflight_swap_growth = initial.swapout_pages > preflight_host.swapout_pages;
    if preflight_host.pressure != 0
        || initial.pressure != 0
        || preflight_host.throttled_pages != 0
        || initial.throttled_pages != 0
        || preflight_swap_growth
        || initial.available_memory_bytes <= available_floor
        || initial_cpu_hundredths > CPU_TRIP_PERCENT * 100
    {
        return Err(ExactStreamSupervisorError::UnsafeInitialHost {
            available_memory_bytes: initial.available_memory_bytes,
            required_floor_bytes: available_floor,
            pressure: initial.pressure,
            cpu_hundredths_percent: initial_cpu_hundredths,
            swapout_growth: preflight_swap_growth,
        });
    }

    let absolute_group_ceiling =
        percent_floor(initial.total_memory_bytes, ABSOLUTE_CEILING_PERCENT);
    let initial_headroom = initial.available_memory_bytes - available_floor;
    let group_rss_limit = absolute_group_ceiling.min(initial_headroom);
    let untracked_memory_reserve =
        percent_ceil(initial.total_memory_bytes, UNTRACKED_MEMORY_RESERVE_PERCENT).max(ONE_GIB);
    let rust_heap_limit = group_rss_limit.checked_sub(untracked_memory_reserve);
    if group_rss_limit == 0 || rust_heap_limit.is_none_or(|limit| limit == 0) {
        return Err(ExactStreamSupervisorError::UnsafeInitialHost {
            available_memory_bytes: initial.available_memory_bytes,
            required_floor_bytes: available_floor
                .checked_add(untracked_memory_reserve)
                .unwrap_or(u64::MAX),
            pressure: initial.pressure,
            cpu_hundredths_percent: initial_cpu_hundredths,
            swapout_growth: preflight_swap_growth,
        });
    }
    let rust_heap_limit = rust_heap_limit.expect("positive heap headroom was checked");

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
    let (child_liveness_reader, parent_liveness_writer) =
        create_liveness_pipe().map_err(ExactStreamSupervisorError::Spawn)?;
    let inherited_reader = child_liveness_reader.as_raw_fd();
    let marker = format!(
        "{}:{CHILD_LIVENESS_FD}:{rust_heap_limit}",
        std::process::id()
    );
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env(EXPLORE_STREAM_CHILD_MARKER, marker)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0);
    // SAFETY: the closure performs only async-signal-safe descriptor syscalls.
    // It duplicates the read side of a fresh anonymous pipe to a fixed child
    // descriptor and makes only that duplicate survive exec.
    unsafe {
        command.pre_exec(move || {
            if dup2(inherited_reader, CHILD_LIVENESS_FD) < 0 {
                return Err(io::Error::last_os_error());
            }
            if inherited_reader != CHILD_LIVENESS_FD {
                close(inherited_reader);
            }
            set_close_on_exec(CHILD_LIVENESS_FD, false)
        });
    }
    let child = command.spawn().map_err(ExactStreamSupervisorError::Spawn)?;
    // This instant is after a successful spawn, so using it as the zero-CPU
    // baseline can only shorten the first accounting interval. That makes the
    // first rate estimate conservative.
    let child_spawned_at = Instant::now();
    drop(child_liveness_reader);
    let mut child = ChildGroupGuard::new(child, parent_liveness_writer);
    if let Err(error) = child.heartbeat() {
        if let Ok(Some(status)) = child.try_wait() {
            return finish_exited_child(
                &mut child,
                status,
                None,
                group_rss_limit,
                initial.available_memory_bytes,
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
        let child_status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
        };
        if let Some(status) = child_status {
            return finish_exited_child(
                &mut child,
                status,
                last_group,
                group_rss_limit,
                last_host.available_memory_bytes,
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
                let child_status = match child.try_wait() {
                    Ok(status) => status,
                    Err(error) => return Err(contain_after_monitor_error(&mut child, error)),
                };
                if let Some(status) = child_status {
                    return finish_exited_child(
                        &mut child,
                        status,
                        last_group,
                        group_rss_limit,
                        last_host.available_memory_bytes,
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
                last_host.available_memory_bytes,
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
            return contain(
                &mut child,
                ExactStreamContainmentReason::GroupCpu,
                Some(group),
                group_rss_limit,
                Some(last_host.available_memory_bytes),
                available_floor,
            );
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
            match host_cpu_upper_hundredths(&last_host_cpu, &host_cpu) {
                Ok(Some(hundredths)) => {
                    last_host_cpu = host_cpu;
                    next_host_cpu_sample = now.checked_add(HOST_CPU_DECISION_WINDOW).unwrap_or(now);
                    if hundredths > CPU_TRIP_PERCENT * 100 {
                        return contain(
                            &mut child,
                            ExactStreamContainmentReason::HostCpu,
                            Some(group),
                            group_rss_limit,
                            Some(last_host.available_memory_bytes),
                            available_floor,
                        );
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
            let reason = if host.pressure != 0 || host.throttled_pages != 0 {
                Some(ExactStreamContainmentReason::HostPressure)
            } else if host.swapout_pages > last_host.swapout_pages {
                Some(ExactStreamContainmentReason::SwapGrowth)
            } else if host.available_memory_bytes < available_floor {
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
                    last_host.available_memory_bytes,
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
    observed_available_memory_bytes: u64,
    available_memory_floor_bytes: u64,
) -> Result<ExactStreamSupervisionOutcome, ExactStreamSupervisorError> {
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
                observed_available_memory_bytes: Some(observed_available_memory_bytes),
                available_memory_floor_bytes,
            },
        ));
    }
    Ok(ExactStreamSupervisionOutcome::Exited(status))
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
        },
    ))
}

#[cfg(target_os = "macos")]
struct ChildGroupGuard {
    child: Child,
    process_group: u32,
    liveness_writer: Option<File>,
    leader_reaped: bool,
    armed: bool,
}

#[cfg(target_os = "macos")]
impl ChildGroupGuard {
    fn new(child: Child, liveness_writer: File) -> Self {
        let process_group = child.id();
        Self {
            child,
            process_group,
            liveness_writer: Some(liveness_writer),
            leader_reaped: false,
            armed: true,
        }
    }

    fn process_group(&self) -> u32 {
        self.process_group
    }

    fn heartbeat(&self) -> io::Result<()> {
        let writer = self.liveness_writer.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "child liveness writer is closed")
        })?;
        let byte = HEARTBEAT_BYTE;
        loop {
            // SAFETY: the writer descriptor is owned by this armed guard and
            // `byte` is readable for exactly one byte. F_SETNOSIGPIPE was set
            // before spawn, so an exited child yields EPIPE rather than a
            // process-level SIGPIPE.
            let written =
                unsafe { write(writer.as_raw_fd(), (&byte as *const u8).cast::<c_void>(), 1) };
            if written == 1 {
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
                "child liveness heartbeat wrote no byte",
            ));
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.leader_reaped = true;
        }
        Ok(status)
    }

    /// Finish a normal leader exit. Returns true when residual group members
    /// existed and had to be contained before the status could be returned.
    fn finish_exited_group(&mut self) -> io::Result<bool> {
        self.liveness_writer.take();
        let residual = process_group_exists(self.process_group)?;
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
        let _ = signal_process_group(self.process_group, SIGSTOP);
        let mut last_error = signal_process_group(self.process_group, SIGKILL).err();
        if !self.leader_reaped {
            let _ = self.child.kill();
        }

        for _ in 0..100 {
            if !self.leader_reaped {
                match self.child.try_wait() {
                    Ok(Some(_)) => self.leader_reaped = true,
                    Ok(None) => {}
                    Err(error) => last_error = Some(error),
                }
            }
            match process_group_exists(self.process_group) {
                Ok(false) if self.leader_reaped => {
                    self.armed = false;
                    return Ok(());
                }
                Ok(false) => {}
                Ok(true) => {
                    if let Err(error) = signal_process_group(self.process_group, SIGKILL) {
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
                    "process group {} or its leader remained after bounded SIGKILL verification",
                    self.process_group
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
        let _ = signal_process_group(self.process_group, SIGKILL);
        let _ = self.child.kill();
        for _ in 0..10 {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
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
fn process_group_exists(process_group: u32) -> io::Result<bool> {
    signal_process_group(process_group, 0)
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct GroupSnapshot {
    rss_bytes: u64,
    cumulative_cpu_centiseconds: u64,
    process_id: u32,
    process_started: ProcessStartIdentity,
    sample_started: Instant,
    sample_finished: Instant,
}

#[cfg(target_os = "macos")]
fn collect_group_snapshot(process_group: u32) -> io::Result<Option<GroupSnapshot>> {
    // The stream child currently launches only bounded host-telemetry helpers;
    // they remain in this process group so containment can never orphan one.
    // RSS therefore covers every member visible at the sample. Per-group CPU
    // is a supplemental leader-thread signal only: the separate Mach host
    // counters are the authoritative cap and retain work from helpers that
    // start and exit between two `ps` samples. Arbitrary evaluator subprocess
    // workers remain forbidden until event-backed tree memory accounting is
    // integrated.
    let sample_started = Instant::now();
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,pgid=,rss=,lstart=,time="])
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
        cumulative_cpu_centiseconds: process.cumulative_cpu_centiseconds,
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
            (
                previous.cumulative_cpu_centiseconds,
                previous.sample_finished,
            )
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
        .cumulative_cpu_centiseconds
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
fn host_cpu_upper_hundredths(
    previous: &HostCpuTicks,
    current: &HostCpuTicks,
) -> io::Result<Option<u64>> {
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
    Ok(Some(hundredths))
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
