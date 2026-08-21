//! Durable raw-byte storage for one observable exploration stream.
//!
//! This adapter deliberately knows nothing about event codecs, journal-head
//! derivation, evidence roots, or frontier semantics.  It owns the single
//! [`RunStoreGuard`] writer authority and maps already-validated bytes onto a
//! small immutable namespace:
//!
//! - `run-opened-v1` is the unique sequence-zero genesis record;
//! - `fence-v1-{generation:032x}-{receipt_hash}` durably advances writer
//!   authority from generation two onward (generation one is bound by the
//!   genesis record itself) before that writer may append events;
//! - `blob-v1-{kind}-{sha256}` stores content-addressed immutable bytes; and
//! - `event-v1-{sequence:032x}-{journal_head}` stores committed events from
//!   sequence one onward.
//!
//! There is intentionally no mutable `HEAD`.  A reopened adapter reconstructs
//! the tail by scanning immutable event names, rejects gaps and forks, and then
//! advances an in-memory cursor while it retains the exclusive run-store lock.

use super::run_store::{RunStoreError, RunStoreGuard, RunStoreLimits, RUN_STORE_MAX_NAME_BYTES};
use sha2::{Digest, Sha256};
use std::fmt;
use std::num::NonZeroU64;
use std::path::Path;

const RUN_OPENED_ENTRY: &str = "run-opened-v1";
const RUN_OPENED_FAMILY: &str = "run-opened";
const FENCE_PREFIX: &str = "fence-v1-";
const FENCE_FAMILY: &str = "fence-v1";
const INITIAL_FENCE_RECEIPT_DOMAIN_V1: &[u8] = b"futuruna.explore.initial-writer-fence-receipt.v1";
const FENCE_RECEIPT_DOMAIN_V1: &[u8] = b"futuruna.explore.writer-fence-receipt.v1";
const INITIAL_FENCE_PRIOR_RUN_OPENED_V1: &[u8] = b"run-opened-genesis";
const FENCE_PRIOR_GENESIS_ANCHOR_V1: &[u8] = b"genesis-anchor";
const FENCE_PRIOR_RECEIPT_V1: &[u8] = b"prior-receipt";
const BLOB_PREFIX: &str = "blob-v1-";
const BLOB_FAMILY: &str = "blob-v1";
const EVENT_PREFIX: &str = "event-v1-";
const EVENT_FAMILY: &str = "event-v1";
const SHA256_HEX_BYTES: usize = 64;
const FENCE_GENERATION_HEX_BYTES: usize = 32;
const EVENT_SEQUENCE_HEX_BYTES: usize = 32;
const INITIAL_FENCE_GENERATION: u64 = 1;
const FIRST_PERSISTED_FENCE_GENERATION: u64 = 2;
const FIRST_EVENT_SEQUENCE: u64 = 1;

#[derive(Debug)]
pub(crate) enum ExploreRunStreamStoreError {
    Store(RunStoreError),
    InvalidSha256 {
        field: &'static str,
        value: String,
    },
    Sha256Mismatch {
        name: String,
        expected: String,
        actual: String,
    },
    InvalidBlobKind {
        kind: String,
        reason: &'static str,
    },
    MalformedRecognizedName(String),
    MissingGenesis,
    EventSequenceGap {
        expected: u64,
        found: u64,
    },
    EventSequenceFork {
        sequence: u64,
        first_head: String,
        second_head: String,
    },
    EmptyWriterFenceIdentity,
    InitialWriterFenceAlreadyConsumed,
    WriterFenceRequired,
    WriterFenceMismatch {
        active_generation: u64,
        supplied_generation: u64,
    },
    HistoricalWriterFenceNotFound {
        generation: u64,
    },
    HistoricalWriterFenceMismatch {
        generation: u64,
    },
    WriterFenceGenerationExhausted,
    FenceGenerationGap {
        expected: u64,
        found: u64,
    },
    FenceGenerationFork {
        generation: u64,
        first_receipt: String,
        second_receipt: String,
    },
    FenceChainConflict {
        generation: u64,
        expected_prior: String,
        found_prior: String,
    },
    FenceReceiptMalformed(String),
    UnexpectedEventConflict {
        sequence: u64,
        expected_next: Option<u64>,
    },
    ExactReadbackMismatch(String),
    NeedsReopen,
}

impl fmt::Display for ExploreRunStreamStoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(out),
            Self::InvalidSha256 { field, value } => write!(
                out,
                "exploration stream {field} must be a lowercase SHA-256 digest, got {value:?}"
            ),
            Self::Sha256Mismatch {
                name,
                expected,
                actual,
            } => write!(
                out,
                "exploration stream entry {name:?} hashes to {actual}, expected {expected}"
            ),
            Self::InvalidBlobKind { kind, reason } => {
                write!(out, "invalid exploration stream blob kind {kind:?}: {reason}")
            }
            Self::MalformedRecognizedName(name) => write!(
                out,
                "malformed entry in the exploration stream namespace: {name:?}"
            ),
            Self::MissingGenesis => write!(
                out,
                "exploration stream entries exist without the required run-opened-v1 genesis"
            ),
            Self::EventSequenceGap { expected, found } => write!(
                out,
                "exploration stream event sequence is not contiguous: expected {expected}, found {found}"
            ),
            Self::EventSequenceFork {
                sequence,
                first_head,
                second_head,
            } => write!(
                out,
                "exploration stream sequence {sequence} has conflicting successors {first_head} and {second_head}"
            ),
            Self::EmptyWriterFenceIdentity => write!(
                out,
                "exploration writer-fence identity bytes must not be empty"
            ),
            Self::InitialWriterFenceAlreadyConsumed => write!(
                out,
                "the exploration stream genesis already consumed its initial writer fence"
            ),
            Self::WriterFenceRequired => write!(
                out,
                "a durable writer fence must be acquired before appending exploration events"
            ),
            Self::WriterFenceMismatch {
                active_generation,
                supplied_generation,
            } => write!(
                out,
                "writer fence generation {supplied_generation} is not the active exploration writer generation {active_generation}"
            ),
            Self::HistoricalWriterFenceNotFound { generation } => write!(
                out,
                "exploration writer-fence generation {generation} has no durable receipt"
            ),
            Self::HistoricalWriterFenceMismatch { generation } => write!(
                out,
                "exploration writer-fence generation {generation} does not match its durable receipt"
            ),
            Self::WriterFenceGenerationExhausted => write!(
                out,
                "the exploration writer-fence generation space is exhausted"
            ),
            Self::FenceGenerationGap { expected, found } => write!(
                out,
                "exploration writer-fence generation is not contiguous: expected {expected}, found {found}"
            ),
            Self::FenceGenerationFork {
                generation,
                first_receipt,
                second_receipt,
            } => write!(
                out,
                "exploration writer-fence generation {generation} has conflicting receipts {first_receipt} and {second_receipt}"
            ),
            Self::FenceChainConflict {
                generation,
                expected_prior,
                found_prior,
            } => write!(
                out,
                "exploration writer-fence generation {generation} names prior link {found_prior}, expected {expected_prior}"
            ),
            Self::FenceReceiptMalformed(name) => write!(
                out,
                "exploration writer-fence entry {name:?} has a malformed canonical receipt"
            ),
            Self::UnexpectedEventConflict {
                sequence,
                expected_next,
            } => match expected_next {
                Some(expected_next) => write!(
                    out,
                    "cannot append exploration stream sequence {sequence}; the next sequence is {expected_next}"
                ),
                None => write!(
                    out,
                    "cannot append exploration stream sequence {sequence}; the sequence space is exhausted"
                ),
            },
            Self::ExactReadbackMismatch(name) => write!(
                out,
                "immutable exploration stream entry failed exact readback: {name:?}"
            ),
            Self::NeedsReopen => write!(
                out,
                "the exploration stream store must be reopened after an inconclusive immutable installation"
            ),
        }
    }
}

