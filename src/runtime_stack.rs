//! Stack headroom for recursive evaluation. Ordinary recursion is limited
//! only by the evaluating thread's real stack: a nested call is refused with
//! a located error while enough stack remains to report it.

use std::cell::Cell;

/// Default stack for the thread that runs the `runa` command. The memory is
/// reserved, not committed: only the depth a program actually reaches uses it.
const DEFAULT_INTERPRETER_STACK_MIB: usize = 4096;

/// Stack a new function or rule call must leave free.
const CALL_RESERVE_BYTES: usize = 1 << 20;
/// Stack any expression evaluation must leave free. Smaller than the call
/// reserve so that recursive calls report the function or rule by name.
const EXPRESSION_RESERVE_BYTES: usize = 256 << 10;

const BOUND_NOT_COMPUTED: usize = 0;
const BOUND_UNAVAILABLE: usize = 1;

thread_local! {
    static STACK_LOW_BOUND: Cell<usize> = const { Cell::new(BOUND_NOT_COMPUTED) };
}

/// Stack size for the thread that runs the `runa` command, in bytes.
/// `FUTURUNA_STACK_MB` overrides the default.
pub fn interpreter_stack_bytes() -> usize {
    std::env::var("FUTURUNA_STACK_MB")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|mebibytes| *mebibytes > 0)
        .unwrap_or(DEFAULT_INTERPRETER_STACK_MIB)
        .saturating_mul(1 << 20)
}

/// `None` when this platform does not expose the current thread's stack.
pub(crate) fn call_stack_exhausted() -> Option<bool> {
    stack_below(CALL_RESERVE_BYTES)
}

pub(crate) fn expression_stack_exhausted() -> Option<bool> {
    stack_below(EXPRESSION_RESERVE_BYTES)
}

#[inline]
fn stack_below(reserve: usize) -> Option<bool> {
    let low = STACK_LOW_BOUND.with(|bound| {
        let mut value = bound.get();
        if value == BOUND_NOT_COMPUTED {
            value = current_thread_stack_low_bound()
                .filter(|low| *low > BOUND_UNAVAILABLE)
                .unwrap_or(BOUND_UNAVAILABLE);
            bound.set(value);
        }
        value
    });
    if low == BOUND_UNAVAILABLE {
        return None;
    }
    let marker = 0u8;
    let position = std::hint::black_box(std::ptr::addr_of!(marker)) as usize;
    Some(position < low.saturating_add(reserve))
}

#[cfg(target_os = "macos")]
fn current_thread_stack_low_bound() -> Option<usize> {
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_get_stackaddr_np(thread: usize) -> *mut u8;
        fn pthread_get_stacksize_np(thread: usize) -> usize;
    }
    // SAFETY: both functions only inspect the calling thread's descriptor.
    let (top, size) = unsafe {
        let thread = pthread_self();
        (
            pthread_get_stackaddr_np(thread) as usize,
            pthread_get_stacksize_np(thread),
        )
    };
    (top != 0 && size != 0 && size <= top).then(|| top - size)
}

#[cfg(target_os = "linux")]
fn current_thread_stack_low_bound() -> Option<usize> {
    /// Larger and more aligned than `pthread_attr_t` on every Linux libc.
    #[repr(C, align(16))]
    struct ThreadAttributes([u8; 128]);
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_getattr_np(thread: usize, attributes: *mut ThreadAttributes) -> i32;
        fn pthread_attr_getstack(
            attributes: *const ThreadAttributes,
            address: *mut *mut u8,
            size: *mut usize,
        ) -> i32;
        fn pthread_attr_destroy(attributes: *mut ThreadAttributes) -> i32;
    }
    let mut attributes = ThreadAttributes([0; 128]);
    let mut address = std::ptr::null_mut();
    let mut size = 0usize;
    // SAFETY: the attribute storage outlives every call and is destroyed
    // exactly once after `pthread_getattr_np` initialized it.
    let status = unsafe {
        if pthread_getattr_np(pthread_self(), &mut attributes) != 0 {
            return None;
        }
        let status = pthread_attr_getstack(&attributes, &mut address, &mut size);
        pthread_attr_destroy(&mut attributes);
        status
    };
    (status == 0 && !address.is_null() && size != 0).then_some(address as usize)
}

#[cfg(windows)]
fn current_thread_stack_low_bound() -> Option<usize> {
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low_limit: *mut usize, high_limit: *mut usize);
    }
    let (mut low, mut high) = (0usize, 0usize);
    // SAFETY: both pointers refer to live locals.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    (low != 0 && high > low).then_some(low)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn current_thread_stack_low_bound() -> Option<usize> {
    None
}
