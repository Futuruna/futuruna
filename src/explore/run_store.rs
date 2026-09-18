//! Owner-local, append-only filesystem storage for exploration run artifacts.
//!
//! A successful [`RunStoreGuard::open`] retains both the directory descriptor and
//! an exclusive advisory lock.  After acquisition, no operation resolves the
//! caller's directory path again: every name lookup is relative to that retained
//! descriptor.  This matters if an ancestor is renamed or replaced while a run
//! is active.
//!
//! This module deliberately does not expose a durability receipt, deletion,
//! overwrite, scavenging, or recovery API.  Installing an entry either confirms
//! an existing byte-for-byte-identical file or performs this sequence:
//!
//! 1. create a same-directory private temporary file with `openat(O_EXCL)`,
//! 2. write, `fsync`, and read it back exactly,
//! 3. add the final name with no-overwrite `linkat`,
//! 4. `fsync` the directory,
//! 5. remove only the inode-correlated temporary name with `unlinkat`,
//! 6. `fsync` the directory again, then reopen/read/sync the final entry.
//!
//! An error after the hard link is made never claims completion and may leave an
//! intentionally visible residue.  There is no recovery claim here; a future
//! adapter must define that policy separately.
//!
//! The security boundary is the effective user: same-EUID processes are trusted
//! to honor the advisory lock and not mutate this owner-only directory behind a
//! live guard.  Descriptor anchoring and inode correlation fail closed for
//! namespace replacement and cooperative races, but POSIX has no atomic
//! compare-inode-and-unlink primitive against a hostile same-EUID mutator.

use std::fmt;
use std::io;
use std::path::Path;

/// Absolute policy bounds keep all read and enumeration allocation finite even
/// if a caller supplies hostile configuration.
pub const RUN_STORE_MAX_ENTRY_BYTES: u64 = 1 << 30;
pub const RUN_STORE_MAX_LIST_ENTRIES: usize = 1_000_000;
pub const RUN_STORE_MAX_SCAN_ENTRIES: usize = 2_000_000;
pub const RUN_STORE_MAX_NAME_BYTES: usize = 160;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunStoreLimits {
    max_entry_bytes: u64,
    max_list_entries: usize,
    max_scan_entries: usize,
}

impl RunStoreLimits {
    pub fn new(
        max_entry_bytes: u64,
        max_list_entries: usize,
        max_scan_entries: usize,
    ) -> Result<Self, RunStoreError> {
        if max_entry_bytes == 0 || max_entry_bytes > RUN_STORE_MAX_ENTRY_BYTES {
            return Err(RunStoreError::InvalidLimits(
                "max_entry_bytes must be within the hard nonzero bound",
            ));
        }
        if max_entry_bytes.checked_add(1).is_none() || usize::try_from(max_entry_bytes + 1).is_err()
        {
            return Err(RunStoreError::InvalidLimits(
                "max_entry_bytes cannot be represented for bounded reads",
            ));
        }
        if max_list_entries == 0 || max_list_entries > RUN_STORE_MAX_LIST_ENTRIES {
            return Err(RunStoreError::InvalidLimits(
                "max_list_entries must be within the hard nonzero bound",
            ));
        }
        if max_scan_entries < max_list_entries || max_scan_entries > RUN_STORE_MAX_SCAN_ENTRIES {
            return Err(RunStoreError::InvalidLimits(
                "max_scan_entries must cover max_list_entries and stay bounded",
            ));
        }
        Ok(Self {
            max_entry_bytes,
            max_list_entries,
            max_scan_entries,
        })
    }

    pub fn max_entry_bytes(self) -> u64 {
        self.max_entry_bytes
    }

    pub fn max_list_entries(self) -> usize {
        self.max_list_entries
    }

    pub fn max_scan_entries(self) -> usize {
        self.max_scan_entries
    }
}

impl Default for RunStoreLimits {
    fn default() -> Self {
        Self {
            max_entry_bytes: 64 << 20,
            max_list_entries: 100_000,
            max_scan_entries: 200_000,
        }
    }
}

/// Descriptive metadata only.  It is not a durability receipt or authority.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RunStoreEntry {
    name: String,
    bytes: u64,
}

impl RunStoreEntry {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Debug)]
pub enum RunStoreError {
    Unsupported,
    InvalidLimits(&'static str),
    InvalidDirectory(&'static str),
    InvalidEntryName(String),
    InvalidEntry {
        name: String,
        reason: &'static str,
    },
    LockBusy,
    EntryNotFound(String),
    EntryTooLarge {
        name: String,
        bytes: u64,
        limit: u64,
    },
    ImmutableConflict(String),
    ScanLimitExceeded(usize),
    ListLimitExceeded(usize),
    TemporaryNamespaceExhausted,
    Io {
        operation: &'static str,
        source: io::Error,
    },
    CleanupFailed {
        primary: Box<RunStoreError>,
        cleanup: Box<RunStoreError>,
    },
}

impl fmt::Display for RunStoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(out, "run-store filesystem locking is unsupported here"),
            Self::InvalidLimits(reason) => write!(out, "invalid run-store limits: {reason}"),
            Self::InvalidDirectory(reason) => write!(out, "invalid run-store directory: {reason}"),
            Self::InvalidEntryName(name) => write!(out, "unsafe run-store entry name: {name:?}"),
            Self::InvalidEntry { name, reason } => {
                write!(out, "invalid run-store entry {name:?}: {reason}")
            }
            Self::LockBusy => write!(out, "the run-store directory is already locked"),
            Self::EntryNotFound(name) => write!(out, "run-store entry not found: {name:?}"),
            Self::EntryTooLarge { name, bytes, limit } => write!(
                out,
                "run-store entry {name:?} has {bytes} bytes, exceeding limit {limit}"
            ),
            Self::ImmutableConflict(name) => {
                write!(out, "immutable run-store entry differs: {name:?}")
            }
            Self::ScanLimitExceeded(limit) => {
                write!(out, "run-store directory scan exceeded limit {limit}")
            }
            Self::ListLimitExceeded(limit) => {
                write!(out, "run-store entry list exceeded limit {limit}")
            }
            Self::TemporaryNamespaceExhausted => {
                write!(out, "could not allocate a bounded private temporary name")
            }
            Self::Io { operation, source } => write!(out, "{operation}: {source}"),
            Self::CleanupFailed { primary, cleanup } => {
                write!(
                    out,
                    "{primary}; private temporary cleanup also failed: {cleanup}"
                )
            }
        }
    }
}

