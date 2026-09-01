//! Closed relational elaboration for bounded `? explore` declarations.
//!
//! The parser and type checker deliberately retain source expressions.  This
//! pass is the trust boundary that proves each dependent source and successor
//! domain finite, deterministic, and exact before an executor may see it.

use super::*;
use std::path::PathBuf;
use std::sync::Arc;

mod authenticated_treap;
#[cfg(any())]
mod boundary_plan;
#[cfg(any())]
mod boundary_search;
mod case_graph;
#[cfg(any())]
mod certified_region;
#[cfg(any())]
mod classification_regions;
#[cfg(any())]
mod exact;
#[cfg(any())]
mod exact_stream;
mod mechanism;
pub(crate) use mechanism::MechanismObservationIr;
mod mechanism_incidence;
pub(crate) use mechanism_incidence::{
    ClosedMechanismIncidence, MechanismCaseTerminal, MechanismCaseTerminalRecord,
    MechanismCountEvidence, MechanismIncidenceCatalogBuilder, MechanismIncidenceCounts,
    MechanismIncidenceError, MechanismIncidenceInsert, MechanismIncidenceRoot,
    MechanismIncidenceSnapshot, MechanismRequestScope, MechanismSignatureDefinition,
    MechanismSignatureId, MechanismTargetCaseSetRoot, MechanismTargetDiscoveryRevision,
    MechanismTargetSeal, MechanismTargetSealId, MechanismTargetSealUpstream,
    MechanismTerminalDiscoveryRevision, MechanismUnavailableReasonId,
    MECHANISM_TARGET_SEAL_VERSION,
};
mod mechanism_support;
pub(crate) use mechanism_support::{
    MechanismClosedStarterProjectionAuthority, MechanismClosedSubjectStarterProjectionAuthority,
    MechanismStarterProjectionPlanId, MechanismStarterUpperProvenance,
    MechanismSupportCatalogBuilder, MechanismSupportClosureReceipt, MechanismSupportClosureRoot,
    MechanismSupportCount, MechanismSupportError, MechanismSupportFacet,
    MechanismSupportFiberExprRoot, MechanismSupportFrontierRoot, MechanismSupportKey,
    MechanismSupportResidualRoot, MechanismSupportStarterCursor, MechanismSupportStarterMember,
    MechanismSupportStarterPage, MechanismSupportSubject, MechanismSupportSubjectStarterPage,
    MechanismSupportView, MechanismSupportViewRoot, MECHANISM_SUPPORT_VERSION,
    MECHANISM_SUPPORT_VIEW_VERSION,
};
mod relational_mechanism_starter_authorization;
pub(crate) use relational_mechanism_starter_authorization::{
    find_relational_mechanism_starter_value_authorization,
    relational_mechanism_starter_value_authorization_for_view,
    RelationalMechanismStarterAuthorizationError, RelationalMechanismStarterAuthorizedProjection,
    RelationalMechanismStarterValueAuthorization, RelationalMechanismStarterValueAuthorizationId,
    RelationalMechanismStarterValueRole, RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION,
};
mod relational_mechanism_starter_projection;
pub(crate) use relational_mechanism_starter_projection::{
    RelationalMechanismStarterProjectionAccumulator, RelationalMechanismStarterProjectionClosure,
    RelationalMechanismStarterProjectionClosureRoot,
    RelationalMechanismStarterProjectionContentRoot, RelationalMechanismStarterProjectionError,
    RelationalMechanismStarterProjectionJob, RelationalMechanismStarterProjectionJobId,
    RelationalMechanismStarterProjectionMember, RelationalMechanismStarterProjectionMemberId,
    RelationalMechanismStarterProjectionPage, RelationalMechanismStarterProjectionPageId,
    RelationalMechanismStarterProjectionPageManifestRoot,
    RelationalMechanismStarterProjectionPageRoot, RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION,
};
mod structural_mechanism;
pub(crate) use structural_mechanism::{
    ExecutionProfileId, StructuralActivationCallCount, StructuralActivationContextDefinition,
    StructuralActivationContextId, StructuralCatalogMembershipRoot, StructuralCatalogRevision,
    StructuralContextCount, StructuralDefinitionCatalogRoot, StructuralDefinitionKind,
    StructuralDefinitionRef, StructuralEdgeCount, StructuralEdgeDefinition, StructuralEdgeId,
    StructuralEndpointContext, StructuralEndpointExecutionTotals, StructuralExecutionProfile,
    StructuralExpectedSignatureSetRoot, StructuralFrameCount, StructuralFrameDefinition,
    StructuralFrameId, StructuralMechanismCatalogBuilder, StructuralMechanismDefinition,
    StructuralMechanismError, StructuralMechanismId, StructuralMembershipRoot, StructuralNodeCount,
    StructuralNodeDefinition, StructuralNodeId, StructuralNodeOwnership, StructuralOwnershipCount,
    StructuralQuotientClosureReceipt, StructuralQuotientClosureRoot, StructuralQuotientCounts,
    StructuralSignatureAssignment, StructuralSignatureQuotientArtifact,
    StructuralSignatureToQuotientRoot, STRUCTURAL_DEFINITION_CATALOG_VERSION,
    STRUCTURAL_MECHANISM_QUOTIENT_VERSION, STRUCTURAL_QUOTIENT_CLOSURE_VERSION,
};
mod relational_mechanism_executor;
pub(crate) use relational_mechanism_executor::{
    derive_relational_structural_mechanism_v1, replay_relational_mechanism_case,
    RelationalIfDecisionOutcome, RelationalMechanismActivationStep, RelationalMechanismCalleeId,
    RelationalMechanismEndpoint, RelationalMechanismEndpointReplayProgress,
    RelationalMechanismEndpointReplayRequest, RelationalMechanismEndpointTraceEvidence,
    RelationalMechanismEndpointTraceProposal, RelationalMechanismEndpointTraceRoot,
    RelationalMechanismEventKind, RelationalMechanismEventOutcome,
    RelationalMechanismOccurrenceProposal, RelationalMechanismOccurrenceSlot,
    RelationalMechanismPermanentUnavailable, RelationalMechanismReplayError,
    RelationalMechanismReplayEvidence, RelationalMechanismReplayObservationId,
    RelationalMechanismReplayOutcome, RelationalMechanismReplayPause,
    RelationalMechanismReplayReceipt, RelationalMechanismReplayReceiptId,
    RelationalMechanismReplayRunError, RelationalMechanismReplayRuntime, RelationalMechanismSiteId,
    RelationalMechanismSiteKind, RelationalMechanismUnavailableEvidence,
    RelationalRuleAttemptOutcome, RelationalRuleSelectionOutcome, RelationalShortCircuitOutcome,
    RelationalStructuralMechanismError, RELATIONAL_MECHANISM_REPLAY_ABI_VERSION,
};
mod relational_interpreter_mechanism;
pub(crate) use relational_interpreter_mechanism::{
    RelationalInterpreterMechanismReplayError, RelationalInterpreterMechanismReplayRuntime,
};
#[cfg(any())]
mod mechanism_request;
mod relational_mechanism_step_driver;
#[cfg(any())]
pub(crate) use mechanism_request::{
    build_checked_mechanism_request_v1, MechanismObservationSelectionV1,
};
#[cfg(any())]
mod mechanism_snapshot;
#[cfg(any())]
mod mechanism_stream;
#[cfg(any())]
mod probe;
#[cfg(any())]
mod probe_codec;
#[cfg(any())]
mod probe_io;
#[cfg(any())]
mod probe_runner;
mod relation;
pub(crate) use relation::{
    AdmissionCatalog, AdmissionCatalogBuilder, AdmissionContentRoot, AdmissionCounts,
    AdmissionDecision, AdmissionFrontierRoot, AdmissionId, CatalogSource, CatalogSuccessor,
    FindPolarity, MechanismRequestId, MechanismTargetId, QuestionCatalog, QuestionCatalogBuilder,
    QuestionContentRoot, QuestionFrontierRoot, QuestionId, RelationCatalog, RelationCatalogBuilder,
    RelationCatalogError, RelationCatalogSnapshot, RelationClassificationError,
    RelationContentRoot, RelationCountEvidence, RelationEnumerationCounts, RelationFrontierRoot,
    RelationId, RelationLineageId, RelationProvenance, RelationSupportId, RelationalCaseId,
    RelationalCaseRef, SelectionCounts, SelectionDecision, SourceKey, SourceRow, SuccessorKey,
    SuccessorRow, ViewId, ViewInputId,
};
mod relational_ir;
pub(crate) use relational_ir::relational_tys_equivalent;
pub use relational_ir::{
    ExploreAdmissionIr, ExploreAggregateFieldIr, ExploreAggregateReducerIr, ExploreAnalysisNodeIr,
    ExploreFindIr, ExploreFiniteDomainIr, ExploreMechanismRequestIr, ExploreMechanismTargetIr,
    ExploreParetoObjectiveIr, ExploreQueryIr, ExploreResultChoiceIr, ExploreResultFieldIr,
    ExploreResultGrainIr, ExploreResultHavingIr, ExploreResultInputIr, ExploreResultViewIr,
    ExploreSourceBindingIr, ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr,
    ExploreSourceDependencyIr, ExploreSourceRelationIr, ExploreSuccessorKindIr,
    ExploreSuccessorRelationIr, EXPLORE_RELATIONAL_IR_VERSION,
};
pub(crate) use relational_ir::{
    ExploreStarterProjectionFacetIr, ExploreStarterProjectionIr, ExploreStarterProjectionSubjectIr,
};
mod relational_analysis_plan;
pub(crate) use relational_analysis_plan::{
    RelationalAnalysisDependencyId, RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration,
    RelationalAnalysisPlan, RelationalAnalysisPlanError, RelationalAnalysisPlanRoot,
    RelationalCheckedAnalysisGraphDigest, RelationalMechanismLayerRegistration,
    RelationalMechanismObservationDigest, RelationalMechanismObservationId,
    RelationalResolvedMechanismTarget, RelationalResolvedResultInput,
    RelationalResultLayerRegistration, RelationalResultSpecDigest,
    RELATIONAL_ANALYSIS_PLAN_VERSION,
};
mod relational_analysis_catalog;
pub(crate) use relational_analysis_catalog::{
    ClosedRelationalAnalysisCatalog, RelationalAnalysisCatalogBuilder,
    RelationalAnalysisCatalogError, RelationalAnalysisCatalogRoot,
    RelationalAnalysisCatalogSnapshot, RelationalAnalysisLayerSnapshot,
    RelationalAnalysisLayerStatus, RelationalMechanismLayerSnapshot, RelationalResultLayerSnapshot,
    RelationalResultLayerSnapshotState, RelationalResultPublication, RelationalResultPublicationId,
    RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION, RELATIONAL_RESULT_PUBLICATION_VERSION,
};
mod relational_analysis_journal;
pub(crate) use relational_analysis_journal::{
    RelationalAnalysisEvidenceEvent, RelationalAnalysisEvidenceEventDigest,
    RelationalAnalysisJournalApply, RelationalAnalysisJournalError,
    RelationalAnalysisJournalScopeRoot, RelationalAnalysisJournalState,
    RelationalSelectedPopulationAuthority, RelationalSelectedQuestionSeal,
    RelationalSelectedQuestionSealId, RELATIONAL_ANALYSIS_EVENT_SCHEMA_VERSION,
    RELATIONAL_SELECTED_QUESTION_SEAL_VERSION,
};
mod relational_frontier;
pub(crate) use relational_frontier::{
    CanonicalSourcePrefix, MechanismEndpoint, RelationalWorkFrontier, WorkCompletionRef,
    WorkCursor, WorkEvidenceId, WorkFrontierCompaction, WorkFrontierError, WorkFrontierRoot,
    WorkFrontierSnapshot, WorkNodeId, WorkNodeProgress, WorkNodeSnapshot, WorkNodeSpec,
    RELATIONAL_FRONTIER_SNAPSHOT_VERSION, WORK_FRONTIER_MAX_COMPACTION_NODES,
};
mod relational_case_executor;
pub(crate) use relational_case_executor::{
    RelationalCaseClassification, RelationalCaseExecutor, RelationalCaseExecutorError,
    RelationalConcreteCase, RelationalSuccessorAdvance, RelationalSuccessorCursor,
    RelationalSuccessorCursorSnapshot, RelationalSuccessorFiber, SuccessorFiberExhaustionReceipt,
    SuccessorFiberExhaustionReceiptId, RELATIONAL_SUCCESSOR_CURSOR_VERSION,
};
mod relational_bounded_chunk_partition;
pub(crate) use relational_bounded_chunk_partition::{
    decode_relational_case_chunk_finite_ordinals, derive_relational_case_chunk_subinterval_cell,
    plan_relational_bounded_case_chunks, reverify_relational_case_chunk_partition_artifact,
    RelationalCaseChunk, RelationalCaseChunkDescriptor, RelationalCaseChunkId,
    RelationalCaseChunkInjectivityBinding, RelationalCaseChunkPartition,
    RelationalCaseChunkPartitionArtifact, RelationalCaseChunkPartitionArtifactId,
    RelationalCaseChunkPartitionError, RelationalCaseChunkPlanningOutcome,
    RelationalCaseChunkShape, RelationalCaseChunkUnsupported, VerifiedRelationalCaseChunkPartition,
    RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1, RELATIONAL_CASE_CHUNK_PARTITION_VERSION,
};
mod relational_classified_sweep;
pub(crate) use relational_classified_sweep::{
    classify_relational_case_chunk, classify_relational_case_chunk_slice,
    finalize_relational_classified_case_chunk, reverify_relational_classified_chunk_artifact,
    reverify_relational_classified_chunk_slice_artifact, RelationalClassifiedCaseOutcome,
    RelationalClassifiedChunk, RelationalClassifiedChunkAccumulator,
    RelationalClassifiedChunkArtifact, RelationalClassifiedChunkArtifactId,
    RelationalClassifiedChunkSlice, RelationalClassifiedChunkSliceArtifact,
    RelationalClassifiedChunkSliceId, RelationalClassifiedChunkSliceRun,
    RelationalClassifiedChunkTranscriptRoot, RelationalClassifiedEvidenceBinding,
    RelationalClassifiedRun, RelationalClassifiedRunDescriptor,
    RelationalClassifiedRunEvidenceBindings, RelationalClassifiedRunId,
    RelationalClassifiedSweepError, VerifiedRelationalClassifiedChunk,
    RELATIONAL_CLASSIFIED_CHUNK_SLICE_VERSION, RELATIONAL_CLASSIFIED_CHUNK_VERSION,
};
pub(crate) mod relational_classification_capsule;
mod relational_classification_evaluator;
mod relational_native_classifier;
pub use relational_native_classifier::RelationalNativeClassifierProtocolV2;
pub(crate) use relational_native_classifier::{
    RelationalNativeClassifierFallbackBackendV2, RelationalNativeClassifierUnavailable,
    RelationalNativeClassifierV2,
};
mod relational_case_support_projection;
mod relational_case_transition_projection;
mod relational_selected_run_materialization;
pub(crate) use relational_selected_run_materialization::{
    materialize_relational_selected_run, reverify_relational_selected_run_materialization_artifact,
    RelationalSelectedCaseRecord, RelationalSelectedRunMaterialization,
    RelationalSelectedRunMaterializationArtifact, RelationalSelectedRunMaterializationArtifactId,
    RelationalSelectedRunMaterializationError, VerifiedRelationalSelectedRunMaterialization,
    RELATIONAL_SELECTED_RUN_MATERIALIZATION_VERSION,
};
mod relational_selected_run_step_driver;
pub(crate) use relational_selected_run_step_driver::{
    RelationalSelectedRunStepBatch, RelationalSelectedRunStepDriver,
    RelationalSelectedRunStepDriverError, RelationalSelectedRunStepOutcome,
    RelationalSelectedRunStepQuantum,
};
mod relational_classified_sweep_step_driver;
pub(crate) use relational_classified_sweep_step_driver::{
    RelationalClassifiedSweepStepBatch, RelationalClassifiedSweepStepDriver,
    RelationalClassifiedSweepStepDriverError, RelationalClassifiedSweepStepOutcome,
    RelationalClassifiedSweepStepQuantum,
};
mod relational_executor;
pub(crate) use relational_executor::{
    RelationalBindingSelection, RelationalBoundValue, RelationalCompletedSource,
    RelationalExpressionRuntime, RelationalFiberMember, RelationalFiniteFiber,
    RelationalSourceAdvance, RelationalSourceContinuation, RelationalSourceCursor,
    RelationalSourceCursorSnapshot, RelationalSourceEnumerator, RelationalSourceExecutorError,
    RelationalSourcePrefixSnapshot, SourceBindingExhaustionReceipt,
    SourceBindingExhaustionReceiptId, RELATIONAL_SOURCE_CURSOR_VERSION,
};
mod relational_source_closure;
pub(crate) use relational_source_closure::{
    PreparedSourceTraversalObservation, SourceRelationExhaustionReceipt,
    SourceRelationExhaustionReceiptId, SourceTraversalAccumulator, SourceTraversalAdvanceId,
    SourceTraversalClosureError, SourceTraversalEdgeId, SourceTraversalEdgeRoot,
    SourceTraversalFrontierRoot, SourceTraversalObservation,
    RELATIONAL_SOURCE_CLOSURE_PRODUCER_ABI_VERSION, RELATIONAL_SOURCE_CLOSURE_SCHEMA_VERSION,
};
mod relational_certified_source_summary;
mod relational_source_image_exactness;
mod relational_support_planner;
mod relational_uniform_admission_proof;
pub(crate) use relational_certified_source_summary::{
    certify_relational_source_summary, RelationalCertifiedSourceSummaryArtifact,
    RelationalCertifiedSourceSummaryArtifactId, RelationalCertifiedSourceSummaryCertification,
    RelationalCertifiedSourceSummaryError, RelationalCertifiedSourceSummaryUnsupported,
    VerifiedRelationalCertifiedSourceSummary, RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION,
};
pub(crate) use relational_source_image_exactness::{
    prove_relational_source_image_exactness, reverify_relational_source_image_exactness_artifact,
    CertifiedSourcePopulationBinding, CertifiedSourcePopulationRoot,
    RelationalSourceImageCardinalityEvidenceBinding, RelationalSourceImageEvidenceBinding,
    RelationalSourceImageExactnessProof, RelationalSourceImageExactnessProofArtifact,
    RelationalSourceImageExactnessProofError, RelationalSourceImageInjectivityEvidenceBinding,
    VerifiedRelationalSourceImageExactnessProof, RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION,
};
pub(crate) use relational_support_planner::{
    RelationalBindingStage, RelationalBindingStageId, RelationalCaseImageInjectivityProofArtifact,
    RelationalCaseImageInjectivityProofError, RelationalCoverageQualifier,
    RelationalCoverageStatus, RelationalDependencyKeyRecipe, RelationalDimensionId,
    RelationalExactEmptyReason, RelationalFiniteDomainRecipeKind, RelationalFiniteFactorRecipe,
    RelationalFiniteFactorStage, RelationalObligationActivation, RelationalPlannedPopulation,
    RelationalPlannedSupport, RelationalRootObligationPlan, RelationalSingletonMapStage,
    RelationalStagedObligationDescriptor, RelationalSuccessorRecipeKind,
    RelationalSupportCellCatalog, RelationalSupportExactness, RelationalSupportOpenReason,
    RelationalSupportPlan, RelationalSupportPlanRoot, RelationalSupportPlanner,
    RelationalSupportPlannerError, RelationalSupportPopulationKind,
    RelationalSupportPopulationRecipe, RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION,
    RELATIONAL_SUPPORT_PLANNER_VERSION,
};
mod relational_proof_strategy;
mod relational_support_step_driver;
pub(crate) use relational_proof_strategy::{
    assess_relational_selected_support, RelationalAxisProofPlan, RelationalCheckedGuardAtom,
    RelationalChildObligationSet, RelationalExactSelectedSupport,
    RelationalExactSelectedSupportBasis, RelationalIntegerAxis, RelationalIntegerAxisSupportKind,
    RelationalIntervalCertificateKind, RelationalIntervalCertificateObligation,
    RelationalMonotonicityDirection, RelationalOrdinalInterval, RelationalProofStrategyError,
    RelationalProofStrategyInventory, RelationalSelectedLeafEvidence,
    RelationalSelectedSupportAssessment, RelationalSelectedSupportResidual,
    RelationalSplitCandidate, RelationalSplitOrigin, RelationalSplitPriority,
    RelationalStrategyResidualReason, RelationalStructuralAxisRefinement,
    RELATIONAL_PROOF_STRATEGY_VERSION,
};
pub(crate) use relational_support_step_driver::RelationalSupportStepQuantum;
mod relational_classified_population;
mod relational_population;
pub(crate) use relational_classified_population::{
    CertifiedRelationalClassificationCounts, CertifiedRelationalClassificationCountsError,
};
mod relational_region_proof;
pub(crate) use relational_population::{
    CertifiedSelectedFragment, CertifiedSelectedPopulationError, CertifiedSelectedPopulationRoot,
    CertifiedSelectedPopulationSnapshot, ClosedCertifiedSelectedPopulation,
    CERTIFIED_SELECTED_POPULATION_SNAPSHOT_VERSION,
};
mod relational_journal;
pub(crate) use relational_journal::{
    ClosedCertifiedRelationalCore, ClosedCertifiedRelationalEvidence,
    ClosedExtensionalRelationalEvidence, RelationalCheckpointEvent, RelationalCheckpointRoot,
    RelationalCoreEvidenceRoot, RelationalEvidenceEvent, RelationalExhaustionEvidenceRoot,
    RelationalExplorationEvidenceRoot, RelationalExtensionalContentRoot, RelationalJournal,
    RelationalJournalContract, RelationalJournalEntry, RelationalJournalError,
    RelationalJournalEvent, RelationalJournalHead, RelationalJournalId, RelationalJournalSnapshot,
    RelationalSchedulerView, RELATIONAL_JOURNAL_SCHEMA_VERSION,
};
mod relational_durable_journal;
mod relational_journal_codec;
mod relational_journal_store;
pub(crate) use relational_journal_store::{
    RawRelationalJournalFrame, RawRelationalJournalFrameIter, RawRelationalJournalSegment,
    RawRelationalJournalSegmentReplay, RelationalJournalSegmentAppend,
    RelationalJournalSegmentDigest, RelationalJournalSegmentLimits,
    RelationalJournalSegmentReceipt, RelationalJournalSegmentStore,
    RelationalJournalSegmentStoreError, RelationalJournalStoreAnchor,
    RelationalJournalStoreFinalized, RELATIONAL_JOURNAL_FRAME_HARD_MAX_BYTES,
    RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_BYTES, RELATIONAL_JOURNAL_SEGMENT_HARD_MAX_FRAMES,
    RELATIONAL_JOURNAL_SEGMENT_SCHEMA_VERSION, RELATIONAL_JOURNAL_STORE_HARD_MAX_SEGMENTS,
};
mod relational_step_driver;
pub(crate) use relational_step_driver::{
    RelationalConcreteQuiescence, RelationalStepBatch, RelationalStepDriver,
    RelationalStepDriverError, RelationalStepOutcome, RelationalStepQuantum,
};
mod relational_incidence_result_step_driver;
mod relational_public;
mod relational_result_publication;
mod relational_result_step_driver;
mod relational_stream_driver;
mod relational_stream_run_loop;
pub(crate) use relational_incidence_result_step_driver::{
    RelationalIncidenceResultStepBatch, RelationalIncidenceResultStepDriver,
    RelationalIncidenceResultStepDriverError, RelationalIncidenceResultStepOutcome,
    RelationalIncidenceResultStepQuantum, RelationalIncidenceResultStepQuiescence,
};
pub use relational_public::{
    execute_checked_relational_stream_slice, prepare_checked_relational_stream,
    ExploreNativeClassifierAdmissionV2, ExploreNativeClassifierFindV2,
    ExploreNativeClassifierIdentityV2, ExploreNativeClassifierPlanV2,
    ExploreNativeClassifierSourceBindingKindV2, ExploreNativeClassifierSourceBindingV2,
    ExploreStreamCheckpoint, ExploreStreamCount, ExploreStreamCoverageBindingRole,
    ExploreStreamCoverageClassification, ExploreStreamCoverageConstructorLayout,
    ExploreStreamCoverageEntry, ExploreStreamCoverageFieldPathSegment,
    ExploreStreamCoverageGapReason, ExploreStreamCoverageLiteralKind,
    ExploreStreamCoverageRootRole, ExploreStreamCoverageSubject, ExploreStreamEpochOptions,
    ExploreStreamGroupedResultPreview, ExploreStreamIdentity, ExploreStreamLayer,
    ExploreStreamLayerStatus, ExploreStreamLifecycle, ExploreStreamMechanismLayer,
    ExploreStreamMechanismSupportTotals, ExploreStreamMechanismTarget,
    ExploreStreamObserverMemoStats, ExploreStreamOuterContainment, ExploreStreamPauseReason,
    ExploreStreamPopulationCounts, ExploreStreamPreparationError, ExploreStreamPreviewLimit,
    ExploreStreamPreviewStatus, ExploreStreamProjectedValue, ExploreStreamPublication,
    ExploreStreamPublicationArtifact, ExploreStreamResultEvidence, ExploreStreamResultField,
    ExploreStreamResultGroupRow, ExploreStreamResultLayer, ExploreStreamSliceOptions,
    ExploreStreamSliceReport, ExploreStreamSourceCoverage, PreparedRelationalExplore,
    RelationalExploreEpoch, EXPLORE_RELATIONAL_STREAM_REPORT_VERSION,
};
pub(crate) use relational_result_step_driver::{
    RelationalResultStepBatch, RelationalResultStepDriver, RelationalResultStepDriverError,
    RelationalResultStepOutcome, RelationalResultStepQuantum, RelationalResultStepQuiescence,
};
mod result_projection;
pub(crate) use result_projection::{
    IndexedResultProjectionRecord, ResultProjectionCatalogBuilder, ResultProjectionClosure,
    ResultProjectionError, ResultProjectionGroup, ResultProjectionRecord, ResultProjectionRecordId,
    ResultProjectionRoot, ResultProjectionSnapshot, RESULT_PROJECTION_SNAPSHOT_VERSION,
};
mod result_view;
pub(crate) use result_view::{
    CertifiedResultInputRoot, ClosedResultView, EvaluatedResultContribution,
    MechanismIncidenceRowId, ResultClosedGroupRef, ResultClosedRowRef, ResultCountDistinctSnapshot,
    ResultGroupDisposition, ResultGroupKey, ResultGroupSnapshot, ResultOutputRow, ResultValue,
    ResultViewBuilder, ResultViewChoice, ResultViewCount, ResultViewCounts, ResultViewError,
    ResultViewFinishError, ResultViewGrain, ResultViewHaving, ResultViewInputKind,
    ResultViewInputRowId, ResultViewOutput, ResultViewProjectionError, ResultViewProjector,
    ResultViewRoot, ResultViewSnapshot, ResultViewSpec, ResultViewSpecRoot, ResultViewStatus,
};
mod result_evidence;
pub(crate) use result_evidence::{
    RelationalResultEvidenceCatalog, RelationalResultEvidenceCatalogBuilder,
    RelationalResultEvidenceId, RelationalResultEvidenceRecord, RelationalResultEvidenceRoot,
    RelationalResultEvidenceSnapshot, RelationalResultInputSeal, ResultEvidenceError,
    ResultEvidenceUpstreamRoot, ResultInputCoverageCommitment, ResultInputCoverageRoot,
    RELATIONAL_RESULT_EVIDENCE_SNAPSHOT_VERSION,
};
mod relational_result_executor;
pub(crate) use relational_result_executor::{
    RelationalResultBinding, RelationalResultEvidence, RelationalResultExecution,
    RelationalResultExecutor, RelationalResultExecutorError, RelationalResultExpressionRuntime,
};
mod support_cell;
pub(crate) use support_cell::{
    AdmissionClassificationClaim, CertifiedInjective, ExactCardinalityClaim, InjectiveMappingClaim,
    RetainedSupportExamples, SelectionClassificationClaim, SupportCardinality, SupportCell,
    SupportCellClaim, SupportCellError, SupportCellEvidence, SupportCellEvidenceId, SupportCellId,
    SupportCellObligation, SupportCellSpace, SupportExampleId, SupportExampleRetention,
    SupportExpr, SupportExprId, SupportExprKind, SupportExtensionalTarget,
    SupportMaterializationCursor, SupportMaterializationCursorId, SupportMaterializerId,
    SupportObserverId, SupportPartitionCertificate, SupportPartitionId, SupportPartitionKind,
    SupportPartitionObligation, SupportProducerId, SupportProofObligationId, SupportProofReceipt,
    SupportProofReceiptId, SupportProofVerifierId, UniformMechanismClaim, UniformValueClaim,
    SUPPORT_MATERIALIZATION_CURSOR_VERSION,
};
mod support_evidence;
pub(crate) use support_evidence::{
    SupportCursorInsert, SupportEvidenceCatalogBuilder, SupportEvidenceCount,
    SupportEvidenceCounts, SupportEvidenceError, SupportEvidenceKind, SupportEvidenceRecord,
    SupportEvidenceRoot, SupportEvidenceSnapshot, SupportLayerReference, SupportObligationRecord,
    SupportObligationRefinement, SupportObligationRefinementId, SupportObserverLayerScope,
    SupportPresentationCounts, SupportReferenceKind, SupportRetainedExampleInsert,
    SupportRetainedExamplesSnapshot, SUPPORT_EVIDENCE_SNAPSHOT_VERSION,
};
mod support_journal;
pub(crate) use support_journal::{
    SupportJournalApply, SupportJournalError, SupportJournalEvent, SupportJournalEventDigest,
    SUPPORT_JOURNAL_EVENT_SCHEMA_VERSION,
};
mod report;
mod resource_governor;
mod resource_sampler;
#[cfg(any())]
mod run_state;
mod run_store;
mod run_stream;
#[cfg(any())]
mod run_stream_codec;
#[cfg(any())]
mod run_stream_store;
#[cfg(any())]
mod source_events;
#[cfg(any())]
mod source_proof_plan;
#[cfg(any())]
mod stream_coordinator;
#[cfg(any())]
mod stream_identity;
#[cfg(any())]
mod stream_probe;
#[cfg(any())]
mod stream_proof;
#[cfg(any())]
mod stream_replay;
mod stream_resource;
#[cfg(any())]
mod stream_snapshot;
mod transition;

pub(crate) use transition::TransitionSchemaIdentities;

const EXPLORE_GROUND_COLLECTION_LIMIT: u64 = 1_000_000;
const EXPLORE_GROUND_WORK_LIMIT: u64 = 4_000_000;
const EXPLORE_FINITE_PLAN_WORK_LIMIT: usize = 100_000;
const EXPLORE_RECURSION_LIMIT: usize = 64;
const EXPLORE_GROUND_RECURSION_LIMIT: usize = 16;
const RELATIONAL_EXPRESSION_INITIAL_STEP_LIMIT: usize = 1_000_000;
const RELATIONAL_EXPRESSION_HARD_STEP_LIMIT: usize = 4_000_000;

/// Canonical first-order value used for domain identity, ordering, SMT
/// constants, and replay.  Floats use their exact IEEE bits rather than the
/// interpreter's approximate equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExploreValue {
    Int(i64),
    FloatBits(u64),
    String(String),
    Character(char),
    Boolean(bool),
    Unit,
    List(Vec<ExploreValue>),
    Set(Vec<ExploreValue>),
    Tuple(Vec<ExploreValue>),
    Constructor {
        type_name: String,
        variant: String,
        positional: bool,
        /// Immutable constructor payload shared by replay-folded evidence.
        ///
        /// `Arc` identity is purely operational: equality, ordering, hashing,
        /// and journal encoding continue to inspect the complete field slice.
        /// Large fixed Context values can therefore be cloned across relation,
        /// traversal, and result layers without cloning every field string and
        /// nested value.
        fields: Arc<[(String, ExploreValue)]>,
    },
}

fn runtime_value_from_explore_value(value: &ExploreValue) -> Value {
    match value {
        ExploreValue::Int(value) => Value::Int(*value),
        ExploreValue::FloatBits(bits) => Value::Float(f64::from_bits(*bits)),
        ExploreValue::String(value) => Value::Str(value.clone()),
        ExploreValue::Character(value) => Value::Char(*value),
        ExploreValue::Boolean(value) => Value::Bool(*value),
        ExploreValue::Unit => Value::Unit,
        ExploreValue::List(values) => values.iter().rev().fold(
            Value::Constructor("Nil".into(), vec![].into()),
            |tail, value| {
                Value::Constructor(
                    "Cons".into(),
                    vec![runtime_value_from_explore_value(value), tail].into(),
                )
            },
        ),
        ExploreValue::Set(values) => Value::Set(
            values
                .iter()
                .map(|value| {
                    (
                        value.runtime_display_key(),
                        runtime_value_from_explore_value(value),
                    )
                })
                .collect(),
        ),
        ExploreValue::Tuple(values) => Value::Tuple(
            values
                .iter()
                .map(runtime_value_from_explore_value)
                .collect(),
        ),
        ExploreValue::Constructor {
            variant,
            positional: true,
            fields,
            ..
        } => Value::Constructor(
            variant.clone(),
            fields
                .iter()
                .map(|(_, value)| runtime_value_from_explore_value(value))
                .collect::<Vec<_>>()
                .into(),
        ),
        ExploreValue::Constructor {
            variant,
            positional: false,
            fields,
            ..
        } => Value::NamedConstructor(
            variant.clone(),
            fields
                .iter()
                .map(|(name, value)| (name.clone(), runtime_value_from_explore_value(value)))
                .collect::<Vec<_>>()
                .into(),
        ),
    }
}

