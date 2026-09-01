//! Crash-resumable public materialization of a relational Explore journal.
//!
//! The durable semantic journal remains the authority. This module owns only
//! bounded NDJSON projections plus two small, atomically replaced control
//! files. A cursor advances by source ordinals, never by content-hash order.
//! At most one artifact batch is marked pending before bytes are appended, so
//! reopen can compare a torn suffix with records rederived from the journal
//! and either adopt complete matching lines or discard one incomplete tail.
//!
//! Ungrouped, unchosen `each case` views may publish row-local `SELECT` values
//! from accepted result evidence while FIND is open. Every other result follows
//! its already-journaled canonical projection ordinals and receives a final
//! closure record when that projection becomes exact. Compact mechanism files
//! follow the replay-derived append-only discovery stream while analysis
//! remains open: a first-seen signature publishes one content-addressed
//! descriptor before any dependent terminal, and an independently resumable
//! sidecar publishes the canonical definition chunks afterward. One exact
//! closure marker follows the final discovery event once the journal retains
//! the request closure; definition payload catch-up never hides that answer.
//! A second request-local sidecar streams structural assignments in discovery
//! order, then the exact quotient closure, and only after support closure one
//! bounded factorized support summary for each canonical mechanism and each
//! node/edge facet followed by its compact support receipt. A third,
//! independently resumable sidecar becomes readable at quotient closure and
//! publishes the normalized structural frame/context/node/edge/mechanism/profile
//! catalog in fixed typed chunks. Automatic structural rows never construct the
//! correlated starter union or expand a structural subject into subject x case
//! rows. Each explicitly authored starter consumer names one mechanism, node,
//! or edge facet plus one checked selected-case value view; its independently
//! resumable sidecar pages exactly that subject's typed starter relation from
//! closed support authority. A fixed case/support graph follows the same
//! crash-resumable flat cursor. Classified support runs
//! expose the structural path from their bounded partition; a fully
//! materialized extensional run exposes exact classification regions instead.
//! Both paths publish selected case identities only when a checked result
//! surface explicitly authorizes `CaseId` publication.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::{NonZeroU16, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView, ExprKind, Ty};

use super::mechanism_incidence::{
    MechanismCaseTerminal, MechanismCountEvidence, MechanismPublicationDiscoveryEvent,
    MechanismRequestScope, MechanismSignatureDefinition, MechanismSignatureId,
    MechanismUnavailableReasonDefinition,
};
use super::mechanism_support::{
    MechanismClosedSubjectStarterProjectionAuthority, MechanismFactorizedStarterBoundBasis,
    MechanismSupportCatalogBuilder, MechanismSupportClosureReceipt, MechanismSupportCount,
    MechanismSupportFacet, MechanismSupportKey, MechanismSupportStarterCursor,
    MechanismSupportSubject, AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT,
    MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION, MECHANISM_STARTER_PROJECTION_PLAN_VERSION,
    MECHANISM_SUPPORT_FIBER_EXPR_VERSION,
};
use super::relational_analysis_catalog::{
    RelationalAnalysisLayerSnapshot, RelationalAnalysisLayerStatus,
    RelationalMechanismClosureReceipt, RelationalResultLayerSnapshotState,
    RelationalResultPublication,
};
use super::relational_analysis_journal::{
    RelationalSelectedPopulationAuthority, RelationalSelectedQuestionSealId,
};
use super::relational_analysis_plan::RelationalAnalysisLayerId;
use super::relational_case_support_projection::{
    derive_relational_case_support_projection, RelationalCaseIdPublicationAuthority,
    RelationalCaseIdPublicationAuthorization, RelationalCaseSupportClosureAuthority,
    RelationalCaseSupportCount, RelationalCaseSupportOpenReason, RelationalCaseSupportProjection,
    RelationalCaseSupportProjectionFrontier, RelationalCaseSupportProjectionMetadata,
    RelationalCaseSupportProjectionRecord, RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA,
    RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION,
};
use super::relational_case_transition_projection::{
    derive_relational_case_transition_projection, RelationalCaseTransitionProjection,
    RelationalCaseTransitionProjectionRecord, RELATIONAL_CASE_TRANSITION_PROJECTION_SCHEMA,
    RELATIONAL_CASE_TRANSITION_PROJECTION_VERSION,
};
use super::relational_ir::{
    ExploreAnalysisNodeIr, ExploreMechanismTargetIr, ExploreResultGrainIr, ExploreResultInputIr,
    ExploreStarterProjectionFacetIr, ExploreStarterProjectionSubjectIr,
};
use super::relational_journal::{RelationalJournal, RelationalJournalContract};
use super::relational_mechanism_executor::{
    RelationalIfDecisionOutcome, RelationalMechanismCalleeId,
    RelationalMechanismEndpointDagSummary, RelationalMechanismEventOutcome,
    RelationalMechanismSignatureDagIndex, RelationalMechanismSiteId,
    RelationalMechanismUnavailableEvidence, RelationalRuleAttemptOutcome,
    RelationalRuleSelectionOutcome, RelationalShortCircuitOutcome,
};
use super::relational_mechanism_starter_authorization::{
    find_relational_mechanism_starter_value_authorization,
    relational_mechanism_starter_value_authorization_for_view,
    RelationalMechanismStarterAuthorizationError, RelationalMechanismStarterValueAuthorization,
    RelationalMechanismStarterValueRole, RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION,
};
use super::relational_mechanism_starter_projection::{
    RelationalMechanismStarterProjectionAccumulator, RelationalMechanismStarterProjectionClosure,
    RelationalMechanismStarterProjectionContentRoot, RelationalMechanismStarterProjectionJob,
    RelationalMechanismStarterProjectionPage, RelationalMechanismStarterProjectionPageManifestRoot,
    RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION,
};
use super::relational_public::{
    ExploreStreamCount, ExploreStreamCoverageBindingRole, ExploreStreamCoverageClassification,
    ExploreStreamCoverageGapReason, ExploreStreamCoverageLiteralKind,
    ExploreStreamCoverageRootRole, ExploreStreamCoverageSubject, ExploreStreamLayer,
    ExploreStreamLayerStatus, ExploreStreamLifecycle, ExploreStreamMechanismTarget,
    ExploreStreamPauseReason, ExploreStreamSliceReport,
};
use super::result_projection::{IndexedResultProjectionRecord, ResultProjectionRecord};
use super::result_view::{ResultGroupDisposition, ResultValue, ResultViewInputRowId};
use super::structural_mechanism::{
    StructuralDefinitionCatalogRoot, StructuralDefinitionKind, StructuralDefinitionRef,
    StructuralEndpointExecutionTotals, StructuralMechanismCatalogBuilder, StructuralMechanismId,
    StructuralNodeId, StructuralQuotientClosureReceipt, StructuralSignatureAssignment,
    STRUCTURAL_DEFINITION_CATALOG_VERSION, STRUCTURAL_MECHANISM_QUOTIENT_VERSION,
};
use super::{
    ExploreValue, MechanismRequestId, MechanismTargetId, RelationalCaseId, SourceKey, SuccessorKey,
    TransitionSchemaIdentities, ViewId,
};

pub(crate) const RELATIONAL_PUBLICATION_SCHEMA_VERSION: u32 = 9;

const CURSOR_FILE: &str = ".publication-cursor-v9.json";
const MANIFEST_FILE: &str = "manifest.json";
const MACOS_METADATA_FILE: &str = ".DS_Store";
const RESULT_PREFIX_ROOT_V9: &[u8] = b"futuruna.explore.publication-prefix.v9";
const RESULT_PREFIX_EXTEND_V9: &[u8] = b"futuruna.explore.publication-prefix-extend.v9";
const CASE_SUPPORT_ARTIFACT_KEY: &str = "graph:case-support";
const CASE_SUPPORT_ARTIFACT_NAME: &str = "case-support";
const CASE_SUPPORT_ARTIFACT_PATH: &str = "graphs/case-support.ndjson";
const CASE_TRANSITIONS_ARTIFACT_KEY: &str = "graph:case-transitions";
const CASE_TRANSITIONS_ARTIFACT_NAME: &str = "case-transitions";
const CASE_TRANSITIONS_ARTIFACT_PATH: &str = "graphs/case-transitions.ndjson";
const MECHANISM_DEFINITION_ENCODING: &str = "futuruna.relational-mechanism-signature-definition";
const MECHANISM_DEFINITION_ENCODING_VERSION: u32 = 1;
// Hex doubles this payload in JSON. Keeping it well below the default 1 MiB
// line bound leaves ample room for the attribution envelope and identifiers.
const MECHANISM_DEFINITION_CHUNK_BYTES: usize = 24 << 10;
const STRUCTURAL_DEFINITION_CHUNK_ITEMS: usize = 128;
const MECHANISM_STARTER_PAGE_MEMBER_LIMIT: NonZeroU16 =
    NonZeroU16::new(64).expect("nonzero constant");
const CONTROL_TEMP_ATTEMPTS: u64 = 128;
#[cfg(unix)]
const OWNER_ONLY_DIRECTORY_MODE: u32 = 0o700;
#[cfg(unix)]
const OWNER_ONLY_FILE_MODE: u32 = 0o600;

static CONTROL_TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

/// A durable journal coordinate suitable for public attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalPublicationCheckpoint {
    next_sequence: u64,
    head: [u8; 32],
}

impl RelationalPublicationCheckpoint {
    pub(crate) const fn new(next_sequence: u64, head: [u8; 32]) -> Self {
        Self {
            next_sequence,
            head,
        }
    }

    pub(crate) const fn next_sequence(self) -> u64 {
        self.next_sequence
    }

    pub(crate) const fn head(self) -> [u8; 32] {
        self.head
    }
}

/// Authority seam required by a materialized view of an append-only journal.
///
/// The production journal fold deliberately forgets historical entries. A
/// caller must therefore ask the durable segmented store—not the current head
/// alone—whether an older publisher cursor belongs to the same installed
/// chain. Implementations must return `false` for a sequence/head fork.
pub(crate) trait RelationalPublicationAuthority {
    fn journal(&self) -> Result<&RelationalJournal, String>;

    fn durable_checkpoint(&self) -> Result<RelationalPublicationCheckpoint, String>;

    fn authenticates_durable_prefix(
        &self,
        checkpoint: RelationalPublicationCheckpoint,
    ) -> Result<bool, String>;
}

/// Hard bounds for one publication call. They are operational and do not
/// participate in query, journal, result, or mechanism identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalPublicationLimits {
    max_line_bytes: usize,
    max_batch_bytes: usize,
    max_records_per_artifact: NonZeroUsize,
    max_control_bytes: usize,
    max_recovery_tail_bytes: usize,
}

impl RelationalPublicationLimits {
    pub(crate) fn new(
        max_line_bytes: usize,
        max_batch_bytes: usize,
        max_records_per_artifact: NonZeroUsize,
        max_control_bytes: usize,
        max_recovery_tail_bytes: usize,
    ) -> Result<Self, RelationalPublicationError> {
        if max_line_bytes == 0
            || max_batch_bytes < max_line_bytes
            || max_control_bytes == 0
            || max_recovery_tail_bytes < max_batch_bytes
        {
            return Err(RelationalPublicationError::InvalidLimits);
        }
        Ok(Self {
            max_line_bytes,
            max_batch_bytes,
            max_records_per_artifact,
            max_control_bytes,
            max_recovery_tail_bytes,
        })
    }

    pub(crate) const fn max_line_bytes(self) -> usize {
        self.max_line_bytes
    }

    pub(crate) const fn max_batch_bytes(self) -> usize {
        self.max_batch_bytes
    }

    pub(crate) const fn max_records_per_artifact(self) -> NonZeroUsize {
        self.max_records_per_artifact
    }

    pub(crate) const fn max_control_bytes(self) -> usize {
        self.max_control_bytes
    }

    pub(crate) const fn max_recovery_tail_bytes(self) -> usize {
        self.max_recovery_tail_bytes
    }
}

impl Default for RelationalPublicationLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: 1 << 20,
            max_batch_bytes: 8 << 20,
            max_records_per_artifact: NonZeroUsize::new(4096).expect("nonzero constant"),
            max_control_bytes: 4 << 20,
            max_recovery_tail_bytes: 16 << 20,
        }
    }
}

/// Exact envelope budget used while choosing a deterministic typed starter
/// page boundary. The checkpoint contributes bytes to the NDJSON envelope, so
/// sizing only the inner record would not be sound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PublicationLineBudget {
    checkpoint: RelationalPublicationCheckpoint,
    max_line_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultPublicationSource {
    /// Discovery-ordinal SELECT values whose row membership is already final.
    EarlyEachCase,
    /// Canonical durable projection records. Their ordinal is append-only even
    /// while the bounded projection prefix is still being journaled.
    DurableProjection,
}

/// Upstream population named by one result artifact. Mechanism-incidence
/// results need the request identity at publication time so a closed
/// projection over successful incidences cannot erase permanently unavailable
/// target cases from its public certainty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultPublicationInput {
    SourceOrSelected,
    MechanismIncidence { request_id: MechanismRequestId },
}