impl std::error::Error for ExploreRunStreamStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RunStoreError> for ExploreRunStreamStoreError {
    fn from(error: RunStoreError) -> Self {
        Self::Store(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RawExploreStreamEvent {
    pub(crate) sequence: u64,
    pub(crate) journal_head: Box<str>,
    pub(crate) bytes: Vec<u8>,
}

/// Durable proof that this store consumed one writer generation while holding
/// its exclusive lock. Fields are private and there is no public constructor:
/// only a successful exact installation can mint a receipt.
pub(crate) struct ExploreWriterFenceReceipt {
    generation: NonZeroU64,
    receipt_hash: Box<str>,
    writer_lease_identity: Box<[u8]>,
}

impl ExploreWriterFenceReceipt {
    pub(crate) fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    pub(crate) fn receipt_hash(&self) -> &str {
        &self.receipt_hash
    }

    /// Exact canonical identity bytes supplied by the coordinator. The store
    /// neither truncates nor interprets this writer/lease identity.
    pub(crate) fn writer_lease_identity(&self) -> &[u8] {
        &self.writer_lease_identity
    }
}

/// Deterministic generation-one receipt material prepared while an empty
/// store's exclusive lock is held. This is not a durable writer grant: the
/// coordinator first embeds its hash in `RunOpened`, then
/// [`ExploreRunStreamStore::install_genesis`] consumes it while exactly
/// installing those bytes.
pub(crate) struct PreparedInitialWriterFence {
    generation: NonZeroU64,
    receipt_hash: Box<str>,
    writer_lease_identity: Box<[u8]>,
}

impl PreparedInitialWriterFence {
    pub(crate) fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    pub(crate) fn receipt_hash(&self) -> &str {
        &self.receipt_hash
    }

    pub(crate) fn writer_lease_identity(&self) -> &[u8] {
        &self.writer_lease_identity
    }
}

pub(crate) struct RawExploreStreamReplay<'a> {
    guard: &'a RunStoreGuard,
    events: std::vec::IntoIter<EventName>,
    failed: bool,
}

impl Iterator for RawExploreStreamReplay<'_> {
    type Item = Result<RawExploreStreamEvent, ExploreRunStreamStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let event = self.events.next()?;
        match self.guard.read_entry(&event.name) {
            Ok(bytes) => Some(Ok(RawExploreStreamEvent {
                sequence: event.sequence,
                journal_head: event.journal_head,
                bytes,
            })),
            Err(error) => {
                self.failed = true;
                Some(Err(error.into()))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.failed {
            (0, Some(0))
        } else {
            self.events.size_hint()
        }
    }
}

impl std::iter::FusedIterator for RawExploreStreamReplay<'_> {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EventName {
    sequence: u64,
    journal_head: Box<str>,
    name: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FenceName {
    generation: u64,
    receipt_hash: Box<str>,
    prior: Option<FencePrior>,
    writer_lease_identity: Option<Box<[u8]>>,
    name: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FencePrior {
    GenesisAnchor(Box<str>),
    Receipt(Box<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FencePriorRef<'a> {
    GenesisAnchor(&'a str),
    Receipt(&'a str),
}

impl FencePrior {
    fn as_ref(&self) -> FencePriorRef<'_> {
        match self {
            Self::GenesisAnchor(hash) => FencePriorRef::GenesisAnchor(hash),
            Self::Receipt(hash) => FencePriorRef::Receipt(hash),
        }
    }
}

impl FencePriorRef<'_> {
    fn into_owned(self) -> FencePrior {
        match self {
            Self::GenesisAnchor(hash) => FencePrior::GenesisAnchor(hash.into()),
            Self::Receipt(hash) => FencePrior::Receipt(hash.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveWriterFence {
    generation: NonZeroU64,
    receipt_hash: Box<str>,
    writer_lease_identity: Box<[u8]>,
}

#[derive(Debug)]
struct StoreIndex {
    genesis_present: bool,
    genesis_anchor: Option<Box<str>>,
    fences: Vec<FenceName>,
    events: Vec<EventName>,
}

/// Single-writer durable storage for raw exploration-stream records.
///
/// This type intentionally has no `Clone` implementation: ownership of its
/// contained [`RunStoreGuard`] is ownership of the writer lock.
pub(crate) struct ExploreRunStreamStore {
    guard: RunStoreGuard,
    genesis_present: bool,
    genesis_anchor: Option<Box<str>>,
    next_fence_generation: u128,
    fences: Vec<FenceName>,
    fence_tail: Option<(u64, Box<str>)>,
    active_fence: Option<ActiveWriterFence>,
    next_event_sequence: u128,
    tail: Option<(u64, Box<str>)>,
    needs_reopen: bool,
}

impl ExploreRunStreamStore {
    pub(crate) fn open_or_create(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
    ) -> Result<Self, ExploreRunStreamStoreError> {
        Self::from_guard(RunStoreGuard::open_or_create(directory, limits)?)
    }

    pub(crate) fn open(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
    ) -> Result<Self, ExploreRunStreamStoreError> {
        Self::from_guard(RunStoreGuard::open(directory, limits)?)
    }

    pub(crate) fn from_guard(guard: RunStoreGuard) -> Result<Self, ExploreRunStreamStoreError> {
        let StoreIndex {
            genesis_present,
            genesis_anchor,
            fences,
            events,
        } = scan_store(&guard)?;
        let fence_tail = fences
            .last()
            .map(|fence| (fence.generation, fence.receipt_hash.clone()));
        let next_fence_generation = fence_tail.as_ref().map_or(
            u128::from(FIRST_PERSISTED_FENCE_GENERATION),
            |(generation, _)| u128::from(*generation) + 1,
        );
        let tail = events
            .last()
            .map(|event| (event.sequence, event.journal_head.clone()));
        let next_event_sequence = tail
            .as_ref()
            .map_or(u128::from(FIRST_EVENT_SEQUENCE), |(sequence, _)| {
                u128::from(*sequence) + 1
            });
        Ok(Self {
            guard,
            genesis_present,
            genesis_anchor,
            next_fence_generation,
            fences,
            fence_tail,
            // Reopening proves ownership of the directory lock, not ownership
            // of a prior process's lease. Every adapter instance must consume
            // a fresh durable generation before it can append an event.
            active_fence: None,
            next_event_sequence,
            tail,
            needs_reopen: false,
        })
    }

    pub(crate) fn limits(&self) -> RunStoreLimits {
        self.guard.limits()
    }

    pub(crate) fn next_event_sequence(&self) -> Option<u64> {
        u64::try_from(self.next_event_sequence).ok()
    }

    pub(crate) fn next_writer_fence_generation(&self) -> Option<NonZeroU64> {
        if self.genesis_present {
            u64::try_from(self.next_fence_generation)
                .ok()
                .and_then(NonZeroU64::new)
        } else {
            NonZeroU64::new(INITIAL_FENCE_GENERATION)
        }
    }

    pub(crate) fn active_writer_fence(&self) -> Option<(NonZeroU64, &str, &[u8])> {
        self.active_fence.as_ref().map(|fence| {
            (
                fence.generation,
                fence.receipt_hash.as_ref(),
                fence.writer_lease_identity.as_ref(),
            )
        })
    }

    /// Verify a lease decoded from historical journal bytes against the
    /// durable fence namespace. This is a read-only replay check and never
    /// activates that historical lease for new appends.
    pub(crate) fn verify_historical_writer_fence(
        &self,
        generation: NonZeroU64,
        receipt_hash: &str,
        canonical_identity: &[u8],
    ) -> Result<(), ExploreRunStreamStoreError> {
        self.require_writable()?;
        require_lowercase_sha256("historical writer-fence receipt hash", receipt_hash)?;
        if canonical_identity.is_empty() {
            return Err(ExploreRunStreamStoreError::EmptyWriterFenceIdentity);
        }

        if generation.get() == INITIAL_FENCE_GENERATION {
            if !self.genesis_present {
                return Err(ExploreRunStreamStoreError::MissingGenesis);
            }
            let encoded_len = checked_receipt_size(
                &self.guard,
                "initial-writer-fence-receipt",
                initial_fence_receipt_encoded_len(canonical_identity),
            )?;
            let canonical_receipt = encode_initial_fence_receipt(canonical_identity, encoded_len);
            if sha256_hex(&canonical_receipt) == receipt_hash {
                return Ok(());
            }
            return Err(ExploreRunStreamStoreError::HistoricalWriterFenceMismatch {
                generation: generation.get(),
            });
        }

        let fence_index = self
            .fences
            .binary_search_by_key(&generation.get(), |fence| fence.generation)
            .map_err(
                |_| ExploreRunStreamStoreError::HistoricalWriterFenceNotFound {
                    generation: generation.get(),
                },
            )?;
        let fence = &self.fences[fence_index];
        let recorded_identity = fence.writer_lease_identity.as_deref().ok_or_else(|| {
            ExploreRunStreamStoreError::FenceReceiptMalformed(fence.name.to_string())
        })?;
        if fence.receipt_hash.as_ref() != receipt_hash || recorded_identity != canonical_identity {
            return Err(ExploreRunStreamStoreError::HistoricalWriterFenceMismatch {
                generation: generation.get(),
            });
        }
        Ok(())
    }

    /// Derive the generation-one receipt hash needed to encode `RunOpened`.
    /// Preparation neither writes storage nor authorizes event appends.
    pub(crate) fn prepare_initial_writer_fence(
        &self,
        writer_lease_identity: &[u8],
    ) -> Result<PreparedInitialWriterFence, ExploreRunStreamStoreError> {
        self.require_writable()?;
        if self.genesis_present {
            return Err(ExploreRunStreamStoreError::InitialWriterFenceAlreadyConsumed);
        }
        if writer_lease_identity.is_empty() {
            return Err(ExploreRunStreamStoreError::EmptyWriterFenceIdentity);
        }
        let encoded_len = checked_receipt_size(
            &self.guard,
            "initial-writer-fence-receipt",
            initial_fence_receipt_encoded_len(writer_lease_identity),
        )?;
        let canonical_bytes = encode_initial_fence_receipt(writer_lease_identity, encoded_len);
        Ok(PreparedInitialWriterFence {
            generation: NonZeroU64::new(INITIAL_FENCE_GENERATION)
                .expect("the initial writer-fence generation is nonzero"),
            receipt_hash: sha256_hex(&canonical_bytes).into_boxed_str(),
            writer_lease_identity: writer_lease_identity.to_vec().into_boxed_slice(),
        })
    }

    /// Atomically make a prepared generation-one receipt durable by installing
    /// the exact `RunOpened` bytes that embed it. Standalone fence files begin
    /// at generation two, avoiding a receipt/genesis hash cycle.
    pub(crate) fn install_genesis(
        &mut self,
        prepared_fence: PreparedInitialWriterFence,
        bytes: &[u8],
    ) -> Result<ExploreWriterFenceReceipt, ExploreRunStreamStoreError> {
        self.require_writable()?;
        if self.genesis_present {
            return Err(ExploreRunStreamStoreError::InitialWriterFenceAlreadyConsumed);
        }
        let genesis_anchor = sha256_hex(bytes);
        self.install_exact(RUN_OPENED_ENTRY, bytes)?;
        self.genesis_present = true;
        self.genesis_anchor = Some(genesis_anchor.into_boxed_str());
        self.next_fence_generation = u128::from(FIRST_PERSISTED_FENCE_GENERATION);
        self.active_fence = Some(ActiveWriterFence {
            generation: prepared_fence.generation,
            receipt_hash: prepared_fence.receipt_hash.clone(),
            writer_lease_identity: prepared_fence.writer_lease_identity.clone(),
        });
        Ok(ExploreWriterFenceReceipt {
            generation: prepared_fence.generation,
            receipt_hash: prepared_fence.receipt_hash,
            writer_lease_identity: prepared_fence.writer_lease_identity,
        })
    }

    pub(crate) fn read_genesis(&self) -> Result<Option<Vec<u8>>, ExploreRunStreamStoreError> {
        let bytes = read_optional(&self.guard, RUN_OPENED_ENTRY)?;
        if bytes.is_none() && self.genesis_present {
            Err(ExploreRunStreamStoreError::MissingGenesis)
        } else {
            Ok(bytes)
        }
    }

    /// Durably consume the next writer generation under this adapter's held
    /// run-store lock. Reopening never reactivates the prior receipt: a new
    /// coordinator must call this method before it can append any event.
    pub(crate) fn acquire_writer_fence(
        &mut self,
        writer_lease_identity: &[u8],
    ) -> Result<ExploreWriterFenceReceipt, ExploreRunStreamStoreError> {
        self.require_writable()?;
        if !self.genesis_present {
            return Err(ExploreRunStreamStoreError::MissingGenesis);
        }
        if writer_lease_identity.is_empty() {
            return Err(ExploreRunStreamStoreError::EmptyWriterFenceIdentity);
        }
        let generation = self
            .next_writer_fence_generation()
            .ok_or(ExploreRunStreamStoreError::WriterFenceGenerationExhausted)?;
        let prior = match &self.fence_tail {
            Some((_, receipt_hash)) => FencePriorRef::Receipt(receipt_hash),
            None => FencePriorRef::GenesisAnchor(
                self.genesis_anchor
                    .as_deref()
                    .ok_or(ExploreRunStreamStoreError::MissingGenesis)?,
            ),
        };
        let encoded_len = checked_receipt_size(
            &self.guard,
            "writer-fence-receipt",
            fence_receipt_encoded_len(prior, writer_lease_identity),
        )?;
        let canonical_bytes =
            encode_fence_receipt(generation, prior, writer_lease_identity, encoded_len);
        let owned_prior = prior.into_owned();
        let receipt_hash = sha256_hex(&canonical_bytes);
        let name = fence_entry_name(generation.get(), &receipt_hash)?;
        self.install_exact(&name, &canonical_bytes)?;

        self.fences.push(FenceName {
            generation: generation.get(),
            receipt_hash: receipt_hash.clone().into_boxed_str(),
            prior: Some(owned_prior),
            writer_lease_identity: Some(writer_lease_identity.to_vec().into_boxed_slice()),
            name: name.into_boxed_str(),
        });
        self.fence_tail = Some((generation.get(), receipt_hash.clone().into_boxed_str()));
        self.next_fence_generation = u128::from(generation.get()) + 1;
        self.active_fence = Some(ActiveWriterFence {
            generation,
            receipt_hash: receipt_hash.clone().into_boxed_str(),
            writer_lease_identity: writer_lease_identity.to_vec().into_boxed_slice(),
        });
        Ok(ExploreWriterFenceReceipt {
            generation,
            receipt_hash: receipt_hash.into_boxed_str(),
            writer_lease_identity: writer_lease_identity.to_vec().into_boxed_slice(),
        })
    }

    pub(crate) fn install_blob(
        &mut self,
        kind: &str,
        sha256: &str,
        bytes: &[u8],
    ) -> Result<(), ExploreRunStreamStoreError> {
        self.require_writable()?;
        if !self.genesis_present {
            return Err(ExploreRunStreamStoreError::MissingGenesis);
        }
        require_lowercase_sha256("blob digest", sha256)?;
        let name = blob_entry_name(kind, sha256)?;
        verify_content_digest(&name, sha256, bytes)?;
        self.install_exact(&name, bytes)
    }

    pub(crate) fn read_blob(
        &self,
        kind: &str,
        sha256: &str,
    ) -> Result<Vec<u8>, ExploreRunStreamStoreError> {
        require_lowercase_sha256("blob digest", sha256)?;
        let name = blob_entry_name(kind, sha256)?;
        let bytes = self.guard.read_entry(&name)?;
        verify_content_digest(&name, sha256, &bytes)?;
        Ok(bytes)
    }

    pub(crate) fn append_event(
        &mut self,
        writer_fence: &ExploreWriterFenceReceipt,
        sequence: u64,
        journal_head: &str,
        bytes: &[u8],
    ) -> Result<(), ExploreRunStreamStoreError> {
        self.require_writable()?;
        if !self.genesis_present {
            return Err(ExploreRunStreamStoreError::MissingGenesis);
        }
        self.require_active_writer_fence(writer_fence)?;
        require_lowercase_sha256("event journal head", journal_head)?;
        let name = event_entry_name(sequence, journal_head)?;
        let attempted = u128::from(sequence);

        if attempted < self.next_event_sequence {
            if let Some((tail_sequence, tail_head)) = &self.tail {
                if *tail_sequence == sequence && tail_head.as_ref() != journal_head {
                    return Err(ExploreRunStreamStoreError::EventSequenceFork {
                        sequence,
                        first_head: tail_head.to_string(),
                        second_head: journal_head.to_owned(),
                    });
                }
            }
            let Some(existing) = read_optional(&self.guard, &name)? else {
                return Err(ExploreRunStreamStoreError::UnexpectedEventConflict {
                    sequence,
                    expected_next: self.next_event_sequence(),
                });
            };
            if existing != bytes {
                return Err(ExploreRunStreamStoreError::ExactReadbackMismatch(name));
            }
            return Ok(());
        }

        if attempted > self.next_event_sequence {
            return Err(ExploreRunStreamStoreError::EventSequenceGap {
                expected: self.next_event_sequence().unwrap_or(u64::MAX),
                found: sequence,
            });
        }

        let Some(next_after_commit) = sequence.checked_add(1) else {
            return Err(ExploreRunStreamStoreError::UnexpectedEventConflict {
                sequence,
                expected_next: None,
            });
        };
        self.install_exact(&name, bytes)?;
        self.tail = Some((sequence, journal_head.into()));
        self.next_event_sequence = u128::from(next_after_commit);
        Ok(())
    }

    pub(crate) fn read_event(
        &self,
        sequence: u64,
        journal_head: &str,
    ) -> Result<Vec<u8>, ExploreRunStreamStoreError> {
        require_lowercase_sha256("event journal head", journal_head)?;
        let name = event_entry_name(sequence, journal_head)?;
        self.guard.read_entry(&name).map_err(Into::into)
    }

    pub(crate) fn replay_events(
        &self,
    ) -> Result<RawExploreStreamReplay<'_>, ExploreRunStreamStoreError> {
        let index = scan_store(&self.guard)?;
        Ok(RawExploreStreamReplay {
            guard: &self.guard,
            events: index.events.into_iter(),
            failed: false,
        })
    }

    fn require_writable(&self) -> Result<(), ExploreRunStreamStoreError> {
        if self.needs_reopen {
            Err(ExploreRunStreamStoreError::NeedsReopen)
        } else {
            Ok(())
        }
    }

    fn require_active_writer_fence(
        &self,
        supplied: &ExploreWriterFenceReceipt,
    ) -> Result<(), ExploreRunStreamStoreError> {
        let Some(active) = &self.active_fence else {
            return Err(ExploreRunStreamStoreError::WriterFenceRequired);
        };
        if active.generation != supplied.generation
            || active.receipt_hash.as_ref() != supplied.receipt_hash.as_ref()
            || active.writer_lease_identity.as_ref() != supplied.writer_lease_identity.as_ref()
        {
            return Err(ExploreRunStreamStoreError::WriterFenceMismatch {
                active_generation: active.generation.get(),
                supplied_generation: supplied.generation.get(),
            });
        }
        Ok(())
    }

    fn install_exact(
        &mut self,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), ExploreRunStreamStoreError> {
        ensure_entry_name_bound(name)?;
        ensure_entry_size(&self.guard, name, bytes)?;
        let installed = self
            .guard
            .install_immutable(name, bytes)
            .and_then(|_| self.guard.read_entry(name));
        match installed {
            Ok(readback) if readback == bytes => Ok(()),
            Ok(_) => {
                self.needs_reopen = true;
                Err(ExploreRunStreamStoreError::ExactReadbackMismatch(
                    name.to_owned(),
                ))
            }
            Err(error) => {
                // The immutable final name may have become visible. Force a
                // reopen so a new guard rescans the append-only namespace
                // before any later write trusts an in-memory tail.
                self.needs_reopen = true;
                Err(error.into())
            }
        }
    }
}

fn scan_store(guard: &RunStoreGuard) -> Result<StoreIndex, ExploreRunStreamStoreError> {
    let mut genesis_present = false;
    let mut genesis_anchor = None;
    let mut recognized_non_genesis = false;
    let mut fences = Vec::new();
    let mut events = Vec::new();

    for entry in guard.list_entries()? {
        let name = entry.name();
        if name == RUN_OPENED_ENTRY {
            let bytes = guard.read_entry(name)?;
            genesis_present = true;
            genesis_anchor = Some(sha256_hex(&bytes).into_boxed_str());
        } else if name.starts_with(RUN_OPENED_FAMILY) {
            return Err(ExploreRunStreamStoreError::MalformedRecognizedName(
                name.to_owned(),
            ));
        } else if name.starts_with(FENCE_FAMILY) {
            recognized_non_genesis = true;
            let mut fence = parse_fence_entry_name(name)?;
            let bytes = guard.read_entry(name)?;
            verify_content_digest(name, &fence.receipt_hash, &bytes)?;
            let receipt = parse_fence_receipt(name, &bytes)?;
            if receipt.generation.get() != fence.generation {
                return Err(ExploreRunStreamStoreError::FenceReceiptMalformed(
                    name.to_owned(),
                ));
            }
            let canonical = encode_fence_receipt(
                receipt.generation,
                receipt.prior,
                receipt.writer_lease_identity,
                bytes.len(),
            );
            if canonical != bytes {
                return Err(ExploreRunStreamStoreError::FenceReceiptMalformed(
                    name.to_owned(),
                ));
            }
            fence.prior = Some(receipt.prior.into_owned());
            fence.writer_lease_identity =
                Some(receipt.writer_lease_identity.to_vec().into_boxed_slice());
            fences.push(fence);
        } else if name.starts_with(EVENT_FAMILY) {
            recognized_non_genesis = true;
            events.push(parse_event_entry_name(name)?);
        } else if name.starts_with(BLOB_FAMILY) {
            recognized_non_genesis = true;
            parse_blob_entry_name(name)?;
        }
    }

    if recognized_non_genesis && !genesis_present {
        return Err(ExploreRunStreamStoreError::MissingGenesis);
    }

    fences.sort_unstable_by(|left, right| {
        left.generation
            .cmp(&right.generation)
            .then_with(|| left.receipt_hash.cmp(&right.receipt_hash))
    });

    let mut expected_generation = u128::from(FIRST_PERSISTED_FENCE_GENERATION);
    let mut previous_fence: Option<&FenceName> = None;
    for fence in &fences {
        if let Some(previous) = previous_fence {
            if fence.generation == previous.generation {
                return Err(ExploreRunStreamStoreError::FenceGenerationFork {
                    generation: fence.generation,
                    first_receipt: previous.receipt_hash.to_string(),
                    second_receipt: fence.receipt_hash.to_string(),
                });
            }
        }
        if u128::from(fence.generation) != expected_generation {
            return Err(ExploreRunStreamStoreError::FenceGenerationGap {
                expected: u64::try_from(expected_generation).unwrap_or(u64::MAX),
                found: fence.generation,
            });
        }
        let expected_prior = match previous_fence {
            Some(previous) => FencePriorRef::Receipt(&previous.receipt_hash),
            None => FencePriorRef::GenesisAnchor(
                genesis_anchor
                    .as_deref()
                    .ok_or(ExploreRunStreamStoreError::MissingGenesis)?,
            ),
        };
        let found_prior = fence.prior.as_ref().ok_or_else(|| {
            ExploreRunStreamStoreError::FenceReceiptMalformed(fence.name.to_string())
        })?;
        if found_prior.as_ref() != expected_prior {
            return Err(ExploreRunStreamStoreError::FenceChainConflict {
                generation: fence.generation,
                expected_prior: describe_fence_prior(expected_prior),
                found_prior: describe_fence_prior(found_prior.as_ref()),
            });
        }
        expected_generation += 1;
        previous_fence = Some(fence);
    }

    events.sort_unstable_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.journal_head.cmp(&right.journal_head))
    });

    let mut expected = FIRST_EVENT_SEQUENCE;
    let mut previous: Option<&EventName> = None;
    for event in &events {
        if let Some(previous) = previous {
            if event.sequence == previous.sequence {
                return Err(ExploreRunStreamStoreError::EventSequenceFork {
                    sequence: event.sequence,
                    first_head: previous.journal_head.to_string(),
                    second_head: event.journal_head.to_string(),
                });
            }
        }
        if event.sequence != expected {
            return Err(ExploreRunStreamStoreError::EventSequenceGap {
                expected,
                found: event.sequence,
            });
        }
        expected =
            expected
                .checked_add(1)
                .ok_or(ExploreRunStreamStoreError::UnexpectedEventConflict {
                    sequence: event.sequence,
                    expected_next: None,
                })?;
        previous = Some(event);
    }

    Ok(StoreIndex {
        genesis_present,
        genesis_anchor,
        fences,
        events,
    })
}

fn fence_entry_name(
    generation: u64,
    receipt_hash: &str,
) -> Result<String, ExploreRunStreamStoreError> {
    if generation < FIRST_PERSISTED_FENCE_GENERATION {
        return Err(ExploreRunStreamStoreError::FenceGenerationGap {
            expected: FIRST_PERSISTED_FENCE_GENERATION,
            found: generation,
        });
    }
    require_lowercase_sha256("writer-fence receipt hash", receipt_hash)?;
    let name = format!("{FENCE_PREFIX}{generation:032x}-{receipt_hash}");
    ensure_entry_name_bound(&name)?;
    Ok(name)
}

fn parse_fence_entry_name(name: &str) -> Result<FenceName, ExploreRunStreamStoreError> {
    let malformed = || ExploreRunStreamStoreError::MalformedRecognizedName(name.to_owned());
    let rest = name.strip_prefix(FENCE_PREFIX).ok_or_else(malformed)?;
    let (generation_hex, receipt_hash) = rest.split_once('-').ok_or_else(malformed)?;
    if generation_hex.len() != FENCE_GENERATION_HEX_BYTES
        || !generation_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(malformed());
    }
    require_lowercase_sha256("writer-fence receipt hash", receipt_hash).map_err(|_| malformed())?;
    let generation = u64::from_str_radix(generation_hex, 16).map_err(|_| malformed())?;
    if generation < FIRST_PERSISTED_FENCE_GENERATION
        || format!("{generation:032x}") != generation_hex
        || fence_entry_name(generation, receipt_hash).map_err(|_| malformed())? != name
    {
        return Err(malformed());
    }
    Ok(FenceName {
        generation,
        receipt_hash: receipt_hash.into(),
        prior: None,
        writer_lease_identity: None,
        name: name.into(),
    })
}

struct ParsedFenceReceipt<'a> {
    generation: NonZeroU64,
    prior: FencePriorRef<'a>,
    writer_lease_identity: &'a [u8],
}

struct FenceReceiptCursor<'a> {
    remaining: &'a [u8],
}

impl<'a> FenceReceiptCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn segment(&mut self) -> Option<&'a [u8]> {
        let length_bytes: [u8; 8] = self.remaining.get(..8)?.try_into().ok()?;
        let length = usize::try_from(u64::from_le_bytes(length_bytes)).ok()?;
        let end = 8_usize.checked_add(length)?;
        let segment = self.remaining.get(8..end)?;
        self.remaining = self.remaining.get(end..)?;
        Some(segment)
    }

