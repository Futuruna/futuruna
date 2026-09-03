//! Crash-prefix-safe ownership of a streaming relational journal and its
//! immutable segmented storage.
//!
//! The semantic fold may run ahead of the last installed segment while the
//! store buffers a bounded tail. That tail is deliberately provisional: only
//! [`RelationalDurableJournal::flush_for_pause`] turns it into a publishable
//! cursor. If semantic application succeeds but encoding or installation
//! fails, this coordinator is poisoned. It exposes neither the advanced fold
//! nor another append; the caller must drop it and reopen the last immutable
//! prefix.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use super::relational_analysis_plan::RelationalAnalysisPlanRoot;
use super::relational_journal::{
    RelationalEvidenceEvent, RelationalJournal, RelationalJournalContract, RelationalJournalError,
    RelationalJournalEvent, RelationalJournalHead,
};
use super::relational_journal_codec::{
    decode_relational_journal_entry, encode_relational_journal_entry, RelationalJournalCodecError,
    RelationalJournalCodecLimits, RelationalJournalPackedFrameBuilder,
    RelationalJournalPackedFrameReader, RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES,
};
use super::relational_journal_store::{
    RelationalJournalSegmentLimits, RelationalJournalSegmentReceipt, RelationalJournalSegmentStore,
    RelationalJournalSegmentStoreError, RelationalJournalStoreAnchor,
    RelationalJournalStoreFinalized,
};
use super::relational_region_proof::RelationalRegionReplayAuthority;
use super::relational_result_publication::{
    RelationalPublicationAuthority, RelationalPublicationCheckpoint,
};
use super::run_store::RunStoreLimits;

/// Cross-layer allocation limits whose construction proves that every
/// canonical codec entry can fit one segment frame. Operational schedulers may
/// use smaller batch bounds without changing this durable format contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalDurableJournalLimits {
    run_store: RunStoreLimits,
    segment: RelationalJournalSegmentLimits,
    codec: RelationalJournalCodecLimits,
}

impl RelationalDurableJournalLimits {
    pub(crate) fn new(
        run_store: RunStoreLimits,
        segment: RelationalJournalSegmentLimits,
        codec: RelationalJournalCodecLimits,
    ) -> Result<Self, RelationalDurableJournalError> {
        validate_cross_layer_limits(segment, codec)?;
        Ok(Self {
            run_store,
            segment,
            codec,
        })
    }

    pub(crate) const fn run_store(self) -> RunStoreLimits {
        self.run_store
    }

    pub(crate) const fn segment(self) -> RelationalJournalSegmentLimits {
        self.segment
    }

    pub(crate) const fn codec(self) -> RelationalJournalCodecLimits {
        self.codec
    }
}

impl Default for RelationalDurableJournalLimits {
    fn default() -> Self {
        let segment = RelationalJournalSegmentLimits::default();
        let codec = RelationalJournalCodecLimits::new(
            segment
                .max_frame_bytes()
                .checked_sub(RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES)
                .expect("the default physical frame fits a packed-entry prefix"),
            512 << 10,
            256 << 10,
            65_536,
            64,
            131_072,
        )
        .expect("the durable journal default codec limits fit its default segment frame");
        Self::new(RunStoreLimits::default(), segment, codec)
            .expect("the durable journal default limits agree across layers")
    }
}

/// Provisional progress from one successfully buffered semantic batch. The
/// semantic and physical counts are deliberately distinct: only the former is
/// exploration progress; the latter describes storage work.
///
/// `durable_*` can lag `next_sequence`/`head` until a pause flush. Callers may
/// use the latter for continued in-process planning, but must publish only a
/// [`RelationalDurableCheckpoint`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalDurableAppend {
    first_sequence: u64,
    semantic_event_count: NonZeroU64,
    physical_frame_count: NonZeroU64,
    next_sequence: u64,
    head: RelationalJournalHead,
    installed_segment_count: u64,
    durable_next_sequence: u64,
    durable_head: RelationalJournalHead,
}

impl RelationalDurableAppend {
    pub(crate) const fn first_sequence(&self) -> u64 {
        self.first_sequence
    }

