//! Canonical concrete successor and classification leaf execution.
//!
//! This is the trusted singleton-cell fallback beneath symbolic support
//! backends. It evaluates the checked TO, WHERE, and FIND descriptors through
//! the same expression runtime used by dependent FROM execution. One source
//! opens one canonically ordered, set-normalized successor fiber; duplicate
//! producer occurrences remain provenance support and never become extra
//! cases.
//!
//! Successor provenance is exact within that source-specific TO fiber. The
//! parent [`SourceRow`] retains FROM provenance separately; composing the two
//! into correlated or weighted full producer paths belongs to the factorized
//! `SupportExpr` layer rather than to this flat concrete-row fallback.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{
    AdmissionDecision, FindPolarity, QuestionId, RelationId, RelationLineageId, RelationProvenance,
    RelationSupportId, RelationalCaseId, RelationalCaseRef, SelectionDecision, SourceKey,
    SourceRow, SuccessorKey, SuccessorRow,
};
use super::relational_executor::{
    RelationalBoundValue, RelationalExpressionRuntime, RelationalFiberMember,
    RelationalFiniteFiber, RelationalSourceExecutorError,
};
use super::relational_frontier::WorkNodeSpec;
use super::relational_ir::{
    ExploreFindIr, ExploreFiniteDomainIr, ExploreQueryIr, ExploreSuccessorKindIr,
};
use super::relational_journal::RelationalJournalEvent;
use super::transition::canonical_explore_value_digest;
use super::ExploreValue;
use crate::{
    CheckedExploreQueryView, ExploreAdmissionScope, ExploreRelationMultiplicity, Expr, Ty,
};

pub(crate) const RELATIONAL_SUCCESSOR_CURSOR_VERSION: u32 = 1;
pub(crate) const SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION: u32 = 1;

const SUCCESSOR_LINEAGE_PREIMAGE_V1: &[u8] = b"futuruna.explore.successor-lineage-preimage.v1";
const SUCCESSOR_SUPPORT_PREIMAGE_V1: &[u8] = b"futuruna.explore.successor-support-preimage.v1";
const SUCCESSOR_FIBER_ROWS_COMMITMENT_V1: &[u8] =
    b"futuruna.explore.successor-fiber-rows-commitment.v1";
const SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_HASH_V1: &[u8] =
    b"futuruna.explore.successor-fiber-exhaustion-receipt.v1";

/// Content identity of one executor-issued successor-fiber exhaustion receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SuccessorFiberExhaustionReceiptId([u8; 32]);

impl SuccessorFiberExhaustionReceiptId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Durable next-member cursor for one source's dependent TO fiber.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSuccessorCursor {
    relation_id: RelationId,
    source_key: SourceKey,
    next_successor_ordinal: u128,
}

impl RelationalSuccessorCursor {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn next_successor_ordinal(&self) -> u128 {
        self.next_successor_ordinal
    }

    pub(crate) const fn snapshot(&self) -> RelationalSuccessorCursorSnapshot {
        RelationalSuccessorCursorSnapshot {
            version: RELATIONAL_SUCCESSOR_CURSOR_VERSION,
            relation_id: self.relation_id,
            source_key: self.source_key,
            next_successor_ordinal: self.next_successor_ordinal,
        }
    }

    const fn with_next_successor_ordinal(&self, next_successor_ordinal: u128) -> Self {
        Self {
            relation_id: self.relation_id,
            source_key: self.source_key,
            next_successor_ordinal,
        }
    }
}

/// Codec-ready successor cursor. It contains no evaluator or scheduler state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSuccessorCursorSnapshot {
    pub(crate) version: u32,
    pub(crate) relation_id: RelationId,
    pub(crate) source_key: SourceKey,
    pub(crate) next_successor_ordinal: u128,
}

/// One opened TO fiber bound to its canonical source coordinate.
#[derive(Clone, Debug)]
pub(crate) struct RelationalSuccessorFiber {
    relation_id: RelationId,
    source_key: SourceKey,
    finite: RelationalFiniteFiber,
}

impl RelationalSuccessorFiber {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn cardinality(&self) -> u128 {
        self.finite.cardinality()
    }
}

/// Semantic proof that the checked successor executor reached the terminal
/// member of one source-bound TO fiber.
///
/// There is deliberately no general constructor. The receipt is issued only
/// by the executor branch that reopens the exact fiber and observes canonical
/// exhaustion, so a persisted successor cursor is never completion authority
/// by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SuccessorFiberExhaustionReceipt {
    version: u32,
    id: SuccessorFiberExhaustionReceiptId,
    relation_id: RelationId,
    source_key: SourceKey,
    terminal_ordinal: u128,
    emitted_row_count: u128,
    emitted_rows_commitment: [u8; 32],
}

impl SuccessorFiberExhaustionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        version: u32,
        relation_id: RelationId,
        source_key: SourceKey,
        terminal_ordinal: u128,
        emitted_row_count: u128,
        emitted_rows_commitment: [u8; 32],
    ) -> Result<Self, RelationalCaseExecutorError> {
        let id = derive_successor_fiber_exhaustion_receipt_id(
            version,
            relation_id,
            source_key,
            terminal_ordinal,
            emitted_row_count,
            emitted_rows_commitment,
        );
        let restored = Self {
            version,
            id,
            relation_id,
            source_key,
            terminal_ordinal,
            emitted_row_count,
            emitted_rows_commitment,
        };
        restored.validate_identity()?;
        Ok(restored)
    }

    fn issue(
        relation_id: RelationId,
        cursor: &RelationalSuccessorCursor,
        fiber: &RelationalSuccessorFiber,
    ) -> Result<Self, RelationalCaseExecutorError> {
        if fiber.relation_id != relation_id
            || cursor.relation_id != relation_id
            || fiber.source_key != cursor.source_key
        {
            return Err(RelationalCaseExecutorError::SuccessorFiberMismatch);
        }
        let terminal_ordinal = cursor.next_successor_ordinal;
        let emitted_row_count = fiber.cardinality();
        if terminal_ordinal != emitted_row_count {
            return Err(RelationalCaseExecutorError::ExhaustionBeforeTerminal {
                terminal_ordinal,
                cardinality: emitted_row_count,
            });
        }
        let version = SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION;
        let emitted_rows_commitment = successor_fiber_rows_commitment(fiber)?;
        let id = derive_successor_fiber_exhaustion_receipt_id(
            version,
            relation_id,
            cursor.source_key,
            terminal_ordinal,
            emitted_row_count,
            emitted_rows_commitment,
        );
        Ok(Self {
            version,
            id,
            relation_id,
            source_key: cursor.source_key,
            terminal_ordinal,
            emitted_row_count,
            emitted_rows_commitment,
        })
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn id(&self) -> SuccessorFiberExhaustionReceiptId {
        self.id
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn terminal_ordinal(&self) -> u128 {
        self.terminal_ordinal
    }

    pub(crate) const fn emitted_row_count(&self) -> u128 {
        self.emitted_row_count
    }

    pub(crate) const fn emitted_rows_commitment(&self) -> [u8; 32] {
        self.emitted_rows_commitment
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RelationalCaseExecutorError> {
        if self.version != SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION {
            return Err(
                RelationalCaseExecutorError::UnsupportedExhaustionReceiptVersion {
                    actual: self.version,
                    expected: SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION,
                },
            );
        }
        if self.terminal_ordinal != self.emitted_row_count {
            return Err(
                RelationalCaseExecutorError::ExhaustionReceiptCountMismatch {
                    terminal_ordinal: self.terminal_ordinal,
                    emitted_row_count: self.emitted_row_count,
                },
            );
        }
        let derived = derive_successor_fiber_exhaustion_receipt_id(
            self.version,
            self.relation_id,
            self.source_key,
            self.terminal_ordinal,
            self.emitted_row_count,
            self.emitted_rows_commitment,
        );
        if derived != self.id {
            return Err(RelationalCaseExecutorError::ExhaustionReceiptIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

/// One concrete, canonical member of the `(Context, Before, After)` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalConcreteCase {
    source_key: SourceKey,
    successor_key: SuccessorKey,
    case_id: RelationalCaseId,
    successor: SuccessorRow,
}

impl RelationalConcreteCase {
    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(&self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) fn successor(&self) -> &SuccessorRow {
        &self.successor
    }

    pub(crate) fn discovered_event(&self) -> RelationalJournalEvent {
        RelationalJournalEvent::successor_discovered_with_ids(
            self.source_key,
            self.successor_key,
            self.case_id,
            self.successor.clone(),
        )
    }
}

/// The complete semantic consequence of one checked, syntactically singleton
/// TO relation. Keeping the exhaustion receipt beside the sole case lets the
/// source scheduler fuse the functional transition without manufacturing an
/// intermediate successor work cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSingletonTransition {
    case: RelationalConcreteCase,
    exhaustion_receipt: SuccessorFiberExhaustionReceipt,
}

impl RelationalSingletonTransition {
    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case.case_id
    }

    pub(crate) fn into_parts(self) -> (RelationalConcreteCase, SuccessorFiberExhaustionReceipt) {
        (self.case, self.exhaustion_receipt)
    }
}

/// One lazy ordinal step in a source's canonical successor fiber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSuccessorAdvance {
    Yielded {
        member: RelationalFiberMember,
        case: RelationalConcreteCase,
        resume: RelationalSuccessorCursor,
    },
    Exhausted {
        cursor: RelationalSuccessorCursor,
        cardinality: u128,
        receipt: SuccessorFiberExhaustionReceipt,
    },
}

/// Admission evidence produced by the dedicated WHERE work leaf.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalAdmissionClassification {
    case_id: RelationalCaseId,
    decision: AdmissionDecision,
}

impl RelationalAdmissionClassification {
    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn decision(&self) -> AdmissionDecision {
        self.decision
    }

    pub(crate) fn event(&self) -> RelationalJournalEvent {
        RelationalJournalEvent::admission_classified(self.case_id, self.decision)
    }
}

/// One canonical semantic question and the authored FIND index that evaluates
/// it. Equivalent authored aliases collapse to one entry keyed by QuestionId.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalQuestionEvaluation {
    question_id: QuestionId,
    find_index: usize,
}