    fn is_finished(&self) -> bool {
        self.remaining.is_empty()
    }
}

fn parse_fence_receipt<'a>(
    name: &str,
    bytes: &'a [u8],
) -> Result<ParsedFenceReceipt<'a>, ExploreRunStreamStoreError> {
    let malformed = || ExploreRunStreamStoreError::FenceReceiptMalformed(name.to_owned());
    let mut cursor = FenceReceiptCursor::new(bytes);
    if cursor.segment().ok_or_else(malformed)? != FENCE_RECEIPT_DOMAIN_V1 {
        return Err(malformed());
    }
    let generation_bytes: [u8; 8] = cursor
        .segment()
        .ok_or_else(malformed)?
        .try_into()
        .map_err(|_| malformed())?;
    let generation = NonZeroU64::new(u64::from_le_bytes(generation_bytes)).ok_or_else(malformed)?;
    if generation.get() < FIRST_PERSISTED_FENCE_GENERATION {
        return Err(malformed());
    }
    let prior_kind = cursor.segment().ok_or_else(malformed)?;
    let prior_hash =
        std::str::from_utf8(cursor.segment().ok_or_else(malformed)?).map_err(|_| malformed())?;
    require_lowercase_sha256("writer-fence prior hash", prior_hash).map_err(|_| malformed())?;
    let prior = if prior_kind == FENCE_PRIOR_GENESIS_ANCHOR_V1 {
        FencePriorRef::GenesisAnchor(prior_hash)
    } else if prior_kind == FENCE_PRIOR_RECEIPT_V1 {
        FencePriorRef::Receipt(prior_hash)
    } else {
        return Err(malformed());
    };
    let writer_lease_identity = cursor.segment().ok_or_else(malformed)?;
    if writer_lease_identity.is_empty() || !cursor.is_finished() {
        return Err(malformed());
    }
    Ok(ParsedFenceReceipt {
        generation,
        prior,
        writer_lease_identity,
    })
}

