//! Bounded post-FIND execution for selected-case mechanism requests.
//!
//! This module plans one deterministic, head-bound quantum per call. It is not
//! an execution loop: an outer durable coordinator owns append, retry,
//! deadlines, and resource policy. The analysis catalog itself is the durable
//! progress frontier. Target members are admitted in canonical `CaseId` order;
//! each already-selected member may be replayed while FIND remains open; a
//! signature definition is journaled once before compact per-case replay
//! receipts may reference it; and request closure is emitted only after the
//! exact selected target seals and every durable target member has one
//! observed or permanently-unavailable terminal. Once that raw incidence is
//! closed, the same chunk-resume protocol derives exactly one structural
//! quotient assignment per raw signature and seals their complete set.
//!
//! The operational chunk bound is absent from query, plan, event, incidence,
//! and journal identities. A pause produces no semantic event and therefore
//! cannot become complement closure or synthetic unavailability.
//!
//! The generic work frontier currently rejects completion evidence for its
//! mechanism-shaped node variants. This driver therefore does not manufacture
//! a hash-only `WorkCompletionRef`; the ordered target/terminal indexes and the
//! explicit incidence and structural-quotient closures are its only durable
//! resume authority.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView};

use super::mechanism::MechanismObservationIr;
use super::mechanism_incidence::{
    MechanismIncidenceRoot, MechanismSignatureId, MechanismUnavailableReasonId,
};
use super::relation::{
    AdmissionId, MechanismRequestId, QuestionId, RelationId, RelationalCaseId, SelectionDecision,
    ViewId,
};
use super::relational_analysis_catalog::{
    RelationalAnalysisCatalogBuilder, RelationalAnalysisCatalogError, RelationalAnalysisLayerStatus,
};
use super::relational_analysis_journal::{
    RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError,
    RelationalAnalysisJournalState, RelationalSelectedQuestionSeal,
    RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanError, RelationalAnalysisPlanRoot, RelationalResolvedMechanismTarget,
};
use super::relational_ir::{ExploreAnalysisNodeIr, ExploreMechanismTargetIr};
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalSchedulerView,
};
use super::relational_mechanism_executor::{
    derive_relational_structural_mechanism_v1, replay_relational_mechanism_case,
    RelationalMechanismEndpoint, RelationalMechanismReplayError,
    RelationalMechanismReplayObservationId, RelationalMechanismReplayOutcome,
    RelationalMechanismReplayPause, RelationalMechanismReplayRunError,
    RelationalMechanismReplayRuntime,
};
use super::structural_mechanism::{
    relational_structural_derivation_budget, ExecutionProfileId, StructuralMechanismId,
    StructuralQuotientClosureRoot, StructuralSignatureQuotientArtifact,
};
use super::transition::{TransitionId, TransitionSchemaIdentities};

/// Operational description of one emitted mechanism quantum. This is
/// observability metadata, not semantic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStepQuantum {
    AdmitSelectedTargetCases {
        request_id: MechanismRequestId,
        first_case_id: RelationalCaseId,
        case_count: NonZeroU16,
        seals_target: bool,
    },
    SealSelectedTarget {
        request_id: MechanismRequestId,
        exact_case_count: u128,
    },
    ReplayObserved {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        signature_id: MechanismSignatureId,
    },
    ReplayPermanentlyUnavailable {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        endpoint: RelationalMechanismEndpoint,
        reason_id: MechanismUnavailableReasonId,
    },
    CloseIncidence {
        request_id: MechanismRequestId,
        incidence_root: MechanismIncidenceRoot,
    },
    DeriveStructuralQuotient {
        request_id: MechanismRequestId,
        signature_id: MechanismSignatureId,
        structural_mechanism_id: StructuralMechanismId,
        execution_profile_id: ExecutionProfileId,
    },
    CloseStructuralQuotient {
        request_id: MechanismRequestId,
        structural_root: StructuralQuotientClosureRoot,
    },
}

/// One ordered batch bound to the journal prefix from which it was planned.
/// A durable adapter may append a proper prefix: target evidence is before its
/// seal, and terminal evidence is durable before the separate closure event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalMechanismStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalMechanismStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalMechanismStepQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

/// Honest non-progress states. In particular, `ReplayPaused` records no
/// terminal and chosen-view targets are explicit rather than guessed from a
/// selected population.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStepQuiescence {
    AwaitingSelectedQuestion,
    ReplayPaused {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        endpoint: RelationalMechanismEndpoint,
        reason: RelationalMechanismReplayPause,
    },
    SelectedMechanismsComplete,
    DeferredChosenView {
        request_id: MechanismRequestId,
        view_id: ViewId,
    },
    AnalysisAlreadyClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStepOutcome {
    Emitted(RelationalMechanismStepBatch),
    Quiescent(RelationalMechanismStepQuiescence),
}

struct SelectedMechanismLayer<'ir> {
    observation: &'ir MechanismObservationIr,
    replay_observation_id: RelationalMechanismReplayObservationId,
}

/// Invocation-local offsets into journal-rebuilt discovery indexes. They are
/// scheduling hints only: durable target membership, terminal evidence, and
/// structural assignments remain the resume authority, and exact closures
/// still validate their canonical set commitments.
#[derive(Clone, Copy, Debug, Default)]
struct SelectedMechanismDiscoveryCursor {
    target_ordinal: usize,
    terminal_ordinal: usize,
}