/// Closed mapping from the query's authored FIND addresses to its unique
/// semantic questions. Entries are sorted by QuestionId, never source name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalQuestionEvaluationPlan {
    relation_id: RelationId,
    query_address: usize,
    authored_find_count: usize,
    questions: Box<[RelationalQuestionEvaluation]>,
}

impl RelationalQuestionEvaluationPlan {
    pub(crate) fn question_ids(&self) -> impl ExactSizeIterator<Item = QuestionId> + '_ {
        self.questions.iter().map(|question| question.question_id)
    }

    pub(crate) const fn unique_question_count(&self) -> usize {
        self.questions.len()
    }

    fn question(&self, question_id: QuestionId) -> Option<RelationalQuestionEvaluation> {
        self.questions
            .binary_search_by_key(&question_id, |question| question.question_id)
            .ok()
            .map(|index| self.questions[index])
    }

    fn matches_executor(&self, executor: &RelationalCaseExecutor<'_>) -> bool {
        self.relation_id == executor.relation_id
            && self.query_address == std::ptr::from_ref(executor.query).addr()
            && self.authored_find_count == executor.query.finds.len()
    }
}

/// FIND evidence produced only for one already admitted concrete case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalQuestionClassification {
    question_id: QuestionId,
    case_id: RelationalCaseId,
    decision: SelectionDecision,
}

impl RelationalQuestionClassification {
    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn decision(&self) -> SelectionDecision {
        self.decision
    }

    pub(crate) fn event(&self) -> RelationalJournalEvent {
        RelationalJournalEvent::question_classified(self.question_id, self.case_id, self.decision)
    }
}

/// Paired convenience result for callers executing admission and every unique
/// semantic question immediately. A rejected case has no question evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseClassification {
    admission: RelationalAdmissionClassification,
    questions: Box<[RelationalQuestionClassification]>,
}

impl RelationalCaseClassification {
    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.admission.case_id
    }

    pub(crate) const fn admission(&self) -> AdmissionDecision {
        self.admission.decision
    }

    pub(crate) fn question_decision(&self, question_id: QuestionId) -> Option<SelectionDecision> {
        self.questions
            .binary_search_by_key(&question_id, |question| question.question_id)
            .ok()
            .map(|index| self.questions[index].decision)
    }

    pub(crate) const fn admission_evidence(&self) -> RelationalAdmissionClassification {
        self.admission
    }

    pub(crate) fn question_evidence(&self) -> &[RelationalQuestionClassification] {
        &self.questions
    }

    pub(crate) fn admission_event(&self) -> RelationalJournalEvent {
        self.admission.event()
    }

    pub(crate) fn question_events(
        &self,
    ) -> impl ExactSizeIterator<Item = RelationalJournalEvent> + '_ {
        self.questions
            .iter()
            .map(RelationalQuestionClassification::event)
    }
}

/// Concrete TO/WHERE/FIND executor for one checked query descriptor.
pub(crate) struct RelationalCaseExecutor<'a> {
    relation_id: RelationId,
    query: &'a ExploreQueryIr,
}