impl ExploreValue {
    pub fn int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    fn runtime_display_key(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::FloatBits(bits) => f64::from_bits(*bits).to_string(),
            Self::String(value) => value.clone(),
            Self::Character(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Unit => "()".to_string(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Set(values) => format!(
                "{{{}}}",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Tuple(values) => format!(
                "({})",
                values
                    .iter()
                    .map(Self::runtime_display_key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } if variant == "Nil" && fields.is_empty() => "[]".to_string(),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } if variant == "Cons" && fields.len() == 2 => {
                let mut items = vec![&fields[0].1];
                let mut tail = &fields[1].1;
                while let Self::Constructor {
                    variant,
                    positional: true,
                    fields,
                    ..
                } = tail
                {
                    if variant != "Cons" || fields.len() != 2 {
                        break;
                    }
                    items.push(&fields[0].1);
                    tail = &fields[1].1;
                }
                format!(
                    "[{}]",
                    items
                        .into_iter()
                        .map(Self::runtime_display_key)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Constructor {
                variant,
                positional: _,
                fields,
                ..
            } if fields.is_empty() => variant.clone(),
            Self::Constructor {
                variant,
                positional: true,
                fields,
                ..
            } => format!(
                "{}({})",
                variant,
                fields
                    .iter()
                    .map(|(_, value)| value.runtime_display_key())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Constructor {
                variant,
                positional: false,
                fields,
                ..
            } => format!(
                "{}({})",
                variant,
                fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value.runtime_display_key()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

fn explore_value_node_count(value: &ExploreValue, cap: u64) -> u64 {
    let exceeded = cap.saturating_add(1);
    let mut count = 0_u64;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        count = count.saturating_add(1);
        if count > cap {
            return exceeded;
        }
        match value {
            ExploreValue::List(values)
            | ExploreValue::Set(values)
            | ExploreValue::Tuple(values) => stack.extend(values),
            ExploreValue::Constructor { fields, .. } => {
                stack.extend(fields.iter().map(|(_, value)| value));
            }
            _ => {}
        }
    }
    count
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreCardinality {
    Exact(u128),
    ExceedsU128,
}

impl ExploreCardinality {
    fn zero() -> Self {
        Self::Exact(0)
    }

    fn one() -> Self {
        Self::Exact(1)
    }

    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_add(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }

    fn multiply(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(0), _) | (_, Self::Exact(0)) => Self::zero(),
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_mul(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }

    pub fn exact(&self) -> Option<u128> {
        match self {
            Self::Exact(value) => Some(*value),
            Self::ExceedsU128 => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExploreEnumeratedSource {
    ExplicitList,
    NamedList { name: String },
    NamedSet { name: String },
}

#[derive(Debug, Clone)]
pub struct ExploreFiniteFieldPlan {
    pub name: String,
    pub plan: ExploreFiniteTypePlan,
}

#[derive(Debug, Clone)]
pub struct ExploreFiniteVariantPlan {
    pub name: String,
    pub positional: bool,
    pub fields: Vec<ExploreFiniteFieldPlan>,
}

/// A lazy, exact description of every inhabitant of a finite declared type.
/// It avoids allocating a large Cartesian product during type checking.
#[derive(Debug, Clone)]
pub enum ExploreFiniteTypePlan {
    Unit,
    Bool,
    Tuple {
        elements: Vec<ExploreFiniteTypePlan>,
        cardinality: ExploreCardinality,
    },
    Sum {
        type_name: String,
        variants: Vec<ExploreFiniteVariantPlan>,
        cardinality: ExploreCardinality,
    },
}

impl ExploreFiniteTypePlan {
    pub fn cardinality(&self) -> ExploreCardinality {
        match self {
            Self::Unit => ExploreCardinality::one(),
            Self::Bool => ExploreCardinality::Exact(2),
            Self::Tuple { cardinality, .. } => cardinality.clone(),
            Self::Sum { cardinality, .. } => cardinality.clone(),
        }
    }

    /// Materialize a small finite type for diagnostics/tests/replay.  The
    /// universe itself remains lazy and exact when the limit is exceeded.
    pub fn enumerate(&self, limit: usize) -> Result<Vec<ExploreValue>, String> {
        let count = self
            .cardinality()
            .exact()
            .ok_or_else(|| "finite type has more than u128::MAX inhabitants".to_string())?;
        if count > limit as u128 {
            return Err(format!(
                "finite type has {} inhabitants, exceeding materialization limit {}",
                count, limit
            ));
        }
        self.enumerate_unchecked()
    }

    fn enumerate_unchecked(&self) -> Result<Vec<ExploreValue>, String> {
        match self {
            Self::Unit => Ok(vec![ExploreValue::Unit]),
            Self::Bool => Ok(vec![
                ExploreValue::Boolean(false),
                ExploreValue::Boolean(true),
            ]),
            Self::Tuple { elements, .. } => {
                let mut combinations = vec![Vec::new()];
                for element in elements {
                    let element_values = element.enumerate_unchecked()?;
                    let mut next = Vec::new();
                    for prefix in combinations {
                        for value in &element_values {
                            let mut combined = prefix.clone();
                            combined.push(value.clone());
                            next.push(combined);
                        }
                    }
                    combinations = next;
                }
                Ok(combinations.into_iter().map(ExploreValue::Tuple).collect())
            }
            Self::Sum {
                type_name,
                variants,
                ..
            } => {
                let mut values = Vec::new();
                for variant in variants {
                    let mut combinations = vec![Vec::<(String, ExploreValue)>::new()];
                    for field in &variant.fields {
                        let field_values = field.plan.enumerate_unchecked()?;
                        let mut next = Vec::new();
                        for prefix in combinations {
                            for value in &field_values {
                                let mut combined = prefix.clone();
                                combined.push((field.name.clone(), value.clone()));
                                next.push(combined);
                            }
                        }
                        combinations = next;
                    }
                    for fields in combinations {
                        values.push(ExploreValue::Constructor {
                            type_name: type_name.clone(),
                            variant: variant.name.clone(),
                            positional: variant.positional,
                            fields: fields.into(),
                        });
                    }
                }
                Ok(values)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExploreExactDomain {
    Enumerated {
        values: Vec<ExploreValue>,
        source: ExploreEnumeratedSource,
    },
    IntRange {
        start: i64,
        end_exclusive: i64,
        cardinality: u64,
    },
    FiniteType {
        ty: Ty,
        plan: ExploreFiniteTypePlan,
    },
}

impl ExploreExactDomain {
    pub fn cardinality(&self) -> ExploreCardinality {
        match self {
            Self::Enumerated { values, .. } => ExploreCardinality::Exact(values.len() as u128),
            Self::IntRange { cardinality, .. } => ExploreCardinality::Exact(*cardinality as u128),
            Self::FiniteType { plan, .. } => plan.cardinality(),
        }
    }

    /// Materialize a deliberately small exact domain for the exhaustive
    /// developer preview. Solver-backed exploration keeps ranges and finite
    /// plans lazy; this path refuses to cross its explicit case cap.
    pub fn enumerate_preview(&self, limit: usize) -> Result<Vec<ExploreValue>, String> {
        let count = self
            .cardinality()
            .exact()
            .ok_or_else(|| "exploration domain has more than u128::MAX values".to_string())?;
        if count > limit as u128 {
            return Err(format!(
                "exploration domain has {} values, exceeding preview limit {}",
                count, limit
            ));
        }
        match self {
            Self::Enumerated { values, .. } => Ok(values.clone()),
            Self::IntRange {
                start, cardinality, ..
            } => Ok((0..*cardinality)
                .map(|offset| ExploreValue::Int((*start as i128 + offset as i128) as i64))
                .collect()),
            Self::FiniteType { plan, .. } => plan.enumerate(limit),
        }
    }
}

// Retained only as source-local migration history while the relational IR
// replaces the Cartesian compiler contract. Keeping the dead definitions out
// of name resolution is important: the relational language no longer owns the
// old axis-role, endpoint-product, or sliced-input types.
#[cfg(any())]
mod retired_cartesian_ir {
    use super::*;

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreDimensionIr {
        /// Source-independent link back to the normalized typed bound that owns
        /// this generator axis. Product construction uses this identity rather
        /// than presentation names, which may repeat across transition roles.
        pub bound_index: usize,
        pub name: String,
        pub value_ty: Ty,
        pub domain: ExploreExactDomain,
        pub role: ExploreGeneratorAxisRole,
        pub role_field_index: usize,
        pub span: Span,
    }

    #[derive(Debug, Clone)]
    pub(crate) enum ExploreFactValue {
        Fixed(ExploreValue),
        Derived {
            expression: Expr,
            dependencies: BTreeSet<String>,
        },
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreFactIr {
        /// Normalized typed-bound identity used when materializing State/Context
        /// products. It is not inferred from the fact's display name.
        pub bound_index: usize,
        pub role: ExploreGeneratorAxisRole,
        pub role_field_index: usize,
        pub name: String,
        pub value_ty: Ty,
        pub value: ExploreFactValue,
        pub span: Span,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreConstraintIr {
        pub predicate: Expr,
        pub scope: ExploreConstraintScope,
        pub span: Span,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreBoundaryIr {
        pub axis: String,
        pub axis_dimension_index: usize,
        pub step: i64,
        /// Both the before value and checked `before + step` value must be members
        /// of the declared axis domain.
        pub requires_both_endpoints_in_domain: bool,
        /// Source-order derived facts whose transitive dependencies include the
        /// axis.  They are recomputed after substituting the upper endpoint.
        pub recomputed_fact_indices: Vec<usize>,
        pub eligible_axis_pairs: ExploreCardinality,
        pub eligible_unconstrained_pairs: ExploreCardinality,
        pub span: Span,
    }

    #[derive(Debug, Clone)]
    pub(crate) enum ExploreAfterFieldSourceIr {
        FrameBefore {
            before_field_index: usize,
        },
        Derived {
            expression: Expr,
            environment: TypedExploreDerivedEnvironment,
            /// Canonical after-construction DAG predecessors. The evaluator exposes
            /// only these already-constructed fields through the runtime-only
            /// partial `after` product; the partial value never becomes a state.
            after_dependencies: Vec<ExploreAfterDependencyIr>,
        },
        /// One canonical generator coordinate supplies this field. The domain is
        /// owned by `ExploreUniverseIr::dimensions`; transition construction only
        /// retains the closed coordinate index.
        IndependentDomain {
            dimension_index: usize,
        },
    }

    /// One compiler-owned edge in the normalized after-construction DAG.
    /// `binding_name` is the checked State-field spelling used to validate the
    /// indexed edge and expose `after.FIELD`. Any compact bare alias is carried
    /// separately by `ExploreFlatAliasIr`; runtime construction never infers
    /// either relation from mutable environment contents or incidental field order.
    #[derive(Debug, Clone)]
    pub(crate) struct ExploreAfterDependencyIr {
        pub field_index: usize,
        pub binding_name: String,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreAfterFieldIr {
        pub field_index: usize,
        pub name: String,
        pub value_ty: Ty,
        pub source: ExploreAfterFieldSourceIr,
        pub span: Span,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExploreAfterMembershipPreconstructionIr {
        /// The checked after construction is `before + step` for this Int field.
        /// Membership can therefore close before any fallible derived evaluation.
        RelativeIntStep { step: i64 },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ExploreAfterMembershipIr {
        pub after_field_index: usize,
        pub before_dimension_index: usize,
        pub preconstruction: ExploreAfterMembershipPreconstructionIr,
    }

    #[derive(Debug, Clone)]
    pub(crate) enum ExploreProductFieldSourceIr {
        Dimension { dimension_index: usize },
        Fact { fact_index: usize },
        TransitionExpression { expression: Expr },
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreProductFieldIr {
        pub field_index: usize,
        pub name: String,
        pub value_ty: Ty,
        pub source: ExploreProductFieldSourceIr,
        pub span: Span,
    }

    /// A closed product schema: every field source already names a closed
    /// generator/fact slot or a checked transition expression. Exact execution
    /// never resolves product membership through source bounds or display names.
    #[derive(Debug, Clone)]
    pub(crate) struct ExploreProductSchemaIr {
        pub identity: TypedExploreProductSchemaIdentity,
        pub fields: Vec<ExploreProductFieldIr>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExploreFlatAliasRole {
        Context { field_index: usize },
        State { field_index: usize },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExploreFlatAliasSource {
        Dimension { dimension_index: usize },
        Fact { fact_index: usize },
    }

    /// Closed provenance for compact source aliases. This is an evaluation view
    /// over the canonical frame, never an alternative transition model.
    #[derive(Debug, Clone)]
    pub(crate) struct ExploreFlatAliasIr {
        pub name: String,
        pub role: ExploreFlatAliasRole,
        pub source: ExploreFlatAliasSource,
    }

    /// Closed, non-optional before/context/after transition contract consumed by
    /// exact execution. Field sources, endpoint membership, and scoped validity
    /// define semantics; `boundary_hint` can only accelerate them.
    #[derive(Debug, Clone)]
    pub(crate) struct ExploreTransitionIr {
        pub normalization_version: u32,
        pub state_schema: ExploreProductSchemaIr,
        pub context_schema: ExploreProductSchemaIr,
        pub after_fields: Vec<ExploreAfterFieldIr>,
        pub after_membership: Vec<ExploreAfterMembershipIr>,
        pub flat_aliases: Vec<ExploreFlatAliasIr>,
        /// Checked optimizer metadata. Semantic endpoint construction, membership,
        /// and validity are represented elsewhere in this transition IR.
        pub boundary_hint: Option<ExploreBoundaryIr>,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExploreUniverseIr {
        pub dimensions: Vec<ExploreDimensionIr>,
        pub facts: Vec<ExploreFactIr>,
        pub constraints: Vec<ExploreConstraintIr>,
        pub sliced_inputs: Vec<TypedExploreInput>,
        /// Product before `where` and before the queried rule.  This is never
        /// presented as the admissible/result count.
        pub cartesian_count_before_constraints: ExploreCardinality,
    }
}

// Retained only as source-local migration history while the relational public
// execution path replaces the Cartesian exact executor. This module has no
// normal or test reachability and is not a compatibility surface.
#[rustfmt::skip]
#[cfg(any())]
mod retired_cartesian_public_execution {
use super::*;

/// Default answer-search cap for the public exact-finite executor.
///
/// The internal reference engine can be driven without a case cap, but a
/// first-class API must never make a huge finite Cartesian product
/// operationally unbounded by default. Hitting this limit produces an honest
/// `Partial` report with a canonical open suffix.
pub const DEFAULT_EXPLORE_EXACT_CASE_LIMIT: u128 = 100_000;

/// Operational controls for the public exact-finite Explore backend.
///
/// These values are run metadata rather than query identity. Raising a limit
/// may only refine open evidence; it cannot change a previously closed case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExactOptions {
    pub case_limit: NonZeroU128,
}

impl Default for ExploreExactOptions {
    fn default() -> Self {
        Self {
            case_limit: NonZeroU128::new(DEFAULT_EXPLORE_EXACT_CASE_LIMIT)
                .expect("the default Explore case limit is positive"),
        }
    }
}

/// Optional milestone at which one durable Explore invocation should publish
/// a paused snapshot instead of beginning singleton case work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamPauseAfter {
    Probes,
}

/// Explicit graph disclosure requested for one durable Explore stream.
///
/// The request is part of immutable run identity. A run created with omitted
/// graph evidence cannot later be reopened with that graph enabled, or vice
/// versa. Search-decision and semantic-transition graphs use independent
/// values of this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamGraphRequest {
    Omit,
    Full,
}

impl ExploreStreamSliceOptions {
    fn report_request(&self) -> report::ExploreReportRequest {
        report::ExploreReportRequest {
            search_decision_dag: match self.search_decision_dag {
                ExploreStreamGraphRequest::Omit => report::ExploreSearchDecisionDagRequest::Omit,
                ExploreStreamGraphRequest::Full => report::ExploreSearchDecisionDagRequest::Include,
            },
            semantic_transition_graph: match self.semantic_transition_graph {
                ExploreStreamGraphRequest::Omit => {
                    report::ExploreSemanticTransitionGraphRequest::Omit
                }
                ExploreStreamGraphRequest::Full => {
                    report::ExploreSemanticTransitionGraphRequest::Include
                }
            },
            ledger: report::ExploreLedgerRequest::Omit,
        }
    }
}

/// Controls for one resumable Explore invocation.
///
/// Time, milestone, and finalization choices are operational. The explicit
/// graph disclosure requests are immutable report identity. Reopening the same
/// `run_state` may vary slice controls but must repeat both requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamSliceOptions {
    pub run_state: PathBuf,
    pub max_runtime: Option<Duration>,
    pub pause_after: Option<ExploreStreamPauseAfter>,
    /// Privacy-sensitive search-decision DAG disclosure. Omitted streams
    /// publish counts and result rows without exposing the search partition.
    pub search_decision_dag: ExploreStreamGraphRequest,
    /// Privacy-sensitive semantic State/Context/Transition disclosure.
    /// Transition populations remain available even when this graph is
    /// omitted or exceeds its all-or-none publication cap.
    pub semantic_transition_graph: ExploreStreamGraphRequest,
    /// Opt in to the bounded atomic-v1 terminal replay/publication phase once
    /// case classification is closed. This does not replace the required
    /// invocation time/milestone control.
    pub finalize: bool,
}

/// Honest nonterminal outcome of one bounded invocation. Case classification
/// closure is reported separately because final representative/extrema replay
/// is its own required frontier obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamSliceStop {
    ProbeMilestone,
    /// A preceding invocation committed a journal-only pause. This resumed
    /// invocation serviced that pending observer boundary before advancing the
    /// semantic frontier; the artifact says whether materialization succeeded
    /// or had to remain deferred.
    SnapshotCatchUp,
    TimeLimit,
    ResourcePressure {
        detail: String,
    },
    /// One CaseId remains open because the immutable evaluator contract hit a
    /// deterministic per-case limit. Reopening unchanged will retry that same
    /// rank; it is not an ordinary productive pause.
    EvaluationLimit {
        blocked_rank: u128,
        reason: ExploreExecutionStopReason,
    },
    /// Classification is closed, but the current atomic finalizer cannot fit
    /// this answer inside its versioned witness/snapshot/publication envelope.
    /// The evidence remains valid and resumable for a future chunked finalizer.
    FinalizationLimit {
        phase: String,
        detail: String,
    },
    ClassificationClosedFinalizationPending,
    /// This invocation closed the required frontier, published the immutable
    /// terminal answer and committed its terminal seal.
    TerminalSealed(ExploreStreamTerminalStatus),
    AlreadySealed(ExploreStreamTerminalStatus),
}

/// Terminal kind recovered from an already sealed durable run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamTerminalStatus {
    Completed,
    Partial,
    Unknown,
    Unsupported,
    Error,
    Cancelled,
}

/// Public cursor for one observable point in the append-only Explore stream.
///
/// Hashes use canonical lowercase SHA-256 spelling. A materialized snapshot
/// report exposes its pre-publication and publication cursors; a journal-only
/// pause has only the final pause cursor because no view record was minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStreamLifecycle {
    Running,
    Paused,
    Sealed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamCursor {
    pub run_id: String,
    pub sequence: u64,
    pub journal_head: String,
    pub evidence_root: String,
    pub lifecycle: ExploreStreamLifecycle,
    pub last_coverage_epoch: Option<u64>,
}

/// Why this invocation committed a replayable journal pause without also
/// materializing its potentially much larger observer view. This is an
/// operational view status, not evidence that a requested graph or count view
/// hit a semantic or schema capacity bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamObserverDeferral {
    TimeLimit,
    ResourceAdmission { detail: String },
}

/// Observable artifact status returned by one durable invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreStreamArtifact {
    /// Cursor-bearing, bounded observable checkpoint followed by one LF. The
    /// bytes are installed content-addressably and named by a subsequent
    /// `SnapshotPublished` journal record before the invocation pauses.
    CheckpointSnapshotJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
    },
    /// Cursor-bearing bounded receipt published when an admitted full-snapshot
    /// attempt reports capacity at this cursor. This is neither a partial
    /// snapshot nor a claim that a later attempt can never fit.
    CheckpointSnapshotUnavailableJsonLine {
        canonical_json_line: Vec<u8>,
        blob_digest: String,
        checkpoint_cursor: ExploreStreamCursor,
        publication_cursor: ExploreStreamCursor,
        detail: String,
    },
    /// The append-only journal is already a complete resume checkpoint. When
    /// the bounded snapshot phase is not admitted, pausing must not spend the
    /// host reserve to manufacture a materialized view.
    JournalOnlyCheckpoint {
        observer_deferral: ExploreStreamObserverDeferral,
    },
    /// History-independent immutable terminal answer bytes and their raw blob
    /// address. The final cursor commits the separate semantic payload hash.
    TerminalResultJson {
        canonical_json: Vec<u8>,
        blob_digest: String,
    },
}

/// One canonical observable or terminal point in the durable stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreStreamSliceReport {
    pub stop: ExploreStreamSliceStop,
    /// Cursor after the publication/pause or terminal-seal records committed by
    /// this invocation.
    pub final_cursor: ExploreStreamCursor,
    pub probe_milestone_complete: bool,
    /// Whole singleton cases evaluated and committed by this invocation.
    pub singleton_cases_evaluated_this_slice: u128,
    /// Total newly closed support, including weighted proof/structural regions.
    pub closed_cases_this_slice: u128,
    pub artifact: ExploreStreamArtifact,
}

/// One declared search axis in canonical transition-role order, before
/// constraints or question evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlanAxis {
    pub name: String,
    pub bound_index: usize,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
    pub cardinality: ExploreCardinality,
}

/// Static boundary-search shape derived by the ordinary Explore elaborator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlanBoundary {
    pub axis: String,
    pub axis_dimension_index: usize,
    pub step: i64,
    /// Product of the boundary-eligible axis pairs and every other declared
    /// axis, before `where` constraints or question evaluation.
    pub eligible_unconstrained_pairs: ExploreCardinality,
}

/// A no-execution cost/search plan for one checked Explore query.
///
/// This is planning metadata, not result evidence: it evaluates no cases,
/// establishes no closure, and contains no mechanism or symbolic candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreCostPlan {
    pub query_name: String,
    pub axes: Vec<ExploreCostPlanAxis>,
    /// `U`: the declared Cartesian product before constraints and before the
    /// queried rule.
    pub declared_cartesian_count: ExploreCardinality,
    pub boundary: Option<ExploreCostPlanBoundary>,
    pub requested_case_limit: u128,
    /// Number of singleton assignments a naive exact exhaustion would plan to
    /// classify under the requested cap.
    pub naive_singleton_classifications: u128,
    /// Assignments necessarily left open by that cap. Available only when `U`
    /// fits in `u128`; this is still a cost estimate, not observed closure.
    pub naive_remaining_open_lower_bound: Option<u128>,
}

/// Certainty attached to one nonnegative Explore population count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreCountEvidence {
    Exact(u128),
    LowerBound(u128),
    Unknown,
}

/// Exact populations remain distinct: declared assignments (`U`), admissible
/// cases (`D`), matching cases (`M`) and emitted result identities (`R`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionCounts {
    pub declared_assignments: ExploreCountEvidence,
    pub admissible_configurations: ExploreCountEvidence,
    pub matching_configurations: ExploreCountEvidence,
    pub distinct_result_keys: ExploreCountEvidence,
}

/// Group populations surrounding the post-aggregation `having` view.
/// Suppressed cases remain part of D/M and of any requested case evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionGroupCounts {
    pub raw_groups: ExploreCountEvidence,
    pub emitted_groups: ExploreCountEvidence,
    pub suppressed_groups: ExploreCountEvidence,
    pub qualifying_configurations: ExploreCountEvidence,
    pub suppressed_configurations: ExploreCountEvidence,
}

/// Public, name-stable description of the post-aggregation result view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionGroupFilter {
    All,
    Varies { extrema_name: String },
}

/// Matching coverage over the admissible population. This is independent of
/// whether execution itself completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionCoverage {
    Empty,
    None,
    Some,
    All,
    Undetermined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionClosure {
    Open,
    Closed,
}

/// Closure of answer/case/value layers. Mechanism evidence is deliberately
/// reported separately and never downgrades a closed answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionClosures {
    /// Key discovery plus any requested extrema aggregation and `having`
    /// classification.
    pub projection: ExploreExecutionClosure,
    pub admissibility: ExploreExecutionClosure,
    pub polarity: ExploreExecutionClosure,
    pub representatives: ExploreExecutionClosure,
    pub rows: ExploreExecutionClosure,
    pub views: ExploreExecutionClosure,
}

/// One name/value pair authorized by the query's `key` or `show` projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionField {
    pub name: String,
    pub value: ExploreValue,
}

/// Exact closed extrema of one integer measure within a projected key group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionExtrema {
    pub name: String,
    pub minimum: i64,
    pub maximum: i64,
    pub spread: u128,
    pub minimum_tie_support: u128,
    pub maximum_tie_support: u128,
    /// Canonical domain ordinals of a freshly replayed minimum witness.
    pub minimum_witness_case_id: Vec<u128>,
    /// Canonical domain ordinals of a freshly replayed maximum witness.
    pub maximum_witness_case_id: Vec<u128>,
}

/// One canonical projected result with a replay-confirmed representative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionRow {
    pub key: Vec<ExploreExecutionField>,
    pub extrema: Vec<ExploreExecutionExtrema>,
    pub shown: Vec<ExploreExecutionField>,
    /// Exact when the projected key class is closed; otherwise a confirmed
    /// lower bound over the evaluated closed cases.
    pub support: ExploreCountEvidence,
    /// Domain ordinals in canonical Context → Before → independent-After axis
    /// order, not raw hidden input values.
    pub representative_case_id: Vec<u128>,
}

/// Structural identity and presentation label for one CaseId coordinate.
/// Labels may repeat across roles; consumers must use the indexed descriptor
/// rather than parse or compare the display spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionDimension {
    pub bound_index: usize,
    pub role: ExploreGeneratorAxisRole,
    pub role_field_index: usize,
    pub label: String,
}

impl ExploreExecutionDimension {
    pub fn qualified_label(&self) -> String {
        let role = match self.role {
            ExploreGeneratorAxisRole::Context => "context",
            ExploreGeneratorAxisRole::Before => "before",
            ExploreGeneratorAxisRole::AfterIndependent => "after",
        };
        format!("{role}.{}", self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionLimitResource {
    Steps,
    CollectionMembers { operation: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionPhase {
    Initialization,
    DerivedFact { name: String },
    BoundaryEndpoint,
    Constraint { index: usize },
    Question,
    Key { name: String },
    Extrema { name: String },
    Show { name: String },
    Objective,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionStopReason {
    CaseLimit {
        limit: u128,
    },
    RuntimeLimit {
        resource: ExploreExecutionLimitResource,
        limit: u128,
        observed: u128,
        phase: ExploreExecutionPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionMethod {
    ExactFiniteExhaustion,
    ExactFiniteCertifiedClosure,
}

/// Terminal answer status. `Partial` contains only evidence already closed or
/// replay-confirmed; `Unsupported` is never presented as a proof of absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreExecutionOutcome {
    Complete {
        method: ExploreExecutionMethod,
        evidence: ExploreExecutionEvidence,
    },
    Partial {
        stop: ExploreExecutionStopReason,
        evidence: ExploreExecutionEvidence,
    },
    Unknown {
        reason: String,
        evidence: ExploreExecutionEvidence,
    },
    Unsupported {
        diagnostic: String,
    },
    Error {
        diagnostics: Vec<String>,
    },
}

impl ExploreExecutionOutcome {
    pub fn evidence(&self) -> Option<&ExploreExecutionEvidence> {
        match self {
            Self::Complete { evidence, .. }
            | Self::Partial { evidence, .. }
            | Self::Unknown { evidence, .. } => Some(evidence),
            Self::Unsupported { .. } | Self::Error { .. } => None,
        }
    }
}

/// Mechanism tracing is orthogonal to exact case closure. The first public
/// exact backend exposes the absence of mechanism evidence explicitly rather
/// than claiming that zero mechanisms exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionMechanismEvidence {
    UnavailableDeferred,
}

/// Work accounting for the exact search order. Source-event identities stay
/// private scheduling metadata; this evidence reports only auditable counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreExecutionSearchEvidence {
    Canonical {
        classified_cases: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
    SourceCandidateFirst {
        distinct_source_candidates: u128,
        scheduled_source_candidates: u128,
        evaluated_source_candidates: u128,
        scheduled_fallback_cases: u128,
        evaluated_fallback_cases: u128,
        singleton_closed_cases: u128,
        certified_region_closed_cases: u128,
        pending_evaluations: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionEvidence {
    /// Structural descriptors in canonical CaseId axis order.
    pub dimensions: Vec<ExploreExecutionDimension>,
    pub axis_cardinalities: Vec<u128>,
    pub key_names: Vec<String>,
    pub extrema_names: Vec<String>,
    pub shown_names: Vec<String>,
    pub search: ExploreExecutionSearchEvidence,
    pub counts: ExploreExecutionCounts,
    pub group_counts: ExploreExecutionGroupCounts,
    pub group_filter: ExploreExecutionGroupFilter,
    pub coverage: ExploreExecutionCoverage,
    pub closures: ExploreExecutionClosures,
    pub results: Vec<ExploreExecutionRow>,
}

/// Backend-neutral public view of one exact-finite Explore run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreExecutionReport {
    pub query_name: String,
    pub polarity: ExplorePolarity,
    pub outcome: ExploreExecutionOutcome,
    pub mechanism: ExploreExecutionMechanismEvidence,
    pub limits: ExploreExecutionLimits,
}

#[derive(Debug, Clone)]
pub enum ExploreExecutionPreparationError {
    Diagnostics(Vec<Diagnostic>),
    Selection(String),
    Execution(String),
}

impl std::fmt::Display for ExploreExecutionPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "exploration has {} type-check diagnostic{}",
                diagnostics.len(),
                if diagnostics.len() == 1 { "" } else { "s" }
            ),
            Self::Selection(message) | Self::Execution(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ExploreExecutionPreparationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreExecutionLimits {
    pub case_limit: u128,
    pub step_limit: usize,
    pub collection_limit: usize,
}

fn public_count(count: report::ExploreCount) -> ExploreCountEvidence {
    match count {
        report::ExploreCount::Exact(value) => ExploreCountEvidence::Exact(value),
        report::ExploreCount::LowerBound(value) => ExploreCountEvidence::LowerBound(value),
        report::ExploreCount::Unknown => ExploreCountEvidence::Unknown,
    }
}

fn public_closure(closure: report::ExploreClosure) -> ExploreExecutionClosure {
    match closure {
        report::ExploreClosure::Open => ExploreExecutionClosure::Open,
        report::ExploreClosure::Closed => ExploreExecutionClosure::Closed,
    }
}

fn public_phase(phase: report::ExploreEvaluationPhase) -> ExploreExecutionPhase {
    match phase {
        report::ExploreEvaluationPhase::Initialization => ExploreExecutionPhase::Initialization,
        report::ExploreEvaluationPhase::DerivedFact { name } => {
            ExploreExecutionPhase::DerivedFact { name }
        }
        report::ExploreEvaluationPhase::BoundaryEndpoint => ExploreExecutionPhase::BoundaryEndpoint,
        report::ExploreEvaluationPhase::Constraint { index } => {
            ExploreExecutionPhase::Constraint { index }
        }
        report::ExploreEvaluationPhase::Question => ExploreExecutionPhase::Question,
        report::ExploreEvaluationPhase::Key { name } => ExploreExecutionPhase::Key { name },
        report::ExploreEvaluationPhase::Extrema { name } => ExploreExecutionPhase::Extrema { name },
        report::ExploreEvaluationPhase::Show { name } => ExploreExecutionPhase::Show { name },
        report::ExploreEvaluationPhase::Objective => ExploreExecutionPhase::Objective,
        report::ExploreEvaluationPhase::Replay => ExploreExecutionPhase::Replay,
    }
}

fn public_stop(stop: report::ExploreStopReason) -> ExploreExecutionStopReason {
    match stop {
        report::ExploreStopReason::CaseLimit { limit } => {
            ExploreExecutionStopReason::CaseLimit { limit }
        }
        report::ExploreStopReason::RuntimeLimit {
            resource,
            limit,
            observed,
            phase,
        } => ExploreExecutionStopReason::RuntimeLimit {
            resource: match resource {
                report::ExploreLimitResource::Steps => ExploreExecutionLimitResource::Steps,
                report::ExploreLimitResource::CollectionMembers { operation } => {
                    ExploreExecutionLimitResource::CollectionMembers { operation }
                }
            },
            limit,
            observed,
            phase: public_phase(phase),
        },
    }
}

fn public_evidence(evidence: report::ExploreExactEvidence) -> ExploreExecutionEvidence {
    let schema = evidence.schema;
    let counts = evidence.counts;
    let group_counts = evidence.group_counts;
    let closures = evidence.closures;
    let search = match evidence.search {
        report::ExploreSearchEvidence::Canonical {
            classified_cases,
            remaining_open_cases,
            exhausted,
        } => ExploreExecutionSearchEvidence::Canonical {
            classified_cases,
            remaining_open_cases,
            exhausted,
        },
        report::ExploreSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates,
            scheduled_source_candidates,
            evaluated_source_candidates,
            scheduled_fallback_cases,
            evaluated_fallback_cases,
            singleton_closed_cases,
            certified_region_closed_cases,
            pending_evaluations,
            remaining_open_cases,
            exhausted,
        } => ExploreExecutionSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates,
            scheduled_source_candidates,
            evaluated_source_candidates,
            scheduled_fallback_cases,
            evaluated_fallback_cases,
            singleton_closed_cases,
            certified_region_closed_cases,
            pending_evaluations,
            remaining_open_cases,
            exhausted,
        },
    };
    let group_filter = match schema.group_filter {
        report::ExploreGroupFilter::All => ExploreExecutionGroupFilter::All,
        report::ExploreGroupFilter::Varies { extrema_index } => {
            ExploreExecutionGroupFilter::Varies {
                extrema_name: schema
                    .extrema_names
                    .get(extrema_index)
                    .cloned()
                    .expect("validated Explore varies index names an extrema field"),
            }
        }
    };
    ExploreExecutionEvidence {
        dimensions: schema
            .dimensions
            .iter()
            .map(|dimension| ExploreExecutionDimension {
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                label: dimension.label.clone(),
            })
            .collect(),
        axis_cardinalities: schema.axis_cardinalities.into_vec(),
        key_names: schema.key_names.clone().into_vec(),
        extrema_names: schema.extrema_names.clone().into_vec(),
        shown_names: schema.shown_names.clone().into_vec(),
        search,
        counts: ExploreExecutionCounts {
            declared_assignments: public_count(counts.declared_assignments),
            admissible_configurations: public_count(counts.admissible_configurations),
            matching_configurations: public_count(counts.matching_configurations),
            distinct_result_keys: public_count(counts.distinct_result_keys),
        },
        group_counts: ExploreExecutionGroupCounts {
            raw_groups: public_count(group_counts.raw_groups),
            emitted_groups: public_count(group_counts.emitted_groups),
            suppressed_groups: public_count(group_counts.suppressed_groups),
            qualifying_configurations: public_count(group_counts.qualifying_configurations),
            suppressed_configurations: public_count(group_counts.suppressed_configurations),
        },
        group_filter,
        coverage: match evidence.coverage {
            report::ExploreCoverage::Empty => ExploreExecutionCoverage::Empty,
            report::ExploreCoverage::None => ExploreExecutionCoverage::None,
            report::ExploreCoverage::Some => ExploreExecutionCoverage::Some,
            report::ExploreCoverage::All => ExploreExecutionCoverage::All,
            report::ExploreCoverage::Undetermined => ExploreExecutionCoverage::Undetermined,
        },
        closures: ExploreExecutionClosures {
            projection: public_closure(closures.projection),
            admissibility: public_closure(closures.admissibility),
            polarity: public_closure(closures.polarity),
            representatives: public_closure(closures.representatives),
            rows: public_closure(closures.rows),
            views: public_closure(closures.views),
        },
        results: evidence
            .results
            .into_vec()
            .into_iter()
            .map(|row| ExploreExecutionRow {
                key: schema
                    .key_names
                    .iter()
                    .cloned()
                    .zip(row.key.values().iter().cloned())
                    .map(|(name, value)| ExploreExecutionField { name, value })
                    .collect(),
                extrema: schema
                    .extrema_names
                    .iter()
                    .cloned()
                    .zip(row.extrema.into_vec())
                    .map(|(name, summary)| ExploreExecutionExtrema {
                        name,
                        minimum: summary.minimum,
                        maximum: summary.maximum,
                        spread: summary.spread,
                        minimum_tie_support: summary.minimum_tie_support,
                        maximum_tie_support: summary.maximum_tie_support,
                        minimum_witness_case_id: summary.minimum_witness.ordinals().to_vec(),
                        maximum_witness_case_id: summary.maximum_witness.ordinals().to_vec(),
                    })
                    .collect(),
                shown: schema
                    .shown_names
                    .iter()
                    .cloned()
                    .zip(row.shown.into_vec())
                    .map(|(name, value)| ExploreExecutionField { name, value })
                    .collect(),
                support: public_count(row.support),
                representative_case_id: row.representative.ordinals().to_vec(),
            })
            .collect(),
    }
}

fn public_exact_report(
    report: report::ExploreExactReport,
    options: ExploreExactOptions,
) -> ExploreExecutionReport {
    let report::ExploreExactReport {
        query_name,
        polarity,
        mechanism,
        outcome,
    } = report;
    let outcome = match outcome {
        report::ExploreExactOutcome::Complete { method, evidence } => {
            ExploreExecutionOutcome::Complete {
                method: match method {
                    report::ExploreCompletionMethod::ExactFiniteExhaustion => {
                        ExploreExecutionMethod::ExactFiniteExhaustion
                    }
                    report::ExploreCompletionMethod::ExactFiniteCertifiedClosure => {
                        ExploreExecutionMethod::ExactFiniteCertifiedClosure
                    }
                },
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Partial { stop, evidence } => {
            ExploreExecutionOutcome::Partial {
                stop: public_stop(stop),
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Unknown { reason, evidence } => {
            ExploreExecutionOutcome::Unknown {
                reason,
                evidence: public_evidence(evidence),
            }
        }
        report::ExploreExactOutcome::Unsupported { diagnostic } => {
            ExploreExecutionOutcome::Unsupported { diagnostic }
        }
        report::ExploreExactOutcome::Error { diagnostics } => ExploreExecutionOutcome::Error {
            diagnostics: diagnostics.into_vec(),
        },
    };
    let mechanism = match mechanism {
        report::ExploreMechanismEvidence::Unavailable {
            reason: report::ExploreMechanismUnavailableReason::Deferred,
        } => ExploreExecutionMechanismEvidence::UnavailableDeferred,
    };
    ExploreExecutionReport {
        query_name,
        polarity,
        outcome,
        mechanism,
        limits: ExploreExecutionLimits {
            case_limit: options.case_limit.get(),
            step_limit: report::DEFAULT_EXPLORE_STEP_LIMIT,
            collection_limit: report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
        },
    }
}

/// Execute one already checked and elaborated finite Explore query.
///
/// This is the durable exact backend used by the public command. It consumes
/// ordinary `check_with_artifacts` evidence, requires a caller-supplied finite
/// case cap, and publishes
/// only replay-confirmed projected values. Its report request is deliberately
/// the privacy-safe baseline: projected rows only, with no case ledger or case
/// graph disclosure.
fn execute_exact(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    options: ExploreExactOptions,
) -> Result<ExploreExecutionReport, String> {
    // RELATIONAL-IR MIGRATION BREAKPOINT: source proof planning, exact case
    // evaluation, classification regions, mechanism requests, cost planning,
    // and stream replay still read `.universe/.transition/.query.output`.
    // They must consume the relational enumeration frontier and the named
    // classification/view DAG before this public execution path is restored.
    let budget = report::ExploreExecutionBudget::new(
        Some(options.case_limit.get()),
        report::DEFAULT_EXPLORE_STEP_LIMIT,
        report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
    )?;
    let report = match source_proof_plan::prepare_source_proof_plan(
        artifacts,
        accepted_query_index,
        source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
    ) {
        Ok(plan) => exact::execute_exact_finite_candidate_first(
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report::ExploreReportRequest::baseline(),
            budget,
            &plan,
        ),
        // Source proof is an optimization. Unsupported or bounded-out
        // analysis cannot shrink U and therefore falls back to canonical
        // exact evaluation under the same caller case limit.
        Err(error) if error.permits_canonical_fallback() => exact::execute_exact_finite(
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report::ExploreReportRequest::baseline(),
            budget,
        ),
        // A proof artifact that was produced but fails extraction,
        // certification, or accounting is an integrity failure. It must not
        // be hidden by silently retrying the same query canonically.
        Err(error) => return Err(error.to_string()),
    }?;
    Ok(public_exact_report(report, options))
}

fn select_checked_exact_query_index(
    artifacts: &TypeCheckArtifacts,
    query_name: Option<&str>,
) -> Result<usize, ExploreExecutionPreparationError> {
    if let Some(query_name) = query_name {
        return artifacts
            .exploration_universes
            .iter()
            .position(|candidate| candidate.query.name.as_deref() == Some(query_name))
            .ok_or_else(|| {
                ExploreExecutionPreparationError::Selection(format!(
                    "exploration `{query_name}` was not found"
                ))
            });
    }
    if artifacts.exploration_universes.len() == 1 {
        return Ok(0);
    }
    if artifacts.exploration_universes.is_empty() {
        return Err(ExploreExecutionPreparationError::Selection(
            "the program contains no selectable exploration".to_string(),
        ));
    }
    let names = artifacts
        .exploration_universes
        .iter()
        .filter_map(|candidate| candidate.query.name.as_deref())
        .collect::<Vec<_>>()
        .join(", ");
    Err(ExploreExecutionPreparationError::Selection(format!(
        "the program contains multiple explorations; select one with --query ({names})"
    )))
}

fn cost_plan(query: &ExploreQueryIr, options: ExploreExactOptions) -> ExploreCostPlan {
    let declared_cartesian_count = query.universe.cartesian_count_before_constraints.clone();
    let exact_declared = declared_cartesian_count.exact();
    let requested_case_limit = options.case_limit.get();
    let naive_singleton_classifications = exact_declared
        .map(|declared| declared.min(requested_case_limit))
        .unwrap_or(requested_case_limit);
    ExploreCostPlan {
        query_name: query
            .query
            .name
            .clone()
            .unwrap_or_else(|| "<anonymous>".to_string()),
        axes: query
            .universe
            .dimensions
            .iter()
            .map(|dimension| ExploreCostPlanAxis {
                name: dimension.name.clone(),
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                cardinality: dimension.domain.cardinality(),
            })
            .collect(),
        declared_cartesian_count,
        boundary: query
            .boundary_hint()
            .map(|boundary| ExploreCostPlanBoundary {
                axis: boundary.axis.clone(),
                axis_dimension_index: boundary.axis_dimension_index,
                step: boundary.step,
                eligible_unconstrained_pairs: boundary.eligible_unconstrained_pairs.clone(),
            }),
        requested_case_limit,
        naive_singleton_classifications,
        naive_remaining_open_lower_bound: exact_declared
            .map(|declared| declared.saturating_sub(naive_singleton_classifications)),
    }
}

/// Check, elaborate, and select one exact-finite exploration without
/// initializing an interpreter or evaluating any case.
///
/// Query selection is shared with [`execute_checked_exact`]. The returned
/// metadata describes the declared search shape and a naive cap-limited cost;
/// it is not result evidence and does not establish closure.
pub fn plan_checked_exact(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreExactOptions,
) -> Result<ExploreCostPlan, ExploreExecutionPreparationError> {
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir, source);
    if !artifacts.diagnostics.is_empty() {
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }
    let selected = select_checked_exact_query_index(&artifacts, query_name)?;
    Ok(cost_plan(
        &artifacts.exploration_universes[selected],
        options,
    ))
}

/// Check, elaborate, select and execute one exact-finite exploration as one
/// inseparable operation. This prevents callers from combining statements,
/// artifacts and a query IR produced by different checks.
pub fn execute_checked_exact(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreExactOptions,
) -> Result<ExploreExecutionReport, ExploreExecutionPreparationError> {
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir.clone(), source);
    if !artifacts.diagnostics.is_empty() {
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }

    let selected = select_checked_exact_query_index(&artifacts, query_name)?;

    execute_exact(
        statements,
        source_dir.as_deref(),
        &artifacts,
        selected,
        options,
    )
    .map_err(ExploreExecutionPreparationError::Execution)
}

enum ExactStreamWorkAdmission {
    Granted(stream_resource::ExactStreamWorkInFlight),
    TimeLimit,
    ResourcePause(stream_resource::ExactStreamResourcePauseReason),
}

fn admit_exact_stream_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    subject: stream_resource::ExactStreamWorkSubject,
    deadline: Option<Instant>,
) -> Result<ExactStreamWorkAdmission, ExploreExecutionPreparationError> {
    loop {
        let now = Instant::now();
        if deadline.is_some_and(|deadline| now >= deadline) {
            let _ = resources.stop_at_work_boundary();
            return Ok(ExactStreamWorkAdmission::TimeLimit);
        }

        let owned = resources.conservative_in_process_owned_snapshot();
        let poll = resources.poll(owned, None, Some(subject));
        match poll.action {
            stream_resource::ExactStreamResourceAction::Dispatch(permit) => {
                if permit.subject() != subject {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor dispatched authority for another Explore work subject"
                            .to_string(),
                    ));
                }
                let in_flight = resources.begin_work(permit).map_err(|error| {
                    ExploreExecutionPreparationError::Execution(format!(
                        "cannot consume exact-stream resource permit: {error:?}"
                    ))
                })?;
                return Ok(ExactStreamWorkAdmission::Granted(in_flight));
            }
            stream_resource::ExactStreamResourceAction::Pause(reason) => {
                return Ok(ExactStreamWorkAdmission::ResourcePause(reason));
            }
            stream_resource::ExactStreamResourceAction::Wait(_) => {
                let now = Instant::now();
                let mut wake = poll
                    .next_host_sample_due
                    .unwrap_or_else(|| now.checked_add(Duration::from_millis(10)).unwrap_or(now));
                if let Some(deadline) = deadline {
                    wake = wake.min(deadline);
                }
                if wake > now {
                    std::thread::sleep(wake.saturating_duration_since(now));
                } else {
                    std::thread::yield_now();
                }
            }
        }
    }
}

/// Try exactly once to admit the optional materialized-view phase. A semantic
/// work loop may wait for a stable resource window; an invocation that has
/// already reached a useful pause boundary must instead durably pause and let
/// a later invocation mint the view. This keeps checkpointing from consuming
/// the host reserve precisely when the governor has withdrawn work authority.
fn try_admit_exact_stream_snapshot_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    deadline: Option<Instant>,
) -> Result<ExactStreamWorkAdmission, ExploreExecutionPreparationError> {
    let now = Instant::now();
    if deadline.is_some_and(|deadline| now >= deadline) {
        let _ = resources.stop_at_work_boundary();
        return Ok(ExactStreamWorkAdmission::TimeLimit);
    }

    let subject = stream_resource::ExactStreamWorkSubject::SnapshotPublicationPhase;
    let owned = resources.conservative_in_process_owned_snapshot();
    let poll = resources.poll(owned, None, Some(subject));
    match poll.action {
        stream_resource::ExactStreamResourceAction::Dispatch(permit) => {
            if permit.subject() != subject {
                return Err(ExploreExecutionPreparationError::Execution(
                    "resource governor dispatched authority for another Explore snapshot phase"
                        .to_string(),
                ));
            }
            let in_flight = resources.begin_work(permit).map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot consume exact-stream snapshot resource permit: {error:?}"
                ))
            })?;
            Ok(ExactStreamWorkAdmission::Granted(in_flight))
        }
        stream_resource::ExactStreamResourceAction::Pause(reason)
        | stream_resource::ExactStreamResourceAction::Wait(reason) => {
            Ok(ExactStreamWorkAdmission::ResourcePause(reason))
        }
    }
}

fn finish_exact_stream_work(
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    in_flight: stream_resource::ExactStreamWorkInFlight,
) -> Result<(), ExploreExecutionPreparationError> {
    resources
        .finish_or_abandon_work(in_flight)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot close exact-stream resource work unit: {error:?}"
            ))
        })
}

