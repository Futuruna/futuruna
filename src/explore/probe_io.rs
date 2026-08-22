//! Strict private storage for exported/imported probe transcripts.
//!
//! The target is never opened for writing. A complete canonical artifact is
//! written to a private, unique sibling and atomically renamed over the target
//! while an invocation-wide sibling lock is held. The implementation assumes
//! the caller selected a trusted parent directory whose ancestry is not
//! mutated by a noncooperating actor during the invocation. It detects a
//! changed final parent entry and rejects symlink or non-regular targets; it
//! does not claim `openat(2)`-style hostile-directory confinement through the
//! standard library's pathname APIs. This module exposes neither paths nor
//! artifact contents through public Explore evidence. The unified observable
//! coordinator must use the run-store journal as its primary durability
//! boundary; this single-file store must not recreate a separate probe-only
//! lifecycle or forced two-invocation gate.

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

use super::probe::{CorruptProbeArtifact, ProbeArtifact};
use super::probe_codec::{decode_probe_artifact_v2, encode_probe_artifact_v2, ProbeCodecError};

const LOCK_SUFFIX: &str = ".probe-lock";
const TEMP_SUFFIX: &str = ".probe-tmp";
const PRIVATE_FILE_MODE: u32 = 0o600;
const TEMP_CREATE_ATTEMPTS: usize = 1_024;

static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) enum ProbeStoreError {
    InvalidTarget,
    UnsupportedPlatformSemantics(&'static str),
    LockContended,
    TemporaryNamespaceExhausted,
    Codec(ProbeCodecError),
    Io {
        operation: &'static str,
        source: io::Error,
    },
    ParentDirectoryDurabilityUnavailable {
        source: io::Error,
    },
    /// Rename succeeded, so callers must not assume the prior target remains.
    CommitDurabilityUncertain {
        source: io::Error,
    },
    #[cfg(test)]
    InjectedPreRenameFailure,
}

impl fmt::Display for ProbeStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget => formatter.write_str(
                "probe artifact target must name one file inside an existing directory",
            ),
            Self::UnsupportedPlatformSemantics(detail) => {
                write!(formatter, "unsupported probe artifact storage semantics: {detail}")
            }
            Self::LockContended => formatter.write_str(
                "another invocation holds the staged-probe artifact lock",
            ),
            Self::TemporaryNamespaceExhausted => formatter.write_str(
                "could not allocate a unique staged-probe checkpoint sibling",
            ),
            Self::Codec(error) => write!(formatter, "probe artifact codec rejected checkpoint: {error}"),
            Self::Io { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            Self::ParentDirectoryDurabilityUnavailable { source } => write!(
                formatter,
                "probe parent-directory durability is unavailable: {source}"
            ),
            Self::CommitDurabilityUncertain { source } => write!(
                formatter,
                "probe artifact target was atomically replaced, but parent-directory sync failed: {source}"
            ),
            #[cfg(test)]
            Self::InjectedPreRenameFailure => {
                formatter.write_str("injected probe checkpoint failure before rename")
            }
        }
    }
}

impl Error for ProbeStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Io { source, .. }
            | Self::ParentDirectoryDurabilityUnavailable { source }
            | Self::CommitDurabilityUncertain { source } => Some(source),
            Self::InvalidTarget
            | Self::UnsupportedPlatformSemantics(_)
            | Self::LockContended
            | Self::TemporaryNamespaceExhausted => None,
            #[cfg(test)]
            Self::InjectedPreRenameFailure => None,
        }
    }
}

impl From<ProbeCodecError> for ProbeStoreError {
    fn from(error: ProbeCodecError) -> Self {
        Self::Codec(error)
    }
}

fn io_error(operation: &'static str, source: io::Error) -> ProbeStoreError {
    ProbeStoreError::Io { operation, source }
}

/// Result of reading the exact target while its invocation lock is held.
/// Only `io::ErrorKind::NotFound` denotes absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoredProbeArtifact {
    Missing,
    Canonical(ProbeArtifact),
    Corrupt(CorruptProbeArtifact),
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn for_file(file: &File) -> Result<Self, ProbeStoreError> {
        file.metadata()
            .map(|metadata| Self::from_metadata(&metadata))
            .map_err(|error| io_error("probe private-file identity check", error))
    }

    fn path_matches(self, path: &Path) -> bool {
        fs::symlink_metadata(path)
            .map(|metadata| Self::from_metadata(&metadata) == self)
            .unwrap_or(false)
    }
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity;