impl ResultPublicationInput {
    const fn mechanism_request_id(self) -> Option<MechanismRequestId> {
        match self {
            Self::SourceOrSelected => None,
            Self::MechanismIncidence { request_id } => Some(request_id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PublicationArtifactPlan {
    Result {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        view_id: ViewId,
        select_names: Box<[Box<str>]>,
        source: ResultPublicationSource,
        input: ResultPublicationInput,
    },
    Mechanism {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        request_id: MechanismRequestId,
        definitions_artifact_key: Box<str>,
        definitions_artifact_path: Box<str>,
    },
    MechanismDefinitions {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        request_id: MechanismRequestId,
        discovery_artifact_key: Box<str>,
    },
    MechanismStructural {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        request_id: MechanismRequestId,
        definitions_artifact_key: Box<str>,
        definitions_artifact_path: Box<str>,
    },
    MechanismStructuralDefinitions {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        request_id: MechanismRequestId,
        structural_artifact_key: Box<str>,
        structural_artifact_path: Box<str>,
    },
    SubjectStarters {
        key: Box<str>,
        name: Box<str>,
        path: PathBuf,
        consumer_id: [u8; 32],
        request_id: MechanismRequestId,
        target: MechanismTargetId,
        subject: MechanismSupportSubject,
        authorization: RelationalMechanismStarterValueAuthorization,
        transition_schemas: TransitionSchemaIdentities,
        structural_artifact_key: Box<str>,
        structural_artifact_path: Box<str>,
    },
    CaseSupport {
        authorization: Option<RelationalCaseIdPublicationAuthorization>,
    },
    CaseTransitions {
        authorization: RelationalMechanismStarterValueAuthorization,
        transition_schemas: TransitionSchemaIdentities,
    },
}

impl PublicationArtifactPlan {
    fn key(&self) -> &str {
        match self {
            Self::Result { key, .. }
            | Self::Mechanism { key, .. }
            | Self::MechanismDefinitions { key, .. }
            | Self::MechanismStructural { key, .. }
            | Self::MechanismStructuralDefinitions { key, .. }
            | Self::SubjectStarters { key, .. } => key,
            Self::CaseSupport { .. } => CASE_SUPPORT_ARTIFACT_KEY,
            Self::CaseTransitions { .. } => CASE_TRANSITIONS_ARTIFACT_KEY,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Result { name, .. }
            | Self::Mechanism { name, .. }
            | Self::MechanismDefinitions { name, .. }
            | Self::MechanismStructural { name, .. }
            | Self::MechanismStructuralDefinitions { name, .. }
            | Self::SubjectStarters { name, .. } => name,
            Self::CaseSupport { .. } => CASE_SUPPORT_ARTIFACT_NAME,
            Self::CaseTransitions { .. } => CASE_TRANSITIONS_ARTIFACT_NAME,
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::Result { path, .. }
            | Self::Mechanism { path, .. }
            | Self::MechanismDefinitions { path, .. }
            | Self::MechanismStructural { path, .. }
            | Self::MechanismStructuralDefinitions { path, .. }
            | Self::SubjectStarters { path, .. } => path,
            Self::CaseSupport { .. } => Path::new(CASE_SUPPORT_ARTIFACT_PATH),
            Self::CaseTransitions { .. } => Path::new(CASE_TRANSITIONS_ARTIFACT_PATH),
        }
    }

    const fn kind(&self) -> &'static str {
        match self {
            Self::Result { .. } => "result_view",
            Self::Mechanism { .. } => "mechanism_incidence",
            Self::MechanismDefinitions { .. } => "mechanism_definitions",
            Self::MechanismStructural { .. } => "mechanism_structural_support",
            Self::MechanismStructuralDefinitions { .. } => "mechanism_structural_definitions",
            Self::SubjectStarters { .. } => "subject_starter_support",
            Self::CaseSupport { .. } => "case_support_graph",
            Self::CaseTransitions { .. } => "selected_case_transition_graph",
        }
    }
}

/// Name/identity/SELECT contract for one checked query's public files.
#[derive(Clone, Debug)]
pub(crate) struct RelationalPublicationPlan {
    query_name: Box<str>,
    checked_program: Box<str>,
    contract: RelationalJournalContract,
    journal_id: [u8; 32],
    starter_consumer_set_id: [u8; 32],
    artifacts: Box<[PublicationArtifactPlan]>,
}

impl RelationalPublicationPlan {
    pub(crate) fn from_checked(
        checked: &CheckedExploreQueryView<'_>,
        contract: RelationalJournalContract,
    ) -> Result<Self, RelationalPublicationError> {
        if contract.relation_id() != checked.relation_id()
            || contract.admission_id() != checked.admission_id()
            || contract.question_id() != checked.question_id()
            || hex(contract.analysis_graph_digest()) != checked.analysis_graph_hash()
        {
            return Err(RelationalPublicationError::PlanIdentityMismatch);
        }

        let mut artifacts = Vec::with_capacity(
            checked
                .closed_query
                .analysis
                .len()
                .checked_mul(4)
                .and_then(|count| count.checked_add(checked.starter_projection_consumers().len()))
                .and_then(|count| count.checked_add(2))
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
        );
        let mut definition_artifacts = Vec::new();
        let mut structural_artifacts = Vec::new();
        let mut structural_definition_artifacts = Vec::new();
        let mut structural_references = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for (node_index, (node, identity)) in checked.analysis_nodes().enumerate() {
            let safe_name = safe_artifact_name(node.name())?;
            let artifact = match (node, identity) {
                (
                    ExploreAnalysisNodeIr::Result(view),
                    CheckedExploreAnalysisIdentity::View { view_id },
                ) => {
                    let input = match &view.input {
                        super::relational_ir::ExploreResultInputIr::Sources
                        | super::relational_ir::ExploreResultInputIr::Selected => {
                            ResultPublicationInput::SourceOrSelected
                        }
                        super::relational_ir::ExploreResultInputIr::MechanismIncidence {
                            request_node_index,
                        } => {
                            if *request_node_index >= node_index {
                                return Err(RelationalPublicationError::PlanIdentityMismatch);
                            }
                            let Some(CheckedExploreAnalysisIdentity::Mechanisms {
                                request_id, ..
                            }) = checked.artifact.analysis.get(*request_node_index)
                            else {
                                return Err(RelationalPublicationError::PlanIdentityMismatch);
                            };
                            ResultPublicationInput::MechanismIncidence {
                                request_id: *request_id,
                            }
                        }
                    };
                    // With no reducers, the checked result staging rules make
                    // every each-case SELECT field row-local. If reducers are
                    // declared, publication conservatively waits for the
                    // durable projection instead of rediscovering staging or
                    // switching source order after emitting an open prefix.
                    let source = if matches!(input, ResultPublicationInput::SourceOrSelected)
                        && matches!(view.grain, ExploreResultGrainIr::EachCase { .. })
                        && view.choose.is_none()
                        && view.aggregates.is_empty()
                    {
                        ResultPublicationSource::EarlyEachCase
                    } else {
                        ResultPublicationSource::DurableProjection
                    };
                    PublicationArtifactPlan::Result {
                        key: format!("view:{}", hex(view_id.bytes())).into_boxed_str(),
                        name: view.name.clone().into_boxed_str(),
                        path: PathBuf::from("views").join(format!("{safe_name}.ndjson")),
                        view_id: *view_id,
                        select_names: view
                            .select
                            .iter()
                            .map(|field| field.name.clone().into_boxed_str())
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        source,
                        input,
                    }
                }
                (
                    ExploreAnalysisNodeIr::Mechanisms(request),
                    CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. },
                ) => {
                    let request_id_hex = hex(request_id.bytes());
                    let discovery_artifact_key =
                        format!("mechanism:{request_id_hex}").into_boxed_str();
                    let definitions_artifact_key =
                        format!("mechanism-definitions:{request_id_hex}").into_boxed_str();
                    let structural_artifact_key =
                        format!("mechanism-structural:{request_id_hex}").into_boxed_str();
                    let structural_definitions_artifact_key =
                        format!("mechanism-structural-definitions:{request_id_hex}")
                            .into_boxed_str();
                    let definitions_path =
                        PathBuf::from("mechanisms").join(format!("{safe_name}.definitions.ndjson"));
                    let definitions_artifact_path =
                        path_to_manifest_string(&definitions_path)?.into_boxed_str();
                    let definitions = PublicationArtifactPlan::MechanismDefinitions {
                        key: definitions_artifact_key.clone(),
                        name: format!("{}_definitions", request.name).into_boxed_str(),
                        path: definitions_path,
                        request_id: *request_id,
                        discovery_artifact_key: discovery_artifact_key.clone(),
                    };
                    if !paths.insert(definitions.path().to_path_buf()) {
                        return Err(RelationalPublicationError::ArtifactPathCollision(
                            definitions.path().to_path_buf(),
                        ));
                    }
                    definition_artifacts.push(definitions);
                    let structural_path =
                        PathBuf::from("mechanisms").join(format!("{safe_name}.structural.ndjson"));
                    let structural_artifact_path =
                        path_to_manifest_string(&structural_path)?.into_boxed_str();
                    let target = checked_mechanism_target_at(checked, node_index, *request_id)?;
                    if structural_references
                        .insert(
                            *request_id,
                            (
                                target,
                                structural_artifact_key.clone(),
                                structural_artifact_path.clone(),
                            ),
                        )
                        .is_some()
                    {
                        return Err(RelationalPublicationError::PlanIdentityMismatch);
                    }
                    let structural_definitions_path = PathBuf::from("mechanisms")
                        .join(format!("{safe_name}.structural-definitions.ndjson"));
                    let structural_definitions_artifact_path =
                        path_to_manifest_string(&structural_definitions_path)?.into_boxed_str();
                    let structural = PublicationArtifactPlan::MechanismStructural {
                        key: structural_artifact_key.clone(),
                        name: format!("{}_structural", request.name).into_boxed_str(),
                        path: structural_path,
                        request_id: *request_id,
                        definitions_artifact_key: structural_definitions_artifact_key.clone(),
                        definitions_artifact_path: structural_definitions_artifact_path,
                    };
                    if !paths.insert(structural.path().to_path_buf()) {
                        return Err(RelationalPublicationError::ArtifactPathCollision(
                            structural.path().to_path_buf(),
                        ));
                    }
                    structural_artifacts.push(structural);
                    let structural_definitions =
                        PublicationArtifactPlan::MechanismStructuralDefinitions {
                            key: structural_definitions_artifact_key,
                            name: format!("{}_structural_definitions", request.name)
                                .into_boxed_str(),
                            path: structural_definitions_path,
                            request_id: *request_id,
                            structural_artifact_key,
                            structural_artifact_path,
                        };
                    if !paths.insert(structural_definitions.path().to_path_buf()) {
                        return Err(RelationalPublicationError::ArtifactPathCollision(
                            structural_definitions.path().to_path_buf(),
                        ));
                    }
                    structural_definition_artifacts.push(structural_definitions);
                    PublicationArtifactPlan::Mechanism {
                        key: discovery_artifact_key,
                        name: request.name.clone().into_boxed_str(),
                        path: PathBuf::from("mechanisms").join(format!("{safe_name}.ndjson")),
                        request_id: *request_id,
                        definitions_artifact_key,
                        definitions_artifact_path,
                    }
                }
                _ => return Err(RelationalPublicationError::PlanIdentityMismatch),
            };
            if !paths.insert(artifact.path().to_path_buf()) {
                return Err(RelationalPublicationError::ArtifactPathCollision(
                    artifact.path().to_path_buf(),
                ));
            }
            artifacts.push(artifact);
        }
        // Structural assignments and the compact support quotient are answer
        // artifacts, so service them before the potentially much larger raw
        // definition payload sidecars.
        artifacts.extend(structural_artifacts);
        // Explicit typed starter consumers are independently resumable and
        // closure-gated. They follow the compact structural support which
        // names the factorized plan identities they select.
        for (projection, identity) in checked.starter_projection_consumers() {
            if projection.subject != identity.subject {
                return Err(RelationalPublicationError::PlanIdentityMismatch);
            }
            let Some((target, structural_artifact_key, structural_artifact_path)) =
                structural_references.get(&identity.request_id)
            else {
                return Err(RelationalPublicationError::PlanIdentityMismatch);
            };
            if checked_mechanism_target_at(
                checked,
                projection.request_node_index,
                identity.request_id,
            )? != *target
                || !matches!(
                    checked.artifact.analysis.get(projection.value_view_node_index),
                    Some(CheckedExploreAnalysisIdentity::View { view_id })
                        if *view_id == identity.authorizing_view_id
                )
            {
                return Err(RelationalPublicationError::PlanIdentityMismatch);
            }
            let authorization = relational_mechanism_starter_value_authorization_for_view(
                *checked,
                identity.authorizing_view_id,
            )
            .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
            let safe_name = safe_artifact_name(&projection.name)?;
            let path = PathBuf::from("starters").join(format!("{safe_name}.ndjson"));
            let artifact = PublicationArtifactPlan::SubjectStarters {
                key: format!("subject-starters:{}", hex(identity.id.bytes())).into_boxed_str(),
                name: projection.name.clone().into_boxed_str(),
                path,
                consumer_id: identity.id.bytes(),
                request_id: identity.request_id,
                target: *target,
                subject: mechanism_support_subject(identity.subject),
                authorization,
                transition_schemas: checked.transition_schemas().clone(),
                structural_artifact_key: structural_artifact_key.clone(),
                structural_artifact_path: structural_artifact_path.clone(),
            };
            if !paths.insert(artifact.path().to_path_buf()) {
                return Err(RelationalPublicationError::ArtifactPathCollision(
                    artifact.path().to_path_buf(),
                ));
            }
            artifacts.push(artifact);
        }
        let case_support = PublicationArtifactPlan::CaseSupport {
            authorization: checked_case_id_publication_authorization(checked),
        };
        if !paths.insert(case_support.path().to_path_buf()) {
            return Err(RelationalPublicationError::ArtifactPathCollision(
                case_support.path().to_path_buf(),
            ));
        }
        artifacts.push(case_support);
        let case_transition_authorization =
            match find_relational_mechanism_starter_value_authorization(*checked) {
                Ok(authorization) => Some(authorization),
                Err(RelationalMechanismStarterAuthorizationError::NoCompatibleSelectedCaseView) => {
                    None
                }
                Err(error) => {
                    return Err(RelationalPublicationError::Analysis(error.to_string()));
                }
            };
        if let Some(authorization) = case_transition_authorization {
            let case_transitions = PublicationArtifactPlan::CaseTransitions {
                authorization,
                transition_schemas: checked.transition_schemas().clone(),
            };
            if !paths.insert(case_transitions.path().to_path_buf()) {
                return Err(RelationalPublicationError::ArtifactPathCollision(
                    case_transitions.path().to_path_buf(),
                ));
            }
            artifacts.push(case_transitions);
        }
        // Normalized structural definitions become independently readable at
        // quotient closure. They remain behind compact answers in servicing
        // order and ahead of the much larger raw signature payload lane.
        artifacts.extend(structural_definition_artifacts);
        // Definition payloads are deliberately serviced after every compact
        // answer, result, and case-support artifact. Their independent cursors
        // may lag without delaying interesting semantic output.
        artifacts.extend(definition_artifacts);

        Ok(Self {
            query_name: checked.closed_query.name.clone().into_boxed_str(),
            checked_program: checked.program_hash().to_string().into_boxed_str(),
            contract,
            journal_id: contract.id().bytes(),
            starter_consumer_set_id: checked.starter_consumer_set_id().bytes(),
            artifacts: artifacts.into_boxed_slice(),
        })
    }

    pub(crate) fn query_name(&self) -> &str {
        &self.query_name
    }

    pub(crate) const fn journal_id(&self) -> [u8; 32] {
        self.journal_id
    }
}

fn checked_mechanism_target_at(
    checked: &CheckedExploreQueryView<'_>,
    request_node_index: usize,
    expected_request_id: MechanismRequestId,
) -> Result<MechanismTargetId, RelationalPublicationError> {
    let (
        Some(ExploreAnalysisNodeIr::Mechanisms(request)),
        Some(CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. }),
    ) = (
        checked.closed_query.analysis.get(request_node_index),
        checked.artifact.analysis.get(request_node_index),
    )
    else {
        return Err(RelationalPublicationError::PlanIdentityMismatch);
    };
    if *request_id != expected_request_id {
        return Err(RelationalPublicationError::PlanIdentityMismatch);
    }
    match &request.target {
        ExploreMechanismTargetIr::SelectedCases => Ok(MechanismTargetId::Selected),
        ExploreMechanismTargetIr::ViewChosen { view_node_index } => {
            let Some(CheckedExploreAnalysisIdentity::View { view_id }) =
                checked.artifact.analysis.get(*view_node_index)
            else {
                return Err(RelationalPublicationError::PlanIdentityMismatch);
            };
            Ok(MechanismTargetId::ChosenView(*view_id))
        }
    }
}

const fn mechanism_support_subject(
    subject: ExploreStarterProjectionSubjectIr,
) -> MechanismSupportSubject {
    match subject {
        ExploreStarterProjectionSubjectIr::Mechanism(mechanism_id) => {
            MechanismSupportSubject::Mechanism(mechanism_id)
        }
        ExploreStarterProjectionSubjectIr::Node { facet, node_id } => {
            MechanismSupportSubject::Node {
                facet: mechanism_support_facet(facet),
                node_id,
            }
        }
        ExploreStarterProjectionSubjectIr::Edge { facet, edge_id } => {
            MechanismSupportSubject::Edge {
                facet: mechanism_support_facet(facet),
                edge_id,
            }
        }
    }
}

const fn mechanism_support_facet(facet: ExploreStarterProjectionFacetIr) -> MechanismSupportFacet {
    match facet {
        ExploreStarterProjectionFacetIr::Activation => MechanismSupportFacet::Activation,
        ExploreStarterProjectionFacetIr::DifferentialParticipation => {
            MechanismSupportFacet::DifferentialParticipation
        }
    }
}

fn checked_case_id_publication_authorization(
    checked: &CheckedExploreQueryView<'_>,
) -> Option<RelationalCaseIdPublicationAuthorization> {
    checked
        .analysis_nodes()
        .filter_map(|(node, identity)| {
            let (
                ExploreAnalysisNodeIr::Result(view),
                CheckedExploreAnalysisIdentity::View { view_id },
            ) = (node, identity)
            else {
                return None;
            };
            (matches!(&view.input, ExploreResultInputIr::Selected)
                && matches!(&view.grain, ExploreResultGrainIr::EachCase { .. })
                && view.aggregates.is_empty()
                && view.having.is_none()
                && view.choose.is_none()
                && view.select.iter().any(|field| {
                    matches!(&field.value.kind, ExprKind::Var(name) if name == "case_id")
                        && matches!(&field.ty, Ty::Name(name) if name == "CaseId")
                }))
            .then_some(*view_id)
        })
        .min()
        .map(RelationalCaseIdPublicationAuthorization::from_checked_result_view)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalPublicationSummary {
    manifest_path: PathBuf,
    lines_appended: u64,
    source_ordinals_advanced: u64,
    artifacts_caught_up: usize,
    artifact_count: usize,
}

impl RelationalPublicationSummary {
    pub(crate) fn manifest_path(&self) -> &Path {
        self.manifest_path.as_path()
    }

    pub(crate) const fn lines_appended(&self) -> u64 {
        self.lines_appended
    }

    pub(crate) const fn source_ordinals_advanced(&self) -> u64 {
        self.source_ordinals_advanced
    }

    pub(crate) const fn artifacts_caught_up(&self) -> usize {
        self.artifacts_caught_up
    }

    pub(crate) const fn artifact_count(&self) -> usize {
        self.artifact_count
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CursorCheckpoint {
    next_sequence: u64,
    journal_head: String,
}

impl CursorCheckpoint {
    fn from_checkpoint(checkpoint: RelationalPublicationCheckpoint) -> Self {
        Self {
            next_sequence: checkpoint.next_sequence,
            journal_head: hex(checkpoint.head),
        }
    }

    fn decode(&self) -> Result<RelationalPublicationCheckpoint, RelationalPublicationError> {
        Ok(RelationalPublicationCheckpoint {
            next_sequence: self.next_sequence,
            head: decode_hex_digest(&self.journal_head)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LastLineCursor {
    start: u64,
    bytes: u64,
    digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct CursorDigest(#[serde(with = "hex_digest_wire")] [u8; 32]);

impl CursorDigest {
    const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SubjectStarterTargetCursor {
    Selected,
    ChosenView { view_id: CursorDigest },
}

impl SubjectStarterTargetCursor {
    const fn from_semantic(target: MechanismTargetId) -> Self {
        match target {
            MechanismTargetId::Selected => Self::Selected,
            MechanismTargetId::ChosenView(view_id) => Self::ChosenView {
                view_id: CursorDigest::new(view_id.bytes()),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SubjectStarterFacetCursor {
    Activation,
    DifferentialParticipation,
}

impl SubjectStarterFacetCursor {
    const fn from_semantic(facet: MechanismSupportFacet) -> Self {
        match facet {
            MechanismSupportFacet::Activation => Self::Activation,
            MechanismSupportFacet::DifferentialParticipation => Self::DifferentialParticipation,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SubjectStarterSubjectCursor {
    Mechanism {
        mechanism_id: CursorDigest,
    },
    Node {
        facet: SubjectStarterFacetCursor,
        node_id: CursorDigest,
    },
    Edge {
        facet: SubjectStarterFacetCursor,
        edge_id: CursorDigest,
    },
}

impl SubjectStarterSubjectCursor {
    const fn from_semantic(subject: MechanismSupportSubject) -> Self {
        match subject {
            MechanismSupportSubject::Mechanism(mechanism_id) => Self::Mechanism {
                mechanism_id: CursorDigest::new(mechanism_id.bytes()),
            },
            MechanismSupportSubject::Node { facet, node_id } => Self::Node {
                facet: SubjectStarterFacetCursor::from_semantic(facet),
                node_id: CursorDigest::new(node_id.bytes()),
            },
            MechanismSupportSubject::Edge { facet, edge_id } => Self::Edge {
                facet: SubjectStarterFacetCursor::from_semantic(facet),
                edge_id: CursorDigest::new(edge_id.bytes()),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SubjectStarterCursorIdentity {
    consumer_id: CursorDigest,
    request_id: CursorDigest,
    target: SubjectStarterTargetCursor,
    subject: SubjectStarterSubjectCursor,
}

impl SubjectStarterCursorIdentity {
    const fn new(
        consumer_id: [u8; 32],
        request_id: MechanismRequestId,
        target: MechanismTargetId,
        subject: MechanismSupportSubject,
    ) -> Self {
        Self {
            consumer_id: CursorDigest::new(consumer_id),
            request_id: CursorDigest::new(request_id.bytes()),
            target: SubjectStarterTargetCursor::from_semantic(target),
            subject: SubjectStarterSubjectCursor::from_semantic(subject),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MechanismStarterKeyCursor {
    source_key: CursorDigest,
    successor_key: CursorDigest,
}

impl MechanismStarterKeyCursor {
    const fn from_semantic(cursor: MechanismSupportStarterCursor) -> Self {
        Self {
            source_key: CursorDigest::new(cursor.source_key().bytes()),
            successor_key: CursorDigest::new(cursor.successor_key().bytes()),
        }
    }

    const fn into_semantic(self) -> MechanismSupportStarterCursor {
        MechanismSupportStarterCursor::new(
            SourceKey::from_journal_codec_bytes(self.source_key.bytes()),
            SuccessorKey::from_journal_codec_bytes(self.successor_key.bytes()),
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MechanismStarterAccumulatorCursor {
    job_id: CursorDigest,
    #[serde(with = "decimal_u128_wire")]
    next_page_ordinal: u128,
    last_cursor: Option<MechanismStarterKeyCursor>,
    last_source_key: Option<CursorDigest>,
    #[serde(with = "decimal_u128_wire")]
    exact_member_count: u128,
    #[serde(with = "decimal_u128_wire")]
    exact_starter_count: u128,
    content_root: CursorDigest,
    page_manifest_root: CursorDigest,
    exhausted: bool,
}

impl MechanismStarterAccumulatorCursor {
    fn from_accumulator(accumulator: RelationalMechanismStarterProjectionAccumulator) -> Self {
        Self {
            job_id: CursorDigest::new(accumulator.job_id().bytes()),
            next_page_ordinal: accumulator.next_page_ordinal(),
            last_cursor: accumulator
                .last_cursor()
                .map(MechanismStarterKeyCursor::from_semantic),
            last_source_key: accumulator
                .last_source_key()
                .map(|key| CursorDigest::new(key.bytes())),
            exact_member_count: accumulator.exact_member_count(),
            exact_starter_count: accumulator.exact_starter_count(),
            content_root: CursorDigest::new(accumulator.content_root().bytes()),
            page_manifest_root: CursorDigest::new(accumulator.page_manifest_root().bytes()),
            exhausted: accumulator.exhausted(),
        }
    }

    fn restore(
        self,
        job: RelationalMechanismStarterProjectionJob,
    ) -> Result<RelationalMechanismStarterProjectionAccumulator, RelationalPublicationError> {
        if self.job_id.bytes() != job.id().bytes() {
            return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
        }
        RelationalMechanismStarterProjectionAccumulator::restore_from_authenticated_checkpoint(
            job,
            self.next_page_ordinal,
            self.last_cursor.map(MechanismStarterKeyCursor::into_semantic),
            self.last_source_key
                .map(|key| SourceKey::from_journal_codec_bytes(key.bytes())),
            self.exact_member_count,
            self.exact_starter_count,
            RelationalMechanismStarterProjectionContentRoot::from_authenticated_checkpoint_bytes(
                self.content_root.bytes(),
            ),
            RelationalMechanismStarterProjectionPageManifestRoot::from_authenticated_checkpoint_bytes(
                self.page_manifest_root.bytes(),
            ),
            self.exhausted,
        )
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))
    }
}

/// Next source position for one public artifact. Result views and the
/// case/support graph retain a flat append-only ordinal. A mechanism discovery
/// event is one compact public record. Canonical definition payloads advance
/// on a separate signature-local cursor so they cannot block incidences. The
/// closure-frozen normalized structural catalog has its own definition/part
/// cursor and is independently root-bound to the structural quotient.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ArtifactSourceCursor {
    Flat {
        #[serde(with = "decimal_u128_wire")]
        next_source_ordinal: u128,
    },
    MechanismDiscovery {
        #[serde(with = "decimal_u128_wire")]
        event_ordinal: u128,
        closure_emitted: bool,
    },
    MechanismDefinitions {
        #[serde(with = "decimal_u128_wire")]
        signature_ordinal: u128,
        #[serde(with = "decimal_u128_wire")]
        definition_part_ordinal: u128,
        closure_emitted: bool,
    },
    StructuralDefinitions {
        header_emitted: bool,
        #[serde(with = "decimal_u128_wire")]
        definition_ordinal: u128,
        #[serde(with = "decimal_u128_wire")]
        definition_part_ordinal: u128,
        closure_emitted: bool,
    },
    SubjectStarters {
        identity: SubjectStarterCursorIdentity,
        header_emitted: bool,
        accumulator: Option<MechanismStarterAccumulatorCursor>,
        closure_emitted: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationSourceCoordinate {
    Flat {
        source_ordinal: u128,
    },
    MechanismEvent {
        event_ordinal: u128,
    },
    MechanismDefinition {
        signature_ordinal: u128,
        definition_part_ordinal: u128,
    },
    MechanismClosure {
        event_end: u128,
    },
    MechanismDefinitionsClosure {
        signature_end: u128,
    },
    StructuralDefinitionsHeader,
    StructuralDefinition {
        definition_ordinal: u128,
        definition_part_ordinal: u128,
    },
    StructuralDefinitionsClosure {
        definition_end: u128,
    },
    SubjectStartersHeader {
        subject: MechanismSupportSubject,
    },
    SubjectStartersPage {
        subject: MechanismSupportSubject,
        page_ordinal: u128,
    },
    SubjectStartersClosure {
        subject: MechanismSupportSubject,
    },
}

impl PublicationSourceCoordinate {
    fn describe(self) -> String {
        match self {
            Self::Flat { source_ordinal } => source_ordinal.to_string(),
            Self::MechanismEvent { event_ordinal } => format!("event {event_ordinal}"),
            Self::MechanismDefinition {
                signature_ordinal,
                definition_part_ordinal,
            } => {
                format!("signature {signature_ordinal}, definition part {definition_part_ordinal}")
            }
            Self::MechanismClosure { event_end } => {
                format!("closure after event prefix {event_end}")
            }
            Self::MechanismDefinitionsClosure { signature_end } => {
                format!("definition closure after signature prefix {signature_end}")
            }
            Self::StructuralDefinitionsHeader => "structural definition catalog header".into(),
            Self::StructuralDefinition {
                definition_ordinal,
                definition_part_ordinal,
            } => format!(
                "structural definition {definition_ordinal}, part {definition_part_ordinal}"
            ),
            Self::StructuralDefinitionsClosure { definition_end } => {
                format!("structural definition closure after {definition_end} definitions")
            }
            Self::SubjectStartersHeader { subject } => {
                format!("subject starter {:?} header", subject)
            }
            Self::SubjectStartersPage {
                subject,
                page_ordinal,
            } => format!("subject starter {:?}, page {page_ordinal}", subject),
            Self::SubjectStartersClosure { subject } => {
                format!("subject starter {:?} closure", subject)
            }
        }
    }

    fn mechanism_json(self) -> Option<JsonValue> {
        match self {
            Self::Flat { .. } => None,
            Self::MechanismEvent { event_ordinal } => Some(json!({
                "kind": "mechanism_event",
                "event_ordinal": event_ordinal.to_string(),
            })),
            Self::MechanismDefinition {
                signature_ordinal,
                definition_part_ordinal,
            } => Some(json!({
                "kind": "mechanism_definition_part",
                "signature_ordinal": signature_ordinal.to_string(),
                "definition_part_ordinal": definition_part_ordinal.to_string(),
            })),
            Self::MechanismClosure { event_end } => Some(json!({
                "kind": "mechanism_closure",
                "event_end": event_end.to_string(),
            })),
            Self::MechanismDefinitionsClosure { signature_end } => Some(json!({
                "kind": "mechanism_definitions_closure",
                "signature_end": signature_end.to_string(),
            })),
            Self::StructuralDefinitionsHeader => Some(json!({
                "kind": "structural_definitions_header",
            })),
            Self::StructuralDefinition {
                definition_ordinal,
                definition_part_ordinal,
            } => Some(json!({
                "kind": "structural_definition_part",
                "definition_ordinal": definition_ordinal.to_string(),
                "definition_part_ordinal": definition_part_ordinal.to_string(),
            })),
            Self::StructuralDefinitionsClosure { definition_end } => Some(json!({
                "kind": "structural_definitions_closure",
                "definition_end": definition_end.to_string(),
            })),
            Self::SubjectStartersHeader { subject } => Some(json!({
                "kind": "subject_starters_header",
                "subject": public_mechanism_support_subject(subject),
            })),
            Self::SubjectStartersPage {
                subject,
                page_ordinal,
            } => Some(json!({
                "kind": "subject_starters_page",
                "subject": public_mechanism_support_subject(subject),
                "page_ordinal": page_ordinal.to_string(),
            })),
            Self::SubjectStartersClosure { subject } => Some(json!({
                "kind": "subject_starters_closure",
                "subject": public_mechanism_support_subject(subject),
            })),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ArtifactCursor {
    kind: String,
    path: String,
    source: ArtifactSourceCursor,
    #[serde(with = "decimal_u128_wire")]
    line_count: u128,
    byte_len: u64,
    prefix_digest: String,
    last_line: Option<LastLineCursor>,
}

/// Source authority frozen before a pending batch appends bytes. Mechanism
/// replay may advance before crash recovery runs, so the old checkpoint may
/// authorize only this event prefix and, when present, this exact closure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PendingArtifactSourceEnd {
    /// Freeze the exact flat prefix authorized by the pending checkpoint.
    Flat {
        #[serde(with = "decimal_u128_wire")]
        source_end: u128,
    },
    MechanismDiscovery {
        #[serde(with = "decimal_u128_wire")]
        event_end: u128,
        closure_root: Option<String>,
    },
    MechanismDefinitions {
        #[serde(with = "decimal_u128_wire")]
        signature_end: u128,
        closure_root: Option<String>,
    },
    StructuralDefinitions {
        #[serde(with = "decimal_u128_wire")]
        definition_end: u128,
        structural_quotient_root: Option<String>,
        definition_catalog_root: Option<String>,
    },
    SubjectStarters {
        identity: SubjectStarterCursorIdentity,
        structural_quotient_root: Option<CursorDigest>,
        mechanism_support_root: Option<CursorDigest>,
        projection_plan_id: Option<CursorDigest>,
        projection_job_id: Option<CursorDigest>,
    },
}

fn source_cursor_matches_artifact(
    source: ArtifactSourceCursor,
    artifact: &PublicationArtifactPlan,
) -> bool {
    match (source, artifact) {
        (
            ArtifactSourceCursor::Flat { .. },
            PublicationArtifactPlan::Result { .. }
            | PublicationArtifactPlan::MechanismStructural { .. }
            | PublicationArtifactPlan::CaseSupport { .. }
            | PublicationArtifactPlan::CaseTransitions { .. },
        )
        | (
            ArtifactSourceCursor::MechanismDiscovery { .. },
            PublicationArtifactPlan::Mechanism { .. },
        )
        | (
            ArtifactSourceCursor::MechanismDefinitions { .. },
            PublicationArtifactPlan::MechanismDefinitions { .. },
        )
        | (
            ArtifactSourceCursor::StructuralDefinitions { .. },
            PublicationArtifactPlan::MechanismStructuralDefinitions { .. },
        ) => true,
        (
            ArtifactSourceCursor::SubjectStarters { identity, .. },
            PublicationArtifactPlan::SubjectStarters {
                consumer_id,
                request_id,
                target,
                subject,
                ..
            },
        ) => {
            identity
                == SubjectStarterCursorIdentity::new(*consumer_id, *request_id, *target, *subject)
        }
        _ => false,
    }
}

fn pending_source_end_matches_artifact(
    source_end: &PendingArtifactSourceEnd,
    artifact: &PublicationArtifactPlan,
) -> bool {
    match (source_end, artifact) {
        (
            PendingArtifactSourceEnd::Flat { .. },
            PublicationArtifactPlan::Result { .. }
            | PublicationArtifactPlan::MechanismStructural { .. }
            | PublicationArtifactPlan::CaseSupport { .. }
            | PublicationArtifactPlan::CaseTransitions { .. },
        ) => true,
        (
            PendingArtifactSourceEnd::MechanismDiscovery { closure_root, .. },
            PublicationArtifactPlan::Mechanism { .. },
        ) => closure_root
            .as_deref()
            .is_none_or(|root| decode_hex_digest(root).is_ok()),
        (
            PendingArtifactSourceEnd::MechanismDefinitions { closure_root, .. },
            PublicationArtifactPlan::MechanismDefinitions { .. },
        ) => closure_root
            .as_deref()
            .is_none_or(|root| decode_hex_digest(root).is_ok()),
        (
            PendingArtifactSourceEnd::StructuralDefinitions {
                structural_quotient_root,
                definition_catalog_root,
                ..
            },
            PublicationArtifactPlan::MechanismStructuralDefinitions { .. },
        ) => match (
            structural_quotient_root.as_deref(),
            definition_catalog_root.as_deref(),
        ) {
            (None, None) => true,
            (Some(structural_root), Some(definition_root)) => {
                decode_hex_digest(structural_root).is_ok()
                    && decode_hex_digest(definition_root).is_ok()
            }
            _ => false,
        },
        (
            PendingArtifactSourceEnd::SubjectStarters {
                identity,
                structural_quotient_root,
                mechanism_support_root,
                projection_plan_id,
                projection_job_id,
            },
            PublicationArtifactPlan::SubjectStarters {
                consumer_id,
                request_id,
                target,
                subject,
                ..
            },
        ) => {
            *identity
                == SubjectStarterCursorIdentity::new(*consumer_id, *request_id, *target, *subject)
                && matches!(
                    (
                        structural_quotient_root,
                        mechanism_support_root,
                        projection_plan_id,
                        projection_job_id,
                    ),
                    (None, None, None, None) | (Some(_), Some(_), Some(_), Some(_))
                )
        }
        _ => false,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PendingArtifactBatch {
    checkpoint: CursorCheckpoint,
    artifact_key: String,
    first_source: ArtifactSourceCursor,
    source_end: PendingArtifactSourceEnd,
    #[serde(with = "decimal_u128_wire")]
    first_line_count: u128,
    first_byte_len: u64,
    first_prefix_digest: String,
    first_last_line: Option<LastLineCursor>,
    /// Freeze the line budget which chose any starter-page boundary in this
    /// pending batch. Recovery may run with a larger operational limit but
    /// must rederive byte-for-byte identical pages.
    max_line_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationCursor {
    schema_version: u32,
    journal_id: String,
    query_name: String,
    checkpoint: CursorCheckpoint,
    artifacts: BTreeMap<String, ArtifactCursor>,
    pending: Option<PendingArtifactBatch>,
}

/// Publication ordinals are exact `u128` values. serde_json can emit those as
/// JSON numbers but its default deserializer does not accept `deserialize_u128`
/// on the supported dependency version. Write the same decimal-string form as
/// the public manifest and accept in-range numeric v7 cursors so a crash-safe
/// stream can repair itself in place.
mod decimal_u128_wire {
    use std::fmt;

    use serde::de::{Error as _, Visitor};
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DecimalU128Visitor)
    }

    struct DecimalU128Visitor;

    impl Visitor<'_> for DecimalU128Visitor {
        type Value = u128;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a decimal u128 string or non-negative integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            value.parse::<u128>().map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(u128::from(value))
        }

        fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            u128::try_from(value).map_err(E::custom)
        }
    }
}

mod hex_digest_wire {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&super::hex(*value))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        super::decode_hex_digest(&value).map_err(D::Error::custom)
    }
}

enum PublicationRecord {
    Emit(JsonValue),
    Skip,
    NotReady,
    Exhausted,
}

enum AddressedPublicationRecord {
    Emit {
        coordinate: PublicationSourceCoordinate,
        next: ArtifactSourceCursor,
        value: JsonValue,
    },
    Skip {
        next: ArtifactSourceCursor,
    },
    NotReady,
    Exhausted,
}

/// Publication-only coverage for a result whose reducer input contains only
/// successful mechanism incidences. The semantic result remains an exact
/// projection of that relation; this qualifier prevents it from being
/// presented as an exact answer over all requested target cases when one or
/// more cases have a durable unavailable terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanismResultInputCoverage {
    request_id: MechanismRequestId,
    target_is_sealed: bool,
    frontier_complete: bool,
    target_cases: u128,
    incidence_cases: u128,
    unavailable_cases: u128,
}

impl MechanismResultInputCoverage {
    const fn open(request_id: MechanismRequestId) -> Self {
        Self {
            request_id,
            target_is_sealed: false,
            frontier_complete: false,
            target_cases: 0,
            incidence_cases: 0,
            unavailable_cases: 0,
        }
    }

    const fn certainty_frontier(self) -> &'static str {
        if !self.frontier_complete {
            "open"
        } else if self.unavailable_cases == 0 {
            "exact"
        } else if self.incidence_cases == 0 {
            "unknown"
        } else {
            "lower_bound"
        }
    }
}

/// Invocation-local cache for definitions actually addressed by mechanism
/// publication. It is never serialized and cannot affect semantic identity.
/// Open publication therefore pays DAG-index cost only for signatures whose
/// discovery events are reached during this bounded call or tail recovery.
enum PublicationCaseSupportProjection<'journal> {
    Partitioned(RelationalCaseSupportProjection<'journal>),
    ClassificationSummary(RelationalClassificationSummaryProjection<'journal>),
}

enum PublicationCaseSupportRecord {
    Partitioned(RelationalCaseSupportProjectionRecord),
    ClassificationSummary(RelationalClassificationSummaryProjectionRecord),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalClassificationSummaryOutcome {
    Rejected,
    AdmittedNotSelected,
    AdmittedSelected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalClassificationSummaryClosureMetadata {
    classification_authority: RelationalPublishedClassificationAuthority,
    support_evidence_root: [u8; 32],
    selected_question_seal_id: RelationalSelectedQuestionSealId,
    selected_population_authority: RelationalPublishedSelectedPopulationAuthority,
    exact_logical_case_count: u128,
    exact_admitted_case_count: u128,
    exact_selected_case_count: u128,
    authorized_case_record_count: u128,
    data_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalPublishedClassificationAuthority {
    CertifiedSupport,
    ExtensionalCatalog,
    ComposedExactEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalPublishedSelectedPopulationAuthority {
    ExtensionalQuestion { question_content_root: [u8; 32] },
    CertifiedSupport { population_root: [u8; 32] },
}

enum RelationalClassificationSummaryProjectionRecord {
    Root {
        contract: RelationalJournalContract,
        support_plan_root: [u8; 32],
        classification_authority: RelationalPublishedClassificationAuthority,
        exact_logical_case_count: u128,
        exact_admitted_case_count: u128,
        exact_selected_case_count: u128,
        case_id_authority: Option<RelationalCaseIdPublicationAuthority>,
    },
    Region {
        question_id: super::relation::QuestionId,
        region_ordinal: u8,
        exact_case_count: u128,
        outcome: RelationalClassificationSummaryOutcome,
    },
    AuthorizedCase {
        question_id: super::relation::QuestionId,
        selected_region_ordinal: u8,
        case_id: RelationalCaseId,
        authority: RelationalCaseIdPublicationAuthority,
    },
    Closure(RelationalClassificationSummaryClosureMetadata),
}

struct RelationalClassificationSummaryProjection<'journal> {
    contract: RelationalJournalContract,
    support_plan_root: [u8; 32],
    selected_case_ids: &'journal [RelationalCaseId],
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    rejected_case_count: u128,
    admitted_not_selected_case_count: u128,
    closure: RelationalClassificationSummaryClosureMetadata,
}

impl PublicationCaseSupportProjection<'_> {
    fn available_source_record_count(&self) -> u128 {
        match self {
            Self::Partitioned(projection) => projection.available_source_record_count(),
            Self::ClassificationSummary(projection) => projection.available_source_record_count(),
        }
    }

    fn is_open(&self) -> bool {
        match self {
            Self::Partitioned(projection) => matches!(
                projection.metadata().frontier,
                RelationalCaseSupportProjectionFrontier::Open(_)
            ),
            Self::ClassificationSummary(_) => false,
        }
    }

    fn record_at(
        &self,
        source_ordinal: u128,
    ) -> Result<Option<PublicationCaseSupportRecord>, RelationalPublicationError> {
        match self {
            Self::Partitioned(projection) => projection
                .record_at(source_ordinal)
                .map(|record| record.map(PublicationCaseSupportRecord::Partitioned))
                .map_err(|error| RelationalPublicationError::CaseSupport(error.to_string())),
            Self::ClassificationSummary(projection) => projection
                .record_at(source_ordinal)
                .map(|record| record.map(PublicationCaseSupportRecord::ClassificationSummary)),
        }
    }
}

impl RelationalClassificationSummaryProjection<'_> {
    const REGION_COUNT: u128 = 3;
    const SELECTED_REGION_ORDINAL: u8 = 2;

    fn authorized_case_record_count(&self) -> u128 {
        if self.authorization.is_some() {
            self.closure.exact_selected_case_count
        } else {
            0
        }
    }

    fn available_source_record_count(&self) -> u128 {
        self.closure
            .data_record_count
            .checked_add(1)
            .expect("validated extensional case-support record count")
    }

    fn record_at(
        &self,
        source_ordinal: u128,
    ) -> Result<Option<RelationalClassificationSummaryProjectionRecord>, RelationalPublicationError>
    {
        if source_ordinal >= self.available_source_record_count() {
            return Ok(None);
        }
        if source_ordinal == 0 {
            return Ok(Some(
                RelationalClassificationSummaryProjectionRecord::Root {
                    contract: self.contract,
                    support_plan_root: self.support_plan_root,
                    classification_authority: self.closure.classification_authority,
                    exact_logical_case_count: self.closure.exact_logical_case_count,
                    exact_admitted_case_count: self.closure.exact_admitted_case_count,
                    exact_selected_case_count: self.closure.exact_selected_case_count,
                    case_id_authority: self
                        .authorization
                        .map(RelationalCaseIdPublicationAuthorization::authority),
                },
            ));
        }
        if source_ordinal <= Self::REGION_COUNT {
            let (outcome, exact_case_count) = match source_ordinal {
                1 => (
                    RelationalClassificationSummaryOutcome::Rejected,
                    self.rejected_case_count,
                ),
                2 => (
                    RelationalClassificationSummaryOutcome::AdmittedNotSelected,
                    self.admitted_not_selected_case_count,
                ),
                3 => (
                    RelationalClassificationSummaryOutcome::AdmittedSelected,
                    self.closure.exact_selected_case_count,
                ),
                _ => unreachable!("region ordinal is range checked"),
            };
            return Ok(Some(
                RelationalClassificationSummaryProjectionRecord::Region {
                    question_id: self.contract.question_id(),
                    region_ordinal: u8::try_from(source_ordinal - 1)
                        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?,
                    exact_case_count,
                    outcome,
                },
            ));
        }

        let case_ordinal = source_ordinal
            .checked_sub(1 + Self::REGION_COUNT)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
        let authorized_case_count = self.authorized_case_record_count();
        if case_ordinal < authorized_case_count {
            let authority = self
                .authorization
                .ok_or_else(|| {
                    RelationalPublicationError::CaseSupport(
                        "extensional case-support ordinal requires CaseId authority".into(),
                    )
                })?
                .authority();
            let case_index = usize::try_from(case_ordinal)
                .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
            let case_id = *self.selected_case_ids.get(case_index).ok_or_else(|| {
                RelationalPublicationError::CaseSupport(
                    "extensional selected-case index disagrees with exact closure".into(),
                )
            })?;
            return Ok(Some(
                RelationalClassificationSummaryProjectionRecord::AuthorizedCase {
                    question_id: self.contract.question_id(),
                    selected_region_ordinal: Self::SELECTED_REGION_ORDINAL,
                    case_id,
                    authority,
                },
            ));
        }
        if source_ordinal == self.closure.data_record_count {
            return Ok(Some(
                RelationalClassificationSummaryProjectionRecord::Closure(self.closure),
            ));
        }
        Err(RelationalPublicationError::CaseSupport(
            "extensional case-support ordinal index disagrees with exact closure".into(),
        ))
    }
}

struct PublicationOrdinalIndex<'journal> {
    case_support: Option<PublicationCaseSupportProjection<'journal>>,
    case_transitions: Option<RelationalCaseTransitionProjection>,
    mechanisms: BTreeMap<MechanismRequestId, MechanismDefinitionOrdinalIndex>,
}

#[derive(Default)]
struct MechanismDefinitionOrdinalIndex {
    descriptors: BTreeMap<MechanismSignatureId, MechanismDefinitionPublicationIndex>,
    payloads: BTreeMap<MechanismSignatureId, MechanismDefinitionPayloadIndex>,
}

struct MechanismDefinitionPublicationIndex {
    dag: RelationalMechanismSignatureDagIndex,
    chunk_count: u128,
}

struct MechanismDefinitionPayloadIndex {
    definition_digest: [u8; 32],
    definition_bytes: usize,
    chunk_count: u128,
}

impl<'journal> PublicationOrdinalIndex<'journal> {
    fn from_journal(
        journal: &'journal RelationalJournal,
        plan: &RelationalPublicationPlan,
    ) -> Result<Self, RelationalPublicationError> {
        let Some(authorization) = plan.artifacts.iter().find_map(|artifact| match artifact {
            PublicationArtifactPlan::CaseSupport { authorization } => Some(*authorization),
            PublicationArtifactPlan::Result { .. }
            | PublicationArtifactPlan::Mechanism { .. }
            | PublicationArtifactPlan::MechanismDefinitions { .. }
            | PublicationArtifactPlan::MechanismStructural { .. }
            | PublicationArtifactPlan::MechanismStructuralDefinitions { .. }
            | PublicationArtifactPlan::SubjectStarters { .. }
            | PublicationArtifactPlan::CaseTransitions { .. } => None,
        }) else {
            return Err(RelationalPublicationError::PlanIdentityMismatch);
        };
        let case_transitions = plan
            .artifacts
            .iter()
            .find_map(|artifact| match artifact {
                PublicationArtifactPlan::CaseTransitions {
                    authorization,
                    transition_schemas,
                } => Some((authorization.clone(), transition_schemas.clone())),
                PublicationArtifactPlan::Result { .. }
                | PublicationArtifactPlan::Mechanism { .. }
                | PublicationArtifactPlan::MechanismDefinitions { .. }
                | PublicationArtifactPlan::MechanismStructural { .. }
                | PublicationArtifactPlan::MechanismStructuralDefinitions { .. }
                | PublicationArtifactPlan::SubjectStarters { .. }
                | PublicationArtifactPlan::CaseSupport { .. } => None,
            })
            .map(|(authorization, transition_schemas)| {
                let scheduler = journal
                    .scheduler_view()
                    .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
                let selected_question = journal
                    .analysis_state()
                    .and_then(|analysis| analysis.selected_question());
                derive_relational_case_transition_projection(
                    scheduler,
                    transition_schemas,
                    authorization,
                    selected_question,
                )
                .map_err(|error| RelationalPublicationError::CaseTransitions(error.to_string()))
            })
            .transpose()?;
        Ok(Self {
            case_support: derive_case_support_for_publication(journal, authorization)?,
            case_transitions,
            mechanisms: BTreeMap::new(),
        })
    }

    fn mechanism_definition(
        &mut self,
        request_id: MechanismRequestId,
        definition: &MechanismSignatureDefinition,
        expected_scope: MechanismRequestScope,
    ) -> Result<&MechanismDefinitionPublicationIndex, RelationalPublicationError> {
        let mechanism = self.mechanisms.entry(request_id).or_default();
        if !mechanism.descriptors.contains_key(&definition.id()) {
            let dag =
                RelationalMechanismSignatureDagIndex::from_definition(definition, expected_scope)
                    .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
            let chunk_count =
                mechanism_definition_chunk_count(definition.canonical_definition().len())?;
            mechanism.descriptors.insert(
                definition.id(),
                MechanismDefinitionPublicationIndex { dag, chunk_count },
            );
        }
        mechanism
            .descriptors
            .get(&definition.id())
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)
    }

    fn mechanism_definition_payload(
        &mut self,
        request_id: MechanismRequestId,
        definition: &MechanismSignatureDefinition,
    ) -> Result<&MechanismDefinitionPayloadIndex, RelationalPublicationError> {
        let mechanism = self.mechanisms.entry(request_id).or_default();
        if !mechanism.payloads.contains_key(&definition.id()) {
            let definition_digest: [u8; 32] =
                Sha256::digest(definition.canonical_definition()).into();
            let derived_id = MechanismSignatureId::from_canonical_differential_signature_digest(
                request_id,
                definition_digest,
            );
            if definition.id().request_id() != request_id
                || definition_digest != definition.canonical_differential_digest()
                || derived_id != definition.id()
            {
                return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
            }
            let definition_bytes = definition.canonical_definition().len();
            let chunk_count = mechanism_definition_chunk_count(definition_bytes)?;
            mechanism.payloads.insert(
                definition.id(),
                MechanismDefinitionPayloadIndex {
                    definition_digest,
                    definition_bytes,
                    chunk_count,
                },
            );
        }
        let payload = mechanism
            .payloads
            .get(&definition.id())
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
        if payload.definition_digest != definition.canonical_differential_digest()
            || payload.definition_bytes != definition.canonical_definition().len()
        {
            return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
        }
        Ok(payload)
    }
}

fn derive_case_support_for_publication<'journal>(
    journal: &'journal RelationalJournal,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
) -> Result<Option<PublicationCaseSupportProjection<'journal>>, RelationalPublicationError> {
    let scheduler = journal
        .scheduler_view()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    let Some(partition) = scheduler.verified_case_chunk_partition() else {
        return derive_classification_summary_for_publication(journal, authorization).map(
            |projection| projection.map(PublicationCaseSupportProjection::ClassificationSummary),
        );
    };
    let classified_chunks = scheduler.classified_chunk_artifacts();
    let closure_ready = scheduler.support_catalog_is_sealed()
        && classified_chunks.len() == partition.artifact().chunks().len()
        && scheduler.selected_run_materializations_cover_classified_prefix();
    let closure_authority = if closure_ready {
        match (
            scheduler.certified_root_case_cardinality(),
            journal
                .analysis_state()
                .and_then(|analysis| analysis.selected_question()),
        ) {
            (Some(case_count), Some(selected_question))
                if matches!(
                    selected_question.authority(),
                    RelationalSelectedPopulationAuthority::CertifiedSupport { .. }
                ) =>
            {
                let support_evidence_root = scheduler
                    .support_evidence_root()
                    .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
                Some(
                    RelationalCaseSupportClosureAuthority::from_authenticated_certified_support(
                        true,
                        case_count,
                        support_evidence_root,
                        selected_question,
                    )
                    .map_err(|error| RelationalPublicationError::CaseSupport(error.to_string()))?,
                )
            }
            _ => None,
        }
    } else {
        None
    };
    derive_relational_case_support_projection(
        partition,
        classified_chunks,
        |cell_id| scheduler.selected_run_materialization(cell_id),
        authorization,
        closure_authority,
    )
    .map(PublicationCaseSupportProjection::Partitioned)
    .map(Some)
    .map_err(|error| RelationalPublicationError::CaseSupport(error.to_string()))
}

fn derive_classification_summary_for_publication<'journal>(
    journal: &'journal RelationalJournal,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
) -> Result<Option<RelationalClassificationSummaryProjection<'journal>>, RelationalPublicationError>
{
    let scheduler = journal
        .scheduler_view()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    let Some(selected_question) = journal
        .analysis_state()
        .and_then(|analysis| analysis.selected_question())
    else {
        return Ok(None);
    };
    selected_question
        .validate_identity()
        .map_err(|error| RelationalPublicationError::CaseSupport(error.to_string()))?;
    let selected_population_authority = match selected_question.authority() {
        RelationalSelectedPopulationAuthority::ExtensionalQuestion { content_root } => {
            RelationalPublishedSelectedPopulationAuthority::ExtensionalQuestion {
                question_content_root: content_root.bytes(),
            }
        }
        RelationalSelectedPopulationAuthority::CertifiedSupport {
            population_root, ..
        } => RelationalPublishedSelectedPopulationAuthority::CertifiedSupport {
            population_root: population_root.bytes(),
        },
    };
    let contract = scheduler.contract();
    if selected_question.question_id() != contract.question_id() {
        return Err(RelationalPublicationError::CaseSupport(
            "classification-summary selected-question seal names another question".into(),
        ));
    }
    let support_plan_root = scheduler.support_plan_root().ok_or_else(|| {
        RelationalPublicationError::CaseSupport(
            "classification-summary case-support closure has no support-plan root".into(),
        )
    })?;
    let selected_case_ids = scheduler.selected_discovery_suffix(0);
    let sealed_selected_case_count = selected_question.result_input_seal().coverage().row_count();
    let relation_closed = scheduler.relation_enumeration_is_complete();
    let observed_logical_case_count = scheduler.case_count() as u128;
    let exact_logical_case_count = match scheduler.certified_root_case_cardinality() {
        Some(certified) if relation_closed && observed_logical_case_count != certified => {
            return Err(RelationalPublicationError::CaseSupport(
                "closed relation size disagrees with its certified case cardinality".into(),
            ));
        }
        Some(certified) => certified,
        None if relation_closed => observed_logical_case_count,
        None => return Ok(None),
    };
    let classification_progress = scheduler
        .classification_progress_counts()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    let certified_classification = classification_progress.filter(|progress| {
        progress.is_complete()
            && progress.classified() == progress.candidates()
            && progress.candidates() == exact_logical_case_count
    });
    let extensional_admission_is_closed = relation_closed
        && scheduler.admission_decision_count() as u128 == observed_logical_case_count;
    let exact_admitted_case_count = match scheduler.certified_root_admission_decision() {
        Some(super::relation::AdmissionDecision::Admitted) => exact_logical_case_count,
        Some(super::relation::AdmissionDecision::Rejected) => 0,
        None if certified_classification.is_some() => certified_classification
            .expect("branch checked the certified classification")
            .admitted(),
        None if extensional_admission_is_closed => scheduler.admitted_count() as u128,
        None => return Ok(None),
    };
    let exact_selected_case_count = sealed_selected_case_count;
    let classification_authority = if certified_classification.is_some() {
        RelationalPublishedClassificationAuthority::CertifiedSupport
    } else if scheduler.concrete_base_is_classified() {
        RelationalPublishedClassificationAuthority::ExtensionalCatalog
    } else {
        RelationalPublishedClassificationAuthority::ComposedExactEvidence
    };
    let rejected_case_count = exact_logical_case_count
        .checked_sub(exact_admitted_case_count)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let admitted_not_selected_case_count = exact_admitted_case_count
        .checked_sub(exact_selected_case_count)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    if exact_admitted_case_count > exact_logical_case_count
        || exact_selected_case_count > exact_admitted_case_count
        || selected_case_ids.len() as u128 != exact_selected_case_count
        || sealed_selected_case_count != exact_selected_case_count
    {
        return Err(RelationalPublicationError::CaseSupport(
            "classification-summary case-support counts disagree with the selected-question seal"
                .into(),
        ));
    }
    let authorized_case_record_count = if authorization.is_some() {
        exact_selected_case_count
    } else {
        0
    };
    let data_record_count = 1_u128
        .checked_add(RelationalClassificationSummaryProjection::REGION_COUNT)
        .and_then(|count| count.checked_add(authorized_case_record_count))
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let support_evidence_root = scheduler
        .support_evidence_root()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    let closure = RelationalClassificationSummaryClosureMetadata {
        classification_authority,
        support_evidence_root: support_evidence_root.bytes(),
        selected_question_seal_id: selected_question.id(),
        selected_population_authority,
        exact_logical_case_count,
        exact_admitted_case_count,
        exact_selected_case_count,
        authorized_case_record_count,
        data_record_count,
    };
    Ok(Some(RelationalClassificationSummaryProjection {
        contract,
        support_plan_root: support_plan_root.bytes(),
        selected_case_ids,
        authorization,
        rejected_case_count,
        admitted_not_selected_case_count,
        closure,
    }))
}

impl MechanismDefinitionPayloadIndex {
    fn part_count(&self) -> Result<u128, RelationalPublicationError> {
        self.chunk_count
            .checked_add(2)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)
    }
}

#[derive(Default)]
struct AppendSummary {
    lines: u64,
    ordinals: u64,
}

/// Materialize one bounded suffix of every public artifact and atomically
/// refresh `manifest.json` at the authority's current durable checkpoint.
pub(crate) fn publish_relational_result_artifacts<A: RelationalPublicationAuthority>(
    output_directory: impl AsRef<Path>,
    authority: &A,
    plan: &RelationalPublicationPlan,
    report: &ExploreStreamSliceReport,
    limits: RelationalPublicationLimits,
) -> Result<RelationalPublicationSummary, RelationalPublicationError> {
    let output_directory = output_directory.as_ref();
    validate_output_directory(output_directory)?;
    validate_report(plan, report)?;

    let journal = authority
        .journal()
        .map_err(RelationalPublicationError::Authority)?;
    let current = authority
        .durable_checkpoint()
        .map_err(RelationalPublicationError::Authority)?;
    if journal.contract() != plan.contract
        || journal.next_sequence() != current.next_sequence
        || journal.head().bytes() != current.head
        || report.checkpoint.next_sequence != current.next_sequence
        || decode_hex_digest(&report.checkpoint.journal_head)? != current.head
    {
        return Err(RelationalPublicationError::CurrentCheckpointMismatch);
    }
    let mut ordinal_index = PublicationOrdinalIndex::from_journal(journal, plan)?;

    create_owner_only_directory(output_directory)
        .map_err(|error| io_error(output_directory, error))?;
    validate_publication_namespace(
        output_directory,
        plan,
        cursor_path_exists(output_directory),
        limits,
    )?;
    prepare_owner_only_publication_namespace(
        output_directory,
        plan.artifacts.iter().map(PublicationArtifactPlan::path),
    )?;
    let cursor_path = output_directory.join(CURSOR_FILE);
    let mut cursor = load_or_create_cursor(&cursor_path, output_directory, plan, current, limits)?;
    validate_cursor_plan(&cursor, plan)?;
    authenticate_cursor(authority, &cursor)?;
    let manifest_path = output_directory.join(MANIFEST_FILE);
    validate_existing_manifest(&manifest_path, plan, authority, limits)?;
    validate_source_cursors(plan, journal, &mut ordinal_index, &cursor)?;
    validate_committed_files(output_directory, plan, &cursor, limits)?;

    if cursor.pending.is_some() {
        recover_pending_batch(
            output_directory,
            authority,
            plan,
            journal,
            &mut ordinal_index,
            &mut cursor,
            limits,
        )?;
        write_cursor(&cursor_path, &cursor, limits)?;
    }

    let mut appended = AppendSummary::default();
    for artifact in plan.artifacts.iter() {
        let batch = append_artifact_batch(
            output_directory,
            &cursor_path,
            artifact,
            journal,
            &mut ordinal_index,
            current,
            &mut cursor,
            limits,
        )?;
        appended.lines = appended
            .lines
            .checked_add(batch.lines)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
        appended.ordinals = appended
            .ordinals
            .checked_add(batch.ordinals)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    }

    cursor.checkpoint = CursorCheckpoint::from_checkpoint(current);
    write_cursor(&cursor_path, &cursor, limits)?;
    let cursor_digest = digest_control_value(&cursor, limits)?;
    let manifest = build_manifest(
        plan,
        report,
        journal,
        &mut ordinal_index,
        &cursor,
        cursor_digest,
    )?;
    atomic_write_json(&manifest_path, &manifest, true, limits.max_control_bytes)?;

    let artifacts_caught_up = plan
        .artifacts
        .iter()
        .filter(|artifact| artifact_is_caught_up(artifact, journal, &mut ordinal_index, &cursor))
        .count();
    Ok(RelationalPublicationSummary {
        manifest_path,
        lines_appended: appended.lines,
        source_ordinals_advanced: appended.ordinals,
        artifacts_caught_up,
        artifact_count: plan.artifacts.len(),
    })
}

fn cursor_path_exists(output_directory: &Path) -> bool {
    output_directory.join(CURSOR_FILE).exists()
}

fn create_owner_only_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(OWNER_ONLY_DIRECTORY_MODE);
    builder.create(path)
}

#[cfg(unix)]
fn tighten_directory_permissions(path: &Path) -> Result<(), RelationalPublicationError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RelationalPublicationError::UnsafeOutputPath(
            path.to_path_buf(),
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(OWNER_ONLY_DIRECTORY_MODE))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn tighten_directory_permissions(_path: &Path) -> Result<(), RelationalPublicationError> {
    Ok(())
}

#[cfg(unix)]
fn tighten_file_permissions_if_present(path: &Path) -> Result<(), RelationalPublicationError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RelationalPublicationError::UnsafeOutputPath(
            path.to_path_buf(),
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(OWNER_ONLY_FILE_MODE))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn tighten_file_permissions_if_present(_path: &Path) -> Result<(), RelationalPublicationError> {
    Ok(())
}

fn prepare_owner_only_publication_namespace<'a>(
    output_directory: &Path,
    artifact_paths: impl IntoIterator<Item = &'a Path>,
) -> Result<(), RelationalPublicationError> {
    // Namespace validation has already established that these paths belong to
    // this publication. Tightening before reading the cursor closes legacy
    // group/world access on every authenticated resume.
    tighten_directory_permissions(output_directory)?;
    for name in ["views", "mechanisms", "starters", "graphs"] {
        let directory = output_directory.join(name);
        create_owner_only_directory(&directory).map_err(|error| io_error(&directory, error))?;
        tighten_directory_permissions(&directory)?;
    }
    for name in [CURSOR_FILE, MANIFEST_FILE] {
        tighten_file_permissions_if_present(&output_directory.join(name))?;
    }
    for relative in artifact_paths {
        tighten_file_permissions_if_present(&output_directory.join(relative))?;
    }
    for entry in
        fs::read_dir(output_directory).map_err(|error| io_error(output_directory, error))?
    {
        let entry = entry.map_err(|error| io_error(output_directory, error))?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(".futuruna-publication-tmp-"))
        {
            tighten_file_permissions_if_present(&entry.path())?;
        }
    }
    Ok(())
}

fn owner_only_open_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    options.mode(OWNER_ONLY_FILE_MODE);
    options
}

fn tighten_open_file_permissions(file: &File) -> std::io::Result<()> {
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(OWNER_ONLY_FILE_MODE))?;
    Ok(())
}

fn open_owner_only_append_file(path: &Path) -> std::io::Result<File> {
    let mut options = owner_only_open_options();
    let file = options.create(true).append(true).open(path)?;
    tighten_open_file_permissions(&file)?;
    Ok(file)
}

fn create_new_owner_only_file(path: &Path) -> std::io::Result<File> {
    let mut options = owner_only_open_options();
    let file = options.write(true).create_new(true).open(path)?;
    tighten_open_file_permissions(&file)?;
    Ok(file)
}

fn is_ignored_macos_metadata(entry: &fs::DirEntry, file_type: &fs::FileType) -> bool {
    entry.file_name() == MACOS_METADATA_FILE && file_type.is_file()
}

fn validate_publication_namespace(
    output_directory: &Path,
    plan: &RelationalPublicationPlan,
    owned: bool,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    if !owned {
        for entry in
            fs::read_dir(output_directory).map_err(|error| io_error(output_directory, error))?
        {
            let entry = entry.map_err(|error| io_error(output_directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_error(entry.path(), error))?;
            if is_ignored_macos_metadata(&entry, &file_type) {
                continue;
            }
            let is_abandoned_control_temporary = entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".futuruna-publication-tmp-"));
            if !is_abandoned_control_temporary
                || file_type.is_symlink()
                || !file_type.is_file()
                || entry
                    .metadata()
                    .map_err(|error| io_error(entry.path(), error))?
                    .len()
                    > limits.max_control_bytes as u64
            {
                return Err(RelationalPublicationError::UntrackedExistingPublication(
                    output_directory.to_path_buf(),
                ));
            }
        }
        return Ok(());
    }
    let allowed_files = plan
        .artifacts
        .iter()
        .map(|artifact| artifact.path().to_path_buf())
        .collect::<BTreeSet<_>>();
    for entry in
        fs::read_dir(output_directory).map_err(|error| io_error(output_directory, error))?
    {
        let entry = entry.map_err(|error| io_error(output_directory, error))?;
        let name = entry.file_name();
        let relative = PathBuf::from(&name);
        let metadata = entry
            .file_type()
            .map_err(|error| io_error(entry.path(), error))?;
        if is_ignored_macos_metadata(&entry, &metadata) {
            continue;
        }
        if metadata.is_symlink() {
            return Err(RelationalPublicationError::UnsafeOutputPath(entry.path()));
        }
        if name == CURSOR_FILE || name == MANIFEST_FILE {
            if !metadata.is_file() {
                return Err(RelationalPublicationError::UnsafeOutputPath(entry.path()));
            }
            continue;
        }
        if name
            .to_str()
            .is_some_and(|name| name.starts_with(".futuruna-publication-tmp-"))
        {
            if !metadata.is_file()
                || entry
                    .metadata()
                    .map_err(|error| io_error(entry.path(), error))?
                    .len()
                    > limits.max_control_bytes as u64
            {
                return Err(RelationalPublicationError::UnownedNamespaceEntry(
                    entry.path(),
                ));
            }
            // A process may die after syncing an atomic control-file
            // temporary but before rename. It has no authority and is ignored;
            // future atomics use create_new names and never consume it.
            continue;
        }
        if name == "views" || name == "mechanisms" || name == "starters" || name == "graphs" {
            if !metadata.is_dir() {
                return Err(RelationalPublicationError::UnsafeOutputPath(entry.path()));
            }
            for child in
                fs::read_dir(entry.path()).map_err(|error| io_error(entry.path(), error))?
            {
                let child = child.map_err(|error| io_error(entry.path(), error))?;
                let child_type = child
                    .file_type()
                    .map_err(|error| io_error(child.path(), error))?;
                let child_relative = relative.join(child.file_name());
                if is_ignored_macos_metadata(&child, &child_type) {
                    continue;
                }
                if child_type.is_symlink() || !child_type.is_file() {
                    return Err(RelationalPublicationError::UnsafeOutputPath(child.path()));
                }
                if !allowed_files.contains(&child_relative) {
                    return Err(RelationalPublicationError::UnownedNamespaceEntry(
                        child.path(),
                    ));
                }
            }
            continue;
        }
        return Err(RelationalPublicationError::UnownedNamespaceEntry(
            entry.path(),
        ));
    }
    Ok(())
}

fn validate_output_directory(path: &Path) -> Result<(), RelationalPublicationError> {
    if path.as_os_str().is_empty() {
        return Err(RelationalPublicationError::EmptyOutputDirectory);
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(RelationalPublicationError::UnsafeOutputPath(
                path.to_path_buf(),
            ));
        }
    }
    Ok(())
}

fn validate_report(
    plan: &RelationalPublicationPlan,
    report: &ExploreStreamSliceReport,
) -> Result<(), RelationalPublicationError> {
    if report.query_name != plan.query_name.as_ref()
        || report.identity.checked_program != plan.checked_program.as_ref()
        || report.identity.relation_id != hex(plan.contract.relation_id().bytes())
        || report.identity.admission_id != hex(plan.contract.admission_id().bytes())
        || report.identity.question_id != hex(plan.contract.question_id().bytes())
        || report.identity.analysis_graph_digest != hex(plan.contract.analysis_graph_digest())
        || report.identity.journal_id != hex(plan.journal_id)
    {
        return Err(RelationalPublicationError::ReportIdentityMismatch);
    }
    Ok(())
}

fn load_or_create_cursor(
    cursor_path: &Path,
    output_directory: &Path,
    plan: &RelationalPublicationPlan,
    current: RelationalPublicationCheckpoint,
    limits: RelationalPublicationLimits,
) -> Result<PublicationCursor, RelationalPublicationError> {
    if cursor_path.exists() {
        let mut cursor: PublicationCursor =
            read_control_json(cursor_path, limits.max_control_bytes)?;
        if cursor.schema_version != RELATIONAL_PUBLICATION_SCHEMA_VERSION {
            return Err(RelationalPublicationError::UnsupportedCursorVersion {
                actual: cursor.schema_version,
                expected: RELATIONAL_PUBLICATION_SCHEMA_VERSION,
            });
        }
        if cursor.journal_id != hex(plan.journal_id)
            || cursor.query_name != plan.query_name.as_ref()
        {
            return Err(RelationalPublicationError::CursorIdentityMismatch);
        }
        let missing = additive_artifact_keys(
            cursor.artifacts.keys().cloned(),
            plan.artifacts.iter().map(|artifact| {
                (
                    artifact.key().to_string(),
                    matches!(
                        artifact,
                        PublicationArtifactPlan::SubjectStarters { .. }
                            | PublicationArtifactPlan::CaseTransitions { .. }
                    ),
                )
            }),
        )?;
        for artifact in plan
            .artifacts
            .iter()
            .filter(|artifact| missing.contains(artifact.key()))
        {
            cursor.artifacts.insert(
                artifact.key().to_string(),
                initial_artifact_cursor(artifact)?,
            );
        }
        return Ok(cursor);
    }

    let manifest_path = output_directory.join(MANIFEST_FILE);
    if manifest_path.exists()
        || plan
            .artifacts
            .iter()
            .any(|artifact| output_directory.join(artifact.path()).exists())
    {
        return Err(RelationalPublicationError::UntrackedExistingPublication(
            output_directory.to_path_buf(),
        ));
    }

    let mut artifacts = BTreeMap::new();
    for artifact in plan.artifacts.iter() {
        if artifacts
            .insert(
                artifact.key().to_string(),
                initial_artifact_cursor(artifact)?,
            )
            .is_some()
        {
            return Err(RelationalPublicationError::CursorArtifactSetMismatch);
        }
    }
    let cursor = PublicationCursor {
        schema_version: RELATIONAL_PUBLICATION_SCHEMA_VERSION,
        journal_id: hex(plan.journal_id),
        query_name: plan.query_name.to_string(),
        checkpoint: CursorCheckpoint::from_checkpoint(current),
        artifacts,
        pending: None,
    };
    write_cursor(cursor_path, &cursor, limits)?;
    Ok(cursor)
}

fn additive_artifact_keys(
    stored_keys: impl IntoIterator<Item = String>,
    planned: impl IntoIterator<Item = (String, bool)>,
) -> Result<BTreeSet<String>, RelationalPublicationError> {
    let mut planned_by_key = BTreeMap::new();
    for (key, appendable) in planned {
        if planned_by_key.insert(key, appendable).is_some() {
            return Err(RelationalPublicationError::CursorArtifactSetMismatch);
        }
    }
    let stored_keys = stored_keys.into_iter().collect::<BTreeSet<_>>();
    if stored_keys
        .iter()
        .any(|stored| !planned_by_key.contains_key(stored))
    {
        return Err(RelationalPublicationError::CursorArtifactSetMismatch);
    }
    let mut missing = BTreeSet::new();
    for (key, appendable) in planned_by_key {
        if !stored_keys.contains(&key) {
            if !appendable {
                return Err(RelationalPublicationError::CursorArtifactSetMismatch);
            }
            missing.insert(key);
        }
    }
    Ok(missing)
}

fn initial_artifact_cursor(
    artifact: &PublicationArtifactPlan,
) -> Result<ArtifactCursor, RelationalPublicationError> {
    let key = artifact.key();
    Ok(ArtifactCursor {
        kind: artifact.kind().into(),
        path: path_to_manifest_string(artifact.path())?,
        source: match artifact {
            PublicationArtifactPlan::Result { .. }
            | PublicationArtifactPlan::MechanismStructural { .. }
            | PublicationArtifactPlan::CaseSupport { .. }
            | PublicationArtifactPlan::CaseTransitions { .. } => ArtifactSourceCursor::Flat {
                next_source_ordinal: 0,
            },
            PublicationArtifactPlan::Mechanism { .. } => ArtifactSourceCursor::MechanismDiscovery {
                event_ordinal: 0,
                closure_emitted: false,
            },
            PublicationArtifactPlan::MechanismDefinitions { .. } => {
                ArtifactSourceCursor::MechanismDefinitions {
                    signature_ordinal: 0,
                    definition_part_ordinal: 0,
                    closure_emitted: false,
                }
            }
            PublicationArtifactPlan::MechanismStructuralDefinitions { .. } => {
                ArtifactSourceCursor::StructuralDefinitions {
                    header_emitted: false,
                    definition_ordinal: 0,
                    definition_part_ordinal: 0,
                    closure_emitted: false,
                }
            }
            PublicationArtifactPlan::SubjectStarters {
                consumer_id,
                request_id,
                target,
                subject,
                ..
            } => ArtifactSourceCursor::SubjectStarters {
                identity: SubjectStarterCursorIdentity::new(
                    *consumer_id,
                    *request_id,
                    *target,
                    *subject,
                ),
                header_emitted: false,
                accumulator: None,
                closure_emitted: false,
            },
        },
        line_count: 0,
        byte_len: 0,
        prefix_digest: hex(publication_prefix_genesis(key)),
        last_line: None,
    })
}

fn validate_cursor_plan(
    cursor: &PublicationCursor,
    plan: &RelationalPublicationPlan,
) -> Result<(), RelationalPublicationError> {
    if cursor.schema_version != RELATIONAL_PUBLICATION_SCHEMA_VERSION {
        return Err(RelationalPublicationError::UnsupportedCursorVersion {
            actual: cursor.schema_version,
            expected: RELATIONAL_PUBLICATION_SCHEMA_VERSION,
        });
    }
    if cursor.journal_id != hex(plan.journal_id) || cursor.query_name != plan.query_name.as_ref() {
        return Err(RelationalPublicationError::CursorIdentityMismatch);
    }
    if cursor.artifacts.len() != plan.artifacts.len() {
        return Err(RelationalPublicationError::CursorArtifactSetMismatch);
    }
    for artifact in plan.artifacts.iter() {
        let stored = cursor
            .artifacts
            .get(artifact.key())
            .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
        if stored.kind != artifact.kind()
            || stored.path != path_to_manifest_string(artifact.path())?
            || decode_hex_digest(&stored.prefix_digest).is_err()
            || !source_cursor_matches_artifact(stored.source, artifact)
        {
            return Err(RelationalPublicationError::CursorArtifactMismatch(
                artifact.key().into(),
            ));
        }
    }
    if let Some(pending) = &cursor.pending {
        let stored = cursor
            .artifacts
            .get(&pending.artifact_key)
            .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
        let artifact = plan
            .artifacts
            .iter()
            .find(|artifact| artifact.key() == pending.artifact_key)
            .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
        if stored.source != pending.first_source
            || stored.line_count != pending.first_line_count
            || stored.byte_len != pending.first_byte_len
            || stored.prefix_digest != pending.first_prefix_digest
            || stored.last_line != pending.first_last_line
            || pending.max_line_bytes == 0
        {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
        if !pending_source_end_matches_artifact(&pending.source_end, artifact) {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
    }
    Ok(())
}

fn authenticate_cursor<A: RelationalPublicationAuthority>(
    authority: &A,
    cursor: &PublicationCursor,
) -> Result<(), RelationalPublicationError> {
    authenticate_checkpoint(authority, cursor.checkpoint.decode()?)?;
    if let Some(pending) = &cursor.pending {
        authenticate_checkpoint(authority, pending.checkpoint.decode()?)?;
    }
    Ok(())
}

fn authenticate_checkpoint<A: RelationalPublicationAuthority>(
    authority: &A,
    checkpoint: RelationalPublicationCheckpoint,
) -> Result<(), RelationalPublicationError> {
    if authority
        .authenticates_durable_prefix(checkpoint)
        .map_err(RelationalPublicationError::Authority)?
    {
        Ok(())
    } else {
        Err(RelationalPublicationError::PublicationFork {
            next_sequence: checkpoint.next_sequence,
            head: hex(checkpoint.head),
        })
    }
}

fn validate_committed_files(
    output_directory: &Path,
    plan: &RelationalPublicationPlan,
    cursor: &PublicationCursor,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    for artifact in plan.artifacts.iter() {
        let stored = cursor
            .artifacts
            .get(artifact.key())
            .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
        let path = output_directory.join(artifact.path());
        let pending = cursor
            .pending
            .as_ref()
            .is_some_and(|pending| pending.artifact_key == artifact.key());
        validate_artifact_file(&path, stored, pending, limits)?;
    }
    Ok(())
}

fn validate_source_cursors(
    plan: &RelationalPublicationPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
) -> Result<(), RelationalPublicationError> {
    for artifact in plan.artifacts.iter() {
        let stored = cursor
            .artifacts
            .get(artifact.key())
            .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
        match (artifact, stored.source) {
            (
                PublicationArtifactPlan::Result { .. }
                | PublicationArtifactPlan::MechanismStructural { .. }
                | PublicationArtifactPlan::CaseSupport { .. }
                | PublicationArtifactPlan::CaseTransitions { .. },
                ArtifactSourceCursor::Flat {
                    next_source_ordinal,
                },
            ) => {
                let available =
                    available_source_record_count(artifact, journal, ordinal_index)?.unwrap_or(0);
                if next_source_ordinal > available {
                    return Err(RelationalPublicationError::PublicationSourceAhead {
                        artifact: artifact.key().into(),
                        next_source_ordinal,
                        available,
                    });
                }
            }
            (
                PublicationArtifactPlan::Mechanism { .. },
                ArtifactSourceCursor::MechanismDiscovery { .. },
            ) => {
                let _ = record_at(
                    artifact,
                    journal,
                    ordinal_index,
                    cursor,
                    stored.source,
                    None,
                    None,
                )?;
            }
            (
                PublicationArtifactPlan::MechanismDefinitions { .. },
                ArtifactSourceCursor::MechanismDefinitions { .. },
            ) => {
                let source_end = pending_source_end(artifact, journal, ordinal_index, cursor)?;
                let _ = record_at(
                    artifact,
                    journal,
                    ordinal_index,
                    cursor,
                    stored.source,
                    Some(&source_end),
                    None,
                )?;
            }
            (
                PublicationArtifactPlan::MechanismStructuralDefinitions { .. },
                ArtifactSourceCursor::StructuralDefinitions { .. },
            ) => {
                let source_end = pending_source_end(artifact, journal, ordinal_index, cursor)?;
                let _ = record_at(
                    artifact,
                    journal,
                    ordinal_index,
                    cursor,
                    stored.source,
                    Some(&source_end),
                    None,
                )?;
            }
            (
                PublicationArtifactPlan::SubjectStarters { .. },
                ArtifactSourceCursor::SubjectStarters { .. },
            ) => {
                let source_end = pending_source_end(artifact, journal, ordinal_index, cursor)?;
                let _ = record_at(
                    artifact,
                    journal,
                    ordinal_index,
                    cursor,
                    stored.source,
                    Some(&source_end),
                    None,
                )?;
            }
            _ => {
                return Err(RelationalPublicationError::CursorArtifactMismatch(
                    artifact.key().into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_artifact_file(
    path: &Path,
    cursor: &ArtifactCursor,
    pending: bool,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && cursor.byte_len == 0 => {
            return Ok(());
        }
        Err(error) => return Err(io_error(path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RelationalPublicationError::UnsafeOutputPath(
            path.to_path_buf(),
        ));
    }
    let actual = metadata.len();
    if actual < cursor.byte_len {
        return Err(RelationalPublicationError::PublicationTruncated {
            path: path.to_path_buf(),
            expected_at_least: cursor.byte_len,
            actual,
        });
    }
    if !pending && actual != cursor.byte_len {
        return Err(RelationalPublicationError::PublicationAhead {
            path: path.to_path_buf(),
            committed: cursor.byte_len,
            actual,
        });
    }
    if pending
        && actual
            .checked_sub(cursor.byte_len)
            .is_none_or(|tail| tail > limits.max_recovery_tail_bytes as u64)
    {
        return Err(RelationalPublicationError::RecoveryTailTooLarge {
            path: path.to_path_buf(),
            bytes: actual.saturating_sub(cursor.byte_len),
            limit: limits.max_recovery_tail_bytes,
        });
    }
    validate_last_line(path, cursor, limits)
}

fn validate_last_line(
    path: &Path,
    cursor: &ArtifactCursor,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    match (&cursor.last_line, cursor.line_count, cursor.byte_len) {
        (None, 0, 0) => return Ok(()),
        (Some(last), lines, bytes) if lines > 0 => {
            let end = last
                .start
                .checked_add(last.bytes)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
            if end != bytes || last.bytes == 0 || last.bytes > limits.max_line_bytes as u64 {
                return Err(RelationalPublicationError::LastLineCursorMismatch(
                    path.to_path_buf(),
                ));
            }
            let length = usize::try_from(last.bytes)
                .map_err(|_| RelationalPublicationError::LastLineCursorMismatch(path.into()))?;
            let mut line = vec![0_u8; length];
            let mut file = File::open(path).map_err(|error| io_error(path, error))?;
            file.seek(SeekFrom::Start(last.start))
                .map_err(|error| io_error(path, error))?;
            file.read_exact(&mut line)
                .map_err(|error| io_error(path, error))?;
            if line.last() != Some(&b'\n') || hex(Sha256::digest(&line).into()) != last.digest {
                return Err(RelationalPublicationError::LastLineDigestMismatch(
                    path.to_path_buf(),
                ));
            }
            Ok(())
        }
        _ => Err(RelationalPublicationError::LastLineCursorMismatch(
            path.to_path_buf(),
        )),
    }
}

fn recover_pending_batch<A: RelationalPublicationAuthority>(
    output_directory: &Path,
    authority: &A,
    plan: &RelationalPublicationPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &mut PublicationCursor,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    let pending = cursor
        .pending
        .clone()
        .ok_or(RelationalPublicationError::PendingCursorMismatch)?;
    let checkpoint = pending.checkpoint.decode()?;
    let pending_max_line_bytes = usize::try_from(pending.max_line_bytes)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    if pending_max_line_bytes == 0 {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    let pending_line_budget = PublicationLineBudget {
        checkpoint,
        max_line_bytes: pending_max_line_bytes,
    };
    authenticate_checkpoint(authority, checkpoint)?;
    let artifact = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.key() == pending.artifact_key)
        .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
    if matches!(
        artifact,
        PublicationArtifactPlan::MechanismDefinitions { .. }
            | PublicationArtifactPlan::MechanismStructuralDefinitions { .. }
            | PublicationArtifactPlan::SubjectStarters { .. }
    ) && pending.source_end != pending_source_end(artifact, journal, ordinal_index, cursor)?
    {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    let path = output_directory.join(artifact.path());
    let mut working = cursor
        .artifacts
        .get(artifact.key())
        .cloned()
        .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
    if matches!(
        record_at(
            artifact,
            journal,
            ordinal_index,
            cursor,
            working.source,
            Some(&pending.source_end),
            Some(pending_line_budget),
        )?,
        AddressedPublicationRecord::NotReady | AddressedPublicationRecord::Exhausted
    ) {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }

    let actual_len = match fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && working.byte_len == 0 => 0,
        Err(error) => return Err(io_error(&path, error)),
    };
    let tail_len = actual_len.checked_sub(working.byte_len).ok_or(
        RelationalPublicationError::PublicationTruncated {
            path: path.clone(),
            expected_at_least: working.byte_len,
            actual: actual_len,
        },
    )?;
    if tail_len > limits.max_recovery_tail_bytes as u64 {
        return Err(RelationalPublicationError::RecoveryTailTooLarge {
            path,
            bytes: tail_len,
            limit: limits.max_recovery_tail_bytes,
        });
    }

    let mut tail = vec![
        0_u8;
        usize::try_from(tail_len).map_err(|_| {
            RelationalPublicationError::RecoveryTailTooLarge {
                path: path.clone(),
                bytes: tail_len,
                limit: limits.max_recovery_tail_bytes,
            }
        })?
    ];
    if !tail.is_empty() {
        let mut file = File::open(&path).map_err(|error| io_error(&path, error))?;
        file.seek(SeekFrom::Start(working.byte_len))
            .map_err(|error| io_error(&path, error))?;
        file.read_exact(&mut tail)
            .map_err(|error| io_error(&path, error))?;
    }

    let complete_len = tail
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let mut tail_cursor = 0_usize;
    while tail_cursor < complete_len {
        let line_end = tail[tail_cursor..complete_len]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| tail_cursor + offset + 1)
            .ok_or(RelationalPublicationError::PendingCursorMismatch)?;
        let actual_line = &tail[tail_cursor..line_end];
        let (coordinate, next, expected_line) = next_emitted_line(
            artifact,
            journal,
            ordinal_index,
            cursor,
            working.source,
            Some(&pending.source_end),
            checkpoint,
            RelationalPublicationLimits {
                max_line_bytes: pending_max_line_bytes,
                ..limits
            },
        )?
        .ok_or_else(|| RelationalPublicationError::PublicationAhead {
            path: path.clone(),
            committed: working.byte_len,
            actual: actual_len,
        })?;
        if actual_line != expected_line.as_slice() {
            return Err(RelationalPublicationError::PublicationContradiction {
                path: path.clone(),
                source_coordinate: coordinate.describe(),
            });
        }
        working.source = next;
        accept_line(&mut working, actual_line)?;
        tail_cursor = line_end;
    }

    let partial = &tail[complete_len..];
    if !partial.is_empty() {
        let (coordinate, _, expected_line) = next_emitted_line(
            artifact,
            journal,
            ordinal_index,
            cursor,
            working.source,
            Some(&pending.source_end),
            checkpoint,
            RelationalPublicationLimits {
                max_line_bytes: pending_max_line_bytes,
                ..limits
            },
        )?
        .ok_or_else(|| RelationalPublicationError::PublicationAhead {
            path: path.clone(),
            committed: working.byte_len,
            actual: actual_len,
        })?;
        if !expected_line.starts_with(partial) {
            return Err(RelationalPublicationError::PublicationContradiction {
                path: path.clone(),
                source_coordinate: coordinate.describe(),
            });
        }
        let file = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| io_error(&path, error))?;
        file.set_len(working.byte_len)
            .map_err(|error| io_error(&path, error))?;
        file.sync_all().map_err(|error| io_error(&path, error))?;
    }

    cursor.artifacts.insert(artifact.key().to_string(), working);
    cursor.checkpoint = pending.checkpoint;
    cursor.pending = None;
    Ok(())
}

fn append_artifact_batch(
    output_directory: &Path,
    cursor_path: &Path,
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    current: RelationalPublicationCheckpoint,
    cursor: &mut PublicationCursor,
    limits: RelationalPublicationLimits,
) -> Result<AppendSummary, RelationalPublicationError> {
    let initial = cursor
        .artifacts
        .get(artifact.key())
        .cloned()
        .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
    // A short observable slice may yield before a durable result projection
    // has registered its source. That is ordinary NotReady state, not a torn
    // pending batch. Do not freeze an end cursor until the flat source has an
    // authenticated frontier to freeze.
    if matches!(initial.source, ArtifactSourceCursor::Flat { .. })
        && available_source_record_count(artifact, journal, ordinal_index)?.is_none()
    {
        return Ok(AppendSummary::default());
    }
    let source_end = pending_source_end(artifact, journal, ordinal_index, cursor)?;
    let line_budget = PublicationLineBudget {
        checkpoint: current,
        max_line_bytes: limits.max_line_bytes,
    };
    let first = record_at(
        artifact,
        journal,
        ordinal_index,
        cursor,
        initial.source,
        Some(&source_end),
        Some(line_budget),
    )?;
    if matches!(
        &first,
        AddressedPublicationRecord::NotReady | AddressedPublicationRecord::Exhausted
    ) {
        return Ok(AppendSummary::default());
    }
    let pending = PendingArtifactBatch {
        checkpoint: CursorCheckpoint::from_checkpoint(current),
        artifact_key: artifact.key().into(),
        first_source: initial.source,
        source_end,
        first_line_count: initial.line_count,
        first_byte_len: initial.byte_len,
        first_prefix_digest: initial.prefix_digest.clone(),
        first_last_line: initial.last_line.clone(),
        max_line_bytes: u64::try_from(limits.max_line_bytes)
            .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?,
    };
    cursor.pending = Some(pending);
    write_cursor(cursor_path, cursor, limits)?;

    let path = output_directory.join(artifact.path());
    ensure_safe_artifact_target(&path)?;
    let mut file = open_owner_only_append_file(&path).map_err(|error| io_error(&path, error))?;
    let mut working = initial;
    let opened_len = file
        .metadata()
        .map_err(|error| io_error(&path, error))?
        .len();
    if opened_len != working.byte_len {
        return Err(RelationalPublicationError::PublicationAhead {
            path,
            committed: working.byte_len,
            actual: opened_len,
        });
    }
    let mut summary = AppendSummary::default();
    let mut batch_bytes = 0_usize;
    let mut actions = 0_usize;
    let mut prefetched = Some(first);
    while actions < limits.max_records_per_artifact.get() {
        let record = match prefetched.take() {
            Some(record) => record,
            None => record_at(
                artifact,
                journal,
                ordinal_index,
                cursor,
                working.source,
                cursor.pending.as_ref().map(|pending| &pending.source_end),
                Some(line_budget),
            )?,
        };
        match record {
            AddressedPublicationRecord::Emit {
                coordinate,
                next,
                value,
            } => {
                let line = encode_publication_line(artifact, coordinate, current, value, limits)?;
                let next_batch_bytes = batch_bytes
                    .checked_add(line.len())
                    .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
                if next_batch_bytes > limits.max_batch_bytes && actions > 0 {
                    break;
                }
                file.write_all(&line)
                    .map_err(|error| io_error(&path, error))?;
                accept_line(&mut working, &line)?;
                batch_bytes = next_batch_bytes;
                summary.lines = summary
                    .lines
                    .checked_add(1)
                    .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
                working.source = next;
            }
            AddressedPublicationRecord::Skip { next } => working.source = next,
            AddressedPublicationRecord::NotReady | AddressedPublicationRecord::Exhausted => break,
        }
        summary.ordinals = summary
            .ordinals
            .checked_add(1)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
        actions += 1;
    }
    file.sync_all().map_err(|error| io_error(&path, error))?;

    cursor.artifacts.insert(artifact.key().to_string(), working);
    cursor.checkpoint = CursorCheckpoint::from_checkpoint(current);
    cursor.pending = None;
    write_cursor(cursor_path, cursor, limits)?;
    Ok(summary)
}

fn next_emitted_line(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
    mut source: ArtifactSourceCursor,
    source_end: Option<&PendingArtifactSourceEnd>,
    checkpoint: RelationalPublicationCheckpoint,
    limits: RelationalPublicationLimits,
) -> Result<
    Option<(PublicationSourceCoordinate, ArtifactSourceCursor, Vec<u8>)>,
    RelationalPublicationError,
> {
    let mut skipped = 0_usize;
    while skipped < limits.max_records_per_artifact.get() {
        match record_at(
            artifact,
            journal,
            ordinal_index,
            cursor,
            source,
            source_end,
            Some(PublicationLineBudget {
                checkpoint,
                max_line_bytes: limits.max_line_bytes,
            }),
        )? {
            AddressedPublicationRecord::Emit {
                coordinate,
                next,
                value,
            } => {
                return Ok(Some((
                    coordinate,
                    next,
                    encode_publication_line(artifact, coordinate, checkpoint, value, limits)?,
                )));
            }
            AddressedPublicationRecord::Skip { next } => {
                source = next;
                skipped += 1;
            }
            AddressedPublicationRecord::NotReady | AddressedPublicationRecord::Exhausted => {
                return Ok(None);
            }
        }
    }
    Err(RelationalPublicationError::RecoverySkipLimit {
        artifact: artifact.key().into(),
        limit: limits.max_records_per_artifact.get(),
    })
}

fn accept_line(cursor: &mut ArtifactCursor, line: &[u8]) -> Result<(), RelationalPublicationError> {
    let line_len =
        u64::try_from(line.len()).map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let start = cursor.byte_len;
    cursor.byte_len = cursor
        .byte_len
        .checked_add(line_len)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    cursor.line_count = cursor
        .line_count
        .checked_add(1)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let line_digest: [u8; 32] = Sha256::digest(line).into();
    let prior = decode_hex_digest(&cursor.prefix_digest)?;
    cursor.prefix_digest = hex(extend_publication_prefix(prior, line_digest));
    cursor.last_line = Some(LastLineCursor {
        start,
        bytes: line_len,
        digest: hex(line_digest),
    });
    Ok(())
}

fn record_at(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
    source: ArtifactSourceCursor,
    source_end: Option<&PendingArtifactSourceEnd>,
    line_budget: Option<PublicationLineBudget>,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    if let ArtifactSourceCursor::Flat {
        next_source_ordinal,
    } = source
    {
        match source_end {
            Some(PendingArtifactSourceEnd::Flat { source_end }) => {
                let live_end = available_source_record_count(artifact, journal, ordinal_index)?
                    .ok_or(RelationalPublicationError::PendingCursorMismatch)?;
                if live_end < *source_end || next_source_ordinal > *source_end {
                    return Err(RelationalPublicationError::PendingCursorMismatch);
                }
                if next_source_ordinal == *source_end {
                    return Ok(AddressedPublicationRecord::Exhausted);
                }
            }
            Some(
                PendingArtifactSourceEnd::MechanismDiscovery { .. }
                | PendingArtifactSourceEnd::MechanismDefinitions { .. }
                | PendingArtifactSourceEnd::StructuralDefinitions { .. }
                | PendingArtifactSourceEnd::SubjectStarters { .. },
            ) => return Err(RelationalPublicationError::PendingCursorMismatch),
            None => {}
        }
    }
    match (artifact, source) {
        (
            PublicationArtifactPlan::Result {
                view_id,
                select_names,
                source: ResultPublicationSource::EarlyEachCase,
                ..
            },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => address_flat_record(
            artifact,
            next_source_ordinal,
            early_each_case_record(journal, *view_id, select_names, next_source_ordinal)?,
        ),
        (
            PublicationArtifactPlan::Result {
                view_id,
                select_names,
                source: ResultPublicationSource::DurableProjection,
                ..
            },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => address_flat_record(
            artifact,
            next_source_ordinal,
            durable_projection_record(journal, *view_id, select_names, next_source_ordinal)?,
        ),
        (
            PublicationArtifactPlan::MechanismStructural { request_id, .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => address_flat_record(
            artifact,
            next_source_ordinal,
            structural_sidecar_record(artifact, journal, *request_id, next_source_ordinal)?,
        ),
        (
            PublicationArtifactPlan::CaseSupport { .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => address_flat_record(
            artifact,
            next_source_ordinal,
            case_support_record(
                artifact,
                ordinal_index.case_support.as_ref(),
                next_source_ordinal,
            )?,
        ),
        (
            PublicationArtifactPlan::CaseTransitions { .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => address_flat_record(
            artifact,
            next_source_ordinal,
            case_transition_record(
                artifact,
                journal,
                ordinal_index.case_transitions.as_ref(),
                next_source_ordinal,
            )?,
        ),
        (
            PublicationArtifactPlan::Mechanism { request_id, .. },
            ArtifactSourceCursor::MechanismDiscovery {
                event_ordinal,
                closure_emitted,
            },
        ) => mechanism_discovery_record(
            artifact,
            journal,
            ordinal_index,
            *request_id,
            event_ordinal,
            closure_emitted,
            source_end,
        ),
        (
            PublicationArtifactPlan::MechanismDefinitions { request_id, .. },
            ArtifactSourceCursor::MechanismDefinitions {
                signature_ordinal,
                definition_part_ordinal,
                closure_emitted,
            },
        ) => mechanism_definition_record(
            artifact,
            journal,
            ordinal_index,
            *request_id,
            signature_ordinal,
            definition_part_ordinal,
            closure_emitted,
            cursor,
            source_end,
        ),
        (
            PublicationArtifactPlan::MechanismStructuralDefinitions { request_id, .. },
            ArtifactSourceCursor::StructuralDefinitions {
                header_emitted,
                definition_ordinal,
                definition_part_ordinal,
                closure_emitted,
            },
        ) => structural_definition_record(
            artifact,
            journal,
            *request_id,
            header_emitted,
            definition_ordinal,
            definition_part_ordinal,
            closure_emitted,
            source_end,
        ),
        (
            PublicationArtifactPlan::SubjectStarters {
                consumer_id,
                request_id,
                target,
                subject,
                authorization,
                transition_schemas,
                structural_artifact_key,
                structural_artifact_path,
                ..
            },
            ArtifactSourceCursor::SubjectStarters {
                identity,
                header_emitted,
                accumulator,
                closure_emitted,
            },
        ) => subject_starter_record(
            artifact,
            journal,
            *consumer_id,
            *request_id,
            *target,
            *subject,
            authorization,
            transition_schemas,
            structural_artifact_key,
            structural_artifact_path,
            identity,
            header_emitted,
            accumulator,
            closure_emitted,
            source_end,
            line_budget,
        ),
        _ => Err(RelationalPublicationError::CursorArtifactMismatch(
            artifact.key().into(),
        )),
    }
}

fn address_flat_record(
    artifact: &PublicationArtifactPlan,
    source_ordinal: u128,
    record: PublicationRecord,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    Ok(match record {
        PublicationRecord::Emit(value) => AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::Flat { source_ordinal },
            next: next_flat_source(artifact, source_ordinal)?,
            value,
        },
        PublicationRecord::Skip => AddressedPublicationRecord::Skip {
            next: next_flat_source(artifact, source_ordinal)?,
        },
        PublicationRecord::NotReady => AddressedPublicationRecord::NotReady,
        PublicationRecord::Exhausted => AddressedPublicationRecord::Exhausted,
    })
}

fn next_flat_source(
    artifact: &PublicationArtifactPlan,
    source_ordinal: u128,
) -> Result<ArtifactSourceCursor, RelationalPublicationError> {
    Ok(ArtifactSourceCursor::Flat {
        next_source_ordinal: source_ordinal.checked_add(1).ok_or_else(|| {
            RelationalPublicationError::SourceOrdinalOverflow {
                artifact: artifact.key().into(),
                ordinal: source_ordinal,
            }
        })?,
    })
}

struct StructuralSidecarAuthority<'journal> {
    structural: Option<&'journal StructuralMechanismCatalogBuilder>,
    structural_closure: Option<StructuralQuotientClosureReceipt>,
    support: Option<(
        &'journal MechanismSupportCatalogBuilder,
        MechanismSupportClosureReceipt,
    )>,
    assignment_count: u128,
    support_subject_count: u128,
}

impl StructuralSidecarAuthority<'_> {
    fn available_source_record_count(&self) -> Result<u128, RelationalPublicationError> {
        let Some(_) = self.structural else {
            return Ok(0);
        };
        let Some(_) = self.structural_closure else {
            return Ok(self.assignment_count);
        };
        let structural_end = self
            .assignment_count
            .checked_add(1)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
        if self.support.is_none() {
            return Ok(structural_end);
        }
        structural_end
            .checked_add(self.support_subject_count)
            .and_then(|count| count.checked_add(1))
            .ok_or(RelationalPublicationError::ArithmeticOverflow)
    }
}

fn structural_sidecar_authority(
    journal: &RelationalJournal,
    request_id: MechanismRequestId,
) -> Result<StructuralSidecarAuthority<'_>, RelationalPublicationError> {
    let Some(analysis) = journal.analysis_state() else {
        return Ok(StructuralSidecarAuthority {
            structural: None,
            structural_closure: None,
            support: None,
            assignment_count: 0,
            support_subject_count: 0,
        });
    };
    let Some(structural) = analysis.structural_mechanism_catalog(request_id) else {
        if analysis.structural_quotient_closure(request_id).is_some()
            || analysis.mechanism_support_closure(request_id).is_some()
            || analysis
                .mechanism_support_catalog(request_id)
                .is_some_and(|catalog| catalog.closure().is_some())
        {
            return Err(RelationalPublicationError::MissingAnalysisLayer);
        }
        return Ok(StructuralSidecarAuthority {
            structural: None,
            structural_closure: None,
            support: None,
            assignment_count: 0,
            support_subject_count: 0,
        });
    };
    if structural.request_id() != request_id
        || structural.assignment_count() != structural.assignment_discovery_count()
    {
        return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
    }
    let assignment_count = structural.assignment_count() as u128;
    let mechanism_count = structural.structural_mechanism_count() as u128;
    let node_count = structural.canonical_node_ids().len() as u128;
    let edge_count = structural.canonical_edge_ids().len() as u128;
    let support_subject_count = node_count
        .checked_mul(2)
        .and_then(|count| {
            edge_count
                .checked_mul(2)
                .and_then(|edges| count.checked_add(edges))
        })
        .and_then(|count| count.checked_add(mechanism_count))
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let structural_closure = analysis.structural_quotient_closure(request_id);
    if structural.closure() != structural_closure {
        return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
    }
    if let Some(closure) = structural_closure {
        let counts = closure.counts();
        let ordinal_counts = structural
            .canonical_subject_ordinal_counts()
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
        let raw_closure = analysis
            .mechanism_closure(request_id)
            .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
        if closure.request_id() != request_id
            || closure.expected_signature_count() != assignment_count
            || counts.assignments() != assignment_count
            || counts.mechanisms() != mechanism_count
            || counts.nodes() != node_count
            || counts.edges() != edge_count
            || ordinal_counts
                != (
                    structural.structural_mechanism_count(),
                    structural.canonical_node_ids().len(),
                    structural.canonical_edge_ids().len(),
                )
            || counts.execution_profiles() != structural.execution_profile_count() as u128
            || raw_closure.request_id() != request_id
            || raw_closure.counts().distinct_signatures()
                != MechanismCountEvidence::Exact(closure.expected_signature_count())
        {
            return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
        }
    }

    let support_catalog = analysis.mechanism_support_catalog(request_id);
    let support_closure = analysis.mechanism_support_closure(request_id);
    let support = match (support_catalog, support_closure) {
        (None, None) => None,
        (Some(catalog), None) if catalog.closure().is_none() => None,
        (Some(catalog), Some(closure)) if catalog.closure() == Some(closure) => {
            let structural_closure =
                structural_closure.ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            let raw_closure = analysis
                .mechanism_closure(request_id)
                .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            if catalog.scope().request_id() != request_id
                || catalog.scope().target() != closure.target()
                || closure.request_id() != request_id
                || closure.structural_root() != structural_closure.root()
                || closure.incidence_root() != raw_closure.incidence_root()
                || closure
                    .successful_case_count()
                    .checked_add(closure.unavailable_case_count())
                    != Some(closure.target_case_count())
            {
                return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
            }
            Some((catalog, closure))
        }
        _ => return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch),
    };

    Ok(StructuralSidecarAuthority {
        structural: Some(structural),
        structural_closure,
        support,
        assignment_count,
        support_subject_count,
    })
}

#[derive(Clone, Copy)]
struct StructuralDefinitionCatalogAuthority<'journal> {
    catalog: &'journal StructuralMechanismCatalogBuilder,
    closure: StructuralQuotientClosureReceipt,
    definition_catalog_root: StructuralDefinitionCatalogRoot,
    definition_count: u128,
}

/// Resolve only the closure-frozen normalized definition catalog. This lane is
/// deliberately independent of mechanism-support closure and never consults
/// raw signature payloads or per-signature membership.
fn structural_definition_catalog_authority(
    journal: &RelationalJournal,
    request_id: MechanismRequestId,
) -> Result<Option<StructuralDefinitionCatalogAuthority<'_>>, RelationalPublicationError> {
    let Some(analysis) = journal.analysis_state() else {
        return Ok(None);
    };
    let catalog = analysis.structural_mechanism_catalog(request_id);
    let closure = analysis.structural_quotient_closure(request_id);
    match (catalog, closure) {
        (None, None) => Ok(None),
        (Some(catalog), None)
            if catalog.closure().is_none() && catalog.definition_catalog_root().is_none() =>
        {
            Ok(None)
        }
        (Some(catalog), Some(closure)) => {
            let definition_catalog_root = catalog
                .definition_catalog_root()
                .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
            let definition_count = catalog
                .canonical_definition_count()
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
            let counts = closure.counts();
            let expected_definition_count = counts
                .frames()
                .checked_add(counts.activation_contexts())
                .and_then(|count| count.checked_add(counts.nodes()))
                .and_then(|count| count.checked_add(counts.edges()))
                .and_then(|count| count.checked_add(counts.mechanisms()))
                .and_then(|count| count.checked_add(counts.execution_profiles()))
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
            if catalog.request_id() != request_id
                || closure.request_id() != request_id
                || catalog.closure() != Some(closure)
                || definition_count != expected_definition_count
            {
                return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
            }
            Ok(Some(StructuralDefinitionCatalogAuthority {
                catalog,
                closure,
                definition_catalog_root,
                definition_count,
            }))
        }
        _ => Err(RelationalPublicationError::MissingAnalysisLayer),
    }
}

/// Resolve the closure-frozen structural subject coordinate in O(1). Facets
/// are an ordinal role over structural node/edge identity and therefore do not
/// require another catalog-side vector.
fn structural_support_subject_at(
    structural: &StructuralMechanismCatalogBuilder,
    ordinal: u128,
) -> Result<Option<MechanismSupportSubject>, RelationalPublicationError> {
    let (mechanisms, nodes, edges) = structural
        .canonical_subject_ordinal_counts()
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    let mechanism_end = mechanisms as u128;
    if ordinal < mechanism_end {
        let index =
            usize::try_from(ordinal).map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
        return Ok(structural
            .canonical_mechanism_id_at(index)
            .map(MechanismSupportSubject::Mechanism));
    }
    let node_ordinal = ordinal
        .checked_sub(mechanism_end)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let node_end = (nodes as u128)
        .checked_mul(2)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    if node_ordinal < node_end {
        let index = usize::try_from(node_ordinal / 2)
            .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
        let facet = if node_ordinal % 2 == 0 {
            MechanismSupportFacet::Activation
        } else {
            MechanismSupportFacet::DifferentialParticipation
        };
        return Ok(structural
            .canonical_node_id_at(index)
            .map(|node_id| MechanismSupportSubject::Node { facet, node_id }));
    }
    let edge_ordinal = node_ordinal
        .checked_sub(node_end)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let edge_end = (edges as u128)
        .checked_mul(2)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    if edge_ordinal < edge_end {
        let index = usize::try_from(edge_ordinal / 2)
            .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
        let facet = if edge_ordinal % 2 == 0 {
            MechanismSupportFacet::Activation
        } else {
            MechanismSupportFacet::DifferentialParticipation
        };
        return Ok(structural
            .canonical_edge_id_at(index)
            .map(|edge_id| MechanismSupportSubject::Edge { facet, edge_id }));
    }
    Ok(None)
}

fn structural_sidecar_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    request_id: MechanismRequestId,
    source_ordinal: u128,
) -> Result<PublicationRecord, RelationalPublicationError> {
    let PublicationArtifactPlan::MechanismStructural {
        definitions_artifact_key,
        definitions_artifact_path,
        ..
    } = artifact
    else {
        return Err(RelationalPublicationError::CursorArtifactMismatch(
            artifact.key().into(),
        ));
    };
    let authority = structural_sidecar_authority(journal, request_id)?;
    let available = authority.available_source_record_count()?;
    if source_ordinal > available {
        return Err(RelationalPublicationError::PublicationSourceAhead {
            artifact: artifact.key().into(),
            next_source_ordinal: source_ordinal,
            available,
        });
    }
    if source_ordinal == available {
        return if authority.support.is_some() {
            Ok(PublicationRecord::Exhausted)
        } else {
            Ok(PublicationRecord::NotReady)
        };
    }
    let structural = authority
        .structural
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    if source_ordinal < authority.assignment_count {
        let assignment_index = usize::try_from(source_ordinal)
            .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
        let assignment = structural
            .assignment_discovery_at(assignment_index)
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
        return Ok(PublicationRecord::Emit(public_structural_assignment(
            request_id,
            source_ordinal,
            assignment,
            structural,
        )?));
    }

    let structural_closure = authority
        .structural_closure
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let definition_catalog_root = structural
        .definition_catalog_root()
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    if source_ordinal == authority.assignment_count {
        return Ok(PublicationRecord::Emit(public_structural_closure(
            structural_closure,
            definition_catalog_root,
            definitions_artifact_key,
            definitions_artifact_path,
        )));
    }
    let support_ordinal = source_ordinal
        .checked_sub(authority.assignment_count)
        .and_then(|ordinal| ordinal.checked_sub(1))
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let (support, support_closure) = authority
        .support
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    if support_ordinal < authority.support_subject_count {
        let subject = structural_support_subject_at(structural, support_ordinal)?
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
        return Ok(PublicationRecord::Emit(public_structural_subject_support(
            request_id,
            support_ordinal,
            subject,
            structural,
            structural_closure,
            definition_catalog_root,
            definitions_artifact_key,
            definitions_artifact_path,
            support,
            support_closure,
        )?));
    }
    if support_ordinal == authority.support_subject_count {
        return Ok(PublicationRecord::Emit(public_mechanism_support_closure(
            support_closure,
            authority.support_subject_count,
            definition_catalog_root,
            definitions_artifact_key,
            definitions_artifact_path,
        )));
    }
    Err(RelationalPublicationError::PublicationSourceAhead {
        artifact: artifact.key().into(),
        next_source_ordinal: source_ordinal,
        available,
    })
}

fn public_structural_assignment(
    request_id: MechanismRequestId,
    assignment_ordinal: u128,
    assignment: &StructuralSignatureAssignment,
    structural: &StructuralMechanismCatalogBuilder,
) -> Result<JsonValue, RelationalPublicationError> {
    let prefix_len = usize::try_from(
        assignment_ordinal
            .checked_add(1)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
    )
    .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let prefix_revision = structural
        .assignment_discovery_prefix_revision(prefix_len)
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    Ok(json!({
        "kind": "structural_assignment",
        "request_id": hex(request_id.bytes()),
        "assignment_ordinal": assignment_ordinal.to_string(),
        "raw_signature_id": hex(assignment.signature_id().bytes()),
        "structural_mechanism_id": hex(assignment.mechanism_id().bytes()),
        "execution_profile_id": hex(assignment.profile_id().bytes()),
        "membership_root": hex(assignment.membership_root().bytes()),
        "membership_counts": {
            "nodes": assignment.node_membership().len().to_string(),
            "edges": assignment.edge_membership().len().to_string(),
            "differential_nodes": assignment.differential_node_membership().len().to_string(),
            "differential_edges": assignment.differential_edge_membership().len().to_string(),
        },
        "discovery_prefix_revision": hex(prefix_revision.bytes()),
    }))
}

fn public_structural_closure(
    closure: StructuralQuotientClosureReceipt,
    definition_catalog_root: StructuralDefinitionCatalogRoot,
    definitions_artifact_key: &str,
    definitions_artifact_path: &str,
) -> JsonValue {
    let counts = closure.counts();
    json!({
        "kind": "structural_quotient_closure",
        "closure_version": closure.closure_version(),
        "quotient_version": closure.quotient_version(),
        "request_id": hex(closure.request_id().bytes()),
        "expected_raw_signature_count": closure.expected_signature_count().to_string(),
        "expected_raw_signature_set_root": hex(closure.expected_signature_set_root().bytes()),
        "signature_to_quotient_root": hex(closure.signature_to_quotient_root().bytes()),
        "catalog_membership_root": hex(closure.catalog_membership_root().bytes()),
        "counts": {
            "structural_assignments": counts.assignments().to_string(),
            "frames": counts.frames().to_string(),
            "activation_contexts": counts.activation_contexts().to_string(),
            "nodes": counts.nodes().to_string(),
            "edges": counts.edges().to_string(),
            "structural_mechanisms": counts.mechanisms().to_string(),
            "execution_profiles": counts.execution_profiles().to_string(),
        },
        "structural_quotient_root": hex(closure.root().bytes()),
        "structural_definition_catalog_root": hex(definition_catalog_root.bytes()),
        "structural_definitions": public_structural_definition_artifact_reference(
            definitions_artifact_key,
            definitions_artifact_path,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn public_structural_subject_support(
    request_id: MechanismRequestId,
    support_ordinal: u128,
    subject: MechanismSupportSubject,
    structural: &StructuralMechanismCatalogBuilder,
    structural_closure: StructuralQuotientClosureReceipt,
    definition_catalog_root: StructuralDefinitionCatalogRoot,
    definitions_artifact_key: &str,
    definitions_artifact_path: &str,
    support: &MechanismSupportCatalogBuilder,
    support_closure: MechanismSupportClosureReceipt,
) -> Result<JsonValue, RelationalPublicationError> {
    let key = MechanismSupportKey::new(support.scope(), subject);
    let target_frontier_open = !support.target_is_complete();
    let summary = support
        .derive_closed_factorized_subject_summary(key, structural)
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
    if summary.key() != key
        || key.request_id() != request_id
        || support.closure() != Some(support_closure)
        || structural.closure() != Some(structural_closure)
        || target_frontier_open
    {
        return Err(RelationalPublicationError::MechanismOrdinalIndexMismatch);
    }
    let starter_projection = json!({
        "status": "not_materialized",
        "publication_authorization": "requires_explicit_subject_starters_declaration",
        "authorized_job_id": null,
        "artifact": null,
        "cells_serialized": false,
    });
    Ok(json!({
        "kind": "structural_subject_support",
        "request_id": hex(request_id.bytes()),
        "subject_ordinal": support_ordinal.to_string(),
        "target": public_mechanism_target_id(support_closure.target()),
        "subject": public_mechanism_support_subject(subject),
        "contributing_raw_signature_count": summary.contributing_signature_count().to_string(),
        "materialization": "factorized_summary",
        "cells_serialized": false,
        "factorized_summary_version": MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION,
        "factorized_support_root": hex(summary.root().bytes()),
        "projection_plan_version": MECHANISM_STARTER_PROJECTION_PLAN_VERSION,
        "projection_plan_id": hex(summary.projection_plan_id().bytes()),
        "factorized_signature_prefix_root": hex(summary.signature_prefix_root()),
        "signature_scan": {
            "limit": AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT.to_string(),
            "inspected": summary.inspected_signature_count().to_string(),
            "contributing": summary.contributing_signature_count().to_string(),
            "complete": summary.signature_scan_complete(),
            "status": if summary.signature_scan_complete() { "complete" } else { "capped" },
        },
        "shared_residual_root": hex(summary.shared_residual_root().bytes()),
        "case_count": public_mechanism_support_count(summary.case_count()),
        "origin_preimage_support": {
            "meaning": "request_target_conditioned_origins_with_a_supporting_successor",
            "status": "not_materialized",
            "distinct_starter_count": public_mechanism_support_count(summary.starter_count()),
            "correlated_origin_successor": {
                "root": null,
                "status": "not_materialized",
                "identity_kind": "authenticated_fiber_expression",
                "fiber_expr_version": MECHANISM_SUPPORT_FIBER_EXPR_VERSION,
                "coordinate_contract": {
                    "coordinate_system": "origin_preimage",
                    "source_key": "SourceKey",
                    "source_value": "(Context, Before)",
                    "fiber_member_key": "SuccessorKey",
                    "fiber_member_value": "After",
                    "type_binding": "mechanism_request_relation",
                },
                "inner": {
                    "expression_root": hex(summary.inner_fiber_expr_root().bytes()),
                    "representation": "factorized_subject_signature_union",
                    "status": "not_materialized",
                },
                "outer": {
                    "expression_root": hex(summary.outer_fiber_expr_root().bytes()),
                    "representation": if summary.fiber_expr_bounds_are_equal() {
                        "same_as_inner"
                    } else {
                        "inner_union_shared_possible_support_residual"
                    },
                    "shared_residual_root": hex(summary.shared_residual_root().bytes()),
                    "shared_residual_case_count": support_closure.unavailable_case_count().to_string(),
                    "opaque_undiscovered_target": target_frontier_open,
                    "target_frontier": if target_frontier_open { "open" } else { "closed" },
                    "status": "not_materialized",
                },
                "bounds_relation": if summary.fiber_expr_bounds_are_equal() {
                    "equal"
                } else {
                    "inner_subset_outer"
                },
                "materialized_content_root": null,
                "typed_values_serialized": false,
                "cells_serialized": false,
            },
            "starter_bound_basis": public_factorized_starter_bound_basis(
                summary.starter_bound_basis(),
            ),
        },
        "starter_projection": starter_projection,
        "raw_incidence_root": hex(support_closure.incidence_root().bytes()),
        "structural_quotient_root": hex(structural_closure.root().bytes()),
        "structural_definition_catalog_root": hex(definition_catalog_root.bytes()),
        "structural_definitions": public_structural_definition_artifact_reference(
            definitions_artifact_key,
            definitions_artifact_path,
        ),
        "mechanism_support_closure_root": hex(support_closure.root().bytes()),
    }))
}

struct SubjectStarterPublicationAuthority<'journal> {
    structural: &'journal StructuralMechanismCatalogBuilder,
    structural_closure: StructuralQuotientClosureReceipt,
    support: &'journal MechanismSupportCatalogBuilder,
    support_closure: MechanismSupportClosureReceipt,
    key_authority: MechanismClosedSubjectStarterProjectionAuthority,
}

fn mechanism_starter_unavailable_residual_case_count(
    journal: &RelationalJournal,
    request_id: MechanismRequestId,
) -> Result<Option<u128>, RelationalPublicationError> {
    let authority = structural_sidecar_authority(journal, request_id)?;
    Ok(authority.support.and_then(|(_, closure)| {
        let unavailable = closure.unavailable_case_count();
        (unavailable != 0).then_some(unavailable)
    }))
}

fn subject_starter_publication_authority<'journal>(
    journal: &'journal RelationalJournal,
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
) -> Result<Option<SubjectStarterPublicationAuthority<'journal>>, RelationalPublicationError> {
    let authority = structural_sidecar_authority(journal, request_id)?;
    let (Some(structural), Some(structural_closure), Some((support, support_closure))) = (
        authority.structural,
        authority.structural_closure,
        authority.support,
    ) else {
        return Ok(None);
    };
    // Typed correlated projection requires every target terminal. A closed
    // unavailable residual remains valid compact support evidence but cannot
    // authorize any exact subject fiber.
    if support_closure.unavailable_case_count() != 0 {
        return Ok(None);
    }
    let key = MechanismSupportKey::new(support.scope(), subject);
    if key.request_id() != request_id
        || key.target() != target
        || support_closure.request_id() != request_id
        || support_closure.target() != target
    {
        return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
    }
    let key_authority = support
        .derive_closed_subject_starter_projection_authority(key, structural)
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
    if key_authority.structural_root() != structural_closure.root()
        || key_authority.support_root() != support_closure.root()
        || key_authority.subject() != subject
    {
        return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
    }
    Ok(Some(SubjectStarterPublicationAuthority {
        structural,
        structural_closure,
        support,
        support_closure,
        key_authority,
    }))
}

fn subject_starter_projection_job(
    journal: &RelationalJournal,
    authority: &SubjectStarterPublicationAuthority<'_>,
    transition_schemas: &TransitionSchemaIdentities,
    authorization: &RelationalMechanismStarterValueAuthorization,
) -> Result<RelationalMechanismStarterProjectionJob, RelationalPublicationError> {
    RelationalMechanismStarterProjectionJob::new(
        authority.key_authority,
        journal.contract().relation_id(),
        transition_schemas,
        authorization,
    )
    .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))
}

fn checked_subject_starter_publication_authority<'journal>(
    journal: &'journal RelationalJournal,
    identity: SubjectStarterCursorIdentity,
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
    transition_schemas: &TransitionSchemaIdentities,
    authorization: &RelationalMechanismStarterValueAuthorization,
    source_end: Option<&PendingArtifactSourceEnd>,
) -> Result<
    Option<(
        SubjectStarterPublicationAuthority<'journal>,
        RelationalMechanismStarterProjectionJob,
    )>,
    RelationalPublicationError,
> {
    let live = subject_starter_publication_authority(journal, request_id, target, subject)?;
    let Some(source_end) = source_end else {
        return live
            .map(|authority| {
                let job = subject_starter_projection_job(
                    journal,
                    &authority,
                    transition_schemas,
                    authorization,
                )?;
                Ok((authority, job))
            })
            .transpose();
    };
    let PendingArtifactSourceEnd::SubjectStarters {
        identity: frozen_identity,
        structural_quotient_root,
        mechanism_support_root,
        projection_plan_id,
        projection_job_id,
    } = source_end
    else {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    };
    if *frozen_identity != identity {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    match (
        live,
        structural_quotient_root,
        mechanism_support_root,
        projection_plan_id,
        projection_job_id,
    ) {
        (None, None, None, None, None) => Ok(None),
        (Some(live), Some(structural_root), Some(support_root), Some(plan_id), Some(job_id)) => {
            let job =
                subject_starter_projection_job(journal, &live, transition_schemas, authorization)?;
            if live.structural_closure.root().bytes() != structural_root.bytes()
                || live.support_closure.root().bytes() != support_root.bytes()
                || live.key_authority.projection_plan_id().bytes() != plan_id.bytes()
                || job.id().bytes() != job_id.bytes()
            {
                return Err(RelationalPublicationError::PendingCursorMismatch);
            }
            Ok(Some((live, job)))
        }
        _ => Err(RelationalPublicationError::PendingCursorMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn subject_starter_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    consumer_id: [u8; 32],
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
    authorization: &RelationalMechanismStarterValueAuthorization,
    transition_schemas: &TransitionSchemaIdentities,
    structural_artifact_key: &str,
    structural_artifact_path: &str,
    identity: SubjectStarterCursorIdentity,
    header_emitted: bool,
    accumulator_cursor: Option<MechanismStarterAccumulatorCursor>,
    closure_emitted: bool,
    source_end: Option<&PendingArtifactSourceEnd>,
    line_budget: Option<PublicationLineBudget>,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    let expected_identity =
        SubjectStarterCursorIdentity::new(consumer_id, request_id, target, subject);
    if !authorization.validate_identity()
        || authorization.question_id() != journal.contract().question_id()
        || identity != expected_identity
        || (!header_emitted && (accumulator_cursor.is_some() || closure_emitted))
    {
        return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
    }
    let Some((authority, job)) = checked_subject_starter_publication_authority(
        journal,
        identity,
        request_id,
        target,
        subject,
        transition_schemas,
        authorization,
        source_end,
    )?
    else {
        return if header_emitted || accumulator_cursor.is_some() || closure_emitted {
            Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch)
        } else if source_end.is_some() {
            Ok(AddressedPublicationRecord::Exhausted)
        } else {
            Ok(AddressedPublicationRecord::NotReady)
        };
    };
    if closure_emitted {
        let Some(accumulator_cursor) = accumulator_cursor else {
            return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
        };
        let accumulator = accumulator_cursor.restore(job)?;
        return if header_emitted && accumulator.exhausted() && accumulator.finish(job).is_ok() {
            Ok(AddressedPublicationRecord::Exhausted)
        } else {
            Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch)
        };
    }
    if !header_emitted {
        let accumulator = RelationalMechanismStarterProjectionAccumulator::new(job);
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::SubjectStartersHeader { subject },
            next: ArtifactSourceCursor::SubjectStarters {
                identity,
                header_emitted: true,
                accumulator: Some(MechanismStarterAccumulatorCursor::from_accumulator(
                    accumulator,
                )),
                closure_emitted: false,
            },
            value: public_subject_starter_header(
                consumer_id,
                request_id,
                target,
                subject,
                authority.key_authority,
                job,
                authorization,
                structural_artifact_key,
                structural_artifact_path,
            ),
        });
    }
    let Some(accumulator_cursor) = accumulator_cursor else {
        return Err(RelationalPublicationError::MechanismStarterSourceCoordinateMismatch);
    };
    let mut accumulator = accumulator_cursor.restore(job)?;
    if accumulator.exhausted() {
        let closure = accumulator
            .finish(job)
            .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::SubjectStartersClosure { subject },
            next: ArtifactSourceCursor::SubjectStarters {
                identity,
                header_emitted,
                accumulator: Some(accumulator_cursor),
                closure_emitted: true,
            },
            value: public_subject_starter_closure(
                consumer_id,
                request_id,
                target,
                subject,
                job,
                authorization,
                closure,
            ),
        });
    }

    let scheduler = journal
        .scheduler_view()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    let mut page_member_limit = MECHANISM_STARTER_PAGE_MEMBER_LIMIT;
    let (page, value) = loop {
        let page = job
            .derive_next_page(
                authority.support,
                authority.structural,
                &accumulator,
                page_member_limit,
                |case_id| scheduler.case(case_id),
            )
            .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
        let value = public_subject_starter_page(
            consumer_id,
            request_id,
            target,
            subject,
            job,
            authorization,
            &page,
        );
        let Some(line_budget) = line_budget else {
            break (page, value);
        };
        let coordinate = PublicationSourceCoordinate::SubjectStartersPage {
            subject,
            page_ordinal: page.page_ordinal(),
        };
        let line =
            publication_line_bytes(artifact, coordinate, line_budget.checkpoint, value.clone())?;
        if line.len() <= line_budget.max_line_bytes {
            break (page, value);
        }
        if page_member_limit.get() == 1 {
            return Err(RelationalPublicationError::LineTooLarge {
                artifact: artifact.key().into(),
                bytes: line.len(),
                limit: line_budget.max_line_bytes,
            });
        }
        page_member_limit = NonZeroU16::new((page_member_limit.get() / 2).max(1))
            .expect("halving a nonzero starter page cap remains nonzero");
    };
    let page_ordinal = page.page_ordinal();
    accumulator
        .accept_page(&page)
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
    Ok(AddressedPublicationRecord::Emit {
        coordinate: PublicationSourceCoordinate::SubjectStartersPage {
            subject,
            page_ordinal,
        },
        next: ArtifactSourceCursor::SubjectStarters {
            identity,
            header_emitted,
            accumulator: Some(MechanismStarterAccumulatorCursor::from_accumulator(
                accumulator,
            )),
            closure_emitted: false,
        },
        value,
    })
}

fn public_mechanism_starter_authorization(
    authorization: &RelationalMechanismStarterValueAuthorization,
) -> JsonValue {
    json!({
        "version": authorization.version(),
        "authorization_id": hex(authorization.authorization_id().bytes()),
        "question_id": hex(authorization.question_id().bytes()),
        "authorizing_view_id": hex(authorization.view_id().bytes()),
        "authorizing_view_name": authorization.authorizing_view_name(),
        "role_schema_digest": hex(authorization.role_schema_digest()),
        "projections": authorization.projections().iter().map(|projection| json!({
            "role": projection.role().binding_name(),
            "select_index": projection.select_index().to_string(),
            "output_name": projection.output_name(),
            "type_digest": hex(projection.type_digest()),
        })).collect::<Vec<_>>(),
    })
}

#[allow(clippy::too_many_arguments)]
fn public_subject_starter_header(
    consumer_id: [u8; 32],
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
    authority: MechanismClosedSubjectStarterProjectionAuthority,
    job: RelationalMechanismStarterProjectionJob,
    authorization: &RelationalMechanismStarterValueAuthorization,
    structural_artifact_key: &str,
    structural_artifact_path: &str,
) -> JsonValue {
    json!({
        "kind": "subject_starters_header",
        "consumer_id": hex(consumer_id),
        "request_id": hex(request_id.bytes()),
        "target": public_mechanism_target_id(target),
        "subject": public_mechanism_support_subject(subject),
        "projection_plan_version": MECHANISM_STARTER_PROJECTION_PLAN_VERSION,
        "projection_plan_id": hex(authority.projection_plan_id().bytes()),
        "projection_job_version": RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION,
        "projection_job_id": hex(job.id().bytes()),
        "authorization": public_mechanism_starter_authorization(authorization),
        "correlated_fiber_expression_root": hex(authority.correlated_fiber_expr_root().bytes()),
        "exact_case_count": authority.exact_case_count().to_string(),
        "exact_distinct_starter_count": null,
        "structural_quotient_root": hex(authority.structural_root().bytes()),
        "mechanism_support_closure_root": hex(authority.support_root().bytes()),
        "structural_support": {
            "artifact_key": structural_artifact_key,
            "path": structural_artifact_path,
        },
        "page_member_limit": MECHANISM_STARTER_PAGE_MEMBER_LIMIT.get(),
    })
}

#[allow(clippy::too_many_arguments)]
fn public_subject_starter_page(
    consumer_id: [u8; 32],
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
    job: RelationalMechanismStarterProjectionJob,
    authorization: &RelationalMechanismStarterValueAuthorization,
    page: &RelationalMechanismStarterProjectionPage,
) -> JsonValue {
    json!({
        "kind": "subject_starters_page",
        "consumer_id": hex(consumer_id),
        "request_id": hex(request_id.bytes()),
        "target": public_mechanism_target_id(target),
        "subject": public_mechanism_support_subject(subject),
        "projection_plan_id": hex(job.projection_plan_id().bytes()),
        "projection_job_id": hex(job.id().bytes()),
        "authorization_id": hex(authorization.authorization_id().bytes()),
        "structural_quotient_root": hex(job.authority().structural_root().bytes()),
        "mechanism_support_closure_root": hex(job.authority().support_root().bytes()),
        "exact_case_count": job.authority().exact_case_count().to_string(),
        "page_ordinal": page.page_ordinal().to_string(),
        "page_id": hex(page.id().bytes()),
        "page_root": hex(page.root().bytes()),
        "start_after": public_mechanism_starter_key_cursor(page.start_after()),
        "end_cursor": public_mechanism_starter_key_cursor(page.end_cursor()),
        "exhausted": page.exhausted(),
        "members": page.members().iter().map(|member| json!({
            "member_id": hex(member.id().bytes()),
            "raw_signature_id": hex(member.raw_signature_id().bytes()),
            "case_id": hex(member.case_id().bytes()),
            "source_key": hex(member.source_key().bytes()),
            "context": public_explore_value(member.context()),
            "before": public_explore_value(member.before()),
            "successor_key": hex(member.successor_key().bytes()),
            "after": public_explore_value(member.after()),
        })).collect::<Vec<_>>(),
    })
}

fn public_mechanism_starter_key_cursor(cursor: Option<MechanismSupportStarterCursor>) -> JsonValue {
    cursor.map_or(JsonValue::Null, |cursor| {
        json!({
            "source_key": hex(cursor.source_key().bytes()),
            "successor_key": hex(cursor.successor_key().bytes()),
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn public_subject_starter_closure(
    consumer_id: [u8; 32],
    request_id: MechanismRequestId,
    target: MechanismTargetId,
    subject: MechanismSupportSubject,
    job: RelationalMechanismStarterProjectionJob,
    authorization: &RelationalMechanismStarterValueAuthorization,
    closure: RelationalMechanismStarterProjectionClosure,
) -> JsonValue {
    json!({
        "kind": "subject_starters_closure",
        "consumer_id": hex(consumer_id),
        "request_id": hex(request_id.bytes()),
        "target": public_mechanism_target_id(target),
        "subject": public_mechanism_support_subject(subject),
        "projection_plan_id": hex(closure.projection_plan_id().bytes()),
        "projection_job_id": hex(closure.job_id().bytes()),
        "authorization_id": hex(authorization.authorization_id().bytes()),
        "projection_closure_root": hex(closure.root().bytes()),
        "content_root": hex(closure.content_root().bytes()),
        "exact_case_count": closure.exact_case_count().to_string(),
        "exact_distinct_starter_count": closure.exact_starter_count().to_string(),
        "page_count": closure.page_count().to_string(),
        "page_manifest_root": hex(closure.page_manifest_root().bytes()),
        "structural_quotient_root": hex(job.authority().structural_root().bytes()),
        "mechanism_support_closure_root": hex(job.authority().support_root().bytes()),
    })
}

fn public_mechanism_support_subject(subject: MechanismSupportSubject) -> JsonValue {
    match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => json!({
            "kind": "mechanism",
            "facet": null,
            "structural_mechanism_id": hex(mechanism_id.bytes()),
        }),
        MechanismSupportSubject::Node { facet, node_id } => json!({
            "kind": "node",
            "facet": mechanism_support_facet_name(facet),
            "structural_node_id": hex(node_id.bytes()),
        }),
        MechanismSupportSubject::Edge { facet, edge_id } => json!({
            "kind": "edge",
            "facet": mechanism_support_facet_name(facet),
            "structural_edge_id": hex(edge_id.bytes()),
        }),
    }
}

const fn mechanism_support_facet_name(facet: MechanismSupportFacet) -> &'static str {
    match facet {
        MechanismSupportFacet::Activation => "activation",
        MechanismSupportFacet::DifferentialParticipation => "differential_participation",
    }
}

fn public_mechanism_support_closure(
    closure: MechanismSupportClosureReceipt,
    structural_subject_count: u128,
    definition_catalog_root: StructuralDefinitionCatalogRoot,
    definitions_artifact_key: &str,
    definitions_artifact_path: &str,
) -> JsonValue {
    json!({
        "kind": "mechanism_support_closure",
        "request_id": hex(closure.request_id().bytes()),
        "target": public_mechanism_target_id(closure.target()),
        "target_seal_id": hex(closure.target_seal_id().bytes()),
        "raw_incidence_root": hex(closure.incidence_root().bytes()),
        "structural_quotient_root": hex(closure.structural_root().bytes()),
        "shared_residual_root": hex(closure.residual_root().bytes()),
        "counts": {
            "structural_subject_support_rows": structural_subject_count.to_string(),
            "target_cases": closure.target_case_count().to_string(),
            "successful_cases": closure.successful_case_count().to_string(),
            "unavailable_cases": closure.unavailable_case_count().to_string(),
            "signature_fibers": closure.signature_fiber_count().to_string(),
            "target_starters": closure.target_starter_count().to_string(),
        },
        "structural_definition_catalog_root": hex(definition_catalog_root.bytes()),
        "structural_definitions": public_structural_definition_artifact_reference(
            definitions_artifact_key,
            definitions_artifact_path,
        ),
        "mechanism_support_closure_root": hex(closure.root().bytes()),
    })
}

fn public_structural_definition_artifact_reference(key: &str, path: &str) -> JsonValue {
    json!({
        "artifact_key": key,
        "path": path,
        "availability": "structural_quotient_closed",
    })
}

fn public_mechanism_support_count(count: MechanismSupportCount) -> JsonValue {
    match count {
        MechanismSupportCount::Unknown {
            confirmed_lower_bound,
        } => json!({
            "status": "unknown",
            "confirmed_lower_bound": confirmed_lower_bound.to_string(),
        }),
        MechanismSupportCount::Interval {
            lower_bound,
            upper_bound,
        } => json!({
            "status": "interval",
            "lower_bound": lower_bound.to_string(),
            "upper_bound": upper_bound.to_string(),
        }),
        MechanismSupportCount::Exact(value) => json!({
            "status": "exact",
            "value": value.to_string(),
        }),
    }
}

fn public_factorized_starter_bound_basis(basis: MechanismFactorizedStarterBoundBasis) -> JsonValue {
    match basis {
        MechanismFactorizedStarterBoundBasis::ExactEmpty => json!({
            "kind": "exact_empty",
        }),
        MechanismFactorizedStarterBoundBasis::ExactFactorizedBoundCollapse => json!({
            "kind": "exact_factorized_bound_collapse",
            "evidence": "factorized_support_root",
        }),
        MechanismFactorizedStarterBoundBasis::ExactTargetStarterSaturation {
            target_starter_root,
        } => json!({
            "kind": "exact_target_starter_saturation",
            "target_starter_set_root": hex(target_starter_root),
        }),
        MechanismFactorizedStarterBoundBasis::ConservativeTargetProjectionUpper {
            target_starter_root,
        } => json!({
            "kind": "conservative_target_projection_upper",
            "target_starter_set_root": hex(target_starter_root),
        }),
    }
}

fn public_mechanism_target_id(target: MechanismTargetId) -> JsonValue {
    match target {
        MechanismTargetId::Selected => json!({
            "kind": "selected",
        }),
        MechanismTargetId::ChosenView(view_id) => json!({
            "kind": "chosen_view",
            "view_id": hex(view_id.bytes()),
        }),
    }
}

fn case_support_record(
    artifact: &PublicationArtifactPlan,
    projection: Option<&PublicationCaseSupportProjection<'_>>,
    source_ordinal: u128,
) -> Result<PublicationRecord, RelationalPublicationError> {
    let Some(projection) = projection else {
        return if source_ordinal == 0 {
            Ok(PublicationRecord::NotReady)
        } else {
            Err(RelationalPublicationError::PublicationSourceAhead {
                artifact: artifact.key().into(),
                next_source_ordinal: source_ordinal,
                available: 0,
            })
        };
    };
    let available = projection.available_source_record_count();
    if source_ordinal > available {
        return Err(RelationalPublicationError::PublicationSourceAhead {
            artifact: artifact.key().into(),
            next_source_ordinal: source_ordinal,
            available,
        });
    }
    if let Some(record) = projection.record_at(source_ordinal)? {
        return Ok(PublicationRecord::Emit(public_case_support_record(record)));
    }
    if source_ordinal != available {
        return Err(RelationalPublicationError::CaseSupport(
            "case-support projection omitted an addressable source ordinal".into(),
        ));
    }
    Ok(if projection.is_open() {
        PublicationRecord::NotReady
    } else {
        PublicationRecord::Exhausted
    })
}

fn case_transition_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    projection: Option<&RelationalCaseTransitionProjection>,
    source_ordinal: u128,
) -> Result<PublicationRecord, RelationalPublicationError> {
    let Some(projection) = projection else {
        return if source_ordinal == 0 {
            Ok(PublicationRecord::NotReady)
        } else {
            Err(RelationalPublicationError::PublicationSourceAhead {
                artifact: artifact.key().into(),
                next_source_ordinal: source_ordinal,
                available: 0,
            })
        };
    };
    let available = projection.available_source_record_count();
    if source_ordinal > available {
        return Err(RelationalPublicationError::PublicationSourceAhead {
            artifact: artifact.key().into(),
            next_source_ordinal: source_ordinal,
            available,
        });
    }
    if let Some(record) = projection.record_at(source_ordinal) {
        return Ok(PublicationRecord::Emit(public_case_transition_record(
            artifact, journal, record,
        )?));
    }
    if source_ordinal != available {
        return Err(RelationalPublicationError::CaseTransitions(
            "selected case-transition projection omitted an addressable source ordinal".into(),
        ));
    }
    Ok(if projection.is_open() {
        PublicationRecord::NotReady
    } else {
        PublicationRecord::Exhausted
    })
}

fn public_case_transition_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    record: RelationalCaseTransitionProjectionRecord,
) -> Result<JsonValue, RelationalPublicationError> {
    let PublicationArtifactPlan::CaseTransitions {
        authorization,
        transition_schemas,
    } = artifact
    else {
        return Err(RelationalPublicationError::PlanIdentityMismatch);
    };
    match record {
        RelationalCaseTransitionProjectionRecord::Header {
            projection_id,
            contract,
            state_schema_id,
            context_schema_id,
            transition_type_id,
            authorization_id,
            authorizing_view_id,
        } => {
            if state_schema_id != transition_schemas.state_schema_id()
                || context_schema_id != transition_schemas.context_schema_id()
                || transition_type_id != transition_schemas.transition_type_id()
                || authorization_id != authorization.authorization_id()
                || authorizing_view_id != authorization.view_id()
            {
                return Err(RelationalPublicationError::CaseTransitions(
                    "selected case-transition header disagrees with its checked publication plan"
                        .into(),
                ));
            }
            Ok(json!({
                "kind": "header",
                "projection_schema": RELATIONAL_CASE_TRANSITION_PROJECTION_SCHEMA,
                "projection_version": RELATIONAL_CASE_TRANSITION_PROJECTION_VERSION,
                "projection_id": hex(projection_id.bytes()),
                "relation_id": hex(contract.relation_id().bytes()),
                "admission_id": hex(contract.admission_id().bytes()),
                "question_id": hex(contract.question_id().bytes()),
                "state_schema_id": hex(state_schema_id.bytes()),
                "context_schema_id": hex(context_schema_id.bytes()),
                "transition_type_id": hex(transition_type_id.bytes()),
                "value_authorization": {
                    "authorization_id": hex(authorization_id.bytes()),
                    "authorizing_view_id": hex(authorizing_view_id.bytes()),
                    "authorizing_view_name": authorization.authorizing_view_name(),
                },
                "source_order": "journal_selected_discovery",
                "node_encoding": "endpoint_state_ids_on_case_supported_edges",
            }))
        }
        RelationalCaseTransitionProjectionRecord::CaseTransition(member) => {
            let scheduler = journal
                .scheduler_view()
                .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
            let case = scheduler.case(member.case_id()).ok_or_else(|| {
                RelationalPublicationError::CaseTransitions(
                    "selected case-transition member no longer resolves in the journal".into(),
                )
            })?;
            if case.source_key() != member.source_key()
                || case.successor_key() != member.successor_key()
            {
                return Err(RelationalPublicationError::CaseTransitions(
                    "selected case-transition member disagrees with its journal coordinate".into(),
                ));
            }
            Ok(json!({
                "kind": "case_transition",
                "case_id": hex(member.case_id().bytes()),
                "source_key": hex(member.source_key().bytes()),
                "successor_key": hex(member.successor_key().bytes()),
                "transition_id": hex(member.transition_id().bytes()),
                "before_state_id": hex(member.before_state_id().bytes()),
                "after_state_id": hex(member.after_state_id().bytes()),
                "state_schema_id": hex(transition_schemas.state_schema_id().bytes()),
                "context_schema_id": hex(transition_schemas.context_schema_id().bytes()),
                "transition_type_id": hex(transition_schemas.transition_type_id().bytes()),
                "context": public_explore_value(case.context()),
                "before": public_explore_value(case.before()),
                "after": public_explore_value(case.after()),
                "authorizing_view_id": hex(authorization.view_id().bytes()),
            }))
        }
        RelationalCaseTransitionProjectionRecord::Closure(closure) => Ok(json!({
            "kind": "closure",
            "frontier": "exact",
            "selected_question_seal_id": hex(closure.selected_question_seal_id().bytes()),
            "selected_case_set_root": hex(closure.selected_case_set_root().bytes()),
            "content_root": hex(closure.content_root().bytes()),
            "data_record_count": closure.data_record_count().to_string(),
            "counts": {
                "selected_cases": {
                    "status": "exact",
                    "value": closure.exact_case_count().to_string(),
                },
                "state_nodes": {
                    "status": "exact",
                    "value": closure.exact_state_count().to_string(),
                },
                "semantic_transitions": {
                    "status": "exact",
                    "value": closure.exact_transition_count().to_string(),
                },
            },
        })),
        RelationalCaseTransitionProjectionRecord::CapacityLimited(capacity) => Ok(json!({
            "kind": "capacity_limited",
            "frontier": "capacity_limited",
            "complete": false,
            "reason": "v1_collision_checked_projection_member_limit",
            "maximum_members": capacity.maximum_members().to_string(),
            "required_at_least": capacity.required_at_least().to_string(),
            "selected_question_seal_id": null,
            "selected_case_set_root": null,
            "content_root": null,
        })),
    }
}

fn public_case_support_record(record: PublicationCaseSupportRecord) -> JsonValue {
    match record {
        PublicationCaseSupportRecord::Partitioned(record) => {
            public_partitioned_case_support_record(record)
        }
        PublicationCaseSupportRecord::ClassificationSummary(record) => {
            public_classification_summary_record(record)
        }
    }
}

fn public_partitioned_case_support_record(
    record: RelationalCaseSupportProjectionRecord,
) -> JsonValue {
    match record {
        RelationalCaseSupportProjectionRecord::Root {
            relation_id,
            admission_id,
            question_id,
            support_plan_root,
            partition_artifact_id,
            exact_logical_case_count,
            planned_chunk_count,
            case_id_authority,
        } => json!({
            "kind": "root",
            "projection_schema": RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA,
            "projection_version": RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION,
            "relation_id": hex(relation_id.bytes()),
            "admission_id": hex(admission_id.bytes()),
            "question_id": hex(question_id.bytes()),
            "support_plan_root": hex(support_plan_root.bytes()),
            "partition_artifact_id": hex(partition_artifact_id.bytes()),
            "exact_logical_case_count": exact_logical_case_count.to_string(),
            "planned_chunk_count": planned_chunk_count.to_string(),
            "case_id_authority": case_id_authority.map(public_case_id_authority),
        }),
        RelationalCaseSupportProjectionRecord::Chunk {
            partition_artifact_id,
            chunk_artifact_id,
            chunk_ordinal,
            exact_case_count,
            rejected_case_count,
            admitted_not_selected_case_count,
            admitted_selected_case_count,
            region_count,
        } => json!({
            "kind": "chunk",
            "partition_artifact_id": hex(partition_artifact_id.bytes()),
            "chunk_artifact_id": hex(chunk_artifact_id.bytes()),
            "chunk_ordinal": chunk_ordinal.to_string(),
            "exact_case_count": exact_case_count.to_string(),
            "rejected_case_count": rejected_case_count.to_string(),
            "admitted_not_selected_case_count": admitted_not_selected_case_count.to_string(),
            "admitted_selected_case_count": admitted_selected_case_count.to_string(),
            "region_count": region_count.to_string(),
        }),
        RelationalCaseSupportProjectionRecord::Region {
            chunk_artifact_id,
            run_id,
            run_ordinal,
            exact_case_count,
            outcome,
        } => json!({
            "kind": "region",
            "chunk_artifact_id": hex(chunk_artifact_id.bytes()),
            "run_id": hex(run_id.bytes()),
            "run_ordinal": run_ordinal,
            "exact_case_count": exact_case_count.to_string(),
            "outcome": public_case_support_outcome(outcome),
        }),
        RelationalCaseSupportProjectionRecord::SelectedMaterialization {
            run_id,
            artifact_id,
            exact_case_count,
            materialized_cases_root,
        } => json!({
            "kind": "selected_materialization",
            "run_id": hex(run_id.bytes()),
            "materialization_artifact_id": hex(artifact_id.bytes()),
            "exact_case_count": exact_case_count.to_string(),
            "materialized_cases_root": hex(materialized_cases_root),
        }),
        RelationalCaseSupportProjectionRecord::AuthorizedCase {
            materialization_artifact_id,
            case_id,
            authority,
        } => json!({
            "kind": "authorized_case",
            "materialization_artifact_id": hex(materialization_artifact_id.bytes()),
            "case_id": hex(case_id.bytes()),
            "authority": public_case_id_authority(authority),
        }),
        RelationalCaseSupportProjectionRecord::Closure(closure) => json!({
            "kind": "closure",
            "frontier": "exact",
            "partition_artifact_id": hex(closure.partition_artifact_id.bytes()),
            "support_evidence_root": hex(closure.support_evidence_root.bytes()),
            "selected_question_seal_id": hex(closure.selected_question_seal_id.bytes()),
            "exact_logical_case_count": closure.exact_logical_case_count.to_string(),
            "exact_selected_case_count": closure.exact_selected_case_count.to_string(),
            "classified_chunk_count": closure.classified_chunk_count.to_string(),
            "region_count": closure.region_count.to_string(),
            "selected_materialization_count": closure.selected_materialization_count.to_string(),
            "authorized_case_record_count": closure.authorized_case_record_count.to_string(),
            "data_record_count": closure.data_record_count.to_string(),
        }),
    }
}

fn public_classification_summary_record(
    record: RelationalClassificationSummaryProjectionRecord,
) -> JsonValue {
    match record {
        RelationalClassificationSummaryProjectionRecord::Root {
            contract,
            support_plan_root,
            classification_authority,
            exact_logical_case_count,
            exact_admitted_case_count,
            exact_selected_case_count,
            case_id_authority,
        } => json!({
            "kind": "root",
            "projection_kind": "classification_summary",
            "classification_authority": public_classification_authority(
                classification_authority,
            ),
            "projection_schema": RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA,
            "projection_version": RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION,
            "relation_id": hex(contract.relation_id().bytes()),
            "admission_id": hex(contract.admission_id().bytes()),
            "question_id": hex(contract.question_id().bytes()),
            "support_plan_root": hex(support_plan_root),
            "exact_logical_case_count": exact_logical_case_count.to_string(),
            "exact_admitted_case_count": exact_admitted_case_count.to_string(),
            "exact_selected_case_count": exact_selected_case_count.to_string(),
            "classification_region_count": RelationalClassificationSummaryProjection::REGION_COUNT
                .to_string(),
            "case_id_authority": case_id_authority.map(public_case_id_authority),
        }),
        RelationalClassificationSummaryProjectionRecord::Region {
            question_id,
            region_ordinal,
            exact_case_count,
            outcome,
        } => json!({
            "kind": "region",
            "projection_kind": "classification_summary",
            "parent_question_id": hex(question_id.bytes()),
            "region_ordinal": region_ordinal,
            "exact_case_count": exact_case_count.to_string(),
            "outcome": public_classification_summary_outcome(outcome),
        }),
        RelationalClassificationSummaryProjectionRecord::AuthorizedCase {
            question_id,
            selected_region_ordinal,
            case_id,
            authority,
        } => json!({
            "kind": "authorized_case",
            "projection_kind": "classification_summary",
            "parent_question_id": hex(question_id.bytes()),
            "parent_region_ordinal": selected_region_ordinal,
            "case_id": hex(case_id.bytes()),
            "authority": public_case_id_authority(authority),
        }),
        RelationalClassificationSummaryProjectionRecord::Closure(closure) => json!({
            "kind": "closure",
            "projection_kind": "classification_summary",
            "classification_authority": public_classification_authority(
                closure.classification_authority,
            ),
            "frontier": "exact",
            "support_evidence_root": hex(closure.support_evidence_root),
            "selected_question_seal_id": hex(closure.selected_question_seal_id.bytes()),
            "selected_population_authority": public_published_selected_population_authority(
                closure.selected_population_authority,
            ),
            "exact_logical_case_count": closure.exact_logical_case_count.to_string(),
            "exact_admitted_case_count": closure.exact_admitted_case_count.to_string(),
            "exact_selected_case_count": closure.exact_selected_case_count.to_string(),
            "classification_region_count": RelationalClassificationSummaryProjection::REGION_COUNT
                .to_string(),
            "authorized_case_record_count": closure.authorized_case_record_count.to_string(),
            "data_record_count": closure.data_record_count.to_string(),
        }),
    }
}

fn public_classification_summary_outcome(
    outcome: RelationalClassificationSummaryOutcome,
) -> &'static str {
    match outcome {
        RelationalClassificationSummaryOutcome::Rejected => "rejected",
        RelationalClassificationSummaryOutcome::AdmittedNotSelected => "admitted_not_selected",
        RelationalClassificationSummaryOutcome::AdmittedSelected => "admitted_selected",
    }
}

fn public_classification_authority(
    authority: RelationalPublishedClassificationAuthority,
) -> &'static str {
    match authority {
        RelationalPublishedClassificationAuthority::CertifiedSupport => "certified_support",
        RelationalPublishedClassificationAuthority::ExtensionalCatalog => "extensional_catalog",
        RelationalPublishedClassificationAuthority::ComposedExactEvidence => {
            "composed_exact_evidence"
        }
    }
}

fn public_published_selected_population_authority(
    authority: RelationalPublishedSelectedPopulationAuthority,
) -> JsonValue {
    match authority {
        RelationalPublishedSelectedPopulationAuthority::ExtensionalQuestion {
            question_content_root,
        } => json!({
            "kind": "extensional_question",
            "question_content_root": hex(question_content_root),
        }),
        RelationalPublishedSelectedPopulationAuthority::CertifiedSupport { population_root } => {
            json!({
                "kind": "certified_support",
                "certified_selected_population_root": hex(population_root),
            })
        }
    }
}

fn public_case_id_authority(authority: RelationalCaseIdPublicationAuthority) -> JsonValue {
    match authority {
        RelationalCaseIdPublicationAuthority::ResultView(view_id) => json!({
            "kind": "checked_result_view",
            "view_id": hex(view_id.bytes()),
        }),
    }
}

fn public_case_support_outcome(
    outcome: super::relational_classified_sweep::RelationalClassifiedCaseOutcome,
) -> &'static str {
    match outcome {
        super::relational_classified_sweep::RelationalClassifiedCaseOutcome::Rejected => {
            "rejected"
        }
        super::relational_classified_sweep::RelationalClassifiedCaseOutcome::AdmittedNotSelected => {
            "admitted_not_selected"
        }
        super::relational_classified_sweep::RelationalClassifiedCaseOutcome::AdmittedSelected => {
            "admitted_selected"
        }
    }
}

fn public_case_support_metadata(metadata: RelationalCaseSupportProjectionMetadata) -> JsonValue {
    json!({
        "frontier": public_case_support_frontier(metadata.frontier),
        "counts": {
            "logical_cases": {
                "status": "exact",
                "value": metadata.exact_logical_case_count.to_string(),
            },
            "classified_cases": public_case_support_count(metadata.classified_case_count),
            "selected_cases": public_case_support_count(metadata.selected_case_count),
            "materialized_selected_cases": public_case_support_count(
                metadata.materialized_selected_case_count,
            ),
            "planned_chunks": metadata.planned_chunk_count.to_string(),
            "classified_chunks": metadata.classified_chunk_count.to_string(),
            "published_chunks": metadata.published_chunk_count.to_string(),
            "published_regions": metadata.published_region_count.to_string(),
            "published_selected_materializations": metadata
                .published_selected_materialization_count
                .to_string(),
            "authorized_case_records": metadata.authorized_case_record_count.to_string(),
            "available_source_records": metadata.available_source_record_count.to_string(),
        },
    })
}

fn public_case_support_projection_metadata(
    projection: &PublicationCaseSupportProjection<'_>,
) -> JsonValue {
    match projection {
        PublicationCaseSupportProjection::Partitioned(projection) => {
            let mut metadata = public_case_support_metadata(projection.metadata());
            if let Some(object) = metadata.as_object_mut() {
                object.insert(
                    "projection_kind".into(),
                    JsonValue::String("partitioned_support".into()),
                );
            }
            metadata
        }
        PublicationCaseSupportProjection::ClassificationSummary(projection) => {
            let closure = projection.closure;
            json!({
                "projection_kind": "classification_summary",
                "frontier": {
                    "status": "exact",
                    "classification_authority": public_classification_authority(
                        closure.classification_authority,
                    ),
                    "support_evidence_root": hex(closure.support_evidence_root),
                    "selected_question_seal_id": hex(closure.selected_question_seal_id.bytes()),
                    "selected_population_authority": public_published_selected_population_authority(
                        closure.selected_population_authority,
                    ),
                    "data_record_count": closure.data_record_count.to_string(),
                },
                "counts": {
                    "logical_cases": {
                        "status": "exact",
                        "value": closure.exact_logical_case_count.to_string(),
                    },
                    "admitted_cases": {
                        "status": "exact",
                        "value": closure.exact_admitted_case_count.to_string(),
                    },
                    "selected_cases": {
                        "status": "exact",
                        "value": closure.exact_selected_case_count.to_string(),
                    },
                    "materialized_selected_cases": {
                        "status": "exact",
                        "value": closure.exact_selected_case_count.to_string(),
                    },
                    "classification_regions": RelationalClassificationSummaryProjection::REGION_COUNT
                        .to_string(),
                    "authorized_case_records": closure.authorized_case_record_count.to_string(),
                    "available_source_records": projection.available_source_record_count().to_string(),
                },
            })
        }
    }
}

fn public_case_transition_projection_metadata(
    projection: &RelationalCaseTransitionProjection,
) -> JsonValue {
    let metadata = projection.metadata();
    let selected_case_lower_bound = metadata.capacity().map_or_else(
        || metadata.selected_case_count(),
        |capacity| capacity.required_at_least(),
    );
    json!({
        "projection_id": hex(metadata.projection_id().bytes()),
        "frontier": metadata.closure().map_or_else(|| {
            metadata.capacity().map_or_else(|| json!({
                "status": "open",
                "reason": "awaiting_selected_question_seal",
            }), |capacity| json!({
                "status": "capacity_limited",
                "reason": "v1_collision_checked_projection_member_limit",
                "maximum_members": capacity.maximum_members().to_string(),
                "required_at_least": capacity.required_at_least().to_string(),
                "exact_closure_claimed": false,
            }))
        },
            |closure| json!({
                "status": "exact",
                "selected_question_seal_id": hex(
                    closure.selected_question_seal_id().bytes()
                ),
                "selected_case_set_root": hex(closure.selected_case_set_root().bytes()),
                "content_root": hex(closure.content_root().bytes()),
            }),
        ),
        "counts": {
            "selected_cases": {
                "status": if metadata.closure().is_some() { "exact" } else { "lower_bound" },
                "value": selected_case_lower_bound.to_string(),
                "scope": "global_selected_frontier",
            },
            "materialized_case_edges": {
                "status": "exact",
                "value": metadata.selected_case_count().to_string(),
                "scope": "retained_graph_prefix",
            },
            "state_nodes": {
                "status": if metadata.closure().is_some() { "exact" } else { "lower_bound" },
                "value": metadata.distinct_state_count().to_string(),
                "scope": if metadata.capacity().is_some() { "retained_prefix" } else { "observed_frontier" },
            },
            "semantic_transitions": {
                "status": if metadata.closure().is_some() { "exact" } else { "lower_bound" },
                "value": metadata.distinct_transition_count().to_string(),
                "scope": if metadata.capacity().is_some() { "retained_prefix" } else { "observed_frontier" },
            },
            "available_source_records": metadata.available_source_record_count().to_string(),
        },
    })
}

fn public_case_support_count(count: RelationalCaseSupportCount) -> JsonValue {
    json!({
        "status": if count.is_exact() { "exact" } else { "lower_bound" },
        "value": count.value().to_string(),
    })
}

fn public_case_support_frontier(frontier: RelationalCaseSupportProjectionFrontier) -> JsonValue {
    match frontier {
        RelationalCaseSupportProjectionFrontier::Open(reason) => json!({
            "status": "open",
            "reason": match reason {
                RelationalCaseSupportOpenReason::AwaitingClassifiedChunk {
                    next_chunk_ordinal,
                } => json!({
                    "kind": "awaiting_classified_chunk",
                    "next_chunk_ordinal": next_chunk_ordinal.to_string(),
                }),
                RelationalCaseSupportOpenReason::AwaitingSelectedMaterialization {
                    chunk_ordinal,
                    run_ordinal,
                } => json!({
                    "kind": "awaiting_selected_materialization",
                    "chunk_ordinal": chunk_ordinal.to_string(),
                    "run_ordinal": run_ordinal,
                }),
                RelationalCaseSupportOpenReason::AwaitingClosureAuthority => json!({
                    "kind": "awaiting_closure_authority",
                }),
            },
        }),
        RelationalCaseSupportProjectionFrontier::Exact(closure) => json!({
            "status": "exact",
            "partition_artifact_id": hex(closure.partition_artifact_id.bytes()),
            "support_evidence_root": hex(closure.support_evidence_root.bytes()),
            "selected_question_seal_id": hex(closure.selected_question_seal_id.bytes()),
            "data_record_count": closure.data_record_count.to_string(),
        }),
    }
}

fn mechanism_result_input_coverage(
    journal: &RelationalJournal,
    input: ResultPublicationInput,
) -> Result<Option<MechanismResultInputCoverage>, RelationalPublicationError> {
    let Some(request_id) = input.mechanism_request_id() else {
        return Ok(None);
    };
    let Some(analysis) = journal.analysis_state() else {
        return Ok(Some(MechanismResultInputCoverage::open(request_id)));
    };
    match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            let status = open.layer_status(RelationalAnalysisLayerId::Mechanisms(request_id));
            if status.is_none() {
                return Ok(Some(MechanismResultInputCoverage::open(request_id)));
            }
            let incidence = open
                .mechanism_incidence(request_id)
                .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
            let target_cases = incidence.target_case_count() as u128;
            let terminal_cases = incidence.terminal_case_count() as u128;
            let incidence_cases = incidence.incidence_case_count() as u128;
            let unavailable_cases = terminal_cases
                .checked_sub(incidence_cases)
                .ok_or(RelationalPublicationError::AnalysisCatalogStateMismatch)?;
            Ok(Some(MechanismResultInputCoverage {
                request_id,
                target_is_sealed: incidence.target_is_sealed(),
                frontier_complete: matches!(
                    status,
                    Some(RelationalAnalysisLayerStatus::MechanismClosed)
                ) && incidence.frontier_is_complete(),
                target_cases,
                incidence_cases,
                unavailable_cases,
            }))
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Mechanisms(request_id))
                .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            let RelationalAnalysisLayerSnapshot::Mechanisms(mechanism) = layer else {
                return Err(RelationalPublicationError::AnalysisLayerKindMismatch);
            };
            let incidence = mechanism.incidence();
            let counts = incidence.counts();
            Ok(Some(MechanismResultInputCoverage {
                request_id,
                target_is_sealed: incidence.target_is_sealed(),
                frontier_complete: incidence.frontier_is_complete(),
                target_cases: counts.target_cases().confirmed_lower_bound(),
                incidence_cases: counts.incidence_cases().confirmed_lower_bound(),
                unavailable_cases: counts.unavailable_cases().confirmed_lower_bound(),
            }))
        }
        _ => Err(RelationalPublicationError::AnalysisCatalogStateMismatch),
    }
}

fn early_each_case_record(
    journal: &RelationalJournal,
    view_id: ViewId,
    select_names: &[Box<str>],
    source_ordinal: u128,
) -> Result<PublicationRecord, RelationalPublicationError> {
    let ordinal = usize::try_from(source_ordinal).map_err(|_| {
        RelationalPublicationError::SourceOrdinalOverflow {
            artifact: format!("view:{}", hex(view_id.bytes())),
            ordinal: source_ordinal,
        }
    })?;
    let scheduler = journal
        .scheduler_view()
        .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?;
    // Deliberately use the replay-built append order. Result evidence itself
    // is CaseId-sorted, and a later hash may sort before every prior record.
    let Some(case_id) = scheduler
        .selected_discovery_suffix(ordinal)
        .first()
        .copied()
    else {
        // Exhausted means caught up with this durable prefix, not that FIND is
        // semantically closed. A later authenticated head may extend it.
        return Ok(PublicationRecord::Exhausted);
    };
    let Some(analysis) = journal.analysis_state() else {
        return Ok(PublicationRecord::NotReady);
    };
    let row_id = ResultViewInputRowId::Case(case_id);
    let record = match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            match open.layer_status(RelationalAnalysisLayerId::Result(view_id)) {
                Some(RelationalAnalysisLayerStatus::ResultUnregistered) | None => {
                    return Ok(PublicationRecord::NotReady);
                }
                Some(_) => {}
            }
            let evidence = open
                .result_evidence(view_id)
                .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
            evidence.record(row_id)
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Result(view_id))
                .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
                return Err(RelationalPublicationError::AnalysisLayerKindMismatch);
            };
            let RelationalResultLayerSnapshotState::Registered { evidence, .. } = result.state()
            else {
                return Ok(PublicationRecord::NotReady);
            };
            evidence.record(row_id)
        }
        _ => return Err(RelationalPublicationError::AnalysisCatalogStateMismatch),
    };
    let Some(record) = record else {
        return Ok(PublicationRecord::NotReady);
    };
    if record.row_id() != row_id {
        return Err(RelationalPublicationError::ResultEvidenceRowMismatch);
    }
    let values = record
        .early_select_iter()
        .map(|value| value.ok_or(RelationalPublicationError::ResultSelectNotRowLocal { view_id }))
        .collect::<Result<Vec<_>, _>>()?;
    let values = public_selected_values(select_names, values.into_iter())?;
    Ok(PublicationRecord::Emit(json!({
        "kind": "selected_case",
        "row_id": public_row_id(row_id),
        "values": values,
    })))
}

fn durable_projection_record(
    journal: &RelationalJournal,
    view_id: ViewId,
    select_names: &[Box<str>],
    source_ordinal: u128,
) -> Result<PublicationRecord, RelationalPublicationError> {
    let ordinal = usize::try_from(source_ordinal).map_err(|_| {
        RelationalPublicationError::SourceOrdinalOverflow {
            artifact: format!("view:{}", hex(view_id.bytes())),
            ordinal: source_ordinal,
        }
    })?;
    let Some(analysis) = journal.analysis_state() else {
        return Ok(PublicationRecord::NotReady);
    };
    match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            let status = open.layer_status(RelationalAnalysisLayerId::Result(view_id));
            if matches!(
                status,
                Some(RelationalAnalysisLayerStatus::ResultUnregistered) | None
            ) {
                return Ok(PublicationRecord::NotReady);
            }
            let projection = open
                .result_projection(view_id)
                .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
            if let Some(record) = projection.record(source_ordinal) {
                return public_projection_record(record, select_names);
            }
            if source_ordinal == projection.len() as u128 {
                let publication = open
                    .result_publication(view_id)
                    .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
                if let Some(publication) = publication {
                    return Ok(PublicationRecord::Emit(public_projection_closure(
                        projection.len() as u128,
                        projection.root().bytes(),
                        publication,
                    )));
                }
            }
            Ok(PublicationRecord::Exhausted)
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Result(view_id))
                .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
                return Err(RelationalPublicationError::AnalysisLayerKindMismatch);
            };
            let RelationalResultLayerSnapshotState::Registered {
                projection,
                publication,
                ..
            } = result.state()
            else {
                return Ok(PublicationRecord::NotReady);
            };
            if let Some(record) = projection.records().get(ordinal) {
                return public_projection_record(record, select_names);
            }
            if source_ordinal == projection.records().len() as u128 {
                if let Some(publication) = publication {
                    return Ok(PublicationRecord::Emit(public_projection_closure(
                        projection.records().len() as u128,
                        projection.root().bytes(),
                        *publication,
                    )));
                }
            }
            Ok(PublicationRecord::Exhausted)
        }
        _ => Err(RelationalPublicationError::AnalysisCatalogStateMismatch),
    }
}