fn public_exact_stream_cursor(cursor: run_stream::ExploreRunCursor) -> ExploreStreamCursor {
    ExploreStreamCursor {
        run_id: cursor.run_id().to_lowercase_hex(),
        sequence: cursor.sequence(),
        journal_head: cursor.journal_head().to_lowercase_hex(),
        evidence_root: cursor.evidence_root().to_lowercase_hex(),
        lifecycle: match cursor.lifecycle() {
            run_stream::RunLifecycle::Running => ExploreStreamLifecycle::Running,
            run_stream::RunLifecycle::Paused => ExploreStreamLifecycle::Paused,
            run_stream::RunLifecycle::Sealed => ExploreStreamLifecycle::Sealed,
        },
        last_coverage_epoch: cursor.last_coverage_epoch().map(|epoch| epoch.get()),
    }
}

/// Publish a replay-verifiable checkpoint for the current running cursor, then
/// append the invocation's pause record. Keeping those as two ordered records
/// avoids the circularity of making a snapshot hash name the event that names
/// that same hash. The returned report carries both cursors and the typed stop.
fn publish_prepared_snapshot_and_pause_exact_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    prepared_snapshot: stream_coordinator::PreparedExactObservableSnapshotPublication,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let materialization_capacity_detail = prepared_snapshot
        .materialization_capacity_detail()
        .map(str::to_string);
    let probe_milestone_complete = prepared_snapshot.probe_milestone_complete();
    let checkpoint_cursor = prepared_snapshot.cursor();
    checkpoint_cursor.sequence().checked_add(2).ok_or_else(|| {
        ExploreExecutionPreparationError::Execution(
            "exact-stream journal sequence cannot fit checkpoint publication and pause".to_string(),
        )
    })?;
    let closed_cases_this_slice = prepared_snapshot
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one invocation".to_string(),
            )
        })?;
    let blob_digest = coordinator
        .publish_prepared_snapshot(&prepared_snapshot)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot publish exact-stream checkpoint: {error}"
            ))
        })?;
    let publication_cursor = coordinator.stream().cursor();
    let final_cursor = coordinator.pause(pause_reason).map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "checkpoint {} was published at sequence {}, but the exact stream could not append its pause record: {error}",
            blob_digest.to_lowercase_hex(),
            publication_cursor.sequence(),
        ))
    })?;
    let blob_digest = blob_digest.to_lowercase_hex();
    let checkpoint_cursor = public_exact_stream_cursor(checkpoint_cursor);
    let publication_cursor = public_exact_stream_cursor(publication_cursor);
    let canonical_json_line = prepared_snapshot.into_canonical_json_line();
    let artifact = match materialization_capacity_detail {
        Some(detail) => ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
            detail,
        },
        None => ExploreStreamArtifact::CheckpointSnapshotJsonLine {
            canonical_json_line,
            blob_digest,
            checkpoint_cursor,
            publication_cursor,
        },
    };
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(final_cursor),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact,
    })
}

fn pause_exact_stream_slice_without_snapshot(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    observer_deferral: ExploreStreamObserverDeferral,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let probe_milestone_complete = coordinator
        .probe_progress()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive journal-only source-probe progress: {error}"
            ))
        })?
        .complete();
    let closed_cases_this_slice = coordinator
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one invocation".to_string(),
            )
        })?;
    let final_cursor = coordinator.pause(pause_reason).map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "cannot append journal-only exact-stream pause: {error}"
        ))
    })?;
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(final_cursor),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact: ExploreStreamArtifact::JournalOnlyCheckpoint { observer_deferral },
    })
}

/// Mint a materialized snapshot only while the 80%-ceiling governor grants a
/// bounded phase. The append-only journal remains the authoritative resume
/// checkpoint, so denied view work degrades to a typed journal-only pause
/// rather than borrowing memory from the host reserve.
fn publish_or_defer_and_pause_exact_stream_slice(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    _query: &ExploreQueryIr,
    deadline: Option<Instant>,
    pause_reason: run_stream::PauseReason,
    stop: ExploreStreamSliceStop,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    match try_admit_exact_stream_snapshot_work(resources, deadline)? {
        ExactStreamWorkAdmission::Granted(in_flight) => {
            let mut snapshot_authority = match in_flight.into_snapshot_publication_authority() {
                Ok(authority) => authority,
                Err(in_flight) => {
                    finish_exact_stream_work(resources, in_flight)?;
                    return Err(ExploreExecutionPreparationError::Execution(
                        "admitted Explore work unit did not carry snapshot-publication authority"
                            .to_string(),
                    ));
                }
            };
            let prepared_snapshot = match coordinator
                .prepare_observable_snapshot_publication(&mut snapshot_authority)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
                    return Err(ExploreExecutionPreparationError::Execution(format!(
                        "cannot prepare exact-stream snapshot publication: {error}"
                    )));
                }
            };
            let publication = publish_prepared_snapshot_and_pause_exact_stream_slice(
                coordinator,
                prepared_snapshot,
                pause_reason,
                stop,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
            finish_exact_stream_work(resources, snapshot_authority.into_in_flight())?;
            publication
        }
        ExactStreamWorkAdmission::TimeLimit => pause_exact_stream_slice_without_snapshot(
            coordinator,
            pause_reason,
            stop,
            ExploreStreamObserverDeferral::TimeLimit,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        ),
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            pause_exact_stream_slice_without_snapshot(
                coordinator,
                pause_reason,
                stop,
                ExploreStreamObserverDeferral::ResourceAdmission {
                    detail: reason.code().to_string(),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
    }
}

fn render_exact_stream_terminal(
    coordinator: &stream_coordinator::ExactStreamCoordinator<'_>,
    stop: ExploreStreamSliceStop,
    terminal_result_json: Vec<u8>,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let probe_milestone_complete = coordinator
        .probe_progress()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive terminal source-probe progress: {error}"
            ))
        })?
        .complete();
    let closed_cases_this_slice = coordinator
        .closed_case_count()
        .checked_sub(closed_cases_at_slice_start)
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream closed support regressed during one terminal invocation".to_string(),
            )
        })?;
    let terminal_blob_digest = coordinator
        .published_terminal_result()
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "terminal artifact has no durable publication receipt".to_string(),
            )
        })?
        .blob_digest()
        .to_lowercase_hex();
    Ok(ExploreStreamSliceReport {
        stop,
        final_cursor: public_exact_stream_cursor(coordinator.stream().cursor()),
        probe_milestone_complete,
        singleton_cases_evaluated_this_slice,
        closed_cases_this_slice,
        artifact: ExploreStreamArtifact::TerminalResultJson {
            canonical_json: terminal_result_json,
            blob_digest: terminal_blob_digest,
        },
    })
}

fn render_already_sealed_exact_stream(
    coordinator: &stream_coordinator::ExactStreamCoordinator<'_>,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    let status = match coordinator
        .stream()
        .terminal_seal()
        .ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "sealed exact stream is missing its terminal commitment".to_string(),
            )
        })?
        .kind()
    {
        run_stream::TerminalSealKind::Completed => ExploreStreamTerminalStatus::Completed,
        run_stream::TerminalSealKind::Partial => ExploreStreamTerminalStatus::Partial,
        run_stream::TerminalSealKind::Unknown => ExploreStreamTerminalStatus::Unknown,
        run_stream::TerminalSealKind::Unsupported => ExploreStreamTerminalStatus::Unsupported,
        run_stream::TerminalSealKind::Error => ExploreStreamTerminalStatus::Error,
        run_stream::TerminalSealKind::Cancelled => ExploreStreamTerminalStatus::Cancelled,
    };
    let terminal_result_json =
        coordinator
            .read_verified_terminal_result_bytes()
            .map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot read verified terminal artifact from sealed exact Explore run: {error}"
                ))
            })?;
    render_exact_stream_terminal(
        coordinator,
        ExploreStreamSliceStop::AlreadySealed(status),
        terminal_result_json,
        0,
        closed_cases_at_slice_start,
    )
}

enum ExactStreamFinalizationAttempt {
    Sealed(Vec<u8>),
    WitnessOpen {
        rank: u128,
        reason: report::ExploreStopReason,
    },
    LimitReached {
        phase: &'static str,
        detail: String,
    },
}

/// Run the semantic portion of the atomic-v1 finalizer. Production callers
/// must hold the admitted `FinalizationPhase` work unit around this call; the
/// cardinality-one lifecycle test invokes it directly to avoid live telemetry.
fn attempt_atomic_exact_stream_finalization(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    graph_publications: &stream_coordinator::PreparedExactGraphPublicationsV1,
) -> Result<ExactStreamFinalizationAttempt, ExploreExecutionPreparationError> {
    match coordinator.close_replay_obligation().map_err(|error| {
        ExploreExecutionPreparationError::Execution(format!(
            "cannot close exact terminal replay obligation: {error}"
        ))
    })? {
        stream_coordinator::ExactReplayClosureAdvance::AlreadyClosed
        | stream_coordinator::ExactReplayClosureAdvance::Closed { .. } => {}
        stream_coordinator::ExactReplayClosureAdvance::WitnessOpen { rank, reason } => {
            return Ok(ExactStreamFinalizationAttempt::WitnessOpen { rank, reason });
        }
        stream_coordinator::ExactReplayClosureAdvance::LimitReached { detail } => {
            return Ok(ExactStreamFinalizationAttempt::LimitReached {
                phase: "witness_replay",
                detail,
            });
        }
    }

    let receipt = match coordinator.published_terminal_result() {
        Some(receipt) => receipt,
        None => match coordinator
            .publish_current_terminal_result(graph_publications)
            .map_err(|error| {
                ExploreExecutionPreparationError::Execution(format!(
                    "cannot publish exact terminal result: {error}"
                ))
            })? {
            stream_coordinator::ExactTerminalPublicationAdvanceV1::Published(receipt) => receipt,
            stream_coordinator::ExactTerminalPublicationAdvanceV1::LimitReached {
                phase,
                detail,
            } => {
                return Ok(ExactStreamFinalizationAttempt::LimitReached { phase, detail });
            }
        },
    };
    coordinator
        .seal_completed_exact_exhaustion(receipt)
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot seal completed exact exploration: {error}"
            ))
        })?;
    let bytes = coordinator
        .read_verified_terminal_result_bytes()
        .map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot read back sealed exact terminal result: {error}"
            ))
        })?;
    Ok(ExactStreamFinalizationAttempt::Sealed(bytes))
}

/// Handle the exact point where CaseId classification is closed.
///
/// Without explicit opt-in this remains a cheap durable pause. With opt-in,
/// the existing v1 finalizer is admitted as one atomic resource work unit:
/// at most 65,536 freshly replayed witnesses and 32 MiB of retained replay
/// bodies, followed by one full terminal JSON blob capped by its renderer. It
/// is not a resumable inner loop. The process supervisor may interrupt it;
/// replay then retries from the last committed replay-closure, publication, or
/// seal event.
fn finalize_or_pause_classification_closed_stream(
    coordinator: &mut stream_coordinator::ExactStreamCoordinator<'_>,
    resources: &mut stream_resource::ExactStreamOneWorkerEnvelope,
    query: &ExploreQueryIr,
    finalize: bool,
    deadline: Option<Instant>,
    singleton_cases_evaluated_this_slice: u128,
    closed_cases_at_slice_start: u128,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if !finalize {
        return publish_or_defer_and_pause_exact_stream_slice(
            coordinator,
            resources,
            query,
            deadline,
            run_stream::PauseReason::FinalizationPending,
            ExploreStreamSliceStop::ClassificationClosedFinalizationPending,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    let work_subject = stream_resource::ExactStreamWorkSubject::FinalizationPhase;
    let in_flight = match admit_exact_stream_work(resources, work_subject, deadline)? {
        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
        ExactStreamWorkAdmission::TimeLimit => {
            return publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::TimeLimit,
                ExploreStreamSliceStop::TimeLimit,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            return publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::ResourcePressure,
                ExploreStreamSliceStop::ResourcePressure {
                    detail: reason.code().to_string(),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }
    };
    if in_flight.subject() != work_subject {
        return Err(ExploreExecutionPreparationError::Execution(
            "resource governor admitted another work unit instead of terminal finalization"
                .to_string(),
        ));
    }

    // Atomic-v1 may only clone/finalize reducer state that already fits the
    // identity-bound observable snapshot envelope. Larger exact answers remain
    // valid at the finalization frontier for a future chunked publisher.
    let atomic_snapshot = coordinator.exact_snapshot();
    if !atomic_snapshot.result_group_scan_complete {
        let detail = format!(
            "{} observed raw groups do not fit the bounded atomic snapshot envelope",
            atomic_snapshot.observed_result_group_count
        );
        finish_exact_stream_work(resources, in_flight)?;
        return publish_or_defer_and_pause_exact_stream_slice(
            coordinator,
            resources,
            query,
            deadline,
            run_stream::PauseReason::FinalizationPending,
            ExploreStreamSliceStop::FinalizationLimit {
                phase: "result_snapshot".to_string(),
                detail,
            },
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }
    let graph_publications = match coordinator.prepare_graph_publications() {
        Ok(publications) => publications,
        Err(error) => {
            finish_exact_stream_work(resources, in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "cannot prepare final graph publications: {error}"
            )));
        }
    };
    // Deterministic all-or-none caps for either requested graph are terminal
    // publication statuses, not semantic or operational finalization failures.
    // The renderer emits exact counts and typed capacity status without a DAG
    // or semantic-transition prefix.
    drop(atomic_snapshot);

    let attempt = attempt_atomic_exact_stream_finalization(coordinator, &graph_publications);
    finish_exact_stream_work(resources, in_flight)?;

    match attempt? {
        ExactStreamFinalizationAttempt::Sealed(bytes) => render_exact_stream_terminal(
            coordinator,
            ExploreStreamSliceStop::TerminalSealed(ExploreStreamTerminalStatus::Completed),
            bytes,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        ),
        ExactStreamFinalizationAttempt::WitnessOpen { rank, reason } => {
            publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::EvaluationLimit,
                ExploreStreamSliceStop::EvaluationLimit {
                    blocked_rank: rank,
                    reason: public_stop(reason),
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
        ExactStreamFinalizationAttempt::LimitReached { phase, detail } => {
            publish_or_defer_and_pause_exact_stream_slice(
                coordinator,
                resources,
                query,
                deadline,
                run_stream::PauseReason::FinalizationPending,
                ExploreStreamSliceStop::FinalizationLimit {
                    phase: phase.to_string(),
                    detail,
                },
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            )
        }
    }
}

/// Check, open or resume, and advance one bounded durable exact Explore slice.
///
/// Terminal witness replay remains opt-in because its first-generation
/// manifest is one bounded-but-atomic work unit. Without `finalize`, closed
/// classification pauses at the explicit finalization frontier. A hard process
/// kill may omit the final pause or terminal record but cannot make an
/// uncommitted CaseId, replay closure, publication, or seal disappear from the
/// recovered durable state.
pub fn execute_checked_exact_stream_slice(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreStreamSliceOptions,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    execute_checked_stream_slice_v1(statements, source_dir, source, query_name, options, None)
}

#[allow(clippy::too_many_arguments)]
fn execute_checked_stream_slice_v1(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreStreamSliceOptions,
    resources: Option<stream_resource::ExactStreamOneWorkerEnvelope>,
) -> Result<ExploreStreamSliceReport, ExploreExecutionPreparationError> {
    if options.max_runtime.is_some_and(|runtime| runtime.is_zero()) {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream max_runtime must be positive".to_string(),
        ));
    }
    if options.max_runtime.is_none() && options.pause_after.is_none() {
        return Err(ExploreExecutionPreparationError::Execution(
            "a first-generation exact stream slice requires max_runtime or pause_after".to_string(),
        ));
    }
    if options.finalize && options.pause_after.is_some() {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream finalize cannot be combined with pause_after".to_string(),
        ));
    }
    if options.run_state.as_os_str().is_empty() {
        return Err(ExploreExecutionPreparationError::Execution(
            "exact-stream run_state path must not be empty".to_string(),
        ));
    }

    let started = Instant::now();
    let deadline = match options.max_runtime {
        Some(runtime) => Some(started.checked_add(runtime).ok_or_else(|| {
            ExploreExecutionPreparationError::Execution(
                "exact-stream runtime deadline exceeds the monotonic clock".to_string(),
            )
        })?),
        None => None,
    };
    let mut resources = match resources {
        Some(resources) => resources,
        None => stream_resource::ExactStreamOneWorkerEnvelope::new().map_err(|reason| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot initialize exact-stream resource governor: {}",
                reason.code()
            ))
        })?,
    };
    let preparation_in_flight = match admit_exact_stream_work(
        &mut resources,
        stream_resource::ExactStreamWorkSubject::PreparationPhase,
        deadline,
    )? {
        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
        ExactStreamWorkAdmission::TimeLimit => {
            return Err(ExploreExecutionPreparationError::Execution(
                "exact-stream time limit elapsed before checked preparation could be admitted; no run-state transition was made"
                    .to_string(),
            ))
        }
        ExactStreamWorkAdmission::ResourcePause(reason) => {
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "exact-stream checked preparation was not admitted under the host resource envelope: {}",
                reason.code()
            )))
        }
    };
    let artifacts = TypeChecker::check_with_artifacts(statements, source_dir.clone(), source);
    if !artifacts.diagnostics.is_empty() {
        finish_exact_stream_work(&mut resources, preparation_in_flight)?;
        return Err(ExploreExecutionPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }
    let selected = match select_checked_exact_query_index(&artifacts, query_name) {
        Ok(selected) => selected,
        Err(error) => {
            finish_exact_stream_work(&mut resources, preparation_in_flight)?;
            return Err(error);
        }
    };
    let query = &artifacts.exploration_universes[selected];
    let report_request = options.report_request();
    let coordinator_result = stream_coordinator::ExactStreamCoordinator::open_or_create(
        &options.run_state,
        run_store::RunStoreLimits::default(),
        statements,
        source_dir.as_deref(),
        &artifacts,
        selected,
        report_request,
    );
    let mut coordinator = match coordinator_result {
        Ok(coordinator) => coordinator,
        Err(error) => {
            finish_exact_stream_work(&mut resources, preparation_in_flight)?;
            return Err(ExploreExecutionPreparationError::Execution(format!(
                "cannot open durable exact Explore stream: {error}"
            )));
        }
    };
    let closed_cases_at_slice_start = coordinator.closed_case_count();

    if coordinator.stream().lifecycle() == run_stream::RunLifecycle::Sealed {
        let report = render_already_sealed_exact_stream(&coordinator, closed_cases_at_slice_start);
        finish_exact_stream_work(&mut resources, preparation_in_flight)?;
        return report;
    }
    let pending_observable_snapshot_on_resume = coordinator.pending_observable_snapshot_on_resume();
    finish_exact_stream_work(&mut resources, preparation_in_flight)?;

    // The journal is already the resume checkpoint, but a time-boxed slice may
    // have ended without enough admitted tail to mint its observer view. Give
    // that view first claim on the next invocation so repeated deadlines cannot
    // indefinitely hide otherwise durable progress.
    if pending_observable_snapshot_on_resume {
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::Explicit,
            ExploreStreamSliceStop::SnapshotCatchUp,
            0,
            closed_cases_at_slice_start,
        );
    }

    let mut singleton_cases_evaluated_this_slice = 0_u128;

    let probe_case_batch_cap =
        NonZeroU16::new(stream_coordinator::EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
            .expect("the first-generation source-probe batch cap is positive");
    while !coordinator.probe_phase_complete() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = resources.stop_at_work_boundary();
            return publish_or_defer_and_pause_exact_stream_slice(
                &mut coordinator,
                &mut resources,
                query,
                deadline,
                run_stream::PauseReason::TimeLimit,
                ExploreStreamSliceStop::TimeLimit,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        }

        match coordinator.probe_phase().map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot derive source-probe phase: {error}"
            ))
        })? {
            stream_probe::ExactSourceProbePhaseV1::Unprepared => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let probe_result = match source_proof_plan::prepare_source_proof_plan(
                    &artifacts,
                    selected,
                    source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
                ) {
                    Ok(plan) => coordinator
                        .persist_source_probe_manifest(&plan)
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    Err(error) if error.permits_canonical_fallback() => coordinator
                        .persist_probe_fallback_manifest()
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                finish_exact_stream_work(&mut resources, in_flight)?;
                probe_result.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::Prepared => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let coverage = coordinator
                    .accept_prepared_probe_coverage(NonZeroU64::new(1).expect("one is nonzero"))
                    .map_err(|error| error.to_string());
                finish_exact_stream_work(&mut resources, in_flight)?;
                coverage.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::CoverageAccepted => {
                let in_flight = match admit_exact_stream_work(
                    &mut resources,
                    stream_resource::ExactStreamWorkSubject::ProbePhase,
                    deadline,
                )? {
                    ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                    ExactStreamWorkAdmission::TimeLimit => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::TimeLimit,
                            ExploreStreamSliceStop::TimeLimit,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    ExactStreamWorkAdmission::ResourcePause(reason) => {
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::ResourcePressure,
                            ExploreStreamSliceStop::ResourcePressure {
                                detail: reason.code().to_string(),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                };
                let completion = coordinator
                    .complete_prepared_probe()
                    .map_err(|error| error.to_string());
                finish_exact_stream_work(&mut resources, in_flight)?;
                completion.map_err(ExploreExecutionPreparationError::Execution)?;
            }
            stream_probe::ExactSourceProbePhaseV1::CandidateActive => {
                let rank = coordinator
                    .next_probe_candidate_rank_hint()
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "active source-probe phase has no still-open discovered candidate"
                                .to_string(),
                        )
                    })?;
                let work_subject = stream_resource::ExactStreamWorkSubject::ProbeCandidateBatch {
                    first_rank: rank,
                    case_cap: probe_case_batch_cap,
                };
                let in_flight =
                    match admit_exact_stream_work(&mut resources, work_subject, deadline)? {
                        ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
                        ExactStreamWorkAdmission::TimeLimit => {
                            return publish_or_defer_and_pause_exact_stream_slice(
                                &mut coordinator,
                                &mut resources,
                                query,
                                deadline,
                                run_stream::PauseReason::TimeLimit,
                                ExploreStreamSliceStop::TimeLimit,
                                singleton_cases_evaluated_this_slice,
                                closed_cases_at_slice_start,
                            );
                        }
                        ExactStreamWorkAdmission::ResourcePause(reason) => {
                            return publish_or_defer_and_pause_exact_stream_slice(
                                &mut coordinator,
                                &mut resources,
                                query,
                                deadline,
                                run_stream::PauseReason::ResourcePressure,
                                ExploreStreamSliceStop::ResourcePressure {
                                    detail: reason.code().to_string(),
                                },
                                singleton_cases_evaluated_this_slice,
                                closed_cases_at_slice_start,
                            );
                        }
                    };
                if in_flight.subject() != work_subject
                    || in_flight.first_case_id_rank() != Some(rank)
                {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource governor admitted another source-probe candidate block"
                            .to_string(),
                    ));
                }
                let closed_cases_before_batch = coordinator.closed_case_count();
                let advance =
                    coordinator.advance_bounded_probe_candidate_batch(probe_case_batch_cap);
                finish_exact_stream_work(&mut resources, in_flight)?;
                match advance.map_err(|error| {
                    ExploreExecutionPreparationError::Execution(format!(
                        "cannot advance durable source-probe candidate block: {error}"
                    ))
                })? {
                    stream_coordinator::ExactProbeCandidateBatchAdvance::CandidatesComplete => {
                        continue;
                    }
                    stream_coordinator::ExactProbeCandidateBatchAdvance::Committed {
                        ranks,
                        canonical_blob_bytes,
                        closed_case_count,
                        stop,
                    } => {
                        let expected_closed_case_count = closed_cases_before_batch
                            .checked_add(ranks.len() as u128)
                            .ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "source-probe closed case count exceeds u128::MAX".to_string(),
                                )
                            })?;
                        if ranks.is_empty()
                            || canonical_blob_bytes == 0
                            || !ranks.contains(&rank)
                            || closed_case_count != expected_closed_case_count
                            || closed_case_count != coordinator.closed_case_count()
                        {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "source-probe candidate block returned inconsistent evidence"
                                    .to_string(),
                            ));
                        }
                        singleton_cases_evaluated_this_slice = singleton_cases_evaluated_this_slice
                            .checked_add(ranks.len() as u128)
                            .ok_or_else(|| {
                                ExploreExecutionPreparationError::Execution(
                                    "source-probe evaluated case count exceeds u128::MAX"
                                        .to_string(),
                                )
                            })?;
                        match stop {
                            stream_coordinator::ExactProbeCandidateBatchStop::CaseCapReached {
                                next_rank,
                            }
                            | stream_coordinator::ExactProbeCandidateBatchStop::ByteTargetReached {
                                next_rank,
                            } => {
                                if ranks.contains(&next_rank) {
                                    return Err(ExploreExecutionPreparationError::Execution(
                                        "source-probe block reports a committed rank as its next candidate"
                                            .to_string(),
                                    ));
                                }
                            }
                            stream_coordinator::ExactProbeCandidateBatchStop::CandidatesComplete => {
                            }
                            stream_coordinator::ExactProbeCandidateBatchStop::CaseOpen {
                                rank: open_rank,
                                reason,
                            } => {
                                if ranks.contains(&open_rank) {
                                    return Err(ExploreExecutionPreparationError::Execution(
                                        "source-probe block reports its limited candidate as committed"
                                            .to_string(),
                                    ));
                                }
                                return publish_or_defer_and_pause_exact_stream_slice(
                                    &mut coordinator,
                                    &mut resources,
                                    query,
                                    deadline,
                                    run_stream::PauseReason::EvaluationLimit,
                                    ExploreStreamSliceStop::EvaluationLimit {
                                        blocked_rank: open_rank,
                                        reason: public_stop(reason),
                                    },
                                    singleton_cases_evaluated_this_slice,
                                    closed_cases_at_slice_start,
                                );
                            }
                        }
                    }
                    stream_coordinator::ExactProbeCandidateBatchAdvance::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if open_rank != rank {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "source-probe evaluator blocked another rank than dispatched"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                }
            }
            stream_probe::ExactSourceProbePhaseV1::Complete => break,
        }
    }

    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        let _ = resources.stop_at_work_boundary();
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::TimeLimit,
            ExploreStreamSliceStop::TimeLimit,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    if options.pause_after == Some(ExploreStreamPauseAfter::Probes) {
        return publish_or_defer_and_pause_exact_stream_slice(
            &mut coordinator,
            &mut resources,
            query,
            deadline,
            run_stream::PauseReason::ProbeMilestone,
            ExploreStreamSliceStop::ProbeMilestone,
            singleton_cases_evaluated_this_slice,
            closed_cases_at_slice_start,
        );
    }

    let case_batch_cap =
        NonZeroU16::new(stream_coordinator::EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
            .expect("the first-generation exact-stream batch cap is positive");
    loop {
        let Some(rank) = coordinator.next_open_rank_hint() else {
            return finalize_or_pause_classification_closed_stream(
                &mut coordinator,
                &mut resources,
                query,
                options.finalize,
                deadline,
                singleton_cases_evaluated_this_slice,
                closed_cases_at_slice_start,
            );
        };
        let work_subject = stream_resource::ExactStreamWorkSubject::BoundedCaseIdBatch {
            first_rank: rank,
            case_cap: case_batch_cap,
        };
        let in_flight = match admit_exact_stream_work(&mut resources, work_subject, deadline)? {
            ExactStreamWorkAdmission::Granted(in_flight) => in_flight,
            ExactStreamWorkAdmission::TimeLimit => {
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::TimeLimit,
                    ExploreStreamSliceStop::TimeLimit,
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
            ExactStreamWorkAdmission::ResourcePause(reason) => {
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::ResourcePressure,
                    ExploreStreamSliceStop::ResourcePressure {
                        detail: reason.code().to_string(),
                    },
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
        };
        if in_flight.subject() != work_subject || in_flight.first_case_id_rank() != Some(rank) {
            return Err(ExploreExecutionPreparationError::Execution(
                "resource governor began another bounded CaseId block than the coordinator scheduled"
                    .to_string(),
            ));
        }
        let closed_cases_before_batch = coordinator.closed_case_count();
        let advance = coordinator.advance_bounded_case_batch(case_batch_cap);
        finish_exact_stream_work(&mut resources, in_flight)?;
        match advance.map_err(|error| {
            ExploreExecutionPreparationError::Execution(format!(
                "cannot advance durable exact Explore evidence block: {error}"
            ))
        })? {
            stream_coordinator::ExactStreamBatchAdvance::Committed {
                ranks,
                canonical_blob_bytes,
                closed_case_count,
                stop,
            } => {
                let expected_closed_case_count = closed_cases_before_batch
                    .checked_add(ranks.len() as u128)
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "committed exact-stream closed case count exceeds u128::MAX"
                                .to_string(),
                        )
                    })?;
                if ranks.is_empty()
                    || canonical_blob_bytes == 0
                    || !ranks.contains(&rank)
                    || closed_case_count != expected_closed_case_count
                    || closed_case_count != coordinator.closed_case_count()
                {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource-bound CaseId block returned inconsistent committed evidence"
                            .to_string(),
                    ));
                }
                singleton_cases_evaluated_this_slice = singleton_cases_evaluated_this_slice
                    .checked_add(ranks.len() as u128)
                    .ok_or_else(|| {
                        ExploreExecutionPreparationError::Execution(
                            "committed exact-stream case count exceeds u128::MAX".to_string(),
                        )
                    })?;
                match stop {
                    stream_coordinator::ExactStreamBatchStop::CaseCapReached { next_rank }
                    | stream_coordinator::ExactStreamBatchStop::ByteTargetReached { next_rank } => {
                        if ranks.contains(&next_rank) {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "bounded exact evidence block reports a committed rank as its next open CaseId"
                                    .to_string(),
                            ));
                        }
                    }
                    stream_coordinator::ExactStreamBatchStop::CaseOpen {
                        rank: open_rank,
                        reason,
                    } => {
                        if ranks.contains(&open_rank) {
                            return Err(ExploreExecutionPreparationError::Execution(
                                "bounded exact evidence block reports its evaluation-limited CaseId as committed"
                                    .to_string(),
                            ));
                        }
                        return publish_or_defer_and_pause_exact_stream_slice(
                            &mut coordinator,
                            &mut resources,
                            query,
                            deadline,
                            run_stream::PauseReason::EvaluationLimit,
                            ExploreStreamSliceStop::EvaluationLimit {
                                blocked_rank: open_rank,
                                reason: public_stop(reason),
                            },
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                    stream_coordinator::ExactStreamBatchStop::ClassificationClosedFinalizationPending => {
                        return finalize_or_pause_classification_closed_stream(
                            &mut coordinator,
                            &mut resources,
                            query,
                            options.finalize,
                            deadline,
                            singleton_cases_evaluated_this_slice,
                            closed_cases_at_slice_start,
                        );
                    }
                }
            }
            stream_coordinator::ExactStreamBatchAdvance::CaseOpen {
                rank: open_rank,
                reason,
            } => {
                if open_rank != rank {
                    return Err(ExploreExecutionPreparationError::Execution(
                        "resource-bound CaseId disagrees with the open exact rank".to_string(),
                    ));
                }
                return publish_or_defer_and_pause_exact_stream_slice(
                    &mut coordinator,
                    &mut resources,
                    query,
                    deadline,
                    run_stream::PauseReason::EvaluationLimit,
                    ExploreStreamSliceStop::EvaluationLimit {
                        blocked_rank: open_rank,
                        reason: public_stop(reason),
                    },
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
            stream_coordinator::ExactStreamBatchAdvance::ClassificationClosedFinalizationPending => {
                return finalize_or_pause_classification_closed_stream(
                    &mut coordinator,
                    &mut resources,
                    query,
                    options.finalize,
                    deadline,
                    singleton_cases_evaluated_this_slice,
                    closed_cases_at_slice_start,
                );
            }
        }
    }
}

}