    pub(crate) const fn semantic_event_count(&self) -> NonZeroU64 {
        self.semantic_event_count
    }

    /// Count of bounded physical storage frames produced for this semantic
    /// batch. This is deliberately separate from semantic progress.
    pub(crate) const fn physical_frame_count(&self) -> NonZeroU64 {
        self.physical_frame_count
    }

    pub(crate) const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) const fn head(&self) -> RelationalJournalHead {
        self.head
    }

    pub(crate) const fn installed_segment_count(&self) -> u64 {
        self.installed_segment_count
    }

    pub(crate) const fn durable_next_sequence(&self) -> u64 {
        self.durable_next_sequence
    }

    pub(crate) const fn durable_head(&self) -> RelationalJournalHead {
        self.durable_head
    }
}

/// The only journal cursor that may be exposed as resumable progress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalDurableCheckpoint {
    next_sequence: u64,
    head: RelationalJournalHead,
    installed_segment: Option<RelationalJournalSegmentReceipt>,
    durable_segment_count: usize,
}

impl RelationalDurableCheckpoint {
    pub(crate) const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) const fn head(&self) -> RelationalJournalHead {
        self.head
    }

    pub(crate) const fn installed_segment(&self) -> Option<&RelationalJournalSegmentReceipt> {
        self.installed_segment.as_ref()
    }

    pub(crate) const fn durable_segment_count(&self) -> usize {
        self.durable_segment_count
    }
}

/// Final storage ownership after the writer lock has been released.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalDurableJournalFinalized {
    checkpoint: RelationalDurableCheckpoint,
    store: RelationalJournalStoreFinalized,
}

impl RelationalDurableJournalFinalized {
    pub(crate) const fn checkpoint(&self) -> &RelationalDurableCheckpoint {
        &self.checkpoint
    }

    pub(crate) const fn store(&self) -> &RelationalJournalStoreFinalized {
        &self.store
    }
}