/// Checked-query-bound scheduler for selected-case mechanism layers.
pub(crate) struct RelationalMechanismStepDriver<'query> {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    analysis_plan_root: RelationalAnalysisPlanRoot,
    transition_schemas: &'query TransitionSchemaIdentities,
    selected: BTreeMap<MechanismRequestId, SelectedMechanismLayer<'query>>,
    first_deferred_chosen: Option<(MechanismRequestId, ViewId)>,
    /// Purely operational CPU/memory bound; absent from every semantic ID.
    max_target_cases_per_quantum: NonZeroU16,
    /// Purely operational journal-frame payload bound. Artifact identity is
    /// defined by the complete payload digest and typed claim, not this size.
    mechanism_artifact_chunk_bytes: NonZeroU32,
    /// Per-request readiness cursors. A fresh driver repairs them once by
    /// walking durable membership; normal calls then touch only the newly
    /// discovered selected suffix.
    discovery_cursors: RefCell<BTreeMap<MechanismRequestId, SelectedMechanismDiscoveryCursor>>,
    /// One process-local deterministic structural producer. It avoids
    /// rederiving a complete canonical payload for every bounded chunk; a
    /// fresh driver rederives and authenticates it once against the durable
    /// pending prefix before continuing.
    structural_artifact_cache: RefCell<Option<StructuralSignatureQuotientArtifact>>,
}