/// One named, canonical value in the retired exhaustive developer-preview ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg(any())]
pub struct ExplorePreviewField {
    pub name: String,
    pub value: ExploreValue,
}

/// One matching complete assignment evaluated by the ordinary interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(any())]
pub struct ExplorePreviewRow {
    pub inputs: Vec<ExplorePreviewField>,
    pub key: Vec<ExplorePreviewField>,
    pub shown: Vec<ExplorePreviewField>,
}

/// Exact-finite result used only by the hidden `__explore-preview` command.
/// It is intentionally smaller than the accepted public Explore report RFC.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(any())]
pub struct ExplorePreviewReport {
    pub query_name: String,
    pub polarity: ExplorePolarity,
    pub declared_assignments: u64,
    pub eligible_configurations: u64,
    pub evaluated_configurations: u64,
    pub matching_configurations: u64,
    pub distinct_keys: u64,
    pub rows: Vec<ExplorePreviewRow>,
}

#[derive(Debug, Clone)]
struct SourcedBinding {
    expression: Expr,
    annotated_ty: Option<Ty>,
    origin: String,
}

#[derive(Debug, Clone)]
struct SourcedFunction {
    params: Vec<Param>,
    return_ty: Option<Ty>,
    effects: Vec<String>,
    body: Expr,
    origin: String,
}

#[derive(Debug, Clone, Default)]
struct GroundDefinitions {
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    functions: BTreeMap<(String, usize), Vec<SourcedFunction>>,
    rules: BTreeMap<(String, usize), Vec<String>>,
    rule_definitions: BTreeMap<(String, usize), Vec<Rule>>,
    constructors: BTreeMap<(String, usize), Vec<String>>,
    unsupported_callables: BTreeMap<(String, usize), Vec<String>>,
    unsupported_values: BTreeMap<String, Vec<String>>,
    origin_order: BTreeMap<String, usize>,
    runtime_declarations: Vec<Stmt>,
    rule_dispatch_return_types: BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_return_issues: BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_boolean_miss_safe_keys: BTreeSet<RuleDispatchKey>,
    explore_rule_return_types_by_arity: BTreeMap<(String, usize), Ty>,
    explore_rule_return_issues: BTreeMap<(String, usize), String>,
}

#[derive(Debug)]
struct ExploreGroundEvaluator<'a> {
    catalog: &'a calculate::TypeCatalog,
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    functions: BTreeMap<(String, usize), Vec<SourcedFunction>>,
    rules: BTreeMap<(String, usize), Vec<String>>,
    constructors: BTreeMap<(String, usize), Vec<String>>,
    unsupported_callables: BTreeMap<(String, usize), Vec<String>>,
    unsupported_values: BTreeMap<String, Vec<String>>,
    origin_order: BTreeMap<String, usize>,
    origin_stack: Vec<String>,
    locals: BTreeMap<String, ExploreValue>,
    memo: BTreeMap<String, ExploreValue>,
    memo_order: Vec<String>,
    visiting: Vec<String>,
    visiting_calls: Vec<(String, usize)>,
    work_remaining: u64,
}

impl<'a> ExploreGroundEvaluator<'a> {
    fn new(catalog: &'a calculate::TypeCatalog, definitions: GroundDefinitions) -> Self {
        Self {
            catalog,
            bindings: definitions.bindings,
            functions: definitions.functions,
            rules: definitions.rules,
            constructors: definitions.constructors,
            unsupported_callables: definitions.unsupported_callables,
            unsupported_values: definitions.unsupported_values,
            origin_order: definitions.origin_order,
            origin_stack: Vec::new(),
            locals: BTreeMap::new(),
            memo: BTreeMap::new(),
            memo_order: Vec::new(),
            visiting: Vec::new(),
            visiting_calls: Vec::new(),
            work_remaining: EXPLORE_GROUND_WORK_LIMIT,
        }
    }

    fn charge_work(&mut self, amount: u64, operation: &str) -> Result<(), String> {
        let Some(remaining) = self.work_remaining.checked_sub(amount) else {
            return Err(format!(
                "ground exploration {} exceeds the checked work limit {}",
                operation, EXPLORE_GROUND_WORK_LIMIT
            ));
        };
        self.work_remaining = remaining;
        Ok(())
    }

    fn charge_value_clone(&mut self, value: &ExploreValue, operation: &str) -> Result<(), String> {
        self.charge_work(
            explore_value_node_count(value, self.work_remaining),
            operation,
        )
    }

    fn ensure_origin_visible(&self, target: &str, symbol: &str) -> Result<(), String> {
        let Some(current) = self.origin_stack.last() else {
            return Ok(());
        };
        let current_order = self
            .origin_order
            .get(current)
            .copied()
            .unwrap_or(usize::MAX);
        let target_order = self.origin_order.get(target).copied().unwrap_or(usize::MAX);
        if target_order > current_order {
            return Err(format!(
                "ground exploration declaration from `{}` depends on later declaration `{}` from `{}`; imported finite data must be closed over its initialized dependency prefix",
                current, symbol, target
            ));
        }
        Ok(())
    }

    fn set_local(&mut self, name: impl Into<String>, value: ExploreValue) {
        self.locals.insert(name.into(), value);
    }

