//! Process-wide Rust-heap containment for a durable Explore child.
//!
//! A prospective supervised child accounts requested bytes from process start.
//! An ordinary `runa` process permanently selects a direct-System fast path at
//! the beginning of `main`; it can never install a limit later. After a child
//! has validated its parent identity, fresh
//! process-group shape and inherited liveness pipe, it installs exactly one
//! supervisor-computed limit before either CLI thread is spawned. This makes
//! allocations which race with limit installation fail closed as well.
//!
//! This is deliberately a live *Rust allocation-request* limit, not a hard
//! resident-memory quota. It does not account thread stacks, allocator
//! metadata or rounding, direct libc/FFI allocation, explicit `mmap`, shared
//! mappings, or subprocesses. The supervisor must retain an untracked-memory
//! reserve and its sampled process-group/host guards. Returning null on a
//! denied allocation lets fallible Rust allocation report an error; an
//! infallible allocation terminates only the recoverable Explore child.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

const LIMIT_NOT_INSTALLED: usize = usize::MAX;
const ACCOUNTING_PERMANENTLY_DISABLED: usize = usize::MAX - 1;

struct ExactStreamHeapAccounting {
    live_reserved_bytes: AtomicUsize,
    peak_reserved_bytes: AtomicUsize,
    limit_bytes: AtomicUsize,
}

impl ExactStreamHeapAccounting {
    const fn unlimited() -> Self {
        Self {
            live_reserved_bytes: AtomicUsize::new(0),
            peak_reserved_bytes: AtomicUsize::new(0),
            limit_bytes: AtomicUsize::new(LIMIT_NOT_INSTALLED),
        }
    }

