//! Bounded post-FIND execution for selected-case result views.
//!
//! This is a planner, not a run loop. One call inspects an authenticated
//! journal prefix and emits at most one selected-result quantum: spec
//! registration, a bounded ordered row-evidence chunk (optionally followed by
//! its exact input seal), an empty/input-only seal, or terminal publication.
//! The outer durable loop owns append, retries, deadlines, and resources.
//!
//! Row evidence remains owned exactly once by the analysis journal. At the
//! publication boundary it is borrowed and deterministically re-evaluated
//! against relation-owned case bindings before an ephemeral reducer is
//! finished. The resulting output-record cache is invocation-local and may be
//! discarded; durable resume is defined solely by the journaled prefix.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView};

use super::relation::{
    AdmissionId, MechanismRequestId, QuestionId, RelationId, RelationalCaseId, SelectionDecision,
    SourceKey, ViewId,
};
use super::relational_analysis_catalog::{
    RelationalAnalysisCatalogBuilder, RelationalAnalysisCatalogError, RelationalAnalysisLayerStatus,
};
use super::relational_analysis_journal::{
    RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError, RelationalSelectedQuestionSeal,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanError, RelationalAnalysisPlanRoot, RelationalResolvedResultInput,
};
use super::relational_certified_source_summary::{
    certify_relational_source_summary, RelationalCertifiedSourceSummaryArtifact,
    RelationalCertifiedSourceSummaryCertification, RelationalCertifiedSourceSummaryError,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::relational_ir::{ExploreAnalysisNodeIr, ExploreResultInputIr};
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalSchedulerView,
};
use super::relational_result_executor::{
    RelationalResultExecutor, RelationalResultExecutorError, RelationalResultExpressionRuntime,
};
use super::result_evidence::{
    RelationalResultEvidenceCatalogBuilder, RelationalResultEvidenceRecord,
    RelationalResultEvidenceRoot,
};
use super::result_projection::{
    IndexedResultProjectionRecord, ResultProjectionCatalogBuilder, ResultProjectionError,
    ValidatedResultProjectionPrefix,
};
use super::result_view::{ResultViewInputRowId, ResultViewRoot};

/// Operational description of one emitted result quantum. None of these
/// fields participates in semantic identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultStepQuantum {
    RegisterSourceSpec {
        view_id: ViewId,
    },
    EvaluateSourceRows {
        view_id: ViewId,
        first_source_key: SourceKey,
        row_count: NonZeroU16,
        seals_input: bool,
    },
    SealSourceInput {
        view_id: ViewId,
    },
    CertifySourceSummary {
        view_id: ViewId,
        exact_input_count: u128,
    },
    PublishSourceProjectionRecords {
        view_id: ViewId,
        first_ordinal: u128,
        record_count: NonZeroU16,
    },
    PublishSourceResult {
        view_id: ViewId,
        result_root: ResultViewRoot,
    },
    RegisterSelectedSpec {
        view_id: ViewId,
    },
    EvaluateSelectedRows {
        view_id: ViewId,
        first_case_id: RelationalCaseId,
        row_count: NonZeroU16,
        seals_input: bool,
    },
    SealSelectedInput {
        view_id: ViewId,
    },
    PublishSelectedProjectionRecords {
        view_id: ViewId,
        first_ordinal: u128,
        record_count: NonZeroU16,
    },
    PublishSelectedResult {
        view_id: ViewId,
        result_root: ResultViewRoot,
    },
}

/// One head-bound ordered batch. A durable adapter may append a proper prefix:
/// evidence rediscovery is idempotent, and the input seal is always after all
/// row consequences in the same batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalResultStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalResultStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalResultStepQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

/// Honest non-progress states. Mechanism-incidence result inputs are explicit
/// here rather than silently interpreted as selected cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultStepQuiescence {
    AwaitingSelectedQuestion {
        question_id: QuestionId,
    },
    AwaitingSourceMaterialization {
        view_id: ViewId,
    },
    SelectedResultsComplete,
    DeferredMechanismIncidence {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    AnalysisAlreadyClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultStepOutcome {
    Emitted(RelationalResultStepBatch),
    Quiescent(RelationalResultStepQuiescence),
}

struct SelectedResultLayer<'ir> {
    input: RelationalResolvedResultInput,
    question_id: QuestionId,
    executor: RelationalResultExecutor<'ir>,
}

struct SourceResultLayer<'ir> {
    input: RelationalResolvedResultInput,
    executor: RelationalResultExecutor<'ir>,
}

struct CachedSelectedProjection {
    evidence_root: RelationalResultEvidenceRoot,
    result_root: ResultViewRoot,
    records: Box<[IndexedResultProjectionRecord]>,
    validated_prefix: Option<ValidatedResultProjectionPrefix>,
}

/// Checked-query-bound selected-result scheduler.
pub(crate) struct RelationalResultStepDriver<'query> {
    checked: CheckedExploreQueryView<'query>,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_ids: Box<[QuestionId]>,
    analysis_plan_root: RelationalAnalysisPlanRoot,
    sources: BTreeMap<ViewId, SourceResultLayer<'query>>,
    selected: BTreeMap<ViewId, SelectedResultLayer<'query>>,
    first_deferred_incidence: Option<(ViewId, MechanismRequestId)>,
    /// Purely operational CPU bound; absent from every spec, event identity,
    /// journal contract, and result root.
    max_rows_per_quantum: NonZeroU16,
    /// Invocation-local projection cache. It is not semantic state: after a
    /// restart the exact reduction is recomputed once from durable evidence,
    /// then bounded publication resumes at the journaled record ordinal.
    publication_cache: RefCell<BTreeMap<ViewId, CachedSelectedProjection>>,
    /// Invocation-local offsets into the journal-rebuilt selected discovery
    /// index. Durable evidence remains the resume authority; these offsets
    /// merely prevent each open-prefix turn from rescanning all earlier rows.
    selected_discovery_cursors: RefCell<BTreeMap<ViewId, usize>>,
    source_cursors: RefCell<BTreeMap<ViewId, usize>>,
    /// Invocation-local cold-replay gate. A durable summary artifact is not
    /// consumed merely because its self-authenticating bytes replayed: once
    /// per invocation it must still match the current compiler theorem and
    /// the bounded representatives evaluated by the checked runtime.
    rebound_certified_source_summaries: RefCell<BTreeSet<ViewId>>,
}

