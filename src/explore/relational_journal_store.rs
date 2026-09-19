//! Bounded immutable segments for packed relational-journal physical frames.
//!
//! This layer deliberately does not know how to encode or decode a
//! `RelationalJournalEntry`. The caller supplies a bounded packed frame plus
//! its semantic sequence range and chain heads. Physical frames are buffered
//! into bounded segments and
//! each complete segment is installed as one immutable [`RunStoreGuard`] entry.
//! Reopening scans only installed final names; an unflushed in-memory tail has
//! no durable meaning and is intentionally absent from replay.
//!
//! Segment replay validates the storage envelope before yielding any bytes:
//! canonical names and headers, SHA-256 digests, contiguous segment and
//! physical-frame semantic ranges, prior-segment and journal-head anchors,
//! bounded frame lengths, and exact exhaustion of every segment. Decoding the
//! length-delimited semantic entries inside each frame remains the outer
//! journal's responsibility.
//!
//! The configured bounds are allocation bounds, not merely validation hints.
//! The writer retains one segment buffer of at most `max_segment_bytes`; an
//! install/readback can transiently hold one additional segment-sized vector.
//! Discovery reads and validates one segment at a time and retains only bounded
//! descriptors. Replay owns at most one readback segment at a time, and its
//! frame iterator borrows payload slices instead of cloning them.

use std::error::Error;
use std::fmt;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::run_store::{
    RunStoreError, RunStoreGuard, RunStoreLimits, RUN_STORE_MAX_ENTRY_BYTES,
    RUN_STORE_MAX_LIST_ENTRIES, RUN_STORE_MAX_NAME_BYTES,
};

pub(crate) const RELATIONAL_JOURNAL_SEGMENT_SCHEMA_VERSION: u32 = 2;

pub(crate) const RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_BYTES: usize = 64 << 20;
pub(crate) const RELATIONAL_JOURNAL_FRAME_HARD_MAX_BYTES: usize = 16 << 20;
pub(crate) const RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_FRAMES: u64 = 1_000_000;
pub(crate) const RELATIONAL_JOURNAL_STORE_HARD_MAX_SEGMENTS: usize = 1_000_000;

const SEGMENT_MAGIC: &[u8; 8] = b"FTRJSEG2";
const SEGMENT_PREFIX: &str = "reljournal-segment-v2-";
const SEGMENT_FAMILY: &str = "reljournal-segment";
const LEGACY_RUN_OPENED_FAMILY: &str = "run-opened";
const LEGACY_FENCE_V1_FAMILY: &str = "fence-v1";
const LEGACY_BLOB_V1_FAMILY: &str = "blob-v1";
const LEGACY_EVENT_V1_FAMILY: &str = "event-v1";
const SEGMENT_ORDINAL_HEX_BYTES: usize = 16;
const SEGMENT_SEQUENCE_HEX_BYTES: usize = 16;
const SHA256_HEX_BYTES: usize = 64;

const SEGMENT_FLAG_HAS_PRIOR: u32 = 0x01;
const SEGMENT_KNOWN_FLAGS: u32 = SEGMENT_FLAG_HAS_PRIOR;

// Fixed header, all integers big-endian:
// magic[8], schema[4], header_len[4], ordinal[8], first[8], last[8],
// physical_frame_count[8], semantic_event_count[8], frame_bytes_len[8],
// flags[4], reserved[4],
// prior_segment[32], prior_head[32], terminal_head[32], frame_digest[32].
const SEGMENT_HEADER_BYTES: usize = 200;

// first_sequence[8], last_sequence[8], semantic_event_count[8],
// previous_head[32], terminal_head[32], payload_len[8].
const FRAME_PREFIX_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalSegmentLimits {
    max_segment_bytes: usize,
    max_frame_bytes: usize,
    /// Physical packed frames, not semantic journal entries.
    max_frames_per_segment: u64,
    max_segments: usize,
}

impl RelationalJournalSegmentLimits {
    pub(crate) fn new(
        max_segment_bytes: usize,
        max_frame_bytes: usize,
        max_frames_per_segment: u64,
        max_segments: usize,
    ) -> Result<Self, RelationalJournalSegmentStoreError> {
        if !(SEGMENT_HEADER_BYTES + FRAME_PREFIX_BYTES..=RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_BYTES)
            .contains(&max_segment_bytes)
        {
            return Err(RelationalJournalSegmentStoreError::InvalidLimits(
                "max_segment_bytes is outside the hard nonzero segment bound",
            ));
        }
        if max_frame_bytes > RELATIONAL_JOURNAL_FRAME_HARD_MAX_BYTES
            || SEGMENT_HEADER_BYTES
                .checked_add(FRAME_PREFIX_BYTES)
                .and_then(|bytes| bytes.checked_add(max_frame_bytes))
                .is_none_or(|bytes| bytes > max_segment_bytes)
        {
            return Err(RelationalJournalSegmentStoreError::InvalidLimits(
                "max_frame_bytes cannot fit one frame in a bounded segment",
            ));
        }
        if max_frames_per_segment == 0
            || max_frames_per_segment > RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_FRAMES
        {
            return Err(RelationalJournalSegmentStoreError::InvalidLimits(
                "max_frames_per_segment is outside the hard nonzero bound",
            ));
        }
        if max_segments == 0 || max_segments > RELATIONAL_JOURNAL_STORE_HARD_MAX_SEGMENTS {
            return Err(RelationalJournalSegmentStoreError::InvalidLimits(
                "max_segments is outside the hard nonzero bound",
            ));
        }
        Ok(Self {
            max_segment_bytes,
            max_frame_bytes,
            max_frames_per_segment,
            max_segments,
        })
    }

    pub(crate) const fn max_segment_bytes(self) -> usize {
        self.max_segment_bytes
    }

    pub(crate) const fn max_frame_bytes(self) -> usize {
        self.max_frame_bytes
    }

    pub(crate) const fn max_frames_per_segment(self) -> u64 {
        self.max_frames_per_segment
    }

    pub(crate) const fn max_segments(self) -> usize {
        self.max_segments
    }
}

impl Default for RelationalJournalSegmentLimits {
    fn default() -> Self {
        Self {
            // A bounded 65,536-coordinate page may alternate at every unit.
            // Its authenticated run descriptors need <8 MiB for one question,
            // even when there is no compression. This is a physical buffer
            // bound, not a relaxation of the host-wide resource governor.
            max_segment_bytes: 16 << 20,
            max_frame_bytes: 8 << 20,
            max_frames_per_segment: 65_536,
            max_segments: 100_000,
        }
    }
}

/// Expected beginning of this segment namespace. For the ordinary outer
/// journal this is sequence zero and its contract-derived genesis head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalStoreAnchor {
    first_sequence: u64,
    initial_head: [u8; 32],
}

impl RelationalJournalStoreAnchor {
    pub(crate) const fn new(first_sequence: u64, initial_head: [u8; 32]) -> Self {
        Self {
            first_sequence,
            initial_head,
        }
    }

    pub(crate) const fn first_sequence(self) -> u64 {
        self.first_sequence
    }