    fn eval(&mut self, expression: &Expr, expected: Option<&Ty>) -> Result<ExploreValue, String> {
        self.charge_work(1, "expression evaluation")?;
        match &expression.kind {
            ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
            ExprKind::Lit(Literal::Float(value)) => Ok(ExploreValue::FloatBits(value.to_bits())),
            ExprKind::Lit(Literal::Str(value)) => Ok(ExploreValue::String(value.clone())),
            ExprKind::Lit(Literal::Char(value)) => Ok(ExploreValue::Character(*value)),
            ExprKind::Lit(Literal::Bool(value)) => Ok(ExploreValue::Boolean(*value)),
            ExprKind::Unit => Ok(ExploreValue::Unit),
            ExprKind::List(items) => {
                if items.len() > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                    return Err(format!(
                        "ground list literal exceeds materialization limit {}",
                        EXPLORE_GROUND_COLLECTION_LIMIT
                    ));
                }
                self.charge_work(items.len() as u64, "list materialization")?;
                let item_ty = collection_item_ty(expected);
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item, item_ty.as_ref())?);
                }
                Ok(ExploreValue::List(values))
            }
            ExprKind::Tuple(items) => {
                self.charge_work(items.len() as u64, "tuple materialization")?;
                let item_tys = tuple_item_tys(expected);
                if item_tys
                    .as_ref()
                    .is_some_and(|types| types.len() != items.len())
                {
                    return Err(format!(
                        "ground tuple has {} elements but expected type `{}` has {}",
                        items.len(),
                        expected.expect("tuple types were present"),
                        item_tys.as_ref().map_or(0, Vec::len)
                    ));
                }
                let mut values = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    values.push(
                        self.eval(item, item_tys.as_ref().and_then(|types| types.get(index)))?,
                    );
                }
                Ok(ExploreValue::Tuple(values))
            }
            ExprKind::Var(name) => {
                if self.locals.contains_key(name) {
                    let nodes = explore_value_node_count(
                        self.locals.get(name).expect("checked local"),
                        self.work_remaining,
                    );
                    self.charge_work(nodes, "local value copy")?;
                    return Ok(self.locals.get(name).expect("checked local").clone());
                }
                if self.bindings.contains_key(name) && self.unsupported_values.contains_key(name) {
                    return Err(format!(
                        "ground exploration name `{}` has both an ordinary binding and a runtime value declaration; exact resolution is ambiguous",
                        name
                    ));
                }
                if self.bindings.contains_key(name) {
                    return self.eval_binding(name, expected);
                }
                if let Some(origins) = self.unsupported_values.get(name) {
                    return Err(format!(
                        "ground exploration name `{}` is shadowed by a runtime value declared in {}",
                        name,
                        origins.join(", ")
                    ));
                }
                let function_count = self
                    .functions
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, definitions)| definitions.len())
                    .sum::<usize>();
                let rule_count = self
                    .rules
                    .keys()
                    .filter(|(candidate, _)| candidate == name)
                    .count();
                let unsupported_count = self
                    .unsupported_callables
                    .keys()
                    .filter(|(candidate, _)| candidate == name)
                    .count();
                if function_count > 0 || rule_count > 0 || unsupported_count > 0 {
                    return Err(format!(
                        "ground exploration name `{}` is ambiguous between a bare value/constructor and a callable declaration",
                        name
                    ));
                }
                let constructor_count = self
                    .constructors
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, origins)| origins.len())
                    .sum::<usize>();
                if constructor_count > 1 {
                    return Err(format!(
                        "ground exploration constructor `{}` has {} visible declarations and cannot identify one exact value",
                        name, constructor_count
                    ));
                }
                if let Some(expected) = expected {
                    if let Some(value) = self.eval_nullary_constructor(expected, name)? {
                        return Ok(value);
                    }
                }
                Err(format!("unresolved ground name `{}`", name))
            }
            ExprKind::Field(receiver, field) => {
                let receiver = self.eval(receiver, None)?;
                let value = match receiver {
                    ExploreValue::Constructor {
                        positional: false,
                        fields,
                        ..
                    } => fields
                        .iter()
                        .find_map(|(name, value)| (name == field).then(|| value.clone())),
                    _ => None,
                };
                value
                    .ok_or_else(|| format!("ground exploration value has no named field `{field}`"))
            }
            ExprKind::UnOp(operator, value) => {
                let value = self.eval(value, expected)?;
                match (operator.as_str(), value) {
                    ("-", ExploreValue::Int(value)) => {
                        value.checked_neg().map(ExploreValue::Int).ok_or_else(|| {
                            "integer negation overflow in exploration bound".to_string()
                        })
                    }
                    ("-", ExploreValue::FloatBits(bits)) => {
                        Ok(ExploreValue::FloatBits((-f64::from_bits(bits)).to_bits()))
                    }
                    ("+", ExploreValue::Int(value)) => Ok(ExploreValue::Int(value)),
                    ("!", ExploreValue::Boolean(value)) => Ok(ExploreValue::Boolean(!value)),
                    _ => Err(format!(
                        "unsupported unary operator `{}` in ground exploration expression",
                        operator
                    )),
                }
            }
            ExprKind::BinOp(operator, left, right) => {
                let left = self.eval(left, None)?;
                let right = self.eval(right, None)?;
                eval_ground_binary(operator, left, right)
            }
            ExprKind::If(condition, then_value, else_value) => {
                match self.eval(condition, Some(&Ty::Name("Bool".to_string())))? {
                    ExploreValue::Boolean(true) => self.eval(then_value, expected),
                    ExploreValue::Boolean(false) => self.eval(else_value, expected),
                    _ => Err("ground exploration `if` condition is not Boolean".to_string()),
                }
            }
            ExprKind::Block(statements) => self.eval_block(statements, expected),
            ExprKind::App(function, arguments) => {
                let ExprKind::Var(name) = &function.kind else {
                    return Err(
                        "qualified or computed calls are not exact ground domain expressions"
                            .to_string(),
                    );
                };
                if self.locals.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a local value; expose an unambiguous pure helper or literal finite collection",
                        name
                    ));
                }
                if self.bindings.contains_key(name) && self.unsupported_values.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` has both an ordinary binding and a runtime value declaration; exact resolution is ambiguous",
                        name
                    ));
                }
                if self.bindings.contains_key(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a top-level binding; expose an unambiguous pure helper or literal finite collection",
                        name
                    ));
                }
                if let Some(origins) = self.unsupported_values.get(name) {
                    return Err(format!(
                        "ground exploration call `{}` is shadowed by a runtime value declared in {}; expose an unambiguous pure helper or literal finite collection",
                        name,
                        origins.join(", ")
                    ));
                }
                let function_key = (name.clone(), arguments.len());
                let function_count = self
                    .functions
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .map(|(_, definitions)| definitions.len())
                    .sum::<usize>();
                let has_function = self.functions.contains_key(&function_key);
                let constructor_origins = self
                    .constructors
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                let unsupported_origins = self
                    .unsupported_callables
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                if !unsupported_origins.is_empty() {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves to an unsupported callable from {}; expose an unambiguous pure top-level `>` helper or literal finite collection",
                        name,
                        arguments.len(),
                        unsupported_origins.join(", ")
                    ));
                }
                if let Some(origins) = self
                    .rules
                    .iter()
                    .filter(|((candidate, _), _)| candidate == name)
                    .flat_map(|(_, origins)| origins.iter())
                    .cloned()
                    .reduce(|mut joined, origin| {
                        joined.push_str(", ");
                        joined.push_str(&origin);
                        joined
                    })
                {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves to a rule from {}; expose an unambiguous pure `>` helper or literal finite collection",
                        name,
                        arguments.len(),
                        origins
                    ));
                }
                if ground_intrinsic_arity(name).is_some() && function_count > 0 {
                    return Err(format!(
                        "ground exploration intrinsic `{}` is shadowed by a program function; exact import-time call resolution is ambiguous",
                        name
                    ));
                }
                if has_function && !constructor_origins.is_empty() {
                    return Err(format!(
                        "ground exploration call `{}` is ambiguous between a function and constructor declared in {}; expose an unambiguous pure helper",
                        name,
                        constructor_origins.join(", ")
                    ));
                }
                if has_function && function_count != 1 {
                    return Err(format!(
                        "ground exploration helper `{}` has {} declarations across arities; exact runtime resolution is ambiguous",
                        name, function_count
                    ));
                }
                if has_function {
                    return self.eval_function(name, arguments, expected);
                }
                if function_count > 0 {
                    return Err(format!(
                        "ground exploration call `{}({} arguments)` resolves by name to a function declared with a different arity; exact runtime resolution is ambiguous",
                        name,
                        arguments.len()
                    ));
                }
                let is_intrinsic = ground_intrinsic_arity(name) == Some(arguments.len());
                if is_intrinsic && !constructor_origins.is_empty() {
                    return Err(format!(
                        "ground exploration intrinsic `{}({} arguments)` is shadowed by a constructor declared in {}; expose an unambiguous literal finite collection",
                        name,
                        arguments.len(),
                        constructor_origins.join(", ")
                    ));
                }
                if name == "range" && arguments.len() == 2 {
                    let int_ty = Ty::Name("Int".to_string());
                    let start = self
                        .eval(&arguments[0], Some(&int_ty))?
                        .int()
                        .ok_or_else(|| "ground `range` start is not an Int".to_string())?;
                    let end_exclusive = self
                        .eval(&arguments[1], Some(&int_ty))?
                        .int()
                        .ok_or_else(|| "ground `range` end is not an Int".to_string())?;
                    let cardinality = exact_range_cardinality(start, end_exclusive)?;
                    if cardinality > EXPLORE_GROUND_COLLECTION_LIMIT {
                        return Err(format!(
                            "ground `range({}, {})` has {} members, exceeding materialization limit {}; use `range` directly as the exploration domain",
                            start, end_exclusive, cardinality, EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(cardinality, "range materialization")?;
                    let values = (0..cardinality)
                        .map(|offset| ExploreValue::Int((start as i128 + offset as i128) as i64))
                        .collect();
                    return Ok(ExploreValue::List(values));
                }
                if name == "set_from_list" && arguments.len() == 1 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_from_list` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let item_ty = collection_item_ty(expected).ok_or_else(|| {
                        "`set_from_list` ground domain needs an expected `Set(T)` type".to_string()
                    })?;
                    let list_ty = Ty::App(Box::new(Ty::Name("List".to_string())), vec![item_ty]);
                    let ExploreValue::List(values) = self.eval(&arguments[0], Some(&list_ty))?
                    else {
                        return Err("`set_from_list` argument is not a finite list".to_string());
                    };
                    self.charge_work(values.len() as u64, "set construction")?;
                    return Ok(ExploreValue::Set(runtime_set_values(values)));
                }
                if name == "set_new" && arguments.is_empty() {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err("`set_new` ground result must have type `Set(T)`".to_string());
                    }
                    return Ok(ExploreValue::Set(Vec::new()));
                }
                if name == "concat" && arguments.len() == 2 {
                    let ExploreValue::List(mut left) = self.eval(&arguments[0], expected)? else {
                        return Err("`concat` left argument is not a finite list".to_string());
                    };
                    let ExploreValue::List(right) = self.eval(&arguments[1], expected)? else {
                        return Err("`concat` right argument is not a finite list".to_string());
                    };
                    let size = left
                        .len()
                        .checked_add(right.len())
                        .ok_or_else(|| "ground `concat` collection size overflow".to_string())?;
                    if size > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                        return Err(format!(
                            "ground `concat` has {} members, exceeding materialization limit {}",
                            size, EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(size as u64, "concat materialization")?;
                    left.extend(right);
                    return Ok(ExploreValue::List(left));
                }
                if name == "distinct" && arguments.len() == 1 {
                    let ExploreValue::List(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`distinct` argument is not a finite list".to_string());
                    };
                    self.charge_work(values.len() as u64, "distinct traversal")?;
                    return Ok(ExploreValue::List(deduplicate_runtime_list(values)));
                }
                if name == "set_insert" && arguments.len() == 2 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_insert` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let ExploreValue::Set(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`set_insert` first argument is not a finite set".to_string());
                    };
                    let item_ty = collection_item_ty(expected);
                    let inserted = self.eval(&arguments[1], item_ty.as_ref())?;
                    let mut values = runtime_set_map(values);
                    values
                        .entry(inserted.runtime_display_key())
                        .or_insert(inserted);
                    if values.len() > EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                        return Err(format!(
                            "ground `set_insert` has {} members, exceeding materialization limit {}",
                            values.len(),
                            EXPLORE_GROUND_COLLECTION_LIMIT
                        ));
                    }
                    self.charge_work(values.len() as u64, "set insertion")?;
                    return Ok(ExploreValue::Set(values.into_values().collect()));
                }
                if name == "set_remove" && arguments.len() == 2 {
                    if !matches!(expected.and_then(collection_kind), Some("Set")) {
                        return Err(
                            "`set_remove` ground result must have type `Set(T)`".to_string()
                        );
                    }
                    let ExploreValue::Set(values) = self.eval(&arguments[0], expected)? else {
                        return Err("`set_remove` first argument is not a finite set".to_string());
                    };
                    let item_ty = collection_item_ty(expected);
                    let removed = self.eval(&arguments[1], item_ty.as_ref())?;
                    self.charge_work(values.len() as u64, "set removal traversal")?;
                    let mut values = runtime_set_map(values);
                    values.remove(&removed.runtime_display_key());
                    return Ok(ExploreValue::Set(values.into_values().collect()));
                }
                self.eval_constructor(expected, name, arguments)
            }
            _ => Err(format!(
                "unsupported ground exploration expression: {:?}",
                expression.kind
            )),
        }
    }

    fn eval_block(
        &mut self,
        statements: &[Stmt],
        expected: Option<&Ty>,
    ) -> Result<ExploreValue, String> {
        let mut shadowed = Vec::new();
        let result = (|| {
            let mut result = ExploreValue::Unit;
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    Stmt::Bind(Pat::Var(name), ty, expression) => {
                        let value = self.eval(expression, ty.as_ref())?;
                        let previous = self.locals.insert(name.clone(), value);
                        shadowed.push((name.clone(), previous));
                        result = ExploreValue::Unit;
                    }
                    Stmt::Expr(expression) if index + 1 == statements.len() => {
                        result = self.eval(expression, expected)?;
                    }
                    Stmt::Expr(expression) => {
                        self.eval(expression, None)?;
                        result = ExploreValue::Unit;
                    }
                    _ => {
                        return Err(
                            "ground exploration helper blocks support only pure bindings and expressions"
                                .to_string(),
                        );
                    }
                }
            }
            Ok(result)
        })();
        for (name, previous) in shadowed.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        result
    }

    fn eval_function(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<&Ty>,
    ) -> Result<ExploreValue, String> {
        let key = (name.to_string(), arguments.len());
        let definition_count = self
            .functions
            .iter()
            .filter(|((candidate, _), _)| candidate == name)
            .map(|(_, definitions)| definitions.len())
            .sum::<usize>();
        if definition_count != 1 {
            return Err(format!(
                "ground exploration helper `{}` has {} declarations across arities; exact runtime resolution is ambiguous",
                name, definition_count
            ));
        }
        let definitions = self.functions.get(&key).cloned().unwrap_or_default();
        if definitions.len() != 1 {
            return Err(format!(
                "ground exploration helper `{}({} arguments)` has {} definitions",
                name,
                arguments.len(),
                definitions.len()
            ));
        }
        let definition = &definitions[0];
        self.ensure_origin_visible(&definition.origin, name)?;
        if self.origin_stack.len() >= EXPLORE_GROUND_RECURSION_LIMIT {
            return Err(format!(
                "ground exploration helper recursion exceeds the safe depth limit {}",
                EXPLORE_GROUND_RECURSION_LIMIT
            ));
        }
        if !definition.effects.is_empty() {
            return Err(format!(
                "ground exploration helper `{}({} arguments)` declares effects",
                name,
                arguments.len()
            ));
        }
        if let Some(start) = self
            .visiting_calls
            .iter()
            .position(|candidate| candidate == &key)
        {
            let mut cycle = self.visiting_calls[start..]
                .iter()
                .map(|(name, arity)| format!("{}({})", name, arity))
                .collect::<Vec<_>>();
            cycle.push(format!("{}({})", name, arguments.len()));
            return Err(format!(
                "recursive ground exploration helper call: {}",
                cycle.join(" -> ")
            ));
        }
        let mut values = Vec::with_capacity(arguments.len());
        for (argument, parameter) in arguments.iter().zip(&definition.params) {
            values.push(self.eval(argument, parameter.ty.as_ref())?);
        }
        let mut shadowed = Vec::new();
        for (parameter, value) in definition.params.iter().zip(values) {
            let previous = self.locals.insert(parameter.name.clone(), value);
            shadowed.push((parameter.name.clone(), previous));
        }
        self.visiting_calls.push(key);
        self.origin_stack.push(definition.origin.clone());
        let result = self.eval(&definition.body, definition.return_ty.as_ref().or(expected));
        self.origin_stack.pop();
        self.visiting_calls.pop();
        for (name, previous) in shadowed.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        result.map_err(|message| {
            format!(
                "ground exploration helper `{}` from {} failed: {}",
                name, definition.origin, message
            )
        })
    }

    fn eval_binding(&mut self, name: &str, expected: Option<&Ty>) -> Result<ExploreValue, String> {
        let definitions = self
            .bindings
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unresolved ground binding `{}`", name))?;
        if definitions.len() != 1 {
            let origins = definitions
                .iter()
                .map(|definition| definition.origin.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "ground exploration binding `{}` has {} definitions ({})",
                name,
                definitions.len(),
                origins
            ));
        }
        let definition = &definitions[0];
        self.ensure_origin_visible(&definition.origin, name)?;
        if self.memo.contains_key(name) {
            let nodes = explore_value_node_count(
                self.memo.get(name).expect("checked memoized binding"),
                self.work_remaining,
            );
            self.charge_work(nodes, "memoized binding copy")?;
            return Ok(self
                .memo
                .get(name)
                .expect("checked memoized binding")
                .clone());
        }
        if let Some(start) = self.visiting.iter().position(|candidate| candidate == name) {
            let mut cycle = self.visiting[start..].to_vec();
            cycle.push(name.to_string());
            return Err(format!(
                "cyclic ground exploration binding dependency: {}",
                cycle.join(" -> ")
            ));
        }
        if self.origin_stack.len() >= EXPLORE_GROUND_RECURSION_LIMIT {
            return Err(format!(
                "ground exploration binding recursion exceeds the safe depth limit {}",
                EXPLORE_GROUND_RECURSION_LIMIT
            ));
        }
        self.visiting.push(name.to_string());
        let expected = definition.annotated_ty.as_ref().or(expected);
        let saved_locals = std::mem::take(&mut self.locals);
        self.origin_stack.push(definition.origin.clone());
        let value = self.eval(&definition.expression, expected);
        self.origin_stack.pop();
        self.locals = saved_locals;
        self.visiting.pop();
        let value = value?;
        self.charge_value_clone(&value, "binding memoization")?;
        self.memo.insert(name.to_string(), value.clone());
        self.memo_order.push(name.to_string());
        Ok(value)
    }

    fn eval_nullary_constructor(
        &self,
        expected: &Ty,
        constructor: &str,
    ) -> Result<Option<ExploreValue>, String> {
        let Some((type_name, substitutions)) = instantiated_named_type(expected, self.catalog)?
        else {
            return Ok(None);
        };
        if self.catalog.is_rule_scope(&type_name) {
            return Err(format!(
                "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
                type_name
            ));
        }
        let constructor_origins = self
            .constructors
            .get(&(constructor.to_string(), 0))
            .cloned()
            .unwrap_or_default();
        if constructor_origins.len() == 1 {
            self.ensure_origin_visible(&constructor_origins[0], constructor)?;
        }
        for variant in self.catalog.resolved_variants(&type_name)? {
            if variant.name == constructor && variant.fields.is_empty() {
                return Ok(Some(ExploreValue::Constructor {
                    type_name,
                    variant: constructor.to_string(),
                    // Bare nullary names always evaluate as positional
                    // Value::Constructor, even when an explicit `Foo()` call
                    // uses the declaration's named-constructor shape.
                    positional: true,
                    fields: Arc::from([]),
                }));
            }
        }
        let _ = substitutions;
        Ok(None)
    }

    fn eval_constructor(
        &mut self,
        expected: Option<&Ty>,
        constructor: &str,
        arguments: &[Expr],
    ) -> Result<ExploreValue, String> {
        let expected = expected.ok_or_else(|| {
            format!(
                "constructor `{}` in a ground domain needs an expected declared type",
                constructor
            )
        })?;
        let Some((type_name, substitutions)) = instantiated_named_type(expected, self.catalog)?
        else {
            return Err(format!(
                "constructor `{}` cannot inhabit primitive type `{}`",
                constructor, expected
            ));
        };
        if self.catalog.is_rule_scope(&type_name) {
            return Err(format!(
                "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
                type_name
            ));
        }
        let constructor_origins = self
            .constructors
            .get(&(constructor.to_string(), arguments.len()))
            .cloned()
            .unwrap_or_default();
        if constructor_origins.len() == 1 {
            self.ensure_origin_visible(&constructor_origins[0], constructor)?;
        }
        let variant = self
            .catalog
            .resolved_variants(&type_name)?
            .into_iter()
            .find(|variant| variant.name == constructor)
            .ok_or_else(|| {
                format!(
                    "type `{}` has no constructor `{}` in ground exploration domain",
                    expected, constructor
                )
            })?;
        if variant.fields.len() != arguments.len() {
            return Err(format!(
                "constructor `{}` expects {} fields but got {}",
                constructor,
                variant.fields.len(),
                arguments.len()
            ));
        }
        let mut values = Vec::with_capacity(arguments.len());
        if arguments
            .iter()
            .any(|argument| named_arg_parts(argument).is_some())
        {
            for field in &variant.fields {
                let argument = arguments
                    .iter()
                    .find_map(|argument| {
                        named_arg_parts(argument)
                            .filter(|(name, _)| *name == field.name)
                            .map(|(_, value)| value)
                    })
                    .ok_or_else(|| {
                        format!(
                            "constructor `{}` is missing field `{}`",
                            constructor, field.name
                        )
                    })?;
                let field_ty = calculate::substitute_type(&field.ty, &substitutions);
                values.push((field.name.clone(), self.eval(argument, Some(&field_ty))?));
            }
        } else {
            for (field, argument) in variant.fields.iter().zip(arguments) {
                let field_ty = calculate::substitute_type(&field.ty, &substitutions);
                values.push((field.name.clone(), self.eval(argument, Some(&field_ty))?));
            }
        }
        Ok(ExploreValue::Constructor {
            type_name,
            variant: variant.name,
            // A nullary variant has one semantic inhabitant.  Futuruna's
            // runtime happens to represent bare `Foo` and explicit `Foo()`
            // with different constructor layouts, but that layout detail
            // must not create two exploration-domain values.
            positional: variant.fields.is_empty() || variant.positional,
            fields: values.into(),
        })
    }
}

struct ExploreRuntimeGroundEvaluator {
    interpreter: Interpreter,
    observer_memo_enabled: bool,
    expression_step_limit: usize,
    base_env: Env,
    bindings: BTreeMap<String, Vec<SourcedBinding>>,
    evaluated_bindings: BTreeSet<String>,
    locals: BTreeMap<String, Value>,
}

fn evaluate_relational_expression_with_bounded_retry<T>(
    step_limit: &mut usize,
    mut evaluate: impl FnMut(usize) -> Result<T, ExploreRuntimeFailure>,
) -> Result<T, ExploreRuntimeFailure> {
    // A relational expression is the smallest semantic retry unit. The
    // interpreter resets its counter for every attempt, while the checked
    // observer memo may retain exact values and their original step charges.
    // Growing one invocation-local high-water mark therefore changes only
    // scheduling cost: no partial result is accepted, and every expression
    // remains bounded by the hard ceiling below.
    loop {
        let attempted_limit = *step_limit;
        match evaluate(attempted_limit) {
            Err(ExploreRuntimeFailure::OperationalLimit {
                resource: ExploreRuntimeResource::ExpressionSteps,
                limit,
                ..
            }) if limit == attempted_limit as u128
                && attempted_limit < RELATIONAL_EXPRESSION_HARD_STEP_LIMIT =>
            {
                *step_limit = attempted_limit
                    .saturating_mul(2)
                    .min(RELATIONAL_EXPRESSION_HARD_STEP_LIMIT);
            }
            result => return result,
        }
    }
}

impl ExploreRuntimeGroundEvaluator {
    fn new(definitions: &GroundDefinitions) -> Self {
        Self::new_with_observer_memo(definitions, None)
    }

    fn new_with_observer_memo(
        definitions: &GroundDefinitions,
        observer_memo_plan: Option<CheckedExactObserverMemoPlan>,
    ) -> Self {
        let prelude = parse_prelude();
        let mut interpreter = Interpreter::new();
        interpreter.suppress_output = true;
        interpreter.install_rule_dispatch_return_metadata(
            &definitions.rule_dispatch_return_types,
            &definitions.rule_dispatch_return_issues,
            &definitions.rule_dispatch_boolean_miss_safe_keys,
        );
        let mut base_env = interpreter.default_env();
        // Register the same prelude-then-program order without first cloning
        // every large imported declaration into a temporary concatenated AST.
        interpreter.register_static_declarations(&prelude, &mut base_env);
        interpreter.register_static_declarations(&definitions.runtime_declarations, &mut base_env);
        interpreter.seal_exact_exploration_static_declarations();
        let observer_memo_enabled = observer_memo_plan
            .and_then(|plan| {
                interpreter
                    .install_checked_exact_observer_memo_plan(plan)
                    .ok()
            })
            .is_some();
        Self {
            interpreter,
            observer_memo_enabled,
            expression_step_limit: RELATIONAL_EXPRESSION_INITIAL_STEP_LIMIT,
            base_env,
            bindings: definitions.bindings.clone(),
            evaluated_bindings: BTreeSet::new(),
            locals: BTreeMap::new(),
        }
    }

    fn set_local(&mut self, name: impl Into<String>, value: Value) {
        self.locals.insert(name.into(), value);
    }

    fn clear_locals(&mut self) {
        self.locals.clear();
    }

    fn observer_memo_stats(&self) -> (bool, CheckedExactObserverMemoStats) {
        (
            self.observer_memo_enabled,
            self.interpreter.checked_exact_observer_memo_stats(),
        )
    }

    fn evaluate_required_bindings(&mut self, order: &[String]) -> Result<(), String> {
        for name in order {
            if self.evaluated_bindings.contains(name) {
                continue;
            }
            let Some(definitions) = self.bindings.get(name) else {
                return Err(format!(
                    "ground exploration binding `{}` disappeared from the checked declaration graph",
                    name
                ));
            };
            if definitions.len() != 1 {
                return Err(format!(
                    "ground exploration binding `{}` has {} definitions",
                    name,
                    definitions.len()
                ));
            }
            let value = evaluate_relational_expression_with_bounded_retry(
                &mut self.expression_step_limit,
                |step_limit| {
                    self.interpreter.eval_exact_exploration(
                        &definitions[0].expression,
                        &self.base_env,
                        step_limit,
                        EXPLORE_GROUND_COLLECTION_LIMIT as usize,
                    )
                },
            )
            .map_err(|failure| failure.to_string())?;
            self.base_env.set(name.clone(), value);
            self.evaluated_bindings.insert(name.clone());
        }
        Ok(())
    }

    fn eval(&mut self, expression: &Expr, binding_order: &[String]) -> Result<Value, String> {
        self.evaluate_required_bindings(binding_order)?;
        let mut env = self.base_env.child();
        for (name, value) in &self.locals {
            env.set(name.clone(), value.clone());
        }
        evaluate_relational_expression_with_bounded_retry(
            &mut self.expression_step_limit,
            |step_limit| {
                self.interpreter.eval_exact_exploration(
                    expression,
                    &env,
                    step_limit,
                    EXPLORE_GROUND_COLLECTION_LIMIT as usize,
                )
            },
        )
        .map_err(|failure| failure.to_string())
    }
}

/// Production adapter from the checked interpreter to the relational
/// executor's small expression boundary.
///
/// Immutable top-level bindings are evaluated once in a deterministic
/// dependency order and retained in the interpreter environment. Source
/// binders are replaced for every fiber/case evaluation, so pausing and
/// resuming work cannot leak values from a previous prefix.
struct RelationalExpressionBindingPlan {
    /// Process-local address of one expression inside the heap-stable checked
    /// query owned by a prepared Explore epoch. This is lookup state only: it
    /// never enters query identity, the journal, or published evidence.
    expression_address: usize,
    binding_order: Box<[String]>,
}

struct RelationalInterpreterExpressionRuntime {
    catalog: Arc<calculate::TypeCatalog>,
    binding_plans: Box<[RelationalExpressionBindingPlan]>,
    evaluator: ExploreRuntimeGroundEvaluator,
}

impl RelationalInterpreterExpressionRuntime {
    fn new(
        catalog: Arc<calculate::TypeCatalog>,
        definitions: &GroundDefinitions,
        query: &ExploreQueryIr,
        observer_memo_plan: Option<CheckedExactObserverMemoPlan>,
    ) -> Result<Self, String> {
        let binding_plans = planned_relational_binding_orders(query, definitions)?;
        let evaluator =
            ExploreRuntimeGroundEvaluator::new_with_observer_memo(definitions, observer_memo_plan);
        Ok(Self {
            catalog,
            binding_plans,
            evaluator,
        })
    }

    fn observer_memo_stats(&self) -> (bool, CheckedExactObserverMemoStats) {
        self.evaluator.observer_memo_stats()
    }
}

fn planned_relational_binding_order<'plan>(
    plans: &'plan [RelationalExpressionBindingPlan],
    expression: &Expr,
) -> Result<&'plan [String], String> {
    let expression_address = expression as *const Expr as usize;
    plans
        .iter()
        .find(|plan| plan.expression_address == expression_address)
        .map(|plan| plan.binding_order.as_ref())
        .ok_or_else(|| {
            "relational runtime received an expression outside its checked query plan".to_string()
        })
}

/// Precompute immutable-binding closure once for every expression root the
/// relational executors can present. The address is only an invocation-local
/// lookup into this borrowed, producer-owned closed query; it never enters a
/// semantic identity, journal event, snapshot, or result root.
fn planned_relational_binding_orders(
    query: &ExploreQueryIr,
    definitions: &GroundDefinitions,
) -> Result<Box<[RelationalExpressionBindingPlan]>, String> {
    let names = definitions
        .bindings
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut plans = Vec::<RelationalExpressionBindingPlan>::new();
    for expression in relational_runtime_expression_roots(query) {
        let expression_address = expression as *const Expr as usize;
        if plans
            .iter()
            .any(|plan| plan.expression_address == expression_address)
        {
            continue;
        }
        let roots = expression_query_dependencies(expression, &names, definitions);
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        for root in roots {
            append_required_binding(
                &root,
                &names,
                definitions,
                &mut visiting,
                &mut visited,
                &mut order,
            )?;
        }
        plans.push(RelationalExpressionBindingPlan {
            expression_address,
            binding_order: order.into_boxed_slice(),
        });
    }
    Ok(plans.into_boxed_slice())
}

fn relational_runtime_expression_roots(query: &ExploreQueryIr) -> Vec<&Expr> {
    let mut expressions = Vec::new();
    for binding in query.source.bindings.iter() {
        match &binding.kind {
            ExploreSourceBindingKindIr::Singleton { value } => expressions.push(value),
            ExploreSourceBindingKindIr::Finite { domain } => {
                append_relational_finite_domain_expressions(domain, &mut expressions)
            }
        }
    }
    match &query.successor.kind {
        ExploreSuccessorKindIr::Singleton { value } => expressions.push(value),
        ExploreSuccessorKindIr::Finite { domain } => {
            append_relational_finite_domain_expressions(domain, &mut expressions)
        }
    }
    expressions.extend(
        query
            .admissions
            .iter()
            .map(|admission| &admission.predicate),
    );
    if let Some(predicate) = query.find.predicate() {
        expressions.push(predicate);
    }
    for node in query.analysis.iter() {
        match node {
            ExploreAnalysisNodeIr::Result(view) => {
                if let ExploreResultGrainIr::GroupBy { fields, .. } = &view.grain {
                    expressions.extend(fields.iter().map(|field| &field.value));
                }
                expressions.extend(view.measures.iter().map(|field| &field.value));
                for aggregate in view.aggregates.iter() {
                    match &aggregate.reducer {
                        ExploreAggregateReducerIr::CountDistinct { value, .. } => {
                            expressions.push(value)
                        }
                    }
                }
                expressions.extend(view.select.iter().map(|field| &field.value));
                match &view.choose {
                    None => {}
                    Some(ExploreResultChoiceIr::Optimize { objective, .. }) => {
                        expressions.push(objective)
                    }
                    Some(ExploreResultChoiceIr::Pareto { objectives, .. }) => {
                        expressions.extend(objectives.iter().map(|objective| &objective.value))
                    }
                }
            }
            // Mechanism replay has a separate checked, fresh-evaluator plan.
            // If its endpoint ever leaks onto this route, lookup must fail.
            ExploreAnalysisNodeIr::Mechanisms(_) => {}
        }
    }
    expressions
}

fn append_relational_finite_domain_expressions<'a>(
    domain: &'a ExploreFiniteDomainIr,
    expressions: &mut Vec<&'a Expr>,
) {
    match domain {
        ExploreFiniteDomainIr::Exact(_) => {}
        ExploreFiniteDomainIr::Collection { expression, .. } => expressions.push(expression),
        ExploreFiniteDomainIr::IntRange {
            start,
            end_exclusive,
        } => {
            expressions.push(start);
            expressions.push(end_exclusive);
        }
    }
}

fn append_required_binding(
    name: &str,
    names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), String> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_string()) {
        return Err(format!(
            "relational exploration immutable binding dependency cycle reaches `{name}`"
        ));
    }
    let declarations = definitions.bindings.get(name).ok_or_else(|| {
        format!("relational exploration binding `{name}` disappeared from the checked graph")
    })?;
    let [declaration] = declarations.as_slice() else {
        return Err(format!(
            "relational exploration binding `{name}` has {} definitions",
            declarations.len()
        ));
    };
    for dependency in expression_query_dependencies(&declaration.expression, names, definitions) {
        append_required_binding(&dependency, names, definitions, visiting, visited, order)?;
    }
    visiting.remove(name);
    visited.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}

impl RelationalExpressionRuntime for RelationalInterpreterExpressionRuntime {
    fn evaluate(
        &mut self,
        expression: &Expr,
        expected_ty: &Ty,
        earlier_bindings: &[RelationalBoundValue<'_>],
    ) -> Result<ExploreValue, String> {
        let binding_order = planned_relational_binding_order(&self.binding_plans, expression)?;
        self.evaluator.clear_locals();
        for binding in earlier_bindings {
            self.evaluator.set_local(
                binding.name,
                runtime_value_from_explore_value(binding.value),
            );
        }
        let value = self.evaluator.eval(expression, binding_order)?;
        runtime_value_to_explore_value(&value, expected_ty, self.catalog.as_ref())
    }
}

impl RelationalResultExpressionRuntime for RelationalInterpreterExpressionRuntime {
    fn evaluate(
        &mut self,
        expression: &Expr,
        expected_ty: &Ty,
        bindings: &[RelationalResultBinding],
    ) -> Result<ResultValue, String> {
        // Semantic IDs intentionally have no interpreter `Value`
        // representation. A bare variable can preserve that typed identity
        // without stringifying it; every other expression goes through the
        // checked interpreter below.
        if let ExprKind::Var(name) = &expression.kind {
            if let Some(binding) = bindings.iter().rev().find(|binding| binding.name() == name) {
                return checked_direct_result_value(
                    binding.value(),
                    name,
                    expected_ty,
                    self.catalog.as_ref(),
                );
            }
        }

        // Resolve the sequential result environment before adapting it. This
        // makes a later SELECT alias shadow an earlier group/aggregate binding
        // with the same name, independent of map iteration order.
        let mut effective_bindings = BTreeMap::<String, &ResultValue>::new();
        for binding in bindings {
            effective_bindings.insert(binding.name().to_string(), binding.value());
        }

        // Opaque IDs that are merely present in an incidence row do not block
        // an unrelated expression. If the expression actually uses one in a
        // computation, fail closed instead of silently exposing its digest as
        // a Futuruna String.
        let mut referenced_names = BTreeSet::new();
        collect_true_free_vars(expression, &mut referenced_names, &BTreeSet::new());
        for name in referenced_names {
            let Some(value) = effective_bindings.get(&name) else {
                continue;
            };
            if let Some(kind) = opaque_result_value_kind(value) {
                return Err(format!(
                    "result expression uses opaque {kind} binding `{name}` inside a computation; semantic identifiers may currently be consumed only as a bare bound variable"
                ));
            }
        }

        let binding_order = planned_relational_binding_order(&self.binding_plans, expression)?;
        self.evaluator.clear_locals();
        for (name, value) in effective_bindings {
            if let ResultValue::Value(value) = value {
                self.evaluator
                    .set_local(name, runtime_value_from_explore_value(value));
            }
        }
        let value = self.evaluator.eval(expression, binding_order)?;
        runtime_value_to_explore_value(&value, expected_ty, self.catalog.as_ref())
            .map(ResultValue::Value)
    }
}

fn checked_direct_result_value(
    value: &ResultValue,
    binding_name: &str,
    expected_ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<ResultValue, String> {
    match value {
        ResultValue::Value(value) => runtime_value_to_explore_value(
            &runtime_value_from_explore_value(value),
            expected_ty,
            catalog,
        )
        .map(ResultValue::Value),
        ResultValue::CaseId(_) if matches!(expected_ty, Ty::Name(name) if name == "CaseId") => {
            Ok(value.clone())
        }
        ResultValue::TransitionId(_) if matches!(expected_ty, Ty::Name(name) if name == "TransitionId") => {
            Ok(value.clone())
        }
        ResultValue::SignatureId(_) if matches!(expected_ty, Ty::Name(name) if name == "MechanismSignatureId") => {
            Ok(value.clone())
        }
        ResultValue::StructuralMechanismId(_) if matches!(expected_ty, Ty::Name(name) if name == "StructuralMechanismId") => {
            Ok(value.clone())
        }
        ResultValue::ExecutionProfileId(_) if matches!(expected_ty, Ty::Name(name) if name == "ExecutionProfileId") => {
            Ok(value.clone())
        }
        ResultValue::CaseId(_)
        | ResultValue::TransitionId(_)
        | ResultValue::SignatureId(_)
        | ResultValue::StructuralMechanismId(_)
        | ResultValue::ExecutionProfileId(_) => {
            let kind = opaque_result_value_kind(value).expect("checked opaque result value");
            Err(format!(
                "opaque {kind} binding `{binding_name}` cannot satisfy expected type `{expected_ty}`"
            ))
        }
    }
}

fn opaque_result_value_kind(value: &ResultValue) -> Option<&'static str> {
    match value {
        ResultValue::Value(_) => None,
        ResultValue::CaseId(_) => Some("case ID"),
        ResultValue::TransitionId(_) => Some("transition ID"),
        ResultValue::SignatureId(_) => Some("mechanism signature ID"),
        ResultValue::StructuralMechanismId(_) => Some("structural mechanism ID"),
        ResultValue::ExecutionProfileId(_) => Some("execution profile ID"),
    }
}

fn eval_ground_binary(
    operator: &str,
    left: ExploreValue,
    right: ExploreValue,
) -> Result<ExploreValue, String> {
    match (operator, left, right) {
        ("+", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_add(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer addition overflow in exploration bound".to_string()),
        ("-", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_sub(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer subtraction overflow in exploration bound".to_string()),
        ("*", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_mul(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer multiplication overflow in exploration bound".to_string()),
        ("/", ExploreValue::Int(_), ExploreValue::Int(0)) => {
            Err("division by zero in exploration bound".to_string())
        }
        ("/", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_div(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer division overflow in exploration bound".to_string()),
        ("%", ExploreValue::Int(_), ExploreValue::Int(0)) => {
            Err("remainder by zero in exploration bound".to_string())
        }
        ("%", ExploreValue::Int(left), ExploreValue::Int(right)) => left
            .checked_rem(right)
            .map(ExploreValue::Int)
            .ok_or_else(|| "integer remainder overflow in exploration bound".to_string()),
        ("<", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left < right))
        }
        ("<=", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left <= right))
        }
        (">", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left > right))
        }
        (">=", ExploreValue::Int(left), ExploreValue::Int(right)) => {
            Ok(ExploreValue::Boolean(left >= right))
        }
        ("==", left, right) => ground_runtime_equality(&left, &right)
            .map(ExploreValue::Boolean)
            .ok_or_else(|| {
                format!(
                    "ground equality does not produce a Boolean for values {:?} and {:?} under Futuruna runtime semantics",
                    left, right
                )
            }),
        ("!=", left, right) => Ok(ExploreValue::Boolean(
            ground_runtime_equality(&left, &right).map_or(true, |equal| !equal),
        )),
        ("&&", ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => {
            Ok(ExploreValue::Boolean(left && right))
        }
        ("||", ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => {
            Ok(ExploreValue::Boolean(left || right))
        }
        (operator, left, right) => Err(format!(
            "operator `{}` does not support ground values {:?} and {:?}",
            operator, left, right
        )),
    }
}

/// Mirror `Interpreter::eval_binop("==", ...)` for the first-order values
/// accepted by ground domain evaluation. `None` means ordinary execution
/// returns a non-Boolean value for this equality shape.
fn ground_runtime_equality(left: &ExploreValue, right: &ExploreValue) -> Option<bool> {
    match (left, right) {
        (ExploreValue::Int(left), ExploreValue::Int(right)) => Some(left == right),
        (ExploreValue::FloatBits(left), ExploreValue::FloatBits(right)) => {
            Some(f64::from_bits(*left) == f64::from_bits(*right))
        }
        (ExploreValue::String(left), ExploreValue::String(right)) => Some(left == right),
        (ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => Some(left == right),
        (
            ExploreValue::Constructor {
                variant: left_variant,
                positional: true,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                variant: right_variant,
                positional: true,
                fields: right_fields,
                ..
            },
        ) => Some(
            left_variant == right_variant
                && left_fields.len() == right_fields.len()
                && left_fields
                    .iter()
                    .zip(right_fields.iter())
                    .all(|((_, left), (_, right))| {
                        ground_runtime_equality(left, right).unwrap_or(false)
                    }),
        ),
        (
            left @ ExploreValue::Constructor {
                positional: false, ..
            },
            right @ ExploreValue::Constructor {
                positional: false, ..
            },
        ) => Some(ground_values_equal(left, right)),
        (left @ ExploreValue::Constructor { .. }, right)
        | (left, right @ ExploreValue::Constructor { .. }) => {
            Some(ground_values_equal(left, right))
        }
        // Source lists and the supported list-producing helpers execute as
        // positional Cons/Nil values.  Interpreter::eval_binop therefore
        // compares each Cons field with direct runtime equality rather than
        // the broader fact-matching equality used by Value::List.
        (ExploreValue::List(left), ExploreValue::List(right)) => Some(
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| ground_runtime_equality(left, right).unwrap_or(false)),
        ),
        _ => None,
    }
}

/// Mirror `values_equal`, which is deliberately different from direct Float
/// equality when values are nested in lists or named constructors.
fn ground_values_equal(left: &ExploreValue, right: &ExploreValue) -> bool {
    match (left, right) {
        (ExploreValue::Int(left), ExploreValue::Int(right)) => left == right,
        (ExploreValue::FloatBits(left), ExploreValue::FloatBits(right)) => {
            (f64::from_bits(*left) - f64::from_bits(*right)).abs() < f64::EPSILON
        }
        (ExploreValue::String(left), ExploreValue::String(right)) => left == right,
        (ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => left == right,
        (ExploreValue::Character(left), ExploreValue::Character(right)) => left == right,
        (
            ExploreValue::Constructor {
                variant: left_variant,
                positional: left_positional,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                variant: right_variant,
                positional: right_positional,
                fields: right_fields,
                ..
            },
        ) => {
            left_positional == right_positional
                && left_variant == right_variant
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields.iter()).all(
                    |((left_name, left), (right_name, right))| {
                        (*left_positional || left_name == right_name)
                            && ground_values_equal(left, right)
                    },
                )
        }
        (ExploreValue::List(left), ExploreValue::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| ground_values_equal(left, right))
        }
        _ => false,
    }
}

fn collection_item_ty(ty: Option<&Ty>) -> Option<Ty> {
    let Ty::App(base, arguments) = ty? else {
        return None;
    };
    if matches!(base.as_ref(), Ty::Name(name) if (name == "List" || name == "Set") && arguments.len() == 1)
    {
        arguments.first().cloned()
    } else {
        None
    }
}

fn tuple_item_tys(ty: Option<&Ty>) -> Option<Vec<Ty>> {
    let Ty::App(constructor, arguments) = ty? else {
        return None;
    };
    matches!(constructor.as_ref(), Ty::Name(name) if name == "Tuple").then(|| arguments.clone())
}

fn collection_kind(ty: &Ty) -> Option<&str> {
    let Ty::App(base, arguments) = ty else {
        return None;
    };
    match base.as_ref() {
        Ty::Name(name) if (name == "List" || name == "Set") && arguments.len() == 1 => {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn strict_runtime_list_items(value: &Value) -> Result<Vec<&Value>, String> {
    if let Value::List(items) = value {
        return Ok(items.iter().collect());
    }
    let mut items = Vec::new();
    let mut current = value;
    loop {
        match current {
            Value::Constructor(name, fields) if name == "Nil" && fields.is_empty() => {
                return Ok(items);
            }
            Value::Constructor(name, fields) if name == "Cons" && fields.len() == 2 => {
                if items.len() >= EXPLORE_GROUND_COLLECTION_LIMIT as usize {
                    return Err(format!(
                        "ground list exceeds materialization limit {}",
                        EXPLORE_GROUND_COLLECTION_LIMIT
                    ));
                }
                items.push(&fields[0]);
                current = &fields[1];
            }
            _ => {
                return Err(
                    "ground List value is not a complete Cons/Nil chain or runtime List"
                        .to_string(),
                );
            }
        }
    }
}

fn runtime_value_to_explore_value(
    value: &Value,
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<ExploreValue, String> {
    match ty {
        Ty::Unit => {
            return matches!(value, Value::Unit)
                .then_some(ExploreValue::Unit)
                .ok_or_else(|| "runtime value does not have type Unit".to_string());
        }
        Ty::Name(name) => {
            let primitive = match (name.as_str(), value) {
                ("Unit", Value::Unit) => Some(ExploreValue::Unit),
                ("Int", Value::Int(value)) => Some(ExploreValue::Int(*value)),
                ("Nat", Value::Int(value)) if *value >= 0 => Some(ExploreValue::Int(*value)),
                ("Float", Value::Float(value)) => Some(ExploreValue::FloatBits(value.to_bits())),
                ("String", Value::Str(value)) => Some(ExploreValue::String(value.clone())),
                ("Bool", Value::Bool(value)) => Some(ExploreValue::Boolean(*value)),
                ("Char", Value::Char(value)) => Some(ExploreValue::Character(*value)),
                ("Any" | "_", _) => {
                    return Err(format!(
                        "runtime ground value cannot use open exploration type `{}`",
                        name
                    ));
                }
                _ => None,
            };
            if let Some(primitive) = primitive {
                return Ok(primitive);
            }
            if matches!(
                name.as_str(),
                "Unit" | "Int" | "Nat" | "Float" | "String" | "Bool" | "Char"
            ) {
                return Err(format!("runtime value does not have type `{}`", name));
            }
        }
        Ty::Optional(inner) => {
            return runtime_value_to_explore_value(
                value,
                &Ty::App(
                    Box::new(Ty::Name("Option".to_string())),
                    vec![*inner.clone()],
                ),
                catalog,
            );
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "List") => {
            if arguments.len() != 1 {
                return Err(format!("invalid ground List type `{}`", ty));
            }
            let items = strict_runtime_list_items(value)?;
            let mut converted = Vec::with_capacity(items.len());
            for (index, item) in items.into_iter().enumerate() {
                converted.push(
                    runtime_value_to_explore_value(item, &arguments[0], catalog).map_err(|_| {
                        format!(
                            "ground list member {} does not have declared type `{}`",
                            index + 1,
                            arguments[0]
                        )
                    })?,
                );
            }
            return Ok(ExploreValue::List(converted));
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "Set") => {
            if arguments.len() != 1 {
                return Err(format!("invalid ground Set type `{}`", ty));
            }
            let Value::Set(items) = value else {
                return Err(format!("runtime value does not have type `{}`", ty));
            };
            let mut converted = Vec::with_capacity(items.len());
            for (index, item) in items.values().enumerate() {
                converted.push(
                    runtime_value_to_explore_value(item, &arguments[0], catalog).map_err(|_| {
                        format!(
                            "ground set member {} does not have declared type `{}`",
                            index + 1,
                            arguments[0]
                        )
                    })?,
                );
            }
            return Ok(ExploreValue::Set(converted));
        }
        Ty::App(base, arguments) if matches!(base.as_ref(), Ty::Name(name) if name == "Tuple") => {
            let Value::Tuple(items) = value else {
                return Err(format!("runtime value does not have type `{}`", ty));
            };
            if items.len() != arguments.len() {
                return Err(format!(
                    "runtime tuple has {} fields but `{}` requires {}",
                    items.len(),
                    ty,
                    arguments.len()
                ));
            }
            return items
                .iter()
                .zip(arguments)
                .map(|(item, ty)| runtime_value_to_explore_value(item, ty, catalog))
                .collect::<Result<Vec<_>, _>>()
                .map(ExploreValue::Tuple);
        }
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            return Err(format!(
                "runtime ground value cannot use unsupported exploration type `{}`",
                ty
            ));
        }
        _ => {}
    }

    let Some((type_name, substitutions)) = instantiated_named_type(ty, catalog)? else {
        return Err(format!(
            "runtime value cannot be converted to declared type `{}`",
            ty
        ));
    };
    if catalog.is_rule_scope(&type_name) {
        return Err(format!(
            "rule scope `{}` is an open runtime scope and cannot be used in an exact exploration domain",
            type_name
        ));
    }
    let (variant_name, positional, runtime_fields): (&str, bool, Vec<(&str, &Value)>) = match value
    {
        Value::Constructor(name, fields) => (
            name,
            true,
            fields
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let _ = index;
                    ("", value)
                })
                .collect(),
        ),
        Value::NamedConstructor(name, fields) => (
            name,
            false,
            fields
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
        ),
        _ => {
            return Err(format!(
                "runtime value does not have declared type `{}`",
                ty
            ));
        }
    };
    let declaration = catalog
        .resolved_variants(&type_name)?
        .into_iter()
        .find(|variant| variant.name == variant_name)
        .ok_or_else(|| {
            format!(
                "runtime constructor `{}` does not inhabit declared type `{}`",
                variant_name, ty
            )
        })?;
    if runtime_fields.len() != declaration.fields.len()
        || (!declaration.fields.is_empty() && declaration.positional != positional)
    {
        return Err(format!(
            "runtime constructor `{}` has a shape incompatible with `{}`",
            variant_name, ty
        ));
    }
    let mut fields = Vec::with_capacity(declaration.fields.len());
    for (index, field) in declaration.fields.iter().enumerate() {
        let runtime_value = if positional {
            runtime_fields[index].1
        } else {
            runtime_fields
                .iter()
                .find(|(name, _)| *name == field.name)
                .map(|(_, value)| *value)
                .ok_or_else(|| {
                    format!(
                        "runtime constructor `{}` is missing field `{}`",
                        variant_name, field.name
                    )
                })?
        };
        let field_ty = calculate::substitute_type(&field.ty, &substitutions);
        fields.push((
            field.name.clone(),
            runtime_value_to_explore_value(runtime_value, &field_ty, catalog)?,
        ));
    }
    Ok(ExploreValue::Constructor {
        type_name,
        variant: variant_name.to_string(),
        // Normalize both runtime spellings of a nullary constructor to the
        // single declared inhabitant used by finite-type enumeration.
        positional: declaration.fields.is_empty() || positional,
        fields: fields.into(),
    })
}

fn instantiated_named_type(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
) -> Result<Option<(String, BTreeMap<String, Ty>)>, String> {
    let (name, arguments) = match ty {
        Ty::Name(name) => (name.clone(), Vec::new()),
        Ty::App(base, arguments) => {
            let Ty::Name(name) = base.as_ref() else {
                return Ok(None);
            };
            (name.clone(), arguments.clone())
        }
        Ty::Optional(inner) => ("Option".to_string(), vec![*inner.clone()]),
        _ => return Ok(None),
    };
    if !catalog.contains_type(&name) {
        return Ok(None);
    }
    let parameters = catalog.type_parameters(&name)?;
    if parameters.len() != arguments.len() {
        return Err(format!(
            "type `{}` expects {} arguments but got {}",
            name,
            parameters.len(),
            arguments.len()
        ));
    }
    Ok(Some((
        name,
        parameters.into_iter().zip(arguments).collect(),
    )))
}

fn collect_declared_type_dependencies(ty: &Ty, dependencies: &mut BTreeSet<String>) {
    match ty {
        Ty::Name(name) => {
            dependencies.insert(name.clone());
        }
        Ty::App(base, arguments) => {
            collect_declared_type_dependencies(base, dependencies);
            for argument in arguments {
                collect_declared_type_dependencies(argument, dependencies);
            }
        }
        Ty::Optional(inner) => {
            dependencies.insert("Option".to_string());
            collect_declared_type_dependencies(inner, dependencies);
        }
        Ty::Arrow(input, output) => {
            collect_declared_type_dependencies(input, dependencies);
            collect_declared_type_dependencies(output, dependencies);
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
            collect_declared_type_dependencies(inner, dependencies)
        }
        Ty::Var(_) | Ty::Unit | Ty::Hole => {}
    }
}

fn declaration_reaches_type(
    catalog: &calculate::TypeCatalog,
    current: &str,
    target: &str,
    visiting: &mut BTreeSet<String>,
) -> Result<bool, String> {
    if visiting.len() >= EXPLORE_RECURSION_LIMIT {
        return Err(format!(
            "finite type dependency exceeds the safe depth limit {}",
            EXPLORE_RECURSION_LIMIT
        ));
    }
    if !visiting.insert(current.to_string()) {
        return Ok(false);
    }
    for variant in catalog.resolved_variants(current)? {
        for field in variant.fields {
            let mut dependencies = BTreeSet::new();
            collect_declared_type_dependencies(&field.ty, &mut dependencies);
            for dependency in dependencies {
                if dependency == target {
                    visiting.remove(current);
                    return Ok(true);
                }
                if catalog.type_parameters(&dependency).is_ok()
                    && declaration_reaches_type(catalog, &dependency, target, visiting)?
                {
                    visiting.remove(current);
                    return Ok(true);
                }
            }
        }
    }
    visiting.remove(current);
    Ok(false)
}

fn finite_type_plan(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
    path: &str,
    active: &mut BTreeSet<String>,
) -> Result<ExploreFiniteTypePlan, String> {
    let mut budget = EXPLORE_FINITE_PLAN_WORK_LIMIT;
    finite_type_plan_with_budget(ty, catalog, path, active, &mut budget, 0)
}

fn finite_type_plan_with_budget(
    ty: &Ty,
    catalog: &calculate::TypeCatalog,
    path: &str,
    active: &mut BTreeSet<String>,
    budget: &mut usize,
    depth: usize,
) -> Result<ExploreFiniteTypePlan, String> {
    if depth >= EXPLORE_RECURSION_LIMIT {
        return Err(format!(
            "`values({})` exceeds the finite-type depth limit {}",
            ty, EXPLORE_RECURSION_LIMIT
        ));
    }
    let Some(remaining) = budget.checked_sub(1) else {
        return Err(format!(
            "`values({})` exceeds the finite-type plan work limit {}",
            ty, EXPLORE_FINITE_PLAN_WORK_LIMIT
        ));
    };
    *budget = remaining;
    match ty {
        Ty::Unit => return Ok(ExploreFiniteTypePlan::Unit),
        Ty::Name(name) if name == "Unit" => return Ok(ExploreFiniteTypePlan::Unit),
        Ty::Name(name) if name == "Bool" => return Ok(ExploreFiniteTypePlan::Bool),
        Ty::App(constructor, elements) if matches!(constructor.as_ref(), Ty::Name(name) if name == "Tuple") =>
        {
            let identity = ty.to_string();
            if !active.insert(identity.clone()) {
                return Err(format!(
                    "`values({})` is recursive through `{}` and is not finite",
                    ty, path
                ));
            }
            let mut plans = Vec::with_capacity(elements.len());
            let mut cardinality = ExploreCardinality::one();
            for (index, element) in elements.iter().enumerate() {
                let plan = finite_type_plan_with_budget(
                    element,
                    catalog,
                    &format!("{}[{}]", path, index),
                    active,
                    budget,
                    depth + 1,
                )?;
                cardinality = cardinality.multiply(plan.cardinality());
                plans.push(plan);
            }
            active.remove(&identity);
            return Ok(ExploreFiniteTypePlan::Tuple {
                elements: plans,
                cardinality,
            });
        }
        Ty::Optional(inner) => {
            return finite_type_plan_with_budget(
                &Ty::App(
                    Box::new(Ty::Name("Option".to_string())),
                    vec![*inner.clone()],
                ),
                catalog,
                path,
                active,
                budget,
                depth + 1,
            );
        }
        Ty::Name(name)
            if matches!(
                name.as_str(),
                "Int"
                    | "Nat"
                    | "Any"
                    | "Float"
                    | "String"
                    | "Char"
                    | "List"
                    | "Set"
                    | "Map"
                    | "Stream"
            ) =>
        {
            return Err(format!(
                "`values({})` is unbounded at `{}`; provide an explicit list or range",
                ty, path
            ));
        }
        Ty::App(base, _) if matches!(base.as_ref(), Ty::Name(name) if matches!(name.as_str(), "List" | "Set" | "Map" | "Stream")) =>
        {
            return Err(format!(
                "`values({})` is unbounded at `{}`; provide an explicit finite collection",
                ty, path
            ));
        }
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            return Err(format!(
                "`values({})` cannot enumerate `{}` at `{}`",
                ty, ty, path
            ));
        }
        _ => {}
    }

    let identity = ty.to_string();
    if !active.insert(identity.clone()) {
        return Err(format!(
            "`values({})` is recursive through `{}` and is not finite",
            ty, path
        ));
    }
    let Some((type_name, substitutions)) = instantiated_named_type(ty, catalog)? else {
        active.remove(&identity);
        return Err(format!("`values({})` names an unknown finite type", ty));
    };
    if declaration_reaches_type(catalog, &type_name, &type_name, &mut BTreeSet::new())? {
        active.remove(&identity);
        return Err(format!(
            "`values({})` cannot enumerate recursive declared type `{}`",
            ty, type_name
        ));
    }
    if catalog.is_rule_scope(&type_name) {
        active.remove(&identity);
        return Err(format!(
            "`values({})` cannot enumerate rule scope `{}`",
            ty, type_name
        ));
    }
    let variants = catalog.resolved_variants(&type_name)?;
    let mut seen_variants = BTreeSet::new();
    let mut plans = Vec::with_capacity(variants.len());
    let mut total = ExploreCardinality::zero();
    for variant in variants {
        let Some(remaining) = budget.checked_sub(1) else {
            active.remove(&identity);
            return Err(format!(
                "`values({})` exceeds the finite-type plan work limit {}",
                ty, EXPLORE_FINITE_PLAN_WORK_LIMIT
            ));
        };
        *budget = remaining;
        if !seen_variants.insert(variant.name.clone()) {
            active.remove(&identity);
            return Err(format!(
                "finite type `{}` resolves constructor `{}` more than once",
                type_name, variant.name
            ));
        }
        let canonical_positional = variant.fields.is_empty() || variant.positional;
        let mut fields = Vec::with_capacity(variant.fields.len());
        let mut count = ExploreCardinality::one();
        for field in variant.fields {
            let field_ty = calculate::substitute_type(&field.ty, &substitutions);
            let field_path = format!("{}.{}.{}", path, variant.name, field.name);
            let plan = finite_type_plan_with_budget(
                &field_ty,
                catalog,
                &field_path,
                active,
                budget,
                depth + 1,
            )?;
            count = count.multiply(plan.cardinality());
            fields.push(ExploreFiniteFieldPlan {
                name: field.name,
                plan,
            });
        }
        total = total.add(count);
        plans.push(ExploreFiniteVariantPlan {
            name: variant.name,
            positional: canonical_positional,
            fields,
        });
    }
    active.remove(&identity);
    Ok(ExploreFiniteTypePlan::Sum {
        type_name,
        variants: plans,
        cardinality: total,
    })
}

fn collect_ground_bindings(
    statements: &[Stmt],
    source_dir: Option<&str>,
) -> Result<GroundDefinitions, Vec<String>> {
    let mut definitions = GroundDefinitions::default();
    let mut visited = BTreeSet::new();
    let mut errors = Vec::new();
    let statement_refs = statements.iter().collect::<Vec<_>>();
    collect_ground_bindings_inner(
        &statement_refs,
        source_dir,
        "<root>",
        &mut visited,
        &mut definitions,
        &mut errors,
    );
    if errors.is_empty() {
        Ok(definitions)
    } else {
        Err(errors)
    }
}

fn ground_declaration_identity(statement: &Stmt) -> Option<(String, String, String)> {
    match statement {
        Stmt::Defn(definition) => {
            let name = match definition {
                Defn::Fn { name, .. } | Defn::Actor { name, .. } | Defn::Module { name, .. } => {
                    name
                }
            };
            Some((
                "definition".to_string(),
                name.clone(),
                content_hash_defn(definition),
            ))
        }
        Stmt::TypeDecl(declaration) => {
            let (kind, name) = match declaration {
                TypeDecl::ADT { name, .. } => ("adt", name),
                TypeDecl::WhenType { name, .. } => ("when", name),
                TypeDecl::EffectDecl { name, .. } => ("effect", name),
                TypeDecl::TraitDecl { name, .. } => ("trait", name),
                TypeDecl::ImplBlock {
                    trait_name,
                    for_type,
                    ..
                } => {
                    return Some((
                        "impl".to_string(),
                        format!("{} for {}", trait_name, for_type),
                        content_hash_type(declaration),
                    ));
                }
                TypeDecl::RuleScope { name, .. } => ("rule-scope", name),
            };
            Some((
                kind.to_string(),
                name.clone(),
                content_hash_type(declaration),
            ))
        }
        _ => None,
    }
}

fn standard_prelude_declaration_identities() -> Vec<(String, String, String)> {
    parse_prelude()
        .iter()
        .filter_map(ground_declaration_identity)
        .collect()
}

fn leading_injected_prelude_indices(statements: &[&Stmt], origin: &str) -> BTreeSet<usize> {
    if origin != "<root>" {
        return BTreeSet::new();
    }
    let prelude = standard_prelude_declaration_identities();
    let mut cursor = 0;
    let mut indices = BTreeSet::new();
    for (index, statement) in statements.iter().copied().enumerate() {
        let Some(identity) = ground_declaration_identity(statement) else {
            break;
        };
        let Some(relative) = prelude[cursor..]
            .iter()
            .position(|candidate| candidate == &identity)
        else {
            break;
        };
        cursor += relative + 1;
        indices.insert(index);
    }
    indices
}

fn collect_ground_bindings_inner(
    statements: &[&Stmt],
    source_dir: Option<&str>,
    origin: &str,
    visited: &mut BTreeSet<String>,
    definitions: &mut GroundDefinitions,
    errors: &mut Vec<String>,
) {
    let injected_prelude = leading_injected_prelude_indices(statements, origin);
    if !injected_prelude.is_empty() && !definitions.origin_order.contains_key("<prelude>") {
        let next = definitions.origin_order.len();
        definitions
            .origin_order
            .insert("<prelude>".to_string(), next);
    }
    let mut saw_local_program_statement = false;
    for (index, statement) in statements.iter().copied().enumerate() {
        if injected_prelude.contains(&index) {
            continue;
        }
        match statement {
            Stmt::Import(path) | Stmt::HashImport(_, path) => {
                if saw_local_program_statement {
                    errors.push(format!(
                        "exploration import `{}` appears after a local declaration or executable statement; exact ground evaluation requires imports in the module prefix",
                        path
                    ));
                }
            }
            Stmt::Annot(_, _)
            | Stmt::Use(_)
            | Stmt::RustBlock(_)
            | Stmt::Depend(_, _)
            | Stmt::QualifiedImport(_, _) => {}
            _ => saw_local_program_statement = true,
        }
    }

    for statement in statements.iter().copied() {
        match statement {
            Stmt::Import(path) => {
                let Some(directory) = source_dir else {
                    errors.push(format!(
                        "cannot resolve exploration import `{}` without a source directory",
                        path
                    ));
                    continue;
                };
                let Some(file_path) = Interpreter::resolve_import_path_for_source(path, directory)
                else {
                    errors.push(format!("cannot resolve exploration import `{}`", path));
                    continue;
                };
                let canonical = std::fs::canonicalize(&file_path)
                    .unwrap_or_else(|_| PathBuf::from(&file_path))
                    .to_string_lossy()
                    .to_string();
                if !visited.insert(canonical.clone()) {
                    continue;
                }
                let module = match parse_source_module_file_cached(Path::new(&file_path)) {
                    Ok(module) => module,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let nested_dir = Path::new(&file_path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string());
                let nested_statements = module.statements().iter().collect::<Vec<_>>();
                collect_ground_bindings_inner(
                    &nested_statements,
                    Some(&nested_dir),
                    &canonical,
                    visited,
                    definitions,
                    errors,
                );
            }
            Stmt::HashImport(hash, path) => {
                let Some(directory) = source_dir else {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}` without a source directory",
                        hash, path
                    ));
                    continue;
                };
                let Some(file_path) = Interpreter::resolve_import_path_for_source(path, directory)
                else {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}`",
                        hash, path
                    ));
                    continue;
                };
                let canonical = std::fs::canonicalize(&file_path)
                    .unwrap_or_else(|_| PathBuf::from(&file_path))
                    .to_string_lossy()
                    .to_string();
                let import_key = format!("{}#{}", canonical, hash);
                if !visited.insert(import_key.clone()) {
                    continue;
                }
                let module = match parse_source_module_file_cached(Path::new(&file_path)) {
                    Ok(module) => module,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let matched = module
                    .statements()
                    .iter()
                    .filter(|statement| match statement {
                        Stmt::Defn(definition) => content_hash_defn(definition) == *hash,
                        Stmt::TypeDecl(declaration) => content_hash_type(declaration) == *hash,
                        _ => false,
                    })
                    .collect::<Vec<_>>();
                if matched.len() != 1 {
                    errors.push(format!(
                        "cannot resolve exploration hash import `#{}` from `{}`: expected exactly one matching definition, found {}",
                        hash,
                        path,
                        matched.len()
                    ));
                    continue;
                }
                let nested_dir = Path::new(&file_path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string());
                collect_ground_bindings_inner(
                    &matched,
                    Some(&nested_dir),
                    &import_key,
                    visited,
                    definitions,
                    errors,
                );
            }
            _ => {}
        }
    }

    if !definitions.origin_order.contains_key(origin) {
        let next = definitions.origin_order.len();
        definitions.origin_order.insert(origin.to_string(), next);
    }

    for (index, statement) in statements.iter().copied().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Bind(Pat::Var(name), annotated_ty, expression) = statement else {
            continue;
        };
        definitions
            .bindings
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push(SourcedBinding {
                expression: expression.clone(),
                annotated_ty: annotated_ty.clone(),
                origin: statement_origin.to_string(),
            });
    }
    for (index, statement) in statements.iter().copied().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let mut names = BTreeSet::new();
        match statement {
            Stmt::Bind(pattern, _, _) if !matches!(pattern, Pat::Var(_)) => {
                collect_pattern_names(pattern, &mut names)
            }
            Stmt::MonadicBind(pattern, _, _) => collect_pattern_names(pattern, &mut names),
            Stmt::StreamBind(name, _)
            | Stmt::QualifiedImport(name, _)
            | Stmt::Defn(Defn::Actor { name, .. })
            | Stmt::Defn(Defn::Module { name, .. })
            | Stmt::Rule(Rule::ReactiveScope { name, .. }) => {
                names.insert(name.clone());
            }
            _ => {}
        }
        for name in names {
            definitions
                .unsupported_values
                .entry(name)
                .or_insert_with(Vec::new)
                .push(statement_origin.to_string());
        }
    }
    for (index, statement) in statements.iter().copied().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Defn(Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        }) = statement
        else {
            continue;
        };
        definitions
            .functions
            .entry((name.clone(), params.len()))
            .or_insert_with(Vec::new)
            .push(SourcedFunction {
                params: params.clone(),
                return_ty: ret_ty.clone(),
                effects: effects.clone(),
                body: body.clone(),
                origin: statement_origin.to_string(),
            });
    }
    for (index, statement) in statements.iter().copied().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        let Stmt::Rule(rule) = statement else {
            continue;
        };
        let Some((name, arity)) = ground_rule_name_arity(rule) else {
            continue;
        };
        definitions
            .rules
            .entry((name.clone(), arity))
            .or_insert_with(Vec::new)
            .push(statement_origin.to_string());
        definitions
            .rule_definitions
            .entry((name, arity))
            .or_insert_with(Vec::new)
            .push(rule.clone());
    }
    for (index, statement) in statements.iter().enumerate() {
        let statement_origin = if injected_prelude.contains(&index) {
            "<prelude>"
        } else {
            origin
        };
        match statement {
            Stmt::Defn(Defn::Actor { name, handlers, .. }) => {
                definitions
                    .unsupported_callables
                    .entry((name.clone(), handlers.len()))
                    .or_insert_with(Vec::new)
                    .push(statement_origin.to_string());
            }
            Stmt::TypeDecl(TypeDecl::ADT {
                variants, methods, ..
            }) => {
                for variant in variants {
                    definitions
                        .constructors
                        .entry((variant.name.clone(), variant.fields.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
                record_unsupported_methods(methods, statement_origin, definitions);
            }
            Stmt::TypeDecl(TypeDecl::WhenType { variants, .. }) => {
                for variant in variants {
                    definitions
                        .constructors
                        .entry((variant.name.clone(), variant.fields.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                record_unsupported_methods(methods, statement_origin, definitions);
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { ops, .. }) => {
                for (name, parameters, _) in ops {
                    definitions
                        .unsupported_callables
                        .entry((name.clone(), parameters.len()))
                        .or_insert_with(Vec::new)
                        .push(statement_origin.to_string());
                }
            }
            Stmt::TypeDecl(TypeDecl::RuleScope {
                name, params, body, ..
            }) => {
                definitions
                    .constructors
                    .entry((name.clone(), params.len()))
                    .or_insert_with(Vec::new)
                    .push(statement_origin.to_string());
                for member in body {
                    if let Stmt::Defn(Defn::Fn { name, params, .. }) = member {
                        definitions
                            .unsupported_callables
                            .entry((name.clone(), params.len()))
                            .or_insert_with(Vec::new)
                            .push(statement_origin.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    definitions
        .runtime_declarations
        .extend(
            statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    let statement = *statement;
                    (!injected_prelude.contains(&index)
                        && matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Rule(_)))
                    .then(|| statement.clone())
                }),
        );
}

fn record_unsupported_methods(methods: &[Defn], origin: &str, definitions: &mut GroundDefinitions) {
    for method in methods {
        if let Defn::Fn { name, params, .. } = method {
            definitions
                .unsupported_callables
                .entry((name.clone(), params.len()))
                .or_insert_with(Vec::new)
                .push(origin.to_string());
        }
    }
}

fn ground_rule_name_arity(rule: &Rule) -> Option<(String, usize)> {
    let head = match rule {
        Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. } => {
            head
        }
        Rule::ReactiveScope { .. } => return None,
    };
    match &head.kind {
        ExprKind::Var(name) => Some((name.clone(), 0)),
        ExprKind::App(function, arguments) => {
            let ExprKind::Var(name) = &function.kind else {
                return None;
            };
            Some((name.clone(), arguments.len()))
        }
        _ => None,
    }
}

fn ground_intrinsic_arity(name: &str) -> Option<usize> {
    match name {
        "range" => Some(2),
        "set_from_list" | "distinct" => Some(1),
        "set_new" => Some(0),
        "concat" | "set_insert" | "set_remove" => Some(2),
        _ => None,
    }
}

fn replay_builtin_arity(name: &str) -> Option<usize> {
    static BUILTIN_ARITIES: OnceLock<BTreeMap<String, usize>> = OnceLock::new();
    let canonical = builtin_canonical(name);
    BUILTIN_ARITIES
        .get_or_init(|| TypeChecker::new().builtins)
        .get(canonical)
        .copied()
        // `format_f` is an interpreter-only compatibility builtin.  Keep it
        // out of the language-wide TypeChecker inventory, but include it when
        // auditing the canonical interpreter's Pipe value lookup.
        .or_else(|| (canonical == "format_f").then_some(2))
}

fn collect_ground_rule_pattern_names(expression: &Expr, names: &mut BTreeSet<String>) {
    if let ExprKind::App(function, arguments) = &expression.kind {
        if matches!(&function.kind, ExprKind::Var(name) if name == "__typed")
            && arguments.len() == 2
        {
            collect_ground_rule_pattern_names(&arguments[0], names);
            return;
        }
    }
    match &expression.kind {
        ExprKind::Var(name)
            if name != "_" && !name.chars().next().is_some_and(char::is_uppercase) =>
        {
            names.insert(name.clone());
        }
        ExprKind::App(_, arguments) | ExprKind::Tuple(arguments) => {
            for argument in arguments {
                collect_ground_rule_pattern_names(argument, names);
            }
        }
        _ => {}
    }
}

fn ground_rule_bound_names(rule: &Rule) -> BTreeSet<String> {
    let (head, body) = match rule {
        Rule::Clause { head, body } => (head, body.as_ref()),
        Rule::Default { head, .. } | Rule::Exception { head, .. } => (head, None),
        Rule::ReactiveScope { .. } => return BTreeSet::new(),
    };
    let mut bound = BTreeSet::new();
    if let ExprKind::App(_, arguments) = &head.kind {
        for argument in arguments {
            collect_ground_rule_pattern_names(argument, &mut bound);
        }
    }

    // Rule conjunction/disjunction goals introduce logic variables in the
    // same places that Interpreter::apply_rule clears from the caller env.
    fn collect_goal_names(expression: &Expr, names: &mut BTreeSet<String>) {
        match &expression.kind {
            ExprKind::Conjunction(goals) | ExprKind::Disjunction(goals) => {
                for goal in goals {
                    collect_goal_names(goal, names);
                }
            }
            ExprKind::App(_, arguments) => {
                for argument in arguments {
                    collect_ground_rule_pattern_names(argument, names);
                }
            }
            _ => {}
        }
    }
    if body.is_some_and(|body| {
        matches!(
            &body.kind,
            ExprKind::Conjunction(_) | ExprKind::Disjunction(_)
        )
    }) {
        collect_goal_names(body.expect("checked rule body"), &mut bound);
    }
    bound
}

fn ground_rule_expressions(rule: &Rule) -> Vec<&Expr> {
    match rule {
        Rule::Clause { body, .. } => body.iter().collect(),
        Rule::Default {
            value, condition, ..
        }
        | Rule::Exception {
            value, condition, ..
        } => std::iter::once(value).chain(condition.iter()).collect(),
        Rule::ReactiveScope { .. } => Vec::new(),
    }
}

fn expression_query_dependencies(
    expression: &Expr,
    names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
) -> BTreeSet<String> {
    let mut free = BTreeSet::new();
    collect_true_free_vars(expression, &mut free, &BTreeSet::new());
    free.retain(|name| names.contains(name));
    let mut memo = BTreeMap::new();
    let mut work_remaining = EXPLORE_FINITE_PLAN_WORK_LIMIT;
    free.extend(expression_dynamic_helper_dependencies(
        expression,
        names,
        definitions,
        &mut BTreeSet::new(),
        &mut memo,
        &mut work_remaining,
        0,
    ));
    free
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayCallableKind {
    Function,
    Rule,
    Constructor,
    Intrinsic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplayCallableIdentity {
    kind: ReplayCallableKind,
    arity: usize,
}

fn exact_source_declaration_identity(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
) -> Option<ReplayCallableIdentity> {
    let key = (name.to_string(), arity);
    let function_count = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .map(|(_, declarations)| declarations.len())
        .sum::<usize>();
    if function_count == 1 && definitions.functions.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Function,
            arity,
        });
    }
    if definitions.rule_definitions.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Rule,
            arity,
        });
    }
    if definitions.constructors.contains_key(&key) {
        return Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Constructor,
            arity,
        });
    }
    None
}

fn pipe_effective_callable_identity(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
) -> Result<Option<ReplayCallableIdentity>, String> {
    let key = (name.to_string(), arity);
    let function_count = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .map(|(_, declarations)| declarations.len())
        .sum::<usize>();
    if function_count == 1 {
        return if definitions.functions.contains_key(&key) {
            Ok(Some(ReplayCallableIdentity {
                kind: ReplayCallableKind::Function,
                arity,
            }))
        } else {
            Ok(None)
        };
    }

    let constructor_arities = definitions
        .constructors
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .flat_map(|((_, declared_arity), declarations)| {
            std::iter::repeat_n(*declared_arity, declarations.len())
        })
        .collect::<Vec<_>>();
    if constructor_arities.len() > 1 {
        return Err(format!(
            "exploration replay pipe constructor `{}` has multiple runtime declarations and cannot identify one exact callable",
            name
        ));
    }
    if let Some(declared_arity) = constructor_arities.first().copied() {
        if declared_arity != arity {
            return Err(format!(
                "exploration replay pipe constructor `{}` resolves its source form at {} argument{} but executes at {} argument{}",
                name,
                declared_arity,
                if declared_arity == 1 { "" } else { "s" },
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Constructor,
            arity,
        }));
    }

    if let Some(declared_arity) = replay_builtin_arity(name) {
        if declared_arity != arity {
            return Err(format!(
                "exploration replay pipe built-in `{}` is declared for {} argument{} but receives {} argument{} at runtime",
                name,
                declared_arity,
                if declared_arity == 1 { "" } else { "s" },
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Intrinsic,
            arity,
        }));
    }
    if definitions.rule_definitions.contains_key(&key) {
        return Ok(Some(ReplayCallableIdentity {
            kind: ReplayCallableKind::Rule,
            arity,
        }));
    }
    Ok(None)
}

fn explore_replay_pipe_call_site_issue(
    call: &RuntimeCallUse,
    definitions: &GroundDefinitions,
) -> Option<String> {
    let effective =
        match pipe_effective_callable_identity(&call.name, call.effective_arity, definitions) {
            Ok(identity) => identity,
            Err(issue) => return Some(issue),
        };

    if replay_builtin_arity(&call.name).is_some()
        && definitions
            .rule_definitions
            .contains_key(&(call.name.clone(), call.effective_arity))
    {
        return Some(format!(
            "exploration replay pipe call `{}` executes the built-in intrinsic instead of the exact rule with the same runtime name",
            call.name
        ));
    }

    let Some(source_arity) = call.source_arity else {
        return None;
    };
    let Some(source) = exact_source_declaration_identity(&call.name, source_arity, definitions)
    else {
        return None;
    };
    if effective != Some(source) {
        let subject = if source.kind == ReplayCallableKind::Constructor {
            "pipe constructor"
        } else {
            "pipe call"
        };
        return Some(format!(
            "exploration replay {} `{}` resolves its source form at {} argument{} but executes at {} argument{}",
            subject,
            call.name,
            source_arity,
            if source_arity == 1 { "" } else { "s" },
            call.effective_arity,
            if call.effective_arity == 1 { "" } else { "s" }
        ));
    }
    None
}

fn explore_replay_callable_identity_issue(
    name: &str,
    arity: usize,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    validated: &mut BTreeSet<(String, usize)>,
) -> Option<String> {
    let key = (name.to_string(), arity);
    if validated.contains(&key) || !visiting.insert(key.clone()) {
        return None;
    }

    let exact_rule = definitions.rule_definitions.contains_key(&key);
    if exact_rule {
        if let Some(issue) = definitions.explore_rule_return_issues.get(&key) {
            visiting.remove(&key);
            return Some(format!(
                "exploration replay cannot classify reachable rule `{}({} argument{})`: {}",
                name,
                arity,
                if arity == 1 { "" } else { "s" },
                issue
            ));
        }
        if !definitions
            .explore_rule_return_types_by_arity
            .contains_key(&key)
        {
            visiting.remove(&key);
            return Some(format!(
                "exploration replay cannot classify the exact return type of reachable rule `{}({} argument{})`",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ));
        }
    }
    let issue = if definitions.bindings.contains_key(name) {
        Some(if exact_rule {
            format!(
                "exploration replay rule call `{}` is shadowed by a top-level binding",
                name
            )
        } else {
            format!(
                "exploration replay call `{}` is shadowed by a top-level binding",
                name
            )
        })
    } else if definitions.unsupported_values.contains_key(name) {
        Some(format!(
            "exploration replay call `{}` is shadowed by a runtime value declaration",
            name
        ))
    } else if definitions
        .unsupported_callables
        .keys()
        .any(|(candidate, _)| candidate == name)
    {
        Some(if exact_rule {
            format!(
                "exploration replay rule call `{}` collides with an unsupported callable sharing one runtime name",
                name
            )
        } else {
            format!(
                "exploration replay call `{}` collides with an unsupported callable sharing one runtime name",
                name
            )
        })
    } else {
        None
    };
    if issue.is_some() {
        visiting.remove(&key);
        return issue;
    }

    let function_arities = definitions
        .functions
        .iter()
        .filter(|((candidate, _), _)| candidate == name)
        .flat_map(|((_, arity), declarations)| std::iter::repeat_n(*arity, declarations.len()))
        .collect::<Vec<_>>();
    let issue = if function_arities.len() > 1 {
        let declared_arities = function_arities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|arity| arity.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "exploration replay cannot resolve helper `{}({} argument{})` exactly: `{}` has declarations across arities ({}), but ordinary runtime functions resolve by bare name; give every reachable helper a unique name",
            name,
            arity,
            if arity == 1 { "" } else { "s" },
            name,
            declared_arities
        ))
    } else if function_arities.len() == 1 {
        let exact = definitions.functions.get(&key);
        if exact.is_none_or(|declarations| declarations.len() != 1) {
            Some(format!(
                "exploration replay call `{}({} argument{})` resolves by signature to a different callable, but a different-arity ordinary function with the same runtime name shadows it",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ))
        } else if definitions
            .rules
            .keys()
            .any(|(candidate, _)| candidate == name)
        {
            Some(format!(
                "exploration replay call `{}` is ambiguous between a function and rule sharing one runtime name",
                name
            ))
        } else if definitions
            .constructors
            .keys()
            .any(|(candidate, _)| candidate == name)
        {
            Some(format!(
                "exploration replay call `{}` is ambiguous between a function and constructor sharing one runtime name",
                name
            ))
        } else {
            let definition = &exact.expect("one exact helper definition")[0];
            let bound = definition
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect::<BTreeSet<_>>();
            expression_replay_callable_identity_issue(
                &definition.body,
                &bound,
                definitions,
                visiting,
                validated,
            )
        }
    } else if let Some(rules) = definitions.rule_definitions.get(&key) {
        if definitions.constructors.contains_key(&key) {
            Some(format!(
                "exploration replay constructor `{}({} argument{})` takes precedence over the rule with the same runtime signature",
                name,
                arity,
                if arity == 1 { "" } else { "s" }
            ))
        } else {
            rules.iter().find_map(|rule| {
                let bound = ground_rule_bound_names(rule);
                ground_rule_expressions(rule)
                    .into_iter()
                    .find_map(|expression| {
                        expression_replay_callable_identity_issue(
                            expression,
                            &bound,
                            definitions,
                            visiting,
                            validated,
                        )
                    })
            })
        }
    } else if definitions
        .constructors
        .get(&key)
        .is_some_and(|declarations| declarations.len() > 1)
    {
        Some(format!(
            "exploration replay constructor `{}({} argument{})` has multiple visible runtime declarations",
            name,
            arity,
            if arity == 1 { "" } else { "s" }
        ))
    } else if definitions.constructors.contains_key(&key)
        && replay_builtin_arity(name) == Some(arity)
    {
        Some(format!(
            "exploration replay constructor `{}({} argument{})` collides with a built-in intrinsic sharing one runtime name",
            name,
            arity,
            if arity == 1 { "" } else { "s" }
        ))
    } else {
        None
    };

    visiting.remove(&key);
    if issue.is_none() {
        validated.insert(key);
    }
    issue
}