struct HeldProbeLock {
    path: PathBuf,
    _file: File,
    identity: FileIdentity,
    parent_directory: File,
}

impl HeldProbeLock {
    fn sync_parent(&self) -> io::Result<()> {
        self.parent_directory.sync_all()
    }

    #[cfg(unix)]
    fn verify_owned(&self) -> Result<(), ProbeStoreError> {
        if self.identity.path_matches(&self.path) {
            Ok(())
        } else {
            Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "invocation lock identity changed while the guard was active",
            ))
        }
    }
}

impl Drop for HeldProbeLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // Cooperative writers cannot replace this path while the held
            // create-new lock exists. If its identity changed, leave the path
            // untouched. Hostile directory mutation is outside this store's
            // explicitly documented pathname threat model.
            if self.identity.path_matches(&self.path) {
                let _ = fs::remove_file(&self.path);
                let _ = self.parent_directory.sync_all();
            }
        }
    }
}

struct ExactTemporaryFile {
    path: PathBuf,
    file: Option<File>,
    identity: FileIdentity,
    renamed: bool,
}

impl ExactTemporaryFile {
    fn file_mut(&mut self) -> Result<&mut File, ProbeStoreError> {
        self.file.as_mut().ok_or_else(|| {
            ProbeStoreError::UnsupportedPlatformSemantics(
                "checkpoint temporary file was already closed",
            )
        })
    }

    fn close(&mut self) {
        drop(self.file.take());
    }

    #[cfg(unix)]
    fn verify_exact_path(&self) -> Result<(), ProbeStoreError> {
        if self.identity.path_matches(&self.path) {
            Ok(())
        } else {
            Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "checkpoint sibling identity changed before atomic rename",
            ))
        }
    }
}

impl Drop for ExactTemporaryFile {
    fn drop(&mut self) {
        self.close();
        #[cfg(unix)]
        {
            if !self.renamed && self.identity.path_matches(&self.path) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// Invocation-wide checkpoint guard for cooperative writers in one stable,
/// trusted parent directory.
pub(crate) struct ProbeArtifactStore {
    target: PathBuf,
    parent: PathBuf,
    parent_identity: FileIdentity,
    lock: HeldProbeLock,
}

impl ProbeArtifactStore {
    pub(crate) fn acquire(target: impl AsRef<Path>) -> Result<Self, ProbeStoreError> {
        #[cfg(not(unix))]
        {
            let _ = target;
            return Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "atomic replacement, invocation locks, and exact mode 0600 require Unix",
            ));
        }

        #[cfg(unix)]
        {
            Self::acquire_unix(target.as_ref())
        }
    }

    #[cfg(unix)]
    fn acquire_unix(target: &Path) -> Result<Self, ProbeStoreError> {
        let file_name = target.file_name().ok_or(ProbeStoreError::InvalidTarget)?;
        if file_name.is_empty() {
            return Err(ProbeStoreError::InvalidTarget);
        }
        let requested_parent = normalized_parent(target)?;
        let requested_parent_metadata = fs::symlink_metadata(&requested_parent)
            .map_err(|error| io_error("probe parent-directory inspection", error))?;
        if requested_parent_metadata.file_type().is_symlink() || !requested_parent_metadata.is_dir()
        {
            return Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "probe artifact parent must be a stable, non-symlink directory",
            ));
        }
        let parent = fs::canonicalize(&requested_parent)
            .map_err(|error| io_error("probe parent-directory canonicalization", error))?;
        let parent_directory =
            File::open(&parent).map_err(|error| io_error("probe parent-directory open", error))?;
        let parent_metadata = parent_directory
            .metadata()
            .map_err(|error| io_error("probe parent-directory metadata", error))?;
        if !parent_metadata.is_dir() {
            return Err(ProbeStoreError::InvalidTarget);
        }
        let parent_identity = FileIdentity::from_metadata(&parent_metadata);
        verify_directory_identity(&parent, parent_identity)?;

