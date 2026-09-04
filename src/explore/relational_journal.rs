//! Authenticated append-only journal state machine for relational Explore.
//!
//! This module defines the event validation, hash chain and replay semantics
//! which a durable framed store must preserve. It is not by itself durable:
//! the storage adapter must install each encoded entry before publication.
//! Its chain commits semantic evidence events, while invocation limits and
//! scheduler order remain outside the contract. Replaying a valid chain
//! rebuilds the same stable relation, admission, named FIND set, and semantic work
//! frontiers. Runtime interleaving remains outside the semantic contract, but
//! accepted automatic-observation progress is bound into the checkpoint root.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::{Arc, Weak};

use sha2::{Digest, Sha256};

use super::mechanism_support::{
    MechanismAutomaticObservationSchedulerSummary,
    MechanismExplicitObservationRegistrationDisposition,
    MechanismExplicitObservationRegistrationPhase, MechanismExplicitObservationSchedulerSummary,
    MechanismFactorizedSupportObservationSummary, MechanismFactorizedSupportObservationSummaryRoot,
    MechanismSupportCheckpointCursor, MechanismSupportClosureRoot, MechanismSupportError,
    MechanismSupportFrontierRoot, MechanismSupportFrontierSummary, MechanismSupportSlice,
    MechanismSupportSubject,
};
use super::relation::{
    install_selected_case_batch, AdmissionCatalog, AdmissionCatalogBuilder, AdmissionContentRoot,
    AdmissionCounts, AdmissionDecision, AdmissionFrontierRoot, AdmissionId, MechanismRequestId,
    QuestionCatalog, QuestionCatalogBuilder, QuestionContentRoot, QuestionFrontierRoot, QuestionId,
    RelationCatalog, RelationCatalogBuilder, RelationCatalogError, RelationCatalogSnapshot,
    RelationClassificationError, RelationContentRoot, RelationCountEvidence, RelationFrontierRoot,
    RelationId, RelationProvenance, RelationalCaseId, RelationalCaseRef, SelectedCaseBatchError,
    SelectedCaseBatchRow, SelectionCounts, SelectionDecision, SourceKey, SourceRow, SuccessorKey,
    SuccessorRow, ViewInputId,
};
use super::relational_analysis_catalog::{
    RelationalAnalysisCatalogRoot, RelationalAnalysisCatalogSnapshot,
};
use super::relational_analysis_journal::{
    RelationalAnalysisClosureSetRoot, RelationalAnalysisEvidenceEvent,
    RelationalAnalysisJournalError, RelationalAnalysisJournalScopeRoot,
    RelationalAnalysisJournalState, RelationalSelectedPopulationAuthority,
    RelationalSelectedQuestionSeal,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerRegistration, RelationalAnalysisPlan, RelationalAnalysisPlanRoot,
    RelationalResolvedMechanismTarget, RelationalResolvedResultInput,
};
use super::relational_bounded_chunk_partition::{
    reverify_relational_case_chunk_partition_artifact, RelationalCaseChunkId,
    RelationalCaseChunkPartitionArtifact, RelationalCaseChunkPartitionArtifactId,
    RelationalCaseChunkPartitionError, RelationalCaseChunkShape,
    VerifiedRelationalCaseChunkPartition,
};
use super::relational_candidate_schedule::RelationalCandidateNominationRoot;
use super::relational_case_executor::{
    SuccessorFiberExhaustionReceipt, SuccessorFiberExhaustionReceiptId,
};
use super::relational_certified_source_summary::{
    reverify_relational_certified_source_summary_artifact, RelationalCertifiedSourceSummaryError,
};
use super::relational_classified_population::{
    CertifiedRelationalClassificationCountsError, RelationalClassificationProgressCounts,
};
use super::relational_classified_sweep::{
    finalize_relational_classified_case_chunk, reverify_relational_classified_chunk_artifact,
    reverify_relational_classified_chunk_slice_artifact, RelationalClassifiedCaseOutcome,
    RelationalClassifiedChunkAccumulator, RelationalClassifiedChunkArtifact,
    RelationalClassifiedChunkArtifactId, RelationalClassifiedChunkSliceArtifact,
    RelationalClassifiedSweepError, VerifiedRelationalClassifiedChunk,
};
use super::relational_executor::{RelationalSourceAdvance, RelationalSourceContinuation};
use super::relational_frontier::{
    RelationalWorkFrontier, WorkCompletionRef, WorkFrontierCompaction, WorkFrontierError,
    WorkFrontierRoot, WorkFrontierSnapshot, WorkNodeId, WorkNodeSnapshot, WorkNodeSpec,
};
use super::relational_population::{
    CertifiedSelectedPopulationError, ClosedCertifiedSelectedPopulation,
};
use super::relational_region_proof::{
    RelationalCertifiedRegionConclusion, RelationalRegionProofArtifact, RelationalRegionProofError,
    RelationalRegionProofSubject, RelationalRegionReplayAuthority,
};
use super::relational_selected_run_materialization::{
    reverify_relational_selected_run_materialization_artifact,
    RelationalSelectedRunMaterializationArtifact, RelationalSelectedRunMaterializationArtifactId,
    RelationalSelectedRunMaterializationError, VerifiedRelationalSelectedRunMaterialization,
};
use super::relational_source_closure::{
    SourceRelationExhaustionReceipt, SourceRelationExhaustionReceiptId, SourceTraversalAccumulator,
    SourceTraversalAdvanceId, SourceTraversalClosureError, SourceTraversalFrontierRoot,
    SourceTraversalObservation,
};
use super::relational_source_image_exactness::{
    reverify_relational_source_image_exactness_artifact, CertifiedSourcePopulationBinding,
    RelationalSourceImageExactnessProofArtifact, RelationalSourceImageExactnessProofError,
    RelationalSourceImageExactnessProofShape,
};
use super::relational_support_planner::{
    reverify_relational_case_image_injectivity_artifact, RelationalCaseImageAssignmentKind,
    RelationalCaseImageInjectivityProofArtifact, RelationalCaseImageInjectivityProofError,
    RelationalCaseImagePreimageKind, RelationalObligationActivation, RelationalRootObligationPlan,
    RelationalSourceAssignmentImageProof, RelationalStagedObligationDescriptor,
    RelationalSuccessorRecipeKind, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::relational_transition_support::{
    RelationalTransitionSupportCounts, RelationalTransitionSupportError,
    RelationalTransitionSupportIndex, RelationalTransitionSupportRoot,
};
use super::relational_uniform_admission_proof::{
    reverify_relational_uniform_admission_artifact, RelationalUniformAdmissionProofArtifact,
    RelationalUniformAdmissionProofError,
};
use super::result_evidence::RelationalResultInputSeal;
use super::support_cell::{
    relational_case_chunk_partition_gateway, relational_case_image_proof_gateway,
    relational_classified_sweep_gateway, relational_region_proof_gateway,
    relational_uniform_admission_proof_gateway, AdmissionClassificationClaim,
    ExactCardinalityClaim, InjectiveMappingClaim, SelectionClassificationClaim,
    SupportCellEvidenceId, SupportCellId, SupportCellObligation, SupportMaterializationCursor,
    SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceCatalogBuilder, SupportEvidenceError, SupportEvidenceKind,
    SupportEvidenceRecord, SupportEvidenceRoot, SupportEvidenceSnapshot, SupportObligationRecord,
    SupportObligationRefinement, ValidatedSupportEvidenceClosure,
};
use super::support_journal::{SupportJournalError, SupportJournalEvent};
use super::transition::canonical_explore_value_digest;
use super::transition::{ContextSchemaId, StateSchemaId, TransitionTypeId};
use super::ExploreValue;

pub(crate) const RELATIONAL_JOURNAL_SCHEMA_VERSION: u32 = 28;
/// V3 adds distinct authenticated SourceEvent and LiftedCandidate decisions.
/// The ordering and reason are observable journal policy, even though neither
/// participates in arrival-order-independent semantic evidence roots.
pub(crate) const RELATIONAL_SCHEDULER_POLICY_VERSION: u32 = 3;

const JOURNAL_CONTRACT_HASH_V28: &[u8] = b"futuruna.explore.relational-journal-contract.v28";
const JOURNAL_GENESIS_HASH_V28: &[u8] = b"futuruna.explore.relational-journal-genesis.v28";
const JOURNAL_EVENT_HASH_V24: &[u8] = b"futuruna.explore.relational-journal-event.v24";
const JOURNAL_ENTRY_HASH_V28: &[u8] = b"futuruna.explore.relational-journal-entry.v28";
const SCHEDULER_WORK_FINGERPRINT_V2: &[u8] =
    b"futuruna.explore.relational-scheduler-work-fingerprint.v2";
const CORE_EVIDENCE_ROOT_HASH_V6: &[u8] = b"futuruna.explore.relational-core-evidence-root.v6";
const EXPLORATION_EVIDENCE_ROOT_HASH_V2: &[u8] =
    b"futuruna.explore.relational-exploration-evidence-root.v2";
const EXHAUSTION_EVIDENCE_ROOT_HASH_V2: &[u8] =
    b"futuruna.explore.relational-exhaustion-evidence-root.v2";
const EXTENSIONAL_CONTENT_ROOT_HASH_V5: &[u8] =
    b"futuruna.explore.relational-extensional-content-root.v5";
const CHECKPOINT_ROOT_HASH_V9: &[u8] = b"futuruna.explore.relational-checkpoint-root.v9";
const QUESTION_FRONTIER_SET_ROOT_HASH_V1: &[u8] = b"futuruna.explore.question-frontier-set-root.v1";
const QUESTION_CONTENT_SET_ROOT_HASH_V1: &[u8] = b"futuruna.explore.question-content-set-root.v1";
const MECHANISM_SUPPORT_OBSERVATION_POINT_ID_V2: &[u8] =
    b"futuruna.explore.mechanism-support-observation-point-id.v2";
const MECHANISM_SUPPORT_OBSERVATION_CHAIN_GENESIS_V2: &[u8] =
    b"futuruna.explore.mechanism-support-observation-chain-genesis.v2";
const MECHANISM_SUPPORT_OBSERVATION_CHAIN_STEP_V2: &[u8] =
    b"futuruna.explore.mechanism-support-observation-chain-step.v2";
const MECHANISM_SUPPORT_OBSERVATION_DEMAND_CHAIN_GENESIS_V1: &[u8] =
    b"futuruna.explore.mechanism-support-observation-demand-chain-genesis.v1";
const MECHANISM_SUPPORT_OBSERVATION_DEMAND_CHAIN_STEP_V1: &[u8] =
    b"futuruna.explore.mechanism-support-observation-demand-chain-step.v1";
const MECHANISM_SUPPORT_OBSERVATION_DEMAND_CLAIM_HASH_V1: &[u8] =
    b"futuruna.explore.mechanism-support-observation-demand-claim.v1";

pub(crate) const MECHANISM_SUPPORT_OBSERVATION_POINT_VERSION: u32 = 2;
pub(crate) const MECHANISM_SUPPORT_OBSERVATION_DEMAND_REGISTRATION_VERSION: u32 = 1;
pub(crate) const MECHANISM_SUPPORT_OBSERVATION_BACKFILL_VERSION: u32 = 1;

/// Semantic replay bound for every independently authenticated mechanism-
/// support checkpoint lane. Runtime limits may choose a smaller quantum, but
/// no journal producer or crafted frame may make replay cross a larger delta.
const RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA: u128 = 256;

type SharedConstructorFields = Arc<[(String, ExploreValue)]>;
type WeakConstructorFields = Weak<[(String, ExploreValue)]>;

/// Process-local hash-consing for constructor payloads retained by the
/// journal fold. Digest buckets are only an index: every hit compares the
/// complete constructor metadata and field content before sharing storage.
/// Weak entries ensure an operational cache never pins values after semantic
/// evidence releases them.
#[derive(Clone, Debug, Default)]
struct RelationalConstructorInterner {
    buckets: BTreeMap<[u8; 32], Vec<RelationalConstructorInternEntry>>,
}

#[derive(Clone, Debug)]
struct RelationalConstructorInternEntry {
    type_name: Box<str>,
    variant: Box<str>,
    positional: bool,
    fields: WeakConstructorFields,
}

#[derive(Debug, Default)]
struct PendingConstructorInterns {
    buckets: BTreeMap<[u8; 32], Vec<PendingConstructorInternEntry>>,
}

#[derive(Debug)]
struct PendingConstructorInternEntry {
    type_name: Box<str>,
    variant: Box<str>,
    positional: bool,
    fields: SharedConstructorFields,
}

impl RelationalConstructorInterner {
    fn prepare_event(
        &self,
        relation_id: RelationId,
        event: &mut RelationalJournalEvent,
    ) -> PendingConstructorInterns {
        let mut pending = PendingConstructorInterns::default();
        let mut visitor = |value: &mut ExploreValue| {
            self.canonicalize_value(value, &mut pending);
        };
        match event {
            RelationalJournalEvent::Evidence(
                RelationalEvidenceEvent::SourceTraversalObserved { advance, .. },
            ) => advance.canonicalize_value_storage(relation_id, &mut visitor),
            RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                RelationalAnalysisEvidenceEvent::ResultEvidenceAccepted { record, .. },
            )) => record.canonicalize_value_storage(&mut visitor),
            RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                RelationalAnalysisEvidenceEvent::ResultProjectionRecordAccepted { record, .. },
            )) => record.canonicalize_value_storage(&mut visitor),
            _ => {}
        }
        pending
    }

    fn canonicalize_value(
        &self,
        value: &mut ExploreValue,
        pending: &mut PendingConstructorInterns,
    ) {
        match value {
            ExploreValue::List(values)
            | ExploreValue::Set(values)
            | ExploreValue::Tuple(values) => {
                for value in values {
                    self.canonicalize_value(value, pending);
                }
                return;
            }
            ExploreValue::Constructor { .. } => {}
            ExploreValue::Int(_)
            | ExploreValue::FloatBits(_)
            | ExploreValue::String(_)
            | ExploreValue::Character(_)
            | ExploreValue::Boolean(_)
            | ExploreValue::Unit => return,
        }

        let digest = canonical_explore_value_digest(value);
        let ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } = value
        else {
            unreachable!("constructor matching was established above")
        };

        if let Some(existing) = self.buckets.get(&digest).and_then(|bucket| {
            bucket.iter().find_map(|entry| {
                if entry.type_name.as_ref() != type_name
                    || entry.variant.as_ref() != variant
                    || entry.positional != *positional
                {
                    return None;
                }
                entry
                    .fields
                    .upgrade()
                    .filter(|candidate| candidate.as_ref() == fields.as_ref())
            })
        }) {
            *fields = existing;
            return;
        }

        if let Some(existing) = pending.buckets.get(&digest).and_then(|bucket| {
            bucket.iter().find(|entry| {
                entry.type_name.as_ref() == type_name
                    && entry.variant.as_ref() == variant
                    && entry.positional == *positional
                    && entry.fields.as_ref() == fields.as_ref()
            })
        }) {
            *fields = existing.fields.clone();
            return;
        }
        // Only descend after both outer fast paths miss. A hot fixed Context
        // can therefore replace its complete field backing without first
        // copy-on-writing that backing merely to normalize children which the
        // canonical outer value already owns.
        for (_, value) in Arc::make_mut(fields).iter_mut() {
            self.canonicalize_value(value, pending);
        }
        pending
            .buckets
            .entry(digest)
            .or_default()
            .push(PendingConstructorInternEntry {
                type_name: type_name.clone().into_boxed_str(),
                variant: variant.clone().into_boxed_str(),
                positional: *positional,
                fields: fields.clone(),
            });
    }

    fn commit(&mut self, pending: PendingConstructorInterns) {
        for (digest, entries) in pending.buckets {
            for entry in entries {
                let bucket = self.buckets.entry(digest).or_default();
                bucket.retain(|existing| existing.fields.strong_count() != 0);
                let already_present = bucket.iter().any(|existing| {
                    existing.type_name == entry.type_name
                        && existing.variant == entry.variant
                        && existing.positional == entry.positional
                        && existing
                            .fields
                            .upgrade()
                            .is_some_and(|fields| fields.as_ref() == entry.fields.as_ref())
                });
                if !already_present {
                    bucket.push(RelationalConstructorInternEntry {
                        type_name: entry.type_name,
                        variant: entry.variant,
                        positional: entry.positional,
                        fields: Arc::downgrade(&entry.fields),
                    });
                }
            }
        }
    }
}

/// Arrival-order-independent semantic commitment to the relation,
/// classification and exact-support core of one exploration prefix.
///
/// Journal order, work progress, cursors and retained examples are excluded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCoreEvidenceRoot([u8; 32]);

impl RelationalCoreEvidenceRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Whole-exploration evidence commitment over the independently meaningful
/// base core and post-FIND analysis DAG. The optional terminal catalog and
/// closure-set roots prevent a complete-looking open catalog from being
/// confused with an accepted `AnalysisClosed` event or with a different set
/// of request-local raw, structural, and support closures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalExplorationEvidenceRoot([u8; 32]);

impl RelationalExplorationEvidenceRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed extensional identity of the concrete relation plus admission/FIND
/// content and the support proof catalog. It is deliberately a different type
/// from the prefix-oriented [`RelationalCoreEvidenceRoot`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalExtensionalContentRoot([u8; 32]);

impl RelationalExtensionalContentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent commitment to the concrete source traversal
/// frontier, its optional aggregate closure receipt, and producer-issued
/// successor exhaustion receipts. Operational scheduler cursors never enter
/// this root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalExhaustionEvidenceRoot([u8; 32]);

impl RelationalExhaustionEvidenceRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent resumability state. This is not answer identity
/// and is distinct from the ordered journal head.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCheckpointRoot([u8; 32]);

impl RelationalCheckpointRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical commitment to every registered QuestionId and its current FIND
/// frontier. This is distinct from each question-local frontier root and
/// commits the empty question set as a valid exploration state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalQuestionFrontierSetRoot([u8; 32]);

impl RelationalQuestionFrontierSetRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical commitment to every closed QuestionId -> FIND content root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalQuestionContentSetRoot([u8; 32]);

impl RelationalQuestionContentSetRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Semantic configuration sealed by one journal chain.
///
/// The analysis graph digest commits the resolved result/mechanism DAG. Its
/// producer identity is independent of authored order when dependency edges
/// and semantic layer identities are unchanged. CPU,
/// RAM, workers, deadlines, checkpoint paths, and declaration names are
/// intentionally absent so a run may resume under different safe resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalContract {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_ids: Arc<[QuestionId]>,
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    analysis_graph_digest: [u8; 32],
}

impl RelationalJournalContract {
    pub(crate) fn new(
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_ids: impl IntoIterator<Item = QuestionId>,
        state_schema_id: StateSchemaId,
        context_schema_id: ContextSchemaId,
        transition_type_id: TransitionTypeId,
        analysis_graph_digest: [u8; 32],
    ) -> Self {
        let question_ids: Arc<[QuestionId]> = question_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .into();
        Self {
            relation_id,
            admission_id,
            question_ids,
            state_schema_id,
            context_schema_id,
            transition_type_id,
            analysis_graph_digest,
        }
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        &self.question_ids
    }

    pub(crate) fn contains_question(&self, question_id: QuestionId) -> bool {
        self.question_ids.binary_search(&question_id).is_ok()
    }

    pub(crate) const fn state_schema_id(&self) -> StateSchemaId {
        self.state_schema_id
    }

    pub(crate) const fn context_schema_id(&self) -> ContextSchemaId {
        self.context_schema_id
    }

    pub(crate) const fn transition_type_id(&self) -> TransitionTypeId {
        self.transition_type_id
    }

    pub(crate) const fn analysis_graph_digest(&self) -> [u8; 32] {
        self.analysis_graph_digest
    }

    pub(crate) fn id(&self) -> RelationalJournalId {
        let mut hasher = ChainHasher::new(JOURNAL_CONTRACT_HASH_V28);
        hasher.u32(RELATIONAL_JOURNAL_SCHEMA_VERSION);
        hasher.digest(self.relation_id.bytes());
        hasher.digest(self.admission_id.bytes());
        hasher.u64(self.question_ids.len() as u64);
        for question_id in self.question_ids.iter() {
            hasher.digest(question_id.bytes());
        }
        hasher.digest(self.state_schema_id.bytes());
        hasher.digest(self.context_schema_id.bytes());
        hasher.digest(self.transition_type_id.bytes());
        hasher.digest(self.analysis_graph_digest);
        RelationalJournalId(hasher.finish())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalJournalId([u8; 32]);

impl RelationalJournalId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalJournalHead([u8; 32]);

impl RelationalJournalHead {
    fn genesis(contract_id: RelationalJournalId) -> Self {
        let mut hasher = ChainHasher::new(JOURNAL_GENESIS_HASH_V28);
        hasher.digest(contract_id.bytes());
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One answer-defining mutation of the open relational evidence state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalEvidenceEvent {
    /// Register the complete checked post-FIND dependency DAG. This prevents
    /// an execution from presenting a closed base core while silently
    /// forgetting result or mechanism layers requested by the query.
    AnalysisPlanRegistered {
        plan_root: RelationalAnalysisPlanRoot,
        plan: Box<RelationalAnalysisPlan>,
    },
    /// Install the complete logical support plan as the first semantic bridge
    /// from the checked query into proof work. The plan root is independently
    /// rederived before any catalog state changes.
    SupportPlanRegistered {
        plan_root: RelationalSupportPlanRoot,
        plan: Box<RelationalSupportPlan>,
    },
    /// Canonical producer-chain artifact. Replay treats its structural
    /// identity as data only, re-verifies it against the installed support
    /// plan, and privately remints root injectivity plus any exact-cardinality
    /// evidence established by the same proof.
    RelationalCaseImageInjectivityProofAccepted {
        artifact: Box<RelationalCaseImageInjectivityProofArtifact>,
    },
    /// Canonical assignment-to-source-image exactness artifact. Replay treats
    /// its identity as data, re-verifies the recognized producer chain against
    /// the installed support plan, then atomically installs both typed source
    /// evidence values as auxiliary obligations without declaring another
    /// support root cell.
    RelationalSourceImageExactnessProofAccepted {
        artifact: Box<RelationalSourceImageExactnessProofArtifact>,
    },
    /// Canonical bounded partition retained after its named root injectivity
    /// evidence is durable. Replay independently rebuilds the partition,
    /// restricts injectivity to every exact child, and atomically replaces the
    /// open root admission obligation with same-claim child obligations.
    RelationalCaseChunkPartitionAccepted {
        artifact: Box<RelationalCaseChunkPartitionArtifact>,
    },
    /// One exhaustively classified bounded chunk. Replay recovers the exact
    /// accepted chunk partition and child injectivity evidence, reconstructs
    /// homogeneous run cells and their typed conclusions, and commits every
    /// support consequence plus the derived root cursor advance atomically.
    RelationalClassifiedChunkAccepted {
        artifact: Box<RelationalClassifiedChunkArtifact>,
    },
    /// One producer-proved canonical child with zero selected cases. The
    /// serialized artifact is evidence, never authority: replay must possess
    /// the matching checked capsule authority and reproduce the theorem before
    /// any support fact or classified cursor advances.
    RelationalRegionProofAccepted {
        artifact: Box<RelationalRegionProofArtifact>,
    },
    /// Sparse concrete realization of one already accepted admitted+selected
    /// support run. Replay re-verifies the exact plan/chunk/run scope, then
    /// atomically installs only content-derived concrete relation and
    /// admission/FIND evidence. Source and successor enumeration stay open.
    RelationalSelectedRunMaterializationAccepted {
        artifact: Box<RelationalSelectedRunMaterializationArtifact>,
    },
    /// Canonical plan-bound admission artifact. Replay validates identity,
    /// re-runs the complete recognized recipe, and privately remints the typed
    /// root-admission evidence.
    RelationalUniformAdmissionProofAccepted {
        artifact: Box<RelationalUniformAdmissionProofArtifact>,
    },
    /// One replayable edge or fiber-exhaustion observation from the checked
    /// dependent source enumerator. The claimed ID commits the entire advance,
    /// including prefix support and terminal row provenance.
    SourceTraversalObserved {
        advance_id: SourceTraversalAdvanceId,
        advance: Box<RelationalSourceAdvance>,
    },
    /// Seal source discovery only with the aggregate receipt privately minted
    /// after the complete dependent-product traversal tree was verified.
    SourceEnumerationSealed {
        receipt_id: SourceRelationExhaustionReceiptId,
        receipt: Box<SourceRelationExhaustionReceipt>,
    },
    SuccessorDiscovered {
        source_key: SourceKey,
        successor_key: SuccessorKey,
        case_id: RelationalCaseId,
        row: SuccessorRow,
    },
    SuccessorFiberExhaustionAccepted {
        receipt_id: SuccessorFiberExhaustionReceiptId,
        receipt: SuccessorFiberExhaustionReceipt,
    },
    SuccessorEnumerationSealed {
        source_key: SourceKey,
        receipt_id: SuccessorFiberExhaustionReceiptId,
    },
    AdmissionClassified {
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    },
    QuestionClassified {
        question_id: QuestionId,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    },
    Support(SupportJournalEvent),
    /// One post-FIND result/mechanism DAG mutation. Its subordinate digest is
    /// embedded in this journal's single ordered semantic chain; there is no
    /// second independently advancing analysis log.
    Analysis(RelationalAnalysisEvidenceEvent),
}

/// Stable identity of one replay-derived support observation at an exact
/// durable checkpoint. The identity changes when either the support summary,
/// lifecycle status, or linear predecessor changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportObservationPointId([u8; 32]);

impl MechanismSupportObservationPointId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Request-local append-only commitment to every accepted observation point
/// in journal order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismSupportObservationChainRoot([u8; 32]);

impl MechanismSupportObservationChainRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanismSupportObservationDemandChainRoot([u8; 32]);

impl MechanismSupportObservationDemandChainRoot {
    const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MechanismSupportObservationStatus {
    Open,
    Sealed {
        support_root: MechanismSupportClosureRoot,
    },
}

impl MechanismSupportObservationStatus {
    pub(crate) const fn support_root(self) -> Option<MechanismSupportClosureRoot> {
        match self {
            Self::Open => None,
            Self::Sealed { support_root } => Some(support_root),
        }
    }

    pub(crate) const fn is_sealed(self) -> bool {
        matches!(self, Self::Sealed { .. })
    }
}

/// Fixed-size journal claim. The compact summary payload is deliberately not
/// serialized: replay re-derives it from the exact imported support prefix and
/// rejects a mismatching root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportObservationClaim {
    version: u32,
    point_id: MechanismSupportObservationPointId,
    slice: MechanismSupportSlice,
    cursor: MechanismSupportCheckpointCursor,
    frontier_root: MechanismSupportFrontierRoot,
    summary_root: MechanismFactorizedSupportObservationSummaryRoot,
    status: MechanismSupportObservationStatus,
    supersedes: Option<MechanismSupportObservationPointId>,
}

impl MechanismSupportObservationClaim {
    pub(crate) fn new(
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        summary_root: MechanismFactorizedSupportObservationSummaryRoot,
        status: MechanismSupportObservationStatus,
        supersedes: Option<MechanismSupportObservationPointId>,
    ) -> Self {
        let version = MECHANISM_SUPPORT_OBSERVATION_POINT_VERSION;
        let point_id = derive_mechanism_support_observation_point_id(
            version,
            slice,
            cursor,
            frontier_root,
            summary_root,
            status,
            supersedes,
        );
        Self {
            version,
            point_id,
            slice,
            cursor,
            frontier_root,
            summary_root,
            status,
            supersedes,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn restore_from_journal_codec(
        version: u32,
        point_id: MechanismSupportObservationPointId,
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        summary_root: MechanismFactorizedSupportObservationSummaryRoot,
        status: MechanismSupportObservationStatus,
        supersedes: Option<MechanismSupportObservationPointId>,
    ) -> Self {
        Self {
            version,
            point_id,
            slice,
            cursor,
            frontier_root,
            summary_root,
            status,
            supersedes,
        }
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn point_id(self) -> MechanismSupportObservationPointId {
        self.point_id
    }

    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn cursor(self) -> MechanismSupportCheckpointCursor {
        self.cursor
    }

    pub(crate) const fn frontier_root(self) -> MechanismSupportFrontierRoot {
        self.frontier_root
    }

    pub(crate) const fn summary_root(self) -> MechanismFactorizedSupportObservationSummaryRoot {
        self.summary_root
    }

    pub(crate) const fn status(self) -> MechanismSupportObservationStatus {
        self.status
    }

    pub(crate) const fn supersedes(self) -> Option<MechanismSupportObservationPointId> {
        self.supersedes
    }

    fn validate_identity(self) -> bool {
        self.version == MECHANISM_SUPPORT_OBSERVATION_POINT_VERSION
            && self.point_id
                == derive_mechanism_support_observation_point_id(
                    self.version,
                    self.slice,
                    self.cursor,
                    self.frontier_root,
                    self.summary_root,
                    self.status,
                    self.supersedes,
                )
    }
}

/// Replay-owned payload for publication and scheduler inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportObservationPoint {
    claim: MechanismSupportObservationClaim,
    summary: MechanismFactorizedSupportObservationSummary,
}

impl MechanismSupportObservationPoint {
    pub(crate) const fn claim(&self) -> MechanismSupportObservationClaim {
        self.claim
    }

    pub(crate) const fn point_id(&self) -> MechanismSupportObservationPointId {
        self.claim.point_id()
    }

    pub(crate) const fn slice(&self) -> MechanismSupportSlice {
        self.claim.slice()
    }

    pub(crate) const fn status(&self) -> MechanismSupportObservationStatus {
        self.claim.status()
    }

    pub(crate) const fn summary(&self) -> &MechanismFactorizedSupportObservationSummary {
        &self.summary
    }
}

/// Replay-checkable attachment of one compact observation reader to an exact
/// durable support prefix. This is operational extension state: neither the
/// claim nor its scheduler roots enter the answer-defining analysis DAG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportObservationDemandRegistrationClaim {
    version: u32,
    slice: MechanismSupportSlice,
    cursor: MechanismSupportCheckpointCursor,
    frontier_root: MechanismSupportFrontierRoot,
    disposition: MechanismExplicitObservationRegistrationDisposition,
    phase: MechanismExplicitObservationRegistrationPhase,
    registration_structural_cursor: u128,
    prior_scheduler: MechanismExplicitObservationSchedulerSummary,
    next_scheduler: MechanismExplicitObservationSchedulerSummary,
}

impl MechanismSupportObservationDemandRegistrationClaim {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        disposition: MechanismExplicitObservationRegistrationDisposition,
        phase: MechanismExplicitObservationRegistrationPhase,
        registration_structural_cursor: u128,
        prior_scheduler: MechanismExplicitObservationSchedulerSummary,
        next_scheduler: MechanismExplicitObservationSchedulerSummary,
    ) -> Self {
        Self {
            version: MECHANISM_SUPPORT_OBSERVATION_DEMAND_REGISTRATION_VERSION,
            slice,
            cursor,
            frontier_root,
            disposition,
            phase,
            registration_structural_cursor,
            prior_scheduler,
            next_scheduler,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn restore_from_journal_codec(
        version: u32,
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        disposition: MechanismExplicitObservationRegistrationDisposition,
        phase: MechanismExplicitObservationRegistrationPhase,
        registration_structural_cursor: u128,
        prior_scheduler: MechanismExplicitObservationSchedulerSummary,
        next_scheduler: MechanismExplicitObservationSchedulerSummary,
    ) -> Self {
        Self {
            version,
            slice,
            cursor,
            frontier_root,
            disposition,
            phase,
            registration_structural_cursor,
            prior_scheduler,
            next_scheduler,
        }
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn cursor(self) -> MechanismSupportCheckpointCursor {
        self.cursor
    }

    pub(crate) const fn frontier_root(self) -> MechanismSupportFrontierRoot {
        self.frontier_root
    }

    pub(crate) const fn disposition(self) -> MechanismExplicitObservationRegistrationDisposition {
        self.disposition
    }

    pub(crate) const fn phase(self) -> MechanismExplicitObservationRegistrationPhase {
        self.phase
    }

    pub(crate) const fn registration_structural_cursor(self) -> u128 {
        self.registration_structural_cursor
    }

    pub(crate) const fn prior_scheduler(self) -> MechanismExplicitObservationSchedulerSummary {
        self.prior_scheduler
    }

    pub(crate) const fn next_scheduler(self) -> MechanismExplicitObservationSchedulerSummary {
        self.next_scheduler
    }
}

/// One deterministic page of late-reader catch-up. The exact support anchor,
/// prior scheduler and successor scheduler make discarded proposals harmless:
/// replay either remints this page byte-for-byte or rejects it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MechanismSupportObservationBackfillClaim {
    version: u32,
    slice: MechanismSupportSlice,
    cursor: MechanismSupportCheckpointCursor,
    frontier_root: MechanismSupportFrontierRoot,
    phase: MechanismExplicitObservationRegistrationPhase,
    registration_structural_cursor: u128,
    from_structural_cursor: u128,
    through_structural_cursor: u128,
    completed: bool,
    prior_scheduler: MechanismExplicitObservationSchedulerSummary,
    next_scheduler: MechanismExplicitObservationSchedulerSummary,
}

impl MechanismSupportObservationBackfillClaim {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        phase: MechanismExplicitObservationRegistrationPhase,
        registration_structural_cursor: u128,
        from_structural_cursor: u128,
        through_structural_cursor: u128,
        completed: bool,
        prior_scheduler: MechanismExplicitObservationSchedulerSummary,
        next_scheduler: MechanismExplicitObservationSchedulerSummary,
    ) -> Self {
        Self {
            version: MECHANISM_SUPPORT_OBSERVATION_BACKFILL_VERSION,
            slice,
            cursor,
            frontier_root,
            phase,
            registration_structural_cursor,
            from_structural_cursor,
            through_structural_cursor,
            completed,
            prior_scheduler,
            next_scheduler,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn restore_from_journal_codec(
        version: u32,
        slice: MechanismSupportSlice,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        phase: MechanismExplicitObservationRegistrationPhase,
        registration_structural_cursor: u128,
        from_structural_cursor: u128,
        through_structural_cursor: u128,
        completed: bool,
        prior_scheduler: MechanismExplicitObservationSchedulerSummary,
        next_scheduler: MechanismExplicitObservationSchedulerSummary,
    ) -> Self {
        Self {
            version,
            slice,
            cursor,
            frontier_root,
            phase,
            registration_structural_cursor,
            from_structural_cursor,
            through_structural_cursor,
            completed,
            prior_scheduler,
            next_scheduler,
        }
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn slice(self) -> MechanismSupportSlice {
        self.slice
    }

    pub(crate) const fn cursor(self) -> MechanismSupportCheckpointCursor {
        self.cursor
    }

    pub(crate) const fn frontier_root(self) -> MechanismSupportFrontierRoot {
        self.frontier_root
    }

    pub(crate) const fn phase(self) -> MechanismExplicitObservationRegistrationPhase {
        self.phase
    }

    pub(crate) const fn registration_structural_cursor(self) -> u128 {
        self.registration_structural_cursor
    }

    pub(crate) const fn from_structural_cursor(self) -> u128 {
        self.from_structural_cursor
    }

    pub(crate) const fn through_structural_cursor(self) -> u128 {
        self.through_structural_cursor
    }

    pub(crate) const fn completed(self) -> bool {
        self.completed
    }

    pub(crate) const fn prior_scheduler(self) -> MechanismExplicitObservationSchedulerSummary {
        self.prior_scheduler
    }

    pub(crate) const fn next_scheduler(self) -> MechanismExplicitObservationSchedulerSummary {
        self.next_scheduler
    }
}

/// Why the deterministic relational coordinator selected one emitted batch.
///
/// `priority()` is the stable coordinator tier under
/// [`RELATIONAL_SCHEDULER_POLICY_VERSION`]: lower values are offered first.
/// Candidate reasons intentionally share the base tier while retaining unique
/// canonical tags. The decision deliberately omits work
/// subjects because the immediately following batch events already commit
/// their exact IDs and payloads.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalSchedulerDecision {
    AnalysisRegistration,
    InterruptedMechanismArtifactRecovery,
    ExplicitObservation,
    ReadyResult,
    ReadyIncidenceResult,
    MechanismSupport,
    ReadyMechanism,
    BaseFrontier,
    BaseCandidateCheckedGuard,
    BaseCandidateSourceEvent,
    BaseCandidateLifted,
    BaseCandidateCertifiedPieceBoundary,
    BaseCandidateLowerRangeEndpoint,
    BaseCandidateUpperRangeEndpoint,
    BaseCandidateCertificateMidpoint,
    BaseCandidateResidual,
    BaseClassifiedPrefixAdvance,
    SelectedQuestionBind,
    AnalysisClose,
}

impl RelationalSchedulerDecision {
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::AnalysisRegistration => 0,
            Self::InterruptedMechanismArtifactRecovery => 1,
            Self::ExplicitObservation => 2,
            Self::ReadyResult => 3,
            Self::ReadyIncidenceResult => 4,
            Self::MechanismSupport => 5,
            Self::ReadyMechanism => 6,
            Self::BaseFrontier
            | Self::BaseCandidateCheckedGuard
            | Self::BaseCandidateSourceEvent
            | Self::BaseCandidateLifted
            | Self::BaseCandidateCertifiedPieceBoundary
            | Self::BaseCandidateLowerRangeEndpoint
            | Self::BaseCandidateUpperRangeEndpoint
            | Self::BaseCandidateCertificateMidpoint
            | Self::BaseCandidateResidual
            | Self::BaseClassifiedPrefixAdvance => 7,
            Self::SelectedQuestionBind => 8,
            Self::AnalysisClose => 9,
        }
    }

    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::AnalysisRegistration => 0x01,
            Self::InterruptedMechanismArtifactRecovery => 0x02,
            Self::ExplicitObservation => 0x03,
            Self::ReadyResult => 0x04,
            Self::ReadyIncidenceResult => 0x05,
            Self::MechanismSupport => 0x06,
            Self::ReadyMechanism => 0x07,
            Self::BaseFrontier => 0x08,
            Self::SelectedQuestionBind => 0x09,
            Self::AnalysisClose => 0x0a,
            Self::BaseCandidateCheckedGuard => 0x0b,
            Self::BaseCandidateCertifiedPieceBoundary => 0x0c,
            Self::BaseCandidateLowerRangeEndpoint => 0x0d,
            Self::BaseCandidateUpperRangeEndpoint => 0x0e,
            Self::BaseCandidateCertificateMidpoint => 0x0f,
            Self::BaseCandidateResidual => 0x10,
            Self::BaseClassifiedPrefixAdvance => 0x11,
            Self::BaseCandidateSourceEvent => 0x12,
            Self::BaseCandidateLifted => 0x13,
        }
    }

    pub(crate) const fn requires_candidate_nomination_root(self) -> bool {
        matches!(
            self,
            Self::BaseCandidateCheckedGuard
                | Self::BaseCandidateSourceEvent
                | Self::BaseCandidateLifted
                | Self::BaseCandidateCertifiedPieceBoundary
                | Self::BaseCandidateLowerRangeEndpoint
                | Self::BaseCandidateUpperRangeEndpoint
                | Self::BaseCandidateCertificateMidpoint
                | Self::BaseCandidateResidual
        )
    }
}

/// One resumability mutation. Checkpoints are authenticated by journal order,
/// but are deliberately absent from the arrival-order-independent semantic
/// evidence roots: a scheduler may reach the same proof frontier by a
/// different sequence of legal pauses and work choices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCheckpointEvent {
    /// Persist the policy and reason for a coordinator-emitted batch. It is
    /// operational provenance only: it changes the authenticated journal head
    /// but no relation, question, mechanism, result, or semantic evidence root.
    SchedulerDecisionRecorded {
        policy_version: u32,
        decision: RelationalSchedulerDecision,
        nomination_root: Option<RelationalCandidateNominationRoot>,
        /// Canonical digest of the complete ordered work-event batch selected
        /// at this prefix. A decision-only crash prefix therefore still says
        /// exactly what was attempted, without making that work semantic
        /// evidence or claiming it completed.
        work_fingerprint: [u8; 32],
    },
    /// One caller-bounded checked prefix of the selected canonical classified
    /// chunk. Replay validates and folds the artifact into an operational
    /// accumulator without executing user code. It cannot advance classified
    /// support or the semantic chunk cursor; only the separately accepted
    /// canonical whole-chunk artifact may do that.
    RelationalClassifiedChunkSliceCheckpointed {
        artifact: Box<RelationalClassifiedChunkSliceArtifact>,
    },
    /// Advance exactly one occupied canonical classified slot into the
    /// committed root prefix. Sparse classification evidence may arrive in
    /// any deterministic scheduler order; this bounded checkpoint is the
    /// sole authority that advances the root materialization cursor.
    RelationalClassifiedPrefixAdvanced {
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_ordinal: u128,
        artifact_digest: [u8; 32],
    },
    /// Declare one ordinary semantic work node. Its ID commits both the spec
    /// and the canonical dependency set, so replay can reject an altered
    /// claim before it changes the frontier.
    WorkNodeInserted {
        node_id: WorkNodeId,
        spec: WorkNodeSpec,
        dependencies: Box<[WorkNodeId]>,
    },
    /// Publish an immutable completed readiness token as soon as its row or
    /// prefix exists. Readiness never waits for the surrounding enumerator to
    /// close.
    WorkReadinessMaterialized {
        node_id: WorkNodeId,
        spec: WorkNodeSpec,
    },
    WorkCursorAdvanced {
        node_id: WorkNodeId,
        next_member_ordinal: u128,
    },
    SupportMaterializationCheckpointed {
        cursor: SupportMaterializationCursor,
    },
    /// Persist one exact request-local mechanism-support prefix after replay
    /// has imported the named target, terminal, and structural-assignment
    /// cursors through their checked upstream authorities.
    SupportFrontierCheckpointed {
        request_id: MechanismRequestId,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
    },
    /// Attach one checked compact reader to the current request-local support
    /// prefix. Duplicate names are resolved before this slice-level event;
    /// replay stores at most one registration per stable slice.
    SupportObservationDemandRegistered {
        claim: MechanismSupportObservationDemandRegistrationClaim,
    },
    /// Advance the canonical minimum late-registration catch-up by one
    /// protocol-bounded structural-assignment page.
    SupportObservationBackfillCheckpointed {
        claim: MechanismSupportObservationBackfillClaim,
    },
    /// Append one replay-derived observation of a structural support slice at
    /// the exact latest durable support frontier.
    SupportSubjectObserved {
        claim: MechanismSupportObservationClaim,
    },
    WorkNodeCompleted {
        node_id: WorkNodeId,
        completion: WorkCompletionRef,
    },
    /// Deterministically discard a bounded set of completed leaf work nodes.
    /// Semantic evidence stays in its catalogs and the outer journal; only the
    /// resumability projection shrinks.
    WorkFrontierCompacted { receipt: WorkFrontierCompaction },
}

/// One authenticated journal frame. Event class is explicit so answer
/// identity can never accidentally inherit scheduler or cursor history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalJournalEvent {
    Evidence(RelationalEvidenceEvent),
    Checkpoint(RelationalCheckpointEvent),
}

impl RelationalJournalEvent {
    pub(crate) const fn scheduler_decision_recorded(
        decision: RelationalSchedulerDecision,
        nomination_root: Option<RelationalCandidateNominationRoot>,
        work_fingerprint: [u8; 32],
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::SchedulerDecisionRecorded {
            policy_version: RELATIONAL_SCHEDULER_POLICY_VERSION,
            decision,
            nomination_root,
            work_fingerprint,
        })
    }

    pub(crate) fn analysis_plan_registered(plan: RelationalAnalysisPlan) -> Self {
        Self::Evidence(RelationalEvidenceEvent::AnalysisPlanRegistered {
            plan_root: plan.root(),
            plan: Box::new(plan),
        })
    }

    pub(crate) fn support_plan_registered(plan: RelationalSupportPlan) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SupportPlanRegistered {
            plan_root: plan.root(),
            plan: Box::new(plan),
        })
    }

    pub(crate) fn relational_case_image_injectivity_proof_accepted(
        artifact: RelationalCaseImageInjectivityProofArtifact,
    ) -> Self {
        Self::Evidence(
            RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn relational_source_image_exactness_proof_accepted(
        artifact: RelationalSourceImageExactnessProofArtifact,
    ) -> Self {
        Self::Evidence(
            RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn relational_uniform_admission_proof_accepted(
        artifact: RelationalUniformAdmissionProofArtifact,
    ) -> Self {
        Self::Evidence(
            RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn relational_case_chunk_partition_accepted(
        artifact: RelationalCaseChunkPartitionArtifact,
    ) -> Self {
        Self::Evidence(
            RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn relational_classified_chunk_accepted(
        artifact: RelationalClassifiedChunkArtifact,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::RelationalClassifiedChunkAccepted {
            artifact: Box::new(artifact),
        })
    }

    pub(crate) const fn relational_classified_prefix_advanced(
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_ordinal: u128,
        artifact_digest: [u8; 32],
    ) -> Self {
        Self::Checkpoint(
            RelationalCheckpointEvent::RelationalClassifiedPrefixAdvanced {
                partition_artifact_id,
                chunk_ordinal,
                artifact_digest,
            },
        )
    }

    pub(crate) fn relational_region_proof_accepted(
        artifact: RelationalRegionProofArtifact,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::RelationalRegionProofAccepted {
            artifact: Box::new(artifact),
        })
    }

    pub(crate) fn relational_selected_run_materialization_accepted(
        artifact: RelationalSelectedRunMaterializationArtifact,
    ) -> Self {
        Self::Evidence(
            RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn source_traversal_observed(
        relation_id: RelationId,
        support_plan_root: RelationalSupportPlanRoot,
        advance: RelationalSourceAdvance,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SourceTraversalObserved {
            advance_id: SourceTraversalAdvanceId::derive(relation_id, support_plan_root, &advance),
            advance: Box::new(advance),
        })
    }

    pub(crate) fn successor_discovered(
        relation_id: RelationId,
        source_key: SourceKey,
        row: SuccessorRow,
    ) -> Self {
        let successor_key = SuccessorKey::derive(relation_id, source_key, &row);
        Self::successor_discovered_with_ids(
            source_key,
            successor_key,
            RelationalCaseId::derive(relation_id, source_key, successor_key),
            row,
        )
    }

    pub(crate) fn successor_discovered_with_ids(
        source_key: SourceKey,
        successor_key: SuccessorKey,
        case_id: RelationalCaseId,
        row: SuccessorRow,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SuccessorDiscovered {
            source_key,
            successor_key,
            case_id,
            row,
        })
    }

    pub(crate) fn source_enumeration_sealed(receipt: SourceRelationExhaustionReceipt) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SourceEnumerationSealed {
            receipt_id: receipt.id(),
            receipt: Box::new(receipt),
        })
    }

    pub(crate) fn successor_fiber_exhaustion_accepted(
        receipt: SuccessorFiberExhaustionReceipt,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted {
            receipt_id: receipt.id(),
            receipt,
        })
    }

    pub(crate) fn successor_enumeration_sealed(receipt: &SuccessorFiberExhaustionReceipt) -> Self {
        Self::Evidence(RelationalEvidenceEvent::SuccessorEnumerationSealed {
            source_key: receipt.source_key(),
            receipt_id: receipt.id(),
        })
    }

    pub(crate) const fn admission_classified(
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::AdmissionClassified { case_id, decision })
    }

    pub(crate) const fn question_classified(
        question_id: QuestionId,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::QuestionClassified {
            question_id,
            case_id,
            decision,
        })
    }

    pub(crate) const fn support(event: SupportJournalEvent) -> Self {
        Self::Evidence(RelationalEvidenceEvent::Support(event))
    }

    pub(crate) const fn analysis(event: RelationalAnalysisEvidenceEvent) -> Self {
        Self::Evidence(RelationalEvidenceEvent::Analysis(event))
    }

    pub(crate) fn work_node_inserted(
        spec: WorkNodeSpec,
        dependencies: impl IntoIterator<Item = WorkNodeId>,
    ) -> Result<Self, WorkFrontierError> {
        let dependencies = dependencies
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let node_id = RelationalWorkFrontier::derive_node_id(&spec, dependencies.iter().copied())?;
        Ok(Self::Checkpoint(
            RelationalCheckpointEvent::WorkNodeInserted {
                node_id,
                spec,
                dependencies,
            },
        ))
    }

    pub(crate) fn work_readiness_materialized(
        spec: WorkNodeSpec,
    ) -> Result<Self, WorkFrontierError> {
        let node_id = RelationalWorkFrontier::derive_node_id(&spec, [])?;
        Ok(Self::Checkpoint(
            RelationalCheckpointEvent::WorkReadinessMaterialized { node_id, spec },
        ))
    }

    pub(crate) const fn work_cursor_advanced(
        node_id: WorkNodeId,
        next_member_ordinal: u128,
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::WorkCursorAdvanced {
            node_id,
            next_member_ordinal,
        })
    }

    pub(crate) const fn work_node_completed(
        node_id: WorkNodeId,
        completion: WorkCompletionRef,
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::WorkNodeCompleted {
            node_id,
            completion,
        })
    }

    pub(crate) const fn work_frontier_compacted(receipt: WorkFrontierCompaction) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::WorkFrontierCompacted { receipt })
    }

    pub(crate) fn support_materialization_checkpointed(
        cursor: SupportMaterializationCursor,
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::SupportMaterializationCheckpointed { cursor })
    }

    pub(crate) const fn support_frontier_checkpointed(
        request_id: MechanismRequestId,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::SupportFrontierCheckpointed {
            request_id,
            cursor,
            frontier_root,
        })
    }

    pub(crate) const fn support_subject_observed(claim: MechanismSupportObservationClaim) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::SupportSubjectObserved { claim })
    }

    pub(crate) const fn support_observation_demand_registered(
        claim: MechanismSupportObservationDemandRegistrationClaim,
    ) -> Self {
        Self::Checkpoint(RelationalCheckpointEvent::SupportObservationDemandRegistered { claim })
    }

    pub(crate) const fn support_observation_backfill_checkpointed(
        claim: MechanismSupportObservationBackfillClaim,
    ) -> Self {
        Self::Checkpoint(
            RelationalCheckpointEvent::SupportObservationBackfillCheckpointed { claim },
        )
    }

    pub(crate) fn relational_classified_chunk_slice_checkpointed(
        artifact: RelationalClassifiedChunkSliceArtifact,
    ) -> Self {
        Self::Checkpoint(
            RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed {
                artifact: Box::new(artifact),
            },
        )
    }

    pub(crate) fn source_key(&self) -> Option<SourceKey> {
        match self {
            Self::Evidence(
                RelationalEvidenceEvent::SuccessorDiscovered { source_key, .. }
                | RelationalEvidenceEvent::SuccessorEnumerationSealed { source_key, .. },
            ) => Some(*source_key),
            Self::Evidence(RelationalEvidenceEvent::SourceTraversalObserved {
                advance, ..
            }) => match advance.as_ref() {
                RelationalSourceAdvance::Yielded {
                    continuation: RelationalSourceContinuation::Source(source),
                    ..
                } => Some(source.source_key()),
                RelationalSourceAdvance::Yielded { .. }
                | RelationalSourceAdvance::Exhausted { .. } => None,
            },
            Self::Evidence(RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted {
                receipt,
                ..
            }) => Some(receipt.source_key()),
            Self::Evidence(
                RelationalEvidenceEvent::AnalysisPlanRegistered { .. }
                | RelationalEvidenceEvent::SupportPlanRegistered { .. }
                | RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted { .. }
                | RelationalEvidenceEvent::RelationalClassifiedChunkAccepted { .. }
                | RelationalEvidenceEvent::RelationalRegionProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted { .. }
                | RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted { .. }
                | RelationalEvidenceEvent::SourceEnumerationSealed { .. }
                | RelationalEvidenceEvent::AdmissionClassified { .. }
                | RelationalEvidenceEvent::QuestionClassified { .. }
                | RelationalEvidenceEvent::Support(_)
                | RelationalEvidenceEvent::Analysis(_),
            )
            | Self::Checkpoint(_) => None,
        }
    }

    pub(crate) const fn case_id(&self) -> Option<RelationalCaseId> {
        match self {
            Self::Evidence(
                RelationalEvidenceEvent::SuccessorDiscovered { case_id, .. }
                | RelationalEvidenceEvent::AdmissionClassified { case_id, .. }
                | RelationalEvidenceEvent::QuestionClassified { case_id, .. },
            ) => Some(*case_id),
            Self::Evidence(
                RelationalEvidenceEvent::AnalysisPlanRegistered { .. }
                | RelationalEvidenceEvent::SupportPlanRegistered { .. }
                | RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted { .. }
                | RelationalEvidenceEvent::RelationalClassifiedChunkAccepted { .. }
                | RelationalEvidenceEvent::RelationalRegionProofAccepted { .. }
                | RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted { .. }
                | RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted { .. }
                | RelationalEvidenceEvent::SourceTraversalObserved { .. }
                | RelationalEvidenceEvent::SourceEnumerationSealed { .. }
                | RelationalEvidenceEvent::SuccessorEnumerationSealed { .. }
                | RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted { .. }
                | RelationalEvidenceEvent::Support(_)
                | RelationalEvidenceEvent::Analysis(_),
            )
            | Self::Checkpoint(_) => None,
        }
    }

    pub(crate) const fn work_node_id(&self) -> Option<WorkNodeId> {
        match self {
            Self::Checkpoint(
                RelationalCheckpointEvent::WorkNodeInserted { node_id, .. }
                | RelationalCheckpointEvent::WorkReadinessMaterialized { node_id, .. }
                | RelationalCheckpointEvent::WorkCursorAdvanced { node_id, .. }
                | RelationalCheckpointEvent::WorkNodeCompleted { node_id, .. },
            ) => Some(*node_id),
            Self::Evidence(_)
            | Self::Checkpoint(RelationalCheckpointEvent::SchedulerDecisionRecorded { .. })
            | Self::Checkpoint(
                RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::RelationalClassifiedPrefixAdvanced {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportMaterializationCheckpointed {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportFrontierCheckpointed { .. })
            | Self::Checkpoint(
                RelationalCheckpointEvent::SupportObservationDemandRegistered { .. }
                | RelationalCheckpointEvent::SupportObservationBackfillCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::SupportSubjectObserved { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkFrontierCompacted { .. }) => None,
        }
    }

    pub(crate) const fn compacted_work_node_count(&self) -> Option<u32> {
        match self {
            Self::Checkpoint(RelationalCheckpointEvent::WorkFrontierCompacted { receipt }) => {
                Some(receipt.removed_nodes())
            }
            Self::Evidence(_)
            | Self::Checkpoint(RelationalCheckpointEvent::SchedulerDecisionRecorded { .. })
            | Self::Checkpoint(
                RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::RelationalClassifiedPrefixAdvanced {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkNodeInserted { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkReadinessMaterialized { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkCursorAdvanced { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportMaterializationCheckpointed {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportFrontierCheckpointed { .. })
            | Self::Checkpoint(
                RelationalCheckpointEvent::SupportObservationDemandRegistered { .. }
                | RelationalCheckpointEvent::SupportObservationBackfillCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::SupportSubjectObserved { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkNodeCompleted { .. }) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalEntry {
    sequence: u64,
    previous: RelationalJournalHead,
    event: RelationalJournalEvent,
    head: RelationalJournalHead,
}

impl RelationalJournalEntry {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn previous(&self) -> RelationalJournalHead {
        self.previous
    }

    pub(crate) fn event(&self) -> &RelationalJournalEvent {
        &self.event
    }

    pub(crate) const fn head(&self) -> RelationalJournalHead {
        self.head
    }

    /// Rebuild one durable frame only after checking its cursor anchors and
    /// recomputing the semantic journal head from the decoded event. The codec
    /// never receives a constructor for an arbitrary `RelationalJournalHead`.
    pub(super) fn restore_from_journal_codec(
        contract: RelationalJournalContract,
        expected_sequence: u64,
        expected_previous: RelationalJournalHead,
        sequence: u64,
        previous: [u8; 32],
        event: RelationalJournalEvent,
        claimed_head: [u8; 32],
    ) -> Result<Self, RelationalJournalError> {
        if sequence != expected_sequence {
            return Err(RelationalJournalError::SequenceMismatch {
                expected: expected_sequence,
                found: sequence,
            });
        }
        if previous != expected_previous.bytes() {
            return Err(RelationalJournalError::PreviousHeadMismatch { sequence });
        }
        let head = journal_entry_head(contract.id(), sequence, expected_previous, &event);
        if claimed_head != head.bytes() {
            return Err(RelationalJournalError::EntryHeadMismatch { sequence });
        }
        Ok(Self {
            sequence,
            previous: expected_previous,
            event,
            head,
        })
    }
}

/// Compact replay authority for the ordered classified partition prefix.
///
/// The generic support cursor mirrors this state for operational checkpoint
/// consumers, but cannot advance it. Only accepted concrete-sweep or regional
/// certificate evidence appends one exact canonical ordinal/artifact binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassifiedSupportFragment {
    Concrete(RelationalClassifiedChunkArtifact),
    CertifiedZeroSelected(RelationalRegionProofArtifact),
}

impl RelationalClassifiedSupportFragment {
    pub(crate) const fn artifact_digest(&self) -> [u8; 32] {
        match self {
            Self::Concrete(artifact) => artifact.id().bytes(),
            Self::CertifiedZeroSelected(artifact) => artifact.certificate_id(),
        }
    }

    pub(crate) const fn concrete(&self) -> Option<&RelationalClassifiedChunkArtifact> {
        match self {
            Self::Concrete(artifact) => Some(artifact),
            Self::CertifiedZeroSelected(_) => None,
        }
    }

    pub(crate) const fn certificate(&self) -> Option<&RelationalRegionProofArtifact> {
        match self {
            Self::Concrete(_) => None,
            Self::CertifiedZeroSelected(artifact) => Some(artifact),
        }
    }

    pub(crate) const fn chunk_ordinal(&self) -> u128 {
        match self {
            Self::Concrete(artifact) => artifact.chunk_ordinal(),
            Self::CertifiedZeroSelected(artifact) => match artifact.subject() {
                RelationalRegionProofSubject::CanonicalChunk { chunk_ordinal, .. } => chunk_ordinal,
                RelationalRegionProofSubject::Root => 0,
            },
        }
    }

    pub(crate) const fn chunk_id(&self) -> Option<RelationalCaseChunkId> {
        match self {
            Self::Concrete(artifact) => Some(artifact.chunk_id()),
            Self::CertifiedZeroSelected(artifact) => match artifact.subject() {
                RelationalRegionProofSubject::CanonicalChunk { chunk_id, .. } => Some(chunk_id),
                RelationalRegionProofSubject::Root => None,
            },
        }
    }

    pub(crate) const fn chunk_cell_id(&self) -> SupportCellId {
        match self {
            Self::Concrete(artifact) => artifact.chunk_cell_id(),
            Self::CertifiedZeroSelected(artifact) => match artifact.subject() {
                RelationalRegionProofSubject::Root => artifact.root_cell_id(),
                RelationalRegionProofSubject::CanonicalChunk { chunk_cell_id, .. } => chunk_cell_id,
            },
        }
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        match self {
            Self::Concrete(artifact) => artifact.interval_start(),
            Self::CertifiedZeroSelected(artifact) => artifact.coordinate_start(),
        }
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        match self {
            Self::Concrete(artifact) => artifact.interval_end_exclusive(),
            Self::CertifiedZeroSelected(artifact) => artifact.coordinate_end_exclusive(),
        }
    }

    pub(crate) const fn exact_case_count(&self) -> u128 {
        match self {
            Self::Concrete(artifact) => artifact.evaluated_case_count(),
            Self::CertifiedZeroSelected(artifact) => artifact.case_cardinality(),
        }
    }

    pub(crate) const fn rejected_count(&self) -> u128 {
        match self {
            Self::Concrete(artifact) => artifact.rejected_count(),
            Self::CertifiedZeroSelected(artifact) => match artifact.conclusion() {
                RelationalCertifiedRegionConclusion::Rejected => artifact.case_cardinality(),
                RelationalCertifiedRegionConclusion::AdmittedNotSelected => 0,
            },
        }
    }

    pub(crate) fn admitted_not_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        match self {
            Self::Concrete(artifact) => artifact.admitted_not_selected_count(question_id),
            Self::CertifiedZeroSelected(artifact) if artifact.question_id() == question_id => {
                Some(match artifact.conclusion() {
                    RelationalCertifiedRegionConclusion::Rejected => 0,
                    RelationalCertifiedRegionConclusion::AdmittedNotSelected => {
                        artifact.case_cardinality()
                    }
                })
            }
            Self::CertifiedZeroSelected(_) => None,
        }
    }

    pub(crate) fn admitted_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        match self {
            Self::Concrete(artifact) => artifact.admitted_selected_count(question_id),
            Self::CertifiedZeroSelected(artifact) if artifact.question_id() == question_id => {
                Some(0)
            }
            Self::CertifiedZeroSelected(_) => None,
        }
    }
}

/// One immutable case/support fact in replay discovery order.
///
/// The retained payload remains owned by the journal's canonical sparse
/// catalogs. This borrowed view adds only the logical coordinates needed by a
/// publisher to address a bounded package and to filter the artifact's
/// checked question mask. Discovery order is operational: it is never hashed
/// into relation, question, support, or closure identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportDiscoveryEvent<'a> {
    ClassifiedFragment {
        chunk_ordinal: u128,
        fragment: &'a RelationalClassifiedSupportFragment,
    },
    SelectedRunMaterialization {
        chunk_ordinal: u128,
        run_ordinal: u16,
        materialization: &'a RelationalSelectedRunMaterializationArtifact,
    },
}

/// Compact coordinate into the canonical sparse catalogs.
///
/// Storing coordinates instead of cloning content IDs or artifacts gives the
/// append-only publisher a replay-stable merge order without retaining a
/// second copy of semantic evidence. Every coordinate is resolved and checked
/// again by [`RelationalSchedulerView::case_support_discovery_event_at`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalCaseSupportDiscoveryCoordinate {
    ClassifiedFragment {
        chunk_ordinal: usize,
    },
    SelectedRunMaterialization {
        chunk_ordinal: usize,
        run_ordinal: u16,
    },
}

/// Zero-copy borrowed view of the durable contiguous classified prefix.
///
/// The sparse slot table is the replay-derived storage authority and the
/// classified progress record proves that every slot before `len` is
/// occupied. Keeping the prefix as this lightweight view avoids rebuilding an
/// allocated table of references on every scheduler/publication observation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalClassifiedSupportPrefix<'a> {
    slots: &'a [Option<RelationalClassifiedSupportFragment>],
}

impl<'a> RelationalClassifiedSupportPrefix<'a> {
    pub(crate) const fn len(self) -> usize {
        self.slots.len()
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.slots.is_empty()
    }

    pub(crate) fn get(self, index: usize) -> Option<&'a RelationalClassifiedSupportFragment> {
        self.slots.get(index).and_then(Option::as_ref)
    }

    pub(crate) fn iter(
        self,
    ) -> impl ExactSizeIterator<Item = &'a RelationalClassifiedSupportFragment> + 'a {
        self.slots.iter().map(|slot| {
            slot.as_ref()
                .expect("classified progress admits only an occupied contiguous prefix")
        })
    }
}

impl std::ops::Index<usize> for RelationalClassifiedSupportPrefix<'_> {
    type Output = RelationalClassifiedSupportFragment;

    fn index(&self, index: usize) -> &Self::Output {
        self.slots[index]
            .as_ref()
            .expect("classified progress admits only an occupied contiguous prefix")
    }
}

fn classified_child_resolver_node_id(
    cell_id: SupportCellId,
    obligation_id: SupportProofObligationId,
) -> Result<WorkNodeId, WorkFrontierError> {
    let readiness = WorkNodeSpec::SupportCellReady { cell_id };
    let readiness_id = RelationalWorkFrontier::derive_node_id(&readiness, [])?;
    RelationalWorkFrontier::derive_node_id(
        &WorkNodeSpec::ResolveSupportObligation {
            cell_id,
            obligation_id,
        },
        [readiness_id],
    )
}

/// One-chunk replay/apply accelerator for consecutive selected-run artifacts.
///
/// The classified artifact ID is content-derived and every cache fill runs the
/// full retained-chunk reverifier first. This cache is neither serialized nor
/// committed by any root; fresh journal replay therefore starts empty and
/// independently validates the first selected event for each encountered
/// chunk. Replacement keeps retained memory bounded to one V1 chunk.
#[derive(Clone, Debug)]
struct RelationalSelectedRunClassifiedVerificationCache {
    artifact_id: RelationalClassifiedChunkArtifactId,
    verified: VerifiedRelationalClassifiedChunk,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedSweepProgress {
    partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
    root_cell_id: SupportCellId,
    root_materializer_id: super::support_cell::SupportMaterializerId,
    interval_start: u128,
    interval_end_exclusive: u128,
    accepted_chunks: Vec<RelationalClassifiedSweepAcceptedChunk>,
    next_chunk_ordinal: u128,
    next_coordinate_ordinal: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalClassifiedSweepAcceptedChunk {
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    artifact_digest: [u8; 32],
    interval_start: u128,
    interval_end_exclusive: u128,
}

impl RelationalClassifiedSweepProgress {
    fn from_partition(
        artifact: &RelationalCaseChunkPartitionArtifact,
    ) -> Result<Self, RelationalJournalError> {
        artifact.validate_identity()?;
        Ok(Self {
            partition_artifact_id: artifact.id(),
            root_cell_id: artifact.root_cell_id(),
            root_materializer_id: artifact.root_materializer_id(),
            interval_start: artifact.interval_start(),
            interval_end_exclusive: artifact.interval_end_exclusive(),
            // The canonical partition fixes the complete progress capacity up
            // front. Later atomic chunk commits therefore append one bounded
            // delta without allocating or cloning their accepted prefix.
            accepted_chunks: Vec::with_capacity(artifact.chunks().len()),
            next_chunk_ordinal: 0,
            next_coordinate_ordinal: 0,
        })
    }

    fn validate_partition(
        &self,
        artifact: &RelationalCaseChunkPartitionArtifact,
    ) -> Result<(), RelationalJournalError> {
        if self.partition_artifact_id != artifact.id()
            || self.root_cell_id != artifact.root_cell_id()
            || self.root_materializer_id != artifact.root_materializer_id()
            || self.interval_start != artifact.interval_start()
            || self.interval_end_exclusive != artifact.interval_end_exclusive()
        {
            return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_fragment(
        &self,
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        chunk_cell_id: SupportCellId,
        chunk_materializer_id: super::support_cell::SupportMaterializerId,
        artifact_digest: [u8; 32],
        interval_start: u128,
        interval_end_exclusive: u128,
    ) -> Result<bool, RelationalJournalError> {
        if partition_artifact_id != self.partition_artifact_id
            || chunk_cell_id == self.root_cell_id
            || chunk_materializer_id != self.root_materializer_id
            || interval_start < self.interval_start
            || interval_end_exclusive > self.interval_end_exclusive
        {
            return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
        }
        let expected = RelationalClassifiedSweepAcceptedChunk {
            chunk_id,
            chunk_ordinal,
            artifact_digest,
            interval_start,
            interval_end_exclusive,
        };
        let ordinal = usize::try_from(chunk_ordinal).map_err(|_| {
            RelationalJournalError::ClassifiedSweepProgressGap {
                expected: self.next_chunk_ordinal,
                actual: chunk_ordinal,
            }
        })?;
        if let Some(existing) = self.accepted_chunks.get(ordinal) {
            if existing == &expected {
                return Ok(false);
            }
            return Err(RelationalJournalError::ClassifiedSweepProgressConflict { chunk_ordinal });
        }
        if ordinal != self.accepted_chunks.len() || chunk_ordinal != self.next_chunk_ordinal {
            return Err(RelationalJournalError::ClassifiedSweepProgressGap {
                expected: self.next_chunk_ordinal,
                actual: chunk_ordinal,
            });
        }
        let relative_start = interval_start
            .checked_sub(self.interval_start)
            .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
        let relative_end = interval_end_exclusive
            .checked_sub(self.interval_start)
            .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
        if relative_start != self.next_coordinate_ordinal || relative_start >= relative_end {
            return Err(
                RelationalJournalError::ClassifiedSweepProgressCoordinateMismatch {
                    expected: self.next_coordinate_ordinal,
                    actual: relative_start,
                },
            );
        }
        self.next_chunk_ordinal.checked_add(1).ok_or(
            RelationalJournalError::ClassifiedSweepProgressGap {
                expected: self.next_chunk_ordinal,
                actual: chunk_ordinal,
            },
        )?;
        Ok(true)
    }

    fn commit_validated_fragment(
        &mut self,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        artifact_digest: [u8; 32],
        interval_start: u128,
        interval_end_exclusive: u128,
    ) {
        debug_assert!(self.accepted_chunks.len() < self.accepted_chunks.capacity());
        self.accepted_chunks
            .push(RelationalClassifiedSweepAcceptedChunk {
                chunk_id,
                chunk_ordinal,
                artifact_digest,
                interval_start,
                interval_end_exclusive,
            });
        self.next_chunk_ordinal += 1;
        self.next_coordinate_ordinal = interval_end_exclusive
            .checked_sub(self.interval_start)
            .expect("the preflight validated the fragment interval");
    }

    pub(crate) const fn partition_artifact_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.partition_artifact_id
    }

    pub(crate) const fn root_cell_id(&self) -> SupportCellId {
        self.root_cell_id
    }

    pub(crate) const fn root_materializer_id(&self) -> super::support_cell::SupportMaterializerId {
        self.root_materializer_id
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) fn committed_prefix_count(&self) -> usize {
        self.accepted_chunks.len()
    }

    pub(crate) const fn next_chunk_ordinal(&self) -> u128 {
        self.next_chunk_ordinal
    }

    pub(crate) const fn next_coordinate_ordinal(&self) -> u128 {
        self.next_coordinate_ordinal
    }

    pub(crate) fn last_artifact_digest(&self) -> Option<[u8; 32]> {
        self.accepted_chunks
            .last()
            .map(|chunk| chunk.artifact_digest)
    }
}

#[derive(Clone, Debug)]
struct RelationalEvidenceState {
    contract: RelationalJournalContract,
    /// Accepted coordinator scheduling-decision counts by stable priority in
    /// this exact journal prefix. They are replay-derived operational state,
    /// excluded from semantic evidence roots, and give fair selectors a
    /// durable tier-local tie-breaker instead of a process-local cursor.
    scheduler_decision_counts: [u64; 10],
    /// Process-local checked proof authority. It is deliberately excluded
    /// from journal hashes and snapshots; a retained region event cannot be
    /// replayed at all unless this exact authority is rebound by preparation.
    region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    constructor_interner: RelationalConstructorInterner,
    relation: RelationCatalogBuilder,
    admission: AdmissionCatalogBuilder,
    questions: BTreeMap<QuestionId, QuestionCatalogBuilder>,
    transition_support: RelationalTransitionSupportIndex,
    analysis_plan: Option<RelationalAnalysisPlan>,
    analysis: Option<RelationalAnalysisJournalState>,
    support_plan: Option<RelationalSupportPlan>,
    source_image_exactness: Option<RelationalSourceImageExactnessProofArtifact>,
    source_traversal: Option<SourceTraversalAccumulator>,
    source_relation_exhaustion: Option<SourceRelationExhaustionReceipt>,
    /// Opaque partition authority reconstructed only while replay applies the
    /// authenticated partition artifact against the exact durable root proof.
    /// This is an operational index, not an additional journal commitment.
    verified_case_chunk_partition: Option<VerifiedRelationalCaseChunkPartition>,
    classified_sweep_progress: Option<RelationalClassifiedSweepProgress>,
    /// Replay-derived operational prefix of the one canonical chunk currently
    /// being evaluated. The latest slice identity is authenticated by the
    /// journal and checkpoint root, while final support evidence remains
    /// absent until the complete canonical chunk artifact is accepted.
    classified_chunk_accumulator: Option<RelationalClassifiedChunkAccumulator>,
    /// Canonical-partition-sized sparse acceptance table. Each semantic
    /// classified artifact occupies its exact ordinal independently of the
    /// root prefix, allowing candidate-first execution without inventing a
    /// second partition or losing resumability.
    classified_support_fragment_slots: Vec<Option<RelationalClassifiedSupportFragment>>,
    /// Number of occupied sparse classified slots. This is replay-derived
    /// operational state, excluded from every semantic root, and replaces a
    /// full sparse-table scan in scheduler selection.
    accepted_classified_fragment_count: usize,
    /// Replay-derived first-arrival order for independently publishable
    /// case/support packages. Entries are compact logical coordinates into the
    /// sparse fragment and materialization catalogs; duplicates, prefix
    /// promotion, and work-frontier compaction never append here. This is
    /// operational addressing state and is excluded from semantic roots and
    /// snapshots.
    case_support_discovery: Vec<RelationalCaseSupportDiscoveryCoordinate>,
    /// Exact canonical child-admission resolver nodes installed by the
    /// partition, indexed back to chunk/cell/obligation. The node identity
    /// includes its readiness dependency, so a same-spec node with different
    /// dependencies cannot retire an unrelated pending chunk.
    classified_child_by_resolver_node:
        BTreeMap<WorkNodeId, (usize, SupportCellId, SupportProofObligationId)>,
    /// Occupied classified chunks whose child-admission resolver has not yet
    /// recorded its completion checkpoint. Derived solely from accepted
    /// artifact and work-completion events; it is scheduling state, not proof.
    pending_classified_work_completion_ordinals: BTreeSet<usize>,
    /// Selected concrete runs accepted but not yet materialized, ordered by
    /// canonical chunk/run position. Sparse later chunks may enter first; a
    /// subsequently accepted lower chunk naturally becomes the next key.
    pending_selected_run_positions: BTreeSet<(usize, u16)>,
    /// Process-local replay/apply accelerator for the most recently verified
    /// classified chunk used by selected-run admission. Operational only and
    /// bounded to one canonical chunk.
    selected_run_classified_verification_cache:
        Option<RelationalSelectedRunClassifiedVerificationCache>,
    selected_run_materializations:
        BTreeMap<SupportCellId, RelationalSelectedRunMaterializationArtifact>,
    selected_run_materialization_ids:
        BTreeMap<RelationalSelectedRunMaterializationArtifactId, SupportCellId>,
    successor_exhaustion_receipts:
        BTreeMap<SuccessorFiberExhaustionReceiptId, SuccessorFiberExhaustionReceipt>,
    support: SupportEvidenceCatalogBuilder,
    /// Latest explicitly journaled request-local support checkpoint. The
    /// analysis builders remain the replay authority; this compact projection
    /// exists solely to bind resumability into the outer checkpoint root.
    latest_support_frontiers:
        BTreeMap<MechanismRequestId, RelationalMechanismSupportCheckpointReceipt>,
    /// Durable scheduler projection paired with each accepted request-local
    /// support prefix. Planning may speculatively advance the analysis cache;
    /// only this map participates in the outer checkpoint root.
    latest_support_schedulers:
        BTreeMap<MechanismRequestId, MechanismAutomaticObservationSchedulerSummary>,
    /// Durable extension scheduler paired with the same exact support prefix.
    /// Unlike the automatic scheduler this may continue after analysis close
    /// and is committed only by explicit checkpoint events.
    latest_explicit_support_schedulers:
        BTreeMap<MechanismRequestId, MechanismExplicitObservationSchedulerSummary>,
    /// First registration of each stable observation slice, in attachment
    /// order for streaming publication and indexed for O(log D) deduplication.
    mechanism_support_observation_demands:
        BTreeMap<MechanismRequestId, MechanismSupportObservationDemandLog>,
    /// Replay-derived append-only support observations, partitioned by
    /// mechanism request so each public artifact can stream by ordinal.
    mechanism_support_observations: BTreeMap<MechanismRequestId, MechanismSupportObservationLog>,
    work: RelationalWorkFrontier,
}

/// Durable request-local anchor for the factorized support join. The cursor
/// is part of the receipt: a root alone cannot bound how much work replay is
/// authorized to perform before checking the claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalMechanismSupportCheckpointReceipt {
    cursor: MechanismSupportCheckpointCursor,
    frontier: MechanismSupportFrontierSummary,
}

#[derive(Clone, Debug)]
struct MechanismSupportObservationDemandLog {
    registrations: Vec<MechanismSupportObservationDemandRegistrationClaim>,
    by_slice: BTreeMap<MechanismSupportSlice, usize>,
    chain_root: MechanismSupportObservationDemandChainRoot,
}

impl MechanismSupportObservationDemandLog {
    fn new(request_id: MechanismRequestId) -> Self {
        Self {
            registrations: Vec::new(),
            by_slice: BTreeMap::new(),
            chain_root: mechanism_support_observation_demand_chain_genesis(request_id),
        }
    }

    fn registration(
        &self,
        slice: MechanismSupportSlice,
    ) -> Option<&MechanismSupportObservationDemandRegistrationClaim> {
        let ordinal = *self.by_slice.get(&slice)?;
        self.registrations
            .get(ordinal)
            .filter(|claim| claim.slice() == slice)
    }
}

#[derive(Clone, Debug)]
struct MechanismSupportObservationLog {
    points: Vec<MechanismSupportObservationPoint>,
    first_by_slice: BTreeMap<MechanismSupportSlice, usize>,
    latest_by_slice: BTreeMap<MechanismSupportSlice, LatestMechanismSupportObservation>,
    automatic_point_count: u128,
    automatic_observed_slice_count: u128,
    automatic_sealed_slice_count: u128,
    automatic_sealed_cursor: Option<MechanismSupportSlice>,
    explicit_observed_slice_count: u128,
    explicit_sealed_slice_count: u128,
    chain_root: MechanismSupportObservationChainRoot,
    automatic_chain_root: MechanismSupportObservationChainRoot,
}

#[derive(Clone, Copy, Debug)]
struct LatestMechanismSupportObservation {
    ordinal: usize,
    point_id: MechanismSupportObservationPointId,
}

impl MechanismSupportObservationLog {
    fn new(request_id: MechanismRequestId) -> Self {
        Self {
            points: Vec::new(),
            first_by_slice: BTreeMap::new(),
            latest_by_slice: BTreeMap::new(),
            automatic_point_count: 0,
            automatic_observed_slice_count: 0,
            automatic_sealed_slice_count: 0,
            automatic_sealed_cursor: None,
            explicit_observed_slice_count: 0,
            explicit_sealed_slice_count: 0,
            chain_root: mechanism_support_observation_chain_genesis(request_id),
            automatic_chain_root: mechanism_support_observation_chain_genesis(request_id),
        }
    }

    fn first_point(
        &self,
        slice: MechanismSupportSlice,
    ) -> Option<(usize, &MechanismSupportObservationPoint)> {
        let ordinal = *self.first_by_slice.get(&slice)?;
        self.points
            .get(ordinal)
            .filter(|point| point.slice() == slice)
            .map(|point| (ordinal, point))
    }

    fn latest_point(
        &self,
        slice: MechanismSupportSlice,
    ) -> Option<&MechanismSupportObservationPoint> {
        let latest = self.latest_by_slice.get(&slice)?;
        self.points
            .get(latest.ordinal)
            .filter(|point| point.point_id() == latest.point_id)
    }

    fn all_automatic_slices_are_sealed(&self) -> bool {
        self.automatic_observed_slice_count == self.automatic_sealed_slice_count
    }
}

fn validate_support_frontier_enrichment(
    request_id: MechanismRequestId,
    durable: MechanismSupportFrontierSummary,
    current: MechanismSupportFrontierSummary,
) -> Result<(), RelationalJournalError> {
    if durable.imported_prefix_root() != current.imported_prefix_root()
        || !optional_commitment_is_monotone(durable.target_seal_id(), current.target_seal_id())
        || !optional_commitment_is_monotone(
            durable.incidence_closure_root(),
            current.incidence_closure_root(),
        )
        || !optional_commitment_is_monotone(
            durable.structural_closure_root(),
            current.structural_closure_root(),
        )
    {
        return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
    }
    Ok(())
}

fn optional_commitment_is_monotone<T: Copy + Eq>(prior: Option<T>, next: Option<T>) -> bool {
    match prior {
        Some(prior) => next == Some(prior),
        None => true,
    }
}

fn validate_support_checkpoint_delta(
    request_id: MechanismRequestId,
    current: MechanismSupportCheckpointCursor,
    requested: MechanismSupportCheckpointCursor,
) -> Result<bool, RelationalJournalError> {
    let lanes = [
        (
            "target-discovery",
            current.target_discovery(),
            requested.target_discovery(),
        ),
        (
            "incidence-terminal",
            current.terminal_discovery(),
            requested.terminal_discovery(),
        ),
        (
            "structural-assignment",
            current.structural_assignment(),
            requested.structural_assignment(),
        ),
    ];
    let mut advanced = false;
    for (lane, current, requested) in lanes {
        let delta = requested.checked_sub(current).ok_or(
            RelationalJournalError::SupportCheckpointCursorRegression {
                request_id,
                lane,
                current,
                requested,
            },
        )?;
        if delta > RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA {
            return Err(RelationalJournalError::SupportCheckpointLaneDeltaExceeded {
                request_id,
                lane,
                delta,
                limit: RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA,
            });
        }
        advanced |= delta != 0;
    }
    Ok(advanced)
}

fn validate_support_scheduler_summary(
    request_id: MechanismRequestId,
    cursor: MechanismSupportCheckpointCursor,
    scheduler: MechanismAutomaticObservationSchedulerSummary,
) -> Result<(), RelationalJournalError> {
    let registry = scheduler.registry();
    let dirty = scheduler.dirty();
    if registry.indexed_assignment_count() != cursor.structural_assignment()
        || registry.slice_count() > registry.indexed_assignment_count()
        || dirty.slice_count() > registry.slice_count()
    {
        return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
    }
    Ok(())
}

fn validate_explicit_support_scheduler_summary(
    request_id: MechanismRequestId,
    scheduler: MechanismExplicitObservationSchedulerSummary,
) -> Result<(), RelationalJournalError> {
    let registry = scheduler.registry();
    let pending = scheduler.pending_backfill();
    let dirty = scheduler.dirty();
    let unsealed = scheduler.unsealed();
    if registry
        .ready_slice_count()
        .checked_add(pending.slice_count())
        != Some(registry.slice_count())
        || dirty.slice_count() > registry.ready_slice_count()
        || unsealed.slice_count() > registry.slice_count()
    {
        return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
    }
    Ok(())
}

impl RelationalEvidenceState {
    fn new(
        contract: RelationalJournalContract,
        region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    ) -> Self {
        let mut support = SupportEvidenceCatalogBuilder::new();
        support
            .register_admission(contract.admission_id, contract.relation_id)
            .expect("an empty support catalog accepts its contract admission layer");
        let mut transition_support = RelationalTransitionSupportIndex::new(
            contract.state_schema_id,
            contract.context_schema_id,
            contract.transition_type_id,
        );
        let questions = contract
            .question_ids()
            .iter()
            .copied()
            .map(|question_id| {
                support
                    .register_question(question_id, contract.admission_id)
                    .expect("an empty support catalog accepts every contract question layer");
                assert!(transition_support.register_question(question_id));
                (
                    question_id,
                    QuestionCatalogBuilder::new(
                        contract.relation_id,
                        contract.admission_id,
                        question_id,
                    ),
                )
            })
            .collect();
        Self {
            contract: contract.clone(),
            scheduler_decision_counts: [0; 10],
            region_replay_authority,
            constructor_interner: RelationalConstructorInterner::default(),
            relation: RelationCatalogBuilder::new(contract.relation_id),
            admission: AdmissionCatalogBuilder::new(contract.relation_id, contract.admission_id),
            questions,
            transition_support,
            analysis_plan: None,
            analysis: None,
            support_plan: None,
            source_image_exactness: None,
            source_traversal: None,
            source_relation_exhaustion: None,
            verified_case_chunk_partition: None,
            classified_sweep_progress: None,
            classified_chunk_accumulator: None,
            classified_support_fragment_slots: Vec::new(),
            accepted_classified_fragment_count: 0,
            case_support_discovery: Vec::new(),
            classified_child_by_resolver_node: BTreeMap::new(),
            pending_classified_work_completion_ordinals: BTreeSet::new(),
            pending_selected_run_positions: BTreeSet::new(),
            selected_run_classified_verification_cache: None,
            selected_run_materializations: BTreeMap::new(),
            selected_run_materialization_ids: BTreeMap::new(),
            successor_exhaustion_receipts: BTreeMap::new(),
            support,
            latest_support_frontiers: BTreeMap::new(),
            latest_support_schedulers: BTreeMap::new(),
            latest_explicit_support_schedulers: BTreeMap::new(),
            mechanism_support_observation_demands: BTreeMap::new(),
            mechanism_support_observations: BTreeMap::new(),
            work: RelationalWorkFrontier::new(),
        }
    }

    fn classified_child_resolver_is_open(
        &self,
        chunk_ordinal: usize,
        cell_id: SupportCellId,
        obligation_id: SupportProofObligationId,
    ) -> Result<bool, RelationalJournalError> {
        let resolver_id = classified_child_resolver_node_id(cell_id, obligation_id)?;
        if self.classified_child_by_resolver_node.get(&resolver_id)
            != Some(&(chunk_ordinal, cell_id, obligation_id))
        {
            return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
        }
        Ok(self
            .work
            .get(resolver_id)
            .is_some_and(|node| !node.progress.is_complete()))
    }

    fn question(
        &self,
        question_id: QuestionId,
    ) -> Result<&QuestionCatalogBuilder, RelationalJournalError> {
        self.questions
            .get(&question_id)
            .ok_or(RelationalJournalError::UnknownQuestion { question_id })
    }

    fn question_mut(
        &mut self,
        question_id: QuestionId,
    ) -> Result<&mut QuestionCatalogBuilder, RelationalJournalError> {
        self.questions
            .get_mut(&question_id)
            .ok_or(RelationalJournalError::UnknownQuestion { question_id })
    }

    /// The classified-region optimization still carries one selection
    /// outcome per support run. It is therefore available only when the
    /// contract itself contains exactly the artifact's explicit question;
    /// zero- and plural-question contracts continue through the shared
    /// concrete traversal and never acquire an ambient primary question.
    fn require_single_question_optimization(
        &self,
        question_id: QuestionId,
    ) -> Result<(), RelationalJournalError> {
        match self.contract.question_ids() {
            [registered] if *registered == question_id => Ok(()),
            _ => {
                Err(RelationalJournalError::SingleQuestionOptimizationScopeMismatch { question_id })
            }
        }
    }

    fn root_admission_obligation(
        &self,
    ) -> Option<&SupportCellObligation<AdmissionClassificationClaim>> {
        let plan = self.support_plan.as_ref()?;
        let RelationalRootObligationPlan::CellBacked {
            root_cell_id,
            descriptors,
        } = plan.root_obligations()
        else {
            return None;
        };
        descriptors.iter().find_map(|descriptor| {
            let RelationalStagedObligationDescriptor::Root {
                activation: RelationalObligationActivation::RootCasePopulation,
                obligation: SupportObligationRecord::Admission(obligation),
            } = descriptor
            else {
                return None;
            };
            (obligation.cell_id() == *root_cell_id
                && obligation.claim().admission_id() == plan.admission_id())
            .then_some(obligation)
        })
    }

    fn remint_certified_source_population(
        &self,
    ) -> Result<Option<CertifiedSourcePopulationBinding>, RelationalJournalError> {
        let Some(artifact) = self.source_image_exactness.as_ref() else {
            return Ok(None);
        };
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let verified = reverify_relational_source_image_exactness_artifact(artifact, plan)?;
        let binding = verified.population_binding();
        let planned_source_cell = plan
            .source_rows()
            .cell()
            .ok_or(RelationalJournalError::SourceImageCellMissing)?;
        let source_cell = plan
            .all_cells()
            .iter()
            .find(|cell| cell.id() == planned_source_cell.id())
            .ok_or(RelationalJournalError::SourceImageCellMissing)?;
        if source_cell != planned_source_cell {
            return Err(RelationalJournalError::SourceImageCellMismatch);
        }
        if binding.plan_root() != plan.root()
            || binding.relation_id() != self.relation.relation_id()
            || binding.certificate_id() != artifact.certificate_id()
            || binding.source_cell_id() != source_cell.id()
            || binding.source_materializer_id() != source_cell.materializer_id()
            || binding.exact_cardinality() != artifact.exact_source_cardinality()
        {
            return Err(RelationalJournalError::SourceImageProofBindingMismatch);
        }
        match self.support.cell(binding.source_cell_id()) {
            Some(durable) if durable == source_cell => {}
            Some(_) => return Err(RelationalJournalError::SourceImageCellMismatch),
            None => return Err(RelationalJournalError::SourceImageCellMissing),
        }
        if self
            .support
            .root_obligation_is_open(verified.injectivity().obligation().id())
            != Some(false)
            || self
                .support
                .root_obligation_is_open(verified.exact_cardinality().obligation().id())
                != Some(false)
        {
            return Err(RelationalJournalError::SourceImageCertifiedEvidenceMissing);
        }
        match self
            .support
            .evidence_record(binding.injectivity_evidence_id())
        {
            Some(SupportEvidenceRecord::Injectivity(durable))
                if durable == verified.injectivity() => {}
            Some(_) => return Err(RelationalJournalError::SourceImageCertifiedEvidenceMismatch),
            None => return Err(RelationalJournalError::SourceImageCertifiedEvidenceMissing),
        }
        match self
            .support
            .evidence_record(binding.cardinality_evidence_id())
        {
            Some(SupportEvidenceRecord::Cardinality(durable))
                if durable == verified.exact_cardinality() => {}
            Some(_) => return Err(RelationalJournalError::SourceImageCertifiedEvidenceMismatch),
            None => return Err(RelationalJournalError::SourceImageCertifiedEvidenceMissing),
        }
        Ok(Some(binding))
    }

    fn concrete_source_traversal_has_started(&self) -> bool {
        self.source_traversal
            .as_ref()
            .is_some_and(SourceTraversalAccumulator::has_observations)
            || self.source_relation_exhaustion.is_some()
    }

    fn require_source_traversal_branch_open(&self) -> Result<(), RelationalJournalError> {
        // Accepting the canonical partition chooses the classified branch.
        // Waiting until chunk zero would leave a replay window in which the
        // ordinary source traversal could start and strand the partition's
        // refined child obligations permanently.
        if self.classified_sweep_progress.is_some() {
            return Err(RelationalJournalError::SourceTraversalConflictsWithClassifiedSweep);
        }
        Ok(())
    }

    fn certified_root_admission_decision(&self) -> Option<AdmissionDecision> {
        let obligation = self.root_admission_obligation()?;
        let evidence = self
            .support
            .admission_evidence_for_obligation(obligation.id())?;
        (evidence.obligation() == obligation).then(|| *evidence.conclusion())
    }

    fn ensure_concrete_admission_compatible(
        &self,
        certified: AdmissionDecision,
    ) -> Result<(), RelationalJournalError> {
        let contradictory = match certified {
            AdmissionDecision::Admitted => AdmissionDecision::Rejected,
            AdmissionDecision::Rejected => AdmissionDecision::Admitted,
        };
        if self.admission.contains_decision(contradictory) {
            return Err(
                RelationalJournalError::UniformAdmissionConcreteContradiction {
                    certified,
                    concrete: contradictory,
                },
            );
        }
        Ok(())
    }

    fn apply(&mut self, event: &RelationalJournalEvent) -> Result<(), RelationalJournalError> {
        if self
            .analysis
            .as_ref()
            .is_some_and(RelationalAnalysisJournalState::has_pending_mechanism_artifact)
            && !matches!(
                event,
                RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                    RelationalAnalysisEvidenceEvent::MechanismArtifactChunkAccepted { .. }
                        | RelationalAnalysisEvidenceEvent::MechanismArtifactClosed { .. }
                )) | RelationalJournalEvent::Checkpoint(_)
            )
        {
            return Err(RelationalAnalysisJournalError::MechanismArtifactInterleaving.into());
        }
        match event {
            RelationalJournalEvent::Evidence(event) => self.apply_evidence(event),
            RelationalJournalEvent::Checkpoint(event) => self.apply_checkpoint(event),
        }
    }

    fn apply_evidence(
        &mut self,
        event: &RelationalEvidenceEvent,
    ) -> Result<(), RelationalJournalError> {
        match event {
            RelationalEvidenceEvent::AnalysisPlanRegistered { plan_root, plan } => {
                self.register_analysis_plan(*plan_root, plan)?;
            }
            RelationalEvidenceEvent::SupportPlanRegistered { plan_root, plan } => {
                self.register_support_plan(*plan_root, plan)?;
            }
            RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted { artifact } => {
                self.accept_relational_case_image_injectivity_proof(artifact)?;
            }
            RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted { artifact } => {
                self.accept_relational_source_image_exactness_proof(artifact)?;
            }
            RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted { artifact } => {
                self.accept_relational_case_chunk_partition(artifact)?;
            }
            RelationalEvidenceEvent::RelationalClassifiedChunkAccepted { artifact } => {
                self.accept_relational_classified_chunk(artifact)?;
            }
            RelationalEvidenceEvent::RelationalRegionProofAccepted { artifact } => {
                self.accept_relational_region_proof(artifact)?;
            }
            RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted { artifact } => {
                self.accept_relational_selected_run_materialization(artifact)?;
            }
            RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted { artifact } => {
                self.accept_relational_uniform_admission_proof(artifact)?;
            }
            RelationalEvidenceEvent::SourceTraversalObserved {
                advance_id,
                advance,
            } => {
                self.require_source_traversal_branch_open()?;
                self.observe_source_traversal(*advance_id, advance)?;
            }
            RelationalEvidenceEvent::SourceEnumerationSealed {
                receipt_id,
                receipt,
            } => {
                self.require_source_traversal_branch_open()?;
                self.seal_source_enumeration(*receipt_id, receipt)?;
            }
            RelationalEvidenceEvent::SuccessorDiscovered {
                source_key,
                successor_key,
                case_id,
                row,
            } => {
                let derived_successor =
                    SuccessorKey::derive(self.relation.relation_id(), *source_key, row);
                if derived_successor != *successor_key {
                    return Err(RelationalJournalError::SuccessorKeyClaimMismatch {
                        claimed: *successor_key,
                        derived: derived_successor,
                    });
                }
                let derived_case = RelationalCaseId::derive(
                    self.relation.relation_id(),
                    *source_key,
                    derived_successor,
                );
                if derived_case != *case_id {
                    return Err(RelationalJournalError::CaseIdClaimMismatch {
                        claimed: *case_id,
                        derived: derived_case,
                    });
                }
                let source = self.relation.source_row(*source_key).ok_or(
                    RelationCatalogError::UnknownSource {
                        source_key: *source_key,
                    },
                )?;
                let prepared_transition = self.transition_support.preflight_universe(
                    &self.relation,
                    *case_id,
                    *source_key,
                    source,
                    *successor_key,
                    row,
                )?;
                let prepared_relation = self
                    .relation
                    .preflight_insert_successor(*source_key, row.clone())?;
                debug_assert_eq!(prepared_relation.successor_key(), *successor_key);
                debug_assert_eq!(prepared_relation.case_id(), *case_id);
                self.relation.commit_preflight_successor(prepared_relation);
                self.transition_support.commit_universe(prepared_transition);
            }
            RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted {
                receipt_id,
                receipt,
            } => {
                receipt
                    .validate_identity()
                    .map_err(|_| RelationalJournalError::InvalidSuccessorExhaustionReceipt)?;
                if *receipt_id != receipt.id()
                    || receipt.relation_id() != self.relation.relation_id()
                    || !self.relation.contains_source(receipt.source_key())
                {
                    return Err(RelationalJournalError::InvalidSuccessorExhaustionReceipt);
                }
                match self.successor_exhaustion_receipts.get(receipt_id) {
                    Some(existing) if existing == receipt => {}
                    Some(_) => return Err(RelationalJournalError::ExhaustionReceiptCollision),
                    None => {
                        self.successor_exhaustion_receipts
                            .insert(*receipt_id, receipt.clone());
                    }
                }
            }
            RelationalEvidenceEvent::SuccessorEnumerationSealed {
                source_key,
                receipt_id,
            } => {
                let receipt = self
                    .successor_exhaustion_receipts
                    .get(receipt_id)
                    .ok_or(RelationalJournalError::ExhaustionReceiptMissing)?;
                let discovered = self.relation.successor_count(*source_key)? as u128;
                if receipt.source_key() != *source_key
                    || receipt.relation_id() != self.relation.relation_id()
                    || receipt.emitted_row_count() != discovered
                {
                    return Err(RelationalJournalError::ExhaustionReceiptCoverageMismatch);
                }
                self.relation.seal_successor_enumeration(*source_key)?;
            }
            RelationalEvidenceEvent::AdmissionClassified { case_id, decision } => {
                if let Some(certified) = self.certified_root_admission_decision() {
                    if certified != *decision {
                        return Err(
                            RelationalJournalError::UniformAdmissionConcreteContradiction {
                                certified,
                                concrete: *decision,
                            },
                        );
                    }
                }
                let insert =
                    self.admission
                        .preflight_classify_open(&self.relation, *case_id, *decision)?;
                let prepared_transition = self
                    .transition_support
                    .preflight_admission(*case_id, *decision)?;
                self.transition_support
                    .commit_classification(prepared_transition);
                self.admission
                    .commit_preflight_classification(*case_id, *decision, insert);
            }
            RelationalEvidenceEvent::QuestionClassified {
                question_id,
                case_id,
                decision,
            } => {
                let insert = self.question(*question_id)?.preflight_classify_open(
                    &self.relation,
                    &self.admission,
                    *case_id,
                    *decision,
                )?;
                let prepared_transition = self.transition_support.preflight_question(
                    *question_id,
                    *case_id,
                    *decision,
                )?;
                self.transition_support
                    .commit_classification(prepared_transition);
                self.question_mut(*question_id)?
                    .commit_preflight_classification(*case_id, *decision, insert);
            }
            RelationalEvidenceEvent::Support(event) => {
                self.apply_support_event(event)?;
            }
            RelationalEvidenceEvent::Analysis(event) => {
                self.apply_analysis_event(event)?;
            }
        }
        Ok(())
    }

    fn register_analysis_plan(
        &mut self,
        claimed_root: RelationalAnalysisPlanRoot,
        plan: &RelationalAnalysisPlan,
    ) -> Result<(), RelationalJournalError> {
        if !plan.validate_root() || plan.root() != claimed_root {
            return Err(RelationalJournalError::AnalysisPlanRootMismatch {
                claimed: claimed_root,
                derived: plan.root(),
            });
        }
        if plan.question_ids() != self.contract.question_ids()
            || plan.producer_graph_digest().bytes() != self.contract.analysis_graph_digest()
        {
            return Err(RelationalJournalError::AnalysisPlanScopeMismatch);
        }
        match &self.analysis_plan {
            Some(existing) if existing == plan => {
                if self.analysis.is_none() {
                    return Err(RelationalJournalError::AnalysisStateMissing);
                }
                Ok(())
            }
            Some(existing) => Err(RelationalJournalError::AnalysisPlanReplacement {
                first: existing.root(),
                second: plan.root(),
            }),
            None => {
                let analysis = RelationalAnalysisJournalState::new(plan)?;
                let mut support = self.support.clone();
                for choice in plan.choice_registrations() {
                    support.register_choice(choice.choice_id(), choice.input_question_id())?;
                }
                for registration in plan.layer_registrations() {
                    match registration {
                        RelationalAnalysisLayerRegistration::Result(result) => {
                            let input = match result.input() {
                                RelationalResolvedResultInput::Sources(relation_id) => {
                                    ViewInputId::Sources(relation_id)
                                }
                                RelationalResolvedResultInput::Selected(question_id) => {
                                    ViewInputId::Selected(question_id)
                                }
                                RelationalResolvedResultInput::Choice(choice_id) => {
                                    ViewInputId::Choice(choice_id)
                                }
                                RelationalResolvedResultInput::MechanismIncidence(request_id) => {
                                    ViewInputId::MechanismIncidence(request_id)
                                }
                            };
                            support.register_view(result.view_id(), input)?;
                        }
                        RelationalAnalysisLayerRegistration::Mechanisms(mechanism) => {
                            let question_id = match mechanism.target() {
                                RelationalResolvedMechanismTarget::Selected(question_id) => {
                                    question_id
                                }
                                RelationalResolvedMechanismTarget::Choice(choice_id) => plan
                                    .choice_registration(choice_id)
                                    .ok_or(RelationalJournalError::AnalysisPlanScopeMismatch)?
                                    .input_question_id(),
                            };
                            support
                                .register_mechanism_request(mechanism.request_id(), question_id)?;
                        }
                    }
                }
                self.support = support;
                self.analysis_plan = Some(plan.clone());
                self.analysis = Some(analysis);
                Ok(())
            }
        }
    }

    fn register_support_plan(
        &mut self,
        claimed_root: RelationalSupportPlanRoot,
        plan: &RelationalSupportPlan,
    ) -> Result<(), RelationalJournalError> {
        self.analysis_plan
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisPlanMissing)?;
        if !plan.validate_root() || plan.root() != claimed_root {
            return Err(RelationalJournalError::SupportPlanRootMismatch {
                claimed: claimed_root,
                derived: plan.root(),
            });
        }
        if plan.relation_id() != self.relation.relation_id()
            || plan.admission_id() != self.admission.admission_id()
            || plan.question_ids() != self.contract.question_ids()
        {
            return Err(RelationalJournalError::SupportPlanScopeMismatch);
        }
        match &self.support_plan {
            Some(existing) if existing == plan => return Ok(()),
            Some(existing) => {
                return Err(RelationalJournalError::SupportPlanReplacement {
                    first: existing.root(),
                    second: plan.root(),
                });
            }
            None => {}
        }

        let source_traversal = SourceTraversalAccumulator::for_plan(plan)?;
        let mut support = self.support.clone();
        support.insert_cell_catalog(plan.all_cells().iter().cloned())?;
        match plan.root_obligations() {
            RelationalRootObligationPlan::ResolvedExactEmpty { admission_id } => {
                if *admission_id != self.admission.admission_id() {
                    return Err(RelationalJournalError::SupportPlanScopeMismatch);
                }
            }
            RelationalRootObligationPlan::CellBacked {
                root_cell_id,
                descriptors,
            } => {
                support.declare_known_root_cell(*root_cell_id)?;
                for descriptor in descriptors {
                    if let RelationalStagedObligationDescriptor::Root {
                        activation,
                        obligation,
                    } = descriptor
                    {
                        if *activation != RelationalObligationActivation::RootCasePopulation {
                            return Err(RelationalJournalError::InvalidSupportPlanActivation);
                        }
                        support.declare_root_obligation_record(obligation.clone())?;
                    }
                }
            }
        }
        support.seal_root_frontier()?;
        if matches!(
            plan.root_obligations(),
            RelationalRootObligationPlan::ResolvedExactEmpty { .. }
        ) {
            support.seal_obligation_frontier()?;
            support.seal_catalog()?;
        }
        self.support = support;
        self.source_traversal = Some(source_traversal);
        self.support_plan = Some(plan.clone());
        Ok(())
    }

    fn accept_relational_case_image_injectivity_proof(
        &mut self,
        artifact: &RelationalCaseImageInjectivityProofArtifact,
    ) -> Result<(), RelationalJournalError> {
        let (verified, declared_injectivity, declared_cardinality) = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;
            let verified = reverify_relational_case_image_injectivity_artifact(artifact, plan)?;
            let case_cell = plan
                .cases()
                .cell()
                .ok_or(RelationalJournalError::CaseImageRootInjectivityObligationMissing)?;
            let expected_injectivity = SupportCellObligation::new(
                case_cell,
                InjectiveMappingClaim::new(case_cell.materializer_id()),
            )
            .map_err(RelationalCaseImageInjectivityProofError::from)?;
            let declared_injectivity = match self.support.obligation(expected_injectivity.id()) {
                Some(SupportObligationRecord::Injectivity(declared))
                    if declared == &expected_injectivity
                        && self
                            .support
                            .root_obligation_is_open(expected_injectivity.id())
                            .is_some() =>
                {
                    declared.clone()
                }
                _ => return Err(RelationalJournalError::CaseImageRootInjectivityObligationMissing),
            };
            let declared_cardinality = match artifact.exact_case_cardinality() {
                Some(exact_count) => {
                    let expected = SupportCellObligation::new(case_cell, ExactCardinalityClaim)
                        .map_err(RelationalCaseImageInjectivityProofError::from)?;
                    let declared =
                        match self.support.obligation(expected.id()) {
                            Some(SupportObligationRecord::Cardinality(declared))
                                if declared == &expected
                                    && self
                                        .support
                                        .root_obligation_is_open(expected.id())
                                        .is_some() =>
                            {
                                declared.clone()
                            }
                            _ => return Err(
                                RelationalJournalError::CaseImageRootCardinalityObligationMissing,
                            ),
                        };
                    Some((declared, exact_count))
                }
                None => None,
            };
            (verified, declared_injectivity, declared_cardinality)
        };

        let injectivity = relational_case_image_proof_gateway::injectivity(
            verified.proof(),
            declared_injectivity,
        )
        .map_err(RelationalCaseImageInjectivityProofError::from)?;
        let cardinality = declared_cardinality
            .map(|(obligation, exact_count)| {
                relational_case_image_proof_gateway::cardinality(
                    verified.proof(),
                    obligation,
                    exact_count,
                )
            })
            .transpose()
            .map_err(RelationalCaseImageInjectivityProofError::from)?;

        // One retained producer artifact is the authority for both claims. Use
        // a private catalog clone so replay can never retain only half of that
        // semantic event; resolver completions remain separate, retryable
        // checkpoint frames.
        let mut support = self.support.clone();
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Injectivity(injectivity))
            .apply(&mut support)?;
        if let Some(cardinality) = cardinality {
            SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Cardinality(cardinality))
                .apply(&mut support)?;
        }
        self.support = support;
        Ok(())
    }

    fn accept_relational_source_image_exactness_proof(
        &mut self,
        artifact: &RelationalSourceImageExactnessProofArtifact,
    ) -> Result<(), RelationalJournalError> {
        let (verified, source_cell) = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;
            let verified = reverify_relational_source_image_exactness_artifact(artifact, plan)?;
            let planned_source_cell = plan
                .source_rows()
                .cell()
                .ok_or(RelationalJournalError::SourceImageCellMissing)?;
            let source_cell = plan
                .all_cells()
                .iter()
                .find(|cell| cell.id() == planned_source_cell.id())
                .ok_or(RelationalJournalError::SourceImageCellMissing)?;
            if source_cell != planned_source_cell {
                return Err(RelationalJournalError::SourceImageCellMismatch);
            }
            if source_cell.id() != artifact.source_row_cell_id()
                || source_cell.materializer_id() != artifact.source_materializer_id()
            {
                return Err(RelationalJournalError::SourceImageCellMismatch);
            }
            (verified, source_cell.clone())
        };
        let binding = verified.population_binding();
        if binding.plan_root() != artifact.plan_root()
            || binding.relation_id() != self.relation.relation_id()
            || binding.certificate_id() != artifact.certificate_id()
            || binding.source_cell_id() != source_cell.id()
            || binding.source_materializer_id() != source_cell.materializer_id()
            || binding.exact_cardinality() != artifact.exact_source_cardinality()
        {
            return Err(RelationalJournalError::SourceImageProofBindingMismatch);
        }
        if let Some(existing) = self.source_image_exactness.as_ref() {
            if existing != artifact {
                return Err(
                    RelationalJournalError::SourceImageExactnessProofReplacement {
                        first: existing.certificate_id(),
                        second: artifact.certificate_id(),
                    },
                );
            }
        }

        let injectivity = verified.injectivity().clone();
        let cardinality = verified.exact_cardinality().clone();
        if binding.injectivity_evidence_id() != injectivity.id()
            || binding.cardinality_evidence_id() != cardinality.id()
        {
            return Err(RelationalJournalError::SourceImageProofBindingMismatch);
        }

        // The registered support plan already catalogs this source cell. Keep
        // insertion idempotent as an exact structural check, then add only the
        // two auxiliary obligations. In particular, this event never calls a
        // root-cell declaration API and cannot change the sealed root-cell
        // frontier.
        let mut support = self.support.clone();
        support.insert_cell(source_cell.clone())?;
        if support.cell(source_cell.id()) != Some(&source_cell) {
            return Err(RelationalJournalError::SourceImageCellMismatch);
        }
        support.declare_root_obligation_record(SupportObligationRecord::Injectivity(
            injectivity.obligation().clone(),
        ))?;
        support.declare_root_obligation_record(SupportObligationRecord::Cardinality(
            cardinality.obligation().clone(),
        ))?;
        support.insert_declared_evidence_record(SupportEvidenceRecord::Injectivity(injectivity))?;
        support.insert_declared_evidence_record(SupportEvidenceRecord::Cardinality(cardinality))?;

        self.support = support;
        self.source_image_exactness = Some(artifact.clone());
        Ok(())
    }

    fn accept_relational_case_chunk_partition(
        &mut self,
        artifact: &RelationalCaseChunkPartitionArtifact,
    ) -> Result<(), RelationalJournalError> {
        if self.concrete_source_traversal_has_started() {
            return Err(RelationalJournalError::ClassifiedSweepConflictsWithSourceTraversal);
        }
        let durable_root_injectivity = match self
            .support
            .evidence_record(artifact.injectivity_evidence_id())
        {
            Some(SupportEvidenceRecord::Injectivity(evidence))
                if evidence.id() == artifact.injectivity_evidence_id()
                    && evidence.obligation().cell_id() == artifact.root_cell_id()
                    && evidence.obligation().claim().materializer_id()
                        == artifact.root_materializer_id() =>
            {
                evidence.clone()
            }
            Some(_) => {
                return Err(RelationalJournalError::CaseChunkRootInjectivityEvidenceMismatch);
            }
            None => {
                return Err(RelationalJournalError::CaseChunkRootInjectivityEvidenceMissing);
            }
        };

        let verified = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;
            reverify_relational_case_chunk_partition_artifact(
                artifact,
                plan,
                &durable_root_injectivity,
            )?
        };
        let partition = verified.partition();
        let parent_admission = self
            .root_admission_obligation()
            .cloned()
            .ok_or(RelationalJournalError::CaseChunkRootAdmissionObligationMissing)?;
        let parent_admission_record = match self.support.obligation(parent_admission.id()) {
            Some(SupportObligationRecord::Admission(declared)) if declared == &parent_admission => {
                SupportObligationRecord::Admission(declared.clone())
            }
            _ => {
                return Err(RelationalJournalError::CaseChunkRootAdmissionObligationMissing);
            }
        };

        let child_admissions = partition
            .chunks()
            .iter()
            .map(|chunk| {
                SupportCellObligation::new(
                    chunk.cell(),
                    AdmissionClassificationClaim::new(artifact.admission_id()),
                )
                .map(SupportObligationRecord::Admission)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(RelationalCaseChunkPartitionError::from)?;
        let mut classified_children = BTreeMap::new();
        for (child_ordinal, child) in child_admissions.iter().enumerate() {
            let resolver_id = classified_child_resolver_node_id(child.cell_id(), child.id())?;
            if classified_children
                .insert(resolver_id, (child_ordinal, child.cell_id(), child.id()))
                .is_some()
            {
                return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
            }
        }
        let refinement = SupportObligationRefinement::new(
            &parent_admission_record,
            partition.certificate(),
            child_admissions.iter(),
        )?;
        match self.support.refinement_for_parent(parent_admission.id()) {
            Some(existing) if existing != &refinement => {
                return Err(RelationalJournalError::CaseChunkAdmissionRefinementMismatch);
            }
            Some(_) => {}
            None if self.support.root_obligation_is_open(parent_admission.id()) != Some(true) => {
                return Err(RelationalJournalError::CaseChunkRootAdmissionNotOpen);
            }
            None => {}
        }

        // Register the classified cursor namespace with the accepted proper
        // partition. A generic root cursor predating this authority could
        // otherwise strand or skip a classified prefix.
        let classified_state = match &self.classified_sweep_progress {
            Some(existing) => {
                existing.validate_partition(artifact)?;
                if self.classified_support_fragment_slots.len() != artifact.chunks().len()
                    || self.classified_child_by_resolver_node != classified_children
                {
                    return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
                }
                None
            }
            None => {
                if self
                    .support
                    .latest_cursor(artifact.root_cell_id())
                    .is_some()
                {
                    return Err(RelationalJournalError::CaseChunkRootCursorAlreadyExists);
                }
                let progress = RelationalClassifiedSweepProgress::from_partition(artifact)?;
                let mut slots = Vec::new();
                slots
                    .try_reserve_exact(artifact.chunks().len())
                    .map_err(|_| {
                        RelationalJournalError::ClassifiedChunkArtifactRetentionAllocationFailed
                    })?;
                slots.resize_with(artifact.chunks().len(), || None);
                Some((progress, slots, classified_children))
            }
        };

        // The retained artifact is one semantic event. All derived support
        // mutations happen against a private clone so no valid replay prefix
        // can observe only some children, evidence records, or the admission
        // refinement.
        let mut support = self.support.clone();
        support.insert_cell_catalog(partition.chunks().iter().map(|chunk| chunk.cell().clone()))?;
        support.insert_known_partition(partition.certificate().clone())?;
        for child_ordinal in 0..partition.chunks().len() {
            let evidence =
                relational_case_chunk_partition_gateway::injectivity(&verified, child_ordinal)
                    .map_err(RelationalCaseChunkPartitionError::from)?;
            support.declare_root_obligation_record(SupportObligationRecord::Injectivity(
                evidence.obligation().clone(),
            ))?;
            support
                .insert_declared_evidence_record(SupportEvidenceRecord::Injectivity(evidence))?;
        }
        SupportJournalEvent::obligation_refined(refinement, child_admissions)?
            .apply(&mut support)?;
        self.support = support;
        self.verified_case_chunk_partition = Some(verified);
        if let Some((progress, slots, classified_children)) = classified_state {
            self.classified_sweep_progress = Some(progress);
            self.classified_support_fragment_slots = slots;
            self.classified_child_by_resolver_node = classified_children;
        }
        Ok(())
    }

    fn accept_relational_classified_chunk_slice(
        &mut self,
        artifact: &RelationalClassifiedChunkSliceArtifact,
    ) -> Result<(), RelationalJournalError> {
        if self.concrete_source_traversal_has_started() {
            return Err(RelationalJournalError::ClassifiedSweepConflictsWithSourceTraversal);
        }
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let verified_partition = self
            .verified_case_chunk_partition
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
        if verified_partition.artifact().plan_root() != plan.root()
            || verified_partition.artifact().relation_id() != plan.relation_id()
            || verified_partition.artifact().admission_id() != plan.admission_id()
            || verified_partition.artifact().question_ids() != plan.question_ids()
            || artifact.question_ids() != verified_partition.artifact().question_ids()
            || verified_partition.artifact().id() != artifact.chunk_partition_id()
        {
            return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
        }
        let progress = self
            .classified_sweep_progress
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedSweepProgressMissing)?;
        progress.validate_partition(verified_partition.artifact())?;
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal())
            .map_err(|_| RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        if !matches!(
            self.classified_support_fragment_slots.get(chunk_ordinal),
            Some(None)
        ) {
            return Err(
                RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                    chunk_ordinal: artifact.chunk_ordinal(),
                },
            );
        }
        if let Some(accumulator) = self.classified_chunk_accumulator.as_ref() {
            if accumulator.chunk_ordinal() != artifact.chunk_ordinal() {
                return Err(
                    RelationalJournalError::ClassifiedChunkSliceProgressMismatch {
                        expected_chunk_ordinal: accumulator.chunk_ordinal(),
                        actual_chunk_ordinal: artifact.chunk_ordinal(),
                    },
                );
            }
        }
        let chunk = verified_partition
            .partition()
            .chunks()
            .get(chunk_ordinal)
            .ok_or(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        if self.support.cell(chunk.cell().id()) != Some(chunk.cell()) {
            return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
        }
        let expected_chunk_injectivity =
            relational_case_chunk_partition_gateway::injectivity(verified_partition, chunk_ordinal)
                .map_err(RelationalClassifiedSweepError::from)?;
        let durable_chunk_injectivity = match self
            .support
            .evidence_record(expected_chunk_injectivity.id())
        {
            Some(SupportEvidenceRecord::Injectivity(evidence))
                if evidence == &expected_chunk_injectivity =>
            {
                evidence.clone()
            }
            Some(_) => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMismatch);
            }
            None => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMissing);
            }
        };

        let next = reverify_relational_classified_chunk_slice_artifact(
            artifact,
            plan,
            verified_partition,
            &durable_chunk_injectivity,
            self.classified_chunk_accumulator.as_ref(),
        )?;
        self.classified_chunk_accumulator = Some(next);
        Ok(())
    }

    fn accept_relational_classified_chunk(
        &mut self,
        artifact: &RelationalClassifiedChunkArtifact,
    ) -> Result<(), RelationalJournalError> {
        if self.concrete_source_traversal_has_started() {
            return Err(RelationalJournalError::ClassifiedSweepConflictsWithSourceTraversal);
        }
        let (verified, chunk_admission, run_admissions, run_refinement, finalizes_active_slice) = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;

            // Partition authority was reconstructed exactly once when replay
            // accepted the authenticated partition event and its durable root
            // proof. Later bounded events may index that opaque authority, but
            // must still match its plan and artifact identities.
            let verified_partition = self
                .verified_case_chunk_partition
                .as_ref()
                .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
            if verified_partition.artifact().plan_root() != plan.root()
                || verified_partition.artifact().relation_id() != plan.relation_id()
                || verified_partition.artifact().admission_id() != plan.admission_id()
                || verified_partition.artifact().question_ids() != plan.question_ids()
                || artifact.question_ids() != verified_partition.artifact().question_ids()
                || verified_partition.artifact().id() != artifact.chunk_partition_id()
            {
                return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
            }
            let classified_sweep_progress = self
                .classified_sweep_progress
                .as_ref()
                .ok_or(RelationalJournalError::ClassifiedSweepProgressMissing)?;
            classified_sweep_progress.validate_partition(verified_partition.artifact())?;

            let chunk_ordinal = usize::try_from(artifact.chunk_ordinal())
                .map_err(|_| RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
            let chunk = verified_partition
                .partition()
                .chunks()
                .get(chunk_ordinal)
                .ok_or(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
            if self.support.cell(chunk.cell().id()) != Some(chunk.cell()) {
                return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
            }
            let expected_chunk_injectivity = relational_case_chunk_partition_gateway::injectivity(
                verified_partition,
                chunk_ordinal,
            )
            .map_err(RelationalClassifiedSweepError::from)?;
            let durable_chunk_injectivity = match self
                .support
                .evidence_record(expected_chunk_injectivity.id())
            {
                Some(SupportEvidenceRecord::Injectivity(evidence))
                    if evidence == &expected_chunk_injectivity =>
                {
                    evidence.clone()
                }
                Some(_) => {
                    return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMismatch);
                }
                None => {
                    return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMissing);
                }
            };
            let finalizes_active_slice = match self
                .classified_support_fragment_slots
                .get(chunk_ordinal)
            {
                Some(Some(RelationalClassifiedSupportFragment::Concrete(existing)))
                    if existing == artifact =>
                {
                    false
                }
                Some(Some(_)) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    );
                }
                Some(None) => {
                    let accumulator = self.classified_chunk_accumulator.as_ref().ok_or(
                        RelationalJournalError::ClassifiedChunkSliceAccumulatorMissing {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    )?;
                    if accumulator.chunk_ordinal() != artifact.chunk_ordinal() {
                        return Err(
                            RelationalJournalError::ClassifiedChunkSliceProgressMismatch {
                                expected_chunk_ordinal: accumulator.chunk_ordinal(),
                                actual_chunk_ordinal: artifact.chunk_ordinal(),
                            },
                        );
                    }
                    let expected = finalize_relational_classified_case_chunk(
                        plan,
                        verified_partition,
                        chunk_ordinal,
                        &durable_chunk_injectivity,
                        accumulator,
                    )?;
                    if expected.artifact() != artifact {
                        return Err(
                            RelationalJournalError::ClassifiedChunkSliceFinalArtifactMismatch {
                                chunk_ordinal: artifact.chunk_ordinal(),
                            },
                        );
                    }
                    true
                }
                None => {
                    return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
                }
            };
            let verified = reverify_relational_classified_chunk_artifact(
                artifact,
                plan,
                verified_partition,
                &durable_chunk_injectivity,
            )?;

            // The partition event atomically installed the exact root
            // refinement before publishing this cached authority. Refinements
            // are immutable per parent, so bounded child events need only
            // verify that this durable binding still names the cached
            // partition; rebuilding all sibling obligations would be a
            // quadratic replay tax.
            let root_admission = self
                .root_admission_obligation()
                .cloned()
                .ok_or(RelationalJournalError::ClassifiedChunkRootAdmissionRefinementMissing)?;
            match self.support.refinement_for_parent(root_admission.id()) {
                Some(durable)
                    if durable.partition_id()
                        == verified_partition.partition().certificate().id()
                        && durable.child_obligation_ids().len()
                            == verified_partition.partition().chunks().len() => {}
                Some(_) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkRootAdmissionRefinementMismatch,
                    );
                }
                None => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkRootAdmissionRefinementMissing,
                    );
                }
            }

            let chunk_admission = SupportCellObligation::new(
                chunk.cell(),
                AdmissionClassificationClaim::new(plan.admission_id()),
            )
            .map_err(RelationalClassifiedSweepError::from)?;
            match self.support.obligation(chunk_admission.id()) {
                Some(SupportObligationRecord::Admission(durable))
                    if durable == &chunk_admission => {}
                _ => {
                    return Err(RelationalJournalError::ClassifiedChunkAdmissionObligationMissing);
                }
            }
            let chunk_admission_record =
                SupportObligationRecord::Admission(chunk_admission.clone());
            let run_admissions = verified
                .runs()
                .iter()
                .map(|run| {
                    SupportCellObligation::new(
                        run.cell(),
                        AdmissionClassificationClaim::new(plan.admission_id()),
                    )
                    .map(SupportObligationRecord::Admission)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(RelationalClassifiedSweepError::from)?;
            let run_refinement = verified
                .partition()
                .map(|partition| {
                    SupportObligationRefinement::new(
                        &chunk_admission_record,
                        partition,
                        run_admissions.iter(),
                    )
                })
                .transpose()?;
            match (
                run_refinement.as_ref(),
                self.support.refinement_for_parent(chunk_admission.id()),
            ) {
                (Some(expected), Some(durable)) if expected == durable => {}
                (Some(_), Some(_)) | (None, Some(_)) => {
                    return Err(RelationalJournalError::ClassifiedChunkAdmissionStateMismatch);
                }
                (Some(_), None)
                    if self
                        .support
                        .admission_evidence_for_obligation(chunk_admission.id())
                        .is_some() =>
                {
                    return Err(RelationalJournalError::ClassifiedChunkAdmissionStateMismatch);
                }
                (Some(_), None) | (None, None) => {}
            }

            let mut admitted_selection_activations = BTreeSet::new();
            for descriptor in plan.obligations() {
                if let RelationalStagedObligationDescriptor::SelectionOnAdmitted {
                    activation,
                    question_id,
                } = descriptor
                {
                    if *activation
                        != RelationalObligationActivation::AdmissionDecision(
                            AdmissionDecision::Admitted,
                        )
                        || plan.question_ids().binary_search(question_id).is_err()
                    {
                        return Err(RelationalJournalError::InvalidSupportPlanActivation);
                    }
                    if !admitted_selection_activations.insert(*question_id) {
                        return Err(RelationalJournalError::InvalidSupportPlanActivation);
                    }
                }
            }
            if admitted_selection_activations
                .iter()
                .copied()
                .ne(plan.question_ids().iter().copied())
            {
                return Err(RelationalJournalError::InvalidSupportPlanActivation);
            }

            (
                verified,
                chunk_admission,
                run_admissions,
                run_refinement,
                finalizes_active_slice,
            )
        };

        // Retain the exact accepted producer artifact in its pre-sized sparse
        // canonical slot. Root-prefix order is advanced separately, so a
        // candidate-selected child can expose useful selected runs without
        // claiming that any preceding child has been classified.
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal())
            .map_err(|_| RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        let retain_new_classified_artifact =
            match self.classified_support_fragment_slots.get(chunk_ordinal) {
                Some(Some(RelationalClassifiedSupportFragment::Concrete(existing)))
                    if existing == artifact =>
                {
                    false
                }
                Some(Some(_)) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    );
                }
                Some(None) => true,
                None => {
                    return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
                }
            };
        let retained_classified_artifact = retain_new_classified_artifact
            .then(|| RelationalClassifiedSupportFragment::Concrete(artifact.clone()));
        let classified_resolver_is_open = retain_new_classified_artifact
            && self.classified_child_resolver_is_open(
                chunk_ordinal,
                chunk_admission.cell_id(),
                chunk_admission.id(),
            )?;
        if retain_new_classified_artifact {
            self.case_support_discovery
                .try_reserve(1)
                .map_err(|_| RelationalJournalError::CaseSupportDiscoveryAllocationFailed)?;
        }

        // Derive every bounded gateway payload before beginning the support
        // transaction. The transaction itself then has only established
        // catalog validations and exact-key undo records to manage.
        let run_cells = verified
            .runs()
            .iter()
            .map(|run| run.cell().clone())
            .collect::<Vec<_>>();
        let run_partition = verified.partition().cloned();
        let mut structural_evidence = Vec::new();
        let mut classification_evidence = Vec::new();
        for run_ordinal in 0..verified.runs().len() {
            if verified.bindings()[run_ordinal].injectivity().is_some() {
                let evidence =
                    relational_classified_sweep_gateway::injectivity(&verified, run_ordinal)
                        .map_err(RelationalClassifiedSweepError::from)?;
                structural_evidence.push((
                    SupportObligationRecord::Injectivity(evidence.obligation().clone()),
                    SupportEvidenceRecord::Injectivity(evidence),
                ));
            }

            let cardinality =
                relational_classified_sweep_gateway::cardinality(&verified, run_ordinal)
                    .map_err(RelationalClassifiedSweepError::from)?;
            structural_evidence.push((
                SupportObligationRecord::Cardinality(cardinality.obligation().clone()),
                SupportEvidenceRecord::Cardinality(cardinality),
            ));

            let admission = relational_classified_sweep_gateway::admission(&verified, run_ordinal)
                .map_err(RelationalClassifiedSweepError::from)?;
            let mut selections = Vec::new();
            if verified.runs()[run_ordinal]
                .descriptor()
                .outcome()
                .admission()
                == AdmissionDecision::Admitted
            {
                selections
                    .try_reserve_exact(artifact.question_ids().len())
                    .map_err(|_| SupportEvidenceError::AtomicAppendReservationFailed)?;
                for question_id in artifact.question_ids() {
                    let selection = relational_classified_sweep_gateway::selection(
                        &verified,
                        run_ordinal,
                        *question_id,
                    )
                    .map_err(RelationalClassifiedSweepError::from)?;
                    selections.push((
                        SupportObligationRecord::Selection(selection.obligation().clone()),
                        SupportEvidenceRecord::Selection(selection),
                    ));
                }
            }
            classification_evidence.push((
                SupportEvidenceRecord::Admission(admission),
                selections,
                verified.runs()[run_ordinal].cell().id(),
            ));
        }

        let canonical_refinement = if let Some(refinement) = run_refinement {
            match SupportJournalEvent::obligation_refined(refinement, run_admissions)? {
                SupportJournalEvent::ObligationRefined {
                    refinement,
                    child_obligations,
                    ..
                } => Some((refinement, child_obligations)),
                _ => unreachable!("obligation_refined constructs its matching event variant"),
            }
        } else if verified.runs().len() != 1
            || verified.runs()[0].cell().id() != chunk_admission.cell_id()
        {
            return Err(RelationalJournalError::ClassifiedChunkAdmissionStateMismatch);
        } else {
            None
        };

        // A classified chunk is one semantic event. The bounded undo log
        // preserves its all-or-nothing replay contract while touching only the
        // new run records, rather than cloning the complete accumulated support
        // catalog (twice) for every chunk. Root cursor publication is a
        // separate bounded checkpoint after the canonical slot is occupied.
        let selection_evidence_count = classification_evidence
            .iter()
            .try_fold(0usize, |count, (_, selections, _)| {
                count.checked_add(selections.len())
            })
            .ok_or(SupportEvidenceError::AtomicAppendReservationFailed)?;
        let undo_capacity = verified
            .runs()
            .len()
            .checked_mul(9)
            .and_then(|capacity| capacity.checked_add(3))
            .and_then(|capacity| {
                selection_evidence_count
                    .checked_mul(2)
                    .and_then(|selection_capacity| capacity.checked_add(selection_capacity))
            })
            .ok_or(SupportEvidenceError::AtomicAppendReservationFailed)?;
        let mut support = self.support.begin_append_transaction(undo_capacity)?;
        for cell in run_cells {
            support.insert_known_cell(cell)?;
        }
        if let Some(partition) = run_partition {
            support.insert_known_partition(partition)?;
        }
        for (obligation, evidence) in structural_evidence {
            support.declare_root_obligation_record(obligation)?;
            support.insert_declared_evidence_record(evidence)?;
        }
        if let Some((refinement, child_obligations)) = canonical_refinement {
            support.insert_obligation_refinement_with_children(refinement, child_obligations)?;
        }

        for (admission, selections, cell_id) in classification_evidence {
            support.insert_declared_evidence_record(admission)?;
            for (obligation, evidence) in selections {
                support.declare_root_obligation_record(obligation)?;
                support.insert_declared_evidence_record(evidence)?;
            }
            support.seal_known_leaf(cell_id)?;
        }
        support.commit();
        if finalizes_active_slice {
            self.classified_chunk_accumulator = None;
        }
        if let Some(retained_fragment) = retained_classified_artifact {
            self.classified_support_fragment_slots[chunk_ordinal] = Some(retained_fragment);
            self.accepted_classified_fragment_count = self
                .accepted_classified_fragment_count
                .checked_add(1)
                .expect("occupied classified slots cannot exceed the partition length");
            debug_assert!(
                self.accepted_classified_fragment_count
                    <= self.classified_support_fragment_slots.len()
            );
            if classified_resolver_is_open {
                self.pending_classified_work_completion_ordinals
                    .insert(chunk_ordinal);
            }
            for run in artifact
                .runs()
                .iter()
                .filter(|run| run.outcome().any_selected())
            {
                self.pending_selected_run_positions
                    .insert((chunk_ordinal, run.ordinal()));
            }
            self.case_support_discovery.push(
                RelationalCaseSupportDiscoveryCoordinate::ClassifiedFragment { chunk_ordinal },
            );
        }
        // The acceptance path already paid the full structural reverification
        // cost for this exact content-derived artifact. Retain that authority
        // only as the bounded one-chunk selected-run accelerator; journal
        // replay rebuilds it from the classified event rather than trusting a
        // serialized cache.
        self.selected_run_classified_verification_cache =
            Some(RelationalSelectedRunClassifiedVerificationCache {
                artifact_id: artifact.id(),
                verified,
            });
        Ok(())
    }

    fn accept_relational_region_proof(
        &mut self,
        artifact: &RelationalRegionProofArtifact,
    ) -> Result<(), RelationalJournalError> {
        if self.concrete_source_traversal_has_started() {
            return Err(RelationalJournalError::ClassifiedSweepConflictsWithSourceTraversal);
        }
        let authority = self
            .region_replay_authority
            .as_ref()
            .cloned()
            .ok_or(RelationalJournalError::RegionProofReplayAuthorityMissing)?;
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        self.require_single_question_optimization(artifact.question_id())?;
        if authority.support_plan_root() != plan.root()
            || authority.classification_capsule_id() != artifact.classification_capsule_id()
        {
            return Err(RelationalJournalError::RegionProofReplayAuthorityMismatch);
        }
        let verified_partition = self
            .verified_case_chunk_partition
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
        let progress = self
            .classified_sweep_progress
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedSweepProgressMissing)?;
        progress.validate_partition(verified_partition.artifact())?;
        let RelationalRegionProofSubject::CanonicalChunk {
            partition_artifact_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
        } = artifact.subject()
        else {
            return Err(RelationalJournalError::RegionProofSubjectMismatch);
        };
        if partition_artifact_id != verified_partition.artifact().id()
            || artifact.root_cell_id() != verified_partition.artifact().root_cell_id()
            || verified_partition.artifact().question_ids() != &[artifact.question_id()]
            || artifact.plan_root() != plan.root()
            || artifact.relation_id() != plan.relation_id()
            || artifact.admission_id() != plan.admission_id()
            || plan
                .question_ids()
                .binary_search(&artifact.question_id())
                .is_err()
        {
            return Err(RelationalJournalError::RegionProofSubjectMismatch);
        }
        let chunk_index = usize::try_from(chunk_ordinal)
            .map_err(|_| RelationalJournalError::RegionProofSubjectMismatch)?;
        let chunk = verified_partition
            .partition()
            .chunks()
            .get(chunk_index)
            .ok_or(RelationalJournalError::RegionProofSubjectMismatch)?;
        if chunk.descriptor().id() != chunk_id
            || chunk.descriptor().ordinal() != chunk_ordinal
            || chunk.cell().id() != chunk_cell_id
            || chunk.cell().materializer_id() != chunk_materializer_id
            || chunk.descriptor().interval_start() != artifact.coordinate_start()
            || chunk.descriptor().interval_end_exclusive() != artifact.coordinate_end_exclusive()
            || chunk.descriptor().cardinality() != artifact.case_cardinality()
            || self.support.cell(chunk_cell_id) != Some(chunk.cell())
        {
            return Err(RelationalJournalError::RegionProofSubjectMismatch);
        }

        let expected_chunk_injectivity =
            relational_case_chunk_partition_gateway::injectivity(verified_partition, chunk_index)
                .map_err(RelationalRegionProofError::from)?;
        match self
            .support
            .evidence_record(expected_chunk_injectivity.id())
        {
            Some(SupportEvidenceRecord::Injectivity(durable))
                if durable == &expected_chunk_injectivity => {}
            Some(_) => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMismatch);
            }
            None => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMissing);
            }
        }
        let chunk_admission = SupportCellObligation::new(
            chunk.cell(),
            AdmissionClassificationClaim::new(plan.admission_id()),
        )
        .map_err(RelationalRegionProofError::from)?;
        let chunk_admission_id = chunk_admission.id();
        match self.support.obligation(chunk_admission.id()) {
            Some(SupportObligationRecord::Admission(durable)) if durable == &chunk_admission => {}
            _ => return Err(RelationalJournalError::ClassifiedChunkAdmissionObligationMissing),
        }

        let verified = authority.reverify_canonical_child(artifact, verified_partition)?;
        if !matches!(
            artifact.conclusion(),
            RelationalCertifiedRegionConclusion::Rejected
                | RelationalCertifiedRegionConclusion::AdmittedNotSelected
        ) {
            return Err(RelationalJournalError::RegionProofConclusionUnsupported);
        }
        let cardinality_obligation =
            SupportCellObligation::new(chunk.cell(), ExactCardinalityClaim)
                .map_err(RelationalRegionProofError::from)?;
        let cardinality = relational_region_proof_gateway::cardinality(
            &verified,
            cardinality_obligation.clone(),
            artifact.case_cardinality(),
        )
        .map_err(RelationalRegionProofError::from)?;
        let admission = relational_region_proof_gateway::admission(
            &verified,
            chunk_admission,
            artifact.conclusion().admission(),
        )
        .map_err(RelationalRegionProofError::from)?;
        let selection = match artifact.conclusion().selection() {
            Some(decision) => {
                let obligation = SupportCellObligation::new(
                    chunk.cell(),
                    SelectionClassificationClaim::new(artifact.question_id()),
                )
                .map_err(RelationalRegionProofError::from)?;
                Some((
                    obligation.clone(),
                    relational_region_proof_gateway::selection(&verified, obligation, decision)
                        .map_err(RelationalRegionProofError::from)?,
                ))
            }
            None => None,
        };

        let retain =
            match self.classified_support_fragment_slots.get(chunk_index) {
                Some(Some(RelationalClassifiedSupportFragment::CertifiedZeroSelected(
                    existing,
                ))) if existing == artifact => false,
                Some(Some(_)) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                            chunk_ordinal,
                        },
                    );
                }
                Some(None) => true,
                None => {
                    return Err(RelationalJournalError::RegionProofSubjectMismatch);
                }
            };
        if retain && self.classified_chunk_accumulator.is_some() {
            return Err(RelationalJournalError::RegionProofConflictsWithConcreteSlice);
        }
        let retained = retain
            .then(|| RelationalClassifiedSupportFragment::CertifiedZeroSelected(artifact.clone()));
        let classified_resolver_is_open = retain
            && self.classified_child_resolver_is_open(
                chunk_index,
                chunk_cell_id,
                chunk_admission_id,
            )?;
        if retain {
            self.case_support_discovery
                .try_reserve(1)
                .map_err(|_| RelationalJournalError::CaseSupportDiscoveryAllocationFailed)?;
        }

        let undo_capacity = if selection.is_some() { 9 } else { 6 };
        let mut support = self.support.begin_append_transaction(undo_capacity)?;
        support.declare_root_obligation_record(SupportObligationRecord::Cardinality(
            cardinality_obligation,
        ))?;
        support.insert_declared_evidence_record(SupportEvidenceRecord::Cardinality(cardinality))?;
        support.insert_declared_evidence_record(SupportEvidenceRecord::Admission(admission))?;
        if let Some((obligation, evidence)) = selection {
            support
                .declare_root_obligation_record(SupportObligationRecord::Selection(obligation))?;
            support.insert_declared_evidence_record(SupportEvidenceRecord::Selection(evidence))?;
        }
        support.seal_known_leaf(chunk_cell_id)?;
        support.commit();
        if let Some(retained) = retained {
            self.classified_support_fragment_slots[chunk_index] = Some(retained);
            self.accepted_classified_fragment_count = self
                .accepted_classified_fragment_count
                .checked_add(1)
                .expect("occupied classified slots cannot exceed the partition length");
            debug_assert!(
                self.accepted_classified_fragment_count
                    <= self.classified_support_fragment_slots.len()
            );
            if classified_resolver_is_open {
                self.pending_classified_work_completion_ordinals
                    .insert(chunk_index);
            }
            self.case_support_discovery.push(
                RelationalCaseSupportDiscoveryCoordinate::ClassifiedFragment {
                    chunk_ordinal: chunk_index,
                },
            );
        }
        Ok(())
    }

    fn accept_relational_classified_prefix_advance(
        &mut self,
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_ordinal: u128,
        artifact_digest: [u8; 32],
    ) -> Result<(), RelationalJournalError> {
        let (root_cell, relative_end, advances, chunk_id, interval_start, interval_end_exclusive) = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;
            let verified_partition = self
                .verified_case_chunk_partition
                .as_ref()
                .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
            let progress = self
                .classified_sweep_progress
                .as_ref()
                .ok_or(RelationalJournalError::ClassifiedSweepProgressMissing)?;
            progress.validate_partition(verified_partition.artifact())?;
            if partition_artifact_id != verified_partition.artifact().id() {
                return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
            }

            let chunk_index = usize::try_from(chunk_ordinal).map_err(|_| {
                RelationalJournalError::ClassifiedSweepProgressGap {
                    expected: progress.next_chunk_ordinal(),
                    actual: chunk_ordinal,
                }
            })?;
            let chunk = verified_partition
                .partition()
                .chunks()
                .get(chunk_index)
                .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
            let fragment = self
                .classified_support_fragment_slots
                .get(chunk_index)
                .and_then(Option::as_ref)
                .ok_or(RelationalJournalError::ClassifiedSweepProgressGap {
                    expected: progress.next_chunk_ordinal(),
                    actual: chunk_ordinal,
                })?;
            if fragment.chunk_id() != Some(chunk.descriptor().id())
                || fragment.chunk_ordinal() != chunk.descriptor().ordinal()
                || fragment.chunk_cell_id() != chunk.cell().id()
                || fragment.interval_start() != chunk.descriptor().interval_start()
                || fragment.interval_end_exclusive() != chunk.descriptor().interval_end_exclusive()
                || fragment.exact_case_count() != chunk.descriptor().cardinality()
                || fragment.artifact_digest() != artifact_digest
            {
                return Err(RelationalJournalError::ClassifiedSweepProgressConflict {
                    chunk_ordinal,
                });
            }

            let root_cell = plan
                .cases()
                .cell()
                .ok_or(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch)?;
            if root_cell.id() != progress.root_cell_id()
                || root_cell.materializer_id() != progress.root_materializer_id()
            {
                return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
            }
            let relative_end = fragment
                .interval_end_exclusive()
                .checked_sub(progress.interval_start())
                .ok_or(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch)?;
            let advances = progress.validate_fragment(
                partition_artifact_id,
                chunk.descriptor().id(),
                chunk_ordinal,
                chunk.cell().id(),
                chunk.cell().materializer_id(),
                artifact_digest,
                fragment.interval_start(),
                fragment.interval_end_exclusive(),
            )?;
            (
                root_cell.clone(),
                relative_end,
                advances,
                chunk.descriptor().id(),
                fragment.interval_start(),
                fragment.interval_end_exclusive(),
            )
        };

        let cursor = SupportMaterializationCursor::at_start(&root_cell)
            .and_then(|start| {
                start.advance(
                    &root_cell,
                    relative_end,
                    artifact_digest.to_vec().into_boxed_slice(),
                )
            })
            .map_err(RelationalClassifiedSweepError::from)?;
        if !advances {
            let chunk_index = usize::try_from(chunk_ordinal)
                .map_err(|_| RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
            if self.support.cursor_at(root_cell.id(), relative_end) != Some(&cursor) {
                return Err(RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch);
            }
            return Ok(());
        }

        let progress = self
            .classified_sweep_progress
            .as_ref()
            .expect("prefix preflight required classified progress");
        if self
            .support
            .cursor_at(root_cell.id(), relative_end)
            .is_some()
        {
            return Err(RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch);
        }
        match (
            progress.next_coordinate_ordinal(),
            progress.last_artifact_digest(),
            self.support.latest_cursor(root_cell.id()),
        ) {
            (0, None, None) => {}
            (expected, Some(previous_digest), Some(previous)) => {
                previous
                    .validate_for(&root_cell)
                    .map_err(RelationalClassifiedSweepError::from)?;
                if previous.next_coordinate_ordinal() != expected {
                    return Err(
                        RelationalJournalError::ClassifiedChunkCursorPredecessorMismatch {
                            expected,
                            actual: previous.next_coordinate_ordinal(),
                        },
                    );
                }
                if previous.checkpoint() != previous_digest.as_slice() {
                    return Err(RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch);
                }
            }
            (expected, _, Some(previous)) => {
                return Err(
                    RelationalJournalError::ClassifiedChunkCursorPredecessorMismatch {
                        expected,
                        actual: previous.next_coordinate_ordinal(),
                    },
                );
            }
            (expected, _, None) => {
                return Err(
                    RelationalJournalError::ClassifiedChunkCursorPredecessorMissing { expected },
                );
            }
        }

        self.support.insert_cursor(cursor)?;
        self.classified_sweep_progress
            .as_mut()
            .expect("prefix preflight required classified progress")
            .commit_validated_fragment(
                chunk_id,
                chunk_ordinal,
                artifact_digest,
                interval_start,
                interval_end_exclusive,
            );
        Ok(())
    }

    fn reverify_retained_classified_chunk(
        &self,
        artifact: &RelationalClassifiedChunkArtifact,
    ) -> Result<VerifiedRelationalClassifiedChunk, RelationalJournalError> {
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let verified_partition = self
            .verified_case_chunk_partition
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
        if verified_partition.artifact().plan_root() != plan.root()
            || verified_partition.artifact().relation_id() != plan.relation_id()
            || verified_partition.artifact().admission_id() != plan.admission_id()
            || verified_partition.artifact().question_ids() != plan.question_ids()
            || artifact.question_ids() != verified_partition.artifact().question_ids()
            || verified_partition.artifact().id() != artifact.chunk_partition_id()
        {
            return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
        }
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal())
            .map_err(|_| RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        let chunk = verified_partition
            .partition()
            .chunks()
            .get(chunk_ordinal)
            .ok_or(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        if self.support.cell(chunk.cell().id()) != Some(chunk.cell()) {
            return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
        }
        let expected_chunk_injectivity =
            relational_case_chunk_partition_gateway::injectivity(verified_partition, chunk_ordinal)
                .map_err(RelationalClassifiedSweepError::from)?;
        let durable_chunk_injectivity = match self
            .support
            .evidence_record(expected_chunk_injectivity.id())
        {
            Some(SupportEvidenceRecord::Injectivity(evidence))
                if evidence == &expected_chunk_injectivity =>
            {
                evidence
            }
            Some(_) => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMismatch);
            }
            None => {
                return Err(RelationalJournalError::ClassifiedChunkInjectivityEvidenceMissing);
            }
        };
        reverify_relational_classified_chunk_artifact(
            artifact,
            plan,
            verified_partition,
            durable_chunk_injectivity,
        )
        .map_err(RelationalJournalError::from)
    }

    fn reverify_selected_run_materialization_with_cached_classified_chunk(
        &mut self,
        artifact: &RelationalSelectedRunMaterializationArtifact,
        chunk_ordinal: usize,
        classified_artifact_id: RelationalClassifiedChunkArtifactId,
    ) -> Result<VerifiedRelationalSelectedRunMaterialization, RelationalJournalError> {
        let cache_hit = self
            .selected_run_classified_verification_cache
            .as_ref()
            .is_some_and(|cached| cached.artifact_id == classified_artifact_id);
        if !cache_hit {
            let verified = {
                let classified_artifact = self
                    .classified_support_fragment_slots
                    .get(chunk_ordinal)
                    .and_then(Option::as_ref)
                    .and_then(RelationalClassifiedSupportFragment::concrete)
                    .ok_or(
                        RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    )?;
                if classified_artifact.id() != classified_artifact_id {
                    return Err(RelationalJournalError::SelectedRunClassifiedArtifactMismatch);
                }
                self.reverify_retained_classified_chunk(classified_artifact)?
            };
            self.selected_run_classified_verification_cache =
                Some(RelationalSelectedRunClassifiedVerificationCache {
                    artifact_id: classified_artifact_id,
                    verified,
                });
        }

        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let verified_partition = self
            .verified_case_chunk_partition
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
        let cached = self
            .selected_run_classified_verification_cache
            .as_ref()
            .filter(|cached| cached.artifact_id == classified_artifact_id)
            .ok_or(RelationalJournalError::SelectedRunClassifiedArtifactMismatch)?;
        reverify_relational_selected_run_materialization_artifact(
            artifact,
            plan,
            verified_partition,
            &cached.verified,
            artifact.run_ordinal(),
        )
        .map_err(RelationalJournalError::from)
    }

    fn accept_relational_selected_run_materialization(
        &mut self,
        artifact: &RelationalSelectedRunMaterializationArtifact,
    ) -> Result<(), RelationalJournalError> {
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal()).map_err(|_| {
            RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                chunk_ordinal: artifact.chunk_ordinal(),
            }
        })?;
        let classified_fragment = self
            .classified_support_fragment_slots
            .get(chunk_ordinal)
            .and_then(Option::as_ref)
            .ok_or(
                RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                    chunk_ordinal: artifact.chunk_ordinal(),
                },
            )?;
        let classified_artifact = classified_fragment.concrete().ok_or(
            RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                chunk_ordinal: artifact.chunk_ordinal(),
            },
        )?;
        if classified_artifact.id() != artifact.classified_chunk_artifact_id() {
            return Err(RelationalJournalError::SelectedRunClassifiedArtifactMismatch);
        }
        let classified_artifact_id = classified_artifact.id();
        let verified = self.reverify_selected_run_materialization_with_cached_classified_chunk(
            artifact,
            chunk_ordinal,
            classified_artifact_id,
        )?;
        for question_id in artifact.selected_question_ids() {
            if !self.contract.contains_question(*question_id) {
                return Err(RelationalJournalError::UnknownQuestion {
                    question_id: *question_id,
                });
            }
        }
        let run_cell_id = verified.artifact().run_cell_id();

        if let Some(existing) = self.selected_run_materializations.get(&run_cell_id) {
            return if existing == artifact {
                Ok(())
            } else {
                Err(RelationalJournalError::SelectedRunMaterializationConflict { run_cell_id })
            };
        }
        if self
            .selected_run_materialization_ids
            .contains_key(&artifact.id())
        {
            return Err(
                RelationalJournalError::SelectedRunMaterializationArtifactIdentityCollision {
                    artifact_id: artifact.id(),
                },
            );
        }
        self.case_support_discovery
            .try_reserve(1)
            .map_err(|_| RelationalJournalError::CaseSupportDiscoveryAllocationFailed)?;
        let retained_artifact = artifact.clone();

        // The sparse materializer represents admitted-and-selected cases, so
        // it extends all three semantic transition layers. Stage that bounded
        // authenticated delta independently before any durable catalog is
        // mutated. This makes a transition collision or allocation failure an
        // all-or-nothing rejection alongside the relation/classification
        // batch instead of exposing a partially installed selected witness.
        let mut transition_support = self.transition_support.begin_append_transaction();
        for record in verified.cases().iter() {
            transition_support.insert_universe(
                &self.relation,
                record.case_id(),
                record.source_key(),
                record.source(),
                record.successor_key(),
                record.successor(),
            )?;
            transition_support.classify_admission(record.case_id(), AdmissionDecision::Admitted)?;
            for question_id in artifact.selected_question_ids() {
                transition_support.classify_question(
                    *question_id,
                    record.case_id(),
                    SelectionDecision::Selected,
                )?;
            }
        }

        // Validate against the durable prefixes and build one bounded local
        // relation delta before mutating any concrete catalog. The final merge
        // has no semantic failure path and does not clone the selected prefix.
        // No enumeration seal is minted here: these remain sparse witnesses
        // emerging from an open certified population.
        install_selected_case_batch(
            &mut self.relation,
            &mut self.admission,
            &mut self.questions,
            artifact.selected_question_ids(),
            verified.cases().iter().map(|record| {
                SelectedCaseBatchRow::new(
                    record.source_key(),
                    record.source().clone(),
                    record.successor_key(),
                    record.successor().clone(),
                    record.case_id(),
                )
            }),
        )?;
        transition_support.commit();

        // All semantic conflicts were rejected before the batch merge. These
        // two indexes only publish the already accepted bounded artifact.
        let previous = self
            .selected_run_materializations
            .insert(run_cell_id, retained_artifact);
        debug_assert!(previous.is_none());
        let previous_id = self
            .selected_run_materialization_ids
            .insert(artifact.id(), run_cell_id);
        debug_assert!(previous_id.is_none());
        self.pending_selected_run_positions
            .remove(&(chunk_ordinal, artifact.run_ordinal()));
        self.case_support_discovery.push(
            RelationalCaseSupportDiscoveryCoordinate::SelectedRunMaterialization {
                chunk_ordinal,
                run_ordinal: artifact.run_ordinal(),
            },
        );
        Ok(())
    }

    fn accept_relational_uniform_admission_proof(
        &mut self,
        artifact: &RelationalUniformAdmissionProofArtifact,
    ) -> Result<(), RelationalJournalError> {
        let (verified, declared_obligation, decision) = {
            let plan = self
                .support_plan
                .as_ref()
                .ok_or(RelationalJournalError::SupportPlanMissing)?;
            let verified = reverify_relational_uniform_admission_artifact(artifact, plan)?;
            let case_cell = plan
                .cases()
                .cell()
                .ok_or(RelationalJournalError::UniformAdmissionRootObligationMissing)?;
            let expected = SupportCellObligation::new(
                case_cell,
                AdmissionClassificationClaim::new(plan.admission_id()),
            )
            .map_err(RelationalUniformAdmissionProofError::from)?;
            let declared_obligation = match self.support.obligation(expected.id()) {
                Some(SupportObligationRecord::Admission(declared))
                    if declared == &expected
                        && self
                            .support
                            .root_obligation_is_open(expected.id())
                            .is_some() =>
                {
                    declared.clone()
                }
                _ => return Err(RelationalJournalError::UniformAdmissionRootObligationMissing),
            };
            let decision = *verified.evidence().conclusion();
            (verified, declared_obligation, decision)
        };

        let evidence = relational_uniform_admission_proof_gateway::admission(
            verified.proof(),
            declared_obligation,
            decision,
        )
        .map_err(RelationalUniformAdmissionProofError::from)?;
        self.apply_support_event(&SupportJournalEvent::evidence_accepted(
            SupportEvidenceRecord::Admission(evidence),
        ))
    }

    fn observe_source_traversal(
        &mut self,
        claimed_advance_id: SourceTraversalAdvanceId,
        advance: &RelationalSourceAdvance,
    ) -> Result<(), RelationalJournalError> {
        let terminal_source = match advance {
            RelationalSourceAdvance::Yielded {
                continuation: RelationalSourceContinuation::Source(source),
                ..
            } => Some(source),
            RelationalSourceAdvance::Yielded { .. } | RelationalSourceAdvance::Exhausted { .. } => {
                None
            }
        };
        let traversal = self
            .source_traversal
            .as_mut()
            .ok_or(RelationalJournalError::SourceTraversalMissing)?;
        let prepared = traversal.prepare_claimed_observation(claimed_advance_id, advance)?;
        let inserted = match prepared.observation() {
            SourceTraversalObservation::Yielded { inserted, .. }
            | SourceTraversalObservation::Exhausted { inserted, .. } => inserted,
        };

        if self.source_relation_exhaustion.is_some() && inserted {
            return Err(RelationalJournalError::SourceTraversalAlreadySealed);
        }
        if let Some(source) = terminal_source {
            let derived = SourceKey::derive(self.relation.relation_id(), source.row());
            if derived != source.source_key() {
                return Err(RelationalJournalError::SourceKeyClaimMismatch {
                    claimed: source.source_key(),
                    derived,
                });
            }
            // Relation insertion performs every collision check before
            // mutation. Once source closure is sealed, an identical replay is
            // already present and must not call the closed insert path.
            if inserted || !self.relation.source_enumeration_is_closed() {
                self.relation.insert_source(source.row().clone())?;
            }
        }
        prepared.commit();
        Ok(())
    }

    fn seal_source_enumeration(
        &mut self,
        claimed_receipt_id: SourceRelationExhaustionReceiptId,
        receipt: &SourceRelationExhaustionReceipt,
    ) -> Result<(), RelationalJournalError> {
        receipt.validate_identity()?;
        if claimed_receipt_id != receipt.id()
            || receipt.relation_id() != self.relation.relation_id()
        {
            return Err(RelationalJournalError::InvalidSourceRelationExhaustionReceipt);
        }
        let support_plan_root = self
            .support_plan
            .as_ref()
            .map(RelationalSupportPlan::root)
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        match &self.source_relation_exhaustion {
            Some(existing) if existing == receipt => return Ok(()),
            Some(_) => return Err(RelationalJournalError::SourceRelationExhaustionReplacement),
            None => {}
        }
        let traversal = self
            .source_traversal
            .as_ref()
            .ok_or(RelationalJournalError::SourceTraversalMissing)?;
        if receipt.support_plan_root() != support_plan_root
            || receipt.binding_count() != traversal.binding_count()
        {
            return Err(RelationalJournalError::InvalidSourceRelationExhaustionReceipt);
        }

        // Full-tree validation happens once at closure, never once per edge.
        let expected = traversal.finish()?;
        if &expected != receipt {
            return Err(RelationalJournalError::SourceRelationExhaustionReceiptMismatch);
        }
        // The traversal replay above independently rebuilt all three compact
        // roots/counts from the prior incremental events. The relation catalog
        // is a separate accumulator, so compare its canonical source-key set as
        // a second coverage check before either structure is allowed to close.
        let (discovered_source_key_root, discovered_source_key_count) =
            self.relation.source_key_set_commitment();
        if discovered_source_key_root != receipt.source_key_root()
            || discovered_source_key_count != receipt.source_key_count()
        {
            return Err(RelationalJournalError::SourceRelationCoverageMismatch);
        }

        self.relation.seal_source_enumeration();
        self.source_relation_exhaustion = Some(receipt.clone());
        // The aggregate receipt commits the complete checked traversal tree.
        // Once installed, retaining every prefix, edge, source-value claim and
        // fiber receipt would duplicate potentially millions of records. No
        // later legitimate source work exists after this seal, so replay can
        // fold the same prefix into the compact receipt and release the tree.
        self.source_traversal = None;
        Ok(())
    }

    fn apply_support_event(
        &mut self,
        event: &SupportJournalEvent,
    ) -> Result<(), RelationalJournalError> {
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;

        let activates_selection = match event {
            SupportJournalEvent::EvidenceAccepted {
                evidence: SupportEvidenceRecord::Admission(evidence),
                ..
            } if *evidence.conclusion() == AdmissionDecision::Admitted => {
                Some(evidence.obligation().cell_id())
            }
            _ => None,
        };
        let root_admission_decision = match event {
            SupportJournalEvent::EvidenceAccepted {
                evidence: SupportEvidenceRecord::Admission(evidence),
                ..
            } if self
                .root_admission_obligation()
                .is_some_and(|obligation| obligation == evidence.obligation()) =>
            {
                Some(*evidence.conclusion())
            }
            _ => None,
        };
        let sealing_support = matches!(
            event,
            SupportJournalEvent::ObligationFrontierSealed | SupportJournalEvent::CatalogSealed
        );

        if sealing_support {
            // A sparse classified sweep may have discharged every obligation
            // currently visible in the support catalog while later canonical
            // chunk slots are still absent. Neither semantic seal may freeze
            // that partial catalog: an accepted classified child can need to
            // declare new support roots. Prefix promotion may safely follow a
            // seal because it only advances a cursor over already accepted
            // slots, so require occupancy rather than prefix completion here.
            if self.classified_sweep_progress.is_some()
                && (self.classified_chunk_accumulator.is_some()
                    || self.accepted_classified_fragment_count
                        != self.classified_support_fragment_slots.len())
            {
                return Err(RelationalJournalError::ClassifiedSupportCoveragePending);
            }
            // Sealing changes only one monotone bit, so validate the current
            // immutable prefix before mutation instead of cloning the complete
            // support catalog for rollback. Open-obligation membership and
            // sparse classified occupancy are unchanged by either seal.
            let closure = self.support.validated_closure()?;
            if matches!(event, SupportJournalEvent::ObligationFrontierSealed)
                && closure.has_open_obligation_kind(SupportEvidenceKind::Admission)
            {
                return Err(RelationalJournalError::StagedObligationActivationPending);
            }
            drop(closure);
            event.apply(&mut self.support)?;
            return Ok(());
        }

        if activates_selection.is_none() && root_admission_decision.is_none() {
            event.apply(&mut self.support)?;
            return Ok(());
        }

        let mut support = self.support.clone();
        event.apply(&mut support)?;
        if let Some(decision) = root_admission_decision {
            self.ensure_concrete_admission_compatible(decision)?;
        }
        if let Some(cell_id) = activates_selection {
            let cell = support
                .cell(cell_id)
                .cloned()
                .ok_or(RelationalJournalError::SupportPlanScopeMismatch)?;
            for descriptor in plan.obligations() {
                if let RelationalStagedObligationDescriptor::SelectionOnAdmitted {
                    activation,
                    question_id,
                } = descriptor
                {
                    if *activation
                        != RelationalObligationActivation::AdmissionDecision(
                            AdmissionDecision::Admitted,
                        )
                        || !self.contract.contains_question(*question_id)
                    {
                        return Err(RelationalJournalError::InvalidSupportPlanActivation);
                    }
                    let obligation = SupportObligationRecord::Selection(
                        SupportCellObligation::new(
                            &cell,
                            SelectionClassificationClaim::new(*question_id),
                        )
                        .map_err(|_| RelationalJournalError::InvalidSupportPlanActivation)?,
                    );
                    support.declare_root_obligation_record(obligation)?;
                }
            }
        }
        self.support = support;
        Ok(())
    }

    fn apply_analysis_event(
        &mut self,
        event: &RelationalAnalysisEvidenceEvent,
    ) -> Result<(), RelationalJournalError> {
        if let RelationalAnalysisEvidenceEvent::ResultInputSealedFromSources { seal, .. } = event {
            let expected = self.remint_source_result_input_seal()?;
            if *seal != expected {
                return Err(RelationalJournalError::SourceResultInputSealBaseMismatch);
            }
        }
        if let RelationalAnalysisEvidenceEvent::CertifiedSourceSummaryAccepted {
            view_id,
            artifact,
            ..
        } = event
        {
            let plan = self
                .analysis_plan
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisPlanMissing)?;
            let source = self
                .remint_certified_source_population()?
                .ok_or(RelationalJournalError::SourceImageCertifiedEvidenceMissing)?;
            let spec = self
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?
                .open_catalog()
                .ok_or(RelationalJournalError::AnalysisNotClosed)?
                .result_spec(*view_id)
                .map_err(RelationalAnalysisJournalError::Catalog)?;
            reverify_relational_certified_source_summary_artifact(artifact, plan, spec, source)?;
        }
        if let RelationalAnalysisEvidenceEvent::SelectedQuestionBound { seal, .. } = event {
            let expected =
                self.remint_selected_question_seal(seal.question_id(), seal.authority())?;
            if *seal != expected {
                return Err(RelationalJournalError::SelectedQuestionSealBaseMismatch);
            }
        }
        if let RelationalAnalysisEvidenceEvent::SupportClosed { request_id, .. } = event {
            // Closure consumes no derived suffix. Every independently growing
            // support lane must already be complete and named by the latest
            // durable checkpoint receipt.
            let durable = self
                .latest_support_frontiers
                .get(request_id)
                .copied()
                .ok_or(
                    RelationalJournalError::SupportClosureFrontierCheckpointMissing {
                        request_id: *request_id,
                    },
                )?;
            {
                let analysis = self
                    .analysis
                    .as_mut()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?;
                let (current, available) = analysis.support_checkpoint_cursors(*request_id)?;
                if current != available {
                    return Err(RelationalJournalError::SupportClosureCheckpointIncomplete {
                        request_id: *request_id,
                    });
                }
                if durable.cursor != current {
                    return Err(
                        RelationalJournalError::SupportClosureFrontierCheckpointMissing {
                            request_id: *request_id,
                        },
                    );
                }
                let derived_frontier = analysis.checkpoint_support_frontier(*request_id)?;
                if durable.frontier != derived_frontier {
                    return Err(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch {
                            request_id: *request_id,
                        },
                    );
                }
            }
            let support_already_closed = self
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.mechanism_support_closure(*request_id))
                .is_some();
            if !support_already_closed {
                self.require_support_observation_ready_to_close(*request_id)?;
            }
            self.analysis
                .as_mut()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?
                .apply(event)?;
            return Ok(());
        }
        if matches!(
            event,
            RelationalAnalysisEvidenceEvent::AnalysisClosed { .. }
        ) {
            self.require_all_support_observations_sealed()?;
        }
        self.analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .apply(event)?;
        Ok(())
    }

    fn restore_analysis_support_checkpoint_through(
        &mut self,
        request_id: MechanismRequestId,
        cursor: MechanismSupportCheckpointCursor,
    ) -> Result<usize, RelationalJournalError> {
        let relation = &self.relation;
        Ok(self
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .restore_support_checkpoint_through(request_id, cursor, |case_id| {
                relation.case(case_id)
            })?)
    }

    fn advance_analysis_support_checkpoint_bounded(
        &mut self,
        request_id: MechanismRequestId,
        maximum_cases: NonZeroU16,
    ) -> Result<
        (
            usize,
            MechanismSupportCheckpointCursor,
            MechanismSupportCheckpointCursor,
        ),
        RelationalJournalError,
    > {
        let relation = &self.relation;
        Ok(self
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .advance_support_checkpoint_bounded(request_id, maximum_cases, |case_id| {
                relation.case(case_id)
            })?)
    }

    fn remint_source_result_input_seal(
        &self,
    ) -> Result<RelationalResultInputSeal, RelationalJournalError> {
        let receipt = self
            .source_relation_exhaustion
            .as_ref()
            .ok_or(RelationalJournalError::SourceEnumerationOpen)?;
        if !self.relation.source_enumeration_is_closed() {
            return Err(RelationalJournalError::SourceEnumerationOpen);
        }
        let (source_key_root, source_key_count) = self.relation.source_key_set_commitment();
        if source_key_root != receipt.source_key_root()
            || source_key_count != receipt.source_key_count()
        {
            return Err(RelationalJournalError::SourceRelationCoverageMismatch);
        }
        Ok(RelationalResultInputSeal::from_sources(
            self.relation.relation_id(),
            source_key_root,
            self.relation.source_keys(),
        ))
    }

    /// Independently remint the exact selected-population bridge from this
    /// journal's own base evidence. A valid receipt from another run with the
    /// same QuestionId is not sufficient: the whole extensional or certified
    /// support authority must agree byte-for-byte.
    fn remint_selected_question_seal(
        &self,
        question_id: QuestionId,
        authority: RelationalSelectedPopulationAuthority,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        match authority {
            RelationalSelectedPopulationAuthority::ExtensionalQuestion { .. } => {
                self.remint_extensional_selected_question_seal(question_id)
            }
            RelationalSelectedPopulationAuthority::CertifiedSupport { .. } => {
                self.remint_certified_selected_question_seal(question_id)
            }
        }
    }

    fn remint_extensional_selected_question_seal(
        &self,
        question_id: QuestionId,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        let relation = self.relation.close_borrowed()?;
        let admission = self.admission.close_borrowed(&relation)?;
        let question = self
            .question(question_id)?
            .close_borrowed(&relation, &admission)?;
        Ok(RelationalSelectedQuestionSeal::from_borrowed_closed_question(&question)?)
    }

    fn remint_certified_selected_question_seal(
        &self,
        question_id: QuestionId,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let support = self.support.validated_closure()?;
        if !support.catalog_is_sealed() {
            return Err(RelationalJournalError::SupportCatalogOpen);
        }
        let population = ClosedCertifiedSelectedPopulation::derive_from_validated_support(
            plan,
            &support,
            question_id,
        )?;
        if population.question_id() != question_id {
            return Err(RelationalJournalError::SelectedQuestionSealBaseMismatch);
        }
        let selected_case_ids =
            self.certified_selected_materialization_case_ids(question_id, &population)?;
        Ok(RelationalSelectedQuestionSeal::from_certified_population(
            &population,
            selected_case_ids,
        )?)
    }

    /// Prove that the sparse concrete rows are exactly the selected support
    /// population before exposing their CaseIds to result and mechanism
    /// consumers. Classification support remains the independent population
    /// authority; materialization supplies the real rows, never synthetic
    /// representatives for support cells.
    fn certified_selected_materialization_case_ids(
        &self,
        question_id: QuestionId,
        population: &ClosedCertifiedSelectedPopulation,
    ) -> Result<Vec<RelationalCaseId>, RelationalJournalError> {
        let certified = population.exact_cardinality();
        let question = self.question(question_id)?;
        let catalog = question.selected_count() as u128;

        let Some(progress) = self.classified_sweep_progress.as_ref() else {
            if population.is_exact_empty()
                && self.classified_support_fragment_slots.is_empty()
                && !self
                    .selected_run_materializations
                    .values()
                    .any(|artifact| artifact.contains_question(question_id))
                && catalog == 0
            {
                return Ok(Vec::new());
            }
            return Err(RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen);
        };

        let partition_cardinality = progress
            .interval_end_exclusive()
            .checked_sub(progress.interval_start())
            .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
        if progress.next_coordinate_ordinal() != partition_cardinality
            || progress.committed_prefix_count() != self.classified_support_fragment_slots.len()
            || self
                .classified_support_fragment_slots
                .iter()
                .any(Option::is_none)
        {
            return Err(RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen);
        }

        let mut expected_run_count = 0usize;
        let mut materialized = 0u128;
        let mut all_materialized_cases_are_selected = true;
        for classified in self.classified_support_fragment_slots.iter().flatten() {
            let Some(classified) = classified.concrete() else {
                continue;
            };
            let question_index = classified
                .question_ids()
                .binary_search(&question_id)
                .map_err(|_| RelationalJournalError::UnknownQuestion { question_id })?;
            for run in classified.runs() {
                if run.outcome().selection(question_index) != Some(SelectionDecision::Selected) {
                    continue;
                }
                expected_run_count = expected_run_count
                    .checked_add(1)
                    .ok_or(RelationalJournalError::SequenceOverflow)?;
                let artifact = self
                    .selected_run_materializations
                    .get(&run.cell_id())
                    .ok_or(RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen)?;
                if !artifact.contains_question(question_id) {
                    return Err(
                        RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen,
                    );
                }
                materialized = materialized
                    .checked_add(artifact.materialized_case_count())
                    .ok_or(RelationalJournalError::SequenceOverflow)?;
                all_materialized_cases_are_selected &= artifact.cases().iter().all(|record| {
                    question.decision(record.case_id()) == Some(SelectionDecision::Selected)
                });
            }
        }

        if self
            .selected_run_materializations
            .values()
            .filter(|artifact| artifact.contains_question(question_id))
            .count()
            != expected_run_count
            || !all_materialized_cases_are_selected
            || materialized != certified
            || catalog != certified
        {
            return Err(
                RelationalJournalError::CertifiedSelectedMaterializationCaseSetMismatch {
                    certified,
                    materialized,
                    catalog,
                },
            );
        }

        let selected_case_ids = question.selected_case_ids().collect::<Vec<_>>();
        if selected_case_ids.len() as u128 != certified {
            return Err(
                RelationalJournalError::CertifiedSelectedMaterializationCaseSetMismatch {
                    certified,
                    materialized,
                    catalog: selected_case_ids.len() as u128,
                },
            );
        }
        Ok(selected_case_ids)
    }

    fn validate_closed_analysis_bridge(&self) -> Result<(), RelationalJournalError> {
        let analysis = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        if !analysis.is_closed() || analysis.closed_closure_set_root().is_none() {
            return Err(RelationalJournalError::AnalysisNotClosed);
        }
        for question_id in self.contract.question_ids() {
            let supplied = analysis
                .selected_question(*question_id)
                .ok_or(RelationalJournalError::AnalysisNotClosed)?;
            let expected =
                self.remint_selected_question_seal(*question_id, supplied.authority())?;
            if supplied != expected {
                return Err(RelationalJournalError::SelectedQuestionSealBaseMismatch);
            }
        }
        Ok(())
    }

    fn apply_checkpoint(
        &mut self,
        event: &RelationalCheckpointEvent,
    ) -> Result<(), RelationalJournalError> {
        match event {
            RelationalCheckpointEvent::SchedulerDecisionRecorded {
                policy_version,
                decision,
                nomination_root,
                ..
            } => {
                if *policy_version != RELATIONAL_SCHEDULER_POLICY_VERSION {
                    return Err(RelationalJournalError::SchedulerPolicyVersionMismatch {
                        expected: RELATIONAL_SCHEDULER_POLICY_VERSION,
                        actual: *policy_version,
                    });
                }
                match (
                    decision.requires_candidate_nomination_root(),
                    nomination_root.is_some(),
                ) {
                    (true, false) => {
                        return Err(RelationalJournalError::CandidateNominationRootMissing {
                            decision: *decision,
                        });
                    }
                    (false, true) => {
                        return Err(RelationalJournalError::UnexpectedCandidateNominationRoot {
                            decision: *decision,
                        });
                    }
                    _ => {}
                }
                let count = &mut self.scheduler_decision_counts[usize::from(decision.priority())];
                *count = count
                    .checked_add(1)
                    .ok_or(RelationalJournalError::SequenceOverflow)?;
            }
            RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { artifact } => {
                self.accept_relational_classified_chunk_slice(artifact)?;
            }
            RelationalCheckpointEvent::RelationalClassifiedPrefixAdvanced {
                partition_artifact_id,
                chunk_ordinal,
                artifact_digest,
            } => {
                self.accept_relational_classified_prefix_advance(
                    *partition_artifact_id,
                    *chunk_ordinal,
                    *artifact_digest,
                )?;
            }
            RelationalCheckpointEvent::WorkNodeInserted {
                node_id,
                spec,
                dependencies,
            } => {
                let derived =
                    RelationalWorkFrontier::derive_node_id(spec, dependencies.iter().copied())?;
                if derived != *node_id {
                    return Err(RelationalJournalError::WorkNodeIdClaimMismatch {
                        claimed: *node_id,
                        derived,
                    });
                }
                self.work
                    .insert(spec.clone(), dependencies.iter().copied())?;
            }
            RelationalCheckpointEvent::WorkReadinessMaterialized { node_id, spec } => {
                let derived = RelationalWorkFrontier::derive_node_id(spec, [])?;
                if derived != *node_id {
                    return Err(RelationalJournalError::WorkNodeIdClaimMismatch {
                        claimed: *node_id,
                        derived,
                    });
                }
                self.validate_readiness_subject(spec)?;
                self.work.materialize_ready(spec.clone())?;
            }
            RelationalCheckpointEvent::WorkCursorAdvanced {
                node_id,
                next_member_ordinal,
            } => {
                self.work
                    .advance_next_member(*node_id, *next_member_ordinal)?;
            }
            RelationalCheckpointEvent::SupportMaterializationCheckpointed { cursor } => {
                if self
                    .classified_sweep_progress
                    .as_ref()
                    .is_some_and(|progress| progress.root_cell_id() == cursor.cell_id())
                {
                    return Err(
                        RelationalJournalError::ClassifiedRootMaterializationCheckpointForbidden,
                    );
                }
                self.support.insert_cursor(cursor.clone())?;
            }
            RelationalCheckpointEvent::SupportFrontierCheckpointed {
                request_id,
                cursor,
                frontier_root,
            } => {
                if self
                    .analysis
                    .as_ref()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?
                    .mechanism_support_closure(*request_id)
                    .is_some()
                {
                    return Err(RelationalJournalError::SupportCheckpointAfterClosure {
                        request_id: *request_id,
                    });
                }
                let prior = self.latest_support_frontiers.get(request_id).copied();
                let prior_cursor = prior
                    .map_or_else(MechanismSupportCheckpointCursor::default, |receipt| {
                        receipt.cursor
                    });
                validate_support_checkpoint_delta(*request_id, prior_cursor, *cursor)?;
                let prior_dirty_count = self
                    .latest_support_schedulers
                    .get(request_id)
                    .map_or(0, |scheduler| scheduler.dirty().slice_count());
                let prior_explicit_pending_count = self
                    .latest_explicit_support_schedulers
                    .get(request_id)
                    .map_or(0, |scheduler| scheduler.pending_backfill().slice_count());
                // Normal cursor advancement may not skip dirty work from the
                // durable prefix. Discarded-proposal recovery starts from a
                // clean durable scheduler even when the live derived suffix
                // has already become dirty; same-cursor frontier enrichment
                // never advances a lane.
                if *cursor != prior_cursor
                    && (prior_dirty_count != 0 || prior_explicit_pending_count != 0)
                {
                    return Err(
                        RelationalJournalError::SupportCheckpointObservationPending {
                            request_id: *request_id,
                        },
                    );
                }
                self.restore_analysis_support_checkpoint_through(*request_id, *cursor)?;
                let analysis = self
                    .analysis
                    .as_mut()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?;
                let derived = analysis.checkpoint_support_frontier(*request_id)?;
                if derived.root() != *frontier_root {
                    return Err(RelationalJournalError::SupportFrontierRootClaimMismatch {
                        request_id: *request_id,
                        claimed: *frontier_root,
                        derived: derived.root(),
                    });
                }
                if let Some(prior) = prior.filter(|prior| prior.cursor == *cursor) {
                    validate_support_frontier_enrichment(*request_id, prior.frontier, derived)?;
                }
                let scheduler = analysis
                    .mechanism_support_scheduler_summary(*request_id)
                    .ok_or(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch {
                            request_id: *request_id,
                        },
                    )?;
                let explicit_scheduler = analysis
                    .mechanism_explicit_observation_scheduler_summary(*request_id)
                    .ok_or(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch {
                            request_id: *request_id,
                        },
                    )?;
                validate_explicit_support_scheduler_summary(*request_id, explicit_scheduler)?;
                validate_support_scheduler_summary(*request_id, *cursor, scheduler)?;
                self.latest_support_frontiers.insert(
                    *request_id,
                    RelationalMechanismSupportCheckpointReceipt {
                        cursor: *cursor,
                        frontier: derived,
                    },
                );
                self.latest_support_schedulers
                    .insert(*request_id, scheduler);
                self.latest_explicit_support_schedulers
                    .insert(*request_id, explicit_scheduler);
            }
            RelationalCheckpointEvent::SupportObservationDemandRegistered { claim } => {
                self.accept_support_observation_demand_registration(*claim)?;
            }
            RelationalCheckpointEvent::SupportObservationBackfillCheckpointed { claim } => {
                self.accept_support_observation_backfill(*claim)?;
            }
            RelationalCheckpointEvent::SupportSubjectObserved { claim } => {
                self.accept_mechanism_support_observation(*claim)?;
            }
            RelationalCheckpointEvent::WorkNodeCompleted {
                node_id,
                completion,
            } => {
                let completed_classified_child = self
                    .classified_child_by_resolver_node
                    .get(node_id)
                    .map(|(chunk_ordinal, _, _)| *chunk_ordinal);
                self.validate_completion_reference(completion)?;
                self.work.complete(*node_id, completion.clone())?;
                if let Some(chunk_ordinal) = completed_classified_child {
                    self.pending_classified_work_completion_ordinals
                        .remove(&chunk_ordinal);
                }
            }
            RelationalCheckpointEvent::WorkFrontierCompacted { receipt } => {
                self.work.compact(*receipt)?;
            }
        }
        Ok(())
    }

    fn require_support_extension_anchor(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<RelationalMechanismSupportCheckpointReceipt, RelationalJournalError> {
        let durable = self
            .latest_support_frontiers
            .get(&request_id)
            .copied()
            .ok_or(RelationalJournalError::SupportObservationFrontierMissing { request_id })?;
        let analysis = self
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        if analysis.mechanism_support_checkpoint_cursor(request_id) != durable.cursor {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }
        if analysis.mechanism_support_closure(request_id).is_none() && !analysis.is_closed() {
            let derived = analysis.checkpoint_support_frontier(request_id)?;
            if derived != durable.frontier {
                return Err(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                );
            }
        }
        let automatic = analysis
            .mechanism_support_scheduler_summary(request_id)
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        let explicit = analysis
            .mechanism_explicit_observation_scheduler_summary(request_id)
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        if self.latest_support_schedulers.get(&request_id).copied() != Some(automatic)
            || self
                .latest_explicit_support_schedulers
                .get(&request_id)
                .copied()
                != Some(explicit)
        {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }
        Ok(durable)
    }

    fn accept_support_observation_demand_registration(
        &mut self,
        claim: MechanismSupportObservationDemandRegistrationClaim,
    ) -> Result<(), RelationalJournalError> {
        let request_id = claim.slice().key().request_id();
        let durable = self.require_support_extension_anchor(request_id)?;
        if durable.cursor != claim.cursor() || durable.frontier.root() != claim.frontier_root() {
            return Err(RelationalJournalError::SupportObservationFrontierMismatch { request_id });
        }
        if self
            .mechanism_support_observation_demands
            .get(&request_id)
            .and_then(|log| log.registration(claim.slice()))
            .is_some()
        {
            return Err(
                RelationalJournalError::SupportObservationDemandAlreadyRegistered { request_id },
            );
        }
        let durable_scheduler = self
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        validate_explicit_support_scheduler_summary(request_id, durable_scheduler)?;
        if durable_scheduler != claim.prior_scheduler() {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }

        let prepared = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .prepare_explicit_support_observation_registration(claim.slice())?;
        let expected = MechanismSupportObservationDemandRegistrationClaim::new(
            prepared.slice(),
            durable.cursor,
            durable.frontier.root(),
            prepared.disposition(),
            prepared.registration_phase(),
            prepared.registration_structural_cursor(),
            prepared.prior_scheduler_summary(),
            prepared.next_scheduler_summary(),
        );
        if claim != expected
            || claim.version() != MECHANISM_SUPPORT_OBSERVATION_DEMAND_REGISTRATION_VERSION
        {
            return Err(
                RelationalJournalError::SupportObservationDemandClaimMismatch { request_id },
            );
        }

        let mut prepared_log = if self
            .mechanism_support_observation_demands
            .contains_key(&request_id)
        {
            self.mechanism_support_observation_demands
                .get_mut(&request_id)
                .expect("checked explicit observation demand log remains present")
                .registrations
                .try_reserve(1)
                .map_err(
                    |_| RelationalJournalError::SupportObservationPointAllocationFailed {
                        request_id,
                    },
                )?;
            None
        } else {
            let mut log = MechanismSupportObservationDemandLog::new(request_id);
            log.registrations.try_reserve(1).map_err(|_| {
                RelationalJournalError::SupportObservationPointAllocationFailed { request_id }
            })?;
            Some(log)
        };
        let ordinal = self
            .mechanism_support_observation_demands
            .get(&request_id)
            .map_or(0, |log| log.registrations.len());
        self.analysis
            .as_mut()
            .expect("prepared explicit observation registration retains analysis state")
            .commit_explicit_support_observation_registration(prepared);
        let committed = self
            .analysis
            .as_ref()
            .expect("committed explicit observation registration retains analysis state")
            .mechanism_explicit_observation_scheduler_summary(request_id)
            .expect("prepared explicit observation registration retains its scheduler");
        assert_eq!(committed, claim.next_scheduler());
        self.latest_explicit_support_schedulers
            .insert(request_id, committed);
        if let Some(log) = prepared_log.take() {
            let previous = self
                .mechanism_support_observation_demands
                .insert(request_id, log);
            debug_assert!(previous.is_none());
        }
        let log = self
            .mechanism_support_observation_demands
            .get_mut(&request_id)
            .expect("reserved explicit observation demand log remains installed");
        let previous = log.by_slice.insert(claim.slice(), ordinal);
        debug_assert!(previous.is_none());
        log.chain_root = extend_mechanism_support_observation_demand_chain(
            request_id,
            log.chain_root,
            ordinal as u128,
            claim,
        );
        log.registrations.push(claim);
        Ok(())
    }

    fn accept_support_observation_backfill(
        &mut self,
        claim: MechanismSupportObservationBackfillClaim,
    ) -> Result<(), RelationalJournalError> {
        let request_id = claim.slice().key().request_id();
        let durable = self.require_support_extension_anchor(request_id)?;
        if durable.cursor != claim.cursor() || durable.frontier.root() != claim.frontier_root() {
            return Err(RelationalJournalError::SupportObservationFrontierMismatch { request_id });
        }
        if self
            .mechanism_support_observation_demands
            .get(&request_id)
            .and_then(|log| log.registration(claim.slice()))
            .is_none()
        {
            return Err(RelationalJournalError::SupportObservationDemandMissing { request_id });
        }
        let durable_scheduler = self
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        validate_explicit_support_scheduler_summary(request_id, durable_scheduler)?;
        if durable_scheduler != claim.prior_scheduler() {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }
        let delta = claim
            .through_structural_cursor()
            .checked_sub(claim.from_structural_cursor())
            .filter(|delta| *delta != 0 && *delta <= RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA)
            .and_then(|delta| u16::try_from(delta).ok())
            .and_then(NonZeroU16::new)
            .ok_or(
                RelationalJournalError::SupportObservationBackfillClaimMismatch { request_id },
            )?;
        let prepared = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .prepare_next_explicit_support_observation_backfill(request_id, delta)?
            .ok_or(
                RelationalJournalError::SupportObservationBackfillClaimMismatch { request_id },
            )?;
        let expected = MechanismSupportObservationBackfillClaim::new(
            prepared.slice(),
            durable.cursor,
            durable.frontier.root(),
            prepared.registration_phase(),
            prepared.registration_structural_cursor(),
            prepared.from_structural_cursor(),
            prepared.through_structural_cursor(),
            prepared.completed(),
            prepared.prior_scheduler_summary(),
            prepared.next_scheduler_summary(),
        );
        if claim != expected || claim.version() != MECHANISM_SUPPORT_OBSERVATION_BACKFILL_VERSION {
            return Err(
                RelationalJournalError::SupportObservationBackfillClaimMismatch { request_id },
            );
        }
        self.analysis
            .as_mut()
            .expect("prepared explicit observation backfill retains analysis state")
            .commit_explicit_support_observation_backfill(prepared);
        let committed = self
            .analysis
            .as_ref()
            .expect("committed explicit observation backfill retains analysis state")
            .mechanism_explicit_observation_scheduler_summary(request_id)
            .expect("prepared explicit observation backfill retains its scheduler");
        assert_eq!(committed, claim.next_scheduler());
        self.latest_explicit_support_schedulers
            .insert(request_id, committed);
        Ok(())
    }

    fn accept_mechanism_support_observation(
        &mut self,
        claim: MechanismSupportObservationClaim,
    ) -> Result<(), RelationalJournalError> {
        if !claim.validate_identity() {
            return Err(RelationalJournalError::SupportObservationPointIdentityMismatch);
        }
        let request_id = claim.slice().key().request_id();
        let durable = self.require_support_extension_anchor(request_id)?;
        if durable.cursor != claim.cursor() || durable.frontier.root() != claim.frontier_root() {
            return Err(RelationalJournalError::SupportObservationFrontierMismatch { request_id });
        }
        let is_automatic = matches!(
            claim.slice().subject(),
            MechanismSupportSubject::Mechanism(_)
        );
        let (prior, first_exists, observed_slice_count, sealed_slice_count, point_ordinal) =
            match self.mechanism_support_observations.get(&request_id) {
                Some(log) => {
                    if log
                        .automatic_observed_slice_count
                        .checked_add(log.explicit_observed_slice_count)
                        != Some(log.first_by_slice.len() as u128)
                        || log.first_by_slice.len() != log.latest_by_slice.len()
                        || log.automatic_sealed_slice_count > log.automatic_observed_slice_count
                        || log.explicit_sealed_slice_count > log.explicit_observed_slice_count
                        || (log.automatic_sealed_cursor.is_none()
                            != (log.automatic_sealed_slice_count == 0))
                    {
                        return Err(RelationalJournalError::SupportObservationCountOverflow {
                            request_id,
                        });
                    }
                    (
                        log.latest_point(claim.slice())
                            .map(|point| (point.claim(), point.summary().root())),
                        log.first_by_slice.contains_key(&claim.slice()),
                        if is_automatic {
                            log.automatic_observed_slice_count
                        } else {
                            log.explicit_observed_slice_count
                        },
                        if is_automatic {
                            log.automatic_sealed_slice_count
                        } else {
                            log.explicit_sealed_slice_count
                        },
                        log.points.len(),
                    )
                }
                None => (None, false, 0, 0, 0),
            };
        if first_exists != prior.is_some() {
            return Err(RelationalJournalError::SupportObservationCountOverflow { request_id });
        }

        let expected_status = {
            let analysis = self
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?;
            analysis.mechanism_support_closure(request_id).map_or(
                MechanismSupportObservationStatus::Open,
                |closure| MechanismSupportObservationStatus::Sealed {
                    support_root: closure.root(),
                },
            )
        };
        if claim.status() != expected_status {
            return Err(RelationalJournalError::SupportObservationStatusMismatch { request_id });
        }

        let expected_slice = if is_automatic {
            let analysis = self
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?;
            let sealed_cursor = self
                .mechanism_support_observations
                .get(&request_id)
                .and_then(|log| log.automatic_sealed_cursor);
            let slice = if expected_status.is_sealed() {
                analysis.next_support_observation_slice_after(request_id, sealed_cursor)?
            } else {
                analysis.next_dirty_support_observation_slice(request_id)?
            };
            let scheduler = analysis
                .mechanism_support_scheduler_summary(request_id)
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            validate_support_scheduler_summary(request_id, claim.cursor(), scheduler)?;
            if self.latest_support_schedulers.get(&request_id).copied() != Some(scheduler) {
                return Err(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                );
            }
            slice
        } else {
            if self
                .mechanism_support_observation_demands
                .get(&request_id)
                .and_then(|log| log.registration(claim.slice()))
                .is_none()
            {
                return Err(RelationalJournalError::SupportObservationDemandMissing { request_id });
            }
            let analysis = self
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?;
            let scheduler = analysis
                .mechanism_explicit_observation_scheduler_summary(request_id)
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            validate_explicit_support_scheduler_summary(request_id, scheduler)?;
            if self
                .latest_explicit_support_schedulers
                .get(&request_id)
                .copied()
                != Some(scheduler)
            {
                return Err(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                );
            }
            if expected_status.is_sealed() {
                analysis.next_unsealed_explicit_support_observation_slice(request_id)
            } else {
                analysis.next_dirty_explicit_support_observation_slice(request_id)
            }
        };
        if expected_slice != Some(claim.slice()) {
            return Err(RelationalJournalError::SupportObservationSliceNotScheduled { request_id });
        }
        let summary = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .derive_support_observation_summary(claim.slice(), claim.cursor(), durable.frontier)?;
        if summary.root() != claim.summary_root() {
            return Err(RelationalJournalError::SupportObservationSummaryMismatch { request_id });
        }

        let expected_supersedes = prior.map(|(prior_claim, _)| prior_claim.point_id());
        if claim.supersedes() != expected_supersedes {
            return Err(
                RelationalJournalError::SupportObservationSupersedesMismatch { request_id },
            );
        }
        if prior.is_some_and(|(prior_claim, _)| prior_claim.status().is_sealed()) {
            return Err(RelationalJournalError::SupportObservationAfterSeal { request_id });
        }
        if is_automatic
            && claim.status().is_sealed()
            && !prior.is_some_and(|(prior_claim, _)| {
                prior_claim.status() == MechanismSupportObservationStatus::Open
            })
        {
            return Err(
                RelationalJournalError::SupportObservationSealPredecessorMissing { request_id },
            );
        }
        if prior.is_some_and(|(prior_claim, prior_summary_root)| {
            prior_claim.cursor() == claim.cursor()
                && prior_claim.frontier_root() == claim.frontier_root()
                && prior_summary_root == summary.root()
                && prior_claim.status() == claim.status()
        }) {
            return Err(RelationalJournalError::SupportObservationDidNotAdvance { request_id });
        }

        let next_observed_slice_count = if prior.is_none() {
            observed_slice_count
                .checked_add(1)
                .ok_or(RelationalJournalError::SupportObservationCountOverflow { request_id })?
        } else {
            observed_slice_count
        };
        let next_sealed_slice_count = if claim.status().is_sealed() {
            sealed_slice_count
                .checked_add(1)
                .ok_or(RelationalJournalError::SupportObservationCountOverflow { request_id })?
        } else {
            sealed_slice_count
        };
        let next_automatic_point_count = if is_automatic {
            self.mechanism_support_observations
                .get(&request_id)
                .map_or(0, |log| log.automatic_point_count)
                .checked_add(1)
                .ok_or(RelationalJournalError::SupportObservationCountOverflow { request_id })?
        } else {
            0
        };

        let mut prepared_log = if self
            .mechanism_support_observations
            .contains_key(&request_id)
        {
            self.mechanism_support_observations
                .get_mut(&request_id)
                .expect("checked observation log remains present")
                .points
                .try_reserve(1)
                .map_err(
                    |_| RelationalJournalError::SupportObservationPointAllocationFailed {
                        request_id,
                    },
                )?;
            None
        } else {
            let mut log = MechanismSupportObservationLog::new(request_id);
            log.points.try_reserve(1).map_err(|_| {
                RelationalJournalError::SupportObservationPointAllocationFailed { request_id }
            })?;
            Some(log)
        };

        let point = MechanismSupportObservationPoint { claim, summary };
        if is_automatic {
            let scheduler = self
                .latest_support_schedulers
                .get(&request_id)
                .copied()
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            let prepared_ack = if claim.status() == MechanismSupportObservationStatus::Open {
                Some(
                    self.analysis
                        .as_ref()
                        .ok_or(RelationalJournalError::AnalysisStateMissing)?
                        .prepare_support_observation_ack(claim.slice())?,
                )
            } else {
                None
            };
            let expected_committed_dirty = prepared_ack
                .as_ref()
                .map_or(scheduler.dirty(), |ack| ack.next_dirty_summary());
            if let Some(ack) = prepared_ack {
                if ack.prior_dirty_summary() != scheduler.dirty() {
                    return Err(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                    );
                }
                self.analysis
                    .as_mut()
                    .expect("prepared support-observation acknowledgement retains analysis state")
                    .commit_support_observation_ack(ack);
            }
            let committed = self
                .analysis
                .as_ref()
                .expect("validated support observation retains analysis state")
                .mechanism_support_scheduler_summary(request_id)
                .expect("validated support observation retains its scheduler");
            assert_eq!(committed.registry(), scheduler.registry());
            assert_eq!(committed.dirty(), expected_committed_dirty);
            self.latest_support_schedulers.insert(request_id, committed);
        } else {
            let scheduler = self
                .latest_explicit_support_schedulers
                .get(&request_id)
                .copied()
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            let next_scheduler = if claim.status() == MechanismSupportObservationStatus::Open {
                let ack = self
                    .analysis
                    .as_ref()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?
                    .prepare_explicit_support_observation_ack(claim.slice())?;
                if ack.prior_scheduler_summary() != scheduler {
                    return Err(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                    );
                }
                let next = ack.next_scheduler_summary();
                self.analysis
                    .as_mut()
                    .expect("prepared explicit observation acknowledgement retains analysis state")
                    .commit_explicit_support_observation_ack(ack);
                next
            } else {
                let ack = self
                    .analysis
                    .as_ref()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?
                    .prepare_explicit_support_observation_seal_ack(claim.slice())?;
                if ack.prior_scheduler_summary() != scheduler {
                    return Err(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                    );
                }
                let next = ack.next_scheduler_summary();
                self.analysis
                    .as_mut()
                    .expect("prepared explicit observation seal retains analysis state")
                    .commit_explicit_support_observation_seal_ack(ack);
                next
            };
            let committed = self
                .analysis
                .as_ref()
                .expect("validated explicit observation retains analysis state")
                .mechanism_explicit_observation_scheduler_summary(request_id)
                .expect("validated explicit observation retains its scheduler");
            assert_eq!(committed, next_scheduler);
            self.latest_explicit_support_schedulers
                .insert(request_id, committed);
        }
        if let Some(log) = prepared_log.take() {
            let previous = self.mechanism_support_observations.insert(request_id, log);
            debug_assert!(previous.is_none());
        }
        let log = self
            .mechanism_support_observations
            .get_mut(&request_id)
            .expect("reserved support-observation log is installed before commit");
        log.chain_root = extend_mechanism_support_observation_chain(
            request_id,
            log.chain_root,
            point_ordinal as u128,
            point.point_id(),
        );
        if is_automatic {
            log.automatic_chain_root = extend_mechanism_support_observation_chain(
                request_id,
                log.automatic_chain_root,
                log.automatic_point_count,
                point.point_id(),
            );
            log.automatic_point_count = next_automatic_point_count;
        }
        if prior.is_none() {
            let previous = log.first_by_slice.insert(point.slice(), point_ordinal);
            debug_assert!(previous.is_none());
        }
        log.latest_by_slice.insert(
            point.slice(),
            LatestMechanismSupportObservation {
                ordinal: point_ordinal,
                point_id: point.point_id(),
            },
        );
        if is_automatic {
            log.automatic_observed_slice_count = next_observed_slice_count;
            if point.status().is_sealed() {
                log.automatic_sealed_slice_count = next_sealed_slice_count;
                log.automatic_sealed_cursor = Some(point.slice());
            }
        } else {
            log.explicit_observed_slice_count = next_observed_slice_count;
            if point.status().is_sealed() {
                log.explicit_sealed_slice_count = next_sealed_slice_count;
            }
        }
        log.points.push(point);
        Ok(())
    }

    fn next_mechanism_support_observation_claim(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<Option<MechanismSupportObservationClaim>, RelationalJournalError> {
        let Some(durable) = self.latest_support_frontiers.get(&request_id).copied() else {
            return Ok(None);
        };
        let analysis = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        let status = analysis.mechanism_support_closure(request_id).map_or(
            MechanismSupportObservationStatus::Open,
            |closure| MechanismSupportObservationStatus::Sealed {
                support_root: closure.root(),
            },
        );
        let log = self.mechanism_support_observations.get(&request_id);
        let sealed_cursor = log.and_then(|log| log.automatic_sealed_cursor);
        let slice = if status.is_sealed() {
            analysis.next_support_observation_slice_after(request_id, sealed_cursor)?
        } else {
            analysis.next_dirty_support_observation_slice(request_id)?
        };
        let Some(slice) = slice else {
            return Ok(None);
        };
        let summary =
            analysis.derive_support_observation_summary(slice, durable.cursor, durable.frontier)?;
        let prior = self
            .mechanism_support_observations
            .get(&request_id)
            .and_then(|log| log.latest_point(slice));
        if prior.is_some_and(|point| point.status().is_sealed()) {
            return Err(RelationalJournalError::SupportObservationAfterSeal { request_id });
        }
        if prior.is_some_and(|point| {
            point.claim().cursor() == durable.cursor
                && point.claim().frontier_root() == durable.frontier.root()
                && point.summary().root() == summary.root()
                && point.status() == status
        }) {
            return Err(RelationalJournalError::SupportObservationDidNotAdvance { request_id });
        }
        if status.is_sealed()
            && !prior.is_some_and(|point| point.status() == MechanismSupportObservationStatus::Open)
        {
            return Err(
                RelationalJournalError::SupportObservationSealPredecessorMissing { request_id },
            );
        }
        Ok(Some(MechanismSupportObservationClaim::new(
            slice,
            durable.cursor,
            durable.frontier.root(),
            summary.root(),
            status,
            prior.map(MechanismSupportObservationPoint::point_id),
        )))
    }

    fn next_explicit_mechanism_support_observation_claim(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<Option<MechanismSupportObservationClaim>, RelationalJournalError> {
        let Some(durable) = self.latest_support_frontiers.get(&request_id).copied() else {
            return Ok(None);
        };
        let analysis = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        let scheduler = analysis
            .mechanism_explicit_observation_scheduler_summary(request_id)
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        validate_explicit_support_scheduler_summary(request_id, scheduler)?;
        if self
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
            != Some(scheduler)
        {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }
        if scheduler.pending_backfill().slice_count() != 0 {
            return Ok(None);
        }
        let status = analysis.mechanism_support_closure(request_id).map_or(
            MechanismSupportObservationStatus::Open,
            |closure| MechanismSupportObservationStatus::Sealed {
                support_root: closure.root(),
            },
        );
        let slice = if status.is_sealed() {
            analysis.next_unsealed_explicit_support_observation_slice(request_id)
        } else {
            analysis.next_dirty_explicit_support_observation_slice(request_id)
        };
        let Some(slice) = slice else {
            return Ok(None);
        };
        if self
            .mechanism_support_observation_demands
            .get(&request_id)
            .and_then(|log| log.registration(slice))
            .is_none()
        {
            return Err(RelationalJournalError::SupportObservationDemandMissing { request_id });
        }
        let summary =
            analysis.derive_support_observation_summary(slice, durable.cursor, durable.frontier)?;
        let prior = self
            .mechanism_support_observations
            .get(&request_id)
            .and_then(|log| log.latest_point(slice));
        if prior.is_some_and(|point| point.status().is_sealed()) {
            return Err(RelationalJournalError::SupportObservationAfterSeal { request_id });
        }
        if prior.is_some_and(|point| {
            point.claim().cursor() == durable.cursor
                && point.claim().frontier_root() == durable.frontier.root()
                && point.summary().root() == summary.root()
                && point.status() == status
        }) {
            return Err(RelationalJournalError::SupportObservationDidNotAdvance { request_id });
        }
        Ok(Some(MechanismSupportObservationClaim::new(
            slice,
            durable.cursor,
            durable.frontier.root(),
            summary.root(),
            status,
            prior.map(MechanismSupportObservationPoint::point_id),
        )))
    }

    fn require_support_observation_ready_to_close(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<(), RelationalJournalError> {
        let durable = self
            .latest_support_frontiers
            .get(&request_id)
            .ok_or(RelationalJournalError::SupportObservationFrontierMissing { request_id })?;
        let scheduler = self
            .latest_support_schedulers
            .get(&request_id)
            .copied()
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        validate_support_scheduler_summary(request_id, durable.cursor, scheduler)?;
        let analysis_scheduler = self
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .mechanism_support_scheduler_summary(request_id)
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        let observed_slice_count = self
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.automatic_observed_slice_count);
        if scheduler != analysis_scheduler
            || scheduler.dirty().slice_count() != 0
            || observed_slice_count != scheduler.registry().slice_count()
        {
            return Err(RelationalJournalError::SupportObservationClosurePending { request_id });
        }
        Ok(())
    }

    fn require_all_support_observations_sealed(&self) -> Result<(), RelationalJournalError> {
        let plan = self
            .analysis_plan
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisPlanMissing)?;
        for registration in plan.layer_registrations() {
            let RelationalAnalysisLayerRegistration::Mechanisms(registration) = registration else {
                continue;
            };
            let request_id = registration.request_id();
            let scheduler = self
                .latest_support_schedulers
                .get(&request_id)
                .copied()
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            let registry_count = scheduler.registry().slice_count();
            let (observed_count, sealed_count, sealed_cursor, all_slices_are_sealed) = self
                .mechanism_support_observations
                .get(&request_id)
                .map_or((0, 0, None, true), |log| {
                    (
                        log.automatic_observed_slice_count,
                        log.automatic_sealed_slice_count,
                        log.automatic_sealed_cursor,
                        log.all_automatic_slices_are_sealed(),
                    )
                });
            let analysis = self
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?;
            let analysis_scheduler = analysis
                .mechanism_support_scheduler_summary(request_id)
                .ok_or(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                )?;
            let seal_sweep_is_complete = analysis
                .next_support_observation_slice_after(request_id, sealed_cursor)?
                .is_none();
            if analysis.mechanism_support_closure(request_id).is_none()
                || scheduler != analysis_scheduler
                || scheduler.dirty().slice_count() != 0
                || observed_count != registry_count
                || sealed_count != registry_count
                || !all_slices_are_sealed
                || !seal_sweep_is_complete
            {
                return Err(RelationalJournalError::SupportObservationClosurePending {
                    request_id,
                });
            }
        }
        Ok(())
    }

    fn validate_readiness_subject(
        &self,
        spec: &WorkNodeSpec,
    ) -> Result<(), RelationalJournalError> {
        match spec {
            WorkNodeSpec::SourcePrefixReady { relation_id, .. } => {
                if *relation_id != self.relation.relation_id() {
                    return Err(RelationalJournalError::ReadinessRelationMismatch);
                }
            }
            WorkNodeSpec::SourceRowReady {
                relation_id,
                source_key,
            } => {
                if *relation_id != self.relation.relation_id() {
                    return Err(RelationalJournalError::ReadinessRelationMismatch);
                }
                if !self.relation.contains_source(*source_key) {
                    return Err(RelationalJournalError::UnknownReadinessSource {
                        source_key: *source_key,
                    });
                }
            }
            WorkNodeSpec::CaseReady { case_id } => {
                if !self.relation.contains_case(*case_id) {
                    return Err(RelationalJournalError::UnknownReadinessCase { case_id: *case_id });
                }
            }
            WorkNodeSpec::SupportCellReady { cell_id } => {
                if self.support.cell(*cell_id).is_none() {
                    return Err(RelationalJournalError::UnknownReadinessCell { cell_id: *cell_id });
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_completion_reference(
        &self,
        completion: &WorkCompletionRef,
    ) -> Result<(), RelationalJournalError> {
        match completion {
            WorkCompletionRef::SourcePrefixReady { .. }
            | WorkCompletionRef::SourceRowReady { .. }
            | WorkCompletionRef::CaseReady { .. }
            | WorkCompletionRef::SupportCellReady { .. } => {
                return Err(RelationalJournalError::ReadinessCompletionMustBeDerived);
            }
            WorkCompletionRef::SourceBindingExhausted {
                relation_id,
                binding_index,
                prefix,
                terminal_ordinal,
                receipt_id,
            } => {
                if *relation_id != self.relation.relation_id() {
                    return Err(RelationalJournalError::CompletionRelationMismatch);
                }
                let receipt = self
                    .source_traversal
                    .as_ref()
                    .and_then(|traversal| traversal.fiber_receipt(*receipt_id))
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                if receipt.relation_id() != *relation_id
                    || receipt.binding_index() != *binding_index
                    || receipt.prefix_digest() != prefix.digest()
                    || receipt.terminal_ordinal() != *terminal_ordinal
                {
                    return Err(RelationalJournalError::CompletionEvidenceSubjectMismatch);
                }
            }
            WorkCompletionRef::SuccessorsSealed {
                relation_id,
                source_key,
                terminal_ordinal,
                receipt_id,
                ..
            } => {
                if *relation_id != self.relation.relation_id() {
                    return Err(RelationalJournalError::CompletionRelationMismatch);
                }
                let receipt = self
                    .successor_exhaustion_receipts
                    .get(receipt_id)
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                if receipt.relation_id() != *relation_id
                    || receipt.source_key() != *source_key
                    || receipt.terminal_ordinal() != *terminal_ordinal
                {
                    return Err(RelationalJournalError::CompletionEvidenceSubjectMismatch);
                }
                if !self.relation.successor_enumeration_is_closed(*source_key)? {
                    return Err(RelationalJournalError::CompletionPrecedesEvidence);
                }
            }
            WorkCompletionRef::AdmissionDecided {
                admission_id,
                case_id,
                decision,
            } => {
                if *admission_id != self.admission.admission_id()
                    || self.admission.decision(*case_id) != Some(*decision)
                {
                    return Err(RelationalJournalError::CompletionPrecedesEvidence);
                }
            }
            WorkCompletionRef::FindDecided {
                question_id,
                case_id,
                decision,
            } => {
                if self.question(*question_id)?.decision(*case_id) != Some(*decision) {
                    return Err(RelationalJournalError::CompletionPrecedesEvidence);
                }
            }
            WorkCompletionRef::DirectSupportEvidence {
                cell_id,
                obligation_id,
                evidence_id,
            } => {
                let evidence = self
                    .support
                    .evidence_record(*evidence_id)
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                if evidence.cell_id() != *cell_id || evidence.obligation_id() != *obligation_id {
                    return Err(RelationalJournalError::CompletionEvidenceSubjectMismatch);
                }
            }
            WorkCompletionRef::SupportObligationRefined {
                cell_id,
                obligation_id,
                refinement_id,
            } => {
                let refinement = self
                    .support
                    .obligation_refinement(*refinement_id)
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                if refinement.parent_obligation_id() != *obligation_id
                    || self.support.obligation_cell(*obligation_id) != Some(*cell_id)
                {
                    return Err(RelationalJournalError::CompletionEvidenceSubjectMismatch);
                }
            }
            WorkCompletionRef::SupportMaterializationExhausted {
                cell_id,
                cardinality_obligation_id,
                evidence_id,
            } => {
                self.support
                    .cell(*cell_id)
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                let evidence = self
                    .support
                    .evidence_record(*evidence_id)
                    .ok_or(RelationalJournalError::CompletionPrecedesEvidence)?;
                if evidence.kind() != SupportEvidenceKind::Cardinality
                    || evidence.cell_id() != *cell_id
                    || evidence.obligation_id() != *cardinality_obligation_id
                {
                    return Err(RelationalJournalError::CompletionEvidenceSubjectMismatch);
                }
            }
        }
        Ok(())
    }
}

/// Open append-only journal and the state reconstructed by its prefix.
#[derive(Clone, Debug)]
pub(crate) struct RelationalJournal {
    contract: RelationalJournalContract,
    sequence: u64,
    head: RelationalJournalHead,
    entries: Vec<RelationalJournalEntry>,
    retain_history: bool,
    state: RelationalEvidenceState,
}

/// One bounded request-local support lifecycle proposal. Every independently
/// growing lane is imported in sparse checkpoint quanta; semantic closure is
/// proposed only after the exact complete cursor has been durably bound.
pub(crate) enum RelationalMechanismSupportStepEvents {
    Observed {
        point_id: MechanismSupportObservationPointId,
        slice: MechanismSupportSlice,
        status: MechanismSupportObservationStatus,
        events: Box<[RelationalJournalEvent]>,
    },
    Checkpoint {
        accepted_target_cases: usize,
        cursor: MechanismSupportCheckpointCursor,
        frontier_root: MechanismSupportFrontierRoot,
        events: Box<[RelationalJournalEvent]>,
    },
    Closed {
        checkpointed_frontier: bool,
        cursor: MechanismSupportCheckpointCursor,
        support_root: MechanismSupportClosureRoot,
        events: Box<[RelationalJournalEvent]>,
    },
    /// The imported prefixes are caught up to all currently visible upstream
    /// work, but one or both upstream semantic closures are still absent.
    /// This is ordinary open-stream quiescence, not a failed checkpoint.
    Idle,
}

/// One bounded operational extension step for explicitly requested support
/// readers. It can continue after the immutable analysis DAG has closed.
pub(crate) enum RelationalExplicitMechanismSupportStepEvents {
    Backfilled {
        slice: MechanismSupportSlice,
        from_structural_cursor: u128,
        through_structural_cursor: u128,
        completed: bool,
        events: Box<[RelationalJournalEvent]>,
    },
    Observed {
        point_id: MechanismSupportObservationPointId,
        slice: MechanismSupportSlice,
        status: MechanismSupportObservationStatus,
        events: Box<[RelationalJournalEvent]>,
    },
    Idle,
}

/// Read-only, incrementally indexed scheduler projection of a journal prefix.
///
/// Unlike [`RelationalJournalSnapshot`], this view does not sort relation rows
/// or clone whole catalogs. Most concrete quanta borrow the incrementally
/// validated mutable indexes and remain proportional to the records they
/// touch. Closure-only methods explicitly validate and canonically hash the
/// borrowed support catalog without materializing an owned snapshot.
#[derive(Clone, Copy)]
pub(crate) struct RelationalSchedulerView<'a> {
    journal: &'a RelationalJournal,
    sequence: u64,
}

impl<'a> RelationalSchedulerView<'a> {
    pub(crate) const fn contract(self) -> &'a RelationalJournalContract {
        &self.journal.contract
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn head(self) -> RelationalJournalHead {
        self.journal.head
    }

    pub(crate) const fn scheduler_decision_count(
        self,
        decision: RelationalSchedulerDecision,
    ) -> u64 {
        self.journal.state.scheduler_decision_counts[decision.priority() as usize]
    }

    pub(crate) const fn transition_support(self) -> &'a RelationalTransitionSupportIndex {
        &self.journal.state.transition_support
    }

    pub(crate) fn analysis_plan_root(self) -> Option<RelationalAnalysisPlanRoot> {
        self.journal
            .state
            .analysis_plan
            .as_ref()
            .map(RelationalAnalysisPlan::root)
    }

    pub(crate) fn analysis_scope_root(self) -> Option<RelationalAnalysisJournalScopeRoot> {
        self.journal
            .state
            .analysis
            .as_ref()
            .and_then(RelationalAnalysisJournalState::scope_root)
    }

    pub(crate) fn analysis_is_closed(self) -> bool {
        self.journal
            .state
            .analysis
            .as_ref()
            .is_some_and(RelationalAnalysisJournalState::is_closed)
    }

    pub(crate) fn analysis_closure_set_root(self) -> Option<RelationalAnalysisClosureSetRoot> {
        self.journal
            .state
            .analysis
            .as_ref()
            .and_then(RelationalAnalysisJournalState::closed_closure_set_root)
    }

    pub(crate) fn support_plan_root(self) -> Option<RelationalSupportPlanRoot> {
        self.journal
            .state
            .support_plan
            .as_ref()
            .map(RelationalSupportPlan::root)
    }

    pub(crate) const fn support_catalog_is_sealed(self) -> bool {
        self.journal.state.support.catalog_is_sealed()
    }

    pub(crate) fn support_validated_closure(
        self,
    ) -> Result<ValidatedSupportEvidenceClosure<'a>, RelationalJournalError> {
        Ok(self.journal.state.support.validated_closure()?)
    }

    /// `Some(true)` names a registered root obligation that still needs a
    /// resolver. `Some(false)` has already been proved or refined, while
    /// `None` is not a registered root obligation in this journal prefix.
    pub(crate) fn support_root_obligation_is_open(
        self,
        obligation_id: SupportProofObligationId,
    ) -> Option<bool> {
        self.journal
            .state
            .support
            .root_obligation_is_open(obligation_id)
    }

    pub(crate) fn support_evidence_record(
        self,
        evidence_id: SupportCellEvidenceId,
    ) -> Option<&'a SupportEvidenceRecord> {
        self.journal.state.support.evidence_record(evidence_id)
    }

    /// Reverify and remint the exact source-population handle from the
    /// retained plan-bound artifact and both exact durable evidence records.
    /// `None` means no such proof event has been accepted in this prefix.
    pub(crate) fn certified_source_population(
        self,
    ) -> Result<Option<CertifiedSourcePopulationBinding>, RelationalJournalError> {
        self.journal.state.remint_certified_source_population()
    }

    pub(crate) fn support_refinement_for_parent(
        self,
        parent_obligation_id: SupportProofObligationId,
    ) -> Option<&'a SupportObligationRefinement> {
        self.journal
            .state
            .support
            .refinement_for_parent(parent_obligation_id)
    }

    pub(crate) fn support_admission_evidence_id_for_obligation(
        self,
        obligation_id: SupportProofObligationId,
    ) -> Option<SupportCellEvidenceId> {
        self.journal
            .state
            .support
            .admission_evidence_for_obligation(obligation_id)
            .map(|evidence| evidence.id())
    }

    /// Typed contiguous classified prefix rebuilt only from accepted outer
    /// evidence events. This, rather than a generic cursor, is the scheduler's
    /// authority for choosing the next canonical chunk.
    pub(crate) fn classified_sweep_progress(
        self,
    ) -> Result<Option<&'a RelationalClassifiedSweepProgress>, RelationalJournalError> {
        Ok(self.journal.state.classified_sweep_progress.as_ref())
    }

    /// Replay-derived checked prefix of the currently selected canonical
    /// chunk. This is
    /// operational checkpoint authority only: support and classified progress
    /// still stop at the preceding complete chunk until this accumulator is
    /// finalized and its canonical whole-chunk artifact is accepted.
    pub(crate) fn classified_chunk_accumulator(
        self,
    ) -> Result<Option<&'a RelationalClassifiedChunkAccumulator>, RelationalJournalError> {
        Ok(self.journal.state.classified_chunk_accumulator.as_ref())
    }

    /// Retained accepted chunk payloads in canonical partition ordinal order.
    /// The typed progress record remains the cursor authority; this slice is
    /// the replay input for sparse selected-run realization.
    pub(crate) fn classified_support_fragments(
        self,
    ) -> Result<RelationalClassifiedSupportPrefix<'a>, RelationalJournalError> {
        let committed = self
            .journal
            .state
            .classified_sweep_progress
            .as_ref()
            .map_or(0, RelationalClassifiedSweepProgress::committed_prefix_count);
        let slots = self
            .journal
            .state
            .classified_support_fragment_slots
            .get(..committed)
            .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
        Ok(RelationalClassifiedSupportPrefix { slots })
    }

    /// Borrow one independently accepted canonical classified slot. An empty
    /// slot is ordinary residual work; it says nothing about later slots.
    pub(crate) fn classified_support_fragment_at(
        self,
        chunk_ordinal: usize,
    ) -> Result<Option<&'a RelationalClassifiedSupportFragment>, RelationalJournalError> {
        Ok(self
            .journal
            .state
            .classified_support_fragment_slots
            .get(chunk_ordinal)
            .and_then(Option::as_ref))
    }

    pub(crate) fn classified_support_slot_is_occupied(self, chunk_ordinal: usize) -> bool {
        self.journal
            .state
            .classified_support_fragment_slots
            .get(chunk_ordinal)
            .is_some_and(Option::is_some)
    }

    pub(crate) fn accepted_classified_fragment_count(self) -> usize {
        self.journal.state.accepted_classified_fragment_count
    }

    /// Number of first-arrival case/support facts reconstructed by this exact
    /// journal prefix. The sequence is an operational publication cursor and
    /// does not participate in any semantic evidence root.
    pub(crate) fn case_support_discovery_event_count(self) -> usize {
        self.journal.state.case_support_discovery.len()
    }

    /// Resolve one compact discovery coordinate to its immutable retained
    /// payload. A caller may inspect the classified question mask or the
    /// materialization's selected-question set and omit records irrelevant to
    /// its per-question projection without changing this shared order.
    pub(crate) fn case_support_discovery_event_at(
        self,
        event_ordinal: usize,
    ) -> Result<Option<RelationalCaseSupportDiscoveryEvent<'a>>, RelationalJournalError> {
        let Some(coordinate) = self
            .journal
            .state
            .case_support_discovery
            .get(event_ordinal)
            .copied()
        else {
            return Ok(None);
        };
        match coordinate {
            RelationalCaseSupportDiscoveryCoordinate::ClassifiedFragment { chunk_ordinal } => {
                let fragment = self
                    .journal
                    .state
                    .classified_support_fragment_slots
                    .get(chunk_ordinal)
                    .and_then(Option::as_ref)
                    .ok_or(RelationalJournalError::CaseSupportDiscoveryIndexMismatch {
                        event_ordinal,
                    })?;
                let logical_chunk_ordinal = u128::try_from(chunk_ordinal).map_err(|_| {
                    RelationalJournalError::CaseSupportDiscoveryIndexMismatch { event_ordinal }
                })?;
                if fragment.chunk_ordinal() != logical_chunk_ordinal {
                    return Err(RelationalJournalError::CaseSupportDiscoveryIndexMismatch {
                        event_ordinal,
                    });
                }
                Ok(Some(
                    RelationalCaseSupportDiscoveryEvent::ClassifiedFragment {
                        chunk_ordinal: logical_chunk_ordinal,
                        fragment,
                    },
                ))
            }
            RelationalCaseSupportDiscoveryCoordinate::SelectedRunMaterialization {
                chunk_ordinal,
                run_ordinal,
            } => {
                let classified = self
                    .journal
                    .state
                    .classified_support_fragment_slots
                    .get(chunk_ordinal)
                    .and_then(Option::as_ref)
                    .and_then(RelationalClassifiedSupportFragment::concrete)
                    .ok_or(RelationalJournalError::CaseSupportDiscoveryIndexMismatch {
                        event_ordinal,
                    })?;
                let run = classified
                    .runs()
                    .get(usize::from(run_ordinal))
                    .filter(|run| run.ordinal() == run_ordinal)
                    .ok_or(RelationalJournalError::CaseSupportDiscoveryIndexMismatch {
                        event_ordinal,
                    })?;
                let materialization = self
                    .journal
                    .state
                    .selected_run_materializations
                    .get(&run.cell_id())
                    .filter(|materialization| {
                        materialization.classified_chunk_artifact_id() == classified.id()
                            && materialization.chunk_ordinal() == classified.chunk_ordinal()
                            && materialization.run_ordinal() == run_ordinal
                            && materialization.run_cell_id() == run.cell_id()
                    })
                    .ok_or(RelationalJournalError::CaseSupportDiscoveryIndexMismatch {
                        event_ordinal,
                    })?;
                Ok(Some(
                    RelationalCaseSupportDiscoveryEvent::SelectedRunMaterialization {
                        chunk_ordinal: classified.chunk_ordinal(),
                        run_ordinal,
                        materialization,
                    },
                ))
            }
        }
    }

    /// Lowest canonical classified child whose semantic artifact is durable
    /// but whose matching resolver completion checkpoint is not. This is a
    /// replay-derived recovery index and never substitutes for work-node or
    /// support-evidence validation.
    pub(crate) fn next_pending_classified_work_completion_ordinal(self) -> Option<usize> {
        self.journal
            .state
            .pending_classified_work_completion_ordinals
            .first()
            .copied()
    }

    /// Lowest canonical selected run that has not yet published its concrete
    /// materialization. Ordering the sparse set by `(chunk, run)` preserves
    /// canonical projection even when candidate chunks arrive out of order.
    pub(crate) fn next_pending_selected_run_position(self) -> Option<(usize, u16)> {
        self.journal
            .state
            .pending_selected_run_positions
            .first()
            .copied()
    }

    /// Return the one currently legal root-prefix promotion, if its sparse
    /// slot is occupied. The tuple exposes scheduler quantum metadata without
    /// requiring callers to destructure private checkpoint variants.
    pub(crate) fn next_classified_prefix_advance_event(
        self,
    ) -> Result<Option<(u128, [u8; 32], RelationalJournalEvent)>, RelationalJournalError> {
        let Some(progress) = self.journal.state.classified_sweep_progress.as_ref() else {
            return Ok(None);
        };
        let chunk_ordinal = progress.next_chunk_ordinal();
        let chunk_index = usize::try_from(chunk_ordinal).map_err(|_| {
            RelationalJournalError::ClassifiedSweepProgressGap {
                expected: chunk_ordinal,
                actual: chunk_ordinal,
            }
        })?;
        let Some(fragment) = self
            .journal
            .state
            .classified_support_fragment_slots
            .get(chunk_index)
            .and_then(Option::as_ref)
        else {
            return Ok(None);
        };
        let artifact_digest = fragment.artifact_digest();
        Ok(Some((
            chunk_ordinal,
            artifact_digest,
            RelationalJournalEvent::relational_classified_prefix_advanced(
                progress.partition_artifact_id(),
                chunk_ordinal,
                artifact_digest,
            ),
        )))
    }

    /// Borrow the opaque partition authority reconstructed when the
    /// authenticated partition event entered this journal state. The value is
    /// deliberately absent before that event and is never restored from a
    /// standalone cache or snapshot.
    pub(crate) fn verified_case_chunk_partition(
        self,
    ) -> Result<Option<&'a VerifiedRelationalCaseChunkPartition>, RelationalJournalError> {
        Ok(self.journal.state.verified_case_chunk_partition.as_ref())
    }

    pub(crate) fn selected_run_materialization(
        self,
        run_cell_id: SupportCellId,
    ) -> Result<Option<&'a RelationalSelectedRunMaterializationArtifact>, RelationalJournalError>
    {
        Ok(self
            .journal
            .state
            .selected_run_materializations
            .get(&run_cell_id))
    }

    /// Whether every admitted+selected run in every occupied classified slot
    /// has exactly one admitted sparse materialization and no other run-cell
    /// payload is present. Exact full-population closure separately requires
    /// every canonical partition slot to be occupied; prefix promotion is only
    /// an operational checkpoint.
    pub(crate) fn selected_run_materializations_cover_classified_slots(
        self,
        question_id: QuestionId,
    ) -> Result<bool, RelationalJournalError> {
        self.journal.state.question(question_id)?;
        let mut expected = 0usize;
        for artifact in self
            .journal
            .state
            .classified_support_fragment_slots
            .iter()
            .flatten()
        {
            let Some(artifact) = artifact.concrete() else {
                continue;
            };
            let question_index = artifact
                .question_ids()
                .binary_search(&question_id)
                .map_err(|_| RelationalJournalError::UnknownQuestion { question_id })?;
            for run in artifact.runs() {
                if run.outcome().selection(question_index) != Some(SelectionDecision::Selected) {
                    continue;
                }
                expected = match expected.checked_add(1) {
                    Some(expected) => expected,
                    None => return Ok(false),
                };
                if !self
                    .journal
                    .state
                    .selected_run_materializations
                    .get(&run.cell_id())
                    .is_some_and(|artifact| artifact.contains_question(question_id))
                {
                    return Ok(false);
                }
            }
        }
        Ok(self
            .journal
            .state
            .selected_run_materializations
            .values()
            .filter(|artifact| artifact.contains_question(question_id))
            .count()
            == expected)
    }

    pub(crate) fn selected_run_materialization_count(
        self,
        question_id: QuestionId,
    ) -> Result<usize, RelationalJournalError> {
        self.journal.state.question(question_id)?;
        Ok(self
            .journal
            .state
            .selected_run_materializations
            .values()
            .filter(|artifact| artifact.contains_question(question_id))
            .count())
    }

    pub(crate) fn selected_run_materializations(
        self,
        question_id: QuestionId,
    ) -> Result<
        impl Iterator<Item = &'a RelationalSelectedRunMaterializationArtifact> + 'a,
        RelationalJournalError,
    > {
        self.journal.state.question(question_id)?;
        Ok(self
            .journal
            .state
            .selected_run_materializations
            .values()
            .filter(move |artifact| artifact.contains_question(question_id)))
    }

    /// Concrete selected CaseIds admitted by sparse run artifacts. They are
    /// content-derived and unique across artifacts; this prefix iterator does
    /// not itself claim that the selected population is closed.
    pub(crate) fn materialized_selected_case_ids(
        self,
        question_id: QuestionId,
    ) -> Result<impl Iterator<Item = RelationalCaseId> + 'a, RelationalJournalError> {
        self.journal.state.question(question_id)?;
        Ok(self
            .journal
            .state
            .selected_run_materializations
            .values()
            .filter(move |artifact| artifact.contains_question(question_id))
            .flat_map(|artifact| artifact.cases().iter().map(|record| record.case_id())))
    }

    /// Canonical concrete selected CaseId order from the incrementally
    /// authenticated FIND catalog. On the classified branch, complete sparse
    /// run coverage proves this is the concrete image of the selected support
    /// population rather than merely an observed lower bound.
    pub(crate) fn canonical_concrete_selected_case_ids(
        self,
        question_id: QuestionId,
    ) -> Result<impl Iterator<Item = RelationalCaseId> + 'a, RelationalJournalError> {
        Ok(self
            .journal
            .state
            .question(question_id)?
            .selected_case_ids())
    }

    /// Borrow the latest authenticated operational cursor for one support
    /// cell. The caller must still validate it against the exact planned cell;
    /// this view confers no proof or closure authority.
    pub(crate) fn support_latest_materialization_cursor(
        self,
        cell_id: SupportCellId,
    ) -> Option<&'a SupportMaterializationCursor> {
        self.journal.state.support.latest_cursor(cell_id)
    }

    pub(crate) fn certified_root_case_cardinality(self) -> Option<u128> {
        let root_obligations = self.journal.state.support_plan.as_ref()?.root_obligations();
        if let Some(exact_cardinality) = root_obligations.resolved_exact_cardinality() {
            return Some(exact_cardinality);
        }
        let RelationalRootObligationPlan::CellBacked {
            root_cell_id,
            descriptors,
        } = root_obligations
        else {
            return None;
        };
        descriptors.iter().find_map(|descriptor| {
            let RelationalStagedObligationDescriptor::Root {
                activation: RelationalObligationActivation::RootCasePopulation,
                obligation: SupportObligationRecord::Cardinality(obligation),
            } = descriptor
            else {
                return None;
            };
            let evidence = self
                .journal
                .state
                .support
                .cardinality_evidence_for_obligation(obligation.id())?;
            (evidence.obligation().cell_id() == *root_cell_id).then(|| evidence.exact_cardinality())
        })
    }

    pub(crate) fn certified_root_admission_decision(self) -> Option<AdmissionDecision> {
        self.journal.state.certified_root_admission_decision()
    }

    /// Derive logical classification counts from the case-root support DAG.
    ///
    /// Before an exact case cardinality is durably certified there is no
    /// denominator against which sealed leaves can be audited, so `None` is
    /// returned and report callers retain their extensional fallback. Once
    /// available, auxiliary source-proof cells are excluded by the derivation,
    /// which walks only the installed case-root subtree.
    pub(crate) fn classification_progress_counts(
        self,
        question_id: QuestionId,
    ) -> Result<Option<RelationalClassificationProgressCounts>, RelationalJournalError> {
        self.journal.state.question(question_id)?;
        if self.certified_root_case_cardinality().is_none() {
            return Ok(None);
        }
        let plan = self
            .journal
            .state
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        RelationalClassificationProgressCounts::derive_from_builder(
            plan,
            &self.journal.state.support,
            question_id,
        )
        .map(Some)
        .map_err(RelationalJournalError::from)
    }

    /// Materialize only the support proof root. A driver calls this at
    /// support quiescence, not once per concrete member.
    pub(crate) fn support_evidence_root(
        self,
    ) -> Result<SupportEvidenceRoot, RelationalJournalError> {
        Ok(self.support_validated_closure()?.root())
    }

    pub(crate) const fn source_enumeration_is_closed(self) -> bool {
        self.journal.state.relation.source_enumeration_is_closed()
    }

    pub(crate) fn source_traversal_is_started(self) -> bool {
        self.journal
            .state
            .source_traversal
            .as_ref()
            .is_some_and(SourceTraversalAccumulator::has_observations)
            || self.journal.state.source_relation_exhaustion.is_some()
    }

    pub(crate) fn relation_enumeration_is_complete(self) -> bool {
        self.journal.state.relation.enumeration_is_complete()
    }

    /// Whether the concrete U/D/M transition relation is extensionally
    /// complete. Proof closure alone does not satisfy this predicate: a
    /// symbolic answer may still require its authenticated materializer.
    pub(crate) fn transition_support_is_extentionally_closed(
        self,
        question_id: QuestionId,
    ) -> Result<bool, RelationalJournalError> {
        Ok(self.relation_enumeration_is_complete()
            && self.admission_decision_count() == self.case_count()
            && self.question_decision_count(question_id)? == self.admitted_count())
    }

    pub(crate) fn source_row(self, source_key: SourceKey) -> Option<&'a SourceRow> {
        self.journal.state.relation.source_row(source_key)
    }

    pub(crate) fn source_keys(self) -> impl Iterator<Item = SourceKey> + 'a {
        self.journal.state.relation.source_keys()
    }

    pub(crate) fn source_result_input_seal(
        self,
    ) -> Result<RelationalResultInputSeal, RelationalJournalError> {
        self.journal.state.remint_source_result_input_seal()
    }

    pub(crate) fn case(self, case_id: RelationalCaseId) -> Option<RelationalCaseRef<'a>> {
        self.journal.state.relation.case(case_id)
    }

    pub(crate) fn case_count(self) -> usize {
        self.journal.state.relation.case_count()
    }

    pub(crate) fn source_count(self) -> usize {
        self.journal.state.relation.source_count()
    }

    pub(crate) fn admission_decision(self, case_id: RelationalCaseId) -> Option<AdmissionDecision> {
        self.journal.state.admission.decision(case_id)
    }

    pub(crate) fn question_decision(
        self,
        question_id: QuestionId,
        case_id: RelationalCaseId,
    ) -> Result<Option<SelectionDecision>, RelationalJournalError> {
        Ok(self.journal.state.question(question_id)?.decision(case_id))
    }

    pub(crate) fn admission_decision_count(self) -> usize {
        self.journal.state.admission.decision_count()
    }

    pub(crate) fn admitted_count(self) -> usize {
        self.journal.state.admission.admitted_count()
    }

    pub(crate) fn question_decision_count(
        self,
        question_id: QuestionId,
    ) -> Result<usize, RelationalJournalError> {
        Ok(self.journal.state.question(question_id)?.decision_count())
    }

    pub(crate) fn selected_count(
        self,
        question_id: QuestionId,
    ) -> Result<usize, RelationalJournalError> {
        Ok(self.journal.state.question(question_id)?.selected_count())
    }

    /// Canonical selected CaseIds from the incremental FIND catalog. This is
    /// a borrow-only scheduling index, not proof that FIND has closed; callers
    /// must separately require the authenticated selected-question seal.
    pub(crate) fn selected_case_ids(
        self,
        question_id: QuestionId,
    ) -> Result<impl Iterator<Item = RelationalCaseId> + 'a, RelationalJournalError> {
        Ok(self
            .journal
            .state
            .question(question_id)?
            .selected_case_ids())
    }

    /// Borrow the operational selected-discovery suffix reconstructed by
    /// journal replay. Its ordinal is suitable only for an invocation-local
    /// catch-up cursor; canonical roots and exact seals continue to use the
    /// arrival-order-independent classification map.
    pub(crate) fn selected_discovery_suffix(
        self,
        question_id: QuestionId,
        from_ordinal: usize,
    ) -> Result<&'a [RelationalCaseId], RelationalJournalError> {
        Ok(self
            .journal
            .state
            .question(question_id)?
            .selected_discovery_suffix(from_ordinal))
    }

    pub(crate) fn concrete_base_is_classified(
        self,
        question_id: QuestionId,
    ) -> Result<bool, RelationalJournalError> {
        Ok(self.relation_enumeration_is_complete()
            && self.admission_decision_count() == self.case_count()
            && self.question_decision_count(question_id)? == self.admitted_count())
    }

    pub(crate) fn work_node(self, node_id: WorkNodeId) -> Option<WorkNodeSnapshot> {
        self.journal.state.work.get(node_id)
    }

    pub(crate) fn work_node_count(self) -> usize {
        self.journal.state.work.len()
    }

    pub(crate) fn completed_work_node_count(self) -> usize {
        self.journal.state.work.completed_len()
    }

    pub(crate) fn runnable_work_nodes(self) -> impl Iterator<Item = WorkNodeSnapshot> + 'a {
        self.journal
            .state
            .work
            .runnable_node_ids()
            .filter_map(|node_id| self.journal.state.work.get(node_id))
    }

    pub(crate) fn open_work_nodes(self) -> impl Iterator<Item = WorkNodeSnapshot> + 'a {
        self.journal
            .state
            .work
            .open_node_ids()
            .filter_map(|node_id| self.journal.state.work.get(node_id))
    }
}

impl RelationalJournal {
    pub(crate) fn new(contract: RelationalJournalContract) -> Self {
        Self::with_history_retention(contract, true, None)
    }

    pub(crate) fn new_with_region_replay_authority(
        contract: RelationalJournalContract,
        authority: Arc<RelationalRegionReplayAuthority>,
    ) -> Self {
        Self::with_history_retention(contract, true, Some(authority))
    }

    /// Construct the production fold used with an external durable segment
    /// sink. Applied entries are returned to that sink and are not retained a
    /// second time beside their folded catalog state.
    pub(crate) fn new_streaming(contract: RelationalJournalContract) -> Self {
        Self::with_history_retention(contract, false, None)
    }

    pub(crate) fn new_streaming_with_region_replay_authority(
        contract: RelationalJournalContract,
        authority: Arc<RelationalRegionReplayAuthority>,
    ) -> Self {
        Self::with_history_retention(contract, false, Some(authority))
    }

    fn with_history_retention(
        contract: RelationalJournalContract,
        retain_history: bool,
        region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    ) -> Self {
        let head = RelationalJournalHead::genesis(contract.id());
        let state = RelationalEvidenceState::new(contract.clone(), region_replay_authority);
        Self {
            contract,
            sequence: 0,
            head,
            entries: Vec::new(),
            retain_history,
            state,
        }
    }

    pub(crate) const fn contract(&self) -> &RelationalJournalContract {
        &self.contract
    }

    pub(crate) const fn head(&self) -> RelationalJournalHead {
        self.head
    }

    pub(crate) const fn next_sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn entries(&self) -> &[RelationalJournalEntry] {
        assert!(
            self.retain_history,
            "streaming relational journals keep history in their durable sink"
        );
        &self.entries
    }

    pub(crate) fn region_replay_authority(&self) -> Option<&RelationalRegionReplayAuthority> {
        self.state.region_replay_authority.as_deref()
    }

    pub(crate) fn scheduler_view(
        &self,
    ) -> Result<RelationalSchedulerView<'_>, RelationalJournalError> {
        Ok(RelationalSchedulerView {
            journal: self,
            sequence: self.sequence,
        })
    }

    /// Bind one checked executor advance to this journal's registered support
    /// plan. The returned frame remains unapplied so a durable adapter can
    /// install it before publishing the new head.
    pub(crate) fn source_traversal_event(
        &self,
        advance: RelationalSourceAdvance,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let support_plan_root = self
            .state
            .support_plan
            .as_ref()
            .map(RelationalSupportPlan::root)
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        Ok(RelationalJournalEvent::source_traversal_observed(
            self.contract.relation_id(),
            support_plan_root,
            advance,
        ))
    }

    /// Verify the whole concrete dependent-product tree and build its one-time
    /// aggregate seal frame. This is intentionally O(tree size) once at
    /// closure; ordinary edge ingestion remains incremental.
    pub(crate) fn source_enumeration_seal_event(
        &self,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        if let Some(receipt) = &self.state.source_relation_exhaustion {
            return Ok(RelationalJournalEvent::source_enumeration_sealed(
                receipt.clone(),
            ));
        }
        let receipt = self
            .state
            .source_traversal
            .as_ref()
            .ok_or(RelationalJournalError::SourceTraversalMissing)?
            .finish()?;
        Ok(RelationalJournalEvent::source_enumeration_sealed(receipt))
    }

    /// Mint the post-FIND bridge from a completely materialized relation and
    /// classification catalog. This performs its O(N) canonical close once at
    /// the base/analysis boundary, never once per case.
    pub(crate) fn selected_question_extensional_event(
        &self,
        question_id: QuestionId,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let seal = self
            .state
            .remint_extensional_selected_question_seal(question_id)?;
        Ok(RelationalJournalEvent::analysis(
            RelationalAnalysisEvidenceEvent::selected_question_bound(seal),
        ))
    }

    /// Mint the post-FIND bridge from exact sealed SupportCell evidence and
    /// the independently complete concrete CaseId image. Exact-empty support
    /// needs no rows; positive support must be covered by every selected run.
    pub(crate) fn selected_question_certified_event(
        &self,
        question_id: QuestionId,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let seal = self
            .state
            .remint_certified_selected_question_seal(question_id)?;
        Ok(RelationalJournalEvent::analysis(
            RelationalAnalysisEvidenceEvent::selected_question_bound(seal),
        ))
    }

    /// Derive, rather than accept, the only terminal analysis event valid for
    /// the current journal prefix.
    pub(crate) fn analysis_terminal_event(
        &self,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        self.state.require_all_support_observations_sealed()?;
        let event = self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .terminal_event()?;
        Ok(RelationalJournalEvent::analysis(event))
    }

    pub(crate) fn support_observation_demand_registration_event(
        &mut self,
        slice: MechanismSupportSlice,
    ) -> Result<Option<RelationalJournalEvent>, RelationalJournalError> {
        let request_id = slice.key().request_id();
        if self
            .state
            .mechanism_support_observation_demands
            .get(&request_id)
            .and_then(|log| log.registration(slice))
            .is_some()
        {
            return Ok(None);
        }
        if !self
            .state
            .latest_support_frontiers
            .contains_key(&request_id)
        {
            return Ok(None);
        }
        let durable = self.state.require_support_extension_anchor(request_id)?;
        let durable_scheduler = self
            .state
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
            .ok_or(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id })?;
        validate_explicit_support_scheduler_summary(request_id, durable_scheduler)?;
        let prepared = self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .prepare_explicit_support_observation_registration(slice)?;
        if prepared.prior_scheduler_summary() != durable_scheduler {
            return Err(RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id });
        }
        let claim = MechanismSupportObservationDemandRegistrationClaim::new(
            prepared.slice(),
            durable.cursor,
            durable.frontier.root(),
            prepared.disposition(),
            prepared.registration_phase(),
            prepared.registration_structural_cursor(),
            prepared.prior_scheduler_summary(),
            prepared.next_scheduler_summary(),
        );
        Ok(Some(
            RelationalJournalEvent::support_observation_demand_registered(claim),
        ))
    }

    pub(crate) fn explicit_support_observation_step_events(
        &mut self,
        request_id: MechanismRequestId,
        maximum_assignments: NonZeroU16,
    ) -> Result<RelationalExplicitMechanismSupportStepEvents, RelationalJournalError> {
        if !self
            .state
            .latest_support_frontiers
            .contains_key(&request_id)
        {
            return Ok(RelationalExplicitMechanismSupportStepEvents::Idle);
        }
        let durable = self.state.require_support_extension_anchor(request_id)?;
        let Some(durable_scheduler) = self
            .state
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
        else {
            return Ok(RelationalExplicitMechanismSupportStepEvents::Idle);
        };
        validate_explicit_support_scheduler_summary(request_id, durable_scheduler)?;
        let runtime_limit =
            u128::from(maximum_assignments.get()).min(RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA);
        let runtime_limit = NonZeroU16::new(
            u16::try_from(runtime_limit).expect("the explicit backfill protocol bound fits u16"),
        )
        .expect("a nonzero runtime limit remains nonzero after protocol capping");
        if durable_scheduler.pending_backfill().slice_count() != 0 {
            let prepared = self
                .state
                .analysis
                .as_ref()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?
                .prepare_next_explicit_support_observation_backfill(request_id, runtime_limit)?
                .ok_or(
                    RelationalJournalError::SupportObservationBackfillClaimMismatch { request_id },
                )?;
            if prepared.prior_scheduler_summary() != durable_scheduler {
                return Err(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                );
            }
            let claim = MechanismSupportObservationBackfillClaim::new(
                prepared.slice(),
                durable.cursor,
                durable.frontier.root(),
                prepared.registration_phase(),
                prepared.registration_structural_cursor(),
                prepared.from_structural_cursor(),
                prepared.through_structural_cursor(),
                prepared.completed(),
                prepared.prior_scheduler_summary(),
                prepared.next_scheduler_summary(),
            );
            return Ok(RelationalExplicitMechanismSupportStepEvents::Backfilled {
                slice: prepared.slice(),
                from_structural_cursor: prepared.from_structural_cursor(),
                through_structural_cursor: prepared.through_structural_cursor(),
                completed: prepared.completed(),
                events: vec![
                    RelationalJournalEvent::support_observation_backfill_checkpointed(claim),
                ]
                .into_boxed_slice(),
            });
        }
        if let Some(claim) = self
            .state
            .next_explicit_mechanism_support_observation_claim(request_id)?
        {
            return Ok(RelationalExplicitMechanismSupportStepEvents::Observed {
                point_id: claim.point_id(),
                slice: claim.slice(),
                status: claim.status(),
                events: vec![RelationalJournalEvent::support_subject_observed(claim)]
                    .into_boxed_slice(),
            });
        }
        Ok(RelationalExplicitMechanismSupportStepEvents::Idle)
    }

    /// Advance at most one protocol-bounded suffix on each support lane and
    /// prepare either its sparse durable checkpoint or, on a later turn, the
    /// semantic closure for an already-durable final checkpoint. Planning is
    /// anchored to the latest durable receipt. If an earlier proposal advanced
    /// derived caches but was never installed, the next call remints exactly
    /// that proposal instead of silently accumulating another suffix.
    pub(crate) fn support_lifecycle_step_events(
        &mut self,
        request_id: MechanismRequestId,
        maximum_lane_delta: NonZeroU16,
    ) -> Result<RelationalMechanismSupportStepEvents, RelationalJournalError> {
        let durable = self
            .state
            .latest_support_frontiers
            .get(&request_id)
            .copied();
        let anchor_cursor = durable
            .map_or_else(MechanismSupportCheckpointCursor::default, |receipt| {
                receipt.cursor
            });
        let (mut cursor, mut available) = self
            .state
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .support_checkpoint_cursors(request_id)?;
        let derived_state_is_ahead =
            validate_support_checkpoint_delta(request_id, anchor_cursor, cursor)?;

        // A discarded proposal may leave replay-derived caches one bounded
        // suffix ahead of the durable checkpoint. Re-emit that exact frontier
        // before interpreting it as an observable prefix.
        if derived_state_is_ahead {
            let frontier = self
                .state
                .analysis
                .as_mut()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?
                .checkpoint_support_frontier(request_id)?;
            let frontier_root = frontier.root();
            return Ok(RelationalMechanismSupportStepEvents::Checkpoint {
                accepted_target_cases: 0,
                cursor,
                frontier_root,
                events: vec![RelationalJournalEvent::support_frontier_checkpointed(
                    request_id,
                    cursor,
                    frontier_root,
                )]
                .into_boxed_slice(),
            });
        }
        if let Some(durable) = durable {
            let frontier = self
                .state
                .analysis
                .as_mut()
                .ok_or(RelationalJournalError::AnalysisStateMissing)?
                .checkpoint_support_frontier(request_id)?;
            validate_support_frontier_enrichment(request_id, durable.frontier, frontier)?;
            if durable.frontier != frontier {
                let frontier_root = frontier.root();
                return Ok(RelationalMechanismSupportStepEvents::Checkpoint {
                    accepted_target_cases: 0,
                    cursor,
                    frontier_root,
                    events: vec![RelationalJournalEvent::support_frontier_checkpointed(
                        request_id,
                        cursor,
                        frontier_root,
                    )]
                    .into_boxed_slice(),
                });
            }
        }
        if let Some(claim) = self.next_mechanism_support_observation_claim(request_id)? {
            return Ok(RelationalMechanismSupportStepEvents::Observed {
                point_id: claim.point_id(),
                slice: claim.slice(),
                status: claim.status(),
                events: vec![RelationalJournalEvent::support_subject_observed(claim)]
                    .into_boxed_slice(),
            });
        }
        if self
            .state
            .latest_explicit_support_schedulers
            .get(&request_id)
            .is_some_and(|scheduler| scheduler.pending_backfill().slice_count() != 0)
        {
            return Err(RelationalJournalError::SupportCheckpointObservationPending { request_id });
        }
        if self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .mechanism_support_closure(request_id)
            .is_some()
        {
            return Ok(RelationalMechanismSupportStepEvents::Idle);
        }
        let runtime_limit =
            u128::from(maximum_lane_delta.get()).min(RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA);
        let runtime_limit = NonZeroU16::new(
            u16::try_from(runtime_limit).expect("the protocol support-checkpoint bound fits u16"),
        )
        .expect("a nonzero runtime limit remains nonzero after protocol capping");
        let (accepted_target_cases, advanced, upstream) = self
            .state
            .advance_analysis_support_checkpoint_bounded(request_id, runtime_limit)?;
        validate_support_checkpoint_delta(request_id, cursor, advanced)?;
        cursor = advanced;
        available = upstream;

        let frontier = self
            .state
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .checkpoint_support_frontier(request_id)?;
        let frontier_root = frontier.root();
        let next_receipt = RelationalMechanismSupportCheckpointReceipt { cursor, frontier };
        let checkpoint_required = durable != Some(next_receipt);
        // A final cursor is still checkpointed in its own quantum. The
        // semantic close is minted only on the next turn, when that exact
        // receipt is already part of the durable journal fold.
        if checkpoint_required {
            return Ok(RelationalMechanismSupportStepEvents::Checkpoint {
                accepted_target_cases,
                cursor,
                frontier_root,
                events: vec![RelationalJournalEvent::support_frontier_checkpointed(
                    request_id,
                    cursor,
                    frontier_root,
                )]
                .into_boxed_slice(),
            });
        }
        if cursor != available {
            return Err(RelationalJournalError::SupportCheckpointDidNotAdvance { request_id });
        }

        let upstreams_are_closed = self.state.analysis.as_ref().is_some_and(|analysis| {
            analysis.mechanism_closure(request_id).is_some()
                && analysis.structural_quotient_closure(request_id).is_some()
        });
        if !upstreams_are_closed {
            return Ok(RelationalMechanismSupportStepEvents::Idle);
        }

        let closure_event = self
            .state
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .support_closure_event(request_id)?;
        let RelationalAnalysisEvidenceEvent::SupportClosed { support_root, .. } = closure_event
        else {
            unreachable!("support closure factory returns its closure event")
        };

        Ok(RelationalMechanismSupportStepEvents::Closed {
            checkpointed_frontier: false,
            cursor,
            support_root,
            events: vec![RelationalJournalEvent::analysis(closure_event)].into_boxed_slice(),
        })
    }

    pub(crate) const fn analysis_state(&self) -> Option<&RelationalAnalysisJournalState> {
        self.state.analysis.as_ref()
    }

    fn next_mechanism_support_observation_claim(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<Option<MechanismSupportObservationClaim>, RelationalJournalError> {
        self.state
            .next_mechanism_support_observation_claim(request_id)
    }

    pub(crate) fn mechanism_support_observation_pending(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<bool, RelationalJournalError> {
        let Some(scheduler) = self
            .state
            .latest_support_schedulers
            .get(&request_id)
            .copied()
        else {
            return Ok(false);
        };
        let analysis = self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        if analysis.mechanism_support_closure(request_id).is_some() {
            let sealed = self
                .state
                .mechanism_support_observations
                .get(&request_id)
                .map_or(0, |log| log.automatic_sealed_slice_count);
            Ok(sealed != scheduler.registry().slice_count())
        } else {
            Ok(scheduler.dirty().slice_count() != 0)
        }
    }

    /// Read-only scheduling treats a frontier conflict as recovery work: it
    /// means a discarded bounded proposal or same-cursor seal enrichment must
    /// be re-emitted before an observation can be derived from durable state.
    pub(crate) fn mechanism_support_observation_or_recovery_pending(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<bool, RelationalJournalError> {
        let analysis = self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        let derived_cursor = analysis.mechanism_support_checkpoint_cursor(request_id);
        let durable = self.state.latest_support_frontiers.get(&request_id);
        // A first checkpoint is observable even for an empty prefix. Likewise,
        // a discarded proposal must be re-emitted before another request with
        // a continuously ready upstream lane can monopolize scheduling.
        if durable.is_none() || durable.is_some_and(|receipt| receipt.cursor != derived_cursor) {
            return Ok(true);
        }
        match self
            .state
            .next_mechanism_support_observation_claim(request_id)
        {
            Ok(claim) => Ok(claim.is_some()),
            Err(RelationalJournalError::Analysis(
                RelationalAnalysisJournalError::MechanismSupport(
                    MechanismSupportError::FrontierConflict,
                ),
            )) => Ok(true),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn mechanism_support_observation_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.points.len() as u128)
    }

    pub(crate) fn mechanism_support_automatic_observation_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.automatic_point_count)
    }

    pub(crate) fn mechanism_support_observation_at(
        &self,
        request_id: MechanismRequestId,
        ordinal: usize,
    ) -> Option<&MechanismSupportObservationPoint> {
        self.state
            .mechanism_support_observations
            .get(&request_id)?
            .points
            .get(ordinal)
    }

    pub(crate) fn mechanism_support_observation_latest(
        &self,
        slice: MechanismSupportSlice,
    ) -> Option<&MechanismSupportObservationPoint> {
        self.state
            .mechanism_support_observations
            .get(&slice.key().request_id())?
            .latest_point(slice)
    }

    pub(crate) fn mechanism_support_observation_first(
        &self,
        slice: MechanismSupportSlice,
    ) -> Option<(u128, &MechanismSupportObservationPoint)> {
        let (ordinal, point) = self
            .state
            .mechanism_support_observations
            .get(&slice.key().request_id())?
            .first_point(slice)?;
        Some((ordinal as u128, point))
    }

    pub(crate) fn mechanism_support_observation_chain_root(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismSupportObservationChainRoot> {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map(|log| log.chain_root)
    }

    pub(crate) fn mechanism_support_automatic_observation_chain_root(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismSupportObservationChainRoot> {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .filter(|log| log.automatic_point_count != 0)
            .map(|log| log.automatic_chain_root)
    }

    pub(crate) fn mechanism_support_observed_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.automatic_observed_slice_count)
    }

    pub(crate) fn mechanism_support_sealed_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.automatic_sealed_slice_count)
    }

    pub(crate) fn mechanism_support_registered_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .latest_support_schedulers
            .get(&request_id)
            .map_or(0, |scheduler| scheduler.registry().slice_count())
    }

    pub(crate) fn mechanism_support_dirty_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .latest_support_schedulers
            .get(&request_id)
            .map_or(0, |scheduler| scheduler.dirty().slice_count())
    }

    pub(crate) fn durable_mechanism_support_scheduler_summary(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismAutomaticObservationSchedulerSummary> {
        self.state
            .latest_support_schedulers
            .get(&request_id)
            .copied()
    }

    pub(crate) fn durable_explicit_mechanism_support_scheduler_summary(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismExplicitObservationSchedulerSummary> {
        self.state
            .latest_explicit_support_schedulers
            .get(&request_id)
            .copied()
    }

    pub(crate) fn mechanism_support_observation_demand_registered(
        &self,
        slice: MechanismSupportSlice,
    ) -> bool {
        self.state
            .mechanism_support_observation_demands
            .get(&slice.key().request_id())
            .and_then(|log| log.registration(slice))
            .is_some()
    }

    pub(crate) fn mechanism_support_observation_demand_is_sealed(
        &self,
        slice: MechanismSupportSlice,
    ) -> bool {
        self.mechanism_support_observation_demand_registered(slice)
            && self
                .mechanism_support_observation_latest(slice)
                .is_some_and(|point| point.status().is_sealed())
    }

    pub(crate) fn mechanism_support_observation_demand_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observation_demands
            .get(&request_id)
            .map_or(0, |log| log.registrations.len() as u128)
    }

    pub(crate) fn mechanism_support_observation_demand_at(
        &self,
        request_id: MechanismRequestId,
        ordinal: usize,
    ) -> Option<&MechanismSupportObservationDemandRegistrationClaim> {
        self.state
            .mechanism_support_observation_demands
            .get(&request_id)?
            .registrations
            .get(ordinal)
    }

    pub(crate) fn mechanism_support_explicit_observed_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.explicit_observed_slice_count)
    }

    pub(crate) fn mechanism_support_explicit_sealed_slice_count(
        &self,
        request_id: MechanismRequestId,
    ) -> u128 {
        self.state
            .mechanism_support_observations
            .get(&request_id)
            .map_or(0, |log| log.explicit_sealed_slice_count)
    }

    pub(crate) fn mechanism_support_initial_observation_point_id(
        &self,
        request_id: MechanismRequestId,
    ) -> Option<MechanismSupportObservationPointId> {
        self.state
            .mechanism_support_observations
            .get(&request_id)?
            .points
            .iter()
            .find(|point| {
                matches!(
                    point.slice().subject(),
                    MechanismSupportSubject::Mechanism(_)
                )
            })
            .map(MechanismSupportObservationPoint::point_id)
    }

    /// Prepare one bounded, deterministic checkpoint compaction. The receipt
    /// is minted from the current frontier and will be independently rederived
    /// during append/replay before any work record is removed.
    pub(crate) fn work_frontier_compaction_event(
        &self,
        maximum_nodes: NonZeroU32,
    ) -> Result<Option<RelationalJournalEvent>, RelationalJournalError> {
        Ok(self
            .state
            .work
            .compaction_receipt(maximum_nodes)?
            .map(RelationalJournalEvent::work_frontier_compacted))
    }

    /// Atomically validate and append one evidence event.
    pub(crate) fn append(
        &mut self,
        event: RelationalJournalEvent,
    ) -> Result<&RelationalJournalEntry, RelationalJournalError> {
        assert!(
            self.retain_history,
            "use append_streaming with an externally retained journal"
        );
        // Reserve before changing evidence state so the infallible push cannot
        // allocate after a successfully validated semantic mutation.
        self.entries.reserve(1);
        let entry = self.apply_next(event)?;
        self.entries.push(entry);
        Ok(self
            .entries
            .last()
            .expect("an appended relational journal entry exists"))
    }

    /// Apply one frame to a memory-bounded journal fold and return the owned
    /// chain entry for immediate durable installation. If installation fails,
    /// the coordinator must discard this in-memory fold and replay the last
    /// fsynced prefix; it must never publish the advanced head.
    pub(crate) fn append_streaming(
        &mut self,
        event: RelationalJournalEvent,
    ) -> Result<RelationalJournalEntry, RelationalJournalError> {
        assert!(
            !self.retain_history,
            "append_streaming is reserved for externally retained history"
        );
        self.apply_next(event)
    }

    fn apply_next(
        &mut self,
        mut event: RelationalJournalEvent,
    ) -> Result<RelationalJournalEntry, RelationalJournalError> {
        let sequence = self.sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(RelationalJournalError::SequenceOverflow)?;
        let previous = self.head;
        let next_head = journal_entry_head(self.contract.id(), sequence, previous, &event);
        let pending = self
            .state
            .constructor_interner
            .prepare_event(self.contract.relation_id(), &mut event);
        self.state.apply(&event)?;
        self.state.constructor_interner.commit(pending);
        self.sequence = next_sequence;
        self.head = next_head;
        Ok(RelationalJournalEntry {
            sequence,
            previous,
            event,
            head: next_head,
        })
    }

    /// Verify and replay a stored chain from genesis.
    pub(crate) fn replay(
        contract: RelationalJournalContract,
        entries: impl IntoIterator<Item = RelationalJournalEntry>,
    ) -> Result<Self, RelationalJournalError> {
        Self::replay_with_retention(contract, entries, true, None)
    }

    /// Replay a retained chain whose regional events require the exact
    /// producer-owned checked query and classification capsule. Keeping this
    /// authority out of the encoded event is intentional: journal bytes are
    /// evidence to re-check, never a self-authorizing proof program.
    pub(crate) fn replay_with_region_replay_authority(
        contract: RelationalJournalContract,
        entries: impl IntoIterator<Item = RelationalJournalEntry>,
        authority: Arc<RelationalRegionReplayAuthority>,
    ) -> Result<Self, RelationalJournalError> {
        Self::replay_with_retention(contract, entries, true, Some(authority))
    }

    /// Rebuild the production fold from durable segments without retaining a
    /// second in-memory copy of every decoded frame.
    pub(crate) fn replay_streaming(
        contract: RelationalJournalContract,
        entries: impl IntoIterator<Item = RelationalJournalEntry>,
    ) -> Result<Self, RelationalJournalError> {
        Self::replay_with_retention(contract, entries, false, None)
    }

    /// Incrementally apply one codec-validated durable entry without retaining
    /// its history beside the folded semantic state.
    pub(crate) fn replay_streaming_entry(
        &mut self,
        mut supplied: RelationalJournalEntry,
    ) -> Result<(), RelationalJournalError> {
        assert!(
            !self.retain_history,
            "incremental replay is reserved for streaming relational journals"
        );
        let expected_sequence = self.sequence;
        if supplied.sequence != expected_sequence {
            return Err(RelationalJournalError::SequenceMismatch {
                expected: expected_sequence,
                found: supplied.sequence,
            });
        }
        if supplied.previous != self.head {
            return Err(RelationalJournalError::PreviousHeadMismatch {
                sequence: supplied.sequence,
            });
        }
        let expected_head = journal_entry_head(
            self.contract.id(),
            supplied.sequence,
            supplied.previous,
            &supplied.event,
        );
        if supplied.head != expected_head {
            return Err(RelationalJournalError::EntryHeadMismatch {
                sequence: supplied.sequence,
            });
        }
        let pending = self
            .state
            .constructor_interner
            .prepare_event(self.contract.relation_id(), &mut supplied.event);
        self.state.apply(&supplied.event)?;
        self.state.constructor_interner.commit(pending);
        self.sequence = supplied
            .sequence
            .checked_add(1)
            .ok_or(RelationalJournalError::SequenceOverflow)?;
        self.head = supplied.head;
        Ok(())
    }

    fn replay_with_retention(
        contract: RelationalJournalContract,
        entries: impl IntoIterator<Item = RelationalJournalEntry>,
        retain_history: bool,
        region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    ) -> Result<Self, RelationalJournalError> {
        let mut journal =
            Self::with_history_retention(contract.clone(), retain_history, region_replay_authority);
        for mut supplied in entries {
            let expected_sequence = journal.sequence;
            if supplied.sequence != expected_sequence {
                return Err(RelationalJournalError::SequenceMismatch {
                    expected: expected_sequence,
                    found: supplied.sequence,
                });
            }
            if supplied.previous != journal.head {
                return Err(RelationalJournalError::PreviousHeadMismatch {
                    sequence: supplied.sequence,
                });
            }
            let expected_head = journal_entry_head(
                contract.id(),
                supplied.sequence,
                supplied.previous,
                &supplied.event,
            );
            if supplied.head != expected_head {
                return Err(RelationalJournalError::EntryHeadMismatch {
                    sequence: supplied.sequence,
                });
            }
            let pending = journal
                .state
                .constructor_interner
                .prepare_event(contract.relation_id(), &mut supplied.event);
            journal.state.apply(&supplied.event)?;
            journal.state.constructor_interner.commit(pending);
            journal.sequence = journal
                .sequence
                .checked_add(1)
                .ok_or(RelationalJournalError::SequenceOverflow)?;
            journal.head = supplied.head;
            if journal.retain_history {
                journal.entries.push(supplied);
            }
        }
        Ok(journal)
    }

    pub(crate) fn snapshot(&self) -> Result<RelationalJournalSnapshot, RelationalJournalError> {
        let relation = self.state.relation.snapshot();
        let admission_frontier_root = self.state.admission.frontier_root(relation.frontier_root());
        let question_frontier_roots = self
            .state
            .questions
            .iter()
            .map(|(question_id, question)| {
                (
                    *question_id,
                    question.frontier_root(admission_frontier_root),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let question_frontier_root =
            relational_question_frontier_set_root(&question_frontier_roots);
        let admission = self.state.admission.counts_at(&relation)?;
        let questions = self
            .state
            .questions
            .iter()
            .map(|(question_id, question)| {
                Ok((
                    *question_id,
                    question.counts_at(&relation, &self.state.admission)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RelationalJournalError>>()?;
        let analysis_plan_root = self
            .state
            .analysis_plan
            .as_ref()
            .map(RelationalAnalysisPlan::root);
        let support_plan_root = self
            .state
            .support_plan
            .as_ref()
            .map(RelationalSupportPlan::root);
        let source_traversal_frontier_root = self
            .state
            .source_traversal
            .as_ref()
            .map(SourceTraversalAccumulator::frontier_root);
        let source_relation_exhaustion_receipt_id = self
            .state
            .source_relation_exhaustion
            .as_ref()
            .map(SourceRelationExhaustionReceipt::id);
        let exhaustion_evidence_root = relational_exhaustion_evidence_root(
            self.state.source_traversal.as_ref(),
            self.state.source_relation_exhaustion.as_ref(),
            &self.state.successor_exhaustion_receipts,
        );
        let support = self.state.support.snapshot()?;
        let core_evidence_root = relational_core_evidence_root(
            &self.contract,
            analysis_plan_root,
            support_plan_root,
            exhaustion_evidence_root,
            relation.frontier_root(),
            admission_frontier_root,
            question_frontier_root,
            self.state.transition_support.root(),
            support.root(),
        );
        let analysis_scope_root = self
            .state
            .analysis
            .as_ref()
            .and_then(RelationalAnalysisJournalState::scope_root);
        let analysis_catalog = self
            .state
            .analysis
            .as_ref()
            .map(RelationalAnalysisJournalState::snapshot);
        let analysis_terminal_root = self
            .state
            .analysis
            .as_ref()
            .and_then(RelationalAnalysisJournalState::closed_catalog)
            .map(|catalog| catalog.root());
        let analysis_closure_set_root = self
            .state
            .analysis
            .as_ref()
            .and_then(RelationalAnalysisJournalState::closed_closure_set_root);
        let exploration_evidence_root = relational_exploration_evidence_root(
            &self.contract,
            core_evidence_root,
            analysis_scope_root,
            analysis_catalog.as_ref().map(|catalog| catalog.root()),
            analysis_terminal_root,
            analysis_closure_set_root,
        );
        let work = self.state.work.snapshot()?;
        let classified_chunk_accumulator = self.state.classified_chunk_accumulator.as_ref();
        let checkpoint_root = relational_checkpoint_root(
            &self.contract,
            question_frontier_root,
            work.root,
            &support,
            classified_chunk_accumulator,
            &self.state.latest_support_frontiers,
            &self.state.latest_support_schedulers,
            &self.state.latest_explicit_support_schedulers,
            &self.state.mechanism_support_observation_demands,
            &self.state.mechanism_support_observations,
        );
        Ok(RelationalJournalSnapshot {
            version: RELATIONAL_JOURNAL_SCHEMA_VERSION,
            contract: self.contract.clone(),
            sequence: self.sequence,
            head: self.head,
            relation_frontier_root: relation.frontier_root(),
            admission_frontier_root,
            question_frontier_root,
            question_frontier_roots,
            transition_support_root: self.state.transition_support.root(),
            transition_support_counts: self.state.transition_support.counts(),
            analysis_plan_root,
            support_plan_root,
            source_traversal_frontier_root,
            source_relation_exhaustion_receipt_id,
            exhaustion_evidence_root,
            support_evidence_root: support.root(),
            core_evidence_root,
            analysis_scope_root,
            analysis_catalog,
            analysis_terminal_root,
            analysis_closure_set_root,
            exploration_evidence_root,
            work_frontier_root: work.root,
            checkpoint_root,
            relation,
            admission,
            questions,
            support,
            work,
        })
    }

    /// Close the relation/classification/support core from proof evidence.
    ///
    /// This path requires no extensional `RelationContentRoot`. It is named as
    /// a core close because requested result and mechanism catalogs are not yet
    /// part of this journal state and therefore cannot be certified here.
    /// Open checkpoint work is not a semantic blocker: speculative or
    /// superseded scheduler nodes may remain after every committed obligation
    /// is discharged, and are still visible through the checkpoint root.
    pub(crate) fn finish_certified_core(
        &self,
        question_id: QuestionId,
    ) -> Result<ClosedCertifiedRelationalCore, RelationalJournalError> {
        self.state.question(question_id)?;
        let snapshot = self.snapshot()?;
        if snapshot.analysis_plan_root().is_none() {
            return Err(RelationalJournalError::AnalysisPlanMissing);
        }
        if snapshot.support_plan_root().is_none() {
            return Err(RelationalJournalError::SupportPlanMissing);
        }
        if !snapshot.support().catalog_is_sealed() {
            return Err(RelationalJournalError::SupportCatalogOpen);
        }
        if let Some(progress) = self.state.classified_sweep_progress.as_ref() {
            let committed = progress.committed_prefix_count();
            let total = self.state.classified_support_fragment_slots.len();
            if committed != total
                || self
                    .state
                    .classified_support_fragment_slots
                    .iter()
                    .any(Option::is_none)
                || self.state.classified_chunk_accumulator.is_some()
            {
                return Err(RelationalJournalError::ClassifiedSweepPrefixOpen {
                    committed: committed as u128,
                    total: total as u128,
                });
            }
        }
        let support_plan = self
            .state
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let selected_population = ClosedCertifiedSelectedPopulation::derive(
            support_plan,
            snapshot.support(),
            question_id,
        )?;
        Ok(ClosedCertifiedRelationalCore {
            contract: self.contract.clone(),
            journal_head: self.head,
            core_evidence_root: snapshot.core_evidence_root(),
            checkpoint_root: snapshot.checkpoint_root(),
            selected_population,
            snapshot,
        })
    }

    /// Finish a symbolically certified exploration only after the post-FIND
    /// analysis DAG has accepted its own terminal event and its selected seal
    /// still remints from this exact base prefix.
    pub(crate) fn finish_certified(
        self,
        question_id: QuestionId,
    ) -> Result<ClosedCertifiedRelationalEvidence, RelationalJournalError> {
        self.state.validate_closed_analysis_bridge()?;
        let core = self.finish_certified_core(question_id)?;
        let exploration_evidence_root = core.snapshot().exploration_evidence_root();
        let analysis = self
            .state
            .analysis
            .ok_or(RelationalJournalError::AnalysisStateMissing)?;
        Ok(ClosedCertifiedRelationalEvidence {
            core,
            exploration_evidence_root,
            analysis,
        })
    }

    /// Close the fully materialized relational path.
    ///
    /// This deliberately has an extensional name: exact symbolic closure is a
    /// different operation and must not be forced to mint a
    /// `RelationContentRoot` for cases it never materialized. Concrete catalog
    /// seals, rather than arbitrary checkpoint-node completion, are the
    /// exhaustion authority on this path.
    pub(crate) fn finish_extensional(
        self,
    ) -> Result<ClosedExtensionalRelationalEvidence, RelationalJournalError> {
        self.state.validate_closed_analysis_bridge()?;
        let final_snapshot = self.snapshot()?;
        let analysis_scope_root = final_snapshot
            .analysis_scope_root()
            .ok_or(RelationalJournalError::AnalysisNotClosed)?;
        let analysis_catalog_root = final_snapshot
            .analysis_terminal_root()
            .ok_or(RelationalJournalError::AnalysisNotClosed)?;
        let analysis_closure_set_root = final_snapshot
            .analysis_closure_set_root()
            .ok_or(RelationalJournalError::AnalysisNotClosed)?;
        let exploration_evidence_root = final_snapshot.exploration_evidence_root();
        let RelationalEvidenceState {
            contract: state_contract,
            scheduler_decision_counts: _,
            region_replay_authority: _,
            constructor_interner: _,
            relation,
            admission,
            questions,
            transition_support,
            analysis_plan,
            analysis,
            support_plan,
            source_image_exactness: _,
            source_traversal,
            source_relation_exhaustion,
            verified_case_chunk_partition,
            classified_sweep_progress: _,
            classified_chunk_accumulator,
            classified_support_fragment_slots: _,
            accepted_classified_fragment_count: _,
            case_support_discovery: _,
            classified_child_by_resolver_node: _,
            pending_classified_work_completion_ordinals: _,
            pending_selected_run_positions: _,
            selected_run_classified_verification_cache: _,
            selected_run_materializations: _,
            selected_run_materialization_ids: _,
            successor_exhaustion_receipts,
            support,
            latest_support_frontiers,
            latest_support_schedulers,
            latest_explicit_support_schedulers,
            mechanism_support_observation_demands,
            mechanism_support_observations,
            work,
        } = self.state;
        debug_assert_eq!(state_contract, self.contract);
        let analysis_plan_root = analysis_plan
            .as_ref()
            .map(RelationalAnalysisPlan::root)
            .ok_or(RelationalJournalError::AnalysisPlanMissing)?;
        let support_plan = support_plan.ok_or(RelationalJournalError::SupportPlanMissing)?;
        let support_plan_root = support_plan.root();
        let source_relation_exhaustion = source_relation_exhaustion
            .ok_or(RelationalJournalError::SourceRelationExhaustionReceiptMissing)?;
        let source_relation_exhaustion_receipt_id = source_relation_exhaustion.id();
        let exhaustion_evidence_root = relational_exhaustion_evidence_root(
            source_traversal.as_ref(),
            Some(&source_relation_exhaustion),
            &successor_exhaustion_receipts,
        );
        let work_snapshot = work.snapshot()?;
        let support = support.snapshot()?;
        let relation_frontier_root = relation.snapshot().frontier_root();
        let admission_frontier_root = admission.frontier_root(relation_frontier_root);
        let question_frontier_roots = questions
            .iter()
            .map(|(question_id, question)| {
                (
                    *question_id,
                    question.frontier_root(admission_frontier_root),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let question_frontier_root =
            relational_question_frontier_set_root(&question_frontier_roots);
        let core_evidence_root = relational_core_evidence_root(
            &self.contract,
            Some(analysis_plan_root),
            Some(support_plan_root),
            exhaustion_evidence_root,
            relation_frontier_root,
            admission_frontier_root,
            question_frontier_root,
            transition_support.root(),
            support.root(),
        );
        let relation = relation.finish()?;
        let admission = admission.finish(&relation)?;
        let questions = questions
            .into_iter()
            .map(|(question_id, question)| {
                Ok((question_id, question.finish(&relation, &admission)?))
            })
            .collect::<Result<BTreeMap<_, _>, RelationalJournalError>>()?;
        let question_content_roots = questions
            .iter()
            .map(|(question_id, question)| (*question_id, question.content_root()))
            .collect::<BTreeMap<_, _>>();
        let question_content_root = relational_question_content_set_root(&question_content_roots);
        let extensional_content_root = relational_extensional_content_root(
            &self.contract,
            analysis_plan_root,
            support_plan_root,
            source_relation_exhaustion_receipt_id,
            exhaustion_evidence_root,
            relation.content_root(),
            admission.content_root(),
            question_content_root,
            transition_support.root(),
            support.root(),
        );
        let classified_chunk_accumulator = classified_chunk_accumulator.as_ref();
        let checkpoint_root = relational_checkpoint_root(
            &self.contract,
            question_frontier_root,
            work_snapshot.root,
            &support,
            classified_chunk_accumulator,
            &latest_support_frontiers,
            &latest_support_schedulers,
            &latest_explicit_support_schedulers,
            &mechanism_support_observation_demands,
            &mechanism_support_observations,
        );
        let analysis = analysis.ok_or(RelationalJournalError::AnalysisStateMissing)?;
        Ok(ClosedExtensionalRelationalEvidence {
            contract: self.contract.clone(),
            journal_head: self.head,
            relation_content_root: relation.content_root(),
            admission_content_root: admission.content_root(),
            question_content_root,
            question_content_roots,
            analysis_plan_root,
            support_plan_root,
            source_relation_exhaustion_receipt_id,
            exhaustion_evidence_root,
            support_evidence_root: support.root(),
            core_evidence_root,
            analysis_scope_root,
            analysis_catalog_root,
            analysis_closure_set_root,
            exploration_evidence_root,
            extensional_content_root,
            work_frontier_root: work_snapshot.root,
            checkpoint_root,
            analysis,
            relation,
            admission,
            questions,
            transition_support,
            support,
        })
    }
}

/// Certified closure of the base relation/classification/support obligation
/// DAG. It deliberately exposes only frontier commitments for concrete rows;
/// callers must use `finish_extensional` to claim a `RelationContentRoot`.
#[derive(Clone, Debug)]
pub(crate) struct ClosedCertifiedRelationalCore {
    contract: RelationalJournalContract,
    journal_head: RelationalJournalHead,
    core_evidence_root: RelationalCoreEvidenceRoot,
    checkpoint_root: RelationalCheckpointRoot,
    selected_population: ClosedCertifiedSelectedPopulation,
    snapshot: RelationalJournalSnapshot,
}

impl ClosedCertifiedRelationalCore {
    pub(crate) const fn contract(&self) -> &RelationalJournalContract {
        &self.contract
    }

    pub(crate) const fn journal_head(&self) -> RelationalJournalHead {
        self.journal_head
    }

    pub(crate) const fn core_evidence_root(&self) -> RelationalCoreEvidenceRoot {
        self.core_evidence_root
    }

    pub(crate) const fn checkpoint_root(&self) -> RelationalCheckpointRoot {
        self.checkpoint_root
    }

    pub(crate) const fn selected_population(&self) -> &ClosedCertifiedSelectedPopulation {
        &self.selected_population
    }

    pub(crate) const fn support_evidence_root(&self) -> SupportEvidenceRoot {
        self.snapshot.support_evidence_root()
    }

    pub(crate) fn analysis_plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.snapshot
            .analysis_plan_root()
            .expect("a closed relational core always has an analysis plan")
    }

    pub(crate) const fn snapshot(&self) -> &RelationalJournalSnapshot {
        &self.snapshot
    }
}

/// Terminal whole-exploration artifact for a support-certified base. It keeps
/// the typed symbolic population distinct from an extensional relation root
/// while retaining immutable result views and mechanism-incidence closures.
#[derive(Clone, Debug)]
pub(crate) struct ClosedCertifiedRelationalEvidence {
    core: ClosedCertifiedRelationalCore,
    exploration_evidence_root: RelationalExplorationEvidenceRoot,
    analysis: RelationalAnalysisJournalState,
}

impl ClosedCertifiedRelationalEvidence {
    pub(crate) const fn core(&self) -> &ClosedCertifiedRelationalCore {
        &self.core
    }

    pub(crate) const fn exploration_evidence_root(&self) -> RelationalExplorationEvidenceRoot {
        self.exploration_evidence_root
    }

    pub(crate) const fn analysis(&self) -> &RelationalAnalysisJournalState {
        &self.analysis
    }

    pub(crate) fn analysis_catalog_root(&self) -> RelationalAnalysisCatalogRoot {
        self.analysis
            .closed_catalog()
            .expect("terminal certified evidence retains a closed analysis catalog")
            .root()
    }

    pub(crate) fn analysis_closure_set_root(&self) -> RelationalAnalysisClosureSetRoot {
        self.analysis
            .closed_closure_set_root()
            .expect("terminal certified evidence retains the analysis closure-set root")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RelationalJournalSnapshot {
    version: u32,
    contract: RelationalJournalContract,
    sequence: u64,
    head: RelationalJournalHead,
    relation_frontier_root: RelationFrontierRoot,
    admission_frontier_root: AdmissionFrontierRoot,
    /// Aggregate commitment to the exact canonical QuestionId -> frontier
    /// root map, including registered questions with empty prefixes.
    question_frontier_root: RelationalQuestionFrontierSetRoot,
    question_frontier_roots: BTreeMap<QuestionId, QuestionFrontierRoot>,
    transition_support_root: RelationalTransitionSupportRoot,
    transition_support_counts: RelationalTransitionSupportCounts,
    analysis_plan_root: Option<RelationalAnalysisPlanRoot>,
    support_plan_root: Option<RelationalSupportPlanRoot>,
    source_traversal_frontier_root: Option<SourceTraversalFrontierRoot>,
    source_relation_exhaustion_receipt_id: Option<SourceRelationExhaustionReceiptId>,
    exhaustion_evidence_root: RelationalExhaustionEvidenceRoot,
    support_evidence_root: SupportEvidenceRoot,
    core_evidence_root: RelationalCoreEvidenceRoot,
    analysis_scope_root: Option<RelationalAnalysisJournalScopeRoot>,
    analysis_catalog: Option<RelationalAnalysisCatalogSnapshot>,
    analysis_terminal_root: Option<RelationalAnalysisCatalogRoot>,
    analysis_closure_set_root: Option<RelationalAnalysisClosureSetRoot>,
    exploration_evidence_root: RelationalExplorationEvidenceRoot,
    work_frontier_root: WorkFrontierRoot,
    checkpoint_root: RelationalCheckpointRoot,
    relation: RelationCatalogSnapshot,
    admission: AdmissionCounts,
    questions: BTreeMap<QuestionId, SelectionCounts>,
    support: SupportEvidenceSnapshot,
    work: WorkFrontierSnapshot,
}

impl RelationalJournalSnapshot {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn contract(&self) -> &RelationalJournalContract {
        &self.contract
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn head(&self) -> RelationalJournalHead {
        self.head
    }

    pub(crate) const fn relation_frontier_root(&self) -> RelationFrontierRoot {
        self.relation_frontier_root
    }

    pub(crate) const fn admission_frontier_root(&self) -> AdmissionFrontierRoot {
        self.admission_frontier_root
    }

    pub(crate) const fn question_frontier_root(&self) -> RelationalQuestionFrontierSetRoot {
        self.question_frontier_root
    }

    pub(crate) fn question_frontier_roots(
        &self,
    ) -> impl ExactSizeIterator<Item = (QuestionId, QuestionFrontierRoot)> + '_ {
        self.question_frontier_roots
            .iter()
            .map(|(question_id, root)| (*question_id, *root))
    }

    pub(crate) fn question_frontier_root_for(
        &self,
        question_id: QuestionId,
    ) -> Option<QuestionFrontierRoot> {
        self.question_frontier_roots.get(&question_id).copied()
    }

    pub(crate) const fn transition_support_root(&self) -> RelationalTransitionSupportRoot {
        self.transition_support_root
    }

    pub(crate) const fn transition_support_counts(&self) -> &RelationalTransitionSupportCounts {
        &self.transition_support_counts
    }

    pub(crate) const fn analysis_plan_root(&self) -> Option<RelationalAnalysisPlanRoot> {
        self.analysis_plan_root
    }

    pub(crate) const fn support_plan_root(&self) -> Option<RelationalSupportPlanRoot> {
        self.support_plan_root
    }

    pub(crate) const fn source_traversal_frontier_root(
        &self,
    ) -> Option<SourceTraversalFrontierRoot> {
        self.source_traversal_frontier_root
    }

    pub(crate) const fn source_relation_exhaustion_receipt_id(
        &self,
    ) -> Option<SourceRelationExhaustionReceiptId> {
        self.source_relation_exhaustion_receipt_id
    }

    pub(crate) const fn exhaustion_evidence_root(&self) -> RelationalExhaustionEvidenceRoot {
        self.exhaustion_evidence_root
    }

    pub(crate) const fn support_evidence_root(&self) -> SupportEvidenceRoot {
        self.support_evidence_root
    }

    pub(crate) const fn core_evidence_root(&self) -> RelationalCoreEvidenceRoot {
        self.core_evidence_root
    }

    pub(crate) const fn analysis_scope_root(&self) -> Option<RelationalAnalysisJournalScopeRoot> {
        self.analysis_scope_root
    }

    pub(crate) const fn analysis_catalog(&self) -> Option<&RelationalAnalysisCatalogSnapshot> {
        self.analysis_catalog.as_ref()
    }

    pub(crate) const fn analysis_terminal_root(&self) -> Option<RelationalAnalysisCatalogRoot> {
        self.analysis_terminal_root
    }

    pub(crate) const fn analysis_closure_set_root(
        &self,
    ) -> Option<RelationalAnalysisClosureSetRoot> {
        self.analysis_closure_set_root
    }

    pub(crate) const fn exploration_evidence_root(&self) -> RelationalExplorationEvidenceRoot {
        self.exploration_evidence_root
    }

    pub(crate) const fn work_frontier_root(&self) -> WorkFrontierRoot {
        self.work_frontier_root
    }

    pub(crate) const fn checkpoint_root(&self) -> RelationalCheckpointRoot {
        self.checkpoint_root
    }

    pub(crate) fn relation(&self) -> &RelationCatalogSnapshot {
        &self.relation
    }

    pub(crate) const fn admission(&self) -> AdmissionCounts {
        self.admission
    }

    pub(crate) fn questions(
        &self,
    ) -> impl ExactSizeIterator<Item = (QuestionId, SelectionCounts)> + '_ {
        self.questions
            .iter()
            .map(|(question_id, counts)| (*question_id, *counts))
    }

    pub(crate) fn question(&self, question_id: QuestionId) -> Option<SelectionCounts> {
        self.questions.get(&question_id).copied()
    }

    pub(crate) const fn support(&self) -> &SupportEvidenceSnapshot {
        &self.support
    }

    pub(crate) fn work(&self) -> &WorkFrontierSnapshot {
        &self.work
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClosedExtensionalRelationalEvidence {
    contract: RelationalJournalContract,
    journal_head: RelationalJournalHead,
    relation_content_root: RelationContentRoot,
    admission_content_root: AdmissionContentRoot,
    /// Aggregate commitment to the canonical QuestionId -> content-root map.
    question_content_root: RelationalQuestionContentSetRoot,
    question_content_roots: BTreeMap<QuestionId, QuestionContentRoot>,
    analysis_plan_root: RelationalAnalysisPlanRoot,
    support_plan_root: RelationalSupportPlanRoot,
    source_relation_exhaustion_receipt_id: SourceRelationExhaustionReceiptId,
    exhaustion_evidence_root: RelationalExhaustionEvidenceRoot,
    support_evidence_root: SupportEvidenceRoot,
    core_evidence_root: RelationalCoreEvidenceRoot,
    analysis_scope_root: RelationalAnalysisJournalScopeRoot,
    analysis_catalog_root: RelationalAnalysisCatalogRoot,
    analysis_closure_set_root: RelationalAnalysisClosureSetRoot,
    exploration_evidence_root: RelationalExplorationEvidenceRoot,
    extensional_content_root: RelationalExtensionalContentRoot,
    work_frontier_root: WorkFrontierRoot,
    checkpoint_root: RelationalCheckpointRoot,
    analysis: RelationalAnalysisJournalState,
    relation: RelationCatalog,
    admission: AdmissionCatalog,
    questions: BTreeMap<QuestionId, QuestionCatalog>,
    transition_support: RelationalTransitionSupportIndex,
    support: SupportEvidenceSnapshot,
}

impl ClosedExtensionalRelationalEvidence {
    pub(crate) const fn contract(&self) -> &RelationalJournalContract {
        &self.contract
    }

    pub(crate) const fn journal_head(&self) -> RelationalJournalHead {
        self.journal_head
    }

    pub(crate) const fn relation_content_root(&self) -> RelationContentRoot {
        self.relation_content_root
    }

    pub(crate) const fn admission_content_root(&self) -> AdmissionContentRoot {
        self.admission_content_root
    }

    pub(crate) const fn question_content_root(&self) -> RelationalQuestionContentSetRoot {
        self.question_content_root
    }

    pub(crate) fn question_content_roots(
        &self,
    ) -> impl ExactSizeIterator<Item = (QuestionId, QuestionContentRoot)> + '_ {
        self.question_content_roots
            .iter()
            .map(|(question_id, root)| (*question_id, *root))
    }

    pub(crate) fn question_content_root_for(
        &self,
        question_id: QuestionId,
    ) -> Option<QuestionContentRoot> {
        self.question_content_roots.get(&question_id).copied()
    }

    pub(crate) const fn transition_support(&self) -> &RelationalTransitionSupportIndex {
        &self.transition_support
    }

    pub(crate) const fn analysis_plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.analysis_plan_root
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn source_relation_exhaustion_receipt_id(
        &self,
    ) -> SourceRelationExhaustionReceiptId {
        self.source_relation_exhaustion_receipt_id
    }

    pub(crate) const fn exhaustion_evidence_root(&self) -> RelationalExhaustionEvidenceRoot {
        self.exhaustion_evidence_root
    }

    pub(crate) const fn support_evidence_root(&self) -> SupportEvidenceRoot {
        self.support_evidence_root
    }

    pub(crate) const fn core_evidence_root(&self) -> RelationalCoreEvidenceRoot {
        self.core_evidence_root
    }

    pub(crate) const fn analysis_scope_root(&self) -> RelationalAnalysisJournalScopeRoot {
        self.analysis_scope_root
    }

    pub(crate) const fn analysis_catalog_root(&self) -> RelationalAnalysisCatalogRoot {
        self.analysis_catalog_root
    }

    pub(crate) const fn analysis_closure_set_root(&self) -> RelationalAnalysisClosureSetRoot {
        self.analysis_closure_set_root
    }

    pub(crate) const fn exploration_evidence_root(&self) -> RelationalExplorationEvidenceRoot {
        self.exploration_evidence_root
    }

    pub(crate) const fn extensional_content_root(&self) -> RelationalExtensionalContentRoot {
        self.extensional_content_root
    }

    pub(crate) const fn work_frontier_root(&self) -> WorkFrontierRoot {
        self.work_frontier_root
    }

    pub(crate) const fn checkpoint_root(&self) -> RelationalCheckpointRoot {
        self.checkpoint_root
    }

    pub(crate) const fn analysis(&self) -> &RelationalAnalysisJournalState {
        &self.analysis
    }

    pub(crate) fn relation(&self) -> &RelationCatalog {
        &self.relation
    }

    pub(crate) fn admission(&self) -> &AdmissionCatalog {
        &self.admission
    }

    pub(crate) fn question(&self, question_id: QuestionId) -> Option<&QuestionCatalog> {
        self.questions.get(&question_id)
    }

    pub(crate) fn questions(
        &self,
    ) -> impl ExactSizeIterator<Item = (QuestionId, &QuestionCatalog)> + '_ {
        self.questions
            .iter()
            .map(|(question_id, question)| (*question_id, question))
    }

    pub(crate) const fn support(&self) -> &SupportEvidenceSnapshot {
        &self.support
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalJournalError {
    Relation(RelationCatalogError),
    Classification(RelationClassificationError),
    TransitionSupport(RelationalTransitionSupportError),
    SupportEvidence(SupportEvidenceError),
    SupportJournal(SupportJournalError),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    SourceImageProof(RelationalSourceImageExactnessProofError),
    CertifiedSourceSummary(RelationalCertifiedSourceSummaryError),
    CaseChunkPartition(RelationalCaseChunkPartitionError),
    ClassifiedSweep(RelationalClassifiedSweepError),
    RegionProof(RelationalRegionProofError),
    SelectedRunMaterialization(RelationalSelectedRunMaterializationError),
    ClassificationCounts(CertifiedRelationalClassificationCountsError),
    UniformAdmissionProof(RelationalUniformAdmissionProofError),
    Work(WorkFrontierError),
    SourceTraversal(SourceTraversalClosureError),
    CertifiedPopulation(CertifiedSelectedPopulationError),
    Analysis(RelationalAnalysisJournalError),
    UnknownQuestion {
        question_id: QuestionId,
    },
    SingleQuestionOptimizationScopeMismatch {
        question_id: QuestionId,
    },
    AnalysisPlanRootMismatch {
        claimed: RelationalAnalysisPlanRoot,
        derived: RelationalAnalysisPlanRoot,
    },
    AnalysisPlanScopeMismatch,
    AnalysisPlanReplacement {
        first: RelationalAnalysisPlanRoot,
        second: RelationalAnalysisPlanRoot,
    },
    AnalysisPlanMissing,
    AnalysisStateMissing,
    SourceResultInputSealBaseMismatch,
    SelectedQuestionSealBaseMismatch,
    AnalysisNotClosed,
    SupportPlanRootMismatch {
        claimed: RelationalSupportPlanRoot,
        derived: RelationalSupportPlanRoot,
    },
    SupportPlanScopeMismatch,
    SupportPlanReplacement {
        first: RelationalSupportPlanRoot,
        second: RelationalSupportPlanRoot,
    },
    SupportPlanMissing,
    SourceImageCellMissing,
    SourceImageCellMismatch,
    SourceImageProofBindingMismatch,
    SourceImageCertifiedEvidenceMissing,
    SourceImageCertifiedEvidenceMismatch,
    SourceImageExactnessProofReplacement {
        first: [u8; 32],
        second: [u8; 32],
    },
    CaseImageRootInjectivityObligationMissing,
    CaseImageRootCardinalityObligationMissing,
    CaseChunkRootInjectivityEvidenceMissing,
    CaseChunkRootInjectivityEvidenceMismatch,
    CaseChunkRootAdmissionObligationMissing,
    CaseChunkRootAdmissionNotOpen,
    CaseChunkAdmissionRefinementMismatch,
    CaseChunkRootCursorAlreadyExists,
    ClassifiedSweepProgressMissing,
    ClassifiedSweepProgressScopeMismatch,
    ClassifiedSweepProgressGap {
        expected: u128,
        actual: u128,
    },
    ClassifiedSweepProgressConflict {
        chunk_ordinal: u128,
    },
    ClassifiedSweepProgressCoordinateMismatch {
        expected: u128,
        actual: u128,
    },
    ClassifiedSweepPrefixOpen {
        committed: u128,
        total: u128,
    },
    ClassifiedSweepConflictsWithSourceTraversal,
    SourceTraversalConflictsWithClassifiedSweep,
    ClassifiedRootMaterializationCheckpointForbidden,
    ClassifiedChunkCanonicalPartitionUnavailable,
    ClassifiedChunkPartitionIdentityMismatch,
    ClassifiedChunkRootInjectivityEvidenceMissing,
    ClassifiedChunkRootInjectivityEvidenceMismatch,
    ClassifiedChunkInjectivityEvidenceMissing,
    ClassifiedChunkInjectivityEvidenceMismatch,
    ClassifiedChunkSliceProgressMismatch {
        expected_chunk_ordinal: u128,
        actual_chunk_ordinal: u128,
    },
    ClassifiedChunkSliceAccumulatorMissing {
        chunk_ordinal: u128,
    },
    ClassifiedChunkSliceFinalArtifactMismatch {
        chunk_ordinal: u128,
    },
    ClassifiedChunkRootAdmissionRefinementMissing,
    ClassifiedChunkRootAdmissionRefinementMismatch,
    ClassifiedChunkAdmissionObligationMissing,
    ClassifiedChunkAdmissionStateMismatch,
    ClassifiedChunkCursorBoundsMismatch,
    ClassifiedChunkCursorCheckpointMismatch,
    ClassifiedChunkCursorPredecessorMissing {
        expected: u128,
    },
    ClassifiedChunkCursorPredecessorMismatch {
        expected: u128,
        actual: u128,
    },
    ClassifiedChunkArtifactRetentionGap {
        expected: u128,
        actual: u128,
    },
    ClassifiedChunkArtifactRetentionConflict {
        chunk_ordinal: u128,
    },
    ClassifiedChunkArtifactRetentionAllocationFailed,
    CaseSupportDiscoveryAllocationFailed,
    CaseSupportDiscoveryIndexMismatch {
        event_ordinal: usize,
    },
    RegionProofReplayAuthorityMissing,
    RegionProofReplayAuthorityMismatch,
    RegionProofSubjectMismatch,
    RegionProofConclusionUnsupported,
    RegionProofConflictsWithConcreteSlice,
    SelectedRunClassifiedArtifactMissing {
        chunk_ordinal: u128,
    },
    SelectedRunClassifiedArtifactMismatch,
    SelectedRunMaterializationConflict {
        run_cell_id: SupportCellId,
    },
    SelectedRunMaterializationArtifactIdentityCollision {
        artifact_id: RelationalSelectedRunMaterializationArtifactId,
    },
    SelectedRunCaseAlreadyMaterialized {
        case_id: RelationalCaseId,
    },
    SelectedRunCatalogBatchAllocationFailed,
    CertifiedSelectedMaterializationCoverageOpen,
    CertifiedSelectedMaterializationCaseSetMismatch {
        certified: u128,
        materialized: u128,
        catalog: u128,
    },
    UniformAdmissionRootObligationMissing,
    UniformAdmissionConcreteContradiction {
        certified: AdmissionDecision,
        concrete: AdmissionDecision,
    },
    InvalidSupportPlanActivation,
    ClassifiedSupportCoveragePending,
    StagedObligationActivationPending,
    InvalidSourceExhaustionReceipt,
    InvalidSuccessorExhaustionReceipt,
    ExhaustionReceiptCollision,
    ExhaustionReceiptMissing,
    ExhaustionReceiptCoverageMismatch,
    SourceTraversalMissing,
    SourceTraversalAlreadySealed,
    SourceEnumerationOpen,
    InvalidSourceRelationExhaustionReceipt,
    SourceRelationExhaustionReplacement,
    SourceRelationExhaustionReceiptMismatch,
    SourceRelationCoverageMismatch,
    SourceRelationExhaustionReceiptMissing,
    SourceKeyClaimMismatch {
        claimed: SourceKey,
        derived: SourceKey,
    },
    SuccessorKeyClaimMismatch {
        claimed: SuccessorKey,
        derived: SuccessorKey,
    },
    CaseIdClaimMismatch {
        claimed: RelationalCaseId,
        derived: RelationalCaseId,
    },
    WorkNodeIdClaimMismatch {
        claimed: WorkNodeId,
        derived: WorkNodeId,
    },
    SchedulerPolicyVersionMismatch {
        expected: u32,
        actual: u32,
    },
    CandidateNominationRootMissing {
        decision: RelationalSchedulerDecision,
    },
    UnexpectedCandidateNominationRoot {
        decision: RelationalSchedulerDecision,
    },
    SupportFrontierRootClaimMismatch {
        request_id: MechanismRequestId,
        claimed: MechanismSupportFrontierRoot,
        derived: MechanismSupportFrontierRoot,
    },
    SupportCheckpointCursorRegression {
        request_id: MechanismRequestId,
        lane: &'static str,
        current: u128,
        requested: u128,
    },
    SupportCheckpointLaneDeltaExceeded {
        request_id: MechanismRequestId,
        lane: &'static str,
        delta: u128,
        limit: u128,
    },
    SupportCheckpointDidNotAdvance {
        request_id: MechanismRequestId,
    },
    SupportCheckpointAnchorRootMismatch {
        request_id: MechanismRequestId,
    },
    SupportCheckpointObservationPending {
        request_id: MechanismRequestId,
    },
    SupportCheckpointAfterClosure {
        request_id: MechanismRequestId,
    },
    SupportClosureCheckpointIncomplete {
        request_id: MechanismRequestId,
    },
    SupportClosureFrontierCheckpointMissing {
        request_id: MechanismRequestId,
    },
    SupportObservationPointIdentityMismatch,
    SupportObservationFrontierMissing {
        request_id: MechanismRequestId,
    },
    SupportObservationFrontierMismatch {
        request_id: MechanismRequestId,
    },
    SupportObservationSummaryMismatch {
        request_id: MechanismRequestId,
    },
    SupportObservationStatusMismatch {
        request_id: MechanismRequestId,
    },
    SupportObservationSliceNotScheduled {
        request_id: MechanismRequestId,
    },
    SupportObservationSupersedesMismatch {
        request_id: MechanismRequestId,
    },
    SupportObservationAfterSeal {
        request_id: MechanismRequestId,
    },
    SupportObservationSealPredecessorMissing {
        request_id: MechanismRequestId,
    },
    SupportObservationDidNotAdvance {
        request_id: MechanismRequestId,
    },
    SupportObservationPointAllocationFailed {
        request_id: MechanismRequestId,
    },
    SupportObservationCountOverflow {
        request_id: MechanismRequestId,
    },
    SupportObservationClosurePending {
        request_id: MechanismRequestId,
    },
    SupportObservationDemandAlreadyRegistered {
        request_id: MechanismRequestId,
    },
    SupportObservationDemandMissing {
        request_id: MechanismRequestId,
    },
    SupportObservationDemandClaimMismatch {
        request_id: MechanismRequestId,
    },
    SupportObservationBackfillClaimMismatch {
        request_id: MechanismRequestId,
    },
    ReadinessRelationMismatch,
    UnknownReadinessSource {
        source_key: SourceKey,
    },
    UnknownReadinessCase {
        case_id: RelationalCaseId,
    },
    UnknownReadinessCell {
        cell_id: SupportCellId,
    },
    ReadinessCompletionMustBeDerived,
    CompletionRelationMismatch,
    CompletionPrecedesEvidence,
    CompletionEvidenceSubjectMismatch,
    OpenWorkFrontier {
        remaining: usize,
    },
    SupportCatalogOpen,
    SequenceOverflow,
    SequenceMismatch {
        expected: u64,
        found: u64,
    },
    PreviousHeadMismatch {
        sequence: u64,
    },
    EntryHeadMismatch {
        sequence: u64,
    },
}

impl From<RelationCatalogError> for RelationalJournalError {
    fn from(error: RelationCatalogError) -> Self {
        Self::Relation(error)
    }
}

impl From<RelationClassificationError> for RelationalJournalError {
    fn from(error: RelationClassificationError) -> Self {
        Self::Classification(error)
    }
}

impl From<RelationalTransitionSupportError> for RelationalJournalError {
    fn from(error: RelationalTransitionSupportError) -> Self {
        Self::TransitionSupport(error)
    }
}

impl From<SelectedCaseBatchError> for RelationalJournalError {
    fn from(error: SelectedCaseBatchError) -> Self {
        match error {
            SelectedCaseBatchError::Catalog(error) => Self::Relation(error),
            SelectedCaseBatchError::Classification(error) => Self::Classification(error),
            SelectedCaseBatchError::InvalidQuestionSet => {
                Self::ClassifiedSweep(RelationalClassifiedSweepError::InvalidQuestionSet)
            }
            SelectedCaseBatchError::UnknownQuestion { question_id } => {
                Self::UnknownQuestion { question_id }
            }
            SelectedCaseBatchError::SourceKeyClaimMismatch { claimed, derived } => {
                Self::SourceKeyClaimMismatch { claimed, derived }
            }
            SelectedCaseBatchError::SuccessorKeyClaimMismatch { claimed, derived } => {
                Self::SuccessorKeyClaimMismatch { claimed, derived }
            }
            SelectedCaseBatchError::CaseIdClaimMismatch { claimed, derived } => {
                Self::CaseIdClaimMismatch { claimed, derived }
            }
            SelectedCaseBatchError::DuplicateCase { case_id }
            | SelectedCaseBatchError::CaseAlreadyPresent { case_id } => {
                Self::SelectedRunCaseAlreadyMaterialized { case_id }
            }
            SelectedCaseBatchError::AllocationFailed => {
                Self::SelectedRunCatalogBatchAllocationFailed
            }
        }
    }
}

impl From<SupportEvidenceError> for RelationalJournalError {
    fn from(error: SupportEvidenceError) -> Self {
        Self::SupportEvidence(error)
    }
}

impl From<SupportJournalError> for RelationalJournalError {
    fn from(error: SupportJournalError) -> Self {
        Self::SupportJournal(error)
    }
}

impl From<RelationalCaseImageInjectivityProofError> for RelationalJournalError {
    fn from(error: RelationalCaseImageInjectivityProofError) -> Self {
        Self::CaseImageProof(error)
    }
}

impl From<RelationalSourceImageExactnessProofError> for RelationalJournalError {
    fn from(error: RelationalSourceImageExactnessProofError) -> Self {
        Self::SourceImageProof(error)
    }
}

impl From<RelationalCertifiedSourceSummaryError> for RelationalJournalError {
    fn from(error: RelationalCertifiedSourceSummaryError) -> Self {
        Self::CertifiedSourceSummary(error)
    }
}

impl From<RelationalCaseChunkPartitionError> for RelationalJournalError {
    fn from(error: RelationalCaseChunkPartitionError) -> Self {
        Self::CaseChunkPartition(error)
    }
}

impl From<RelationalClassifiedSweepError> for RelationalJournalError {
    fn from(error: RelationalClassifiedSweepError) -> Self {
        Self::ClassifiedSweep(error)
    }
}

impl From<RelationalRegionProofError> for RelationalJournalError {
    fn from(error: RelationalRegionProofError) -> Self {
        Self::RegionProof(error)
    }
}

impl From<RelationalSelectedRunMaterializationError> for RelationalJournalError {
    fn from(error: RelationalSelectedRunMaterializationError) -> Self {
        Self::SelectedRunMaterialization(error)
    }
}

impl From<CertifiedRelationalClassificationCountsError> for RelationalJournalError {
    fn from(error: CertifiedRelationalClassificationCountsError) -> Self {
        Self::ClassificationCounts(error)
    }
}

impl From<RelationalUniformAdmissionProofError> for RelationalJournalError {
    fn from(error: RelationalUniformAdmissionProofError) -> Self {
        Self::UniformAdmissionProof(error)
    }
}

impl From<WorkFrontierError> for RelationalJournalError {
    fn from(error: WorkFrontierError) -> Self {
        Self::Work(error)
    }
}

impl From<SourceTraversalClosureError> for RelationalJournalError {
    fn from(error: SourceTraversalClosureError) -> Self {
        Self::SourceTraversal(error)
    }
}

impl From<CertifiedSelectedPopulationError> for RelationalJournalError {
    fn from(error: CertifiedSelectedPopulationError) -> Self {
        Self::CertifiedPopulation(error)
    }
}

impl From<RelationalAnalysisJournalError> for RelationalJournalError {
    fn from(error: RelationalAnalysisJournalError) -> Self {
        Self::Analysis(error)
    }
}

impl fmt::Display for RelationalJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Relation(error) => fmt::Display::fmt(error, formatter),
            Self::Classification(error) => fmt::Display::fmt(error, formatter),
            Self::TransitionSupport(error) => fmt::Display::fmt(error, formatter),
            Self::SupportEvidence(error) => fmt::Display::fmt(error, formatter),
            Self::SupportJournal(error) => fmt::Display::fmt(error, formatter),
            Self::CaseImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::SourceImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::CertifiedSourceSummary(error) => fmt::Display::fmt(error, formatter),
            Self::CaseChunkPartition(error) => fmt::Display::fmt(error, formatter),
            Self::ClassifiedSweep(error) => fmt::Display::fmt(error, formatter),
            Self::RegionProof(error) => fmt::Display::fmt(error, formatter),
            Self::SelectedRunMaterialization(error) => fmt::Display::fmt(error, formatter),
            Self::ClassificationCounts(error) => fmt::Display::fmt(error, formatter),
            Self::UniformAdmissionProof(error) => fmt::Display::fmt(error, formatter),
            Self::Work(error) => fmt::Display::fmt(error, formatter),
            Self::SourceTraversal(error) => fmt::Display::fmt(error, formatter),
            Self::CertifiedPopulation(error) => fmt::Display::fmt(error, formatter),
            Self::Analysis(error) => fmt::Display::fmt(error, formatter),
            Self::UnknownQuestion { question_id } => write!(
                formatter,
                "relational journal question {question_id:?} is not registered by its contract"
            ),
            Self::SingleQuestionOptimizationScopeMismatch { question_id } => write!(
                formatter,
                "classified-region optimization for question {question_id:?} requires a contract with exactly that one question"
            ),
            Self::AnalysisPlanRootMismatch { .. } => formatter
                .write_str("relational analysis-plan root does not match its complete payload"),
            Self::AnalysisPlanScopeMismatch => formatter.write_str(
                "relational analysis plan belongs to another question or checked analysis graph",
            ),
            Self::AnalysisPlanReplacement { .. } => formatter
                .write_str("relational journal cannot replace its registered analysis plan"),
            Self::AnalysisPlanMissing => formatter.write_str(
                "relational core cannot close before the complete analysis DAG is registered",
            ),
            Self::AnalysisStateMissing => formatter.write_str(
                "relational analysis evidence cannot precede analysis-plan state registration",
            ),
            Self::SourceResultInputSealBaseMismatch => formatter.write_str(
                "source result-input seal does not match this journal's exact source closure",
            ),
            Self::SelectedQuestionSealBaseMismatch => formatter.write_str(
                "selected-question seal does not match this journal's exact base evidence",
            ),
            Self::AnalysisNotClosed => formatter.write_str(
                "relational exploration cannot finish before the analysis DAG closes",
            ),
            Self::SupportPlanRootMismatch { .. } => formatter
                .write_str("relational support-plan root does not match its complete payload"),
            Self::SupportPlanScopeMismatch => formatter
                .write_str("relational support plan belongs to another checked query layer"),
            Self::SupportPlanReplacement { .. } => formatter
                .write_str("relational journal cannot replace its registered support plan"),
            Self::SupportPlanMissing => formatter
                .write_str("relational support evidence cannot precede support-plan registration"),
            Self::SourceImageCellMissing => formatter.write_str(
                "source-image exactness proof has no matching source cell in the installed plan catalog",
            ),
            Self::SourceImageCellMismatch => formatter.write_str(
                "source-image exactness proof source cell differs from the installed support catalog",
            ),
            Self::SourceImageProofBindingMismatch => formatter.write_str(
                "source-image exactness proof reminted a different plan-bound population binding",
            ),
            Self::SourceImageCertifiedEvidenceMissing => formatter.write_str(
                "certified source population is missing a declared and accepted typed evidence record",
            ),
            Self::SourceImageCertifiedEvidenceMismatch => formatter.write_str(
                "certified source population durable evidence differs from the reverified proof",
            ),
            Self::SourceImageExactnessProofReplacement { .. } => formatter.write_str(
                "relational journal cannot replace its retained source-image exactness proof",
            ),
            Self::CaseImageRootInjectivityObligationMissing => formatter.write_str(
                "relational case-image proof has no matching declared root injectivity obligation",
            ),
            Self::CaseImageRootCardinalityObligationMissing => formatter.write_str(
                "relational case-image proof has no matching declared root cardinality obligation",
            ),
            Self::CaseChunkRootInjectivityEvidenceMissing => formatter.write_str(
                "case-chunk partition precedes its exact durable root injectivity evidence",
            ),
            Self::CaseChunkRootInjectivityEvidenceMismatch => formatter.write_str(
                "case-chunk partition root injectivity evidence names another claim subject",
            ),
            Self::CaseChunkRootAdmissionObligationMissing => formatter.write_str(
                "case-chunk partition has no matching declared root admission obligation",
            ),
            Self::CaseChunkRootAdmissionNotOpen => formatter.write_str(
                "case-chunk partition cannot refine an admission obligation already resolved another way",
            ),
            Self::CaseChunkAdmissionRefinementMismatch => formatter.write_str(
                "retained case-chunk artifact disagrees with the durable admission refinement",
            ),
            Self::CaseChunkRootCursorAlreadyExists => formatter.write_str(
                "case-chunk partition cannot reserve a root already carrying a generic materialization cursor",
            ),
            Self::ClassifiedSweepProgressMissing => formatter.write_str(
                "classified chunk precedes its typed canonical partition progress record",
            ),
            Self::ClassifiedSweepProgressScopeMismatch => formatter.write_str(
                "classified progress belongs to another partition, root, materializer, or interval",
            ),
            Self::ClassifiedSweepProgressGap { expected, actual } => write!(
                formatter,
                "classified progress expected chunk ordinal {expected}, found {actual}"
            ),
            Self::ClassifiedSweepProgressConflict { chunk_ordinal } => write!(
                formatter,
                "classified progress ordinal {chunk_ordinal} is already bound to another artifact"
            ),
            Self::ClassifiedSweepProgressCoordinateMismatch { expected, actual } => write!(
                formatter,
                "classified progress expected relative coordinate {expected}, found {actual}"
            ),
            Self::ClassifiedSweepPrefixOpen { committed, total } => write!(
                formatter,
                "classified support closure requires the complete canonical prefix; committed {committed} of {total} chunks"
            ),
            Self::ClassifiedSweepConflictsWithSourceTraversal => formatter.write_str(
                "classified sweep cannot begin or advance after concrete source traversal",
            ),
            Self::SourceTraversalConflictsWithClassifiedSweep => formatter.write_str(
                "concrete source traversal cannot begin or seal after classified sweep evidence",
            ),
            Self::ClassifiedRootMaterializationCheckpointForbidden => formatter.write_str(
                "generic materialization checkpoints cannot advance a classified case root",
            ),
            Self::ClassifiedChunkCanonicalPartitionUnavailable => formatter.write_str(
                "classified-chunk acceptance requires an accepted proper bounded case partition",
            ),
            Self::ClassifiedChunkPartitionIdentityMismatch => formatter.write_str(
                "classified chunk does not name the canonical installed case partition and child",
            ),
            Self::ClassifiedChunkRootInjectivityEvidenceMissing => formatter.write_str(
                "classified chunk precedes its exact durable root injectivity evidence",
            ),
            Self::ClassifiedChunkRootInjectivityEvidenceMismatch => formatter.write_str(
                "classified chunk root injectivity evidence differs from the checked producer proof",
            ),
            Self::ClassifiedChunkInjectivityEvidenceMissing => formatter.write_str(
                "classified chunk precedes its exact durable child injectivity evidence",
            ),
            Self::ClassifiedChunkInjectivityEvidenceMismatch => formatter.write_str(
                "classified chunk child injectivity evidence differs from the canonical restriction",
            ),
            Self::ClassifiedChunkSliceProgressMismatch {
                expected_chunk_ordinal,
                actual_chunk_ordinal,
            } => write!(
                formatter,
                "classified slice expected canonical chunk ordinal {expected_chunk_ordinal}, found {actual_chunk_ordinal}"
            ),
            Self::ClassifiedChunkSliceAccumulatorMissing { chunk_ordinal } => write!(
                formatter,
                "classified chunk {chunk_ordinal} has no complete authenticated slice accumulator"
            ),
            Self::ClassifiedChunkSliceFinalArtifactMismatch { chunk_ordinal } => write!(
                formatter,
                "classified chunk {chunk_ordinal} differs from the canonical artifact finalized from its authenticated slices"
            ),
            Self::ClassifiedChunkRootAdmissionRefinementMissing => formatter.write_str(
                "classified chunk precedes the canonical root-to-chunk admission refinement",
            ),
            Self::ClassifiedChunkRootAdmissionRefinementMismatch => formatter.write_str(
                "classified chunk disagrees with the durable root-to-chunk admission refinement",
            ),
            Self::ClassifiedChunkAdmissionObligationMissing => formatter.write_str(
                "classified chunk has no matching declared chunk admission obligation",
            ),
            Self::ClassifiedChunkAdmissionStateMismatch => formatter.write_str(
                "classified chunk admission is already proved or refined incompatibly",
            ),
            Self::ClassifiedChunkCursorBoundsMismatch => formatter.write_str(
                "classified chunk bounds do not map to a valid root materialization cursor range",
            ),
            Self::ClassifiedChunkCursorCheckpointMismatch => formatter.write_str(
                "classified chunk cursor endpoint is already bound to another checkpoint",
            ),
            Self::ClassifiedChunkCursorPredecessorMissing { expected } => write!(
                formatter,
                "classified chunk cursor has no durable predecessor at relative ordinal {expected}"
            ),
            Self::ClassifiedChunkCursorPredecessorMismatch { expected, actual } => write!(
                formatter,
                "classified chunk cursor expected relative predecessor {expected}, found {actual}"
            ),
            Self::ClassifiedChunkArtifactRetentionGap { expected, actual } => write!(
                formatter,
                "classified chunk payload retention expected ordinal {expected}, found {actual}"
            ),
            Self::ClassifiedChunkArtifactRetentionConflict { chunk_ordinal } => write!(
                formatter,
                "classified chunk ordinal {chunk_ordinal} is already bound to another retained payload"
            ),
            Self::ClassifiedChunkArtifactRetentionAllocationFailed => formatter.write_str(
                "classified chunk payload retention exceeded available bounded memory",
            ),
            Self::CaseSupportDiscoveryAllocationFailed => formatter.write_str(
                "case/support publication discovery could not reserve one bounded coordinate",
            ),
            Self::CaseSupportDiscoveryIndexMismatch { event_ordinal } => write!(
                formatter,
                "case/support publication discovery coordinate {event_ordinal} does not resolve to its retained artifact",
            ),
            Self::RegionProofReplayAuthorityMissing => formatter.write_str(
                "relational region proof replay has no producer-owned checked authority",
            ),
            Self::RegionProofReplayAuthorityMismatch => formatter.write_str(
                "relational region proof replay authority does not match its capsule or support plan",
            ),
            Self::RegionProofSubjectMismatch => formatter.write_str(
                "relational region proof does not name the next canonical partition child",
            ),
            Self::RegionProofConclusionUnsupported => formatter.write_str(
                "relational region proof conclusion is outside the zero-selected V1 policy",
            ),
            Self::RegionProofConflictsWithConcreteSlice => formatter.write_str(
                "relational region proof cannot supersede a checkpointed concrete child slice",
            ),
            Self::SelectedRunClassifiedArtifactMissing { chunk_ordinal } => write!(
                formatter,
                "selected-run materialization has no accepted classified chunk at ordinal {chunk_ordinal}"
            ),
            Self::SelectedRunClassifiedArtifactMismatch => formatter.write_str(
                "selected-run materialization names a different classified chunk artifact",
            ),
            Self::SelectedRunMaterializationConflict { .. } => formatter.write_str(
                "selected support run is already bound to another concrete materialization artifact",
            ),
            Self::SelectedRunMaterializationArtifactIdentityCollision { .. } => formatter.write_str(
                "selected-run materialization artifact identity collides with another support run",
            ),
            Self::SelectedRunCaseAlreadyMaterialized { .. } => formatter.write_str(
                "selected-run materialization repeats a concrete CaseId from another accepted run",
            ),
            Self::SelectedRunCatalogBatchAllocationFailed => formatter.write_str(
                "selected-run materialization could not reserve its bounded catalog delta",
            ),
            Self::CertifiedSelectedMaterializationCoverageOpen => formatter.write_str(
                "certified selected closure requires the complete classified partition and every selected-run materialization",
            ),
            Self::CertifiedSelectedMaterializationCaseSetMismatch { .. } => formatter.write_str(
                "concrete selected cases do not exactly cover the certified support population",
            ),
            Self::UniformAdmissionRootObligationMissing => formatter.write_str(
                "relational uniform-admission proof has no matching declared root admission obligation",
            ),
            Self::UniformAdmissionConcreteContradiction {
                certified,
                concrete,
            } => write!(
                formatter,
                "concrete admission decision {concrete:?} contradicts certified uniform root decision {certified:?}",
            ),
            Self::InvalidSupportPlanActivation => formatter
                .write_str("relational support plan contains an invalid staged activation"),
            Self::ClassifiedSupportCoveragePending => formatter.write_str(
                "classified support cannot seal before every canonical chunk artifact is accepted",
            ),
            Self::StagedObligationActivationPending => formatter.write_str(
                "support obligation discovery cannot seal before admission-driven FIND work is known",
            ),
            Self::InvalidSourceExhaustionReceipt => formatter
                .write_str("source-binding exhaustion receipt is invalid or out of scope"),
            Self::InvalidSuccessorExhaustionReceipt => formatter
                .write_str("successor-fiber exhaustion receipt is invalid or out of scope"),
            Self::ExhaustionReceiptCollision => formatter
                .write_str("exhaustion receipt ID has conflicting content"),
            Self::ExhaustionReceiptMissing => formatter
                .write_str("semantic enumeration seal precedes its producer exhaustion receipt"),
            Self::ExhaustionReceiptCoverageMismatch => formatter.write_str(
                "producer exhaustion receipt does not cover the discovered relation members",
            ),
            Self::SourceTraversalMissing => formatter.write_str(
                "source traversal evidence cannot precede support-plan registration",
            ),
            Self::SourceTraversalAlreadySealed => formatter.write_str(
                "new source traversal evidence cannot follow aggregate source closure",
            ),
            Self::SourceEnumerationOpen => formatter.write_str(
                "source result input cannot seal before exact source traversal closure",
            ),
            Self::InvalidSourceRelationExhaustionReceipt => formatter.write_str(
                "aggregate source-relation exhaustion receipt is invalid or out of scope",
            ),
            Self::SourceRelationExhaustionReplacement => formatter.write_str(
                "sealed source-relation exhaustion evidence cannot be replaced",
            ),
            Self::SourceRelationExhaustionReceiptMismatch => formatter.write_str(
                "aggregate source-relation receipt differs from the verified traversal tree",
            ),
            Self::SourceRelationCoverageMismatch => formatter.write_str(
                "aggregate source-relation receipt does not match discovered source rows",
            ),
            Self::SourceRelationExhaustionReceiptMissing => formatter.write_str(
                "source relation cannot seal without an aggregate producer-exhaustion receipt",
            ),
            Self::SourceKeyClaimMismatch { .. } => {
                formatter.write_str("relational journal SourceKey claim does not match row")
            }
            Self::SuccessorKeyClaimMismatch { .. } => formatter
                .write_str("relational journal SuccessorKey claim does not match successor row"),
            Self::CaseIdClaimMismatch { .. } => {
                formatter.write_str("relational journal CaseId claim does not match successor row")
            }
            Self::WorkNodeIdClaimMismatch { .. } => {
                formatter.write_str("relational journal WorkNodeId claim does not match work spec")
            }
            Self::SchedulerPolicyVersionMismatch { expected, actual } => write!(
                formatter,
                "relational scheduler policy version {actual} does not match supported version {expected}",
            ),
            Self::CandidateNominationRootMissing { decision } => write!(
                formatter,
                "relational candidate scheduler decision {decision:?} is missing its nomination root",
            ),
            Self::UnexpectedCandidateNominationRoot { decision } => write!(
                formatter,
                "non-candidate relational scheduler decision {decision:?} carries a nomination root",
            ),
            Self::SupportFrontierRootClaimMismatch { .. } => formatter.write_str(
                "relational journal mechanism-support frontier claim does not match replay state",
            ),
            Self::SupportCheckpointCursorRegression { lane, .. } => write!(
                formatter,
                "relational journal mechanism-support {lane} checkpoint cursor regressed",
            ),
            Self::SupportCheckpointLaneDeltaExceeded { lane, .. } => write!(
                formatter,
                "relational journal mechanism-support {lane} checkpoint delta exceeds the protocol bound",
            ),
            Self::SupportCheckpointDidNotAdvance { .. } => formatter.write_str(
                "mechanism-support checkpoint planning made no progress before full closure",
            ),
            Self::SupportCheckpointAnchorRootMismatch { .. } => formatter.write_str(
                "mechanism-support derived state does not match its latest durable checkpoint anchor",
            ),
            Self::SupportCheckpointObservationPending { .. } => formatter.write_str(
                "mechanism-support checkpoint cannot advance past an unobserved dirty mechanism slice",
            ),
            Self::SupportCheckpointAfterClosure { .. } => formatter.write_str(
                "mechanism-support checkpoint cannot advance or replace a closed request",
            ),
            Self::SupportClosureCheckpointIncomplete { .. } => formatter.write_str(
                "mechanism-support closure requires every authenticated checkpoint lane to be complete",
            ),
            Self::SupportClosureFrontierCheckpointMissing { .. } => formatter.write_str(
                "mechanism-support closure requires its complete durable frontier checkpoint",
            ),
            Self::SupportObservationPointIdentityMismatch => formatter.write_str(
                "mechanism-support observation point identity does not match its claim",
            ),
            Self::SupportObservationFrontierMissing { .. } => formatter.write_str(
                "mechanism-support observation requires a durable frontier checkpoint",
            ),
            Self::SupportObservationFrontierMismatch { .. } => formatter.write_str(
                "mechanism-support observation does not match the latest durable frontier",
            ),
            Self::SupportObservationSummaryMismatch { .. } => formatter.write_str(
                "mechanism-support observation summary differs from replay-derived support",
            ),
            Self::SupportObservationStatusMismatch { .. } => formatter.write_str(
                "mechanism-support observation status differs from replay lifecycle state",
            ),
            Self::SupportObservationSliceNotScheduled { .. } => formatter.write_str(
                "mechanism-support observation slice has not been scheduled by this journal",
            ),
            Self::SupportObservationSupersedesMismatch { .. } => formatter.write_str(
                "mechanism-support observation does not extend the latest point for its slice",
            ),
            Self::SupportObservationAfterSeal { .. } => formatter.write_str(
                "sealed mechanism-support observation slices cannot be extended",
            ),
            Self::SupportObservationSealPredecessorMissing { .. } => formatter.write_str(
                "sealed mechanism-support observation requires an earlier open point",
            ),
            Self::SupportObservationDidNotAdvance { .. } => formatter.write_str(
                "mechanism-support observation duplicates its previous durable state",
            ),
            Self::SupportObservationPointAllocationFailed { .. } => formatter.write_str(
                "mechanism-support observation could not reserve durable point storage",
            ),
            Self::SupportObservationCountOverflow { .. } => formatter.write_str(
                "mechanism-support observation counters exceed the journal representation",
            ),
            Self::SupportObservationClosurePending { .. } => formatter.write_str(
                "mechanism-support lifecycle cannot close while a scheduled observation is pending or unsealed",
            ),
            Self::SupportObservationDemandAlreadyRegistered { .. } => formatter.write_str(
                "mechanism-support observation demand repeats an already journaled slice",
            ),
            Self::SupportObservationDemandMissing { .. } => formatter.write_str(
                "mechanism-support observation work has no registered slice demand",
            ),
            Self::SupportObservationDemandClaimMismatch { .. } => formatter.write_str(
                "mechanism-support observation registration differs from replay-derived state",
            ),
            Self::SupportObservationBackfillClaimMismatch { .. } => formatter.write_str(
                "mechanism-support observation backfill differs from the canonical bounded page",
            ),
            Self::ReadinessRelationMismatch => {
                formatter.write_str("relational journal readiness belongs to another relation")
            }
            Self::UnknownReadinessSource { .. } => {
                formatter.write_str("relational journal source readiness precedes source evidence")
            }
            Self::UnknownReadinessCase { .. } => {
                formatter.write_str("relational journal case readiness precedes case evidence")
            }
            Self::UnknownReadinessCell { .. } => {
                formatter.write_str("relational journal support readiness precedes cell evidence")
            }
            Self::ReadinessCompletionMustBeDerived => formatter.write_str(
                "readiness completion is derived by materialization, not supplied evidence",
            ),
            Self::CompletionRelationMismatch => {
                formatter.write_str("relational journal completion belongs to another relation")
            }
            Self::CompletionPrecedesEvidence => formatter
                .write_str("relational journal work completion precedes its durable evidence"),
            Self::CompletionEvidenceSubjectMismatch => formatter
                .write_str("relational journal work completion evidence names another subject"),
            Self::OpenWorkFrontier { remaining } => write!(
                formatter,
                "relational journal cannot finish with {remaining} open work nodes"
            ),
            Self::SupportCatalogOpen => formatter
                .write_str("relational journal cannot finish while support evidence is open"),
            Self::SequenceOverflow => formatter.write_str("relational journal sequence overflow"),
            Self::SequenceMismatch { .. } => {
                formatter.write_str("relational journal sequence is not contiguous")
            }
            Self::PreviousHeadMismatch { .. } => {
                formatter.write_str("relational journal previous head does not match chain")
            }
            Self::EntryHeadMismatch { .. } => {
                formatter.write_str("relational journal entry digest does not match content")
            }
        }
    }
}

impl Error for RelationalJournalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TransitionSupport(error) => Some(error),
            Self::CaseImageProof(error) => Some(error),
            Self::SourceImageProof(error) => Some(error),
            Self::CertifiedSourceSummary(error) => Some(error),
            Self::CaseChunkPartition(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::RegionProof(error) => Some(error),
            Self::SelectedRunMaterialization(error) => Some(error),
            Self::ClassificationCounts(error) => Some(error),
            Self::UniformAdmissionProof(error) => Some(error),
            _ => None,
        }
    }
}

fn relational_question_frontier_set_root(
    roots: &BTreeMap<QuestionId, QuestionFrontierRoot>,
) -> RelationalQuestionFrontierSetRoot {
    let mut hasher = ChainHasher::new(QUESTION_FRONTIER_SET_ROOT_HASH_V1);
    hasher.u64(roots.len() as u64);
    for (question_id, root) in roots {
        hasher.digest(question_id.bytes());
        hasher.digest(root.bytes());
    }
    RelationalQuestionFrontierSetRoot(hasher.finish())
}

fn relational_question_content_set_root(
    roots: &BTreeMap<QuestionId, QuestionContentRoot>,
) -> RelationalQuestionContentSetRoot {
    let mut hasher = ChainHasher::new(QUESTION_CONTENT_SET_ROOT_HASH_V1);
    hasher.u64(roots.len() as u64);
    for (question_id, root) in roots {
        hasher.digest(question_id.bytes());
        hasher.digest(root.bytes());
    }
    RelationalQuestionContentSetRoot(hasher.finish())
}

fn relational_core_evidence_root(
    contract: &RelationalJournalContract,
    analysis_plan: Option<RelationalAnalysisPlanRoot>,
    support_plan: Option<RelationalSupportPlanRoot>,
    exhaustion: RelationalExhaustionEvidenceRoot,
    relation: RelationFrontierRoot,
    admission: AdmissionFrontierRoot,
    question: RelationalQuestionFrontierSetRoot,
    transition_support: RelationalTransitionSupportRoot,
    support: SupportEvidenceRoot,
) -> RelationalCoreEvidenceRoot {
    let mut hasher = ChainHasher::new(CORE_EVIDENCE_ROOT_HASH_V6);
    hasher.digest(contract.id().bytes());
    match analysis_plan {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    match support_plan {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    hasher.digest(exhaustion.bytes());
    hasher.digest(relation.bytes());
    hasher.digest(admission.bytes());
    hasher.digest(question.bytes());
    hasher.digest(transition_support.bytes());
    hasher.digest(support.bytes());
    RelationalCoreEvidenceRoot(hasher.finish())
}

fn relational_exploration_evidence_root(
    contract: &RelationalJournalContract,
    core: RelationalCoreEvidenceRoot,
    analysis_scope: Option<RelationalAnalysisJournalScopeRoot>,
    analysis_catalog: Option<RelationalAnalysisCatalogRoot>,
    analysis_terminal: Option<RelationalAnalysisCatalogRoot>,
    analysis_closure_set: Option<RelationalAnalysisClosureSetRoot>,
) -> RelationalExplorationEvidenceRoot {
    let mut hasher = ChainHasher::new(EXPLORATION_EVIDENCE_ROOT_HASH_V2);
    hasher.digest(contract.id().bytes());
    hasher.digest(core.bytes());
    match analysis_scope {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    match analysis_catalog {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    match analysis_terminal {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    match analysis_closure_set {
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
        None => hasher.tag(0x02),
    }
    RelationalExplorationEvidenceRoot(hasher.finish())
}

fn relational_extensional_content_root(
    contract: &RelationalJournalContract,
    analysis_plan: RelationalAnalysisPlanRoot,
    support_plan: RelationalSupportPlanRoot,
    source_relation_exhaustion: SourceRelationExhaustionReceiptId,
    exhaustion: RelationalExhaustionEvidenceRoot,
    relation: RelationContentRoot,
    admission: AdmissionContentRoot,
    question: RelationalQuestionContentSetRoot,
    transition_support: RelationalTransitionSupportRoot,
    support: SupportEvidenceRoot,
) -> RelationalExtensionalContentRoot {
    let mut hasher = ChainHasher::new(EXTENSIONAL_CONTENT_ROOT_HASH_V5);
    hasher.digest(contract.id().bytes());
    hasher.digest(analysis_plan.bytes());
    hasher.digest(support_plan.bytes());
    hasher.digest(source_relation_exhaustion.bytes());
    hasher.digest(exhaustion.bytes());
    hasher.digest(relation.bytes());
    hasher.digest(admission.bytes());
    hasher.digest(question.bytes());
    hasher.digest(transition_support.bytes());
    hasher.digest(support.bytes());
    RelationalExtensionalContentRoot(hasher.finish())
}

fn relational_exhaustion_evidence_root(
    source_traversal: Option<&SourceTraversalAccumulator>,
    source_relation_exhaustion: Option<&SourceRelationExhaustionReceipt>,
    successor_receipts: &BTreeMap<
        SuccessorFiberExhaustionReceiptId,
        SuccessorFiberExhaustionReceipt,
    >,
) -> RelationalExhaustionEvidenceRoot {
    let mut hasher = ChainHasher::new(EXHAUSTION_EVIDENCE_ROOT_HASH_V2);
    match source_traversal {
        Some(traversal) => {
            hasher.tag(0x01);
            hasher.digest(traversal.frontier_root().bytes());
        }
        None => hasher.tag(0x02),
    }
    match source_relation_exhaustion {
        Some(receipt) => {
            hasher.tag(0x01);
            hasher.digest(receipt.id().bytes());
        }
        None => hasher.tag(0x02),
    }
    hasher.u64(successor_receipts.len() as u64);
    for receipt_id in successor_receipts.keys() {
        hasher.digest(receipt_id.bytes());
    }
    RelationalExhaustionEvidenceRoot(hasher.finish())
}

fn relational_checkpoint_root(
    contract: &RelationalJournalContract,
    question_frontier: RelationalQuestionFrontierSetRoot,
    work: WorkFrontierRoot,
    support: &SupportEvidenceSnapshot,
    classified_chunk_accumulator: Option<&RelationalClassifiedChunkAccumulator>,
    latest_support_frontiers: &BTreeMap<
        MechanismRequestId,
        RelationalMechanismSupportCheckpointReceipt,
    >,
    latest_support_schedulers: &BTreeMap<
        MechanismRequestId,
        MechanismAutomaticObservationSchedulerSummary,
    >,
    latest_explicit_support_schedulers: &BTreeMap<
        MechanismRequestId,
        MechanismExplicitObservationSchedulerSummary,
    >,
    mechanism_support_observation_demands: &BTreeMap<
        MechanismRequestId,
        MechanismSupportObservationDemandLog,
    >,
    mechanism_support_observations: &BTreeMap<MechanismRequestId, MechanismSupportObservationLog>,
) -> RelationalCheckpointRoot {
    let mut hasher = ChainHasher::new(CHECKPOINT_ROOT_HASH_V9);
    hasher.digest(contract.id().bytes());
    hasher.digest(question_frontier.bytes());
    hasher.digest(work.bytes());
    hasher.u64(support.latest_cursors().len() as u64);
    for cursor in support.latest_cursors() {
        hasher.digest(cursor.cell_id().bytes());
        hasher.digest(cursor.id().bytes());
    }
    match classified_chunk_accumulator {
        Some(accumulator) => {
            hasher.tag(0x01);
            hasher.u64(accumulator.question_ids().len() as u64);
            for question_id in accumulator.question_ids() {
                hasher.digest(question_id.bytes());
            }
            hasher.digest(accumulator.chunk_partition_id().bytes());
            hasher.digest(accumulator.chunk_id().bytes());
            hasher.u128(accumulator.chunk_ordinal());
            hasher.digest(accumulator.chunk_cell_id().bytes());
            hasher.u128(accumulator.interval_start());
            hasher.u128(accumulator.interval_end_exclusive());
            hasher.u128(accumulator.next_coordinate());
            hasher.digest(accumulator.transcript_root().bytes());
            match accumulator.last_slice_id() {
                Some(slice_id) => {
                    hasher.tag(0x01);
                    hasher.digest(slice_id.bytes());
                }
                None => hasher.tag(0x02),
            }
        }
        None => hasher.tag(0x02),
    }
    hasher.u64(latest_support_frontiers.len() as u64);
    for (request_id, receipt) in latest_support_frontiers {
        hasher.digest(request_id.bytes());
        hasher.u128(receipt.cursor.target_discovery());
        hasher.u128(receipt.cursor.terminal_discovery());
        hasher.u128(receipt.cursor.structural_assignment());
        hasher.digest(receipt.frontier.root().bytes());
    }
    hasher.u64(latest_support_schedulers.len() as u64);
    for (request_id, scheduler) in latest_support_schedulers {
        let registry = scheduler.registry();
        let dirty = scheduler.dirty();
        hasher.digest(request_id.bytes());
        hasher.digest(registry.root().bytes());
        hasher.u128(registry.slice_count());
        hasher.u128(registry.indexed_assignment_count());
        hasher.digest(dirty.root().bytes());
        hasher.u128(dirty.slice_count());
    }
    hasher.u64(latest_explicit_support_schedulers.len() as u64);
    for (request_id, scheduler) in latest_explicit_support_schedulers {
        hasher.digest(request_id.bytes());
        hash_explicit_support_observation_scheduler(&mut hasher, *scheduler);
    }
    hasher.u64(mechanism_support_observation_demands.len() as u64);
    for (request_id, log) in mechanism_support_observation_demands {
        hasher.digest(request_id.bytes());
        hasher.u128(log.registrations.len() as u128);
        hasher.digest(log.chain_root.bytes());
    }
    hasher.u64(mechanism_support_observations.len() as u64);
    for (request_id, log) in mechanism_support_observations {
        hasher.digest(request_id.bytes());
        hasher.u128(log.points.len() as u128);
        hasher.digest(log.chain_root.bytes());
        hasher.u128(log.automatic_point_count);
        hasher.digest(log.automatic_chain_root.bytes());
        hasher.u128(log.automatic_observed_slice_count);
        hasher.u128(log.automatic_sealed_slice_count);
        match log.automatic_sealed_cursor {
            Some(slice) => {
                hasher.tag(0x01);
                hasher.digest(slice.id().bytes());
            }
            None => hasher.tag(0x02),
        }
        hasher.u128(log.explicit_observed_slice_count);
        hasher.u128(log.explicit_sealed_slice_count);
    }
    RelationalCheckpointRoot(hasher.finish())
}

fn hash_explicit_support_observation_scheduler(
    hasher: &mut ChainHasher,
    scheduler: MechanismExplicitObservationSchedulerSummary,
) {
    let registry = scheduler.registry();
    let pending = scheduler.pending_backfill();
    let dirty = scheduler.dirty();
    let unsealed = scheduler.unsealed();
    hasher.digest(registry.root().bytes());
    hasher.u128(registry.slice_count());
    hasher.u128(registry.ready_slice_count());
    hasher.digest(pending.root().bytes());
    hasher.u128(pending.slice_count());
    hasher.digest(dirty.root().bytes());
    hasher.u128(dirty.slice_count());
    hasher.digest(unsealed.root().bytes());
    hasher.u128(unsealed.slice_count());
}

fn hash_explicit_support_observation_phase(
    hasher: &mut ChainHasher,
    phase: MechanismExplicitObservationRegistrationPhase,
) {
    match phase {
        MechanismExplicitObservationRegistrationPhase::Open => hasher.tag(0x01),
        MechanismExplicitObservationRegistrationPhase::Sealed { support_root } => {
            hasher.tag(0x02);
            hasher.digest(support_root.bytes());
        }
    }
}

fn hash_support_observation_demand_registration_claim(
    hasher: &mut ChainHasher,
    claim: MechanismSupportObservationDemandRegistrationClaim,
) {
    hasher.u32(claim.version());
    hasher.digest(claim.slice().id().bytes());
    hasher.u128(claim.cursor().target_discovery());
    hasher.u128(claim.cursor().terminal_discovery());
    hasher.u128(claim.cursor().structural_assignment());
    hasher.digest(claim.frontier_root().bytes());
    hasher.tag(match claim.disposition() {
        MechanismExplicitObservationRegistrationDisposition::Registered => 0x01,
        MechanismExplicitObservationRegistrationDisposition::AlreadyRegistered => 0x02,
        MechanismExplicitObservationRegistrationDisposition::AutomaticWholeMechanism => 0x03,
    });
    hash_explicit_support_observation_phase(hasher, claim.phase());
    hasher.u128(claim.registration_structural_cursor());
    hash_explicit_support_observation_scheduler(hasher, claim.prior_scheduler());
    hash_explicit_support_observation_scheduler(hasher, claim.next_scheduler());
}

fn journal_entry_head(
    contract_id: RelationalJournalId,
    sequence: u64,
    previous: RelationalJournalHead,
    event: &RelationalJournalEvent,
) -> RelationalJournalHead {
    let mut hasher = ChainHasher::new(JOURNAL_ENTRY_HASH_V28);
    hasher.digest(contract_id.bytes());
    hasher.u64(sequence);
    hasher.digest(previous.bytes());
    hasher.digest(journal_event_digest(event));
    RelationalJournalHead(hasher.finish())
}

fn journal_event_digest(event: &RelationalJournalEvent) -> [u8; 32] {
    let mut hasher = ChainHasher::new(JOURNAL_EVENT_HASH_V24);
    match event {
        RelationalJournalEvent::Evidence(event) => {
            hasher.tag(0x01);
            hash_evidence_event(&mut hasher, event);
        }
        RelationalJournalEvent::Checkpoint(event) => {
            hasher.tag(0x02);
            hash_checkpoint_event(&mut hasher, event);
        }
    }
    hasher.finish()
}

/// Bind one operational scheduler choice to the exact ordered work it
/// selected. The fingerprint is journal provenance only; semantic roots do
/// not consume it, and the work events retain their ordinary validation.
pub(crate) fn relational_scheduler_work_fingerprint(
    decision: RelationalSchedulerDecision,
    events: &[RelationalJournalEvent],
) -> [u8; 32] {
    let mut hasher = ChainHasher::new(SCHEDULER_WORK_FINGERPRINT_V2);
    hasher.u32(RELATIONAL_SCHEDULER_POLICY_VERSION);
    hasher.tag(decision.canonical_tag());
    hasher.u64(events.len() as u64);
    for event in events {
        hasher.digest(journal_event_digest(event));
    }
    hasher.finish()
}

fn hash_evidence_event(hasher: &mut ChainHasher, event: &RelationalEvidenceEvent) {
    match event {
        RelationalEvidenceEvent::AnalysisPlanRegistered { plan_root, plan } => {
            hasher.tag(0x0b);
            hasher.digest(plan_root.bytes());
            hasher.digest(plan.root().bytes());
        }
        RelationalEvidenceEvent::SupportPlanRegistered { plan_root, plan } => {
            hasher.tag(0x01);
            hasher.digest(plan_root.bytes());
            hasher.digest(plan.root().bytes());
        }
        RelationalEvidenceEvent::RelationalCaseImageInjectivityProofAccepted { artifact } => {
            hasher.tag(0x0d);
            hash_relational_case_image_injectivity_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalSourceImageExactnessProofAccepted { artifact } => {
            hasher.tag(0x11);
            hash_relational_source_image_exactness_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalCaseChunkPartitionAccepted { artifact } => {
            hasher.tag(0x0f);
            hash_relational_case_chunk_partition_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalClassifiedChunkAccepted { artifact } => {
            hasher.tag(0x10);
            hash_relational_classified_chunk_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalRegionProofAccepted { artifact } => {
            hasher.tag(0x13);
            hash_relational_region_proof_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalSelectedRunMaterializationAccepted { artifact } => {
            hasher.tag(0x12);
            hash_relational_selected_run_materialization_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::RelationalUniformAdmissionProofAccepted { artifact } => {
            hasher.tag(0x0e);
            hash_relational_uniform_admission_artifact(hasher, artifact);
        }
        RelationalEvidenceEvent::SourceTraversalObserved {
            advance_id,
            advance: _,
        } => {
            hasher.tag(0x02);
            hasher.digest(advance_id.bytes());
        }
        RelationalEvidenceEvent::SourceEnumerationSealed {
            receipt_id,
            receipt: _,
        } => {
            hasher.tag(0x03);
            hasher.digest(receipt_id.bytes());
        }
        RelationalEvidenceEvent::SuccessorDiscovered {
            source_key,
            successor_key,
            case_id,
            row,
        } => {
            hasher.tag(0x04);
            hasher.digest(source_key.bytes());
            hasher.digest(successor_key.bytes());
            hasher.digest(case_id.bytes());
            hash_provenance(hasher, row.provenance());
        }
        RelationalEvidenceEvent::SuccessorFiberExhaustionAccepted {
            receipt_id,
            receipt,
        } => {
            hasher.tag(0x05);
            hasher.digest(receipt_id.bytes());
            hasher.digest(receipt.id().bytes());
        }
        RelationalEvidenceEvent::SuccessorEnumerationSealed {
            source_key,
            receipt_id,
        } => {
            hasher.tag(0x06);
            hasher.digest(source_key.bytes());
            hasher.digest(receipt_id.bytes());
        }
        RelationalEvidenceEvent::AdmissionClassified { case_id, decision } => {
            hasher.tag(0x07);
            hasher.digest(case_id.bytes());
            hasher.tag(match decision {
                AdmissionDecision::Rejected => 0x01,
                AdmissionDecision::Admitted => 0x02,
            });
        }
        RelationalEvidenceEvent::QuestionClassified {
            question_id,
            case_id,
            decision,
        } => {
            hasher.tag(0x08);
            hasher.digest(question_id.bytes());
            hasher.digest(case_id.bytes());
            hasher.tag(match decision {
                SelectionDecision::NotSelected => 0x01,
                SelectionDecision::Selected => 0x02,
            });
        }
        RelationalEvidenceEvent::Support(event) => {
            hasher.tag(0x09);
            hasher.digest(event.digest().bytes());
        }
        RelationalEvidenceEvent::Analysis(event) => {
            hasher.tag(0x0c);
            hasher.digest(event.digest().bytes());
        }
    }
}

fn hash_relational_case_chunk_partition_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalCaseChunkPartitionArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.id().bytes());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.digest(artifact.admission_id().bytes());
    hasher.u64(artifact.question_ids().len() as u64);
    for question_id in artifact.question_ids() {
        hasher.digest(question_id.bytes());
    }
    hasher.digest(artifact.case_image_certificate_id());
    hasher.digest(artifact.injectivity_evidence_id().bytes());
    hasher.digest(artifact.root_cell_id().bytes());
    hasher.digest(artifact.root_materializer_id().bytes());
    hasher.tag(match artifact.shape() {
        RelationalCaseChunkShape::BareOrdinalInterval => 0x01,
        RelationalCaseChunkShape::ProductFactor => 0x02,
        RelationalCaseChunkShape::ProductRankInterval => 0x03,
    });
    match artifact.factor_index() {
        Some(factor_index) => {
            hasher.tag(0x01);
            hasher.u32(factor_index);
        }
        None => hasher.tag(0x02),
    }
    hasher.u128(artifact.interval_start());
    hasher.u128(artifact.interval_end_exclusive());
    hasher.u128(artifact.max_chunk_coordinates());
    hasher.u64(artifact.chunks().len() as u64);
    for chunk in artifact.chunks() {
        hasher.digest(chunk.id().bytes());
        hasher.u128(chunk.ordinal());
        hasher.digest(chunk.cell_id().bytes());
        hasher.u128(chunk.interval_start());
        hasher.u128(chunk.interval_end_exclusive());
    }
    hasher.digest(artifact.partition_id().bytes());
}

fn hash_relational_classified_chunk_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalClassifiedChunkArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.id().bytes());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.digest(artifact.admission_id().bytes());
    hasher.u64(artifact.question_ids().len() as u64);
    for question_id in artifact.question_ids() {
        hasher.digest(question_id.bytes());
    }
    hasher.digest(artifact.chunk_partition_id().bytes());
    hasher.digest(artifact.chunk_id().bytes());
    hasher.u128(artifact.chunk_ordinal());
    hasher.digest(artifact.chunk_cell_id().bytes());
    hasher.digest(artifact.chunk_materializer_id().bytes());
    hasher.u128(artifact.interval_start());
    hasher.u128(artifact.interval_end_exclusive());
    hasher.u128(artifact.evaluated_case_count());
    hasher.digest(artifact.evaluated_cases_root());
    hasher.u128(artifact.rejected_count());
    hasher.u128(artifact.admitted_count());
    hasher.u64(artifact.admitted_selected_counts().len() as u64);
    for count in artifact.admitted_selected_counts() {
        hasher.u128(*count);
    }
    hasher.u64(artifact.runs().len() as u64);
    for run in artifact.runs() {
        hasher.digest(run.id().bytes());
        hasher.u32(u32::from(run.ordinal()));
        hasher.digest(run.cell_id().bytes());
        hasher.u128(run.interval_start());
        hasher.u128(run.interval_end_exclusive());
        hasher.tag(run.outcome().canonical_tag());
        if let Some(mask) = run.outcome().decision_mask() {
            hasher.bytes(mask.bytes());
        }
    }
    match artifact.partition_id() {
        Some(partition_id) => {
            hasher.tag(0x01);
            hasher.digest(partition_id.bytes());
        }
        None => hasher.tag(0x02),
    }
}

fn hash_relational_region_proof_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalRegionProofArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.certificate_id());
    hasher.digest(artifact.replay_authority_id());
    hasher.digest(artifact.classification_capsule_id().bytes());
    hasher.digest(artifact.successor_root_id().bytes());
    hasher.digest(artifact.find_root_id().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.digest(artifact.admission_id().bytes());
    hasher.digest(artifact.question_id().bytes());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.root_cell_id().bytes());
    match artifact.subject() {
        RelationalRegionProofSubject::Root => hasher.tag(0x01),
        RelationalRegionProofSubject::CanonicalChunk {
            partition_artifact_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
        } => {
            hasher.tag(0x02);
            hasher.digest(partition_artifact_id.bytes());
            hasher.digest(chunk_id.bytes());
            hasher.u128(chunk_ordinal);
            hasher.digest(chunk_cell_id.bytes());
            hasher.digest(chunk_materializer_id.bytes());
        }
    }
    hasher.tag(artifact.conclusion().canonical_tag());
    hasher.digest(artifact.starter_region_id().bytes());
    hasher.digest(artifact.source_assignment_cell_id().bytes());
    hasher.digest(artifact.source_row_cell_id().bytes());
    hasher.digest(artifact.successor_coordinate_cell_id().bytes());
    hasher.digest(artifact.axis_stage_id().bytes());
    hasher.digest(artifact.axis_dimension_id().bytes());
    hasher.digest(artifact.axis_cell_id().bytes());
    hasher.i64(artifact.value_start());
    hasher.i64(artifact.value_end_exclusive());
    hasher.u128(artifact.coordinate_start());
    hasher.u128(artifact.coordinate_end_exclusive());
    hasher.u128(artifact.case_cardinality());
    hasher.digest(artifact.selected_formula_digest());
}

fn hash_relational_selected_run_materialization_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalSelectedRunMaterializationArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.id().bytes());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.digest(artifact.admission_id().bytes());
    hasher.u64(artifact.selected_question_ids().len() as u64);
    for question_id in artifact.selected_question_ids() {
        hasher.digest(question_id.bytes());
    }
    hasher.digest(artifact.classified_chunk_artifact_id().bytes());
    hasher.digest(artifact.chunk_partition_id().bytes());
    hasher.digest(artifact.chunk_id().bytes());
    hasher.u128(artifact.chunk_ordinal());
    hasher.digest(artifact.chunk_cell_id().bytes());
    hasher.digest(artifact.chunk_materializer_id().bytes());
    hasher.digest(artifact.run_id().bytes());
    hasher.u32(u32::from(artifact.run_ordinal()));
    hasher.digest(artifact.run_cell_id().bytes());
    hasher.digest(artifact.run_materializer_id().bytes());
    hasher.u128(artifact.interval_start());
    hasher.u128(artifact.interval_end_exclusive());
    hasher.u128(artifact.materialized_case_count());
    hasher.digest(artifact.materialized_cases_root());
    hasher.u64(artifact.cases().len() as u64);
    for record in artifact.cases() {
        hasher.u128(record.coordinate_ordinal());
        hasher.digest(record.source_key().bytes());
        hasher.digest(canonical_explore_value_digest(record.source().context()));
        hasher.digest(canonical_explore_value_digest(record.source().before()));
        hash_provenance(hasher, record.source().provenance());
        hasher.digest(record.successor_key().bytes());
        hasher.digest(canonical_explore_value_digest(record.successor().after()));
        hash_provenance(hasher, record.successor().provenance());
        hasher.digest(record.case_id().bytes());
    }
}

fn hash_relational_case_image_injectivity_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalCaseImageInjectivityProofArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.certificate_id());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.u64(artifact.binding_stage_ids().len() as u64);
    for stage_id in artifact.binding_stage_ids() {
        hasher.digest(stage_id.bytes());
    }
    hasher.u64(artifact.finite_factor_cell_ids().len() as u64);
    for cell_id in artifact.finite_factor_cell_ids() {
        hasher.digest(cell_id.bytes());
    }
    hasher.tag(match artifact.assignment_kind() {
        RelationalCaseImageAssignmentKind::IndependentProduct => 0x01,
        RelationalCaseImageAssignmentKind::DependentJoin => 0x02,
    });
    hasher.tag(match artifact.source_assignment_image_proof() {
        RelationalSourceAssignmentImageProof::Unproven => 0x01,
        RelationalSourceAssignmentImageProof::DirectEndpointCoordinates => 0x02,
        RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate => 0x03,
    });
    if let Some(reference) = artifact.source_image_proof_reference() {
        hasher.digest(reference.compiler_certificate_id());
        hasher.digest(reference.source_exactness_certificate_id());
        hasher.digest(reference.source_injectivity_evidence_id().bytes());
        hasher.digest(reference.source_population_root().bytes());
    }
    hasher.digest(artifact.source_assignment_cell_id().bytes());
    hasher.digest(artifact.source_row_cell_id().bytes());
    hasher.digest(artifact.successor_coordinate_cell_id().bytes());
    hasher.tag(match artifact.successor_kind() {
        RelationalSuccessorRecipeKind::Singleton => 0x01,
        RelationalSuccessorRecipeKind::FiniteExact => 0x02,
        RelationalSuccessorRecipeKind::FiniteCollection => 0x03,
        RelationalSuccessorRecipeKind::FiniteIntRange => 0x04,
    });
    hasher.tag(match artifact.preimage_kind() {
        RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin => 0x01,
        RelationalCaseImagePreimageKind::ComposedSingletonAssignment => 0x02,
    });
    hasher.digest(artifact.case_cell_id().bytes());
    hasher.digest(artifact.case_materializer_id().bytes());
    match artifact.exact_case_cardinality() {
        Some(count) => {
            hasher.tag(0x01);
            hasher.u128(count);
        }
        None => hasher.tag(0x02),
    }
}

fn hash_relational_source_image_exactness_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalSourceImageExactnessProofArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.certificate_id());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.u64(artifact.binding_stage_ids().len() as u64);
    for stage_id in artifact.binding_stage_ids() {
        hasher.digest(stage_id.bytes());
    }
    match artifact.shape() {
        RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
            context_stage_id,
            before_stage_id,
            before_dimension_id,
            before_factor_cell_id,
        } => {
            hasher.digest(context_stage_id.bytes());
            hasher.digest(before_stage_id.bytes());
            hasher.digest(before_dimension_id.bytes());
            hasher.digest(before_factor_cell_id.bytes());
        }
        RelationalSourceImageExactnessProofShape::SeparatedProjection {
            compiler_certificate_id,
            factors,
            witness_ids,
        } => {
            hasher.digest(*compiler_certificate_id);
            hasher.u64(factors.len() as u64);
            for factor in factors.iter().copied() {
                hasher.digest(factor.stage_id().bytes());
                hasher.digest(factor.dimension_id().bytes());
                hasher.digest(factor.factor_cell_id().bytes());
                hasher.u128(factor.exact_cardinality());
            }
            hasher.u64(witness_ids.len() as u64);
            for witness_id in witness_ids.iter() {
                hasher.digest(*witness_id);
            }
        }
    }
    hasher.digest(artifact.source_assignment_cell_id().bytes());
    hasher.digest(artifact.source_assignment_producer_id().bytes());
    hasher.digest(artifact.source_assignment_materializer_id().bytes());
    hasher.digest(artifact.source_row_cell_id().bytes());
    hasher.digest(artifact.source_materializer_id().bytes());
    hasher.u128(artifact.exact_source_cardinality());
}

fn hash_relational_uniform_admission_artifact(
    hasher: &mut ChainHasher,
    artifact: &RelationalUniformAdmissionProofArtifact,
) {
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.certificate_id());
    hasher.digest(artifact.plan_root().bytes());
    hasher.digest(artifact.relation_id().bytes());
    hasher.digest(artifact.admission_id().bytes());
    hasher.digest(artifact.case_cell_id().bytes());
    hasher.u32(artifact.predicate_count());
    hasher.digest(artifact.recipe_digest());
    hasher.tag(match artifact.decision() {
        AdmissionDecision::Rejected => 0x01,
        AdmissionDecision::Admitted => 0x02,
    });
}

#[allow(clippy::too_many_arguments)]
fn derive_mechanism_support_observation_point_id(
    version: u32,
    slice: MechanismSupportSlice,
    cursor: MechanismSupportCheckpointCursor,
    frontier_root: MechanismSupportFrontierRoot,
    summary_root: MechanismFactorizedSupportObservationSummaryRoot,
    status: MechanismSupportObservationStatus,
    supersedes: Option<MechanismSupportObservationPointId>,
) -> MechanismSupportObservationPointId {
    let mut hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_POINT_ID_V2);
    hasher.u32(version);
    hasher.digest(slice.id().bytes());
    hasher.u128(cursor.target_discovery());
    hasher.u128(cursor.terminal_discovery());
    hasher.u128(cursor.structural_assignment());
    hasher.digest(frontier_root.bytes());
    hasher.digest(summary_root.bytes());
    match status {
        MechanismSupportObservationStatus::Open => hasher.tag(0x01),
        MechanismSupportObservationStatus::Sealed { support_root } => {
            hasher.tag(0x02);
            hasher.digest(support_root.bytes());
        }
    }
    match supersedes {
        Some(point_id) => {
            hasher.tag(0x01);
            hasher.digest(point_id.bytes());
        }
        None => hasher.tag(0x02),
    }
    MechanismSupportObservationPointId(hasher.finish())
}

fn mechanism_support_observation_chain_genesis(
    request_id: MechanismRequestId,
) -> MechanismSupportObservationChainRoot {
    let mut hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_CHAIN_GENESIS_V2);
    hasher.u32(MECHANISM_SUPPORT_OBSERVATION_POINT_VERSION);
    hasher.digest(request_id.bytes());
    MechanismSupportObservationChainRoot(hasher.finish())
}

fn extend_mechanism_support_observation_chain(
    request_id: MechanismRequestId,
    prior: MechanismSupportObservationChainRoot,
    point_ordinal: u128,
    point_id: MechanismSupportObservationPointId,
) -> MechanismSupportObservationChainRoot {
    let mut hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_CHAIN_STEP_V2);
    hasher.u32(MECHANISM_SUPPORT_OBSERVATION_POINT_VERSION);
    hasher.digest(request_id.bytes());
    hasher.digest(prior.bytes());
    hasher.u128(point_ordinal);
    hasher.digest(point_id.bytes());
    MechanismSupportObservationChainRoot(hasher.finish())
}

fn mechanism_support_observation_demand_chain_genesis(
    request_id: MechanismRequestId,
) -> MechanismSupportObservationDemandChainRoot {
    let mut hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_DEMAND_CHAIN_GENESIS_V1);
    hasher.u32(MECHANISM_SUPPORT_OBSERVATION_DEMAND_REGISTRATION_VERSION);
    hasher.digest(request_id.bytes());
    MechanismSupportObservationDemandChainRoot(hasher.finish())
}

fn extend_mechanism_support_observation_demand_chain(
    request_id: MechanismRequestId,
    prior: MechanismSupportObservationDemandChainRoot,
    ordinal: u128,
    claim: MechanismSupportObservationDemandRegistrationClaim,
) -> MechanismSupportObservationDemandChainRoot {
    let mut claim_hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_DEMAND_CLAIM_HASH_V1);
    hash_support_observation_demand_registration_claim(&mut claim_hasher, claim);
    let claim_digest = claim_hasher.finish();
    let mut hasher = ChainHasher::new(MECHANISM_SUPPORT_OBSERVATION_DEMAND_CHAIN_STEP_V1);
    hasher.u32(MECHANISM_SUPPORT_OBSERVATION_DEMAND_REGISTRATION_VERSION);
    hasher.digest(request_id.bytes());
    hasher.digest(prior.bytes());
    hasher.u128(ordinal);
    hasher.digest(claim_digest);
    MechanismSupportObservationDemandChainRoot(hasher.finish())
}

fn hash_checkpoint_event(hasher: &mut ChainHasher, event: &RelationalCheckpointEvent) {
    match event {
        RelationalCheckpointEvent::SchedulerDecisionRecorded {
            policy_version,
            decision,
            nomination_root,
            work_fingerprint,
        } => {
            hasher.tag(0x12);
            hasher.u32(*policy_version);
            hasher.tag(decision.canonical_tag());
            match nomination_root {
                Some(root) => {
                    hasher.tag(0x01);
                    hasher.digest(root.bytes());
                }
                None => hasher.tag(0x00),
            }
            hasher.digest(*work_fingerprint);
        }
        RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { artifact } => {
            hasher.tag(0x0d);
            hasher.digest(artifact.id().bytes());
        }
        RelationalCheckpointEvent::RelationalClassifiedPrefixAdvanced {
            partition_artifact_id,
            chunk_ordinal,
            artifact_digest,
        } => {
            hasher.tag(0x13);
            hasher.digest(partition_artifact_id.bytes());
            hasher.u128(*chunk_ordinal);
            hasher.digest(*artifact_digest);
        }
        RelationalCheckpointEvent::WorkNodeInserted {
            node_id,
            dependencies,
            ..
        } => {
            hasher.tag(0x07);
            hasher.digest(node_id.bytes());
            hasher.u64(dependencies.len() as u64);
            for dependency in dependencies {
                hasher.digest(dependency.bytes());
            }
        }
        RelationalCheckpointEvent::WorkReadinessMaterialized { node_id, .. } => {
            hasher.tag(0x08);
            hasher.digest(node_id.bytes());
        }
        RelationalCheckpointEvent::WorkCursorAdvanced {
            node_id,
            next_member_ordinal,
        } => {
            hasher.tag(0x09);
            hasher.digest(node_id.bytes());
            hasher.u128(*next_member_ordinal);
        }
        RelationalCheckpointEvent::SupportMaterializationCheckpointed { cursor } => {
            hasher.tag(0x0a);
            hasher.digest(cursor.id().bytes());
        }
        RelationalCheckpointEvent::SupportFrontierCheckpointed {
            request_id,
            cursor,
            frontier_root,
        } => {
            hasher.tag(0x0e);
            hasher.digest(request_id.bytes());
            hasher.u128(cursor.target_discovery());
            hasher.u128(cursor.terminal_discovery());
            hasher.u128(cursor.structural_assignment());
            hasher.digest(frontier_root.bytes());
        }
        RelationalCheckpointEvent::SupportObservationDemandRegistered { claim } => {
            hasher.tag(0x10);
            hash_support_observation_demand_registration_claim(hasher, *claim);
        }
        RelationalCheckpointEvent::SupportObservationBackfillCheckpointed { claim } => {
            hasher.tag(0x11);
            hasher.u32(claim.version());
            hasher.digest(claim.slice().id().bytes());
            hasher.u128(claim.cursor().target_discovery());
            hasher.u128(claim.cursor().terminal_discovery());
            hasher.u128(claim.cursor().structural_assignment());
            hasher.digest(claim.frontier_root().bytes());
            hash_explicit_support_observation_phase(hasher, claim.phase());
            hasher.u128(claim.registration_structural_cursor());
            hasher.u128(claim.from_structural_cursor());
            hasher.u128(claim.through_structural_cursor());
            hasher.tag(if claim.completed() { 0x01 } else { 0x02 });
            hash_explicit_support_observation_scheduler(hasher, claim.prior_scheduler());
            hash_explicit_support_observation_scheduler(hasher, claim.next_scheduler());
        }
        RelationalCheckpointEvent::SupportSubjectObserved { claim } => {
            hasher.tag(0x0f);
            hasher.digest(claim.point_id().bytes());
        }
        RelationalCheckpointEvent::WorkNodeCompleted {
            node_id,
            completion,
        } => {
            hasher.tag(0x0b);
            hasher.digest(node_id.bytes());
            hasher.digest(completion.evidence_id().bytes());
        }
        RelationalCheckpointEvent::WorkFrontierCompacted { receipt } => {
            hasher.tag(0x0c);
            hasher.digest(receipt.id());
        }
    }
}

fn hash_provenance(hasher: &mut ChainHasher, provenance: &RelationProvenance) {
    hasher.u64(provenance.lineage().len() as u64);
    for lineage in provenance.lineage() {
        hasher.digest(lineage.bytes());
    }
    hasher.u64(provenance.support().len() as u64);
    for support in provenance.support() {
        hasher.digest(support.bytes());
    }
}

struct ChainHasher(Sha256);

impl ChainHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::mechanism_support::{
        closed_subject_starter_fixture, MechanismSupportFacet, MechanismSupportKey,
    };
    use crate::explore::{
        ExploreValue, RelationalAnalysisPlan, RelationalBoundValue, RelationalCaseExecutor,
        RelationalExpressionRuntime, RelationalSourceEnumerator, RelationalSuccessorAdvance,
        RelationalSupportPlanner, RelationalTransitionLayer,
    };
    use crate::{Expr, ExprKind, Lexer, Literal, Parser, Ty, TypeChecker};

    struct LiteralRuntime;

    impl RelationalExpressionRuntime for LiteralRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            _earlier_bindings: &[RelationalBoundValue<'_>],
        ) -> Result<ExploreValue, String> {
            match &expression.kind {
                ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
                other => Err(format!(
                    "journal fixture expected an integer literal, got {other:?}"
                )),
            }
        }
    }

    fn contract_with_questions(
        question_ids: impl IntoIterator<Item = QuestionId>,
    ) -> RelationalJournalContract {
        let relation = RelationId::from_canonical_semantic_preimage(b"plural journal relation");
        let admission =
            AdmissionId::from_canonical_admission_preimage(relation, b"plural journal admission");
        RelationalJournalContract::new(
            relation,
            admission,
            question_ids,
            StateSchemaId::from_bytes([0x31; 32]),
            ContextSchemaId::from_bytes([0x32; 32]),
            TransitionTypeId::from_bytes([0x33; 32]),
            [0x34; 32],
        )
    }

    #[test]
    fn journal_contract_canonicalizes_the_complete_question_set_and_rejects_foreign_events() {
        let admission = contract_with_questions([]).admission_id();
        let first = QuestionId::from_canonical_find_preimage(
            admission,
            b"first",
            super::super::relation::FindPolarity::All,
        );
        let second = QuestionId::from_canonical_find_preimage(
            admission,
            b"second",
            super::super::relation::FindPolarity::Matches,
        );
        let contract = contract_with_questions([second, first, second]);
        let canonical = contract_with_questions([first, second]);
        let mut expected = vec![first, second];
        expected.sort_unstable();
        assert_eq!(contract, canonical);
        assert_eq!(contract.question_ids(), expected);
        let empty_contract = contract_with_questions([]);
        assert!(empty_contract.question_ids().is_empty());
        let empty_snapshot = RelationalJournal::new(empty_contract).snapshot().unwrap();
        assert_eq!(empty_snapshot.questions().count(), 0);
        assert_eq!(empty_snapshot.question_frontier_roots().count(), 0);

        let mut journal = RelationalJournal::new(contract);
        let plural_snapshot = journal.snapshot().unwrap();
        assert_eq!(plural_snapshot.questions().count(), 2);
        assert!(plural_snapshot.question(first).is_some());
        assert!(plural_snapshot.question(second).is_some());
        let foreign = QuestionId::from_canonical_find_preimage(
            admission,
            b"foreign",
            super::super::relation::FindPolarity::Violations,
        );
        assert!(matches!(
            journal.append(RelationalJournalEvent::question_classified(
                foreign,
                RelationalCaseId::from_journal_codec_bytes([0x35; 32]),
                SelectionDecision::Selected,
            )),
            Err(RelationalJournalError::UnknownQuestion { question_id })
                if question_id == foreign
        ));
    }

    #[test]
    fn explicit_observation_claims_remint_and_replay_the_same_demand_chain() {
        let fixture = closed_subject_starter_fixture();
        let mut support = fixture.support.clone();
        let scope = support.scope();
        let request_id = scope.request_id();
        let cursor = support.checkpoint_cursor();
        let frontier_root = MechanismSupportFrontierRoot::from_journal_codec_bytes([0x71; 32]);
        let automatic_scheduler = support.automatic_observation_scheduler_summary();
        let node_slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            scope,
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
        ));
        let edge_slice = MechanismSupportSlice::total(MechanismSupportKey::new(
            scope,
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::Activation,
                edge_id: fixture.edge_ids[0],
            },
        ));

        let node_registration = support
            .prepare_explicit_observation_demand_registration(node_slice, &fixture.structural)
            .expect("prepare explicit node demand");
        let node_claim = MechanismSupportObservationDemandRegistrationClaim::new(
            node_registration.slice(),
            cursor,
            frontier_root,
            node_registration.disposition(),
            node_registration.registration_phase(),
            node_registration.registration_structural_cursor(),
            node_registration.prior_scheduler_summary(),
            node_registration.next_scheduler_summary(),
        );
        support.commit_explicit_observation_demand_registration(node_registration);

        // Preparing is non-mutating: a discarded bounded page must remint the
        // exact claim when resumed from the same durable scheduler anchor.
        let discarded = support
            .prepare_next_explicit_observation_backfill(&fixture.structural, NonZeroU16::MIN)
            .expect("prepare bounded node backfill")
            .expect("registered node requires backfill");
        let discarded_claim = MechanismSupportObservationBackfillClaim::new(
            discarded.slice(),
            cursor,
            frontier_root,
            discarded.registration_phase(),
            discarded.registration_structural_cursor(),
            discarded.from_structural_cursor(),
            discarded.through_structural_cursor(),
            discarded.completed(),
            discarded.prior_scheduler_summary(),
            discarded.next_scheduler_summary(),
        );
        let resumed = support
            .prepare_next_explicit_observation_backfill(&fixture.structural, NonZeroU16::MIN)
            .expect("resume bounded node backfill")
            .expect("discarded page remains pending");
        let resumed_claim = MechanismSupportObservationBackfillClaim::new(
            resumed.slice(),
            cursor,
            frontier_root,
            resumed.registration_phase(),
            resumed.registration_structural_cursor(),
            resumed.from_structural_cursor(),
            resumed.through_structural_cursor(),
            resumed.completed(),
            resumed.prior_scheduler_summary(),
            resumed.next_scheduler_summary(),
        );
        assert_eq!(resumed_claim, discarded_claim);
        assert_eq!(resumed.through_structural_cursor(), 1);
        support.commit_explicit_observation_backfill(resumed);

        let edge_registration = support
            .prepare_explicit_observation_demand_registration(edge_slice, &fixture.structural)
            .expect("prepare explicit edge demand");
        let edge_claim = MechanismSupportObservationDemandRegistrationClaim::new(
            edge_registration.slice(),
            cursor,
            frontier_root,
            edge_registration.disposition(),
            edge_registration.registration_phase(),
            edge_registration.registration_structural_cursor(),
            edge_registration.prior_scheduler_summary(),
            edge_registration.next_scheduler_summary(),
        );
        support.commit_explicit_observation_demand_registration(edge_registration);
        assert_eq!(
            support.automatic_observation_scheduler_summary(),
            automatic_scheduler
        );

        let claims = [node_claim, edge_claim];
        let replay = |claims: &[MechanismSupportObservationDemandRegistrationClaim]| {
            claims.iter().copied().enumerate().fold(
                mechanism_support_observation_demand_chain_genesis(request_id),
                |root, (ordinal, claim)| {
                    extend_mechanism_support_observation_demand_chain(
                        request_id,
                        root,
                        ordinal as u128,
                        claim,
                    )
                },
            )
        };
        let mut log = MechanismSupportObservationDemandLog::new(request_id);
        for claim in claims {
            let ordinal = log.registrations.len();
            assert!(log.by_slice.insert(claim.slice(), ordinal).is_none());
            log.chain_root = extend_mechanism_support_observation_demand_chain(
                request_id,
                log.chain_root,
                ordinal as u128,
                claim,
            );
            log.registrations.push(claim);
        }

        assert_eq!(log.chain_root, replay(&claims));
        assert_eq!(log.registration(node_slice), Some(&node_claim));
        assert_eq!(log.registration(edge_slice), Some(&edge_claim));
        assert_ne!(log.chain_root, replay(&[edge_claim, node_claim]));
    }

    #[test]
    fn replay_rebuilds_the_same_interleaved_frontier_and_authenticated_exhaustion() {
        let source = r#"
? explore journal_fixture {
    from {
        given before = 199999
        given context = 1
    }
    transition after = 200000
    find all_cases = all
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse journal Explore fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("joined checked Explore query");
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan analysis DAG");
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact source support");
        let question_id = checked.question_ids()[0];
        let contract = RelationalJournalContract::new(
            checked.relation_id(),
            checked.admission_id(),
            checked.question_ids().iter().copied(),
            checked.transition_schemas().state_schema_id(),
            checked.transition_schemas().context_schema_id(),
            checked.transition_schemas().transition_type_id(),
            analysis_plan.producer_graph_digest().bytes(),
        );
        let mut journal = RelationalJournal::new(contract.clone());
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::support_plan_registered(
                support_plan,
            ))
            .unwrap();

        let sources =
            RelationalSourceEnumerator::new(contract.relation_id(), &checked.closed_query.source)
                .expect("construct checked source enumerator");
        let mut runtime = LiteralRuntime;
        let root = sources.root_cursor().unwrap();
        let first = sources.advance(&root, &mut runtime).unwrap();
        let (root_resume, child) = match &first {
            RelationalSourceAdvance::Yielded {
                resume,
                continuation: RelationalSourceContinuation::Expand(child),
                ..
            } => (resume.clone(), child.clone()),
            other => panic!("first singleton must open context work: {other:?}"),
        };
        let event = journal.source_traversal_event(first).unwrap();
        journal.append(event).unwrap();

        let second = sources.advance(&child, &mut runtime).unwrap();
        let (child_resume, source_key) = match &second {
            RelationalSourceAdvance::Yielded {
                resume,
                continuation: RelationalSourceContinuation::Source(source),
                ..
            } => (resume.clone(), source.source_key()),
            other => panic!("second singleton must yield the source row: {other:?}"),
        };
        let event = journal.source_traversal_event(second).unwrap();
        journal.append(event).unwrap();

        let source_ready =
            RelationalJournalEvent::work_readiness_materialized(WorkNodeSpec::SourceRowReady {
                relation_id: contract.relation_id(),
                source_key,
            })
            .unwrap();
        let source_ready_id = source_ready.work_node_id().unwrap();
        journal.append(source_ready).unwrap();
        let successor_work = RelationalJournalEvent::work_node_inserted(
            WorkNodeSpec::ExpandSuccessors {
                relation_id: contract.relation_id(),
                source_key,
            },
            [source_ready_id],
        )
        .unwrap();
        let successor_work_id = successor_work.work_node_id().unwrap();
        journal.append(successor_work).unwrap();

        let source_row = journal
            .scheduler_view()
            .unwrap()
            .source_row(source_key)
            .expect("source traversal inserted its terminal row")
            .clone();
        let cases = RelationalCaseExecutor::new(contract.relation_id(), checked.closed_query)
            .expect("construct checked successor executor");
        let successor_cursor = cases.root_cursor(source_key, &source_row).unwrap();
        let successor = cases
            .advance(&successor_cursor, &source_row, &mut runtime)
            .unwrap();
        let (case, successor_resume) = match successor {
            RelationalSuccessorAdvance::Yielded { case, resume, .. } => (case, resume),
            other => panic!("singleton successor must yield one case: {other:?}"),
        };
        let case_id = case.case_id();
        let successor_key = case.successor_key();
        journal.append(case.discovered_event()).unwrap();

        let case_ready =
            RelationalJournalEvent::work_readiness_materialized(WorkNodeSpec::CaseReady {
                case_id,
            })
            .unwrap();
        let case_ready_id = case_ready.work_node_id().unwrap();
        journal.append(case_ready).unwrap();
        let admission_work = RelationalJournalEvent::work_node_inserted(
            WorkNodeSpec::EvaluateAdmission {
                admission_id: contract.admission_id(),
                case_id,
            },
            [case_ready_id],
        )
        .unwrap();
        let admission_work_id = admission_work.work_node_id().unwrap();
        journal.append(admission_work).unwrap();

        journal
            .append(RelationalJournalEvent::admission_classified(
                case_id,
                AdmissionDecision::Admitted,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::work_node_completed(
                admission_work_id,
                WorkCompletionRef::AdmissionDecided {
                    admission_id: contract.admission_id(),
                    case_id,
                    decision: AdmissionDecision::Admitted,
                },
            ))
            .unwrap();
        let find_work = RelationalJournalEvent::work_node_inserted(
            WorkNodeSpec::EvaluateFind {
                question_id,
                case_id,
            },
            [case_ready_id, admission_work_id],
        )
        .unwrap();
        let find_work_id = find_work.work_node_id().unwrap();
        journal.append(find_work).unwrap();
        journal
            .append(RelationalJournalEvent::question_classified(
                question_id,
                case_id,
                SelectionDecision::Selected,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::work_node_completed(
                find_work_id,
                WorkCompletionRef::FindDecided {
                    question_id,
                    case_id,
                    decision: SelectionDecision::Selected,
                },
            ))
            .unwrap();

        let open = journal.snapshot().unwrap();
        assert_eq!(
            open.relation().counts().cases(),
            RelationCountEvidence::LowerBound(1)
        );
        assert_eq!(
            open.question(question_id).unwrap().selected(),
            RelationCountEvidence::LowerBound(1)
        );
        assert_eq!(
            open.work()
                .nodes
                .iter()
                .filter(|node| !node.progress.is_complete())
                .count(),
            1
        );

        let exhausted_successors = cases
            .advance(&successor_resume, &source_row, &mut runtime)
            .unwrap();
        let (terminal_ordinal, successor_receipt) = match exhausted_successors {
            RelationalSuccessorAdvance::Exhausted {
                cardinality,
                receipt,
                ..
            } => (cardinality, receipt),
            other => panic!("successor cursor must authenticate exhaustion: {other:?}"),
        };
        journal
            .append(RelationalJournalEvent::work_cursor_advanced(
                successor_work_id,
                terminal_ordinal,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::successor_fiber_exhaustion_accepted(
                successor_receipt.clone(),
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::successor_enumeration_sealed(
                &successor_receipt,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::work_node_completed(
                successor_work_id,
                WorkCompletionRef::SuccessorsSealed {
                    relation_id: contract.relation_id(),
                    source_key,
                    terminal_ordinal,
                    receipt_id: successor_receipt.id(),
                },
            ))
            .unwrap();

        for cursor in [child_resume, root_resume] {
            let exhausted = sources.advance(&cursor, &mut runtime).unwrap();
            assert!(matches!(
                &exhausted,
                RelationalSourceAdvance::Exhausted { receipt, .. }
                    if receipt.terminal_ordinal() == 1
            ));
            let event = journal.source_traversal_event(exhausted).unwrap();
            journal.append(event).unwrap();
        }
        let source_seal = journal.source_enumeration_seal_event().unwrap();
        journal.append(source_seal).unwrap();

        let closed_snapshot = journal.snapshot().unwrap();
        assert_eq!(
            closed_snapshot.relation().counts().cases(),
            RelationCountEvidence::Exact(1)
        );
        assert_eq!(
            closed_snapshot.admission().admitted(),
            RelationCountEvidence::Exact(1)
        );
        assert_eq!(
            closed_snapshot.question(question_id).unwrap().selected(),
            RelationCountEvidence::Exact(1)
        );
        let transition_counts = closed_snapshot.transition_support_counts();
        assert_eq!(transition_counts.states(), 2);
        for layer in [
            RelationalTransitionLayer::Universe,
            RelationalTransitionLayer::Admitted,
            RelationalTransitionLayer::Matched(question_id),
        ] {
            assert_eq!(transition_counts.cases(layer), Some(1));
            assert_eq!(transition_counts.transitions(layer), Some(1));
            let support = journal
                .scheduler_view()
                .unwrap()
                .transition_support()
                .support_at_ordinal(layer, 0)
                .unwrap()
                .unwrap();
            assert_eq!(support.case_id(), case_id);
            assert_eq!(support.source_key(), source_key);
            assert_eq!(support.successor_key(), successor_key);
        }
        assert!(closed_snapshot
            .work()
            .nodes
            .iter()
            .all(|node| node.progress.is_complete()));

        let entries = journal.entries().to_vec();
        let replayed = RelationalJournal::replay(contract.clone(), entries.clone()).unwrap();
        let replayed_snapshot = replayed.snapshot().unwrap();
        assert_eq!(replayed.head(), journal.head());
        assert_eq!(
            replayed_snapshot.relation_frontier_root(),
            closed_snapshot.relation_frontier_root()
        );
        assert_eq!(
            replayed_snapshot.work_frontier_root(),
            closed_snapshot.work_frontier_root()
        );
        assert_eq!(
            replayed_snapshot.checkpoint_root(),
            closed_snapshot.checkpoint_root()
        );
        assert_eq!(replayed_snapshot.admission(), closed_snapshot.admission());
        assert_eq!(
            replayed_snapshot.question(question_id),
            closed_snapshot.question(question_id)
        );
        assert_eq!(
            replayed_snapshot.core_evidence_root(),
            closed_snapshot.core_evidence_root()
        );
        assert_eq!(
            replayed_snapshot.transition_support_root(),
            closed_snapshot.transition_support_root()
        );
        assert_eq!(
            replayed_snapshot.transition_support_counts(),
            closed_snapshot.transition_support_counts()
        );

        let mut tampered = entries;
        tampered[0].head = RelationalJournalHead([0; 32]);
        assert!(matches!(
            RelationalJournal::replay(contract, tampered),
            Err(RelationalJournalError::EntryHeadMismatch { sequence: 0 })
        ));
    }
}