fn public_projection_record(
    indexed: &IndexedResultProjectionRecord,
    select_names: &[Box<str>],
) -> Result<PublicationRecord, RelationalPublicationError> {
    match indexed.record() {
        ResultProjectionRecord::Row(row) => Ok(PublicationRecord::Emit(json!({
            "kind": "result_row",
            "row_id": public_row_id(row.row_id()),
            "values": public_selected_values(select_names, row.values().iter())?,
        }))),
        ResultProjectionRecord::Group(group) => match group.disposition() {
            ResultGroupDisposition::Provisional {
                currently_passes_having,
            } => match group.projected_values() {
                Some(values) => Ok(PublicationRecord::Emit(json!({
                    "kind": "result_group",
                    "disposition": "provisional",
                    "currently_passes_having": currently_passes_having,
                    "values": public_selected_values(select_names, values.iter())?,
                }))),
                // A grouped choice exposes only its selected rows. Group keys
                // and reducer state are not SELECT-authorized values.
                None => Ok(PublicationRecord::Skip),
            },
            ResultGroupDisposition::ExactExcluded => Ok(PublicationRecord::Skip),
            ResultGroupDisposition::ExactIncluded => match group.projected_values() {
                Some(values) => Ok(PublicationRecord::Emit(json!({
                    "kind": "result_group",
                    "disposition": "exact",
                    "values": public_selected_values(select_names, values.iter())?,
                }))),
                // A grouped choice publishes its selected rows as following
                // ChosenRow records. Group keys/reducer state are not SELECT.
                None => Ok(PublicationRecord::Skip),
            },
        },
        ResultProjectionRecord::ChosenRow { row, .. } => Ok(PublicationRecord::Emit(json!({
            "kind": "chosen_result_row",
            "row_id": public_row_id(row.row_id()),
            "values": public_selected_values(select_names, row.values().iter())?,
        }))),
    }
}