impl std::error::Error for RunStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::CleanupFailed { primary, .. } => Some(primary.as_ref()),
            _ => None,
        }
    }
}

fn io_error(operation: &'static str, source: io::Error) -> RunStoreError {
    RunStoreError::Io { operation, source }
}

fn validate_entry_name(name: &str) -> Result<(), RunStoreError> {
    let bytes = name.as_bytes();
    let edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    let body = |byte: u8| edge(byte) || matches!(byte, b'.' | b'_' | b'-');
    if bytes.is_empty()
        || bytes.len() > RUN_STORE_MAX_NAME_BYTES
        || !edge(bytes[0])
        || !edge(bytes[bytes.len() - 1])
        || !bytes.iter().copied().all(body)
        || bytes.windows(2).any(|pair| pair == b"..")
    {
        return Err(RunStoreError::InvalidEntryName(name.to_owned()));
    }
    Ok(())
}

const LOCK_NAME: &str = ".futuruna-run-store.lock";
const TEMP_PREFIX: &str = ".futuruna-tmp-";

fn is_internal_temp_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(TEMP_PREFIX) else {
        return false;
    };
    let mut fields = suffix.split('-');
    let Some(process) = fields.next() else {
        return false;
    };
    let Some(counter) = fields.next() else {
        return false;
    };
    fields.next().is_none()
        && !process.is_empty()
        && !counter.is_empty()
        && process
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && counter
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
mod supported_unix {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::fs::{self, File, Metadata, Permissions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::mem::MaybeUninit;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
    use std::sync::atomic::{AtomicU64, Ordering};

    const O_RDONLY: c_int = 0;
    const O_RDWR: c_int = 2;
    const LOCK_EX: c_int = 2;
    const LOCK_NB: c_int = 4;
    const AT_SYMLINK_NOFOLLOW: c_int = if cfg!(target_os = "linux") {
        0x100
    } else {
        0x20
    };

    #[cfg(target_os = "linux")]
    const O_CREAT: c_int = 0o100;
    #[cfg(target_os = "linux")]
    const O_EXCL: c_int = 0o200;
    #[cfg(target_os = "linux")]
    const O_DIRECTORY: c_int = 0o200000;
    #[cfg(target_os = "linux")]
    const O_NOFOLLOW: c_int = 0o400000;
    #[cfg(target_os = "linux")]
    const O_CLOEXEC: c_int = 0o2000000;
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: c_int = 0o4000;
    #[cfg(target_os = "linux")]
    const O_NOCTTY: c_int = 0o400;

    #[cfg(target_os = "macos")]
    const O_CREAT: c_int = 0x0200;
    #[cfg(target_os = "macos")]
    const O_EXCL: c_int = 0x0800;
    #[cfg(target_os = "macos")]
    const O_DIRECTORY: c_int = 0x0010_0000;
    #[cfg(target_os = "macos")]
    const O_NOFOLLOW: c_int = 0x0100;
    #[cfg(target_os = "macos")]
    const O_CLOEXEC: c_int = 0x0100_0000;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: c_int = 0x0004;
    #[cfg(target_os = "macos")]
    const O_NOCTTY: c_int = 0x0002_0000;

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    #[repr(C)]
    struct PlatformStat {
        st_dev: u64,
        st_ino: u64,
        st_nlink: u64,
        st_mode: u32,
        st_uid: u32,
        st_gid: u32,
        pad0: i32,
        st_rdev: u64,
        st_size: i64,
        st_blksize: i64,
        st_blocks: i64,
        st_atime: i64,
        st_atime_nsec: i64,
        st_mtime: i64,
        st_mtime_nsec: i64,
        st_ctime: i64,
        st_ctime_nsec: i64,
        unused: [i64; 3],
    }

    #[cfg(target_os = "macos")]
    #[allow(dead_code)]
    #[repr(C)]
    struct PlatformTimespec {
        tv_sec: i64,
        tv_nsec: i64,
    }

    #[cfg(target_os = "macos")]
    #[allow(dead_code)]
    #[repr(C)]
    struct PlatformStat {
        st_dev: i32,
        st_mode: u16,
        st_nlink: u16,
        st_ino: u64,
        st_uid: u32,
        st_gid: u32,
        st_rdev: i32,
        st_atimespec: PlatformTimespec,
        st_mtimespec: PlatformTimespec,
        st_ctimespec: PlatformTimespec,
        st_birthtimespec: PlatformTimespec,
        st_size: i64,
        st_blocks: i64,
        st_blksize: i32,
        st_flags: u32,
        st_gen: u32,
        st_lspare: i32,
        st_qspare: [i64; 2],
    }