fn expression_replay_callable_identity_issue(
    expression: &Expr,
    bound: &BTreeSet<String>,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    validated: &mut BTreeSet<(String, usize)>,
) -> Option<String> {
    collect_scoped_runtime_calls(expression, bound)
        .into_iter()
        .find_map(|call| {
            if call.lexically_bound {
                return Some(format!(
                    "exploration replay call `{}` resolves through a lexical value instead of one exact top-level callable",
                    call.name
                ));
            }
            if matches!(call.name.as_str(), "findall" | "search") {
                return Some(format!(
                    "exploration replay runtime special form `{}({} argument{})` is not an exact replay callable",
                    call.name,
                    call.effective_arity,
                    if call.effective_arity == 1 { "" } else { "s" }
                ));
            }
            if !call.through_pipe
                && replay_builtin_arity(&call.name) == Some(call.effective_arity)
                && !definitions
                    .rule_definitions
                    .contains_key(&(call.name.clone(), call.effective_arity))
                && !definitions
                    .constructors
                    .contains_key(&(call.name.clone(), call.effective_arity))
                && definitions
                    .constructors
                    .keys()
                    .any(|(candidate, _)| candidate == &call.name)
            {
                return Some(format!(
                    "exploration replay direct built-in call `{}({} argument{})` is shadowed at runtime by a different-arity constructor with the same name",
                    call.name,
                    call.effective_arity,
                    if call.effective_arity == 1 { "" } else { "s" }
                ));
            }
            if call.through_pipe {
                if let Some(issue) = explore_replay_pipe_call_site_issue(&call, definitions) {
                    return Some(issue);
                }
            }
            explore_replay_callable_identity_issue(
                &call.name,
                call.effective_arity,
                definitions,
                visiting,
                validated,
            )
        })
}