impl<'query> RelationalResultStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalResultStepDriverError> {
        Self::from_checked_with_max_rows_per_quantum(checked, NonZeroU16::MIN)
    }

    pub(crate) fn from_checked_with_max_rows_per_quantum(
        checked: &'query CheckedExploreQueryView<'_>,
        max_rows_per_quantum: NonZeroU16,
    ) -> Result<Self, RelationalResultStepDriverError> {
        let plan = RelationalAnalysisPlan::from_checked(checked)?;
        let mut sources = BTreeMap::new();
        let mut selected = BTreeMap::new();
        let mut deferred = BTreeMap::new();

        for (node, identity) in checked.analysis_nodes() {
            let (
                ExploreAnalysisNodeIr::Result(result),
                CheckedExploreAnalysisIdentity::View { view_id, .. },
            ) = (node, identity)
            else {
                continue;
            };
            let registration = plan
                .registration(RelationalAnalysisLayerId::Result(*view_id))
                .ok_or(RelationalResultStepDriverError::AnalysisLayerMissing(
                    RelationalAnalysisLayerId::Result(*view_id),
                ))?;
            let RelationalAnalysisLayerRegistration::Result(registration) = registration else {
                return Err(RelationalResultStepDriverError::AnalysisLayerKindMismatch(
                    RelationalAnalysisLayerId::Result(*view_id),
                ));
            };

            match (&result.input, registration.input()) {
                (
                    ExploreResultInputIr::Sources,
                    input @ RelationalResolvedResultInput::Sources(relation_id),
                ) => {
                    if relation_id != checked.relation_id() {
                        return Err(RelationalResultStepDriverError::JournalScopeMismatch);
                    }
                    let layer = SourceResultLayer {
                        input,
                        executor: RelationalResultExecutor::lower(*view_id, result)?,
                    };
                    if sources.insert(*view_id, layer).is_some() {
                        return Err(RelationalResultStepDriverError::DuplicateResultView(
                            *view_id,
                        ));
                    }
                }
                (
                    ExploreResultInputIr::Find { find_index, .. },
                    input @ RelationalResolvedResultInput::Selected(question_id),
                ) => {
                    if checked.find_question_ids().get(*find_index) != Some(&question_id) {
                        return Err(RelationalResultStepDriverError::JournalScopeMismatch);
                    }
                    let layer = SelectedResultLayer {
                        input,
                        question_id,
                        executor: RelationalResultExecutor::lower(*view_id, result)?,
                    };
                    if selected.insert(*view_id, layer).is_some() {
                        return Err(RelationalResultStepDriverError::DuplicateResultView(
                            *view_id,
                        ));
                    }
                }
                (
                    ExploreResultInputIr::MechanismIncidence { .. },
                    RelationalResolvedResultInput::MechanismIncidence(request_id),
                ) => {
                    deferred.insert(*view_id, request_id);
                }
                _ => {
                    return Err(RelationalResultStepDriverError::AnalysisLayerKindMismatch(
                        RelationalAnalysisLayerId::Result(*view_id),
                    ));
                }
            }
        }

        Ok(Self {
            checked: *checked,
            relation_id: checked.relation_id(),
            admission_id: checked.admission_id(),
            question_ids: plan.question_ids().to_vec().into_boxed_slice(),
            analysis_plan_root: plan.root(),
            sources,
            selected,
            first_deferred_incidence: deferred.into_iter().next(),
            max_rows_per_quantum,
            publication_cache: RefCell::new(BTreeMap::new()),
            selected_discovery_cursors: RefCell::new(BTreeMap::new()),
            source_cursors: RefCell::new(BTreeMap::new()),
            rebound_certified_source_summaries: RefCell::new(BTreeSet::new()),
        })
    }

    pub(crate) const fn max_rows_per_quantum(&self) -> NonZeroU16 {
        self.max_rows_per_quantum
    }

    /// Whether a replayed terminal analysis still needs the bounded checked
    /// source-summary gateway before it may be reported as complete. Most
    /// streams have no certified source summary and can therefore recognize
    /// an already-complete journal without waiting for a host work permit.
    pub(crate) fn certified_source_summary_rebind_required(
        &self,
        journal: &RelationalJournal,
    ) -> bool {
        let Some(analysis) = journal.analysis_state() else {
            return false;
        };
        let rebound = self.rebound_certified_source_summaries.borrow();
        self.sources.keys().any(|view_id| {
            analysis.certified_source_summary(*view_id).is_some() && !rebound.contains(view_id)
        })
    }

    /// Rebind every retained compact source summary to this invocation's
    /// checked compiler theorem and runtime witnesses. The stream driver calls
    /// this before honoring a replayed terminal analysis bit; `step` repeats
    /// the gateway for direct users and the invocation-local set makes the
    /// second call free.
    pub(crate) fn rebind_certified_source_summaries<R>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<(), RelationalResultStepDriverError>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    {
        let view = journal.scheduler_view()?;
        self.validate_scope(view)?;
        let Some(analysis) = journal.analysis_state() else {
            return Ok(());
        };
        for view_id in self.sources.keys().copied() {
            if self
                .rebound_certified_source_summaries
                .borrow()
                .contains(&view_id)
            {
                continue;
            }
            let Some(artifact) = analysis.certified_source_summary(view_id) else {
                continue;
            };
            let source = view.certified_source_population()?.ok_or(
                RelationalResultStepDriverError::CertifiedSourcePopulationMissing(view_id),
            )?;
            match certify_relational_source_summary(&self.checked, view_id, source, runtime)? {
                RelationalCertifiedSourceSummaryCertification::Certified(verified)
                    if verified.artifact() == artifact => {}
                RelationalCertifiedSourceSummaryCertification::Certified(_) => {
                    return Err(
                        RelationalResultStepDriverError::CertifiedSourceSummaryArtifactMismatch(
                            view_id,
                        ),
                    );
                }
                RelationalCertifiedSourceSummaryCertification::Unsupported(_) => {
                    return Err(
                        RelationalResultStepDriverError::CertifiedSourceSummaryNoLongerRecognized(
                            view_id,
                        ),
                    );
                }
            }
            self.rebound_certified_source_summaries
                .borrow_mut()
                .insert(view_id);
        }
        Ok(())
    }

    /// Plan at most one result-layer quantum against the current durable
    /// prefix. Returned events are unapplied.
    pub(crate) fn step<R>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<RelationalResultStepOutcome, RelationalResultStepDriverError>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    {
        self.rebind_certified_source_summaries(journal, runtime)?;
        let view = journal.scheduler_view()?;
        self.validate_scope(view)?;

        let analysis = journal
            .analysis_state()
            .ok_or(RelationalResultStepDriverError::AnalysisStateMissing)?;
        if analysis.is_closed() {
            return Ok(RelationalResultStepOutcome::Quiescent(
                RelationalResultStepQuiescence::AnalysisAlreadyClosed,
            ));
        }
        let catalog = analysis
            .open_catalog()
            .ok_or(RelationalResultStepDriverError::AnalysisCatalogMissing)?;
        if catalog.plan_root() != self.analysis_plan_root {
            return Err(RelationalResultStepDriverError::AnalysisPlanRootMismatch {
                expected: self.analysis_plan_root,
                actual: catalog.plan_root(),
            });
        }

        let mut first_awaiting_source_materialization = None;
        for (view_id, layer) in &self.sources {
            let layer_id = RelationalAnalysisLayerId::Result(*view_id);
            let status = catalog.layer_status(layer_id).ok_or(
                RelationalResultStepDriverError::AnalysisLayerMissing(layer_id),
            )?;
            match status {
                RelationalAnalysisLayerStatus::ResultUnregistered => {
                    return Ok(self.batch(
                        view,
                        RelationalResultStepQuantum::RegisterSourceSpec { view_id: *view_id },
                        vec![RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::result_spec_registered(
                                layer.input,
                                layer.executor.spec().clone(),
                            ),
                        )],
                    ));
                }
                RelationalAnalysisLayerStatus::ResultInputOpen => {
                    self.require_source_registered_spec(catalog, *view_id, layer)?;
                    if let Some(outcome) =
                        self.step_source_rows(view, catalog, *view_id, layer, runtime)?
                    {
                        return Ok(outcome);
                    }
                    first_awaiting_source_materialization.get_or_insert(*view_id);
                }
                RelationalAnalysisLayerStatus::ResultAwaitingPublication => {
                    self.require_source_registered_spec(catalog, *view_id, layer)?;
                    return self.publish_source_result(
                        view,
                        catalog,
                        *view_id,
                        layer,
                        analysis.certified_source_summary(*view_id),
                        runtime,
                    );
                }
                RelationalAnalysisLayerStatus::ResultPublished => {
                    self.require_source_registered_spec(catalog, *view_id, layer)?;
                    self.publication_cache.borrow_mut().remove(view_id);
                    self.source_cursors.borrow_mut().remove(view_id);
                }
                RelationalAnalysisLayerStatus::MechanismTargetOpen
                | RelationalAnalysisLayerStatus::MechanismTerminalOpen
                | RelationalAnalysisLayerStatus::MechanismClosed => {
                    return Err(RelationalResultStepDriverError::AnalysisLayerKindMismatch(
                        layer_id,
                    ));
                }
            }
        }

        let mut first_awaiting_selected_question = None;
        for (view_id, layer) in &self.selected {
            let question_seal = analysis.selected_question(layer.question_id);
            let layer_id = RelationalAnalysisLayerId::Result(*view_id);
            let status = catalog.layer_status(layer_id).ok_or(
                RelationalResultStepDriverError::AnalysisLayerMissing(layer_id),
            )?;
            match status {
                RelationalAnalysisLayerStatus::ResultUnregistered => {
                    return Ok(self.batch(
                        view,
                        RelationalResultStepQuantum::RegisterSelectedSpec { view_id: *view_id },
                        vec![RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::result_spec_registered(
                                layer.input,
                                layer.executor.spec().clone(),
                            ),
                        )],
                    ));
                }
                RelationalAnalysisLayerStatus::ResultInputOpen => {
                    self.require_registered_spec(catalog, *view_id, layer)?;
                    if let Some(outcome) = self.step_selected_rows(
                        view,
                        catalog,
                        *view_id,
                        layer,
                        question_seal,
                        runtime,
                    )? {
                        return Ok(outcome);
                    }
                    first_awaiting_selected_question.get_or_insert(layer.question_id);
                }
                RelationalAnalysisLayerStatus::ResultAwaitingPublication => {
                    self.require_registered_spec(catalog, *view_id, layer)?;
                    let question_seal = question_seal.ok_or(
                        RelationalResultStepDriverError::ResultLayerStateMismatch(*view_id),
                    )?;
                    return self.publish_selected_result(
                        view,
                        catalog,
                        *view_id,
                        layer,
                        question_seal,
                        runtime,
                    );
                }
                RelationalAnalysisLayerStatus::ResultPublished => {
                    self.require_registered_spec(catalog, *view_id, layer)?;
                    self.publication_cache.borrow_mut().remove(view_id);
                    self.selected_discovery_cursors.borrow_mut().remove(view_id);
                }
                RelationalAnalysisLayerStatus::MechanismTargetOpen
                | RelationalAnalysisLayerStatus::MechanismTerminalOpen
                | RelationalAnalysisLayerStatus::MechanismClosed => {
                    return Err(RelationalResultStepDriverError::AnalysisLayerKindMismatch(
                        layer_id,
                    ));
                }
            }
        }

        if let Some(question_id) = first_awaiting_selected_question {
            return Ok(RelationalResultStepOutcome::Quiescent(
                RelationalResultStepQuiescence::AwaitingSelectedQuestion { question_id },
            ));
        }
        if let Some(view_id) = first_awaiting_source_materialization {
            return Ok(RelationalResultStepOutcome::Quiescent(
                RelationalResultStepQuiescence::AwaitingSourceMaterialization { view_id },
            ));
        }

        Ok(RelationalResultStepOutcome::Quiescent(
            match self.first_deferred_incidence {
                Some((view_id, request_id)) => {
                    RelationalResultStepQuiescence::DeferredMechanismIncidence {
                        view_id,
                        request_id,
                    }
                }
                None => RelationalResultStepQuiescence::SelectedResultsComplete,
            },
        ))
    }

    fn step_source_rows<R>(
        &self,
        view: RelationalSchedulerView<'_>,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SourceResultLayer<'_>,
        runtime: &mut R,
    ) -> Result<Option<RelationalResultStepOutcome>, RelationalResultStepDriverError>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    {
        if let Some(source) = view.certified_source_population()? {
            match certify_relational_source_summary(&self.checked, view_id, source, runtime)? {
                RelationalCertifiedSourceSummaryCertification::Certified(verified) => {
                    let evidence = catalog.result_evidence(view_id)?;
                    if !evidence.is_empty() || evidence.input_is_sealed() {
                        return Err(
                            RelationalResultStepDriverError::CertifiedSourceSummaryMixedEvidence(
                                view_id,
                            ),
                        );
                    }
                    let exact_input_count = verified.artifact().exact_cardinality();
                    return Ok(Some(self.batch(
                        view,
                        RelationalResultStepQuantum::CertifySourceSummary {
                            view_id,
                            exact_input_count,
                        },
                        vec![RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::certified_source_summary_accepted(
                                verified.into_artifact(),
                            ),
                        )],
                    )));
                }
                RelationalCertifiedSourceSummaryCertification::Unsupported(_) => {}
            }
        }
        if !view.source_enumeration_is_closed() {
            return Ok(None);
        }
        let expected_rows = view.source_count() as u128;
        let evidence = catalog.result_evidence(view_id)?;
        if evidence.input_is_sealed() {
            return Err(RelationalResultStepDriverError::ResultLayerStateMismatch(
                view_id,
            ));
        }
        let mut sources = self.missing_source_chunk(view, view_id, evidence);
        if sources.is_empty() && evidence.len() as u128 != expected_rows {
            self.source_cursors.borrow_mut().insert(view_id, 0);
            sources = self.missing_source_chunk(view, view_id, evidence);
        }

        if sources.is_empty() {
            self.validate_terminal_source_coverage(view, evidence, expected_rows, 0)?;
            let source_seal = view.source_result_input_seal()?;
            return Ok(Some(self.batch(
                view,
                RelationalResultStepQuantum::SealSourceInput { view_id },
                vec![RelationalJournalEvent::analysis(
                    RelationalAnalysisEvidenceEvent::result_input_sealed_from_sources(
                        view_id,
                        source_seal,
                    ),
                )],
            )));
        }

        let first_source_key = sources[0];
        let row_count = NonZeroU16::new(
            u16::try_from(sources.len())
                .map_err(|_| RelationalResultStepDriverError::ChunkRowCountOverflow)?,
        )
        .ok_or(RelationalResultStepDriverError::ChunkMadeNoProgress)?;
        let mut events = Vec::with_capacity(sources.len() + 1);
        for source_key in sources {
            let source = view
                .source_row(source_key)
                .ok_or(RelationalResultStepDriverError::UnknownSource(source_key))?;
            let evaluated = layer
                .executor
                .evaluate_concrete_source(source_key, source, runtime)?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_evidence_accepted(
                    RelationalResultEvidenceRecord::from_evaluated(&evaluated),
                ),
            ));
        }

        let projected_rows = (evidence.len() as u128)
            .checked_add(u128::from(row_count.get()))
            .ok_or(RelationalResultStepDriverError::SourceRowCountOverflow)?;
        if projected_rows > expected_rows {
            return Err(
                RelationalResultStepDriverError::SourceCoverageCountMismatch {
                    expected: expected_rows,
                    actual: projected_rows,
                },
            );
        }
        let seals_input = projected_rows == expected_rows;
        if seals_input {
            self.validate_terminal_source_coverage(
                view,
                evidence,
                expected_rows,
                u128::from(row_count.get()),
            )?;
            let source_seal = view.source_result_input_seal()?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_input_sealed_from_sources(
                    view_id,
                    source_seal,
                ),
            ));
        }

        Ok(Some(self.batch(
            view,
            RelationalResultStepQuantum::EvaluateSourceRows {
                view_id,
                first_source_key,
                row_count,
                seals_input,
            },
            events,
        )))
    }

    fn missing_source_chunk(
        &self,
        view: RelationalSchedulerView<'_>,
        view_id: ViewId,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Vec<SourceKey> {
        let source_count = view.source_count();
        let mut cursors = self.source_cursors.borrow_mut();
        let cursor = cursors.entry(view_id).or_default();
        if *cursor > source_count || evidence.len() < *cursor {
            *cursor = 0;
        }

        let mut durable_prefix = 0usize;
        let mut sources = Vec::with_capacity(usize::from(self.max_rows_per_quantum.get()));
        for source_key in view.source_keys().skip(*cursor) {
            if evidence
                .record(ResultViewInputRowId::Source(source_key))
                .is_some()
            {
                if sources.is_empty() {
                    durable_prefix += 1;
                }
            } else {
                sources.push(source_key);
                if sources.len() == usize::from(self.max_rows_per_quantum.get()) {
                    break;
                }
            }
        }
        *cursor += durable_prefix;
        sources
    }

    fn step_selected_rows<R: RelationalResultExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SelectedResultLayer<'_>,
        question_seal: Option<RelationalSelectedQuestionSeal>,
        runtime: &mut R,
    ) -> Result<Option<RelationalResultStepOutcome>, RelationalResultStepDriverError> {
        let evidence = catalog.result_evidence(view_id)?;
        if evidence.input_is_sealed() {
            return Err(RelationalResultStepDriverError::ResultLayerStateMismatch(
                view_id,
            ));
        }
        let expected_rows =
            question_seal.map(|seal| seal.result_input_seal().coverage().row_count());
        let mut cases = self.missing_selected_chunk(view, view_id, layer.question_id, evidence)?;

        // A driver is normally bound to one monotonically advancing journal.
        // If it is reused with an unusual restored/forked prefix, exact count
        // evidence exposes any stale operational offset. Reset once and walk
        // the replay-built discovery index; semantic closure still comes from
        // the selected-question seal and canonical row-set commitment.
        if cases.is_empty()
            && expected_rows.is_some_and(|expected| evidence.len() as u128 != expected)
        {
            self.selected_discovery_cursors
                .borrow_mut()
                .insert(view_id, 0);
            cases = self.missing_selected_chunk(view, view_id, layer.question_id, evidence)?;
        }

        if cases.is_empty() {
            let Some(question_seal) = question_seal else {
                return Ok(None);
            };
            let expected_rows = expected_rows.expect("a selected-question seal has an exact count");
            self.validate_terminal_selected_coverage(
                view,
                layer.question_id,
                evidence,
                expected_rows,
                0,
            )?;
            return Ok(Some(self.batch(
                view,
                RelationalResultStepQuantum::SealSelectedInput { view_id },
                vec![RelationalJournalEvent::analysis(
                    RelationalAnalysisEvidenceEvent::result_input_sealed_from_selected(
                        view_id,
                        question_seal,
                    ),
                )],
            )));
        }

        let first_case_id = cases[0];
        let row_count = NonZeroU16::new(
            u16::try_from(cases.len())
                .map_err(|_| RelationalResultStepDriverError::ChunkRowCountOverflow)?,
        )
        .ok_or(RelationalResultStepDriverError::ChunkMadeNoProgress)?;
        let mut events = Vec::with_capacity(cases.len() + 1);
        for case_id in cases {
            let case =
                view.case(case_id)
                    .ok_or(RelationalResultStepDriverError::UnknownSelectedCase(
                        case_id,
                    ))?;
            let evaluated = layer.executor.evaluate_concrete_case(case, runtime)?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_evidence_accepted(
                    RelationalResultEvidenceRecord::from_evaluated(&evaluated),
                ),
            ));
        }

        let projected_rows = (evidence.len() as u128)
            .checked_add(u128::from(row_count.get()))
            .ok_or(RelationalResultStepDriverError::SelectedRowCountOverflow)?;
        if let Some(expected_rows) = expected_rows {
            if projected_rows > expected_rows {
                return Err(
                    RelationalResultStepDriverError::SelectedCoverageCountMismatch {
                        expected: expected_rows,
                        actual: projected_rows,
                    },
                );
            }
        }
        // Exact cardinality plus the durable/pending subset checks proves that
        // equality covers every selected member; no hash-order "has more"
        // probe is needed.
        let seals_input = expected_rows == Some(projected_rows);
        if seals_input {
            let question_seal = question_seal
                .expect("only an exact selected-question population can seal result input");
            self.validate_terminal_selected_coverage(
                view,
                layer.question_id,
                evidence,
                projected_rows,
                u128::from(row_count.get()),
            )?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_input_sealed_from_selected(
                    view_id,
                    question_seal,
                ),
            ));
        }

        Ok(Some(self.batch(
            view,
            RelationalResultStepQuantum::EvaluateSelectedRows {
                view_id,
                first_case_id,
                row_count,
                seals_input,
            },
            events,
        )))
    }

    fn missing_selected_chunk(
        &self,
        view: RelationalSchedulerView<'_>,
        view_id: ViewId,
        question_id: QuestionId,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<Vec<RelationalCaseId>, RelationalResultStepDriverError> {
        let selected_count = view.selected_count(question_id)?;
        let mut cursors = self.selected_discovery_cursors.borrow_mut();
        let cursor = cursors.entry(view_id).or_default();
        if *cursor > selected_count || evidence.len() < *cursor {
            *cursor = 0;
        }

        let mut durable_prefix = 0usize;
        let mut cases = Vec::with_capacity(usize::from(self.max_rows_per_quantum.get()));
        for case_id in view
            .selected_discovery_suffix(question_id, *cursor)?
            .iter()
            .copied()
        {
            if evidence
                .record(ResultViewInputRowId::Case(case_id))
                .is_some()
            {
                if cases.is_empty() {
                    durable_prefix += 1;
                }
            } else {
                cases.push(case_id);
                if cases.len() == usize::from(self.max_rows_per_quantum.get()) {
                    break;
                }
            }
        }
        // Only skip records already present in the durable catalog. Missing
        // rows planned in this batch stay at the cursor until append succeeds,
        // so stale-head/resource rejection is retry-safe.
        *cursor += durable_prefix;
        Ok(cases)
    }

    fn publish_selected_result<R: RelationalResultExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SelectedResultLayer<'_>,
        question_seal: RelationalSelectedQuestionSeal,
        runtime: &mut R,
    ) -> Result<RelationalResultStepOutcome, RelationalResultStepDriverError> {
        let evidence = catalog.result_evidence(view_id)?;
        if evidence.input_seal() != Some(question_seal.result_input_seal()) {
            return Err(RelationalResultStepDriverError::ResultInputSealMismatch(
                view_id,
            ));
        }
        let evidence_root = evidence.root();
        let needs_rebuild = self
            .publication_cache
            .borrow()
            .get(&view_id)
            .is_none_or(|cached| cached.evidence_root != evidence_root);
        if needs_rebuild {
            // The reducer is intentionally invocation-local. Durable row
            // evidence stays in `evidence`; re-evaluation rehydrates only
            // relation-owned base bindings and verifies the deterministic
            // runtime still produces the exact journaled record before
            // reducer insertion. One restart costs one O(N) rebuild, not one
            // rebuild per bounded output chunk.
            let borrowed_group_close = layer.executor.spec().supports_borrowed_group_close();
            let mut execution = (!borrowed_group_close).then(|| layer.executor.execution());
            let mut durable_contributions = Vec::with_capacity(if borrowed_group_close {
                evidence.len()
            } else {
                0
            });
            for record in evidence.records() {
                let ResultViewInputRowId::Case(case_id) = record.row_id() else {
                    return Err(RelationalResultStepDriverError::UnexpectedResultRowKind(
                        view_id,
                    ));
                };
                if view.question_decision(layer.question_id, case_id)?
                    != Some(SelectionDecision::Selected)
                {
                    return Err(
                        RelationalResultStepDriverError::ResultEvidenceOutsideSelectedPopulation {
                            view_id,
                            case_id,
                        },
                    );
                }
                let case = view.case(case_id).ok_or(
                    RelationalResultStepDriverError::UnknownSelectedCase(case_id),
                )?;
                let mut evaluated = layer.executor.evaluate_concrete_case(case, runtime)?;
                let rehydrated = RelationalResultEvidenceRecord::from_evaluated(&evaluated);
                if &rehydrated != record {
                    return Err(RelationalResultStepDriverError::DurableEvidenceMismatch {
                        view_id,
                        case_id,
                    });
                }
                if borrowed_group_close {
                    durable_contributions.push(record.contribution());
                } else {
                    evaluated.reuse_verified_durable_value_storage(
                        record.contribution(),
                        record.materialize_early_select(),
                    );
                    execution
                        .as_mut()
                        .expect("row-state close is present outside the borrowed grouped path")
                        .insert(evaluated)?;
                }
            }
            let (result_root, records) = if borrowed_group_close {
                let closed = layer
                    .executor
                    .close_grouped_without_choice_from_borrowed(&durable_contributions, runtime)?;
                let records = ResultProjectionCatalogBuilder::records_from_compact(&closed);
                (closed.root(), records)
            } else {
                let mut execution = execution
                    .expect("row-state close is present outside the borrowed grouped path");
                execution.seal_input();
                let closed = execution.finish(runtime)?;
                let records = ResultProjectionCatalogBuilder::records_from_closed(&closed)?;
                (closed.root(), records)
            };
            self.publication_cache.borrow_mut().insert(
                view_id,
                CachedSelectedProjection {
                    evidence_root,
                    result_root,
                    records,
                    validated_prefix: None,
                },
            );
        }

        let projection = catalog.result_projection(view_id)?;
        let result_root = {
            let mut cache = self.publication_cache.borrow_mut();
            let cached = cache.get_mut(&view_id).ok_or(
                RelationalResultStepDriverError::ProjectionCacheMissing(view_id),
            )?;
            Self::validate_cached_projection_prefix(projection, cached, view_id)?;

            let first_ordinal = projection.len();
            if first_ordinal < cached.records.len() {
                let end = first_ordinal
                    .saturating_add(usize::from(self.max_rows_per_quantum.get()))
                    .min(cached.records.len());
                let chunk = &cached.records[first_ordinal..end];
                let record_count = NonZeroU16::new(
                    u16::try_from(chunk.len())
                        .map_err(|_| RelationalResultStepDriverError::ChunkRowCountOverflow)?,
                )
                .ok_or(RelationalResultStepDriverError::ChunkMadeNoProgress)?;
                let events = chunk
                    .iter()
                    .cloned()
                    .map(|record| {
                        RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::result_projection_record_accepted(
                                view_id, record,
                            ),
                        )
                    })
                    .collect();
                return Ok(self.batch(
                    view,
                    RelationalResultStepQuantum::PublishSelectedProjectionRecords {
                        view_id,
                        first_ordinal: first_ordinal as u128,
                        record_count,
                    },
                    events,
                ));
            }

            // The complete durable projection is now the publication input.
            // Release the invocation-owned record copy before closure
            // reconstruction so it cannot become a third full output form.
            cache
                .remove(&view_id)
                .ok_or(RelationalResultStepDriverError::ProjectionCacheMissing(
                    view_id,
                ))?
                .result_root
        };
        let event =
            RelationalAnalysisEvidenceEvent::durable_result_view_published(catalog, view_id)?;
        let RelationalAnalysisEvidenceEvent::ResultViewPublished {
            result_root: durable_root,
            ..
        } = &event
        else {
            unreachable!("durable result publication constructor returns its named variant")
        };
        if *durable_root != result_root {
            return Err(
                RelationalResultStepDriverError::DurableProjectionRootMismatch {
                    view_id,
                    evaluated: result_root,
                    durable: *durable_root,
                },
            );
        }
        Ok(self.batch(
            view,
            RelationalResultStepQuantum::PublishSelectedResult {
                view_id,
                result_root,
            },
            vec![RelationalJournalEvent::analysis(event)],
        ))
    }

    fn publish_source_result<R>(
        &self,
        view: RelationalSchedulerView<'_>,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SourceResultLayer<'_>,
        certified_artifact: Option<&RelationalCertifiedSourceSummaryArtifact>,
        runtime: &mut R,
    ) -> Result<RelationalResultStepOutcome, RelationalResultStepDriverError>
    where
        R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
    {
        let evidence = catalog.result_evidence(view_id)?;
        if evidence.input_seal().is_none() {
            return Err(RelationalResultStepDriverError::ResultInputSealMismatch(
                view_id,
            ));
        }
        let evidence_root = evidence.root();
        let needs_rebuild = self
            .publication_cache
            .borrow()
            .get(&view_id)
            .is_none_or(|cached| cached.evidence_root != evidence_root);
        if needs_rebuild {
            let (result_root, records) = if let Some(artifact) = certified_artifact {
                let expected_seal =
                    super::result_evidence::RelationalResultInputSeal::from_certified_source_summary(
                        artifact,
                    );
                if !evidence.is_empty() || evidence.input_seal() != Some(expected_seal) {
                    return Err(
                        RelationalResultStepDriverError::CertifiedSourceSummaryMixedEvidence(
                            view_id,
                        ),
                    );
                }
                let source = view.certified_source_population()?.ok_or(
                    RelationalResultStepDriverError::CertifiedSourcePopulationMissing(view_id),
                )?;
                let verified = match certify_relational_source_summary(
                    &self.checked,
                    view_id,
                    source,
                    runtime,
                )? {
                    RelationalCertifiedSourceSummaryCertification::Certified(verified)
                        if verified.artifact() == artifact =>
                    {
                        verified
                    }
                    RelationalCertifiedSourceSummaryCertification::Certified(_) => {
                        return Err(
                            RelationalResultStepDriverError::CertifiedSourceSummaryArtifactMismatch(
                                view_id,
                            ),
                        );
                    }
                    RelationalCertifiedSourceSummaryCertification::Unsupported(_) => {
                        return Err(
                            RelationalResultStepDriverError::CertifiedSourceSummaryNoLongerRecognized(
                                view_id,
                            ),
                        );
                    }
                };
                let closed = verified.close(&layer.executor, runtime)?;
                let records = ResultProjectionCatalogBuilder::records_from_compact(&closed);
                (closed.root(), records)
            } else {
                let borrowed_group_close = layer.executor.spec().supports_borrowed_group_close();
                let mut execution = (!borrowed_group_close).then(|| layer.executor.execution());
                let mut durable_contributions = Vec::with_capacity(if borrowed_group_close {
                    evidence.len()
                } else {
                    0
                });
                for record in evidence.records() {
                    let ResultViewInputRowId::Source(source_key) = record.row_id() else {
                        return Err(RelationalResultStepDriverError::UnexpectedResultRowKind(
                            view_id,
                        ));
                    };
                    let source = view
                        .source_row(source_key)
                        .ok_or(RelationalResultStepDriverError::UnknownSource(source_key))?;
                    let mut evaluated = layer
                        .executor
                        .evaluate_concrete_source(source_key, source, runtime)?;
                    let rehydrated = RelationalResultEvidenceRecord::from_evaluated(&evaluated);
                    if &rehydrated != record {
                        return Err(
                            RelationalResultStepDriverError::DurableSourceEvidenceMismatch {
                                view_id,
                                source_key,
                            },
                        );
                    }
                    if borrowed_group_close {
                        durable_contributions.push(record.contribution());
                    } else {
                        evaluated.reuse_verified_durable_value_storage(
                            record.contribution(),
                            record.materialize_early_select(),
                        );
                        execution
                            .as_mut()
                            .expect("row-state close is present outside the borrowed grouped path")
                            .insert(evaluated)?;
                    }
                }
                if borrowed_group_close {
                    let closed = layer.executor.close_grouped_without_choice_from_borrowed(
                        &durable_contributions,
                        runtime,
                    )?;
                    let records = ResultProjectionCatalogBuilder::records_from_compact(&closed);
                    (closed.root(), records)
                } else {
                    let mut execution = execution
                        .expect("row-state close is present outside the borrowed grouped path");
                    execution.seal_input();
                    let closed = execution.finish(runtime)?;
                    let records = ResultProjectionCatalogBuilder::records_from_closed(&closed)?;
                    (closed.root(), records)
                }
            };
            self.publication_cache.borrow_mut().insert(
                view_id,
                CachedSelectedProjection {
                    evidence_root,
                    result_root,
                    records,
                    validated_prefix: None,
                },
            );
        }

        let projection = catalog.result_projection(view_id)?;
        let result_root = {
            let mut cache = self.publication_cache.borrow_mut();
            let cached = cache.get_mut(&view_id).ok_or(
                RelationalResultStepDriverError::ProjectionCacheMissing(view_id),
            )?;
            Self::validate_cached_projection_prefix(projection, cached, view_id)?;

            let first_ordinal = projection.len();
            if first_ordinal < cached.records.len() {
                let end = first_ordinal
                    .saturating_add(usize::from(self.max_rows_per_quantum.get()))
                    .min(cached.records.len());
                let chunk = &cached.records[first_ordinal..end];
                let record_count = NonZeroU16::new(
                    u16::try_from(chunk.len())
                        .map_err(|_| RelationalResultStepDriverError::ChunkRowCountOverflow)?,
                )
                .ok_or(RelationalResultStepDriverError::ChunkMadeNoProgress)?;
                let events = chunk
                    .iter()
                    .cloned()
                    .map(|record| {
                        RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::result_projection_record_accepted(
                                view_id, record,
                            ),
                        )
                    })
                    .collect();
                return Ok(self.batch(
                    view,
                    RelationalResultStepQuantum::PublishSourceProjectionRecords {
                        view_id,
                        first_ordinal: first_ordinal as u128,
                        record_count,
                    },
                    events,
                ));
            }

            // Drop the completed source-result preparation before the
            // durable closure is reconstructed. A rejected publication event
            // is retry-safe: the cache is operational and will be rebuilt.
            cache
                .remove(&view_id)
                .ok_or(RelationalResultStepDriverError::ProjectionCacheMissing(
                    view_id,
                ))?
                .result_root
        };
        let event = match certified_artifact {
            Some(artifact) => {
                RelationalAnalysisEvidenceEvent::certified_source_result_view_published(
                    catalog, artifact,
                )?
            }
            None => {
                RelationalAnalysisEvidenceEvent::durable_result_view_published(catalog, view_id)?
            }
        };
        let RelationalAnalysisEvidenceEvent::ResultViewPublished {
            result_root: durable_root,
            ..
        } = &event
        else {
            unreachable!("durable result publication constructor returns its named variant")
        };
        if *durable_root != result_root {
            return Err(
                RelationalResultStepDriverError::DurableProjectionRootMismatch {
                    view_id,
                    evaluated: result_root,
                    durable: *durable_root,
                },
            );
        }
        Ok(self.batch(
            view,
            RelationalResultStepQuantum::PublishSourceResult {
                view_id,
                result_root,
            },
            vec![RelationalJournalEvent::analysis(event)],
        ))
    }

    fn validate_cached_projection_prefix(
        projection: &ResultProjectionCatalogBuilder,
        cached: &mut CachedSelectedProjection,
        view_id: ViewId,
    ) -> Result<(), RelationalResultStepDriverError> {
        match projection.validate_expected_prefix(&cached.records, &mut cached.validated_prefix) {
            Ok(()) => Ok(()),
            Err(ResultProjectionError::ExpectedPrefixTooShort {
                durable_records,
                expected_records,
            }) => Err(RelationalResultStepDriverError::DurableProjectionTooLong {
                view_id,
                durable_records,
                evaluated_records: expected_records,
            }),
            Err(ResultProjectionError::ExpectedRecordMismatch { ordinal }) => {
                Err(RelationalResultStepDriverError::DurableProjectionMismatch { view_id, ordinal })
            }
            Err(error) => Err(error.into()),
        }
    }

    fn validate_terminal_source_coverage(
        &self,
        view: RelationalSchedulerView<'_>,
        evidence: &RelationalResultEvidenceCatalogBuilder,
        expected_rows: u128,
        pending_rows: u128,
    ) -> Result<(), RelationalResultStepDriverError> {
        let actual_rows = (evidence.len() as u128)
            .checked_add(pending_rows)
            .ok_or(RelationalResultStepDriverError::SourceRowCountOverflow)?;
        if actual_rows != expected_rows {
            return Err(
                RelationalResultStepDriverError::SourceCoverageCountMismatch {
                    expected: expected_rows,
                    actual: actual_rows,
                },
            );
        }
        for record in evidence.records() {
            let ResultViewInputRowId::Source(source_key) = record.row_id() else {
                return Err(RelationalResultStepDriverError::UnexpectedResultRowKind(
                    evidence.view_id(),
                ));
            };
            if view.source_row(source_key).is_none() {
                return Err(RelationalResultStepDriverError::UnknownSource(source_key));
            }
        }
        Ok(())
    }

    fn validate_terminal_selected_coverage(
        &self,
        view: RelationalSchedulerView<'_>,
        question_id: QuestionId,
        evidence: &RelationalResultEvidenceCatalogBuilder,
        expected_rows: u128,
        pending_rows: u128,
    ) -> Result<(), RelationalResultStepDriverError> {
        let durable_rows = evidence.len() as u128;
        let actual_rows = durable_rows
            .checked_add(pending_rows)
            .ok_or(RelationalResultStepDriverError::SelectedRowCountOverflow)?;
        if actual_rows != expected_rows {
            return Err(
                RelationalResultStepDriverError::SelectedCoverageCountMismatch {
                    expected: expected_rows,
                    actual: actual_rows,
                },
            );
        }
        for record in evidence.records() {
            let ResultViewInputRowId::Case(case_id) = record.row_id() else {
                return Err(RelationalResultStepDriverError::UnexpectedResultRowKind(
                    evidence.view_id(),
                ));
            };
            if view.question_decision(question_id, case_id)? != Some(SelectionDecision::Selected) {
                return Err(
                    RelationalResultStepDriverError::ResultEvidenceOutsideSelectedPopulation {
                        view_id: evidence.view_id(),
                        case_id,
                    },
                );
            }
        }
        Ok(())
    }

    fn require_registered_spec(
        &self,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SelectedResultLayer<'_>,
    ) -> Result<(), RelationalResultStepDriverError> {
        if catalog.result_spec(view_id)? != layer.executor.spec() {
            return Err(RelationalResultStepDriverError::RegisteredSpecMismatch(
                view_id,
            ));
        }
        Ok(())
    }

    fn require_source_registered_spec(
        &self,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &SourceResultLayer<'_>,
    ) -> Result<(), RelationalResultStepDriverError> {
        if catalog.result_spec(view_id)? != layer.executor.spec() {
            return Err(RelationalResultStepDriverError::RegisteredSpecMismatch(
                view_id,
            ));
        }
        Ok(())
    }

    fn validate_scope(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalResultStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != self.relation_id
            || contract.admission_id() != self.admission_id
            || contract.question_ids() != self.question_ids.as_ref()
        {
            return Err(RelationalResultStepDriverError::JournalScopeMismatch);
        }
        match view.analysis_plan_root() {
            Some(actual) if actual == self.analysis_plan_root => Ok(()),
            Some(actual) => Err(RelationalResultStepDriverError::AnalysisPlanRootMismatch {
                expected: self.analysis_plan_root,
                actual,
            }),
            None => Err(RelationalResultStepDriverError::AnalysisPlanMissing),
        }
    }

    fn batch(
        &self,
        view: RelationalSchedulerView<'_>,
        quantum: RelationalResultStepQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalResultStepOutcome {
        debug_assert!(!events.is_empty());
        RelationalResultStepOutcome::Emitted(RelationalResultStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum,
            events: events.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultStepDriverError {
    AnalysisPlan(RelationalAnalysisPlanError),
    Catalog(RelationalAnalysisCatalogError),
    AnalysisJournal(RelationalAnalysisJournalError),
    Journal(RelationalJournalError),
    ResultExecutor(RelationalResultExecutorError),
    ResultProjection(ResultProjectionError),
    CertifiedSourceSummary(RelationalCertifiedSourceSummaryError),
    JournalScopeMismatch,
    AnalysisPlanMissing,
    AnalysisPlanRootMismatch {
        expected: RelationalAnalysisPlanRoot,
        actual: RelationalAnalysisPlanRoot,
    },
    AnalysisStateMissing,
    AnalysisCatalogMissing,
    AnalysisLayerMissing(RelationalAnalysisLayerId),
    AnalysisLayerKindMismatch(RelationalAnalysisLayerId),
    DuplicateResultView(ViewId),
    RegisteredSpecMismatch(ViewId),
    ResultLayerStateMismatch(ViewId),
    ResultInputSealMismatch(ViewId),
    UnknownSource(SourceKey),
    UnknownSelectedCase(RelationalCaseId),
    UnexpectedResultRowKind(ViewId),
    ResultEvidenceOutsideSelectedPopulation {
        view_id: ViewId,
        case_id: RelationalCaseId,
    },
    DurableEvidenceMismatch {
        view_id: ViewId,
        case_id: RelationalCaseId,
    },
    DurableSourceEvidenceMismatch {
        view_id: ViewId,
        source_key: SourceKey,
    },
    CertifiedSourcePopulationMissing(ViewId),
    CertifiedSourceSummaryMixedEvidence(ViewId),
    CertifiedSourceSummaryArtifactMismatch(ViewId),
    CertifiedSourceSummaryNoLongerRecognized(ViewId),
    ProjectionCacheMissing(ViewId),
    DurableProjectionMismatch {
        view_id: ViewId,
        ordinal: u128,
    },
    DurableProjectionTooLong {
        view_id: ViewId,
        durable_records: u128,
        evaluated_records: u128,
    },
    DurableProjectionRootMismatch {
        view_id: ViewId,
        evaluated: ResultViewRoot,
        durable: ResultViewRoot,
    },
    SelectedCoverageCountMismatch {
        expected: u128,
        actual: u128,
    },
    SourceCoverageCountMismatch {
        expected: u128,
        actual: u128,
    },
    SourceRowCountOverflow,
    SelectedRowCountOverflow,
    ChunkRowCountOverflow,
    ChunkMadeNoProgress,
}

impl fmt::Display for RelationalResultStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisPlan(error) => write!(formatter, "result analysis plan failed: {error}"),
            Self::Catalog(error) => write!(formatter, "result analysis catalog failed: {error}"),
            Self::AnalysisJournal(error) => {
                write!(formatter, "result analysis event failed: {error}")
            }
            Self::Journal(error) => write!(formatter, "result journal step failed: {error}"),
            Self::ResultExecutor(error) => write!(formatter, "result execution failed: {error}"),
            Self::ResultProjection(error) => {
                write!(formatter, "result projection publication failed: {error}")
            }
            Self::CertifiedSourceSummary(error) => {
                write!(formatter, "certified source summary failed: {error}")
            }
            Self::JournalScopeMismatch => {
                formatter.write_str("result driver and relational journal scopes differ")
            }
            Self::AnalysisPlanMissing => {
                formatter.write_str("result execution requires a registered analysis plan")
            }
            Self::AnalysisPlanRootMismatch { .. } => {
                formatter.write_str("result driver and journal analysis plan roots differ")
            }
            Self::AnalysisStateMissing => {
                formatter.write_str("registered analysis plan has no analysis journal state")
            }
            Self::AnalysisCatalogMissing => {
                formatter.write_str("open result execution has no analysis catalog")
            }
            Self::AnalysisLayerMissing(_) => {
                formatter.write_str("checked result layer is absent from the analysis catalog")
            }
            Self::AnalysisLayerKindMismatch(_) => {
                formatter.write_str("checked result layer has a different analysis kind or input")
            }
            Self::DuplicateResultView(_) => {
                formatter.write_str("checked query repeats a semantic result ViewId")
            }
            Self::RegisteredSpecMismatch(_) => formatter
                .write_str("journaled result spec differs from the checked lowered result spec"),
            Self::ResultLayerStateMismatch(_) => {
                formatter.write_str("result layer status and evidence frontier disagree")
            }
            Self::ResultInputSealMismatch(_) => {
                formatter.write_str("result input does not match its exact upstream seal")
            }
            Self::UnknownSource(_) => {
                formatter.write_str("source result row names no durable relational source")
            }
            Self::UnknownSelectedCase(_) => {
                formatter.write_str("selected result row names no durable relational case")
            }
            Self::UnexpectedResultRowKind(_) => {
                formatter.write_str("result evidence contains the wrong input-row kind")
            }
            Self::ResultEvidenceOutsideSelectedPopulation { .. } => formatter
                .write_str("result evidence contains a case outside the selected population"),
            Self::DurableEvidenceMismatch { .. } => formatter.write_str(
                "rehydrated checked result evaluation differs from its durable row evidence",
            ),
            Self::DurableSourceEvidenceMismatch { .. } => formatter.write_str(
                "rehydrated source result evaluation differs from its durable row evidence",
            ),
            Self::CertifiedSourcePopulationMissing(_) => formatter.write_str(
                "certified source summary lost its installed exact source-population proof",
            ),
            Self::CertifiedSourceSummaryMixedEvidence(_) => formatter.write_str(
                "certified source summary cannot be mixed with concrete source-row evidence",
            ),
            Self::CertifiedSourceSummaryArtifactMismatch(_) => formatter.write_str(
                "fresh checked source-summary evaluation differs from the durable proof artifact",
            ),
            Self::CertifiedSourceSummaryNoLongerRecognized(_) => formatter.write_str(
                "durable certified source summary is no longer recognized by the checked query",
            ),
            Self::ProjectionCacheMissing(_) => formatter
                .write_str("result projection cache disappeared during one publication quantum"),
            Self::DurableProjectionMismatch { .. } => formatter.write_str(
                "durable result projection prefix differs from deterministic reevaluation",
            ),
            Self::DurableProjectionTooLong { .. } => formatter.write_str(
                "durable result projection contains records beyond deterministic reevaluation",
            ),
            Self::DurableProjectionRootMismatch { .. } => formatter.write_str(
                "durable result projection root differs from deterministic reevaluation",
            ),
            Self::SelectedCoverageCountMismatch { expected, actual } => write!(
                formatter,
                "selected result coverage has {actual} rows; exact seal requires {expected}"
            ),
            Self::SourceCoverageCountMismatch { expected, actual } => write!(
                formatter,
                "source result coverage has {actual} rows; exact seal requires {expected}"
            ),
            Self::SourceRowCountOverflow => {
                formatter.write_str("source result row count overflowed u128")
            }
            Self::SelectedRowCountOverflow => {
                formatter.write_str("selected result row count overflowed u128")
            }
            Self::ChunkRowCountOverflow => {
                formatter.write_str("result evidence chunk exceeded its u16 operational bound")
            }
            Self::ChunkMadeNoProgress => {
                formatter.write_str("nonempty result evidence chunk reported zero rows")
            }
        }
    }
}