    impl PlatformStat {
        fn identity(&self) -> FileIdentity {
            FileIdentity {
                device: self.st_dev as u64,
                inode: self.st_ino,
            }
        }

        fn mode(&self) -> u32 {
            self.st_mode as u32
        }
    }

    #[allow(dead_code)]
    #[repr(C)]
    struct DirectoryStream {
        _private: [u8; 0],
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    #[repr(C)]
    struct DirectoryEntry {
        d_ino: u64,
        d_off: i64,
        d_reclen: u16,
        d_type: u8,
        d_name: [c_char; 256],
    }

    #[cfg(target_os = "macos")]
    #[allow(dead_code)]
    #[repr(C)]
    struct DirectoryEntry {
        d_ino: u64,
        d_seekoff: u64,
        d_reclen: u16,
        d_namlen: u16,
        d_type: u8,
        d_name: [c_char; 1024],
    }

    unsafe extern "C" {
        #[link_name = "open"]
        fn c_open(path: *const c_char, flags: c_int, ...) -> c_int;
        #[link_name = "openat"]
        fn c_openat(directory: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
        #[link_name = "linkat"]
        fn c_linkat(
            old_directory: c_int,
            old_path: *const c_char,
            new_directory: c_int,
            new_path: *const c_char,
            flags: c_int,
        ) -> c_int;
        #[link_name = "unlinkat"]
        fn c_unlinkat(directory: c_int, path: *const c_char, flags: c_int) -> c_int;
        #[link_name = "flock"]
        fn c_flock(descriptor: c_int, operation: c_int) -> c_int;
        #[link_name = "closedir"]
        fn c_closedir(stream: *mut DirectoryStream) -> c_int;
        #[link_name = "close"]
        fn c_close(descriptor: c_int) -> c_int;
        #[link_name = "geteuid"]
        fn c_geteuid() -> c_uint;
    }

    #[cfg(any(
        all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    unsafe extern "C" {
        #[link_name = "fstatat"]
        fn c_fstatat(
            directory: c_int,
            path: *const c_char,
            status: *mut PlatformStat,
            flags: c_int,
        ) -> c_int;
        #[link_name = "fdopendir"]
        fn c_fdopendir(descriptor: c_int) -> *mut DirectoryStream;
        #[link_name = "readdir"]
        fn c_readdir(stream: *mut DirectoryStream) -> *mut DirectoryEntry;
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    unsafe extern "C" {
        #[link_name = "fstatat$INODE64"]
        fn c_fstatat(
            directory: c_int,
            path: *const c_char,
            status: *mut PlatformStat,
            flags: c_int,
        ) -> c_int;
        #[link_name = "fdopendir$INODE64"]
        fn c_fdopendir(descriptor: c_int) -> *mut DirectoryStream;
        #[link_name = "readdir$INODE64"]
        fn c_readdir(stream: *mut DirectoryStream) -> *mut DirectoryEntry;
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        #[link_name = "__errno_location"]
        fn errno_location() -> *mut c_int;
    }

    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        #[link_name = "__error"]
        fn errno_location() -> *mut c_int;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FileIdentity {
        device: u64,
        inode: u64,
    }

    impl FileIdentity {
        fn of(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
    }

    const S_IFMT: u32 = 0o170000;
    const S_IFREG: u32 = 0o100000;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct NameSnapshot {
        identity: FileIdentity,
        mode: u32,
    }

    impl NameSnapshot {
        fn is_regular(self) -> bool {
            self.mode & S_IFMT == S_IFREG
        }
    }

    /// The lock and directory descriptors are the authority.  The guard has no
    /// `Clone` implementation and exposes neither descriptor nor a clone method.
    pub struct RunStoreGuard {
        directory_file: File,
        directory_identity: FileIdentity,
        lock_file: File,
        lock_identity: FileIdentity,
        owner_uid: u32,
        limits: RunStoreLimits,
    }

    impl RunStoreGuard {
        /// Race-safely create an owner-only store directory when absent, then
        /// acquire the same descriptor-anchored guard as [`Self::open`].  An
        /// existing file, symlink, foreign-owned directory, or directory with
        /// group/other access is still rejected by `open`.
        pub fn open_or_create(
            directory: impl AsRef<Path>,
            limits: RunStoreLimits,
        ) -> Result<Self, RunStoreError> {
            let directory = directory.as_ref();
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error("create run-store directory", error)),
            }
            Self::open(directory, limits)
        }

        pub fn open(
            directory: impl AsRef<Path>,
            limits: RunStoreLimits,
        ) -> Result<Self, RunStoreError> {
            // Validate the supplied namespace only during acquisition.  It is
            // intentionally not retained in the resulting guard.
            let directory = directory.as_ref();
            let before = fs::symlink_metadata(directory)
                .map_err(|error| io_error("inspect run-store directory", error))?;
            if before.file_type().is_symlink() {
                return Err(RunStoreError::InvalidDirectory(
                    "the directory itself must not be a symbolic link",
                ));
            }
            let owner_uid = unsafe { c_geteuid() } as u32;
            verify_directory_metadata(&before, owner_uid)?;

            let path = CString::new(directory.as_os_str().as_bytes()).map_err(|_| {
                RunStoreError::InvalidDirectory("the directory path contains a NUL byte")
            })?;
            let descriptor = unsafe {
                c_open(
                    path.as_ptr(),
                    O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC,
                    0,
                )
            };
            let directory_file = file_from_descriptor(descriptor, "open run-store directory")?;
            let opened = directory_file
                .metadata()
                .map_err(|error| io_error("inspect opened run-store directory", error))?;
            verify_directory_metadata(&opened, owner_uid)?;
            if FileIdentity::of(&before) != FileIdentity::of(&opened) {
                return Err(RunStoreError::InvalidDirectory(
                    "the directory changed while it was being opened",
                ));
            }

            let directory_identity = FileIdentity::of(&opened);
            let (lock_file, lock_identity) = acquire_lock(&directory_file, owner_uid)?;
            let guard = Self {
                directory_file,
                directory_identity,
                lock_file,
                lock_identity,
                owner_uid,
                limits,
            };
            guard.validate_guard()?;
            Ok(guard)
        }

        pub fn acquire(
            directory: impl AsRef<Path>,
            limits: RunStoreLimits,
        ) -> Result<Self, RunStoreError> {
            Self::open(directory, limits)
        }

        pub fn limits(&self) -> RunStoreLimits {
            self.limits
        }

        pub fn read_entry(&self, name: &str) -> Result<Vec<u8>, RunStoreError> {
            validate_entry_name(name)?;
            self.validate_guard()?;
            let (mut file, identity, _) = self.open_data_name(name, 1)?;
            let bytes = self.read_bounded(name, &mut file)?;
            self.revalidate_data_name(name, identity, 1)?;
            self.validate_guard()?;
            Ok(bytes)
        }

        pub fn list_entries(&self) -> Result<Vec<RunStoreEntry>, RunStoreError> {
            self.validate_guard()?;
            let mut stream = DirectoryReader::open(&self.directory_file, self.directory_identity)?;
            let mut scanned = 0usize;
            let mut entries = Vec::new();
            while let Some(raw_name) = stream.next_name()? {
                scanned = scanned
                    .checked_add(1)
                    .ok_or(RunStoreError::ScanLimitExceeded(
                        self.limits.max_scan_entries,
                    ))?;
                if scanned > self.limits.max_scan_entries {
                    return Err(RunStoreError::ScanLimitExceeded(
                        self.limits.max_scan_entries,
                    ));
                }
                let name =
                    std::str::from_utf8(&raw_name).map_err(|_| RunStoreError::InvalidEntry {
                        name: "<non-UTF-8>".to_owned(),
                        reason: "directory entry names must be UTF-8",
                    })?;
                if matches!(name, "." | ".." | LOCK_NAME) || is_internal_temp_name(name) {
                    continue;
                }
                validate_entry_name(name)?;
                let (_, _, bytes) = self.open_data_name(name, 1)?;
                entries.push(RunStoreEntry {
                    name: name.to_owned(),
                    bytes,
                });
                if entries.len() > self.limits.max_list_entries {
                    return Err(RunStoreError::ListLimitExceeded(
                        self.limits.max_list_entries,
                    ));
                }
            }
            entries.sort_unstable();
            self.validate_guard()?;
            Ok(entries)
        }

        pub fn install_immutable(
            &self,
            name: &str,
            bytes: &[u8],
        ) -> Result<RunStoreEntry, RunStoreError> {
            validate_entry_name(name)?;
            self.validate_guard()?;
            let byte_count =
                u64::try_from(bytes.len()).map_err(|_| RunStoreError::EntryTooLarge {
                    name: name.to_owned(),
                    bytes: u64::MAX,
                    limit: self.limits.max_entry_bytes,
                })?;
            if byte_count > self.limits.max_entry_bytes {
                return Err(RunStoreError::EntryTooLarge {
                    name: name.to_owned(),
                    bytes: byte_count,
                    limit: self.limits.max_entry_bytes,
                });
            }
            if self.name_exists(name)? {
                return self.accept_existing(name, bytes);
            }

            let (temporary_name, mut temporary_file, temporary_identity) =
                self.create_temporary()?;
            let prepared = (|| {
                temporary_file
                    .write_all(bytes)
                    .map_err(|error| io_error("write private temporary entry", error))?;
                temporary_file
                    .sync_all()
                    .map_err(|error| io_error("sync private temporary entry", error))?;
                temporary_file
                    .seek(SeekFrom::Start(0))
                    .map_err(|error| io_error("rewind private temporary entry", error))?;
                let readback = self.read_bounded(&temporary_name, &mut temporary_file)?;
                if readback != bytes {
                    return Err(RunStoreError::ImmutableConflict(name.to_owned()));
                }
                self.revalidate_data_name(&temporary_name, temporary_identity, 1)
            })();
            if let Err(primary) = prepared {
                drop(temporary_file);
                return self.fail_after_private_cleanup(
                    &temporary_name,
                    temporary_identity,
                    primary,
                );
            }

            let link_result = self.link_names(&temporary_name, name);
            if let Err(primary) = link_result {
                drop(temporary_file);
                if is_already_exists(&primary) {
                    self.cleanup_private_temporary(&temporary_name, temporary_identity)?;
                    return self.accept_existing(name, bytes);
                }
                return self.fail_after_private_cleanup(
                    &temporary_name,
                    temporary_identity,
                    primary,
                );
            }

            // From here onward the final name may be reachable.  No failing path
            // reports success or silently attempts recovery.
            self.directory_file
                .sync_all()
                .map_err(|error| io_error("sync directory after immutable link", error))?;
            self.verify_linked_pair(&temporary_name, name, temporary_identity)?;
            self.unlink_name(&temporary_name, "unlink installed private temporary entry")?;
            drop(temporary_file);
            self.directory_file
                .sync_all()
                .map_err(|error| io_error("sync directory after temporary unlink", error))?;

            let entry = self.accept_existing(name, bytes)?;
            self.validate_guard()?;
            Ok(entry)
        }

        fn validate_guard(&self) -> Result<(), RunStoreError> {
            let directory = self
                .directory_file
                .metadata()
                .map_err(|error| io_error("inspect held run-store directory", error))?;
            verify_directory_metadata(&directory, self.owner_uid)?;
            if FileIdentity::of(&directory) != self.directory_identity {
                return Err(RunStoreError::InvalidDirectory(
                    "the held directory descriptor changed identity",
                ));
            }

            let held_lock = self
                .lock_file
                .metadata()
                .map_err(|error| io_error("inspect held run-store lock", error))?;
            verify_lock_metadata(&held_lock, self.owner_uid)?;
            if FileIdentity::of(&held_lock) != self.lock_identity {
                return Err(RunStoreError::InvalidDirectory(
                    "the held lock descriptor changed identity",
                ));
            }
            let lock_snapshot = snapshot_name(self.directory_file.as_raw_fd(), LOCK_NAME)?.ok_or(
                RunStoreError::InvalidDirectory(
                    "the lock name disappeared while the guard was live",
                ),
            )?;
            if !lock_snapshot.is_regular() {
                return Err(RunStoreError::InvalidDirectory(
                    "the lock name is not a regular file",
                ));
            }
            let reopened = openat_file(
                self.directory_file.as_raw_fd(),
                LOCK_NAME,
                O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
                0,
                "reopen run-store lock relative to held directory",
            )?;
            let reopened_metadata = reopened
                .metadata()
                .map_err(|error| io_error("inspect reopened run-store lock", error))?;
            verify_lock_metadata(&reopened_metadata, self.owner_uid)?;
            if FileIdentity::of(&reopened_metadata) != lock_snapshot.identity
                || FileIdentity::of(&reopened_metadata) != self.lock_identity
            {
                return Err(RunStoreError::InvalidDirectory(
                    "the lock name no longer identifies the held lock inode",
                ));
            }
            Ok(())
        }

        fn name_exists(&self, name: &str) -> Result<bool, RunStoreError> {
            probe_name(self.directory_file.as_raw_fd(), name)
        }

        fn open_data_name(
            &self,
            name: &str,
            expected_links: u64,
        ) -> Result<(File, FileIdentity, u64), RunStoreError> {
            let snapshot = snapshot_name(self.directory_file.as_raw_fd(), name)?
                .ok_or_else(|| RunStoreError::EntryNotFound(name.to_owned()))?;
            if !snapshot.is_regular() {
                return Err(RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "fstatat rejected a non-regular entry before open",
                });
            }
            let file = openat_file(
                self.directory_file.as_raw_fd(),
                name,
                O_RDONLY | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
                0,
                "open run-store entry relative to held directory",
            )
            .map_err(|error| normalize_entry_open_error(name, error))?;
            let metadata = file
                .metadata()
                .map_err(|error| io_error("inspect opened run-store entry", error))?;
            verify_data_metadata(name, &metadata, self.owner_uid, expected_links)?;
            if FileIdentity::of(&metadata) != snapshot.identity {
                return Err(RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "entry changed between fstatat and post-open fstat",
                });
            }
            if metadata.len() > self.limits.max_entry_bytes {
                return Err(RunStoreError::EntryTooLarge {
                    name: name.to_owned(),
                    bytes: metadata.len(),
                    limit: self.limits.max_entry_bytes,
                });
            }
            let identity = FileIdentity::of(&metadata);
            self.revalidate_data_name(name, identity, expected_links)?;
            Ok((file, identity, metadata.len()))
        }