fn validate_query_replay_callable_identities(
    query: &TypedExploreQuery,
    definitions: &GroundDefinitions,
) -> Vec<Diagnostic> {
    let semantic_case_names = BTreeSet::from([
        "after".to_string(),
        "before".to_string(),
        "context".to_string(),
    ]);
    let mut diagnostics = Vec::new();
    let mut validated = BTreeSet::new();

    let mut check_expression = |expression: &Expr, bound: &BTreeSet<String>| {
        if let Some(message) = expression_replay_callable_identity_issue(
            expression,
            bound,
            definitions,
            &mut BTreeSet::new(),
            &mut validated,
        ) {
            diagnostics.push(Diagnostic::error_at(expression.span, message));
        }
    };

    let mut available_source_names = BTreeSet::new();
    for binding in &query.source.bindings {
        match &binding.kind {
            TypedExploreSourceBindingKind::Singleton { value } => {
                check_expression(value, &available_source_names);
            }
            TypedExploreSourceBindingKind::Finite { domain } => match domain {
                TypedExploreDomain::FiniteExpr { expression, .. } => {
                    check_expression(expression, &available_source_names);
                }
                TypedExploreDomain::Range {
                    start,
                    end_exclusive,
                } => {
                    check_expression(start, &available_source_names);
                    check_expression(end_exclusive, &available_source_names);
                }
                TypedExploreDomain::Values { .. } => {}
            },
        }
        available_source_names.insert(binding.name.clone());
    }

    match &query.successor.kind {
        TypedExploreSuccessorKind::Singleton { value } => {
            check_expression(value, &semantic_case_names);
        }
        TypedExploreSuccessorKind::Finite { domain } => match domain {
            TypedExploreDomain::FiniteExpr { expression, .. } => {
                check_expression(expression, &semantic_case_names);
            }
            TypedExploreDomain::Range {
                start,
                end_exclusive,
            } => {
                check_expression(start, &semantic_case_names);
                check_expression(end_exclusive, &semantic_case_names);
            }
            TypedExploreDomain::Values { .. } => {}
        },
    }

    for admission in &query.admissions {
        check_expression(&admission.predicate, &semantic_case_names);
    }
    match &query.selection {
        TypedExploreSelection::All { .. } => {}
        TypedExploreSelection::Matches { predicate, .. }
        | TypedExploreSelection::Violations { predicate, .. } => {
            check_expression(predicate, &semantic_case_names);
        }
    }

    let mechanism_names = BTreeSet::from(["context".to_string(), "state".to_string()]);
    for node in &query.analysis {
        match node {
            TypedExploreAnalysisNode::Result(view) => {
                let mut view_names = if matches!(&view.input, TypedExploreResultInput::Sources) {
                    BTreeSet::from(["before".to_string(), "context".to_string()])
                } else {
                    semantic_case_names.clone()
                };
                if matches!(
                    &view.input,
                    TypedExploreResultInput::MechanismIncidence { .. }
                ) {
                    view_names.extend([
                        "case_id".to_string(),
                        "transition_id".to_string(),
                        "signature_id".to_string(),
                    ]);
                }
                match &view.grain {
                    TypedExploreResultGrain::EachCase { .. }
                    | TypedExploreResultGrain::EachIncidence { .. }
                    | TypedExploreResultGrain::GroupAll { .. } => {}
                    TypedExploreResultGrain::GroupBy { fields, .. } => {
                        for field in fields {
                            check_expression(&field.value, &view_names);
                            view_names.insert(field.name.clone());
                        }
                    }
                }
                for field in &view.measures {
                    check_expression(&field.value, &view_names);
                    view_names.insert(field.name.clone());
                }
                for field in &view.aggregates {
                    match &field.reducer {
                        TypedExploreAggregateReducer::CountDistinct { value, .. } => {
                            check_expression(value, &view_names);
                        }
                    }
                    view_names.insert(field.name.clone());
                }
                for field in &view.select {
                    check_expression(&field.value, &view_names);
                    view_names.insert(field.name.clone());
                }
                match &view.choose {
                    None => {}
                    Some(TypedExploreResultChoice::Optimize { objective, .. }) => {
                        check_expression(objective, &view_names);
                    }
                    Some(TypedExploreResultChoice::Pareto { objectives, .. }) => {
                        for objective in objectives {
                            check_expression(&objective.value, &view_names);
                        }
                    }
                }
            }
            TypedExploreAnalysisNode::Mechanisms(request) => {
                check_expression(&request.endpoint_template, &mechanism_names);
            }
        }
    }

    diagnostics
}

fn expression_dynamic_helper_dependencies(
    expression: &Expr,
    query_local_names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
    visiting: &mut BTreeSet<(String, usize)>,
    memo: &mut BTreeMap<(String, usize), BTreeSet<String>>,
    work_remaining: &mut usize,
    depth: usize,
) -> BTreeSet<String> {
    if depth >= EXPLORE_RECURSION_LIMIT || *work_remaining == 0 {
        return query_local_names.clone();
    }
    *work_remaining -= 1;
    let mut calls = Vec::new();
    walk_ast_expr(expression, &mut |child| {
        let AstChild::Expr(expression) = child else {
            return;
        };
        let ExprKind::App(function, arguments) = &expression.kind else {
            return;
        };
        let ExprKind::Var(name) = &function.kind else {
            return;
        };
        calls.push((name.clone(), arguments.len()));
    });

    let mut dependencies = BTreeSet::new();
    for (name, arity) in calls {
        if *work_remaining == 0 {
            dependencies.extend(query_local_names.iter().cloned());
            break;
        }
        *work_remaining -= 1;
        let key = (name.clone(), arity);
        if query_local_names.contains(&name) {
            dependencies.insert(name.clone());
        }
        let any_rule = definitions
            .rule_definitions
            .keys()
            .any(|(candidate, _)| candidate == &name);
        let any_function = definitions
            .functions
            .keys()
            .any(|(candidate, _)| candidate == &name);
        let any_unsupported_callable = definitions
            .unsupported_callables
            .keys()
            .any(|(candidate, _)| candidate == &name);
        if definitions.bindings.contains_key(&name) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if any_rule
            && (any_function
                || any_unsupported_callable
                || definitions.unsupported_values.contains_key(&name))
        {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if any_rule {
            if let Some(cached) = memo.get(&key) {
                dependencies.extend(cached.iter().cloned());
                continue;
            }
            let Some(rules) = definitions.rule_definitions.get(&key) else {
                // Runtime rule lookup is name based. If the exact arity cannot
                // be identified, retain every query local conservatively.
                dependencies.extend(query_local_names.iter().cloned());
                continue;
            };
            if !visiting.insert(key.clone()) {
                continue;
            }
            let mut resolved = BTreeSet::new();
            for rule in rules {
                let bound = ground_rule_bound_names(rule);
                for expression in ground_rule_expressions(rule) {
                    let mut free = BTreeSet::new();
                    collect_true_free_vars(expression, &mut free, &bound);
                    free.retain(|name| query_local_names.contains(name));
                    resolved.extend(free);
                    resolved.extend(expression_dynamic_helper_dependencies(
                        expression,
                        query_local_names,
                        definitions,
                        visiting,
                        memo,
                        work_remaining,
                        depth + 1,
                    ));
                }
            }
            visiting.remove(&key);
            memo.insert(key, resolved.clone());
            dependencies.extend(resolved);
            continue;
        }
        if any_unsupported_callable || definitions.unsupported_values.contains_key(&name) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        let all_definitions = definitions
            .functions
            .iter()
            .filter(|((candidate, _), _)| candidate == &name)
            .flat_map(|(_, definitions)| definitions.iter())
            .collect::<Vec<_>>();
        if all_definitions.is_empty() {
            continue;
        }
        if let Some(cached) = memo.get(&key) {
            dependencies.extend(cached.iter().cloned());
            continue;
        }
        let exact = definitions.functions.get(&key);
        if all_definitions.len() != 1 || exact.is_none_or(|definitions| definitions.len() != 1) {
            dependencies.extend(query_local_names.iter().cloned());
            continue;
        }
        if !visiting.insert(key.clone()) {
            continue;
        }
        let definition = &exact.expect("one exact helper definition")[0];
        let bound = definition
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<BTreeSet<_>>();
        let mut free = BTreeSet::new();
        collect_true_free_vars(&definition.body, &mut free, &bound);
        free.retain(|name| query_local_names.contains(name));
        let mut resolved = free;
        resolved.extend(expression_dynamic_helper_dependencies(
            &definition.body,
            query_local_names,
            definitions,
            visiting,
            memo,
            work_remaining,
            depth + 1,
        ));
        visiting.remove(&key);
        memo.insert(key, resolved.clone());
        dependencies.extend(resolved);
    }
    dependencies
}

fn deduplicate_runtime_list(values: Vec<ExploreValue>) -> Vec<ExploreValue> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.runtime_display_key()))
        .collect()
}

fn runtime_set_map(values: Vec<ExploreValue>) -> BTreeMap<String, ExploreValue> {
    let mut set = BTreeMap::new();
    for value in values {
        set.entry(value.runtime_display_key()).or_insert(value);
    }
    set
}

fn runtime_set_values(values: Vec<ExploreValue>) -> Vec<ExploreValue> {
    runtime_set_map(values).into_values().collect()
}

fn exact_range_cardinality(start: i64, end_exclusive: i64) -> Result<u64, String> {
    if start > end_exclusive {
        return Err(format!(
            "exploration range start {} is greater than end {}",
            start, end_exclusive
        ));
    }
    let distance = (end_exclusive as i128) - (start as i128);
    u64::try_from(distance).map_err(|_| {
        format!(
            "exploration range {}..{} has a cardinality that cannot be represented",
            start, end_exclusive
        )
    })
}