fn public_projection_closure(
    record_count: u128,
    projection_root: [u8; 32],
    publication: RelationalResultPublication,
) -> JsonValue {
    json!({
        "kind": "result_projection_closure",
        "projection_frontier": "exact",
        "record_count": record_count.to_string(),
        "projection_root": hex(projection_root),
        "publication_id": hex(publication.id().bytes()),
        "evidence_root": hex(publication.evidence_root().bytes()),
        "result_root": hex(publication.result_root().bytes()),
    })
}

fn public_mechanism_result_input_coverage(coverage: MechanismResultInputCoverage) -> JsonValue {
    json!({
        "request_id": hex(coverage.request_id.bytes()),
        "frontier": coverage.certainty_frontier(),
        "input_relation": "successful_incidences_only",
        "unavailable_rows_included": false,
        "target_cases": public_publication_count(
            coverage.target_cases,
            coverage.target_is_sealed,
        ),
        "incidence_cases": public_publication_count(
            coverage.incidence_cases,
            coverage.frontier_complete,
        ),
        "unavailable_cases": public_publication_count(
            coverage.unavailable_cases,
            coverage.frontier_complete,
        ),
    })
}

fn public_publication_count(value: u128, exact: bool) -> JsonValue {
    if exact {
        json!({
            "status": "exact",
            "value": value.to_string(),
        })
    } else {
        json!({
            "status": "lower_bound",
            "value": value.to_string(),
        })
    }
}

