//! Bounded post-mechanism execution for incidence-backed result views.
//!
//! This is the downstream half of the relational analysis DAG, not a run
//! loop. It registers checked incidence-result specs immediately, consumes
//! newly durable successful mechanism terminals while their upstream request
//! is still open, and emits one head-bound evidence quantum at a time. Exact
//! input closure and projection publication remain unavailable until the
//! mechanism-incidence layer has its own durable closure event.
//!
//! Unavailable mechanism terminals are deliberately not result rows. The
//! typed incidence input seal commits exactly the successful
//! `(case, transition, signature)` relation, so neither a target count nor a
//! terminal count can be substituted for result coverage.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView};

use super::mechanism_incidence::{
    MechanismCaseTerminal, MechanismIncidenceCatalogBuilder, MechanismSignatureId,
};
use super::relation::{
    AdmissionId, MechanismRequestId, QuestionId, RelationId, RelationalCaseId, ViewId,
};
use super::relational_analysis_catalog::{
    RelationalAnalysisCatalogBuilder, RelationalAnalysisCatalogError,
    RelationalAnalysisLayerStatus, RelationalMechanismClosureReceipt,
};
use super::relational_analysis_journal::{
    RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanError, RelationalAnalysisPlanRoot, RelationalResolvedResultInput,
};
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
    RelationalResultEvidenceRoot, ResultEvidenceError,
};
use super::result_projection::{
    IndexedResultProjectionRecord, ResultProjectionCatalogBuilder, ResultProjectionError,
    ValidatedResultProjectionPrefix,
};
use super::result_view::{
    MechanismIncidenceRowId, ResultViewInputKind, ResultViewInputRowId, ResultViewRoot,
};
use super::structural_mechanism::{
    ExecutionProfileId, StructuralMechanismCatalogBuilder, StructuralMechanismId,
};
use super::transition::TransitionId;

/// Operational description of one emitted incidence-result quantum. These
/// fields are observability metadata and never participate in semantic IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIncidenceResultStepQuantum {
    RegisterSpec {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    EvaluateRows {
        view_id: ViewId,
        request_id: MechanismRequestId,
        first_row_id: MechanismIncidenceRowId,
        row_count: NonZeroU16,
        seals_input: bool,
    },
    SealInput {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    PublishProjectionRecords {
        view_id: ViewId,
        request_id: MechanismRequestId,
        first_ordinal: u128,
        record_count: NonZeroU16,
    },
    PublishResult {
        view_id: ViewId,
        request_id: MechanismRequestId,
        result_root: ResultViewRoot,
    },
}

/// One head-bound ordered batch. The outer durable coordinator owns append,
/// retry, deadline, and resource decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalIncidenceResultStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalIncidenceResultStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalIncidenceResultStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalIncidenceResultStepQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

/// Honest non-progress states. `AwaitingMechanismIncidence` means every
/// currently durable successful terminal has result evidence; it does not
/// claim that the upstream request or result input is closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIncidenceResultStepQuiescence {
    AwaitingMechanismIncidence {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    IncidenceResultsComplete,
    AnalysisAlreadyClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIncidenceResultStepOutcome {
    Emitted(RelationalIncidenceResultStepBatch),
    Quiescent(RelationalIncidenceResultStepQuiescence),
}

struct IncidenceResultLayer<'ir> {
    input: RelationalResolvedResultInput,
    request_id: MechanismRequestId,
    executor: RelationalResultExecutor<'ir>,
}

struct CachedIncidenceProjection {
    evidence_root: RelationalResultEvidenceRoot,
    result_root: ResultViewRoot,
    records: Box<[IndexedResultProjectionRecord]>,
    validated_prefix: Option<ValidatedResultProjectionPrefix>,
}

/// Invocation-local progress through a replay-built terminal discovery
/// sequence. `durable_incidence_rows` lets a reused driver detect a stale
/// cursor even though unavailable terminals intentionally have no result row.
#[derive(Clone, Copy, Debug, Default)]
struct IncidenceDiscoveryCursor {
    terminal_ordinal: usize,
    durable_incidence_rows: usize,
}

#[derive(Clone, Copy)]
struct AvailableIncidenceRow {
    row_id: MechanismIncidenceRowId,
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    signature_id: MechanismSignatureId,
    structural_mechanism_id: StructuralMechanismId,
    execution_profile_id: ExecutionProfileId,
}

/// Checked-query-bound scheduler for all mechanism-incidence result layers.
pub(crate) struct RelationalIncidenceResultStepDriver<'query> {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_ids: Box<[QuestionId]>,
    analysis_plan_root: RelationalAnalysisPlanRoot,
    layers: BTreeMap<ViewId, IncidenceResultLayer<'query>>,
    /// Purely operational CPU/frame bound. It is absent from all result specs,
    /// evidence identities, journal contracts, and projection roots.
    max_rows_per_quantum: NonZeroU16,
    /// Invocation-local reduction cache. A restart performs one O(N)
    /// evidence rehydration, then resumes bounded publication by durable
    /// projection ordinal rather than rebuilding once per output chunk.
    publication_cache: RefCell<BTreeMap<ViewId, CachedIncidenceProjection>>,
    /// Per-view operational terminal offsets. These never enter result or
    /// incidence identities; journaled evidence and exact seals remain the
    /// durable resume authority.
    incidence_discovery_cursors: RefCell<BTreeMap<ViewId, IncidenceDiscoveryCursor>>,
}

impl<'query> RelationalIncidenceResultStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalIncidenceResultStepDriverError> {
        Self::from_checked_with_max_rows_per_quantum(checked, NonZeroU16::MIN)
    }

    pub(crate) fn from_checked_with_max_rows_per_quantum(
        checked: &'query CheckedExploreQueryView<'_>,
        max_rows_per_quantum: NonZeroU16,
    ) -> Result<Self, RelationalIncidenceResultStepDriverError> {
        let plan = RelationalAnalysisPlan::from_checked(checked)?;
        let mut layers = BTreeMap::new();

        for (node, identity) in checked.analysis_nodes() {
            let (
                ExploreAnalysisNodeIr::Result(result),
                CheckedExploreAnalysisIdentity::View { view_id, .. },
            ) = (node, identity)
            else {
                continue;
            };
            let ExploreResultInputIr::MechanismIncidence { .. } = &result.input else {
                continue;
            };
            let layer_id = RelationalAnalysisLayerId::Result(*view_id);
            let registration = plan
                .registration(layer_id)
                .ok_or(RelationalIncidenceResultStepDriverError::AnalysisLayerMissing(layer_id))?;
            let RelationalAnalysisLayerRegistration::Result(registration) = registration else {
                return Err(
                    RelationalIncidenceResultStepDriverError::AnalysisLayerKindMismatch(layer_id),
                );
            };
            let input @ RelationalResolvedResultInput::MechanismIncidence(request_id) =
                registration.input()
            else {
                return Err(
                    RelationalIncidenceResultStepDriverError::AnalysisLayerKindMismatch(layer_id),
                );
            };
            if plan
                .registration(RelationalAnalysisLayerId::Mechanisms(request_id))
                .is_none()
            {
                return Err(
                    RelationalIncidenceResultStepDriverError::AnalysisLayerMissing(
                        RelationalAnalysisLayerId::Mechanisms(request_id),
                    ),
                );
            }
            let layer = IncidenceResultLayer {
                input,
                request_id,
                executor: RelationalResultExecutor::lower(*view_id, result)?,
            };
            if layers.insert(*view_id, layer).is_some() {
                return Err(
                    RelationalIncidenceResultStepDriverError::DuplicateResultView(*view_id),
                );
            }
        }

        Ok(Self {
            relation_id: checked.relation_id(),
            admission_id: checked.admission_id(),
            question_ids: plan.question_ids().to_vec().into_boxed_slice(),
            analysis_plan_root: plan.root(),
            layers,
            max_rows_per_quantum,
            publication_cache: RefCell::new(BTreeMap::new()),
            incidence_discovery_cursors: RefCell::new(BTreeMap::new()),
        })
    }

    pub(crate) const fn max_rows_per_quantum(&self) -> NonZeroU16 {
        self.max_rows_per_quantum
    }

    /// Plan at most one incidence-result quantum against the authenticated
    /// current prefix. Returned events are unapplied.
    pub(crate) fn step<R: RelationalResultExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<RelationalIncidenceResultStepOutcome, RelationalIncidenceResultStepDriverError>
    {
        let view = journal.scheduler_view()?;
        self.validate_scope(view)?;

        let analysis = journal
            .analysis_state()
            .ok_or(RelationalIncidenceResultStepDriverError::AnalysisStateMissing)?;
        if analysis.is_closed() {
            return Ok(RelationalIncidenceResultStepOutcome::Quiescent(
                RelationalIncidenceResultStepQuiescence::AnalysisAlreadyClosed,
            ));
        }
        let catalog = analysis
            .open_catalog()
            .ok_or(RelationalIncidenceResultStepDriverError::AnalysisCatalogMissing)?;
        if catalog.plan_root() != self.analysis_plan_root {
            return Err(
                RelationalIncidenceResultStepDriverError::AnalysisPlanRootMismatch {
                    expected: self.analysis_plan_root,
                    actual: catalog.plan_root(),
                },
            );
        }

        let mut first_awaiting = None;
        for (view_id, layer) in &self.layers {
            let layer_id = RelationalAnalysisLayerId::Result(*view_id);
            let status = catalog
                .layer_status(layer_id)
                .ok_or(RelationalIncidenceResultStepDriverError::AnalysisLayerMissing(layer_id))?;
            match status {
                RelationalAnalysisLayerStatus::ResultUnregistered => {
                    return Ok(self.batch(
                        view,
                        RelationalIncidenceResultStepQuantum::RegisterSpec {
                            view_id: *view_id,
                            request_id: layer.request_id,
                        },
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
                    if let Some(outcome) =
                        self.step_incidence_rows(view, analysis, catalog, *view_id, layer, runtime)?
                    {
                        return Ok(outcome);
                    }
                    first_awaiting.get_or_insert((*view_id, layer.request_id));
                }
                RelationalAnalysisLayerStatus::ResultAwaitingPublication => {
                    self.require_registered_spec(catalog, *view_id, layer)?;
                    let closure = analysis.mechanism_closure(layer.request_id).ok_or(
                        RelationalIncidenceResultStepDriverError::MechanismClosureArtifactMissing {
                            view_id: *view_id,
                            request_id: layer.request_id,
                        },
                    )?;
                    let structural = analysis
                        .structural_mechanism_catalog(layer.request_id)
                        .ok_or(
                            RelationalIncidenceResultStepDriverError::StructuralCatalogMissing {
                                view_id: *view_id,
                                request_id: layer.request_id,
                            },
                        )?;
                    return self.publish_incidence_result(
                        view, catalog, structural, *view_id, layer, closure, runtime,
                    );
                }
                RelationalAnalysisLayerStatus::ResultPublished => {
                    self.require_registered_spec(catalog, *view_id, layer)?;
                    self.publication_cache.borrow_mut().remove(view_id);
                    self.incidence_discovery_cursors
                        .borrow_mut()
                        .remove(view_id);
                }
                RelationalAnalysisLayerStatus::MechanismTargetOpen
                | RelationalAnalysisLayerStatus::MechanismTerminalOpen
                | RelationalAnalysisLayerStatus::MechanismClosed
                | RelationalAnalysisLayerStatus::ChoiceInputOpen
                | RelationalAnalysisLayerStatus::ChoiceMembersOpen
                | RelationalAnalysisLayerStatus::ChoiceClosed => {
                    return Err(
                        RelationalIncidenceResultStepDriverError::AnalysisLayerKindMismatch(
                            layer_id,
                        ),
                    );
                }
            }
        }

        Ok(RelationalIncidenceResultStepOutcome::Quiescent(
            match first_awaiting {
                Some((view_id, request_id)) => {
                    RelationalIncidenceResultStepQuiescence::AwaitingMechanismIncidence {
                        view_id,
                        request_id,
                    }
                }
                None => RelationalIncidenceResultStepQuiescence::IncidenceResultsComplete,
            },
        ))
    }

    fn step_incidence_rows<R: RelationalResultExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        analysis: &super::relational_analysis_journal::RelationalAnalysisJournalState,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &IncidenceResultLayer<'_>,
        runtime: &mut R,
    ) -> Result<
        Option<RelationalIncidenceResultStepOutcome>,
        RelationalIncidenceResultStepDriverError,
    > {
        let incidence = catalog.mechanism_incidence(layer.request_id)?;
        let evidence = catalog.result_evidence(view_id)?;
        if evidence.input_is_sealed() {
            return Err(
                RelationalIncidenceResultStepDriverError::ResultLayerStateMismatch(view_id),
            );
        }
        if evidence.len() > incidence.incidence_case_count() {
            return Err(
                RelationalIncidenceResultStepDriverError::IncidenceCoverageCountMismatch {
                    expected: incidence.incidence_case_count() as u128,
                    actual: evidence.len() as u128,
                },
            );
        }
        let closure = analysis.mechanism_closure(layer.request_id);
        let structural = analysis.structural_mechanism_catalog(layer.request_id);
        let structural_closure = analysis.structural_quotient_closure(layer.request_id);
        let expected_rows = closure
            .zip(structural_closure)
            .map(|(closure, _)| closure.result_input_seal().coverage().row_count());
        if let Some(expected_rows) = expected_rows {
            let actual_rows = incidence.incidence_case_count() as u128;
            if actual_rows != expected_rows {
                return Err(
                    RelationalIncidenceResultStepDriverError::IncidenceCoverageCountMismatch {
                        expected: expected_rows,
                        actual: actual_rows,
                    },
                );
            }
        }

        let mut rows = self.missing_incidence_chunk(
            incidence,
            structural,
            evidence,
            view_id,
            layer.request_id,
        )?;

        // Exact closure exposes a stale invocation-local cursor without
        // trusting it. Reset once and repair from the journal-rebuilt
        // terminal discovery sequence; canonical row-set validation below is
        // still the closure authority.
        if rows.is_empty()
            && expected_rows.is_some_and(|expected| evidence.len() as u128 != expected)
        {
            self.incidence_discovery_cursors
                .borrow_mut()
                .insert(view_id, IncidenceDiscoveryCursor::default());
            rows = self.missing_incidence_chunk(
                incidence,
                structural,
                evidence,
                view_id,
                layer.request_id,
            )?;
        }

        if rows.is_empty() {
            let (Some(closure), Some(structural_closure), Some(expected_rows)) =
                (closure, structural_closure, expected_rows)
            else {
                return Ok(None);
            };
            self.validate_terminal_incidence_coverage(
                incidence,
                evidence,
                view_id,
                layer.request_id,
                expected_rows,
                &[],
            )?;
            return Ok(Some(self.batch(
                view,
                RelationalIncidenceResultStepQuantum::SealInput {
                    view_id,
                    request_id: layer.request_id,
                },
                vec![RelationalJournalEvent::analysis(
                    RelationalAnalysisEvidenceEvent::result_input_sealed_from_mechanisms(
                        view_id,
                        closure,
                        structural_closure,
                    ),
                )],
            )));
        }

        let first_row_id = rows[0].row_id;
        let row_count = NonZeroU16::new(
            u16::try_from(rows.len())
                .map_err(|_| RelationalIncidenceResultStepDriverError::ChunkRowCountOverflow)?,
        )
        .ok_or(RelationalIncidenceResultStepDriverError::ChunkMadeNoProgress)?;
        let mut events = Vec::with_capacity(rows.len() + usize::from(closure.is_some()));
        for row in &rows {
            let case = view.case(row.case_id).ok_or(
                RelationalIncidenceResultStepDriverError::UnknownIncidenceCase(row.case_id),
            )?;
            let evaluated = layer.executor.evaluate_concrete_incidence(
                case,
                row.transition_id,
                row.signature_id,
                row.structural_mechanism_id,
                row.execution_profile_id,
                runtime,
            )?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_evidence_accepted(
                    RelationalResultEvidenceRecord::from_evaluated(&evaluated),
                ),
            ));
        }

        let projected_rows = (evidence.len() as u128)
            .checked_add(u128::from(row_count.get()))
            .ok_or(RelationalIncidenceResultStepDriverError::IncidenceRowCountOverflow)?;
        if let Some(expected_rows) = expected_rows {
            if projected_rows > expected_rows {
                return Err(
                    RelationalIncidenceResultStepDriverError::IncidenceCoverageCountMismatch {
                        expected: expected_rows,
                        actual: projected_rows,
                    },
                );
            }
        }

        // Every pending row is a distinct incidence member absent from the
        // durable evidence catalog. Exact cardinality equality therefore
        // establishes full coverage without probing hash order for a suffix.
        let seals_input = expected_rows == Some(projected_rows);
        if seals_input {
            let closure = closure.expect("only closed mechanism incidence can seal result input");
            let structural_closure =
                structural_closure.expect("only closed structural incidence can seal result input");
            self.validate_terminal_incidence_coverage(
                incidence,
                evidence,
                view_id,
                layer.request_id,
                projected_rows,
                &rows,
            )?;
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::result_input_sealed_from_mechanisms(
                    view_id,
                    closure,
                    structural_closure,
                ),
            ));
        }

        Ok(Some(self.batch(
            view,
            RelationalIncidenceResultStepQuantum::EvaluateRows {
                view_id,
                request_id: layer.request_id,
                first_row_id,
                row_count,
                seals_input,
            },
            events,
        )))
    }

    fn missing_incidence_chunk(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
        structural: Option<&StructuralMechanismCatalogBuilder>,
        evidence: &RelationalResultEvidenceCatalogBuilder,
        view_id: ViewId,
        request_id: MechanismRequestId,
    ) -> Result<Vec<AvailableIncidenceRow>, RelationalIncidenceResultStepDriverError> {
        let terminal_count = incidence.terminal_case_count();
        let mut cursors = self.incidence_discovery_cursors.borrow_mut();
        let cursor = cursors.entry(view_id).or_default();
        if cursor.terminal_ordinal > terminal_count
            || cursor.durable_incidence_rows > evidence.len()
        {
            *cursor = IncidenceDiscoveryCursor::default();
        }

        let limit = usize::from(self.max_rows_per_quantum.get());
        let mut rows = Vec::with_capacity(limit);
        let mut durable_terminal_prefix = 0usize;
        let mut durable_incidence_prefix = 0usize;
        for record in incidence
            .terminal_discovery_suffix(cursor.terminal_ordinal)
            .iter()
            .copied()
        {
            let MechanismCaseTerminal::Incidence {
                transition_id,
                signature_id,
            } = record.terminal()
            else {
                if rows.is_empty() {
                    durable_terminal_prefix += 1;
                }
                continue;
            };
            let row_id =
                MechanismIncidenceRowId::new(record.case_id(), transition_id, signature_id);
            self.validate_row_request(view_id, request_id, row_id)?;
            if evidence
                .record(ResultViewInputRowId::Incidence(row_id))
                .is_some()
            {
                if rows.is_empty() {
                    durable_terminal_prefix += 1;
                    durable_incidence_prefix += 1;
                }
                continue;
            }
            let Some(assignment) = structural.and_then(|catalog| catalog.assignment(signature_id))
            else {
                // Structural classification is a durable part of the
                // mechanism-incidence row. Preserve the terminal cursor and
                // resume when the assignment arrives.
                break;
            };
            rows.push(AvailableIncidenceRow {
                row_id,
                case_id: record.case_id(),
                transition_id,
                signature_id,
                structural_mechanism_id: assignment.mechanism_id(),
                execution_profile_id: assignment.profile_id(),
            });
            if rows.len() == limit {
                break;
            }
        }
        // Skip only terminal records whose downstream consequence is already
        // durable (or deliberately absent for `Unavailable`). Planned rows
        // remain at the cursor until the outer append succeeds.
        cursor.terminal_ordinal += durable_terminal_prefix;
        cursor.durable_incidence_rows += durable_incidence_prefix;
        Ok(rows)
    }

    fn publish_incidence_result<R: RelationalResultExpressionRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        catalog: &RelationalAnalysisCatalogBuilder,
        structural: &StructuralMechanismCatalogBuilder,
        view_id: ViewId,
        layer: &IncidenceResultLayer<'_>,
        closure: RelationalMechanismClosureReceipt,
        runtime: &mut R,
    ) -> Result<RelationalIncidenceResultStepOutcome, RelationalIncidenceResultStepDriverError>
    {
        let evidence = catalog.result_evidence(view_id)?;
        let Some(input_seal) = evidence.input_seal() else {
            return Err(RelationalIncidenceResultStepDriverError::ResultInputSealMismatch(view_id));
        };
        let structural_closure = structural.closure().ok_or(
            RelationalIncidenceResultStepDriverError::StructuralCatalogNotClosed {
                view_id,
                request_id: layer.request_id,
            },
        )?;
        let expected_input_seal = closure
            .result_input_seal()
            .with_structural_quotient(structural_closure.root())?;
        let coverage = input_seal.coverage();
        if input_seal != expected_input_seal
            || coverage.input_kind() != ResultViewInputKind::Incidence
            || coverage.row_count()
                != catalog
                    .mechanism_incidence(layer.request_id)?
                    .incidence_case_count() as u128
        {
            return Err(RelationalIncidenceResultStepDriverError::ResultInputSealMismatch(view_id));
        }
        let evidence_root = evidence.root();
        let needs_rebuild = self
            .publication_cache
            .borrow()
            .get(&view_id)
            .is_none_or(|cached| cached.evidence_root != evidence_root);
        if needs_rebuild {
            let incidence = catalog.mechanism_incidence(layer.request_id)?;
            let borrowed_group_close = layer.executor.spec().supports_borrowed_group_close();
            let mut execution = (!borrowed_group_close).then(|| layer.executor.execution());
            let mut durable_contributions = Vec::with_capacity(if borrowed_group_close {
                evidence.len()
            } else {
                0
            });
            for record in evidence.records() {
                let ResultViewInputRowId::Incidence(row_id) = record.row_id() else {
                    return Err(
                        RelationalIncidenceResultStepDriverError::UnexpectedResultRowKind(view_id),
                    );
                };
                self.validate_row_against_incidence(incidence, view_id, layer.request_id, row_id)?;
                let case = view.case(row_id.case_id()).ok_or(
                    RelationalIncidenceResultStepDriverError::UnknownIncidenceCase(
                        row_id.case_id(),
                    ),
                )?;
                let assignment = structural.assignment(row_id.signature_id()).ok_or(
                    RelationalIncidenceResultStepDriverError::StructuralAssignmentMissing {
                        view_id,
                        signature_id: row_id.signature_id(),
                    },
                )?;
                let mut evaluated = layer.executor.evaluate_concrete_incidence(
                    case,
                    row_id.transition_id(),
                    row_id.signature_id(),
                    assignment.mechanism_id(),
                    assignment.profile_id(),
                    runtime,
                )?;
                let rehydrated = RelationalResultEvidenceRecord::from_evaluated(&evaluated);
                if &rehydrated != record {
                    return Err(
                        RelationalIncidenceResultStepDriverError::DurableEvidenceMismatch {
                            view_id,
                            row_id,
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
                CachedIncidenceProjection {
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
            let cached = cache
                .get_mut(&view_id)
                .ok_or(RelationalIncidenceResultStepDriverError::ProjectionCacheMissing(view_id))?;
            Self::validate_cached_projection_prefix(projection, cached, view_id)?;

            let first_ordinal = projection.len();
            if first_ordinal < cached.records.len() {
                let end = first_ordinal
                    .saturating_add(usize::from(self.max_rows_per_quantum.get()))
                    .min(cached.records.len());
                let chunk = &cached.records[first_ordinal..end];
                let record_count = NonZeroU16::new(u16::try_from(chunk.len()).map_err(|_| {
                    RelationalIncidenceResultStepDriverError::ChunkRowCountOverflow
                })?)
                .ok_or(RelationalIncidenceResultStepDriverError::ChunkMadeNoProgress)?;
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
                    RelationalIncidenceResultStepQuantum::PublishProjectionRecords {
                        view_id,
                        request_id: layer.request_id,
                        first_ordinal: first_ordinal as u128,
                        record_count,
                    },
                    events,
                ));
            }

            // The journal-owned projection is complete. Release the second
            // full record copy before deriving the authenticated closure.
            cache
                .remove(&view_id)
                .ok_or(RelationalIncidenceResultStepDriverError::ProjectionCacheMissing(view_id))?
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
                RelationalIncidenceResultStepDriverError::DurableProjectionRootMismatch {
                    view_id,
                    evaluated: result_root,
                    durable: *durable_root,
                },
            );
        }
        Ok(self.batch(
            view,
            RelationalIncidenceResultStepQuantum::PublishResult {
                view_id,
                request_id: layer.request_id,
                result_root,
            },
            vec![RelationalJournalEvent::analysis(event)],
        ))
    }

    fn validate_cached_projection_prefix(
        projection: &ResultProjectionCatalogBuilder,
        cached: &mut CachedIncidenceProjection,
        view_id: ViewId,
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        match projection.validate_expected_prefix(&cached.records, &mut cached.validated_prefix) {
            Ok(()) => Ok(()),
            Err(ResultProjectionError::ExpectedPrefixTooShort {
                durable_records,
                expected_records,
            }) => Err(
                RelationalIncidenceResultStepDriverError::DurableProjectionTooLong {
                    view_id,
                    durable_records,
                    evaluated_records: expected_records,
                },
            ),
            Err(ResultProjectionError::ExpectedRecordMismatch { ordinal }) => Err(
                RelationalIncidenceResultStepDriverError::DurableProjectionMismatch {
                    view_id,
                    ordinal,
                },
            ),
            Err(error) => Err(error.into()),
        }
    }

    fn validate_terminal_incidence_coverage(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
        evidence: &RelationalResultEvidenceCatalogBuilder,
        view_id: ViewId,
        request_id: MechanismRequestId,
        expected_rows: u128,
        pending_rows: &[AvailableIncidenceRow],
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        let actual_rows = (evidence.len() as u128)
            .checked_add(pending_rows.len() as u128)
            .ok_or(RelationalIncidenceResultStepDriverError::IncidenceRowCountOverflow)?;
        if actual_rows != expected_rows {
            return Err(
                RelationalIncidenceResultStepDriverError::IncidenceCoverageCountMismatch {
                    expected: expected_rows,
                    actual: actual_rows,
                },
            );
        }
        for record in evidence.records() {
            let ResultViewInputRowId::Incidence(row_id) = record.row_id() else {
                return Err(
                    RelationalIncidenceResultStepDriverError::UnexpectedResultRowKind(view_id),
                );
            };
            self.validate_row_against_incidence(incidence, view_id, request_id, row_id)?;
        }
        for pending in pending_rows {
            self.validate_row_against_incidence(incidence, view_id, request_id, pending.row_id)?;
        }
        Ok(())
    }

    fn validate_row_against_incidence(
        &self,
        incidence: &MechanismIncidenceCatalogBuilder,
        view_id: ViewId,
        request_id: MechanismRequestId,
        row_id: MechanismIncidenceRowId,
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        self.validate_row_request(view_id, request_id, row_id)?;
        let expected = MechanismCaseTerminal::Incidence {
            transition_id: row_id.transition_id(),
            signature_id: row_id.signature_id(),
        };
        if incidence.terminal(row_id.case_id()) != Some(expected) {
            return Err(
                RelationalIncidenceResultStepDriverError::ResultEvidenceOutsideIncidence {
                    view_id,
                    row_id,
                },
            );
        }
        Ok(())
    }

    fn validate_row_request(
        &self,
        view_id: ViewId,
        request_id: MechanismRequestId,
        row_id: MechanismIncidenceRowId,
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        if row_id.signature_id().request_id() != request_id {
            return Err(
                RelationalIncidenceResultStepDriverError::IncidenceRequestMismatch {
                    view_id,
                    expected: request_id,
                    actual: row_id.signature_id().request_id(),
                },
            );
        }
        Ok(())
    }

    fn require_registered_spec(
        &self,
        catalog: &RelationalAnalysisCatalogBuilder,
        view_id: ViewId,
        layer: &IncidenceResultLayer<'_>,
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        if catalog.result_spec(view_id)? != layer.executor.spec() {
            return Err(RelationalIncidenceResultStepDriverError::RegisteredSpecMismatch(view_id));
        }
        Ok(())
    }

    fn validate_scope(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalIncidenceResultStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != self.relation_id
            || contract.admission_id() != self.admission_id
            || contract.question_ids() != self.question_ids.as_ref()
        {
            return Err(RelationalIncidenceResultStepDriverError::JournalScopeMismatch);
        }
        match view.analysis_plan_root() {
            Some(actual) if actual == self.analysis_plan_root => Ok(()),
            Some(actual) => Err(
                RelationalIncidenceResultStepDriverError::AnalysisPlanRootMismatch {
                    expected: self.analysis_plan_root,
                    actual,
                },
            ),
            None => Err(RelationalIncidenceResultStepDriverError::AnalysisPlanMissing),
        }
    }

    fn batch(
        &self,
        view: RelationalSchedulerView<'_>,
        quantum: RelationalIncidenceResultStepQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalIncidenceResultStepOutcome {
        debug_assert!(!events.is_empty());
        RelationalIncidenceResultStepOutcome::Emitted(RelationalIncidenceResultStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum,
            events: events.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIncidenceResultStepDriverError {
    AnalysisPlan(RelationalAnalysisPlanError),
    Catalog(RelationalAnalysisCatalogError),
    AnalysisJournal(RelationalAnalysisJournalError),
    Journal(RelationalJournalError),
    ResultExecutor(RelationalResultExecutorError),
    ResultProjection(ResultProjectionError),
    ResultEvidence(ResultEvidenceError),
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
    MechanismClosureArtifactMissing {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    StructuralCatalogMissing {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    StructuralCatalogNotClosed {
        view_id: ViewId,
        request_id: MechanismRequestId,
    },
    StructuralAssignmentMissing {
        view_id: ViewId,
        signature_id: MechanismSignatureId,
    },
    UnknownIncidenceCase(RelationalCaseId),
    UnexpectedResultRowKind(ViewId),
    IncidenceRequestMismatch {
        view_id: ViewId,
        expected: MechanismRequestId,
        actual: MechanismRequestId,
    },
    ResultEvidenceOutsideIncidence {
        view_id: ViewId,
        row_id: MechanismIncidenceRowId,
    },
    DurableEvidenceMismatch {
        view_id: ViewId,
        row_id: MechanismIncidenceRowId,
    },
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
    IncidenceCoverageCountMismatch {
        expected: u128,
        actual: u128,
    },
    IncidenceRowCountOverflow,
    ChunkRowCountOverflow,
    ChunkMadeNoProgress,
}

impl fmt::Display for RelationalIncidenceResultStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisPlan(error) => {
                write!(formatter, "incidence-result analysis plan failed: {error}")
            }
            Self::Catalog(error) => {
                write!(
                    formatter,
                    "incidence-result analysis catalog failed: {error}"
                )
            }
            Self::AnalysisJournal(error) => {
                write!(formatter, "incidence-result analysis event failed: {error}")
            }
            Self::Journal(error) => write!(formatter, "incidence-result journal failed: {error}"),
            Self::ResultExecutor(error) => {
                write!(formatter, "incidence-result execution failed: {error}")
            }
            Self::ResultProjection(error) => {
                write!(formatter, "incidence-result publication failed: {error}")
            }
            Self::ResultEvidence(error) => {
                write!(formatter, "incidence-result input evidence failed: {error}")
            }
            Self::JournalScopeMismatch => {
                formatter.write_str("incidence-result driver and relational journal scopes differ")
            }
            Self::AnalysisPlanMissing => formatter
                .write_str("incidence-result execution requires a registered analysis plan"),
            Self::AnalysisPlanRootMismatch { .. } => formatter
                .write_str("incidence-result driver and journal analysis plan roots differ"),
            Self::AnalysisStateMissing => {
                formatter.write_str("registered analysis plan has no analysis journal state")
            }
            Self::AnalysisCatalogMissing => {
                formatter.write_str("open incidence-result execution has no analysis catalog")
            }
            Self::AnalysisLayerMissing(_) => formatter
                .write_str("checked incidence-result layer is absent from the analysis catalog"),
            Self::AnalysisLayerKindMismatch(_) => formatter
                .write_str("checked incidence-result layer has a different analysis kind or input"),
            Self::DuplicateResultView(_) => {
                formatter.write_str("checked query repeats a semantic incidence-result ViewId")
            }
            Self::RegisteredSpecMismatch(_) => formatter
                .write_str("journaled incidence-result spec differs from its checked lowered spec"),
            Self::ResultLayerStateMismatch(_) => {
                formatter.write_str("incidence-result layer status and evidence frontier disagree")
            }
            Self::ResultInputSealMismatch(_) => formatter
                .write_str("incidence-result input is not sealed by its mechanism incidence"),
            Self::MechanismClosureArtifactMissing { .. } => formatter.write_str(
                "incidence-result input is sealed but its durable mechanism closure is absent",
            ),
            Self::StructuralCatalogMissing { .. } => formatter
                .write_str("incidence-result publication requires its durable structural catalog"),
            Self::StructuralCatalogNotClosed { .. } => formatter
                .write_str("incidence-result publication requires a closed structural quotient"),
            Self::StructuralAssignmentMissing { .. } => {
                formatter.write_str("mechanism-incidence row has no durable structural assignment")
            }
            Self::UnknownIncidenceCase(_) => {
                formatter.write_str("mechanism incidence names no durable relational case")
            }
            Self::UnexpectedResultRowKind(_) => {
                formatter.write_str("incidence-result evidence contains a selected-case row")
            }
            Self::IncidenceRequestMismatch { .. } => formatter
                .write_str("incidence-result row signature belongs to another mechanism request"),
            Self::ResultEvidenceOutsideIncidence { .. } => formatter.write_str(
                "incidence-result evidence is absent from the durable mechanism incidence",
            ),
            Self::DurableEvidenceMismatch { .. } => formatter.write_str(
                "rehydrated checked incidence-result evaluation differs from durable evidence",
            ),
            Self::ProjectionCacheMissing(_) => formatter.write_str(
                "incidence-result projection cache disappeared during one publication quantum",
            ),
            Self::DurableProjectionMismatch { .. } => formatter.write_str(
                "durable incidence-result projection differs from deterministic reevaluation",
            ),
            Self::DurableProjectionTooLong { .. } => formatter.write_str(
                "durable incidence-result projection exceeds deterministic reevaluation",
            ),
            Self::DurableProjectionRootMismatch { .. } => formatter.write_str(
                "durable incidence-result projection root differs from checked reevaluation",
            ),
            Self::IncidenceCoverageCountMismatch { expected, actual } => write!(
                formatter,
                "incidence-result coverage has {actual} rows; exact seal requires {expected}",
            ),
            Self::IncidenceRowCountOverflow => {
                formatter.write_str("incidence-result row count overflowed u128")
            }
            Self::ChunkRowCountOverflow => {
                formatter.write_str("incidence-result evidence chunk exceeded its u16 bound")
            }
            Self::ChunkMadeNoProgress => {
                formatter.write_str("nonempty incidence-result chunk reported zero rows")
            }
        }
    }
}

impl Error for RelationalIncidenceResultStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisPlan(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::AnalysisJournal(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::ResultExecutor(error) => Some(error),
            Self::ResultProjection(error) => Some(error),
            Self::ResultEvidence(error) => Some(error),
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
            | Self::MechanismClosureArtifactMissing { .. }
            | Self::StructuralCatalogMissing { .. }
            | Self::StructuralCatalogNotClosed { .. }
            | Self::StructuralAssignmentMissing { .. }
            | Self::UnknownIncidenceCase(_)
            | Self::UnexpectedResultRowKind(_)
            | Self::IncidenceRequestMismatch { .. }
            | Self::ResultEvidenceOutsideIncidence { .. }
            | Self::DurableEvidenceMismatch { .. }
            | Self::ProjectionCacheMissing(_)
            | Self::DurableProjectionMismatch { .. }
            | Self::DurableProjectionTooLong { .. }
            | Self::DurableProjectionRootMismatch { .. }
            | Self::IncidenceCoverageCountMismatch { .. }
            | Self::IncidenceRowCountOverflow
            | Self::ChunkRowCountOverflow
            | Self::ChunkMadeNoProgress => None,
        }
    }
}

impl From<RelationalAnalysisPlanError> for RelationalIncidenceResultStepDriverError {
    fn from(error: RelationalAnalysisPlanError) -> Self {
        Self::AnalysisPlan(error)
    }
}

impl From<RelationalAnalysisCatalogError> for RelationalIncidenceResultStepDriverError {
    fn from(error: RelationalAnalysisCatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<RelationalAnalysisJournalError> for RelationalIncidenceResultStepDriverError {
    fn from(error: RelationalAnalysisJournalError) -> Self {
        Self::AnalysisJournal(error)
    }
}

impl From<RelationalJournalError> for RelationalIncidenceResultStepDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<RelationalResultExecutorError> for RelationalIncidenceResultStepDriverError {
    fn from(error: RelationalResultExecutorError) -> Self {
        Self::ResultExecutor(error)
    }
}

impl From<ResultProjectionError> for RelationalIncidenceResultStepDriverError {
    fn from(error: ResultProjectionError) -> Self {
        Self::ResultProjection(error)
    }
}

impl From<ResultEvidenceError> for RelationalIncidenceResultStepDriverError {
    fn from(error: ResultEvidenceError) -> Self {
        Self::ResultEvidence(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::mechanism_support::{
        MechanismSupportCheckpointCursor, MechanismSupportFrontierRoot,
    };
    use crate::explore::relational_case_executor::{
        RelationalCaseExecutor, RelationalSuccessorAdvance,
    };
    use crate::explore::relational_executor::{
        RelationalBoundValue, RelationalExpressionRuntime, RelationalSourceAdvance,
        RelationalSourceContinuation, RelationalSourceEnumerator,
    };
    use crate::explore::relational_journal::{
        RelationalEvidenceEvent, RelationalJournalContract, RelationalMechanismSupportStepEvents,
    };
    use crate::explore::relational_mechanism_executor::{
        derive_relational_structural_mechanism_v1, replay_relational_mechanism_case,
        RelationalMechanismActivationStep, RelationalMechanismCalleeId,
        RelationalMechanismEndpointReplayProgress, RelationalMechanismEndpointReplayRequest,
        RelationalMechanismEndpointTraceProposal, RelationalMechanismReplayOutcome,
        RelationalMechanismReplayRuntime, RelationalMechanismSiteId,
    };
    use crate::explore::relational_support_planner::RelationalSupportPlanner;
    use crate::explore::result_view::{ResultValue, ResultViewInputRowId};
    use crate::explore::structural_mechanism::relational_structural_derivation_budget;
    use crate::{Expr, ExprKind, Lexer, Literal, Parser, Ty, TypeChecker};

    struct LiteralRuntime;

    impl RelationalExpressionRuntime for LiteralRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            _earlier_bindings: &[RelationalBoundValue<'_>],
        ) -> Result<super::super::ExploreValue, String> {
            match expression.kind {
                ExprKind::Lit(Literal::Int(value)) => Ok(super::super::ExploreValue::Int(value)),
                ref other => Err(format!(
                    "structural-gate fixture expected an integer literal, got {other:?}"
                )),
            }
        }
    }

    struct VariableResultRuntime;

    impl RelationalResultExpressionRuntime for VariableResultRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            bindings: &[super::super::relational_result_executor::RelationalResultBinding],
        ) -> Result<ResultValue, String> {
            let ExprKind::Var(name) = &expression.kind else {
                return Err(format!(
                    "structural-gate result fixture expected a variable, got {:?}",
                    expression.kind
                ));
            };
            bindings
                .iter()
                .rev()
                .find(|binding| binding.name() == name)
                .map(|binding| binding.value().clone())
                .ok_or_else(|| format!("unbound structural-gate result variable `{name}`"))
        }
    }

    struct EmptyTraceRuntime;

    impl RelationalMechanismReplayRuntime for EmptyTraceRuntime {
        type Error = String;

        fn replay_fresh_endpoint(
            &mut self,
            request: RelationalMechanismEndpointReplayRequest<'_>,
        ) -> Result<RelationalMechanismEndpointReplayProgress, Self::Error> {
            let observation = request.observation();
            let analysis_program = &observation.template_site.analysis_program;
            let call_site =
                RelationalMechanismSiteId::from_checked_expression(&observation.template_site)
                    .map_err(|error| error.to_string())?;
            let callee = RelationalMechanismSiteId::from_checked_callable(
                analysis_program,
                &observation.endpoint_template,
            )
            .map_err(|error| error.to_string())?;
            let root = RelationalMechanismActivationStep::new(
                call_site,
                RelationalMechanismCalleeId::function(callee).map_err(|error| error.to_string())?,
                0,
            )
            .map_err(|error| error.to_string())?;
            Ok(RelationalMechanismEndpointReplayProgress::Complete(
                RelationalMechanismEndpointTraceProposal::empty(root),
            ))
        }
    }

    fn append_batch(journal: &mut RelationalJournal, batch: RelationalIncidenceResultStepBatch) {
        assert_eq!(batch.expected_sequence(), journal.next_sequence());
        assert_eq!(batch.expected_head(), journal.head());
        for event in batch.into_events().into_vec() {
            journal.append(event).expect("append driver event");
        }
    }

    fn append_support_checkpoint(
        journal: &mut RelationalJournal,
        request_id: MechanismRequestId,
        expected_cursor: MechanismSupportCheckpointCursor,
        expected_accepted_target_cases: usize,
    ) -> MechanismSupportFrontierRoot {
        let RelationalMechanismSupportStepEvents::Checkpoint {
            accepted_target_cases,
            cursor,
            frontier_root,
            events,
        } = journal
            .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
            .expect("advance bounded support frontier")
        else {
            panic!("visible support suffix must emit one checkpoint");
        };
        assert_eq!(accepted_target_cases, expected_accepted_target_cases);
        assert_eq!(cursor, expected_cursor);
        for event in events.into_vec() {
            journal.append(event).expect("append support checkpoint");
        }
        frontier_root
    }

    fn append_support_observation(
        journal: &mut RelationalJournal,
        request_id: MechanismRequestId,
        expected_sealed: bool,
    ) {
        let RelationalMechanismSupportStepEvents::Observed { status, events, .. } = journal
            .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
            .expect("derive support observation")
        else {
            panic!("durable support frontier must emit its pending observation");
        };
        assert_eq!(status.is_sealed(), expected_sealed);
        for event in events.into_vec() {
            journal.append(event).expect("append support observation");
        }
    }

    fn expect_awaiting(
        outcome: RelationalIncidenceResultStepOutcome,
        view_id: ViewId,
        request_id: MechanismRequestId,
    ) {
        assert_eq!(
            outcome,
            RelationalIncidenceResultStepOutcome::Quiescent(
                RelationalIncidenceResultStepQuiescence::AwaitingMechanismIncidence {
                    view_id,
                    request_id,
                }
            )
        );
    }

    #[test]
    fn support_observation_lifecycle_gates_incidence_and_closure() {
        let source = r#"
> structural_gate_observe(state: Int, context: Int) -> Int {
    state + context
}

? explore structural_gate_fixture {
    from {
        given before = 1
        given context = 0
    }
    transition after = 2
    find all_cases = all
    mechanisms paths from find all_cases using structural_gate_observe
    results incidences from mechanisms paths {
        each incidence
        select [structural_mechanism_id, execution_profile_id]
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let user_statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse structural-gate Explore fixture");
        let statements = crate::prepend_prelude(crate::parse_prelude(), &user_statements);
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("joined checked Explore fixture");
        let question_id = checked.question_ids()[0];
        let analysis_plan = RelationalAnalysisPlan::from_checked(&checked).expect("analysis plan");
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("support plan");
        let contract = RelationalJournalContract::new(
            checked.relation_id(),
            checked.admission_id(),
            checked.question_ids().iter().copied(),
            checked.transition_schemas().state_schema_id(),
            checked.transition_schemas().context_schema_id(),
            checked.transition_schemas().transition_type_id(),
            analysis_plan.producer_graph_digest().bytes(),
        );
        let mut journal = RelationalJournal::new(contract);
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .expect("register analysis plan");
        journal
            .append(RelationalJournalEvent::support_plan_registered(
                support_plan,
            ))
            .expect("register support plan");

        let sources =
            RelationalSourceEnumerator::new(checked.relation_id(), &checked.closed_query.source)
                .expect("source enumerator");
        let mut literal_runtime = LiteralRuntime;
        let mut cursors = vec![sources.root_cursor().expect("root source cursor")];
        let mut source_key = None;
        while let Some(cursor) = cursors.pop() {
            let advance = sources
                .advance(&cursor, &mut literal_runtime)
                .expect("advance fixture source");
            if let RelationalSourceAdvance::Yielded {
                resume,
                continuation,
                ..
            } = &advance
            {
                cursors.push(resume.clone());
                match continuation {
                    RelationalSourceContinuation::Expand(child) => cursors.push(child.clone()),
                    RelationalSourceContinuation::Source(source) => {
                        assert!(source_key.replace(source.source_key()).is_none());
                    }
                }
            }
            let event = journal
                .source_traversal_event(advance)
                .expect("bind source traversal to support plan");
            journal.append(event).expect("append source traversal");
        }
        journal
            .append(
                journal
                    .source_enumeration_seal_event()
                    .expect("source relation seal"),
            )
            .expect("append source relation seal");
        let source_key = source_key.expect("one fixture source");
        let source_row = journal
            .scheduler_view()
            .expect("scheduler view")
            .source_row(source_key)
            .expect("durable source row")
            .clone();
        let cases = RelationalCaseExecutor::new(checked.relation_id(), checked.closed_query)
            .expect("case executor");
        let successor = cases
            .advance(
                &cases
                    .root_cursor(source_key, &source_row)
                    .expect("root successor cursor"),
                &source_row,
                &mut literal_runtime,
            )
            .expect("advance singleton successor");
        let (case_id, successor_resume, discovered) = match successor {
            RelationalSuccessorAdvance::Yielded { case, resume, .. } => {
                (case.case_id(), resume, case.discovered_event())
            }
            other => panic!("singleton successor must yield one case: {other:?}"),
        };
        journal.append(discovered).expect("append concrete case");
        let exhausted = cases
            .advance(&successor_resume, &source_row, &mut literal_runtime)
            .expect("exhaust singleton successor");
        let receipt = match exhausted {
            RelationalSuccessorAdvance::Exhausted { receipt, .. } => receipt,
            other => panic!("singleton successor must exhaust: {other:?}"),
        };
        journal
            .append(RelationalJournalEvent::successor_fiber_exhaustion_accepted(
                receipt.clone(),
            ))
            .expect("append successor exhaustion receipt");
        journal
            .append(RelationalJournalEvent::successor_enumeration_sealed(
                &receipt,
            ))
            .expect("append successor seal");
        journal
            .append(RelationalJournalEvent::admission_classified(
                case_id,
                super::super::relation::AdmissionDecision::Admitted,
            ))
            .expect("classify admission");
        journal
            .append(RelationalJournalEvent::question_classified(
                question_id,
                case_id,
                super::super::relation::SelectionDecision::Selected,
            ))
            .expect("classify FIND");
        journal
            .append(
                journal
                    .selected_question_extensional_event(question_id)
                    .expect("selected-question closure"),
            )
            .expect("bind selected question");

        let mut request = None;
        let mut result_view = None;
        for (node, identity) in checked.analysis_nodes() {
            match (node, identity) {
                (
                    ExploreAnalysisNodeIr::Mechanisms(_),
                    CheckedExploreAnalysisIdentity::Mechanisms {
                        request_id,
                        observation,
                        endpoint_totality,
                    },
                ) => request = Some((*request_id, observation, endpoint_totality.certificate_id())),
                (
                    ExploreAnalysisNodeIr::Result(result),
                    CheckedExploreAnalysisIdentity::View { view_id, .. },
                ) if matches!(
                    result.input,
                    ExploreResultInputIr::MechanismIncidence { .. }
                ) =>
                {
                    result_view = Some(*view_id)
                }
                _ => {}
            }
        }
        let (request_id, observation, endpoint_totality_certificate_id) =
            request.expect("mechanism request identity");
        let view_id = result_view.expect("incidence result identity");
        let driver = RelationalIncidenceResultStepDriver::from_checked(&checked)
            .expect("incidence-result driver");
        let mut result_runtime = VariableResultRuntime;
        let RelationalIncidenceResultStepOutcome::Emitted(register) = driver
            .step(&journal, &mut result_runtime)
            .expect("register incidence result")
        else {
            panic!("unregistered incidence view must emit its spec");
        };
        assert_eq!(
            register.quantum(),
            RelationalIncidenceResultStepQuantum::RegisterSpec {
                view_id,
                request_id,
            }
        );
        append_batch(&mut journal, register);

        let question_seal = journal
            .analysis_state()
            .and_then(|analysis| analysis.selected_question(checked.question_ids()[0]))
            .expect("selected-question seal");
        journal
            .append(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_target_case_accepted(
                    request_id, case_id,
                ),
            ))
            .expect("append mechanism target case");
        journal
            .append(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_target_sealed_from_selected(
                    request_id,
                    question_seal,
                ),
            ))
            .expect("seal mechanism target");

        let evidence_contract = journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .mechanism_evidence_contract(request_id)
            .expect("mechanism evidence contract");
        let case = journal
            .scheduler_view()
            .expect("scheduler view")
            .case(case_id)
            .expect("durable mechanism case");
        let evidence = match replay_relational_mechanism_case(
            &mut EmptyTraceRuntime,
            evidence_contract.scope(),
            endpoint_totality_certificate_id,
            observation,
            checked.transition_schemas(),
            case,
        )
        .expect("replay mechanism fixture")
        {
            RelationalMechanismReplayOutcome::Observed(evidence) => evidence,
            other => panic!("fixture replay must produce incidence: {other:?}"),
        };
        let transition_id = evidence.transition_id();
        let signature_id = evidence.signature_id();
        for event in RelationalAnalysisEvidenceEvent::mechanism_signature_artifact_events(
            evidence.definition(),
        )
        .expect("signature artifact events")
        .into_vec()
        {
            journal
                .append(RelationalJournalEvent::analysis(event))
                .expect("append signature artifact event");
        }
        for event in RelationalAnalysisEvidenceEvent::
            mechanism_compact_incidence_artifact_events_with_chunk_bytes(
                evidence_contract,
                &evidence,
                4096,
            )
            .expect("incidence artifact events")
            .into_vec()
        {
            journal
                .append(RelationalJournalEvent::analysis(event))
                .expect("append incidence artifact event");
        }

        let terminal_prefix_root = append_support_checkpoint(
            &mut journal,
            request_id,
            MechanismSupportCheckpointCursor::new(1, 1, 0),
            1,
        );
        assert!(matches!(
            journal
                .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
                .expect("open caught-up support frontier"),
            RelationalMechanismSupportStepEvents::Idle
        ));

        // A successful raw terminal is not yet a typed structural incidence
        // row. The driver must leave it pending, not evaluate it with guessed
        // or placeholder quotient IDs.
        expect_awaiting(
            driver
                .step(&journal, &mut result_runtime)
                .expect("raw-incidence wait"),
            view_id,
            request_id,
        );
        let result_evidence = journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .result_evidence(view_id)
            .expect("result evidence catalog");
        assert_eq!(result_evidence.len(), 0);
        assert!(!result_evidence.input_is_sealed());

        let raw_closure = journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .mechanism_closure_receipt(request_id)
            .expect("raw incidence closure receipt");
        journal
            .append(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_incidence_closed(raw_closure),
            ))
            .expect("append raw incidence closure");
        let raw_closed_prefix_root = append_support_checkpoint(
            &mut journal,
            request_id,
            MechanismSupportCheckpointCursor::new(1, 1, 0),
            0,
        );
        assert_ne!(raw_closed_prefix_root, terminal_prefix_root);
        assert!(matches!(
            journal
                .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
                .expect("raw-closed support frontier awaiting structural assignment"),
            RelationalMechanismSupportStepEvents::Idle
        ));
        expect_awaiting(
            driver
                .step(&journal, &mut result_runtime)
                .expect("closed-raw wait for structural assignment"),
            view_id,
            request_id,
        );

        let definition = journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .mechanism_incidence(request_id)
            .expect("mechanism incidence catalog")
            .signature_definition(signature_id)
            .expect("raw signature definition")
            .clone();
        let artifact = derive_relational_structural_mechanism_v1(
            &definition,
            evidence_contract.scope(),
            relational_structural_derivation_budget(),
        )
        .expect("derive structural quotient artifact");
        let structural_mechanism_id = artifact.mechanism().id();
        let execution_profile_id = artifact.profile().id();
        loop {
            let event = journal
                .analysis_state()
                .expect("analysis state")
                .next_structural_quotient_artifact_event(&artifact, 4096)
                .expect("next structural artifact event");
            let artifact_is_closed = matches!(
                event,
                RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { .. }
            );
            journal
                .append(RelationalJournalEvent::analysis(event))
                .expect("append structural artifact event");
            if artifact_is_closed {
                break;
            }
        }
        let assignment = journal
            .analysis_state()
            .and_then(|analysis| analysis.structural_mechanism_catalog(request_id))
            .and_then(|catalog| catalog.assignment(signature_id))
            .expect("durable structural signature assignment");
        assert_eq!(assignment.mechanism_id(), structural_mechanism_id);
        assert_eq!(assignment.profile_id(), execution_profile_id);

        let assigned_prefix_root = append_support_checkpoint(
            &mut journal,
            request_id,
            MechanismSupportCheckpointCursor::new(1, 1, 1),
            0,
        );
        assert_ne!(assigned_prefix_root, terminal_prefix_root);
        append_support_observation(&mut journal, request_id, false);
        assert!(matches!(
            journal
                .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
                .expect("assigned but unsealed support frontier"),
            RelationalMechanismSupportStepEvents::Idle
        ));

        let RelationalIncidenceResultStepOutcome::Emitted(evaluate) = driver
            .step(&journal, &mut result_runtime)
            .expect("evaluate structurally assigned incidence")
        else {
            panic!("durable structural assignment must release one incidence row");
        };
        assert_eq!(
            evaluate.quantum(),
            RelationalIncidenceResultStepQuantum::EvaluateRows {
                view_id,
                request_id,
                first_row_id: MechanismIncidenceRowId::new(case_id, transition_id, signature_id,),
                row_count: NonZeroU16::MIN,
                seals_input: false,
            }
        );
        assert_eq!(evaluate.events().len(), 1);
        let record = match &evaluate.events()[0] {
            RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                RelationalAnalysisEvidenceEvent::ResultEvidenceAccepted { record, .. },
            )) => record,
            other => panic!("expected typed result evidence, got {other:?}"),
        };
        assert_eq!(
            record.row_id(),
            ResultViewInputRowId::Incidence(MechanismIncidenceRowId::new(
                case_id,
                transition_id,
                signature_id,
            ))
        );
        assert_eq!(
            record
                .early_select_iter()
                .map(|value| value.cloned())
                .collect::<Vec<_>>(),
            vec![
                Some(ResultValue::StructuralMechanismId(structural_mechanism_id,)),
                Some(ResultValue::ExecutionProfileId(execution_profile_id)),
            ]
        );
        append_batch(&mut journal, evaluate);

        // Raw incidence closure alone is insufficient: even with every typed
        // row durable, the result input stays open until the structural
        // quotient has its own exact closure authority.
        expect_awaiting(
            driver
                .step(&journal, &mut result_runtime)
                .expect("wait for structural quotient closure"),
            view_id,
            request_id,
        );
        assert!(!journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .result_evidence(view_id)
            .expect("result evidence catalog")
            .input_is_sealed());

        let structural_close_event = journal
            .analysis_state()
            .expect("analysis state")
            .structural_quotient_closure_event(request_id)
            .expect("structural quotient closure event");
        journal
            .append(RelationalJournalEvent::analysis(structural_close_event))
            .expect("append structural quotient closure");
        let structural_closure = journal
            .analysis_state()
            .and_then(|analysis| analysis.structural_quotient_closure(request_id))
            .expect("durable structural quotient closure");

        let sealed_frontier_root = append_support_checkpoint(
            &mut journal,
            request_id,
            MechanismSupportCheckpointCursor::new(1, 1, 1),
            0,
        );
        assert_ne!(sealed_frontier_root, assigned_prefix_root);
        // Attaching upstream closure roots enriches the same support prefix,
        // but does not change this mechanism's own indexed support. Its prior
        // open point remains valid historical evidence; the next point is the
        // final sealed successor after support closure.
        let RelationalMechanismSupportStepEvents::Closed {
            checkpointed_frontier,
            cursor,
            events,
            ..
        } = journal
            .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
            .expect("close fully sealed support frontier")
        else {
            panic!("durable final support checkpoint must release closure");
        };
        assert!(!checkpointed_frontier);
        assert_eq!(cursor, MechanismSupportCheckpointCursor::new(1, 1, 1));
        for event in events.into_vec() {
            journal.append(event).expect("append support closure");
        }
        append_support_observation(&mut journal, request_id, true);
        assert!(matches!(
            journal
                .support_lifecycle_step_events(request_id, NonZeroU16::MIN)
                .expect("sealed observed support frontier"),
            RelationalMechanismSupportStepEvents::Idle
        ));

        let RelationalIncidenceResultStepOutcome::Emitted(seal) = driver
            .step(&journal, &mut result_runtime)
            .expect("seal incidence result input")
        else {
            panic!("both exact upstream closures must release the input seal");
        };
        assert_eq!(
            seal.quantum(),
            RelationalIncidenceResultStepQuantum::SealInput {
                view_id,
                request_id,
            }
        );
        assert_eq!(seal.events().len(), 1);
        match &seal.events()[0] {
            RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                RelationalAnalysisEvidenceEvent::ResultInputSealedFromMechanisms {
                    view_id: sealed_view,
                    request_id: sealed_request,
                    incidence_root,
                    structural_root,
                },
            )) => {
                assert_eq!(*sealed_view, view_id);
                assert_eq!(*sealed_request, request_id);
                assert_eq!(*incidence_root, raw_closure.incidence_root());
                assert_eq!(*structural_root, structural_closure.root());
            }
            other => panic!("expected two-root incidence input seal, got {other:?}"),
        }
        append_batch(&mut journal, seal);

        let durable_seal = journal
            .analysis_state()
            .and_then(|analysis| analysis.open_catalog())
            .expect("open analysis catalog")
            .result_evidence(view_id)
            .expect("result evidence catalog")
            .input_seal()
            .expect("durable result input seal");
        assert_eq!(durable_seal.coverage().row_count(), 1);
        assert_eq!(
            durable_seal.upstream(),
            super::super::result_evidence::ResultEvidenceUpstreamRoot::
                StructuralMechanismIncidence {
                    request_id,
                    completed_root: raw_closure.incidence_root(),
                    structural_root: structural_closure.root(),
                }
        );
    }
}