/// Generation one uses a separate length-framed domain, the fixed generation,
/// an explicit `RunOpened`-genesis marker, and the complete opaque identity.
fn initial_fence_receipt_encoded_len(writer_lease_identity: &[u8]) -> Option<usize> {
    segment_encoded_len(INITIAL_FENCE_RECEIPT_DOMAIN_V1.len())?
        .checked_add(segment_encoded_len(std::mem::size_of::<u64>())?)?
        .checked_add(segment_encoded_len(
            INITIAL_FENCE_PRIOR_RUN_OPENED_V1.len(),
        )?)?
        .checked_add(segment_encoded_len(writer_lease_identity.len())?)
}

/// Persisted receipts are an unambiguous length-framed tuple of protocol
/// domain, generation, typed prior digest, and the complete opaque identity.
fn fence_receipt_encoded_len(
    prior: FencePriorRef<'_>,
    writer_lease_identity: &[u8],
) -> Option<usize> {
    let (prior_kind, prior_hash) = match prior {
        FencePriorRef::GenesisAnchor(hash) => (FENCE_PRIOR_GENESIS_ANCHOR_V1, hash),
        FencePriorRef::Receipt(hash) => (FENCE_PRIOR_RECEIPT_V1, hash),
    };
    segment_encoded_len(FENCE_RECEIPT_DOMAIN_V1.len())?
        .checked_add(segment_encoded_len(std::mem::size_of::<u64>())?)?
        .checked_add(segment_encoded_len(prior_kind.len())?)?
        .checked_add(segment_encoded_len(prior_hash.len())?)?
        .checked_add(segment_encoded_len(writer_lease_identity.len())?)
}