fn committed_mechanism_definitions_frontier(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    cursor: &PublicationCursor,
) -> Result<(u128, Option<String>), RelationalPublicationError> {
    let PublicationArtifactPlan::MechanismDefinitions {
        request_id,
        discovery_artifact_key,
        ..
    } = artifact
    else {
        return Err(RelationalPublicationError::CursorArtifactMismatch(
            artifact.key().into(),
        ));
    };
    let discovery_cursor = cursor
        .artifacts
        .get(discovery_artifact_key.as_ref())
        .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
    let ArtifactSourceCursor::MechanismDiscovery {
        event_ordinal,
        closure_emitted,
    } = discovery_cursor.source
    else {
        return Err(RelationalPublicationError::CursorArtifactMismatch(
            discovery_artifact_key.to_string(),
        ));
    };

    let Some(analysis) = journal.analysis_state() else {
        if event_ordinal != 0 || closure_emitted {
            return Err(
                RelationalPublicationError::MechanismSourceCoordinateMismatch {
                    artifact: discovery_artifact_key.to_string(),
                },
            );
        }
        return Ok((0, None));
    };
    let discovery = analysis
        .mechanism_publication_discovery(*request_id)
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let live_event_end = discovery.event_count() as u128;
    if event_ordinal > live_event_end {
        return Err(RelationalPublicationError::MechanismSourceCoordinateAhead {
            artifact: discovery_artifact_key.to_string(),
            event_ordinal,
            event_end: live_event_end,
        });
    }
    let committed_event_end = usize::try_from(event_ordinal)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let mut signature_end = 0_u128;
    for event_index in 0..committed_event_end {
        let event = discovery.event_at(event_index).ok_or(
            RelationalPublicationError::MechanismSourceCoordinateMismatch {
                artifact: discovery_artifact_key.to_string(),
            },
        )?;
        if matches!(event, MechanismPublicationDiscoveryEvent::Signature { .. }) {
            signature_end = signature_end
                .checked_add(1)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
        }
    }

    let closure_root = if closure_emitted {
        if event_ordinal != live_event_end {
            return Err(
                RelationalPublicationError::MechanismSourceCoordinateMismatch {
                    artifact: discovery_artifact_key.to_string(),
                },
            );
        }
        let closure = analysis.mechanism_closure(*request_id).ok_or(
            RelationalPublicationError::MechanismSourceCoordinateMismatch {
                artifact: discovery_artifact_key.to_string(),
            },
        )?;
        if closure.publication_event_end() != live_event_end {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
        Some(hex(closure.incidence_root().bytes()))
    } else {
        None
    };
    Ok((signature_end, closure_root))
}

fn pending_source_end(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
) -> Result<PendingArtifactSourceEnd, RelationalPublicationError> {
    let request_id = match artifact {
        PublicationArtifactPlan::Mechanism { request_id, .. } => *request_id,
        PublicationArtifactPlan::MechanismDefinitions { .. } => {
            let (signature_end, closure_root) =
                committed_mechanism_definitions_frontier(artifact, journal, cursor)?;
            return Ok(PendingArtifactSourceEnd::MechanismDefinitions {
                signature_end,
                closure_root,
            });
        }
        PublicationArtifactPlan::MechanismStructuralDefinitions { request_id, .. } => {
            let Some(authority) = structural_definition_catalog_authority(journal, *request_id)?
            else {
                return Ok(PendingArtifactSourceEnd::StructuralDefinitions {
                    definition_end: 0,
                    structural_quotient_root: None,
                    definition_catalog_root: None,
                });
            };
            return Ok(PendingArtifactSourceEnd::StructuralDefinitions {
                definition_end: authority.definition_count,
                structural_quotient_root: Some(hex(authority.closure.root().bytes())),
                definition_catalog_root: Some(hex(authority.definition_catalog_root.bytes())),
            });
        }
        PublicationArtifactPlan::SubjectStarters {
            consumer_id,
            request_id,
            target,
            subject,
            authorization,
            transition_schemas,
            ..
        } => {
            let identity =
                SubjectStarterCursorIdentity::new(*consumer_id, *request_id, *target, *subject);
            let Some(authority) =
                subject_starter_publication_authority(journal, *request_id, *target, *subject)?
            else {
                return Ok(PendingArtifactSourceEnd::SubjectStarters {
                    identity,
                    structural_quotient_root: None,
                    mechanism_support_root: None,
                    projection_plan_id: None,
                    projection_job_id: None,
                });
            };
            let job = subject_starter_projection_job(
                journal,
                &authority,
                transition_schemas,
                authorization,
            )?;
            return Ok(PendingArtifactSourceEnd::SubjectStarters {
                identity,
                structural_quotient_root: Some(CursorDigest::new(
                    authority.structural_closure.root().bytes(),
                )),
                mechanism_support_root: Some(CursorDigest::new(
                    authority.support_closure.root().bytes(),
                )),
                projection_plan_id: Some(CursorDigest::new(
                    authority.key_authority.projection_plan_id().bytes(),
                )),
                projection_job_id: Some(CursorDigest::new(job.id().bytes())),
            });
        }
        PublicationArtifactPlan::Result { .. }
        | PublicationArtifactPlan::MechanismStructural { .. }
        | PublicationArtifactPlan::CaseSupport { .. }
        | PublicationArtifactPlan::CaseTransitions { .. } => {
            let source_end = available_source_record_count(artifact, journal, ordinal_index)?
                .ok_or(RelationalPublicationError::PendingCursorMismatch)?;
            return Ok(PendingArtifactSourceEnd::Flat { source_end });
        }
    };
    let Some(analysis) = journal.analysis_state() else {
        return Ok(PendingArtifactSourceEnd::MechanismDiscovery {
            event_end: 0,
            closure_root: None,
        });
    };
    let discovery = analysis
        .mechanism_publication_discovery(request_id)
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let live_event_end = discovery.event_count() as u128;
    let (event_end, closure_root) = match analysis.mechanism_closure(request_id) {
        Some(closure) => {
            if closure.publication_event_end() != live_event_end {
                return Err(RelationalPublicationError::PendingCursorMismatch);
            }
            (
                closure.publication_event_end(),
                Some(hex(closure.incidence_root().bytes())),
            )
        }
        None => (live_event_end, None),
    };
    Ok(PendingArtifactSourceEnd::MechanismDiscovery {
        event_end,
        closure_root,
    })
}

#[allow(clippy::too_many_arguments)]
fn mechanism_discovery_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    request_id: MechanismRequestId,
    event_ordinal: u128,
    closure_emitted: bool,
    source_end: Option<&PendingArtifactSourceEnd>,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    let Some(analysis) = journal.analysis_state() else {
        let frozen = match source_end {
            Some(PendingArtifactSourceEnd::MechanismDiscovery {
                event_end: 0,
                closure_root: None,
            }) => true,
            Some(_) => return Err(RelationalPublicationError::PendingCursorMismatch),
            None => false,
        };
        return if event_ordinal == 0 && !closure_emitted {
            if frozen {
                Ok(AddressedPublicationRecord::Exhausted)
            } else {
                Ok(AddressedPublicationRecord::NotReady)
            }
        } else {
            Err(RelationalPublicationError::MechanismSourceCoordinateAhead {
                artifact: artifact.key().into(),
                event_ordinal,
                event_end: 0,
            })
        };
    };
    let discovery = analysis
        .mechanism_publication_discovery(request_id)
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let live_event_end = discovery.event_count() as u128;
    let live_closure = analysis.mechanism_closure(request_id);
    if live_closure.is_some_and(|closure| closure.publication_event_end() != live_event_end) {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    let event_end = match source_end {
        Some(PendingArtifactSourceEnd::MechanismDiscovery { event_end, .. }) => {
            if live_event_end < *event_end {
                return Err(RelationalPublicationError::PendingCursorMismatch);
            }
            *event_end
        }
        Some(
            PendingArtifactSourceEnd::Flat { .. }
            | PendingArtifactSourceEnd::MechanismDefinitions { .. }
            | PendingArtifactSourceEnd::StructuralDefinitions { .. }
            | PendingArtifactSourceEnd::SubjectStarters { .. },
        ) => return Err(RelationalPublicationError::PendingCursorMismatch),
        None => live_event_end,
    };

    // Keep the owned live-root string alive for the remainder of this call.
    let live_closure_root = live_closure.map(|closure| hex(closure.incidence_root().bytes()));
    let authorized_closure_root = match source_end {
        Some(PendingArtifactSourceEnd::MechanismDiscovery { closure_root, .. }) => {
            closure_root.as_deref()
        }
        Some(
            PendingArtifactSourceEnd::Flat { .. }
            | PendingArtifactSourceEnd::MechanismDefinitions { .. }
            | PendingArtifactSourceEnd::StructuralDefinitions { .. }
            | PendingArtifactSourceEnd::SubjectStarters { .. },
        ) => return Err(RelationalPublicationError::PendingCursorMismatch),
        None => live_closure_root.as_deref(),
    };
    if authorized_closure_root.is_some() {
        if live_event_end != event_end {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
        validate_authorized_mechanism_closure(live_closure, authorized_closure_root)?;
    }

    if event_ordinal > event_end {
        return Err(RelationalPublicationError::MechanismSourceCoordinateAhead {
            artifact: artifact.key().into(),
            event_ordinal,
            event_end,
        });
    }
    if closure_emitted {
        if event_ordinal != event_end || authorized_closure_root.is_none() {
            return Err(
                RelationalPublicationError::MechanismSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        validate_authorized_mechanism_closure(live_closure, authorized_closure_root)?;
        return Ok(AddressedPublicationRecord::Exhausted);
    }
    if event_ordinal == event_end {
        let Some(authorized_root) = authorized_closure_root else {
            return Ok(AddressedPublicationRecord::NotReady);
        };
        let closure = validate_authorized_mechanism_closure(live_closure, Some(authorized_root))?;
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::MechanismClosure { event_end },
            next: ArtifactSourceCursor::MechanismDiscovery {
                event_ordinal,
                closure_emitted: true,
            },
            value: public_mechanism_closure(request_id, closure),
        });
    }

    let event_index = usize::try_from(event_ordinal)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let event = discovery.event_at(event_index).ok_or(
        RelationalPublicationError::MechanismSourceCoordinateMismatch {
            artifact: artifact.key().into(),
        },
    )?;
    let coordinate = PublicationSourceCoordinate::MechanismEvent { event_ordinal };
    match event {
        MechanismPublicationDiscoveryEvent::Signature { signature_id } => {
            let PublicationArtifactPlan::Mechanism {
                definitions_artifact_key,
                definitions_artifact_path,
                ..
            } = artifact
            else {
                return Err(RelationalPublicationError::CursorArtifactMismatch(
                    artifact.key().into(),
                ));
            };
            let (definition, scope) =
                mechanism_signature_definition(journal, request_id, signature_id)?;
            let publication_index =
                ordinal_index.mechanism_definition(request_id, definition, scope)?;
            Ok(AddressedPublicationRecord::Emit {
                coordinate,
                next: next_mechanism_event(event_ordinal)?,
                value: public_signature_descriptor(
                    definition,
                    publication_index,
                    definitions_artifact_key,
                    definitions_artifact_path,
                ),
            })
        }
        MechanismPublicationDiscoveryEvent::UnavailableReason { reason_id } => {
            let definition = mechanism_unavailable_reason(journal, request_id, reason_id)?;
            Ok(AddressedPublicationRecord::Emit {
                coordinate,
                next: next_mechanism_event(event_ordinal)?,
                value: public_unavailable_reason(definition)?,
            })
        }
        MechanismPublicationDiscoveryEvent::Terminal(terminal) => {
            Ok(AddressedPublicationRecord::Emit {
                coordinate,
                next: next_mechanism_event(event_ordinal)?,
                value: public_mechanism_terminal(terminal),
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn mechanism_definition_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    request_id: MechanismRequestId,
    signature_ordinal: u128,
    definition_part_ordinal: u128,
    closure_emitted: bool,
    cursor: &PublicationCursor,
    source_end: Option<&PendingArtifactSourceEnd>,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    let (signature_end, closure_root) = match source_end {
        Some(PendingArtifactSourceEnd::MechanismDefinitions {
            signature_end,
            closure_root,
        }) => (*signature_end, closure_root.clone()),
        Some(
            PendingArtifactSourceEnd::Flat { .. }
            | PendingArtifactSourceEnd::MechanismDiscovery { .. }
            | PendingArtifactSourceEnd::StructuralDefinitions { .. }
            | PendingArtifactSourceEnd::SubjectStarters { .. },
        ) => return Err(RelationalPublicationError::PendingCursorMismatch),
        None => committed_mechanism_definitions_frontier(artifact, journal, cursor)?,
    };
    let live_signature_end = journal.analysis_state().map_or(Ok(0), |analysis| {
        analysis
            .mechanism_publication_discovery(request_id)
            .map(|discovery| discovery.signature_count() as u128)
            .ok_or(RelationalPublicationError::MissingAnalysisLayer)
    })?;
    if signature_end > live_signature_end {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    if let Some(authorized_root) = closure_root.as_deref() {
        let analysis = journal
            .analysis_state()
            .ok_or(RelationalPublicationError::PendingCursorMismatch)?;
        if signature_end != live_signature_end {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
        validate_authorized_mechanism_closure(
            analysis.mechanism_closure(request_id),
            Some(authorized_root),
        )?;
    }
    if closure_emitted {
        if signature_ordinal != signature_end
            || definition_part_ordinal != 0
            || closure_root.is_none()
        {
            return Err(
                RelationalPublicationError::MechanismSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        return Ok(AddressedPublicationRecord::Exhausted);
    }
    if signature_ordinal > signature_end {
        return Err(
            RelationalPublicationError::MechanismDefinitionSourceCoordinateAhead {
                artifact: artifact.key().into(),
                signature_ordinal,
                signature_end,
            },
        );
    }
    if signature_ordinal == signature_end {
        if definition_part_ordinal != 0 {
            return Err(
                RelationalPublicationError::MechanismSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        let Some(incidence_root) = closure_root.as_deref() else {
            return Ok(AddressedPublicationRecord::NotReady);
        };
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::MechanismDefinitionsClosure { signature_end },
            next: ArtifactSourceCursor::MechanismDefinitions {
                signature_ordinal,
                definition_part_ordinal,
                closure_emitted: true,
            },
            value: public_mechanism_definitions_closure(request_id, signature_end, incidence_root),
        });
    }

    let analysis = journal
        .analysis_state()
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let discovery = analysis
        .mechanism_publication_discovery(request_id)
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let signature_index = usize::try_from(signature_ordinal)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let signature_id = discovery
        .signature_at(signature_index)
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    let (definition, _) = mechanism_signature_definition(journal, request_id, signature_id)?;
    let payload_index = ordinal_index.mechanism_definition_payload(request_id, definition)?;
    let part_count = payload_index.part_count()?;
    if definition_part_ordinal >= part_count {
        return Err(
            RelationalPublicationError::MechanismSourceCoordinateMismatch {
                artifact: artifact.key().into(),
            },
        );
    }
    let value = if definition_part_ordinal == 0 {
        public_signature_definition_payload_header(definition, payload_index)
    } else if definition_part_ordinal <= payload_index.chunk_count {
        public_signature_definition_chunk(
            definition,
            definition_part_ordinal,
            definition_part_ordinal - 1,
        )?
    } else {
        public_signature_definition_complete(definition, payload_index, definition_part_ordinal)
    };
    let next_part = definition_part_ordinal
        .checked_add(1)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let next = if next_part == part_count {
        ArtifactSourceCursor::MechanismDefinitions {
            signature_ordinal: signature_ordinal
                .checked_add(1)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
            definition_part_ordinal: 0,
            closure_emitted: false,
        }
    } else {
        ArtifactSourceCursor::MechanismDefinitions {
            signature_ordinal,
            definition_part_ordinal: next_part,
            closure_emitted: false,
        }
    };
    Ok(AddressedPublicationRecord::Emit {
        coordinate: PublicationSourceCoordinate::MechanismDefinition {
            signature_ordinal,
            definition_part_ordinal,
        },
        next,
        value,
    })
}

#[allow(clippy::too_many_arguments)]
fn structural_definition_record(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    request_id: MechanismRequestId,
    header_emitted: bool,
    definition_ordinal: u128,
    definition_part_ordinal: u128,
    closure_emitted: bool,
    source_end: Option<&PendingArtifactSourceEnd>,
) -> Result<AddressedPublicationRecord, RelationalPublicationError> {
    let live = structural_definition_catalog_authority(journal, request_id)?;
    let (definition_end, structural_root, definition_root) = match source_end {
        Some(PendingArtifactSourceEnd::StructuralDefinitions {
            definition_end,
            structural_quotient_root,
            definition_catalog_root,
        }) => (
            *definition_end,
            structural_quotient_root.clone(),
            definition_catalog_root.clone(),
        ),
        Some(
            PendingArtifactSourceEnd::Flat { .. }
            | PendingArtifactSourceEnd::MechanismDiscovery { .. }
            | PendingArtifactSourceEnd::MechanismDefinitions { .. }
            | PendingArtifactSourceEnd::SubjectStarters { .. },
        ) => return Err(RelationalPublicationError::PendingCursorMismatch),
        None => live.map_or((0, None, None), |authority| {
            (
                authority.definition_count,
                Some(hex(authority.closure.root().bytes())),
                Some(hex(authority.definition_catalog_root.bytes())),
            )
        }),
    };
    let authorized_roots = match (structural_root.as_deref(), definition_root.as_deref()) {
        (None, None) if definition_end == 0 => None,
        (Some(structural_root), Some(definition_root)) => Some((structural_root, definition_root)),
        _ => return Err(RelationalPublicationError::PendingCursorMismatch),
    };
    let Some(authority) = live else {
        if authorized_roots.is_some()
            || header_emitted
            || definition_ordinal != 0
            || definition_part_ordinal != 0
            || closure_emitted
        {
            return Err(RelationalPublicationError::PendingCursorMismatch);
        }
        return Ok(AddressedPublicationRecord::NotReady);
    };
    let live_structural_root = hex(authority.closure.root().bytes());
    let live_definition_root = hex(authority.definition_catalog_root.bytes());
    let Some((authorized_structural_root, authorized_definition_root)) = authorized_roots else {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    };
    if definition_end != authority.definition_count
        || authorized_structural_root != live_structural_root
        || authorized_definition_root != live_definition_root
    {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    if closure_emitted {
        if !header_emitted || definition_ordinal != definition_end || definition_part_ordinal != 0 {
            return Err(
                RelationalPublicationError::StructuralDefinitionSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        return Ok(AddressedPublicationRecord::Exhausted);
    }
    if !header_emitted {
        if definition_ordinal != 0 || definition_part_ordinal != 0 {
            return Err(
                RelationalPublicationError::StructuralDefinitionSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::StructuralDefinitionsHeader,
            next: ArtifactSourceCursor::StructuralDefinitions {
                header_emitted: true,
                definition_ordinal: 0,
                definition_part_ordinal: 0,
                closure_emitted: false,
            },
            value: public_structural_definition_catalog_header(request_id, authority),
        });
    }
    if definition_ordinal > definition_end {
        return Err(
            RelationalPublicationError::StructuralDefinitionSourceCoordinateAhead {
                artifact: artifact.key().into(),
                definition_ordinal,
                definition_end,
            },
        );
    }
    if definition_ordinal == definition_end {
        if definition_part_ordinal != 0 {
            return Err(
                RelationalPublicationError::StructuralDefinitionSourceCoordinateMismatch {
                    artifact: artifact.key().into(),
                },
            );
        }
        return Ok(AddressedPublicationRecord::Emit {
            coordinate: PublicationSourceCoordinate::StructuralDefinitionsClosure {
                definition_end,
            },
            next: ArtifactSourceCursor::StructuralDefinitions {
                header_emitted: true,
                definition_ordinal,
                definition_part_ordinal,
                closure_emitted: true,
            },
            value: public_structural_definition_catalog_closure(request_id, authority),
        });
    }

    let definition_index = usize::try_from(definition_ordinal)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let definition = authority
        .catalog
        .canonical_definition_at(definition_index)
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    let part_count = structural_definition_part_count(definition)?;
    if definition_part_ordinal >= part_count {
        return Err(
            RelationalPublicationError::StructuralDefinitionChunkOutOfRange {
                definition_id: hex(definition.id_bytes()),
                part_ordinal: definition_part_ordinal,
            },
        );
    }
    let value = if definition_part_ordinal == 0 {
        public_structural_definition_header(definition_ordinal, definition)?
    } else {
        public_structural_definition_chunk(definition_ordinal, definition_part_ordinal, definition)?
    };
    let next_part = definition_part_ordinal
        .checked_add(1)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let next = if next_part == part_count {
        ArtifactSourceCursor::StructuralDefinitions {
            header_emitted: true,
            definition_ordinal: definition_ordinal
                .checked_add(1)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
            definition_part_ordinal: 0,
            closure_emitted: false,
        }
    } else {
        ArtifactSourceCursor::StructuralDefinitions {
            header_emitted: true,
            definition_ordinal,
            definition_part_ordinal: next_part,
            closure_emitted: false,
        }
    };
    Ok(AddressedPublicationRecord::Emit {
        coordinate: PublicationSourceCoordinate::StructuralDefinition {
            definition_ordinal,
            definition_part_ordinal,
        },
        next,
        value,
    })
}

fn structural_definition_part_count(
    definition: StructuralDefinitionRef<'_>,
) -> Result<u128, RelationalPublicationError> {
    match definition {
        StructuralDefinitionRef::Frame(_)
        | StructuralDefinitionRef::ActivationContext(_)
        | StructuralDefinitionRef::Edge(_) => Ok(1),
        StructuralDefinitionRef::Node(definition) => structural_definition_lane_part_count(&[
            definition.before_dependencies().len(),
            definition.after_dependencies().len(),
        ]),
        StructuralDefinitionRef::Mechanism(definition) => structural_definition_lane_part_count(&[
            definition.frames().len(),
            definition.activation_contexts().len(),
            definition.nodes().len(),
            definition.edges().len(),
            definition.context_inventory().len(),
            definition.before_roots().len(),
            definition.after_roots().len(),
            definition.ownership().len(),
        ]),
        StructuralDefinitionRef::ExecutionProfile(definition) => {
            structural_definition_lane_part_count(&[
                definition.frames().len(),
                definition.activation_contexts().len(),
                definition.frame_counts().len(),
                definition.context_counts().len(),
                definition.activation_root_counts().len(),
                definition.activation_call_counts().len(),
                definition.node_counts().len(),
                definition.node_root_counts().len(),
                definition.edge_counts().len(),
                definition.ownership_counts().len(),
            ])
        }
    }
}

fn structural_definition_lane_part_count(
    lane_lengths: &[usize],
) -> Result<u128, RelationalPublicationError> {
    lane_lengths.iter().try_fold(1u128, |count, length| {
        count
            .checked_add(structural_definition_chunk_count(*length))
            .ok_or(RelationalPublicationError::ArithmeticOverflow)
    })
}

fn structural_definition_chunk_count(length: usize) -> u128 {
    if length == 0 {
        0
    } else {
        ((length - 1) / STRUCTURAL_DEFINITION_CHUNK_ITEMS + 1) as u128
    }
}

#[derive(Clone, Copy)]
struct StructuralDefinitionChunkWindow {
    lane: &'static str,
    chunk_ordinal: u128,
    item_start: usize,
    item_end: usize,
}

fn structural_definition_chunk_window(
    part_ordinal: u128,
    lanes: &[(&'static str, usize)],
) -> Result<StructuralDefinitionChunkWindow, RelationalPublicationError> {
    let mut remaining = part_ordinal
        .checked_sub(1)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    for (lane, length) in lanes {
        let chunks = structural_definition_chunk_count(*length);
        if remaining < chunks {
            let chunk_index = usize::try_from(remaining)
                .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
            let item_start = chunk_index
                .checked_mul(STRUCTURAL_DEFINITION_CHUNK_ITEMS)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
            let item_end = item_start
                .checked_add(STRUCTURAL_DEFINITION_CHUNK_ITEMS)
                .ok_or(RelationalPublicationError::ArithmeticOverflow)?
                .min(*length);
            return Ok(StructuralDefinitionChunkWindow {
                lane,
                chunk_ordinal: remaining,
                item_start,
                item_end,
            });
        }
        remaining = remaining
            .checked_sub(chunks)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    }
    Err(RelationalPublicationError::ArithmeticOverflow)
}

fn public_structural_definition_catalog_header(
    request_id: MechanismRequestId,
    authority: StructuralDefinitionCatalogAuthority<'_>,
) -> JsonValue {
    json!({
        "kind": "structural_definition_catalog_header",
        "catalog_version": STRUCTURAL_DEFINITION_CATALOG_VERSION,
        "quotient_version": STRUCTURAL_MECHANISM_QUOTIENT_VERSION,
        "request_id": hex(request_id.bytes()),
        "structural_quotient_root": hex(authority.closure.root().bytes()),
        "structural_definition_catalog_root": hex(authority.definition_catalog_root.bytes()),
        "catalog_membership_root": hex(authority.closure.catalog_membership_root().bytes()),
        "definition_count": authority.definition_count.to_string(),
        "section_order": StructuralDefinitionKind::CANONICAL_ORDER
            .iter()
            .map(|kind| kind.code())
            .collect::<Vec<_>>(),
        "section_counts": public_structural_definition_section_counts(authority.closure),
        "chunk_item_limit": STRUCTURAL_DEFINITION_CHUNK_ITEMS.to_string(),
        "contains_raw_signatures": false,
        "contains_cases": false,
        "contains_starter_values": false,
    })
}

fn public_structural_definition_catalog_closure(
    request_id: MechanismRequestId,
    authority: StructuralDefinitionCatalogAuthority<'_>,
) -> JsonValue {
    json!({
        "kind": "structural_definition_catalog_closure",
        "catalog_version": STRUCTURAL_DEFINITION_CATALOG_VERSION,
        "request_id": hex(request_id.bytes()),
        "definition_count": authority.definition_count.to_string(),
        "section_counts": public_structural_definition_section_counts(authority.closure),
        "structural_assignment_count": authority.closure.counts().assignments().to_string(),
        "catalog_membership_root": hex(authority.closure.catalog_membership_root().bytes()),
        "structural_quotient_root": hex(authority.closure.root().bytes()),
        "structural_definition_catalog_root": hex(authority.definition_catalog_root.bytes()),
    })
}

fn public_structural_definition_section_counts(
    closure: StructuralQuotientClosureReceipt,
) -> JsonValue {
    let counts = closure.counts();
    json!({
        "frames": counts.frames().to_string(),
        "activation_contexts": counts.activation_contexts().to_string(),
        "nodes": counts.nodes().to_string(),
        "edges": counts.edges().to_string(),
        "mechanisms": counts.mechanisms().to_string(),
        "execution_profiles": counts.execution_profiles().to_string(),
    })
}

fn public_structural_lane(length: usize) -> JsonValue {
    json!({
        "items": length.to_string(),
        "chunks": structural_definition_chunk_count(length).to_string(),
    })
}

fn public_structural_definition_header(
    definition_ordinal: u128,
    definition: StructuralDefinitionRef<'_>,
) -> Result<JsonValue, RelationalPublicationError> {
    Ok(match definition {
        StructuralDefinitionRef::Frame(definition) => json!({
            "kind": "structural_frame_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::Frame.code(),
            "frame_id": hex(definition.id().bytes()),
            "call_site": public_structural_site(definition.call_site()),
            "callee": public_structural_callee(definition.callee()),
            "part_count": "1",
        }),
        StructuralDefinitionRef::ActivationContext(definition) => json!({
            "kind": "structural_activation_context_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::ActivationContext.code(),
            "activation_context_id": hex(definition.id().bytes()),
            "parent_activation_context_id": definition.parent().map(|id| hex(id.bytes())),
            "frame_id": hex(definition.frame().bytes()),
            "part_count": "1",
        }),
        StructuralDefinitionRef::Node(definition) => json!({
            "kind": "structural_node_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::Node.code(),
            "node_id": hex(definition.id().bytes()),
            "owner_frame_id": hex(definition.owner_frame().bytes()),
            "site": public_structural_site(definition.site()),
            "event_kind": definition.kind().code(),
            "before_outcome": definition.before_outcome().map(public_structural_event_outcome),
            "after_outcome": definition.after_outcome().map(public_structural_event_outcome),
            "lanes": {
                "before_dependencies": public_structural_lane(definition.before_dependencies().len()),
                "after_dependencies": public_structural_lane(definition.after_dependencies().len()),
            },
            "part_count": structural_definition_part_count(
                StructuralDefinitionRef::Node(definition)
            )?.to_string(),
        }),
        StructuralDefinitionRef::Edge(definition) => json!({
            "kind": "structural_edge_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::Edge.code(),
            "edge_id": hex(definition.id().bytes()),
            "endpoint": definition.endpoint().code(),
            "dependent_node_id": hex(definition.dependent().bytes()),
            "dependency_node_id": hex(definition.dependency().bytes()),
            "part_count": "1",
        }),
        StructuralDefinitionRef::Mechanism(definition) => json!({
            "kind": "structural_mechanism_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::Mechanism.code(),
            "structural_mechanism_id": hex(definition.id().bytes()),
            "lanes": {
                "frames": public_structural_lane(definition.frames().len()),
                "activation_contexts": public_structural_lane(definition.activation_contexts().len()),
                "nodes": public_structural_lane(definition.nodes().len()),
                "edges": public_structural_lane(definition.edges().len()),
                "context_inventory": public_structural_lane(definition.context_inventory().len()),
                "before_roots": public_structural_lane(definition.before_roots().len()),
                "after_roots": public_structural_lane(definition.after_roots().len()),
                "ownership": public_structural_lane(definition.ownership().len()),
            },
            "part_count": structural_definition_part_count(
                StructuralDefinitionRef::Mechanism(definition)
            )?.to_string(),
        }),
        StructuralDefinitionRef::ExecutionProfile(definition) => json!({
            "kind": "structural_execution_profile_definition",
            "definition_ordinal": definition_ordinal.to_string(),
            "definition_kind": StructuralDefinitionKind::ExecutionProfile.code(),
            "execution_profile_id": hex(definition.id().bytes()),
            "structural_mechanism_id": hex(definition.mechanism_id().bytes()),
            "before_totals": public_structural_execution_totals(definition.before_totals()),
            "after_totals": public_structural_execution_totals(definition.after_totals()),
            "lanes": {
                "frames": public_structural_lane(definition.frames().len()),
                "activation_contexts": public_structural_lane(definition.activation_contexts().len()),
                "frame_counts": public_structural_lane(definition.frame_counts().len()),
                "context_counts": public_structural_lane(definition.context_counts().len()),
                "activation_root_counts": public_structural_lane(definition.activation_root_counts().len()),
                "activation_call_counts": public_structural_lane(definition.activation_call_counts().len()),
                "node_counts": public_structural_lane(definition.node_counts().len()),
                "node_root_counts": public_structural_lane(definition.node_root_counts().len()),
                "edge_counts": public_structural_lane(definition.edge_counts().len()),
                "ownership_counts": public_structural_lane(definition.ownership_counts().len()),
            },
            "part_count": structural_definition_part_count(
                StructuralDefinitionRef::ExecutionProfile(definition)
            )?.to_string(),
        }),
    })
}

fn public_structural_site(site: &RelationalMechanismSiteId) -> JsonValue {
    json!({
        "kind": site.kind().code(),
        "digest": hex(site.digest_bytes()),
    })
}

fn public_structural_callee(callee: &RelationalMechanismCalleeId) -> JsonValue {
    json!({
        "kind": callee.code(),
        "site": public_structural_site(callee.site()),
    })
}

fn public_structural_event_outcome(outcome: &RelationalMechanismEventOutcome) -> JsonValue {
    match outcome {
        RelationalMechanismEventOutcome::RuleAttempt(outcome) => json!({
            "kind": "rule_attempt",
            "outcome": match outcome {
                RelationalRuleAttemptOutcome::HeadMismatch => "head_mismatch",
                RelationalRuleAttemptOutcome::GuardFalse => "guard_false",
                RelationalRuleAttemptOutcome::BodyFalse => "body_false",
                RelationalRuleAttemptOutcome::Applicable => "applicable",
            },
        }),
        RelationalMechanismEventOutcome::RuleSelection(outcome) => match outcome {
            RelationalRuleSelectionOutcome::NoApplicableRule => json!({
                "kind": "rule_selection",
                "outcome": "no_applicable_rule",
                "selected_site": null,
            }),
            RelationalRuleSelectionOutcome::Selected(site) => json!({
                "kind": "rule_selection",
                "outcome": "selected",
                "selected_site": public_structural_site(site),
            }),
        },
        RelationalMechanismEventOutcome::IfDecision(outcome) => json!({
            "kind": "if_decision",
            "outcome": match outcome {
                RelationalIfDecisionOutcome::Then => "then",
                RelationalIfDecisionOutcome::Else => "else",
            },
        }),
        RelationalMechanismEventOutcome::MatchDecision { arm_index } => json!({
            "kind": "match_decision",
            "arm_index": arm_index.to_string(),
        }),
        RelationalMechanismEventOutcome::ShortCircuit(outcome) => match outcome {
            RelationalShortCircuitOutcome::SkippedRight { result } => json!({
                "kind": "short_circuit",
                "outcome": "skipped_right",
                "result": result,
            }),
            RelationalShortCircuitOutcome::EvaluatedRight { result } => json!({
                "kind": "short_circuit",
                "outcome": "evaluated_right",
                "result": result,
            }),
        },
    }
}

fn public_structural_execution_totals(totals: StructuralEndpointExecutionTotals) -> JsonValue {
    json!({
        "activation_nodes": totals.activation_nodes().to_string(),
        "activation_roots": totals.activation_roots().to_string(),
        "activation_edges": totals.activation_edges().to_string(),
        "event_nodes": totals.event_nodes().to_string(),
        "event_roots": totals.event_roots().to_string(),
        "event_edges": totals.event_edges().to_string(),
        "ownership_occurrences": totals.ownership_occurrences().to_string(),
    })
}

fn public_structural_definition_chunk(
    definition_ordinal: u128,
    part_ordinal: u128,
    definition: StructuralDefinitionRef<'_>,
) -> Result<JsonValue, RelationalPublicationError> {
    let (window, items) = match definition {
        StructuralDefinitionRef::Node(definition) => {
            let lanes = [
                (
                    "before_dependencies",
                    definition.before_dependencies().len(),
                ),
                ("after_dependencies", definition.after_dependencies().len()),
            ];
            let window = structural_definition_chunk_window(part_ordinal, &lanes)?;
            let ids = match window.lane {
                "before_dependencies" => definition.before_dependencies(),
                "after_dependencies" => definition.after_dependencies(),
                _ => return Err(RelationalPublicationError::ArithmeticOverflow),
            };
            let items = ids[window.item_start..window.item_end]
                .iter()
                .map(|id| JsonValue::String(hex(id.bytes())))
                .collect::<Vec<_>>();
            (window, items)
        }
        StructuralDefinitionRef::Mechanism(definition) => {
            let lanes = [
                ("frames", definition.frames().len()),
                (
                    "activation_contexts",
                    definition.activation_contexts().len(),
                ),
                ("nodes", definition.nodes().len()),
                ("edges", definition.edges().len()),
                ("context_inventory", definition.context_inventory().len()),
                ("before_roots", definition.before_roots().len()),
                ("after_roots", definition.after_roots().len()),
                ("ownership", definition.ownership().len()),
            ];
            let window = structural_definition_chunk_window(part_ordinal, &lanes)?;
            let items = match window.lane {
                "frames" => definition.frames()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "activation_contexts" => definition.activation_contexts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "nodes" => definition.nodes()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "edges" => definition.edges()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "context_inventory" => definition.context_inventory()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "activation_context_id": hex(item.context().bytes()),
                        })
                    })
                    .collect(),
                "before_roots" => definition.before_roots()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.bytes())))
                    .collect(),
                "after_roots" => definition.after_roots()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.bytes())))
                    .collect(),
                "ownership" => definition.ownership()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "node_id": hex(item.node().bytes()),
                            "activation_context_id": hex(item.context().bytes()),
                        })
                    })
                    .collect(),
                _ => return Err(RelationalPublicationError::ArithmeticOverflow),
            };
            (window, items)
        }
        StructuralDefinitionRef::ExecutionProfile(definition) => {
            let lanes = [
                ("frames", definition.frames().len()),
                (
                    "activation_contexts",
                    definition.activation_contexts().len(),
                ),
                ("frame_counts", definition.frame_counts().len()),
                ("context_counts", definition.context_counts().len()),
                (
                    "activation_root_counts",
                    definition.activation_root_counts().len(),
                ),
                (
                    "activation_call_counts",
                    definition.activation_call_counts().len(),
                ),
                ("node_counts", definition.node_counts().len()),
                ("node_root_counts", definition.node_root_counts().len()),
                ("edge_counts", definition.edge_counts().len()),
                ("ownership_counts", definition.ownership_counts().len()),
            ];
            let window = structural_definition_chunk_window(part_ordinal, &lanes)?;
            let items = match window.lane {
                "frames" => definition.frames()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "activation_contexts" => definition.activation_contexts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| JsonValue::String(hex(item.id().bytes())))
                    .collect(),
                "frame_counts" => definition.frame_counts()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "frame_id": hex(item.frame().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "context_counts" => definition.context_counts()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "activation_context_id": hex(item.context().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "activation_root_counts" => definition.activation_root_counts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "activation_context_id": hex(item.context().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "activation_call_counts" => definition.activation_call_counts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "parent_activation_context_id": hex(item.parent().bytes()),
                            "child_activation_context_id": hex(item.child().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "node_counts" => definition.node_counts()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "node_id": hex(item.node().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "node_root_counts" => definition.node_root_counts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "node_id": hex(item.node().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "edge_counts" => definition.edge_counts()[window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "edge_id": hex(item.edge().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                "ownership_counts" => definition.ownership_counts()
                    [window.item_start..window.item_end]
                    .iter()
                    .map(|item| {
                        json!({
                            "endpoint": item.endpoint().code(),
                            "node_id": hex(item.node().bytes()),
                            "activation_context_id": hex(item.context().bytes()),
                            "count": item.count().to_string(),
                        })
                    })
                    .collect(),
                _ => return Err(RelationalPublicationError::ArithmeticOverflow),
            };
            (window, items)
        }
        StructuralDefinitionRef::Frame(_)
        | StructuralDefinitionRef::ActivationContext(_)
        | StructuralDefinitionRef::Edge(_) => {
            return Err(
                RelationalPublicationError::StructuralDefinitionChunkOutOfRange {
                    definition_id: hex(definition.id_bytes()),
                    part_ordinal,
                },
            );
        }
    };
    if items.len() > STRUCTURAL_DEFINITION_CHUNK_ITEMS
        || items.len() != window.item_end.saturating_sub(window.item_start)
    {
        return Err(RelationalPublicationError::ArithmeticOverflow);
    }
    Ok(json!({
        "kind": "structural_definition_lane_chunk",
        "definition_ordinal": definition_ordinal.to_string(),
        "definition_part_ordinal": part_ordinal.to_string(),
        "definition_kind": definition.kind().code(),
        "definition_id": hex(definition.id_bytes()),
        "lane": window.lane,
        "chunk_ordinal": window.chunk_ordinal.to_string(),
        "item_start": window.item_start.to_string(),
        "item_end": window.item_end.to_string(),
        "items": items,
    }))
}

fn next_mechanism_event(
    event_ordinal: u128,
) -> Result<ArtifactSourceCursor, RelationalPublicationError> {
    Ok(ArtifactSourceCursor::MechanismDiscovery {
        event_ordinal: event_ordinal
            .checked_add(1)
            .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
        closure_emitted: false,
    })
}

fn validate_authorized_mechanism_closure(
    live: Option<RelationalMechanismClosureReceipt>,
    authorized_root: Option<&str>,
) -> Result<RelationalMechanismClosureReceipt, RelationalPublicationError> {
    let closure = live.ok_or(RelationalPublicationError::PendingCursorMismatch)?;
    let live_root = hex(closure.incidence_root().bytes());
    if authorized_root != Some(live_root.as_str()) {
        return Err(RelationalPublicationError::PendingCursorMismatch);
    }
    Ok(closure)
}

fn mechanism_signature_definition<'a>(
    journal: &'a RelationalJournal,
    request_id: MechanismRequestId,
    signature_id: MechanismSignatureId,
) -> Result<(&'a MechanismSignatureDefinition, MechanismRequestScope), RelationalPublicationError> {
    let analysis = journal
        .analysis_state()
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    if let Some(open) = analysis.open_catalog() {
        let incidence = open
            .mechanism_incidence(request_id)
            .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
        let definition = incidence
            .signature_definition(signature_id)
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
        return Ok((definition, incidence.scope()));
    }
    let incidence = analysis
        .closed_catalog()
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    let incidence = incidence
        .mechanism_incidence(request_id)
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
    let definition = incidence
        .signature_definition(signature_id)
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)?;
    Ok((definition, incidence.scope()))
}

fn mechanism_unavailable_reason<'a>(
    journal: &'a RelationalJournal,
    request_id: MechanismRequestId,
    reason_id: super::mechanism_incidence::MechanismUnavailableReasonId,
) -> Result<&'a MechanismUnavailableReasonDefinition, RelationalPublicationError> {
    let analysis = journal
        .analysis_state()
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
    if let Some(open) = analysis.open_catalog() {
        return open
            .mechanism_incidence(request_id)
            .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?
            .unavailable_reason_definition(reason_id)
            .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch);
    }
    analysis
        .closed_catalog()
        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?
        .mechanism_incidence(request_id)
        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?
        .unavailable_reason_definition(reason_id)
        .ok_or(RelationalPublicationError::MechanismOrdinalIndexMismatch)
}

fn public_mechanism_terminal(
    terminal: super::mechanism_incidence::MechanismCaseTerminalRecord,
) -> JsonValue {
    match terminal.terminal() {
        MechanismCaseTerminal::Incidence {
            transition_id,
            signature_id,
        } => json!({
            "kind": "mechanism_incidence",
            "case_id": hex(terminal.case_id().bytes()),
            "transition_id": hex(transition_id.bytes()),
            "signature_id": hex(signature_id.bytes()),
            "frontier": "exact",
        }),
        MechanismCaseTerminal::Unavailable { reason_id } => json!({
            "kind": "mechanism_unavailable",
            "case_id": hex(terminal.case_id().bytes()),
            "reason_id": hex(reason_id.bytes()),
            "frontier": "unavailable",
        }),
    }
}

fn public_mechanism_closure(
    request_id: MechanismRequestId,
    closure: RelationalMechanismClosureReceipt,
) -> JsonValue {
    let counts = closure.counts();
    json!({
        "kind": "mechanism_incidence_closure",
        "request_id": hex(request_id.bytes()),
        "frontier": "exact",
        "incidence_root": hex(closure.incidence_root().bytes()),
        "counts": {
            "target_cases": public_mechanism_count(counts.target_cases()),
            "terminal_cases": public_mechanism_count(counts.terminal_cases()),
            "incidence_cases": public_mechanism_count(counts.incidence_cases()),
            "unavailable_cases": public_mechanism_count(counts.unavailable_cases()),
            "distinct_mechanisms": public_mechanism_count(counts.distinct_signatures()),
        },
    })
}

fn public_mechanism_definitions_closure(
    request_id: MechanismRequestId,
    signature_count: u128,
    incidence_root: &str,
) -> JsonValue {
    json!({
        "kind": "mechanism_definitions_closure",
        "request_id": hex(request_id.bytes()),
        "frontier": "exact",
        "signature_count": signature_count.to_string(),
        "incidence_root": incidence_root,
    })
}

fn public_mechanism_count(count: MechanismCountEvidence) -> JsonValue {
    match count {
        MechanismCountEvidence::Unknown {
            confirmed_lower_bound,
        } => json!({
            "status": "unknown",
            "confirmed_lower_bound": confirmed_lower_bound.to_string(),
        }),
        MechanismCountEvidence::LowerBound(value) => json!({
            "status": "lower_bound",
            "value": value.to_string(),
        }),
        MechanismCountEvidence::Exact(value) => json!({
            "status": "exact",
            "value": value.to_string(),
        }),
    }
}

fn mechanism_definition_chunk_count(bytes: usize) -> Result<u128, RelationalPublicationError> {
    let chunks = bytes
        .checked_add(MECHANISM_DEFINITION_CHUNK_BYTES - 1)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?
        / MECHANISM_DEFINITION_CHUNK_BYTES;
    Ok(chunks as u128)
}

fn public_signature_descriptor(
    definition: &MechanismSignatureDefinition,
    publication_index: &MechanismDefinitionPublicationIndex,
    definitions_artifact_key: &str,
    definitions_artifact_path: &str,
) -> JsonValue {
    // The canonical definition contains the two structural endpoint DAGs,
    // their roots/dependency edges, checked site/type identities, and control
    // outcomes. State/context values and their value digests live in replay
    // receipts, not these bytes. SELECT remains the only value publication
    // path.
    json!({
        "kind": "mechanism_signature_descriptor",
        "signature_id": hex(definition.id().bytes()),
        "encoding": MECHANISM_DEFINITION_ENCODING,
        "encoding_version": MECHANISM_DEFINITION_ENCODING_VERSION,
        "definition_digest": hex(definition.canonical_differential_digest()),
        "definition_bytes": definition.canonical_definition().len(),
        "definition_artifact": {
            "key": definitions_artifact_key,
            "path": definitions_artifact_path,
            "chunk_encoding": "lowercase_hex",
            "chunk_encoding_version": 1,
            "chunk_bytes": MECHANISM_DEFINITION_CHUNK_BYTES,
            "chunk_count": publication_index.chunk_count,
        },
        "dag_summary": {
            "before": public_signature_endpoint_summary(publication_index.dag.before_summary()),
            "after": public_signature_endpoint_summary(publication_index.dag.after_summary()),
        },
    })
}

fn public_signature_endpoint_summary(summary: RelationalMechanismEndpointDagSummary) -> JsonValue {
    json!({
        "node_count": summary.node_count(),
        "root_count": summary.root_count(),
        "edge_count": summary.edge_count(),
    })
}

fn public_signature_definition_payload_header(
    definition: &MechanismSignatureDefinition,
    payload_index: &MechanismDefinitionPayloadIndex,
) -> JsonValue {
    json!({
        "kind": "mechanism_signature_definition_header",
        "definition_record_ordinal": "0",
        "signature_id": hex(definition.id().bytes()),
        "encoding": MECHANISM_DEFINITION_ENCODING,
        "encoding_version": MECHANISM_DEFINITION_ENCODING_VERSION,
        "definition_digest": hex(payload_index.definition_digest),
        "definition_bytes": payload_index.definition_bytes,
        "chunk_encoding": "lowercase_hex",
        "chunk_encoding_version": 1,
        "chunk_bytes": MECHANISM_DEFINITION_CHUNK_BYTES,
        "chunk_count": payload_index.chunk_count,
    })
}

fn public_signature_definition_complete(
    definition: &MechanismSignatureDefinition,
    payload_index: &MechanismDefinitionPayloadIndex,
    definition_record_ordinal: u128,
) -> JsonValue {
    json!({
        "kind": "mechanism_signature_definition_complete",
        "definition_record_ordinal": definition_record_ordinal.to_string(),
        "signature_id": hex(definition.id().bytes()),
        "definition_digest": hex(payload_index.definition_digest),
        "definition_bytes": payload_index.definition_bytes,
        "chunk_count": payload_index.chunk_count,
    })
}

fn public_signature_definition_chunk(
    definition: &MechanismSignatureDefinition,
    definition_record_ordinal: u128,
    chunk_ordinal: u128,
) -> Result<JsonValue, RelationalPublicationError> {
    let chunk_ordinal_usize = usize::try_from(chunk_ordinal)
        .map_err(|_| RelationalPublicationError::ArithmeticOverflow)?;
    let offset = chunk_ordinal_usize
        .checked_mul(MECHANISM_DEFINITION_CHUNK_BYTES)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?;
    let end = offset
        .checked_add(MECHANISM_DEFINITION_CHUNK_BYTES)
        .ok_or(RelationalPublicationError::ArithmeticOverflow)?
        .min(definition.canonical_definition().len());
    let bytes = definition
        .canonical_definition()
        .get(offset..end)
        .filter(|bytes| !bytes.is_empty())
        .ok_or(
            RelationalPublicationError::MechanismDefinitionChunkOutOfRange {
                signature_id: hex(definition.id().bytes()),
                chunk_ordinal,
            },
        )?;
    let chunk_digest: [u8; 32] = Sha256::digest(bytes).into();
    Ok(json!({
        "kind": "mechanism_signature_definition_chunk",
        "definition_record_ordinal": definition_record_ordinal.to_string(),
        "signature_id": hex(definition.id().bytes()),
        "encoding": "lowercase_hex",
        "encoding_version": 1,
        "chunk_ordinal": chunk_ordinal,
        "byte_offset": offset,
        "byte_length": bytes.len(),
        "chunk_digest": hex(chunk_digest),
        "bytes": hex_slice(bytes),
    }))
}

fn public_unavailable_reason(
    definition: &MechanismUnavailableReasonDefinition,
) -> Result<JsonValue, RelationalPublicationError> {
    let evidence = RelationalMechanismUnavailableEvidence::restore_from_canonical_reason(
        definition.canonical_reason(),
    )
    .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
    Ok(json!({
        "kind": "mechanism_unavailability_reason",
        "reason_id": hex(definition.id().bytes()),
        "reason_kind": evidence.kind().code(),
    }))
}

fn public_selected_values<'value>(
    names: &[Box<str>],
    values: impl ExactSizeIterator<Item = &'value ResultValue>,
) -> Result<JsonValue, RelationalPublicationError> {
    if names.len() != values.len() {
        return Err(RelationalPublicationError::SelectShapeMismatch {
            names: names.len(),
            values: values.len(),
        });
    }
    let fields = names
        .iter()
        .zip(values)
        .map(|(name, value)| (name.to_string(), public_result_value(value)))
        .collect::<JsonMap<_, _>>();
    Ok(JsonValue::Object(fields))
}

fn public_result_value(value: &ResultValue) -> JsonValue {
    match value {
        ResultValue::Value(value) => public_explore_value(value),
        ResultValue::CaseId(id) => json!({ "kind": "case_id", "value": hex(id.bytes()) }),
        ResultValue::TransitionId(id) => {
            json!({ "kind": "transition_id", "value": hex(id.bytes()) })
        }
        ResultValue::SignatureId(id) => {
            json!({ "kind": "signature_id", "value": hex(id.bytes()) })
        }
        ResultValue::StructuralMechanismId(id) => {
            json!({ "kind": "structural_mechanism_id", "value": hex(id.bytes()) })
        }
        ResultValue::ExecutionProfileId(id) => {
            json!({ "kind": "execution_profile_id", "value": hex(id.bytes()) })
        }
    }
}

fn public_explore_value(value: &ExploreValue) -> JsonValue {
    match value {
        ExploreValue::Int(value) => json!(value),
        ExploreValue::FloatBits(bits) => json!({
            "kind": "float_bits",
            "bits": format!("{bits:016x}"),
        }),
        ExploreValue::String(value) => json!(value),
        ExploreValue::Character(value) => json!(value.to_string()),
        ExploreValue::Boolean(value) => json!(value),
        ExploreValue::Unit => json!({ "kind": "unit" }),
        ExploreValue::List(values) => {
            JsonValue::Array(values.iter().map(public_explore_value).collect::<Vec<_>>())
        }
        ExploreValue::Set(values) => json!({
            "kind": "set",
            "items": values.iter().map(public_explore_value).collect::<Vec<_>>(),
        }),
        ExploreValue::Tuple(values) => json!({
            "kind": "tuple",
            "items": values.iter().map(public_explore_value).collect::<Vec<_>>(),
        }),
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            let fields = if *positional {
                JsonValue::Array(
                    fields
                        .iter()
                        .map(|(_, value)| public_explore_value(value))
                        .collect(),
                )
            } else {
                JsonValue::Object(
                    fields
                        .iter()
                        .map(|(name, value)| (name.clone(), public_explore_value(value)))
                        .collect(),
                )
            };
            json!({
                "kind": "constructor",
                "type": type_name,
                "variant": variant,
                "layout": if *positional { "positional" } else { "named" },
                "fields": fields,
            })
        }
    }
}

fn public_row_id(row_id: ResultViewInputRowId) -> JsonValue {
    match row_id {
        ResultViewInputRowId::Source(source_key) => json!({
            "kind": "source",
            "source_key": hex(source_key.bytes()),
        }),
        ResultViewInputRowId::Case(case_id) => json!({
            "kind": "case",
            "case_id": hex(case_id.bytes()),
        }),
        ResultViewInputRowId::Incidence(incidence) => json!({
            "kind": "incidence",
            "case_id": hex(incidence.case_id().bytes()),
            "transition_id": hex(incidence.transition_id().bytes()),
            "signature_id": hex(incidence.signature_id().bytes()),
        }),
    }
}

fn encode_publication_line(
    artifact: &PublicationArtifactPlan,
    source: PublicationSourceCoordinate,
    checkpoint: RelationalPublicationCheckpoint,
    record: JsonValue,
    limits: RelationalPublicationLimits,
) -> Result<Vec<u8>, RelationalPublicationError> {
    let line = publication_line_bytes(artifact, source, checkpoint, record)?;
    if line.len() > limits.max_line_bytes {
        return Err(RelationalPublicationError::LineTooLarge {
            artifact: artifact.key().into(),
            bytes: line.len(),
            limit: limits.max_line_bytes,
        });
    }
    Ok(line)
}

fn publication_line_bytes(
    artifact: &PublicationArtifactPlan,
    source: PublicationSourceCoordinate,
    checkpoint: RelationalPublicationCheckpoint,
    record: JsonValue,
) -> Result<Vec<u8>, RelationalPublicationError> {
    let mut envelope = json!({
        "schema_version": RELATIONAL_PUBLICATION_SCHEMA_VERSION,
        "artifact": artifact.key(),
        "name": artifact.name(),
        "authorized_at": {
            "next_sequence": checkpoint.next_sequence,
            "journal_head": hex(checkpoint.head),
        },
        "record": record,
    });
    let object = envelope
        .as_object_mut()
        .expect("publication envelopes are JSON objects");
    match source {
        PublicationSourceCoordinate::Flat { source_ordinal } => {
            object.insert(
                "source_ordinal".into(),
                JsonValue::String(source_ordinal.to_string()),
            );
        }
        mechanism => {
            object.insert(
                "source_coordinate".into(),
                mechanism
                    .mechanism_json()
                    .expect("mechanism coordinate projects to JSON"),
            );
        }
    }
    let mut line = serde_json::to_vec(&envelope)
        .map_err(|error| RelationalPublicationError::Json(error.to_string()))?;
    line.push(b'\n');
    Ok(line)
}

fn build_manifest(
    plan: &RelationalPublicationPlan,
    report: &ExploreStreamSliceReport,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
    cursor_digest: [u8; 32],
) -> Result<JsonValue, RelationalPublicationError> {
    let artifacts = plan
        .artifacts
        .iter()
        .map(|artifact| {
            let state = cursor
                .artifacts
                .get(artifact.key())
                .ok_or(RelationalPublicationError::CursorArtifactSetMismatch)?;
            let mut descriptor = json!({
                "key": artifact.key(),
                "kind": artifact.kind(),
                "name": artifact.name(),
                "path": state.path,
                "encoding": "application/x-ndjson",
                "published_lines": state.line_count.to_string(),
                "published_bytes": state.byte_len,
                "caught_up_to_journal_prefix": artifact_is_caught_up(
                    artifact,
                    journal,
                    ordinal_index,
                    cursor,
                ),
                "prefix_digest": state.prefix_digest,
                "layer_roots": artifact_layer_roots(artifact, journal, ordinal_index)?,
            });
            let object = descriptor
                .as_object_mut()
                .expect("artifact descriptors are JSON objects");
            match state.source {
                ArtifactSourceCursor::Flat {
                    next_source_ordinal,
                } => {
                    let available =
                        available_source_record_count(artifact, journal, ordinal_index)?;
                    object.insert(
                        "next_source_ordinal".into(),
                        JsonValue::String(next_source_ordinal.to_string()),
                    );
                    object.insert(
                        "available_source_records".into(),
                        available
                            .map(|count| JsonValue::String(count.to_string()))
                            .unwrap_or(JsonValue::Null),
                    );
                }
                ArtifactSourceCursor::MechanismDiscovery {
                    event_ordinal,
                    closure_emitted,
                } => {
                    object.insert(
                        "next_source_coordinate".into(),
                        json!({
                            "kind": "mechanism_discovery",
                            "event_ordinal": event_ordinal.to_string(),
                            "closure_emitted": closure_emitted,
                        }),
                    );
                    let PendingArtifactSourceEnd::MechanismDiscovery {
                        event_end,
                        closure_root,
                    } = pending_source_end(artifact, journal, ordinal_index, cursor)?
                    else {
                        return Err(RelationalPublicationError::CursorArtifactMismatch(
                            artifact.key().into(),
                        ));
                    };
                    object.insert(
                        "available_source_window".into(),
                        json!({
                            "event_end": event_end.to_string(),
                            "closure_available": closure_root.is_some(),
                        }),
                    );
                }
                ArtifactSourceCursor::MechanismDefinitions {
                    signature_ordinal,
                    definition_part_ordinal,
                    closure_emitted,
                } => {
                    object.insert(
                        "next_source_coordinate".into(),
                        json!({
                            "kind": "mechanism_definitions",
                            "signature_ordinal": signature_ordinal.to_string(),
                            "definition_part_ordinal": definition_part_ordinal.to_string(),
                            "closure_emitted": closure_emitted,
                        }),
                    );
                    let PendingArtifactSourceEnd::MechanismDefinitions {
                        signature_end,
                        closure_root,
                    } = pending_source_end(artifact, journal, ordinal_index, cursor)?
                    else {
                        return Err(RelationalPublicationError::CursorArtifactMismatch(
                            artifact.key().into(),
                        ));
                    };
                    object.insert(
                        "available_source_window".into(),
                        json!({
                            "signature_end": signature_end.to_string(),
                            "closure_available": closure_root.is_some(),
                        }),
                    );
                }
                ArtifactSourceCursor::StructuralDefinitions {
                    header_emitted,
                    definition_ordinal,
                    definition_part_ordinal,
                    closure_emitted,
                } => {
                    object.insert(
                        "next_source_coordinate".into(),
                        json!({
                            "kind": "structural_definitions",
                            "header_emitted": header_emitted,
                            "definition_ordinal": definition_ordinal.to_string(),
                            "definition_part_ordinal": definition_part_ordinal.to_string(),
                            "closure_emitted": closure_emitted,
                        }),
                    );
                    let PendingArtifactSourceEnd::StructuralDefinitions {
                        definition_end,
                        structural_quotient_root,
                        definition_catalog_root,
                    } = pending_source_end(artifact, journal, ordinal_index, cursor)?
                    else {
                        return Err(RelationalPublicationError::CursorArtifactMismatch(
                            artifact.key().into(),
                        ));
                    };
                    object.insert(
                        "available_source_window".into(),
                        json!({
                            "definition_end": definition_end.to_string(),
                            "closure_available": structural_quotient_root.is_some()
                                && definition_catalog_root.is_some(),
                            "structural_quotient_root": structural_quotient_root,
                            "structural_definition_catalog_root": definition_catalog_root,
                        }),
                    );
                }
                ArtifactSourceCursor::SubjectStarters {
                    identity,
                    header_emitted,
                    accumulator,
                    closure_emitted,
                } => {
                    object.insert(
                        "next_source_coordinate".into(),
                        json!({
                            "kind": "subject_starters",
                            "consumer_id": hex(identity.consumer_id.bytes()),
                            "request_id": hex(identity.request_id.bytes()),
                            "target": identity.target,
                            "subject": identity.subject,
                            "header_emitted": header_emitted,
                            "next_page_ordinal": accumulator.map(|value| value.next_page_ordinal.to_string()),
                            "last_cursor": accumulator.and_then(|value| value.last_cursor).map(|value| json!({
                                "source_key": hex(value.source_key.bytes()),
                                "successor_key": hex(value.successor_key.bytes()),
                            })),
                            "closure_emitted": closure_emitted,
                        }),
                    );
                    let PendingArtifactSourceEnd::SubjectStarters {
                        identity: available_identity,
                        structural_quotient_root,
                        mechanism_support_root,
                        projection_plan_id,
                        projection_job_id,
                    } = pending_source_end(artifact, journal, ordinal_index, cursor)?
                    else {
                        return Err(RelationalPublicationError::CursorArtifactMismatch(
                            artifact.key().into(),
                        ));
                    };
                    object.insert(
                        "available_source_window".into(),
                        json!({
                            "consumer_id": hex(available_identity.consumer_id.bytes()),
                            "request_id": hex(available_identity.request_id.bytes()),
                            "target": available_identity.target,
                            "subject": available_identity.subject,
                            "closure_available": structural_quotient_root.is_some()
                                && mechanism_support_root.is_some()
                                && projection_plan_id.is_some()
                                && projection_job_id.is_some(),
                            "structural_quotient_root": structural_quotient_root,
                            "mechanism_support_closure_root": mechanism_support_root,
                            "projection_plan_id": projection_plan_id,
                            "projection_job_id": projection_job_id,
                        }),
                    );
                }
            }
            if let PublicationArtifactPlan::Result { input, .. } = artifact {
                if let Some(coverage) = mechanism_result_input_coverage(journal, *input)? {
                    object.insert(
                        "input_frontier".into(),
                        JsonValue::String(coverage.certainty_frontier().into()),
                    );
                    object.insert(
                        "upstream_mechanism_coverage".into(),
                        public_mechanism_result_input_coverage(coverage),
                    );
                }
            }
            if let PublicationArtifactPlan::MechanismStructural {
                definitions_artifact_key,
                definitions_artifact_path,
                ..
            } = artifact
            {
                object.insert(
                    "record_schema".into(),
                    JsonValue::String("futuruna.relational-structural-mechanism-support-v5".into()),
                );
                object.insert(
                    "record_schema_version".into(),
                    JsonValue::Number(5u32.into()),
                );
                object.insert(
                    "factorized_subject_summary_version".into(),
                    JsonValue::Number(MECHANISM_FACTORIZED_SUBJECT_SUMMARY_VERSION.into()),
                );
                object.insert(
                    "starter_projection_plan_version".into(),
                    JsonValue::Number(MECHANISM_STARTER_PROJECTION_PLAN_VERSION.into()),
                );
                object.insert(
                    "support_fiber_expr_version".into(),
                    JsonValue::Number(MECHANISM_SUPPORT_FIBER_EXPR_VERSION.into()),
                );
                object.insert(
                    "source_order".into(),
                    json!([
                        "structural_assignment*",
                        "structural_quotient_closure",
                        "structural_subject_support*",
                        "mechanism_support_closure",
                    ]),
                );
                object.insert(
                    "factorization".into(),
                    JsonValue::String(
                        "one bounded request-target-conditioned signature-fiber summary per mechanism and per node/edge facet; no subject_by_case rows".into(),
                    ),
                );
                object.insert(
                    "materialization".into(),
                    JsonValue::String("factorized_summary".into()),
                );
                object.insert(
                    "automatic_signature_scan_limit".into(),
                    JsonValue::String(AUTOMATIC_SUBJECT_SIGNATURE_SCAN_LIMIT.to_string()),
                );
                object.insert(
                    "correlated_starter_projection".into(),
                    json!({
                        "status": "not_materialized",
                        "identity": "projection_plan_id",
                        "publication_authorization": "requires_explicit_subject_starters_declaration",
                        "artifact": null,
                    }),
                );
                object.insert("cells_serialized".into(), JsonValue::Bool(false));
                object.insert(
                    "structural_definitions".into(),
                    public_structural_definition_artifact_reference(
                        definitions_artifact_key,
                        definitions_artifact_path,
                    ),
                );
            }
            if let PublicationArtifactPlan::SubjectStarters {
                consumer_id,
                request_id,
                target,
                subject,
                authorization,
                structural_artifact_key,
                structural_artifact_path,
                ..
            } = artifact
            {
                object.insert(
                    "record_schema".into(),
                    JsonValue::String("futuruna.relational-subject-starters-v1".into()),
                );
                object.insert(
                    "record_schema_version".into(),
                    JsonValue::Number(1u32.into()),
                );
                object.insert(
                    "consumer_id".into(),
                    JsonValue::String(hex(*consumer_id)),
                );
                object.insert(
                    "request_id".into(),
                    JsonValue::String(hex(request_id.bytes())),
                );
                object.insert("target".into(), public_mechanism_target_id(*target));
                object.insert("subject".into(), public_mechanism_support_subject(*subject));
                let availability = match mechanism_starter_unavailable_residual_case_count(
                    journal,
                    *request_id,
                )? {
                    Some(unavailable_case_count) => json!({
                        "status": "permanently_unavailable",
                        "reason": "closed_mechanism_replay_residual",
                        "unavailable_case_count": unavailable_case_count.to_string(),
                    }),
                    None if subject_starter_publication_authority(
                        journal,
                        *request_id,
                        *target,
                        *subject,
                    )?
                    .is_some() =>
                    {
                        json!({ "status": "exact_projection_available" })
                    }
                    None => json!({ "status": "awaiting_exact_support" }),
                };
                object.insert("availability".into(), availability);
                object.insert(
                    "authorization".into(),
                    public_mechanism_starter_authorization(authorization),
                );
                object.insert(
                    "scope".into(),
                    JsonValue::String("one_explicit_structural_subject".into()),
                );
                object.insert(
                    "source_order".into(),
                    json!([
                        "subject_starters_header",
                        "subject_starters_page*",
                        "subject_starters_closure",
                    ]),
                );
                object.insert(
                    "canonical_projection_order".into(),
                    json!(["source_key", "successor_key"]),
                );
                object.insert(
                    "canonical_member_order".into(),
                    JsonValue::String("SourceKey, SuccessorKey".into()),
                );
                object.insert(
                    "page_member_limit".into(),
                    JsonValue::Number(MECHANISM_STARTER_PAGE_MEMBER_LIMIT.get().into()),
                );
                object.insert("contains_typed_values".into(), JsonValue::Bool(true));
                object.insert(
                    "contains_node_edge_projections".into(),
                    JsonValue::Bool(matches!(
                        subject,
                        MechanismSupportSubject::Node { .. }
                            | MechanismSupportSubject::Edge { .. }
                    )),
                );
                object.insert(
                    "structural_support".into(),
                    json!({
                        "artifact_key": structural_artifact_key,
                        "path": structural_artifact_path,
                    }),
                );
            }
            if let PublicationArtifactPlan::MechanismStructuralDefinitions {
                structural_artifact_key,
                structural_artifact_path,
                ..
            } = artifact
            {
                object.insert(
                    "record_schema".into(),
                    JsonValue::String(
                        "futuruna.relational-structural-definition-catalog-v1".into(),
                    ),
                );
                object.insert(
                    "record_schema_version".into(),
                    JsonValue::Number(1u32.into()),
                );
                object.insert(
                    "availability".into(),
                    JsonValue::String("structural_quotient_closed".into()),
                );
                object.insert(
                    "source_order".into(),
                    json!([
                        "structural_definition_catalog_header",
                        "structural_frame_definition*",
                        "structural_activation_context_definition*",
                        "structural_node_definition*",
                        "structural_edge_definition*",
                        "structural_mechanism_definition*",
                        "structural_execution_profile_definition*",
                        "structural_definition_catalog_closure",
                    ]),
                );
                object.insert(
                    "typed_chunk_item_limit".into(),
                    JsonValue::String(STRUCTURAL_DEFINITION_CHUNK_ITEMS.to_string()),
                );
                object.insert(
                    "chunk_placement".into(),
                    JsonValue::String(
                        "each structural_definition_lane_chunk immediately follows its owning definition header and preserves that definition's declared lane order"
                            .into(),
                    ),
                );
                object.insert(
                    "structural_support".into(),
                    json!({
                        "artifact_key": structural_artifact_key,
                        "path": structural_artifact_path,
                    }),
                );
                object.insert("contains_raw_signatures".into(), JsonValue::Bool(false));
                object.insert("contains_cases".into(), JsonValue::Bool(false));
                object.insert("contains_starter_values".into(), JsonValue::Bool(false));
            }
            if let PublicationArtifactPlan::CaseSupport { authorization } = artifact {
                object.insert(
                    "record_schema".into(),
                    JsonValue::String(RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA.into()),
                );
                object.insert(
                    "record_schema_version".into(),
                    JsonValue::Number(RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION.into()),
                );
                object.insert(
                    "case_id_authority".into(),
                    (*authorization)
                        .map(|authorization| public_case_id_authority(authorization.authority()))
                        .unwrap_or(JsonValue::Null),
                );
                object.insert(
                    "graph_projection".into(),
                    ordinal_index.case_support.as_ref().map_or_else(
                        || {
                            json!({
                                "frontier": {
                                    "status": "open",
                                    "reason": {
                                        "kind": "awaiting_case_projection_authority",
                                    },
                                },
                                "counts": null,
                            })
                        },
                        public_case_support_projection_metadata,
                    ),
                );
            }
            if let PublicationArtifactPlan::CaseTransitions {
                authorization,
                transition_schemas,
            } = artifact
            {
                object.insert(
                    "record_schema".into(),
                    JsonValue::String(RELATIONAL_CASE_TRANSITION_PROJECTION_SCHEMA.into()),
                );
                object.insert(
                    "record_schema_version".into(),
                    JsonValue::Number(RELATIONAL_CASE_TRANSITION_PROJECTION_VERSION.into()),
                );
                object.insert(
                    "value_authorization".into(),
                    json!({
                        "authorization_id": hex(authorization.authorization_id().bytes()),
                        "authorizing_view_id": hex(authorization.view_id().bytes()),
                        "authorizing_view_name": authorization.authorizing_view_name(),
                    }),
                );
                object.insert(
                    "transition_schemas".into(),
                    json!({
                        "state_schema_id": hex(transition_schemas.state_schema_id().bytes()),
                        "context_schema_id": hex(transition_schemas.context_schema_id().bytes()),
                        "transition_type_id": hex(transition_schemas.transition_type_id().bytes()),
                    }),
                );
                object.insert(
                    "graph_projection".into(),
                    ordinal_index.case_transitions.as_ref().map_or_else(
                        || {
                            json!({
                                "frontier": {
                                    "status": "open",
                                    "reason": "awaiting_selected_case_values",
                                },
                                "counts": null,
                            })
                        },
                        public_case_transition_projection_metadata,
                    ),
                );
                object.insert(
                    "source_order".into(),
                    JsonValue::String("journal_selected_discovery".into()),
                );
                object.insert("contains_typed_case_values".into(), JsonValue::Bool(true));
            }
            Ok(descriptor)
        })
        .collect::<Result<Vec<_>, RelationalPublicationError>>()?;

    Ok(json!({
        "schema_version": RELATIONAL_PUBLICATION_SCHEMA_VERSION,
        "authority": "durable_relational_journal",
        "query": report.query_name,
        "identity": {
            "checked_program": report.identity.checked_program,
            "relation_id": report.identity.relation_id,
            "admission_id": report.identity.admission_id,
            "question_id": report.identity.question_id,
            "analysis_graph_digest": report.identity.analysis_graph_digest,
            "starter_consumer_set_id": hex(plan.starter_consumer_set_id),
            "journal_id": report.identity.journal_id,
        },
        "source_coverage": public_source_coverage_json(report),
        "journal": {
            "next_sequence": report.checkpoint.next_sequence,
            "head": report.checkpoint.journal_head,
            "durable_segment_count": report.checkpoint.durable_segment_count,
        },
        "lifecycle": lifecycle_name(report.lifecycle),
        "pause_reason": report.pause_reason.as_ref().map(public_pause_reason_json),
        "closure": {
            "relation": if report.relation_closed { "exact" } else { "open" },
            "find": if report.find_closed { "exact" } else { "open" },
            "analysis": if report.analysis_closed { "exact" } else { "open" },
        },
        "counts": {
            "U_S_sources": public_count_json(report.counts.sources),
            "U_C_cases": public_count_json(report.counts.cases),
            "admission_classified": public_count_json(report.counts.admission_classified),
            "D_C_admitted": public_count_json(report.counts.admitted),
            "rejected": public_count_json(report.counts.rejected),
            "find_classified": public_count_json(report.counts.find_classified),
            "S_C_selected": public_count_json(report.counts.selected),
            "not_selected": public_count_json(report.counts.not_selected),
        },
        "analysis_scope_root": report.analysis_scope_root,
        "analysis_terminal_root": report.analysis_terminal_root,
        "layers": report.layers.iter().map(public_layer_json).collect::<Vec<_>>(),
        "artifacts": artifacts,
        "publication_cursor": {
            "file": CURSOR_FILE,
            "digest": hex(cursor_digest),
            "checkpoint": cursor.checkpoint,
            "pending": cursor.pending.is_some(),
        },
        "limitations": [
            "This is a materialized view; the durable journal is the recovery authority.",
            "Mechanism signature descriptors and canonical raw-definition chunks contain structural control evidence only; state/context values remain absent unless a checked SELECT publishes them.",
            "The structural-definition catalog publishes normalized quotient topology and exact multiplicities in bounded typed chunks; it contains no raw signatures, cases, starter values, or allocating origin preimages.",
            "Structural support publishes a hard-bounded signature-fiber summary for each request-target-conditioned mechanism or node/edge facet; capped scans widen bounds and never fall back to a full case/starter union.",
            "The compact structural sidecar never serializes or links correlated (Context, Before) -> After cells. Only an explicit single-subject starters declaration can materialize one mechanism/node/edge facet through its named checked value view.",
            "Typed subject-starter artifacts contain authorized state and context values and must be treated as confidential output.",
            "The selected case-transition graph contains authorized typed Context, Before, and After values and must be treated as confidential output; its line order is journal discovery order while its closure root commits canonical set content.",
            "Authenticated support roots and structural IDs are audit commitments, not anonymization; low-entropy or externally known inputs may still permit membership inference even when cells are not serialized.",
            "The case/support graph does not serialize raw case state, context, intervals, materializers, or proof payloads; its deterministic artifact IDs and roots are audit commitments rather than hiding commitments, so output containing private low-entropy inputs remains confidential.",
        ],
    }))
}

fn available_source_record_count(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &PublicationOrdinalIndex<'_>,
) -> Result<Option<u128>, RelationalPublicationError> {
    match artifact {
        PublicationArtifactPlan::Result {
            source: ResultPublicationSource::EarlyEachCase,
            ..
        } => Ok(Some(
            journal
                .scheduler_view()
                .map_err(|error| RelationalPublicationError::Journal(error.to_string()))?
                .selected_discovery_suffix(0)
                .len() as u128,
        )),
        PublicationArtifactPlan::Result {
            view_id,
            source: ResultPublicationSource::DurableProjection,
            ..
        } => durable_projection_len(journal, *view_id),
        PublicationArtifactPlan::CaseSupport { .. } => Ok(Some(
            ordinal_index
                .case_support
                .as_ref()
                .map_or(0, |projection| projection.available_source_record_count()),
        )),
        PublicationArtifactPlan::CaseTransitions { .. } => {
            Ok(Some(ordinal_index.case_transitions.as_ref().map_or(
                0,
                RelationalCaseTransitionProjection::available_source_record_count,
            )))
        }
        PublicationArtifactPlan::MechanismStructural { request_id, .. } => Ok(Some(
            structural_sidecar_authority(journal, *request_id)?.available_source_record_count()?,
        )),
        PublicationArtifactPlan::Mechanism { .. }
        | PublicationArtifactPlan::MechanismDefinitions { .. }
        | PublicationArtifactPlan::MechanismStructuralDefinitions { .. }
        | PublicationArtifactPlan::SubjectStarters { .. } => Err(
            RelationalPublicationError::MechanismSourceCoordinateMismatch {
                artifact: artifact.key().into(),
            },
        ),
    }
}

fn durable_projection_len(
    journal: &RelationalJournal,
    view_id: ViewId,
) -> Result<Option<u128>, RelationalPublicationError> {
    let Some(analysis) = journal.analysis_state() else {
        return Ok(None);
    };
    match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            if matches!(
                open.layer_status(RelationalAnalysisLayerId::Result(view_id)),
                Some(RelationalAnalysisLayerStatus::ResultUnregistered) | None
            ) {
                return Ok(None);
            }
            let projection_len = open
                .result_projection(view_id)
                .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?
                .len() as u128;
            let has_closure = open
                .result_publication(view_id)
                .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?
                .is_some();
            Ok(Some(
                projection_len
                    .checked_add(if has_closure { 1 } else { 0 })
                    .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
            ))
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Result(view_id))
                .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
            let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
                return Err(RelationalPublicationError::AnalysisLayerKindMismatch);
            };
            let RelationalResultLayerSnapshotState::Registered {
                projection,
                publication,
                ..
            } = result.state()
            else {
                return Ok(None);
            };
            Ok(Some(
                (projection.records().len() as u128)
                    .checked_add(if publication.is_some() { 1 } else { 0 })
                    .ok_or(RelationalPublicationError::ArithmeticOverflow)?,
            ))
        }
        _ => Err(RelationalPublicationError::AnalysisCatalogStateMismatch),
    }
}