pub(crate) fn elaborate_queries(
    statements: &[Stmt],
    source_dir: Option<&str>,
    queries: &[TypedExploreQuery],
    rule_dispatch_return_types: &BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_return_issues: &BTreeMap<RuleDispatchKey, String>,
    rule_dispatch_boolean_miss_safe_keys: &BTreeSet<RuleDispatchKey>,
    explore_rule_return_types_by_arity: &BTreeMap<(String, usize), Ty>,
    explore_rule_return_issues: &BTreeMap<(String, usize), String>,
    validate_replay_callables: bool,
) -> Result<Vec<ExploreQueryIr>, Vec<Diagnostic>> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }
    let catalog_statements = prepend_prelude(parse_prelude(), statements);
    let catalog = calculate::TypeCatalog::collect_checked(&catalog_statements, source_dir)
        .map_err(|errors| {
            errors
                .into_iter()
                .map(Diagnostic::error)
                .collect::<Vec<_>>()
        })?;
    let mut definitions = collect_ground_bindings(statements, source_dir).map_err(|errors| {
        errors
            .into_iter()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;
    definitions.rule_dispatch_return_types = rule_dispatch_return_types.clone();
    definitions.rule_dispatch_return_issues = rule_dispatch_return_issues.clone();
    definitions.rule_dispatch_boolean_miss_safe_keys = rule_dispatch_boolean_miss_safe_keys.clone();
    definitions.explore_rule_return_types_by_arity = explore_rule_return_types_by_arity.clone();
    definitions.explore_rule_return_issues = explore_rule_return_issues.clone();
    let mut closed_queries = Vec::with_capacity(queries.len());
    let mut diagnostics = Vec::new();

    for query in queries {
        match elaborate_query(query, &catalog, &definitions, validate_replay_callables) {
            Ok(closed_query) => closed_queries.push(closed_query),
            Err(mut query_diagnostics) => diagnostics.append(&mut query_diagnostics),
        }
    }
    if diagnostics.is_empty() {
        Ok(closed_queries)
    } else {
        Err(diagnostics)
    }
}

fn lower_explore_finite_domain(
    domain: &TypedExploreDomain,
    catalog: &calculate::TypeCatalog,
    path: &str,
) -> Result<ExploreFiniteDomainIr, String> {
    match domain {
        TypedExploreDomain::FiniteExpr {
            expression,
            collection_ty,
            element_ty,
        } => Ok(ExploreFiniteDomainIr::Collection {
            expression: expression.clone(),
            collection_ty: collection_ty.clone(),
            element_ty: element_ty.clone(),
        }),
        TypedExploreDomain::Range {
            start,
            end_exclusive,
        } => Ok(ExploreFiniteDomainIr::IntRange {
            start: start.clone(),
            end_exclusive: end_exclusive.clone(),
        }),
        TypedExploreDomain::Values { ty } => {
            let plan = finite_type_plan(ty, catalog, path, &mut BTreeSet::new())?;
            Ok(ExploreFiniteDomainIr::Exact(
                ExploreExactDomain::FiniteType {
                    ty: ty.clone(),
                    plan,
                },
            ))
        }
    }
}

fn typed_explore_domain_dependencies(
    domain: &TypedExploreDomain,
    all_source_names: &BTreeSet<String>,
    definitions: &GroundDefinitions,
) -> BTreeSet<String> {
    match domain {
        TypedExploreDomain::FiniteExpr { expression, .. } => {
            expression_query_dependencies(expression, all_source_names, definitions)
        }
        TypedExploreDomain::Range {
            start,
            end_exclusive,
        } => {
            let mut dependencies =
                expression_query_dependencies(start, all_source_names, definitions);
            dependencies.extend(expression_query_dependencies(
                end_exclusive,
                all_source_names,
                definitions,
            ));
            dependencies
        }
        TypedExploreDomain::Values { .. } => BTreeSet::new(),
    }
}

fn lower_source_binding_dependencies(
    binding: &TypedExploreSourceBinding,
    binding_index: usize,
    all_source_names: &BTreeSet<String>,
    source_indices: &BTreeMap<String, usize>,
    definitions: &GroundDefinitions,
) -> Result<Box<[ExploreSourceDependencyIr]>, String> {
    let dependency_names = match &binding.kind {
        TypedExploreSourceBindingKind::Singleton { value } => {
            expression_query_dependencies(value, all_source_names, definitions)
        }
        TypedExploreSourceBindingKind::Finite { domain } => {
            typed_explore_domain_dependencies(domain, all_source_names, definitions)
        }
    };
    let mut dependencies = Vec::with_capacity(dependency_names.len());
    for dependency_name in dependency_names {
        let dependency_index = source_indices
            .get(&dependency_name)
            .copied()
            .ok_or_else(|| {
                format!(
                    "source binding {} has unresolved dependency {}",
                    binding.name, dependency_name
                )
            })?;
        if dependency_index >= binding_index {
            return Err(format!(
                "source binding {} depends on non-earlier binding {}",
                binding.name, dependency_name
            ));
        }
        dependencies.push(ExploreSourceDependencyIr {
            binding_index: dependency_index,
            binding_name: dependency_name,
        });
    }
    dependencies.sort_by_key(|dependency| dependency.binding_index);
    Ok(dependencies.into_boxed_slice())
}

fn lower_result_field(field: &TypedExploreResultField) -> ExploreResultFieldIr {
    ExploreResultFieldIr {
        name: field.name.clone(),
        value: field.value.clone(),
        ty: field.ty.clone(),
        span: field.span,
    }
}

fn lower_aggregate_field(field: &TypedExploreAggregateField) -> ExploreAggregateFieldIr {
    ExploreAggregateFieldIr {
        name: field.name.clone(),
        reducer: match &field.reducer {
            TypedExploreAggregateReducer::CountDistinct { value, value_ty } => {
                ExploreAggregateReducerIr::CountDistinct {
                    value: value.clone(),
                    value_ty: value_ty.clone(),
                }
            }
        },
        ty: field.ty.clone(),
        span: field.span,
    }
}

fn lower_result_grain(grain: &TypedExploreResultGrain) -> ExploreResultGrainIr {
    match grain {
        TypedExploreResultGrain::EachCase { span } => {
            ExploreResultGrainIr::EachCase { span: *span }
        }
        TypedExploreResultGrain::EachIncidence { span } => {
            ExploreResultGrainIr::EachIncidence { span: *span }
        }
        TypedExploreResultGrain::GroupAll { span } => {
            ExploreResultGrainIr::GroupAll { span: *span }
        }
        TypedExploreResultGrain::GroupBy { fields, span } => ExploreResultGrainIr::GroupBy {
            fields: fields
                .iter()
                .map(lower_result_field)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            span: *span,
        },
    }
}

fn lower_result_choice(choice: &TypedExploreResultChoice) -> ExploreResultChoiceIr {
    match choice {
        TypedExploreResultChoice::Optimize {
            cardinality,
            direction,
            objective,
            objective_ty,
            span,
        } => ExploreResultChoiceIr::Optimize {
            cardinality: *cardinality,
            direction: *direction,
            objective: objective.clone(),
            objective_ty: objective_ty.clone(),
            span: *span,
        },
        TypedExploreResultChoice::Pareto { objectives, span } => ExploreResultChoiceIr::Pareto {
            objectives: objectives
                .iter()
                .map(|objective| ExploreParetoObjectiveIr {
                    direction: objective.direction,
                    value: objective.value.clone(),
                    ty: objective.ty.clone(),
                    span: objective.span,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            span: *span,
        },
    }
}

fn lower_result_view(view: &TypedExploreResultView, node_index: usize) -> ExploreResultViewIr {
    ExploreResultViewIr {
        node_index,
        name: view.name.clone(),
        input: match &view.input {
            TypedExploreResultInput::Sources => ExploreResultInputIr::Sources,
            TypedExploreResultInput::Selected => ExploreResultInputIr::Selected,
            TypedExploreResultInput::MechanismIncidence {
                request_node_index, ..
            } => ExploreResultInputIr::MechanismIncidence {
                request_node_index: *request_node_index,
            },
        },
        grain: lower_result_grain(&view.grain),
        measures: view
            .measures
            .iter()
            .map(lower_result_field)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        aggregates: view
            .aggregates
            .iter()
            .map(lower_aggregate_field)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        having: view.having.as_ref().map(|having| match having {
            TypedExploreResultHaving::Varies {
                measure_name,
                measure_index,
                span,
            } => ExploreResultHavingIr::Varies {
                measure_name: measure_name.clone(),
                measure_index: *measure_index,
                span: *span,
            },
        }),
        select: view
            .select
            .iter()
            .map(lower_result_field)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        choose: view.choose.as_ref().map(lower_result_choice),
        span: view.span,
    }
}

fn elaborate_query(
    query: &TypedExploreQuery,
    catalog: &calculate::TypeCatalog,
    definitions: &GroundDefinitions,
    validate_replay_callables: bool,
) -> Result<ExploreQueryIr, Vec<Diagnostic>> {
    if validate_replay_callables {
        let replay_diagnostics = validate_query_replay_callable_identities(query, definitions);
        if !replay_diagnostics.is_empty() {
            return Err(replay_diagnostics);
        }
    }

    let all_source_names = query
        .source
        .bindings
        .iter()
        .map(|binding| binding.name.clone())
        .collect::<BTreeSet<_>>();
    let mut source_indices = BTreeMap::new();
    for (binding_index, binding) in query.source.bindings.iter().enumerate() {
        if source_indices
            .insert(binding.name.clone(), binding_index)
            .is_some()
        {
            return Err(vec![Diagnostic::error_at(
                binding.span,
                format!("duplicate exploration source binding {}", binding.name),
            )]);
        }
    }

    let mut source_bindings = Vec::with_capacity(query.source.bindings.len());
    for (binding_index, binding) in query.source.bindings.iter().enumerate() {
        let dependencies = lower_source_binding_dependencies(
            binding,
            binding_index,
            &all_source_names,
            &source_indices,
            definitions,
        )
        .map_err(|message| vec![Diagnostic::error_at(binding.span, message)])?;
        let kind = match &binding.kind {
            TypedExploreSourceBindingKind::Singleton { value } => {
                ExploreSourceBindingKindIr::Singleton {
                    value: value.clone(),
                }
            }
            TypedExploreSourceBindingKind::Finite { domain } => {
                ExploreSourceBindingKindIr::Finite {
                    domain: lower_explore_finite_domain(
                        domain,
                        catalog,
                        &format!("source.{}", binding.name),
                    )
                    .map_err(|message| vec![Diagnostic::error_at(binding.span, message)])?,
                }
            }
        };
        let role = if binding_index == query.source.context_binding_index {
            ExploreSourceBindingRoleIr::Context
        } else if binding_index == query.source.before_binding_index {
            ExploreSourceBindingRoleIr::Before
        } else {
            ExploreSourceBindingRoleIr::Auxiliary
        };
        source_bindings.push(ExploreSourceBindingIr {
            binding_index,
            name: binding.name.clone(),
            value_ty: binding.value_ty.clone(),
            role,
            dependencies,
            kind,
            span: binding.span,
        });
    }

    let successor_kind = match &query.successor.kind {
        TypedExploreSuccessorKind::Singleton { value } => ExploreSuccessorKindIr::Singleton {
            value: value.clone(),
        },
        TypedExploreSuccessorKind::Finite { domain } => ExploreSuccessorKindIr::Finite {
            domain: lower_explore_finite_domain(domain, catalog, "successor")
                .map_err(|message| vec![Diagnostic::error_at(query.successor.span, message)])?,
        },
    };

    let find = match &query.selection {
        TypedExploreSelection::All { span } => ExploreFindIr::All { span: *span },
        TypedExploreSelection::Matches { predicate, span } => ExploreFindIr::Matches {
            predicate: predicate.clone(),
            span: *span,
        },
        TypedExploreSelection::Violations { predicate, span } => ExploreFindIr::Violations {
            predicate: predicate.clone(),
            span: *span,
        },
    };

    let mut analysis = Vec::with_capacity(query.analysis.len());
    for (node_index, node) in query.analysis.iter().enumerate() {
        analysis.push(match node {
            TypedExploreAnalysisNode::Result(view) => {
                ExploreAnalysisNodeIr::Result(lower_result_view(view, node_index))
            }
            TypedExploreAnalysisNode::Mechanisms(request) => {
                let target = match &request.target {
                    TypedExploreMechanismTarget::SelectedCases => {
                        ExploreMechanismTargetIr::SelectedCases
                    }
                    TypedExploreMechanismTarget::ViewChosen {
                        view_node_index, ..
                    } => ExploreMechanismTargetIr::ViewChosen {
                        view_node_index: *view_node_index,
                    },
                };
                ExploreAnalysisNodeIr::Mechanisms(ExploreMechanismRequestIr {
                    node_index,
                    name: request.name.clone(),
                    target,
                    callable_name: request.callable_name.clone(),
                    endpoint_template: request.endpoint_template.clone(),
                    observation_ty: request.observation_ty.clone(),
                    span: request.span,
                })
            }
        });
    }

    let closed = ExploreQueryIr {
        name: query.name.clone(),
        source: ExploreSourceRelationIr {
            normalization_version: query.source.normalization_version,
            multiplicity: query.source.multiplicity,
            bindings: source_bindings.into_boxed_slice(),
            context_binding_index: query.source.context_binding_index,
            before_binding_index: query.source.before_binding_index,
            context_ty: query.source.context_ty.clone(),
            before_ty: query.source.before_ty.clone(),
        },
        successor: ExploreSuccessorRelationIr {
            multiplicity: query.successor.multiplicity,
            after_ty: query.successor.after_ty.clone(),
            kind: successor_kind,
            span: query.successor.span,
        },
        admissions: query
            .admissions
            .iter()
            .enumerate()
            .map(|(admission_index, admission)| ExploreAdmissionIr {
                admission_index,
                scope: admission.scope,
                predicate: admission.predicate.clone(),
                span: admission.span,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        find,
        analysis: analysis.into_boxed_slice(),
        starter_projections: query
            .starter_projections
            .iter()
            .map(ExploreStarterProjectionIr::lower)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        span: query.span,
    };
    closed
        .validate()
        .map_err(|message| vec![Diagnostic::error_at(query.span, message)])?;
    Ok(closed)
}

/// Render a small result through the one canonical exact evaluator. This
/// hidden command is a presentation adapter only: it owns no transition,
/// eligibility, question, or output semantics.
#[cfg(any())]
pub fn execute_exhaustive_preview(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    case_limit: usize,
) -> Result<ExplorePreviewReport, String> {
    if case_limit == 0 {
        return Err("exploration preview limit must be positive".to_string());
    }
    let budget = report::ExploreExecutionBudget::new(
        Some(case_limit as u128),
        report::DEFAULT_EXPLORE_STEP_LIMIT,
        report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
    )?;
    let exact = exact::execute_exact_finite(
        statements,
        source_dir,
        artifacts,
        accepted_query_index,
        report::ExploreReportRequest {
            search_decision_dag: report::ExploreSearchDecisionDagRequest::Omit,
            semantic_transition_graph: report::ExploreSemanticTransitionGraphRequest::Omit,
            ledger: report::ExploreLedgerRequest::MatchingConfigurations,
        },
        budget,
    )?;
    let report::ExploreExactReport {
        query_name,
        polarity,
        outcome,
        ..
    } = exact;
    let evidence = match outcome {
        report::ExploreExactOutcome::Complete { evidence, .. } => evidence,
        report::ExploreExactOutcome::Partial { stop, .. } => {
            return Err(format!(
                "exploration did not complete within preview limit {case_limit}: {stop:?}"
            ));
        }
        report::ExploreExactOutcome::Unknown { reason, .. } => return Err(reason),
        report::ExploreExactOutcome::Unsupported { diagnostic } => return Err(diagnostic),
        report::ExploreExactOutcome::Error { diagnostics } => {
            return Err(diagnostics.into_vec().join("; "));
        }
    };
    let exact_u64 = |name: &str, count: report::ExploreCount| {
        count
            .exact()
            .ok_or_else(|| format!("complete exploration has non-exact {name}"))
            .and_then(|value| {
                u64::try_from(value).map_err(|_| format!("exploration {name} exceeds u64::MAX"))
            })
    };
    let declared_assignments = exact_u64(
        "declared assignment count",
        evidence.counts.declared_assignments,
    )?;
    let eligible_configurations = exact_u64(
        "admissible configuration count",
        evidence.counts.admissible_configurations,
    )?;
    let matching_configurations = exact_u64(
        "matching configuration count",
        evidence.counts.matching_configurations,
    )?;
    let distinct_keys = exact_u64(
        "distinct result-key count",
        evidence.counts.distinct_result_keys,
    )?;
    let rows = match evidence.ledger {
        report::ExploreLedgerEvidence::MatchingConfigurations { rows } => rows
            .into_vec()
            .into_iter()
            .map(|row| ExplorePreviewRow {
                inputs: evidence
                    .schema
                    .dimensions
                    .iter()
                    .map(report::ExploreReportDimension::qualified_label)
                    .zip(row.dimensions.into_vec())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
                key: evidence
                    .schema
                    .key_names
                    .iter()
                    .cloned()
                    .zip(row.key.values().iter().cloned())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
                shown: evidence
                    .schema
                    .shown_names
                    .iter()
                    .cloned()
                    .zip(row.shown.into_vec())
                    .map(|(name, value)| ExplorePreviewField { name, value })
                    .collect(),
            })
            .collect::<Vec<_>>(),
        report::ExploreLedgerEvidence::Omitted => {
            return Err("canonical preview execution omitted its requested matching ledger".into());
        }
    };
    let mut rows = rows;
    rows.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.inputs.cmp(&right.inputs))
    });
    Ok(ExplorePreviewReport {
        query_name,
        polarity,
        declared_assignments,
        eligible_configurations,
        evaluated_configurations: eligible_configurations,
        matching_configurations,
        distinct_keys,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifacts(source: &str) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse explore domain fixture");
        TypeChecker::check_with_artifacts(&statements, None, source)
    }

    fn artifacts_with_dir(source: &str, source_dir: &Path) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse imported explore domain fixture");
        TypeChecker::check_with_artifacts(
            &statements,
            Some(source_dir.to_string_lossy().to_string()),
            source,
        )
    }

    #[test]
    fn relational_expression_step_retry_is_adaptive_and_hard_bounded() {
        let mut step_limit = RELATIONAL_EXPRESSION_INITIAL_STEP_LIMIT;
        let mut attempts = Vec::new();
        let value =
            evaluate_relational_expression_with_bounded_retry(&mut step_limit, |attempted_limit| {
                attempts.push(attempted_limit);
                if attempted_limit < RELATIONAL_EXPRESSION_HARD_STEP_LIMIT {
                    Err(ExploreRuntimeFailure::OperationalLimit {
                        resource: ExploreRuntimeResource::ExpressionSteps,
                        limit: attempted_limit as u128,
                        observed: attempted_limit as u128 + 1,
                    })
                } else {
                    Ok(17)
                }
            })
            .expect("the expression fits the hard relational step bound");
        assert_eq!(value, 17);
        assert_eq!(
            attempts,
            [
                RELATIONAL_EXPRESSION_INITIAL_STEP_LIMIT,
                RELATIONAL_EXPRESSION_INITIAL_STEP_LIMIT * 2,
                RELATIONAL_EXPRESSION_HARD_STEP_LIMIT,
            ]
        );
        assert_eq!(step_limit, RELATIONAL_EXPRESSION_HARD_STEP_LIMIT);

        let mut warm_attempts = Vec::new();
        let value =
            evaluate_relational_expression_with_bounded_retry(&mut step_limit, |attempted_limit| {
                warm_attempts.push(attempted_limit);
                Ok::<_, ExploreRuntimeFailure>(23)
            })
            .expect("the learned bound is reused by later expressions");
        assert_eq!(value, 23);
        assert_eq!(warm_attempts, [RELATIONAL_EXPRESSION_HARD_STEP_LIMIT]);

        let error =
            evaluate_relational_expression_with_bounded_retry(&mut step_limit, |attempted_limit| {
                Err::<(), _>(ExploreRuntimeFailure::OperationalLimit {
                    resource: ExploreRuntimeResource::ExpressionSteps,
                    limit: attempted_limit as u128,
                    observed: attempted_limit as u128 + 1,
                })
            })
            .expect_err("the hard relational step bound remains terminal");
        assert_eq!(
            error,
            ExploreRuntimeFailure::OperationalLimit {
                resource: ExploreRuntimeResource::ExpressionSteps,
                limit: RELATIONAL_EXPRESSION_HARD_STEP_LIMIT as u128,
                observed: RELATIONAL_EXPRESSION_HARD_STEP_LIMIT as u128 + 1,
            }
        );
    }

    // Cartesian executor regression history. The relational executor owns the
    // active behavior suite; these tests leave the build graph with the route
    // they exercised.
    #[rustfmt::skip]
    #[cfg(any())]
    mod retired_cartesian_execution_tests {
    use super::*;

    #[test]
    fn exact_evaluator_constructs_explicit_after_fields_in_dag_order() {
        let source = r#"
# OrderedState = OrderedState(earlier: Int, later: Int)

> later(value: Int) -> Int { value + 1 }

| changed(before: OrderedState, after: OrderedState, context: ()) ->
    after.earlier > before.earlier

? explore forward_dependency {
    over changed(before, after, context)
    find matches
    bounds {
        before.earlier = 0
        before.later in range(0, 2)
    }
    transition as OrderedState context () {
        after.earlier = after.later + later(before.earlier)
        after.later = later(before.later)
    }
    output {
        key [later = before.later]
        show [earlier_after = after.earlier, later_after = after.later]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse explicit after-DAG fixture");
        let report = execute_checked_exact(
            &statements,
            None,
            source,
            Some("forward_dependency"),
            ExploreExactOptions {
                case_limit: NonZeroU128::new(2).unwrap(),
            },
        )
        .expect("execute the two-case explicit transition");
        let evidence = match report.outcome {
            ExploreExecutionOutcome::Complete { evidence, .. } => evidence,
            outcome => panic!("explicit after-DAG fixture did not close: {outcome:?}"),
        };

        assert_eq!(evidence.dimensions.len(), 1);
        assert_eq!(evidence.dimensions[0].qualified_label(), "before.later");
        assert_eq!(evidence.dimensions[0].bound_index, 1);
        assert_eq!(
            evidence.dimensions[0].role,
            ExploreGeneratorAxisRole::Before
        );
        assert_eq!(evidence.dimensions[0].role_field_index, 1);
        assert_eq!(evidence.axis_cardinalities, [2]);
        assert_eq!(
            evidence.counts,
            ExploreExecutionCounts {
                declared_assignments: ExploreCountEvidence::Exact(2),
                admissible_configurations: ExploreCountEvidence::Exact(2),
                matching_configurations: ExploreCountEvidence::Exact(2),
                distinct_result_keys: ExploreCountEvidence::Exact(2),
            }
        );
        assert_eq!(evidence.results.len(), 2);
        assert_eq!(evidence.results[0].key[0].value, ExploreValue::Int(0));
        assert_eq!(
            evidence.results[0]
                .shown
                .iter()
                .map(|field| field.value.clone())
                .collect::<Vec<_>>(),
            [ExploreValue::Int(2), ExploreValue::Int(1)]
        );
        assert_eq!(evidence.results[1].key[0].value, ExploreValue::Int(1));
        assert_eq!(
            evidence.results[1]
                .shown
                .iter()
                .map(|field| field.value.clone())
                .collect::<Vec<_>>(),
            [ExploreValue::Int(3), ExploreValue::Int(2)]
        );
    }

    #[test]
    fn exact_evaluator_executes_independent_fanout_and_scoped_constraints() {
        let source = r#"
# IncomeState = IncomeState(income: Int, municipality: Int)
# IncomeContext = IncomeContext(step: Int)

| changed(before: IncomeState, after: IncomeState, context: IncomeContext) ->
    after.income >= before.income under context.step > 0

? explore municipality_fanout {
    over changed(before, after, context)
    find matches
    bounds {
        context.step = 1
        before.income in range(0, 4)
        before.municipality = 1
        where before before.income >= 1
        where after after.income <= 3
        where transition after.municipality != before.municipality
    }
    transition as IncomeState context IncomeContext {
        after.income = before.income + context.step
        after.municipality in [1, 2]
    }
    output {
        key [income = before.income]
        show [municipality = after.municipality, after_income = after.income]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse independent transition fixture");
        let report = execute_checked_exact(
            &statements,
            None,
            source,
            Some("municipality_fanout"),
            ExploreExactOptions {
                case_limit: NonZeroU128::new(8).unwrap(),
            },
        )
        .expect("execute the independent transition fanout");
        let evidence = match report.outcome {
            ExploreExecutionOutcome::Complete { evidence, .. } => evidence,
            outcome => panic!("independent transition fixture did not close: {outcome:?}"),
        };

        assert_eq!(
            evidence
                .dimensions
                .iter()
                .map(ExploreExecutionDimension::qualified_label)
                .collect::<Vec<_>>(),
            ["before.income", "after.municipality"]
        );
        assert_eq!(evidence.axis_cardinalities, [4, 2]);
        assert_eq!(
            evidence.counts,
            ExploreExecutionCounts {
                declared_assignments: ExploreCountEvidence::Exact(8),
                admissible_configurations: ExploreCountEvidence::Exact(2),
                matching_configurations: ExploreCountEvidence::Exact(2),
                distinct_result_keys: ExploreCountEvidence::Exact(2),
            }
        );
        assert_eq!(evidence.results.len(), 2);
        for (row, expected_income) in evidence.results.iter().zip([1_i64, 2]) {
            assert_eq!(row.key[0].value, ExploreValue::Int(expected_income));
            assert_eq!(row.shown[0].value, ExploreValue::Int(2));
            assert_eq!(row.shown[1].value, ExploreValue::Int(expected_income + 1));
        }
    }

    #[test]
    fn explicit_boundary_context_step_excludes_overflow_before_after_construction() {
        let source = r#"
# BoundaryState = BoundaryState(income: Int)
# BoundaryContext = BoundaryContext(step: Int)

| changed(before: BoundaryState, after: BoundaryState, context: BoundaryContext) ->
    after.income > before.income under context.step > 0

? explore overflow_guard {
    over changed(before, after, context)
    find matches
    bounds {
        context.step = 1
        before.income in [9223372036854775807]
    }
    transition as BoundaryState context BoundaryContext {
        after.income = before.income + context.step
    }
    boundaries on before.income by context.step
    output {
        key [income = before.income]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse explicit boundary-overflow fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let mut typed_without_hint = artifacts.exploration_queries[0].clone();
        typed_without_hint.transition.boundary_hint = None;
        let catalog_statements = prepend_prelude(parse_prelude(), &statements);
        let catalog = calculate::TypeCatalog::collect_checked(&catalog_statements, None)
            .expect("collect boundary-overflow catalog");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect boundary-overflow bindings");
        let (universe, transition) =
            elaborate_query(&typed_without_hint, &catalog, definitions, false)
                .expect("lower typed boundary-overflow query without optimizer hint");
        let closed_without_hint = ExploreQueryIr {
            query: typed_without_hint,
            transition,
            universe,
        };
        assert!(closed_without_hint.transition.boundary_hint.is_none());
        assert_eq!(
            closed_without_hint.transition.after_membership,
            artifacts.exploration_universes[0]
                .transition
                .after_membership,
            "removing the typed optimizer hint before closing must preserve canonical membership"
        );
        assert_eq!(
            exact::endpoint_memberships_are_structurally_eligible(
                &closed_without_hint,
                &[ExploreValue::Int(i64::MAX)],
            ),
            Ok(false),
            "canonical membership, not optimizer metadata, must close overflow"
        );
        let report = execute_checked_exact(
            &statements,
            None,
            source,
            Some("overflow_guard"),
            ExploreExactOptions {
                case_limit: NonZeroU128::new(1).unwrap(),
            },
        )
        .expect("close the one-coordinate boundary-overflow fixture");
        let evidence = match report.outcome {
            ExploreExecutionOutcome::Complete { evidence, .. } => evidence,
            outcome => panic!("boundary-overflow fixture did not close: {outcome:?}"),
        };

        assert_eq!(
            evidence.counts,
            ExploreExecutionCounts {
                declared_assignments: ExploreCountEvidence::Exact(1),
                admissible_configurations: ExploreCountEvidence::Exact(0),
                matching_configurations: ExploreCountEvidence::Exact(0),
                distinct_result_keys: ExploreCountEvidence::Exact(0),
            }
        );
        assert!(evidence.results.is_empty());
    }

    #[test]
    fn exact_stream_evaluator_rejects_caller_root_drift_after_checking() {
        let checked_source = r#"
> score(value: Int) -> Int { value }
| eligible(value: Int) -> True
? explore immutable_runtime_root_fixture {
    over eligible(value)
    find matches
    bounds { value in [1] }
    output { key [value] show [result = score(value)] representative first }
}
"#;
        let mut lexer = Lexer::new(checked_source);
        let checked_statements = Parser::new(lexer.tokenize(), checked_source)
            .parse_program()
            .expect("parse checked immutable-root fixture");
        let artifacts =
            TypeChecker::check_with_artifacts(&checked_statements, None, checked_source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );

        let drifted_source = checked_source.replace("{ value }", "{ value + 1 }");
        let mut lexer = Lexer::new(&drifted_source);
        let drifted_statements = Parser::new(lexer.tokenize(), &drifted_source)
            .parse_program()
            .expect("parse drifted immutable-root fixture");
        let error = exact::ExactStreamEvaluator::prepare(
            &drifted_statements,
            None,
            &artifacts,
            0,
            report::DEFAULT_EXPLORE_STEP_LIMIT,
            report::DEFAULT_EXPLORE_COLLECTION_LIMIT,
        )
        .err()
        .expect("caller root drift must not replace the checked runtime snapshot");
        assert!(error.contains("runtime entry syntax differs"), "{error}");
    }

    #[test]
    fn durable_checkpoint_pause_resume_finalize_and_reopen_is_idempotent() {
        let source = r#"
| condition(value: Int) -> True
? explore one_case_stream {
    over condition(value)
    find matches
    bounds { value in [7] }
    output { key [value] representative first }
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse one-case durable-stream fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let selected = 0;
        let query = &artifacts.exploration_universes[selected];
        let graph_request = report::ExploreReportRequest {
            search_decision_dag: report::ExploreSearchDecisionDagRequest::Include,
            semantic_transition_graph: report::ExploreSemanticTransitionGraphRequest::Include,
            ledger: report::ExploreLedgerRequest::Omit,
        };
        assert_eq!(
            query.universe.cartesian_count_before_constraints,
            ExploreCardinality::Exact(1)
        );

        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_durable_lifecycle_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("create one-case durable stream");

        match source_proof_plan::prepare_source_proof_plan(
            &artifacts,
            selected,
            source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT,
        ) {
            Ok(plan) => {
                coordinator
                    .persist_source_probe_manifest(&plan)
                    .expect("persist source-probe manifest");
            }
            Err(error) if error.permits_canonical_fallback() => {
                coordinator
                    .persist_probe_fallback_manifest()
                    .expect("persist canonical probe fallback");
            }
            Err(error) => panic!("one-case source probe failed closed: {error:?}"),
        }
        coordinator
            .accept_prepared_probe_coverage(NonZeroU64::new(1).expect("one is nonzero"))
            .expect("accept one-case probe coverage");
        let probe_progress = coordinator
            .complete_prepared_probe()
            .expect("complete one-case source probe");
        assert!(probe_progress.complete());

        let prepared_checkpoint = coordinator
            .prepare_observable_snapshot_publication_for_test()
            .expect("prepare one-case checkpoint");
        let checkpoint = publish_prepared_snapshot_and_pause_exact_stream_slice(
            &mut coordinator,
            prepared_checkpoint,
            run_stream::PauseReason::ProbeMilestone,
            ExploreStreamSliceStop::ProbeMilestone,
            0,
            0,
        )
        .expect("publish and pause one-case checkpoint");
        assert_eq!(checkpoint.stop, ExploreStreamSliceStop::ProbeMilestone);
        assert_eq!(
            checkpoint.final_cursor.lifecycle,
            ExploreStreamLifecycle::Paused
        );
        assert!(checkpoint.probe_milestone_complete);
        let (checkpoint_cursor, publication_cursor) = match &checkpoint.artifact {
            ExploreStreamArtifact::CheckpointSnapshotJsonLine {
                canonical_json_line,
                checkpoint_cursor,
                publication_cursor,
                ..
            } => {
                assert!(canonical_json_line.ends_with(b"\n"));
                assert_eq!(
                    canonical_json_line
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count(),
                    1
                );
                let rendered =
                    std::str::from_utf8(canonical_json_line).expect("checkpoint JSON is UTF-8");
                assert!(rendered.contains("\"search_decision_dag\":\"full\""));
                assert!(rendered.contains("\"semantic_transition_graph\":\"full\""));
                assert!(rendered
                    .contains("\"schema\":\"futuruna.explore.semantic-transition-graph.v1\""));
                assert!(rendered.contains("\"status\":\"included\""));
                assert!(rendered.contains("\"classification\":\"eligibility_open\""));
                assert!(rendered.contains("\"views\":\"open\""));
                (checkpoint_cursor, publication_cursor)
            }
            ExploreStreamArtifact::TerminalResultJson { .. } => {
                panic!("probe milestone returned a terminal artifact")
            }
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine { .. } => {
                panic!("one-case probe checkpoint unexpectedly hit snapshot capacity")
            }
            ExploreStreamArtifact::JournalOnlyCheckpoint { .. } => {
                panic!("direct graph-bearing checkpoint publication was deferred")
            }
        };
        assert_eq!(checkpoint_cursor.lifecycle, ExploreStreamLifecycle::Running);
        assert_eq!(
            publication_cursor.lifecycle,
            ExploreStreamLifecycle::Running
        );
        assert_eq!(
            publication_cursor.sequence,
            checkpoint_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_eq!(
            checkpoint.final_cursor.sequence,
            publication_cursor
                .sequence
                .checked_add(1)
                .expect("sequence")
        );
        assert_eq!(checkpoint_cursor.run_id, publication_cursor.run_id);
        assert_eq!(checkpoint_cursor.run_id, checkpoint.final_cursor.run_id);
        assert_ne!(
            checkpoint_cursor.journal_head,
            publication_cursor.journal_head
        );
        assert_ne!(
            publication_cursor.journal_head,
            checkpoint.final_cursor.journal_head
        );
        assert_eq!(
            checkpoint_cursor.evidence_root,
            publication_cursor.evidence_root
        );
        assert_eq!(
            publication_cursor.evidence_root,
            checkpoint.final_cursor.evidence_root
        );
        let paused_cursor = checkpoint.final_cursor.clone();
        drop(coordinator);

        let mismatch = match stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            report::ExploreReportRequest::baseline(),
        ) {
            Ok(_) => panic!("search decision DAG authorization is immutable run identity"),
            Err(error) => error,
        };
        assert!(mismatch
            .to_string()
            .contains("stored Explore stream header does not match"));

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume one-case durable stream");
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        let resumed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(resumed_cursor.lifecycle, ExploreStreamLifecycle::Running);
        assert_eq!(resumed_cursor.run_id, paused_cursor.run_id);
        assert_eq!(
            resumed_cursor.sequence,
            paused_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_ne!(resumed_cursor.journal_head, paused_cursor.journal_head);
        assert_eq!(resumed_cursor.evidence_root, paused_cursor.evidence_root);

        let journal_only = pause_exact_stream_slice_without_snapshot(
            &mut coordinator,
            run_stream::PauseReason::TimeLimit,
            ExploreStreamSliceStop::TimeLimit,
            ExploreStreamObserverDeferral::TimeLimit,
            0,
            0,
        )
        .expect("pause one-case stream without materializing another snapshot");
        assert_eq!(
            journal_only.final_cursor.sequence,
            resumed_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert!(matches!(
            journal_only.artifact,
            ExploreStreamArtifact::JournalOnlyCheckpoint {
                observer_deferral: ExploreStreamObserverDeferral::TimeLimit,
            }
        ));
        assert!(coordinator.pending_observable_snapshot_on_resume());
        let journal_only_cursor = journal_only.final_cursor;
        drop(coordinator);

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume journal-only one-case checkpoint");
        let resumed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(
            resumed_cursor.sequence,
            journal_only_cursor
                .sequence
                .checked_add(1)
                .expect("sequence")
        );
        assert_eq!(
            resumed_cursor.evidence_root,
            journal_only_cursor.evidence_root
        );
        assert!(coordinator.pending_observable_snapshot_on_resume());
        let debt_resume_cursor = coordinator.stream().cursor();
        drop(coordinator);

        // Simulate process loss after Resumed but before materialization. A
        // Recovery record must preserve observer debt rather than letting
        // semantic work outrun it.
        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("recover pending observer debt after an interrupted resume");
        assert!(coordinator.pending_observable_snapshot_on_resume());
        assert_eq!(
            coordinator.stream().cursor().sequence(),
            debt_resume_cursor
                .sequence()
                .checked_add(1)
                .expect("sequence")
        );

        let prepared_catch_up = coordinator
            .prepare_observable_snapshot_unavailable_for_test(
                "forced admitted-capacity outcome for lifecycle coverage",
            )
            .expect("prepare pending observer-unavailable receipt");
        let catch_up = publish_prepared_snapshot_and_pause_exact_stream_slice(
            &mut coordinator,
            prepared_catch_up,
            run_stream::PauseReason::Explicit,
            ExploreStreamSliceStop::SnapshotCatchUp,
            0,
            0,
        )
        .expect("materialize the pending observer view before further search");
        assert_eq!(catch_up.stop, ExploreStreamSliceStop::SnapshotCatchUp);
        match &catch_up.artifact {
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine {
                canonical_json_line,
                checkpoint_cursor,
                publication_cursor,
                detail,
                ..
            } => {
                let rendered = std::str::from_utf8(canonical_json_line)
                    .expect("snapshot-unavailable JSON is UTF-8");
                assert!(rendered.contains("\"status\":\"unavailable\""));
                assert!(rendered.contains("\"reason\":{\"kind\":\"capacity\"}"));
                assert!(!rendered.contains("\"configuration\""));
                assert!(!rendered.contains("\"answer\""));
                assert!(!rendered.contains("\"search_decision_dag\""));
                assert_eq!(
                    canonical_json_line
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count(),
                    1
                );
                assert!(
                    canonical_json_line.len()
                        <= stream_snapshot::EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1
                );
                assert_eq!(
                    checkpoint_cursor.evidence_root,
                    publication_cursor.evidence_root
                );
                assert_eq!(
                    publication_cursor.evidence_root,
                    catch_up.final_cursor.evidence_root
                );
                assert_eq!(
                    detail,
                    "forced admitted-capacity outcome for lifecycle coverage"
                );
            }
            _ => panic!("catch-up did not publish the bounded snapshot-unavailable receipt"),
        }
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        let catch_up_cursor = catch_up.final_cursor;
        drop(coordinator);

        let mut coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("resume after materializing the pending observer view");
        assert!(!coordinator.pending_observable_snapshot_on_resume());
        assert_eq!(
            coordinator.stream().cursor().sequence(),
            catch_up_cursor.sequence.checked_add(1).expect("sequence")
        );
        assert_eq!(
            coordinator
                .stream()
                .cursor()
                .evidence_root()
                .to_lowercase_hex(),
            catch_up_cursor.evidence_root
        );

        let case_cap = NonZeroU16::new(1).expect("one is nonzero");
        while let Some(rank) = coordinator.next_open_rank_hint() {
            match coordinator
                .advance_bounded_case_batch(case_cap)
                .expect("classify one-case durable frontier")
            {
                stream_coordinator::ExactStreamBatchAdvance::Committed { ranks, .. } => {
                    assert_eq!(ranks.as_ref(), &[rank]);
                }
                stream_coordinator::ExactStreamBatchAdvance::ClassificationClosedFinalizationPending => {
                    panic!("open-rank hint disagreed with the exact frontier")
                }
                stream_coordinator::ExactStreamBatchAdvance::CaseOpen { .. } => {
                    panic!("one-case fixture hit an evaluation limit")
                }
            }
        }
        assert_eq!(coordinator.closed_case_count(), 1);
        assert!(coordinator.exact_snapshot().result_group_scan_complete);
        let final_graph_publications = coordinator
            .prepare_graph_publications()
            .expect("prepare final one-case graph publications");
        let terminal_result_json = match attempt_atomic_exact_stream_finalization(
            &mut coordinator,
            &final_graph_publications,
        )
        .expect("finalize one-case durable stream")
        {
            ExactStreamFinalizationAttempt::Sealed(bytes) => bytes,
            ExactStreamFinalizationAttempt::WitnessOpen { .. } => {
                panic!("one-case finalization left a replay witness open")
            }
            ExactStreamFinalizationAttempt::LimitReached { .. } => {
                panic!("one-case finalization exceeded an atomic limit")
            }
        };
        let terminal_rendered =
            std::str::from_utf8(&terminal_result_json).expect("terminal JSON is UTF-8");
        assert!(terminal_rendered.contains("\"search_decision_dag\":\"full\""));
        assert!(terminal_rendered.contains("\"semantic_transition_graph\":\"full\""));
        assert!(terminal_rendered
            .contains("\"schema\":\"futuruna.explore.semantic-transition-graph.v1\""));
        assert!(terminal_rendered.contains(
            "\"distinct_declared_transitions\":{\"notation\":\"U_T\",\"lower_bound\":\"1\",\"exact\":\"1\",\"certainty\":\"exact\"}"
        ));
        assert!(terminal_rendered.contains(
            "\"admissible_match\":{\"case_count\":\"1\",\"rank_intervals\":[{\"start\":\"0\",\"end_exclusive\":\"1\"}]}"
        ));
        assert!(terminal_rendered.contains("\"status\":\"included\""));
        assert!(terminal_rendered.contains("\"classification\":\"admissible_match\""));
        assert!(terminal_rendered.contains("\"views\":\"closed\""));
        let sealed_cursor = public_exact_stream_cursor(coordinator.stream().cursor());
        assert_eq!(sealed_cursor.lifecycle, ExploreStreamLifecycle::Sealed);
        let terminal_blob_digest = coordinator
            .published_terminal_result()
            .expect("terminal publication receipt")
            .blob_digest()
            .to_lowercase_hex();
        drop(coordinator);

        let coordinator = stream_coordinator::ExactStreamCoordinator::open_or_create(
            &directory,
            run_store::RunStoreLimits::default(),
            &statements,
            None,
            &artifacts,
            selected,
            graph_request,
        )
        .expect("reopen sealed one-case durable stream");
        assert_eq!(
            public_exact_stream_cursor(coordinator.stream().cursor()),
            sealed_cursor
        );
        let already_sealed =
            render_already_sealed_exact_stream(&coordinator, coordinator.closed_case_count())
                .expect("render already-sealed one-case receipt");
        assert_eq!(
            already_sealed.stop,
            ExploreStreamSliceStop::AlreadySealed(ExploreStreamTerminalStatus::Completed)
        );
        assert_eq!(already_sealed.final_cursor, sealed_cursor);
        assert_eq!(already_sealed.singleton_cases_evaluated_this_slice, 0);
        assert_eq!(already_sealed.closed_cases_this_slice, 0);
        match already_sealed.artifact {
            ExploreStreamArtifact::TerminalResultJson {
                canonical_json,
                blob_digest,
            } => {
                assert_eq!(canonical_json, terminal_result_json);
                assert_eq!(blob_digest, terminal_blob_digest);
            }
            ExploreStreamArtifact::CheckpointSnapshotJsonLine { .. } => {
                panic!("already-sealed reopen returned a checkpoint artifact")
            }
            ExploreStreamArtifact::CheckpointSnapshotUnavailableJsonLine { .. } => {
                panic!("already-sealed reopen returned a snapshot-capacity artifact")
            }
            ExploreStreamArtifact::JournalOnlyCheckpoint { .. } => {
                panic!("already-sealed reopen returned a journal-only checkpoint")
            }
        }
        drop(coordinator);
        std::fs::remove_dir_all(&directory).expect("remove one-case durable-stream fixture");
    }

    }

    #[test]
    fn exact_range_cardinality_handles_full_i64_width() {
        assert_eq!(exact_range_cardinality(7, 7), Ok(0));
        assert!(exact_range_cardinality(8, 7).is_err());
        assert_eq!(exact_range_cardinality(i64::MIN, i64::MAX), Ok(u64::MAX));
    }

    #[test]
    fn canonical_finite_type_source_enumerates_payloads_in_declaration_order() {
        let source = r#"
# Bit = High | Low
# Flag = On | Off
# Payload = Empty | Full(bit: Bit, flag: Flag)

? explore payloads {
    from {
        before in values(Payload)
        context = ()
    }
    to after = before
    find all
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        assert_eq!(query.source.before_binding_index, 0);
        assert_eq!(query.source.context_binding_index, 1);
        let ExploreSourceBindingKindIr::Finite {
            domain: ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { plan, .. }),
        } = &query.source.bindings[0].kind
        else {
            panic!("before must retain the exact finite-type plan")
        };
        assert_eq!(plan.cardinality(), ExploreCardinality::Exact(5));
        let values = plan.enumerate(5).expect("materialize Payload");
        assert!(matches!(
            &values[0],
            ExploreValue::Constructor { variant, fields, .. }
                if variant == "Empty" && fields.is_empty()
        ));
        assert!(matches!(
            &values[1],
            ExploreValue::Constructor { variant, fields, .. }
                if variant == "Full"
                    && matches!(&fields[0].1, ExploreValue::Constructor { variant, .. }
                        if variant == "High")
                    && matches!(&fields[1].1, ExploreValue::Constructor { variant, .. }
                        if variant == "On")
        ));
    }

    #[test]
    fn finite_plan_has_a_total_node_budget() {
        let source = r#"
# Leaf = A | B
# P0 = Node(left: Leaf, right: Leaf)
# P1 = Node(left: P0, right: P0)
# P2 = Node(left: P1, right: P1)
# P3 = Node(left: P2, right: P2)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse repeated-product type fixture");
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect repeated-product types");
        let mut budget = 10;
        let error = finite_type_plan_with_budget(
            &Ty::Name("P3".to_string()),
            &catalog,
            "P3",
            &mut BTreeSet::new(),
            &mut budget,
            0,
        )
        .expect_err("repeated products must exhaust the test plan budget");
        assert!(error.contains("finite-type plan work limit"), "{error}");

        let variant_source = "# Many = A | B | C\n";
        let mut lexer = Lexer::new(variant_source);
        let tokens = lexer.tokenize();
        let variants = Parser::new(tokens, variant_source)
            .parse_program()
            .expect("parse many-variant type");
        let catalog = calculate::TypeCatalog::collect_checked(&variants, None)
            .expect("collect many-variant type");
        let mut budget = 3;
        let error = finite_type_plan_with_budget(
            &Ty::Name("Many".to_string()),
            &catalog,
            "Many",
            &mut BTreeSet::new(),
            &mut budget,
            0,
        )
        .expect_err("variant plan nodes must consume the total plan budget");
        assert!(error.contains("finite-type plan work limit"), "{error}");
    }

    #[test]
    fn canonical_source_ir_retains_ordered_dependent_collection_and_range_fibers() {
        let source = r#"
= choices: List(Int) = [2, 1, 2]
> around(seed: Int) -> List(Int) { [seed, seed + 1] }

? explore dependent_domains {
    from {
        seed in choices
        candidate in around(seed)
        before in range(candidate, candidate + 2)
        context = ()
    }
    to after = before + 1
    find all
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        assert_eq!(
            query
                .source
                .bindings
                .iter()
                .map(|binding| binding.name.as_str())
                .collect::<Vec<_>>(),
            ["seed", "candidate", "before", "context"]
        );
        assert!(matches!(
            &query.source.bindings[0].kind,
            ExploreSourceBindingKindIr::Finite {
                domain: ExploreFiniteDomainIr::Collection { .. }
            }
        ));
        assert!(matches!(
            &query.source.bindings[1].kind,
            ExploreSourceBindingKindIr::Finite {
                domain: ExploreFiniteDomainIr::Collection { .. }
            }
        ));
        assert!(matches!(
            &query.source.bindings[2].kind,
            ExploreSourceBindingKindIr::Finite {
                domain: ExploreFiniteDomainIr::IntRange { .. }
            }
        ));
        assert_eq!(
            query.source.bindings[1]
                .dependencies
                .iter()
                .map(|dependency| dependency.binding_name.as_str())
                .collect::<Vec<_>>(),
            ["seed"]
        );
        assert_eq!(
            query.source.bindings[2]
                .dependencies
                .iter()
                .map(|dependency| dependency.binding_name.as_str())
                .collect::<Vec<_>>(),
            ["candidate"]
        );
        assert!(matches!(
            &query.successor.kind,
            ExploreSuccessorKindIr::Singleton { .. }
        ));
    }

    #[test]
    fn canonical_source_supports_generic_finite_types_and_typed_composites() {
        let source = r#"
# Bit = High | Low
# Flag = On | Off
# Profile(
    option: Option(Bit),
    result: Result(Bit, Flag),
    pair: Pair(Bit, Flag),
    boolean: Bool
)

? explore generic_values {
    from {
        option in values(Option(Bit))
        result in values(Result(Bit, Flag))
        pair in values(Pair(Bit, Flag))
        boolean in values(Bool)
        before = Profile(
            option = option,
            result = result,
            pair = pair,
            boolean = boolean
        )
        context = ()
    }
    to after = before
    find all
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        let cardinalities = query.source.bindings[..4]
            .iter()
            .map(|binding| {
                let ExploreSourceBindingKindIr::Finite {
                    domain:
                        ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { plan, .. }),
                } = &binding.kind
                else {
                    panic!("values(T) must lower to an exact finite-type plan")
                };
                plan.cardinality()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            cardinalities,
            [
                ExploreCardinality::Exact(3),
                ExploreCardinality::Exact(4),
                ExploreCardinality::Exact(4),
                ExploreCardinality::Exact(2),
            ]
        );
        assert_eq!(
            query.source.bindings[4]
                .dependencies
                .iter()
                .map(|dependency| dependency.binding_name.as_str())
                .collect::<Vec<_>>(),
            ["option", "result", "pair", "boolean"]
        );
        assert_eq!(
            query.source.bindings[4].role,
            ExploreSourceBindingRoleIr::Before
        );
    }

    #[test]
    fn canonical_values_domains_fail_closed_for_unbounded_ambiguous_and_open_types() {
        let fixtures = [
            (
                r#"
# FilingStatus = Online | Paper(copies: Int)
? explore invalid {
    from {
        before in values(FilingStatus)
        context = ()
    }
    to after = before
    find all
}
"#,
                "FilingStatus.Paper.copies",
            ),
            (
                r#"
# Status = Alpha
# Status = Beta
? explore invalid {
    from {
        before in values(Status)
        context = ()
    }
    to after = before
    find all
}
"#,
                "multiple declarations",
            ),
            (
                r#"
# Profile(x: Int) {
    | amount() -> x
}
= profiles: List(Profile) = [Profile(1)]
? explore invalid {
    from {
        before in profiles
        context = ()
    }
    to after = before
    find all
}
"#,
                "rule scope",
            ),
            (
                r#"
# Option(a) = Absent | Present(a)
# Status = Active | Inactive
? explore invalid {
    from {
        before in values(Option(Status))
        context = ()
    }
    to after = before
    find all
}
"#,
                "declared type",
            ),
            (
                r#"
# Combined = Base | Third
# Base = First | Second
? explore invalid {
    from {
        before in values(Combined)
        context = ()
    }
    to after = before
    find all
}
"#,
                "already initialized declaration prefix",
            ),
        ];

        for (source, expected) in fixtures {
            let artifacts = artifacts(source);
            assert!(artifacts.exploration_universes.is_empty());
            assert!(
                artifacts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected:?}: {:?}",
                artifacts.diagnostics
            );
        }
    }

    #[test]
    fn canonical_replay_identity_is_checked_across_source_successor_and_find() {
        let ambiguous = r#"
> helper(value: Int) -> Int { value + 1 }
> helper() -> Int { 99 }
| eligible(value: Int) -> True under value > 0

? explore ambiguous_helper {
    from {
        seed in [1, 2]
        before = helper(seed)
        context = ()
    }
    to after = helper(before)
    find matches of eligible(after)
}
"#;
        let ambiguous_artifacts = artifacts(ambiguous);
        assert!(ambiguous_artifacts.exploration_universes.is_empty());
        assert!(
            ambiguous_artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("ordinary runtime functions resolve by bare name")),
            "{:?}",
            ambiguous_artifacts.diagnostics
        );

        let unique = ambiguous.replace("> helper() -> Int { 99 }\n", "");
        let artifacts = artifacts(&unique);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        assert_eq!(
            query.source.bindings[1]
                .dependencies
                .iter()
                .map(|dependency| dependency.binding_name.as_str())
                .collect::<Vec<_>>(),
            ["seed"]
        );
        assert!(matches!(&query.find, ExploreFindIr::Matches { .. }));
    }

    fn ground_binding_value(source: &str, binding: &str) -> Result<ExploreValue, String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse ground-value fixture");
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect ground-value fixture types");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect ground-value declarations");
        ExploreGroundEvaluator::new(&catalog, definitions).eval_binding(binding, None)
    }

    #[test]
    fn ground_set_and_distinct_keep_stable_runtime_display_identity() {
        let set = ground_binding_value(
            r#"
= pairs: Set(Tuple(String, String)) = set_from_list([
    ("a, b", "c"),
    ("a", "b, c")
])
"#,
            "pairs",
        )
        .expect("evaluate runtime Set identity");
        assert!(matches!(set, ExploreValue::Set(values) if values.len() == 1));

        let distinct = ground_binding_value(
            r#"
= pairs: List(Tuple(String, String)) = distinct([
    ("a, b", "c"),
    ("a", "b, c")
])
"#,
            "pairs",
        )
        .expect("evaluate runtime distinct identity");
        assert!(matches!(distinct, ExploreValue::List(values) if values.len() == 1));
    }

    #[test]
    fn canonical_imports_keep_finite_types_collections_and_prefix_scope() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_relational_explore_import_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create Explore import directory");
        std::fs::write(
            directory.join("domain.runa"),
            r#"
# ImportedStatus = Beta | Alpha
# ImportedProfile(status: ImportedStatus, score: Int)
= imported_statuses: List(ImportedStatus) = [Beta, Alpha]
> imported_scores() -> List(Int) { [1, 2, 2] }
"#,
        )
        .expect("write Explore import fixture");

        let source = r#"
@ import ./domain
? explore imported {
    from {
        status in values(ImportedStatus)
        declared_status in imported_statuses
        score in imported_scores()
        before = ImportedProfile(status = status, score = score)
        context = ()
    }
    to after = before
    find all
}
"#;
        let artifacts = artifacts_with_dir(source, &directory);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        let ExploreSourceBindingKindIr::Finite {
            domain: ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { plan, .. }),
        } = &query.source.bindings[0].kind
        else {
            panic!("imported values(T) must retain an exact finite-type plan")
        };
        assert_eq!(plan.cardinality(), ExploreCardinality::Exact(2));
        assert!(matches!(
            &query.source.bindings[1].kind,
            ExploreSourceBindingKindIr::Finite {
                domain: ExploreFiniteDomainIr::Collection { .. }
            }
        ));
        assert!(matches!(
            &query.source.bindings[2].kind,
            ExploreSourceBindingKindIr::Finite {
                domain: ExploreFiniteDomainIr::Collection { .. }
            }
        ));

        std::fs::write(
            directory.join("capturing.runa"),
            "= captured: List(Int) = root_values\n",
        )
        .expect("write prefix-capture fixture");
        let capture = r#"
@ import ./capturing
= root_values: List(Int) = [1, 2]
? explore invalid_capture {
    from {
        before in captured
        context = ()
    }
    to after = before
    find all
}
"#;
        let artifacts = artifacts_with_dir(capture, &directory);
        std::fs::remove_dir_all(&directory).ok();
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("depends on later declaration")),
            "{:?}",
            artifacts.diagnostics
        );
    }

    #[test]
    fn canonical_collection_domains_preserve_checked_range_and_member_failures() {
        let invalid_members = r#"
> choices() -> List(Int) { [True] }
= declared_choices: List(Int) = choices()
? explore invalid_members {
    from {
        before in declared_choices
        context = ()
    }
    to after = before
    find all
}
"#;
        let artifacts = artifacts(invalid_members);
        assert!(artifacts.exploration_universes.is_empty());
        assert!(
            artifacts.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("member 1 does not have declared type")),
            "{:?}",
            artifacts.diagnostics
        );

        let reversed = ground_binding_value("= choices: List(Int) = range(3, 1)\n", "choices")
            .expect_err("reversed range must fail closed");
        assert!(reversed.contains("greater than end"), "{reversed}");

        let oversized =
            ground_binding_value("= choices: List(Int) = range(0, 1000001)\n", "choices")
                .expect_err("oversized named range must fail closed");
        assert!(
            oversized.contains("exceeding materialization limit 1000000"),
            "{oversized}"
        );
    }

    #[test]
    fn preflight_has_a_total_work_budget() {
        let source = r#"
> f0() -> Int { 1 }
> f1() -> Int { f0() + f0() }
> f2() -> Int { f1() + f1() }
> f3() -> Int { f2() + f2() }
> f4() -> Int { f3() + f3() }
= choice: Int = f4()
"#;
        let statements = {
            let mut lexer = Lexer::new(source);
            let tokens = lexer.tokenize();
            Parser::new(tokens, source)
                .parse_program()
                .expect("parse work-budget fixture")
        };
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect work-budget types");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect work-budget declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        evaluator.work_remaining = 20;
        let error = evaluator
            .eval_binding("choice", Some(&Ty::Name("Int".to_string())))
            .expect_err("fan-out must exhaust the preflight budget");
        assert!(error.contains("checked work limit"), "{error}");
    }

    #[test]
    fn preflight_collection_transforms_consume_the_total_work_budget() {
        let source = "= choices: List(Int) = distinct(distinct([1, 2, 3]))\n";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse collection-work fixture");
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect collection-work types");
        let definitions = collect_ground_bindings(&statements, None)
            .expect("collect collection-work declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        evaluator.work_remaining = 10;
        let error = evaluator
            .eval_binding(
                "choices",
                Some(&Ty::App(
                    Box::new(Ty::Name("List".to_string())),
                    vec![Ty::Name("Int".to_string())],
                )),
            )
            .expect_err("nested linear transforms must exhaust the preflight budget");
        assert!(error.contains("checked work limit"), "{error}");
    }

    #[test]
    fn preflight_rejects_deep_acyclic_helper_chains() {
        let mut source = "> f0() -> Int { 1 }\n".to_string();
        for index in 1..=260 {
            source.push_str(&format!("> f{}() -> Int {{ f{}() }}\n", index, index - 1));
        }
        source.push_str("= choice: Int = f260()\n");
        let statements = {
            let mut lexer = Lexer::new(&source);
            let tokens = lexer.tokenize();
            Parser::new(tokens, &source)
                .parse_program()
                .expect("parse helper-depth fixture")
        };
        let catalog = calculate::TypeCatalog::collect_checked(&statements, None)
            .expect("collect helper-depth types");
        let definitions =
            collect_ground_bindings(&statements, None).expect("collect helper-depth declarations");
        let mut evaluator = ExploreGroundEvaluator::new(&catalog, definitions);
        let error = evaluator
            .eval_binding("choice", Some(&Ty::Name("Int".to_string())))
            .expect_err("deep helper chain must fail before stack recursion");
        assert!(error.contains("safe depth limit"), "{error}");
    }

    #[test]
    fn dependency_analysis_is_bounded_for_deep_helper_chains() {
        let mut source = "= later: Int = 1\n> f0() -> Bool { later > 0 }\n".to_string();
        for index in 1..=260 {
            source.push_str(&format!("> f{}() -> Bool {{ f{}() }}\n", index, index - 1));
        }
        source.push_str("= probe: Bool = f260()\n");
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, &source)
            .parse_program()
            .expect("parse dependency-depth fixture");
        let definitions = collect_ground_bindings(&statements, None)
            .expect("collect dependency-depth declarations");
        let probe = statements
            .iter()
            .find_map(|statement| match statement {
                Stmt::Bind(Pat::Var(name), _, expression) if name == "probe" => Some(expression),
                _ => None,
            })
            .expect("probe expression");
        let dependencies = expression_query_dependencies(
            probe,
            &BTreeSet::from(["later".to_string()]),
            &definitions,
        );
        assert_eq!(dependencies, BTreeSet::from(["later".to_string()]));
    }
}