fn encode_initial_fence_receipt(writer_lease_identity: &[u8], capacity: usize) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(capacity);
    encode_receipt_segment(&mut encoded, INITIAL_FENCE_RECEIPT_DOMAIN_V1);
    encode_receipt_segment(&mut encoded, &INITIAL_FENCE_GENERATION.to_le_bytes());
    encode_receipt_segment(&mut encoded, INITIAL_FENCE_PRIOR_RUN_OPENED_V1);
    encode_receipt_segment(&mut encoded, writer_lease_identity);
    encoded
}

fn encode_fence_receipt(
    generation: NonZeroU64,
    prior: FencePriorRef<'_>,
    writer_lease_identity: &[u8],
    capacity: usize,
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(capacity);
    encode_receipt_segment(&mut encoded, FENCE_RECEIPT_DOMAIN_V1);
    encode_receipt_segment(&mut encoded, &generation.get().to_le_bytes());
    match prior {
        FencePriorRef::GenesisAnchor(hash) => {
            encode_receipt_segment(&mut encoded, FENCE_PRIOR_GENESIS_ANCHOR_V1);
            encode_receipt_segment(&mut encoded, hash.as_bytes());
        }
        FencePriorRef::Receipt(hash) => {
            encode_receipt_segment(&mut encoded, FENCE_PRIOR_RECEIPT_V1);
            encode_receipt_segment(&mut encoded, hash.as_bytes());
        }
    }
    encode_receipt_segment(&mut encoded, writer_lease_identity);
    encoded
}