fn artifact_is_caught_up(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &mut PublicationOrdinalIndex<'_>,
    cursor: &PublicationCursor,
) -> bool {
    let Some(state) = cursor.artifacts.get(artifact.key()) else {
        return false;
    };
    match (artifact, state.source) {
        (
            PublicationArtifactPlan::MechanismStructural { .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => {
            return available_source_record_count(artifact, journal, ordinal_index)
                .ok()
                .flatten()
                == Some(next_source_ordinal);
        }
        (
            PublicationArtifactPlan::CaseSupport { .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => {
            let available = ordinal_index
                .case_support
                .as_ref()
                .map_or(0, |projection| projection.available_source_record_count());
            let is_open = ordinal_index
                .case_support
                .as_ref()
                .is_none_or(PublicationCaseSupportProjection::is_open);
            if is_open {
                return next_source_ordinal == available;
            }
        }
        (
            PublicationArtifactPlan::CaseTransitions { .. },
            ArtifactSourceCursor::Flat {
                next_source_ordinal,
            },
        ) => {
            let available = ordinal_index.case_transitions.as_ref().map_or(
                0,
                RelationalCaseTransitionProjection::available_source_record_count,
            );
            let is_open = ordinal_index
                .case_transitions
                .as_ref()
                .is_none_or(RelationalCaseTransitionProjection::is_open);
            if is_open {
                return next_source_ordinal == available;
            }
        }
        (
            PublicationArtifactPlan::Mechanism { request_id, .. },
            ArtifactSourceCursor::MechanismDiscovery {
                event_ordinal,
                closure_emitted: false,
            },
        ) => {
            let frontier = journal
                .analysis_state()
                .map_or(Some((0, false)), |analysis| {
                    analysis
                        .mechanism_publication_discovery(*request_id)
                        .map(|discovery| {
                            (
                                discovery.event_count() as u128,
                                analysis.mechanism_closure(*request_id).is_some(),
                            )
                        })
                });
            if let Some((event_end, closure_authorized)) = frontier {
                if !closure_authorized && event_ordinal == event_end {
                    return true;
                }
            }
        }
        (
            PublicationArtifactPlan::MechanismDefinitions { request_id, .. },
            ArtifactSourceCursor::MechanismDefinitions {
                signature_ordinal,
                definition_part_ordinal,
                closure_emitted: false,
            },
        ) => {
            let frontier = journal
                .analysis_state()
                .map_or(Some((0, false)), |analysis| {
                    analysis
                        .mechanism_publication_discovery(*request_id)
                        .map(|discovery| {
                            (
                                discovery.signature_count() as u128,
                                analysis.mechanism_closure(*request_id).is_some(),
                            )
                        })
                });
            if let Some((signature_end, closure_authorized)) = frontier {
                if !closure_authorized
                    && signature_ordinal == signature_end
                    && definition_part_ordinal == 0
                {
                    return true;
                }
            }
        }
        (
            PublicationArtifactPlan::MechanismStructuralDefinitions { request_id, .. },
            ArtifactSourceCursor::StructuralDefinitions {
                header_emitted: false,
                definition_ordinal: 0,
                definition_part_ordinal: 0,
                closure_emitted: false,
            },
        ) => {
            if matches!(
                structural_definition_catalog_authority(journal, *request_id),
                Ok(None)
            ) {
                return true;
            }
        }
        (
            PublicationArtifactPlan::SubjectStarters { request_id, .. },
            ArtifactSourceCursor::SubjectStarters {
                header_emitted: false,
                accumulator: None,
                closure_emitted: false,
                ..
            },
        ) => {
            if matches!(
                mechanism_starter_unavailable_residual_case_count(journal, *request_id),
                Ok(Some(_))
            ) {
                // A closed unavailable residual permanently prevents an exact
                // typed relation. The factorized structural artifact remains
                // authoritative; this optional lane has no publication debt.
                return true;
            }
        }
        _ => {}
    }
    matches!(
        record_at(
            artifact,
            journal,
            ordinal_index,
            cursor,
            state.source,
            None,
            None,
        ),
        Ok(AddressedPublicationRecord::Exhausted)
    )
}

fn artifact_layer_roots(
    artifact: &PublicationArtifactPlan,
    journal: &RelationalJournal,
    ordinal_index: &PublicationOrdinalIndex<'_>,
) -> Result<JsonValue, RelationalPublicationError> {
    if matches!(artifact, PublicationArtifactPlan::CaseSupport { .. }) {
        let Some(projection) = ordinal_index.case_support.as_ref() else {
            return Ok(JsonValue::Null);
        };
        return match projection {
            PublicationCaseSupportProjection::Partitioned(projection) => {
                let Some(RelationalCaseSupportProjectionRecord::Root {
                    partition_artifact_id,
                    ..
                }) = projection
                    .record_at(0)
                    .map_err(|error| RelationalPublicationError::CaseSupport(error.to_string()))?
                else {
                    return Err(RelationalPublicationError::CaseSupport(
                        "case-support projection omitted its root record".into(),
                    ));
                };
                Ok(match projection.metadata().frontier {
                    RelationalCaseSupportProjectionFrontier::Open(_) => json!({
                        "projection_kind": "partitioned_support",
                        "partition_artifact_id": hex(partition_artifact_id.bytes()),
                        "support_evidence_root": null,
                        "selected_question_seal_id": null,
                    }),
                    RelationalCaseSupportProjectionFrontier::Exact(closure) => json!({
                        "projection_kind": "partitioned_support",
                        "partition_artifact_id": hex(partition_artifact_id.bytes()),
                        "support_evidence_root": hex(closure.support_evidence_root.bytes()),
                        "selected_question_seal_id": hex(closure.selected_question_seal_id.bytes()),
                    }),
                })
            }
            PublicationCaseSupportProjection::ClassificationSummary(projection) => Ok(json!({
                "projection_kind": "classification_summary",
                "partition_artifact_id": null,
                "classification_authority": public_classification_authority(
                    projection.closure.classification_authority,
                ),
                "support_evidence_root": hex(projection.closure.support_evidence_root),
                "selected_question_seal_id": hex(
                    projection.closure.selected_question_seal_id.bytes()
                ),
                "selected_population_authority": public_published_selected_population_authority(
                    projection.closure.selected_population_authority,
                ),
            })),
        };
    }
    if matches!(artifact, PublicationArtifactPlan::CaseTransitions { .. }) {
        let Some(projection) = ordinal_index.case_transitions.as_ref() else {
            return Ok(JsonValue::Null);
        };
        let metadata = projection.metadata();
        return Ok(json!({
            "projection_id": hex(metadata.projection_id().bytes()),
            "selected_question_seal_id": metadata.closure().map(|closure| {
                hex(closure.selected_question_seal_id().bytes())
            }),
            "selected_case_set_root": metadata.closure().map(|closure| {
                hex(closure.selected_case_set_root().bytes())
            }),
            "case_transition_content_root": metadata.closure().map(|closure| {
                hex(closure.content_root().bytes())
            }),
        }));
    }
    let Some(analysis) = journal.analysis_state() else {
        return Ok(JsonValue::Null);
    };
    match artifact {
        PublicationArtifactPlan::Result { view_id, .. } => {
            match (analysis.open_catalog(), analysis.closed_catalog()) {
                (Some(open), None) => {
                    match open.layer_status(RelationalAnalysisLayerId::Result(*view_id)) {
                        Some(RelationalAnalysisLayerStatus::ResultUnregistered) | None => {
                            return Ok(JsonValue::Null);
                        }
                        Some(_) => {}
                    }
                    let spec = open
                        .result_spec(*view_id)
                        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
                    let projection = open
                        .result_projection(*view_id)
                        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
                    let publication = open
                        .result_publication(*view_id)
                        .map_err(|error| RelationalPublicationError::Analysis(error.to_string()))?;
                    Ok(json!({
                        "spec_root": hex(spec.spec_root().bytes()),
                        // The open evidence root is an O(R) derivation today;
                        // do not rescan it merely for presentation.
                        "evidence_root": publication.map(|value| hex(value.evidence_root().bytes())),
                        "projection_root": hex(projection.root().bytes()),
                        "publication_id": publication.map(|value| hex(value.id().bytes())),
                        "result_root": publication.map(|value| hex(value.result_root().bytes())),
                    }))
                }
                (None, Some(closed)) => {
                    let layer = closed
                        .snapshot()
                        .layer(RelationalAnalysisLayerId::Result(*view_id))
                        .ok_or(RelationalPublicationError::MissingAnalysisLayer)?;
                    let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
                        return Err(RelationalPublicationError::AnalysisLayerKindMismatch);
                    };
                    let RelationalResultLayerSnapshotState::Registered {
                        spec,
                        evidence,
                        projection,
                        publication,
                        ..
                    } = result.state()
                    else {
                        return Ok(JsonValue::Null);
                    };
                    Ok(json!({
                        "spec_root": hex(spec.spec_root().bytes()),
                        "evidence_root": hex(evidence.root().bytes()),
                        "projection_root": hex(projection.root().bytes()),
                        "publication_id": publication.map(|value| hex(value.id().bytes())),
                        "result_root": publication.map(|value| hex(value.result_root().bytes())),
                    }))
                }
                _ => Err(RelationalPublicationError::AnalysisCatalogStateMismatch),
            }
        }
        PublicationArtifactPlan::Mechanism { request_id, .. }
        | PublicationArtifactPlan::MechanismDefinitions { request_id, .. } => {
            // Open incidence.root() is a full-map derivation. Publish the
            // compact authenticated root as soon as the exact request closure
            // has reached the journal; final analysis closure is not needed.
            let Some(closure) = analysis.mechanism_closure(*request_id) else {
                return Ok(json!({ "incidence_root": null }));
            };
            Ok(json!({
                "incidence_root": hex(closure.incidence_root().bytes()),
            }))
        }
        PublicationArtifactPlan::MechanismStructural { request_id, .. } => {
            let authority = structural_sidecar_authority(journal, *request_id)?;
            let Some(structural) = authority.structural else {
                return Ok(JsonValue::Null);
            };
            let raw_closure_root = analysis
                .mechanism_closure(*request_id)
                .map(|closure| hex(closure.incidence_root().bytes()));
            let structural_closure_root = authority
                .structural_closure
                .map(|closure| hex(closure.root().bytes()));
            let structural_definition_catalog_root = structural
                .definition_catalog_root()
                .map(|root| hex(root.bytes()));
            let support_closure_root = authority
                .support
                .map(|(_, closure)| hex(closure.root().bytes()));
            let support_residual_root = authority
                .support
                .map(|(_, closure)| hex(closure.residual_root().bytes()));
            let target = authority
                .support
                .map(|(_, closure)| public_mechanism_target_id(closure.target()));
            Ok(json!({
                "request_id": hex(request_id.bytes()),
                "target": target,
                "assignment_discovery_count": authority.assignment_count.to_string(),
                "structural_subject_counts": {
                    "mechanisms": structural.structural_mechanism_count().to_string(),
                    "nodes": structural.canonical_node_ids().len().to_string(),
                    "edges": structural.canonical_edge_ids().len().to_string(),
                    "request_target_support_rows": authority.support_subject_count.to_string(),
                },
                "current_assignment_root": hex(structural.assignment_root()),
                "current_structural_revision": hex(structural.revision().bytes()),
                "raw_incidence_root": raw_closure_root,
                "structural_quotient_root": structural_closure_root,
                "structural_definition_catalog_root": structural_definition_catalog_root,
                "mechanism_support_closure_root": support_closure_root,
                "shared_residual_root": support_residual_root,
            }))
        }
        PublicationArtifactPlan::MechanismStructuralDefinitions { request_id, .. } => {
            let Some(authority) = structural_definition_catalog_authority(journal, *request_id)?
            else {
                return Ok(JsonValue::Null);
            };
            Ok(json!({
                "request_id": hex(request_id.bytes()),
                "structural_quotient_root": hex(authority.closure.root().bytes()),
                "structural_definition_catalog_root": hex(
                    authority.definition_catalog_root.bytes()
                ),
                "catalog_membership_root": hex(
                    authority.closure.catalog_membership_root().bytes()
                ),
            }))
        }
        PublicationArtifactPlan::SubjectStarters {
            consumer_id,
            request_id,
            target,
            subject,
            authorization,
            transition_schemas,
            ..
        } => {
            let Some(authority) =
                subject_starter_publication_authority(journal, *request_id, *target, *subject)?
            else {
                return Ok(JsonValue::Null);
            };
            let job = subject_starter_projection_job(
                journal,
                &authority,
                transition_schemas,
                authorization,
            )?;
            Ok(json!({
                "consumer_id": hex(*consumer_id),
                "request_id": hex(request_id.bytes()),
                "target": public_mechanism_target_id(*target),
                "subject": public_mechanism_support_subject(*subject),
                "projection_plan_id": hex(authority.key_authority.projection_plan_id().bytes()),
                "projection_job_id": hex(job.id().bytes()),
                "authorization_id": hex(authorization.authorization_id().bytes()),
                "authorizing_view_id": hex(authorization.view_id().bytes()),
                "raw_incidence_root": hex(authority.support_closure.incidence_root().bytes()),
                "structural_quotient_root": hex(authority.structural_closure.root().bytes()),
                "mechanism_support_closure_root": hex(authority.support_closure.root().bytes()),
            }))
        }
        PublicationArtifactPlan::CaseSupport { .. } => {
            unreachable!("case-support roots return before consulting the analysis catalog")
        }
        PublicationArtifactPlan::CaseTransitions { .. } => {
            unreachable!("case-transition roots return before consulting the analysis catalog")
        }
    }
}

fn public_count_json(count: ExploreStreamCount) -> JsonValue {
    match count {
        ExploreStreamCount::Unknown {
            confirmed_lower_bound,
        } => json!({
            "status": "unknown",
            "confirmed_lower_bound": confirmed_lower_bound.to_string(),
        }),
        ExploreStreamCount::LowerBound(value) => json!({
            "status": "lower_bound",
            "value": value.to_string(),
        }),
        ExploreStreamCount::Interval {
            lower_bound,
            upper_bound,
        } => json!({
            "status": "interval",
            "lower_bound": lower_bound.to_string(),
            "upper_bound": upper_bound.to_string(),
        }),
        ExploreStreamCount::Exact(value) => json!({
            "status": "exact",
            "value": value.to_string(),
        }),
    }
}

const fn lifecycle_name(lifecycle: ExploreStreamLifecycle) -> &'static str {
    match lifecycle {
        ExploreStreamLifecycle::Paused => "paused",
        ExploreStreamLifecycle::Complete => "complete",
    }
}

fn public_pause_reason_json(reason: &ExploreStreamPauseReason) -> JsonValue {
    match reason {
        ExploreStreamPauseReason::RuntimeLimit => json!({ "kind": "runtime_limit" }),
        ExploreStreamPauseReason::ResourceAdmission { code } => {
            json!({ "kind": "resource_admission", "code": code })
        }
        ExploreStreamPauseReason::MechanismReplay {
            request_id,
            case_id,
            endpoint,
            reason,
        } => json!({
            "kind": "mechanism_replay",
            "request_id": request_id,
            "case_id": case_id,
            "endpoint": endpoint,
            "reason": reason,
        }),
        ExploreStreamPauseReason::AwaitingChosenViewMechanisms {
            request_id,
            view_id,
        } => json!({
            "kind": "awaiting_chosen_view_mechanisms",
            "request_id": request_id,
            "view_id": view_id,
        }),
        ExploreStreamPauseReason::AwaitingSourceResult { view_id } => json!({
            "kind": "awaiting_source_result",
            "view_id": view_id,
        }),
        ExploreStreamPauseReason::AwaitingMechanismIncidenceResult {
            view_id,
            request_id,
        } => json!({
            "kind": "awaiting_mechanism_incidence_result",
            "view_id": view_id,
            "request_id": request_id,
        }),
    }
}

fn public_layer_json(layer: &ExploreStreamLayer) -> JsonValue {
    match layer {
        ExploreStreamLayer::Result(result) => json!({
            "kind": "result",
            "name": result.name,
            "view_id": result.view_id,
            "status": layer_status_name(result.status),
            "input_rows": public_count_json(result.input_rows),
            "projection_records": public_count_json(result.projection_records),
        }),
        ExploreStreamLayer::Mechanisms(mechanism) => json!({
            "kind": "mechanisms",
            "name": mechanism.name,
            "request_id": mechanism.request_id,
            "target": public_mechanism_target_json(&mechanism.target),
            "status": layer_status_name(mechanism.status),
            "target_cases": public_count_json(mechanism.target_cases),
            "terminal_cases": public_count_json(mechanism.terminal_cases),
            "incidence_cases": public_count_json(mechanism.incidence_cases),
            "unavailable_cases": public_count_json(mechanism.unavailable_cases),
            "raw_signatures": public_count_json(mechanism.raw_signatures),
            "structural_assignments": public_count_json(mechanism.structural_assignments),
            "structural_mechanisms": public_count_json(mechanism.structural_mechanisms),
            "execution_profiles": public_count_json(mechanism.execution_profiles),
            "raw_closure_root": mechanism.raw_closure_root,
            "structural_closure_root": mechanism.structural_closure_root,
            "support_closure_root": mechanism.support_closure_root,
            "support_closure_totals": mechanism.support_closure_totals.map(|totals| json!({
                "target_cases": totals.target_cases.to_string(),
                "successful_cases": totals.successful_cases.to_string(),
                "unavailable_cases": totals.unavailable_cases.to_string(),
                "signature_fibers": totals.signature_fibers.to_string(),
                "target_starters": totals.target_starters.to_string(),
            })),
        }),
    }
}

fn public_mechanism_target_json(target: &ExploreStreamMechanismTarget) -> JsonValue {
    match target {
        ExploreStreamMechanismTarget::Selected => json!({
            "kind": "selected",
        }),
        ExploreStreamMechanismTarget::ChosenView { view_id } => json!({
            "kind": "chosen_view",
            "view_id": view_id,
        }),
    }
}

const fn layer_status_name(status: ExploreStreamLayerStatus) -> &'static str {
    match status {
        ExploreStreamLayerStatus::ResultUnregistered => "result_unregistered",
        ExploreStreamLayerStatus::ResultInputOpen => "result_input_open",
        ExploreStreamLayerStatus::ResultAwaitingPublication => "result_awaiting_publication",
        ExploreStreamLayerStatus::ResultPublished => "result_published",
        ExploreStreamLayerStatus::MechanismUnregistered => "mechanism_unregistered",
        ExploreStreamLayerStatus::MechanismTargetOpen => "mechanism_target_open",
        ExploreStreamLayerStatus::MechanismTerminalOpen => "mechanism_terminal_open",
        ExploreStreamLayerStatus::MechanismClosed => "mechanism_closed",
    }
}

fn public_source_coverage_json(report: &ExploreStreamSliceReport) -> JsonValue {
    let coverage = &report.source_coverage;
    json!({
        "version": coverage.version,
        "manifest_digest": coverage.manifest_digest,
        "semantic_dependency_digest": coverage.semantic_dependency_digest,
        "has_gaps": coverage.has_gaps,
        "entries": coverage.entries.iter().map(|entry| json!({
            "subject_id": entry.subject_id,
            "subject": public_coverage_subject_json(&entry.subject),
            "classification": public_coverage_classification_json(&entry.classification),
        })).collect::<Vec<_>>(),
    })
}

fn public_coverage_subject_json(subject: &ExploreStreamCoverageSubject) -> JsonValue {
    match subject {
        ExploreStreamCoverageSubject::SourceBinding {
            binding_index,
            binding_name,
            role,
        } => json!({
            "kind": "source_binding",
            "binding_index": binding_index,
            "binding_name": binding_name,
            "role": coverage_binding_role_name(*role),
        }),
        ExploreStreamCoverageSubject::SchemaRoot { role, type_name } => json!({
            "kind": "schema_root",
            "role": coverage_root_role_name(*role),
            "type_name": type_name,
        }),
        ExploreStreamCoverageSubject::SchemaField {
            role,
            variant_index,
            field_index,
            variant_name,
            field_name,
        } => json!({
            "kind": "schema_field",
            "role": coverage_root_role_name(*role),
            "variant_index": variant_index,
            "field_index": field_index,
            "variant_name": variant_name,
            "field_name": field_name,
        }),
        ExploreStreamCoverageSubject::Literal { kind, value } => json!({
            "kind": "literal",
            "literal_kind": coverage_literal_kind_name(*kind),
            "value": value,
        }),
        ExploreStreamCoverageSubject::TopLevelConstant { addresses } => json!({
            "kind": "top_level_constant",
            "addresses": addresses,
        }),
        ExploreStreamCoverageSubject::ConstructorChoice {
            owner_name,
            variant_name,
            variant_index,
        } => json!({
            "kind": "constructor_choice",
            "owner_name": owner_name,
            "variant_name": variant_name,
            "variant_index": variant_index,
        }),
    }
}

fn public_coverage_classification_json(
    classification: &ExploreStreamCoverageClassification,
) -> JsonValue {
    match classification {
        ExploreStreamCoverageClassification::VariedFiniteDimension { dimension_id } => json!({
            "kind": "varied_finite_dimension",
            "dimension_id": dimension_id,
        }),
        ExploreStreamCoverageClassification::DerivedFromDeclaredDimensions { dimension_ids } => {
            json!({
                "kind": "derived_from_declared_dimensions",
                "dimension_ids": dimension_ids,
            })
        }
        ExploreStreamCoverageClassification::ConditionedSingletonOrSourceRestriction => json!({
            "kind": "conditioned_singleton_or_source_restriction",
        }),
        ExploreStreamCoverageClassification::ExactIrrelevanceCertificate { certificate_digest } => {
            json!({
                "kind": "exact_irrelevance_certificate",
                "certificate_digest": certificate_digest,
            })
        }
        ExploreStreamCoverageClassification::CoverageGap { reason } => json!({
            "kind": "coverage_gap",
            "reason": coverage_gap_reason_name(*reason),
        }),
    }
}

const fn coverage_root_role_name(role: ExploreStreamCoverageRootRole) -> &'static str {
    match role {
        ExploreStreamCoverageRootRole::Context => "context",
        ExploreStreamCoverageRootRole::Before => "before",
    }
}