impl Error for RelationalResultStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisPlan(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::AnalysisJournal(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::ResultExecutor(error) => Some(error),
            Self::ResultProjection(error) => Some(error),
            Self::CertifiedSourceSummary(error) => Some(error),
            Self::JournalScopeMismatch
            | Self::AnalysisPlanMissing
            | Self::AnalysisPlanRootMismatch { .. }
            | Self::AnalysisStateMissing
            | Self::AnalysisCatalogMissing
            | Self::AnalysisLayerMissing(_)
            | Self::AnalysisLayerKindMismatch(_)
            | Self::DuplicateResultView(_)
            | Self::RegisteredSpecMismatch(_)
            | Self::ResultLayerStateMismatch(_)
            | Self::ResultInputSealMismatch(_)
            | Self::UnknownSource(_)
            | Self::UnknownSelectedCase(_)
            | Self::UnexpectedResultRowKind(_)
            | Self::ResultEvidenceOutsideSelectedPopulation { .. }
            | Self::DurableEvidenceMismatch { .. }
            | Self::DurableSourceEvidenceMismatch { .. }
            | Self::CertifiedSourcePopulationMissing(_)
            | Self::CertifiedSourceSummaryMixedEvidence(_)
            | Self::CertifiedSourceSummaryArtifactMismatch(_)
            | Self::CertifiedSourceSummaryNoLongerRecognized(_)
            | Self::ProjectionCacheMissing(_)
            | Self::DurableProjectionMismatch { .. }
            | Self::DurableProjectionTooLong { .. }
            | Self::DurableProjectionRootMismatch { .. }
            | Self::SelectedCoverageCountMismatch { .. }
            | Self::SourceCoverageCountMismatch { .. }
            | Self::SourceRowCountOverflow
            | Self::SelectedRowCountOverflow
            | Self::ChunkRowCountOverflow
            | Self::ChunkMadeNoProgress => None,
        }
    }
}

impl From<RelationalAnalysisPlanError> for RelationalResultStepDriverError {
    fn from(error: RelationalAnalysisPlanError) -> Self {
        Self::AnalysisPlan(error)
    }
}

impl From<RelationalAnalysisCatalogError> for RelationalResultStepDriverError {
    fn from(error: RelationalAnalysisCatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<RelationalAnalysisJournalError> for RelationalResultStepDriverError {
    fn from(error: RelationalAnalysisJournalError) -> Self {
        Self::AnalysisJournal(error)
    }
}

impl From<RelationalJournalError> for RelationalResultStepDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<RelationalResultExecutorError> for RelationalResultStepDriverError {
    fn from(error: RelationalResultExecutorError) -> Self {
        Self::ResultExecutor(error)
    }
}

impl From<ResultProjectionError> for RelationalResultStepDriverError {
    fn from(error: ResultProjectionError) -> Self {
        Self::ResultProjection(error)
    }
}

impl From<RelationalCertifiedSourceSummaryError> for RelationalResultStepDriverError {
    fn from(error: RelationalCertifiedSourceSummaryError) -> Self {
        Self::CertifiedSourceSummary(error)
    }
}
