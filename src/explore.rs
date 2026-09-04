//! Closed relational elaboration for bounded `? explore` declarations.
//!
//! The parser and type checker deliberately retain source expressions.  This
//! pass is the trust boundary that proves each dependent source and successor
//! domain finite, deterministic, and exact before an executor may see it.

use super::*;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

mod authenticated_treap;
mod case_graph;
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
    MechanismCorrelatedSupportStatus, MechanismStarterProjectionExprRoot,
    MechanismStarterProjectionPlanId, MechanismStarterSetStatus, MechanismStarterUpperProvenance,
    MechanismSupportCatalogBuilder, MechanismSupportClosureReceipt, MechanismSupportClosureRoot,
    MechanismSupportCount, MechanismSupportError, MechanismSupportExpressionBounds,
    MechanismSupportFacet, MechanismSupportFiberExprRoot, MechanismSupportFrontierRoot,
    MechanismSupportKey, MechanismSupportResidualRoot, MechanismSupportStarterCursor,
    MechanismSupportStarterMember, MechanismSupportStarterPage, MechanismSupportSubject,
    MechanismSupportSubjectStarterPage, MechanismSupportView, MechanismSupportViewRoot,
    MECHANISM_STARTER_PROJECTION_EXPR_VERSION, MECHANISM_SUPPORT_VERSION,
    MECHANISM_SUPPORT_VIEW_VERSION,
};
mod choice_relation;
pub(crate) use choice_relation::{
    ChoiceCandidate, ChoiceContentRoot, ChoiceCount, ChoiceFrontierRoot, ChoiceInputSeal,
    ChoiceMember, ChoiceRelationBuilder, ChoiceRelationCounts, ChoiceRelationError,
    ChoiceRelationSnapshot, ChoiceRelationSpec, ChoiceRelationStatus, CHOICE_RELATION_VERSION,
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
mod relational_mechanism_starter_regions;
pub(crate) use relational_mechanism_starter_regions::{
    RelationalMechanismStarterFiberId, RelationalMechanismStarterRegion,
    RelationalMechanismStarterRegionAccept, RelationalMechanismStarterRegionAccumulator,
    RelationalMechanismStarterRegionCompletion, RelationalMechanismStarterRegionContentRoot,
    RelationalMechanismStarterRegionCursor, RelationalMechanismStarterRegionError,
    RelationalMechanismStarterRegionFallback, RelationalMechanismStarterRegionFallbackReason,
    RelationalMechanismStarterRegionId, RelationalMechanismStarterRegionLimits,
    RelationalMechanismStarterRegionMemberRef, RelationalMechanismStarterRegionSuccessor,
    RelationalMechanismStarterRegionSummary, RelationalMechanismStarterRegionSummaryRoot,
    RELATIONAL_MECHANISM_STARTER_REGION_VERSION,
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
mod relation;
mod relational_mechanism_step_driver;
pub(crate) use relation::{
    AdmissionCatalog, AdmissionCatalogBuilder, AdmissionContentRoot, AdmissionCounts,
    AdmissionDecision, AdmissionFrontierRoot, AdmissionId, CatalogSource, CatalogSuccessor,
    ChoiceId, FindPolarity, MechanismRequestId, MechanismTargetId, QuestionCatalog,
    QuestionCatalogBuilder, QuestionContentRoot, QuestionFrontierRoot, QuestionId, RelationCatalog,
    RelationCatalogBuilder, RelationCatalogError, RelationCatalogSnapshot,
    RelationClassificationError, RelationContentRoot, RelationCountEvidence,
    RelationEnumerationCounts, RelationFrontierRoot, RelationId, RelationLineageId,
    RelationProvenance, RelationSupportId, RelationalCaseId, RelationalCaseRef, SelectionCounts,
    SelectionDecision, SourceKey, SourceRow, SuccessorKey, SuccessorRow, ViewId, ViewInputId,
};
mod relational_endpoint_totality;
pub(crate) use relational_endpoint_totality::{
    RelationalEndpointAbstractProofRoot, RelationalEndpointProofDomainRoot, RelationalEndpointRole,
    RelationalEndpointTotalityCertificate, RelationalEndpointTotalityCertificateError,
    RelationalEndpointTotalityCertificateId, RelationalEndpointTotalityIssue,
    RelationalEndpointTotalityIssueReason, RelationalEndpointTotalityObligationCount,
    RELATIONAL_ENDPOINT_TOTALITY_CERTIFICATE_VERSION,
};
mod relational_endpoint_totality_proof;
pub(crate) use relational_endpoint_totality_proof::prove_relational_endpoint_totality;
#[cfg(test)]
mod relational_endpoint_totality_tests;
mod relational_ir;
pub(crate) use relational_ir::relational_tys_equivalent;
pub use relational_ir::{
    ExploreAdmissionIr, ExploreAggregateFieldIr, ExploreAggregateReducerIr, ExploreAnalysisNodeIr,
    ExploreChoicePartitionIr, ExploreChoiceRelationIr, ExploreFindIr, ExploreFiniteDomainIr,
    ExploreMechanismRequestIr, ExploreMechanismTargetIr, ExploreNamedFindIr,
    ExploreParetoObjectiveIr, ExploreQueryIr, ExploreResultChoiceIr, ExploreResultFieldIr,
    ExploreResultGrainIr, ExploreResultHavingIr, ExploreResultInputIr, ExploreResultViewIr,
    ExploreSourceBindingIr, ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr,
    ExploreSourceDependencyIr, ExploreSourceProducerRoleIr, ExploreSourceRelationIr,
    ExploreSuccessorKindIr, ExploreSuccessorRelationIr, EXPLORE_RELATIONAL_IR_VERSION,
};
pub(crate) use relational_ir::{
    ExploreMechanismSupportFacetIr, ExploreMechanismSupportSubjectIr, ExploreStarterProjectionIr,
    ExploreSupportObservationDemandIr, ExploreTransitionGraphIr,
};
mod relational_analysis_plan;
pub(crate) use relational_analysis_plan::{
    RelationalAnalysisDependencyId, RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration,
    RelationalAnalysisPlan, RelationalAnalysisPlanError, RelationalAnalysisPlanRoot,
    RelationalCheckedAnalysisGraphDigest, RelationalChoiceRegistration, RelationalChoiceSpecDigest,
    RelationalMechanismLayerRegistration, RelationalMechanismObservationDigest,
    RelationalMechanismObservationId, RelationalResolvedMechanismTarget,
    RelationalResolvedResultInput, RelationalResultLayerRegistration, RelationalResultSpecDigest,
    RELATIONAL_ANALYSIS_PLAN_VERSION,
};
mod relational_analysis_catalog;
pub(crate) use relational_analysis_catalog::{
    ChoiceTargetCase, ClosedRelationalAnalysisCatalog, RelationalAnalysisCatalogBuilder,
    RelationalAnalysisCatalogError, RelationalAnalysisCatalogRoot,
    RelationalAnalysisCatalogSnapshot, RelationalAnalysisLayerSnapshot,
    RelationalAnalysisLayerStatus, RelationalChoiceLayerSnapshot, RelationalMechanismLayerSnapshot,
    RelationalResultLayerSnapshot, RelationalResultLayerSnapshotState, RelationalResultPublication,
    RelationalResultPublicationId, RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION,
    RELATIONAL_RESULT_PUBLICATION_VERSION,
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
mod relational_semantic_transition_graph_projection;
pub(crate) use relational_semantic_transition_graph_projection::{
    RelationalSemanticTransitionGraphCapacity, RelationalSemanticTransitionGraphClosure,
    RelationalSemanticTransitionGraphProjection, RelationalSemanticTransitionGraphProjectionError,
    RelationalSemanticTransitionGraphProjectionId, RelationalSemanticTransitionGraphRecord,
    RelationalSemanticTransitionGraphUnmaterialized,
    RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1,
    RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_SCHEMA,
    RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_VERSION,
};
mod relational_transition_support;
pub(crate) use relational_transition_support::{
    PreparedTransitionClassification, PreparedUniverseTransition, RelationalSemanticTransition,
    RelationalTransitionCaseSupport, RelationalTransitionLayer, RelationalTransitionSupportCounts,
    RelationalTransitionSupportError, RelationalTransitionSupportIndex,
    RelationalTransitionSupportRoot, RELATIONAL_TRANSITION_SUPPORT_VERSION,
};
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
    ExploreNativeClassifierRuleMetadataV2, ExploreNativeClassifierSourceBindingKindV2,
    ExploreNativeClassifierSourceBindingV2, ExploreStreamCheckpoint, ExploreStreamCount,
    ExploreStreamCoverageBindingRole, ExploreStreamCoverageClassification,
    ExploreStreamCoverageConstructorLayout, ExploreStreamCoverageEntry,
    ExploreStreamCoverageFieldPathSegment, ExploreStreamCoverageGapReason,
    ExploreStreamCoverageLiteralKind, ExploreStreamCoverageRootRole, ExploreStreamCoverageSubject,
    ExploreStreamEpochOptions, ExploreStreamFind, ExploreStreamGroupedResultPreview,
    ExploreStreamIdentity, ExploreStreamLayer, ExploreStreamLayerStatus, ExploreStreamLifecycle,
    ExploreStreamMechanismLayer, ExploreStreamMechanismSupportTotals, ExploreStreamMechanismTarget,
    ExploreStreamObserverMemoStats, ExploreStreamOuterContainment, ExploreStreamPauseReason,
    ExploreStreamPopulationCounts, ExploreStreamPreparationError, ExploreStreamPreviewLimit,
    ExploreStreamPreviewStatus, ExploreStreamProjectedValue, ExploreStreamPublication,
    ExploreStreamPublicationArtifact, ExploreStreamResultColumn, ExploreStreamResultEvidence,
    ExploreStreamResultField, ExploreStreamResultGrain, ExploreStreamResultGroupRow,
    ExploreStreamResultInput, ExploreStreamResultLayer, ExploreStreamSliceOptions,
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
mod run_store;
mod stream_resource;
mod transition;

pub(crate) use transition::{
    ContextSchemaId, StateId, StateSchemaId, TransitionId, TransitionSchemaIdentities,
    TransitionTypeId,
};

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
        ExploreValue::Set(values) => {
            let mut entries = BTreeMap::new();
            for value in values {
                runtime_set_insert_value(&mut entries, runtime_value_from_explore_value(value));
            }
            Value::Set(entries)
        }
        ExploreValue::Tuple(values) => Value::Tuple(
            values
                .iter()
                .map(runtime_value_from_explore_value)
                .collect(),
        ),
        ExploreValue::Constructor {
            type_name,
            variant,
            positional: true,
            fields,
        } => {
            let arguments = fields
                .iter()
                .map(|(_, value)| runtime_value_from_explore_value(value))
                .collect::<Vec<_>>()
                .into();
            if let Some((owner, _)) = runtime_nominal_type_parts(type_name) {
                Value::NamespacedConstructor {
                    namespace: RuntimeNamespace::detached(Rc::<str>::from(owner)),
                    name: variant.clone(),
                    arguments,
                    declaration_env: None,
                }
            } else {
                Value::Constructor(variant.clone(), arguments)
            }
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional: false,
            fields,
        } => {
            let fields = fields
                .iter()
                .map(|(name, value)| (name.clone(), runtime_value_from_explore_value(value)))
                .collect::<Vec<_>>()
                .into();
            if let Some((owner, _)) = runtime_nominal_type_parts(type_name) {
                Value::NamespacedNamedConstructor {
                    namespace: RuntimeNamespace::detached(Rc::<str>::from(owner)),
                    name: variant.clone(),
                    fields,
                    declaration_env: None,
                }
            } else {
                Value::NamedConstructor(variant.clone(), fields)
            }
        }
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
                    values.entry(inserted.clone()).or_insert(inserted);
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
                    values.remove(&removed);
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
            // Publish only globals completed before this RHS. Case-local
            // values live in a separate child Env and never hydrate this slot.
            self.interpreter
                .refresh_declaration_environment(&mut self.base_env);
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
            self.interpreter
                .refresh_declaration_environment(&mut self.base_env);
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
    for named_find in query.finds.iter() {
        if let Some(predicate) = named_find.find.predicate() {
            expressions.push(predicate);
        }
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
                type_name: left_type,
                variant: left_variant,
                positional: true,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                type_name: right_type,
                variant: right_variant,
                positional: true,
                fields: right_fields,
                ..
            },
        ) => Some(
            left_type == right_type
                && left_variant == right_variant
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
                type_name: left_type,
                variant: left_variant,
                positional: left_positional,
                fields: left_fields,
                ..
            },
            ExploreValue::Constructor {
                type_name: right_type,
                variant: right_variant,
                positional: right_positional,
                fields: right_fields,
                ..
            },
        ) => {
            left_type == right_type
                && left_positional == right_positional
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
    let (variant_name, positional, runtime_fields, nominal_owner): (
        &str,
        bool,
        Vec<(&str, &Value)>,
        Option<&RuntimeNamespace>,
    ) = match value {
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
            None,
        ),
        Value::NamedConstructor(name, fields) => (
            name,
            false,
            fields
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
            None,
        ),
        Value::NamespacedConstructor {
            namespace,
            name,
            arguments,
            ..
        } => (
            name,
            true,
            arguments.iter().map(|value| ("", value)).collect(),
            Some(namespace),
        ),
        Value::NamespacedNamedConstructor {
            namespace,
            name,
            fields,
            ..
        } => (
            name,
            false,
            fields
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
            Some(namespace),
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
        type_name: nominal_owner
            .map(|owner| runtime_nominal_type_name(owner, &type_name))
            .unwrap_or(type_name),
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
    require_legacy_rule_type: bool,
) -> Option<String> {
    let key = (name.to_string(), arity);
    if validated.contains(&key) || !visiting.insert(key.clone()) {
        return None;
    }

    let exact_rule = definitions.rule_definitions.contains_key(&key);
    if exact_rule && require_legacy_rule_type {
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
            if !require_legacy_rule_type && !definition.effects.is_empty() {
                // Mechanism endpoints use the query-scoped totality proof as
                // their definedness authority. A checked effect row is a
                // decisive leaf obligation, so preserve this callable for the
                // prover instead of demanding replay identities inside a body
                // which cannot be evaluated under a valid certificate.
                None
            } else {
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
                    require_legacy_rule_type,
                )
            }
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
                            require_legacy_rule_type,
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
    require_legacy_rule_type: bool,
) -> Option<String> {
    collect_scoped_runtime_calls(expression, bound)
        .into_iter()
        .find_map(|call| {
            if call.lexically_bound {
                return require_legacy_rule_type.then(|| {
                    format!(
                        "exploration replay call `{}` resolves through a lexical value instead of one exact top-level callable",
                        call.name
                    )
                });
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
                require_legacy_rule_type,
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
    // A mechanism-endpoint validation omits the legacy rule-type premise, so
    // its success cannot satisfy a later, stricter ordinary-expression check.
    let mut endpoint_validated = BTreeSet::new();
    let mut ordinary_validated = BTreeSet::new();

    let mut check_expression =
        |expression: &Expr, bound: &BTreeSet<String>, require_legacy_rule_type: bool| {
            let validated = if require_legacy_rule_type {
                &mut ordinary_validated
            } else {
                &mut endpoint_validated
            };
            if let Some(message) = expression_replay_callable_identity_issue(
                expression,
                bound,
                definitions,
                &mut BTreeSet::new(),
                validated,
                require_legacy_rule_type,
            ) {
                diagnostics.push(Diagnostic::error_at(expression.span, message));
            }
        };

    let mut available_source_names = BTreeSet::new();
    for binding in &query.source.bindings {
        match &binding.kind {
            TypedExploreSourceBindingKind::Singleton { value } => {
                check_expression(value, &available_source_names, true);
            }
            TypedExploreSourceBindingKind::Finite { domain } => match domain {
                TypedExploreDomain::FiniteExpr { expression, .. } => {
                    check_expression(expression, &available_source_names, true);
                }
                TypedExploreDomain::Range {
                    start,
                    end_exclusive,
                } => {
                    check_expression(start, &available_source_names, true);
                    check_expression(end_exclusive, &available_source_names, true);
                }
                TypedExploreDomain::Values { .. } => {}
            },
        }
        available_source_names.insert(binding.name.clone());
    }

    match &query.successor.kind {
        TypedExploreSuccessorKind::Singleton { value } => {
            check_expression(value, &semantic_case_names, true);
        }
        TypedExploreSuccessorKind::Finite { domain } => match domain {
            TypedExploreDomain::FiniteExpr { expression, .. } => {
                check_expression(expression, &semantic_case_names, true);
            }
            TypedExploreDomain::Range {
                start,
                end_exclusive,
            } => {
                check_expression(start, &semantic_case_names, true);
                check_expression(end_exclusive, &semantic_case_names, true);
            }
            TypedExploreDomain::Values { .. } => {}
        },
    }

    for admission in &query.admissions {
        check_expression(&admission.predicate, &semantic_case_names, true);
    }
    for find in &query.finds {
        match &find.selection {
            TypedExploreSelection::All { .. } => {}
            TypedExploreSelection::Matches { predicate, .. }
            | TypedExploreSelection::Violations { predicate, .. } => {
                check_expression(predicate, &semantic_case_names, true);
            }
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
                            check_expression(&field.value, &view_names, true);
                            view_names.insert(field.name.clone());
                        }
                    }
                }
                for field in &view.measures {
                    check_expression(&field.value, &view_names, true);
                    view_names.insert(field.name.clone());
                }
                for field in &view.aggregates {
                    match &field.reducer {
                        TypedExploreAggregateReducer::CountDistinct { value, .. } => {
                            check_expression(value, &view_names, true);
                        }
                    }
                    view_names.insert(field.name.clone());
                }
                for field in &view.select {
                    check_expression(&field.value, &view_names, true);
                    view_names.insert(field.name.clone());
                }
                match &view.choose {
                    None => {}
                    Some(TypedExploreResultChoice::Optimize { objective, .. }) => {
                        check_expression(objective, &view_names, true);
                    }
                    Some(TypedExploreResultChoice::Pareto { objectives, .. }) => {
                        for objective in objectives {
                            check_expression(&objective.value, &view_names, true);
                        }
                    }
                }
            }
            TypedExploreAnalysisNode::Mechanisms(request) => {
                // Endpoint type and definedness authority comes from the
                // exact checked-resolution contract and its request-bounded
                // certificate. This pass still rejects replay collisions and
                // shadowing, but must not reimpose the root-only legacy type
                // catalog on mechanism observers.
                check_expression(&request.endpoint_template, &mechanism_names, false);
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
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn runtime_set_map(values: Vec<ExploreValue>) -> BTreeMap<ExploreValue, ExploreValue> {
    let mut set = BTreeMap::new();
    for value in values {
        set.entry(value.clone()).or_insert(value);
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
            TypedExploreResultInput::Find {
                find_name,
                find_index,
            } => ExploreResultInputIr::Find {
                find_name: find_name.clone(),
                find_index: *find_index,
            },
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
        let producer_role = match binding.producer_role {
            TypedExploreSourceProducerRole::Given => ExploreSourceProducerRoleIr::Given,
            TypedExploreSourceProducerRole::Vary => ExploreSourceProducerRoleIr::Vary,
            TypedExploreSourceProducerRole::Let => ExploreSourceProducerRoleIr::Let,
        };
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
            producer_role,
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

    let finds = query
        .finds
        .iter()
        .map(|named_find| ExploreNamedFindIr {
            name: named_find.name.clone(),
            find: match &named_find.selection {
                TypedExploreSelection::All { span } => ExploreFindIr::All { span: *span },
                TypedExploreSelection::Matches { predicate, span } => ExploreFindIr::Matches {
                    predicate: predicate.clone(),
                    span: *span,
                },
                TypedExploreSelection::Violations { predicate, span } => {
                    ExploreFindIr::Violations {
                        predicate: predicate.clone(),
                        span: *span,
                    }
                }
            },
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let mut analysis = Vec::with_capacity(query.analysis.len());
    for (node_index, node) in query.analysis.iter().enumerate() {
        analysis.push(match node {
            TypedExploreAnalysisNode::Result(view) => {
                ExploreAnalysisNodeIr::Result(lower_result_view(view, node_index))
            }
            TypedExploreAnalysisNode::Mechanisms(request) => {
                let target = match &request.target {
                    TypedExploreMechanismTarget::FindCases { find_index, .. } => {
                        ExploreMechanismTargetIr::Find {
                            find_index: *find_index,
                        }
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
        finds,
        analysis: analysis.into_boxed_slice(),
        observation_demands: query
            .observation_demands
            .iter()
            .map(ExploreSupportObservationDemandIr::lower)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        starter_projections: query
            .starter_projections
            .iter()
            .map(ExploreStarterProjectionIr::lower)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        transition_graphs: query
            .transition_graphs
            .iter()
            .map(ExploreTransitionGraphIr::lower)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        span: query.span,
    };
    closed
        .validate()
        .map_err(|message| vec![Diagnostic::error_at(query.span, message)])?;
    Ok(closed)
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
        vary before in values(Payload)
        given context = ()
    }
    transition after = before
    find payload_cases = all
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
        vary seed in choices
        vary candidate in around(seed)
        vary before in range(candidate, candidate + 2)
        given context = ()
    }
    transition after = before + 1
    find dependent_cases = all
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
        vary option in values(Option(Bit))
        vary result in values(Result(Bit, Flag))
        vary pair in values(Pair(Bit, Flag))
        vary boolean in values(Bool)
        let before = Profile(
            option = option,
            result = result,
            pair = pair,
            boolean = boolean
        )
        given context = ()
    }
    transition after = before
    find generic_cases = all
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
        vary before in values(FilingStatus)
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary before in values(Status)
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary before in profiles
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary before in values(Option(Status))
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary before in values(Combined)
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary seed in [1, 2]
        let before = helper(seed)
        given context = ()
    }
    transition after = helper(before)
    find eligible_cases = matches of eligible(after)
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
        assert!(matches!(
            &query.finds[0].find,
            ExploreFindIr::Matches { .. }
        ));
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
    fn ground_set_and_distinct_use_structural_identity_not_display() {
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
        assert!(matches!(set, ExploreValue::Set(values) if values.len() == 2));

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
        assert!(matches!(distinct, ExploreValue::List(values) if values.len() == 2));
    }

    #[test]
    fn demanded_qualified_constructors_retain_nominal_owner_across_exact_conversion() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_explore_qualified_nominal_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create qualified nominal fixture");
        let dependency = r#"
@ export
# Positional = PWrapped(Int)
@ export
# Named = NWrapped(value: Int)
"#;
        std::fs::write(directory.join("domain.runa"), dependency)
            .expect("write qualified nominal dependency");
        let source = r#"
@ import A from ./domain
@ import B from ./domain
= a_pos = A.PWrapped(7)
= b_pos = B.PWrapped(7)
= a_named = A.NWrapped(value = 8)
= b_named = B.NWrapped(value = 8)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let statements = Parser::new(tokens, source)
            .parse_program()
            .expect("parse qualified nominal root");
        let mut dependency_lexer = Lexer::new(dependency);
        let dependency_tokens = dependency_lexer.tokenize();
        let dependency_statements = Parser::new(dependency_tokens, dependency)
            .parse_program()
            .expect("parse qualified nominal dependency");
        let catalog = calculate::TypeCatalog::collect_checked(&dependency_statements, None)
            .expect("collect qualified nominal types");
        let mut interpreter = Interpreter::new();
        interpreter.source_dir = Some(directory.to_string_lossy().to_string());
        let mut env = interpreter.default_env();
        let roots = ["a_pos", "b_pos", "a_named", "b_named"]
            .into_iter()
            .map(|name| ExploreRuntimeRoot::Value {
                name: name.to_string(),
            })
            .collect();
        interpreter
            .initialize_exploration_program(&roots, &statements, &mut env, 10_000, 1_000)
            .expect("initialize demanded qualified constructors");

        for (left_name, right_name, ty) in [
            ("a_pos", "b_pos", Ty::Name("Positional".to_string())),
            ("a_named", "b_named", Ty::Name("Named".to_string())),
        ] {
            let left_runtime = env.get(left_name).expect("left qualified constructor");
            let right_runtime = env.get(right_name).expect("right qualified constructor");
            let left = runtime_value_to_explore_value(left_runtime, &ty, &catalog)
                .expect("convert left qualified constructor");
            let right = runtime_value_to_explore_value(right_runtime, &ty, &catalog)
                .expect("convert right qualified constructor");
            let (
                ExploreValue::Constructor {
                    type_name: left_type,
                    ..
                },
                ExploreValue::Constructor {
                    type_name: right_type,
                    ..
                },
            ) = (&left, &right)
            else {
                panic!("qualified runtime constructors must remain constructors");
            };
            let (left_owner, left_declared) = runtime_nominal_type_parts(left_type)
                .expect("left exact value carries nominal owner");
            let (right_owner, right_declared) = runtime_nominal_type_parts(right_type)
                .expect("right exact value carries nominal owner");
            assert_ne!(
                left_owner, right_owner,
                "aliases are distinct nominal instances"
            );
            assert_eq!(left_declared, ty.to_string());
            assert_eq!(right_declared, ty.to_string());
            assert_ne!(left, right);
            assert!(values_equal(
                left_runtime,
                &runtime_value_from_explore_value(&left)
            ));
            assert!(values_equal(
                right_runtime,
                &runtime_value_from_explore_value(&right)
            ));
        }

        let _ = std::fs::remove_dir_all(&directory);
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
        vary status in values(ImportedStatus)
        vary declared_status in imported_statuses
        vary score in imported_scores()
        let before = ImportedProfile(status = status, score = score)
        given context = ()
    }
    transition after = before
    find imported_cases = all
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
        vary before in captured
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
        vary before in declared_choices
        given context = ()
    }
    transition after = before
    find invalid_cases = all
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