        fn revalidate_data_name(
            &self,
            name: &str,
            expected_identity: FileIdentity,
            expected_links: u64,
        ) -> Result<(), RunStoreError> {
            let snapshot =
                snapshot_name(self.directory_file.as_raw_fd(), name)?.ok_or_else(|| {
                    RunStoreError::InvalidEntry {
                        name: name.to_owned(),
                        reason: "the entry name disappeared during validation",
                    }
                })?;
            if !snapshot.is_regular() {
                return Err(RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "fstatat rejected a non-regular entry before re-open",
                });
            }
            let reopened = openat_file(
                self.directory_file.as_raw_fd(),
                name,
                O_RDONLY | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
                0,
                "revalidate run-store entry relative to held directory",
            )
            .map_err(|error| normalize_entry_open_error(name, error))?;
            let metadata = reopened
                .metadata()
                .map_err(|error| io_error("inspect revalidated run-store entry", error))?;
            verify_data_metadata(name, &metadata, self.owner_uid, expected_links)?;
            if FileIdentity::of(&metadata) != snapshot.identity
                || FileIdentity::of(&metadata) != expected_identity
            {
                return Err(RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "the entry name changed inode during validation",
                });
            }
            Ok(())
        }

        fn read_bounded(&self, name: &str, file: &mut File) -> Result<Vec<u8>, RunStoreError> {
            let limit = self.limits.max_entry_bytes;
            let mut bytes = Vec::new();
            file.take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| io_error("read bounded run-store entry", error))?;
            let byte_count = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if byte_count > limit {
                return Err(RunStoreError::EntryTooLarge {
                    name: name.to_owned(),
                    bytes: byte_count,
                    limit,
                });
            }
            Ok(bytes)
        }

        fn accept_existing(
            &self,
            name: &str,
            expected: &[u8],
        ) -> Result<RunStoreEntry, RunStoreError> {
            self.validate_guard()?;
            let (mut file, identity, _) = self.open_data_name(name, 1)?;
            let actual = self.read_bounded(name, &mut file)?;
            if actual != expected {
                return Err(RunStoreError::ImmutableConflict(name.to_owned()));
            }
            file.sync_all()
                .map_err(|error| io_error("sync accepted immutable entry", error))?;
            self.revalidate_data_name(name, identity, 1)?;
            self.directory_file
                .sync_all()
                .map_err(|error| io_error("sync directory for accepted immutable entry", error))?;
            self.validate_guard()?;
            Ok(RunStoreEntry {
                name: name.to_owned(),
                bytes: u64::try_from(expected.len()).unwrap_or(u64::MAX),
            })
        }

        fn create_temporary(&self) -> Result<(String, File, FileIdentity), RunStoreError> {
            static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);
            for _ in 0..128 {
                let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
                let name = format!("{TEMP_PREFIX}{:x}-{sequence:x}", std::process::id());
                let file = match openat_file(
                    self.directory_file.as_raw_fd(),
                    &name,
                    O_RDWR | O_CREAT | O_EXCL | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
                    0o600,
                    "create private temporary entry relative to held directory",
                ) {
                    Ok(file) => file,
                    Err(error) if is_already_exists(&error) => continue,
                    Err(error) => return Err(error),
                };
                let prepared = (|| {
                    file.set_permissions(Permissions::from_mode(0o600))
                        .map_err(|error| io_error("set private temporary entry mode", error))?;
                    let metadata = file
                        .metadata()
                        .map_err(|error| io_error("inspect private temporary entry", error))?;
                    verify_data_metadata(&name, &metadata, self.owner_uid, 1)?;
                    let identity = FileIdentity::of(&metadata);
                    self.revalidate_data_name(&name, identity, 1)?;
                    Ok(identity)
                })();
                match prepared {
                    Ok(identity) => return Ok((name, file, identity)),
                    Err(primary) => {
                        let identity = file
                            .metadata()
                            .ok()
                            .map(|metadata| FileIdentity::of(&metadata));
                        drop(file);
                        let Some(identity) = identity else {
                            return Err(primary);
                        };
                        return self.fail_after_private_cleanup(&name, identity, primary);
                    }
                }
            }
            Err(RunStoreError::TemporaryNamespaceExhausted)
        }

        fn cleanup_private_temporary(
            &self,
            name: &str,
            expected_identity: FileIdentity,
        ) -> Result<(), RunStoreError> {
            if !is_internal_temp_name(name) {
                return Err(RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "refused to unlink a name outside the private temporary namespace",
                });
            }
            self.revalidate_data_name(name, expected_identity, 1)?;
            self.unlink_name(name, "unlink private temporary entry")?;
            self.directory_file
                .sync_all()
                .map_err(|error| io_error("sync directory after private temporary cleanup", error))
        }

        fn fail_after_private_cleanup<T>(
            &self,
            name: &str,
            identity: FileIdentity,
            primary: RunStoreError,
        ) -> Result<T, RunStoreError> {
            match self.cleanup_private_temporary(name, identity) {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(RunStoreError::CleanupFailed {
                    primary: Box::new(primary),
                    cleanup: Box::new(cleanup),
                }),
            }
        }

        fn link_names(&self, old_name: &str, new_name: &str) -> Result<(), RunStoreError> {
            let old_name = c_name(old_name)?;
            let new_name = c_name(new_name)?;
            let descriptor = self.directory_file.as_raw_fd();
            let result = unsafe {
                c_linkat(
                    descriptor,
                    old_name.as_ptr(),
                    descriptor,
                    new_name.as_ptr(),
                    0,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(io_error(
                    "atomically link immutable entry relative to held directory",
                    io::Error::last_os_error(),
                ))
            }
        }

        fn verify_linked_pair(
            &self,
            temporary_name: &str,
            final_name: &str,
            expected_identity: FileIdentity,
        ) -> Result<(), RunStoreError> {
            let (_, temporary_identity, _) = self.open_data_name(temporary_name, 2)?;
            let (_, final_identity, _) = self.open_data_name(final_name, 2)?;
            if temporary_identity != expected_identity || final_identity != expected_identity {
                return Err(RunStoreError::InvalidEntry {
                    name: final_name.to_owned(),
                    reason: "hard-link installation did not preserve the temporary inode",
                });
            }
            Ok(())
        }

        fn unlink_name(&self, name: &str, operation: &'static str) -> Result<(), RunStoreError> {
            let name = c_name(name)?;
            if unsafe { c_unlinkat(self.directory_file.as_raw_fd(), name.as_ptr(), 0) } == 0 {
                Ok(())
            } else {
                Err(io_error(operation, io::Error::last_os_error()))
            }
        }
    }

    fn verify_directory_metadata(metadata: &Metadata, owner_uid: u32) -> Result<(), RunStoreError> {
        if !metadata.is_dir() {
            return Err(RunStoreError::InvalidDirectory(
                "the target is not a directory",
            ));
        }
        if metadata.uid() != owner_uid {
            return Err(RunStoreError::InvalidDirectory(
                "the directory is not owned by the effective user",
            ));
        }
        if metadata.mode() & 0o7077 != 0 {
            return Err(RunStoreError::InvalidDirectory(
                "special, group, or other bits are set; mode must be 0700 or stricter",
            ));
        }
        Ok(())
    }

    fn verify_lock_metadata(metadata: &Metadata, owner_uid: u32) -> Result<(), RunStoreError> {
        if !metadata.is_file()
            || metadata.uid() != owner_uid
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || metadata.len() != 0
        {
            return Err(RunStoreError::InvalidDirectory(
                "the lock must be an empty owner-owned regular 0600 file with one link",
            ));
        }
        Ok(())
    }

    fn verify_data_metadata(
        name: &str,
        metadata: &Metadata,
        owner_uid: u32,
        expected_links: u64,
    ) -> Result<(), RunStoreError> {
        if !metadata.is_file() {
            return Err(RunStoreError::InvalidEntry {
                name: name.to_owned(),
                reason: "entry is not a regular file",
            });
        }
        if metadata.uid() != owner_uid {
            return Err(RunStoreError::InvalidEntry {
                name: name.to_owned(),
                reason: "entry is not owned by the effective user",
            });
        }
        if metadata.mode() & 0o7777 != 0o600 {
            return Err(RunStoreError::InvalidEntry {
                name: name.to_owned(),
                reason: "entry mode is not exactly 0600",
            });
        }
        if metadata.nlink() != expected_links {
            return Err(RunStoreError::InvalidEntry {
                name: name.to_owned(),
                reason: "entry link count does not match the installation phase",
            });
        }
        Ok(())
    }

    fn acquire_lock(
        directory: &File,
        owner_uid: u32,
    ) -> Result<(File, FileIdentity), RunStoreError> {
        let descriptor = directory.as_raw_fd();
        let mut created = false;
        let lock = match openat_file(
            descriptor,
            LOCK_NAME,
            O_RDWR | O_CREAT | O_EXCL | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
            0o600,
            "create run-store lock relative to held directory",
        ) {
            Ok(file) => {
                created = true;
                file
            }
            Err(error) if is_already_exists(&error) => {
                let snapshot = snapshot_name(descriptor, LOCK_NAME)?.ok_or(
                    RunStoreError::InvalidDirectory(
                        "the existing lock disappeared before it could be opened",
                    ),
                )?;
                if !snapshot.is_regular() {
                    return Err(RunStoreError::InvalidDirectory(
                        "fstatat rejected a non-regular lock before open",
                    ));
                }
                let file = openat_file(
                    descriptor,
                    LOCK_NAME,
                    O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
                    0,
                    "open run-store lock relative to held directory",
                )?;
                let metadata = file
                    .metadata()
                    .map_err(|error| io_error("post-open fstat of run-store lock", error))?;
                if FileIdentity::of(&metadata) != snapshot.identity {
                    return Err(RunStoreError::InvalidDirectory(
                        "the lock changed between fstatat and post-open fstat",
                    ));
                }
                file
            }
            Err(error) => return Err(error),
        };
        if created {
            lock.set_permissions(Permissions::from_mode(0o600))
                .map_err(|error| io_error("set run-store lock mode", error))?;
            lock.sync_all()
                .map_err(|error| io_error("sync new run-store lock", error))?;
            directory
                .sync_all()
                .map_err(|error| io_error("sync directory for new run-store lock", error))?;
        }
        let metadata = lock
            .metadata()
            .map_err(|error| io_error("inspect run-store lock", error))?;
        verify_lock_metadata(&metadata, owner_uid)?;
        let identity = FileIdentity::of(&metadata);
        if unsafe { c_flock(lock.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                return Err(RunStoreError::LockBusy);
            }
            return Err(io_error(
                "acquire exclusive nonblocking run-store lock",
                error,
            ));
        }

        let snapshot = snapshot_name(descriptor, LOCK_NAME)?.ok_or(
            RunStoreError::InvalidDirectory("the locked name disappeared after flock"),
        )?;
        if !snapshot.is_regular() {
            return Err(RunStoreError::InvalidDirectory(
                "fstatat rejected a non-regular locked name",
            ));
        }
        let reopened = openat_file(
            descriptor,
            LOCK_NAME,
            O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC,
            0,
            "correlate run-store lock with held directory",
        )?;
        let reopened_metadata = reopened
            .metadata()
            .map_err(|error| io_error("inspect correlated run-store lock", error))?;
        verify_lock_metadata(&reopened_metadata, owner_uid)?;
        if FileIdentity::of(&reopened_metadata) != snapshot.identity
            || FileIdentity::of(&reopened_metadata) != identity
        {
            return Err(RunStoreError::InvalidDirectory(
                "the locked inode is not the lock named by the held directory",
            ));
        }
        Ok((lock, identity))
    }

    fn c_name(name: &str) -> Result<CString, RunStoreError> {
        CString::new(name.as_bytes()).map_err(|_| RunStoreError::InvalidEntryName(name.to_owned()))
    }

    fn file_from_descriptor(
        descriptor: RawFd,
        operation: &'static str,
    ) -> Result<File, RunStoreError> {
        if descriptor < 0 {
            Err(io_error(operation, io::Error::last_os_error()))
        } else {
            Ok(unsafe { File::from_raw_fd(descriptor) })
        }
    }

    fn openat_file(
        directory: RawFd,
        name: &str,
        flags: c_int,
        mode: c_uint,
        operation: &'static str,
    ) -> Result<File, RunStoreError> {
        let name = c_name(name)?;
        let descriptor = unsafe { c_openat(directory, name.as_ptr(), flags, mode) };
        file_from_descriptor(descriptor, operation)
    }

    fn snapshot_name(directory: RawFd, name: &str) -> Result<Option<NameSnapshot>, RunStoreError> {
        let name = c_name(name)?;
        let mut status = MaybeUninit::<PlatformStat>::uninit();
        let result = unsafe {
            c_fstatat(
                directory,
                name.as_ptr(),
                status.as_mut_ptr(),
                AT_SYMLINK_NOFOLLOW,
            )
        };
        if result == 0 {
            let status = unsafe { status.assume_init() };
            Ok(Some(NameSnapshot {
                identity: status.identity(),
                mode: status.mode(),
            }))
        } else {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(io_error(
                    "inspect name relative to held run-store directory",
                    error,
                ))
            }
        }
    }

    fn probe_name(directory: RawFd, name: &str) -> Result<bool, RunStoreError> {
        Ok(snapshot_name(directory, name)?.is_some())
    }

    fn is_already_exists(error: &RunStoreError) -> bool {
        matches!(
            error,
            RunStoreError::Io { source, .. }
                if source.kind() == io::ErrorKind::AlreadyExists
        )
    }

    fn normalize_entry_open_error(name: &str, error: RunStoreError) -> RunStoreError {
        match &error {
            RunStoreError::Io { source, .. }
                if matches!(
                    source.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                ) =>
            {
                RunStoreError::InvalidEntry {
                    name: name.to_owned(),
                    reason: "entry changed or was a symbolic link",
                }
            }
            _ => error,
        }
    }

    struct DirectoryReader {
        stream: *mut DirectoryStream,
    }

    impl DirectoryReader {
        fn open(directory: &File, expected: FileIdentity) -> Result<Self, RunStoreError> {
            let descriptor = openat_file(
                directory.as_raw_fd(),
                ".",
                O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC,
                0,
                "open directory stream relative to held directory",
            )?;
            let metadata = descriptor
                .metadata()
                .map_err(|error| io_error("inspect directory stream descriptor", error))?;
            if FileIdentity::of(&metadata) != expected {
                return Err(RunStoreError::InvalidDirectory(
                    "directory stream is not anchored to the held directory inode",
                ));
            }
            let raw_descriptor = descriptor.as_raw_fd();
            std::mem::forget(descriptor);
            let stream = unsafe { c_fdopendir(raw_descriptor) };
            if stream.is_null() {
                let error = io::Error::last_os_error();
                unsafe {
                    c_close(raw_descriptor);
                }
                return Err(io_error("create FD-anchored directory stream", error));
            }
            Ok(Self { stream })
        }

        fn next_name(&mut self) -> Result<Option<Vec<u8>>, RunStoreError> {
            unsafe {
                *errno_location() = 0;
                let entry = c_readdir(self.stream);
                if entry.is_null() {
                    let errno = *errno_location();
                    return if errno == 0 {
                        Ok(None)
                    } else {
                        Err(io_error(
                            "read FD-anchored run-store directory",
                            io::Error::from_raw_os_error(errno),
                        ))
                    };
                }
                Ok(Some(
                    CStr::from_ptr((*entry).d_name.as_ptr()).to_bytes().to_vec(),
                ))
            }
        }
    }

    impl Drop for DirectoryReader {
        fn drop(&mut self) {
            unsafe {
                c_closedir(self.stream);
            }
        }
    }
}