const fn coverage_binding_role_name(role: ExploreStreamCoverageBindingRole) -> &'static str {
    match role {
        ExploreStreamCoverageBindingRole::Auxiliary => "auxiliary",
        ExploreStreamCoverageBindingRole::Context => "context",
        ExploreStreamCoverageBindingRole::Before => "before",
    }
}

const fn coverage_literal_kind_name(kind: ExploreStreamCoverageLiteralKind) -> &'static str {
    match kind {
        ExploreStreamCoverageLiteralKind::Integer => "integer",
        ExploreStreamCoverageLiteralKind::FloatBits => "float_bits",
        ExploreStreamCoverageLiteralKind::String => "string",
        ExploreStreamCoverageLiteralKind::Character => "character",
        ExploreStreamCoverageLiteralKind::Boolean => "boolean",
        ExploreStreamCoverageLiteralKind::Unit => "unit",
    }
}

const fn coverage_gap_reason_name(reason: ExploreStreamCoverageGapReason) -> &'static str {
    match reason {
        ExploreStreamCoverageGapReason::SchemaNotDeclaredRecord => "schema_not_declared_record",
        ExploreStreamCoverageGapReason::SchemaCompositionUnavailable => {
            "schema_composition_unavailable"
        }
        ExploreStreamCoverageGapReason::InterproceduralFieldProvenance => {
            "interprocedural_field_provenance"
        }
        ExploreStreamCoverageGapReason::ConstructorFieldMappingUnavailable => {
            "constructor_field_mapping_unavailable"
        }
        ExploreStreamCoverageGapReason::ConstructorChoiceProvenanceUnavailable => {
            "constructor_choice_provenance_unavailable"
        }
        ExploreStreamCoverageGapReason::UpstreamCoverageGap => "upstream_coverage_gap",
    }
}