        let lock_path = sibling_path(&parent, file_name, LOCK_SUFFIX);
        let lock_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(PRIVATE_FILE_MODE)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(ProbeStoreError::LockContended)
            }
            Err(error) => return Err(io_error("probe invocation-lock acquisition", error)),
        };
        let lock_identity = match FileIdentity::for_file(&lock_file) {
            Ok(identity) => identity,
            Err(error) => {
                // The pathname cannot be proven to still name this open file.
                // Leak the entry rather than risk unlinking another actor's
                // replacement; manual recovery is safer than false cleanup.
                drop(lock_file);
                return Err(error);
            }
        };
        let held = HeldProbeLock {
            path: lock_path,
            _file: lock_file,
            identity: lock_identity,
            parent_directory,
        };
        force_private_mode(&held._file)
            .map_err(|error| io_error("probe invocation-lock permission hardening", error))?;
        held._file
            .sync_all()
            .map_err(|error| io_error("probe invocation-lock sync", error))?;
        verify_directory_identity(&parent, parent_identity)?;
        held.verify_owned()?;

        // This is also a capability preflight: platforms/filesystems that
        // cannot durably sync the parent fail before a target replacement.
        held.sync_parent()
            .map_err(|source| ProbeStoreError::ParentDirectoryDurabilityUnavailable { source })?;

        Ok(Self {
            target: parent.join(file_name),
            parent,
            parent_identity,
            lock: held,
        })
    }

    pub(crate) fn read(&self) -> Result<StoredProbeArtifact, ProbeStoreError> {
        #[cfg(unix)]
        self.verify_guard()?;
        let target_metadata = match fs::symlink_metadata(&self.target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(StoredProbeArtifact::Missing)
            }
            Err(error) => {
                return Ok(corrupt(format!(
                    "probe artifact target could not be read ({:?}): {error}",
                    error.kind()
                )))
            }
        };
        if !target_metadata.file_type().is_file() {
            return Ok(corrupt(
                "probe artifact target must be a regular, non-symlink file".to_string(),
            ));
        }
        let expected_identity = FileIdentity::from_metadata(&target_metadata);
        let mut target_file = match OpenOptions::new().read(true).open(&self.target) {
            Ok(file) => file,
            Err(error) => {
                return Ok(corrupt(format!(
                    "probe artifact target changed or could not be opened ({:?}): {error}",
                    error.kind()
                )))
            }
        };
        let opened_metadata = target_file
            .metadata()
            .map_err(|error| io_error("probe artifact opened-file metadata", error))?;
        if !opened_metadata.file_type().is_file()
            || FileIdentity::from_metadata(&opened_metadata) != expected_identity
        {
            return Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "probe artifact target changed while it was opened",
            ));
        }
        let mut bytes = Vec::new();
        target_file
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("probe artifact read", error))?;
        if let Err(error) = std::str::from_utf8(&bytes) {
            return Ok(corrupt(format!(
                "probe artifact target is not UTF-8 at byte {}",
                error.valid_up_to()
            )));
        }
        Ok(match decode_probe_artifact_v2(&bytes) {
            Ok(artifact) => StoredProbeArtifact::Canonical(artifact),
            Err(error) => corrupt(format!(
                "probe artifact target is corrupt or noncanonical: {error}"
            )),
        })
    }

    pub(crate) fn commit(&self, artifact: &ProbeArtifact) -> Result<(), ProbeStoreError> {
        self.commit_inner(artifact, CommitFault::None)
    }

    fn commit_inner(
        &self,
        artifact: &ProbeArtifact,
        fault: CommitFault,
    ) -> Result<(), ProbeStoreError> {
        #[cfg(not(unix))]
        {
            let _ = (artifact, fault);
            return Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "atomic replacement and exact mode 0600 require Unix",
            ));
        }

        #[cfg(unix)]
        {
            self.verify_guard()?;
            // Validate and encode before a temporary name exists.
            let bytes = encode_probe_artifact_v2(artifact)?;
            self.verify_guard()?;
            self.verify_replaceable_target()?;
            let mut temporary = self.create_temporary()?;
            {
                let file = temporary.file_mut()?;
                file.write_all(&bytes)
                    .map_err(|error| io_error("probe checkpoint write", error))?;
                file.flush()
                    .map_err(|error| io_error("probe checkpoint flush", error))?;
                file.sync_all()
                    .map_err(|error| io_error("probe checkpoint file sync", error))?;
            }
            temporary.close();
            temporary.verify_exact_path()?;
            self.verify_guard()?;
            self.verify_replaceable_target()?;

            #[cfg(test)]
            if fault == CommitFault::BeforeRename {
                return Err(ProbeStoreError::InjectedPreRenameFailure);
            }
            #[cfg(not(test))]
            let _ = fault;

            fs::rename(&temporary.path, &self.target)
                .map_err(|error| io_error("probe checkpoint atomic rename", error))?;
            temporary.renamed = true;
            self.lock
                .sync_parent()
                .map_err(|source| ProbeStoreError::CommitDurabilityUncertain { source })?;
            Ok(())
        }
    }

    #[cfg(unix)]
    fn verify_guard(&self) -> Result<(), ProbeStoreError> {
        verify_directory_identity(&self.parent, self.parent_identity)?;
        self.lock.verify_owned()
    }

    #[cfg(unix)]
    fn verify_replaceable_target(&self) -> Result<(), ProbeStoreError> {
        match fs::symlink_metadata(&self.target) {
            Ok(metadata) if metadata.file_type().is_file() => Ok(()),
            Ok(_) => Err(ProbeStoreError::UnsupportedPlatformSemantics(
                "probe artifact target must be absent or a regular, non-symlink file",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("probe artifact target inspection", error)),
        }
    }

    #[cfg(unix)]
    fn create_temporary(&self) -> Result<ExactTemporaryFile, ProbeStoreError> {
        let file_name = self
            .target
            .file_name()
            .ok_or(ProbeStoreError::InvalidTarget)?;
        for _ in 0..TEMP_CREATE_ATTEMPTS {
            let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
            let suffix = format!("{TEMP_SUFFIX}.{}.{}", std::process::id(), nonce);
            let path = sibling_path(&self.parent, file_name, &suffix);
            let file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(PRIVATE_FILE_MODE)
                .open(&path)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error("probe checkpoint sibling creation", error)),
            };
            let identity = match FileIdentity::for_file(&file) {
                Ok(identity) => identity,
                Err(error) => {
                    // As with the lock, failure to obtain the open inode's
                    // identity makes pathname cleanup unsafe. Leave the entry
                    // for explicit recovery instead of guessing.
                    drop(file);
                    return Err(error);
                }
            };
            let temporary = ExactTemporaryFile {
                path,
                file: Some(file),
                identity,
                renamed: false,
            };
            let temporary_file = temporary.file.as_ref().ok_or_else(|| {
                ProbeStoreError::UnsupportedPlatformSemantics(
                    "new checkpoint sibling lost its open file",
                )
            })?;
            force_private_mode(temporary_file)
                .map_err(|error| io_error("probe checkpoint permission hardening", error))?;
            return Ok(temporary);
        }
        Err(ProbeStoreError::TemporaryNamespaceExhausted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitFault {
    None,
    #[cfg(test)]
    BeforeRename,
}

fn corrupt(reason: String) -> StoredProbeArtifact {
    debug_assert!(!reason.is_empty());
    StoredProbeArtifact::Corrupt(CorruptProbeArtifact {
        reason: reason.into_boxed_str(),
    })
}

fn normalized_parent(target: &Path) -> Result<PathBuf, ProbeStoreError> {
    let parent = target.parent().ok_or(ProbeStoreError::InvalidTarget)?;
    if parent.as_os_str().is_empty() {
        Ok(PathBuf::from("."))
    } else {
        Ok(parent.to_path_buf())
    }
}

#[cfg(unix)]
fn verify_directory_identity(path: &Path, expected: FileIdentity) -> Result<(), ProbeStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("probe parent-directory identity check", error))?;
    if !metadata.file_type().is_symlink()
        && metadata.is_dir()
        && FileIdentity::from_metadata(&metadata) == expected
    {
        Ok(())
    } else {
        Err(ProbeStoreError::UnsupportedPlatformSemantics(
            "probe parent directory changed during the invocation",
        ))
    }
}

