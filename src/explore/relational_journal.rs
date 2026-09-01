//! Authenticated append-only journal state machine for relational Explore.
//!
//! This module defines the event validation, hash chain and replay semantics
//! which a durable framed store must preserve. It is not by itself durable:
//! the storage adapter must install each encoded entry before publication.
//! Its chain commits semantic evidence events, while invocation limits and
//! scheduler order remain outside the contract. Replaying a valid chain
//! rebuilds the same stable relation, admission, FIND, and semantic work
//! frontiers.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::{Arc, Weak};

use sha2::{Digest, Sha256};

use super::mechanism_support::{
    MechanismSupportCheckpointCursor, MechanismSupportClosureRoot, MechanismSupportFrontierRoot,
};
use super::relation::{
    install_selected_case_batch, AdmissionCatalog, AdmissionCatalogBuilder, AdmissionContentRoot,
    AdmissionCounts, AdmissionDecision, AdmissionFrontierRoot, AdmissionId, MechanismRequestId,
    QuestionCatalog, QuestionCatalogBuilder, QuestionContentRoot, QuestionFrontierRoot, QuestionId,
    RelationCatalog, RelationCatalogBuilder, RelationCatalogError, RelationCatalogSnapshot,
    RelationClassificationError, RelationContentRoot, RelationCountEvidence, RelationFrontierRoot,
    RelationId, RelationProvenance, RelationalCaseId, RelationalCaseRef, SelectedCaseBatchError,
    SelectedCaseBatchRow, SelectionCounts, SelectionDecision, SourceKey, SourceRow, SuccessorKey,
    SuccessorRow,
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
use super::relational_analysis_plan::{RelationalAnalysisPlan, RelationalAnalysisPlanRoot};
use super::relational_bounded_chunk_partition::{
    reverify_relational_case_chunk_partition_artifact, RelationalCaseChunkId,
    RelationalCaseChunkPartitionArtifact, RelationalCaseChunkPartitionArtifactId,
    RelationalCaseChunkPartitionError, RelationalCaseChunkShape,
    VerifiedRelationalCaseChunkPartition,
};
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
use super::relational_selected_run_materialization::{
    reverify_relational_selected_run_materialization_artifact,
    RelationalSelectedRunMaterializationArtifact, RelationalSelectedRunMaterializationArtifactId,
    RelationalSelectedRunMaterializationError,
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
use super::relational_uniform_admission_proof::{
    reverify_relational_uniform_admission_artifact, RelationalUniformAdmissionProofArtifact,
    RelationalUniformAdmissionProofError,
};
use super::result_evidence::RelationalResultInputSeal;
use super::support_cell::{
    relational_case_chunk_partition_gateway, relational_case_image_proof_gateway,
    relational_classified_sweep_gateway, relational_uniform_admission_proof_gateway,
    AdmissionClassificationClaim, ExactCardinalityClaim, InjectiveMappingClaim,
    SelectionClassificationClaim, SupportCellEvidenceId, SupportCellId, SupportCellObligation,
    SupportMaterializationCursor, SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceCatalogBuilder, SupportEvidenceError, SupportEvidenceKind,
    SupportEvidenceRecord, SupportEvidenceRoot, SupportEvidenceSnapshot, SupportObligationRecord,
    SupportObligationRefinement, ValidatedSupportEvidenceClosure,
};
use super::support_journal::{SupportJournalError, SupportJournalEvent};
use super::transition::canonical_explore_value_digest;
use super::ExploreValue;

pub(crate) const RELATIONAL_JOURNAL_SCHEMA_VERSION: u32 = 16;

const JOURNAL_CONTRACT_HASH_V16: &[u8] = b"futuruna.explore.relational-journal-contract.v16";
const JOURNAL_GENESIS_HASH_V16: &[u8] = b"futuruna.explore.relational-journal-genesis.v16";
const JOURNAL_EVENT_HASH_V15: &[u8] = b"futuruna.explore.relational-journal-event.v15";
const JOURNAL_ENTRY_HASH_V16: &[u8] = b"futuruna.explore.relational-journal-entry.v16";
const CORE_EVIDENCE_ROOT_HASH_V4: &[u8] = b"futuruna.explore.relational-core-evidence-root.v4";
const EXPLORATION_EVIDENCE_ROOT_HASH_V2: &[u8] =
    b"futuruna.explore.relational-exploration-evidence-root.v2";
const EXHAUSTION_EVIDENCE_ROOT_HASH_V2: &[u8] =
    b"futuruna.explore.relational-exhaustion-evidence-root.v2";
const EXTENSIONAL_CONTENT_ROOT_HASH_V3: &[u8] =
    b"futuruna.explore.relational-extensional-content-root.v3";
const CHECKPOINT_ROOT_HASH_V4: &[u8] = b"futuruna.explore.relational-checkpoint-root.v4";

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

/// Semantic configuration sealed by one journal chain.
///
/// The analysis graph digest commits the resolved result/mechanism DAG. Its
/// producer identity is independent of authored order when dependency edges
/// and semantic layer identities are unchanged. CPU,
/// RAM, workers, deadlines, checkpoint paths, and declaration names are
/// intentionally absent so a run may resume under different safe resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalJournalContract {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    analysis_graph_digest: [u8; 32],
}

impl RelationalJournalContract {
    pub(crate) const fn new(
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        analysis_graph_digest: [u8; 32],
    ) -> Self {
        Self {
            relation_id,
            admission_id,
            question_id,
            analysis_graph_digest,
        }
    }

    pub(crate) const fn relation_id(self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn analysis_graph_digest(self) -> [u8; 32] {
        self.analysis_graph_digest
    }

    pub(crate) fn id(self) -> RelationalJournalId {
        let mut hasher = ChainHasher::new(JOURNAL_CONTRACT_HASH_V16);
        hasher.u32(RELATIONAL_JOURNAL_SCHEMA_VERSION);
        hasher.digest(self.relation_id.bytes());
        hasher.digest(self.admission_id.bytes());
        hasher.digest(self.question_id.bytes());
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
        let mut hasher = ChainHasher::new(JOURNAL_GENESIS_HASH_V16);
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
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    },
    Support(SupportJournalEvent),
    /// One post-FIND result/mechanism DAG mutation. Its subordinate digest is
    /// embedded in this journal's single ordered semantic chain; there is no
    /// second independently advancing analysis log.
    Analysis(RelationalAnalysisEvidenceEvent),
}

/// One resumability mutation. Checkpoints are authenticated by journal order,
/// but are deliberately absent from the arrival-order-independent semantic
/// evidence roots: a scheduler may reach the same proof frontier by a
/// different sequence of legal pauses and work choices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCheckpointEvent {
    /// One caller-bounded checked prefix of the next canonical classified
    /// chunk. Replay validates and folds the artifact into an operational
    /// accumulator without executing user code. It cannot advance classified
    /// support or the semantic chunk cursor; only the separately accepted
    /// canonical whole-chunk artifact may do that.
    RelationalClassifiedChunkSliceCheckpointed {
        artifact: Box<RelationalClassifiedChunkSliceArtifact>,
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
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Self {
        Self::Evidence(RelationalEvidenceEvent::QuestionClassified { case_id, decision })
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
            | Self::Checkpoint(
                RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::SupportMaterializationCheckpointed {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportFrontierCheckpointed { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkFrontierCompacted { .. }) => None,
        }
    }

    pub(crate) const fn compacted_work_node_count(&self) -> Option<u32> {
        match self {
            Self::Checkpoint(RelationalCheckpointEvent::WorkFrontierCompacted { receipt }) => {
                Some(receipt.removed_nodes())
            }
            Self::Evidence(_)
            | Self::Checkpoint(
                RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { .. },
            )
            | Self::Checkpoint(RelationalCheckpointEvent::WorkNodeInserted { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkReadinessMaterialized { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::WorkCursorAdvanced { .. })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportMaterializationCheckpointed {
                ..
            })
            | Self::Checkpoint(RelationalCheckpointEvent::SupportFrontierCheckpointed { .. })
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
/// consumers, but cannot advance it. Only accepted classified-chunk evidence
/// appends one exact canonical ordinal/artifact binding.
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
    artifact_id: RelationalClassifiedChunkArtifactId,
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

    /// Preflight one canonical ordinal without mutating the accepted prefix.
    /// `false` means the exact historical entry is already present.
    fn validate_chunk(
        &self,
        artifact: &RelationalClassifiedChunkArtifact,
    ) -> Result<bool, RelationalJournalError> {
        if artifact.chunk_partition_id() != self.partition_artifact_id
            || artifact.chunk_cell_id() == self.root_cell_id
            || artifact.chunk_materializer_id() != self.root_materializer_id
            || artifact.interval_start() < self.interval_start
            || artifact.interval_end_exclusive() > self.interval_end_exclusive
        {
            return Err(RelationalJournalError::ClassifiedSweepProgressScopeMismatch);
        }
        let expected = RelationalClassifiedSweepAcceptedChunk {
            chunk_id: artifact.chunk_id(),
            chunk_ordinal: artifact.chunk_ordinal(),
            artifact_id: artifact.id(),
            interval_start: artifact.interval_start(),
            interval_end_exclusive: artifact.interval_end_exclusive(),
        };
        let ordinal = usize::try_from(artifact.chunk_ordinal()).map_err(|_| {
            RelationalJournalError::ClassifiedSweepProgressGap {
                expected: self.next_chunk_ordinal,
                actual: artifact.chunk_ordinal(),
            }
        })?;
        if let Some(existing) = self.accepted_chunks.get(ordinal) {
            if existing == &expected {
                return Ok(false);
            }
            return Err(RelationalJournalError::ClassifiedSweepProgressConflict {
                chunk_ordinal: artifact.chunk_ordinal(),
            });
        }
        if ordinal != self.accepted_chunks.len()
            || artifact.chunk_ordinal() != self.next_chunk_ordinal
        {
            return Err(RelationalJournalError::ClassifiedSweepProgressGap {
                expected: self.next_chunk_ordinal,
                actual: artifact.chunk_ordinal(),
            });
        }
        let relative_start = artifact
            .interval_start()
            .checked_sub(self.interval_start)
            .ok_or(RelationalJournalError::ClassifiedSweepProgressScopeMismatch)?;
        let relative_end = artifact
            .interval_end_exclusive()
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
                actual: artifact.chunk_ordinal(),
            },
        )?;
        Ok(true)
    }

    /// Commit a chunk already accepted by `validate_chunk`. The partition-sized
    /// capacity reservation makes this an infallible post-transaction delta.
    fn commit_validated_chunk(&mut self, artifact: &RelationalClassifiedChunkArtifact) {
        debug_assert_eq!(self.validate_chunk(artifact), Ok(true));
        debug_assert!(self.accepted_chunks.len() < self.accepted_chunks.capacity());
        self.accepted_chunks
            .push(RelationalClassifiedSweepAcceptedChunk {
                chunk_id: artifact.chunk_id(),
                chunk_ordinal: artifact.chunk_ordinal(),
                artifact_id: artifact.id(),
                interval_start: artifact.interval_start(),
                interval_end_exclusive: artifact.interval_end_exclusive(),
            });
        self.next_chunk_ordinal += 1;
        self.next_coordinate_ordinal = artifact
            .interval_end_exclusive()
            .checked_sub(self.interval_start)
            .expect("the preflight validated the chunk interval");
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

    pub(crate) fn accepted_chunk_count(&self) -> usize {
        self.accepted_chunks.len()
    }

    pub(crate) const fn next_chunk_ordinal(&self) -> u128 {
        self.next_chunk_ordinal
    }

    pub(crate) const fn next_coordinate_ordinal(&self) -> u128 {
        self.next_coordinate_ordinal
    }

    pub(crate) fn last_artifact_id(&self) -> Option<RelationalClassifiedChunkArtifactId> {
        self.accepted_chunks.last().map(|chunk| chunk.artifact_id)
    }

    fn has_accepted_chunks(&self) -> bool {
        !self.accepted_chunks.is_empty()
    }
}

#[derive(Clone, Debug)]
struct RelationalEvidenceState {
    contract: RelationalJournalContract,
    constructor_interner: RelationalConstructorInterner,
    relation: RelationCatalogBuilder,
    admission: AdmissionCatalogBuilder,
    question: QuestionCatalogBuilder,
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
    classified_chunk_artifacts: Vec<RelationalClassifiedChunkArtifact>,
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
    work: RelationalWorkFrontier,
}

/// Durable request-local anchor for the factorized support join. The cursor
/// is part of the receipt: a root alone cannot bound how much work replay is
/// authorized to perform before checking the claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalMechanismSupportCheckpointReceipt {
    cursor: MechanismSupportCheckpointCursor,
    frontier_root: MechanismSupportFrontierRoot,
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

impl RelationalEvidenceState {
    fn new(contract: RelationalJournalContract) -> Self {
        let mut support = SupportEvidenceCatalogBuilder::new();
        support
            .register_admission(contract.admission_id, contract.relation_id)
            .expect("an empty support catalog accepts its contract admission layer");
        support
            .register_question(contract.question_id, contract.admission_id)
            .expect("an empty support catalog accepts its contract question layer");
        Self {
            contract,
            constructor_interner: RelationalConstructorInterner::default(),
            relation: RelationCatalogBuilder::new(contract.relation_id),
            admission: AdmissionCatalogBuilder::new(contract.relation_id, contract.admission_id),
            question: QuestionCatalogBuilder::new(
                contract.relation_id,
                contract.admission_id,
                contract.question_id,
            ),
            analysis_plan: None,
            analysis: None,
            support_plan: None,
            source_image_exactness: None,
            source_traversal: None,
            source_relation_exhaustion: None,
            verified_case_chunk_partition: None,
            classified_sweep_progress: None,
            classified_chunk_accumulator: None,
            classified_chunk_artifacts: Vec::new(),
            selected_run_materializations: BTreeMap::new(),
            selected_run_materialization_ids: BTreeMap::new(),
            successor_exhaustion_receipts: BTreeMap::new(),
            support,
            latest_support_frontiers: BTreeMap::new(),
            work: RelationalWorkFrontier::new(),
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
                self.relation.insert_successor(*source_key, row.clone())?;
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
                self.admission
                    .classify_open(&self.relation, *case_id, *decision)?;
            }
            RelationalEvidenceEvent::QuestionClassified { case_id, decision } => {
                self.question.classify_open(
                    &self.relation,
                    &self.admission,
                    *case_id,
                    *decision,
                )?;
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
        if plan.question_id() != self.contract.question_id()
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
            || plan.question_id() != self.question.question_id()
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
        let classified_sweep_progress = match &self.classified_sweep_progress {
            Some(existing) => {
                existing.validate_partition(artifact)?;
                existing.clone()
            }
            None => {
                if self
                    .support
                    .latest_cursor(artifact.root_cell_id())
                    .is_some()
                {
                    return Err(RelationalJournalError::CaseChunkRootCursorAlreadyExists);
                }
                RelationalClassifiedSweepProgress::from_partition(artifact)?
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
        self.classified_sweep_progress = Some(classified_sweep_progress);
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
            || verified_partition.artifact().question_id() != plan.question_id()
            || verified_partition.artifact().id() != artifact.chunk_partition_id()
        {
            return Err(RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch);
        }
        let progress = self
            .classified_sweep_progress
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedSweepProgressMissing)?;
        progress.validate_partition(verified_partition.artifact())?;
        if artifact.chunk_ordinal() != progress.next_chunk_ordinal()
            || progress.accepted_chunk_count() != self.classified_chunk_artifacts.len()
        {
            return Err(
                RelationalJournalError::ClassifiedChunkSliceProgressMismatch {
                    expected_chunk_ordinal: progress.next_chunk_ordinal(),
                    actual_chunk_ordinal: artifact.chunk_ordinal(),
                },
            );
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
        let (
            verified,
            chunk_admission,
            run_admissions,
            run_refinement,
            cursor,
            advances_classified_sweep,
            finalizes_active_slice,
        ) = {
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
                || verified_partition.artifact().question_id() != plan.question_id()
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
            let finalizes_active_slice = match self.classified_chunk_artifacts.get(chunk_ordinal) {
                Some(existing) if existing == artifact => false,
                Some(_) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    );
                }
                None => {
                    let accumulator = self.classified_chunk_accumulator.as_ref().ok_or(
                        RelationalJournalError::ClassifiedChunkSliceAccumulatorMissing {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    )?;
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

            let mut admitted_selection_activation = false;
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
                        || *question_id != plan.question_id()
                    {
                        return Err(RelationalJournalError::InvalidSupportPlanActivation);
                    }
                    admitted_selection_activation = true;
                }
            }
            if verified
                .runs()
                .iter()
                .any(|run| run.descriptor().outcome().selection().is_some())
                && !admitted_selection_activation
            {
                return Err(RelationalJournalError::InvalidSupportPlanActivation);
            }

            let root_cell = plan
                .cases()
                .cell()
                .ok_or(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch)?;
            let root_interval_start = verified_partition.artifact().interval_start();
            let relative_start = artifact
                .interval_start()
                .checked_sub(root_interval_start)
                .ok_or(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch)?;
            let relative_end = artifact
                .interval_end_exclusive()
                .checked_sub(root_interval_start)
                .ok_or(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch)?;
            let Some(root_coordinate_count) = root_cell.coordinate_cardinality().exact() else {
                return Err(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch);
            };
            if relative_start >= relative_end || relative_end > root_coordinate_count {
                return Err(RelationalJournalError::ClassifiedChunkCursorBoundsMismatch);
            }

            // Rank-based chunks need no opaque backend continuation. The
            // artifact identity is nevertheless retained as the canonical
            // boundary checkpoint, preventing another classified transcript
            // from claiming the same cursor position.
            let cursor = SupportMaterializationCursor::at_start(root_cell)
                .and_then(|start| {
                    start.advance(
                        root_cell,
                        relative_end,
                        artifact.id().bytes().to_vec().into_boxed_slice(),
                    )
                })
                .map_err(RelationalClassifiedSweepError::from)?;
            let previous_relative = classified_sweep_progress.next_coordinate_ordinal();
            let previous_artifact_id = classified_sweep_progress.last_artifact_id();
            let advanced = classified_sweep_progress.validate_chunk(artifact)?;
            if advanced {
                if self
                    .support
                    .cursor_at(root_cell.id(), relative_end)
                    .is_some()
                {
                    return Err(RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch);
                }
                match (
                    previous_relative,
                    previous_artifact_id,
                    self.support.latest_cursor(root_cell.id()),
                ) {
                    (0, None, None) => {}
                    (expected, Some(previous_artifact_id), Some(previous)) => {
                        previous
                            .validate_for(root_cell)
                            .map_err(RelationalClassifiedSweepError::from)?;
                        if previous.next_coordinate_ordinal() != expected {
                            return Err(
                                RelationalJournalError::ClassifiedChunkCursorPredecessorMismatch {
                                    expected,
                                    actual: previous.next_coordinate_ordinal(),
                                },
                            );
                        }
                        let expected_checkpoint = previous_artifact_id.bytes();
                        if previous.checkpoint() != expected_checkpoint.as_slice() {
                            return Err(
                                RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch,
                            );
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
                            RelationalJournalError::ClassifiedChunkCursorPredecessorMissing {
                                expected,
                            },
                        );
                    }
                }
            } else {
                match self.support.cursor_at(root_cell.id(), relative_end) {
                    Some(durable) if durable == &cursor => {}
                    Some(_) | None => {
                        return Err(
                            RelationalJournalError::ClassifiedChunkCursorCheckpointMismatch,
                        );
                    }
                }
            }

            (
                verified,
                chunk_admission,
                run_admissions,
                run_refinement,
                cursor,
                advanced,
                finalizes_active_slice,
            )
        };

        // Retain the exact accepted producer artifact in canonical chunk
        // order. The typed progress record is still the cursor authority; this
        // parallel payload index exists so later sparse materialization can
        // reverify a selected run without replaying or rerunning user code.
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal())
            .map_err(|_| RelationalJournalError::ClassifiedChunkPartitionIdentityMismatch)?;
        let retain_new_classified_artifact =
            match self.classified_chunk_artifacts.get(chunk_ordinal) {
                Some(existing) if existing == artifact => false,
                Some(_) => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionConflict {
                            chunk_ordinal: artifact.chunk_ordinal(),
                        },
                    );
                }
                None if chunk_ordinal == self.classified_chunk_artifacts.len() => true,
                None => {
                    return Err(
                        RelationalJournalError::ClassifiedChunkArtifactRetentionGap {
                            expected: self.classified_chunk_artifacts.len() as u128,
                            actual: artifact.chunk_ordinal(),
                        },
                    );
                }
            };
        if retain_new_classified_artifact {
            // Capacity is operational, not semantic. Reserve before any state
            // commit so the final canonical push cannot allocate, while
            // avoiding an O(prefix^2) clone of retained transcripts.
            self.classified_chunk_artifacts
                .try_reserve(1)
                .map_err(|_| {
                    RelationalJournalError::ClassifiedChunkArtifactRetentionAllocationFailed
                })?;
        }
        let retained_classified_artifact = retain_new_classified_artifact.then(|| artifact.clone());

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
            let selection = if verified.runs()[run_ordinal]
                .descriptor()
                .outcome()
                .selection()
                .is_some()
            {
                let selection =
                    relational_classified_sweep_gateway::selection(&verified, run_ordinal)
                        .map_err(RelationalClassifiedSweepError::from)?;
                Some((
                    SupportObligationRecord::Selection(selection.obligation().clone()),
                    SupportEvidenceRecord::Selection(selection),
                ))
            } else {
                None
            };
            classification_evidence.push((
                SupportEvidenceRecord::Admission(admission),
                selection,
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
        // catalog (twice) for every chunk. The cursor remains deliberately last
        // and outside the support evidence root.
        let undo_capacity = verified
            .runs()
            .len()
            .checked_mul(9)
            .and_then(|capacity| capacity.checked_add(3))
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

        for (admission, selection, cell_id) in classification_evidence {
            support.insert_declared_evidence_record(admission)?;
            if let Some((obligation, evidence)) = selection {
                support.declare_root_obligation_record(obligation)?;
                support.insert_declared_evidence_record(evidence)?;
            }
            support.seal_known_leaf(cell_id)?;
        }
        support.insert_cursor(cursor)?;
        support.commit();
        if advances_classified_sweep {
            self.classified_sweep_progress
                .as_mut()
                .expect("the classified progress was present during preflight")
                .commit_validated_chunk(artifact);
        }
        if finalizes_active_slice {
            self.classified_chunk_accumulator = None;
        }
        if let Some(artifact) = retained_classified_artifact {
            self.classified_chunk_artifacts.push(artifact);
        }
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
            || verified_partition.artifact().question_id() != plan.question_id()
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

    fn accept_relational_selected_run_materialization(
        &mut self,
        artifact: &RelationalSelectedRunMaterializationArtifact,
    ) -> Result<(), RelationalJournalError> {
        let chunk_ordinal = usize::try_from(artifact.chunk_ordinal()).map_err(|_| {
            RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                chunk_ordinal: artifact.chunk_ordinal(),
            }
        })?;
        let classified_artifact = self.classified_chunk_artifacts.get(chunk_ordinal).ok_or(
            RelationalJournalError::SelectedRunClassifiedArtifactMissing {
                chunk_ordinal: artifact.chunk_ordinal(),
            },
        )?;
        if classified_artifact.id() != artifact.classified_chunk_artifact_id() {
            return Err(RelationalJournalError::SelectedRunClassifiedArtifactMismatch);
        }
        let verified_classified = self.reverify_retained_classified_chunk(classified_artifact)?;
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let verified_partition = self
            .verified_case_chunk_partition
            .as_ref()
            .ok_or(RelationalJournalError::ClassifiedChunkCanonicalPartitionUnavailable)?;
        let verified = reverify_relational_selected_run_materialization_artifact(
            artifact,
            plan,
            verified_partition,
            &verified_classified,
            artifact.run_ordinal(),
        )?;
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
        let retained_artifact = artifact.clone();

        // Validate against the durable prefixes and build one bounded local
        // relation delta before mutating any of the three concrete catalogs.
        // The final merge has no semantic failure path and does not clone the
        // selected prefix. No enumeration seal is minted here: these remain
        // sparse witnesses emerging from an open certified population.
        install_selected_case_batch(
            &mut self.relation,
            &mut self.admission,
            &mut self.question,
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

        // All semantic conflicts were rejected before the batch merge. These
        // two indexes were likewise collision-checked above; their insertion
        // only publishes the already accepted bounded artifact.
        let previous = self
            .selected_run_materializations
            .insert(run_cell_id, retained_artifact);
        debug_assert!(previous.is_none());
        let previous_id = self
            .selected_run_materialization_ids
            .insert(artifact.id(), run_cell_id);
        debug_assert!(previous_id.is_none());
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
        let sealing_obligations = matches!(event, SupportJournalEvent::ObligationFrontierSealed);

        if sealing_obligations {
            // Sealing changes only one monotone bit, so validate the current
            // immutable prefix before mutation instead of cloning the complete
            // support catalog for rollback. Open-obligation membership is
            // unchanged by the seal itself.
            let closure = self.support.validated_closure()?;
            if closure.has_open_obligation_kind(SupportEvidenceKind::Admission) {
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
                        || *question_id != self.question.question_id()
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
            let expected = self.remint_selected_question_seal(seal.authority())?;
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
            if durable.frontier_root != derived_frontier {
                return Err(
                    RelationalJournalError::SupportCheckpointAnchorRootMismatch {
                        request_id: *request_id,
                    },
                );
            }
            analysis.apply(event)?;
            return Ok(());
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
        authority: RelationalSelectedPopulationAuthority,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        match authority {
            RelationalSelectedPopulationAuthority::ExtensionalQuestion { .. } => {
                self.remint_extensional_selected_question_seal()
            }
            RelationalSelectedPopulationAuthority::CertifiedSupport { .. } => {
                self.remint_certified_selected_question_seal()
            }
        }
    }

    fn remint_extensional_selected_question_seal(
        &self,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        let relation = self.relation.close_borrowed()?;
        let admission = self.admission.close_borrowed(&relation)?;
        let question = self.question.close_borrowed(&relation, &admission)?;
        Ok(RelationalSelectedQuestionSeal::from_borrowed_closed_question(&question)?)
    }

    fn remint_certified_selected_question_seal(
        &self,
    ) -> Result<RelationalSelectedQuestionSeal, RelationalJournalError> {
        let plan = self
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let support = self.support.validated_closure()?;
        if !support.catalog_is_sealed() {
            return Err(RelationalJournalError::SupportCatalogOpen);
        }
        let population =
            ClosedCertifiedSelectedPopulation::derive_from_validated_support(plan, &support)?;
        let selected_case_ids = self.certified_selected_materialization_case_ids(&population)?;
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
        population: &ClosedCertifiedSelectedPopulation,
    ) -> Result<Vec<RelationalCaseId>, RelationalJournalError> {
        let certified = population.exact_cardinality();
        let catalog = self.question.selected_count() as u128;

        let Some(progress) = self.classified_sweep_progress.as_ref() else {
            if population.is_exact_empty()
                && self.classified_chunk_artifacts.is_empty()
                && self.selected_run_materializations.is_empty()
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
            || progress.accepted_chunk_count() != self.classified_chunk_artifacts.len()
        {
            return Err(RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen);
        }

        let mut expected_run_count = 0usize;
        let mut materialized = 0u128;
        let mut all_materialized_cases_are_selected = true;
        for classified in &self.classified_chunk_artifacts {
            for run in classified.runs() {
                if run.outcome() != RelationalClassifiedCaseOutcome::AdmittedSelected {
                    continue;
                }
                expected_run_count = expected_run_count
                    .checked_add(1)
                    .ok_or(RelationalJournalError::SequenceOverflow)?;
                let artifact = self
                    .selected_run_materializations
                    .get(&run.cell_id())
                    .ok_or(RelationalJournalError::CertifiedSelectedMaterializationCoverageOpen)?;
                materialized = materialized
                    .checked_add(artifact.materialized_case_count())
                    .ok_or(RelationalJournalError::SequenceOverflow)?;
                all_materialized_cases_are_selected &= artifact.cases().iter().all(|record| {
                    self.question.decision(record.case_id()) == Some(SelectionDecision::Selected)
                });
            }
        }

        if self.selected_run_materializations.len() != expected_run_count
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

        let selected_case_ids = self.question.selected_case_ids().collect::<Vec<_>>();
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
        let supplied = analysis
            .selected_question()
            .ok_or(RelationalJournalError::AnalysisNotClosed)?;
        let expected = self.remint_selected_question_seal(supplied.authority())?;
        if supplied != expected {
            return Err(RelationalJournalError::SelectedQuestionSealBaseMismatch);
        }
        Ok(())
    }

    fn apply_checkpoint(
        &mut self,
        event: &RelationalCheckpointEvent,
    ) -> Result<(), RelationalJournalError> {
        match event {
            RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { artifact } => {
                self.accept_relational_classified_chunk_slice(artifact)?;
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
                let prior = self
                    .latest_support_frontiers
                    .get(request_id)
                    .map_or_else(MechanismSupportCheckpointCursor::default, |receipt| {
                        receipt.cursor
                    });
                validate_support_checkpoint_delta(*request_id, prior, *cursor)?;
                self.restore_analysis_support_checkpoint_through(*request_id, *cursor)?;
                let derived = self
                    .analysis
                    .as_mut()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?
                    .checkpoint_support_frontier(*request_id)?;
                if derived != *frontier_root {
                    return Err(RelationalJournalError::SupportFrontierRootClaimMismatch {
                        request_id: *request_id,
                        claimed: *frontier_root,
                        derived,
                    });
                }
                self.latest_support_frontiers.insert(
                    *request_id,
                    RelationalMechanismSupportCheckpointReceipt {
                        cursor: *cursor,
                        frontier_root: derived,
                    },
                );
            }
            RelationalCheckpointEvent::WorkNodeCompleted {
                node_id,
                completion,
            } => {
                self.validate_completion_reference(completion)?;
                self.work.complete(*node_id, completion.clone())?;
            }
            RelationalCheckpointEvent::WorkFrontierCompacted { receipt } => {
                self.work.compact(*receipt)?;
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
                if *question_id != self.question.question_id()
                    || self.question.decision(*case_id) != Some(*decision)
                {
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
    pub(crate) const fn contract(self) -> RelationalJournalContract {
        self.journal.contract
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn head(self) -> RelationalJournalHead {
        self.journal.head
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

    /// Typed contiguous classified prefix rebuilt only from accepted outer
    /// evidence events. This, rather than a generic cursor, is the scheduler's
    /// authority for choosing the next canonical chunk.
    pub(crate) fn classified_sweep_progress(self) -> Option<&'a RelationalClassifiedSweepProgress> {
        self.journal.state.classified_sweep_progress.as_ref()
    }

    /// Replay-derived checked prefix of the next canonical chunk. This is
    /// operational checkpoint authority only: support and classified progress
    /// still stop at the preceding complete chunk until this accumulator is
    /// finalized and its canonical whole-chunk artifact is accepted.
    pub(crate) fn classified_chunk_accumulator(
        self,
    ) -> Option<&'a RelationalClassifiedChunkAccumulator> {
        self.journal.state.classified_chunk_accumulator.as_ref()
    }

    /// Retained accepted chunk payloads in canonical partition ordinal order.
    /// The typed progress record remains the cursor authority; this slice is
    /// the replay input for sparse selected-run realization.
    pub(crate) fn classified_chunk_artifacts(self) -> &'a [RelationalClassifiedChunkArtifact] {
        &self.journal.state.classified_chunk_artifacts
    }

    /// Borrow the opaque partition authority reconstructed when the
    /// authenticated partition event entered this journal state. The value is
    /// deliberately absent before that event and is never restored from a
    /// standalone cache or snapshot.
    pub(crate) fn verified_case_chunk_partition(
        self,
    ) -> Option<&'a VerifiedRelationalCaseChunkPartition> {
        self.journal.state.verified_case_chunk_partition.as_ref()
    }

    pub(crate) fn selected_run_materialization(
        self,
        run_cell_id: SupportCellId,
    ) -> Option<&'a RelationalSelectedRunMaterializationArtifact> {
        self.journal
            .state
            .selected_run_materializations
            .get(&run_cell_id)
    }

    /// Whether every admitted+selected run in the accepted classified prefix
    /// has exactly one admitted sparse materialization and no other run-cell
    /// payload is present. Full-population closure additionally requires the
    /// classified progress itself to cover the complete canonical partition.
    pub(crate) fn selected_run_materializations_cover_classified_prefix(self) -> bool {
        let mut expected = 0usize;
        for artifact in &self.journal.state.classified_chunk_artifacts {
            for run in artifact.runs() {
                if run.outcome() != RelationalClassifiedCaseOutcome::AdmittedSelected {
                    continue;
                }
                expected = match expected.checked_add(1) {
                    Some(expected) => expected,
                    None => return false,
                };
                if !self
                    .journal
                    .state
                    .selected_run_materializations
                    .contains_key(&run.cell_id())
                {
                    return false;
                }
            }
        }
        self.journal.state.selected_run_materializations.len() == expected
    }

    pub(crate) fn selected_run_materialization_count(self) -> usize {
        self.journal.state.selected_run_materializations.len()
    }

    pub(crate) fn selected_run_materializations(
        self,
    ) -> impl Iterator<Item = &'a RelationalSelectedRunMaterializationArtifact> + 'a {
        self.journal.state.selected_run_materializations.values()
    }

    /// Concrete selected CaseIds admitted by sparse run artifacts. They are
    /// content-derived and unique across artifacts; this prefix iterator does
    /// not itself claim that the selected population is closed.
    pub(crate) fn materialized_selected_case_ids(
        self,
    ) -> impl Iterator<Item = RelationalCaseId> + 'a {
        self.journal
            .state
            .selected_run_materializations
            .values()
            .flat_map(|artifact| artifact.cases().iter().map(|record| record.case_id()))
    }

    /// Canonical concrete selected CaseId order from the incrementally
    /// authenticated FIND catalog. On the classified branch, complete sparse
    /// run coverage proves this is the concrete image of the selected support
    /// population rather than merely an observed lower bound.
    pub(crate) fn canonical_concrete_selected_case_ids(
        self,
    ) -> impl Iterator<Item = RelationalCaseId> + 'a {
        self.journal.state.question.selected_case_ids()
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
    ) -> Result<Option<RelationalClassificationProgressCounts>, RelationalJournalError> {
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

    pub(crate) fn question_decision(self, case_id: RelationalCaseId) -> Option<SelectionDecision> {
        self.journal.state.question.decision(case_id)
    }

    pub(crate) fn admission_decision_count(self) -> usize {
        self.journal.state.admission.decision_count()
    }

    pub(crate) fn admitted_count(self) -> usize {
        self.journal.state.admission.admitted_count()
    }

    pub(crate) fn question_decision_count(self) -> usize {
        self.journal.state.question.decision_count()
    }

    pub(crate) fn selected_count(self) -> usize {
        self.journal.state.question.selected_count()
    }

    /// Canonical selected CaseIds from the incremental FIND catalog. This is
    /// a borrow-only scheduling index, not proof that FIND has closed; callers
    /// must separately require the authenticated selected-question seal.
    pub(crate) fn selected_case_ids(self) -> impl Iterator<Item = RelationalCaseId> + 'a {
        self.journal.state.question.selected_case_ids()
    }

    /// Borrow the operational selected-discovery suffix reconstructed by
    /// journal replay. Its ordinal is suitable only for an invocation-local
    /// catch-up cursor; canonical roots and exact seals continue to use the
    /// arrival-order-independent classification map.
    pub(crate) fn selected_discovery_suffix(self, from_ordinal: usize) -> &'a [RelationalCaseId] {
        self.journal
            .state
            .question
            .selected_discovery_suffix(from_ordinal)
    }

    pub(crate) fn concrete_base_is_classified(self) -> bool {
        self.relation_enumeration_is_complete()
            && self.admission_decision_count() == self.case_count()
            && self.question_decision_count() == self.admitted_count()
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
        Self::with_history_retention(contract, true)
    }

    /// Construct the production fold used with an external durable segment
    /// sink. Applied entries are returned to that sink and are not retained a
    /// second time beside their folded catalog state.
    pub(crate) fn new_streaming(contract: RelationalJournalContract) -> Self {
        Self::with_history_retention(contract, false)
    }

    fn with_history_retention(contract: RelationalJournalContract, retain_history: bool) -> Self {
        Self {
            contract,
            sequence: 0,
            head: RelationalJournalHead::genesis(contract.id()),
            entries: Vec::new(),
            retain_history,
            state: RelationalEvidenceState::new(contract),
        }
    }

    pub(crate) const fn contract(&self) -> RelationalJournalContract {
        self.contract
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
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let seal = self.state.remint_extensional_selected_question_seal()?;
        Ok(RelationalJournalEvent::analysis(
            RelationalAnalysisEvidenceEvent::selected_question_bound(seal),
        ))
    }

    /// Mint the post-FIND bridge from exact sealed SupportCell evidence and
    /// the independently complete concrete CaseId image. Exact-empty support
    /// needs no rows; positive support must be covered by every selected run.
    pub(crate) fn selected_question_certified_event(
        &self,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let seal = self.state.remint_certified_selected_question_seal()?;
        Ok(RelationalJournalEvent::analysis(
            RelationalAnalysisEvidenceEvent::selected_question_bound(seal),
        ))
    }

    /// Derive, rather than accept, the only terminal analysis event valid for
    /// the current journal prefix.
    pub(crate) fn analysis_terminal_event(
        &self,
    ) -> Result<RelationalJournalEvent, RelationalJournalError> {
        let event = self
            .state
            .analysis
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .terminal_event()?;
        Ok(RelationalJournalEvent::analysis(event))
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
        maximum_target_cases: NonZeroU16,
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
            .as_ref()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .support_checkpoint_cursors(request_id)?;
        let derived_state_is_ahead =
            validate_support_checkpoint_delta(request_id, anchor_cursor, cursor)?;

        let accepted_target_cases = if derived_state_is_ahead {
            // A discarded proposal left only bounded derived-cache progress.
            // Re-emit its exact cursor before doing any further work.
            0
        } else {
            if let Some(durable) = durable {
                let anchored_root = self
                    .state
                    .analysis
                    .as_mut()
                    .ok_or(RelationalJournalError::AnalysisStateMissing)?
                    .checkpoint_support_frontier(request_id)?;
                if anchored_root != durable.frontier_root {
                    return Err(
                        RelationalJournalError::SupportCheckpointAnchorRootMismatch { request_id },
                    );
                }
            }
            let runtime_limit = u128::from(maximum_target_cases.get())
                .min(RELATIONAL_SUPPORT_CHECKPOINT_MAX_LANE_DELTA);
            let runtime_limit = NonZeroU16::new(
                u16::try_from(runtime_limit)
                    .expect("the protocol support-checkpoint bound fits u16"),
            )
            .expect("a nonzero runtime limit remains nonzero after protocol capping");
            let (accepted, advanced, upstream) = self
                .state
                .advance_analysis_support_checkpoint_bounded(request_id, runtime_limit)?;
            validate_support_checkpoint_delta(request_id, cursor, advanced)?;
            cursor = advanced;
            available = upstream;
            accepted
        };

        let frontier_root = self
            .state
            .analysis
            .as_mut()
            .ok_or(RelationalJournalError::AnalysisStateMissing)?
            .checkpoint_support_frontier(request_id)?;
        let next_receipt = RelationalMechanismSupportCheckpointReceipt {
            cursor,
            frontier_root,
        };
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
        Self::replay_with_retention(contract, entries, true)
    }

    /// Rebuild the production fold from durable segments without retaining a
    /// second in-memory copy of every decoded frame.
    pub(crate) fn replay_streaming(
        contract: RelationalJournalContract,
        entries: impl IntoIterator<Item = RelationalJournalEntry>,
    ) -> Result<Self, RelationalJournalError> {
        Self::replay_with_retention(contract, entries, false)
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
    ) -> Result<Self, RelationalJournalError> {
        let mut journal = Self::with_history_retention(contract, retain_history);
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
        let question_frontier_root = self.state.question.frontier_root(admission_frontier_root);
        let admission = self.state.admission.counts_at(&relation)?;
        let question = self
            .state
            .question
            .counts_at(&relation, &self.state.admission)?;
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
            self.contract,
            analysis_plan_root,
            support_plan_root,
            exhaustion_evidence_root,
            relation.frontier_root(),
            admission_frontier_root,
            question_frontier_root,
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
            self.contract,
            core_evidence_root,
            analysis_scope_root,
            analysis_catalog.as_ref().map(|catalog| catalog.root()),
            analysis_terminal_root,
            analysis_closure_set_root,
        );
        let work = self.state.work.snapshot()?;
        let checkpoint_root = relational_checkpoint_root(
            self.contract,
            work.root,
            &support,
            self.state.classified_chunk_accumulator.as_ref(),
            &self.state.latest_support_frontiers,
        );
        Ok(RelationalJournalSnapshot {
            version: RELATIONAL_JOURNAL_SCHEMA_VERSION,
            contract: self.contract,
            sequence: self.sequence,
            head: self.head,
            relation_frontier_root: relation.frontier_root(),
            admission_frontier_root,
            question_frontier_root,
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
            question,
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
    ) -> Result<ClosedCertifiedRelationalCore, RelationalJournalError> {
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
        let support_plan = self
            .state
            .support_plan
            .as_ref()
            .ok_or(RelationalJournalError::SupportPlanMissing)?;
        let selected_population =
            ClosedCertifiedSelectedPopulation::derive(support_plan, snapshot.support())?;
        Ok(ClosedCertifiedRelationalCore {
            contract: self.contract,
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
    ) -> Result<ClosedCertifiedRelationalEvidence, RelationalJournalError> {
        self.state.validate_closed_analysis_bridge()?;
        let core = self.finish_certified_core()?;
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
            constructor_interner: _,
            relation,
            admission,
            question,
            analysis_plan,
            analysis,
            support_plan,
            source_image_exactness: _,
            source_traversal,
            source_relation_exhaustion,
            verified_case_chunk_partition: _,
            classified_sweep_progress: _,
            classified_chunk_accumulator,
            classified_chunk_artifacts: _,
            selected_run_materializations: _,
            selected_run_materialization_ids: _,
            successor_exhaustion_receipts,
            support,
            latest_support_frontiers,
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
        let question_frontier_root = question.frontier_root(admission_frontier_root);
        let core_evidence_root = relational_core_evidence_root(
            self.contract,
            Some(analysis_plan_root),
            Some(support_plan_root),
            exhaustion_evidence_root,
            relation_frontier_root,
            admission_frontier_root,
            question_frontier_root,
            support.root(),
        );
        let relation = relation.finish()?;
        let admission = admission.finish(&relation)?;
        let question = question.finish(&relation, &admission)?;
        let extensional_content_root = relational_extensional_content_root(
            self.contract,
            analysis_plan_root,
            support_plan_root,
            source_relation_exhaustion_receipt_id,
            exhaustion_evidence_root,
            relation.content_root(),
            admission.content_root(),
            question.content_root(),
            support.root(),
        );
        let checkpoint_root = relational_checkpoint_root(
            self.contract,
            work_snapshot.root,
            &support,
            classified_chunk_accumulator.as_ref(),
            &latest_support_frontiers,
        );
        let analysis = analysis.ok_or(RelationalJournalError::AnalysisStateMissing)?;
        Ok(ClosedExtensionalRelationalEvidence {
            contract: self.contract,
            journal_head: self.head,
            relation_content_root: relation.content_root(),
            admission_content_root: admission.content_root(),
            question_content_root: question.content_root(),
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
            question,
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
    pub(crate) const fn contract(&self) -> RelationalJournalContract {
        self.contract
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
    question_frontier_root: QuestionFrontierRoot,
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
    question: SelectionCounts,
    support: SupportEvidenceSnapshot,
    work: WorkFrontierSnapshot,
}

impl RelationalJournalSnapshot {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn contract(&self) -> RelationalJournalContract {
        self.contract
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

    pub(crate) const fn question_frontier_root(&self) -> QuestionFrontierRoot {
        self.question_frontier_root
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

    pub(crate) const fn question(&self) -> SelectionCounts {
        self.question
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
    question_content_root: QuestionContentRoot,
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
    question: QuestionCatalog,
    support: SupportEvidenceSnapshot,
}

impl ClosedExtensionalRelationalEvidence {
    pub(crate) const fn contract(&self) -> RelationalJournalContract {
        self.contract
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

    pub(crate) const fn question_content_root(&self) -> QuestionContentRoot {
        self.question_content_root
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

    pub(crate) fn question(&self) -> &QuestionCatalog {
        &self.question
    }

    pub(crate) const fn support(&self) -> &SupportEvidenceSnapshot {
        &self.support
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalJournalError {
    Relation(RelationCatalogError),
    Classification(RelationClassificationError),
    SupportEvidence(SupportEvidenceError),
    SupportJournal(SupportJournalError),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    SourceImageProof(RelationalSourceImageExactnessProofError),
    CertifiedSourceSummary(RelationalCertifiedSourceSummaryError),
    CaseChunkPartition(RelationalCaseChunkPartitionError),
    ClassifiedSweep(RelationalClassifiedSweepError),
    SelectedRunMaterialization(RelationalSelectedRunMaterializationError),
    ClassificationCounts(CertifiedRelationalClassificationCountsError),
    UniformAdmissionProof(RelationalUniformAdmissionProofError),
    Work(WorkFrontierError),
    SourceTraversal(SourceTraversalClosureError),
    CertifiedPopulation(CertifiedSelectedPopulationError),
    Analysis(RelationalAnalysisJournalError),
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
    SupportCheckpointAfterClosure {
        request_id: MechanismRequestId,
    },
    SupportClosureCheckpointIncomplete {
        request_id: MechanismRequestId,
    },
    SupportClosureFrontierCheckpointMissing {
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

impl From<SelectedCaseBatchError> for RelationalJournalError {
    fn from(error: SelectedCaseBatchError) -> Self {
        match error {
            SelectedCaseBatchError::Catalog(error) => Self::Relation(error),
            SelectedCaseBatchError::Classification(error) => Self::Classification(error),
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
            Self::SupportEvidence(error) => fmt::Display::fmt(error, formatter),
            Self::SupportJournal(error) => fmt::Display::fmt(error, formatter),
            Self::CaseImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::SourceImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::CertifiedSourceSummary(error) => fmt::Display::fmt(error, formatter),
            Self::CaseChunkPartition(error) => fmt::Display::fmt(error, formatter),
            Self::ClassifiedSweep(error) => fmt::Display::fmt(error, formatter),
            Self::SelectedRunMaterialization(error) => fmt::Display::fmt(error, formatter),
            Self::ClassificationCounts(error) => fmt::Display::fmt(error, formatter),
            Self::UniformAdmissionProof(error) => fmt::Display::fmt(error, formatter),
            Self::Work(error) => fmt::Display::fmt(error, formatter),
            Self::SourceTraversal(error) => fmt::Display::fmt(error, formatter),
            Self::CertifiedPopulation(error) => fmt::Display::fmt(error, formatter),
            Self::Analysis(error) => fmt::Display::fmt(error, formatter),
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
            Self::SupportCheckpointAfterClosure { .. } => formatter.write_str(
                "mechanism-support checkpoint cannot advance or replace a closed request",
            ),
            Self::SupportClosureCheckpointIncomplete { .. } => formatter.write_str(
                "mechanism-support closure requires every authenticated checkpoint lane to be complete",
            ),
            Self::SupportClosureFrontierCheckpointMissing { .. } => formatter.write_str(
                "mechanism-support closure requires its complete durable frontier checkpoint",
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
            Self::CaseImageProof(error) => Some(error),
            Self::SourceImageProof(error) => Some(error),
            Self::CertifiedSourceSummary(error) => Some(error),
            Self::CaseChunkPartition(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::SelectedRunMaterialization(error) => Some(error),
            Self::ClassificationCounts(error) => Some(error),
            Self::UniformAdmissionProof(error) => Some(error),
            _ => None,
        }
    }
}

fn relational_core_evidence_root(
    contract: RelationalJournalContract,
    analysis_plan: Option<RelationalAnalysisPlanRoot>,
    support_plan: Option<RelationalSupportPlanRoot>,
    exhaustion: RelationalExhaustionEvidenceRoot,
    relation: RelationFrontierRoot,
    admission: AdmissionFrontierRoot,
    question: QuestionFrontierRoot,
    support: SupportEvidenceRoot,
) -> RelationalCoreEvidenceRoot {
    let mut hasher = ChainHasher::new(CORE_EVIDENCE_ROOT_HASH_V4);
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
    hasher.digest(support.bytes());
    RelationalCoreEvidenceRoot(hasher.finish())
}

fn relational_exploration_evidence_root(
    contract: RelationalJournalContract,
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
    contract: RelationalJournalContract,
    analysis_plan: RelationalAnalysisPlanRoot,
    support_plan: RelationalSupportPlanRoot,
    source_relation_exhaustion: SourceRelationExhaustionReceiptId,
    exhaustion: RelationalExhaustionEvidenceRoot,
    relation: RelationContentRoot,
    admission: AdmissionContentRoot,
    question: QuestionContentRoot,
    support: SupportEvidenceRoot,
) -> RelationalExtensionalContentRoot {
    let mut hasher = ChainHasher::new(EXTENSIONAL_CONTENT_ROOT_HASH_V3);
    hasher.digest(contract.id().bytes());
    hasher.digest(analysis_plan.bytes());
    hasher.digest(support_plan.bytes());
    hasher.digest(source_relation_exhaustion.bytes());
    hasher.digest(exhaustion.bytes());
    hasher.digest(relation.bytes());
    hasher.digest(admission.bytes());
    hasher.digest(question.bytes());
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
    contract: RelationalJournalContract,
    work: WorkFrontierRoot,
    support: &SupportEvidenceSnapshot,
    classified_chunk_accumulator: Option<&RelationalClassifiedChunkAccumulator>,
    latest_support_frontiers: &BTreeMap<
        MechanismRequestId,
        RelationalMechanismSupportCheckpointReceipt,
    >,
) -> RelationalCheckpointRoot {
    let mut hasher = ChainHasher::new(CHECKPOINT_ROOT_HASH_V4);
    hasher.digest(contract.id().bytes());
    hasher.digest(work.bytes());
    hasher.u64(support.latest_cursors().len() as u64);
    for cursor in support.latest_cursors() {
        hasher.digest(cursor.cell_id().bytes());
        hasher.digest(cursor.id().bytes());
    }
    match classified_chunk_accumulator {
        Some(accumulator) => {
            hasher.tag(0x01);
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
        hasher.digest(receipt.frontier_root.bytes());
    }
    RelationalCheckpointRoot(hasher.finish())
}

fn journal_entry_head(
    contract_id: RelationalJournalId,
    sequence: u64,
    previous: RelationalJournalHead,
    event: &RelationalJournalEvent,
) -> RelationalJournalHead {
    let mut hasher = ChainHasher::new(JOURNAL_ENTRY_HASH_V16);
    hasher.digest(contract_id.bytes());
    hasher.u64(sequence);
    hasher.digest(previous.bytes());
    hasher.digest(journal_event_digest(event));
    RelationalJournalHead(hasher.finish())
}

fn journal_event_digest(event: &RelationalJournalEvent) -> [u8; 32] {
    let mut hasher = ChainHasher::new(JOURNAL_EVENT_HASH_V15);
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
        RelationalEvidenceEvent::QuestionClassified { case_id, decision } => {
            hasher.tag(0x08);
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
    hasher.digest(artifact.question_id().bytes());
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
    hasher.digest(artifact.question_id().bytes());
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
    hasher.u128(artifact.admitted_not_selected_count());
    hasher.u128(artifact.admitted_selected_count());
    hasher.u64(artifact.runs().len() as u64);
    for run in artifact.runs() {
        hasher.digest(run.id().bytes());
        hasher.u32(u32::from(run.ordinal()));
        hasher.digest(run.cell_id().bytes());
        hasher.u128(run.interval_start());
        hasher.u128(run.interval_end_exclusive());
        hasher.tag(match run.outcome() {
            RelationalClassifiedCaseOutcome::Rejected => 0x01,
            RelationalClassifiedCaseOutcome::AdmittedNotSelected => 0x02,
            RelationalClassifiedCaseOutcome::AdmittedSelected => 0x03,
        });
    }
    match artifact.partition_id() {
        Some(partition_id) => {
            hasher.tag(0x01);
            hasher.digest(partition_id.bytes());
        }
        None => hasher.tag(0x02),
    }
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
    hasher.digest(artifact.question_id().bytes());
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

fn hash_checkpoint_event(hasher: &mut ChainHasher, event: &RelationalCheckpointEvent) {
    match event {
        RelationalCheckpointEvent::RelationalClassifiedChunkSliceCheckpointed { artifact } => {
            hasher.tag(0x0d);
            hasher.digest(artifact.id().bytes());
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
    use crate::explore::{
        ExploreValue, RelationalBoundValue, RelationalCaseExecutor, RelationalExpressionRuntime,
        RelationalSourceEnumerator, RelationalSuccessorAdvance, RelationalSupportPlanner,
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

    #[test]
    fn replay_rebuilds_the_same_interleaved_frontier_and_authenticated_exhaustion() {
        let source = r#"
? explore journal_fixture {
    from {
        before = 199999
        context = 1
    }
    to after = 200000
    find all
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
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact source support");
        let contract = RelationalJournalContract::new(
            checked.relation_id(),
            checked.admission_id(),
            checked.question_id(),
            Sha256::digest(b"journal-test-analysis").into(),
        );
        let mut journal = RelationalJournal::new(contract);
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
                question_id: contract.question_id(),
                case_id,
            },
            [case_ready_id, admission_work_id],
        )
        .unwrap();
        let find_work_id = find_work.work_node_id().unwrap();
        journal.append(find_work).unwrap();
        journal
            .append(RelationalJournalEvent::question_classified(
                case_id,
                SelectionDecision::Selected,
            ))
            .unwrap();
        journal
            .append(RelationalJournalEvent::work_node_completed(
                find_work_id,
                WorkCompletionRef::FindDecided {
                    question_id: contract.question_id(),
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
            open.question().selected(),
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
            closed_snapshot.question().selected(),
            RelationCountEvidence::Exact(1)
        );
        assert!(closed_snapshot
            .work()
            .nodes
            .iter()
            .all(|node| node.progress.is_complete()));

        let entries = journal.entries().to_vec();
        let replayed = RelationalJournal::replay(contract, entries.clone()).unwrap();
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
        assert_eq!(replayed_snapshot.question(), closed_snapshot.question());
        assert_eq!(
            replayed_snapshot.core_evidence_root(),
            closed_snapshot.core_evidence_root()
        );

        let mut tampered = entries;
        tampered[0].head = RelationalJournalHead([0; 32]);
        assert!(matches!(
            RelationalJournal::replay(contract, tampered),
            Err(RelationalJournalError::EntryHeadMismatch { sequence: 0 })
        ));
    }
}