fn segment_encoded_len(length: usize) -> Option<usize> {
    std::mem::size_of::<u64>().checked_add(length)
}

fn encode_receipt_segment(out: &mut Vec<u8>, bytes: &[u8]) {
    let length =
        u64::try_from(bytes.len()).expect("validated writer-fence segment length fits in u64");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(bytes);
}

fn checked_receipt_size(
    guard: &RunStoreGuard,
    name: &'static str,
    encoded_len: Option<usize>,
) -> Result<usize, ExploreRunStreamStoreError> {
    let limit = guard.limits().max_entry_bytes();
    let Some(encoded_len) = encoded_len else {
        return Err(RunStoreError::EntryTooLarge {
            name: name.to_owned(),
            bytes: u64::MAX,
            limit,
        }
        .into());
    };
    let byte_count = match u64::try_from(encoded_len) {
        Ok(byte_count) => byte_count,
        Err(_) => {
            return Err(RunStoreError::EntryTooLarge {
                name: name.to_owned(),
                bytes: u64::MAX,
                limit,
            }
            .into());
        }
    };
    if byte_count > limit {
        Err(RunStoreError::EntryTooLarge {
            name: name.to_owned(),
            bytes: byte_count,
            limit,
        }
        .into())
    } else {
        Ok(encoded_len)
    }
}