    pub(crate) const fn initial_head(self) -> [u8; 32] {
        self.initial_head
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalJournalSegmentDigest([u8; 32]);

impl RelationalJournalSegmentDigest {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Durable identity and chain boundary of one fully installed segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalSegmentReceipt {
    ordinal: u64,
    first_sequence: u64,
    last_sequence: u64,
    /// Number of packed storage envelopes in this segment.
    physical_frame_count: u64,
    /// Number of canonical journal entries recoverable from those envelopes.
    semantic_event_count: u64,
    prior_segment: Option<RelationalJournalSegmentDigest>,
    prior_head: [u8; 32],
    terminal_head: [u8; 32],
    digest: RelationalJournalSegmentDigest,
    bytes: u64,
}

impl RelationalJournalSegmentReceipt {
    pub(crate) const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub(crate) const fn first_sequence(&self) -> u64 {
        self.first_sequence
    }

    pub(crate) const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }

    pub(crate) const fn physical_frame_count(&self) -> u64 {
        self.physical_frame_count
    }

    pub(crate) const fn semantic_event_count(&self) -> u64 {
        self.semantic_event_count
    }

    pub(crate) const fn prior_segment(&self) -> Option<RelationalJournalSegmentDigest> {
        self.prior_segment
    }

    pub(crate) const fn prior_head(&self) -> [u8; 32] {
        self.prior_head
    }

    pub(crate) const fn terminal_head(&self) -> [u8; 32] {
        self.terminal_head
    }

    pub(crate) const fn digest(&self) -> RelationalJournalSegmentDigest {
        self.digest
    }

    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalSegmentAppend {
    installed_segment: Option<RelationalJournalSegmentReceipt>,
    buffered_physical_frames: u64,
    buffered_semantic_events: u64,
    buffered_bytes: usize,
}

impl RelationalJournalSegmentAppend {
    pub(crate) const fn installed_segment(&self) -> Option<&RelationalJournalSegmentReceipt> {
        self.installed_segment.as_ref()
    }

    pub(crate) const fn buffered_physical_frames(&self) -> u64 {
        self.buffered_physical_frames
    }

    pub(crate) const fn buffered_semantic_events(&self) -> u64 {
        self.buffered_semantic_events
    }

    /// Includes the fixed segment header while a tail is buffered.
    pub(crate) const fn buffered_bytes(&self) -> usize {
        self.buffered_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalStoreFinalized {
    flushed_segment: Option<RelationalJournalSegmentReceipt>,
    durable_tail: Option<RelationalJournalSegmentReceipt>,
    next_sequence: Option<u64>,
    terminal_head: [u8; 32],
}

impl RelationalJournalStoreFinalized {
    pub(crate) const fn flushed_segment(&self) -> Option<&RelationalJournalSegmentReceipt> {
        self.flushed_segment.as_ref()
    }

    pub(crate) const fn durable_tail(&self) -> Option<&RelationalJournalSegmentReceipt> {
        self.durable_tail.as_ref()
    }

    pub(crate) const fn next_sequence(&self) -> Option<u64> {
        self.next_sequence
    }

    pub(crate) const fn terminal_head(&self) -> [u8; 32] {
        self.terminal_head
    }
}

/// A validated segment read back exactly from the immutable store.
pub(crate) struct RawRelationalJournalSegment {
    receipt: RelationalJournalSegmentReceipt,
    bytes: Vec<u8>,
}

impl RawRelationalJournalSegment {
    pub(crate) const fn receipt(&self) -> &RelationalJournalSegmentReceipt {
        &self.receipt
    }

    pub(crate) fn frames(&self) -> RawRelationalJournalFrameIter<'_> {
        RawRelationalJournalFrameIter {
            bytes: &self.bytes,
            cursor: SEGMENT_HEADER_BYTES,
            remaining: self.receipt.physical_frame_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawRelationalJournalFrame<'a> {
    first_sequence: u64,
    last_sequence: u64,
    semantic_event_count: u64,
    previous_head: [u8; 32],
    head: [u8; 32],
    bytes: &'a [u8],
}

impl<'a> RawRelationalJournalFrame<'a> {
    pub(crate) const fn first_sequence(self) -> u64 {
        self.first_sequence
    }

    pub(crate) const fn last_sequence(self) -> u64 {
        self.last_sequence
    }

    pub(crate) const fn semantic_event_count(self) -> u64 {
        self.semantic_event_count
    }

    pub(crate) const fn previous_head(self) -> [u8; 32] {
        self.previous_head
    }

    pub(crate) const fn head(self) -> [u8; 32] {
        self.head
    }

    pub(crate) const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

pub(crate) struct RawRelationalJournalFrameIter<'a> {
    bytes: &'a [u8],
    cursor: usize,
    remaining: u64,
}

impl<'a> Iterator for RawRelationalJournalFrameIter<'a> {
    type Item = RawRelationalJournalFrame<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let prefix_end = self
            .cursor
            .checked_add(FRAME_PREFIX_BYTES)
            .expect("validated frame prefix remains in the segment");
        let prefix = &self.bytes[self.cursor..prefix_end];
        let first_sequence =
            u64::from_be_bytes(prefix[0..8].try_into().expect("fixed first-sequence bytes"));
        let last_sequence =
            u64::from_be_bytes(prefix[8..16].try_into().expect("fixed last-sequence bytes"));
        let semantic_event_count = u64::from_be_bytes(
            prefix[16..24]
                .try_into()
                .expect("fixed semantic-event-count bytes"),
        );
        let previous_head = prefix[24..56]
            .try_into()
            .expect("fixed previous-head bytes");
        let head = prefix[56..88]
            .try_into()
            .expect("fixed terminal-head bytes");
        let payload_len = u64::from_be_bytes(
            prefix[88..96]
                .try_into()
                .expect("fixed payload-length bytes"),
        );
        let payload_len = usize::try_from(payload_len)
            .expect("validated payload length is representable on this target");
        let payload_start = prefix_end;
        let payload_end = payload_start
            .checked_add(payload_len)
            .expect("validated frame payload remains in the segment");
        self.cursor = payload_end;
        self.remaining -= 1;
        Some(RawRelationalJournalFrame {
            first_sequence,
            last_sequence,
            semantic_event_count,
            previous_head,
            head,
            bytes: &self.bytes[payload_start..payload_end],
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl std::iter::FusedIterator for RawRelationalJournalFrameIter<'_> {}

pub(crate) struct RawRelationalJournalSegmentReplay<'a> {
    guard: &'a RunStoreGuard,
    limits: RelationalJournalSegmentLimits,
    segments: std::vec::IntoIter<SegmentDescriptor>,
    failed: bool,
}

impl Iterator for RawRelationalJournalSegmentReplay<'_> {
    type Item = Result<RawRelationalJournalSegment, RelationalJournalSegmentStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let descriptor = self.segments.next()?;
        let result = read_validated_segment(self.guard, self.limits, &descriptor);
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.failed {
            (0, Some(0))
        } else {
            self.segments.size_hint()
        }
    }
}

impl std::iter::FusedIterator for RawRelationalJournalSegmentReplay<'_> {}

/// Single-writer segmented storage. The retained guard is the writer lock;
/// consequently this type intentionally has no `Clone` implementation.
pub(crate) struct RelationalJournalSegmentStore {
    guard: RunStoreGuard,
    limits: RelationalJournalSegmentLimits,
    anchor: RelationalJournalStoreAnchor,
    segments: Vec<SegmentDescriptor>,
    durable_next_sequence: Option<u64>,
    next_sequence: Option<u64>,
    durable_head: [u8; 32],
    tail_head: [u8; 32],
    buffer: Vec<u8>,
    buffered: Option<BufferedSegment>,
    needs_reopen: bool,
}

impl RelationalJournalSegmentStore {
    pub(crate) fn open_or_create(
        directory: impl AsRef<Path>,
        store_limits: RunStoreLimits,
        segment_limits: RelationalJournalSegmentLimits,
        anchor: RelationalJournalStoreAnchor,
    ) -> Result<Self, RelationalJournalSegmentStoreError> {
        Self::from_guard(
            RunStoreGuard::open_or_create(directory, store_limits)?,
            segment_limits,
            anchor,
        )
    }

    pub(crate) fn open(
        directory: impl AsRef<Path>,
        store_limits: RunStoreLimits,
        segment_limits: RelationalJournalSegmentLimits,
        anchor: RelationalJournalStoreAnchor,
    ) -> Result<Self, RelationalJournalSegmentStoreError> {
        Self::from_guard(
            RunStoreGuard::open(directory, store_limits)?,
            segment_limits,
            anchor,
        )
    }

    pub(crate) fn from_guard(
        guard: RunStoreGuard,
        limits: RelationalJournalSegmentLimits,
        anchor: RelationalJournalStoreAnchor,
    ) -> Result<Self, RelationalJournalSegmentStoreError> {
        validate_store_limits(&guard, limits)?;
        let index = scan_store(&guard, limits, anchor)?;
        Ok(Self {
            guard,
            limits,
            anchor,
            segments: index.segments,
            durable_next_sequence: index.next_sequence,
            next_sequence: index.next_sequence,
            durable_head: index.terminal_head,
            tail_head: index.terminal_head,
            buffer: Vec::new(),
            buffered: None,
            needs_reopen: false,
        })
    }

    pub(crate) const fn limits(&self) -> RelationalJournalSegmentLimits {
        self.limits
    }

    pub(crate) const fn anchor(&self) -> RelationalJournalStoreAnchor {
        self.anchor
    }

    /// Includes an unflushed tail. `None` means sequence `u64::MAX` was
    /// accepted and no later frame can exist.
    pub(crate) const fn next_sequence(&self) -> Option<u64> {
        self.next_sequence
    }

    /// Durable cursor reconstructed only from fully installed segments.
    pub(crate) const fn durable_next_sequence(&self) -> Option<u64> {
        self.durable_next_sequence
    }

    pub(crate) const fn terminal_head(&self) -> [u8; 32] {
        self.tail_head
    }

    pub(crate) const fn durable_terminal_head(&self) -> [u8; 32] {
        self.durable_head
    }

    pub(crate) fn durable_tail(&self) -> Option<&RelationalJournalSegmentReceipt> {
        self.segments.last().map(|segment| &segment.receipt)
    }

    pub(crate) fn durable_segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Authenticate an exact durable segment boundary without replaying or
    /// cloning journal frames.
    ///
    /// The genesis anchor is a boundary even when no segment has been
    /// installed. Every other accepted coordinate must be the terminal head
    /// immediately after one installed segment. Coordinates within a segment,
    /// beyond the installed tail, or on another head are deliberately false.
    pub(crate) fn authenticates_durable_checkpoint(
        &self,
        next_sequence: u64,
        head: [u8; 32],
    ) -> bool {
        if next_sequence == self.anchor.first_sequence() {
            return head == self.anchor.initial_head();
        }
        let Some(last_sequence) = next_sequence.checked_sub(1) else {
            return false;
        };
        self.segments
            .binary_search_by_key(&last_sequence, |segment| segment.receipt.last_sequence())
            .ok()
            .is_some_and(|index| self.segments[index].receipt.terminal_head() == head)
    }

    pub(crate) fn buffered_physical_frames(&self) -> u64 {
        self.buffered
            .as_ref()
            .map_or(0, |segment| segment.physical_frame_count)
    }

    pub(crate) fn buffered_semantic_events(&self) -> u64 {
        self.buffered
            .as_ref()
            .map_or(0, |segment| segment.semantic_event_count)
    }

    /// At most `limits.max_segment_bytes()`, including the reserved header.
    pub(crate) fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// Buffer one opaque packed physical frame. A full preceding segment is
    /// installed before this frame is accepted; the returned receipt reports
    /// that installation. The physical envelope authenticates its complete
    /// semantic range even though this layer does not decode individual
    /// entries.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn append_physical_frame(
        &mut self,
        first_sequence: u64,
        last_sequence: u64,
        semantic_event_count: u64,
        previous_head: [u8; 32],
        head: [u8; 32],
        frame: &[u8],
    ) -> Result<RelationalJournalSegmentAppend, RelationalJournalSegmentStoreError> {
        self.require_writable()?;
        if frame.is_empty() {
            return Err(RelationalJournalSegmentStoreError::EmptyPhysicalFrame);
        }
        let encoded_frame_bytes = self.validate_frame_size(frame.len())?;
        let expected_sequence = self
            .next_sequence
            .ok_or(RelationalJournalSegmentStoreError::SequenceExhausted)?;
        if first_sequence != expected_sequence {
            return Err(if first_sequence < expected_sequence {
                RelationalJournalSegmentStoreError::SequenceFork {
                    expected: expected_sequence,
                    found: first_sequence,
                }
            } else {
                RelationalJournalSegmentStoreError::SequenceGap {
                    expected: expected_sequence,
                    found: first_sequence,
                }
            });
        }
        validate_semantic_range(first_sequence, last_sequence, semantic_event_count)?;
        if previous_head != self.tail_head {
            return Err(RelationalJournalSegmentStoreError::WrongHeadAnchor {
                sequence: first_sequence,
            });
        }

        let must_flush = self.buffered.as_ref().is_some_and(|buffered| {
            buffered.physical_frame_count == self.limits.max_frames_per_segment
                || self
                    .buffer
                    .len()
                    .checked_add(encoded_frame_bytes)
                    .is_none_or(|bytes| bytes > self.limits.max_segment_bytes)
        });
        let installed_segment = if must_flush {
            self.flush_segment()?
        } else {
            None
        };

        if self.buffered.is_none() {
            self.start_segment(first_sequence, previous_head, encoded_frame_bytes)?;
        } else {
            self.reserve_buffer(encoded_frame_bytes)?;
        }

        self.buffer.extend_from_slice(&first_sequence.to_be_bytes());
        self.buffer.extend_from_slice(&last_sequence.to_be_bytes());
        self.buffer
            .extend_from_slice(&semantic_event_count.to_be_bytes());
        self.buffer.extend_from_slice(&previous_head);
        self.buffer.extend_from_slice(&head);
        self.buffer.extend_from_slice(
            &u64::try_from(frame.len())
                .map_err(|_| RelationalJournalSegmentStoreError::FrameTooLarge {
                    bytes: u64::MAX,
                    limit: self.limits.max_frame_bytes,
                })?
                .to_be_bytes(),
        );
        self.buffer.extend_from_slice(frame);

        let buffered = self
            .buffered
            .as_mut()
            .expect("a segment is open before its first frame is appended");
        buffered.last_sequence = last_sequence;
        buffered.physical_frame_count = buffered.physical_frame_count.checked_add(1).ok_or(
            RelationalJournalSegmentStoreError::ArithmeticOverflow("buffered physical frame count"),
        )?;
        buffered.semantic_event_count = buffered
            .semantic_event_count
            .checked_add(semantic_event_count)
            .ok_or(RelationalJournalSegmentStoreError::ArithmeticOverflow(
                "buffered semantic event count",
            ))?;
        buffered.terminal_head = head;
        self.next_sequence = last_sequence.checked_add(1);
        self.tail_head = head;

        Ok(RelationalJournalSegmentAppend {
            installed_segment,
            buffered_physical_frames: buffered.physical_frame_count,
            buffered_semantic_events: buffered.semantic_event_count,
            buffered_bytes: self.buffer.len(),
        })
    }

    /// Install the current segment so a pause can publish its terminal head.
    /// An empty pause is a no-op.
    pub(crate) fn flush_for_pause(
        &mut self,
    ) -> Result<Option<RelationalJournalSegmentReceipt>, RelationalJournalSegmentStoreError> {
        self.require_writable()?;
        self.flush_segment()
    }

    /// Flush and release the writer guard. No separate mutable `HEAD` or final
    /// marker is created; the last immutable segment is the recoverable tail.
    pub(crate) fn finalize(
        mut self,
    ) -> Result<RelationalJournalStoreFinalized, RelationalJournalSegmentStoreError> {
        self.require_writable()?;
        let flushed_segment = self.flush_segment()?;
        Ok(RelationalJournalStoreFinalized {
            flushed_segment,
            durable_tail: self.segments.last().map(|segment| segment.receipt.clone()),
            next_sequence: self.durable_next_sequence,
            terminal_head: self.durable_head,
        })
    }

    /// Re-scan and strictly validate the immutable namespace. Buffered frames
    /// are intentionally excluded: replay is exactly the durable prefix.
    pub(crate) fn replay_segments(
        &self,
    ) -> Result<RawRelationalJournalSegmentReplay<'_>, RelationalJournalSegmentStoreError> {
        self.require_writable()?;
        let index = scan_store(&self.guard, self.limits, self.anchor)?;
        Ok(RawRelationalJournalSegmentReplay {
            guard: &self.guard,
            limits: self.limits,
            segments: index.segments.into_iter(),
            failed: false,
        })
    }

    fn validate_frame_size(
        &self,
        frame_bytes: usize,
    ) -> Result<usize, RelationalJournalSegmentStoreError> {
        if frame_bytes > self.limits.max_frame_bytes {
            return Err(RelationalJournalSegmentStoreError::FrameTooLarge {
                bytes: u64::try_from(frame_bytes).unwrap_or(u64::MAX),
                limit: self.limits.max_frame_bytes,
            });
        }
        let encoded = FRAME_PREFIX_BYTES.checked_add(frame_bytes).ok_or(
            RelationalJournalSegmentStoreError::ArithmeticOverflow("encoded frame length"),
        )?;
        if SEGMENT_HEADER_BYTES
            .checked_add(encoded)
            .is_none_or(|bytes| bytes > self.limits.max_segment_bytes)
        {
            return Err(RelationalJournalSegmentStoreError::FrameTooLarge {
                bytes: u64::try_from(frame_bytes).unwrap_or(u64::MAX),
                limit: self
                    .limits
                    .max_segment_bytes
                    .saturating_sub(SEGMENT_HEADER_BYTES + FRAME_PREFIX_BYTES),
            });
        }
        Ok(encoded)
    }

    fn start_segment(
        &mut self,
        first_sequence: u64,
        prior_head: [u8; 32],
        first_frame_bytes: usize,
    ) -> Result<(), RelationalJournalSegmentStoreError> {
        let ordinal = u64::try_from(self.segments.len()).map_err(|_| {
            RelationalJournalSegmentStoreError::ArithmeticOverflow("segment ordinal")
        })?;
        if self.segments.len() >= self.limits.max_segments {
            return Err(RelationalJournalSegmentStoreError::SegmentLimitExceeded {
                limit: self.limits.max_segments,
            });
        }
        let required = SEGMENT_HEADER_BYTES.checked_add(first_frame_bytes).ok_or(
            RelationalJournalSegmentStoreError::ArithmeticOverflow("initial segment allocation"),
        )?;
        self.reserve_buffer(required)?;
        self.buffer.resize(SEGMENT_HEADER_BYTES, 0);
        self.buffered = Some(BufferedSegment {
            ordinal,
            first_sequence,
            last_sequence: first_sequence,
            physical_frame_count: 0,
            semantic_event_count: 0,
            prior_segment: self.segments.last().map(|segment| segment.receipt.digest),
            prior_head,
            terminal_head: prior_head,
        });
        Ok(())
    }

    fn reserve_buffer(
        &mut self,
        additional: usize,
    ) -> Result<(), RelationalJournalSegmentStoreError> {
        let final_len = self.buffer.len().checked_add(additional).ok_or(
            RelationalJournalSegmentStoreError::ArithmeticOverflow("segment buffer length"),
        )?;
        if final_len > self.limits.max_segment_bytes {
            return Err(RelationalJournalSegmentStoreError::SegmentTooLarge {
                bytes: u64::try_from(final_len).unwrap_or(u64::MAX),
                limit: self.limits.max_segment_bytes,
            });
        }
        self.buffer.try_reserve_exact(additional).map_err(|_| {
            RelationalJournalSegmentStoreError::BufferAllocationFailed {
                requested: final_len,
            }
        })
    }

    fn flush_segment(
        &mut self,
    ) -> Result<Option<RelationalJournalSegmentReceipt>, RelationalJournalSegmentStoreError> {
        let Some(buffered) = self.buffered else {
            debug_assert!(self.buffer.is_empty());
            return Ok(None);
        };
        debug_assert!(buffered.physical_frame_count > 0);
        debug_assert!(buffered.semantic_event_count > 0);

        let frame_bytes = &self.buffer[SEGMENT_HEADER_BYTES..];
        let frame_digest: [u8; 32] = Sha256::digest(frame_bytes).into();
        let header = encode_segment_header(
            buffered,
            u64::try_from(frame_bytes.len()).map_err(|_| {
                RelationalJournalSegmentStoreError::ArithmeticOverflow("segment frame bytes")
            })?,
            frame_digest,
        );
        self.buffer[..SEGMENT_HEADER_BYTES].copy_from_slice(&header);
        let digest = RelationalJournalSegmentDigest(Sha256::digest(&self.buffer).into());
        let name = segment_entry_name(
            buffered.ordinal,
            buffered.first_sequence,
            buffered.last_sequence,
            digest,
        )?;

        let installed = self
            .guard
            .install_immutable(&name, &self.buffer)
            .and_then(|_| self.guard.read_entry(&name));
        let readback = match installed {
            Ok(readback) if readback == self.buffer => readback,
            Ok(_) => {
                self.needs_reopen = true;
                return Err(RelationalJournalSegmentStoreError::ExactReadbackMismatch(
                    name,
                ));
            }
            Err(error) => {
                self.needs_reopen = true;
                return Err(error.into());
            }
        };

        let name_fields = SegmentName {
            ordinal: buffered.ordinal,
            first_sequence: buffered.first_sequence,
            last_sequence: buffered.last_sequence,
            digest,
        };
        let receipt = match parse_segment(&name, &readback, self.limits, name_fields) {
            Ok(receipt) => receipt,
            Err(error) => {
                // The final immutable name is already visible. Even an
                // encoder/parser disagreement must force a fresh namespace
                // scan before another write can trust the in-memory tail.
                self.needs_reopen = true;
                return Err(error);
            }
        };
        let descriptor = SegmentDescriptor {
            receipt: receipt.clone(),
            name: name.into_boxed_str(),
        };
        self.segments.push(descriptor);
        self.durable_next_sequence = self.next_sequence;
        self.durable_head = buffered.terminal_head;
        self.buffer.clear();
        self.buffered = None;
        Ok(Some(receipt))
    }

    fn require_writable(&self) -> Result<(), RelationalJournalSegmentStoreError> {
        if self.needs_reopen {
            Err(RelationalJournalSegmentStoreError::NeedsReopen)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BufferedSegment {
    ordinal: u64,
    first_sequence: u64,
    last_sequence: u64,
    physical_frame_count: u64,
    semantic_event_count: u64,
    prior_segment: Option<RelationalJournalSegmentDigest>,
    prior_head: [u8; 32],
    terminal_head: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SegmentDescriptor {
    receipt: RelationalJournalSegmentReceipt,
    name: Box<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SegmentName {
    ordinal: u64,
    first_sequence: u64,
    last_sequence: u64,
    digest: RelationalJournalSegmentDigest,
}

struct StoreIndex {
    segments: Vec<SegmentDescriptor>,
    next_sequence: Option<u64>,
    terminal_head: [u8; 32],
}

fn validate_store_limits(
    guard: &RunStoreGuard,
    limits: RelationalJournalSegmentLimits,
) -> Result<(), RelationalJournalSegmentStoreError> {
    if u64::try_from(limits.max_segment_bytes).unwrap_or(u64::MAX)
        > guard.limits().max_entry_bytes()
        || limits.max_segments > guard.limits().max_list_entries()
        || u64::try_from(limits.max_segment_bytes).unwrap_or(u64::MAX) > RUN_STORE_MAX_ENTRY_BYTES
        || limits.max_segments > RUN_STORE_MAX_LIST_ENTRIES
    {
        return Err(RelationalJournalSegmentStoreError::InvalidLimits(
            "segment limits exceed the retained RunStoreGuard allocation bounds",
        ));
    }
    Ok(())
}

fn scan_store(
    guard: &RunStoreGuard,
    limits: RelationalJournalSegmentLimits,
    anchor: RelationalJournalStoreAnchor,
) -> Result<StoreIndex, RelationalJournalSegmentStoreError> {
    let mut segments = Vec::new();
    for entry in guard.list_entries()? {
        let name = entry.name();
        if let Some(family) = legacy_run_state_family(name) {
            return Err(
                RelationalJournalSegmentStoreError::UnsupportedLegacyRunState {
                    family,
                    entry: name.to_owned(),
                },
            );
        }
        if !name.starts_with(SEGMENT_FAMILY) {
            continue;
        }
        if segments.len() >= limits.max_segments {
            return Err(RelationalJournalSegmentStoreError::SegmentLimitExceeded {
                limit: limits.max_segments,
            });
        }
        if entry.bytes() > u64::try_from(limits.max_segment_bytes).unwrap_or(u64::MAX) {
            return Err(RelationalJournalSegmentStoreError::SegmentTooLarge {
                bytes: entry.bytes(),
                limit: limits.max_segment_bytes,
            });
        }
        let name_fields = parse_segment_entry_name(name)?;
        let bytes = guard.read_entry(name)?;
        let receipt = parse_segment(name, &bytes, limits, name_fields)?;
        segments.push(SegmentDescriptor {
            receipt,
            name: name.into(),
        });
    }

    segments.sort_unstable_by(|left, right| {
        left.receipt
            .ordinal
            .cmp(&right.receipt.ordinal)
            .then_with(|| left.receipt.digest.cmp(&right.receipt.digest))
    });

    let mut expected_ordinal = 0_u64;
    let mut expected_sequence = Some(anchor.first_sequence);
    let mut expected_prior_segment = None;
    let mut expected_head = anchor.initial_head;
    let mut previous: Option<&SegmentDescriptor> = None;

    for segment in &segments {
        if let Some(previous) = previous {
            if segment.receipt.ordinal == previous.receipt.ordinal {
                return Err(RelationalJournalSegmentStoreError::SegmentFork {
                    ordinal: segment.receipt.ordinal,
                });
            }
        }
        if segment.receipt.ordinal != expected_ordinal {
            return Err(RelationalJournalSegmentStoreError::SegmentOrdinalGap {
                expected: expected_ordinal,
                found: segment.receipt.ordinal,
            });
        }
        let expected_first =
            expected_sequence.ok_or(RelationalJournalSegmentStoreError::SequenceExhausted)?;
        if segment.receipt.first_sequence != expected_first {
            return Err(if segment.receipt.first_sequence < expected_first {
                RelationalJournalSegmentStoreError::SequenceFork {
                    expected: expected_first,
                    found: segment.receipt.first_sequence,
                }
            } else {
                RelationalJournalSegmentStoreError::SequenceGap {
                    expected: expected_first,
                    found: segment.receipt.first_sequence,
                }
            });
        }
        if segment.receipt.prior_segment != expected_prior_segment {
            return Err(RelationalJournalSegmentStoreError::WrongPriorSegment {
                ordinal: segment.receipt.ordinal,
            });
        }
        if segment.receipt.prior_head != expected_head {
            return Err(RelationalJournalSegmentStoreError::WrongHeadAnchor {
                sequence: segment.receipt.first_sequence,
            });
        }

        expected_ordinal = expected_ordinal.checked_add(1).ok_or(
            RelationalJournalSegmentStoreError::ArithmeticOverflow("next segment ordinal"),
        )?;
        expected_sequence = segment.receipt.last_sequence.checked_add(1);
        expected_prior_segment = Some(segment.receipt.digest);
        expected_head = segment.receipt.terminal_head;
        previous = Some(segment);
    }

    Ok(StoreIndex {
        segments,
        next_sequence: expected_sequence,
        terminal_head: expected_head,
    })
}

fn legacy_run_state_family(name: &str) -> Option<&'static str> {
    [
        LEGACY_RUN_OPENED_FAMILY,
        LEGACY_FENCE_V1_FAMILY,
        LEGACY_BLOB_V1_FAMILY,
        LEGACY_EVENT_V1_FAMILY,
    ]
    .into_iter()
    .find(|family| name.starts_with(*family))
}

fn read_validated_segment(
    guard: &RunStoreGuard,
    limits: RelationalJournalSegmentLimits,
    descriptor: &SegmentDescriptor,
) -> Result<RawRelationalJournalSegment, RelationalJournalSegmentStoreError> {
    let bytes = guard.read_entry(&descriptor.name)?;
    if bytes.len() > limits.max_segment_bytes {
        return Err(RelationalJournalSegmentStoreError::SegmentTooLarge {
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            limit: limits.max_segment_bytes,
        });
    }
    let name_fields = parse_segment_entry_name(&descriptor.name)?;
    let receipt = parse_segment(&descriptor.name, &bytes, limits, name_fields)?;
    if receipt != descriptor.receipt {
        return Err(RelationalJournalSegmentStoreError::SegmentChanged(
            descriptor.name.to_string(),
        ));
    }
    Ok(RawRelationalJournalSegment { receipt, bytes })
}

fn parse_segment(
    name: &str,
    bytes: &[u8],
    limits: RelationalJournalSegmentLimits,
    name_fields: SegmentName,
) -> Result<RelationalJournalSegmentReceipt, RelationalJournalSegmentStoreError> {
    if bytes.len() < SEGMENT_HEADER_BYTES {
        return Err(malformed_segment(
            name,
            "segment is shorter than its fixed header",
        ));
    }
    if bytes.len() > limits.max_segment_bytes {
        return Err(RelationalJournalSegmentStoreError::SegmentTooLarge {
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            limit: limits.max_segment_bytes,
        });
    }
    let actual_digest = RelationalJournalSegmentDigest(Sha256::digest(bytes).into());
    if actual_digest != name_fields.digest {
        return Err(RelationalJournalSegmentStoreError::SegmentDigestMismatch {
            name: name.to_owned(),
            expected: name_fields.digest,
            actual: actual_digest,
        });
    }

    let mut reader = SegmentReader::new(name, bytes);
    if reader.take(SEGMENT_MAGIC.len())? != SEGMENT_MAGIC {
        return Err(malformed_segment(name, "segment magic is not canonical"));
    }
    let schema = reader.u32()?;
    if schema != RELATIONAL_JOURNAL_SEGMENT_SCHEMA_VERSION {
        return Err(RelationalJournalSegmentStoreError::UnsupportedSchema {
            actual: schema,
            expected: RELATIONAL_JOURNAL_SEGMENT_SCHEMA_VERSION,
        });
    }
    if usize::try_from(reader.u32()?).ok() != Some(SEGMENT_HEADER_BYTES) {
        return Err(malformed_segment(
            name,
            "segment header length is not canonical",
        ));
    }
    let ordinal = reader.u64()?;
    let first_sequence = reader.u64()?;
    let last_sequence = reader.u64()?;
    let physical_frame_count = reader.u64()?;
    let semantic_event_count = reader.u64()?;
    let frame_bytes_len = reader.u64()?;
    let flags = reader.u32()?;
    let reserved = reader.u32()?;
    let prior_digest_bytes = reader.digest()?;
    let prior_head = reader.digest()?;
    let terminal_head = reader.digest()?;
    let claimed_frame_digest = reader.digest()?;
    if reader.position != SEGMENT_HEADER_BYTES {
        return Err(malformed_segment(name, "segment header size drifted"));
    }
    if flags & !SEGMENT_KNOWN_FLAGS != 0 || reserved != 0 {
        return Err(malformed_segment(
            name,
            "segment flags or reserved bytes are noncanonical",
        ));
    }
    let prior_segment = if flags & SEGMENT_FLAG_HAS_PRIOR != 0 {
        Some(RelationalJournalSegmentDigest(prior_digest_bytes))
    } else {
        if prior_digest_bytes != [0; 32] {
            return Err(malformed_segment(
                name,
                "absent prior segment must use the zero digest",
            ));
        }
        None
    };
    if ordinal != name_fields.ordinal
        || first_sequence != name_fields.first_sequence
        || last_sequence != name_fields.last_sequence
    {
        return Err(malformed_segment(name, "segment name and header disagree"));
    }
    if physical_frame_count == 0 || physical_frame_count > limits.max_frames_per_segment {
        return Err(malformed_segment(
            name,
            "segment physical frame count is outside its bound",
        ));
    }
    let expected_count = last_sequence
        .checked_sub(first_sequence)
        .and_then(|difference| difference.checked_add(1))
        .ok_or_else(|| malformed_segment(name, "segment sequence range is invalid"))?;
    if semantic_event_count != expected_count {
        return Err(malformed_segment(
            name,
            "segment range does not equal its semantic event count",
        ));
    }
    let actual_frame_bytes = bytes.len() - SEGMENT_HEADER_BYTES;
    if usize::try_from(frame_bytes_len).ok() != Some(actual_frame_bytes) {
        return Err(malformed_segment(
            name,
            "segment frame-byte length is invalid",
        ));
    }
    let frame_digest: [u8; 32] = Sha256::digest(&bytes[SEGMENT_HEADER_BYTES..]).into();
    if frame_digest != claimed_frame_digest {
        return Err(malformed_segment(
            name,
            "segment frame digest does not match",
        ));
    }

    let mut cursor = SEGMENT_HEADER_BYTES;
    let mut expected_sequence = first_sequence;
    let mut expected_head = prior_head;
    let mut observed_semantic_events = 0_u64;
    let mut observed_last_sequence = None;
    for frame_index in 0..physical_frame_count {
        let prefix_end = cursor
            .checked_add(FRAME_PREFIX_BYTES)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed_segment(name, "truncated frame prefix"))?;
        let prefix = &bytes[cursor..prefix_end];
        let frame_first_sequence =
            u64::from_be_bytes(prefix[0..8].try_into().expect("fixed first-sequence bytes"));
        let frame_last_sequence =
            u64::from_be_bytes(prefix[8..16].try_into().expect("fixed last-sequence bytes"));
        let frame_semantic_event_count = u64::from_be_bytes(
            prefix[16..24]
                .try_into()
                .expect("fixed semantic-event-count bytes"),
        );
        let previous_head: [u8; 32] = prefix[24..56]
            .try_into()
            .expect("fixed previous-head bytes");
        let head: [u8; 32] = prefix[56..88]
            .try_into()
            .expect("fixed terminal-head bytes");
        let payload_len = u64::from_be_bytes(
            prefix[88..96]
                .try_into()
                .expect("fixed payload-length bytes"),
        );
        if frame_first_sequence != expected_sequence {
            return Err(if frame_first_sequence < expected_sequence {
                RelationalJournalSegmentStoreError::SequenceFork {
                    expected: expected_sequence,
                    found: frame_first_sequence,
                }
            } else {
                RelationalJournalSegmentStoreError::SequenceGap {
                    expected: expected_sequence,
                    found: frame_first_sequence,
                }
            });
        }
        if semantic_range_count(frame_first_sequence, frame_last_sequence)
            != Some(frame_semantic_event_count)
        {
            return Err(malformed_segment(
                name,
                "physical frame range does not equal its semantic event count",
            ));
        }
        if previous_head != expected_head {
            return Err(RelationalJournalSegmentStoreError::WrongHeadAnchor {
                sequence: frame_first_sequence,
            });
        }
        let payload_len = usize::try_from(payload_len)
            .map_err(|_| malformed_segment(name, "frame payload length is not representable"))?;
        if payload_len > limits.max_frame_bytes {
            return Err(RelationalJournalSegmentStoreError::FrameTooLarge {
                bytes: u64::try_from(payload_len).unwrap_or(u64::MAX),
                limit: limits.max_frame_bytes,
            });
        }
        cursor = prefix_end
            .checked_add(payload_len)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed_segment(name, "truncated frame payload"))?;
        if payload_len == 0 {
            return Err(malformed_segment(
                name,
                "packed physical frame payload is empty",
            ));
        }
        observed_semantic_events = observed_semantic_events
            .checked_add(frame_semantic_event_count)
            .ok_or(RelationalJournalSegmentStoreError::ArithmeticOverflow(
                "semantic events inside segment",
            ))?;
        observed_last_sequence = Some(frame_last_sequence);
        expected_head = head;
        if frame_index + 1 < physical_frame_count {
            expected_sequence = frame_last_sequence.checked_add(1).ok_or(
                RelationalJournalSegmentStoreError::ArithmeticOverflow(
                    "physical frame sequence inside segment",
                ),
            )?;
        }
    }
    if cursor != bytes.len() {
        return Err(RelationalJournalSegmentStoreError::TrailingBytes {
            name: name.to_owned(),
            bytes: bytes.len() - cursor,
        });
    }
    if expected_head != terminal_head {
        return Err(malformed_segment(
            name,
            "terminal journal head does not match last frame",
        ));
    }
    if observed_semantic_events != semantic_event_count
        || observed_last_sequence != Some(last_sequence)
    {
        return Err(malformed_segment(
            name,
            "physical frames do not cover the segment semantic range",
        ));
    }

    Ok(RelationalJournalSegmentReceipt {
        ordinal,
        first_sequence,
        last_sequence,
        physical_frame_count,
        semantic_event_count,
        prior_segment,
        prior_head,
        terminal_head,
        digest: actual_digest,
        bytes: u64::try_from(bytes.len()).map_err(|_| {
            RelationalJournalSegmentStoreError::ArithmeticOverflow("segment byte length")
        })?,
    })
}

fn encode_segment_header(
    buffered: BufferedSegment,
    frame_bytes_len: u64,
    frame_digest: [u8; 32],
) -> [u8; SEGMENT_HEADER_BYTES] {
    let mut header = [0_u8; SEGMENT_HEADER_BYTES];
    let mut cursor = 0;
    put_bytes(&mut header, &mut cursor, SEGMENT_MAGIC);
    put_bytes(
        &mut header,
        &mut cursor,
        &RELATIONAL_JOURNAL_SEGMENT_SCHEMA_VERSION.to_be_bytes(),
    );
    put_bytes(
        &mut header,
        &mut cursor,
        &(SEGMENT_HEADER_BYTES as u32).to_be_bytes(),
    );
    put_bytes(&mut header, &mut cursor, &buffered.ordinal.to_be_bytes());
    put_bytes(
        &mut header,
        &mut cursor,
        &buffered.first_sequence.to_be_bytes(),
    );
    put_bytes(
        &mut header,
        &mut cursor,
        &buffered.last_sequence.to_be_bytes(),
    );
    put_bytes(
        &mut header,
        &mut cursor,
        &buffered.physical_frame_count.to_be_bytes(),
    );
    put_bytes(
        &mut header,
        &mut cursor,
        &buffered.semantic_event_count.to_be_bytes(),
    );
    put_bytes(&mut header, &mut cursor, &frame_bytes_len.to_be_bytes());
    let flags = if buffered.prior_segment.is_some() {
        SEGMENT_FLAG_HAS_PRIOR
    } else {
        0
    };
    put_bytes(&mut header, &mut cursor, &flags.to_be_bytes());
    put_bytes(&mut header, &mut cursor, &0_u32.to_be_bytes());
    put_bytes(
        &mut header,
        &mut cursor,
        &buffered
            .prior_segment
            .map_or([0; 32], RelationalJournalSegmentDigest::bytes),
    );
    put_bytes(&mut header, &mut cursor, &buffered.prior_head);
    put_bytes(&mut header, &mut cursor, &buffered.terminal_head);
    put_bytes(&mut header, &mut cursor, &frame_digest);
    debug_assert_eq!(cursor, SEGMENT_HEADER_BYTES);
    header
}

fn put_bytes<const N: usize>(target: &mut [u8; N], cursor: &mut usize, value: &[u8]) {
    let end = *cursor + value.len();
    target[*cursor..end].copy_from_slice(value);
    *cursor = end;
}

struct SegmentReader<'a> {
    name: &'a str,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> SegmentReader<'a> {
    const fn new(name: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            name,
            bytes,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RelationalJournalSegmentStoreError> {
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| malformed_segment(self.name, "truncated segment header"))?;
        let value = &self.bytes[self.position..end];
        self.position = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, RelationalJournalSegmentStoreError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().expect("fixed u32 bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, RelationalJournalSegmentStoreError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("fixed u64 bytes"),
        ))
    }

    fn digest(&mut self) -> Result<[u8; 32], RelationalJournalSegmentStoreError> {
        Ok(self.take(32)?.try_into().expect("fixed digest bytes"))
    }
}

fn segment_entry_name(
    ordinal: u64,
    first_sequence: u64,
    last_sequence: u64,
    digest: RelationalJournalSegmentDigest,
) -> Result<String, RelationalJournalSegmentStoreError> {
    let name = format!(
        "{SEGMENT_PREFIX}{ordinal:016x}-{first_sequence:016x}-{last_sequence:016x}-{}",
        sha256_hex(digest.bytes())
    );
    if name.len() > RUN_STORE_MAX_NAME_BYTES {
        return Err(RelationalJournalSegmentStoreError::MalformedRecognizedName(
            name,
        ));
    }
    Ok(name)
}

fn parse_segment_entry_name(name: &str) -> Result<SegmentName, RelationalJournalSegmentStoreError> {
    let malformed = || RelationalJournalSegmentStoreError::MalformedRecognizedName(name.to_owned());
    let rest = name.strip_prefix(SEGMENT_PREFIX).ok_or_else(malformed)?;
    let mut fields = rest.split('-');
    let ordinal = parse_canonical_u64_hex(
        fields.next().ok_or_else(malformed)?,
        SEGMENT_ORDINAL_HEX_BYTES,
    )
    .ok_or_else(malformed)?;
    let first_sequence = parse_canonical_u64_hex(
        fields.next().ok_or_else(malformed)?,
        SEGMENT_SEQUENCE_HEX_BYTES,
    )
    .ok_or_else(malformed)?;
    let last_sequence = parse_canonical_u64_hex(
        fields.next().ok_or_else(malformed)?,
        SEGMENT_SEQUENCE_HEX_BYTES,
    )
    .ok_or_else(malformed)?;
    let digest = parse_sha256_hex(fields.next().ok_or_else(malformed)?).ok_or_else(malformed)?;
    if fields.next().is_some() {
        return Err(malformed());
    }
    let parsed = SegmentName {
        ordinal,
        first_sequence,
        last_sequence,
        digest: RelationalJournalSegmentDigest(digest),
    };
    if segment_entry_name(ordinal, first_sequence, last_sequence, parsed.digest)? != name {
        return Err(malformed());
    }
    Ok(parsed)
}

fn parse_canonical_u64_hex(value: &str, width: usize) -> Option<u64> {
    if value.len() != width
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    let parsed = u64::from_str_radix(value, 16).ok()?;
    (format!("{parsed:0width$x}") == value).then_some(parsed)
}

fn parse_sha256_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != SHA256_HEX_BYTES {
        return None;
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        digest[index] = (high << 4) | low;
    }
    Some(digest)
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn sha256_hex(digest: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(SHA256_HEX_BYTES);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn semantic_range_count(first_sequence: u64, last_sequence: u64) -> Option<u64> {
    last_sequence
        .checked_sub(first_sequence)
        .and_then(|difference| difference.checked_add(1))
}

fn validate_semantic_range(
    first_sequence: u64,
    last_sequence: u64,
    semantic_event_count: u64,
) -> Result<(), RelationalJournalSegmentStoreError> {
    if semantic_range_count(first_sequence, last_sequence) != Some(semantic_event_count) {
        return Err(RelationalJournalSegmentStoreError::InvalidSemanticRange {
            first_sequence,
            last_sequence,
            semantic_event_count,
        });
    }
    Ok(())
}

fn malformed_segment(name: &str, reason: &'static str) -> RelationalJournalSegmentStoreError {
    RelationalJournalSegmentStoreError::MalformedSegment {
        name: name.to_owned(),
        reason,
    }
}

#[derive(Debug)]
pub(crate) enum RelationalJournalSegmentStoreError {
    Store(RunStoreError),
    InvalidLimits(&'static str),
    UnsupportedLegacyRunState {
        family: &'static str,
        entry: String,
    },
    UnsupportedSchema {
        actual: u32,
        expected: u32,
    },
    MalformedRecognizedName(String),
    MalformedSegment {
        name: String,
        reason: &'static str,
    },
    SegmentDigestMismatch {
        name: String,
        expected: RelationalJournalSegmentDigest,
        actual: RelationalJournalSegmentDigest,
    },
    SegmentChanged(String),
    ExactReadbackMismatch(String),
    SegmentTooLarge {
        bytes: u64,
        limit: usize,
    },
    FrameTooLarge {
        bytes: u64,
        limit: usize,
    },
    EmptyPhysicalFrame,
    InvalidSemanticRange {
        first_sequence: u64,
        last_sequence: u64,
        semantic_event_count: u64,
    },
    SegmentLimitExceeded {
        limit: usize,
    },
    SegmentOrdinalGap {
        expected: u64,
        found: u64,
    },
    SegmentFork {
        ordinal: u64,
    },
    SequenceGap {
        expected: u64,
        found: u64,
    },
    SequenceFork {
        expected: u64,
        found: u64,
    },
    SequenceExhausted,
    WrongPriorSegment {
        ordinal: u64,
    },
    WrongHeadAnchor {
        sequence: u64,
    },
    TrailingBytes {
        name: String,
        bytes: usize,
    },
    ArithmeticOverflow(&'static str),
    BufferAllocationFailed {
        requested: usize,
    },
    NeedsReopen,
}

impl From<RunStoreError> for RelationalJournalSegmentStoreError {
    fn from(error: RunStoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for RelationalJournalSegmentStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(formatter),
            Self::InvalidLimits(reason) => {
                write!(formatter, "invalid relational journal segment limits: {reason}")
            }
            Self::UnsupportedLegacyRunState { family, entry } => write!(
                formatter,
                "unsupported legacy Explore run-state namespace {family:?} at entry {entry:?}; use a fresh --run-state directory"
            ),
            Self::UnsupportedSchema { actual, expected } => write!(
                formatter,
                "unsupported relational journal segment schema {actual}; expected {expected}"
            ),
            Self::MalformedRecognizedName(name) => write!(
                formatter,
                "malformed entry in the relational journal segment namespace: {name:?}"
            ),
            Self::MalformedSegment { name, reason } => {
                write!(formatter, "malformed relational journal segment {name:?}: {reason}")
            }
            Self::SegmentDigestMismatch { name, .. } => write!(
                formatter,
                "relational journal segment {name:?} does not match its SHA-256 name"
            ),
            Self::SegmentChanged(name) => write!(
                formatter,
                "relational journal segment {name:?} changed after validated discovery"
            ),
            Self::ExactReadbackMismatch(name) => write!(
                formatter,
                "relational journal segment {name:?} did not read back exactly after installation"
            ),
            Self::SegmentTooLarge { bytes, limit } => write!(
                formatter,
                "relational journal segment has {bytes} bytes, exceeding bound {limit}"
            ),
            Self::FrameTooLarge { bytes, limit } => write!(
                formatter,
                "relational journal frame has {bytes} bytes, exceeding bound {limit}"
            ),
            Self::EmptyPhysicalFrame => {
                formatter.write_str("relational journal physical frame payload is empty")
            }
            Self::InvalidSemanticRange {
                first_sequence,
                last_sequence,
                semantic_event_count,
            } => write!(
                formatter,
                "relational journal physical frame range {first_sequence}..={last_sequence} does not contain its claimed {semantic_event_count} semantic events"
            ),
            Self::SegmentLimitExceeded { limit } => write!(
                formatter,
                "relational journal segment count exceeds bound {limit}"
            ),
            Self::SegmentOrdinalGap { expected, found } => write!(
                formatter,
                "relational journal segment ordinal gap: expected {expected}, found {found}"
            ),
            Self::SegmentFork { ordinal } => {
                write!(formatter, "relational journal segment ordinal {ordinal} forks")
            }
            Self::SequenceGap { expected, found } => write!(
                formatter,
                "relational journal sequence gap: expected {expected}, found {found}"
            ),
            Self::SequenceFork { expected, found } => write!(
                formatter,
                "relational journal sequence fork: expected {expected}, found {found}"
            ),
            Self::SequenceExhausted => formatter.write_str(
                "relational journal sequence space is exhausted after u64::MAX",
            ),
            Self::WrongPriorSegment { ordinal } => write!(
                formatter,
                "relational journal segment {ordinal} names the wrong prior segment"
            ),
            Self::WrongHeadAnchor { sequence } => write!(
                formatter,
                "relational journal frame {sequence} names the wrong prior journal head"
            ),
            Self::TrailingBytes { name, bytes } => write!(
                formatter,
                "relational journal segment {name:?} has {bytes} trailing bytes"
            ),
            Self::ArithmeticOverflow(component) => {
                write!(formatter, "relational journal {component} overflow")
            }
            Self::BufferAllocationFailed { requested } => write!(
                formatter,
                "could not allocate bounded relational journal buffer of {requested} bytes"
            ),
            Self::NeedsReopen => formatter.write_str(
                "relational journal segment installation had an uncertain outcome; reopen and rescan before writing",
            ),
        }
    }
}

#[cfg(all(
    test,
    any(
        all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    )
))]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "futuruna-relational-journal-legacy-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).expect("create journal-store test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn legacy_probe_era_namespaces_fail_closed_without_data_entry_mutation() {
        let temp = TestDirectory::new();
        for (ordinal, (entry_name, expected_family)) in [
            ("run-opened-v1", LEGACY_RUN_OPENED_FAMILY),
            ("run-opened-malformed", LEGACY_RUN_OPENED_FAMILY),
            ("fence-v1-legacy", LEGACY_FENCE_V1_FAMILY),
            ("fence-v1malformed", LEGACY_FENCE_V1_FAMILY),
            ("blob-v1-legacy", LEGACY_BLOB_V1_FAMILY),
            ("blob-v1malformed", LEGACY_BLOB_V1_FAMILY),
            ("event-v1-legacy", LEGACY_EVENT_V1_FAMILY),
            ("event-v1malformed", LEGACY_EVENT_V1_FAMILY),
        ]
        .into_iter()
        .enumerate()
        {
            let directory = temp.path().join(format!("case-{ordinal}"));
            let guard = RunStoreGuard::open_or_create(&directory, RunStoreLimits::default())
                .expect("create legacy run-state fixture");
            guard
                .install_immutable(entry_name, b"legacy-run-state")
                .expect("install legacy run-state entry");
            let entries_before = guard.list_entries().expect("list legacy fixture");
            drop(guard);

            let error = match RelationalJournalSegmentStore::open_or_create(
                &directory,
                RunStoreLimits::default(),
                RelationalJournalSegmentLimits::default(),
                RelationalJournalStoreAnchor::new(0, [0x5a; 32]),
            ) {
                Err(error) => error,
                Ok(_) => panic!("legacy run-state namespace was accepted: {entry_name}"),
            };
            assert!(matches!(
                &error,
                RelationalJournalSegmentStoreError::UnsupportedLegacyRunState {
                    family,
                    entry,
                } if *family == expected_family && entry == entry_name
            ));

            let guard = RunStoreGuard::open(&directory, RunStoreLimits::default())
                .expect("reopen rejected legacy fixture");
            assert_eq!(
                guard.list_entries().expect("list rejected legacy fixture"),
                entries_before,
                "rejecting {entry_name} changed the run-state data namespace"
            );
        }

        let unrelated_directory = temp.path().join("unrelated-entry");
        let guard = RunStoreGuard::open_or_create(&unrelated_directory, RunStoreLimits::default())
            .expect("create unrelated-entry fixture");
        guard
            .install_immutable("operator-note-v2", b"not a Futuruna run-state artifact")
            .expect("install unrelated entry");
        let unrelated_entries = guard.list_entries().expect("list unrelated fixture");
        drop(guard);

        let store = RelationalJournalSegmentStore::open_or_create(
            &unrelated_directory,
            RunStoreLimits::default(),
            RelationalJournalSegmentLimits::default(),
            RelationalJournalStoreAnchor::new(0, [0x5a; 32]),
        )
        .expect("unrelated entries must not be mistaken for legacy run state");
        assert_eq!(store.durable_segment_count(), 0);
        drop(store);
        let guard = RunStoreGuard::open(&unrelated_directory, RunStoreLimits::default())
            .expect("reopen unrelated-entry fixture");
        assert_eq!(
            guard.list_entries().expect("relist unrelated fixture"),
            unrelated_entries
        );
    }
}

impl Error for RelationalJournalSegmentStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}