/// One writer and one memory-bounded semantic fold. This type is intentionally
/// not cloneable: duplicating it would duplicate both writer authority and the
/// meaning of its provisional tail.
pub(crate) struct RelationalDurableJournal {
    contract: RelationalJournalContract,
    expected_analysis_plan_root: RelationalAnalysisPlanRoot,
    journal: RelationalJournal,
    store: Option<RelationalJournalSegmentStore>,
    codec_limits: RelationalJournalCodecLimits,
    durable_next_sequence: u64,
    durable_head: RelationalJournalHead,
    /// Semantic authorities for the store's bounded, unflushed tail. Entries
    /// are released as soon as the matching immutable segment is installed.
    pending_heads: VecDeque<PendingSemanticHead>,
    poisoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSemanticHead {
    sequence: u64,
    head: RelationalJournalHead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingPhysicalFrame {
    first_sequence: u64,
    last_sequence: u64,
    semantic_event_count: u64,
    previous_head: RelationalJournalHead,
    terminal_head: RelationalJournalHead,
}

impl PendingPhysicalFrame {
    fn first(entry: &super::relational_journal::RelationalJournalEntry) -> Self {
        Self {
            first_sequence: entry.sequence(),
            last_sequence: entry.sequence(),
            semantic_event_count: 1,
            previous_head: entry.previous(),
            terminal_head: entry.head(),
        }
    }

    fn extend(
        &mut self,
        entry: &super::relational_journal::RelationalJournalEntry,
    ) -> Result<(), RelationalDurableJournalError> {
        if self.last_sequence.checked_add(1) != Some(entry.sequence())
            || self.terminal_head != entry.previous()
        {
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        self.last_sequence = entry.sequence();
        self.semantic_event_count = self.semantic_event_count.checked_add(1).ok_or(
            RelationalDurableJournalError::ArithmeticOverflow(
                "physical-frame semantic event count",
            ),
        )?;
        self.terminal_head = entry.head();
        Ok(())
    }
}

impl RelationalDurableJournal {
    pub(crate) fn open_or_create(
        directory: impl AsRef<Path>,
        contract: RelationalJournalContract,
        expected_analysis_plan_root: RelationalAnalysisPlanRoot,
        limits: RelationalDurableJournalLimits,
    ) -> Result<Self, RelationalDurableJournalError> {
        let anchor = journal_store_anchor(&contract);
        let store = RelationalJournalSegmentStore::open_or_create(
            directory,
            limits.run_store(),
            limits.segment(),
            anchor,
        )?;
        Self::from_store(
            contract,
            expected_analysis_plan_root,
            limits.codec(),
            store,
            None,
        )
    }

    pub(crate) fn open_or_create_with_region_replay_authority(
        directory: impl AsRef<Path>,
        contract: RelationalJournalContract,
        expected_analysis_plan_root: RelationalAnalysisPlanRoot,
        limits: RelationalDurableJournalLimits,
        authority: Arc<RelationalRegionReplayAuthority>,
    ) -> Result<Self, RelationalDurableJournalError> {
        let anchor = journal_store_anchor(&contract);
        let store = RelationalJournalSegmentStore::open_or_create(
            directory,
            limits.run_store(),
            limits.segment(),
            anchor,
        )?;
        Self::from_store(
            contract,
            expected_analysis_plan_root,
            limits.codec(),
            store,
            Some(authority),
        )
    }

    pub(crate) fn open(
        directory: impl AsRef<Path>,
        contract: RelationalJournalContract,
        expected_analysis_plan_root: RelationalAnalysisPlanRoot,
        limits: RelationalDurableJournalLimits,
    ) -> Result<Self, RelationalDurableJournalError> {
        let anchor = journal_store_anchor(&contract);
        let store = RelationalJournalSegmentStore::open(
            directory,
            limits.run_store(),
            limits.segment(),
            anchor,
        )?;
        Self::from_store(
            contract,
            expected_analysis_plan_root,
            limits.codec(),
            store,
            None,
        )
    }

    fn from_store(
        contract: RelationalJournalContract,
        expected_analysis_plan_root: RelationalAnalysisPlanRoot,
        codec_limits: RelationalJournalCodecLimits,
        store: RelationalJournalSegmentStore,
        region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    ) -> Result<Self, RelationalDurableJournalError> {
        let mut journal = match region_replay_authority {
            Some(authority) => RelationalJournal::new_streaming_with_region_replay_authority(
                contract.clone(),
                authority,
            ),
            None => RelationalJournal::new_streaming(contract.clone()),
        };
        {
            let segments = store.replay_segments()?;
            for segment in segments {
                let segment = segment?;
                for frame in segment.frames() {
                    let expected_sequence = journal.next_sequence();
                    let expected_previous = journal.head();
                    if frame.first_sequence() != expected_sequence {
                        return Err(RelationalDurableJournalError::FrameSequenceMismatch {
                            expected: expected_sequence,
                            actual: frame.first_sequence(),
                        });
                    }
                    if frame.previous_head() != expected_previous.bytes() {
                        return Err(RelationalDurableJournalError::FramePreviousHeadMismatch {
                            sequence: expected_sequence,
                        });
                    }
                    let mut entries = RelationalJournalPackedFrameReader::new(
                        frame.bytes(),
                        frame.semantic_event_count(),
                        store.limits().max_frame_bytes(),
                        codec_limits,
                    )?;
                    let mut replayed = 0_u64;
                    while let Some(bytes) = entries.next_entry()? {
                        let entry = decode_relational_journal_entry(
                            contract.clone(),
                            journal.next_sequence(),
                            journal.head(),
                            bytes,
                            codec_limits,
                        )?;
                        validate_initial_analysis_plan_event(
                            expected_analysis_plan_root,
                            entry.sequence(),
                            entry.event(),
                        )?;
                        journal.replay_streaming_entry(entry)?;
                        replayed = replayed.checked_add(1).ok_or(
                            RelationalDurableJournalError::ArithmeticOverflow(
                                "replayed semantic event count",
                            ),
                        )?;
                    }
                    entries.finish()?;
                    let expected_next_sequence = frame.last_sequence().checked_add(1);
                    if replayed != frame.semantic_event_count()
                        || expected_next_sequence != Some(journal.next_sequence())
                        || journal.head().bytes() != frame.head()
                    {
                        return Err(
                            RelationalDurableJournalError::FrameSemanticEnvelopeMismatch {
                                sequence: frame.first_sequence(),
                            },
                        );
                    }
                }
            }
        }
        validate_durable_cursor(&journal, &store)?;
        Ok(Self {
            contract,
            expected_analysis_plan_root,
            durable_next_sequence: journal.next_sequence(),
            durable_head: journal.head(),
            journal,
            store: Some(store),
            codec_limits,
            pending_heads: VecDeque::new(),
            poisoned: false,
        })
    }

    pub(crate) const fn contract(&self) -> &RelationalJournalContract {
        &self.contract
    }

    pub(crate) fn journal(&self) -> Result<&RelationalJournal, RelationalDurableJournalError> {
        self.require_active()?;
        Ok(&self.journal)
    }

    /// Borrow the active fold for planning that advances only replay-derived
    /// caches while minting independently checked events. Callers must still
    /// install every returned semantic event through [`Self::append_events`];
    /// this borrow is not append or publication authority.
    pub(crate) fn journal_mut_for_event_planning(
        &mut self,
    ) -> Result<&mut RelationalJournal, RelationalDurableJournalError> {
        self.require_active()?;
        Ok(&mut self.journal)
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Apply and buffer one head-bound ordered batch.
    ///
    /// Each event is semantically validated before its canonical bytes enter
    /// the segment store. An error after any successful application poisons
    /// this owner, including a codec policy refusal: the only safe recovery is
    /// replay of the last installed segment prefix.
    pub(crate) fn append_events(
        &mut self,
        expected_sequence: u64,
        expected_head: RelationalJournalHead,
        events: impl IntoIterator<Item = RelationalJournalEvent>,
    ) -> Result<RelationalDurableAppend, RelationalDurableJournalError> {
        self.require_active()?;
        if self.journal.next_sequence() != expected_sequence {
            return Err(RelationalDurableJournalError::StaleBatchSequence {
                expected: self.journal.next_sequence(),
                actual: expected_sequence,
            });
        }
        if self.journal.head() != expected_head {
            return Err(RelationalDurableJournalError::StaleBatchHead);
        }

        let first_sequence = expected_sequence;
        let max_frame_bytes = self.store_ref()?.limits().max_frame_bytes();
        let mut frame = RelationalJournalPackedFrameBuilder::new(max_frame_bytes)?;
        let mut frame_envelope = None;
        let mut semantic_event_count = 0_u64;
        let mut physical_frame_count = 0_u64;
        let mut installed_segment_count = 0_u64;
        for event in events {
            validate_initial_analysis_plan_event(
                self.expected_analysis_plan_root,
                self.journal.next_sequence(),
                &event,
            )?;
            if self.pending_heads.try_reserve(1).is_err() {
                self.poisoned = true;
                return Err(RelationalDurableJournalError::AllocationFailed(
                    "pending semantic heads",
                ));
            }
            let entry = match self.journal.append_streaming(event) {
                Ok(entry) => entry,
                Err(error) => {
                    self.poisoned = true;
                    return Err(error.into());
                }
            };
            let encoded = match encode_relational_journal_entry(&entry, self.codec_limits) {
                Ok(encoded) => encoded,
                Err(error) => {
                    self.poisoned = true;
                    return Err(error.into());
                }
            };
            let appended = match frame.try_append(&encoded) {
                Ok(appended) => appended,
                Err(error) => {
                    self.poisoned = true;
                    return Err(error.into());
                }
            };
            if !appended {
                let envelope = frame_envelope
                    .take()
                    .expect("only a nonempty packed frame can report full");
                let installed = match self.install_physical_frame(envelope, &frame) {
                    Ok(installed) => installed,
                    Err(error) => {
                        self.poisoned = true;
                        return Err(error);
                    }
                };
                physical_frame_count = match physical_frame_count.checked_add(1) {
                    Some(count) => count,
                    None => {
                        self.poisoned = true;
                        return Err(RelationalDurableJournalError::ArithmeticOverflow(
                            "physical frame count",
                        ));
                    }
                };
                if installed {
                    installed_segment_count = match installed_segment_count.checked_add(1) {
                        Some(count) => count,
                        None => {
                            self.poisoned = true;
                            return Err(RelationalDurableJournalError::ArithmeticOverflow(
                                "installed segment count",
                            ));
                        }
                    };
                }
                frame.clear();
                match frame.try_append(&encoded) {
                    Ok(true) => {}
                    Ok(false) => unreachable!("one validated entry fits an empty physical frame"),
                    Err(error) => {
                        self.poisoned = true;
                        return Err(error.into());
                    }
                }
            }
            match frame_envelope.as_mut() {
                Some(envelope) => {
                    if let Err(error) = envelope.extend(&entry) {
                        self.poisoned = true;
                        return Err(error);
                    }
                }
                None => frame_envelope = Some(PendingPhysicalFrame::first(&entry)),
            }
            self.pending_heads.push_back(PendingSemanticHead {
                sequence: entry.sequence(),
                head: entry.head(),
            });
            semantic_event_count = match semantic_event_count.checked_add(1) {
                Some(count) => count,
                None => {
                    self.poisoned = true;
                    return Err(RelationalDurableJournalError::ArithmeticOverflow(
                        "batch semantic event count",
                    ));
                }
            };
        }

        let semantic_event_count = NonZeroU64::new(semantic_event_count)
            .ok_or(RelationalDurableJournalError::EmptyBatch)?;
        let envelope = frame_envelope.expect("a nonempty semantic batch owns one physical frame");
        let installed = match self.install_physical_frame(envelope, &frame) {
            Ok(installed) => installed,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        physical_frame_count = physical_frame_count.checked_add(1).ok_or_else(|| {
            self.poisoned = true;
            RelationalDurableJournalError::ArithmeticOverflow("physical frame count")
        })?;
        if installed {
            installed_segment_count = installed_segment_count.checked_add(1).ok_or_else(|| {
                self.poisoned = true;
                RelationalDurableJournalError::ArithmeticOverflow("installed segment count")
            })?;
        }
        let physical_frame_count = NonZeroU64::new(physical_frame_count)
            .expect("a nonempty semantic batch installs at least one physical frame");
        Ok(RelationalDurableAppend {
            first_sequence,
            semantic_event_count,
            physical_frame_count,
            next_sequence: self.journal.next_sequence(),
            head: self.journal.head(),
            installed_segment_count,
            durable_next_sequence: self.durable_next_sequence,
            durable_head: self.durable_head,
        })
    }

    fn install_physical_frame(
        &mut self,
        envelope: PendingPhysicalFrame,
        frame: &RelationalJournalPackedFrameBuilder,
    ) -> Result<bool, RelationalDurableJournalError> {
        if frame.is_empty() || frame.semantic_event_count() != envelope.semantic_event_count {
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        let append = self.store_mut()?.append_physical_frame(
            envelope.first_sequence,
            envelope.last_sequence,
            envelope.semantic_event_count,
            envelope.previous_head.bytes(),
            envelope.terminal_head.bytes(),
            frame.bytes(),
        )?;
        if let Some(receipt) = append.installed_segment() {
            self.accept_installed_segment(receipt)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Install the bounded tail and return the exact publishable cursor.
    pub(crate) fn flush_for_pause(
        &mut self,
    ) -> Result<RelationalDurableCheckpoint, RelationalDurableJournalError> {
        self.require_active()?;
        let installed_segment = match self.store_mut()?.flush_for_pause() {
            Ok(receipt) => receipt,
            Err(error) => {
                self.poisoned = true;
                return Err(error.into());
            }
        };
        if let Some(receipt) = installed_segment.as_ref() {
            if let Err(error) = self.accept_installed_segment(receipt) {
                self.poisoned = true;
                return Err(error);
            }
        }
        if let Err(error) = validate_durable_cursor(&self.journal, self.store_ref()?) {
            self.poisoned = true;
            return Err(error);
        }
        if self.durable_next_sequence != self.journal.next_sequence()
            || self.durable_head != self.journal.head()
            || !self.pending_heads.is_empty()
        {
            self.poisoned = true;
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        Ok(RelationalDurableCheckpoint {
            next_sequence: self.journal.next_sequence(),
            head: self.journal.head(),
            installed_segment,
            durable_segment_count: self.store_ref()?.durable_segment_count(),
        })
    }

    /// Flush the final bounded tail and release the writer guard. Semantic
    /// closure is intentionally checked by the caller before invoking this
    /// storage operation.
    pub(crate) fn finalize(
        mut self,
    ) -> Result<RelationalDurableJournalFinalized, RelationalDurableJournalError> {
        self.require_active()?;
        let durable_segment_count_before = self.store_ref()?.durable_segment_count();
        let store = self
            .store
            .take()
            .ok_or(RelationalDurableJournalError::StoreUnavailable)?
            .finalize()?;
        if let Some(receipt) = store.flushed_segment() {
            self.accept_installed_segment(receipt)?;
        }
        if store.next_sequence() != Some(self.journal.next_sequence())
            || store.terminal_head() != self.journal.head().bytes()
            || self.durable_next_sequence != self.journal.next_sequence()
            || self.durable_head != self.journal.head()
            || !self.pending_heads.is_empty()
        {
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        let installed_segment = store.flushed_segment().cloned();
        let durable_segment_count = durable_segment_count_before
            .checked_add(usize::from(installed_segment.is_some()))
            .ok_or(RelationalDurableJournalError::ArithmeticOverflow(
                "durable segment count",
            ))?;
        Ok(RelationalDurableJournalFinalized {
            checkpoint: RelationalDurableCheckpoint {
                next_sequence: self.journal.next_sequence(),
                head: self.journal.head(),
                installed_segment,
                durable_segment_count,
            },
            store,
        })
    }

    fn require_active(&self) -> Result<(), RelationalDurableJournalError> {
        if self.poisoned {
            Err(RelationalDurableJournalError::NeedsReopen)
        } else if self.store.is_none() {
            Err(RelationalDurableJournalError::StoreUnavailable)
        } else {
            Ok(())
        }
    }

    fn store_ref(&self) -> Result<&RelationalJournalSegmentStore, RelationalDurableJournalError> {
        self.store
            .as_ref()
            .ok_or(RelationalDurableJournalError::StoreUnavailable)
    }

    fn store_mut(
        &mut self,
    ) -> Result<&mut RelationalJournalSegmentStore, RelationalDurableJournalError> {
        self.store
            .as_mut()
            .ok_or(RelationalDurableJournalError::StoreUnavailable)
    }

    fn accept_installed_segment(
        &mut self,
        receipt: &RelationalJournalSegmentReceipt,
    ) -> Result<(), RelationalDurableJournalError> {
        if receipt.first_sequence() != self.durable_next_sequence {
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        let mut terminal = None;
        let mut expected_sequence = Some(receipt.first_sequence());
        let mut semantic_event_count = 0_u64;
        while let Some(pending) = self.pending_heads.pop_front() {
            if pending.sequence > receipt.last_sequence() {
                self.pending_heads.push_front(pending);
                break;
            }
            if Some(pending.sequence) != expected_sequence {
                return Err(RelationalDurableJournalError::DurableCursorMismatch);
            }
            semantic_event_count = semantic_event_count.checked_add(1).ok_or(
                RelationalDurableJournalError::ArithmeticOverflow("installed semantic event count"),
            )?;
            expected_sequence = pending.sequence.checked_add(1);
            terminal = Some(pending);
        }
        let terminal = terminal.ok_or(RelationalDurableJournalError::DurableCursorMismatch)?;
        if terminal.sequence != receipt.last_sequence()
            || semantic_event_count != receipt.semantic_event_count()
            || terminal.head.bytes() != receipt.terminal_head()
        {
            return Err(RelationalDurableJournalError::DurableCursorMismatch);
        }
        self.durable_next_sequence = terminal
            .sequence
            .checked_add(1)
            .ok_or(RelationalDurableJournalError::SequenceExhausted)?;
        self.durable_head = terminal.head;
        Ok(())
    }
}

fn validate_initial_analysis_plan_event(
    expected: RelationalAnalysisPlanRoot,
    sequence: u64,
    event: &RelationalJournalEvent,
) -> Result<(), RelationalDurableJournalError> {
    if sequence != 0 {
        return Ok(());
    }
    let RelationalJournalEvent::Evidence(RelationalEvidenceEvent::AnalysisPlanRegistered {
        plan_root,
        ..
    }) = event
    else {
        return Err(RelationalDurableJournalError::InitialAnalysisPlanMissing);
    };
    if *plan_root != expected {
        return Err(
            RelationalDurableJournalError::ExpectedAnalysisPlanRootMismatch {
                expected,
                actual: *plan_root,
            },
        );
    }
    Ok(())
}

impl RelationalPublicationAuthority for RelationalDurableJournal {
    fn journal(&self) -> Result<&RelationalJournal, String> {
        RelationalDurableJournal::journal(self).map_err(|error| error.to_string())
    }

    fn durable_checkpoint(&self) -> Result<RelationalPublicationCheckpoint, String> {
        self.require_active().map_err(|error| error.to_string())?;
        let store = self.store_ref().map_err(|error| error.to_string())?;
        if self.durable_next_sequence != self.journal.next_sequence()
            || self.durable_head != self.journal.head()
            || !self.pending_heads.is_empty()
            || store.durable_next_sequence() != Some(self.durable_next_sequence)
            || store.durable_terminal_head() != self.durable_head.bytes()
        {
            return Err(RelationalDurableJournalError::DurableCursorMismatch.to_string());
        }
        Ok(RelationalPublicationCheckpoint::new(
            self.durable_next_sequence,
            self.durable_head.bytes(),
        ))
    }

    fn authenticates_durable_prefix(
        &self,
        checkpoint: RelationalPublicationCheckpoint,
    ) -> Result<bool, String> {
        self.require_active().map_err(|error| error.to_string())?;
        if checkpoint.next_sequence() > self.durable_next_sequence {
            return Ok(false);
        }
        Ok(self
            .store_ref()
            .map_err(|error| error.to_string())?
            .authenticates_durable_checkpoint(checkpoint.next_sequence(), checkpoint.head()))
    }
}

fn journal_store_anchor(contract: &RelationalJournalContract) -> RelationalJournalStoreAnchor {
    let genesis = RelationalJournal::new_streaming(contract.clone());
    RelationalJournalStoreAnchor::new(genesis.next_sequence(), genesis.head().bytes())
}

fn validate_cross_layer_limits(
    segment: RelationalJournalSegmentLimits,
    codec: RelationalJournalCodecLimits,
) -> Result<(), RelationalDurableJournalError> {
    if codec
        .max_entry_bytes()
        .checked_add(RELATIONAL_JOURNAL_PACKED_ENTRY_PREFIX_BYTES)
        .is_none_or(|bytes| bytes > segment.max_frame_bytes())
    {
        return Err(RelationalDurableJournalError::IncompatibleLimits(
            "a length-delimited codec entry must fit inside one physical segment frame",
        ));
    }
    Ok(())
}

fn validate_durable_cursor(
    journal: &RelationalJournal,
    store: &RelationalJournalSegmentStore,
) -> Result<(), RelationalDurableJournalError> {
    if store.durable_next_sequence() != Some(journal.next_sequence())
        || store.durable_terminal_head() != journal.head().bytes()
    {
        return Err(RelationalDurableJournalError::DurableCursorMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum RelationalDurableJournalError {
    IncompatibleLimits(&'static str),
    NeedsReopen,
    StoreUnavailable,
    EmptyBatch,
    SequenceExhausted,
    AllocationFailed(&'static str),
    ArithmeticOverflow(&'static str),
    StaleBatchSequence {
        expected: u64,
        actual: u64,
    },
    StaleBatchHead,
    FrameSequenceMismatch {
        expected: u64,
        actual: u64,
    },
    FramePreviousHeadMismatch {
        sequence: u64,
    },
    FrameSemanticEnvelopeMismatch {
        sequence: u64,
    },
    DurableCursorMismatch,
    InitialAnalysisPlanMissing,
    ExpectedAnalysisPlanRootMismatch {
        expected: RelationalAnalysisPlanRoot,
        actual: RelationalAnalysisPlanRoot,
    },
    Journal(RelationalJournalError),
    Codec(RelationalJournalCodecError),
    Store(RelationalJournalSegmentStoreError),
}

impl fmt::Display for RelationalDurableJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompatibleLimits(reason) => {
                write!(
                    formatter,
                    "incompatible relational journal limits: {reason}"
                )
            }
            Self::NeedsReopen => formatter.write_str(
                "relational journal owner is poisoned and must reopen its last durable prefix",
            ),
            Self::StoreUnavailable => {
                formatter.write_str("relational journal store ownership is unavailable")
            }
            Self::EmptyBatch => formatter.write_str("relational journal batch is empty"),
            Self::SequenceExhausted => {
                formatter.write_str("relational journal sequence is exhausted")
            }
            Self::AllocationFailed(resource) => write!(
                formatter,
                "relational journal could not reserve bounded {resource} storage"
            ),
            Self::ArithmeticOverflow(field) => {
                write!(formatter, "relational journal arithmetic overflow: {field}")
            }
            Self::StaleBatchSequence { expected, actual } => write!(
                formatter,
                "relational journal batch sequence is stale: expected {expected}, got {actual}"
            ),
            Self::StaleBatchHead => formatter.write_str("relational journal batch head is stale"),
            Self::FrameSequenceMismatch { expected, actual } => write!(
                formatter,
                "stored relational frame sequence mismatch: expected {expected}, got {actual}"
            ),
            Self::FramePreviousHeadMismatch { sequence } => write!(
                formatter,
                "stored relational frame {sequence} has the wrong previous head"
            ),
            Self::FrameSemanticEnvelopeMismatch { sequence } => write!(
                formatter,
                "stored relational frame {sequence} disagrees with its semantic entry envelope"
            ),
            Self::DurableCursorMismatch => formatter.write_str(
                "installed relational segments and the semantic journal disagree at their tail",
            ),
            Self::InitialAnalysisPlanMissing => formatter.write_str(
                "relational journal semantic event zero must register the checked analysis plan",
            ),
            Self::ExpectedAnalysisPlanRootMismatch { .. } => formatter.write_str(
                "relational journal analysis plan differs from the freshly checked plan",
            ),
            Self::Journal(error) => write!(formatter, "relational journal error: {error}"),
            Self::Codec(error) => write!(formatter, "relational journal codec error: {error}"),
            Self::Store(error) => write!(formatter, "relational journal store error: {error}"),
        }
    }
}

impl Error for RelationalDurableJournalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Journal(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::IncompatibleLimits(_)
            | Self::NeedsReopen
            | Self::StoreUnavailable
            | Self::EmptyBatch
            | Self::SequenceExhausted
            | Self::AllocationFailed(_)
            | Self::ArithmeticOverflow(_)
            | Self::StaleBatchSequence { .. }
            | Self::StaleBatchHead
            | Self::FrameSequenceMismatch { .. }
            | Self::FramePreviousHeadMismatch { .. }
            | Self::FrameSemanticEnvelopeMismatch { .. }
            | Self::DurableCursorMismatch
            | Self::InitialAnalysisPlanMissing
            | Self::ExpectedAnalysisPlanRootMismatch { .. } => None,
        }
    }
}

impl From<RelationalJournalError> for RelationalDurableJournalError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<RelationalJournalCodecError> for RelationalDurableJournalError {
    fn from(error: RelationalJournalCodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<RelationalJournalSegmentStoreError> for RelationalDurableJournalError {
    fn from(error: RelationalJournalSegmentStoreError) -> Self {
        Self::Store(error)
    }
}