fn describe_fence_prior(prior: FencePriorRef<'_>) -> String {
    match prior {
        FencePriorRef::GenesisAnchor(hash) => format!("genesis-anchor:{hash}"),
        FencePriorRef::Receipt(hash) => format!("receipt:{hash}"),
    }
}

fn event_entry_name(
    sequence: u64,
    journal_head: &str,
) -> Result<String, ExploreRunStreamStoreError> {
    if sequence < FIRST_EVENT_SEQUENCE {
        return Err(ExploreRunStreamStoreError::EventSequenceGap {
            expected: FIRST_EVENT_SEQUENCE,
            found: sequence,
        });
    }
    require_lowercase_sha256("event journal head", journal_head)?;
    let name = format!("{EVENT_PREFIX}{sequence:032x}-{journal_head}");
    ensure_entry_name_bound(&name)?;
    Ok(name)
}

fn parse_event_entry_name(name: &str) -> Result<EventName, ExploreRunStreamStoreError> {
    let malformed = || ExploreRunStreamStoreError::MalformedRecognizedName(name.to_owned());
    let rest = name.strip_prefix(EVENT_PREFIX).ok_or_else(malformed)?;
    let (sequence_hex, journal_head) = rest.split_once('-').ok_or_else(malformed)?;
    if sequence_hex.len() != EVENT_SEQUENCE_HEX_BYTES
        || !sequence_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(malformed());
    }
    require_lowercase_sha256("event journal head", journal_head).map_err(|_| malformed())?;
    let sequence = u64::from_str_radix(sequence_hex, 16).map_err(|_| malformed())?;
    if format!("{sequence:032x}") != sequence_hex
        || event_entry_name(sequence, journal_head)? != name
    {
        return Err(malformed());
    }
    Ok(EventName {
        sequence,
        journal_head: journal_head.into(),
        name: name.into(),
    })
}