fn sibling_path(parent: &Path, file_name: &OsStr, suffix: &str) -> PathBuf {
    let mut sibling = OsString::from(".");
    sibling.push(file_name);
    sibling.push(suffix);
    parent.join(sibling)
}

#[cfg(unix)]
fn force_private_mode(file: &File) -> io::Result<()> {
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::num::NonZeroU128;
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::explore::probe::{
        ProbeArtifactState, ProbeCompletionReason, ProbeCounts, ProbeCursor, ProbeFrontierId,
        ProbeFrontierState, ProbePlanContract, ProbeSelector, ProbeSemanticIdentity,
        PROBE_ARTIFACT_SCHEMA_V2,
    };
    use crate::ExplorePolarity;

    static TEST_DIRECTORY_NONCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let nonce = TEST_DIRECTORY_NONCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "futuruna-probe-io-canary-{}-{nonce}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create exact test directory: {error}"),
                }
            }
        }

        fn target(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(seed: u8) -> Box<str> {
        format!("{seed:064x}").into_boxed_str()
    }

    fn fixture() -> ProbeArtifact {
        let contract = ProbePlanContract {
            artifact_schema: PROBE_ARTIFACT_SCHEMA_V2.into(),
            normalization_version: "probe-normalization-v2".into(),
            selector_tie_break_version: "probe-tie-break-v1".into(),
            query_name: "empty_probe".into(),
            identity: ProbeSemanticIdentity {
                program_hash: digest(1),
                analysis_program_hash: digest(2),
                query_hash: digest(3),
                domain_hash: digest(4),
                probe_plan_hash: digest(5),
                evaluator_contract_hash: digest(6),
            },
            polarity: ExplorePolarity::Matches,
            dimensions: Box::new([]),
            axis_cardinalities: Box::new([]),
            boundary: None,
            selectors: vec![ProbeSelector::FrontierMidpoints].into_boxed_slice(),
            semantic_case_cap: NonZeroU128::new(1).unwrap(),
            initial_frontier: ProbeFrontierId::new(digest(7)).unwrap(),
            lift_dimension_indices: Box::new([]),
            retained_configuration_dimension_indices: Box::new([]),
            retained_key_names: Box::new([]),
            retained_shown_names: Box::new([]),
            mechanism_trace_authorized: false,
        };
        ProbeArtifact {
            contract,
            state: ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            cursor: ProbeCursor {
                next_decision: 0,
                frontier: ProbeFrontierState::PlanExhausted,
            },
            counts: ProbeCounts {
                planned_distinct_cases: 0,
                observed_distinct_cases: 0,
                pending_distinct_cases: 0,
                remaining_case_budget: 1,
            },
            observations: Box::new([]),
            transcript: Box::new([]),
            lifted_candidates: Box::new([]),
        }
    }

    #[test]
    fn read_distinguishes_missing_canonical_and_corrupt_bytes() {
        let directory = TestDirectory::new();
        let target = directory.target("probe.json");
        let store = ProbeArtifactStore::acquire(&target).unwrap();
        assert_eq!(store.read().unwrap(), StoredProbeArtifact::Missing);

        let artifact = fixture();
        store.commit(&artifact).unwrap();
        assert_eq!(
            store.read().unwrap(),
            StoredProbeArtifact::Canonical(artifact.clone())
        );

        let mut noncanonical = encode_probe_artifact_v2(&artifact).unwrap();
        noncanonical.push(b'\n');
        fs::write(&target, noncanonical).unwrap();
        assert!(matches!(
            store.read().unwrap(),
            StoredProbeArtifact::Corrupt(_)
        ));

        fs::write(&target, [0xff, 0xfe]).unwrap();
        assert!(matches!(
            store.read().unwrap(),
            StoredProbeArtifact::Corrupt(_)
        ));
    }

    #[test]
    fn symlink_dangling_and_non_regular_targets_are_never_missing_or_followed() {
        let directory = TestDirectory::new();
        let canonical = directory.target("canonical.json");
        fs::write(&canonical, encode_probe_artifact_v2(&fixture()).unwrap()).unwrap();

        let linked = directory.target("linked.json");
        symlink(&canonical, &linked).unwrap();
        let linked_store = ProbeArtifactStore::acquire(&linked).unwrap();
        assert!(matches!(
            linked_store.read().unwrap(),
            StoredProbeArtifact::Corrupt(_)
        ));
        assert!(matches!(
            linked_store.commit(&fixture()),
            Err(ProbeStoreError::UnsupportedPlatformSemantics(_))
        ));
        drop(linked_store);

        let dangling = directory.target("dangling.json");
        symlink(directory.target("absent.json"), &dangling).unwrap();
        let dangling_store = ProbeArtifactStore::acquire(&dangling).unwrap();
        assert!(matches!(
            dangling_store.read().unwrap(),
            StoredProbeArtifact::Corrupt(_)
        ));
        drop(dangling_store);

        let directory_target = directory.target("directory.json");
        fs::create_dir(&directory_target).unwrap();
        let directory_store = ProbeArtifactStore::acquire(&directory_target).unwrap();
        assert!(matches!(
            directory_store.read().unwrap(),
            StoredProbeArtifact::Corrupt(_)
        ));
        assert!(matches!(
            directory_store.commit(&fixture()),
            Err(ProbeStoreError::UnsupportedPlatformSemantics(_))
        ));
    }

    #[test]
    fn symlink_parent_is_rejected_before_lock_acquisition() {
        let directory = TestDirectory::new();
        let real_parent = directory.target("real-parent");
        fs::create_dir(&real_parent).unwrap();
        let linked_parent = directory.target("linked-parent");
        symlink(&real_parent, &linked_parent).unwrap();
        assert!(matches!(
            ProbeArtifactStore::acquire(linked_parent.join("probe.json")),
            Err(ProbeStoreError::UnsupportedPlatformSemantics(_))
        ));
    }

    #[test]
    fn concurrent_lock_acquisition_fails_until_the_guard_drops() {
        let directory = TestDirectory::new();
        let target = directory.target("probe.json");
        let first = ProbeArtifactStore::acquire(&target).unwrap();
        assert!(matches!(
            ProbeArtifactStore::acquire(&target),
            Err(ProbeStoreError::LockContended)
        ));
        drop(first);
        ProbeArtifactStore::acquire(&target).unwrap();
    }

    #[test]
    fn canonical_commit_atomically_replaces_the_old_target() {
        let directory = TestDirectory::new();
        let target = directory.target("probe.json");
        fs::write(&target, b"old checkpoint").unwrap();
        let store = ProbeArtifactStore::acquire(&target).unwrap();
        let artifact = fixture();
        store.commit(&artifact).unwrap();
        assert_eq!(
            store.read().unwrap(),
            StoredProbeArtifact::Canonical(artifact)
        );
    }

    #[test]
    fn pre_rename_failure_preserves_old_target_and_removes_exact_temp() {
        let directory = TestDirectory::new();
        let target = directory.target("probe.json");
        let old = b"old checkpoint";
        fs::write(&target, old).unwrap();
        let store = ProbeArtifactStore::acquire(&target).unwrap();
        assert!(matches!(
            store.commit_inner(&fixture(), CommitFault::BeforeRename),
            Err(ProbeStoreError::InjectedPreRenameFailure)
        ));
        assert_eq!(fs::read(&target).unwrap(), old);
        let names = fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert!(!names
            .iter()
            .any(|name| name.to_string_lossy().contains(TEMP_SUFFIX)));
    }

    #[test]
    fn lock_and_committed_target_are_exactly_owner_private() {
        let directory = TestDirectory::new();
        let target = directory.target("probe.json");
        let store = ProbeArtifactStore::acquire(&target).unwrap();
        let lock_mode = fs::metadata(&store.lock.path).unwrap().permissions().mode() & 0o777;
        assert_eq!(lock_mode, PRIVATE_FILE_MODE);
        store.commit(&fixture()).unwrap();
        let target_mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(target_mode, PRIVATE_FILE_MODE);
    }
}