    fn permanently_disable_for_ordinary_process(&self) {
        // `activate_exact_stream_child_liveness` is the first operation in
        // `main`. A process without the private child marker can never become
        // a supervised child later, so its startup accounting may be discarded
        // and every subsequent allocation can take the direct System path.
        let _ = self.limit_bytes.compare_exchange(
            LIMIT_NOT_INSTALLED,
            ACCOUNTING_PERMANENTLY_DISABLED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    fn permanently_disabled(&self) -> bool {
        self.limit_bytes.load(Ordering::Relaxed) == ACCOUNTING_PERMANENTLY_DISABLED
    }

    /// Reserve requested bytes before entering the system allocator.
    ///
    /// The second limit read closes the race in which installation occurs
    /// between the first limit read and the live-byte CAS. Conversely, if the
    /// reservation CAS precedes installation, the installer's subsequent live
    /// read observes it. Sequential consistency makes those alternatives one
    /// total order; neither can mint an over-limit successful reservation.
    fn try_reserve(&self, requested_bytes: usize) -> bool {
        if requested_bytes == 0 {
            return true;
        }

        let mut current = self.live_reserved_bytes.load(Ordering::SeqCst);
        loop {
            let Some(next) = current.checked_add(requested_bytes) else {
                return false;
            };
            let limit_before_reservation = self.limit_bytes.load(Ordering::SeqCst);
            if next > limit_before_reservation {
                return false;
            }
            match self.live_reserved_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    let limit_after_reservation = self.limit_bytes.load(Ordering::SeqCst);
                    if next > limit_after_reservation {
                        self.release_or_abort(requested_bytes);
                        return false;
                    }
                    self.record_peak(next);
                    return true;
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn release_or_abort(&self, released_bytes: usize) {
        if !self.try_release(released_bytes) {
            // Accounting underflow means the allocator contract has already
            // been violated. Panicking or formatting here could allocate and
            // recurse; abort is the only fail-closed response.
            std::process::abort();
        }
    }

    fn try_release(&self, released_bytes: usize) -> bool {
        if released_bytes == 0 {
            return true;
        }

        let mut current = self.live_reserved_bytes.load(Ordering::SeqCst);
        loop {
            let Some(next) = current.checked_sub(released_bytes) else {
                return false;
            };
            match self.live_reserved_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }

    fn record_peak(&self, reserved_bytes: usize) {
        let mut peak = self.peak_reserved_bytes.load(Ordering::Relaxed);
        while reserved_bytes > peak {
            match self.peak_reserved_bytes.compare_exchange_weak(
                peak,
                reserved_bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(observed) => peak = observed,
            }
        }
    }

    fn install_limit(
        &self,
        limit_bytes: usize,
    ) -> Result<ExactStreamHeapLimitReceipt, ExactStreamHeapLimitInstallError> {
        if limit_bytes == 0 {
            return Err(ExactStreamHeapLimitInstallError::ZeroLimit);
        }
        if limit_bytes >= ACCOUNTING_PERMANENTLY_DISABLED {
            return Err(ExactStreamHeapLimitInstallError::ReservedLimitValue);
        }

        if let Err(installed_limit_bytes) = self.limit_bytes.compare_exchange(
            LIMIT_NOT_INSTALLED,
            limit_bytes,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            return Err(ExactStreamHeapLimitInstallError::AlreadyInstalled {
                installed_limit_bytes,
            });
        }

        // The cap remains installed even when current startup allocations are
        // already too large. The validated child must exit on this error;
        // rolling the limit back would create an unsupervised allocation gap.
        let current_live_requested_bytes = self.live_reserved_bytes.load(Ordering::SeqCst);
        if current_live_requested_bytes > limit_bytes {
            return Err(ExactStreamHeapLimitInstallError::CurrentUsageExceedsLimit {
                current_live_requested_bytes,
                limit_bytes,
            });
        }

        Ok(ExactStreamHeapLimitReceipt {
            limit_bytes,
            current_live_requested_bytes,
            peak_reserved_requested_bytes: self.peak_reserved_bytes.load(Ordering::SeqCst),
        })
    }
}

struct ExactStreamHeapAllocator;

#[global_allocator]
static EXACT_STREAM_HEAP_ALLOCATOR: ExactStreamHeapAllocator = ExactStreamHeapAllocator;

static EXACT_STREAM_HEAP_ACCOUNTING: ExactStreamHeapAccounting =
    ExactStreamHeapAccounting::unlimited();

// SAFETY: while accounting is active, every successful allocation reserves its
// requested byte count before delegating to `System`; every corresponding
// deallocation releases exactly that layout's count after `System` has released
// it. Reallocation reserves the complete new allocation while the old
// allocation is still charged, covering the system allocator's possible
// allocate-copy-free transient. The permanently disabled ordinary-process path
// delegates every operation directly to the same `System` allocator.
unsafe impl GlobalAlloc for ExactStreamHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if EXACT_STREAM_HEAP_ACCOUNTING.permanently_disabled() {
            // SAFETY: the caller supplies the valid layout required by
            // `GlobalAlloc::alloc` and `System` is the wrapped allocator.
            return unsafe { System.alloc(layout) };
        }
        if !EXACT_STREAM_HEAP_ACCOUNTING.try_reserve(layout.size()) {
            return ptr::null_mut();
        }
        // SAFETY: the caller supplies the valid layout required by
        // `GlobalAlloc::alloc` and `System` is the wrapped allocator.
        let allocation = unsafe { System.alloc(layout) };
        if allocation.is_null() {
            EXACT_STREAM_HEAP_ACCOUNTING.release_or_abort(layout.size());
        }
        allocation
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if EXACT_STREAM_HEAP_ACCOUNTING.permanently_disabled() {
            // SAFETY: the caller supplies the valid layout required by
            // `GlobalAlloc::alloc_zeroed` and `System` is the wrapped allocator.
            return unsafe { System.alloc_zeroed(layout) };
        }
        if !EXACT_STREAM_HEAP_ACCOUNTING.try_reserve(layout.size()) {
            return ptr::null_mut();
        }
        // SAFETY: the caller supplies the valid layout required by
        // `GlobalAlloc::alloc_zeroed` and `System` is the wrapped allocator.
        let allocation = unsafe { System.alloc_zeroed(layout) };
        if allocation.is_null() {
            EXACT_STREAM_HEAP_ACCOUNTING.release_or_abort(layout.size());
        }
        allocation
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if EXACT_STREAM_HEAP_ACCOUNTING.permanently_disabled() {
            // SAFETY: the caller guarantees that `pointer` and `layout`
            // identify a live allocation produced by this allocator.
            unsafe { System.dealloc(pointer, layout) };
            return;
        }
        // Release physical storage before accounting capacity so another
        // allocation cannot consume the same budget while these bytes remain
        // owned by the system allocator.
        // SAFETY: the caller guarantees that `pointer` and `layout` identify a
        // live allocation produced by this allocator.
        unsafe { System.dealloc(pointer, layout) };
        EXACT_STREAM_HEAP_ACCOUNTING.release_or_abort(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        if EXACT_STREAM_HEAP_ACCOUNTING.permanently_disabled() {
            // SAFETY: the caller guarantees the wrapped allocator's realloc
            // contract for `pointer`, `old_layout`, and `new_size`.
            return unsafe { System.realloc(pointer, old_layout, new_size) };
        }
        // libc realloc may allocate the complete replacement before it frees
        // the old block. Reserve all `new_size` bytes, not merely the growth
        // delta, so that transient is contained. This can conservatively deny
        // an in-place reallocation near the cap.
        if !EXACT_STREAM_HEAP_ACCOUNTING.try_reserve(new_size) {
            return ptr::null_mut();
        }
        // SAFETY: the caller guarantees `pointer`, `old_layout`, and the
        // non-zero `new_size` satisfy `GlobalAlloc::realloc`'s contract.
        let allocation = unsafe { System.realloc(pointer, old_layout, new_size) };
        if allocation.is_null() {
            // A failed realloc leaves the old allocation live.
            EXACT_STREAM_HEAP_ACCOUNTING.release_or_abort(new_size);
            return allocation;
        }

        // A successful realloc has released/replaced the old allocation; the
        // full new allocation reservation remains charged.
        EXACT_STREAM_HEAP_ACCOUNTING.release_or_abort(old_layout.size());
        allocation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactStreamHeapLimitReceipt {
    pub(crate) limit_bytes: usize,
    pub(crate) current_live_requested_bytes: usize,
    pub(crate) peak_reserved_requested_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactStreamHeapLimitInstallError {
    ZeroLimit,
    LimitDoesNotFitAddressSpace {
        requested_limit_bytes: u64,
    },
    ReservedLimitValue,
    AlreadyInstalled {
        installed_limit_bytes: usize,
    },
    CurrentUsageExceedsLimit {
        current_live_requested_bytes: usize,
        limit_bytes: usize,
    },
}

impl fmt::Display for ExactStreamHeapLimitInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLimit => formatter.write_str("Rust heap limit must be positive"),
            Self::LimitDoesNotFitAddressSpace {
                requested_limit_bytes,
            } => write!(
                formatter,
                "Rust heap limit {requested_limit_bytes} does not fit this address space"
            ),
            Self::ReservedLimitValue => {
                formatter.write_str("Rust heap limit collides with the uninstalled sentinel")
            }
            Self::AlreadyInstalled {
                installed_limit_bytes,
            } => write!(
                formatter,
                "Rust heap limit was already installed at {installed_limit_bytes} bytes"
            ),
            Self::CurrentUsageExceedsLimit {
                current_live_requested_bytes,
                limit_bytes,
            } => write!(
                formatter,
                "current live Rust allocation requests ({current_live_requested_bytes} bytes) exceed the validated child limit ({limit_bytes} bytes)"
            ),
        }
    }
}

impl std::error::Error for ExactStreamHeapLimitInstallError {}

/// Irreversibly install the validated durable-child Rust heap limit.
///
/// The supervisor is the sole intended caller. It must invoke this only after
/// validating the child marker against the actual parent, process-group leader
/// and inherited anonymous liveness descriptor, and before spawning the
/// watchdog or the CLI main thread. A successful or current-usage-rejected
/// install cannot be relaxed for the lifetime of this process.
pub(crate) fn install_validated_exact_stream_child_heap_limit(
    limit_bytes: u64,
) -> Result<ExactStreamHeapLimitReceipt, ExactStreamHeapLimitInstallError> {
    let limit_bytes = usize::try_from(limit_bytes).map_err(|_| {
        ExactStreamHeapLimitInstallError::LimitDoesNotFitAddressSpace {
            requested_limit_bytes: limit_bytes,
        }
    })?;
    EXACT_STREAM_HEAP_ACCOUNTING.install_limit(limit_bytes)
}

/// Permanently select the zero-accounting fast path for an ordinary `runa`
/// process. A supervised child is always a fresh exec with its marker present,
/// so this state never needs to be reversed.
pub(crate) fn disable_exact_stream_heap_accounting_for_ordinary_process() {
    EXACT_STREAM_HEAP_ACCOUNTING.permanently_disable_for_ordinary_process();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_limit_includes_preexisting_live_requests_and_is_single_use() {
        let accounting = ExactStreamHeapAccounting::unlimited();
        assert!(accounting.try_reserve(64));

        let receipt = accounting.install_limit(96).expect("install limit");
        assert_eq!(receipt.limit_bytes, 96);
        assert_eq!(receipt.current_live_requested_bytes, 64);
        assert!(accounting.try_reserve(32));
        assert!(!accounting.try_reserve(1));
        assert_eq!(
            accounting.install_limit(128),
            Err(ExactStreamHeapLimitInstallError::AlreadyInstalled {
                installed_limit_bytes: 96,
            })
        );
    }

    #[test]
    fn over_limit_install_remains_irreversibly_fail_closed() {
        let accounting = ExactStreamHeapAccounting::unlimited();
        assert!(accounting.try_reserve(128));

        assert_eq!(
            accounting.install_limit(64),
            Err(ExactStreamHeapLimitInstallError::CurrentUsageExceedsLimit {
                current_live_requested_bytes: 128,
                limit_bytes: 64,
            })
        );
        assert!(!accounting.try_reserve(1));
        accounting.release_or_abort(128);
        assert!(accounting.try_reserve(64));
    }

    #[test]
    fn ordinary_process_disable_is_irreversible_and_skips_later_install() {
        let accounting = ExactStreamHeapAccounting::unlimited();
        assert!(accounting.try_reserve(64));

        accounting.permanently_disable_for_ordinary_process();
        assert!(accounting.permanently_disabled());
        assert_eq!(
            accounting.install_limit(128),
            Err(ExactStreamHeapLimitInstallError::AlreadyInstalled {
                installed_limit_bytes: ACCOUNTING_PERMANENTLY_DISABLED,
            })
        );
    }

    #[test]
    fn replacement_reservation_models_realloc_transient_without_overflow() {
        let accounting = ExactStreamHeapAccounting::unlimited();
        assert!(accounting.try_reserve(40));
        accounting.install_limit(100).expect("install limit");

        // Realloc keeps the old 40-byte block charged while reserving the
        // complete 60-byte replacement, then releases the old block.
        assert!(accounting.try_reserve(60));
        assert!(!accounting.try_reserve(1));
        accounting.release_or_abort(40);
        assert_eq!(accounting.live_reserved_bytes.load(Ordering::SeqCst), 60);

        accounting
            .live_reserved_bytes
            .store(usize::MAX - 2, Ordering::SeqCst);
        assert!(!accounting.try_reserve(3));
    }
}