fn blob_entry_name(kind: &str, sha256: &str) -> Result<String, ExploreRunStreamStoreError> {
    validate_blob_kind(kind)?;
    require_lowercase_sha256("blob digest", sha256)?;
    let name = format!("{BLOB_PREFIX}{kind}-{sha256}");
    ensure_entry_name_bound(&name)?;
    Ok(name)
}

fn parse_blob_entry_name(name: &str) -> Result<(), ExploreRunStreamStoreError> {
    let malformed = || ExploreRunStreamStoreError::MalformedRecognizedName(name.to_owned());
    let rest = name.strip_prefix(BLOB_PREFIX).ok_or_else(malformed)?;
    let (kind, sha256) = rest.rsplit_once('-').ok_or_else(malformed)?;
    validate_blob_kind(kind).map_err(|_| malformed())?;
    require_lowercase_sha256("blob digest", sha256).map_err(|_| malformed())?;
    if blob_entry_name(kind, sha256)? != name {
        return Err(malformed());
    }
    Ok(())
}

fn validate_blob_kind(kind: &str) -> Result<(), ExploreRunStreamStoreError> {
    let bytes = kind.as_bytes();
    let edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if bytes.is_empty()
        || !edge(bytes[0])
        || !edge(bytes[bytes.len() - 1])
        || !bytes.iter().copied().all(|byte| edge(byte) || byte == b'-')
    {
        return Err(ExploreRunStreamStoreError::InvalidBlobKind {
            kind: kind.to_owned(),
            reason: "use a nonempty lowercase ASCII token with optional interior hyphens",
        });
    }
    let name_bytes = BLOB_PREFIX
        .len()
        .checked_add(bytes.len())
        .and_then(|value| value.checked_add(1 + SHA256_HEX_BYTES));
    if name_bytes.is_none_or(|bytes| bytes > RUN_STORE_MAX_NAME_BYTES) {
        return Err(ExploreRunStreamStoreError::InvalidBlobKind {
            kind: kind.to_owned(),
            reason: "the resulting content-addressed entry name exceeds the run-store bound",
        });
    }
    Ok(())
}

fn require_lowercase_sha256(
    field: &'static str,
    value: &str,
) -> Result<(), ExploreRunStreamStoreError> {
    let valid = value.len() == SHA256_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(ExploreRunStreamStoreError::InvalidSha256 {
            field,
            value: value.to_owned(),
        })
    }
}

fn verify_content_digest(
    name: &str,
    expected: &str,
    bytes: &[u8],
) -> Result<(), ExploreRunStreamStoreError> {
    let actual = sha256_hex(bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(ExploreRunStreamStoreError::Sha256Mismatch {
            name: name.to_owned(),
            expected: expected.to_owned(),
            actual,
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(SHA256_HEX_BYTES);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn ensure_entry_name_bound(name: &str) -> Result<(), ExploreRunStreamStoreError> {
    if name.len() <= RUN_STORE_MAX_NAME_BYTES {
        Ok(())
    } else {
        Err(ExploreRunStreamStoreError::MalformedRecognizedName(
            name.to_owned(),
        ))
    }
}

fn ensure_entry_size(
    guard: &RunStoreGuard,
    name: &str,
    bytes: &[u8],
) -> Result<(), ExploreRunStreamStoreError> {
    let limit = guard.limits().max_entry_bytes();
    let byte_count = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_count <= limit {
        Ok(())
    } else {
        Err(RunStoreError::EntryTooLarge {
            name: name.to_owned(),
            bytes: byte_count,
            limit,
        }
        .into())
    }
}

fn read_optional(guard: &RunStoreGuard, name: &str) -> Result<Option<Vec<u8>>, RunStoreError> {
    match guard.read_entry(name) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(RunStoreError::EntryNotFound(found)) if found == name => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        byte.to_string().repeat(SHA256_HEX_BYTES)
    }

    #[test]
    fn event_names_are_fixed_width_and_round_trip() {
        let head = digest('a');
        let name = event_entry_name(42, &head).unwrap();
        assert_eq!(
            name,
            format!("event-v1-0000000000000000000000000000002a-{head}")
        );
        let parsed = parse_event_entry_name(&name).unwrap();
        assert_eq!(parsed.sequence, 42);
        assert_eq!(parsed.journal_head.as_ref(), head.as_str());
    }

    #[test]
    fn malformed_recognized_event_name_is_rejected() {
        let name = format!("event-v1-1-{}", digest('a'));
        assert!(matches!(
            parse_event_entry_name(&name),
            Err(ExploreRunStreamStoreError::MalformedRecognizedName(_))
        ));
    }

    #[test]
    fn blob_names_allow_hyphenated_kinds_and_round_trip() {
        let hash = digest('b');
        let name = blob_entry_name("probe-observation", &hash).unwrap();
        assert_eq!(name, format!("blob-v1-probe-observation-{hash}"));
        parse_blob_entry_name(&name).unwrap();
    }

    #[test]
    fn sha256_is_lowercase_and_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