impl<'query> RelationalMechanismStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalMechanismStepDriverError> {
        Self::from_checked_with_limits(
            checked,
            NonZeroU16::MIN,
            NonZeroU32::new(RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES as u32)
                .expect("the default mechanism artifact chunk is nonzero"),
        )
    }

    pub(crate) fn from_checked_with_max_target_cases_per_quantum(
        checked: &'query CheckedExploreQueryView<'_>,
        max_target_cases_per_quantum: NonZeroU16,
    ) -> Result<Self, RelationalMechanismStepDriverError> {
        Self::from_checked_with_limits(
            checked,
            max_target_cases_per_quantum,
            NonZeroU32::new(RELATIONAL_MECHANISM_ARTIFACT_DEFAULT_CHUNK_BYTES as u32)
                .expect("the default mechanism artifact chunk is nonzero"),
        )
    }

    pub(crate) fn from_checked_with_limits(
        checked: &'query CheckedExploreQueryView<'_>,
        max_target_cases_per_quantum: NonZeroU16,
        mechanism_artifact_chunk_bytes: NonZeroU32,
    ) -> Result<Self, RelationalMechanismStepDriverError> {
        let max_artifact_chunk =
            super::relational_analysis_journal::RELATIONAL_MECHANISM_ARTIFACT_MAX_CHUNK_BYTES;
        if mechanism_artifact_chunk_bytes.get() as usize > max_artifact_chunk {
            return Err(
                RelationalMechanismStepDriverError::ArtifactChunkBytesTooLarge {
                    actual: mechanism_artifact_chunk_bytes.get(),
                    limit: max_artifact_chunk as u32,
                },
            );
        }
        let plan = RelationalAnalysisPlan::from_checked(checked)?;
        if plan.question_id() != checked.question_id() {
            return Err(RelationalMechanismStepDriverError::CheckedPlanScopeMismatch);
        }

        let mut selected = BTreeMap::new();
        let mut deferred_chosen = BTreeMap::new();
        for (node, identity) in checked.analysis_nodes() {
            let (
                ExploreAnalysisNodeIr::Mechanisms(request),
                CheckedExploreAnalysisIdentity::Mechanisms {
                    request_id,
                    observation,
                },
            ) = (node, identity)
            else {
                continue;
            };

            let layer_id = RelationalAnalysisLayerId::Mechanisms(*request_id);
            let registration = plan.registration(layer_id).ok_or(
                RelationalMechanismStepDriverError::AnalysisLayerMissing(layer_id),
            )?;
            let RelationalAnalysisLayerRegistration::Mechanisms(registration) = registration else {
                return Err(
                    RelationalMechanismStepDriverError::AnalysisLayerKindMismatch(layer_id),
                );
            };
            if registration.request_id() != *request_id {
                return Err(
                    RelationalMechanismStepDriverError::AnalysisLayerKindMismatch(layer_id),
                );
            }

            let replay_observation_id =
                RelationalMechanismReplayObservationId::derive_checked(observation)?;
            match (&request.target, registration.target()) {
                (
                    ExploreMechanismTargetIr::SelectedCases,
                    RelationalResolvedMechanismTarget::Selected(question_id),
                ) => {
                    if question_id != checked.question_id() {
                        return Err(RelationalMechanismStepDriverError::CheckedPlanScopeMismatch);
                    }
                    if selected
                        .insert(
                            *request_id,
                            SelectedMechanismLayer {
                                observation,
                                replay_observation_id,
                            },
                        )
                        .is_some()
                    {
                        return Err(
                            RelationalMechanismStepDriverError::DuplicateMechanismRequest(
                                *request_id,
                            ),
                        );
                    }
                }
                (
                    ExploreMechanismTargetIr::ViewChosen { .. },
                    RelationalResolvedMechanismTarget::ChosenView(view_id),
                ) => {
                    if deferred_chosen.insert(*request_id, view_id).is_some() {
                        return Err(
                            RelationalMechanismStepDriverError::DuplicateMechanismRequest(
                                *request_id,
                            ),
                        );
                    }
                }
                _ => {
                    return Err(
                        RelationalMechanismStepDriverError::AnalysisLayerKindMismatch(layer_id),
                    );
                }
            }
        }

        Ok(Self {
            relation_id: checked.relation_id(),
            admission_id: checked.admission_id(),
            question_id: checked.question_id(),
            analysis_plan_root: plan.root(),
            transition_schemas: checked.transition_schemas(),
            selected,
            first_deferred_chosen: deferred_chosen.into_iter().next(),
            max_target_cases_per_quantum,
            mechanism_artifact_chunk_bytes,
            discovery_cursors: RefCell::new(BTreeMap::new()),
            structural_artifact_cache: RefCell::new(None),
        })
    }

    pub(crate) const fn max_target_cases_per_quantum(&self) -> NonZeroU16 {
        self.max_target_cases_per_quantum
    }

    pub(crate) const fn mechanism_artifact_chunk_bytes(&self) -> NonZeroU32 {
        self.mechanism_artifact_chunk_bytes
    }

    /// Plan at most one selected-mechanism quantum against the current durable
    /// prefix. Returned events are unapplied.
    pub(crate) fn step<R: RelationalMechanismReplayRuntime>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<RelationalMechanismStepOutcome, RelationalMechanismStepRunError<R::Error>> {
        let view = journal
            .scheduler_view()
            .map_err(RelationalMechanismStepDriverError::from)?;
        self.validate_scope(view)?;

        let analysis = journal
            .analysis_state()
            .ok_or(RelationalMechanismStepDriverError::AnalysisStateMissing)?;
        if analysis.is_closed() {
            return Ok(RelationalMechanismStepOutcome::Quiescent(
                RelationalMechanismStepQuiescence::AnalysisAlreadyClosed,
            ));
        }
        let question_seal = analysis.selected_question();
        if let Some(question_seal) = question_seal {
            question_seal
                .validate_identity()
                .map_err(RelationalMechanismStepDriverError::from)?;
            if question_seal.question_id() != self.question_id {
                return Err(
                    RelationalMechanismStepDriverError::SelectedQuestionScopeMismatch.into(),
                );
            }
        }

        let catalog = analysis
            .open_catalog()
            .ok_or(RelationalMechanismStepDriverError::AnalysisCatalogMissing)?;
        if catalog.plan_root() != self.analysis_plan_root {
            return Err(
                RelationalMechanismStepDriverError::AnalysisPlanRootMismatch {
                    expected: self.analysis_plan_root,
                    actual: catalog.plan_root(),
                }
                .into(),
            );
        }

        let pending_request_id = analysis.pending_mechanism_artifact_request_id();
        for (request_id, layer) in &self.selected {
            if pending_request_id.is_some_and(|pending| pending != *request_id) {
                continue;
            }
            let layer_id = RelationalAnalysisLayerId::Mechanisms(*request_id);
            let status = catalog.layer_status(layer_id).ok_or(
                RelationalMechanismStepDriverError::AnalysisLayerMissing(layer_id),
            )?;
            let incidence = catalog.mechanism_incidence(*request_id)?;

            if analysis.mechanism_closure(*request_id).is_some() {
                if status != RelationalAnalysisLayerStatus::MechanismClosed {
                    return Err(
                        RelationalMechanismStepDriverError::ClosedIncidenceStateMismatch(
                            *request_id,
                        )
                        .into(),
                    );
                }
                if analysis.structural_quotient_closure(*request_id).is_some() {
                    continue;
                }
                if let Some(signature_id) = analysis.next_structural_signature_id(*request_id)? {
                    let mut cached = self.structural_artifact_cache.borrow_mut();
                    if cached
                        .as_ref()
                        .map(StructuralSignatureQuotientArtifact::signature_id)
                        != Some(signature_id)
                    {
                        let definition = incidence.signature_definition(signature_id).ok_or(
                            RelationalMechanismStepDriverError::
                                MissingStructuralSignatureDefinition {
                                    request_id: *request_id,
                                    signature_id,
                                },
                        )?;
                        let expected_scope =
                            catalog.mechanism_evidence_contract(*request_id)?.scope();
                        *cached = Some(
                            derive_relational_structural_mechanism_v1(
                                definition,
                                expected_scope,
                                relational_structural_derivation_budget(),
                            )
                            .map_err(RelationalAnalysisJournalError::from)?,
                        );
                    }
                    let artifact = cached
                        .as_ref()
                        .expect("the structural producer cache was just initialized");
                    let structural_mechanism_id = artifact.mechanism().id();
                    let execution_profile_id = artifact.profile().id();
                    let event = analysis.next_structural_quotient_artifact_event(
                        artifact,
                        self.mechanism_artifact_chunk_bytes.get() as usize,
                    )?;
                    drop(cached);
                    return Ok(self.batch(
                        view,
                        RelationalMechanismStepQuantum::DeriveStructuralQuotient {
                            request_id: *request_id,
                            signature_id,
                            structural_mechanism_id,
                            execution_profile_id,
                        },
                        vec![RelationalJournalEvent::analysis(event)],
                    ));
                }

                self.structural_artifact_cache.borrow_mut().take();
                let closure_event = analysis.structural_quotient_closure_event(*request_id)?;
                let RelationalAnalysisEvidenceEvent::StructuralQuotientClosed {
                    structural_root,
                    ..
                } = closure_event
                else {
                    unreachable!("structural closure factory returns its closure event")
                };
                return Ok(self.batch(
                    view,
                    RelationalMechanismStepQuantum::CloseStructuralQuotient {
                        request_id: *request_id,
                        structural_root,
                    },
                    vec![RelationalJournalEvent::analysis(closure_event)],
                ));
            }

            match status {
                RelationalAnalysisLayerStatus::MechanismTargetOpen => {
                    if incidence.target_is_sealed() {
                        return Err(
                            RelationalMechanismStepDriverError::MechanismLayerStateMismatch(
                                *request_id,
                            )
                            .into(),
                        );
                    }
                    let target_cases = incidence.target_case_count();
                    let terminal_cases = incidence.terminal_case_count();
                    if terminal_cases > target_cases {
                        return Err(RelationalMechanismStepDriverError::TerminalCountMismatch {
                            request_id: *request_id,
                            target_cases,
                            terminal_cases,
                        }
                        .into());
                    }
                    // Keep the normal open frontier nearly caught up: replay
                    // an already admitted member before admitting another.
                    // If an externally restored prefix has a target gap in
                    // discovery order, repair that gap first instead of
                    // letting hash order hide it.
                    if terminal_cases < target_cases
                        && self
                            .next_unreplayed_selected_target(view, incidence, *request_id)
                            .is_some()
                    {
                        return self.step_selected_terminal(
                            view,
                            analysis,
                            catalog,
                            incidence,
                            *request_id,
                            layer,
                            runtime,
                        );
                    }
                    if let Some(outcome) = self
                        .step_selected_target(view, incidence, *request_id, question_seal)
                        .map_err(RelationalMechanismStepRunError::from)?
                    {
                        return Ok(outcome);
                    }
                    if terminal_cases < target_cases {
                        return Err(
                            RelationalMechanismStepDriverError::NonCanonicalTerminalPrefix {
                                request_id: *request_id,
                                target_cases,
                                terminal_cases,
                            }
                            .into(),
                        );
                    }
                    return Ok(RelationalMechanismStepOutcome::Quiescent(
                        RelationalMechanismStepQuiescence::AwaitingSelectedQuestion,
                    ));
                }
                RelationalAnalysisLayerStatus::MechanismTerminalOpen => {
                    if !incidence.target_is_sealed() || incidence.frontier_is_complete() {
                        return Err(
                            RelationalMechanismStepDriverError::MechanismLayerStateMismatch(
                                *request_id,
                            )
                            .into(),
                        );
                    }
                    return self.step_selected_terminal(
                        view,
                        analysis,
                        catalog,
                        incidence,
                        *request_id,
                        layer,
                        runtime,
                    );
                }
                RelationalAnalysisLayerStatus::MechanismClosed => {
                    if !incidence.frontier_is_complete() {
                        return Err(
                            RelationalMechanismStepDriverError::MechanismLayerStateMismatch(
                                *request_id,
                            )
                            .into(),
                        );
                    }
                    let closure = catalog.mechanism_closure_receipt(*request_id)?;
                    let incidence_root = closure.incidence_root();
                    return Ok(self.batch(
                        view,
                        RelationalMechanismStepQuantum::CloseIncidence {
                            request_id: *request_id,
                            incidence_root,
                        },
                        vec![RelationalJournalEvent::analysis(
                            RelationalAnalysisEvidenceEvent::mechanism_incidence_closed(closure),
                        )],
                    ));
                }
                RelationalAnalysisLayerStatus::ResultUnregistered
                | RelationalAnalysisLayerStatus::ResultInputOpen
                | RelationalAnalysisLayerStatus::ResultAwaitingPublication
                | RelationalAnalysisLayerStatus::ResultPublished => {
                    return Err(
                        RelationalMechanismStepDriverError::AnalysisLayerKindMismatch(layer_id)
                            .into(),
                    );
                }
            }
        }

        if let Some(request_id) = pending_request_id {
            return Err(
                RelationalMechanismStepDriverError::PendingArtifactRequestUnsupported {
                    request_id,
                }
                .into(),
            );
        }

        Ok(RelationalMechanismStepOutcome::Quiescent(
            match self.first_deferred_chosen {
                Some((request_id, view_id)) => {
                    RelationalMechanismStepQuiescence::DeferredChosenView {
                        request_id,
                        view_id,
                    }
                }
                None => RelationalMechanismStepQuiescence::SelectedMechanismsComplete,
            },
        ))
    }

    fn step_selected_target(
        &self,
        view: RelationalSchedulerView<'_>,
        incidence: &super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
        request_id: MechanismRequestId,
        question_seal: Option<RelationalSelectedQuestionSeal>,
    ) -> Result<Option<RelationalMechanismStepOutcome>, RelationalMechanismStepDriverError> {
        let durable_cases = incidence.target_case_count() as u128;
        let expected_cases = question_seal.map(|seal| seal.mechanism_target().count());
        if let Some(expected_cases) = expected_cases {
            if durable_cases > expected_cases {
                return Err(
                    RelationalMechanismStepDriverError::SelectedTargetCountMismatch {
                        request_id,
                        expected: expected_cases,
                        actual: durable_cases,
                    },
                );
            }
        }

        let cases = self.missing_selected_target_chunk(view, incidence, request_id);
        let pending_cases = cases.len() as u128;
        let projected_cases = durable_cases
            .checked_add(pending_cases)
            .ok_or(RelationalMechanismStepDriverError::SelectedTargetCountOverflow)?;
        if let Some(expected_cases) = expected_cases {
            if projected_cases > expected_cases {
                return Err(
                    RelationalMechanismStepDriverError::SelectedTargetCountMismatch {
                        request_id,
                        expected: expected_cases,
                        actual: projected_cases,
                    },
                );
            }
        }

        if cases.is_empty() {
            let Some(question_seal) = question_seal else {
                return Ok(None);
            };
            let expected_cases = question_seal.mechanism_target().count();
            if durable_cases != expected_cases {
                return Err(
                    RelationalMechanismStepDriverError::NonCanonicalTargetPrefix {
                        request_id,
                        expected: expected_cases,
                        actual: durable_cases,
                    },
                );
            }
            return Ok(Some(self.batch(
                view,
                RelationalMechanismStepQuantum::SealSelectedTarget {
                    request_id,
                    exact_case_count: expected_cases,
                },
                vec![RelationalJournalEvent::analysis(
                    RelationalAnalysisEvidenceEvent::mechanism_target_sealed_from_selected(
                        request_id,
                        question_seal,
                    ),
                )],
            )));
        }

        let first_case_id = cases[0];
        let case_count = NonZeroU16::new(
            u16::try_from(cases.len())
                .map_err(|_| RelationalMechanismStepDriverError::ChunkCaseCountOverflow)?,
        )
        .ok_or(RelationalMechanismStepDriverError::ChunkMadeNoProgress)?;
        let seals_target = false;
        let mut events = Vec::with_capacity(cases.len());
        for case_id in cases {
            if view.question_decision(case_id) != Some(SelectionDecision::Selected) {
                return Err(
                    RelationalMechanismStepDriverError::TargetCaseOutsideSelectedPopulation {
                        request_id,
                        case_id,
                    },
                );
            }
            events.push(RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_target_case_accepted(
                    request_id, case_id,
                ),
            ));
        }
        Ok(Some(self.batch(
            view,
            RelationalMechanismStepQuantum::AdmitSelectedTargetCases {
                request_id,
                first_case_id,
                case_count,
                seals_target,
            },
            events,
        )))
    }

    fn missing_selected_target_chunk(
        &self,
        view: RelationalSchedulerView<'_>,
        incidence: &super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
        request_id: MechanismRequestId,
    ) -> Vec<RelationalCaseId> {
        let selected_count = view.selected_count();
        let mut cursors = self.discovery_cursors.borrow_mut();
        let cursor = cursors.entry(request_id).or_default();
        if cursor.target_ordinal > selected_count
            || incidence.target_case_count() < cursor.target_ordinal
        {
            cursor.target_ordinal = 0;
        }

        let mut durable_prefix = 0usize;
        let mut missing = Vec::with_capacity(usize::from(self.max_target_cases_per_quantum.get()));
        for case_id in view
            .selected_discovery_suffix(cursor.target_ordinal)
            .iter()
            .copied()
        {
            if incidence.contains_target_case(case_id) {
                if missing.is_empty() {
                    durable_prefix += 1;
                }
            } else {
                missing.push(case_id);
                if missing.len() == usize::from(self.max_target_cases_per_quantum.get()) {
                    break;
                }
            }
        }
        // Advance only across evidence already present in the durable prefix.
        // Planned rows remain at the cursor until the outer coordinator has
        // appended them, so a rejected/stale batch can be retried verbatim.
        cursor.target_ordinal += durable_prefix;
        missing
    }

    fn step_selected_terminal<R: RelationalMechanismReplayRuntime>(
        &self,
        view: RelationalSchedulerView<'_>,
        analysis: &RelationalAnalysisJournalState,
        catalog: &RelationalAnalysisCatalogBuilder,
        incidence: &super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
        request_id: MechanismRequestId,
        layer: &SelectedMechanismLayer<'_>,
        runtime: &mut R,
    ) -> Result<RelationalMechanismStepOutcome, RelationalMechanismStepRunError<R::Error>> {
        let target_cases = incidence.target_case_count();
        let terminal_cases = incidence.terminal_case_count();
        if terminal_cases > target_cases {
            return Err(RelationalMechanismStepDriverError::TerminalCountMismatch {
                request_id,
                target_cases,
                terminal_cases,
            }
            .into());
        }

        let next_case = self.next_unreplayed_selected_target(view, incidence, request_id);
        let Some(case_id) = next_case else {
            return Err(if terminal_cases == target_cases {
                RelationalMechanismStepDriverError::MechanismLayerStateMismatch(request_id)
            } else {
                RelationalMechanismStepDriverError::NonCanonicalTerminalPrefix {
                    request_id,
                    target_cases,
                    terminal_cases,
                }
            }
            .into());
        };
        if incidence.terminal(case_id).is_some() {
            return Err(
                RelationalMechanismStepDriverError::NonCanonicalTerminalPrefix {
                    request_id,
                    target_cases,
                    terminal_cases,
                }
                .into(),
            );
        }
        if view.question_decision(case_id) != Some(SelectionDecision::Selected) {
            return Err(
                RelationalMechanismStepDriverError::TargetCaseOutsideSelectedPopulation {
                    request_id,
                    case_id,
                }
                .into(),
            );
        }
        let case =
            view.case(case_id)
                .ok_or(RelationalMechanismStepDriverError::UnknownTargetCase {
                    request_id,
                    case_id,
                })?;
        let contract = catalog.mechanism_evidence_contract(request_id)?;
        let outcome = replay_relational_mechanism_case(
            runtime,
            contract.scope(),
            layer.observation,
            self.transition_schemas,
            case,
        )
        .map_err(RelationalMechanismStepRunError::Replay)?;

        match outcome {
            RelationalMechanismReplayOutcome::Observed(evidence) => {
                if evidence.observation_id() != layer.replay_observation_id
                    || evidence.case_id() != case_id
                {
                    return Err(
                        RelationalMechanismStepDriverError::ReplayEvidenceScopeMismatch {
                            request_id,
                            case_id,
                        }
                        .into(),
                    );
                }
                let transition_id = evidence.transition_id();
                let signature_id = evidence.signature_id();
                let signature_is_interned =
                    incidence.signature_definition(signature_id) == Some(evidence.definition());
                let mut compact_events = Vec::new();
                if !signature_is_interned {
                    compact_events.extend(
                        RelationalAnalysisEvidenceEvent::
                            mechanism_signature_artifact_events_with_chunk_bytes(
                                evidence.definition(),
                                self.mechanism_artifact_chunk_bytes.get() as usize,
                            )?
                            .into_vec(),
                    );
                }
                compact_events.extend(
                    RelationalAnalysisEvidenceEvent::
                        mechanism_compact_incidence_artifact_events_with_chunk_bytes(
                            contract,
                            &evidence,
                            self.mechanism_artifact_chunk_bytes.get() as usize,
                        )?
                        .into_vec(),
                );
                let events = match analysis
                    .resume_mechanism_artifact_events(compact_events.into_boxed_slice())
                {
                    Ok(events) => events,
                    // A journal created before compact incidences may have
                    // stopped inside its self-contained full payload. Rebuild
                    // that exact artifact only for the resume comparison; all
                    // newly opened incidences use the factored protocol above.
                    Err(RelationalAnalysisJournalError::MechanismArtifactResumeMismatch)
                        if analysis.has_pending_mechanism_artifact() =>
                    {
                        let legacy_events = RelationalAnalysisEvidenceEvent::
                            mechanism_incidence_artifact_events_with_chunk_bytes(
                                contract,
                                &evidence,
                                self.mechanism_artifact_chunk_bytes.get() as usize,
                            )?;
                        analysis.resume_mechanism_artifact_events(legacy_events)?
                    }
                    Err(error) => return Err(error.into()),
                };
                Ok(self.batch(
                    view,
                    RelationalMechanismStepQuantum::ReplayObserved {
                        request_id,
                        case_id,
                        transition_id,
                        signature_id,
                    },
                    events
                        .into_vec()
                        .into_iter()
                        .map(RelationalJournalEvent::analysis)
                        .collect(),
                ))
            }
            RelationalMechanismReplayOutcome::PermanentlyUnavailable(evidence) => {
                if evidence.observation_id() != layer.replay_observation_id
                    || evidence.case_id() != case_id
                {
                    return Err(
                        RelationalMechanismStepDriverError::ReplayEvidenceScopeMismatch {
                            request_id,
                            case_id,
                        }
                        .into(),
                    );
                }
                let endpoint = evidence.endpoint();
                let reason_id = evidence.reason_id();
                let events = RelationalAnalysisEvidenceEvent::
                    mechanism_unavailable_artifact_events_with_chunk_bytes(
                        contract, &evidence,
                        self.mechanism_artifact_chunk_bytes.get() as usize,
                    )?;
                let events = analysis.resume_mechanism_artifact_events(events)?;
                Ok(self.batch(
                    view,
                    RelationalMechanismStepQuantum::ReplayPermanentlyUnavailable {
                        request_id,
                        case_id,
                        endpoint,
                        reason_id,
                    },
                    events
                        .into_vec()
                        .into_iter()
                        .map(RelationalJournalEvent::analysis)
                        .collect(),
                ))
            }
            RelationalMechanismReplayOutcome::Paused {
                case_id: paused_case_id,
                endpoint,
                reason,
            } => {
                if paused_case_id != case_id {
                    return Err(
                        RelationalMechanismStepDriverError::ReplayEvidenceScopeMismatch {
                            request_id,
                            case_id,
                        }
                        .into(),
                    );
                }
                Ok(RelationalMechanismStepOutcome::Quiescent(
                    RelationalMechanismStepQuiescence::ReplayPaused {
                        request_id,
                        case_id,
                        endpoint,
                        reason,
                    },
                ))
            }
        }
    }

    fn next_unreplayed_selected_target(
        &self,
        view: RelationalSchedulerView<'_>,
        incidence: &super::mechanism_incidence::MechanismIncidenceCatalogBuilder,
        request_id: MechanismRequestId,
    ) -> Option<RelationalCaseId> {
        let selected_count = view.selected_count();
        let mut cursors = self.discovery_cursors.borrow_mut();
        let cursor = cursors.entry(request_id).or_default();
        if cursor.terminal_ordinal > selected_count
            || incidence.terminal_case_count() < cursor.terminal_ordinal
        {
            cursor.terminal_ordinal = 0;
        }

        for case_id in view
            .selected_discovery_suffix(cursor.terminal_ordinal)
            .iter()
            .copied()
        {
            // Do not cross a selected member that has not durably entered the
            // target yet: it may become replayable after the pending target
            // batch is appended.
            if !incidence.contains_target_case(case_id) {
                return None;
            }
            if incidence.terminal(case_id).is_none() {
                return Some(case_id);
            }
            cursor.terminal_ordinal += 1;
        }
        None
    }

    fn validate_scope(
        &self,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalMechanismStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != self.relation_id
            || contract.admission_id() != self.admission_id
            || contract.question_id() != self.question_id
        {
            return Err(RelationalMechanismStepDriverError::JournalScopeMismatch);
        }
        match view.analysis_plan_root() {
            Some(actual) if actual == self.analysis_plan_root => Ok(()),
            Some(actual) => Err(
                RelationalMechanismStepDriverError::AnalysisPlanRootMismatch {
                    expected: self.analysis_plan_root,
                    actual,
                },
            ),
            None => Err(RelationalMechanismStepDriverError::AnalysisPlanMissing),
        }
    }

    fn batch(
        &self,
        view: RelationalSchedulerView<'_>,
        quantum: RelationalMechanismStepQuantum,
        events: Vec<RelationalJournalEvent>,
    ) -> RelationalMechanismStepOutcome {
        debug_assert!(!events.is_empty());
        RelationalMechanismStepOutcome::Emitted(RelationalMechanismStepBatch {
            expected_sequence: view.sequence(),
            expected_head: view.head(),
            quantum,
            events: events.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStepDriverError {
    AnalysisPlan(RelationalAnalysisPlanError),
    Catalog(RelationalAnalysisCatalogError),
    AnalysisJournal(RelationalAnalysisJournalError),
    Journal(RelationalJournalError),
    ReplayEvidence(RelationalMechanismReplayError),
    CheckedPlanScopeMismatch,
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
    DuplicateMechanismRequest(MechanismRequestId),
    SelectedQuestionScopeMismatch,
    MechanismLayerStateMismatch(MechanismRequestId),
    ClosedIncidenceStateMismatch(MechanismRequestId),
    SelectedTargetCountMismatch {
        request_id: MechanismRequestId,
        expected: u128,
        actual: u128,
    },
    NonCanonicalTargetPrefix {
        request_id: MechanismRequestId,
        expected: u128,
        actual: u128,
    },
    SelectedTargetCountOverflow,
    TargetCaseOutsideSelectedPopulation {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    UnknownTargetCase {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    TerminalCountMismatch {
        request_id: MechanismRequestId,
        target_cases: usize,
        terminal_cases: usize,
    },
    NonCanonicalTerminalPrefix {
        request_id: MechanismRequestId,
        target_cases: usize,
        terminal_cases: usize,
    },
    ReplayEvidenceScopeMismatch {
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    },
    MissingStructuralSignatureDefinition {
        request_id: MechanismRequestId,
        signature_id: MechanismSignatureId,
    },
    PendingArtifactRequestUnsupported {
        request_id: MechanismRequestId,
    },
    ArtifactChunkBytesTooLarge {
        actual: u32,
        limit: u32,
    },
    ChunkCaseCountOverflow,
    ChunkMadeNoProgress,
}

impl fmt::Display for RelationalMechanismStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisPlan(error) => {
                write!(formatter, "mechanism analysis plan failed: {error}")
            }
            Self::Catalog(error) => write!(formatter, "mechanism catalog failed: {error}"),
            Self::AnalysisJournal(error) => {
                write!(formatter, "mechanism analysis event failed: {error}")
            }
            Self::Journal(error) => write!(formatter, "mechanism journal step failed: {error}"),
            Self::ReplayEvidence(error) => {
                write!(formatter, "mechanism replay contract failed: {error}")
            }
            Self::CheckedPlanScopeMismatch => formatter
                .write_str("checked mechanism plan belongs to another relational question"),
            Self::JournalScopeMismatch => {
                formatter.write_str("mechanism driver and relational journal scopes differ")
            }
            Self::AnalysisPlanMissing => {
                formatter.write_str("mechanism execution requires a registered analysis plan")
            }
            Self::AnalysisPlanRootMismatch { .. } => {
                formatter.write_str("mechanism driver and journal analysis plan roots differ")
            }
            Self::AnalysisStateMissing => {
                formatter.write_str("registered analysis plan has no analysis journal state")
            }
            Self::AnalysisCatalogMissing => {
                formatter.write_str("open mechanism execution has no analysis catalog")
            }
            Self::AnalysisLayerMissing(_) => formatter
                .write_str("checked mechanism layer is absent from the analysis catalog"),
            Self::AnalysisLayerKindMismatch(_) => formatter
                .write_str("checked mechanism layer has a different analysis kind or target"),
            Self::DuplicateMechanismRequest(_) => {
                formatter.write_str("checked query repeats a semantic mechanism request")
            }
            Self::SelectedQuestionScopeMismatch => formatter
                .write_str("selected-question seal belongs to another FIND question"),
            Self::MechanismLayerStateMismatch(_) => formatter
                .write_str("mechanism layer status and incidence frontier disagree"),
            Self::ClosedIncidenceStateMismatch(_) => formatter
                .write_str("durable mechanism closure disagrees with the incidence frontier"),
            Self::SelectedTargetCountMismatch {
                expected, actual, ..
            } => write!(
                formatter,
                "selected mechanism target has {actual} cases; exact seal requires {expected}"
            ),
            Self::NonCanonicalTargetPrefix {
                expected, actual, ..
            } => write!(
                formatter,
                "mechanism target prefix ended at {actual} of {expected} cases; ordered resume cannot repair an earlier gap"
            ),
            Self::SelectedTargetCountOverflow => {
                formatter.write_str("selected mechanism target count overflowed u128")
            }
            Self::TargetCaseOutsideSelectedPopulation { .. } => formatter
                .write_str("mechanism target contains a case outside the selected population"),
            Self::UnknownTargetCase { .. } => {
                formatter.write_str("mechanism target names no durable relational case")
            }
            Self::TerminalCountMismatch {
                target_cases,
                terminal_cases,
                ..
            } => write!(
                formatter,
                "mechanism frontier has {terminal_cases} terminals for {target_cases} target cases"
            ),
            Self::NonCanonicalTerminalPrefix {
                target_cases,
                terminal_cases,
                ..
            } => write!(
                formatter,
                "mechanism terminal prefix ended at {terminal_cases} of {target_cases} cases; ordered resume cannot repair an earlier gap"
            ),
            Self::ReplayEvidenceScopeMismatch { .. } => formatter.write_str(
                "mechanism runtime evidence belongs to another request, observation, or case",
            ),
            Self::MissingStructuralSignatureDefinition { .. } => formatter.write_str(
                "closed mechanism incidence omitted a raw signature selected for structural quotienting",
            ),
            Self::PendingArtifactRequestUnsupported { .. } => formatter.write_str(
                "durable open mechanism artifact belongs to a request this driver cannot resume",
            ),
            Self::ArtifactChunkBytesTooLarge { actual, limit } => write!(
                formatter,
                "mechanism artifact chunk size {actual} exceeds protocol limit {limit}"
            ),
            Self::ChunkCaseCountOverflow => formatter
                .write_str("mechanism target chunk exceeded its u16 operational bound"),
            Self::ChunkMadeNoProgress => {
                formatter.write_str("nonempty mechanism target chunk reported zero cases")
            }
        }
    }
}

impl Error for RelationalMechanismStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisPlan(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::AnalysisJournal(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::ReplayEvidence(error) => Some(error),
            Self::CheckedPlanScopeMismatch
            | Self::JournalScopeMismatch
            | Self::AnalysisPlanMissing
            | Self::AnalysisPlanRootMismatch { .. }
            | Self::AnalysisStateMissing
            | Self::AnalysisCatalogMissing
            | Self::AnalysisLayerMissing(_)
            | Self::AnalysisLayerKindMismatch(_)
            | Self::DuplicateMechanismRequest(_)
            | Self::SelectedQuestionScopeMismatch
            | Self::MechanismLayerStateMismatch(_)
            | Self::ClosedIncidenceStateMismatch(_)
            | Self::SelectedTargetCountMismatch { .. }
            | Self::NonCanonicalTargetPrefix { .. }
            | Self::SelectedTargetCountOverflow
            | Self::TargetCaseOutsideSelectedPopulation { .. }
            | Self::UnknownTargetCase { .. }
            | Self::TerminalCountMismatch { .. }
            | Self::NonCanonicalTerminalPrefix { .. }
            | Self::ReplayEvidenceScopeMismatch { .. }
            | Self::MissingStructuralSignatureDefinition { .. }
            | Self::PendingArtifactRequestUnsupported { .. }
            | Self::ArtifactChunkBytesTooLarge { .. }
            | Self::ChunkCaseCountOverflow
            | Self::ChunkMadeNoProgress => None,
        }
    }
}

impl From<RelationalAnalysisPlanError> for RelationalMechanismStepDriverError {
    fn from(error: RelationalAnalysisPlanError) -> Self {
        Self::AnalysisPlan(error)
    }
}

impl From<RelationalAnalysisCatalogError> for RelationalMechanismStepDriverError {
    fn from(error: RelationalAnalysisCatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<RelationalAnalysisJournalError> for RelationalMechanismStepDriverError {
    fn from(error: RelationalAnalysisJournalError) -> Self {
        Self::AnalysisJournal(error)
    }
}

impl From<RelationalJournalError> for RelationalMechanismStepDriverError {
    fn from(error: RelationalJournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<RelationalMechanismReplayError> for RelationalMechanismStepDriverError {
    fn from(error: RelationalMechanismReplayError) -> Self {
        Self::ReplayEvidence(error)
    }
}

/// Driver/proof failures stay distinct from runtime failures; the latter are
/// never converted into permanent-unavailability evidence by this layer.
#[derive(Debug)]
pub(crate) enum RelationalMechanismStepRunError<E> {
    Driver(RelationalMechanismStepDriverError),
    Replay(RelationalMechanismReplayRunError<E>),
}

impl<E> From<RelationalMechanismStepDriverError> for RelationalMechanismStepRunError<E> {
    fn from(error: RelationalMechanismStepDriverError) -> Self {
        Self::Driver(error)
    }
}

impl<E> From<RelationalAnalysisCatalogError> for RelationalMechanismStepRunError<E> {
    fn from(error: RelationalAnalysisCatalogError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E> From<RelationalAnalysisJournalError> for RelationalMechanismStepRunError<E> {
    fn from(error: RelationalAnalysisJournalError) -> Self {
        Self::Driver(error.into())
    }
}

impl<E: fmt::Display> fmt::Display for RelationalMechanismStepRunError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => fmt::Display::fmt(error, formatter),
            Self::Replay(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