fn validate_existing_manifest<A: RelationalPublicationAuthority>(
    path: &Path,
    plan: &RelationalPublicationPlan,
    authority: &A,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    if !path.exists() {
        return Ok(());
    }
    ensure_safe_artifact_target(path)?;
    let manifest: JsonValue = read_control_json(path, limits.max_control_bytes)?;
    let version = manifest
        .get("schema_version")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| RelationalPublicationError::InvalidManifest(path.to_path_buf()))?;
    let journal_id = manifest
        .pointer("/identity/journal_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| RelationalPublicationError::InvalidManifest(path.to_path_buf()))?;
    let query = manifest
        .get("query")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| RelationalPublicationError::InvalidManifest(path.to_path_buf()))?;
    let sequence = manifest
        .pointer("/journal/next_sequence")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| RelationalPublicationError::InvalidManifest(path.to_path_buf()))?;
    let head = manifest
        .pointer("/journal/head")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| RelationalPublicationError::InvalidManifest(path.to_path_buf()))?;
    if version != RELATIONAL_PUBLICATION_SCHEMA_VERSION as u64
        || journal_id != hex(plan.journal_id)
        || query != plan.query_name.as_ref()
    {
        return Err(RelationalPublicationError::ManifestIdentityMismatch);
    }
    authenticate_checkpoint(
        authority,
        RelationalPublicationCheckpoint::new(sequence, decode_hex_digest(head)?),
    )
}

fn write_cursor(
    path: &Path,
    cursor: &PublicationCursor,
    limits: RelationalPublicationLimits,
) -> Result<(), RelationalPublicationError> {
    atomic_write_json(path, cursor, false, limits.max_control_bytes)
}

fn digest_control_value<T: Serialize>(
    value: &T,
    limits: RelationalPublicationLimits,
) -> Result<[u8; 32], RelationalPublicationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| RelationalPublicationError::Json(error.to_string()))?;
    if bytes.len() > limits.max_control_bytes {
        return Err(RelationalPublicationError::ControlFileTooLarge {
            bytes: bytes.len(),
            limit: limits.max_control_bytes,
        });
    }
    Ok(Sha256::digest(bytes).into())
}

fn read_control_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: usize,
) -> Result<T, RelationalPublicationError> {
    ensure_safe_artifact_target(path)?;
    let metadata = fs::metadata(path).map_err(|error| io_error(path, error))?;
    let length = usize::try_from(metadata.len()).map_err(|_| {
        RelationalPublicationError::ControlFileTooLarge {
            bytes: usize::MAX,
            limit,
        }
    })?;
    if length > limit {
        return Err(RelationalPublicationError::ControlFileTooLarge {
            bytes: length,
            limit,
        });
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| RelationalPublicationError::AllocationFailed("control file"))?;
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| io_error(path, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| RelationalPublicationError::Json(error.to_string()))
}

fn atomic_write_json<T: Serialize>(
    path: &Path,
    value: &T,
    pretty: bool,
    limit: usize,
) -> Result<(), RelationalPublicationError> {
    let mut bytes = if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
    .map_err(|error| RelationalPublicationError::Json(error.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(RelationalPublicationError::ControlFileTooLarge {
            bytes: bytes.len(),
            limit,
        });
    }
    atomic_replace(path, &bytes)
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), RelationalPublicationError> {
    let parent = path
        .parent()
        .ok_or_else(|| RelationalPublicationError::UnsafeOutputPath(path.to_path_buf()))?;
    let mut temporary = None;
    for _ in 0..CONTROL_TEMP_ATTEMPTS {
        let nonce = CONTROL_TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".futuruna-publication-tmp-{}-{nonce}",
            std::process::id()
        ));
        match create_new_owner_only_file(&candidate) {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(&candidate, error)),
        }
    }
    let (temporary_path, mut file) = temporary
        .ok_or_else(|| RelationalPublicationError::AtomicTemporaryExhausted(path.to_path_buf()))?;
    let installed = (|| {
        file.write_all(bytes)
            .map_err(|error| io_error(&temporary_path, error))?;
        file.sync_all()
            .map_err(|error| io_error(&temporary_path, error))?;
        drop(file);
        fs::rename(&temporary_path, path).map_err(|error| io_error(path, error))?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error(parent, error))?;
        Ok(())
    })();
    if installed.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    installed
}

fn ensure_safe_artifact_target(path: &Path) -> Result<(), RelationalPublicationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            RelationalPublicationError::UnsafeOutputPath(path.to_path_buf()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
    }
}

fn safe_artifact_name(name: &str) -> Result<&str, RelationalPublicationError> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(RelationalPublicationError::UnsafeArtifactName(name.into()));
    }
    Ok(name)
}

fn path_to_manifest_string(path: &Path) -> Result<String, RelationalPublicationError> {
    let mut components = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(RelationalPublicationError::UnsafeOutputPath(
                path.to_path_buf(),
            ));
        };
        components.push(
            component
                .to_str()
                .ok_or_else(|| RelationalPublicationError::UnsafeOutputPath(path.into()))?,
        );
    }
    if components.is_empty() {
        return Err(RelationalPublicationError::UnsafeOutputPath(
            path.to_path_buf(),
        ));
    }
    Ok(components.join("/"))
}

fn publication_prefix_genesis(artifact_key: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESULT_PREFIX_ROOT_V9);
    hasher.update((artifact_key.len() as u64).to_be_bytes());
    hasher.update(artifact_key.as_bytes());
    hasher.finalize().into()
}

fn extend_publication_prefix(prior: [u8; 32], line_digest: [u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESULT_PREFIX_EXTEND_V9);
    hasher.update(prior);
    hasher.update(line_digest);
    hasher.finalize().into()
}

fn decode_hex_digest(value: &str) -> Result<[u8; 32], RelationalPublicationError> {
    if value.len() != 64 {
        return Err(RelationalPublicationError::InvalidDigest(value.into()));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (decode_hex_nibble(pair[0])? << 4) | decode_hex_nibble(pair[1])?;
    }
    Ok(bytes)
}

fn decode_hex_nibble(value: u8) -> Result<u8, RelationalPublicationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(RelationalPublicationError::InvalidDigest(
            "digest is not canonical lowercase hexadecimal".into(),
        )),
    }
}

fn hex(bytes: [u8; 32]) -> String {
    hex_slice(&bytes)
}

fn hex_slice(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn io_error(path: impl AsRef<Path>, error: std::io::Error) -> RelationalPublicationError {
    RelationalPublicationError::Io {
        path: path.as_ref().to_path_buf(),
        message: error.to_string(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalPublicationError {
    InvalidLimits,
    EmptyOutputDirectory,
    UnsafeOutputPath(PathBuf),
    UnsafeArtifactName(String),
    ArtifactPathCollision(PathBuf),
    UnownedNamespaceEntry(PathBuf),
    UntrackedExistingPublication(PathBuf),
    PlanIdentityMismatch,
    ReportIdentityMismatch,
    CurrentCheckpointMismatch,
    CursorIdentityMismatch,
    CursorArtifactSetMismatch,
    CursorArtifactMismatch(String),
    PendingCursorMismatch,
    UnsupportedCursorVersion {
        actual: u32,
        expected: u32,
    },
    PublicationFork {
        next_sequence: u64,
        head: String,
    },
    PublicationTruncated {
        path: PathBuf,
        expected_at_least: u64,
        actual: u64,
    },
    PublicationAhead {
        path: PathBuf,
        committed: u64,
        actual: u64,
    },
    PublicationSourceAhead {
        artifact: String,
        next_source_ordinal: u128,
        available: u128,
    },
    PublicationContradiction {
        path: PathBuf,
        source_coordinate: String,
    },
    RecoveryTailTooLarge {
        path: PathBuf,
        bytes: u64,
        limit: usize,
    },
    RecoverySkipLimit {
        artifact: String,
        limit: usize,
    },
    LastLineCursorMismatch(PathBuf),
    LastLineDigestMismatch(PathBuf),
    InvalidManifest(PathBuf),
    ManifestIdentityMismatch,
    MissingAnalysisLayer,
    AnalysisLayerKindMismatch,
    AnalysisCatalogStateMismatch,
    MechanismOrdinalIndexMismatch,
    MechanismSourceCoordinateAhead {
        artifact: String,
        event_ordinal: u128,
        event_end: u128,
    },
    MechanismDefinitionSourceCoordinateAhead {
        artifact: String,
        signature_ordinal: u128,
        signature_end: u128,
    },
    MechanismSourceCoordinateMismatch {
        artifact: String,
    },
    MechanismStarterSourceCoordinateAhead {
        artifact: String,
        mechanism_ordinal: u128,
        mechanism_end: u128,
    },
    MechanismStarterSourceCoordinateMismatch,
    ResultEvidenceRowMismatch,
    ResultSelectNotRowLocal {
        view_id: ViewId,
    },
    MechanismDefinitionChunkOutOfRange {
        signature_id: String,
        chunk_ordinal: u128,
    },
    StructuralDefinitionSourceCoordinateAhead {
        artifact: String,
        definition_ordinal: u128,
        definition_end: u128,
    },
    StructuralDefinitionSourceCoordinateMismatch {
        artifact: String,
    },
    StructuralDefinitionChunkOutOfRange {
        definition_id: String,
        part_ordinal: u128,
    },
    SelectShapeMismatch {
        names: usize,
        values: usize,
    },
    SourceOrdinalOverflow {
        artifact: String,
        ordinal: u128,
    },
    LineTooLarge {
        artifact: String,
        bytes: usize,
        limit: usize,
    },
    ControlFileTooLarge {
        bytes: usize,
        limit: usize,
    },
    AtomicTemporaryExhausted(PathBuf),
    AllocationFailed(&'static str),
    ArithmeticOverflow,
    InvalidDigest(String),
    Authority(String),
    Journal(String),
    Analysis(String),
    CaseSupport(String),
    CaseTransitions(String),
    Json(String),
    Io {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for RelationalPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("invalid relational publication limits"),
            Self::EmptyOutputDirectory => {
                formatter.write_str("relational publication output path must not be empty")
            }
            Self::UnsafeOutputPath(path) => write!(
                formatter,
                "relational publication refuses unsafe path `{}`",
                path.display()
            ),
            Self::UnsafeArtifactName(name) => write!(
                formatter,
                "analysis name `{name}` is not safe as a publication artifact name"
            ),
            Self::ArtifactPathCollision(path) => write!(
                formatter,
                "two analysis layers resolve to publication path `{}`",
                path.display()
            ),
            Self::UnownedNamespaceEntry(path) => write!(
                formatter,
                "publication namespace contains unowned entry `{}`",
                path.display()
            ),
            Self::UntrackedExistingPublication(path) => write!(
                formatter,
                "publication directory `{}` has result files but no matching cursor",
                path.display()
            ),
            Self::PlanIdentityMismatch => {
                formatter.write_str("publication plan does not match the checked journal contract")
            }
            Self::ReportIdentityMismatch => {
                formatter.write_str("public Explore report does not match the publication plan")
            }
            Self::CurrentCheckpointMismatch => formatter.write_str(
                "public report, durable checkpoint, and folded journal are not the same prefix",
            ),
            Self::CursorIdentityMismatch => {
                formatter.write_str("publication cursor belongs to another query or journal")
            }
            Self::CursorArtifactSetMismatch => formatter
                .write_str("publication cursor does not name the checked analysis artifact set"),
            Self::CursorArtifactMismatch(artifact) => write!(
                formatter,
                "publication cursor metadata disagrees for artifact `{artifact}`"
            ),
            Self::PendingCursorMismatch => formatter
                .write_str("pending publication batch does not begin at its committed cursor"),
            Self::UnsupportedCursorVersion { actual, expected } => write!(
                formatter,
                "unsupported publication cursor version {actual}; expected {expected}"
            ),
            Self::PublicationFork {
                next_sequence,
                head,
            } => write!(
                formatter,
                "publication checkpoint {next_sequence}@{head} is not an authenticated journal prefix"
            ),
            Self::PublicationTruncated {
                path,
                expected_at_least,
                actual,
            } => write!(
                formatter,
                "publication `{}` was truncated: cursor commits {expected_at_least} bytes, file has {actual}",
                path.display()
            ),
            Self::PublicationAhead {
                path,
                committed,
                actual,
            } => write!(
                formatter,
                "publication `{}` is ahead of its committed cursor ({actual} bytes versus {committed})",
                path.display()
            ),
            Self::PublicationSourceAhead {
                artifact,
                next_source_ordinal,
                available,
            } => write!(
                formatter,
                "publication source cursor for `{artifact}` is ahead ({next_source_ordinal} versus {available} available records)"
            ),
            Self::PublicationContradiction {
                path,
                source_coordinate,
            } => write!(
                formatter,
                "publication `{}` contradicts journal-derived source coordinate {source_coordinate}",
                path.display()
            ),
            Self::RecoveryTailTooLarge { path, bytes, limit } => write!(
                formatter,
                "publication recovery tail `{}` has {bytes} bytes; bounded limit is {limit}",
                path.display()
            ),
            Self::RecoverySkipLimit { artifact, limit } => write!(
                formatter,
                "publication recovery for `{artifact}` crossed more than {limit} non-public records before one line"
            ),
            Self::LastLineCursorMismatch(path) => write!(
                formatter,
                "publication last-line cursor is inconsistent for `{}`",
                path.display()
            ),
            Self::LastLineDigestMismatch(path) => write!(
                formatter,
                "publication last complete line does not match its cursor for `{}`",
                path.display()
            ),
            Self::InvalidManifest(path) => {
                write!(
                    formatter,
                    "invalid publication manifest `{}`",
                    path.display()
                )
            }
            Self::ManifestIdentityMismatch => formatter
                .write_str("existing publication manifest belongs to another query or journal"),
            Self::MissingAnalysisLayer => {
                formatter.write_str("journal analysis catalog omitted a declared layer")
            }
            Self::AnalysisLayerKindMismatch => {
                formatter.write_str("journal analysis layer identity has the wrong semantic kind")
            }
            Self::AnalysisCatalogStateMismatch => formatter
                .write_str("journal analysis owns neither exactly one open nor one closed catalog"),
            Self::MechanismOrdinalIndexMismatch => formatter.write_str(
                "derived mechanism definition index disagrees with journal-owned discovery data",
            ),
            Self::MechanismSourceCoordinateAhead {
                artifact,
                event_ordinal,
                event_end,
            } => write!(
                formatter,
                "mechanism publication source cursor for `{artifact}` is ahead at event {event_ordinal}; available event end is {event_end}"
            ),
            Self::MechanismDefinitionSourceCoordinateAhead {
                artifact,
                signature_ordinal,
                signature_end,
            } => write!(
                formatter,
                "mechanism-definition publication source cursor for `{artifact}` is ahead at signature {signature_ordinal}; available signature end is {signature_end}"
            ),
            Self::MechanismSourceCoordinateMismatch { artifact } => write!(
                formatter,
                "mechanism publication source coordinate is invalid for `{artifact}`"
            ),
            Self::MechanismStarterSourceCoordinateAhead {
                artifact,
                mechanism_ordinal,
                mechanism_end,
            } => write!(
                formatter,
                "mechanism-starter publication source cursor for `{artifact}` is ahead at mechanism {mechanism_ordinal}; available mechanism end is {mechanism_end}"
            ),
            Self::MechanismStarterSourceCoordinateMismatch => formatter.write_str(
                "mechanism-starter publication source coordinate is inconsistent with its authenticated projection checkpoint",
            ),
            Self::ResultEvidenceRowMismatch => formatter
                .write_str("selected discovery row does not match its result evidence identity"),
            Self::ResultSelectNotRowLocal { .. } => formatter.write_str(
                "early each-case publication encountered a SELECT value that is not row-local",
            ),
            Self::MechanismDefinitionChunkOutOfRange {
                signature_id,
                chunk_ordinal,
            } => write!(
                formatter,
                "mechanism signature {signature_id} has no definition chunk {chunk_ordinal}"
            ),
            Self::StructuralDefinitionSourceCoordinateAhead {
                artifact,
                definition_ordinal,
                definition_end,
            } => write!(
                formatter,
                "structural-definition publication source cursor for `{artifact}` is ahead at definition {definition_ordinal}; available definition end is {definition_end}"
            ),
            Self::StructuralDefinitionSourceCoordinateMismatch { artifact } => write!(
                formatter,
                "structural-definition publication source coordinate is invalid for `{artifact}`"
            ),
            Self::StructuralDefinitionChunkOutOfRange {
                definition_id,
                part_ordinal,
            } => write!(
                formatter,
                "structural definition {definition_id} has no part {part_ordinal}"
            ),
            Self::SelectShapeMismatch { names, values } => write!(
                formatter,
                "public SELECT shape has {names} names but {values} values"
            ),
            Self::SourceOrdinalOverflow { artifact, ordinal } => write!(
                formatter,
                "publication source ordinal {ordinal} is not addressable for `{artifact}`"
            ),
            Self::LineTooLarge {
                artifact,
                bytes,
                limit,
            } => write!(
                formatter,
                "one `{artifact}` NDJSON line needs {bytes} bytes; limit is {limit}"
            ),
            Self::ControlFileTooLarge { bytes, limit } => write!(
                formatter,
                "publication control file needs {bytes} bytes; limit is {limit}"
            ),
            Self::AtomicTemporaryExhausted(path) => write!(
                formatter,
                "could not allocate an atomic temporary beside `{}`",
                path.display()
            ),
            Self::AllocationFailed(component) => {
                write!(formatter, "allocation failed for publication {component}")
            }
            Self::ArithmeticOverflow => formatter.write_str("publication arithmetic overflow"),
            Self::InvalidDigest(value) => {
                write!(formatter, "invalid lowercase SHA-256 digest `{value}`")
            }
            Self::Authority(message)
            | Self::Journal(message)
            | Self::Analysis(message)
            | Self::CaseSupport(message)
            | Self::CaseTransitions(message)
            | Self::Json(message) => formatter.write_str(message),
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "publication I/O at `{}`: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for RelationalPublicationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    struct PermissionTestDirectory(PathBuf);

    #[cfg(unix)]
    impl PermissionTestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "futuruna-publication-permissions-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                CONTROL_TEMP_NONCE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).expect("create permission test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    #[cfg(unix)]
    impl Drop for PermissionTestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(unix)]
    fn permission_mode(path: &Path) -> u32 {
        fs::symlink_metadata(path)
            .unwrap_or_else(|error| panic!("read permissions for {}: {error}", path.display()))
            .permissions()
            .mode()
            & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn publication_namespace_is_owner_only_at_creation_and_resume() {
        let directory = PermissionTestDirectory::new();
        let output = directory.path();
        fs::set_permissions(output, fs::Permissions::from_mode(0o777))
            .expect("make legacy output directory permissive");

        for name in ["views", "mechanisms", "starters", "graphs"] {
            let path = output.join(name);
            fs::create_dir(&path).expect("create legacy publication subdirectory");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o777))
                .expect("make legacy publication subdirectory permissive");
        }

        let existing_view = output.join("views/cases.ndjson");
        let existing_graph = output.join("graphs/cases.ndjson");
        let existing_temporary = output.join(".futuruna-publication-tmp-abandoned");
        let existing_files = [
            output.join(CURSOR_FILE),
            output.join(MANIFEST_FILE),
            existing_view.clone(),
            existing_graph.clone(),
            existing_temporary.clone(),
        ];
        for path in &existing_files {
            fs::write(path, b"legacy\n").expect("write legacy publication file");
            fs::set_permissions(path, fs::Permissions::from_mode(0o666))
                .expect("make legacy publication file permissive");
        }

        prepare_owner_only_publication_namespace(
            output,
            [existing_view.as_path(), existing_graph.as_path()],
        )
        .expect("tighten publication namespace");

        assert_eq!(permission_mode(output), OWNER_ONLY_DIRECTORY_MODE);
        for name in ["views", "mechanisms", "starters", "graphs"] {
            assert_eq!(
                permission_mode(&output.join(name)),
                OWNER_ONLY_DIRECTORY_MODE
            );
        }
        for path in &existing_files {
            assert_eq!(permission_mode(path), OWNER_ONLY_FILE_MODE);
        }

        let new_artifact = output.join("mechanisms/new.ndjson");
        drop(open_owner_only_append_file(&new_artifact).expect("create private artifact"));
        assert_eq!(permission_mode(&new_artifact), OWNER_ONLY_FILE_MODE);

        let new_temporary = output.join(".futuruna-publication-tmp-new");
        drop(create_new_owner_only_file(&new_temporary).expect("create private temporary"));
        assert_eq!(permission_mode(&new_temporary), OWNER_ONLY_FILE_MODE);

        atomic_replace(&output.join(CURSOR_FILE), b"{}\n").expect("replace private cursor");
        assert_eq!(
            permission_mode(&output.join(CURSOR_FILE)),
            OWNER_ONLY_FILE_MODE
        );
    }

    #[test]
    fn explicit_consumers_and_case_transitions_are_additive_cursor_extensions() {
        let missing = additive_artifact_keys(
            ["view:core".to_string(), "subject-starters:old".to_string()],
            [
                ("view:core".to_string(), false),
                ("subject-starters:old".to_string(), true),
                ("subject-starters:new".to_string(), true),
                (CASE_TRANSITIONS_ARTIFACT_KEY.to_string(), true),
            ],
        )
        .expect("new publication consumers are appendable");

        assert_eq!(
            missing,
            BTreeSet::from([
                "subject-starters:new".to_string(),
                CASE_TRANSITIONS_ARTIFACT_KEY.to_string(),
            ])
        );
        assert!(matches!(
            additive_artifact_keys(
                ["view:core".to_string(), "subject-starters:old".to_string()],
                [
                    ("view:core".to_string(), false),
                    ("subject-starters:new".to_string(), true),
                ],
            ),
            Err(RelationalPublicationError::CursorArtifactSetMismatch)
        ));
        assert!(matches!(
            additive_artifact_keys(
                ["view:core".to_string()],
                [
                    ("view:core".to_string(), false),
                    ("mechanism:new-core".to_string(), false),
                ],
            ),
            Err(RelationalPublicationError::CursorArtifactSetMismatch)
        ));
    }

    #[test]
    fn subject_starter_cursor_identity_binds_consumer_request_target_and_subject() {
        let request_id = MechanismRequestId::from_journal_codec_bytes([0x21; 32]);
        let node_id = StructuralNodeId::from_checked_source_bytes([0x31; 32]);
        let base = SubjectStarterCursorIdentity::new(
            [0x11; 32],
            request_id,
            MechanismTargetId::Selected,
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id,
            },
        );

        assert_ne!(
            base,
            SubjectStarterCursorIdentity::new(
                [0x12; 32],
                request_id,
                MechanismTargetId::Selected,
                MechanismSupportSubject::Node {
                    facet: MechanismSupportFacet::Activation,
                    node_id,
                },
            )
        );
        assert_ne!(
            base,
            SubjectStarterCursorIdentity::new(
                [0x11; 32],
                MechanismRequestId::from_journal_codec_bytes([0x22; 32]),
                MechanismTargetId::Selected,
                MechanismSupportSubject::Node {
                    facet: MechanismSupportFacet::Activation,
                    node_id,
                },
            )
        );
        assert_ne!(
            base,
            SubjectStarterCursorIdentity::new(
                [0x11; 32],
                request_id,
                MechanismTargetId::ChosenView(ViewId::from_journal_codec_bytes([0x41; 32])),
                MechanismSupportSubject::Node {
                    facet: MechanismSupportFacet::Activation,
                    node_id,
                },
            )
        );
        assert_ne!(
            base,
            SubjectStarterCursorIdentity::new(
                [0x11; 32],
                request_id,
                MechanismTargetId::Selected,
                MechanismSupportSubject::Node {
                    facet: MechanismSupportFacet::DifferentialParticipation,
                    node_id,
                },
            )
        );
    }
}