#[cfg(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
pub use supported_unix::RunStoreGuard;

#[cfg(not(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub struct RunStoreGuard {
    _private: (),
}

#[cfg(not(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
impl RunStoreGuard {
    pub fn open_or_create(
        _directory: impl AsRef<Path>,
        _limits: RunStoreLimits,
    ) -> Result<Self, RunStoreError> {
        Err(RunStoreError::Unsupported)
    }

    pub fn open(
        _directory: impl AsRef<Path>,
        _limits: RunStoreLimits,
    ) -> Result<Self, RunStoreError> {
        Err(RunStoreError::Unsupported)
    }

    pub fn acquire(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
    ) -> Result<Self, RunStoreError> {
        Self::open(directory, limits)
    }

    pub fn limits(&self) -> RunStoreLimits {
        RunStoreLimits::default()
    }

    pub fn read_entry(&self, _name: &str) -> Result<Vec<u8>, RunStoreError> {
        Err(RunStoreError::Unsupported)
    }

    pub fn list_entries(&self) -> Result<Vec<RunStoreEntry>, RunStoreError> {
        Err(RunStoreError::Unsupported)
    }

    pub fn install_immutable(
        &self,
        _name: &str,
        _bytes: &[u8],
    ) -> Result<RunStoreEntry, RunStoreError> {
        Err(RunStoreError::Unsupported)
    }
}