impl<'a> RelationalCaseExecutor<'a> {
    pub(crate) fn new(
        relation_id: RelationId,
        query: &'a ExploreQueryIr,
    ) -> Result<Self, RelationalCaseExecutorError> {
        query
            .validate()
            .map_err(RelationalCaseExecutorError::InvalidQuery)?;
        if !matches!(
            query.successor.multiplicity,
            ExploreRelationMultiplicity::SetNormalized
        ) {
            return Err(RelationalCaseExecutorError::InvalidQuery(
                "successor multiplicity is not set-normalized".to_string(),
            ));
        }
        Ok(Self { relation_id, query })
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    /// Bind checked QuestionIds to authored FIND indices and collapse aliases.
    /// The caller supplies one ID per authored find in declaration order.
    pub(crate) fn checked_question_evaluation_plan(
        &self,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<RelationalQuestionEvaluationPlan, RelationalCaseExecutorError> {
        if checked.relation_id() != self.relation_id
            || !std::ptr::eq(checked.closed_query, self.query)
        {
            return Err(RelationalCaseExecutorError::QuestionPlanMismatch);
        }
        self.build_question_evaluation_plan(checked.find_question_ids())
    }

    #[cfg(test)]
    pub(crate) fn question_evaluation_plan(
        &self,
        find_question_ids: &[QuestionId],
    ) -> Result<RelationalQuestionEvaluationPlan, RelationalCaseExecutorError> {
        self.build_question_evaluation_plan(find_question_ids)
    }

    fn build_question_evaluation_plan(
        &self,
        find_question_ids: &[QuestionId],
    ) -> Result<RelationalQuestionEvaluationPlan, RelationalCaseExecutorError> {
        if find_question_ids.len() != self.query.finds.len() {
            return Err(RelationalCaseExecutorError::QuestionCountMismatch {
                finds: self.query.finds.len(),
                question_ids: find_question_ids.len(),
            });
        }
        let mut unique = BTreeMap::<QuestionId, (usize, FindPolarity)>::new();
        for (find_index, (named_find, question_id)) in self
            .query
            .finds
            .iter()
            .zip(find_question_ids.iter().copied())
            .enumerate()
        {
            let polarity = named_find.find.polarity();
            match unique.entry(question_id) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert((find_index, polarity));
                }
                std::collections::btree_map::Entry::Occupied(slot) if slot.get().1 == polarity => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(RelationalCaseExecutorError::QuestionAliasPolarityMismatch {
                        question_id,
                    });
                }
            }
        }
        Ok(RelationalQuestionEvaluationPlan {
            relation_id: self.relation_id,
            query_address: std::ptr::from_ref(self.query).addr(),
            authored_find_count: self.query.finds.len(),
            questions: unique
                .into_iter()
                .map(
                    |(question_id, (find_index, _))| RelationalQuestionEvaluation {
                        question_id,
                        find_index,
                    },
                )
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    pub(crate) fn root_cursor(
        &self,
        source_key: SourceKey,
        source: &SourceRow,
    ) -> Result<RelationalSuccessorCursor, RelationalCaseExecutorError> {
        self.validate_source(source_key, source)?;
        Ok(RelationalSuccessorCursor {
            relation_id: self.relation_id,
            source_key,
            next_successor_ordinal: 0,
        })
    }

    pub(crate) fn work_spec(
        &self,
        cursor: &RelationalSuccessorCursor,
    ) -> Result<WorkNodeSpec, RelationalCaseExecutorError> {
        self.validate_cursor_identity(cursor)?;
        Ok(WorkNodeSpec::ExpandSuccessors {
            relation_id: self.relation_id,
            source_key: cursor.source_key,
        })
    }

    /// Restore a cursor only after reopening the checked source-specific TO
    /// fiber and validating its canonical exhaustion boundary.
    pub(crate) fn resume_snapshot<R: RelationalExpressionRuntime>(
        &self,
        snapshot: RelationalSuccessorCursorSnapshot,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalSuccessorCursor, RelationalCaseExecutorError> {
        if snapshot.version != RELATIONAL_SUCCESSOR_CURSOR_VERSION {
            return Err(RelationalCaseExecutorError::UnsupportedCursorVersion {
                actual: snapshot.version,
                expected: RELATIONAL_SUCCESSOR_CURSOR_VERSION,
            });
        }
        if snapshot.relation_id != self.relation_id {
            return Err(RelationalCaseExecutorError::CursorRelationMismatch);
        }
        let cursor = RelationalSuccessorCursor {
            relation_id: snapshot.relation_id,
            source_key: snapshot.source_key,
            next_successor_ordinal: snapshot.next_successor_ordinal,
        };
        self.validate_resumed_cursor(&cursor, source, runtime)?;
        Ok(cursor)
    }

    /// Reconstruct the cursor carried by one `ExpandSuccessors` work node.
    pub(crate) fn resume_cursor<R: RelationalExpressionRuntime>(
        &self,
        work_spec: &WorkNodeSpec,
        next_successor_ordinal: u128,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalSuccessorCursor, RelationalCaseExecutorError> {
        self.resume_cursor_with_fiber(work_spec, next_successor_ordinal, source, runtime)
            .map(|(cursor, _)| cursor)
    }

    /// Reconstruct and validate one durable successor cursor while retaining
    /// the source-bound finite fiber opened for that validation. The returned
    /// fiber can serve a bounded ordinal chunk without a second TO evaluation
    /// and remains guarded by `advance_in_fiber`'s relation/source checks.
    pub(crate) fn resume_cursor_with_fiber<R: RelationalExpressionRuntime>(
        &self,
        work_spec: &WorkNodeSpec,
        next_successor_ordinal: u128,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<(RelationalSuccessorCursor, RelationalSuccessorFiber), RelationalCaseExecutorError>
    {
        let WorkNodeSpec::ExpandSuccessors {
            relation_id,
            source_key,
        } = work_spec
        else {
            return Err(RelationalCaseExecutorError::NotSuccessorWork);
        };
        if *relation_id != self.relation_id {
            return Err(RelationalCaseExecutorError::WorkRelationMismatch);
        }
        let cursor = RelationalSuccessorCursor {
            relation_id: *relation_id,
            source_key: *source_key,
            next_successor_ordinal,
        };
        let fiber = self.validate_resumed_cursor(&cursor, source, runtime)?;
        Ok((cursor, fiber))
    }

    /// Evaluate and normalize the exact TO fiber for this source once. A
    /// worker may retain the returned compact fiber across many ordinal steps.
    pub(crate) fn successor_fiber<R: RelationalExpressionRuntime>(
        &self,
        source_key: SourceKey,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalSuccessorFiber, RelationalCaseExecutorError> {
        self.validate_source(source_key, source)?;
        let source_bindings = source_bindings(source);
        let finite = match &self.query.successor.kind {
            ExploreSuccessorKindIr::Singleton { value } => runtime
                .evaluate(value, &self.query.successor.after_ty, &source_bindings)
                .map(RelationalFiniteFiber::singleton)
                .map_err(|message| RelationalCaseExecutorError::Evaluation {
                    phase: "successor singleton".to_string(),
                    message,
                })?,
            ExploreSuccessorKindIr::Finite { domain } => {
                self.evaluate_finite_domain(domain, &source_bindings, runtime)?
            }
        };
        Ok(RelationalSuccessorFiber {
            relation_id: self.relation_id,
            source_key,
            finite,
        })
    }

    /// Execute a TO relation only when the checked query proves its fiber is
    /// singleton from syntax alone. The returned receipt is minted by the same
    /// ordinal advance path as ordinary `ExpandSuccessors` work, preserving
    /// the exact case and exhaustion identities of the unfused schedule.
    pub(crate) fn statically_singleton_transition<R: RelationalExpressionRuntime>(
        &self,
        source_key: SourceKey,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<Option<RelationalSingletonTransition>, RelationalCaseExecutorError> {
        if !matches!(
            &self.query.successor.kind,
            ExploreSuccessorKindIr::Singleton { .. }
        ) {
            return Ok(None);
        }

        let cursor = self.root_cursor(source_key, source)?;
        let fiber = self.successor_fiber(source_key, source, runtime)?;
        if fiber.cardinality() != 1 {
            return Err(RelationalCaseExecutorError::StaticSingletonShapeMismatch);
        }
        let RelationalSuccessorAdvance::Yielded { case, resume, .. } =
            self.advance_in_fiber(&cursor, &fiber)?
        else {
            return Err(RelationalCaseExecutorError::StaticSingletonShapeMismatch);
        };
        let RelationalSuccessorAdvance::Exhausted {
            cardinality,
            receipt,
            ..
        } = self.advance_in_fiber(&resume, &fiber)?
        else {
            return Err(RelationalCaseExecutorError::StaticSingletonShapeMismatch);
        };
        if cardinality != 1 || receipt.terminal_ordinal() != 1 {
            return Err(RelationalCaseExecutorError::StaticSingletonShapeMismatch);
        }
        Ok(Some(RelationalSingletonTransition {
            case,
            exhaustion_receipt: receipt,
        }))
    }

    /// Advance exactly one canonical After member, reopening the TO fiber for
    /// callers that do not retain evaluator-local caches.
    pub(crate) fn advance<R: RelationalExpressionRuntime>(
        &self,
        cursor: &RelationalSuccessorCursor,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalSuccessorAdvance, RelationalCaseExecutorError> {
        self.validate_cursor_identity(cursor)?;
        let fiber = self.successor_fiber(cursor.source_key, source, runtime)?;
        self.advance_in_fiber(cursor, &fiber)
    }

    /// Advance using an already opened source-bound TO fiber.
    pub(crate) fn advance_in_fiber(
        &self,
        cursor: &RelationalSuccessorCursor,
        fiber: &RelationalSuccessorFiber,
    ) -> Result<RelationalSuccessorAdvance, RelationalCaseExecutorError> {
        self.validate_cursor_identity(cursor)?;
        if fiber.relation_id != self.relation_id || fiber.source_key != cursor.source_key {
            return Err(RelationalCaseExecutorError::SuccessorFiberMismatch);
        }
        let Some(member) = fiber
            .finite
            .member_at_ordinal(cursor.next_successor_ordinal)
            .map_err(RelationalCaseExecutorError::FiniteFiber)?
        else {
            let receipt = SuccessorFiberExhaustionReceipt::issue(self.relation_id, cursor, fiber)?;
            return Ok(RelationalSuccessorAdvance::Exhausted {
                cursor: *cursor,
                cardinality: fiber.cardinality(),
                receipt,
            });
        };
        let next_successor_ordinal = cursor
            .next_successor_ordinal
            .checked_add(1)
            .ok_or(RelationalCaseExecutorError::CursorOrdinalOverflow)?;
        let successor = SuccessorRow::new(
            member.value().clone(),
            successor_provenance(self.relation_id, cursor.source_key, &member),
        );
        let successor_key = SuccessorKey::derive(self.relation_id, cursor.source_key, &successor);
        let case_id = RelationalCaseId::derive(self.relation_id, cursor.source_key, successor_key);
        Ok(RelationalSuccessorAdvance::Yielded {
            member,
            case: RelationalConcreteCase {
                source_key: cursor.source_key,
                successor_key,
                case_id,
                successor,
            },
            resume: cursor.with_next_successor_ordinal(next_successor_ordinal),
        })
    }

    /// Evaluate only the scoped WHERE conjunction. This is the implementation
    /// of one `EvaluateAdmission` work leaf.
    pub(crate) fn evaluate_admission<R: RelationalExpressionRuntime>(
        &self,
        source: &SourceRow,
        case: &RelationalConcreteCase,
        runtime: &mut R,
    ) -> Result<RelationalAdmissionClassification, RelationalCaseExecutorError> {
        self.validate_case(source, case)?;
        self.evaluate_admission_values(
            case.case_id,
            source.context(),
            source.before(),
            case.successor.after(),
            runtime,
        )
    }

    /// Evaluate one durable catalog case through the same checked WHERE leaf
    /// as a freshly yielded concrete case. This is the resume boundary: no
    /// private `RelationalConcreteCase` reconstruction or second evaluator is
    /// required.
    pub(crate) fn evaluate_catalog_admission<R: RelationalExpressionRuntime>(
        &self,
        case: RelationalCaseRef<'_>,
        runtime: &mut R,
    ) -> Result<RelationalAdmissionClassification, RelationalCaseExecutorError> {
        if case.relation_id() != self.relation_id {
            return Err(RelationalCaseExecutorError::CaseRelationMismatch);
        }
        self.evaluate_admission_values(
            case.case_id(),
            case.context(),
            case.before(),
            case.after(),
            runtime,
        )
    }

    fn evaluate_admission_values<R: RelationalExpressionRuntime>(
        &self,
        case_id: RelationalCaseId,
        context_value: &ExploreValue,
        before_value: &ExploreValue,
        after_value: &ExploreValue,
        runtime: &mut R,
    ) -> Result<RelationalAdmissionClassification, RelationalCaseExecutorError> {
        let context = RelationalBoundValue {
            name: "context",
            value: context_value,
        };
        let before = RelationalBoundValue {
            name: "before",
            value: before_value,
        };
        let after = RelationalBoundValue {
            name: "after",
            value: after_value,
        };
        let before_bindings = [context, before];
        let after_bindings = [context, after];
        let transition_bindings = [context, before, after];

        for admission in self.query.admissions.iter() {
            let bindings = match admission.scope {
                ExploreAdmissionScope::Before => before_bindings.as_slice(),
                ExploreAdmissionScope::After => after_bindings.as_slice(),
                ExploreAdmissionScope::Transition => transition_bindings.as_slice(),
            };
            let phase = format!("admission {}", admission.admission_index);
            if !evaluate_boolean(&admission.predicate, bindings, runtime, phase)? {
                return Ok(RelationalAdmissionClassification {
                    case_id,
                    decision: AdmissionDecision::Rejected,
                });
            }
        }

        Ok(RelationalAdmissionClassification {
            case_id,
            decision: AdmissionDecision::Admitted,
        })
    }

    /// Evaluate every unique semantic question only after the caller has
    /// durable admission evidence for the same case. Rejection returns an
    /// empty slice without invoking any FIND predicate.
    pub(crate) fn evaluate_questions<R: RelationalExpressionRuntime>(
        &self,
        source: &SourceRow,
        case: &RelationalConcreteCase,
        admission: &RelationalAdmissionClassification,
        questions: &RelationalQuestionEvaluationPlan,
        runtime: &mut R,
    ) -> Result<Box<[RelationalQuestionClassification]>, RelationalCaseExecutorError> {
        self.validate_case(source, case)?;
        if admission.case_id != case.case_id {
            return Err(RelationalCaseExecutorError::AdmissionCaseMismatch {
                admission_case_id: admission.case_id,
                case_id: case.case_id,
            });
        }
        self.evaluate_question_values(
            case.case_id,
            source.context(),
            source.before(),
            case.successor.after(),
            admission,
            questions,
            runtime,
        )
    }

    /// Evaluate all questions for a freshly yielded case after the scheduler
    /// recovered (or just emitted) its durable admission decision.
    pub(crate) fn evaluate_questions_for_admission_decision<R: RelationalExpressionRuntime>(
        &self,
        source: &SourceRow,
        case: &RelationalConcreteCase,
        admission: AdmissionDecision,
        questions: &RelationalQuestionEvaluationPlan,
        runtime: &mut R,
    ) -> Result<Box<[RelationalQuestionClassification]>, RelationalCaseExecutorError> {
        let admission = RelationalAdmissionClassification {
            case_id: case.case_id,
            decision: admission,
        };
        self.evaluate_questions(source, case, &admission, questions, runtime)
    }

    /// Evaluate all questions for a durable catalog case after replay has
    /// recovered its exact admission decision.
    pub(crate) fn evaluate_catalog_questions<R: RelationalExpressionRuntime>(
        &self,
        case: RelationalCaseRef<'_>,
        admission: AdmissionDecision,
        questions: &RelationalQuestionEvaluationPlan,
        runtime: &mut R,
    ) -> Result<Box<[RelationalQuestionClassification]>, RelationalCaseExecutorError> {
        if case.relation_id() != self.relation_id {
            return Err(RelationalCaseExecutorError::CaseRelationMismatch);
        }
        let admission = RelationalAdmissionClassification {
            case_id: case.case_id(),
            decision: admission,
        };
        self.evaluate_question_values(
            case.case_id(),
            case.context(),
            case.before(),
            case.after(),
            &admission,
            questions,
            runtime,
        )
    }

    /// Evaluate exactly one explicitly addressed semantic question. This is
    /// the unfused work-node path: across Q questions it performs Q predicate
    /// evaluations, not Q complete passes over the question set.
    pub(crate) fn evaluate_catalog_question<R: RelationalExpressionRuntime>(
        &self,
        case: RelationalCaseRef<'_>,
        admission: AdmissionDecision,
        questions: &RelationalQuestionEvaluationPlan,
        question_id: QuestionId,
        runtime: &mut R,
    ) -> Result<Option<RelationalQuestionClassification>, RelationalCaseExecutorError> {
        if case.relation_id() != self.relation_id || !questions.matches_executor(self) {
            return Err(RelationalCaseExecutorError::QuestionPlanMismatch);
        }
        let question = questions
            .question(question_id)
            .ok_or(RelationalCaseExecutorError::QuestionPlanMismatch)?;
        if admission == AdmissionDecision::Rejected {
            return Ok(None);
        }
        let transition_bindings = [
            RelationalBoundValue {
                name: "context",
                value: case.context(),
            },
            RelationalBoundValue {
                name: "before",
                value: case.before(),
            },
            RelationalBoundValue {
                name: "after",
                value: case.after(),
            },
        ];
        self.evaluate_question_value(case.case_id(), question, &transition_bindings, runtime)
            .map(Some)
    }

    fn evaluate_question_values<R: RelationalExpressionRuntime>(
        &self,
        case_id: RelationalCaseId,
        context_value: &ExploreValue,
        before_value: &ExploreValue,
        after_value: &ExploreValue,
        admission: &RelationalAdmissionClassification,
        questions: &RelationalQuestionEvaluationPlan,
        runtime: &mut R,
    ) -> Result<Box<[RelationalQuestionClassification]>, RelationalCaseExecutorError> {
        if !questions.matches_executor(self) {
            return Err(RelationalCaseExecutorError::QuestionPlanMismatch);
        }
        if matches!(admission.decision, AdmissionDecision::Rejected) {
            return Ok(Box::new([]));
        }
        let transition_bindings = [
            RelationalBoundValue {
                name: "context",
                value: context_value,
            },
            RelationalBoundValue {
                name: "before",
                value: before_value,
            },
            RelationalBoundValue {
                name: "after",
                value: after_value,
            },
        ];
        let mut classifications = Vec::with_capacity(questions.questions.len());
        for question in questions.questions.iter().copied() {
            classifications.push(self.evaluate_question_value(
                case_id,
                question,
                &transition_bindings,
                runtime,
            )?);
        }
        Ok(classifications.into_boxed_slice())
    }

    fn evaluate_question_value<R: RelationalExpressionRuntime>(
        &self,
        case_id: RelationalCaseId,
        question: RelationalQuestionEvaluation,
        transition_bindings: &[RelationalBoundValue<'_>],
        runtime: &mut R,
    ) -> Result<RelationalQuestionClassification, RelationalCaseExecutorError> {
        let named_find = self
            .query
            .finds
            .get(question.find_index)
            .ok_or(RelationalCaseExecutorError::QuestionPlanMismatch)?;
        let decision = match &named_find.find {
            ExploreFindIr::All { .. } => SelectionDecision::Selected,
            ExploreFindIr::Matches { predicate, .. } => {
                if evaluate_boolean(
                    predicate,
                    transition_bindings,
                    runtime,
                    format!("find {} matches", question.find_index),
                )? {
                    SelectionDecision::Selected
                } else {
                    SelectionDecision::NotSelected
                }
            }
            ExploreFindIr::Violations { predicate, .. } => {
                if evaluate_boolean(
                    predicate,
                    transition_bindings,
                    runtime,
                    format!("find {} violations", question.find_index),
                )? {
                    SelectionDecision::NotSelected
                } else {
                    SelectionDecision::Selected
                }
            }
        };
        Ok(RelationalQuestionClassification {
            question_id: question.question_id,
            case_id,
            decision,
        })
    }

    /// Convenience for a local worker that evaluates admission once and then
    /// every unique semantic question against the same materialized case.
    pub(crate) fn classify<R: RelationalExpressionRuntime>(
        &self,
        source: &SourceRow,
        case: &RelationalConcreteCase,
        questions: &RelationalQuestionEvaluationPlan,
        runtime: &mut R,
    ) -> Result<RelationalCaseClassification, RelationalCaseExecutorError> {
        let admission = self.evaluate_admission(source, case, runtime)?;
        let question_classifications =
            self.evaluate_questions(source, case, &admission, questions, runtime)?;
        Ok(RelationalCaseClassification {
            admission,
            questions: question_classifications,
        })
    }

    fn evaluate_finite_domain<R: RelationalExpressionRuntime>(
        &self,
        domain: &ExploreFiniteDomainIr,
        source_bindings: &[RelationalBoundValue<'_>],
        runtime: &mut R,
    ) -> Result<RelationalFiniteFiber, RelationalCaseExecutorError> {
        match domain {
            ExploreFiniteDomainIr::Exact(domain) => RelationalFiniteFiber::exact(domain)
                .map_err(RelationalCaseExecutorError::FiniteFiber),
            ExploreFiniteDomainIr::Collection {
                expression,
                collection_ty,
                ..
            } => {
                let value = runtime
                    .evaluate(expression, collection_ty, source_bindings)
                    .map_err(|message| RelationalCaseExecutorError::Evaluation {
                        phase: "successor finite collection".to_string(),
                        message,
                    })?;
                RelationalFiniteFiber::from_collection_value(collection_ty, value)
                    .map_err(RelationalCaseExecutorError::FiniteFiber)
            }
            ExploreFiniteDomainIr::IntRange {
                start,
                end_exclusive,
            } => {
                let int_ty = Ty::Name("Int".to_string());
                let start =
                    runtime
                        .evaluate(start, &int_ty, source_bindings)
                        .map_err(|message| RelationalCaseExecutorError::Evaluation {
                            phase: "successor range start".to_string(),
                            message,
                        })?;
                let end_exclusive = runtime
                    .evaluate(end_exclusive, &int_ty, source_bindings)
                    .map_err(|message| RelationalCaseExecutorError::Evaluation {
                        phase: "successor range end".to_string(),
                        message,
                    })?;
                let ExploreValue::Int(start) = start else {
                    return Err(RelationalCaseExecutorError::ExpectedInt(
                        "successor range start",
                    ));
                };
                let ExploreValue::Int(end_exclusive) = end_exclusive else {
                    return Err(RelationalCaseExecutorError::ExpectedInt(
                        "successor range end",
                    ));
                };
                RelationalFiniteFiber::int_range(start, end_exclusive)
                    .map_err(RelationalCaseExecutorError::FiniteFiber)
            }
        }
    }

    fn validate_cursor_identity(
        &self,
        cursor: &RelationalSuccessorCursor,
    ) -> Result<(), RelationalCaseExecutorError> {
        if cursor.relation_id != self.relation_id {
            return Err(RelationalCaseExecutorError::CursorRelationMismatch);
        }
        Ok(())
    }

    fn validate_resumed_cursor<R: RelationalExpressionRuntime>(
        &self,
        cursor: &RelationalSuccessorCursor,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalSuccessorFiber, RelationalCaseExecutorError> {
        self.validate_cursor_identity(cursor)?;
        let fiber = self.successor_fiber(cursor.source_key, source, runtime)?;
        if cursor.next_successor_ordinal > fiber.cardinality() {
            return Err(RelationalCaseExecutorError::OrdinalBeyondCardinality {
                ordinal: cursor.next_successor_ordinal,
                cardinality: fiber.cardinality(),
            });
        }
        Ok(fiber)
    }

    fn validate_source(
        &self,
        source_key: SourceKey,
        source: &SourceRow,
    ) -> Result<(), RelationalCaseExecutorError> {
        let derived = SourceKey::derive(self.relation_id, source);
        if derived != source_key {
            return Err(RelationalCaseExecutorError::SourceKeyMismatch {
                claimed: source_key,
                derived,
            });
        }
        Ok(())
    }

    fn validate_case(
        &self,
        source: &SourceRow,
        case: &RelationalConcreteCase,
    ) -> Result<(), RelationalCaseExecutorError> {
        self.validate_source(case.source_key, source)?;
        let successor_key =
            SuccessorKey::derive(self.relation_id, case.source_key, &case.successor);
        if successor_key != case.successor_key {
            return Err(RelationalCaseExecutorError::SuccessorKeyMismatch {
                claimed: case.successor_key,
                derived: successor_key,
            });
        }
        let case_id = RelationalCaseId::derive(self.relation_id, case.source_key, successor_key);
        if case_id != case.case_id {
            return Err(RelationalCaseExecutorError::CaseIdMismatch {
                claimed: case.case_id,
                derived: case_id,
            });
        }
        Ok(())
    }

    /// Check that a case recovered from the durable catalog is exactly the
    /// same extensional coordinate as a freshly re-evaluated singleton. Row
    /// provenance may have accumulated additional convergent support, so this
    /// intentionally compares semantic values and all stable IDs rather than
    /// requiring provenance equality.
    pub(crate) fn validate_durable_case(
        &self,
        source: &SourceRow,
        candidate: &RelationalConcreteCase,
        durable: RelationalCaseRef<'_>,
    ) -> Result<(), RelationalCaseExecutorError> {
        self.validate_case(source, candidate)?;
        if durable.relation_id() != self.relation_id
            || durable.source_key() != candidate.source_key
            || durable.successor_key() != candidate.successor_key
            || durable.case_id() != candidate.case_id
            || durable.context() != source.context()
            || durable.before() != source.before()
            || durable.after() != candidate.successor.after()
        {
            return Err(RelationalCaseExecutorError::DurableCaseMismatch {
                case_id: candidate.case_id,
            });
        }
        Ok(())
    }
}

fn source_bindings(source: &SourceRow) -> [RelationalBoundValue<'_>; 2] {
    [
        RelationalBoundValue {
            name: "context",
            value: source.context(),
        },
        RelationalBoundValue {
            name: "before",
            value: source.before(),
        },
    ]
}

fn evaluate_boolean<R: RelationalExpressionRuntime>(
    expression: &Expr,
    bindings: &[RelationalBoundValue<'_>],
    runtime: &mut R,
    phase: String,
) -> Result<bool, RelationalCaseExecutorError> {
    let bool_ty = Ty::Name("Bool".to_string());
    let value = runtime
        .evaluate(expression, &bool_ty, bindings)
        .map_err(|message| RelationalCaseExecutorError::Evaluation {
            phase: phase.clone(),
            message,
        })?;
    let ExploreValue::Boolean(value) = value else {
        return Err(RelationalCaseExecutorError::ExpectedBoolean { phase });
    };
    Ok(value)
}

fn successor_provenance(
    relation_id: RelationId,
    source_key: SourceKey,
    member: &RelationalFiberMember,
) -> RelationProvenance {
    let after_digest = canonical_explore_value_digest(member.value());
    let mut lineage_preimage = CanonicalBytes::new(SUCCESSOR_LINEAGE_PREIMAGE_V1);
    lineage_preimage.u32(RELATIONAL_SUCCESSOR_CURSOR_VERSION);
    lineage_preimage.digest(relation_id.bytes());
    lineage_preimage.digest(source_key.bytes());
    lineage_preimage.u128(member.canonical_ordinal());
    lineage_preimage.digest(after_digest);
    let lineage = RelationLineageId::from_canonical_preimage(lineage_preimage.as_slice());

    let support = member.raw_support_ordinals().iter().map(|raw_ordinal| {
        let mut support_preimage = CanonicalBytes::new(SUCCESSOR_SUPPORT_PREIMAGE_V1);
        support_preimage.u32(RELATIONAL_SUCCESSOR_CURSOR_VERSION);
        support_preimage.digest(relation_id.bytes());
        support_preimage.digest(source_key.bytes());
        support_preimage.u128(member.canonical_ordinal());
        support_preimage.u128(*raw_ordinal);
        support_preimage.digest(after_digest);
        RelationSupportId::from_canonical_preimage(support_preimage.as_slice())
    });
    RelationProvenance::new([lineage], support)
}

/// The successor row constructor is a deterministic mapping of the exact
/// canonical member fiber under `(relation, source)`. Committing that checked
/// decoder plan avoids a second pass over a large range or finite-type product
/// at exhaustion while still excluding scheduler and journal arrival order.
fn successor_fiber_rows_commitment(
    fiber: &RelationalSuccessorFiber,
) -> Result<[u8; 32], RelationalCaseExecutorError> {
    let mut hasher = ExhaustionHasher::new(SUCCESSOR_FIBER_ROWS_COMMITMENT_V1);
    hasher.u32(RELATIONAL_SUCCESSOR_CURSOR_VERSION);
    hasher.digest(fiber.relation_id.bytes());
    hasher.digest(fiber.source_key.bytes());
    hasher.u128(fiber.cardinality());
    hasher.digest(
        fiber
            .finite
            .canonical_member_commitment()
            .map_err(RelationalCaseExecutorError::FiniteFiber)?,
    );
    Ok(hasher.finish())
}

fn derive_successor_fiber_exhaustion_receipt_id(
    version: u32,
    relation_id: RelationId,
    source_key: SourceKey,
    terminal_ordinal: u128,
    emitted_row_count: u128,
    emitted_rows_commitment: [u8; 32],
) -> SuccessorFiberExhaustionReceiptId {
    let mut hasher = ExhaustionHasher::new(SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_HASH_V1);
    hasher.u32(version);
    hasher.digest(relation_id.bytes());
    hasher.digest(source_key.bytes());
    hasher.u128(terminal_ordinal);
    hasher.u128(emitted_row_count);
    hasher.digest(emitted_rows_commitment);
    SuccessorFiberExhaustionReceiptId(hasher.finish())
}

struct ExhaustionHasher(Sha256);

impl ExhaustionHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    fn new(domain: &[u8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(domain.len() as u64).to_be_bytes());
        bytes.extend_from_slice(domain);
        Self(bytes)
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.extend_from_slice(&digest);
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseExecutorError {
    InvalidQuery(String),
    Evaluation {
        phase: String,
        message: String,
    },
    ExpectedBoolean {
        phase: String,
    },
    ExpectedInt(&'static str),
    SourceKeyMismatch {
        claimed: SourceKey,
        derived: SourceKey,
    },
    SuccessorKeyMismatch {
        claimed: SuccessorKey,
        derived: SuccessorKey,
    },
    CaseIdMismatch {
        claimed: RelationalCaseId,
        derived: RelationalCaseId,
    },
    AdmissionCaseMismatch {
        admission_case_id: RelationalCaseId,
        case_id: RelationalCaseId,
    },
    QuestionCountMismatch {
        finds: usize,
        question_ids: usize,
    },
    QuestionAliasPolarityMismatch {
        question_id: QuestionId,
    },
    QuestionPlanMismatch,
    DurableCaseMismatch {
        case_id: RelationalCaseId,
    },
    CaseRelationMismatch,
    StaticSingletonShapeMismatch,
    NotSuccessorWork,
    WorkRelationMismatch,
    CursorRelationMismatch,
    UnsupportedCursorVersion {
        actual: u32,
        expected: u32,
    },
    SuccessorFiberMismatch,
    ExhaustionBeforeTerminal {
        terminal_ordinal: u128,
        cardinality: u128,
    },
    UnsupportedExhaustionReceiptVersion {
        actual: u32,
        expected: u32,
    },
    ExhaustionReceiptCountMismatch {
        terminal_ordinal: u128,
        emitted_row_count: u128,
    },
    ExhaustionReceiptIdMismatch {
        claimed: SuccessorFiberExhaustionReceiptId,
        derived: SuccessorFiberExhaustionReceiptId,
    },
    OrdinalBeyondCardinality {
        ordinal: u128,
        cardinality: u128,
    },
    CursorOrdinalOverflow,
    FiniteFiber(RelationalSourceExecutorError),
}

impl fmt::Display for RelationalCaseExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => {
                write!(formatter, "invalid relational case query: {message}")
            }
            Self::Evaluation { phase, message } => {
                write!(
                    formatter,
                    "relational case {phase} evaluation failed: {message}"
                )
            }
            Self::ExpectedBoolean { phase } => {
                write!(formatter, "relational case {phase} did not produce Bool")
            }
            Self::ExpectedInt(subject) => write!(formatter, "{subject} did not produce Int"),
            Self::SourceKeyMismatch { .. } => {
                formatter.write_str("source row does not match its claimed SourceKey")
            }
            Self::SuccessorKeyMismatch { .. } => {
                formatter.write_str("successor row does not match its claimed SuccessorKey")
            }
            Self::CaseIdMismatch { .. } => {
                formatter.write_str("successor coordinate does not match its claimed CaseId")
            }
            Self::AdmissionCaseMismatch { .. } => {
                formatter.write_str("admission evidence belongs to a different case")
            }
            Self::QuestionCountMismatch {
                finds,
                question_ids,
            } => write!(
                formatter,
                "question identity count {question_ids} does not match authored FIND count {finds}"
            ),
            Self::QuestionAliasPolarityMismatch { question_id } => write!(
                formatter,
                "equivalent FIND aliases for question {question_id:?} disagree on polarity"
            ),
            Self::QuestionPlanMismatch => formatter
                .write_str("question evaluation plan does not belong to this checked query"),
            Self::DurableCaseMismatch { .. } => formatter
                .write_str("durable case does not match the re-evaluated singleton transition"),
            Self::CaseRelationMismatch => {
                formatter.write_str("durable case belongs to a different relation")
            }
            Self::StaticSingletonShapeMismatch => formatter
                .write_str("statically singleton TO relation did not have exactly one member"),
            Self::NotSuccessorWork => {
                formatter.write_str("work node is not concrete successor expansion")
            }
            Self::WorkRelationMismatch => {
                formatter.write_str("successor work node belongs to a different relation")
            }
            Self::CursorRelationMismatch => {
                formatter.write_str("successor cursor belongs to a different relation")
            }
            Self::UnsupportedCursorVersion { actual, expected } => write!(
                formatter,
                "unsupported relational successor cursor version {actual}; expected {expected}"
            ),
            Self::SuccessorFiberMismatch => {
                formatter.write_str("successor fiber belongs to a different source")
            }
            Self::ExhaustionBeforeTerminal {
                terminal_ordinal,
                cardinality,
            } => write!(
                formatter,
                "successor exhaustion ordinal {terminal_ordinal} does not equal fiber cardinality {cardinality}"
            ),
            Self::UnsupportedExhaustionReceiptVersion { actual, expected } => write!(
                formatter,
                "unsupported successor exhaustion receipt version {actual}; expected {expected}"
            ),
            Self::ExhaustionReceiptCountMismatch {
                terminal_ordinal,
                emitted_row_count,
            } => write!(
                formatter,
                "successor exhaustion ordinal {terminal_ordinal} does not equal emitted row count {emitted_row_count}"
            ),
            Self::ExhaustionReceiptIdMismatch { .. } => formatter
                .write_str("successor exhaustion receipt ID does not match its semantic content"),
            Self::OrdinalBeyondCardinality {
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "successor ordinal {ordinal} exceeds canonical cardinality {cardinality}"
            ),
            Self::CursorOrdinalOverflow => {
                formatter.write_str("successor cursor cannot advance beyond u128")
            }
            Self::FiniteFiber(error) => write!(formatter, "invalid successor fiber: {error}"),
        }
    }
}

impl Error for RelationalCaseExecutorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FiniteFiber(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::relation::RelationCatalogBuilder;
    use super::super::relational_ir::{
        ExploreAdmissionIr, ExploreNamedFindIr, ExploreSourceBindingIr, ExploreSourceBindingKindIr,
        ExploreSourceBindingRoleIr, ExploreSourceProducerRoleIr, ExploreSourceRelationIr,
        ExploreSuccessorRelationIr,
    };
    use super::*;
    use crate::{ExprKind, Literal, Span, EXPLORE_RELATION_NORMALIZATION_VERSION};

    #[derive(Default)]
    struct TestRuntime {
        evaluations: usize,
        seen_bindings: Vec<Vec<String>>,
    }

    impl RelationalExpressionRuntime for TestRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            bindings: &[RelationalBoundValue<'_>],
        ) -> Result<ExploreValue, String> {
            self.evaluations += 1;
            self.seen_bindings.push(
                bindings
                    .iter()
                    .map(|binding| binding.name.to_string())
                    .collect(),
            );
            fn evaluate(
                expression: &Expr,
                bindings: &[RelationalBoundValue<'_>],
            ) -> Result<ExploreValue, String> {
                match &expression.kind {
                    ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
                    ExprKind::Lit(Literal::Bool(value)) => Ok(ExploreValue::Boolean(*value)),
                    ExprKind::Var(name) => bindings
                        .iter()
                        .find(|binding| binding.name == name.as_str())
                        .map(|binding| binding.value.clone())
                        .ok_or_else(|| format!("unbound test name {name}")),
                    ExprKind::List(values) => values
                        .iter()
                        .map(|value| evaluate(value, bindings))
                        .collect::<Result<Vec<_>, _>>()
                        .map(ExploreValue::List),
                    _ => Err("unsupported test expression".to_string()),
                }
            }
            evaluate(expression, bindings)
        }
    }

    fn int_ty() -> Ty {
        Ty::Name("Int".to_string())
    }

    fn list_int_ty() -> Ty {
        Ty::App(Box::new(Ty::Name("List".to_string())), vec![int_ty()])
    }

    fn int(value: i64) -> Expr {
        Expr::unspanned(ExprKind::Lit(Literal::Int(value)))
    }

    fn boolean(value: bool) -> Expr {
        Expr::unspanned(ExprKind::Lit(Literal::Bool(value)))
    }

    fn var(name: &str) -> Expr {
        Expr::unspanned(ExprKind::Var(name.to_string()))
    }

    fn list(values: &[i64]) -> Expr {
        Expr::unspanned(ExprKind::List(values.iter().copied().map(int).collect()))
    }

    fn source_relation() -> ExploreSourceRelationIr {
        ExploreSourceRelationIr {
            normalization_version: EXPLORE_RELATION_NORMALIZATION_VERSION,
            multiplicity: ExploreRelationMultiplicity::SetNormalized,
            bindings: vec![
                ExploreSourceBindingIr {
                    binding_index: 0,
                    name: "context".to_string(),
                    value_ty: int_ty(),
                    role: ExploreSourceBindingRoleIr::Context,
                    producer_role: ExploreSourceProducerRoleIr::Given,
                    dependencies: Box::new([]),
                    kind: ExploreSourceBindingKindIr::Singleton { value: int(7) },
                    span: Span::dummy(),
                },
                ExploreSourceBindingIr {
                    binding_index: 1,
                    name: "before".to_string(),
                    value_ty: int_ty(),
                    role: ExploreSourceBindingRoleIr::Before,
                    producer_role: ExploreSourceProducerRoleIr::Given,
                    dependencies: Box::new([]),
                    kind: ExploreSourceBindingKindIr::Singleton { value: int(10) },
                    span: Span::dummy(),
                },
            ]
            .into_boxed_slice(),
            context_binding_index: 0,
            before_binding_index: 1,
            context_ty: int_ty(),
            before_ty: int_ty(),
        }
    }

    fn query(
        successor: ExploreSuccessorKindIr,
        admissions: Vec<bool>,
        find: ExploreFindIr,
    ) -> ExploreQueryIr {
        ExploreQueryIr {
            name: "case-leaf".to_string(),
            source: source_relation(),
            successor: ExploreSuccessorRelationIr {
                multiplicity: ExploreRelationMultiplicity::SetNormalized,
                after_ty: int_ty(),
                kind: successor,
                span: Span::dummy(),
            },
            admissions: admissions
                .into_iter()
                .enumerate()
                .map(|(admission_index, value)| {
                    admission(admission_index, ExploreAdmissionScope::Transition, value)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            finds: vec![ExploreNamedFindIr {
                name: "cases".to_string(),
                find,
            }]
            .into_boxed_slice(),
            analysis: Box::new([]),
            observation_demands: Box::new([]),
            starter_projections: Box::new([]),
            transition_graphs: Box::new([]),
            span: Span::dummy(),
        }
    }

    fn question_id(relation_id: RelationId, tag: u8, polarity: FindPolarity) -> QuestionId {
        let admission_id = super::super::relation::AdmissionId::from_canonical_admission_digest(
            relation_id,
            [0xA5; 32],
        );
        QuestionId::from_canonical_find_digest(admission_id, [tag; 32], polarity)
    }

    fn admission(
        admission_index: usize,
        scope: ExploreAdmissionScope,
        value: bool,
    ) -> ExploreAdmissionIr {
        ExploreAdmissionIr {
            admission_index,
            scope,
            predicate: boolean(value),
            span: Span::dummy(),
        }
    }

    fn finite_list(values: &[i64]) -> ExploreSuccessorKindIr {
        ExploreSuccessorKindIr::Finite {
            domain: ExploreFiniteDomainIr::Collection {
                expression: list(values),
                collection_ty: list_int_ty(),
                element_ty: int_ty(),
            },
        }
    }

    fn source(relation_id: RelationId) -> (SourceKey, SourceRow) {
        let row = SourceRow::new(
            ExploreValue::Int(7),
            ExploreValue::Int(10),
            RelationProvenance::default(),
        );
        (SourceKey::derive(relation_id, &row), row)
    }

    fn first_case(
        relation_id: RelationId,
        query: &ExploreQueryIr,
    ) -> (SourceRow, RelationalConcreteCase) {
        let executor = RelationalCaseExecutor::new(relation_id, query).unwrap();
        let (source_key, source) = source(relation_id);
        let cursor = executor.root_cursor(source_key, &source).unwrap();
        let mut runtime = TestRuntime::default();
        let RelationalSuccessorAdvance::Yielded { case, .. } =
            executor.advance(&cursor, &source, &mut runtime).unwrap()
        else {
            panic!("singleton successor must yield one case")
        };
        (source, case)
    }

    #[test]
    fn zero_singleton_and_many_fibers_have_exact_exhaustion_ordinals() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"successor-cardinality");
        let (source_key, source) = source(relation_id);
        let fixtures = [
            (finite_list(&[]), 0),
            (
                ExploreSuccessorKindIr::Singleton {
                    value: var("before"),
                },
                1,
            ),
            (finite_list(&[12, 11, 12]), 2),
        ];

        for (successor, expected) in fixtures {
            let query = query(
                successor,
                vec![],
                ExploreFindIr::All {
                    span: Span::dummy(),
                },
            );
            let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
            let cursor = executor.root_cursor(source_key, &source).unwrap();
            let mut runtime = TestRuntime::default();
            let fiber = executor
                .successor_fiber(source_key, &source, &mut runtime)
                .unwrap();
            assert_eq!(fiber.cardinality(), expected);
            let resumed = executor
                .resume_snapshot(
                    RelationalSuccessorCursorSnapshot {
                        next_successor_ordinal: expected,
                        ..cursor.snapshot()
                    },
                    &source,
                    &mut runtime,
                )
                .unwrap();
            assert!(matches!(
                executor.advance_in_fiber(&resumed, &fiber).unwrap(),
                RelationalSuccessorAdvance::Exhausted { cardinality, .. }
                    if cardinality == expected
            ));
        }
    }

    #[test]
    fn actual_terminal_transition_issues_content_stable_successor_exhaustion_receipt() {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(b"successor-exhaustion-receipt");
        let query = query(
            finite_list(&[12, 11, 12]),
            vec![],
            ExploreFindIr::All {
                span: Span::dummy(),
            },
        );
        let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let (source_key, source) = source(relation_id);
        let start = executor.root_cursor(source_key, &source).unwrap();
        let mut runtime = TestRuntime::default();
        let fiber = executor
            .successor_fiber(source_key, &source, &mut runtime)
            .unwrap();
        let terminal = executor
            .resume_snapshot(
                RelationalSuccessorCursorSnapshot {
                    next_successor_ordinal: fiber.cardinality(),
                    ..start.snapshot()
                },
                &source,
                &mut runtime,
            )
            .unwrap();

        let RelationalSuccessorAdvance::Exhausted {
            cursor,
            cardinality,
            receipt,
        } = executor.advance_in_fiber(&terminal, &fiber).unwrap()
        else {
            panic!("the terminal cursor must reach actual successor exhaustion")
        };
        assert_eq!(cursor, terminal);
        assert_eq!(cardinality, 2);
        assert_eq!(
            receipt.version(),
            SUCCESSOR_FIBER_EXHAUSTION_RECEIPT_VERSION
        );
        assert_eq!(receipt.relation_id(), relation_id);
        assert_eq!(receipt.source_key(), source_key);
        assert_eq!(receipt.terminal_ordinal(), 2);
        assert_eq!(receipt.emitted_row_count(), 2);
        assert_eq!(
            receipt.emitted_rows_commitment(),
            successor_fiber_rows_commitment(&fiber).unwrap()
        );
        receipt.validate_identity().unwrap();

        let RelationalSuccessorAdvance::Exhausted {
            receipt: replayed, ..
        } = executor.advance_in_fiber(&terminal, &fiber).unwrap()
        else {
            panic!("replaying the terminal transition must remain exhausted")
        };
        assert_eq!(receipt, replayed);
        assert_eq!(receipt.id(), replayed.id());
        assert_eq!(receipt.id().bytes(), replayed.id().bytes());
    }

    #[test]
    fn duplicate_to_paths_union_support_without_inflating_cases_or_resume_ids() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"successor-support");
        let query = query(
            finite_list(&[2, 1, 2]),
            vec![],
            ExploreFindIr::All {
                span: Span::dummy(),
            },
        );
        let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let (source_key, source) = source(relation_id);
        let cursor = executor.root_cursor(source_key, &source).unwrap();
        let mut runtime = TestRuntime::default();
        let fiber = executor
            .successor_fiber(source_key, &source, &mut runtime)
            .unwrap();

        let RelationalSuccessorAdvance::Yielded {
            case: first,
            resume,
            ..
        } = executor.advance_in_fiber(&cursor, &fiber).unwrap()
        else {
            panic!("first canonical successor must exist")
        };
        assert_eq!(first.successor().after(), &ExploreValue::Int(1));
        assert_eq!(first.successor().provenance().support().len(), 1);

        let resumed = executor
            .resume_snapshot(resume.snapshot(), &source, &mut runtime)
            .unwrap();
        let RelationalSuccessorAdvance::Yielded {
            member,
            case: second,
            resume: exhausted,
        } = executor.advance(&resumed, &source, &mut runtime).unwrap()
        else {
            panic!("second canonical successor must exist")
        };
        assert_eq!(member.raw_support_ordinals(), &[0, 2]);
        assert_eq!(second.successor().after(), &ExploreValue::Int(2));
        assert_eq!(second.successor().provenance().support().len(), 2);
        assert_ne!(first.case_id(), second.case_id());

        let replayed_second = executor.advance_in_fiber(&resumed, &fiber).unwrap();
        assert!(matches!(
            replayed_second,
            RelationalSuccessorAdvance::Yielded { case, .. }
                if case.case_id() == second.case_id()
                    && case.successor_key() == second.successor_key()
        ));
        assert!(matches!(
            executor.advance_in_fiber(&exhausted, &fiber).unwrap(),
            RelationalSuccessorAdvance::Exhausted { cardinality: 2, .. }
        ));
    }

    #[test]
    fn rejection_emits_no_find_evidence_and_all_polarities_are_distinct() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"case-classification");
        let rejected_query = query(
            ExploreSuccessorKindIr::Singleton { value: int(11) },
            vec![true, false, true],
            ExploreFindIr::Matches {
                predicate: boolean(true),
                span: Span::dummy(),
            },
        );
        let (source, case) = first_case(relation_id, &rejected_query);
        let executor = RelationalCaseExecutor::new(relation_id, &rejected_query).unwrap();
        let rejected_question_id = question_id(relation_id, 1, FindPolarity::Matches);
        let rejected_questions = executor
            .question_evaluation_plan(&[rejected_question_id])
            .unwrap();
        let mut runtime = TestRuntime::default();
        let rejected = executor
            .evaluate_admission(&source, &case, &mut runtime)
            .unwrap();
        assert_eq!(rejected.decision(), AdmissionDecision::Rejected);
        assert!(executor
            .evaluate_questions(&source, &case, &rejected, &rejected_questions, &mut runtime,)
            .unwrap()
            .is_empty());
        assert!(matches!(
            rejected.event(),
            RelationalJournalEvent::Evidence(
                super::super::relational_journal::RelationalEvidenceEvent::AdmissionClassified {
                    decision: AdmissionDecision::Rejected,
                    ..
                }
            )
        ));
        assert_eq!(runtime.evaluations, 2);

        let cases = [
            (
                ExploreFindIr::All {
                    span: Span::dummy(),
                },
                SelectionDecision::Selected,
            ),
            (
                ExploreFindIr::Matches {
                    predicate: boolean(false),
                    span: Span::dummy(),
                },
                SelectionDecision::NotSelected,
            ),
            (
                ExploreFindIr::Violations {
                    predicate: boolean(false),
                    span: Span::dummy(),
                },
                SelectionDecision::Selected,
            ),
            (
                ExploreFindIr::Violations {
                    predicate: boolean(true),
                    span: Span::dummy(),
                },
                SelectionDecision::NotSelected,
            ),
        ];
        for (tag, (find, expected)) in cases.into_iter().enumerate() {
            let polarity = find.polarity();
            let query = query(
                ExploreSuccessorKindIr::Singleton { value: int(11) },
                vec![],
                find,
            );
            let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
            let question_id = question_id(relation_id, tag as u8 + 2, polarity);
            let questions = executor.question_evaluation_plan(&[question_id]).unwrap();
            let mut runtime = TestRuntime::default();
            let classified = executor
                .classify(&source, &case, &questions, &mut runtime)
                .unwrap();
            assert_eq!(classified.admission(), AdmissionDecision::Admitted);
            assert_eq!(classified.question_decision(question_id), Some(expected));
            assert_eq!(classified.question_events().len(), 1);
        }
    }

    #[test]
    fn plural_questions_share_one_case_and_collapse_semantic_aliases() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"plural-questions");
        let mut query = query(
            ExploreSuccessorKindIr::Singleton { value: int(11) },
            vec![],
            ExploreFindIr::Matches {
                predicate: boolean(true),
                span: Span::dummy(),
            },
        );
        query.finds = vec![
            ExploreNamedFindIr {
                name: "positive".to_string(),
                find: ExploreFindIr::Matches {
                    predicate: boolean(true),
                    span: Span::dummy(),
                },
            },
            ExploreNamedFindIr {
                name: "positive_alias".to_string(),
                find: ExploreFindIr::Matches {
                    predicate: boolean(true),
                    span: Span::dummy(),
                },
            },
            ExploreNamedFindIr {
                name: "violations".to_string(),
                find: ExploreFindIr::Violations {
                    predicate: boolean(true),
                    span: Span::dummy(),
                },
            },
        ]
        .into_boxed_slice();
        let positive = question_id(relation_id, 21, FindPolarity::Matches);
        let violations = question_id(relation_id, 22, FindPolarity::Violations);
        let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let questions = executor
            .question_evaluation_plan(&[positive, positive, violations])
            .unwrap();
        assert_eq!(questions.unique_question_count(), 2);
        let planned_question_ids = questions.question_ids().collect::<Vec<_>>();
        assert!(planned_question_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1]));

        let (source, case) = first_case(relation_id, &query);
        let mut runtime = TestRuntime::default();
        let classified = executor
            .classify(&source, &case, &questions, &mut runtime)
            .unwrap();

        assert_eq!(classified.admission(), AdmissionDecision::Admitted);
        assert_eq!(
            classified.question_decision(positive),
            Some(SelectionDecision::Selected)
        );
        assert_eq!(
            classified.question_decision(violations),
            Some(SelectionDecision::NotSelected)
        );
        assert_eq!(classified.question_evidence().len(), 2);
        assert_eq!(runtime.evaluations, 2);
    }

    #[test]
    fn zero_questions_classify_accepted_and_rejected_cases_without_find_evidence() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"zero-questions");
        let fixtures = [
            (Vec::new(), AdmissionDecision::Admitted, 0),
            (vec![false], AdmissionDecision::Rejected, 1),
        ];

        for (admissions, expected_admission, expected_evaluations) in fixtures {
            let mut query = query(
                ExploreSuccessorKindIr::Singleton { value: int(11) },
                admissions,
                ExploreFindIr::All {
                    span: Span::dummy(),
                },
            );
            query.finds = Box::new([]);
            let (source, case) = first_case(relation_id, &query);
            let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
            let questions = executor.question_evaluation_plan(&[]).unwrap();
            assert_eq!(questions.unique_question_count(), 0);
            assert_eq!(questions.question_ids().len(), 0);

            let mut runtime = TestRuntime::default();
            let classified = executor
                .classify(&source, &case, &questions, &mut runtime)
                .unwrap();
            assert_eq!(classified.admission(), expected_admission);
            assert!(classified.question_evidence().is_empty());
            assert_eq!(classified.question_events().len(), 0);
            assert_eq!(runtime.evaluations, expected_evaluations);
        }
    }

    #[test]
    fn foreign_addressed_question_fails_before_rejected_case_short_circuit() {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(b"foreign-addressed-question");
        let query = query(
            ExploreSuccessorKindIr::Singleton { value: int(11) },
            vec![],
            ExploreFindIr::Matches {
                predicate: boolean(true),
                span: Span::dummy(),
            },
        );
        let (source, case) = first_case(relation_id, &query);
        let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let local = question_id(relation_id, 31, FindPolarity::Matches);
        let foreign = question_id(relation_id, 32, FindPolarity::Matches);
        let questions = executor.question_evaluation_plan(&[local]).unwrap();

        let mut catalog = RelationCatalogBuilder::new(relation_id);
        let source_key = catalog.insert_source(source).unwrap();
        let (_, catalog_case_id) = catalog
            .insert_successor(source_key, case.successor().clone())
            .unwrap();
        assert_eq!(catalog_case_id, case.case_id());
        let catalog_case = catalog.case(catalog_case_id).unwrap();

        let mut runtime = TestRuntime::default();
        assert_eq!(
            executor.evaluate_catalog_question(
                catalog_case,
                AdmissionDecision::Rejected,
                &questions,
                foreign,
                &mut runtime,
            ),
            Err(RelationalCaseExecutorError::QuestionPlanMismatch)
        );
        assert_eq!(runtime.evaluations, 0);
    }

    #[test]
    fn admission_scopes_and_find_share_runtime_with_exact_role_environments() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"scoped-classification");
        let mut query = query(
            ExploreSuccessorKindIr::Singleton { value: int(11) },
            vec![],
            ExploreFindIr::Matches {
                predicate: boolean(true),
                span: Span::dummy(),
            },
        );
        query.admissions = vec![
            admission(0, ExploreAdmissionScope::Before, true),
            admission(1, ExploreAdmissionScope::After, true),
            admission(2, ExploreAdmissionScope::Transition, true),
        ]
        .into_boxed_slice();
        let (source, case) = first_case(relation_id, &query);
        let executor = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let question_id = question_id(relation_id, 9, FindPolarity::Matches);
        let questions = executor.question_evaluation_plan(&[question_id]).unwrap();
        let mut runtime = TestRuntime::default();

        let classified = executor
            .classify(&source, &case, &questions, &mut runtime)
            .unwrap();
        assert_eq!(
            classified.question_decision(question_id),
            Some(SelectionDecision::Selected)
        );
        assert_eq!(
            runtime.seen_bindings,
            vec![
                vec!["context".to_string(), "before".to_string()],
                vec!["context".to_string(), "after".to_string()],
                vec![
                    "context".to_string(),
                    "before".to_string(),
                    "after".to_string(),
                ],
                vec![
                    "context".to_string(),
                    "before".to_string(),
                    "after".to_string(),
                ],
            ]
        );
    }
}
